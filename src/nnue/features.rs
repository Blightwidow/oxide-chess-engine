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

    /// Verify that our feature layout matches bullet's Chess768:
    /// bullet: color * 384 + piece_type * 64 + square (piece_type 0-5: pawn..king)
    /// engine: color * 384 + (piece_type - 1) * 64 + square (piece_type 1-6: pawn..king)
    /// These should produce identical indices since bullet's 0 maps to our PAWN(1)-1=0, etc.
    #[test]
    fn feature_layout_matches_bullet_chess768() {
        // bullet piece_type mapping: 0=pawn, 1=knight, 2=bishop, 3=rook, 4=queen, 5=king
        // engine PieceType: PAWN=1, KNIGHT=2, BISHOP=3, ROOK=4, QUEEN=5, KING=6
        let engine_piece_types = [
            PieceType::PAWN,
            PieceType::KNIGHT,
            PieceType::BISHOP,
            PieceType::ROOK,
            PieceType::QUEEN,
            PieceType::KING,
        ];

        for color in 0..2usize {
            for (bullet_pt, &engine_pt) in engine_piece_types.iter().enumerate() {
                for sq in 0..64usize {
                    let bullet_index = color * 384 + bullet_pt * 64 + sq;
                    let engine_index = feature_index(Sides::WHITE, color, engine_pt, sq);
                    assert_eq!(
                        bullet_index, engine_index,
                        "Mismatch: bullet({}, {}, {}) = {} vs engine = {}",
                        color, bullet_pt, sq, bullet_index, engine_index
                    );
                }
            }
        }
    }

    /// Verify specific known indices for spot-checking
    #[test]
    fn feature_index_known_values() {
        // White pawn on a1 (sq 0) from white's perspective: 0*384 + 0*64 + 0 = 0
        assert_eq!(feature_index(Sides::WHITE, Sides::WHITE, PieceType::PAWN, 0), 0);

        // White king on e1 (sq 4) from white's perspective: 0*384 + 5*64 + 4 = 324
        assert_eq!(feature_index(Sides::WHITE, Sides::WHITE, PieceType::KING, 4), 324);

        // Black pawn on a7 (sq 48) from white's perspective: 1*384 + 0*64 + 48 = 432
        assert_eq!(feature_index(Sides::WHITE, Sides::BLACK, PieceType::PAWN, 48), 432);

        // Max index: black king on h8 (sq 63) from white's perspective: 1*384 + 5*64 + 63 = 767
        assert_eq!(feature_index(Sides::WHITE, Sides::BLACK, PieceType::KING, 63), 767);
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
