"""Data loader for preprocessed ChessBoard binary files.

The Rust preprocessor (bin/preprocess.rs) reads .binpack files and writes a flat
binary of 32-byte ChessBoard structs. This module memory-maps that file and
extracts feature indices for the NNUE trainer.

ChessBoard layout (32 bytes, little-endian, STM-relative):
  occ:     u64   — occupancy bitboard (STM-relative: if black to move, bytes are swapped)
  pcs:     [u8; 16] — packed pieces, 4 bits each (bit3=color, bits0-2=piece_type)
  score:   i16   — eval in centipawns (STM-relative)
  result:  u8    — game result: 0=loss, 1=draw, 2=win (STM-relative)
  ksq:     u8    — our king square
  opp_ksq: u8    — opponent king square (XOR 56 from raw)
  extra:   [u8; 3] — padding
"""

import struct
from pathlib import Path

import numpy as np
import torch
from torch.utils.data import IterableDataset, DataLoader

from config import (
    BATCH_SIZE,
    EVAL_SCALE,
    MAX_EVAL,
    WDL_BLEND,
)

ENTRY_SIZE = 32  # bytes per ChessBoard struct
MAX_PIECES = 32  # maximum pieces on the board

# King bucket: rank-based (0-7), with horizontal mirroring for files e-h
# Bucket table for queen-side squares (files a-d): [rank*4 + file] -> bucket
# Since mirroring maps files e-h to d-a, we only need 32 entries
BUCKETS = np.array([rank for rank in range(8) for _ in range(4)], dtype=np.int32)


def king_bucket(king_square: int) -> int:
    """Compute king bucket with horizontal mirroring."""
    file = king_square % 8
    if file > 3:
        king_square ^= 7  # mirror horizontally
    return king_square // 8  # rank = bucket


def needs_mirror(king_square: int) -> bool:
    """Check if king is on files e-h (needs horizontal mirror)."""
    return king_square % 8 > 3


