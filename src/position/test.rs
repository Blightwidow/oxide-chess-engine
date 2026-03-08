#[cfg(test)]
mod test {
    use std::rc::Rc;

    use crate::{
        bitboards::{defs::EMPTY, Bitboards},
        hash::Hasher,
        movegen::Movegen,
        position::Position,
    };

    #[test]
    fn do_undo() {
        let bitboards = Rc::new(Bitboards::new());
        let hasher = Rc::new(Hasher::new());
        let movegen = Movegen::new(Rc::clone(&bitboards));
        let mut initial_position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));
        let mut position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));

        let fen: &str = "r3k2r/p1pNqpb1/bn2pnp1/3P4/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 0 1";
        position.set(fen.to_string());
        initial_position.set(fen.to_string());

        for mv in movegen.legal_moves(&position) {
            position.do_move(mv);
            position.undo_move(mv);

            assert_eq!(position.board, initial_position.board);
            assert_eq!(position.side_to_move, initial_position.side_to_move);
            assert_eq!(position.by_color_bb, initial_position.by_color_bb);
            assert_eq!(position.by_type_bb, initial_position.by_type_bb);
            assert_eq!(position.pinned_bb, initial_position.pinned_bb);
            assert_eq!(position.states.last().unwrap(), initial_position.states.last().unwrap());
        }
    }

    #[test]
    fn pinned_bb() {
        let bitboards = Rc::new(Bitboards::new());
        let hasher = Rc::new(Hasher::new());
        let mut position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));

        let fen: &str = "rnbqkbnr/pp1ppppp/2p5/1B6/4P3/8/PPPP1PPP/RNBQK1NR b KQkq - 1 2";
        position.set(fen.to_string());

        assert_eq!(position.pinned_bb, [EMPTY, EMPTY, EMPTY]);
    }

    #[test]
    fn zobrist_consistency_after_do_undo() {
        let bitboards = Rc::new(Bitboards::new());
        let hasher = Rc::new(Hasher::new());
        let movegen = Movegen::new(Rc::clone(&bitboards));

        let fens = [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r3k2r/p1pNqpb1/bn2pnp1/3P4/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 0 1",
            "rnbqkb1r/pp1p1pPp/8/2p1pP2/1P1P4/3P3P/P1P1P3/RNBQKBNR w KQkq e6 0 1",
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        ];

        for fen in fens {
            let mut position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));
            position.set(fen.to_string());
            let original_zobrist = position.zobrist;

            for mv in movegen.legal_moves(&position) {
                position.do_move(mv);
                position.undo_move(mv);
                assert_eq!(
                    position.zobrist, original_zobrist,
                    "Zobrist mismatch after do/undo {:?} in FEN: {}",
                    mv, fen
                );
            }
        }
    }

    #[test]
    fn castling_do_undo() {
        use crate::defs::{make_piece, PieceType, Sides, square_of};
        use crate::movegen::defs::MoveTypes;

        let bitboards = Rc::new(Bitboards::new());
        let hasher = Rc::new(Hasher::new());
        let movegen = Movegen::new(Rc::clone(&bitboards));

        // Position where white can castle kingside
        let fen = "r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1";
        let mut position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));
        position.set(fen.to_string());

        let initial_board = position.board;
        let initial_zobrist = position.zobrist;

        let castling_moves: Vec<_> = movegen
            .legal_moves(&position)
            .into_iter()
            .filter(|mv| mv.type_of() == MoveTypes::CASTLING)
            .collect();

        assert!(!castling_moves.is_empty(), "Should have castling moves");

        for mv in castling_moves {
            position.do_move(mv);

            // After castling, king should not be on e1
            let e1 = square_of(4, 0);
            assert_ne!(
                position.board[e1],
                make_piece(Sides::WHITE, PieceType::KING),
                "King should have moved from e1"
            );

            position.undo_move(mv);

            assert_eq!(position.board, initial_board, "Board not restored after castling undo");
            assert_eq!(position.zobrist, initial_zobrist, "Zobrist not restored after castling undo");
        }
    }

    #[test]
    fn en_passant_do_undo() {
        use crate::defs::{PieceType, square_of};
        use crate::movegen::defs::MoveTypes;

        let bitboards = Rc::new(Bitboards::new());
        let hasher = Rc::new(Hasher::new());
        let movegen = Movegen::new(Rc::clone(&bitboards));

        // White pawn on e5, black just played d7d5 -> EP available
        let fen = "rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 1";
        let mut position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));
        position.set(fen.to_string());

        let initial_board = position.board;
        let initial_zobrist = position.zobrist;

        let ep_moves: Vec<_> = movegen
            .legal_moves(&position)
            .into_iter()
            .filter(|mv| mv.type_of() == MoveTypes::EN_PASSANT)
            .collect();

        assert!(!ep_moves.is_empty(), "Should have en passant moves");

        for mv in ep_moves {
            position.do_move(mv);

            // The captured pawn on d5 should be gone
            let d5 = square_of(3, 4);
            assert_eq!(position.board[d5], PieceType::NONE, "Captured pawn should be removed");

            position.undo_move(mv);

            assert_eq!(position.board, initial_board, "Board not restored after EP undo");
            assert_eq!(position.zobrist, initial_zobrist, "Zobrist not restored after EP undo");
        }
    }

    #[test]
    fn promotion_do_undo() {
        use crate::defs::{make_piece, type_of_piece, PieceType, Sides};
        use crate::movegen::defs::MoveTypes;

        let bitboards = Rc::new(Bitboards::new());
        let hasher = Rc::new(Hasher::new());
        let movegen = Movegen::new(Rc::clone(&bitboards));

        // White pawn on e7, can promote
        let fen = "8/4P3/8/8/8/8/8/4K2k w - - 0 1";
        let mut position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));
        position.set(fen.to_string());

        let initial_board = position.board;
        let initial_zobrist = position.zobrist;

        let promo_moves: Vec<_> = movegen
            .legal_moves(&position)
            .into_iter()
            .filter(|mv| mv.type_of() == MoveTypes::PROMOTION)
            .collect();

        assert!(!promo_moves.is_empty(), "Should have promotion moves");

        for mv in promo_moves {
            let expected_piece_type = mv.promotion_type();
            position.do_move(mv);

            // After promotion, the target square should have the promoted piece
            let to = mv.to_sq();
            assert_eq!(
                type_of_piece(position.board[to]),
                expected_piece_type,
                "Piece type should be promoted type"
            );
            assert_ne!(
                position.board[to],
                make_piece(Sides::WHITE, PieceType::PAWN),
                "Should no longer be a pawn"
            );

            position.undo_move(mv);

            assert_eq!(position.board, initial_board, "Board not restored after promotion undo");
            assert_eq!(position.zobrist, initial_zobrist, "Zobrist not restored after promotion undo");
        }
    }
}
