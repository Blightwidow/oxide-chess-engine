use std::{
    cmp,
    time::{self, Instant},
};

use crate::{defs::Side, search::defs::SearchLimits};

pub struct TimeManager {
    start_time: Instant,
    cutoff: Option<Instant>,
}

impl TimeManager {
    pub fn new(limits: SearchLimits, side_to_move: Side, game_ply: usize) -> Self {
        let think_time: u64 = limits.time(side_to_move) / cmp::max(40 - game_ply, 3) as u64;
        let start_time = time::Instant::now();
        let cutoff = start_time + time::Duration::from_millis(think_time);

        Self {
            start_time,
            cutoff: Some(cutoff),
        }
    }

    pub fn default() -> Self {
        Self {
            start_time: time::Instant::now(),
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
