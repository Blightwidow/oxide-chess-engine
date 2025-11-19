pub mod defs;
mod fen;
mod test;

use std::rc::Rc;

use crate::bitboards::defs::EMPTY;
use crate::bitboards::Bitboards;
use crate::defs::*;
use crate::misc::bits;
use crate::movegen::defs::{pawn_push, CastlingRight, CastlingRights, Move, MoveType, MoveTypes};

use self::defs::*;

#[derive(Clone)]
pub struct Position {
    pub by_type_bb: [[Bitboard; NrOf::PIECE_TYPES]; NrOf::SIDES],
    pub by_color_bb: [Bitboard; NrOf::SIDES],
    pub pinned_bb: [Bitboard; NrOf::SIDES],
    pub board: [Piece; NrOf::SQUARES],
    pub side_to_move: Side,
    pub states: Vec<StateInfo>,
    pub castling_masks: [CastlingRight; NrOf::SQUARES],
    pub zobrist: u64,
    bitboards: Rc<Bitboards>,
}

impl Position {
    pub fn new(bitboards: Rc<Bitboards>) -> Self {
        Self {
            bitboards,
            by_type_bb: [[EMPTY; NrOf::PIECE_TYPES]; NrOf::SIDES],
            by_color_bb: [EMPTY; NrOf::SIDES],
            pinned_bb: [EMPTY; NrOf::SIDES],
            board: [PieceType::NONE; NrOf::SQUARES],
            side_to_move: Sides::WHITE,
            states: vec![StateInfo::new()],
            castling_masks: Position::castling_masks(),
            zobrist: 0u64,
        }
    }

    // This assume that the move is legal
    // Illegal moves should be filtered out by the move generator before calling this function
    pub fn do_move(&mut self, mv: Move) {
        #[cfg(debug_assertions)]
        assert!(mv.is_ok());

        let us: Side = self.side_to_move;
        let them: Side = self.side_to_move ^ 1;
        let from: Square = mv.from_sq();
        let to: Square = mv.to_sq();
        let piece: Piece = self.piece_on(from);
        let move_type = mv.type_of();
        let captured: Piece = match move_type {
            MoveTypes::EN_PASSANT => self.piece_on((to as isize - pawn_push(us)) as usize),
            MoveTypes::CASTLING => PieceType::NONE,
            _ => self.piece_on(to),
        };
        let mut new_state = *self.states.last().unwrap();

        #[cfg(debug_assertions)]
        {
            assert!(color_of_piece(piece) == us);
            assert!(type_of_piece(captured) != PieceType::KING);
        }

        if captured != PieceType::NONE {
            #[cfg(debug_assertions)]
            assert!(color_of_piece(captured) == them);

            let captured_square: Square = match move_type {
                MoveTypes::EN_PASSANT => (to as isize - pawn_push(us)) as usize,
                _ => to,
            };

            self.remove_piece(captured, captured_square);
        }

        if move_type == MoveTypes::PROMOTION {
            #[cfg(debug_assertions)]
            assert!(mv.promotion_type() != PieceType::PAWN && mv.promotion_type() != PieceType::KING);

            self.remove_piece(piece, from);
            self.put_piece(make_piece(us, mv.promotion_type()), to);
        } else if move_type == MoveTypes::CASTLING {
            self.castle(us, from, to, false);
        } else {
            self.move_piece(piece, from, to);
        }

        new_state.castling_rights &= !self.castling_masks[from];
        new_state.castling_rights &= !self.castling_masks[to];

        for side in [us, them] {
            self.pinned_bb[side] = self.pinned_bb(side);
        }

        if type_of_piece(piece) == PieceType::PAWN && distance(from, to) == 2 {
            new_state.en_passant_square = match us {
                Sides::WHITE => from + 8,
                Sides::BLACK => from - 8,
                _ => panic!("Invalid side"),
            };
        } else {
            new_state.en_passant_square = 0;
        }

        self.side_to_move = them;
        new_state.captured_piece = captured;
        new_state.rule50 = match captured == PieceType::NONE {
            true => new_state.rule50 + 1,
            false => 0,
        };
        new_state.game_ply += 1;
        self.states.push(new_state);
    }

