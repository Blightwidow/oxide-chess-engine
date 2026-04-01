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

    Args:
        entries: numpy array of raw bytes, shape (batch_size, 32)

    Returns:
        dict with stm_indices, stm_offsets, ntm_indices, ntm_offsets, targets
    """
    batch_size = len(entries)

    # Parse fields from the 32-byte structs
    occupancy = np.frombuffer(entries[:, :8].tobytes(), dtype=np.uint64).copy()
    packed_pieces = entries[:, 8:24]  # [batch, 16] bytes
    scores = np.frombuffer(entries[:, 24:26].tobytes(), dtype=np.int16).copy()
    results = entries[:, 26].astype(np.float32) / 2.0  # 0.0, 0.5, 1.0
    our_king_squares = entries[:, 27].astype(np.int32)
    opp_king_squares = entries[:, 28].astype(np.int32)

    # Compute targets: blend of WDL result and sigmoid(score)
    sigmoid_scores = 1.0 / (1.0 + np.exp(-scores.astype(np.float32) / EVAL_SCALE))
    targets = WDL_BLEND * results + (1.0 - WDL_BLEND) * sigmoid_scores

    # Pre-allocate feature index arrays (max 32 pieces per position, 2 perspectives)
    all_stm_indices = []
    all_ntm_indices = []
    stm_offsets = np.zeros(batch_size, dtype=np.int64)
    ntm_offsets = np.zeros(batch_size, dtype=np.int64)

    current_stm_offset = 0
    current_ntm_offset = 0

    for position_index in range(batch_size):
        occupancy_bits = int(occupancy[position_index])
        our_king_sq = int(our_king_squares[position_index])
        opp_king_sq = int(opp_king_squares[position_index])

        # Compute bucket and mirror for each perspective
        stm_mirror = 7 if needs_mirror(our_king_sq) else 0
        ntm_mirror = 7 if needs_mirror(opp_king_sq) else 0
        stm_bucket_offset = 768 * king_bucket(our_king_sq)
        ntm_bucket_offset = 768 * king_bucket(opp_king_sq)

        stm_offsets[position_index] = current_stm_offset
        ntm_offsets[position_index] = current_ntm_offset

        piece_bytes = packed_pieces[position_index]
        piece_index = 0
        occ = occupancy_bits

        while occ:
            square = (occ & -occ).bit_length() - 1  # trailing zeros
            occ &= occ - 1  # clear lowest set bit

            # Unpack piece: 4 bits per piece, 2 pieces per byte
            byte_index = piece_index // 2
            nibble = piece_index & 1
            piece = (int(piece_bytes[byte_index]) >> (4 * nibble)) & 0xF

            color = (piece >> 3) & 1  # 0 = our piece, 1 = opponent piece
            piece_type = piece & 7     # 0=pawn, 1=knight, ..., 5=king

            # Chess768 base features (STM-relative, matching bullet's Chess768)
            stm_base = [0, 384][color] + 64 * piece_type + square
            ntm_base = [384, 0][color] + 64 * piece_type + (square ^ 56)

            # Apply horizontal mirroring and bucket offset
            stm_feature = stm_bucket_offset + (stm_base ^ stm_mirror)
            ntm_feature = ntm_bucket_offset + (ntm_base ^ ntm_mirror)

            all_stm_indices.append(stm_feature)
            all_ntm_indices.append(ntm_feature)

            piece_index += 1
            current_stm_offset += 1
            current_ntm_offset += 1

    return {
        "stm_indices": np.array(all_stm_indices, dtype=np.int64),
        "stm_offsets": stm_offsets,
        "ntm_indices": np.array(all_ntm_indices, dtype=np.int64),
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

        # Create index array for shuffling
        indices = np.arange(self.num_positions)
        if self.shuffle:
            np.random.shuffle(indices)

        # Yield batches
        for start in range(0, self.num_positions - self.batch_size + 1, self.batch_size):
            batch_indices = indices[start : start + self.batch_size]
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
