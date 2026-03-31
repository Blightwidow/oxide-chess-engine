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

### Probcut
If a capture quickly beats beta by a large margin at reduced depth, prune the whole node.
- **Conditions**: non-PV, not in check, depth >= 5, |beta| < VALUE_MATE - 100, no excluded move
- **Margin**: beta + 200 cp (`probcut_beta`)
- **Reduction**: depth - 4
- **SEE filter**: only tries captures with `SEE >= 0` (non-losing trades)
- Returns the shallow search score if it proves the position exceeds `probcut_beta`

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

Good move ordering is critical for alpha-beta efficiency. Moves are generated in stages via a **MovePicker** with lazy legality checking (legality tested per-move instead of generating all legal moves upfront):

| Stage | Description |
|-------|-------------|
| 1 | TT move |
| 2 | Captures (scored by MVV-LVA + capture history) |
| 3 | Killer moves (1st, 2nd) |
| 4 | Countermove |
| 5 | Quiet moves (history + continuation history) |

### Killer Heuristic
Two killer moves stored per ply. Updated on beta cutoffs for quiet moves.

### History Heuristic
A `[64][64]` table indexed by `[from_square][to_square]`. Incremented by `depth^2` on beta cutoffs for quiet moves. On beta cutoff, all quiet moves tried before the cutoff move receive a **history malus** of `-depth^2`.

### Capture History
A `[piece][to_square][captured_piece_type]` table. Updated on beta cutoffs for captures. Used alongside MVV-LVA for capture move ordering.

### Continuation History
Two tables tracking move pair correlations: **1-ply** (previous move → current move) and **2-ply** (two moves ago → current move). Indexed by `[prev_piece_type][prev_to_sq][curr_piece_type][curr_to_sq]`. Used to score quiet moves alongside the main history table.

### History in LMR
Main history scores adjust LMR reductions: `r -= (history / 5000).clamp(-1, 1)`. High-history moves are reduced less, low-history moves are reduced more.

## Quiescence Search

At the leaves of the main search, a quiescence search resolves tactical sequences to avoid evaluation of unstable positions.

- Uses a dedicated `generate_captures()` function for capture-only move generation
- Uses stand-pat (static eval) as a baseline score
- Applies delta pruning and SEE pruning

## Transposition Table

The transposition table stores previously searched positions to avoid redundant work.

- **Entry**: Zobrist key, depth, score, best move, node type (Exact / LowerBound / UpperBound)
- **Default size**: 16 MB (configurable via UCI `Hash` option, 1-512 MB)
- **Replacement**: age-based with a generation counter; stale entries from previous searches are always replaced; among same-age entries, deeper entries are preferred
- **Hashfull**: sampled from the first 1000 entries, reported in UCI info strings

## Constants

| Constant | Value |
|----------|-------|
| MAX_PLY | 128 |
| VALUE_MATE | 32000 |
| VALUE_INFINITE | 32001 |
| VALUE_DRAW | 0 |
