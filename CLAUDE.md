# Oxid' Chess Bot

UCI-compatible chess engine written in Rust.

## Build & Run

```bash
cargo build -r                                          # Release build (native CPU by default via .cargo/config.toml)
RUSTFLAGS="" cargo build -r                             # Release build (generic x86-64, for distribution)
cargo run -r -- <command>                               # Run directly (e.g. `cargo run -r -- bench`)
cargo test                                              # Run tests
cargo clippy                                            # Lint
```

### Quick Performance Check

When changing search or evaluation code, compare nodes/second before and after:

```bash
cargo build -r --target-dir=base   # Build baseline before changes
# Make changes...
cargo build -r                     # Build new version
printf "bench 16 1 11 5\nquit\n" | ./base/release/oxid 2>&1    # Baseline (5 positions, depth 11)
printf "bench 16 1 11 5\nquit\n" | ./target/release/oxid 2>&1  # After changes
```

Compare the `Nodes/second` output. Usage: `bench [hash_mb] [threads] [depth] [count]`.

### SPRT Testing (engine strength regression)

```bash
cargo build -r --target-dir=base   # Build baseline
# Make changes...
cargo build -r                     # Build new version
./bin/fastchess -engine cmd=./target/release/chessbot name=oxid \
  -engine cmd=./base/release/chessbot name=engine_BASE \
  -each tc=8+0.08 -rounds 15000 -repeat -concurrency 6 -recover \
  -sprt elo0=0 elo1=5 alpha=0.05 beta=0.05
```

## Documentation

Detailed docs live in `docs/`. Keep them in sync when making changes:

- `docs/architecture.md` — Module overview, component wiring, core types, move encoding
- `docs/search.md` — Search algorithm, pruning, reductions, extensions, move ordering
- `docs/evaluation.md` — NNUE architecture, handcrafted eval fallback
- `docs/uci.md` — Supported UCI commands and options
- `docs/time.md` — Time allocation, soft/hard limits, adaptive scaling signals

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
| **datagen** | `src/datagen.rs` | Self-play training data generation |
| **tablebase** | `src/tablebase.rs` | Syzygy endgame tablebase probing (via pyrrhic-rs) |
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
- Probcut, SEE pruning, delta pruning in quiescence
- Singular extensions, check extensions, IIR
- Move ordering: TT move > captures (MVV-LVA + capture history) > killers/countermove > quiets (history + continuation history)
- Continuation history (1-ply + 2-ply), capture history, correction history
- Quiescence search (captures, en passant, promotions)
- Syzygy endgame tablebases: root DTZ probe, in-search WDL probe

## Training Data Generation

Self-play data generation for NNUE training:

```bash
# Via UCI command: datagen <depth> <num_games> <output_path>
printf "datagen 8 10000 data/selfplay.txt\n" | ./target/release/oxid

# Via pipeline script (builds, generates, converts to binpack)
./scripts/generate_data.sh 8 10000 selfplay

# Convert plain text to binpack manually
clang++ -O2 -std=c++20 -o tools/plain2binpack tools/plain2binpack.cpp
./tools/plain2binpack data/selfplay.txt data/selfplay.binpack
```

Output format: `FEN;UCI_MOVE;SCORE;PLY;WDL` (semicolon-delimited, one position per line). Games use 8 random opening plies, win adjudication (5 consecutive |score| >= 3000cp), draw adjudication (10 consecutive |score| <= 5cp), and 400-ply max.

## NNUE Training (Rust/bullet)

Training uses [bullet](https://github.com/jw1912/bullet) (Rust NNUE trainer by jw1912). Place `.binpack` files in `training/data/`, then:

```bash
cd training
cargo run --release --features cpu --no-default-features --bin train
```

Resume from checkpoint or train longer:

```bash
cargo run --release --features cpu --no-default-features --bin train -- --resume checkpoints/oxid-60 --end 100
```

Convert checkpoint to `.nnue`:

```bash
cargo run --release --bin convert -- checkpoints/oxid-100/quantised.bin ../nets/oxide-384-sb100.nnue
```

Batch-convert all checkpoints:

```bash
./scripts/convert_checkpoints.sh
```

Architecture: `768×8 → 384 (SCReLU) → concat perspectives (768) → 32 (SCReLU) → 1`

> **Note (2026-04):** A PyTorch-based trainer was attempted but abandoned — all nets trained with it were consistently 300+ Elo weaker than bullet-trained equivalents despite identical architecture and quantization. Root cause was training quality (LR schedule, loss convergence) rather than any code bug. Bullet remains the only supported trainer.
>
> **Note (2026-04-23):** Metal GPU backend (`Blightwidow/bullet-metal` fork) was attempted to speed up training on M2 Pro. Training ran (~5min/superbatch vs ~5min/sb CPU — no speedup) but produced weaker nets (-25 to -105 Elo vs CPU baseline). Determinism test with fixed seed confirmed the backend is **non-deterministic** (two runs with same seed produce different weights at byte 6145 of `quantised.bin`), likely from atomic reductions in backward pass. Gradient noise from the backend swamps training signal. Bullet is pinned to `feab6443` (CPU) until a deterministic GPU path exists (CUDA works; upstream dropped CPU in newer revisions).

## Development Notes

- **Large heap arrays**: Never use `Box::new(large_array)` for arrays >100KB — it creates the value on the stack first, causing stack overflow. Use `alloc_zeroed` + `Box::from_raw` (see `new_conthist()` in `search.rs` for the pattern).
- **Move loop borrows**: The move loop uses a `loop { let mv = { ... }; ... }` pattern so that shared borrows of history tables (for `MovePicker::next`) are released before mutable borrows (for history updates on beta cutoff). Don't refactor this back to `while let`.

## Conventions

- Rust 2021 edition, dependencies: `arrayvec`, `pyrrhic-rs`
- `rustfmt.toml`: `max_width = 120`
- Run `cargo fmt` after changes to format code
- Run `cargo clippy` before committing
- Sub-module definitions go in `defs.rs` files, not inline
- Revert commits with `GIT_EDITOR=true git revert <ref> --no-verify` (avoids interactive editor prompt)
