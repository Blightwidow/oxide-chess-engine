use crate::movegen::defs::Move;

pub struct TranspositionTable {
    entries: Vec<Entry>,
    size: usize,
}

#[derive(Copy, Clone)]
pub struct Entry {
    key: u64,
    data: HashData,
}
#[derive(Copy, Clone)]
pub struct HashData {
    pub depth: u8,
    pub value: i16,
    pub best_move: Move,
    pub node_type: NodeType,
}

impl HashData {
    pub fn default() -> Self {
        Self {
            depth: 0,
            value: 0,
            best_move: Move::none(),
            node_type: NodeType::EXACT,
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum NodeType {
    EXACT,
    LOWERBOUND,
    UPPERBOUND,
}

impl TranspositionTable {
    // Size is in MB
    pub fn new(megabytes: usize) -> Self {
        let size = megabytes * 1024 * 1024;
        let nb_entries = size / std::mem::size_of::<Entry>();

        Self {
            entries: vec![
                Entry {
                    key: 0,
                    data: HashData::default(),
                };
                nb_entries
            ],
            size: nb_entries,
        }
    }

    pub fn store(&mut self, key: u64, data: HashData) {
        let index = key % self.size as u64;
        let entry = &mut self.entries[index as usize];

        if entry.key == key {
            entry.data = data;
        } else if entry.key == 0 {
            entry.key = key;
            entry.data = data;
        }
    }

    pub fn probe(&self, key: u64) -> Option<&HashData> {
        let index = key % self.size as u64;
        let entry = &self.entries[index as usize];

        if entry.key == key {
            return Some(&entry.data);
        }

        None
    }
}
