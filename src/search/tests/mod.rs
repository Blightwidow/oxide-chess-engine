#[cfg(test)]
mod endgame;
#[cfg(test)]
mod evaluation;
#[cfg(test)]
mod opening;
#[cfg(test)]
mod perft;
#[cfg(test)]
mod pitfalls;
#[cfg(test)]
mod tactics;

#[cfg(test)]
use std::rc::Rc;

#[cfg(test)]
use crate::{
    bitboards::Bitboards,
    evaluate::Eval,
    hash::Hasher,
    movegen::Movegen,
    nnue::NnueEval,
    position::Position,
    search::{
        defs::{FEN_START_POSITION, VALUE_MATE},
        Search,
    },
};

#[cfg(test)]
fn make_search() -> Search {
    let bitboards = Rc::new(Bitboards::new());
    let hasher = Rc::new(Hasher::new());
    let movegen = Movegen::new(Rc::clone(&bitboards));
    let position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));
    let nnue = NnueEval::from_bytes(crate::EMBEDDED_NET).unwrap_or_else(|| NnueEval::zero());
    Search::new(position, movegen, Eval::new(), nnue)
}

#[cfg(test)]
fn search_position(fen: &str, depth: u8) -> (String, i16) {
    let mut search = make_search();
    search.position.set(fen.to_string());
    search.nnue.refresh(&search.position);
    let (mv, score) = search.search(depth).expect("Expected a move");
    (format!("{:?}", mv), score)
}

#[cfg(test)]
fn is_mate_score(score: i16) -> bool {
    score.abs() > VALUE_MATE - 100
}

#[cfg(test)]
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

#[cfg(test)]
fn is_draw_score(score: i16) -> bool {
    score.abs() < 15
}

#[cfg(test)]
fn search_returns_no_move(fen: &str, depth: u8) -> bool {
    let mut search = make_search();
    search.position.set(fen.to_string());
    search.nnue.refresh(&search.position);
    search.search(depth).is_none()
}
