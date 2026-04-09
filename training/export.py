"""Export trained PyTorch NNUE weights to OXNN v3 binary format.

OXNN v3 layout:
  Header (24 bytes):
    "OXNN"          4 bytes (magic)
    version=3       u32 LE
    num_buckets=8   u32 LE
    feature_size=768 u32 LE
    hidden_size=384 u32 LE
    l1_size=32      u32 LE
  Weights (all i16 LE):
    L0 weights: [BUCKET_FEATURE_SIZE][HIDDEN_SIZE] row-major
    L0 biases:  [HIDDEN_SIZE]
    L1 weights: [HIDDEN_SIZE*2][L1_SIZE] row-major (engine transposes at load)
    L1 biases:  [L1_SIZE]
    L2 weights: [L1_SIZE]
    L2 bias:    1
"""

import argparse
import struct
from pathlib import Path

import numpy as np
import torch

from config import (
    BUCKET_FEATURE_SIZE,
    FEATURE_SIZE,
    HIDDEN_SIZE,
    L1_SIZE,
    NUM_BUCKETS,
    QA,
    QB,
)
from model import OxideNNUE


def quantize_and_clamp(tensor: torch.Tensor, scale: float) -> np.ndarray:
    """Quantize float tensor to i16 with clamping."""
    scaled = (tensor.detach().cpu().float() * scale).round()
    clamped = scaled.clamp(-32768, 32767)
    return clamped.numpy().astype(np.int16)


def export_oxnn(checkpoint_path: str, output_path: str):
    """Export a PyTorch checkpoint to OXNN v3 format."""
    model = OxideNNUE()
    state = torch.load(checkpoint_path, map_location="cpu", weights_only=True)
    model.load_state_dict(state)
    model.eval()

    # Extract weights
    # Feature transformer: EmbeddingBag weight is [BUCKET_FEATURE_SIZE, HIDDEN_SIZE]
    feature_transformer_weight = model.feature_transformer.weight.weight  # nn.EmbeddingBag.weight
    feature_transformer_bias = model.feature_transformer.bias

    l1_weight = model.l1.weight  # [L1_SIZE, HIDDEN_SIZE*2]
    l1_bias = model.l1.bias      # [L1_SIZE]
    l2_weight = model.l2.weight  # [1, L1_SIZE]
    l2_bias = model.l2.bias      # [1]

    # Quantize
    # L0 weights/biases at QA scale
    quantized_feature_transformer_weight = quantize_and_clamp(feature_transformer_weight, QA)  # [6144, 384]
    quantized_feature_transformer_bias = quantize_and_clamp(feature_transformer_bias, QA)      # [384]

    # L1 weights at QB scale, biases at QA*QB scale
    quantized_l1_weight = quantize_and_clamp(l1_weight, QB)           # [32, 768]
    quantized_l1_bias = quantize_and_clamp(l1_bias, QA * QB)          # [32]

    # L2 weights at QB scale, bias at QA*QB scale
    quantized_l2_weight = quantize_and_clamp(l2_weight, QB)           # [1, 32]
    quantized_l2_bias = quantize_and_clamp(l2_bias, QA * QB)          # [1]

    # Write OXNN v3 file
    with open(output_path, "wb") as output_file:
        # Header (24 bytes)
        output_file.write(b"OXNN")
        output_file.write(struct.pack("<I", 3))            # version
        output_file.write(struct.pack("<I", NUM_BUCKETS))  # num_buckets
        output_file.write(struct.pack("<I", FEATURE_SIZE)) # feature_size
        output_file.write(struct.pack("<I", HIDDEN_SIZE))  # hidden_size
        output_file.write(struct.pack("<I", L1_SIZE))      # l1_size

        # L0 weights: [BUCKET_FEATURE_SIZE][HIDDEN_SIZE] row-major
        output_file.write(quantized_feature_transformer_weight.tobytes())

        # L0 biases: [HIDDEN_SIZE]
        output_file.write(quantized_feature_transformer_bias.tobytes())

        # L1 weights: [HIDDEN_SIZE*2][L1_SIZE] — file format is NOT transposed
        # PyTorch Linear stores weights as [out_features, in_features] = [L1_SIZE, HIDDEN_SIZE*2]
        # We need [HIDDEN_SIZE*2, L1_SIZE] for the file format, so transpose
        quantized_l1_weight_file = quantized_l1_weight.T  # [768, 32]
        assert quantized_l1_weight_file.shape == (HIDDEN_SIZE * 2, L1_SIZE)
        output_file.write(np.ascontiguousarray(quantized_l1_weight_file).tobytes())

        # L1 biases: [L1_SIZE]
        output_file.write(quantized_l1_bias.tobytes())

        # L2 weights: [L1_SIZE] — squeeze from [1, L1_SIZE]
        output_file.write(quantized_l2_weight.reshape(L1_SIZE).tobytes())

        # L2 bias: 1 i16
        output_file.write(quantized_l2_bias.reshape(1).tobytes())

    file_size = Path(output_path).stat().st_size
    expected_size = 24 + (BUCKET_FEATURE_SIZE * HIDDEN_SIZE + HIDDEN_SIZE + HIDDEN_SIZE * 2 * L1_SIZE + L1_SIZE + L1_SIZE + 1) * 2
    assert file_size == expected_size, f"File size mismatch: {file_size} != {expected_size}"
    print(f"Exported OXNN v3: {output_path} ({file_size:,} bytes)")


def main():
    parser = argparse.ArgumentParser(description="Export NNUE weights to OXNN v3 format")
    parser.add_argument("checkpoint", help="Path to model.pt checkpoint file")
    parser.add_argument("output", help="Output .nnue file path")
    arguments = parser.parse_args()

    export_oxnn(arguments.checkpoint, arguments.output)


if __name__ == "__main__":
    main()
