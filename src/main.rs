mod benchmark;
mod bitboards;
mod defs;
mod evaluate;
mod hash;
mod misc;
mod movegen;
mod nnue;
mod position;
mod search;
mod time;
mod uci;

use std::rc::Rc;

use crate::{
    bitboards::Bitboards, evaluate::Eval, hash::Hasher, movegen::Movegen, nnue::NnueEval, position::Position,
    search::Search, uci::Uci,
};

pub const DEFAULT_EVAL_FILE: &str = "nn-8808c22a8203.nnue";
pub const EMBEDDED_NET: &[u8] = include_bytes!(concat!("../nets/", "nn-8808c22a8203.nnue"));

fn main() {
    println!("Oxide v1.0.0 by Theo Dammaretz");

    let bitboards = Rc::new(Bitboards::new());
    let hasher = Rc::new(Hasher::new());
    let movegen = Movegen::new(Rc::clone(&bitboards));
    let position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));
    let eval = Eval::new();
    let nnue = NnueEval::from_bytes(EMBEDDED_NET).expect("embedded NNUE net is invalid");
    println!(
        "info string NNUE {} loaded ({} bytes)",
        DEFAULT_EVAL_FILE,
        EMBEDDED_NET.len()
    );
    let mut search = Search::new(position, movegen, eval, nnue);

    Uci::main_loop(&mut search);
}
