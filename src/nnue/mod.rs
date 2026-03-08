pub mod defs;
pub mod features;
pub mod network;

use crate::{
    defs::*,
    position::Position,
};

use self::{defs::*, features::feature_index, network::Network};

pub struct NnueEval {
    network: Network,
}

impl NnueEval {
    pub fn new(path: &str) -> Option<Self> {
        let network = Network::load(path)?;
        println!("info string NNUE evaluation using {}", path);
        Some(Self { network })
    }

    /// Create an NNUE evaluator with zero weights (returns 0 for all positions).
    /// Used for tests that need a Search but don't depend on evaluation accuracy.
    #[cfg(test)]
    pub fn zeroed() -> Self {
        Self {
            network: Network::zeroed(),
        }
    }

    /// Evaluate the position from the side-to-move's perspective.
    ///
    /// Computes the accumulator from scratch by iterating over all pieces.
    /// This is the "full refresh" approach — incremental updates can be added later.
    pub fn evaluate(&self, position: &Position) -> i16 {
        let us = position.side_to_move;
        let them = us ^ 1;

        // Initialize accumulators with biases
        let mut our_acc = self.network.ft_biases;
        let mut their_acc = self.network.ft_biases;

        // Accumulate features for all pieces on the board
        for sq in RangeOf::SQUARES {
            let piece = position.board[sq];
            if piece == PieceType::NONE {
                continue;
            }

            let pt = type_of_piece(piece);
            let color = color_of_piece(piece);

            let our_feat = feature_index(us, color, pt, sq);
            let their_feat = feature_index(them, color, pt, sq);

            for i in 0..HIDDEN_SIZE {
                our_acc[i] += self.network.ft_weights[our_feat][i];
                their_acc[i] += self.network.ft_weights[their_feat][i];
            }
        }

        self.network.forward(&our_acc, &their_acc)
    }
}
