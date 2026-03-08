# Oxide Chess Bot

A UCI-compatible chess engine written in Rust. Single-threaded, no external dependencies beyond `arrayvec`.

It does not come with a GUI. You can use a separate one like [Cute Chess](https://cutechess.com/) or [Arena](http://www.playwitharena.de/).

## Usage

Run directly from source:

```bash
cargo run -r -- <command>
```

Or build and run the executable:

```bash
cargo build -r
./target/release/oxide <command>
```

### Benchmark

```bash
cargo run -r -- bench              # Default: depth 13, 16 MB hash
cargo run -r -- bench 32 1 15      # Custom: 32 MB hash, 1 thread, depth 15
```

## Testing

```bash
cargo test                  # Unit tests
cargo clippy                # Lint
```

### SPRT Testing (strength regression)

```bash
cargo build -r --target-dir=base   # Build baseline
# Make changes...
cargo build -r                     # Build new version
./bin/fastchess \
    -engine cmd=./target/release/oxide name="vX.X.X" \
    -engine cmd=./base/release/oxide name="vY.Y.Y" \
    -pgnout file="./games/vX.X.X-vY.Y.Y.pgn" \
    -openings file=./data/openings.pgn format=pgn order=random \
    -each tc=8+0.08 \
    -rounds 5000 -repeat \
    -concurrency 8 \
    -recover \
    -sprt elo0=0 elo1=10 alpha=0.05 beta=0.1
```

## Features

### Board Representation

* Magic bitboards for sliding piece attacks
* Bitboards with Little-Endian Rank-File (LERF) mapping
* Hybrid mailbox + bitboard representation
* Incremental Zobrist hashing

### Search

* Negamax with alpha-beta pruning
* Iterative deepening with aspiration windows
* Principal Variation Search (PVS)
* Transposition table with best move, depth, and node type
* Quiescence search (captures, en passant, promotions)
* Check extensions
* Null move pruning
* Reverse futility pruning
* Razoring
* Futility pruning
* Late move pruning (LMP)
* Late move reductions (LMR)
* SEE pruning (static exchange evaluation)
* Delta pruning in quiescence
* Move ordering: TT move > MVV-LVA captures > killer moves > history heuristic

### Evaluation

* NNUE evaluation support (768→256×2→32→1 architecture, integer arithmetic)
* Handcrafted fallback: tapered evaluation (middlegame/endgame interpolation by game phase)
* Piece-square tables (separate MG and EG)
* Material values tuned for MG and EG
* Pawn structure: doubled, isolated, and passed pawn evaluation
* Bishop pair bonus
* Rook on open and semi-open file bonus

## Documentation

Detailed documentation is available in the [`docs/`](docs/) directory:

* [Architecture](docs/architecture.md) — Module overview, component wiring, core types
* [Search](docs/search.md) — All search techniques, pruning, reductions, move ordering
* [Evaluation](docs/evaluation.md) — Tapered eval, material values, piece-square tables
* [UCI Protocol](docs/uci.md) — Supported commands and options

## Acknowledgements

* A huge thanks to [@mvanthoor](https://github.com/mvanthoor) for his work on [Rustic](https://github.com/mvanthoor/rustic) that helped me understand a lot of concepts in Rust.
* Also a big part of my way of thinking was influenced by [Stockfish](https://stockfishchess.org/). It was also a great tool to debug my code.
