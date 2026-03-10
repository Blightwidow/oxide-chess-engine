pub mod defs;
pub mod features;
pub mod network;

use crate::{defs::*, position::Position};

use self::{defs::*, features::feature_index, network::Network};

pub struct NnueEval {
    network: Network,
    /// Stack of (white_accumulator, black_accumulator) pairs.
    /// Top of stack corresponds to the current position.
    stack: Vec<(Accumulator, Accumulator)>,
}

impl NnueEval {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        let network = Network::from_bytes(data)?;
        Some(Self {
            network,
            stack: Vec::with_capacity(128),
        })
    }

    pub fn load(path: &str) -> Option<Self> {
        let data = std::fs::read(path).ok()?;
        Self::from_bytes(&data)
    }

    /// Full recompute of accumulators from the board state.
    /// Must be called after `position.set()`.
    pub fn refresh(&mut self, position: &Position) {
        let mut white_acc = self.network.ft_biases;
        let mut black_acc = self.network.ft_biases;

        for sq in RangeOf::SQUARES {
            let piece = position.board[sq];
            if piece == PieceType::NONE {
                continue;
            }

            let pt = type_of_piece(piece);
            let color = color_of_piece(piece);

            let white_feat = feature_index(Sides::WHITE, color, pt, sq);
            let black_feat = feature_index(Sides::BLACK, color, pt, sq);

            for i in 0..HIDDEN_SIZE {
                white_acc[i] += self.network.ft_weights[white_feat][i];
                black_acc[i] += self.network.ft_weights[black_feat][i];
            }
        }

        self.stack.clear();
        self.stack.push((white_acc, black_acc));
    }

    /// Clone top accumulator pair and push. Called before applying move deltas.
    pub fn push(&mut self) {
        let top = *self.stack.last().unwrap();
        self.stack.push(top);
    }

    /// Pop top accumulator pair. Called on undo.
    pub fn pop(&mut self) {
        self.stack.pop();
    }

    /// Add feature weights to both perspective accumulators on top of stack.
    pub fn activate(&mut self, color: Side, piece_type: Piece, square: Square) {
        let (white_acc, black_acc) = self.stack.last_mut().unwrap();

        let white_feat = feature_index(Sides::WHITE, color, piece_type, square);
        let black_feat = feature_index(Sides::BLACK, color, piece_type, square);

        for i in 0..HIDDEN_SIZE {
            white_acc[i] += self.network.ft_weights[white_feat][i];
            black_acc[i] += self.network.ft_weights[black_feat][i];
        }
    }

    /// Subtract feature weights from both perspective accumulators on top of stack.
    pub fn deactivate(&mut self, color: Side, piece_type: Piece, square: Square) {
        let (white_acc, black_acc) = self.stack.last_mut().unwrap();

        let white_feat = feature_index(Sides::WHITE, color, piece_type, square);
        let black_feat = feature_index(Sides::BLACK, color, piece_type, square);

        for i in 0..HIDDEN_SIZE {
            white_acc[i] -= self.network.ft_weights[white_feat][i];
            black_acc[i] -= self.network.ft_weights[black_feat][i];
        }
    }

    /// Evaluate the position from the side-to-move's perspective.
    /// Reads from the top of the accumulator stack.
    #[inline(always)]
    pub fn evaluate(&self, side_to_move: Side) -> i16 {
        let (white_acc, black_acc) = self.stack.last().unwrap();

        let (our_acc, their_acc) = if side_to_move == Sides::WHITE {
            (white_acc, black_acc)
        } else {
            (black_acc, white_acc)
        };

        self.network.forward(our_acc, their_acc)
    }

    /// Debug: verify incremental accumulator matches a full refresh.
    #[cfg(debug_assertions)]
    pub fn verify(&self, position: &Position) {
        let mut white_acc = self.network.ft_biases;
        let mut black_acc = self.network.ft_biases;

        for sq in RangeOf::SQUARES {
            let piece = position.board[sq];
            if piece == PieceType::NONE {
                continue;
            }
            let pt = type_of_piece(piece);
            let color = color_of_piece(piece);
            let wf = feature_index(Sides::WHITE, color, pt, sq);
            let bf = feature_index(Sides::BLACK, color, pt, sq);
            for i in 0..HIDDEN_SIZE {
                white_acc[i] += self.network.ft_weights[wf][i];
                black_acc[i] += self.network.ft_weights[bf][i];
            }
        }

        let (inc_white, inc_black) = self.stack.last().unwrap();
        assert_eq!(&white_acc, inc_white, "NNUE white accumulator mismatch!");
        assert_eq!(&black_acc, inc_black, "NNUE black accumulator mismatch!");
    }
}
