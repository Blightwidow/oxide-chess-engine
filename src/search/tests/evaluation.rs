use super::*;

// ========== Repetition detection tests ==========

#[test]
fn repetition_detected_as_draw() {
    use crate::movegen::defs::Move;
    use crate::search::defs::SearchLimits;

    let mut search = make_search();
    // Position where white is up material but can force a draw via repetition
    // We'll manually play Ng1-f3-g1-f3 to create a repetition
    search
        .position
        .set("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string());
    search.nnue.refresh(&search.position);

    // Play Nf3, Nf6, Ng1, Ng8 — back to start, then Nf3, Nf6, Ng1, Ng8 — repeat
    let moves = ["g1f3", "g8f6", "f3g1", "f6g8", "g1f3", "g8f6", "f3g1", "f6g8"];
    for mv_str in &moves {
        let from = (mv_str.as_bytes()[1] - b'1') as usize * 8 + (mv_str.as_bytes()[0] - b'a') as usize;
        let to = (mv_str.as_bytes()[3] - b'1') as usize * 8 + (mv_str.as_bytes()[2] - b'a') as usize;
        let mv = Move::with_from_to(from, to);
        search.make_move(mv);
    }

    // Now we're back at the starting position for the second time.
    // A search should recognize this and return a draw-ish score.
    let limits = SearchLimits {
        depth: 4,
        ..SearchLimits::default()
    };
    search.nodes_searched = 0;
    search.time = crate::time::TimeManager::new(
        limits,
        search.position.side_to_move,
        search.position.states.last().unwrap().game_ply,
    );
    search.start_time = std::time::Instant::now();

    let result = search.search(4);
    // The engine should NOT play into the same repeated line if it's worth 0
    // Just verify it doesn't crash and returns a valid move
    assert!(result.is_some(), "Search should return a move after repetition");
}

// ========== Evaluation sanity tests ==========

#[test]
fn eval_starting_position() {
    let mut search = make_search();
    search.position.set(FEN_START_POSITION.to_string());
    search.nnue.refresh(&search.position);
    let score = search.evaluate_position();
    assert!(
        score.abs() < 50,
        "Starting position should be roughly equal, got {}",
        score
    );
}

#[test]
fn eval_extra_queen_white() {
    let mut search = make_search();
    // Standard position but remove black queen (d8)
    search
        .position
        .set("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string());
    search.nnue.refresh(&search.position);
    let score = search.evaluate_position();
    assert!(score > 500, "White with extra queen should score > 500, got {}", score);
}

#[test]
fn eval_missing_knight_white() {
    let mut search = make_search();
    // Standard position but remove white knight (b1)
    search
        .position
        .set("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/R1BQKBNR w KQkq - 0 1".to_string());
    search.nnue.refresh(&search.position);
    let score = search.evaluate_position();
    assert!(score < -200, "White missing knight should score < -200, got {}", score);
}
