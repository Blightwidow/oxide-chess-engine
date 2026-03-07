# Oxide Chess Bot

UCI-compatible chess engine written in Rust.

## Build & Run

```bash
cargo build -r              # Release build
cargo run -r -- <command>   # Run directly (e.g. `cargo run -r -- bench`)
cargo test                  # Run tests
cargo clippy                # Lint
```

### SPRT Testing (engine strength regression)

```bash
cargo build -r --target-dir=base   # Build baseline
# Make changes...
cargo build -r                     # Build new version
./bin/fastchess -engine cmd=./target/release/chessbot name=oxide \
  -engine cmd=./base/release/chessbot name=engine_BASE \
  -each tc=8+0.08 -rounds 15000 -repeat -concurrency 6 -recover \
  -sprt elo0=0 elo1=5 alpha=0.05 beta=0.05
```

## Architecture

Single-threaded engine. Entry point: `src/main.rs` creates core components and runs the UCI loop.

### Core Modules

| Module | Path | Purpose |
|--------|------|---------|
| **uci** | `src/uci.rs` | UCI protocol handler, main input loop |
| **search** | `src/search.rs` | Negamax with alpha-beta pruning, iterative deepening |
| **evaluate** | `src/evaluate.rs` | Tapered eval with piece-square tables, transposition table |
| **position** | `src/position.rs` | Board state, do/undo move |
| **movegen** | `src/movegen.rs` | Legal move generation |
| **bitboards** | `src/bitboards.rs` | Magic bitboards, LERF mapping |
| **time** | `src/time.rs` | Time management for search cutoff |
| **defs** | `src/defs.rs` | Core types: `Bitboard`, `Piece`, `Side`, `Square` |
| **hash** | `src/hash.rs` | Hasher (stub) |
| **benchmark** | `src/benchmark.rs` | Bench positions for `bench` command |

### Module Organization

Each module may have sub-files:
- `defs.rs` — types and constants for that module
- `tables.rs` — lookup tables (e.g. piece-square tables in evaluate)
- `test.rs` — unit tests

### Key Types (src/defs.rs)

All core types are `usize` aliases: `Bitboard = u64`, `Piece`, `Side`, `Square = usize`. Pieces are encoded as `side * 8 + piece_type`. Sides: WHITE=0, BLACK=1.

### Component Wiring

`Bitboards` is shared via `Rc` into `Movegen` and `Position`. `Search` owns `Position`, `Movegen`, `Eval`, and `TimeManager`.

## Conventions

- Rust 2021 edition, single dependency: `arrayvec`
- `rustfmt.toml`: `max_width = 120`
- Run `cargo clippy` before committing
- Sub-module definitions go in `defs.rs` files, not inline
