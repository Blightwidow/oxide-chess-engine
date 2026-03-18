pub mod defs;
mod fen;
mod test;

use std::rc::Rc;

use crate::bitboards::defs::EMPTY;
use crate::bitboards::Bitboards;
use crate::defs::*;
use crate::hash::Hasher;
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
    pub pawn_hash: u64,
    bitboards: Rc<Bitboards>,
    hasher: Rc<Hasher>,
}

impl Position {
    pub fn new(bitboards: Rc<Bitboards>, hasher: Rc<Hasher>) -> Self {
        Self {
            bitboards,
            hasher,
            by_type_bb: [[EMPTY; NrOf::PIECE_TYPES]; NrOf::SIDES],
            by_color_bb: [EMPTY; NrOf::SIDES],
            pinned_bb: [EMPTY; NrOf::SIDES],
            board: [PieceType::NONE; NrOf::SQUARES],
            side_to_move: Sides::WHITE,
            states: vec![StateInfo::new()],
            castling_masks: Position::castling_masks(),
            zobrist: 0u64,
            pawn_hash: 0u64,
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
        new_state.zobrist = self.zobrist;
        new_state.pawn_hash = self.pawn_hash;

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

        new_state.pinned_bb = [self.pinned_bb[0], self.pinned_bb[1]];
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
            new_state.en_passant_square = NONE_SQUARE;
        }

        // Zobrist: update hash
        let old_state = self.states.last().unwrap();
        // XOR out old castling, XOR in new castling
        self.zobrist ^= self.hasher.castling_keys[old_state.castling_rights];
        self.zobrist ^= self.hasher.castling_keys[new_state.castling_rights];
        // XOR out old EP file if any
        if old_state.en_passant_square != NONE_SQUARE {
            self.zobrist ^= self.hasher.en_passant_keys[file_of(old_state.en_passant_square)];
        }
        // XOR in new EP file if any
        if new_state.en_passant_square != NONE_SQUARE {
            self.zobrist ^= self.hasher.en_passant_keys[file_of(new_state.en_passant_square)];
        }

        // Piece updates
        if captured != PieceType::NONE {
            let captured_square: Square = match move_type {
                MoveTypes::EN_PASSANT => (to as isize - pawn_push(us)) as usize,
                _ => to,
            };
            self.zobrist ^= self
                .hasher
                .piece_key(color_of_piece(captured), type_of_piece(captured), captured_square);
            // Pawn hash: remove captured pawn
            if type_of_piece(captured) == PieceType::PAWN {
                self.pawn_hash ^= self.hasher.pawn_keys[color_of_piece(captured)][captured_square];
            }
        }
        if move_type == MoveTypes::PROMOTION {
            self.zobrist ^= self.hasher.piece_key(us, PieceType::PAWN, from);
            self.zobrist ^= self.hasher.piece_key(us, mv.promotion_type(), to);
            // Pawn hash: remove promoted pawn
            self.pawn_hash ^= self.hasher.pawn_keys[us][from];
        } else if move_type == MoveTypes::CASTLING {
            let king_side: bool = to > from;
            let rook_to: Square = match king_side {
                true => to - 2,
                false => to + 3,
            };
            let king_to: Square = match king_side {
                true => from + 2,
                false => from - 2,
            };
            self.zobrist ^= self.hasher.piece_key(us, PieceType::KING, from);
            self.zobrist ^= self.hasher.piece_key(us, PieceType::KING, king_to);
            self.zobrist ^= self.hasher.piece_key(us, PieceType::ROOK, to);
            self.zobrist ^= self.hasher.piece_key(us, PieceType::ROOK, rook_to);
        } else {
            self.zobrist ^= self.hasher.piece_key(us, type_of_piece(piece), from);
            self.zobrist ^= self.hasher.piece_key(us, type_of_piece(piece), to);
            // Pawn hash: update for pawn moves
            if type_of_piece(piece) == PieceType::PAWN {
                self.pawn_hash ^= self.hasher.pawn_keys[us][from];
                self.pawn_hash ^= self.hasher.pawn_keys[us][to];
            }
        }

        // Toggle side
        self.zobrist ^= self.hasher.side_key;

        self.side_to_move = them;
        new_state.captured_piece = captured;
        new_state.rule50 = if captured != PieceType::NONE || type_of_piece(piece) == PieceType::PAWN {
            0
        } else {
            new_state.rule50 + 1
        };
        new_state.game_ply += 1;
        self.states.push(new_state);
    }

