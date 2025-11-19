use crate::defs::{Bitboard, Square};

pub fn lsb(bitboard: Bitboard) -> Square {
    bitboard.trailing_zeros() as Square
}

pub fn pop(bitboard: &mut Bitboard) -> Square {
    let square: Square = lsb(*bitboard);

    *bitboard ^= 1u64 << square;

    square
}