    pub fn undo_move(&mut self, mv: Move) {
        #[cfg(debug_assertions)]
        assert!(mv.is_ok());

        self.side_to_move ^= 1;
        let us: Side = self.side_to_move;
        let them: Side = self.side_to_move ^ 1;
        let from: Square = mv.from_sq();
        let to: Square = mv.to_sq();
        let mut piece: Piece = self.piece_on(to);
        let move_type: MoveType = mv.type_of();
        let last_state: StateInfo = self.states.pop().unwrap();

        #[cfg(debug_assertions)]
        {
            assert!(self.board[from] == PieceType::NONE);
            assert!(color_of_piece(piece) == us || move_type == MoveTypes::CASTLING);
            assert!(type_of_piece(last_state.captured_piece) != PieceType::KING);
        }

        if move_type == MoveTypes::PROMOTION {
            #[cfg(debug_assertions)]
            {
                assert!(type_of_piece(piece) == mv.promotion_type());
                assert!(type_of_piece(piece) >= PieceType::KNIGHT && type_of_piece(piece) < PieceType::KING);
            }

            // In case of promotion, replace the promoted piece by a pawn
            // before continuing to undo the move.
            self.remove_piece(piece, to);
            piece = make_piece(us, PieceType::PAWN);
            self.put_piece(piece, to);
        }

        if move_type == MoveTypes::CASTLING {
            self.castle(us, from, to, true);
        } else {
            self.move_piece(piece, to, from);
            if last_state.captured_piece != PieceType::NONE {
                if move_type == MoveTypes::EN_PASSANT {
                    let captured_square: Square = (to as isize - pawn_push(us)) as usize;
                    self.put_piece(last_state.captured_piece, captured_square);
                } else {
                    self.put_piece(last_state.captured_piece, to);
                }
            }
        }

        for side in [them, us] {
            self.pinned_bb[side] = self.pinned_bb(side);
        }
    }

    pub fn piece_on(&self, square: Square) -> Piece {
        #[cfg(debug_assertions)]
        assert!(is_ok(square), "Invalid square {}", square);

        self.board[square]
    }

    fn put_piece(&mut self, piece: Piece, square: Square) {
        #[cfg(debug_assertions)]
        assert!(piece < 18);

        let bb: Bitboard = square_bb(square);
        let side = color_of_piece(piece);

        self.board[square] = piece;
        self.by_type_bb[side][type_of_piece(piece)] |= bb;
        self.by_type_bb[Sides::BOTH][type_of_piece(piece)] |= bb;
        self.by_color_bb[side] |= bb;
        self.by_color_bb[Sides::BOTH] |= bb;
    }

    fn remove_piece(&mut self, piece: Piece, square: Square) {
        #[cfg(debug_assertions)]
        assert!(piece < 18);

        let bb: Bitboard = square_bb(square);
        let side = color_of_piece(piece);

        self.board[square] = PieceType::NONE;
        self.by_type_bb[side][type_of_piece(piece)] &= !bb;
        self.by_type_bb[Sides::BOTH][type_of_piece(piece)] &= !bb;
        self.by_color_bb[side] &= !bb;
        self.by_color_bb[Sides::BOTH] &= !bb;
    }

    // This function is only for moving and does not handle captures
    fn move_piece(&mut self, piece: Piece, from: Square, to: Square) {
        #[cfg(debug_assertions)]
        {
            assert!(piece < 18);
            assert!(is_ok(from));
            assert!(is_ok(to));
            assert!(self.board[from] == piece);
            assert!(self.board[to] == PieceType::NONE);
        }

        let bb_from: Bitboard = square_bb(from);
        let bb_to: Bitboard = square_bb(to);
        let side: Side = color_of_piece(piece);

        self.board[from] = PieceType::NONE;
        self.board[to] = piece;
        self.by_type_bb[side][type_of_piece(piece)] ^= bb_from | bb_to;
        self.by_type_bb[Sides::BOTH][type_of_piece(piece)] ^= bb_from | bb_to;
        self.by_color_bb[side] ^= bb_from | bb_to;
        self.by_color_bb[Sides::BOTH] ^= bb_from | bb_to;
    }

    fn castle(&mut self, side: Side, from: Square, to: Square, undo: bool) {
        #[cfg(debug_assertions)]
        assert!(side == Sides::WHITE || side == Sides::BLACK);

        let king_side: bool = to > from;
        let rook_from: Square = to;
        let rook_to: Square = match king_side {
            true => rook_from - 2,
            false => rook_from + 3,
        };
        let king_to: Square = match king_side {
            true => from + 2,
            false => from - 2,
        };

        let king: Piece = make_piece(side, PieceType::KING);
        let rook: Piece = make_piece(side, PieceType::ROOK);
        if undo {
            self.remove_piece(king, king_to);
            self.remove_piece(rook, rook_to);

            self.put_piece(king, from);
            self.put_piece(rook, rook_from);
        } else {
            self.remove_piece(king, from);
            self.remove_piece(rook, rook_from);

            self.put_piece(king, king_to);
            self.put_piece(rook, rook_to);
        }
    }

    fn clear(&mut self) {
        for square in 0..NrOf::SQUARES {
            self.remove_piece(self.piece_on(square), square)
        }

        self.pinned_bb = [EMPTY; NrOf::SIDES];
        self.states = vec![StateInfo::new()];
        self.castling_masks = Position::castling_masks();
        self.states = vec![StateInfo::new()];
    }