    pub fn do_null_move(&mut self) {
        let mut new_state = *self.states.last().unwrap();
        new_state.zobrist = self.zobrist;
        new_state.pawn_hash = self.pawn_hash;

        // XOR out en passant from zobrist if any, set EP to NONE_SQUARE
        if new_state.en_passant_square != NONE_SQUARE {
            self.zobrist ^= self.hasher.en_passant_keys[file_of(new_state.en_passant_square)];
            new_state.en_passant_square = NONE_SQUARE;
        }

        // Toggle side in zobrist
        self.zobrist ^= self.hasher.side_key;
        self.side_to_move ^= 1;

        new_state.captured_piece = PieceType::NONE;
        new_state.rule50 += 1;
        new_state.game_ply += 1;
        self.states.push(new_state);
    }

    pub fn undo_null_move(&mut self) {
        let last_state = self.states.pop().unwrap();
        self.zobrist = last_state.zobrist;
        self.pawn_hash = last_state.pawn_hash;
        self.side_to_move ^= 1;
    }

    pub fn undo_move(&mut self, mv: Move) {
        #[cfg(debug_assertions)]
        assert!(mv.is_ok());

        self.side_to_move ^= 1;
        let us: Side = self.side_to_move;
        let from: Square = mv.from_sq();
        let to: Square = mv.to_sq();
        let mut piece: Piece = self.piece_on(to);
        let move_type: MoveType = mv.type_of();
        let last_state: StateInfo = self.states.pop().unwrap();
        self.zobrist = last_state.zobrist;
        self.pawn_hash = last_state.pawn_hash;

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

        self.pinned_bb[0] = last_state.pinned_bb[0];
        self.pinned_bb[1] = last_state.pinned_bb[1];
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
        self.pawn_hash = 0;
    }

    pub fn checkers_bb(&self, defending_side: Side) -> Bitboard {
        #[cfg(debug_assertions)]
        assert!(defending_side == Sides::WHITE || defending_side == Sides::BLACK);

        let them = defending_side ^ 1;
        let ksq = bits::lsb(self.by_type_bb[defending_side][PieceType::KING]);
        let occupied = self.by_color_bb[Sides::BOTH];

        (self
            .bitboards
            .attack_bb(make_piece(defending_side, PieceType::KNIGHT), ksq, occupied)
            & self.by_type_bb[them][PieceType::KNIGHT])
            | (self
                .bitboards
                .attack_bb(make_piece(defending_side, PieceType::BISHOP), ksq, occupied)
                & (self.by_type_bb[them][PieceType::BISHOP] | self.by_type_bb[them][PieceType::QUEEN]))
            | (self
                .bitboards
                .attack_bb(make_piece(defending_side, PieceType::ROOK), ksq, occupied)
                & (self.by_type_bb[them][PieceType::ROOK] | self.by_type_bb[them][PieceType::QUEEN]))
            | (self
                .bitboards
                .attack_bb(make_piece(defending_side, PieceType::PAWN), ksq, EMPTY)
                & self.by_type_bb[them][PieceType::PAWN])
    }

