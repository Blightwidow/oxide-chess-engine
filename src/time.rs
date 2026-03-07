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
        let time = (limits.time(side_to_move) - SAFETY_MARGIN) as f64;
        let increment = limits.increment(side_to_move) as f64;
        let start_time = time::Instant::now();

        let cutoff = if time > 0.0 {
            let move_to_go = cmp::max(40 - game_ply, 3) as f64;
            let time_slice = (increment + time * MAX_USAGE / move_to_go).round() as u64;
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
