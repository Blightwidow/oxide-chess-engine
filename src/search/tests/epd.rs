use super::*;

/// Whether the engine should find (`BestMove`) or avoid (`AvoidMove`) the listed moves.
enum MoveCheck<'a> {
    BestMove(&'a [&'a str]),
    AvoidMove(&'a [&'a str]),
}

struct EpdPosition<'a> {
    fen: &'a str,
    check: MoveCheck<'a>,
    id: &'a str,
}

fn run_epd_suite(name: &str, positions: &[EpdPosition], depth: u8, min_pass: usize) {
    let mut passed = 0;
    let total = positions.len();

    for pos in positions {
        let (mv, _score) = search_position(pos.fen, depth);
        let ok = match &pos.check {
            MoveCheck::BestMove(moves) => moves.contains(&mv.as_str()),
            MoveCheck::AvoidMove(moves) => !moves.contains(&mv.as_str()),
        };
        if ok {
            passed += 1;
        }
        let detail = match &pos.check {
            MoveCheck::BestMove(moves) if !ok => format!("(expected {:?})", moves),
            MoveCheck::AvoidMove(moves) if !ok => format!("(should avoid {:?})", moves),
            _ => String::new(),
        };
        println!(
            "{}: {} — engine played {} {}",
            pos.id,
            if ok { "PASS" } else { "FAIL" },
            mv,
            detail
        );
    }

    println!("\n{}: {}/{} passed (threshold: {})", name, passed, total, min_pass);
    assert!(
        passed >= min_pass,
        "{}: only {}/{} passed, minimum is {}",
        name,
        passed,
        total,
        min_pass
    );
}

