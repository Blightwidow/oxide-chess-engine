use std::rc::Rc;

use oxid::{
    bitboards::Bitboards, evaluate::Eval, hash::Hasher, movegen::Movegen, nnue::NnueEval, position::Position,
    search::Search, uci::Uci, DEFAULT_EVAL_FILE, EMBEDDED_NET,
};

fn main() {
    println!("Oxid' v{} by Theo Dammaretz", env!("CARGO_PKG_VERSION"));

    let bitboards = Rc::new(Bitboards::new());
    #[cfg(feature = "tablebase")]
    oxid::tablebase::init_attack_tables(&bitboards);
    let hasher = Rc::new(Hasher::new());
    let movegen = Movegen::new(Rc::clone(&bitboards));
    let position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));
    let eval = Eval::new();
    let nnue = match NnueEval::from_bytes(EMBEDDED_NET) {
        Some(net) => {
            println!(
                "info string NNUE {} loaded ({} bytes)",
                DEFAULT_EVAL_FILE,
                EMBEDDED_NET.len()
            );
            net
        }
        None => {
            println!("info string WARNING: embedded NNUE net is invalid, using zero weights");
            println!("info string Load a compatible net with: setoption name EvalFile value <path>");
            NnueEval::empty()
        }
    };
    let mut search = Search::new(position, movegen, eval, nnue);

    Uci::main_loop(&mut search);
}
