use super::*;
use crate::movegen::defs::Move;
use crate::search::move_picker::MovePicker;

/// Deterministic xorshift generator so the fuzz walk is reproducible.
struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn sorted_codes(moves: impl Iterator<Item = Move>) -> Vec<String> {
    let mut codes: Vec<String> = moves.map(|mv| format!("{:?}", mv)).collect();
    codes.sort();
    codes
}

/// Drain a MovePicker seeded with the given TT move and killers.
fn picker_moves(search: &Search, tt_move: Move, killers: [Move; 2], countermove: Move) -> Vec<String> {
    let history = [[0i32; 64]; 64];
    let capture_history = [[[0i32; 7]; 64]; 7];
    let check_mask = search.movegen.check_mask(&search.position);
    let mut picker = MovePicker::new(tt_move, killers, countermove, check_mask);
    let mut yielded = Vec::new();
    while let Some(mv) = picker.next(
        &search.position,
        &search.movegen,
        &history,
        None,
        None,
        &capture_history,
    ) {
        yielded.push(mv);
    }
    sorted_codes(yielded.into_iter())
}

#[test]
fn castling_killer_rejected_when_in_check() {
    // White king e1 is checked by the b4 bishop; f1/g1 are not attacked, so a naive
    // path test would wrongly accept O-O from a sibling node's killer slot.
    let mut search = make_search();
    search.position.set("4k3/8/8/8/1b6/8/8/4K2R w K - 0 1".to_string());
    let castle = Move::make(4, 7, 0, crate::movegen::defs::MoveTypes::CASTLING);
    let legal = sorted_codes(search.movegen.legal_moves(&search.position).into_iter());
    let picked = picker_moves(&search, castle, [castle, Move::none()], Move::none());
    assert_eq!(picked, legal);
}

/// Random games played from each start position by the fuzz walk.
const GAMES_PER_FEN: usize = 20;

#[test]
fn move_picker_yields_exactly_legal_moves() {
    let fens = [
        FEN_START_POSITION,
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    ];
    let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15);
    // Pool of moves seen anywhere, used as foreign TT moves / killers.
    let mut pool: Vec<Move> = Vec::new();

    let mut search = make_search();
    for game in 0..GAMES_PER_FEN * fens.len() {
        search.position.set(fens[game % fens.len()].to_string());
        for _ in 0..400 {
            let legal = search.movegen.legal_moves(&search.position);
            if legal.is_empty() || search.position.states.last().unwrap().rule50 >= 100 {
                break;
            }
            pool.extend(legal.iter().copied());
            let pick = |rng: &mut Xorshift, pool: &Vec<Move>| pool[(rng.next() % pool.len() as u64) as usize];
            for _ in 0..4 {
                let tt_move = pick(&mut rng, &pool);
                let killers = [pick(&mut rng, &pool), pick(&mut rng, &pool)];
                let countermove = pick(&mut rng, &pool);
                let picked = picker_moves(&search, tt_move, killers, countermove);
                let expected = sorted_codes(legal.iter().copied());
                assert_eq!(picked, expected, "picker mismatch at {}", search.position.fen());
            }
            let mv = legal[(rng.next() % legal.len() as u64) as usize];
            search.position.do_move(mv);
        }
    }
}
