# Architecture

Oxide is a single-threaded UCI chess engine written in Rust. This document describes how the engine is structured and how its components interact.

## Entry Point

`src/main.rs` initializes the core components and starts the UCI input loop:

```
Bitboards (Rc) ──┬──> Movegen
                  └──> Position <── Hasher (Rc)

Search owns: Position, Movegen, Eval
Uci::main_loop(&mut Search)
```

Shared data (`Bitboards`, `Hasher`) is distributed via `Rc` (reference counting). There is no multi-threading.

## Module Overview

| Module | Path | Purpose |
|--------|------|---------|
| **main** | `src/main.rs` | Initialization, component wiring |
| **uci** | `src/uci.rs` | UCI protocol handler, main input loop |
| **search** | `src/search.rs` | Negamax with alpha-beta, iterative deepening |
| **evaluate** | `src/evaluate.rs` | Tapered eval with piece-square tables |
| **position** | `src/position.rs` | Board state, do/undo move, Zobrist hashing |
| **movegen** | `src/movegen.rs` | Legal move generation |
| **bitboards** | `src/bitboards.rs` | Magic bitboards, attack tables, LERF mapping |
| **time** | `src/time.rs` | Time management for search cutoff |
| **defs** | `src/defs.rs` | Core types: `Bitboard`, `Piece`, `Side`, `Square` |
| **hash** | `src/hash.rs` | Zobrist key generation |
| **benchmark** | `src/benchmark.rs` | 46-position bench suite |
| **misc** | `src/misc.rs` | Bit manipulation utilities |

## Module Organization

Each module may have sub-files following a consistent convention:

- `defs.rs` — types and constants for that module
- `tables.rs` — lookup tables (e.g. piece-square tables in evaluate)
- `test.rs` — unit tests

## Core Types (`src/defs.rs`)

All core types are simple aliases:

- `Bitboard = u64`
- `Piece = usize` — encoded as `side * 8 + piece_type`
- `Side = usize` — `WHITE = 0`, `BLACK = 1`
- `Square = usize` — 0..63, Little-Endian Rank-File mapping (a1=0, h8=63)

## Board Representation

The position uses a hybrid representation:

- **Mailbox**: `board: [Piece; 64]` for O(1) piece lookup by square
- **Bitboards**: `by_type_bb: [[Bitboard; 7]; 2]` for fast set operations per piece type and side
- **Color bitboards**: `by_color_bb: [Bitboard; 3]` (white, black, both)

Moves are encoded as `u16` bitfields:

```
Bits:  [15:14]  [13:12]  [11:6]  [5:0]
       type     promo    from    to
```

Move types: Normal (0), Promotion (1), En Passant (2), Castling (3).

## Dependencies

The engine has a single external dependency: `arrayvec` (stack-allocated move lists capped at 256 moves).
