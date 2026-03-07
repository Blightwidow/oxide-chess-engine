# Search

The search module (`src/search.rs`) implements a negamax alpha-beta search with iterative deepening and numerous pruning and reduction techniques.

## Core Algorithm

- **Negamax with Alpha-Beta Pruning**: Standard minimax reformulated for a single recursive function
- **Iterative Deepening**: Searches depth 1, then 2, etc., using results from shallower searches to improve move ordering
- **Aspiration Windows**: Starting at depth 4, the search window is narrowed to +/-25 centipawns around the previous score. On failure, it widens and re-searches
- **Principal Variation Search (PVS)**: After the first move, uses null-window searches. Re-searches with full window on failure

## Pruning Techniques

### Null Move Pruning (NMP)
Skips a turn to see if the position is still good enough. If the opponent can't take advantage of a free move, the position is likely strong.
- **Conditions**: depth >= 3, not in check, side has non-pawn material
- **Reduction**: `r = 3 + (depth / 4)`

### Reverse Futility Pruning (RFP)
If the static evaluation is far above beta, prune the node.
- **Conditions**: non-PV node, not in check, depth <= 7
- **Margin**: `80 * depth` centipawns

### Razoring
If the static eval is far below alpha at shallow depths, drop into quiescence search.
- **Conditions**: depth <= 2, non-PV, not in check
- **Margins**: 300 cp (depth 1), 600 cp (depth 2)

### Futility Pruning
At shallow depths, skip quiet moves that can't possibly raise the score above alpha.
- **Conditions**: depth <= 2, non-PV, quiet moves (not killer moves)
- **Margins**: 200 cp (depth 1), 400 cp (depth 2)

### Late Move Pruning (LMP)
At shallow depths, skip quiet non-killer moves after a threshold of moves has been searched.
- **Thresholds per depth**: `[0, 3, 6, 10, 15]`

### SEE Pruning
Prunes captures with a negative Static Exchange Evaluation (losing trades).
- Applied to captures in main search (depth <= 3) and in quiescence search
- SEE values: Pawn=100, Knight=300, Bishop=300, Rook=500, Queen=900, King=20000

### Delta Pruning (Quiescence)
In quiescence search, prunes captures that can't raise the score above alpha even with a bonus.
- **Margin**: captured piece value + 200 cp

## Reductions

### Late Move Reductions (LMR)
Moves searched later in the move list are searched at reduced depth, since they're less likely to be good.
- **Pre-computed table**: `LMR[depth][move_number] = floor(ln(depth) * ln(move_num) / 2.0)`
- **Conditions**: non-first moves, non-captures, non-promotions, non-killers, moves_searched >= 3, depth >= 3
- If reduced search fails high, re-search at full depth

## Extensions

### Check Extension
When the side to move is in check, search depth is extended by 1 ply to avoid horizon effects.

## Move Ordering

Good move ordering is critical for alpha-beta efficiency. Moves are scored and sorted using incremental selection sort (avoids fully sorting moves that won't be reached after a beta cutoff).

| Priority | Category | Score |
|----------|----------|-------|
| 1 | TT move (from transposition table) | 1,000,000 |
| 2 | Captures (MVV-LVA) | 100,000 + victim*100 - attacker |
| 3 | Promotions | 100,000 + promotion_piece*100 |
| 4 | Killer move (1st) | 90,000 |
| 5 | Killer move (2nd) | 80,000 |
| 6 | Quiet moves (history heuristic) | history[from][to] |

### Killer Heuristic
Two killer moves stored per ply. Updated on beta cutoffs for quiet moves.

### History Heuristic
A `[64][64]` table indexed by `[from_square][to_square]`. Incremented by `depth^2` on beta cutoffs for quiet moves.

## Quiescence Search

At the leaves of the main search, a quiescence search resolves tactical sequences to avoid evaluation of unstable positions.

- Only considers captures, en passant, and promotions
- Uses stand-pat (static eval) as a baseline score
- Applies delta pruning and SEE pruning

## Transposition Table

The transposition table stores previously searched positions to avoid redundant work.

- **Entry**: Zobrist key, depth, score, best move, node type (Exact / LowerBound / UpperBound)
- **Default size**: 16 MB (configurable via UCI `Hash` option, 1-512 MB)
- **Replacement**: deeper entries preferred; same-key entries always replaced
- **Hashfull**: sampled from the first 1000 entries, reported in UCI info strings

## Constants

| Constant | Value |
|----------|-------|
| MAX_PLY | 128 |
| VALUE_MATE | 32000 |
| VALUE_INFINITE | 32001 |
| VALUE_DRAW | 0 |
