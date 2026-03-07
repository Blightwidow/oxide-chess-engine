use std::fmt;

use crate::defs::*;

pub type MoveType = u16;
pub struct MoveTypes {}

impl MoveTypes {
    pub const NORMAL: u16 = 0;
    pub const PROMOTION: u16 = 0b01 << 14;
    pub const EN_PASSANT: u16 = 0b10 << 14;
    pub const CASTLING: u16 = 0b11 << 14;
}

pub type CastlingRight = usize;
pub struct CastlingRights {}
impl CastlingRights {
    pub const NONE: CastlingRight = 0;
    pub const WHITE_KINGSIDE: CastlingRight = 1;
    pub const WHITE_QUEENSIDE: CastlingRight = 1 << 1;
    pub const BLACK_KINGSIDE: CastlingRight = 1 << 2;
    pub const BLACK_QUEENSIDE: CastlingRight = 1 << 3;
    pub const WHITE: CastlingRight = CastlingRights::WHITE_KINGSIDE | CastlingRights::WHITE_QUEENSIDE;
    pub const BLACK: CastlingRight = CastlingRights::BLACK_KINGSIDE | CastlingRights::BLACK_QUEENSIDE;
}

pub fn pawn_push(side: Side) -> Direction {
    match side {
        Sides::WHITE => Directions::UP,
        Sides::BLACK => Directions::DOWN,
        _ => panic!("Invalid side"),
    }
}

#[derive(Copy, Clone, PartialEq)]
pub struct Move {
    data: u16,
}
impl Move {
    pub fn new(data: u16) -> Self {
        Self { data }
    }

    pub fn with_from_to(from: Square, to: Square) -> Self {
        Self::new(((from << 6) + to) as u16)
    }

    pub fn make(from: Square, to: Square, promotion_type: Piece, movetype: MoveType) -> Self {
        let promotion_value = match promotion_type {
            PieceType::KNIGHT => 0,
            PieceType::BISHOP => PieceType::BISHOP - PieceType::KNIGHT,
            PieceType::ROOK => PieceType::ROOK - PieceType::KNIGHT,
            PieceType::QUEEN => PieceType::QUEEN - PieceType::KNIGHT,
            _ => 0,
        };
        Self::new(movetype + (promotion_value << 12) as u16 + (from << 6) as u16 + to as u16)
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn from_sq(self) -> Square {
        (self.data >> 6) as Square & 0b111111
    }

    pub fn to_sq(self) -> Square {
        (self.data & 0b111111) as Square
    }

    pub fn type_of(&self) -> MoveType {
        self.data & 0xC000
    }

    pub fn promotion_type(&self) -> Piece {
        if self.type_of() != MoveTypes::PROMOTION {
            return PieceType::NONE;
        }
        ((self.data >> 12) & 0b11) as usize + PieceType::KNIGHT
    }

    #[allow(dead_code)]
    pub fn is_ok(&self) -> bool {
        Self::none().data != self.data && Self::null().data != self.data
    }

    #[allow(dead_code)]
    pub fn null() -> Self {
        Self { data: 65 }
    }

    pub fn none() -> Self {
        Self { data: 0 }
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.data == 0 || self.data == 65 {
            return write!(f, "0000");
        }

        let mut to = self.to_sq();

        if self.type_of() == MoveTypes::CASTLING {
            to = match self.to_sq() {
                0 => 2,
                7 => 7 - 1,
                56 => 56 + 2,
                63 => 63 - 1,
                _ => panic!("Invalid castling move"),
            };
        }

        let promotion_string = match self.promotion_type() {
            PieceType::KNIGHT => "n",
            PieceType::BISHOP => "b",
            PieceType::ROOK => "r",
            PieceType::QUEEN => "q",
            PieceType::NONE => "",
            _ => panic!("Invalid promotion type"),
        };

        write!(
            f,
            "{}{}{}",
            pretty_square(self.from_sq()),
            pretty_square(to),
            promotion_string
        )
    }
}
