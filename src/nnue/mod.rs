pub mod defs;
pub mod features;
pub mod network;
pub mod simd;

use crate::misc::bits;
use crate::{defs::*, position::Position};

use self::{defs::*, features::feature_index, network::Network};

#[derive(Clone, Copy)]
struct AccEntry {
    white_acc: Accumulator,
    black_acc: Accumulator,
    white_king_sq: Square,
    black_king_sq: Square,
}

pub struct NnueEval {
    network: Network,
    /// Stack of accumulator entries. Top corresponds to the current position.
    stack: Vec<AccEntry>,
}

impl NnueEval {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        let network = Network::from_bytes(data)?;
        Some(Self {
            network,
            stack: Vec::with_capacity(128),
        })
    }

    /// Create an NnueEval with all-zero weights (for testing).
    #[cfg(test)]
    pub fn zero() -> Self {
        use self::network::Network;
        let network = Network {
            ft_weights: vec![Accumulator::zeroed(); BUCKET_FEATURE_SIZE]
                .into_boxed_slice()
                .try_into()
                .unwrap(),
            ft_biases: Accumulator::zeroed(),
            l1_weights: [[0i16; HIDDEN_SIZE * 2]; L1_SIZE],
            l1_biases: [0i16; L1_SIZE],
            l2_weights: [0i16; L1_SIZE],
            l2_bias: 0,
        };
        Self {
            network,
            stack: Vec::with_capacity(128),
        }
    }

    pub fn load(path: &str) -> Option<Self> {
        let data = std::fs::read(path).ok()?;
        Self::from_bytes(&data)
    }

    /// Full recompute of accumulators from the board state.
    /// Must be called after `position.set()`.
    pub fn refresh(&mut self, position: &Position) {
        let white_king_sq = bits::lsb(position.by_type_bb[Sides::WHITE][PieceType::KING]);
        let black_king_sq = bits::lsb(position.by_type_bb[Sides::BLACK][PieceType::KING]);

        let mut white_acc = self.network.ft_biases;
        let mut black_acc = self.network.ft_biases;

        for sq in RangeOf::SQUARES {
            let piece = position.board[sq];
            if piece == PieceType::NONE {
                continue;
            }

            let pt = type_of_piece(piece);
            let color = color_of_piece(piece);

            let white_feat = feature_index(Sides::WHITE, white_king_sq, color, pt, sq);
            let black_feat = feature_index(Sides::BLACK, black_king_sq, color, pt, sq);

            simd::add_i16_256(&mut white_acc, &self.network.ft_weights[white_feat]);
            simd::add_i16_256(&mut black_acc, &self.network.ft_weights[black_feat]);
        }

        self.stack.clear();
        self.stack.push(AccEntry {
            white_acc,
            black_acc,
            white_king_sq,
            black_king_sq,
        });
    }

    /// Recompute one perspective's accumulator from scratch.
    /// Used when a king move crosses a bucket boundary.
    pub fn refresh_perspective(&mut self, perspective: Side, position: &Position) {
        let entry = self.stack.last_mut().unwrap();
        let king_sq = if perspective == Sides::WHITE {
            entry.white_king_sq
        } else {
            entry.black_king_sq
        };
        let acc = if perspective == Sides::WHITE {
            &mut entry.white_acc
        } else {
            &mut entry.black_acc
        };

        *acc = self.network.ft_biases;

        for sq in RangeOf::SQUARES {
            let piece = position.board[sq];
            if piece == PieceType::NONE {
                continue;
            }
            let pt = type_of_piece(piece);
            let color = color_of_piece(piece);
            let feat = feature_index(perspective, king_sq, color, pt, sq);
            simd::add_i16_256(acc, &self.network.ft_weights[feat]);
        }
    }

    /// Clone top accumulator entry and push. Called before applying move deltas.
    pub fn push(&mut self) {
        let top = *self.stack.last().unwrap();
        self.stack.push(top);
    }

    /// Pop top accumulator entry. Called on undo.
    pub fn pop(&mut self) {
        self.stack.pop();
    }

    /// Update stored king square for one perspective.
    pub fn update_king_sq(&mut self, perspective: Side, king_sq: Square) {
        let entry = self.stack.last_mut().unwrap();
        if perspective == Sides::WHITE {
            entry.white_king_sq = king_sq;
        } else {
            entry.black_king_sq = king_sq;
        }
    }

    /// Add feature weights to both perspective accumulators on top of stack.
    pub fn activate(&mut self, color: Side, piece_type: Piece, square: Square) {
        let entry = self.stack.last_mut().unwrap();

        let white_feat = feature_index(Sides::WHITE, entry.white_king_sq, color, piece_type, square);
        let black_feat = feature_index(Sides::BLACK, entry.black_king_sq, color, piece_type, square);

        simd::add_i16_256(&mut entry.white_acc, &self.network.ft_weights[white_feat]);
        simd::add_i16_256(&mut entry.black_acc, &self.network.ft_weights[black_feat]);
    }

    /// Subtract feature weights from both perspective accumulators on top of stack.
    pub fn deactivate(&mut self, color: Side, piece_type: Piece, square: Square) {
        let entry = self.stack.last_mut().unwrap();

        let white_feat = feature_index(Sides::WHITE, entry.white_king_sq, color, piece_type, square);
        let black_feat = feature_index(Sides::BLACK, entry.black_king_sq, color, piece_type, square);

        simd::sub_i16_256(&mut entry.white_acc, &self.network.ft_weights[white_feat]);
        simd::sub_i16_256(&mut entry.black_acc, &self.network.ft_weights[black_feat]);
    }

    /// Add feature weights to one perspective's accumulator only.
    pub fn activate_single(&mut self, perspective: Side, color: Side, piece_type: Piece, square: Square) {
        let entry = self.stack.last_mut().unwrap();
        let king_sq = if perspective == Sides::WHITE {
            entry.white_king_sq
        } else {
            entry.black_king_sq
        };
        let feat = feature_index(perspective, king_sq, color, piece_type, square);
        let acc = if perspective == Sides::WHITE {
            &mut entry.white_acc
        } else {
            &mut entry.black_acc
        };
        simd::add_i16_256(acc, &self.network.ft_weights[feat]);
    }

    /// Subtract feature weights from one perspective's accumulator only.
    pub fn deactivate_single(&mut self, perspective: Side, color: Side, piece_type: Piece, square: Square) {
        let entry = self.stack.last_mut().unwrap();
        let king_sq = if perspective == Sides::WHITE {
            entry.white_king_sq
        } else {
            entry.black_king_sq
        };
        let feat = feature_index(perspective, king_sq, color, piece_type, square);
        let acc = if perspective == Sides::WHITE {
            &mut entry.white_acc
        } else {
            &mut entry.black_acc
        };
        simd::sub_i16_256(acc, &self.network.ft_weights[feat]);
    }

    /// Evaluate the position from the side-to-move's perspective.
    /// Reads from the top of the accumulator stack.
    #[inline(always)]
    pub fn evaluate(&self, side_to_move: Side) -> i16 {
        let entry = self.stack.last().unwrap();

        let (our_acc, their_acc) = if side_to_move == Sides::WHITE {
            (&entry.white_acc, &entry.black_acc)
        } else {
            (&entry.black_acc, &entry.white_acc)
        };

        self.network.forward(our_acc, their_acc)
    }

    /// Debug: verify incremental accumulator matches a full refresh.
    #[cfg(debug_assertions)]
    pub fn verify(&self, position: &Position) {
        let white_king_sq = bits::lsb(position.by_type_bb[Sides::WHITE][PieceType::KING]);
        let black_king_sq = bits::lsb(position.by_type_bb[Sides::BLACK][PieceType::KING]);

        let mut white_acc = self.network.ft_biases;
        let mut black_acc = self.network.ft_biases;

        for sq in RangeOf::SQUARES {
            let piece = position.board[sq];
            if piece == PieceType::NONE {
                continue;
            }
            let pt = type_of_piece(piece);
            let color = color_of_piece(piece);
            let wf = feature_index(Sides::WHITE, white_king_sq, color, pt, sq);
            let bf = feature_index(Sides::BLACK, black_king_sq, color, pt, sq);
            simd::add_i16_256(&mut white_acc, &self.network.ft_weights[wf]);
            simd::add_i16_256(&mut black_acc, &self.network.ft_weights[bf]);
        }

        let entry = self.stack.last().unwrap();
        assert_eq!(&white_acc, &entry.white_acc, "NNUE white accumulator mismatch!");
        assert_eq!(&black_acc, &entry.black_acc, "NNUE black accumulator mismatch!");
        assert_eq!(white_king_sq, entry.white_king_sq, "NNUE white king sq mismatch!");
        assert_eq!(black_king_sq, entry.black_king_sq, "NNUE black king sq mismatch!");
    }
}
