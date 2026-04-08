"""ctypes bindings for the compiled binpack_loader shared library.

The shared library must be compiled first:
    cd training/dataloader && ./compile.sh

Usage:
    from dataloader._native import create_loader, destroy_loader, next_batch, free_batch
"""

import ctypes
import platform
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Load shared library
# ---------------------------------------------------------------------------

_dataloader_dir = Path(__file__).parent
_build_dir = _dataloader_dir / "build"

if platform.system() == "Darwin":
    _lib_names = ["libbinpack_loader.dylib"]
elif platform.system() == "Windows":
    _lib_names = ["binpack_loader.dll"]
else:
    _lib_names = ["libbinpack_loader.so"]

_lib_path = None
for _name in _lib_names:
    _candidate = _build_dir / _name
    if _candidate.exists():
        _lib_path = _candidate
        break

if _lib_path is None:
    raise RuntimeError(
        f"binpack_loader shared library not found in {_build_dir}.\n"
        f"Run: cd {_dataloader_dir} && ./compile.sh"
    )

_lib = ctypes.CDLL(str(_lib_path))

# ---------------------------------------------------------------------------
# SparseBatch struct (mirrors binpack_loader.cpp)
# ---------------------------------------------------------------------------

class SparseBatch(ctypes.Structure):
    _fields_ = [
        ("stm_indices",    ctypes.POINTER(ctypes.c_int64)),
        ("ntm_indices",    ctypes.POINTER(ctypes.c_int64)),
        ("stm_offsets",    ctypes.POINTER(ctypes.c_int64)),
        ("ntm_offsets",    ctypes.POINTER(ctypes.c_int64)),
        ("targets",        ctypes.POINTER(ctypes.c_float)),
        ("batch_size",     ctypes.c_int),
        ("total_features", ctypes.c_int),
    ]

# ---------------------------------------------------------------------------
# Function signatures
# ---------------------------------------------------------------------------

_lib.binpack_loader_create.argtypes = [
    ctypes.c_char_p,  # data_dir
    ctypes.c_int,     # batch_size
    ctypes.c_float,   # wdl_blend
    ctypes.c_float,   # eval_scale
]
_lib.binpack_loader_create.restype = ctypes.c_void_p

_lib.binpack_loader_destroy.argtypes = [ctypes.c_void_p]
_lib.binpack_loader_destroy.restype = None

_lib.binpack_loader_next_batch.argtypes = [ctypes.c_void_p]
_lib.binpack_loader_next_batch.restype = ctypes.POINTER(SparseBatch)

_lib.binpack_batch_free.argtypes = [ctypes.POINTER(SparseBatch)]
_lib.binpack_batch_free.restype = None

# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

def create_loader(
    data_dir: str,
    batch_size: int,
    wdl_blend: float,
    eval_scale: float,
) -> ctypes.c_void_p:
    """Create a BinpackLoader. Returns an opaque pointer."""
    handle = _lib.binpack_loader_create(
        data_dir.encode(),
        batch_size,
        wdl_blend,
        eval_scale,
    )
    if not handle:
        raise RuntimeError(f"Failed to create BinpackLoader for directory: {data_dir}")
    return handle


def destroy_loader(handle: ctypes.c_void_p) -> None:
    """Destroy a BinpackLoader created by create_loader."""
    _lib.binpack_loader_destroy(handle)


def next_batch(handle: ctypes.c_void_p) -> ctypes.POINTER(SparseBatch):
    """Return a pointer to the next SparseBatch. Caller must call free_batch."""
    return _lib.binpack_loader_next_batch(handle)


def free_batch(batch_ptr: ctypes.POINTER(SparseBatch)) -> None:
    """Free a SparseBatch returned by next_batch."""
    _lib.binpack_batch_free(batch_ptr)
