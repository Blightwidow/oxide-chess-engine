use arrayvec::ArrayVec;

use crate::{
    bitboards::defs::EMPTY,
    defs::*,
    misc::bits,
    movegen::{
        defs::{Move, MoveTypes},
        Movegen,
    },
    position::Position,
};

use super::SEE_VALUES;

// ─── MovePicker Stages ───────────────────────────────────────────────────────

const STAGE_TT_MOVE: u8 = 0;
const STAGE_GENERATE_CAPTURES: u8 = 1;
const STAGE_GOOD_CAPTURES: u8 = 2;
const STAGE_KILLERS: u8 = 3;
const STAGE_GENERATE_QUIETS: u8 = 4;
const STAGE_QUIETS: u8 = 5;
const STAGE_BAD_CAPTURES: u8 = 6;
const STAGE_DONE: u8 = 7;

/// Staged move picker for alpha-beta search.
/// Yields moves lazily: TT move → good captures → killers/countermove → quiets → bad captures.
/// Bad captures (SEE < 0) are deferred after quiets to avoid wasting time on losing exchanges.
pub struct MovePicker {
    stage: u8,
    tt_move: Move,
    killers: [Move; 2],
    countermove: Move,
    captures: ArrayVec<(Move, i32), 256>,
    bad_captures: ArrayVec<(Move, i32), 256>,
    quiets: ArrayVec<(Move, i32), 256>,
    cap_idx: usize,
    bad_cap_idx: usize,
    quiet_idx: usize,
    killer_idx: usize,
    /// Check evasion mask: non-king moves must target a square in this mask.
    /// !EMPTY when not in check, between_bb|checker for single check, EMPTY for double check.
    check_mask: Bitboard,
    pub skip_quiets: bool,
}

impl MovePicker {
    pub fn new(tt_move: Move, killers: [Move; 2], countermove: Move, check_mask: Bitboard) -> Self {
        Self {
            stage: STAGE_TT_MOVE,
            tt_move,
            killers,
            countermove,
            captures: ArrayVec::new(),
            bad_captures: ArrayVec::new(),
            quiets: ArrayVec::new(),
            cap_idx: 0,
            bad_cap_idx: 0,
            quiet_idx: 0,
            killer_idx: 0,
            check_mask,
            skip_quiets: false,
        }
    }

    /// Get the next move. Returns None when all stages are exhausted.
    /// Performs legality checking: only legal moves are returned.
    #[allow(clippy::too_many_arguments)]
    pub fn next(
        &mut self,
        position: &Position,
        movegen: &Movegen,
        history: &[[i32; 64]; 64],
        conthist_1ply: Option<&[[i32; 64]; 7]>,
        conthist_2ply: Option<&[[i32; 64]; 7]>,
        capture_history: &[[[i32; 7]; 64]; 7],
    ) -> Option<Move> {
        loop {
            match self.stage {
                STAGE_TT_MOVE => {
                    self.stage = STAGE_GENERATE_CAPTURES;
                    if self.tt_move != Move::none()
                        && position.is_pseudo_legal(self.tt_move)
                        && obeys_check_mask(self.tt_move, self.check_mask, position)
                        && is_legal(self.tt_move, position)
                    {
                        return Some(self.tt_move);
                    }
                }
                STAGE_GENERATE_CAPTURES => {
                    self.generate_captures(position, movegen, capture_history);
                    self.stage = STAGE_GOOD_CAPTURES;
                }
                STAGE_GOOD_CAPTURES => {
                    if let Some(mv) = self.next_capture() {
                        if mv != self.tt_move {
                            return Some(mv);
                        }
                    } else {
                        self.stage = STAGE_KILLERS;
                    }
                }
                STAGE_KILLERS => {
                    if let Some(mv) = self.next_killer(position) {
                        return Some(mv);
                    } else {
                        self.stage = if self.skip_quiets {
                            STAGE_BAD_CAPTURES
                        } else {
                            STAGE_GENERATE_QUIETS
                        };
                    }
                }
                STAGE_GENERATE_QUIETS => {
                    self.generate_quiets(position, movegen, history, conthist_1ply, conthist_2ply);
                    self.stage = STAGE_QUIETS;
                }
                STAGE_QUIETS => {
                    if let Some(mv) = self.next_quiet() {
                        if mv != self.tt_move
                            && mv != self.killers[0]
                            && mv != self.killers[1]
                            && mv != self.countermove
                        {
                            return Some(mv);
                        }
                    } else {
                        self.stage = STAGE_BAD_CAPTURES;
                    }
                }
                STAGE_BAD_CAPTURES => {
                    if let Some(mv) = self.next_bad_capture() {
                        if mv != self.tt_move {
                            return Some(mv);
                        }
                    } else {
                        self.stage = STAGE_DONE;
                    }
                }
                _ => return None,
            }
        }
    }

