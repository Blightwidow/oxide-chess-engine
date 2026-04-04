# Time Management

The time manager (`src/time.rs`) controls how long the engine spends on each move. It uses a dual-limit design with adaptive scaling based on search feedback.

## Dual-Limit Design

Every timed search has two limits:

- **Soft limit**: When to stop starting new iterative deepening iterations. Checked between depths.
- **Hard limit**: When to abort the search immediately mid-node. Checked every 1024 nodes via a countdown timer to avoid calling `Instant::now()` every node.

The soft limit is dynamically rescaled each iteration based on search signals. The hard limit is fixed at allocation time.

## Time Allocation

Time is allocated from UCI clock info in `TimeManager::new()`:

### Movetime mode
Both limits are set to `movetime - SAFETY_MARGIN`.

### Timed search (wtime/btime + increment)
1. Estimate `moves_to_go` as `min(explicit, max(40 - game_ply/2, MIN_MOVES_TO_GO))`
2. Compute `time_slice = min(increment + time * MAX_USAGE / moves_to_go, time * MAX_TIME_FRACTION)`
3. Set `soft_limit = start + time_slice * SOFT_FACTOR`
4. Set `hard_limit = start + time_slice`

### Increment-only (time=0)
Uses `time_slice = increment * MAX_USAGE`, same soft/hard split.

## Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `SAFETY_MARGIN` | 50 ms | Buffer subtracted from available time |
| `MAX_USAGE` | 0.80 | Use at most 80% of allocated time |
| `SOFT_FACTOR` | 0.40 | Soft limit = 40% of time slice |
| `MIN_MOVES_TO_GO` | 10 | Floor on estimated remaining moves |
| `CHECK_INTERVAL` | 1024 | Nodes between hard-limit time checks |
| `MAX_TIME_FRACTION` | 0.30 | Never use >30% of remaining clock on one move |

## Soft Limit Scaling

Each completed iteration rescales the soft limit via `scale_soft_limit(factor)`, which recomputes the soft deadline from the base allocation. The factor is a product of four signals, clamped to [0.3, 2.0]:

```
combined = clamp(stability * node_tm * score_stability * complexity, 0.3, 2.0)
```

### Best-Move Stability

Tracks how many consecutive iterations the best move stays the same (`stability_count`).

```
stability = 0.5 + 0.8 * (1.0 - min(stability_count / 5, 1.0))
```

- New best move (count=0): factor = 1.3 (search longer)
- Stable for 5+ iterations: factor = 0.5 (stop early)

### Node TM (depth >= 6)

Measures what fraction of root nodes were spent on the best move.

| Condition | Factor | Rationale |
|-----------|--------|-----------|
| Best move has >90% of nodes | 0.6 | Clearly dominant, stop early |
| Best move has <50% of nodes | 1.3 | Contested, search longer |
| Between 50-90% | 1.0 | No adjustment |

### Score Stability (depth >= 6)

Compares the best score to the previous iteration's score. Only active for non-mate scores.

| Condition | Factor | Rationale |
|-----------|--------|-----------|
| Score dropped >30 cp | 1.4 | Something changed, extend |
| Score within +/-10 cp | 0.9 | Stable, slight shrink |
| Otherwise | 1.0 | No adjustment |

### Eval Complexity (depth >= 6)

Measures the centipawn spread between the best and second-best root moves.

| Condition | Factor | Rationale |
|-----------|--------|-----------|
| Spread < 20 cp | 1.2 | Tight game, search longer |
| Spread > 100 cp | 0.7 | One move dominates, stop early |
| Between 20-100 cp | 1.0 | No adjustment |

## Integration with Search

The iterative deepening loop in `search()` interacts with time management at three points:

1. **Before each depth**: Check `should_stop_soft()` — if the soft limit is exceeded, don't start a new iteration.
2. **During root move search**: After each root move, check `should_stop_hard()` — if exceeded, break out and return the current best.
3. **After each completed depth**: Compute the four scaling factors and call `scale_soft_limit(combined)`.

The hard time check uses a countdown (`nodes_until_check`) that decrements every node and only calls `Instant::now()` every `CHECK_INTERVAL` (1024) nodes, keeping overhead negligible.

## Examples

**Easy position** (one move clearly best):
- Node TM: best move gets 95% of nodes → 0.6
- Stability: same best move for 5 iterations → 0.5
- Complexity: 150 cp spread → 0.7
- Combined: 0.5 * 0.6 * 1.0 * 0.7 = 0.21 → clamped to 0.3
- Result: engine moves quickly

**Complex middlegame** (multiple candidate moves):
- Node TM: best move gets 40% of nodes → 1.3
- Stability: best move just changed → 1.3
- Score: dropped 35 cp → 1.4
- Complexity: 15 cp spread → 1.2
- Combined: 1.3 * 1.3 * 1.4 * 1.2 = 2.84 → clamped to 2.0
- Result: engine thinks twice as long as base allocation
