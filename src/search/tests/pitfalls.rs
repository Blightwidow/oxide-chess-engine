use super::*;

#[test]
fn en_passant_discovered_check_illegal() {
    // Black king a4, black pawn d4, white pawn c4 (just pushed), white rook h4.
    // dxc3 e.p. removes both pawns from rank 4, exposing king to rook — illegal.
    let (mv, _) = search_position("8/8/8/8/k1Pp3R/8/8/3K4 b - c3 0 1", 4);
    assert_ne!(
        mv, "d4c3",
        "En passant should be illegal here (discovered check on king), but engine played it"
    );
}

#[test]
fn castling_through_check_illegal() {
    // Bishop on c4 attacks f1 — white cannot castle kingside (king would pass through f1)
    let (mv, _) = search_position("r3k2r/pppppppp/8/8/2b5/8/PPPPPPPP/R3K2R w KQkq - 0 1", 4);
    assert_ne!(
        mv, "e1g1",
        "Castling kingside through check should be illegal, but engine played it"
    );
}

#[test]
fn stalemate_avoidance_when_winning() {
    // White queen + king vs lone king — many moves stalemate (Qa7, Qc8), but mates exist
    // At depth 8, engine should find forced mate
    let (_, score) = search_position("k7/2Q5/8/1K6/8/8/8/8 w - - 0 1", 8);
    assert!(
        is_mate_score(score),
        "Engine should find checkmate and avoid stalemate, got score {}",
        score
    );
}

#[test]
fn pin_awareness() {
    // Black bishop b4 pins white knight c3 to king e1 — knight cannot move
    let (mv, _) = search_position("rnbqk1nr/pppp1ppp/8/4p3/1b2P3/2N5/PPPP1PPP/R1BQKBNR w KQkq - 2 3", 4);
    assert!(
        !mv.starts_with("c3"),
        "Pinned knight on c3 should not move, but engine played {}",
        mv
    );
}

#[test]
fn promotion_finds_queening() {
    // White pawn on a7, trivial promotion
    let (mv, _) = search_position("8/P7/8/8/8/6k1/8/4K3 w - - 0 1", 3);
    assert_eq!(mv, "a7a8q", "Expected queen promotion a7a8q but got {}", mv);
}

#[test]
fn promotion_or_die() {
    // White must promote b7 immediately or lose to black rook
    let (mv, _) = search_position("8/1P6/8/3k4/8/8/6K1/3r4 w - - 0 1", 4);
    assert!(mv.starts_with("b7b8"), "Expected b-pawn promotion but got {}", mv);
}

#[test]
fn zugzwang_pawn_endgame() {
    // Mutual zugzwang: blocked pawns on b5/b6, whoever moves loses their pawn
    // White to move must retreat king, allowing black king to win the b5 pawn
    let (_, score) = search_position("8/8/1p6/1P1k4/1K6/8/8/8 w - - 0 1", 8);
    assert!(
        score < 0,
        "Side to move in zugzwang should have negative score, got {}",
        score
    );
}
