pub mod defs;
pub mod transposition;

use self::{defs::*, transposition::TranspositionTable};

pub struct Eval {
    pub transposition_table: TranspositionTable,
}

impl Eval {
    pub fn new() -> Self {
        Self {
            transposition_table: TranspositionTable::new(DEFAULT_HASH_SIZE),
        }
    }

    pub fn resize_transposition_table(&mut self, megabytes: usize) {
        self.transposition_table = TranspositionTable::new(megabytes);
    }
}