    pub fn is_square_attacked(&self, sq: Square, by_side: Side, occupied: Bitboard) -> bool {
        let bb = &self.by_type_bb[by_side];
        let them = occupied & self.by_color_bb[by_side];
        (self
            .bitboards
            .attack_bb(make_piece(by_side ^ 1, PieceType::PAWN), sq, EMPTY)
            & bb[PieceType::PAWN]
            & them
            != EMPTY)
            || (self.bitboards.attack_bb(make_piece(0, PieceType::KNIGHT), sq, occupied) & bb[PieceType::KNIGHT] & them
                != EMPTY)
            || (self.bitboards.attack_bb(make_piece(0, PieceType::BISHOP), sq, occupied)
                & (bb[PieceType::BISHOP] | bb[PieceType::QUEEN])
                & them
                != EMPTY)
            || (self.bitboards.attack_bb(make_piece(0, PieceType::ROOK), sq, occupied)
                & (bb[PieceType::ROOK] | bb[PieceType::QUEEN])
                & them
                != EMPTY)
            || (self.bitboards.attack_bb(make_piece(0, PieceType::KING), sq, occupied) & bb[PieceType::KING] & them
                != EMPTY)
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

            let ksq = bits::lsb(self.by_type_bb[us][PieceType::KING]);
            return !self.is_square_attacked(ksq, them, occupied);
        }

        // Castling moves generation does not check if the castling path is clear of
        // enemy attacks, it is delayed at a later time: now!
        if move_type == MoveTypes::CASTLING {
            let between_bb = self.bitboards.between_bb[from][to];
            let occupied = self.by_color_bb[Sides::BOTH];

            if between_bb & occupied != EMPTY {
                return false;
            }

            let mut path = between_bb & CASTLING_DESTINATION_BB;
            while path != EMPTY {
                let sq = bits::pop(&mut path);
                if self.is_square_attacked(sq, them, occupied) {
                    return false;
                }
            }

            return true;
        }

        // If the moving piece is a king, check whether the destination square is
        // attacked by the opponent.
        if type_of_piece(piece) == PieceType::KING {
            return !self.is_square_attacked(to, them, self.by_color_bb[Sides::BOTH] ^ square_bb(from));
        }

