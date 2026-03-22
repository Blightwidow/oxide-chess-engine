use crate::defs::{Piece, Side, Sides, Square};

/// Returns true if the king is on the king-side (files e-h), requiring horizontal mirroring.
pub fn needs_mirror(king_sq: Square) -> bool {
    king_sq % 8 >= 4
}

/// Flip a square horizontally (a<->h, b<->g, c<->f, d<->e).
pub fn mirror_horizontal(sq: Square) -> Square {
    sq ^ 7
}

/// Compute the king bucket (0..7) for a perspective-relative king square.
/// Mirrors to queen-side first, then returns the rank.
pub fn king_bucket(perspective_ksq: Square) -> usize {
    let ksq = if needs_mirror(perspective_ksq) {
        mirror_horizontal(perspective_ksq)
    } else {
        perspective_ksq
    };
    ksq / 8
}

/// Check if a king move requires a full accumulator refresh for the given perspective.
/// Returns true if the bucket or mirror state changed.
pub fn needs_refresh(perspective: Side, old_king_sq: Square, new_king_sq: Square) -> bool {
    let old = if perspective == Sides::WHITE {
        old_king_sq
    } else {
        old_king_sq ^ 56
    };
    let new = if perspective == Sides::WHITE {
        new_king_sq
    } else {
        new_king_sq ^ 56
    };
    // Bucket (rank) changed or mirror (file half) changed
    old / 8 != new / 8 || (old % 8 >= 4) != (new % 8 >= 4)
}

