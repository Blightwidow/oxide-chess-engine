# Changelog

All notable changes to Oxide are documented in this file.

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
