#[cfg(test)]
mod test {
    use std::rc::Rc;

    use crate::{
        bitboards::Bitboards,
        evaluate::Eval,
        hash::Hasher,
        movegen::Movegen,
        position::Position,
        search::{
            defs::{FEN_START_POSITION, VALUE_MATE},
            Search,
        },
    };

    fn make_search() -> Search {
        let bitboards = Rc::new(Bitboards::new());
        let hasher = Rc::new(Hasher::new());
        let movegen = Movegen::new(Rc::clone(&bitboards));
        let position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));
        Search::new(position, movegen, Eval::new(), None)
    }

    fn search_position(fen: &str, depth: u8) -> (String, i16) {
        let mut search = make_search();
        search.position.set(fen.to_string());
        let (mv, score) = search.search(depth).expect("Expected a move");
        (format!("{:?}", mv), score)
    }

    fn is_mate_score(score: i16) -> bool {
        score.abs() > VALUE_MATE - 100
    }

    fn mate_in_n(score: i16) -> Option<i32> {
        if !is_mate_score(score) {
            return None;
        }
        // In alpha_beta, mate score = -VALUE_MATE + ply
        // At root (ply=0), search() calls alpha_beta with ply=1
        // So mate-in-1: score = VALUE_MATE - 1
        // mate-in-2: score = VALUE_MATE - 3
        // General: mate-in-N: score = VALUE_MATE - (2*N - 1)
        // So N = (VALUE_MATE - score + 1) / 2
        let distance = (VALUE_MATE as i32) - (score.abs() as i32);
        let n = (distance + 1) / 2;
        Some(n)
    }

    // ========== Perft tests ==========

    #[test]
    fn perft_startpos() {
        let mut search = make_search();

        search.position.set(FEN_START_POSITION.to_string());

        assert_eq!(search.perft(1, true), 20);
        assert_eq!(search.perft(2, true), 400);
        assert_eq!(search.perft(3, true), 8902);
        assert_eq!(search.perft(4, true), 197281);
        assert_eq!(search.perft(5, true), 4865609);
    }

    #[test]
    fn perft_kiwipete() {
        let mut search = make_search();

        search
            .position
            .set("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1".to_string());

        assert_eq!(search.perft(1, true), 48);
        assert_eq!(search.perft(2, true), 2039);
        assert_eq!(search.perft(3, true), 97862);
        assert_eq!(search.perft(4, true), 4085603);
        assert_eq!(search.perft(5, true), 193690690);
    }

    #[test]
    fn perft_edwards() {
        let mut search = make_search();

        search
            .position
            .set("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8".to_string());

        assert_eq!(search.perft(1, true), 44);
        assert_eq!(search.perft(2, true), 1486);
        assert_eq!(search.perft(3, true), 62379);
        assert_eq!(search.perft(4, true), 2103487);
        assert_eq!(search.perft(5, true), 89941194);
    }

    #[test]
    fn perft_endgame() {
        let mut search = make_search();

        search
            .position
            .set("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - -".to_string());

        assert_eq!(search.perft(1, true), 14);
        assert_eq!(search.perft(2, true), 191);
        assert_eq!(search.perft(3, true), 2812);
        assert_eq!(search.perft(4, true), 43238);
        assert_eq!(search.perft(5, true), 674624);
    }

    #[test]
    fn perft_edwards_bis() {
        let mut search = make_search();

        search
            .position
            .set("r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10".to_string());

        assert_eq!(search.perft(1, true), 46);
        assert_eq!(search.perft(2, true), 2079);
        assert_eq!(search.perft(3, true), 89890);
        assert_eq!(search.perft(4, true), 3894594);
        assert_eq!(search.perft(5, true), 164075551);
    }

    // ========== Mate-in-1 tests ==========

    #[test]
    fn mate_in_1_back_rank() {
        let (mv, score) = search_position("6k1/5ppp/8/8/8/8/8/3R2K1 w - - 0 1", 3);
        assert_eq!(mv, "d1d8", "Expected Rd8# but got {}", mv);
        assert!(is_mate_score(score), "Expected mate score but got {}", score);
        assert_eq!(mate_in_n(score), Some(1), "Expected mate in 1 but got {:?}", mate_in_n(score));
    }

    #[test]
    fn mate_in_1_scholars() {
        let (mv, score) = search_position("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4", 3);
        assert_eq!(mv, "h5f7", "Expected Qxf7# but got {}", mv);
        assert!(is_mate_score(score), "Expected mate score but got {}", score);
        assert_eq!(mate_in_n(score), Some(1));
    }

    #[test]
    fn mate_in_1_fools() {
        // Position after 1.f3 e5 2.g4 — black to play Qh4#
        let (mv, score) =
            search_position("rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 2", 3);
        assert_eq!(mv, "d8h4", "Expected Qh4# but got {}", mv);
        assert!(is_mate_score(score), "Expected mate score but got {}", score);
        assert_eq!(mate_in_n(score), Some(1));
    }
    
    #[test]
    fn mate_in_1_rook_king() {
        // Multiple mates exist (Ra1# and Rh1#), just check the engine finds a forced mate
        let (_, score) = search_position("k7/2K5/8/8/8/8/8/1R6 w - - 1 1", 3);
        assert!(is_mate_score(score), "Expected mate score but got {}", score);
        assert_eq!(mate_in_n(score), Some(1));
    }

    // ========== Mate-in-2 tests ==========

    #[test]
    fn mate_in_2_endgame() {
        let (_, score) = search_position("kbK5/pp6/1P6/8/8/8/8/R7 w - - 0 1", 5);
        assert!(is_mate_score(score), "Expected mate score but got {}", score);
        assert_eq!(mate_in_n(score), Some(2), "Expected mate in 2 but got {:?}", mate_in_n(score));
    }

    // ========== Tactical tests ==========

    #[test]
    fn tactical_fork_knight() {
        // White knight can fork king and rook: Nc7+ wins the rook
        // Net gain is approximately rook - knight value (~200cp)
        let (mv, score) = search_position("r3k3/8/8/3N4/8/8/8/4K3 w q - 0 1", 6);
        assert_eq!(mv, "d5c7", "Expected Nc7+ fork but got {}", mv);
        assert!(score > 150, "Expected significant advantage but got {}", score);
    }

    #[test]
    fn tactical_winning_queen() {
        // White can capture undefended queen with Bxe5
        let (mv, score) = search_position("rnb1kbnr/pppppppp/8/4q3/3B4/8/PPP1PPPP/RN1QKBNR w KQkq - 0 1", 4);
        assert_eq!(mv, "d4e5", "Expected Bxe5 winning queen but got {}", mv);
        assert!(score > 500, "Expected large advantage after winning queen but got {}", score);
    }

    // ========== Evaluation sanity tests ==========

    #[test]
    fn eval_starting_position() {
        let mut search = make_search();
        search.position.set(FEN_START_POSITION.to_string());
        let score = search.eval.evaluate(&search.position);
        assert!(
            score.abs() < 50,
            "Starting position should be roughly equal, got {}",
            score
        );
    }

    #[test]
    fn eval_extra_queen_white() {
        // Standard position but remove black queen (d8)
        let mut search = make_search();
        search
            .position
            .set("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string());
        let score = search.eval.evaluate(&search.position);
        assert!(score > 500, "White with extra queen should score > 500, got {}", score);
    }

    #[test]
    fn eval_missing_knight_white() {
        // Standard position but remove white knight (b1)
        let mut search = make_search();
        search
            .position
            .set("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/R1BQKBNR w KQkq - 0 1".to_string());
        let score = search.eval.evaluate(&search.position);
        assert!(score < -200, "White missing knight should score < -200, got {}", score);
    }
}
