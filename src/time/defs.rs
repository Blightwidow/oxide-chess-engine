pub const SAFETY_MARGIN: u64 = 50; // 50ms safety margin applied to hard limit
pub const MAX_USAGE: f64 = 0.8; // 80% of the time allocated
pub const SOFT_FACTOR: f64 = 0.4; // Start new depth only if <40% of allocated time used
pub const MIN_MOVES_TO_GO: usize = 10; // Minimum moves to go for time allocation
pub const CHECK_INTERVAL: usize = 1024; // Only check time every N nodes
pub const MAX_TIME_FRACTION: f64 = 0.3; // Never use more than 30% of remaining time

// ─── Phase D: Node TM ────────────────────────────────────────────────────────
pub const NODE_TM_EARLY_STOP_THRESHOLD: f64 = 0.90; // best move has >90% of root nodes → stop early
pub const NODE_TM_EXTEND_THRESHOLD: f64 = 0.50; // best move has <50% of root nodes → extend
pub const NODE_TM_EARLY_FACTOR: f64 = 0.6; // shrink soft limit when best move dominates
pub const NODE_TM_EXTEND_FACTOR: f64 = 1.3; // extend soft limit when best move is contested

// ─── Phase D: Score stability ────────────────────────────────────────────────
pub const SCORE_DROP_THRESHOLD: i16 = 30; // cp drop between iterations triggering extension
pub const SCORE_DROP_EXTEND_FACTOR: f64 = 1.4; // extend on significant score drop
pub const SCORE_STABLE_FACTOR: f64 = 0.9; // slight shrink when score is stable

// ─── Phase D: Eval complexity (root move score spread) ───────────────────────
pub const COMPLEXITY_TIGHT_THRESHOLD: i16 = 20; // spread between #1 and #2 considered "tight"
pub const COMPLEXITY_WIDE_THRESHOLD: i16 = 100; // spread considered "dominant"
pub const COMPLEXITY_TIGHT_FACTOR: f64 = 1.2; // extend on tight position
pub const COMPLEXITY_EASY_FACTOR: f64 = 0.7; // shrink when one move clearly dominates
