//! Oxid' chess engine.
//!
//! The engine core (board, movegen, search, NNUE) builds for every target.
//! Subsystems that need a terminal or a real filesystem sit behind the `host`
//! feature; Syzygy tablebases sit behind `tablebase` because pyrrhic-rs links C
//! sources. A browser build is therefore:
//!
//! ```text
//! cargo build --release --target wasm32-unknown-unknown --no-default-features --features wasm
//! ```

pub mod bitboards;
pub mod book;
pub mod clock;
pub mod defs;
pub mod evaluate;
pub mod hash;
pub mod misc;
pub mod movegen;
pub mod nnue;
pub mod position;
pub mod search;
pub mod time;

#[cfg(feature = "host")]
pub mod benchmark;
#[cfg(feature = "host")]
pub mod datagen;
#[cfg(feature = "host")]
pub mod eret;
#[cfg(feature = "host")]
pub mod uci;

#[cfg(feature = "tablebase")]
pub mod tablebase;

#[cfg(feature = "wasm")]
pub mod wasm;

/// Filename of the net compiled into the native binary.
#[cfg(not(target_arch = "wasm32"))]
pub const DEFAULT_EVAL_FILE: &str = "nn-b9f535fc9a86.nnue";

/// The net is embedded for native builds only. wasm callers fetch the net over
/// the network and hand the bytes to [`wasm::init`], which keeps the 3 MB of
/// weights out of the `.wasm` blob.
#[cfg(not(target_arch = "wasm32"))]
pub const EMBEDDED_NET: &[u8] = include_bytes!(concat!("../nets/", "nn-b9f535fc9a86.nnue"));
