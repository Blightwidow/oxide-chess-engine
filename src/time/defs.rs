pub const SAFETY_MARGIN: u64 = 50; // 50ms safety margin applied to hard limit
pub const MAX_USAGE: f64 = 0.8; // 80% of the time allocated
pub const SOFT_FACTOR: f64 = 0.4; // Start new depth only if <40% of allocated time used
pub const MIN_MOVES_TO_GO: usize = 10; // Minimum moves to go for time allocation
pub const CHECK_INTERVAL: usize = 1024; // Only check time every N nodes
pub const MAX_TIME_FRACTION: f64 = 0.3; // Never use more than 30% of remaining time
