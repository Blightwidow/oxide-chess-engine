"""Oxide NNUE network definition.

Architecture: 768*8 -> 256 (SCReLU) -> concat perspectives (512) -> 32 (SCReLU) -> 1
Matches the Rust engine's inference in src/nnue/network.rs.
"""

import torch
import torch.nn as nn

from config import BUCKET_FEATURE_SIZE, HIDDEN_SIZE, L1_SIZE


def screlu(x: torch.Tensor) -> torch.Tensor:
    """Squared Clipped ReLU: clamp to [0, 1], then square."""
    return torch.clamp(x, 0.0, 1.0).square()


class FeatureTransformer(nn.Module):
    """Sparse feature transformer using EmbeddingBag.

    Equivalent to a linear layer where only ~30 of 6144 inputs are active (1.0).
    EmbeddingBag(mode='sum') sums the embedding vectors for active indices,
    which is identical to a sparse matrix-vector multiply.
    """

    def __init__(self):
        super().__init__()
        self.weight = nn.EmbeddingBag(BUCKET_FEATURE_SIZE, HIDDEN_SIZE, mode="sum", sparse=False)
        self.bias = nn.Parameter(torch.zeros(HIDDEN_SIZE))

    def forward(self, feature_indices: torch.Tensor, offsets: torch.Tensor) -> torch.Tensor:
        """Forward pass.

        Args:
            feature_indices: 1D tensor of active feature indices for the entire batch,
                             concatenated (e.g. [f0_0, f0_1, ..., f1_0, f1_1, ...]).
            offsets: 1D tensor of length batch_size, where offsets[i] is the start
                     index in feature_indices for sample i.
        """
        return self.weight(feature_indices, offsets) + self.bias


class OxideNNUE(nn.Module):
    """Oxide NNUE evaluation network.

    Dual-perspective architecture with shared feature transformer.
    """

    def __init__(self):
        super().__init__()
        self.feature_transformer = FeatureTransformer()
        self.l1 = nn.Linear(HIDDEN_SIZE * 2, L1_SIZE)
        self.l2 = nn.Linear(L1_SIZE, 1)

    def forward(
        self,
        stm_indices: torch.Tensor,
        stm_offsets: torch.Tensor,
        ntm_indices: torch.Tensor,
        ntm_offsets: torch.Tensor,
    ) -> torch.Tensor:
        """Forward pass for a batch of positions.

        Args:
            stm_indices: Active feature indices for side-to-move perspective.
            stm_offsets: Batch offsets for stm_indices.
            ntm_indices: Active feature indices for not-side-to-move perspective.
            ntm_offsets: Batch offsets for ntm_indices.

        Returns:
            Raw output (pre-sigmoid) of shape (batch_size, 1).
        """
        stm_out = screlu(self.feature_transformer(stm_indices, stm_offsets))
        ntm_out = screlu(self.feature_transformer(ntm_indices, ntm_offsets))
        hidden = torch.cat([stm_out, ntm_out], dim=1)
        return self.l2(screlu(self.l1(hidden)))
