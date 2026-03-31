use super::*;

// ========== Mate-in-1 tests ==========

#[test]
fn mate_in_1_back_rank() {
    let (mv, score) = search_position("6k1/5ppp/8/8/8/8/8/3R2K1 w - - 0 1", 3);
    assert_eq!(mv, "d1d8", "Expected Rd8# but got {}", mv);
    assert!(is_mate_score(score), "Expected mate score but got {}", score);
    assert_eq!(
        mate_in_n(score),
        Some(1),
        "Expected mate in 1 but got {:?}",
        mate_in_n(score)
    );
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
    let (mv, score) = search_position("rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 2", 3);
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
    assert_eq!(
        mate_in_n(score),
        Some(2),
        "Expected mate in 2 but got {:?}",
        mate_in_n(score)
    );
}

// ========== Tactical tests ==========

#[test]
fn tactical_fork_knight() {
    let mut search = make_search();
    // White knight can fork king and rook: Nc7+ wins the rook
    // Net gain is approximately rook - knight value (~200cp)
    search.position.set("r3k3/8/4N3/8/8/7P/4K3/8 w - - 0 1".to_string());
    search.nnue.refresh(&search.position);
    let (mv, score) = search.search(8).expect("Expected a move");
    let mv = format!("{:?}", mv);
    assert_eq!(mv, "e6c7", "Expected Nc7+ fork but got {}", mv);
    assert!(score > 200, "Expected significant advantage but got {}", score);
}

#[test]
fn tactical_winning_queen() {
    let mut search = make_search();
    // White can capture undefended queen with Bxe5
    search
        .position
        .set("rnb1kbnr/pppppppp/8/4q3/3B4/8/PPP1PPPP/RN1QKBNR w KQkq - 0 1".to_string());
    search.nnue.refresh(&search.position);
    let (mv, score) = search.search(4).expect("Expected a move");
    let mv = format!("{:?}", mv);
    assert_eq!(mv, "d4e5", "Expected Bxe5 winning queen but got {}", mv);
    assert!(
        score > 500,
        "Expected large advantage after winning queen but got {}",
        score
    );
}
