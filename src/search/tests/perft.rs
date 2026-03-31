use super::*;

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

    search.position.set("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - -".to_string());

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
