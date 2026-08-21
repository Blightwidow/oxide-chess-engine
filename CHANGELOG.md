# Changelog

All notable changes to Oxid' are documented in this file.

## v2.0.0

### Breaking

- NNUE net format bumped to v3 (384 hidden neurons). v2 nets no longer load — the engine falls back
  to zero weights instead of misreading them, so any external net passed via `EvalFile` must be
  retrained or reconverted.
- The `oxid` binary now declares `required-features = ["host"]`. A bare `cargo build` with
  `--no-default-features` builds the library only.

### NNUE Evaluation

- Upgraded architecture from 256 to 384 hidden neurons: `768x8 -> 384 (SCReLU) -> 768 -> 32 -> 1`, net format bumped to v3
- New embedded net: `nn-b9f535fc9a86.nnue`, trained on 49.78 GB of Stockfish `test80-2023` binpacks (BT4-relabeled), 800 superbatches
- SIMD accumulator updates and SCReLU: AVX2 on x86-64, NEON on aarch64, scalar fallback elsewhere
- Graceful fallback to zero weights when an embedded or loaded net has an incompatible version

### Search

- Aspiration windows shrunk to +/-15 cp with exponential widening (window opens fully at delta >= 1000)
- Singular extensions: double extension when the verification score falls well below `se_beta`, negative extension when the TT move is not singular, plus a ply cap

### Time Management

- Adaptive soft-limit scaling with three new signals at depth >= 6, combined multiplicatively with best-move stability and clamped to [0.3, 2.0]:
  - Node TM: stop early (0.6x) when the best move takes >90% of root nodes, extend (1.3x) below 50%
  - Score stability: extend (1.4x) on >30 cp drops between iterations, shrink (0.9x) when stable
  - Eval complexity: extend (1.2x) when the top-2 spread is <20 cp, shrink (0.7x) above 100 cp
- SPRT [0, 5] passed at +8.5 Elo (10250 games, LLR 2.97)

### Endgames

- Syzygy endgame tablebase probing via `pyrrhic-rs`: root DTZ probe for move selection, in-search WDL probe with TT caching, 3-7 piece tables
- New `SyzygyPath` UCI option

### Opening Book

- Polyglot book support through the new `BookFile` UCI option: root probe before search, weighted-random selection over matching entries, validated against the legal move list
- Separate Polyglot Zobrist hash (781-key table) with en-passant filtering

### WebAssembly

- Engine core builds for `wasm32-unknown-unknown`; crate split into lib plus a thin binary
- Feature gating: `host` (uci/benchmark/eret/datagen), `tablebase` (links C, native only), `wasm` (the wasm-bindgen shim)
- Clock reads moved to `src/clock.rs` — `std` natively, `web-time` on wasm
- JS API: `init(netBytes)` -> handle, `legal_moves(fen)`, `best_move(fen, movetimeMs)`; module is ~156 KB because the net is fetched by the page

### Tooling