#[test]
#[ignore = "Manual test"]
fn bratko_kopec() {
    let positions = [
        EpdPosition {
            fen: "1k1r4/pp1b1R2/3q2pp/4p3/2B5/4Q3/PPP2B2/2K5 b - -",
            check: MoveCheck::BestMove(&["d6d1"]),
            id: "BK.01",
        },
        EpdPosition {
            fen: "3r1k2/4npp1/1ppr3p/p6P/P2PPPP1/1NR5/5K2/2R5 w - -",
            check: MoveCheck::BestMove(&["d4d5"]),
            id: "BK.02",
        },
        EpdPosition {
            fen: "2q1rr1k/3bbnnp/p2p1pp1/2pPp3/PpP1P1P1/1P2BNNP/2BQ1PRK/7R b - -",
            check: MoveCheck::BestMove(&["f6f5"]),
            id: "BK.03",
        },
        EpdPosition {
            fen: "rnbqkb1r/p3pppp/1p6/2ppP3/3N4/2P5/PPP1BPPP/R1BQK2R w KQkq -",
            check: MoveCheck::BestMove(&["e5e6"]),
            id: "BK.04",
        },
        EpdPosition {
            fen: "r1b2rk1/2q1b1pp/p2ppn2/1p6/3QP3/1BN1B3/PPP3PP/R4RK1 w - -",
            check: MoveCheck::BestMove(&["c3d5", "a2a4"]),
            id: "BK.05",
        },
        EpdPosition {
            fen: "2r3k1/pppR1pp1/4p3/4P1P1/5P2/1P4K1/P1P5/8 w - -",
            check: MoveCheck::BestMove(&["g5g6"]),
            id: "BK.06",
        },
        EpdPosition {
            fen: "1nk1r1r1/pp2n1pp/4p3/q2pPp1N/b1pP1P2/B1P2R2/2P1B1PP/R2Q2K1 w - -",
            check: MoveCheck::BestMove(&["h5f6"]),
            id: "BK.07",
        },
        EpdPosition {
            fen: "4b3/p3kp2/6p1/3pP2p/2pP1P2/2P5/P1B5/2K5 w - -",
            check: MoveCheck::BestMove(&["f4f5"]),
            id: "BK.08",
        },
        EpdPosition {
            fen: "2kr1bnr/pbpq4/2n1pp2/3p3p/3P1P1B/2N2N1Q/PPP3PP/2KR1B1R w - -",
            check: MoveCheck::BestMove(&["f4f5"]),
            id: "BK.09",
        },
        EpdPosition {
            fen: "3rr1k1/pp3pp1/1qn2np1/8/3p4/PP1R1P2/2P1NQPP/R1B3K1 b - -",
            check: MoveCheck::BestMove(&["f6e5"]),
            id: "BK.10",
        },
        EpdPosition {
            fen: "2r1nrk1/p2q1ppp/bp1p4/n1pPp3/P1P1P3/2PBB1N1/4QPPP/R4RK1 w - -",
            check: MoveCheck::BestMove(&["f2f4"]),
            id: "BK.11",
        },
        EpdPosition {
            fen: "r3r1k1/ppqb1ppp/8/4p1NQ/8/2P5/PP3PPP/R3R1K1 b - -",
            check: MoveCheck::BestMove(&["d7f5"]),
            id: "BK.12",
        },
        EpdPosition {
            fen: "r2q1rk1/4bppp/p2p4/2pP4/3pP3/3Q4/PP1B1PPP/R3R1K1 w - -",
            check: MoveCheck::BestMove(&["b2b4"]),
            id: "BK.13",
        },
        EpdPosition {
            fen: "rnb2r1k/pp2p2p/2pp2p1/4q2n/8/b1P2N2/PPQ1BPPP/R1B1R1K1 w - -",
            check: MoveCheck::BestMove(&["d1d2", "d1e1"]),
            id: "BK.14",
        },
        EpdPosition {
            fen: "r1bqk2r/pp2bppp/2p5/3pP3/P2Q1P2/2N1B3/1PP3PP/R4RK1 b kq -",
            check: MoveCheck::BestMove(&["g4g7"]),
            id: "BK.15",
        },
        EpdPosition {
            fen: "r2qnrnk/p2b2b1/1p1p2pp/2pPpp2/1PP1P3/PRNBB3/3QNPPP/5RK1 w - -",
            check: MoveCheck::BestMove(&["d2e4"]),
            id: "BK.16",
        },
        EpdPosition {
            fen: "b2b1r1k/3R1ppp/4qP2/4p3/2p1P3/8/PPP3PP/4Q1K1 w - -",
            check: MoveCheck::BestMove(&["h7h5"]),
            id: "BK.17",
        },
        EpdPosition {
            fen: "2rq1rk1/pb1n1ppN/4p3/1pb5/3P1Pn1/P1N5/1PQ1B1PP/R1B2RK1 b - -",
            check: MoveCheck::BestMove(&["c5b3"]),
            id: "BK.18",
        },
        EpdPosition {
            fen: "r4rk1/3nppbp/bq1p1np1/2pP4/8/2N2NPP/PP2PPB1/R1BQR1K1 b - -",
            check: MoveCheck::BestMove(&["e8e4"]),
            id: "BK.19",
        },
        EpdPosition {
            fen: "r1b1k2r/ppppnppp/2n2q2/2b5/3NP3/2P1B3/PP3PPP/RN1QKB1R w KQkq -",
            check: MoveCheck::BestMove(&["g3g4"]),
            id: "BK.20",
        },
        EpdPosition {
            fen: "3r2k1/1p3ppp/2pq4/p1n5/P6P/1P6/1PB2QP1/1K2R3 b - -",
            check: MoveCheck::BestMove(&["f5h6"]),
            id: "BK.21",
        },
        EpdPosition {
            fen: "r1bqkb1r/4npp1/p1p4p/1p1pP1B1/8/1B6/PPPN1PPP/R2Q1RK1 w kq -",
            check: MoveCheck::BestMove(&["b7e4"]),
            id: "BK.22",
        },
        EpdPosition {
            fen: "r2q1rk1/1ppnbppp/p2p1nb1/3Pp3/2P1P1p1/2N2N1P/PPB1QPP1/R1BR2K1 b - -",
            check: MoveCheck::BestMove(&["f7f6"]),
            id: "BK.23",
        },
        EpdPosition {
            fen: "r1bq1rk1/pp2ppbp/2np2p1/2n5/P3PP2/N1P2N2/1PB3PP/R1B1QRK1 b - -",
            check: MoveCheck::BestMove(&["f2f4"]),
            id: "BK.24",
        },
    ];

    run_epd_suite("Bratko-Kopec", &positions, 10, 6);
}

