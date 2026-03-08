use crate::defs::{Piece, Side, Sides, Square};

/// Compute the feature index for a piece on a square from a given perspective.
///
/// Feature layout: `color * 384 + (piece_type - 1) * 64 + square`
/// For black's perspective, colors are swapped and squares are vertically flipped.
pub fn feature_index(perspective: Side, piece_color: Side, piece_type: Piece, square: Square) -> usize {
    let (color, sq) = if perspective == Sides::WHITE {
        (piece_color, square)
    } else {
        (piece_color ^ 1, square ^ 56)
    };
    color * 384 + (piece_type - 1) * 64 + sq
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::defs::PieceType;

    #[test]
    fn feature_index_bounds() {
        for perspective in [Sides::WHITE, Sides::BLACK] {
            for color in [Sides::WHITE, Sides::BLACK] {
                for pt in [
                    PieceType::PAWN,
                    PieceType::KNIGHT,
                    PieceType::BISHOP,
                    PieceType::ROOK,
                    PieceType::QUEEN,
                    PieceType::KING,
                ] {
                    for sq in 0..64 {
                        let idx = feature_index(perspective, color, pt, sq);
                        assert!(idx < 768, "feature index {} out of range", idx);
                    }
                }
            }
        }
    }

    #[test]
    fn perspective_symmetry() {
        // White pawn on e2 from white's perspective should equal
        // black pawn on e7 from black's perspective
        let white_feat = feature_index(Sides::WHITE, Sides::WHITE, PieceType::PAWN, 12); // e2 = sq 12
        let black_feat = feature_index(Sides::BLACK, Sides::BLACK, PieceType::PAWN, 52); // e7 = sq 52
        assert_eq!(white_feat, black_feat);
    }
}
