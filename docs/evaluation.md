# Evaluation

The evaluation module (`src/evaluate.rs`) assigns a centipawn score to a position from the perspective of the side to move.

## Tapered Evaluation

The evaluation blends middlegame (MG) and endgame (EG) scores based on game phase:

```
phase = sum of phase increments for all pieces on the board
score = (mg_score * phase + eg_score * (24 - phase)) / 24
```

If phase >= 24, the position is treated as pure middlegame. As pieces are traded off, the endgame score gains more weight.

### Phase Increments

| Piece | Increment |
|-------|-----------|
| Pawn | 0 |
| Knight | 1 |
| Bishop | 1 |
| Rook | 2 |
| Queen | 4 |

A full set of minor pieces, rooks, and queens totals 24 (the starting phase).

## Material Values

| Piece | Middlegame | Endgame |
|-------|-----------|---------|
| Pawn | 82 | 94 |
| Knight | 337 | 281 |
| Bishop | 365 | 297 |
| Rook | 477 | 512 |
| Queen | 1025 | 936 |

## Piece-Square Tables

Each piece type has separate MG and EG piece-square tables (64 values each). These encode positional bonuses and penalties — e.g., central knights are worth more, rooks on open files are encouraged, kings should shelter in the middlegame but centralize in the endgame.

Tables are defined in `src/evaluate/tables.rs`. White's tables are indexed with a vertical flip (`square ^ 56`) to reuse the same data for both sides.

## Pawn Structure

Evaluated per side using bitboard operations:

**Doubled pawns** — For each file, if more than one friendly pawn occupies it, a penalty is applied per extra pawn.

| Term | MG | EG |
|------|----|----|
| Doubled pawn | -11 | -51 |

**Isolated pawns** — A pawn with no friendly pawns on adjacent files receives a penalty.

| Term | MG | EG |
|------|----|----|
| Isolated pawn | -5 | -15 |

**Passed pawns** — A pawn with no enemy pawns ahead on its file or adjacent files receives a rank-based bonus.

| Relative Rank | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---------------|---|---|---|---|---|---|---|---|
| MG | 0 | 5 | 10 | 20 | 40 | 60 | 100 | 0 |
| EG | 0 | 7 | 16 | 34 | 72 | 128 | 192 | 0 |

Passed pawn masks are precomputed in `Eval::new()` for each square and side.

## Bishop Pair

A side with two or more bishops receives a bonus:

| Term | MG | EG |
|------|----|----|
| Bishop pair | 30 | 50 |

## Rook on Open/Semi-Open File

For each rook, the file is checked for friendly and enemy pawns:

| Term | MG | EG |
|------|----|----|
| Rook open file (no pawns) | 25 | 10 |
| Rook semi-open file (no friendly pawns) | 10 | 7 |

## Evaluation Function

```
For each square on the board:
  if piece present:
    mg_score += material[piece] + pst_mg[piece][square]
    eg_score += material[piece] + pst_eg[piece][square]
    (positive for white, negative for black)

For each side:
  Doubled pawn penalties (per file)
  Isolated pawn penalties (per pawn)
  Passed pawn bonuses (per pawn, by relative rank)
  Bishop pair bonus (if >= 2 bishops)
  Rook open/semi-open file bonuses (per rook)

Taper mg and eg scores by game phase
Return score from perspective of side to move
```

The final score is clamped to `[-VALUE_INFINITE, VALUE_INFINITE]`.

## NNUE Evaluation (Optional)

The engine supports an optional NNUE (Efficiently Updatable Neural Network) evaluation that replaces the handcrafted eval when a trained network file is available.

### Architecture

```
(8 buckets × 768) inputs → [384] accumulator (per perspective) → SCReLU
[768] concatenated → [32] hidden → SCReLU → [1] output → scale to centipawns
```

- **Input features**: 6144 = 8 king buckets × 768 (2 colors × 6 piece types × 64 squares)
- **King bucketing**: Each perspective uses separate feature transformer weights depending on its king's position. Kings are bucketed by rank (0–7) with horizontal mirroring — files e-h are mapped to d-a, so the network only learns queen-side king positions and mirrors for king-side.
- **Horizontal mirroring**: When the perspective's king is on files e-h, all piece squares are flipped horizontally (`sq ^ 7`) to normalize to the queen-side half.
- **Bucket change refresh**: When a king move changes the bucket or mirror state, the moving side's accumulator is recomputed from scratch. The opponent's accumulator is updated incrementally as normal.
- **Perspective**: White and black perspectives computed separately. For black, colors are swapped and squares vertically flipped.
- **Quantization**: Accumulator clipped to [0, 255], output scaled by 400/(255×64)
- **Arithmetic**: Pure integer (i16/i32), no floating point
- **SIMD**: Accumulator updates and SCReLU activation use platform-specific SIMD — NEON on aarch64, AVX2 on x86_64, with scalar fallback for other architectures. Accumulators are 32-byte aligned for AVX2 load/store.

### Network File Format (v3)

Binary `.nnue` file with header:
- Magic: `OXNN` (4 bytes)
- Version: `3` (u32 LE)
- Num buckets, feature size, hidden size, L1 size (4 × u32 LE)
- Weights and biases as i16 little-endian

Configurable via `setoption name EvalFile value <path>`. The default net is embedded at compile time via `include_bytes!`.

### Fallback

If no network file is found, the engine automatically uses the handcrafted evaluation described above.
