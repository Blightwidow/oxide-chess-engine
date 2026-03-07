# Oxide chess bot

A simple Rust chess engine compatible with the [UCI protocol](https://en.wikipedia.org/wiki/Universal_Chess_Interface). It does not come with a GUI. You can dowload a a separate one like [Cute Chess](https://cutechess.com/).

## Usage

You can directly form source

```
cargo run -r -- <command>
```

or build and then run the executable

```
cargo build -r
./taget/release/chessbot <command>
```

## Test your changes

### SPRT Test

```
cargo build -r --target-dir=base
# Change your engine
cargo build -r
./bin/fastchess -engine cmd=./target/release/chessbot name=oxide -engine cmd=./base/release/chessbot name=engine_BASE -each tc=8+0.08 -rounds 15000 -repeat -concurrency 6 -recover -sprt elo0=0 elo1=5 alpha=0.05 beta=0.05
```

## Internal implementation

### Board representation

- Magic bitboards
- Bitboards with Little Endian Rank-File mapping
- 8x8 Board

### Search

- Negamax
- Iterative deepening

### Evaluation

- Centipawn scaling
- Tapered piece square table

## Acknowledgements

- An amazing thanks to @mvanthoor for his work on [Rustic](https://github.com/mvanthoor/rustic) that helped me understand a lot of concepts in Rust.
- Also a big part of my way of thinking was influenced by [Stockfish](https://stockfishchess.org/). It was also a great tool to debug my code.
