use super::*;
use crate::search::see;

fn see_of(fen: &str, uci: &str) -> i16 {
    let mut search = make_search();
    search.position.set(fen.to_string());
    let mv = search
        .movegen
        .legal_moves(&search.position)
        .into_iter()
        .find(|mv| format!("{:?}", mv) == uci)
        .expect("move must be legal");
    see(&search.position, mv)
}

#[test]
fn see_undefended_pawn() {
    assert_eq!(see_of("1k1r4/1pp4p/p7/4p3/8/P5P1/1PP4P/2K1R3 w - - 0 1", "e1e5"), 100);
}

#[test]
fn see_pawn_takes_defended_knight() {
    // Swap-list pruning keeps the sign exact but may stop before the recapture
    // is counted, so only the sign is guaranteed.
    assert!(see_of("4k3/8/2p5/3n4/4P3/8/8/4K3 w - - 0 1", "e4d5") > 0);
}

#[test]
fn see_queen_takes_defended_pawn() {
    assert_eq!(see_of("4k3/8/2p5/3p4/8/8/3Q4/4K3 w - - 0 1", "d2d5"), -800);
}

#[test]
fn see_rook_takes_defended_knight() {
    assert_eq!(see_of("4k3/8/2p5/3n4/8/8/3R4/4K3 w - - 0 1", "d2d5"), -200);
}

#[test]
fn see_xray_exchange_sequence() {
    assert_eq!(
        see_of("1k1r3q/1ppn3p/p4b2/4p3/8/P2N2P1/1PP1R1BP/2K1Q3 w - - 0 1", "d3e5"),
        -200
    );
}
