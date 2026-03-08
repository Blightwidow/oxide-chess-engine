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
#[allow(dead_code)]
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
            node_type: NodeType::Exact,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum NodeType {
    Exact,
    LowerBound,
    UpperBound,
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

        // Replace if: empty slot, same key, or new depth >= existing depth
        if entry.key == 0 || entry.key == key || data.depth >= entry.data.depth {
            entry.key = key;
            entry.data = data;
        }
    }

    pub fn clear(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.key = 0;
            entry.data = HashData::default();
        }
    }

    pub fn hashfull(&self) -> usize {
        let sample = self.entries.len().min(1000);
        let used = self.entries[..sample].iter().filter(|e| e.key != 0).count();
        used * 1000 / sample
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

#[cfg(test)]
mod test {
    use super::*;

    fn make_data(depth: u8, value: i16, node_type: NodeType) -> HashData {
        HashData {
            depth,
            value,
            best_move: Move::none(),
            node_type,
        }
    }

    #[test]
    fn store_and_probe() {
        let mut tt = TranspositionTable::new(1);
        let key = 123456789u64;
        let data = make_data(5, 100, NodeType::Exact);
        tt.store(key, data);

        let result = tt.probe(key).unwrap();
        assert_eq!(result.depth, 5);
        assert_eq!(result.value, 100);
        assert_eq!(result.node_type, NodeType::Exact);
    }

    #[test]
    fn probe_miss() {
        let tt = TranspositionTable::new(1);
        assert!(tt.probe(999).is_none());
    }

    #[test]
    fn depth_replacement_lower_depth_preserved() {
        let mut tt = TranspositionTable::new(1);
        let key_a = 42u64;
        let key_b = key_a + tt.size as u64; // same index, different key

        tt.store(key_a, make_data(5, 100, NodeType::Exact));
        tt.store(key_b, make_data(3, 200, NodeType::LowerBound));

        // Original should be preserved (deeper)
        let result = tt.probe(key_a).unwrap();
        assert_eq!(result.depth, 5);
        assert_eq!(result.value, 100);
    }

    #[test]
    fn depth_replacement_higher_depth_replaces() {
        let mut tt = TranspositionTable::new(1);
        let key_a = 42u64;
        let key_b = key_a + tt.size as u64; // same index, different key

        tt.store(key_a, make_data(5, 100, NodeType::Exact));
        tt.store(key_b, make_data(7, 200, NodeType::LowerBound));

        // New entry should replace (deeper)
        let result = tt.probe(key_b).unwrap();
        assert_eq!(result.depth, 7);
        assert_eq!(result.value, 200);
    }

    #[test]
    fn same_key_always_replaces() {
        let mut tt = TranspositionTable::new(1);
        let key = 42u64;

        tt.store(key, make_data(10, 100, NodeType::Exact));
        tt.store(key, make_data(2, 200, NodeType::UpperBound));

        let result = tt.probe(key).unwrap();
        assert_eq!(result.depth, 2);
        assert_eq!(result.value, 200);
    }

    #[test]
    fn clear_removes_all() {
        let mut tt = TranspositionTable::new(1);
        tt.store(1, make_data(5, 100, NodeType::Exact));
        tt.store(2, make_data(5, 200, NodeType::Exact));
        tt.clear();

        assert!(tt.probe(1).is_none());
        assert!(tt.probe(2).is_none());
    }

    #[test]
    fn hashfull_empty() {
        let tt = TranspositionTable::new(1);
        assert_eq!(tt.hashfull(), 0);
    }

    #[test]
    fn hashfull_nonzero() {
        let mut tt = TranspositionTable::new(1);
        for i in 0..100u64 {
            tt.store(i, make_data(1, 0, NodeType::Exact));
        }
        assert!(tt.hashfull() > 0);
    }
}
