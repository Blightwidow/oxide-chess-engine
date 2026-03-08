#[cfg(test)]
mod test {
    use crate::defs::*;
    use crate::movegen::defs::*;

    const E2: Square = 12; // square_of(4, 1)
    const E4: Square = 28; // square_of(4, 3)
    const E7: Square = 52; // square_of(4, 6)
    const E8: Square = 60; // square_of(4, 7)
    const E1: Square = 4;  // square_of(4, 0)
    const A1: Square = 0;
    const H1: Square = 7;
    const A8: Square = 56;
    const D5: Square = 35; // square_of(3, 4)

    #[test]
    fn roundtrip_from_to() {
        let mv = Move::with_from_to(E2, E4);
        assert_eq!(mv.from_sq(), E2);
        assert_eq!(mv.to_sq(), E4);
    }

    #[test]
    fn promotion_queen() {
        let mv = Move::make(E7, E8, PieceType::QUEEN, MoveTypes::PROMOTION);
        assert_eq!(mv.from_sq(), E7);
        assert_eq!(mv.to_sq(), E8);
        assert_eq!(mv.type_of(), MoveTypes::PROMOTION);
        assert_eq!(mv.promotion_type(), PieceType::QUEEN);
    }

    #[test]
    fn all_promotion_types() {
        for promo in [PieceType::KNIGHT, PieceType::BISHOP, PieceType::ROOK, PieceType::QUEEN] {
            let mv = Move::make(E7, E8, promo, MoveTypes::PROMOTION);
            assert_eq!(mv.promotion_type(), promo, "promotion type mismatch for piece {}", promo);
        }
    }

    #[test]
    fn castling_type() {
        let mv = Move::make(E1, H1, PieceType::NONE, MoveTypes::CASTLING);
        assert_eq!(mv.type_of(), MoveTypes::CASTLING);
    }

    #[test]
    fn en_passant_type() {
        let mv = Move::make(D5, E4, PieceType::NONE, MoveTypes::EN_PASSANT);
        assert_eq!(mv.type_of(), MoveTypes::EN_PASSANT);
        assert_eq!(mv.from_sq(), D5);
        assert_eq!(mv.to_sq(), E4);
    }

    #[test]
    fn none_and_null_not_ok() {
        assert!(!Move::none().is_ok());
        assert!(!Move::null().is_ok());
    }

    #[test]
    fn normal_move_is_ok() {
        assert!(Move::with_from_to(E2, E4).is_ok());
    }

    #[test]
    fn debug_normal_move() {
        let mv = Move::with_from_to(E2, E4);
        assert_eq!(format!("{:?}", mv), "e2e4");
    }

    #[test]
    fn debug_promotion() {
        let mv = Move::make(E7, E8, PieceType::QUEEN, MoveTypes::PROMOTION);
        assert_eq!(format!("{:?}", mv), "e7e8q");

        let mv = Move::make(E7, E8, PieceType::KNIGHT, MoveTypes::PROMOTION);
        assert_eq!(format!("{:?}", mv), "e7e8n");
    }

    #[test]
    fn debug_none_and_null() {
        assert_eq!(format!("{:?}", Move::none()), "0000");
        assert_eq!(format!("{:?}", Move::null()), "0000");
    }

    #[test]
    fn debug_castling_kingside() {
        // White kingside: king on e1 (4), rook on h1 (7) -> display as e1g1
        let mv = Move::make(E1, H1, PieceType::NONE, MoveTypes::CASTLING);
        assert_eq!(format!("{:?}", mv), "e1g1");
    }

    #[test]
    fn debug_castling_queenside() {
        // White queenside: king on e1 (4), rook on a1 (0) -> display as e1c1
        let mv = Move::make(E1, A1, PieceType::NONE, MoveTypes::CASTLING);
        assert_eq!(format!("{:?}", mv), "e1c1");
    }

    #[test]
    fn non_promotion_returns_none_type() {
        let mv = Move::with_from_to(E2, E4);
        assert_eq!(mv.promotion_type(), PieceType::NONE);
    }

    #[test]
    fn different_squares_roundtrip() {
        for from in [A1, H1, A8, E1, E4] {
            for to in [A1, H1, A8, E1, E4] {
                if from == to {
                    continue;
                }
                let mv = Move::with_from_to(from, to);
                assert_eq!(mv.from_sq(), from);
                assert_eq!(mv.to_sq(), to);
            }
        }
    }
}
