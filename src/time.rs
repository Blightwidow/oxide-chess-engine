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
            let soft = start_time + time::Duration::from_millis((time_slice as f64 * SOFT_FACTOR).round() as u64);
            let hard = start_time + time::Duration::from_millis(time_slice);
            Self {
                soft_limit: Some(soft),
                hard_limit: Some(hard),
            }
        } else {
            let time_slice = (increment * MAX_USAGE).round() as u64;
            let soft = start_time + time::Duration::from_millis((time_slice as f64 * SOFT_FACTOR).round() as u64);
            let hard = start_time + time::Duration::from_millis(time_slice);
            Self {
                soft_limit: Some(soft),
                hard_limit: Some(hard),
            }
        }
    }

    pub fn default() -> Self {
        Self {
            soft_limit: None,
            hard_limit: None,
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
