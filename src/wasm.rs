//! wasm-bindgen shim for browser hosts.
//!
//! The net is *not* embedded in the `.wasm` blob — the page fetches
//! `nets/*.nnue` itself and passes the bytes to [`init`], which keeps the
//! module small enough to stream.
//!
//! ```js
//! import initWasm, { init } from "./oxid.js";
//! await initWasm();
//! const netBytes = new Uint8Array(await (await fetch("nn-8808c22a8203.nnue")).arrayBuffer());
//! const engine = init(netBytes);
//! engine.legal_moves("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
//! engine.best_move("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 1000);
//! ```

use std::rc::Rc;

use wasm_bindgen::prelude::*;

use crate::{
    bitboards::Bitboards,
    evaluate::Eval,
    hash::Hasher,
    movegen::Movegen,
    nnue::NnueEval,
    position::Position,
    search::{defs::SearchLimits, Search},
};

/// Opaque engine handle held by JavaScript. Owns the position, movegen tables,
/// transposition table and NNUE weights, so a page creates one and reuses it
/// across moves rather than paying table setup per call.
#[wasm_bindgen]
pub struct Engine {
    search: Search,
}

/// `Position::set` splits on spaces and asserts on fewer than two fields, so
/// reject malformed input here rather than letting a JS caller trip an assert.
fn is_well_formed(fen: &str) -> bool {
    fen.split(' ').filter(|field| !field.is_empty()).count() >= 2
}

#[wasm_bindgen]
impl Engine {
    /// Legal moves in the given position as UCI strings (e.g. `"e2e4"`).
    /// Returns an empty array for a malformed FEN.
    pub fn legal_moves(&mut self, fen: &str) -> Vec<String> {
        if !is_well_formed(fen) {
            return Vec::new();
        }
        self.search.position.set(fen.to_string());
        self.search.nnue.refresh(&self.search.position);
        self.search
            .movegen
            .legal_moves(&self.search.position)
            .iter()
            .map(|mv| format!("{:?}", mv))
            .collect()
    }

    /// Search the given position for `movetime_ms` and return the best move as a
    /// UCI string. Returns an empty string when the position is terminal
    /// (checkmate or stalemate) or the FEN is malformed.
    pub fn best_move(&mut self, fen: &str, movetime_ms: u32) -> String {
        if !is_well_formed(fen) {
            return String::new();
        }
        self.search.position.set(fen.to_string());
        self.search.nnue.refresh(&self.search.position);

        let limits = SearchLimits {
            movetime: movetime_ms as usize,
            ..SearchLimits::default()
        };

        match self.search.run_and_return(limits) {
            Some((best_move, _score)) => format!("{:?}", best_move),
            None => String::new(),
        }
    }
}

/// Build an engine from raw NNUE bytes. Falls back to zero weights (still plays
/// legal moves) if the net is not compatible with this build's architecture.
#[wasm_bindgen]
pub fn init(net_bytes: &[u8]) -> Engine {
    let bitboards = Rc::new(Bitboards::new());
    let hasher = Rc::new(Hasher::new());
    let movegen = Movegen::new(Rc::clone(&bitboards));
    let position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));
    let nnue = NnueEval::from_bytes(net_bytes).unwrap_or_else(NnueEval::empty);

    let mut search = Search::new(position, movegen, Eval::new(), nnue);
    // No stdout on wasm32-unknown-unknown, so UCI info lines are wasted work.
    search.silent = true;

    Engine { search }
}
