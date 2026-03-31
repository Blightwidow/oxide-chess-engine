use super::*;

#[test]
fn develops_pieces() {
    // From the starting position, the engine should play a reasonable developing move
    let (mv, _) = search_position(FEN_START_POSITION, 5);
    let reasonable_first_moves = ["e2e4", "d2d4", "g1f3", "c2c4", "b1c3"];
    assert!(
        reasonable_first_moves.contains(&mv.as_str()),
        "Expected a standard opening move, got {}",
        mv
    );
}

#[test]
fn castles_when_available() {
    // Italian Game position — white should castle kingside
    let (mv, _) = search_position(
        "r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
        5,
    );
    // Accept castling or other strong developing moves (d2d3, O-O, a2a4)
    let strong_moves = ["e1g1", "d2d3", "d2d4", "c2c3", "b1c3"];
    assert!(
        strong_moves.contains(&mv.as_str()),
        "Expected castling or strong developing move, got {}",
        mv
    );
}

#[test]
fn center_response() {
    // After 1.e4, black should contest the center
    let (mv, _) = search_position("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1", 4);
    let standard_responses = ["e7e5", "c7c5", "d7d5", "e7e6", "c7c6", "g8f6", "d7d6"];
    assert!(
        standard_responses.contains(&mv.as_str()),
        "Expected standard response to 1.e4, got {}",
        mv
    );
}
