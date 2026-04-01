# Contributing

## Build & Run

```bash
cargo build -r              # Release build
cargo run -r -- <command>   # Run directly
cargo test                  # Unit tests
cargo clippy                # Lint
cargo fmt                   # Format
```

## Testing

```bash
cargo test                  # All tests (fast, runs on every change)
cargo test -- --ignored     # Include slow/ignored tests
```

Some tests are marked `#[ignore]` because they take significantly longer to run (minutes rather than seconds). These should be run explicitly before submitting changes that affect search or evaluation:

```bash
cargo test bratko_kopec -- --ignored --nocapture
cargo test kaufman -- --ignored --nocapture
cargo test nolot -- --ignored --nocapture
```

- **Bratko-Kopec** (24 positions) — Classic positional/tactical test suite. Checks `bm` (best move) at depth 10.
- **Kaufman** (25 positions) — Mix of tactics and endgames. Checks `bm` and `am` (avoid move) at depth 10.
- **Nolot** (11 positions) — Sharp tactical positions requiring deep calculation. Checks `bm` at depth 10.

All use a minimum pass threshold (~50%) to catch regressions. Run these when modifying search heuristics, pruning, or evaluation.

### SPRT Testing (strength regression)

Used to verify that a change doesn't regress engine strength:

```bash
cargo build -r --target-dir=base   # Build baseline
# Make changes...
cargo build -r                     # Build new version
./bin/fastchess \
    -engine cmd=./target/release/oxid name="vX.X.X" \
    -engine cmd=./base/release/oxid name="vY.Y.Y" \
    -pgnout file="./games/vX.X.X-vY.Y.Y.pgn" \
    -openings file=./data/openings.pgn format=pgn order=random \
    -each tc=8+0.08 \
    -rounds 5000 -repeat \
    -concurrency 8 \
    -recover \
    -sprt elo0=0 elo1=10 alpha=0.05 beta=0.1
```

To SPRT a candidate NNUE net against the current one, use the `EvalFile` UCI option:

```bash
./bin/fastchess \
    -engine cmd=./target/release/oxid name=candidate option.EvalFile=nets/nn-abc123def456.nnue \
    -engine cmd=./target/release/oxid name=baseline \
    -each tc=8+0.08 \
    -rounds 5000 -repeat -concurrency 8 -recover \
    -sprt elo0=0 elo1=5 alpha=0.05 beta=0.05
```

## NNUE Net Workflow

### Converting checkpoints

After training, convert quantized checkpoints to `.nnue` format:

```bash
scripts/convert_checkpoints.sh
```

This produces `nets/nn-{sha256hash12}.nnue` files. All candidate nets are gitignored.

### Promoting a net

Once a net passes SPRT, promote it as the embedded default:

```bash
scripts/promote_net.sh nets/nn-abc123def456.nnue
```

This updates `src/main.rs`, `.gitignore`, and stages the git changes. Review and commit manually.

### How embedding works

The active net is compiled into the binary via `include_bytes!` in `src/main.rs`. No external files are needed at runtime. The `EvalFile` UCI option allows loading a different net for testing without recompiling.

## Conventions

* Rust 2021 edition, single dependency: `arrayvec`
* `rustfmt.toml`: `max_width = 120`
* Run `cargo fmt` and `cargo clippy` before committing
* Sub-module definitions go in `defs.rs` files, not inline

## Documentation

When adding or changing a search technique, evaluation term, or UCI command, update the corresponding doc file in `docs/` and the feature list in `README.md`.

* `docs/architecture.md` — Module overview, component wiring, core types, move encoding
* `docs/search.md` — Search algorithm, pruning, reductions, extensions, move ordering
* `docs/evaluation.md` — Tapered eval, material values, piece-square tables
* `docs/uci.md` — Supported UCI commands and options