#[test]
#[ignore = "Manual test"]
fn kaufman() {
    let positions = [
        EpdPosition {
            fen: "1rbq1rk1/p1b1nppp/1p2p3/8/1B1pN3/P2B4/1P3PPP/2RQ1R1K w - -",
            check: MoveCheck::BestMove(&["e4f6"]),
            id: "KT.01",
        },
        EpdPosition {
            fen: "3r2k1/p2r1p1p/1p2p1p1/q4n2/3P4/PQ5P/1P1RNPP1/3R2K1 b - -",
            check: MoveCheck::BestMove(&["f5d4"]),
            id: "KT.02",
        },
        EpdPosition {
            fen: "3r2k1/1p3ppp/2pq4/p1n5/P6P/1P6/1PB2QP1/1K2R3 w - -",
            check: MoveCheck::AvoidMove(&["e1d1"]),
            id: "KT.03",
        },
        EpdPosition {
            fen: "r1b1r1k1/1ppn1p1p/3pnqp1/8/p1P1P3/5P2/PbNQNBPP/1R2RB1K w - -",
            check: MoveCheck::BestMove(&["b1b2"]),
            id: "KT.04",
        },
        EpdPosition {
            fen: "2r4k/pB4bp/1p4p1/6q1/1P1n4/2N5/P4PPP/2R1Q1K1 b - -",
            check: MoveCheck::BestMove(&["g5c1"]),
            id: "KT.05",
        },
        EpdPosition {
            fen: "r5k1/3n1ppp/1p6/3p1p2/3P1B2/r3P2P/PR3PP1/2R3K1 b - -",
            check: MoveCheck::AvoidMove(&["a3a2"]),
            id: "KT.06",
        },
        EpdPosition {
            fen: "2r2rk1/1bqnbpp1/1p1ppn1p/pP6/N1P1P3/P2B1N1P/1B2QPP1/R2R2K1 b - -",
            check: MoveCheck::BestMove(&["b7e4"]),
            id: "KT.07",
        },
        EpdPosition {
            fen: "5r1k/6pp/1n2Q3/4p3/8/7P/PP4PK/R1B1q3 b - -",
            check: MoveCheck::BestMove(&["h7h6"]),
            id: "KT.08",
        },
        EpdPosition {
            fen: "r3k2r/pbn2ppp/8/1P1pP3/P1qP4/5B2/3Q1PPP/R3K2R w KQkq -",
            check: MoveCheck::BestMove(&["f3e2"]),
            id: "KT.09",
        },
        EpdPosition {
            fen: "3r2k1/ppq2pp1/4p2p/3n3P/3N2P1/2P5/PP2QP2/K2R4 b - -",
            check: MoveCheck::BestMove(&["d5c3"]),
            id: "KT.10",
        },
        EpdPosition {
            fen: "q3rn1k/2QR4/pp2pp2/8/P1P5/1P4N1/6n1/6K1 w - -",
            check: MoveCheck::BestMove(&["g3f5"]),
            id: "KT.11",
        },
        EpdPosition {
            fen: "6k1/p3q2p/1nr3pB/8/3Q1P2/6P1/PP5P/3R2K1 b - -",
            check: MoveCheck::BestMove(&["c6d6"]),
            id: "KT.12",
        },
        EpdPosition {
            fen: "1r4k1/7p/5np1/3p3n/8/2NB4/7P/3N1RK1 w - -",
            check: MoveCheck::BestMove(&["c3d5"]),
            id: "KT.13",
        },
        EpdPosition {
            fen: "1r2r1k1/p4p1p/6pB/q7/8/3Q2P1/PbP2PKP/1R3R2 w - -",
            check: MoveCheck::BestMove(&["b1b2"]),
            id: "KT.14",
        },
        EpdPosition {
            fen: "r2q1r1k/pb3p1p/2n1p2Q/5p2/8/3B2N1/PP3PPP/R3R1K1 w - -",
            check: MoveCheck::BestMove(&["d3f5"]),
            id: "KT.15",
        },
        EpdPosition {
            fen: "8/4p3/p2p4/2pP4/2P1P3/1P4k1/1P1K4/8 w - -",
            check: MoveCheck::BestMove(&["b3b4"]),
            id: "KT.16",
        },
        EpdPosition {
            fen: "1r1q1rk1/p1p2pbp/2pp1np1/6B1/4P3/2NQ4/PPP2PPP/3R1RK1 w - -",
            check: MoveCheck::BestMove(&["e4e5"]),
            id: "KT.17",
        },
        EpdPosition {
            fen: "q4rk1/1n1Qbppp/2p5/1p2p3/1P2P3/2P4P/6P1/2B1NRK1 b - -",
            check: MoveCheck::BestMove(&["a8c8"]),
            id: "KT.18",
        },
        EpdPosition {
            fen: "r2q1r1k/1b1nN2p/pp3pp1/8/Q7/PP5P/1BP2RPN/7K w - -",
            check: MoveCheck::BestMove(&["a4d7"]),
            id: "KT.19",
        },
        EpdPosition {
            fen: "8/5p2/pk2p3/4P2p/2b1pP1P/P3P2B/8/7K w - -",
            check: MoveCheck::BestMove(&["h3g4"]),
            id: "KT.20",
        },
        EpdPosition {
            fen: "8/2k5/4p3/1nb2p2/2K5/8/6B1/8 w - -",
            check: MoveCheck::BestMove(&["c4b5"]),
            id: "KT.21",
        },
        EpdPosition {
            fen: "1B1b4/7K/1p6/1k6/8/8/8/8 w - -",
            check: MoveCheck::BestMove(&["b8a7"]),
            id: "KT.22",
        },
        EpdPosition {
            fen: "rn1q1rk1/1b2bppp/1pn1p3/p2pP3/3P4/P2BBN1P/1P1N1PP1/R2Q1RK1 b - -",
            check: MoveCheck::BestMove(&["b7a6"]),
            id: "KT.23",
        },
        EpdPosition {
            fen: "8/p1ppk1p1/2n2p2/8/4B3/2P1KPP1/1P5P/8 w - -",
            check: MoveCheck::BestMove(&["e4c6"]),
            id: "KT.24",
        },
        EpdPosition {
            fen: "8/3nk3/3pp3/1B6/8/3PPP2/4K3/8 w - -",
            check: MoveCheck::BestMove(&["b5d7"]),
            id: "KT.25",
        },
    ];

    run_epd_suite("Kaufman", &positions, 10, 13);
}

