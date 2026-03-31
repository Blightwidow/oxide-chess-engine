use super::*;

#[test]
fn stalemate_no_legal_moves() {
    // Black king on a8, white queen c7 covers all escapes, white king b6. Not in check = stalemate.
    assert!(
        search_returns_no_move("k7/2Q5/1K6/8/8/8/8/8 b - - 0 1", 1),
        "Stalemate position should return no legal move"
    );
}

#[test]
fn stalemate_avoidance_finds_mate() {
    // White has queen vs lone king — should find checkmate, not accidentally stalemate
    // KQ vs K needs higher depth to find forced mate; at depth 8 engine should see it
    let (_, score) = search_position("k7/2Q5/8/1K6/8/8/8/8 w - - 0 1", 8);
    assert!(
        is_mate_score(score),
        "Engine should find forced mate instead of stalemate, got score {}",
        score
    );
}

#[test]
fn fifty_move_rule_draw() {
    // KR vs K with halfmove clock = 100 — engine recognizes 50-move draw
    let (_, score) = search_position("8/8/3k4/8/4K3/4R3/8/8 w - - 100 100", 4);
    assert!(
        is_draw_score(score),
        "Position with rule50=100 should be a draw, got score {}",
        score
    );
}

#[test]
fn insufficient_material_kvk() {
    let (_, score) = search_position("8/8/3k4/8/4K3/8/8/8 w - - 0 1", 2);
    assert!(is_draw_score(score), "K vs K should be a draw, got score {}", score);
}

#[test]
fn insufficient_material_kbvk() {
    let (_, score) = search_position("8/8/3k4/8/4K3/3B4/8/8 w - - 0 1", 2);
    assert!(is_draw_score(score), "KB vs K should be a draw, got score {}", score);
}

#[test]
fn insufficient_material_knvk() {
    let (_, score) = search_position("8/8/3k4/8/4K3/5N2/8/8 w - - 0 1", 2);
    assert!(is_draw_score(score), "KN vs K should be a draw, got score {}", score);
}

#[test]
fn insufficient_material_kbvkb_same_color() {
    // Both bishops on dark squares (d3: file=3,rank=2, sum=5 odd=dark; a8: file=0,rank=7, sum=7 odd=dark)
    let (_, score) = search_position("b7/8/3k4/8/4K3/3B4/8/8 w - - 0 1", 2);
    assert!(
        is_draw_score(score),
        "KB vs KB same color bishops should be a draw, got score {}",
        score
    );
}

#[test]
fn kp_vs_k_promotion() {
    // White pawn on e7 about to promote, black king far away
    let (mv, score) = search_position("8/4P1k1/8/8/8/8/8/4K3 w - - 0 1", 4);
    assert_eq!(mv, "e7e8q", "Expected queen promotion e7e8q but got {}", mv);
    assert!(score > 500, "Promotion should give large advantage, got {}", score);
}

#[test]
fn kr_vs_k_close_mate() {
    // KR vs K, white should find forced mate quickly
    let (_, score) = search_position("6k1/8/5K2/8/8/8/8/7R w - - 0 1", 3);
    assert!(is_mate_score(score), "Expected mate score but got {}", score);
}
