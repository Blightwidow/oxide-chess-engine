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

## Evaluation Function

```
For each square on the board:
  if piece present:
    mg_score += material[piece] + pst_mg[piece][square]
    eg_score += material[piece] + pst_eg[piece][square]
    (positive for white, negative for black)

Taper mg and eg scores by game phase
Return score from perspective of side to move
```

The final score is clamped to `[-VALUE_INFINITE, VALUE_INFINITE]`.
