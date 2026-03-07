use crate::defs::{Side, Sides};

pub const FEN_START_POSITION: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[derive(Clone, Copy, PartialEq)]
pub struct SearchLimits {
    pub perft: u8,
    pub depth: u8,
    pub ponder: bool,
    pub white_time: u64,
    pub black_time: u64,
    pub white_inc: u64,
    pub black_inc: u64,
    pub moves_to_go: usize,
    pub nodes: usize,
    pub mate: usize,
    pub movetime: usize,
}

impl SearchLimits {
    pub fn default() -> SearchLimits {
        SearchLimits {
            perft: 0,
            depth: 12,
            ponder: false,
            white_time: u64::MAX,
            black_time: u64::MAX,
            white_inc: 0,
            black_inc: 0,
            moves_to_go: 0,
            nodes: usize::MAX,
            mate: 0,
            movetime: usize::MAX,
        }
    }

    pub fn time(&self, side: Side) -> u64 {
        match side {
            Sides::WHITE => self.white_time,
            Sides::BLACK => self.black_time,
            _ => panic!("Invalid side"),
        }
    }

    pub fn increment(&self, side: Side) -> u64 {
        match side {
            Sides::WHITE => self.white_inc,
            Sides::BLACK => self.black_inc,
            _ => panic!("Invalid side"),
        }
    }
}

pub const VALUE_ZERO: i16 = 0;
pub const VALUE_DRAW: i16 = VALUE_ZERO;
pub const VALUE_MATE: i16 = 32000;
pub const VALUE_INFINITE: i16 = 32001;
pub const VALUE_NONE: i16 = 32002;