/// Compute the feature index for a piece on a square from a given perspective,
/// bucketed by the perspective's king position with horizontal mirroring.
///
/// Feature layout: `bucket * 768 + color * 384 + (piece_type - 1) * 64 + square`
/// When king is on files e-h, all squares are mirrored horizontally.
/// For black's perspective, colors are swapped and squares are vertically flipped.
pub fn feature_index(
    perspective: Side,
    king_sq: Square,
    piece_color: Side,
    piece_type: Piece,
    square: Square,
) -> usize {
    let (color, sq, ksq) = if perspective == Sides::WHITE {
        (piece_color, square, king_sq)
    } else {
        (piece_color ^ 1, square ^ 56, king_sq ^ 56)
    };
    let mirror = needs_mirror(ksq);
    let sq = if mirror { mirror_horizontal(sq) } else { sq };
    let ksq = if mirror { mirror_horizontal(ksq) } else { ksq };
    let bucket = ksq / 8;
    bucket * 768 + color * 384 + (piece_type - 1) * 64 + sq
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::defs::PieceType;
    use crate::nnue::defs::BUCKET_FEATURE_SIZE;

    #[test]
    fn mirror_horizontal_roundtrip() {
        for sq in 0..64 {
            assert_eq!(mirror_horizontal(mirror_horizontal(sq)), sq);
        }
    }

    #[test]
    fn king_bucket_range() {
        for sq in 0..64 {
            let bucket = king_bucket(sq);
            assert!(bucket < 8, "bucket {} out of range for sq {}", bucket, sq);
        }
    }

    #[test]
    fn king_bucket_mirrors_correctly() {
        // e1 (sq 4, file 4) should mirror to d1 (sq 3, rank 0) -> bucket 0
        assert_eq!(king_bucket(4), 0);
        // a1 (sq 0, file 0) -> no mirror, rank 0 -> bucket 0
        assert_eq!(king_bucket(0), 0);
        // h8 (sq 63, file 7) -> mirror to a8 (sq 56), rank 7 -> bucket 7
        assert_eq!(king_bucket(63), 7);
        // a8 (sq 56, file 0) -> no mirror, rank 7 -> bucket 7
        assert_eq!(king_bucket(56), 7);
    }

    #[test]
    fn needs_refresh_detects_bucket_change() {
        // King moves from rank 0 to rank 1 (same file side)
        assert!(needs_refresh(Sides::WHITE, 0, 8)); // a1 -> a2
    }

    #[test]
    fn needs_refresh_detects_mirror_change() {
        // King moves from d1 (file 3) to e1 (file 4) - same rank but mirror flips
        assert!(needs_refresh(Sides::WHITE, 3, 4));
    }

    #[test]
    fn needs_refresh_same_bucket_and_mirror() {
        // King moves from a1 to b1 (both file 0-3, rank 0)
        assert!(!needs_refresh(Sides::WHITE, 0, 1));
        // King moves from e1 to f1 (both file 4-7, rank 0)
        assert!(!needs_refresh(Sides::WHITE, 4, 5));
    }

    #[test]
    fn feature_index_bounds() {
        for king_sq in 0..64 {
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
                            let idx = feature_index(perspective, king_sq, color, pt, sq);
                            assert!(
                                idx < BUCKET_FEATURE_SIZE,
                                "feature index {} out of range (ksq={}, persp={}, color={}, pt={}, sq={})",
                                idx,
                                king_sq,
                                perspective,
                                color,
                                pt,
                                sq
                            );
                        }
                    }
                }
            }
        }
    }

    /// Verify that our feature layout matches bullet's ChessBucketsMirrored:
    /// bullet: bucket_offset + (chess768_feature ^ flip)
    /// where bucket_offset = 768 * bucket[ksq], flip = 7 if ksq file > 3 else 0
    /// chess768_feature = color * 384 + piece_type * 64 + square
    #[test]
    fn feature_layout_matches_bullet_chess_buckets_mirrored() {
        // Define bullet's bucket mapping: rank-based with horizontal mirror
        let buckets: [usize; 32] = {
            let mut b = [0usize; 32];
            for i in 0..32 {
                b[i] = i / 4; // rank = bucket
            }
            b
        };

        // Expand to 64 squares (bullet's ChessBucketsMirrored expansion)
        let expanded: [usize; 64] = {
            let mirror_file = [0, 1, 2, 3, 3, 2, 1, 0];
            let mut e = [0usize; 64];
            for idx in 0..64 {
                e[idx] = buckets[(idx / 8) * 4 + mirror_file[idx % 8]];
            }
            e
        };

        let engine_piece_types = [
            PieceType::PAWN,
            PieceType::KNIGHT,
            PieceType::BISHOP,
            PieceType::ROOK,
            PieceType::QUEEN,
            PieceType::KING,
        ];

        // Test from white's (STM) perspective with various king positions
        for ksq in 0..64usize {
            let flip: usize = if ksq % 8 > 3 { 7 } else { 0 };
            let bucket_offset = 768 * expanded[ksq];

            for color in 0..2usize {
                for (bullet_pt, &engine_pt) in engine_piece_types.iter().enumerate() {
                    for sq in 0..64usize {
                        let bullet_feat = bucket_offset + (color * 384 + bullet_pt * 64 + sq) ^ flip;
                        let engine_feat = feature_index(Sides::WHITE, ksq, color, engine_pt, sq);
                        assert_eq!(
                            bullet_feat, engine_feat,
                            "Mismatch at ksq={}, color={}, pt={}, sq={}: bullet={} vs engine={}",
                            ksq, color, bullet_pt, sq, bullet_feat, engine_feat
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn feature_index_known_values() {
        // White pawn on a1, king on a1 (bucket 0, no mirror)
        // 0*768 + 0*384 + 0*64 + 0 = 0
        assert_eq!(feature_index(Sides::WHITE, 0, Sides::WHITE, PieceType::PAWN, 0), 0);

        // White king on e1 (sq 4), king on e1: mirror -> d1 (sq 3), bucket 0
        // piece sq a1 (0) mirrors to h1 (7): 0*768 + 0*384 + 5*64 + 7 = 327
        assert_eq!(feature_index(Sides::WHITE, 4, Sides::WHITE, PieceType::KING, 0), 327);

        // King on a8 (sq 56), bucket 7, no mirror
        // Black king on h8 (sq 63): 7*768 + 1*384 + 5*64 + 63 = 5376 + 384 + 320 + 63 = 6143
        assert_eq!(feature_index(Sides::WHITE, 56, Sides::BLACK, PieceType::KING, 63), 6143);
    }

    #[test]
    fn perspective_symmetry() {
        // White pawn on e2 from white's perspective (king on e1)
        // should equal black pawn on e7 from black's perspective (king on e8)
        let white_feat = feature_index(Sides::WHITE, 4, Sides::WHITE, PieceType::PAWN, 12); // e2
        let black_feat = feature_index(Sides::BLACK, 60, Sides::BLACK, PieceType::PAWN, 52); // e7, king e8
        assert_eq!(white_feat, black_feat);
    }
}