        // A non-king move is legal if and only if it is not pinned or it
        // is moving along the ray towards or away from the king.
        self.pinned_bb[us] & square_bb(from) == EMPTY
            || self
                .bitboards
                .aligned(to, from, bits::lsb(self.by_type_bb[us][PieceType::KING]))
    }

    pub fn display(&self) -> String {
        let piece_char = |piece: Piece| -> char {
            let c = match type_of_piece(piece) {
                PieceType::PAWN => 'p',
                PieceType::KNIGHT => 'n',
                PieceType::BISHOP => 'b',
                PieceType::ROOK => 'r',
                PieceType::QUEEN => 'q',
                PieceType::KING => 'k',
                _ => ' ',
            };
            if color_of_piece(piece) == Sides::WHITE {
                c.to_ascii_uppercase()
            } else {
                c
            }
        };

        let separator = "  +---+---+---+---+---+---+---+---+";
        let mut result = String::new();
        result.push_str(separator);
        result.push('\n');

        for rank in (0..8).rev() {
            result.push_str(&format!("{} ", rank + 1));
            for file in 0..8 {
                let sq = square_of(file, rank);
                let piece = self.board[sq];
                let c = if piece == PieceType::NONE {
                    ' '
                } else {
                    piece_char(piece)
                };
                result.push_str(&format!("| {} ", c));
            }
            result.push_str("|\n");
            result.push_str(separator);
            result.push('\n');
        }

        result.push_str("    a   b   c   d   e   f   g   h\n");
        result
    }

    pub fn attack_bb(&self, piece: Piece, sq: Square, occupied: Bitboard) -> Bitboard {
        self.bitboards.attack_bb(piece, sq, occupied)
    }

    /// Check if a move is pseudo-legal (valid piece movement, ignoring check legality).
    /// Used to validate TT moves and killer moves before calling legal().
    pub fn is_pseudo_legal(&self, mv: Move) -> bool {
        let from = mv.from_sq();
        let to = mv.to_sq();

        if from >= 64 || to >= 64 {
            return false;
        }

        let piece = self.board[from];
        let us = self.side_to_move;

        // No piece on source, or wrong color
        if piece == PieceType::NONE || color_of_piece(piece) != us {
            return false;
        }

        // Can't capture opponent's king
        if self.board[to] != PieceType::NONE && type_of_piece(self.board[to]) == PieceType::KING {
            return false;
        }

        let piece_type = type_of_piece(piece);
        let move_type = mv.type_of();

        match move_type {
            MoveTypes::CASTLING => {
                if piece_type != PieceType::KING {
                    return false;
                }
                let to_piece = self.board[to];
                // to square should have our rook
                if to_piece == PieceType::NONE
                    || type_of_piece(to_piece) != PieceType::ROOK
                    || color_of_piece(to_piece) != us
                {
                    return false;
                }
                // Castling rights must include this specific castling
                self.castling_masks[from] & self.castling_masks[to] & self.states.last().unwrap().castling_rights != 0
            }
            MoveTypes::EN_PASSANT => {
                if piece_type != PieceType::PAWN {
                    return false;
                }
                let ep_sq = self.states.last().unwrap().en_passant_square;
                to == ep_sq && ep_sq != NONE_SQUARE
            }
            MoveTypes::PROMOTION => {
                if piece_type != PieceType::PAWN {
                    return false;
                }
                let rank_7 = if us == Sides::WHITE { 6 } else { 1 };
                if rank_of(from) != rank_7 {
                    return false;
                }
                let up = pawn_push(us);
                let forward = (from as isize + up) as usize;
                if to == forward {
                    // Quiet promotion
                    self.board[to] == PieceType::NONE
                } else {
                    // Capture promotion
                    let them = us ^ 1;
                    self.board[to] != PieceType::NONE
                        && color_of_piece(self.board[to]) == them
                        && self.attack_bb(piece, from, EMPTY) & square_bb(to) != EMPTY
                }
            }
            _ => {
                // Normal move — can't capture own piece
                if self.board[to] != PieceType::NONE && color_of_piece(self.board[to]) == us {
                    return false;
                }

                if piece_type == PieceType::PAWN {
                    let up = pawn_push(us);
                    let forward = (from as isize + up) as usize;
                    let double = (from as isize + 2 * up) as usize;
                    let rank_2 = if us == Sides::WHITE { 1 } else { 6 };

                    if to == forward {
                        self.board[to] == PieceType::NONE
                    } else if to == double {
                        rank_of(from) == rank_2
                            && self.board[forward] == PieceType::NONE
                            && self.board[to] == PieceType::NONE
                    } else {
                        // Pawn capture
                        let them = us ^ 1;
                        self.board[to] != PieceType::NONE
                            && color_of_piece(self.board[to]) == them
                            && self.attack_bb(piece, from, EMPTY) & square_bb(to) != EMPTY
                    }
                } else {
                    // Non-pawn: check attack bitboard
                    self.attack_bb(piece, from, self.by_color_bb[Sides::BOTH]) & square_bb(to) != EMPTY
                }
            }
        }
    }

    pub fn attackers_to(&self, sq: Square, occupied: Bitboard) -> Bitboard {
        (self
            .bitboards
            .attack_bb(make_piece(Sides::WHITE, PieceType::PAWN), sq, EMPTY)
            & self.by_type_bb[Sides::BLACK][PieceType::PAWN])
            | (self
                .bitboards
                .attack_bb(make_piece(Sides::BLACK, PieceType::PAWN), sq, EMPTY)
                & self.by_type_bb[Sides::WHITE][PieceType::PAWN])
            | (self.bitboards.attack_bb(make_piece(0, PieceType::KNIGHT), sq, occupied)
                & self.by_type_bb[Sides::BOTH][PieceType::KNIGHT])
            | (self.bitboards.attack_bb(make_piece(0, PieceType::BISHOP), sq, occupied)
                & (self.by_type_bb[Sides::BOTH][PieceType::BISHOP] | self.by_type_bb[Sides::BOTH][PieceType::QUEEN]))
            | (self.bitboards.attack_bb(make_piece(0, PieceType::ROOK), sq, occupied)
                & (self.by_type_bb[Sides::BOTH][PieceType::ROOK] | self.by_type_bb[Sides::BOTH][PieceType::QUEEN]))
            | (self.bitboards.attack_bb(make_piece(0, PieceType::KING), sq, occupied)
                & self.by_type_bb[Sides::BOTH][PieceType::KING])
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
