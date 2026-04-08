"""Data loader that reads .binpack files directly via a C++ extension.

Replaces the old two-step pipeline (Rust preprocess → memory-mapped binary).
The C++ loader (training/dataloader/) handles filtering, feature extraction,
and shuffle buffering. This module wraps it as a PyTorch IterableDataset.

The yielded tuples are identical to the old PreprocessedDataset:
    (stm_indices, stm_offsets, ntm_indices, ntm_offsets, targets)
"""

import sys
from pathlib import Path

import numpy as np
import torch
from torch.utils.data import IterableDataset

# Locate and import the native bindings (dataloader/ lives next to this file)
_training_dir = Path(__file__).parent
sys.path.insert(0, str(_training_dir))
from dataloader import _native  # noqa: E402

from config import (
    BATCH_SIZE,
    EVAL_SCALE,
    WDL_BLEND,
)


def _ptr_to_numpy(ptr, dtype, count: int) -> np.ndarray:
    """Convert a ctypes pointer to a numpy array (copies from C memory)."""
    return np.ctypeslib.as_array(ptr, shape=(count,)).copy()


class BinpackDataset(IterableDataset):
    """Streaming dataset over a directory of .binpack files.

    Feature extraction and shuffle buffering are handled in C++. Each call to
    __iter__ creates a fresh C++ loader, which re-shuffles file order and
    the 1M-position internal shuffle buffer.
    """

    def __init__(
        self,
        data_dir: str,
        batch_size: int = BATCH_SIZE,
        wdl_blend: float = WDL_BLEND,
        eval_scale: float = EVAL_SCALE,
    ):
        super().__init__()
        self.data_dir = str(Path(data_dir).resolve())
        self.batch_size = batch_size
        self.wdl_blend = wdl_blend
        self.eval_scale = eval_scale

        binpacks = list(Path(data_dir).glob("*.binpack"))
        if not binpacks:
            raise ValueError(f"No .binpack files found in: {data_dir}")
        print(f"Dataset: {len(binpacks)} binpack files in {data_dir}")

    def __iter__(self):
        handle = _native.create_loader(
            self.data_dir,
            self.batch_size,
            self.wdl_blend,
            self.eval_scale,
        )
        try:
            while True:
                batch_ptr = _native.next_batch(handle)
                if not batch_ptr:
                    break

                batch = batch_ptr.contents
                n = batch.batch_size
                f = batch.total_features

                stm_indices = torch.from_numpy(_ptr_to_numpy(batch.stm_indices, np.int64, f))
                stm_offsets = torch.from_numpy(_ptr_to_numpy(batch.stm_offsets, np.int64, n))
                ntm_indices = torch.from_numpy(_ptr_to_numpy(batch.ntm_indices, np.int64, f))
                ntm_offsets = torch.from_numpy(_ptr_to_numpy(batch.ntm_offsets, np.int64, n))
                targets = torch.from_numpy(
                    _ptr_to_numpy(batch.targets, np.float32, n)
                ).unsqueeze(1)

                _native.free_batch(batch_ptr)

                yield stm_indices, stm_offsets, ntm_indices, ntm_offsets, targets
        finally:
            _native.destroy_loader(handle)


def create_dataloader(
    data_dir: str,
    batch_size: int = BATCH_SIZE,
    shuffle: bool = True,  # kept for API compat; shuffling is always done in C++
) -> BinpackDataset:
    """Create a BinpackDataset iterator for training or validation."""
    return BinpackDataset(data_dir, batch_size=batch_size)