    fn generate_captures(&mut self, position: &Position, movegen: &Movegen, capture_history: &[[[i32; 7]; 64]; 7]) {
        let raw = movegen.generate_captures(position);
        let us = position.side_to_move;
        let king_square = bits::lsb(position.by_type_bb[us][PieceType::KING]);

        for mv in raw.iter() {
            if !is_legal_fast(*mv, us, king_square, position) {
                continue;
            }
            let mut score = score_capture(*mv, position);
            let to = mv.to_sq();
            let move_type = mv.type_of();
            let is_actual_capture = position.board[to] != PieceType::NONE || move_type == MoveTypes::EN_PASSANT;

            if is_actual_capture {
                // Blend capture history with MVV-LVA base score
                let piece_type = type_of_piece(position.board[mv.from_sq()]);
                let victim_type = if move_type == MoveTypes::EN_PASSANT {
                    PieceType::PAWN
                } else {
                    type_of_piece(position.board[to])
                };
                score += capture_history[piece_type][to][victim_type] / 32;

                self.captures.push((*mv, score));
            } else {
                // Quiet promotions always go to good captures
                self.captures.push((*mv, score));
            }
        }
    }

    fn generate_quiets(
        &mut self,
        position: &Position,
        movegen: &Movegen,
        history: &[[i32; 64]; 64],
        conthist_1ply: Option<&[[i32; 64]; 7]>,
        conthist_2ply: Option<&[[i32; 64]; 7]>,
    ) {
        let raw = movegen.generate_quiets(position);
        let us = position.side_to_move;
        let king_square = bits::lsb(position.by_type_bb[us][PieceType::KING]);

        for mv in raw.iter() {
            if !is_legal_fast(*mv, us, king_square, position) {
                continue;
            }
            let piece_type = type_of_piece(position.board[mv.from_sq()]);
            let to = mv.to_sq();
            let mut score = history[mv.from_sq()][to];
            if let Some(ch1) = conthist_1ply {
                score += ch1[piece_type][to];
            }
            if let Some(ch2) = conthist_2ply {
                score += ch2[piece_type][to];
            }
            self.quiets.push((*mv, score));
        }
    }

    /// Selection sort: pick highest-scored good capture.
    fn next_capture(&mut self) -> Option<Move> {
        if self.cap_idx >= self.captures.len() {
            return None;
        }
        let mut best = self.cap_idx;
        for j in (self.cap_idx + 1)..self.captures.len() {
            if self.captures[j].1 > self.captures[best].1 {
                best = j;
            }
        }
        self.captures.swap(self.cap_idx, best);
        let mv = self.captures[self.cap_idx].0;
        self.cap_idx += 1;
        Some(mv)
    }

    /// Selection sort: pick highest-scored bad capture (deferred after quiets).
    fn next_bad_capture(&mut self) -> Option<Move> {
        if self.bad_cap_idx >= self.bad_captures.len() {
            return None;
        }
        let mut best = self.bad_cap_idx;
        for j in (self.bad_cap_idx + 1)..self.bad_captures.len() {
            if self.bad_captures[j].1 > self.bad_captures[best].1 {
                best = j;
            }
        }
        self.bad_captures.swap(self.bad_cap_idx, best);
        let mv = self.bad_captures[self.bad_cap_idx].0;
        self.bad_cap_idx += 1;
        Some(mv)
    }

