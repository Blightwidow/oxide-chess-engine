use std::ops::RangeInclusive;

pub type Bitboard = u64;
pub type Piece = usize;
pub type Side = usize;
pub type Square = usize;

pub const NONE_SQUARE: Square = 64;

pub fn is_ok(sq: Square) -> bool {
    sq < 64
}

pub struct Sides;
impl Sides {
    pub const WHITE: Side = 0;
    pub const BLACK: Side = 1;
    pub const BOTH: Side = 2;
}

pub struct PieceType;
impl PieceType {
    pub const NONE: Piece = 0;
    pub const PAWN: Piece = 1;
    pub const KNIGHT: Piece = 2;
    pub const BISHOP: Piece = 3;
    pub const ROOK: Piece = 4;
    pub const QUEEN: Piece = 5;
    pub const KING: Piece = 6;
}

pub fn make_piece(side: Side, piece_type: Piece) -> Piece {
    side * 8 + piece_type
}

pub fn color_of_piece(piece: Piece) -> Side {
    piece / 8
}

pub fn type_of_piece(piece: Piece) -> Side {
    piece % 8
}

pub struct NrOf;
impl NrOf {
    pub const PIECE_TYPES: usize = 7;
    pub const SIDES: usize = 3;
    pub const SQUARES: usize = 64;
}

pub struct RangeOf;
impl RangeOf {
    pub const RANKS: RangeInclusive<usize> = 0..=7;
    pub const FILES: RangeInclusive<usize> = 0..=7;
    pub const SQUARES: RangeInclusive<Square> = 0..=63;
}

pub type Direction = isize;

pub struct Directions;
impl Directions {
    pub const UP: Direction = 8;
    pub const DOWN: Direction = -8;
    pub const LEFT: Direction = -1;
    pub const RIGHT: Direction = 1;
    pub const UP_LEFT: Direction = 7;
    pub const UP_RIGHT: Direction = 9;
    pub const DOWN_LEFT: Direction = -9;
    pub const DOWN_RIGHT: Direction = -7;
}

pub fn file_of(square: Square) -> usize {
    square % 8
}

pub fn rank_of(square: Square) -> usize {
    square / 8
}

pub fn square_of(file: usize, rank: usize) -> Square {
    file + rank * 8
}

pub fn distance(from: Square, to: Square) -> usize {
    let rank_dist = rank_of(from) as isize - rank_of(to) as isize;
    let file_dist = file_of(from) as isize - file_of(to) as isize;

    (rank_dist.abs().max(file_dist.abs())) as usize
}

#[allow(dead_code)]
pub fn pretty_square(square: Square) -> String {
    format!(
        "{}{}",
        "abcdefgh".chars().nth(file_of(square)).unwrap(),
        rank_of(square) + 1
    )
}

#[allow(dead_code)]
pub fn pretty_piece(piece: Piece) -> String {
    format!(
        "{}{}",
        "xpnbrqk".chars().nth(type_of_piece(piece)).unwrap(),
        "WB".chars().nth(color_of_piece(piece)).unwrap(),
    )
}

pub fn shift(bitboard: Bitboard, direction: Direction) -> Bitboard {
    if direction > 0 {
        bitboard << direction
    } else {
        bitboard >> -direction
    }
}

pub fn square_bb(square: Square) -> Bitboard {
    1u64 << square
}