#[test]
#[ignore = "Manual test"]
fn nolot() {
    let positions = [
        EpdPosition {
            fen: "r3qb1k/1b4p1/p2pr2p/3n4/Pnp1N1N1/6RP/1B3PP1/1B1QR1K1 w - -",
            check: MoveCheck::BestMove(&["g4h6"]),
            id: "Nolot.01",
        },
        EpdPosition {
            fen: "r4rk1/pp1n1p1p/1nqP2p1/2b1P1B1/4NQ2/1B3P2/PP2K2P/2R5 w - -",
            check: MoveCheck::BestMove(&["c1c5"]),
            id: "Nolot.02",
        },
        EpdPosition {
            fen: "r2qk2r/ppp1b1pp/2n1p3/3pP1n1/3P2b1/2PB1NN1/PP4PP/R1BQK2R w KQkq -",
            check: MoveCheck::BestMove(&["f3g5"]),
            id: "Nolot.03",
        },
        EpdPosition {
            fen: "r1b1kb1r/1p1n1ppp/p2ppn2/6BB/2qNP3/2N5/PPP2PPP/R2Q1RK1 w kq -",
            check: MoveCheck::BestMove(&["d4e6"]),
            id: "Nolot.04",
        },
        EpdPosition {
            fen: "r2qrb1k/1p1b2p1/p2ppn1p/8/3NP3/1BN5/PPP3QP/1K3RR1 w - -",
            check: MoveCheck::BestMove(&["e4e5"]),
            id: "Nolot.05",
        },
        EpdPosition {
            fen: "rnbqk2r/1p3ppp/p7/1NpPp3/QPP1P1n1/P4N2/4KbPP/R1B2B1R b kq -",
            check: MoveCheck::BestMove(&["a6b5"]),
            id: "Nolot.06",
        },
        EpdPosition {
            fen: "1r1bk2r/2R2ppp/p3p3/1b2P2q/4QP2/4N3/1B4PP/3R2K1 w k -",
            check: MoveCheck::BestMove(&["c7d8"]),
            id: "Nolot.07",
        },
        EpdPosition {
            fen: "r3rbk1/ppq2ppp/2b1pB2/8/6Q1/1P1B3P/P1P2PP1/R2R2K1 w - -",
            check: MoveCheck::BestMove(&["d3h7"]),
            id: "Nolot.08",
        },
        EpdPosition {
            fen: "r4r1k/4bppb/2n1p2p/p1n1P3/1p1p1BNP/3P1NP1/qP2QPB1/2RR2K1 w - -",
            check: MoveCheck::BestMove(&["f3g5"]),
            id: "Nolot.09",
        },
        EpdPosition {
            fen: "r1b2rk1/1p1nbppp/pq1p4/3B4/P2NP3/2N1p3/1PP3PP/R2Q1R1K w - -",
            check: MoveCheck::BestMove(&["f1f7"]),
            id: "Nolot.10",
        },
        EpdPosition {
            fen: "r1b3k1/p2p1nP1/2pqr1Rp/1p2p2P/2B1PnQ1/1P6/P1PP4/1K4R1 w - -",
            check: MoveCheck::BestMove(&["g6h6"]),
            id: "Nolot.11",
        },
    ];

    run_epd_suite("Nolot", &positions, 10, 4);
}
