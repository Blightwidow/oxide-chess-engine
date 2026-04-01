"""Configuration for Oxide NNUE PyTorch trainer."""

import torch

# --- Network architecture (must match engine's src/nnue/defs.rs) ---
FEATURE_SIZE = 768       # 2 colors * 6 piece types * 64 squares
NUM_BUCKETS = 8          # One per rank, with horizontal mirroring
BUCKET_FEATURE_SIZE = NUM_BUCKETS * FEATURE_SIZE  # 6144
HIDDEN_SIZE = 256        # Feature transformer output
L1_SIZE = 32             # Second hidden layer

# --- Quantization (must match engine inference) ---
QA = 255                 # Feature transformer clamp/scale
QB = 64                  # L1/L2 weight scale
SCALE = 400              # Centipawn output scale

# --- Training hyperparameters ---
BATCH_SIZE = 16384
BATCHES_PER_SUPERBATCH = 6104    # ~100M positions per superbatch
END_SUPERBATCH = 60
SAVE_RATE = 10                   # Checkpoint every N superbatches
LEARNING_RATE = 0.001
LR_GAMMA = 0.3                  # StepLR decay factor
LR_STEP = 15                    # Decay every N superbatches
WDL_BLEND = 0.75                # Blend between WDL result and eval target
EVAL_SCALE = 400.0              # Sigmoid scaling for eval targets

# --- Data filtering ---
MIN_PLY = 16
MAX_EVAL = 10000

# --- Paths ---
CHECKPOINT_DIR = "checkpoints"
DATA_DIR = "data"
VALIDATION_DIR = "data/validation"

# --- Device selection ---
def get_device() -> torch.device:
    """Select best available device: MPS > CUDA > CPU."""
    if torch.backends.mps.is_available():
        return torch.device("mps")
    if torch.cuda.is_available():
        return torch.device("cuda")
    return torch.device("cpu")