def extract_features_batch(entries: np.ndarray) -> dict:
    """Extract NNUE features from a batch of ChessBoard entries.

    Vectorized: iterates 64 squares (not batch_size * ~20 pieces) using numpy.

    Args:
        entries: numpy array of raw bytes, shape (batch_size, 32)

    Returns:
        dict with stm_indices, stm_offsets, ntm_indices, ntm_offsets, targets
    """
    batch_size = len(entries)

    # Parse fields from the 32-byte structs
    occupancy = np.frombuffer(entries[:, :8].tobytes(), dtype=np.uint64).copy()
    packed_pieces = entries[:, 8:24]  # (batch_size, 16)
    scores = np.frombuffer(entries[:, 24:26].tobytes(), dtype=np.int16).copy()
    results = entries[:, 26].astype(np.float32) / 2.0
    our_king_squares = entries[:, 27].astype(np.int32)
    opp_king_squares = entries[:, 28].astype(np.int32)

    sigmoid_scores = 1.0 / (1.0 + np.exp(-scores.astype(np.float32) / EVAL_SCALE))
    targets = WDL_BLEND * results + (1.0 - WDL_BLEND) * sigmoid_scores

    # Per-position king bucket and mirror (vectorized)
    stm_needs_mirror = (our_king_squares % 8) > 3
    ntm_needs_mirror = (opp_king_squares % 8) > 3
    stm_mirror = np.where(stm_needs_mirror, np.int64(7), np.int64(0))
    ntm_mirror = np.where(ntm_needs_mirror, np.int64(7), np.int64(0))
    stm_ksq = np.where(stm_needs_mirror, our_king_squares ^ 7, our_king_squares).astype(np.int64)
    ntm_ksq = np.where(ntm_needs_mirror, opp_king_squares ^ 7, opp_king_squares).astype(np.int64)
    stm_bucket_offset = (stm_ksq // 8) * 768
    ntm_bucket_offset = (ntm_ksq // 8) * 768

    # Unpack all 32 piece slots per position: shape (batch_size, 32)
    # Piece slot j: low nibble of byte j//2 (j even) or high nibble (j odd)
    piece_slots = np.empty((batch_size, 32), dtype=np.int64)
    piece_slots[:, 0::2] = packed_pieces & 0x0F
    piece_slots[:, 1::2] = (packed_pieces >> 4) & 0x0F
    color_slots = (piece_slots >> 3) & 1  # 0 = STM piece, 1 = opponent
    type_slots = piece_slots & 7

    # Piece count per position = popcount(occupancy), used for offset computation
    occ_bytes = occupancy.view(np.uint8).reshape(batch_size, 8)
    piece_counts = np.unpackbits(occ_bytes, axis=1, bitorder="little").sum(axis=1).astype(np.int64)

    stm_offsets = np.zeros(batch_size, dtype=np.int64)
    ntm_offsets = np.zeros(batch_size, dtype=np.int64)
    stm_offsets[1:] = np.cumsum(piece_counts[:-1])
    ntm_offsets[1:] = np.cumsum(piece_counts[:-1])

    # Iterate over 64 squares (not batch_size * ~20 pieces).
    # For each square, process only the positions that have a piece there.
    # piece_rank[i] = how many pieces at squares < current sq in position i (= piece slot index).
    piece_rank = np.zeros(batch_size, dtype=np.int32)
    stm_chunks: list[np.ndarray] = []
    ntm_chunks: list[np.ndarray] = []
    pos_id_chunks: list[np.ndarray] = []

    for square in range(64):
        present = ((occupancy >> np.uint64(square)) & np.uint64(1)).astype(bool)
        if not np.any(present):
            continue

        pos_idx = np.where(present)[0]
        ranks = piece_rank[pos_idx]
        colors = color_slots[pos_idx, ranks]
        types = type_slots[pos_idx, ranks]

        stm_base = np.where(colors == 0, np.int64(0), np.int64(384)) + types * 64 + square
        ntm_base = np.where(colors == 0, np.int64(384), np.int64(0)) + types * 64 + (square ^ 56)

        stm_chunks.append(stm_bucket_offset[pos_idx] + (stm_base ^ stm_mirror[pos_idx]))
        ntm_chunks.append(ntm_bucket_offset[pos_idx] + (ntm_base ^ ntm_mirror[pos_idx]))
        pos_id_chunks.append(pos_idx)

        piece_rank[pos_idx] += 1

    # Concatenate features (currently grouped by square) and sort by position
    # so each position's features are contiguous, matching stm_offsets.
    total_features = int(piece_counts.sum())
    stm_indices = np.empty(total_features, dtype=np.int64)
    ntm_indices = np.empty(total_features, dtype=np.int64)

    if stm_chunks:
        stm_cat = np.concatenate(stm_chunks)
        ntm_cat = np.concatenate(ntm_chunks)
        pos_ids = np.concatenate(pos_id_chunks)
        order = np.argsort(pos_ids, kind="stable")
        stm_indices[:] = stm_cat[order]
        ntm_indices[:] = ntm_cat[order]

    return {
        "stm_indices": stm_indices,
        "stm_offsets": stm_offsets,
        "ntm_indices": ntm_indices,
        "ntm_offsets": ntm_offsets,
        "targets": targets.astype(np.float32),
    }


class PreprocessedDataset(IterableDataset):
    """Memory-mapped dataset over preprocessed ChessBoard binary files.

    Yields batches of feature indices and targets, pre-collated for the model.
    Uses memory mapping for efficient random access without loading everything into RAM.
    """

    def __init__(self, data_path: str, batch_size: int = BATCH_SIZE, shuffle: bool = True):
        super().__init__()
        self.data_path = Path(data_path)
        self.batch_size = batch_size
        self.shuffle = shuffle

        file_size = self.data_path.stat().st_size
        if file_size % ENTRY_SIZE != 0:
            raise ValueError(f"File size {file_size} is not a multiple of {ENTRY_SIZE}")
        self.num_positions = file_size // ENTRY_SIZE
        print(f"Dataset: {self.num_positions:,} positions ({file_size / 1e9:.2f} GB)")

    def __len__(self):
        return self.num_positions // self.batch_size

    def __iter__(self):
        # Memory-map the file
        data = np.memmap(self.data_path, dtype=np.uint8, mode="r").reshape(-1, ENTRY_SIZE)

        # Block-level shuffle: divide into blocks of 1M positions, shuffle block order,
        # then shuffle within each block. Avoids allocating a ~19GB index array.
        block_size = 1_000_000
        num_blocks = self.num_positions // block_size
        block_order = np.arange(num_blocks)
        if self.shuffle:
            np.random.shuffle(block_order)

        for block_index in block_order:
            block_start = block_index * block_size
            block_end = min(block_start + block_size, self.num_positions)
            within_block = np.arange(block_start, block_end)
            if self.shuffle:
                np.random.shuffle(within_block)

            for start in range(0, len(within_block) - self.batch_size + 1, self.batch_size):
                batch_indices = within_block[start : start + self.batch_size]
                batch_entries = data[batch_indices]

                features = extract_features_batch(batch_entries)

                yield (
                    torch.from_numpy(features["stm_indices"]),
                    torch.from_numpy(features["stm_offsets"]),
                    torch.from_numpy(features["ntm_indices"]),
                    torch.from_numpy(features["ntm_offsets"]),
                    torch.from_numpy(features["targets"]).unsqueeze(1),
                )


def create_dataloader(
    data_path: str,
    batch_size: int = BATCH_SIZE,
    shuffle: bool = True,
) -> PreprocessedDataset:
    """Create a dataset iterator for training.

    Note: We return the IterableDataset directly since it already yields
    pre-batched tensors. No need for PyTorch DataLoader wrapper.
    """
    return PreprocessedDataset(data_path, batch_size=batch_size, shuffle=shuffle)
