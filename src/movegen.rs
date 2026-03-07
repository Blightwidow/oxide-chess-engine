pub mod defs;

use std::rc::Rc;

use arrayvec::ArrayVec;

use crate::{
    bitboards::{defs::*, Bitboards},
    defs::*,
    misc::bits,
    position::Position,
};

use self::defs::*;

pub type MoveList = ArrayVec<Move, 256>;

pub struct Movegen {
    bitboards: Rc<Bitboards>,
}

impl Movegen {
    pub fn new(bitboards: Rc<Bitboards>) -> Self {
        Self { bitboards }
    }

    pub fn legal_moves(&self, position: &Position) -> MoveList {
        let us = position.side_to_move;
        let king_square: Square = bits::lsb(position.by_type_bb[us][PieceType::KING]);

        let mut movelist = self.generate(position, us);
        movelist.retain(|mv| {
            (position.pinned_bb[us] & square_bb(mv.from_sq()) == EMPTY
                && king_square != mv.from_sq()
                && mv.type_of() != MoveTypes::EN_PASSANT)
                || position.legal(*mv)
        });

        movelist
    }

    fn generate(&self, position: &Position, us: Side) -> MoveList {
        let mut movelist: MoveList = ArrayVec::new();
        let checkers = position.checkers_bb(us);
        let king_square = bits::lsb(position.by_type_bb[us][PieceType::KING]);
        let num_checkers = checkers.count_ones();
        let target_bb: Bitboard = match num_checkers {
            1 => {
                let checker_sq = bits::lsb(checkers);
                self.bitboards.between_bb[king_square][checker_sq] | square_bb(checker_sq)
            }
            _ => FULL,
        };

        if num_checkers <= 1 {
            self.generate_pawns(position, &mut movelist, us, target_bb);
            self.generate_piece(position, &mut movelist, PieceType::KNIGHT, us, target_bb);
            self.generate_piece(position, &mut movelist, PieceType::BISHOP, us, target_bb);
            self.generate_piece(position, &mut movelist, PieceType::ROOK, us, target_bb);
            self.generate_piece(position, &mut movelist, PieceType::QUEEN, us, target_bb);
        }

        self.generate_piece(position, &mut movelist, PieceType::KING, us, FULL);

        if checkers == EMPTY {
            self.generate_castling(position, &mut movelist, us);
        }

        movelist
    }

