pub mod defs;

use std::{
    cmp,
    time::{self, Instant},
};

use crate::{
    defs::Side,
    search::defs::SearchLimits,
};

use self::defs::*;

pub struct TimeManager {
    cutoff: Option<Instant>,
}

impl TimeManager {
    pub fn new(limits: SearchLimits, side_to_move: Side, game_ply: usize) -> Self {
        let start_time = time::Instant::now();

        // movetime takes priority: use it directly, ignoring wtime/btime
        if limits.movetime != usize::MAX {
            let movetime_ms = (limits.movetime as u64).saturating_sub(SAFETY_MARGIN);
            return Self {
                cutoff: Some(start_time + time::Duration::from_millis(movetime_ms)),
            };
        }

        let time = (limits.time(side_to_move).saturating_sub(SAFETY_MARGIN)) as f64;
        let increment = limits.increment(side_to_move) as f64;

        let cutoff = if time > 0.0 {
            let moves_to_go = if limits.moves_to_go > 0 {
                cmp::max(limits.moves_to_go, 1) as f64
            } else {
                40_usize.saturating_sub(game_ply).max(3) as f64
            };
            let time_slice = (increment + time * MAX_USAGE / moves_to_go).round() as u64;
            start_time + time::Duration::from_millis(time_slice)
        } else {
            start_time + time::Duration::from_millis((increment * MAX_USAGE).round() as u64)
        };

        Self {
            cutoff: Some(cutoff),
        }
    }

    pub fn default() -> Self {
        Self {
            cutoff: None,
        }
    }

    pub fn should_stop(&self) -> bool {
        if let Some(cutoff) = self.cutoff {
            return time::Instant::now() >= cutoff;
        }

        false
    }
}