    pub fn checkers(&self, defending_side: Side) -> Vec<Square> {
        #[cfg(debug_assertions)]
        assert!(defending_side == Sides::WHITE || defending_side == Sides::BLACK);

        let mut checkers: Vec<Square> = Vec::new();
        let mut attackers_bb: Bitboard = self.by_color_bb[defending_side ^ 1];
        let kind_bb: Bitboard = self.by_type_bb[defending_side][PieceType::KING];

        while attackers_bb != EMPTY {
            let square: Square = bits::pop(&mut attackers_bb);
            let attack_bb: Bitboard =
                self.bitboards
                    .attack_bb(self.piece_on(square), square, self.by_color_bb[Sides::BOTH]);

            if attack_bb & kind_bb != EMPTY {
                checkers.push(square);
            }
        }

        checkers
    }

    fn attacks_bb(&self, side: Side, occupied: Bitboard) -> Bitboard {
        let mut attacks_bb: Bitboard = EMPTY;
        let mut opponents: Bitboard = self.by_color_bb[side];

        while opponents != EMPTY {
            let square: Square = bits::pop(&mut opponents);

            attacks_bb |= self.bitboards.attack_bb(self.piece_on(square), square, occupied);
        }

        attacks_bb
    }

    fn pinned_bb(&self, side: Side) -> Bitboard {
        #[cfg(debug_assertions)]
        assert!(side == Sides::WHITE || side == Sides::BLACK);

        let mut pinned_bb: Bitboard = EMPTY;
        let opponent: Side = side ^ 1;
        let king: Square = bits::lsb(self.by_type_bb[side][PieceType::KING]);
        let mut attackers_bb: Bitboard = self.by_type_bb[opponent][PieceType::ROOK]
            | self.by_type_bb[opponent][PieceType::QUEEN]
            | self.by_type_bb[opponent][PieceType::BISHOP];

        while attackers_bb != EMPTY {
            let square: Square = bits::pop(&mut attackers_bb);
            let aligned_pieces_bb: Bitboard = self.by_color_bb[Sides::BOTH]
                & self.bitboards.between_bb[square][king]
                & self.bitboards.attack_bb(self.piece_on(square), square, square_bb(king));

            if aligned_pieces_bb.count_ones() == 1 {
                pinned_bb |= aligned_pieces_bb;
            }
        }

        pinned_bb
    }

    pub fn legal(&self, mv: Move) -> bool {
        let us: Side = self.side_to_move;
        let them: Side = us ^ 1;
        let from: Square = mv.from_sq();
        let to: Square = mv.to_sq();
        let piece: Piece = self.piece_on(from);
        let move_type = mv.type_of();

        #[cfg(debug_assertions)]
        {
            assert!(mv.is_ok());
            assert!(color_of_piece(piece) == us);
        }

        // En passant captures are a tricky special case. Because they are rather
        // uncommon, we do it simply by testing whether the king is attacked after
        // the move is made.
        if move_type == MoveTypes::EN_PASSANT {
            let captured_square: Square = (to as isize - pawn_push(us)) as usize;
            let occupied =
                (self.by_color_bb[Sides::BOTH] & !square_bb(from) & !square_bb(captured_square)) | square_bb(to);

            #[cfg(debug_assertions)]
            {
                assert!(self.piece_on(captured_square) == make_piece(them, PieceType::PAWN));
                assert!(self.piece_on(to) == PieceType::NONE);
                assert!(self.piece_on(from) == make_piece(us, PieceType::PAWN));
            }

            return self.attacks_bb(them, occupied) & self.by_type_bb[us][PieceType::KING] == EMPTY;
        }

        // Castling moves generation does not check if the castling path is clear of
        // enemy attacks, it is delayed at a later time: now!
        if move_type == MoveTypes::CASTLING {
            let between_bb = self.bitboards.between_bb[from][to];

            if between_bb & CASTLING_DESTINATION_BB & self.attacks_bb(them, self.by_color_bb[Sides::BOTH]) != EMPTY
                || between_bb & self.by_color_bb[Sides::BOTH] != EMPTY
            {
                return false;
            }

            return true;
        }

        // If the moving piece is a king, check whether the destination square is
        // attacked by the opponent.
        if type_of_piece(piece) == PieceType::KING {
            return (self.attacks_bb(them, self.by_color_bb[Sides::BOTH] ^ square_bb(from))) & square_bb(to) == EMPTY;
        }

        // A non-king move is legal if and only if it is not pinned or it
        // is moving along the ray towards or away from the king.
        self.pinned_bb[us] & square_bb(from) == EMPTY
            || self
                .bitboards
                .aligned(to, from, bits::lsb(self.by_type_bb[us][PieceType::KING]))
    }

    fn castling_masks() -> [usize; NrOf::SQUARES] {
        let mut masks: [usize; NrOf::SQUARES] = [0; NrOf::SQUARES];

        masks[square_of(0, 0)] = CastlingRights::WHITE_QUEENSIDE;
        masks[square_of(7, 0)] = CastlingRights::WHITE_KINGSIDE;
        masks[square_of(0, 7)] = CastlingRights::BLACK_QUEENSIDE;
        masks[square_of(7, 7)] = CastlingRights::BLACK_KINGSIDE;
        masks[square_of(4, 0)] = CastlingRights::WHITE;
        masks[square_of(4, 7)] = CastlingRights::BLACK;

        masks
    }
}