    /// Yield killer moves and countermove if they are pseudo-legal and legal.
    fn next_killer(&mut self, position: &Position) -> Option<Move> {
        while self.killer_idx < 3 {
            let mv = match self.killer_idx {
                0 => self.killers[0],
                1 => self.killers[1],
                2 => self.countermove,
                _ => unreachable!(),
            };
            self.killer_idx += 1;

            if mv == Move::none() || mv == self.tt_move {
                continue;
            }
            // Dedup: killer[1] == killer[0], or countermove == earlier killer
            if self.killer_idx == 2 && mv == self.killers[0] {
                continue;
            }
            if self.killer_idx == 3 && (mv == self.killers[0] || mv == self.killers[1]) {
                continue;
            }
            // Skip if it was already yielded as a capture
            if is_capture(mv, position) {
                continue;
            }
            // Killers from sibling nodes / countermoves need full validation
            if position.is_pseudo_legal(mv) && obeys_check_mask(mv, self.check_mask, position) && is_legal(mv, position)
            {
                return Some(mv);
            }
        }
        None
    }

    /// Selection sort: pick highest-scored quiet.
    fn next_quiet(&mut self) -> Option<Move> {
        if self.quiet_idx >= self.quiets.len() {
            return None;
        }
        let mut best = self.quiet_idx;
        for j in (self.quiet_idx + 1)..self.quiets.len() {
            if self.quiets[j].1 > self.quiets[best].1 {
                best = j;
            }
        }
        self.quiets.swap(self.quiet_idx, best);
        let mv = self.quiets[self.quiet_idx].0;
        self.quiet_idx += 1;
        Some(mv)
    }
}

// ─── QMovePicker ─────────────────────────────────────────────────────────────

const Q_STAGE_TT_MOVE: u8 = 0;
const Q_STAGE_GENERATE_CAPTURES: u8 = 1;
const Q_STAGE_CAPTURES: u8 = 2;
const Q_STAGE_DONE: u8 = 3;

/// Simplified move picker for quiescence search: TT move + captures only.
pub struct QMovePicker {
    stage: u8,
    tt_move: Move,
    check_mask: Bitboard,
    captures: ArrayVec<(Move, i32), 256>,
    cap_idx: usize,
}

impl QMovePicker {
    pub fn new(tt_move: Move, check_mask: Bitboard) -> Self {
        Self {
            stage: Q_STAGE_TT_MOVE,
            tt_move,
            check_mask,
            captures: ArrayVec::new(),
            cap_idx: 0,
        }
    }

    pub fn next(&mut self, position: &Position, movegen: &Movegen) -> Option<Move> {
        loop {
            match self.stage {
                Q_STAGE_TT_MOVE => {
                    self.stage = Q_STAGE_GENERATE_CAPTURES;
                    if self.tt_move != Move::none()
                        && position.is_pseudo_legal(self.tt_move)
                        && is_capture(self.tt_move, position)
                        && obeys_check_mask(self.tt_move, self.check_mask, position)
                        && is_legal(self.tt_move, position)
                    {
                        return Some(self.tt_move);
                    }
                }
                Q_STAGE_GENERATE_CAPTURES => {
                    self.generate_captures(position, movegen);
                    self.stage = Q_STAGE_CAPTURES;
                }
                Q_STAGE_CAPTURES => {
                    if let Some(mv) = self.next_capture() {
                        if mv != self.tt_move {
                            return Some(mv);
                        }
                    } else {
                        self.stage = Q_STAGE_DONE;
                    }
                }
                _ => return None,
            }
        }
    }