    fn generate_pawns(&self, position: &Position, movelist: &mut MoveList, us: Side, target_bb: Bitboard) {
        let them: Side = us ^ 1;
        let up: Direction = match us {
            Sides::WHITE => Directions::UP,
            Sides::BLACK => Directions::DOWN,
            _ => panic!("Invalid side"),
        };
        let rank_7bb: Bitboard = match us {
            Sides::WHITE => RANK_7BB,
            Sides::BLACK => RANK_2BB,
            _ => panic!("Invalid side"),
        };
        let rank_3bb: Bitboard = match us {
            Sides::WHITE => RANK_3BB,
            Sides::BLACK => RANK_6BB,
            _ => panic!("Invalid side"),
        };
        let empty_squares: Bitboard = !position.by_color_bb[Sides::BOTH];
        let pawns_on_rank_7: Bitboard = position.by_type_bb[us][PieceType::PAWN] & rank_7bb;
        let pawns_outside_rank_7: Bitboard = position.by_type_bb[us][PieceType::PAWN] & !rank_7bb;
        let piece = make_piece(us, PieceType::PAWN);

        let mut single_bb: Bitboard = shift(pawns_outside_rank_7, up) & empty_squares;
        // We generate double pawn pushes from the first push to take blockers on the 3rd rank into account
        let mut double_bb: Bitboard = shift(single_bb & rank_3bb, up) & empty_squares & target_bb;
        // Then we filter with the target squares
        single_bb &= target_bb;

        while single_bb != EMPTY {
            let to: Square = bits::pop(&mut single_bb);
            movelist.push(Move::with_from_to((to as isize - up) as usize, to))
        }

        while double_bb != EMPTY {
            let to: Square = bits::pop(&mut double_bb);
            movelist.push(Move::with_from_to((to as isize - up - up) as usize, to))
        }

        if pawns_on_rank_7 != EMPTY {
            let mut promotion_bb: Bitboard = shift(pawns_on_rank_7, up) & empty_squares & target_bb;

            while promotion_bb != EMPTY {
                let to: Square = bits::pop(&mut promotion_bb);

                for piece in [PieceType::KNIGHT, PieceType::BISHOP, PieceType::ROOK, PieceType::QUEEN] {
                    movelist.push(Move::make((to as isize - up) as usize, to, piece, MoveTypes::PROMOTION))
                }
            }

            let mut attackers_bb: Bitboard = pawns_on_rank_7;

            while attackers_bb != EMPTY {
                let from: Square = bits::pop(&mut attackers_bb);
                let mut attack_bb: Bitboard =
                    self.bitboards.attack_bb(piece, from, EMPTY) & (position.by_color_bb[them]) & target_bb;

                while attack_bb != EMPTY {
                    let to: Square = bits::pop(&mut attack_bb);

                    for piece in [PieceType::QUEEN, PieceType::ROOK, PieceType::BISHOP, PieceType::KNIGHT] {
                        movelist.push(Move::make(from, to, piece, MoveTypes::PROMOTION))
                    }
                }
            }
        }

        // Pawns on rank 7 are already handled above
        let mut attackers_bb: Bitboard = pawns_outside_rank_7;
        let state = position.states.last().unwrap();
        // When in check, en passant can resolve it if the captured pawn is the checker.
        // The EP square itself won't be in target_bb, so we include it explicitly.
        let ep_target_bb: Bitboard = match state.en_passant_square {
            NONE_SQUARE => target_bb,
            ep_sq => {
                let up_dir: Direction = match us {
                    Sides::WHITE => Directions::UP,
                    _ => Directions::DOWN,
                };
                let captured_sq = (ep_sq as isize - up_dir) as usize;
                if target_bb & square_bb(captured_sq) != EMPTY {
                    target_bb | square_bb(ep_sq)
                } else {
                    target_bb
                }
            }
        };

        while attackers_bb != EMPTY {
            let from: Square = bits::pop(&mut attackers_bb);
            let en_passant_bb: Bitboard = match state.en_passant_square {
                NONE_SQUARE => EMPTY,
                square => square_bb(square),
            };
            let mut attack_bb: Bitboard =
                self.bitboards.attack_bb(piece, from, EMPTY) & (position.by_color_bb[them] | en_passant_bb) & ep_target_bb;

            while attack_bb != EMPTY {
                let to: Square = bits::pop(&mut attack_bb);
                let move_type = match to == state.en_passant_square {
                    true => MoveTypes::EN_PASSANT,
                    false => MoveTypes::NORMAL,
                };

                movelist.push(Move::make(from, to, PieceType::NONE, move_type));
            }
        }
    }

    fn generate_piece(
        &self,
        position: &Position,
        movelist: &mut MoveList,
        piece: Piece,
        us: Side,
        target_bb: Bitboard,
    ) {
        let piece_type = type_of_piece(piece);

        #[cfg(debug_assertions)]
        assert!(piece_type != PieceType::PAWN, "Invalid piece");

        let mut bitboard: Bitboard = position.by_type_bb[us][piece_type];

        while bitboard != EMPTY {
            let from = bits::pop(&mut bitboard);
            let mut attack_bb =
                self.bitboards
                    .attack_bb(make_piece(us, piece_type), from, position.by_color_bb[Sides::BOTH])
                    & !position.by_color_bb[us]
                    & target_bb;

            while attack_bb != EMPTY {
                let to = bits::pop(&mut attack_bb);
                movelist.push(Move::with_from_to(from, to));
            }
        }
    }

    fn generate_castling(&self, position: &Position, movelist: &mut MoveList, us: Side) {
        let king_square = bits::lsb(position.by_type_bb[us][PieceType::KING]);
        let mut rights = position.castling_masks[king_square] & position.states.last().unwrap().castling_rights;

        while rights != 0 {
            let right = 1 << rights.trailing_zeros() as CastlingRight;
            let to: usize = match right {
                CastlingRights::WHITE_KINGSIDE => king_square + 3,
                CastlingRights::WHITE_QUEENSIDE => king_square - 4,
                CastlingRights::BLACK_KINGSIDE => king_square + 3,
                CastlingRights::BLACK_QUEENSIDE => king_square - 4,
                _ => panic!("Invalid castling right"),
            };

            assert!(is_ok(to));

            movelist.push(Move::make(king_square, to, PieceType::NONE, MoveTypes::CASTLING));

            rights ^= right;
        }
    }
}
