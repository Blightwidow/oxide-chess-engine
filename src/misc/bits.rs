use crate::defs::{Bitboard, Square};

pub fn lsb(bitboard: Bitboard) -> Square {
    bitboard.trailing_zeros() as Square
}

pub fn pop(bitboard: &mut Bitboard) -> Square {
    let square: Square = lsb(*bitboard);

    *bitboard ^= 1u64 << square;

    square
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn lsb_single_bit() {
        assert_eq!(lsb(1u64 << 0), 0);
        assert_eq!(lsb(1u64 << 42), 42);
        assert_eq!(lsb(1u64 << 63), 63);
    }

    #[test]
    fn lsb_multiple_bits() {
        // bits at 3 and 7 -> lowest is 3
        assert_eq!(lsb((1u64 << 3) | (1u64 << 7)), 3);
    }

    #[test]
    fn pop_returns_lsb_and_clears() {
        let mut bb: Bitboard = (1u64 << 5) | (1u64 << 10);
        let sq = pop(&mut bb);
        assert_eq!(sq, 5);
        assert_eq!(bb, 1u64 << 10);
    }

    #[test]
    fn pop_sequence_ascending() {
        let mut bb: Bitboard = (1u64 << 2) | (1u64 << 17) | (1u64 << 55);
        let mut squares = Vec::new();
        while bb != 0 {
            squares.push(pop(&mut bb));
        }
        assert_eq!(squares, vec![2, 17, 55]);
    }

    #[test]
    fn pop_single_bit() {
        let mut bb: Bitboard = 1u64 << 31;
        let sq = pop(&mut bb);
        assert_eq!(sq, 31);
        assert_eq!(bb, 0);
    }
}