    fn generate_captures(&mut self, position: &Position, movegen: &Movegen) {
        let raw = movegen.generate_captures(position);
        let us = position.side_to_move;
        let king_square = bits::lsb(position.by_type_bb[us][PieceType::KING]);

        for mv in raw.iter() {
            // Qsearch only searches captures (including capture-promotions and EP)
            if !is_capture(*mv, position) {
                continue;
            }
            if !is_legal_fast(*mv, us, king_square, position) {
                continue;
            }
            let score = score_capture(*mv, position);
            self.captures.push((*mv, score));
        }
    }

    fn next_capture(&mut self) -> Option<Move> {
        if self.cap_idx >= self.captures.len() {
            return None;
        }
        let mut best = self.cap_idx;
        for j in (self.cap_idx + 1)..self.captures.len() {
            if self.captures[j].1 > self.captures[best].1 {
                best = j;
            }
        }
        self.captures.swap(self.cap_idx, best);
        let mv = self.captures[self.cap_idx].0;
        self.cap_idx += 1;
        Some(mv)
    }
}

// ─── Shared Helpers ──────────────────────────────────────────────────────────

/// Fast legality check for moves from the generator (which already applies check_target).
/// Skip the expensive `position.legal()` when the piece is not pinned, not the king,
/// and not en passant.
#[inline]
fn is_legal_fast(mv: Move, us: Side, king_square: Square, position: &Position) -> bool {
    let from_bb = square_bb(mv.from_sq());
    if position.pinned_bb[us] & from_bb == EMPTY && king_square != mv.from_sq() && mv.type_of() != MoveTypes::EN_PASSANT
    {
        true
    } else {
        position.legal(mv)
    }
}

/// Full legality check for TT moves and killers (which bypass the generator's check_target).
#[inline]
fn is_legal(mv: Move, position: &Position) -> bool {
    let us = position.side_to_move;
    let king_square = bits::lsb(position.by_type_bb[us][PieceType::KING]);
    is_legal_fast(mv, us, king_square, position)
}

/// Check that a non-generator move (TT/killer) obeys the check evasion constraint.
/// King moves always pass (they resolve check by moving). Non-king moves must
/// target a square in check_mask (blocking or capturing the checker).
#[inline]
fn obeys_check_mask(mv: Move, check_mask: Bitboard, position: &Position) -> bool {
    let us = position.side_to_move;
    let king_square = bits::lsb(position.by_type_bb[us][PieceType::KING]);
    if mv.from_sq() == king_square {
        true // King moves resolve check by moving
    } else {
        square_bb(mv.to_sq()) & check_mask != EMPTY
    }
}

/// Check if a move is a capture (including en passant).
#[inline]
fn is_capture(mv: Move, position: &Position) -> bool {
    position.board[mv.to_sq()] != PieceType::NONE || mv.type_of() == MoveTypes::EN_PASSANT
}

/// Score a capture using MVV-LVA (matching the old score_move ordering).
/// Capture-promotions are scored by victim piece (same as regular captures).
/// Quiet promotions are scored by promotion piece type.
#[inline]
fn score_capture(mv: Move, position: &Position) -> i32 {
    let move_type = mv.type_of();
    let to_sq = mv.to_sq();
    // Captures (including capture-promotions): score by MVV-LVA
    let is_capture = position.board[to_sq] != PieceType::NONE || move_type == MoveTypes::EN_PASSANT;
    if is_capture {
        let victim_value = if move_type == MoveTypes::EN_PASSANT {
            SEE_VALUES[PieceType::PAWN] as i32
        } else {
            SEE_VALUES[type_of_piece(position.board[to_sq])] as i32
        };
        let attacker_value = SEE_VALUES[type_of_piece(position.board[mv.from_sq()])] as i32;
        return 100_000 + victim_value * 100 - attacker_value;
    }
    // Quiet promotions: score by promotion piece type
    if move_type == MoveTypes::PROMOTION {
        return 100_000 + SEE_VALUES[mv.promotion_type()] as i32 * 100;
    }
    0
}
