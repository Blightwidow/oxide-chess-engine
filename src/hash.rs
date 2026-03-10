use crate::defs::*;

pub struct Hasher {
    pub piece_keys: [[[u64; NrOf::SQUARES]; NrOf::PIECE_TYPES]; 2],
    pub side_key: u64,
    pub castling_keys: [u64; 16],
    pub en_passant_keys: [u64; 8],
    /// Separate Zobrist keys for pawn hash (used by correction history)
    pub pawn_keys: [[u64; NrOf::SQUARES]; 2],
}

impl Hasher {
    pub fn new() -> Self {
        let mut rng = XorShift64(0x98F1A820C73B4D56);

        let mut piece_keys = [[[0u64; NrOf::SQUARES]; NrOf::PIECE_TYPES]; 2];
        for side_keys in &mut piece_keys {
            for piece_keys in side_keys.iter_mut() {
                for key in piece_keys.iter_mut() {
                    *key = rng.next();
                }
            }
        }

        let side_key = rng.next();

        let mut castling_keys = [0u64; 16];
        for key in &mut castling_keys {
            *key = rng.next();
        }

        let mut en_passant_keys = [0u64; 8];
        for key in &mut en_passant_keys {
            *key = rng.next();
        }

        let mut pawn_keys = [[0u64; NrOf::SQUARES]; 2];
        for side_keys in &mut pawn_keys {
            for key in side_keys.iter_mut() {
                *key = rng.next();
            }
        }

        Self {
            piece_keys,
            side_key,
            castling_keys,
            en_passant_keys,
            pawn_keys,
        }
    }

    pub fn piece_key(&self, side: Side, piece_type: Piece, square: Square) -> u64 {
        self.piece_keys[side][piece_type][square]
    }
}

struct XorShift64(u64);

impl XorShift64 {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}
