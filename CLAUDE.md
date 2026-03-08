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

## Documentation

Detailed docs live in `docs/`. Keep them in sync when making changes:

- `docs/architecture.md` — Module overview, component wiring, core types, move encoding
- `docs/search.md` — Search algorithm, pruning, reductions, extensions, move ordering
- `docs/evaluation.md` — Tapered eval, material values, piece-square tables
- `docs/uci.md` — Supported UCI commands and options

When adding or changing a search technique, evaluation term, or UCI command, update the corresponding doc file and the feature list in `README.md`.

## Architecture

Single-threaded engine. Entry point: `src/main.rs` creates core components and runs the UCI loop.

### Core Modules

| Module | Path | Purpose |
|--------|------|---------|
| **uci** | `src/uci.rs` | UCI protocol handler, main input loop |
| **search** | `src/search.rs` | Negamax with alpha-beta, iterative deepening, pruning |
| **evaluate** | `src/evaluate.rs` | Tapered eval with piece-square tables |
| **evaluate/transposition** | `src/evaluate/transposition.rs` | Transposition table (hash, depth, score, best move, node type) |
| **position** | `src/position.rs` | Board state, do/undo move, Zobrist hashing |
| **movegen** | `src/movegen.rs` | Legal move generation |
| **bitboards** | `src/bitboards.rs` | Magic bitboards, LERF mapping |
| **time** | `src/time.rs` | Time management for search cutoff |
| **defs** | `src/defs.rs` | Core types: `Bitboard`, `Piece`, `Side`, `Square` |
| **hash** | `src/hash.rs` | Zobrist key generation |
| **benchmark** | `src/benchmark.rs` | 46-position bench suite |
| **misc** | `src/misc.rs` | Bit manipulation utilities |

### Module Organization

Each module may have sub-files:
- `defs.rs` — types and constants for that module
- `tables.rs` — lookup tables (e.g. piece-square tables in evaluate)
- `test.rs` — unit tests

### Key Types (src/defs.rs)

All core types are `usize` aliases: `Bitboard = u64`, `Piece`, `Side`, `Square = usize`. Pieces are encoded as `side * 8 + piece_type`. Sides: WHITE=0, BLACK=1.

### Component Wiring

`Bitboards` is shared via `Rc` into `Movegen` and `Position`. `Search` owns `Position`, `Movegen`, `Eval`, and `TimeManager`.

## Search Techniques

Current search features (see `docs/search.md` for details):
- Negamax with alpha-beta, iterative deepening, aspiration windows, PVS
- Null move pruning, reverse futility pruning, razoring, futility pruning
- Late move pruning (LMP), late move reductions (LMR)
- SEE pruning, delta pruning in quiescence
- Check extensions
- Move ordering: TT move > MVV-LVA captures > killer moves > history heuristic
- Quiescence search (captures, en passant, promotions)

## Conventions

- Rust 2021 edition, single dependency: `arrayvec`
- `rustfmt.toml`: `max_width = 120`
- Run `cargo fmt` after changes to format code
- Run `cargo clippy` before committing
- Sub-module definitions go in `defs.rs` files, not inline
