pub mod defs;

use std::{
    cmp,
    time::{self, Instant},
};

use crate::{defs::Side, search::defs::SearchLimits};

use self::defs::*;

pub struct TimeManager {
    soft_limit: Option<Instant>,
    hard_limit: Option<Instant>,
    start_time: Option<Instant>,
    base_soft_ms: Option<u64>,
}

impl TimeManager {
    pub fn new(limits: SearchLimits, side_to_move: Side, game_ply: usize) -> Self {
        let start_time = time::Instant::now();

        // movetime takes priority: use it directly, ignoring wtime/btime
        if limits.movetime != usize::MAX {
            let movetime_ms = (limits.movetime as u64).saturating_sub(SAFETY_MARGIN);
            let cutoff = start_time + time::Duration::from_millis(movetime_ms);
            return Self {
                soft_limit: Some(cutoff),
                hard_limit: Some(cutoff),
                start_time: Some(start_time),
                base_soft_ms: None,
            };
        }

        let time = (limits.time(side_to_move).saturating_sub(SAFETY_MARGIN)) as f64;
        let increment = limits.increment(side_to_move) as f64;

        if time > 0.0 {
            let moves_to_go = if limits.moves_to_go > 0 {
                cmp::max(limits.moves_to_go, 1) as f64
            } else {
                40_usize.saturating_sub(game_ply / 2).max(MIN_MOVES_TO_GO) as f64
            };
            let time_slice = (increment + time * MAX_USAGE / moves_to_go).round() as u64;
            let base_soft_ms = (time_slice as f64 * SOFT_FACTOR).round() as u64;
            let soft = start_time + time::Duration::from_millis(base_soft_ms);
            let hard = start_time + time::Duration::from_millis(time_slice);
            Self {
                soft_limit: Some(soft),
                hard_limit: Some(hard),
                start_time: Some(start_time),
                base_soft_ms: Some(base_soft_ms),
            }
        } else {
            let time_slice = (increment * MAX_USAGE).round() as u64;
            let base_soft_ms = (time_slice as f64 * SOFT_FACTOR).round() as u64;
            let soft = start_time + time::Duration::from_millis(base_soft_ms);
            let hard = start_time + time::Duration::from_millis(time_slice);
            Self {
                soft_limit: Some(soft),
                hard_limit: Some(hard),
                start_time: Some(start_time),
                base_soft_ms: Some(base_soft_ms),
            }
        }
    }

    pub fn default() -> Self {
        Self {
            soft_limit: None,
            hard_limit: None,
            start_time: None,
            base_soft_ms: None,
        }
    }

    /// Rescale the soft limit by a factor (e.g. 0.5 = stop early, 1.3 = search longer).
    /// Only effective when base_soft_ms is available (timed search).
    pub fn scale_soft_limit(&mut self, factor: f64) {
        if let (Some(start), Some(base_ms)) = (self.start_time, self.base_soft_ms) {
            let scaled_ms = (base_ms as f64 * factor).round() as u64;
            self.soft_limit = Some(start + time::Duration::from_millis(scaled_ms));
        }
    }

    /// Check if we should avoid starting a new iterative deepening iteration
    pub fn should_stop_soft(&self) -> bool {
        if let Some(limit) = self.soft_limit {
            return time::Instant::now() >= limit;
        }
        false
    }

    /// Check if we should abort mid-search (hard limit)
    pub fn should_stop_hard(&self) -> bool {
        if let Some(limit) = self.hard_limit {
            return time::Instant::now() >= limit;
        }
        false
    }
}