- Self-play data generation: `datagen` UCI command, `scripts/generate_data.sh` pipeline, `tools/plain2binpack` converter
- ERET (Eigenmann Rapid Engine Test) command for fast strength iteration: `eret [seconds]` or `eret nodes <n>`
- NNUE training moved to Rust [bullet](https://github.com/jw1912/bullet) with its first-party Metal backend (~30s/superbatch on Apple Silicon, ~10x the old CPU path)
- `scripts/promote_net.sh`, `scripts/convert_checkpoints.sh`, `scripts/validate_nets.sh`, `scripts/sprt_all_nets.sh` for net management and testing

### Strength

- Estimated strength: ~2765 Elo, bracketed by SPRT anchor matches at tc=8+0.08:
  - +66.9 +/- 27.1 Elo vs Stash v21 (CCRL blitz 2714), H1 accepted after 526 games
  - -19.5 +/- 18.1 Elo vs Stash v22 (~2770), H0 accepted after 944 games
- `scripts/build_stash.sh` builds the Stash anchors natively on arm64 macOS

## v1.3.0

### Search

- Continuation history (1-ply + 2-ply) for quiet move ordering
- Capture history for capture move ordering, with gravity updates and malus on beta cutoffs
- Quiet moves scored by main history + continuation history; captures by MVV-LVA + capture history
- SEE extracted as a free function

### Performance

- SPRT neutral at tc=8+0.08 — the ordering improvement offsets the table overhead
- Bench: -5% nodes at depth 13 vs v1.2.0

## v1.2.0

### Search

- Fully integrated MovePicker with staged move generation and lazy legality checking
- Added Probcut: shallow verification search at depth >= 5 with beta + 200 margin and SEE >= 0 filter
- Granular history-based scaling for LMR, LMP, and futility thresholds
- Move ordering refined: TT move > good captures (SEE+) > killers > countermove > quiets (history + continuation) > bad captures

### Testing

- Restructured search tests into dedicated test modules: endgame, EPD suites, evaluation, opening, perft, pitfalls, tactics
- Enabled EPD test suites for position analysis

### Benchmark

- Added smaller benchmark configuration for quick performance checks

### Performance

- SPRT: +22.8 Elo over v1.1.1
- Estimated strength: ~2625 Elo

## v1.1.1

### Time Management

- Capped per-move time allocation to 30% of remaining clock to avoid time losses in fast time controls
- Added hard stop checks inside aspiration window loop and singular extension to prevent overruns
- Reduced time check interval from 2048 to 1024 nodes for tighter cutoff granularity

### Performance

- SPRT: +45 Elo over v1.1.0 against Stash

## v1.1.0

### NNUE Evaluation

- Upgraded to king-bucketed NNUE architecture (8 buckets by rank, horizontal mirroring for files e-h)
- Total feature transformer input size: 8 × 768 = 6144 features per perspective
- Incremental accumulator updates with per-perspective bucket-change refresh (full refresh when king changes bucket)
- New embedded net: `nn-8808c22a8203.nnue` (net version 2)

### Search

- Added `improving` heuristic: tracks whether static eval is rising across plies to modulate pruning aggressiveness
- Reverse futility pruning margin now scales with `improving` flag (tighter when improving)
- Late move pruning threshold adjusted based on `improving` status
- Null move pruning uses deeper reduction (depth/3 + 4 base, +1 when not improving)

### Training

- Trainer updated for bucketed architecture with per-bucket feature weights

### Performance

- SPRT: pending — estimated ~2400 Elo (on par with v1.0.1)

## v1.0.1

### Search

- History malus: on quiet beta cutoff, penalize all previously tried quiet moves with -depth² gravity bonus
- Staged move generation scaffolding (`generate_captures`, `generate_quiets`, `is_pseudo_legal`) for future MovePicker integration
- ArrayVec for scored moves in root search, alpha-beta, and quiescence — eliminates heap allocations in the hot loop

### Transposition Table

- Age-based replacement: generation counter incremented per search, stale entries always replaced regardless of depth

### Performance

- SPRT: +19.77 Elo ± 12.99 (1812 games, LLR 2.90) over v1.0.0
- Updated estimated strength to ~2400 Elo

## v1.0.0

### NNUE Evaluation

- Replaced handcrafted evaluation with NNUE (768->256x2->32->1 SCReLU architecture, integer quantized)
- Embedded net in binary via `include_bytes!` — single self-contained executable, no external files needed
- Incremental NNUE accumulator updates (activate/deactivate on do/undo move) instead of full refresh per position
- Pre-computed SCReLU activations and transposed L1 weights for cache-friendly forward pass
- SHA256-based net naming (`nn-{hash12}.nnue`) for reproducibility
- Runtime net loading via `setoption name EvalFile` for SPRT testing without recompiling
- Removed tapered handcrafted eval: piece-square tables, pawn structure (doubled/isolated/passed), bishop pair bonus, rook on open file bonus, game phase interpolation

### Search

- Correction history: tracks static eval error keyed by pawn hash to improve pruning decisions
- Mate score adjustment for TT storage (ply-relative to root-relative conversion)
- Incremental NNUE-aware move wrappers (`do_move_nnue`, `undo_move_nnue`) for all search paths
- Always complete at least depth 1 before checking soft time limit
- Perft no longer requires NNUE weights (runs before search state reset)
- Move ordering now uses dedicated SEE piece values instead of handcrafted middlegame values
- Added documentation comments throughout the search module

### Position

- Added incremental pawn hash (Zobrist) for correction history indexing
- Added `display()` for ASCII board rendering (used by `eval` command)

### Training Infrastructure

- Added Bullet-based NNUE trainer (`training/`) with Chess768 input features and SCReLU activations
- Checkpoint-to-NNUE converter (`training/src/bin/convert.rs`) wrapping quantized weights with OXNN header
- `scripts/convert_checkpoints.sh` — batch-converts training checkpoints to `.nnue` files
- `scripts/promote_net.sh` — promotes a candidate net as the new embedded default (updates source, gitignore, stages git changes)
- `scripts/sprt_all_nets.sh` — batch SPRT testing of candidate nets against the current default
- Only the active net is tracked in git; all candidates are gitignored

### UCI

- Added `eval` command — displays the board and NNUE evaluation from both perspectives
- Added `bench_perft` command — runs perft suites across multiple positions with aggregate stats
- Added `EvalFile` UCI option for runtime net loading
- Changed default hash size from 128 MB to 16 MB

### Build & CI

- Release profile: LTO enabled, single codegen unit for maximum performance
- Native CPU targeting via `.cargo/config.toml` (`target-cpu=native`)
- GitHub Actions release workflow: builds for x86_64/aarch64 Linux, macOS, and x86_64 Windows on tag push

### Testing

- Added unit tests for core types, move encoding, position do/undo, Zobrist consistency, transposition table, NNUE features, and network serialization
- Search tests use the embedded net — no longer skip when the net file is missing

### Project

- Split README into user-facing README and developer-focused CONTRIBUTING.md
- Added openings book (`data/openings.pgn`) for SPRT testing

## v0.2.0 — 2025-03-07

Initial tagged release with handcrafted evaluation.

- Magic bitboards with LERF mapping
- Negamax with alpha-beta, iterative deepening, aspiration windows, PVS
- Null move pruning, reverse futility pruning, razoring, futility pruning
- Late move pruning, late move reductions, SEE pruning, delta pruning
- Check extensions
- Move ordering: TT move, MVV-LVA captures, killer moves, history heuristic
- Transposition table with depth-preferred replacement
- Quiescence search (captures, en passant, promotions)
- Tapered evaluation with piece-square tables
- Pawn structure, bishop pair, rook on open/semi-open file
- Time management with soft/hard limits
- UCI protocol support
- 46-position benchmark suite
