pub mod defs;
mod move_picker;
mod tests;

use std::time;

use arrayvec::ArrayVec;

use crate::{
    bitboards::defs::EMPTY,
    defs::*,
    evaluate::{
        transposition::{HashData, NodeType},
        Eval,
    },
    misc::bits,
    movegen::{
        defs::{pawn_push, Move, MoveTypes},
        Movegen,
    },
    nnue::{self, NnueEval},
    position::Position,
    time::TimeManager,
};

use self::move_picker::{MovePicker, QMovePicker};

use self::defs::*;
use crate::time::defs::CHECK_INTERVAL;

/// Adjust a mate score for TT storage: convert ply-relative to root-relative.
fn adjust_mate_score_to_tt(score: i16, ply: usize) -> i16 {
    if score > VALUE_MATE - 100 {
        score + ply as i16
    } else if score < -VALUE_MATE + 100 {
        score - ply as i16
    } else {
        score
    }
}

/// Adjust a mate score from TT probe: convert root-relative back to ply-relative.
fn adjust_mate_score_from_tt(score: i16, ply: usize) -> i16 {
    if score > VALUE_MATE - 100 {
        score - ply as i16
    } else if score < -VALUE_MATE + 100 {
        score + ply as i16
    } else {
        score
    }
}

// ─── Constants ───────────────────────────────────────────────────────────────

const MAX_PLY: usize = 128;
/// Late Move Pruning: max quiet moves to search at depths 1-4
const LMP_THRESHOLDS: [usize; 5] = [0, 3, 6, 10, 15];
/// Piece values used by Static Exchange Evaluation (SEE)
const SEE_VALUES: [i16; 7] = [0, 100, 300, 300, 500, 900, 20000];
/// Number of entries in the correction history table (per side)
const CORRECTION_HISTORY_SIZE: usize = 16384;
/// Maximum absolute correction value (centipawns * CORRECTION_GRAIN)
const CORRECTION_MAX: i32 = 256 * 32;
/// Granularity for correction history values (fixed-point scaling)
const CORRECTION_GRAIN: i32 = 256;

/// Apply a history gravity update: adds bonus while naturally decaying toward zero.
/// Keeps values bounded within approximately ±16384.
#[inline]
fn update_gravity(entry: &mut i32, bonus: i32) {
    *entry += bonus - bonus * (*entry).abs() / 16384;
}

/// Allocate a zeroed continuation history table directly on the heap.
/// Avoids creating the ~800KB array on the stack (which causes stack overflow).
fn new_conthist() -> Box<[[[[i32; 64]; 7]; 64]; 7]> {
    unsafe {
        let layout = std::alloc::Layout::new::<[[[[i32; 64]; 7]; 64]; 7]>();
        let ptr = std::alloc::alloc_zeroed(layout) as *mut [[[[i32; 64]; 7]; 64]; 7];
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        Box::from_raw(ptr)
    }
}

// ─── Search State ────────────────────────────────────────────────────────────

pub struct Search {
    pub position: Position,
    pub movegen: Movegen,
    pub eval: Eval,
    pub nnue: NnueEval,
    pub nodes_searched: usize,
    seldepth: usize,
    time: TimeManager,
    start_time: time::Instant,
    /// Two killer moves per ply — quiet moves that caused beta cutoffs
    killers: [[Move; 2]; MAX_PLY],
    /// History heuristic table [from_sq][to_sq] — accumulated depth² bonus for quiet beta cutoffs
    history: [[i32; 64]; 64],
    /// Late Move Reduction table [depth][move_number] — precomputed ln(d)*ln(m)/2 reductions
    lmr_table: [[u8; 64]; 128],
    /// Countdown until next time check (avoids calling Instant::now() every node)
    nodes_until_check: usize,
    /// Hard node limit for the current search (usize::MAX = unlimited)
    nodes_limit: usize,
    /// Correction history: tracks static eval error keyed by [side][pawn_hash % SIZE].
    /// Values are stored in fixed-point (scaled by CORRECTION_GRAIN).
    correction_history: Box<[[i32; CORRECTION_HISTORY_SIZE]; 2]>,
    /// Static eval at each ply, used to compute the "improving" flag
    static_eval_stack: [i16; MAX_PLY],
    /// Countermove table: [from_sq][to_sq] → the move that refuted it
    countermoves: [[Move; 64]; 64],
    /// Previous move played (for countermove heuristic)
    prev_move: Move,
    /// Continuation history 1-ply back: [prev_piece_type][prev_to_sq][cur_piece_type][cur_to_sq]
    conthist_1ply: Box<[[[[i32; 64]; 7]; 64]; 7]>,
    /// Continuation history 2-ply back: [grandparent_piece_type][grandparent_to_sq][cur_piece_type][cur_to_sq]
    conthist_2ply: Box<[[[[i32; 64]; 7]; 64]; 7]>,
    /// Capture history: [piece_type][to_sq][victim_type]
    capture_history: Box<[[[i32; 7]; 64]; 7]>,
    /// Piece type of move made at each ply (for continuation history lookups)
    ply_piece_type: [usize; MAX_PLY],
    /// Destination square of move made at each ply (for continuation history lookups)
    ply_to_square: [usize; MAX_PLY],
}

impl Search {
    /// Initialize search with precomputed LMR table.
    pub fn new(position: Position, movegen: Movegen, eval: Eval, nnue: NnueEval) -> Self {
        // Precompute LMR reduction values: R = ln(depth) * ln(move_number) / 2
        let mut lmr_table = [[0u8; 64]; 128];
        for (depth, row) in lmr_table.iter_mut().enumerate().skip(1) {
            for (move_num, entry) in row.iter_mut().enumerate().skip(1) {
                *entry = ((depth as f64).ln() * (move_num as f64).ln() / 2.0) as u8;
            }
        }

        let mut search = Self {
            position,
            movegen,
            nodes_searched: 0,
            seldepth: 0,
            eval,
            nnue,
            time: TimeManager::default(),
            start_time: time::Instant::now(),
            killers: [[Move::none(); 2]; MAX_PLY],
            history: [[0; 64]; 64],
            lmr_table,
            nodes_until_check: CHECK_INTERVAL,
            nodes_limit: usize::MAX,
            correction_history: Box::new([[0i32; CORRECTION_HISTORY_SIZE]; 2]),
            static_eval_stack: [0i16; MAX_PLY],
            countermoves: [[Move::none(); 64]; 64],
            prev_move: Move::none(),
            conthist_1ply: new_conthist(),
            conthist_2ply: new_conthist(),
            capture_history: Box::new([[[0i32; 7]; 64]; 7]),
            ply_piece_type: [PieceType::NONE; MAX_PLY],
            ply_to_square: [0; MAX_PLY],
        };
        search.position.set(FEN_START_POSITION.to_string());
        search.nnue.refresh(&search.position);

        search
    }

    /// Entry point: reset state, run iterative deepening, and print bestmove.
    pub fn run(&mut self, limits: SearchLimits) {
        if limits.perft > 0 {
            self.start_time = time::Instant::now();
            let nodes = self.perft(limits.perft, true);
            println!("\nNodes searched: {}\n", nodes);
            return;
        }

        let result = self.run_and_return(limits);

        if limits.depth > 0 {
            if let Some(mv) = result {
                // Probe TT for ponder move
                self.position.do_move(mv);
                let ponder = self
                    .eval
                    .transposition_table
                    .probe(self.position.zobrist)
                    .map(|entry| entry.best_move)
                    .filter(|m| *m != Move::none());
                self.position.undo_move(mv);

                if let Some(ponder_mv) = ponder {
                    println!("bestmove {:?} ponder {:?}", mv, ponder_mv);
                } else {
                    println!("bestmove {:?}", mv);
                }
            } else {
                println!("bestmove 0000");
            }
        } else {
            let score = self.evaluate_position();
            println!("info depth 0 score cp {}", score);
        }
    }

    /// Run search and return best move without printing bestmove line.
    pub fn run_and_return(&mut self, limits: SearchLimits) -> Option<Move> {
        self.start_time = time::Instant::now();

        // Reset per-search state
        self.nodes_searched = 0;
        self.seldepth = 0;
        self.killers = [[Move::none(); 2]; MAX_PLY];
        self.history = [[0; 64]; 64];
        self.countermoves = [[Move::none(); 64]; 64];
        self.prev_move = Move::none();
        self.static_eval_stack = [0i16; MAX_PLY];
        self.conthist_1ply = new_conthist();
        self.conthist_2ply = new_conthist();
        *self.capture_history = [[[0i32; 7]; 64]; 7];
        self.ply_piece_type = [PieceType::NONE; MAX_PLY];
        self.ply_to_square = [0; MAX_PLY];
        self.nodes_until_check = CHECK_INTERVAL;
        self.nodes_limit = limits.nodes;

        // Increment TT generation for age-based replacement
        self.eval.transposition_table.new_generation();

        self.time = TimeManager::new(
            limits,
            self.position.side_to_move,
            self.position.states.last().unwrap().game_ply,
        );

        self.search(limits.depth).map(|(mv, _score)| mv)
    }

    fn evaluate_position(&self) -> i16 {
        self.nnue.evaluate(self.position.side_to_move)
    }

    /// Load or replace the NNUE network from a file path.
    pub fn load_nnue(&mut self, path: &str) {
        if let Some(nnue) = NnueEval::load(path) {
            println!("info string NNUE file {} loaded", path);
            self.nnue = nnue;
            self.nnue.refresh(&self.position);
        } else {
            println!("info string Failed to load NNUE net: {}", path);
        }
    }

    // ─── NNUE-Aware Move Wrappers ─────────────────────────────────────────────

    /// Apply a move with incremental NNUE accumulator updates.
    /// When a king move crosses a bucket boundary, the moving side's accumulator
    /// is refreshed from scratch; the opponent's is updated incrementally.
    fn do_move_nnue(&mut self, mv: Move) {
        let us = self.position.side_to_move;
        let them = us ^ 1;
        let from = mv.from_sq();
        let to = mv.to_sq();
        let piece = self.position.board[from];
        let piece_type = type_of_piece(piece);
        let move_type = mv.type_of();
        let captured = if move_type == MoveTypes::EN_PASSANT {
            self.position.board[(to as isize - pawn_push(us)) as usize]
        } else if move_type == MoveTypes::CASTLING {
            PieceType::NONE
        } else {
            self.position.board[to]
        };

        let is_king_move = piece_type == PieceType::KING;
        let new_king_sq = if is_king_move {
            if move_type == MoveTypes::CASTLING {
                if to > from {
                    from + 2
                } else {
                    from - 2
                }
            } else {
                to
            }
        } else {
            0 // unused
        };

        let refresh_us = is_king_move && nnue::features::needs_refresh(us, from, new_king_sq);

        self.nnue.push();
        self.position.do_move(mv);

        if is_king_move {
            self.nnue.update_king_sq(us, new_king_sq);
        }

        if refresh_us {
            // Full refresh for moving side's perspective (all features change bucket/mirror)
            self.nnue.refresh_perspective(us, &self.position);

            // Incremental update for opponent's perspective only
            match move_type {
                MoveTypes::CASTLING => {
                    let king_side = to > from;
                    let rook_to = if king_side { to - 2 } else { to + 3 };
                    let king_to = new_king_sq;

                    self.nnue.deactivate_single(them, us, PieceType::KING, from);
                    self.nnue.deactivate_single(them, us, PieceType::ROOK, to);
                    self.nnue.activate_single(them, us, PieceType::KING, king_to);
                    self.nnue.activate_single(them, us, PieceType::ROOK, rook_to);
                }
                _ => {
                    // Normal king move (possibly capture)
                    self.nnue.deactivate_single(them, us, PieceType::KING, from);
                    if captured != PieceType::NONE {
                        self.nnue.deactivate_single(them, them, type_of_piece(captured), to);
                    }
                    self.nnue.activate_single(them, us, PieceType::KING, to);
                }
            }
        } else {
            // No refresh needed — standard incremental update for both perspectives
            match move_type {
                MoveTypes::CASTLING => {
                    let king_side = to > from;
                    let rook_to = if king_side { to - 2 } else { to + 3 };
                    let king_to = if king_side { from + 2 } else { from - 2 };

                    self.nnue.deactivate(us, PieceType::KING, from);
                    self.nnue.deactivate(us, PieceType::ROOK, to);
                    self.nnue.activate(us, PieceType::KING, king_to);
                    self.nnue.activate(us, PieceType::ROOK, rook_to);
                }
                MoveTypes::PROMOTION => {
                    let promo_type = mv.promotion_type();
                    self.nnue.deactivate(us, PieceType::PAWN, from);
                    if captured != PieceType::NONE {
                        self.nnue.deactivate(them, type_of_piece(captured), to);
                    }
                    self.nnue.activate(us, promo_type, to);
                }
                MoveTypes::EN_PASSANT => {
                    let captured_sq = (to as isize - pawn_push(us)) as usize;
                    self.nnue.deactivate(us, PieceType::PAWN, from);
                    self.nnue.deactivate(them, PieceType::PAWN, captured_sq);
                    self.nnue.activate(us, PieceType::PAWN, to);
                }
                _ => {
                    self.nnue.deactivate(us, piece_type, from);
                    if captured != PieceType::NONE {
                        self.nnue.deactivate(them, type_of_piece(captured), to);
                    }
                    self.nnue.activate(us, piece_type, to);
                }
            }
        }

        #[cfg(debug_assertions)]
        self.nnue.verify(&self.position);
    }

    /// Undo a move and pop the NNUE accumulator.
    fn undo_move_nnue(&mut self, mv: Move) {
        self.position.undo_move(mv);
        self.nnue.pop();
    }

    /// Apply null move with NNUE push (no feature changes).
    fn do_null_move_nnue(&mut self) {
        self.nnue.push();
        self.position.do_null_move();
    }

    /// Undo null move and pop the NNUE accumulator.
    fn undo_null_move_nnue(&mut self) {
        self.position.undo_null_move();
        self.nnue.pop();
    }

    /// Public method for making a move with NNUE updates (used by UCI move replay).
    pub fn make_move(&mut self, mv: Move) {
        self.do_move_nnue(mv);
    }

    // ─── Perft ────────────────────────────────────────────────────────────────

    /// Performance test: count leaf nodes at a given depth. Used for move generation correctness.
    pub(crate) fn perft(&mut self, depth: u8, root: bool) -> u64 {
        let mut count: u64;
        let mut nodes: u64 = 0;
        let leaf: bool = depth == 2;
        let moves = self.movegen.legal_moves(&self.position);

        for mv in moves.iter() {
            if depth <= 1 {
                count = 1;
                nodes += 1;
            } else {
                self.position.do_move(*mv);
                count = match leaf {
                    true => self.movegen.legal_moves(&self.position).len() as u64,
                    false => self.perft(depth - 1, false),
                };
                nodes += count;
                self.position.undo_move(*mv);
            }

            if root {
                println!("{:?}: {}", mv, count);
            }
        }

        nodes
    }

    // ─── Time Management ──────────────────────────────────────────────────────

    /// Periodically check if we've exceeded the hard time or node limit.
    /// Returns true if search should abort immediately.
    fn check_time(&mut self) -> bool {
        if self.nodes_searched >= self.nodes_limit {
            return true;
        }
        self.nodes_until_check -= 1;
        if self.nodes_until_check == 0 {
            self.nodes_until_check = CHECK_INTERVAL;
            return self.time.should_stop_hard();
        }
        false
    }

    // ─── Iterative Deepening ───────────────────────────────────────────────────

    /// Iterative deepening loop with aspiration windows.
    /// Searches depth 1, 2, ..., max_depth. At each depth, sorts root moves by
    /// previous iteration scores and uses a narrow aspiration window (±25 cp)
    /// starting at depth 4, widening on fail-high/fail-low.
    fn search(&mut self, max_depth: u8) -> Option<(Move, i16)> {
        let moves = self.movegen.legal_moves(&self.position);

        if moves.is_empty() {
            return None;
        }

        let mut move_scores: ArrayVec<(Move, Option<i16>), 256> = ArrayVec::new();
        let mut best_move: Option<Move> = moves.first().copied();
        let mut best_score_overall: i16 = -VALUE_INFINITE;
        let mut prev_best_move = Move::none();
        let mut stability_count: u32 = 0;

        for current_depth in 1..=max_depth {
            if current_depth > 1 && self.time.should_stop_soft() {
                break;
            }

            self.seldepth = 0;
            let mut best_score;
            let mut current_best_move: Option<Move>;

            let sorted_moves: ArrayVec<Move, 256> = if move_scores.is_empty() {
                moves.iter().copied().collect()
            } else {
                move_scores.sort_by_key(|k| std::cmp::Reverse(k.1));
                let sorted: ArrayVec<Move, 256> = move_scores.iter().map(|(mv, _)| *mv).collect();
                move_scores.clear();
                sorted
            };

            // Aspiration windows
            let (mut alpha, mut beta) = if current_depth >= 4 && best_score_overall.abs() < VALUE_MATE - 100 {
                (
                    best_score_overall.saturating_sub(25),
                    best_score_overall.saturating_add(25),
                )
            } else {
                (-VALUE_INFINITE, VALUE_INFINITE)
            };

            loop {
                best_score = -VALUE_INFINITE;
                current_best_move = None;
                move_scores.clear();

                for &mv in sorted_moves.iter() {
                    // Store root move info for continuation history
                    self.ply_piece_type[0] = type_of_piece(self.position.board[mv.from_sq()]);
                    self.ply_to_square[0] = mv.to_sq();
                    self.do_move_nnue(mv);
                    let score = self
                        .alpha_beta(current_depth - 1, -beta, -alpha, 1, true, Move::none())
                        .map(|s| -s);
                    self.undo_move_nnue(mv);

                    move_scores.push((mv, score));

                    match score {
                        Some(score) => {
                            if score > best_score {
                                best_score = score;
                                current_best_move = Some(mv);
                            }
                            if self.time.should_stop_hard() {
                                break;
                            }
                        }
                        None => {
                            // Time's up
                            break;
                        }
                    }
                }

                // Check if we need to re-search with wider window
                if current_best_move.is_some() {
                    if self.time.should_stop_hard() {
                        break;
                    }
                    if best_score <= alpha {
                        // Fail low - widen alpha
                        alpha = alpha.saturating_sub(100);
                        if alpha <= -VALUE_INFINITE + 100 {
                            alpha = -VALUE_INFINITE;
                        }
                        continue;
                    }
                    if best_score >= beta {
                        // Fail high - widen beta
                        beta = beta.saturating_add(100);
                        if beta >= VALUE_INFINITE - 100 {
                            beta = VALUE_INFINITE;
                        }
                        continue;
                    }
                }
                break;
            }

            if let Some(mv) = current_best_move {
                // Best-move stability: scale soft time limit based on how stable the best move is
                if mv == prev_best_move {
                    stability_count += 1;
                } else {
                    stability_count = 0;
                }
                prev_best_move = mv;
                let factor = 0.5 + 0.8 * (1.0 - (stability_count as f64 / 5.0).min(1.0));
                self.time.scale_soft_limit(factor);

                best_move = Some(mv);
                best_score_overall = best_score;
                let elapsed_ms = self.start_time.elapsed().as_millis().max(1) as usize;
                let nps = self.nodes_searched * 1000 / elapsed_ms;
                let hashfull = self.eval.transposition_table.hashfull();
                println!(
                    "info depth {} seldepth {} multipv 1 score cp {} nodes {} nps {} hashfull {} tbhits 0 time {} pv {:?}",
                    current_depth, self.seldepth, best_score, self.nodes_searched, nps, hashfull, elapsed_ms, mv
                );
            }
        }

        best_move.map(|mv| (mv, best_score_overall))
    }

    // ─── Correction History ─────────────────────────────────────────────────────

    /// Look up the correction for the current position's pawn structure.
    /// Returns a centipawn adjustment to add to the static eval.
    fn correction(&self) -> i32 {
        let side = self.position.side_to_move;
        let idx = (self.position.pawn_hash as usize) % CORRECTION_HISTORY_SIZE;
        self.correction_history[side][idx] / CORRECTION_GRAIN
    }

    /// Update the correction history entry with exponential moving average.
    /// `search_score` is the true search result, `static_eval` is the raw NNUE eval.
    fn update_correction(&mut self, static_eval: i16, search_score: i16, depth: u8) {
        let side = self.position.side_to_move;
        let idx = (self.position.pawn_hash as usize) % CORRECTION_HISTORY_SIZE;
        let error = (search_score as i32 - static_eval as i32) * CORRECTION_GRAIN;
        let weight = (depth as i32).min(16);
        let entry = &mut self.correction_history[side][idx];
        *entry = (*entry * (256 - weight) + error * weight) / 256;
        *entry = (*entry).clamp(-CORRECTION_MAX, CORRECTION_MAX);
    }

    // ─── Alpha-Beta Search ────────────────────────────────────────────────────

    /// Main alpha-beta search with PVS, null move pruning, and various forward pruning.
    ///
    /// Returns `Some(score)` or `None` if time expired.
    ///
    /// Flow:
    ///   1. Time check, draw detection, TT probe
    ///   2. Check extension (+1 depth if in check)
    ///   3. Pre-move pruning (null move, RFP, razoring) — skipped in PV nodes & check
    ///   4. Move loop with incremental sort:
    ///      - Per-move pruning (futility, LMP, SEE)
    ///      - PVS: first move full window, rest null-window + re-search if needed
    ///      - LMR: reduce quiet late moves, re-search at full depth on fail-high
    ///   5. Beta cutoff → update killers/history, store TT as LowerBound
    ///   6. After loop → store TT as Exact (alpha improved) or UpperBound
    fn alpha_beta(
        &mut self,
        depth: u8,
        mut alpha: i16,
        beta: i16,
        ply: usize,
        allow_null: bool,
        excluded_move: Move,
    ) -> Option<i16> {
        if self.check_time() {
            return None;
        }
        self.nodes_searched += 1;
        if ply > self.seldepth {
            self.seldepth = ply;
        }

        // 50-move rule
        if self.position.states.last().unwrap().rule50 >= 100 {
            return Some(VALUE_DRAW);
        }

        // Repetition detection: scan back through previous positions.
        // A position can only repeat after at least 4 half-moves (rule50 >= 4).
        // We scan back within the rule50 window since captures/pawn moves reset it.
        //
        // State layout: states[len-1].zobrist stores the zobrist *before* the last
        // move, i.e. the position at (current_ply - 1). So states[len-k].zobrist
        // gives the position at (current_ply - k). We want same-side positions,
        // which are at even distances: k = 2, 4, 6, ...
        {
            let rule50 = self.position.states.last().unwrap().rule50;
            if rule50 >= 4 {
                let states = &self.position.states;
                let current_zobrist = self.position.zobrist;
                let len = states.len();
                let mut k = 2usize;
                while k <= rule50 && k < len {
                    if states[len - k].zobrist == current_zobrist {
                        return Some(VALUE_DRAW);
                    }
                    k += 2;
                }
            }
        }

        // Insufficient material detection
        if (self.position.by_type_bb[Sides::BOTH][PieceType::PAWN]
            | self.position.by_type_bb[Sides::BOTH][PieceType::ROOK]
            | self.position.by_type_bb[Sides::BOTH][PieceType::QUEEN])
            == EMPTY
        {
            let white_knights = self.position.by_type_bb[Sides::WHITE][PieceType::KNIGHT].count_ones();
            let white_bishops = self.position.by_type_bb[Sides::WHITE][PieceType::BISHOP].count_ones();
            let black_knights = self.position.by_type_bb[Sides::BLACK][PieceType::KNIGHT].count_ones();
            let black_bishops = self.position.by_type_bb[Sides::BLACK][PieceType::BISHOP].count_ones();
            let white_minors = white_knights + white_bishops;
            let black_minors = black_knights + black_bishops;

            // KvK, KvKN, KvKB, KNvK, KBvK
            if white_minors + black_minors <= 1 {
                return Some(VALUE_DRAW);
            }
            // KBvKB with same-color bishops
            if white_minors == 1 && black_minors == 1 && white_bishops == 1 && black_bishops == 1 {
                let wb_sq = bits::lsb(self.position.by_type_bb[Sides::WHITE][PieceType::BISHOP]);
                let bb_sq = bits::lsb(self.position.by_type_bb[Sides::BLACK][PieceType::BISHOP]);
                if (file_of(wb_sq) + rank_of(wb_sq)) % 2 == (file_of(bb_sq) + rank_of(bb_sq)) % 2 {
                    return Some(VALUE_DRAW);
                }
            }
        }

        // TT probe
        let zobrist = self.position.zobrist;
        let is_pv = (beta as i32) - (alpha as i32) > 1;
        let mut tt_move = Move::none();
        if let Some(entry) = self.eval.transposition_table.probe(zobrist) {
            tt_move = entry.best_move;
            // Skip TT cutoffs at PV nodes to avoid truncating the principal variation
            // Also skip when we have an excluded move (singular extension verification)
            if !is_pv && excluded_move == Move::none() && entry.depth >= depth {
                let tt_value = adjust_mate_score_from_tt(entry.value, ply);
                match entry.node_type {
                    NodeType::Exact => return Some(tt_value),
                    NodeType::LowerBound => {
                        if tt_value >= beta {
                            return Some(tt_value);
                        }
                    }
                    NodeType::UpperBound => {
                        if tt_value <= alpha {
                            return Some(tt_value);
                        }
                    }
                }
            }
        }

        // Singular Extensions
        // If the TT move appears uniquely good, extend its search by 1 ply.
        let mut extension: i8 = 0;
        let se_time_ok = !self.check_time();
        if let Some(entry) = self.eval.transposition_table.probe(zobrist) {
            if !is_pv
                && ply > 0
                && depth >= 8
                && se_time_ok
                && excluded_move == Move::none()
                && entry.depth >= depth - 3
                && entry.best_move != Move::none()
                && (entry.node_type == NodeType::LowerBound || entry.node_type == NodeType::Exact)
                && adjust_mate_score_from_tt(entry.value, ply).abs() < VALUE_MATE - 100
            {
                let tt_value = adjust_mate_score_from_tt(entry.value, ply);
                let se_beta = tt_value - (depth as i16) * 2;
                let se_depth = (depth - 1) / 2;

                let score = self.alpha_beta(se_depth, se_beta - 1, se_beta, ply, false, entry.best_move);

                match score {
                    Some(s) if s < se_beta => {
                        extension = 1; // TT move is singular → extend
                    }
                    Some(s) if s >= beta => {
                        return Some(s); // Multi-cut: multiple moves beat beta → prune
                    }
                    None => return None,
                    _ => {}
                }
            }
        }

        // Check extension
        let in_check = self.position.checkers_bb(self.position.side_to_move) != 0;
        if in_check {
            extension = extension.max(1);
        }
        let search_depth = (depth as i8 + extension) as u8;

        // Internal Iterative Reductions (IIR): reduce depth when no TT move guides ordering.
        // The TT gets populated for future iterations anyway.
        let search_depth = if tt_move == Move::none() && search_depth >= 4 && !in_check {
            search_depth - 1
        } else {
            search_depth
        };

        if search_depth == 0 {
            return self.quiescence(alpha, beta, ply);
        }

        // Static eval for pruning decisions, adjusted by correction history
        let raw_static_eval = self.evaluate_position();
        let static_eval =
            (raw_static_eval as i32 + self.correction()).clamp(-VALUE_MATE as i32 + 1, VALUE_MATE as i32 - 1) as i16;

        // Store static eval for improving detection
        if ply < MAX_PLY {
            self.static_eval_stack[ply] = static_eval;
        }
        let improving = !in_check && (2..MAX_PLY).contains(&ply) && static_eval > self.static_eval_stack[ply - 2];

        // Null Move Pruning (NMP)
        // Skip our turn and search with reduced depth. If opponent can't beat beta
        // even with a free move, the position is likely too good to need full search.
        // Disabled in check, at shallow depth, and in zugzwang-prone positions (no pieces).
        if allow_null && !in_check && search_depth >= 3 {
            let us = self.position.side_to_move;
            let non_pawn_material = self.position.by_color_bb[us]
                & !self.position.by_type_bb[us][PieceType::PAWN]
                & !self.position.by_type_bb[us][PieceType::KING];
            if non_pawn_material != 0 {
                let r = 3 + (search_depth as usize / 4) + (!improving as usize); // adaptive reduction
                let reduced_depth = search_depth.saturating_sub(r as u8);

                self.do_null_move_nnue();
                let score = self
                    .alpha_beta(reduced_depth, -beta, -beta + 1, ply + 1, false, Move::none())
                    .map(|s| -s);
                self.undo_null_move_nnue();

                match score {
                    Some(s) if s >= beta => return Some(beta),
                    None => return None,
                    _ => {}
                }
            }
        }

        // Reverse Futility Pruning (RFP)
        // If static eval is far above beta (by margin*depth cp), assume no move will
        // drop the score below beta. Safe to prune the whole subtree.
        if !is_pv && !in_check && search_depth <= 7 && static_eval.abs() < VALUE_MATE - 100 {
            let rfp_margin = (if improving { 75 } else { 60 }) * (search_depth as i16);
            if static_eval - rfp_margin >= beta {
                return Some(static_eval);
            }
        }

        // Razoring
        // If static eval is far below alpha, the position is likely bad. Verify with
        // quiescence search — if qsearch confirms, prune early.
        if !is_pv && !in_check && search_depth <= 2 && static_eval.abs() < VALUE_MATE - 100 {
            let razor_margin = if search_depth == 1 { 300_i16 } else { 600_i16 };
            if static_eval + razor_margin <= alpha {
                let q_score = self.quiescence(alpha, beta, ply);
                match q_score {
                    Some(s) if s <= alpha => return q_score,
                    None => return None,
                    _ => {}
                }
            }
        }

        // ── Probcut ───────────────────────────────────────────────────────────
        // If a capture exists that quickly exceeds beta by a large margin at reduced
        // depth, prune the whole node. Avoids searching positions that are clearly
        // too good (false fail-lows won't be missed since the bound is wide).
        if !is_pv && !in_check && search_depth >= 5 && beta.abs() < VALUE_MATE - 100 && excluded_move == Move::none() {
            let probcut_beta = beta.saturating_add(200);
            let probcut_depth = search_depth.saturating_sub(4);
            let captures = self.movegen.generate_captures(&self.position);
            let probcut_king_sq = bits::lsb(self.position.by_type_bb[self.position.side_to_move][PieceType::KING]);
            for &pc_mv in captures.iter() {
                let pc_to = pc_mv.to_sq();
                let pc_type = pc_mv.type_of();
                let pc_is_capture = self.position.board[pc_to] != PieceType::NONE || pc_type == MoveTypes::EN_PASSANT;
                if !pc_is_capture {
                    continue;
                }
                // generate_captures returns pseudo-legal moves — filter illegal ones
                // (pinned pieces, king moves into check, etc.) before applying.
                let us = self.position.side_to_move;
                let from_bb = square_bb(pc_mv.from_sq());
                let is_legal = if self.position.pinned_bb[us] & from_bb == EMPTY
                    && probcut_king_sq != pc_mv.from_sq()
                    && pc_type != MoveTypes::EN_PASSANT
                {
                    true
                } else {
                    self.position.legal(pc_mv)
                };
                if !is_legal {
                    continue;
                }
                // Quick SEE filter: only try captures that at least break even.
                // Losing captures can't lift the score to probcut_beta.
                if see(&self.position, pc_mv) < 0 {
                    continue;
                }
                self.do_move_nnue(pc_mv);
                let pc_score = self
                    .alpha_beta(
                        probcut_depth,
                        -probcut_beta,
                        -probcut_beta + 1,
                        ply + 1,
                        false,
                        Move::none(),
                    )
                    .map(|s| -s);
                self.undo_move_nnue(pc_mv);
                match pc_score {
                    Some(s) if s >= probcut_beta => return Some(s),
                    None => return None,
                    _ => {}
                }
            }
        }

        // ── Staged Move Loop (MovePicker) ─────────────────────────────────────
        // Lazy generation: TT move → good captures → killers/countermove → quiets → bad captures.
        // Avoids generating quiets entirely when a beta cutoff happens in captures.
        let check_mask = self.movegen.check_mask(&self.position);
        let ply_killers = if ply < MAX_PLY {
            self.killers[ply]
        } else {
            [Move::none(); 2]
        };
        let ply_countermove = if self.prev_move != Move::none() {
            self.countermoves[self.prev_move.from_sq()][self.prev_move.to_sq()]
        } else {
            Move::none()
        };
        let mut picker = MovePicker::new(tt_move, ply_killers, ply_countermove, check_mask);

        let original_alpha = alpha;
        let mut best_move = Move::none();
        let mut best_score = -VALUE_INFINITE;
        let mut moves_searched = 0usize;
        let mut quiet_moves_tried: ArrayVec<Move, 64> = ArrayVec::new();
        let mut captures_tried: ArrayVec<(usize, usize, usize), 32> = ArrayVec::new(); // (piece_type, to_sq, victim_type)

        // Precompute continuation history indices for this ply
        let ch1_idx = if ply >= 1 {
            Some((self.ply_piece_type[ply - 1], self.ply_to_square[ply - 1]))
        } else {
            None
        };
        let ch2_idx = if ply >= 2 {
            Some((self.ply_piece_type[ply - 2], self.ply_to_square[ply - 2]))
        } else {
            None
        };

        // ── Move Loop ─────────────────────────────────────────────────────────
        loop {
            let mv = {
                let ch1 = ch1_idx.map(|(piece, to)| &self.conthist_1ply[piece][to]);
                let ch2 = ch2_idx.map(|(piece, to)| &self.conthist_2ply[piece][to]);
                match picker.next(
                    &self.position,
                    &self.movegen,
                    &self.history,
                    ch1,
                    ch2,
                    &self.capture_history,
                ) {
                    Some(mv) => mv,
                    None => break,
                }
            };

            // Skip excluded move (used by singular extension verification search)
            if mv == excluded_move {
                continue;
            }

            // Check capture/promotion before do_move since board changes
            let to_sq = mv.to_sq();
            let move_type = mv.type_of();
            let is_capture = self.position.board[to_sq] != PieceType::NONE || move_type == MoveTypes::EN_PASSANT;
            let is_promotion = move_type == MoveTypes::PROMOTION;
            let is_quiet = !is_capture && !is_promotion;
            let piece_type = type_of_piece(self.position.board[mv.from_sq()]);
            let victim_type = if is_capture {
                if move_type == MoveTypes::EN_PASSANT {
                    PieceType::PAWN
                } else {
                    type_of_piece(self.position.board[to_sq])
                }
            } else {
                PieceType::NONE
            };
            // A move is "killer-class" if it's a quiet move that got special ordering
            // (killer slot or countermove) — these are not subject to LMP/futility.
            let is_killer = is_quiet
                && ply < MAX_PLY
                && (mv == self.killers[ply][0]
                    || mv == self.killers[ply][1]
                    || (self.prev_move != Move::none()
                        && mv == self.countermoves[self.prev_move.from_sq()][self.prev_move.to_sq()]));

            // Futility Pruning: at low depth, skip quiet moves when static eval + margin
            // can't reach alpha (the move won't improve the situation enough).
            if !is_pv && !in_check && is_quiet && !is_killer && moves_searched > 0 && search_depth <= 3 {
                let futility_margin = match search_depth {
                    1 => 175_i16,
                    2 => 350_i16,
                    _ => 550_i16,
                };
                if static_eval + futility_margin <= alpha {
                    picker.skip_quiets = true;
                    continue;
                }
            }

            // Late Move Pruning (LMP): at low depth, skip quiet moves beyond a threshold.
            // Moves are ordered by score, so late moves are unlikely to be good.
            if !is_pv
                && !in_check
                && is_quiet
                && !is_killer
                && search_depth <= 4
                && moves_searched >= LMP_THRESHOLDS[search_depth as usize] + if improving { 2 } else { 0 }
            {
                picker.skip_quiets = true;
                continue;
            }

            // SEE Pruning: skip losing captures at shallow depth.
            if !is_pv
                && !in_check
                && is_capture
                && moves_searched > 0
                && search_depth <= 3
                && see(&self.position, mv) < 0
            {
                continue;
            }

            // Track quiet moves for history malus on beta cutoff
            if is_quiet && quiet_moves_tried.len() < quiet_moves_tried.capacity() {
                quiet_moves_tried.push(mv);
            }

            // Track captures for capture history malus on beta cutoff
            if is_capture && captures_tried.len() < captures_tried.capacity() {
                captures_tried.push((piece_type, to_sq, victim_type));
            }

            // Store ply move info for continuation history before recursive call
            if ply < MAX_PLY {
                self.ply_piece_type[ply] = piece_type;
                self.ply_to_square[ply] = to_sq;
            }

            self.do_move_nnue(mv);
            let saved_prev_move = self.prev_move;
            self.prev_move = mv;

            // ── PVS + LMR Search Logic ────────────────────────────────────────
            let score;
            if moves_searched == 0 {
                // First move (expected best): search with full alpha-beta window
                score = self
                    .alpha_beta(search_depth - 1, -beta, -alpha, ply + 1, true, Move::none())
                    .map(|s| -s);
            } else {
                // Late Move Reductions (LMR): reduce quiet late moves by a logarithmic
                // amount. If the reduced search fails high, re-search at full depth.
                let do_lmr =
                    !in_check && !is_capture && !is_promotion && !is_killer && moves_searched >= 3 && search_depth >= 3;

                let reduction = if do_lmr {
                    let mut r = self.lmr_table[search_depth as usize][moves_searched.min(63)] as i8;
                    if !improving {
                        r += 1;
                    }
                    // History-based LMR adjustment: reduce less for high-history moves
                    let hist = self.history[mv.from_sq()][to_sq];
                    r -= (hist / 5000).clamp(-1, 1) as i8;
                    r.max(0).min((search_depth as i8) - 2) as u8
                } else {
                    0
                };

                let reduced_depth = (search_depth - 1).saturating_sub(reduction);

                // Step 1: Null-window search (possibly at reduced depth for LMR)
                let mut s = self
                    .alpha_beta(
                        reduced_depth,
                        (-alpha).saturating_sub(1),
                        -alpha,
                        ply + 1,
                        true,
                        Move::none(),
                    )
                    .map(|s| -s);

                // Step 2: LMR re-search — if reduced search beat alpha, try full depth
                if let Some(val) = s {
                    if val > alpha && reduction > 0 {
                        s = self
                            .alpha_beta(
                                search_depth - 1,
                                (-alpha).saturating_sub(1),
                                -alpha,
                                ply + 1,
                                true,
                                Move::none(),
                            )
                            .map(|s| -s);
                    }
                }

                // Step 3: PVS re-search — if null-window beat alpha but not beta,
                // re-search with full window to get exact score
                if let Some(val) = s {
                    if val > alpha && val < beta {
                        s = self
                            .alpha_beta(search_depth - 1, -beta, -alpha, ply + 1, true, Move::none())
                            .map(|s| -s);
                    }
                }

                score = s;
            }

            self.undo_move_nnue(mv);
            self.prev_move = saved_prev_move;

            match score {
                Some(score) => {
                    if score > best_score {
                        best_score = score;
                        best_move = mv;
                    }
                    if score >= beta {
                        let bonus = (search_depth as i32) * (search_depth as i32);

                        if is_capture {
                            // Capture history: bonus for cutoff capture, malus for tried captures
                            update_gravity(&mut self.capture_history[piece_type][to_sq][victim_type], bonus);
                            for &(tried_pt, tried_to, tried_vt) in captures_tried.iter() {
                                if !(tried_pt == piece_type && tried_to == to_sq && tried_vt == victim_type) {
                                    update_gravity(&mut self.capture_history[tried_pt][tried_to][tried_vt], -bonus);
                                }
                            }
                        } else if ply < MAX_PLY {
                            // Quiet beta cutoff: update killers, history, continuation history
                            self.killers[ply][1] = self.killers[ply][0];
                            self.killers[ply][0] = mv;

                            // Main history + continuation history bonus
                            update_gravity(&mut self.history[mv.from_sq()][to_sq], bonus);
                            if let Some((pp, pt)) = ch1_idx {
                                update_gravity(&mut self.conthist_1ply[pp][pt][piece_type][to_sq], bonus);
                            }
                            if let Some((gp, gt)) = ch2_idx {
                                update_gravity(&mut self.conthist_2ply[gp][gt][piece_type][to_sq], bonus);
                            }

                            // History malus: penalize quiet moves tried before cutoff
                            for &tried_mv in quiet_moves_tried.iter() {
                                if tried_mv != mv {
                                    let tried_pt = type_of_piece(self.position.board[tried_mv.from_sq()]);
                                    let tried_to = tried_mv.to_sq();
                                    update_gravity(&mut self.history[tried_mv.from_sq()][tried_to], -bonus);
                                    if let Some((pp, pt)) = ch1_idx {
                                        update_gravity(&mut self.conthist_1ply[pp][pt][tried_pt][tried_to], -bonus);
                                    }
                                    if let Some((gp, gt)) = ch2_idx {
                                        update_gravity(&mut self.conthist_2ply[gp][gt][tried_pt][tried_to], -bonus);
                                    }
                                }
                            }

                            // Countermove heuristic
                            if self.prev_move != Move::none() {
                                self.countermoves[self.prev_move.from_sq()][self.prev_move.to_sq()] = mv;
                            }
                        }

                        self.eval.transposition_table.store(
                            zobrist,
                            HashData {
                                depth: search_depth,
                                value: adjust_mate_score_to_tt(score, ply),
                                best_move: mv,
                                node_type: NodeType::LowerBound,
                            },
                        );
                        return Some(score);
                    }
                    if score > alpha {
                        alpha = score;
                    }
                }
                None => {
                    return None;
                }
            }

            moves_searched += 1;
        }

        // Terminal position: no legal move was found
        if moves_searched == 0 && best_move == Move::none() {
            if in_check {
                return Some(-VALUE_MATE + (ply as i16));
            } else {
                return Some(VALUE_DRAW);
            }
        }

        // ── TT Store ──────────────────────────────────────────────────────────
        // Exact if we improved alpha (found a PV), UpperBound (all-node) otherwise
        let node_type = if alpha > original_alpha {
            NodeType::Exact
        } else {
            NodeType::UpperBound
        };
        // Store best_score (fail-soft) rather than alpha for more accurate TT entries
        let tt_value = if alpha > original_alpha { best_score } else { alpha };
        self.eval.transposition_table.store(
            zobrist,
            HashData {
                depth: search_depth,
                value: adjust_mate_score_to_tt(tt_value, ply),
                best_move,
                node_type,
            },
        );

        // Update correction history: learn from the difference between raw static eval and search score.
        // Only update at non-PV, non-check nodes with non-mate scores for reliability.
        if !is_pv && !in_check && best_score.abs() < VALUE_MATE - 100 {
            self.update_correction(raw_static_eval, best_score, search_depth);
        }

        Some(best_score)
    }

    // ─── Quiescence Search ─────────────────────────────────────────────────────

    /// Quiescence search: resolve tactical sequences (captures, en passant)
    /// to avoid horizon effect. Uses stand-pat score as lower bound, then searches
    /// only tactical moves with delta pruning and SEE pruning.
    fn quiescence(&mut self, mut alpha: i16, beta: i16, ply: usize) -> Option<i16> {
        if self.check_time() {
            return None;
        }
        self.nodes_searched += 1;
        if ply > self.seldepth {
            self.seldepth = ply;
        }

        // Stand pat: assume we can at least achieve the static eval by not capturing.
        // If it already beats beta, prune. Otherwise use it as alpha floor.
        let stand_pat = self.evaluate_position();
        if stand_pat >= beta {
            return Some(beta);
        }
        if stand_pat > alpha {
            alpha = stand_pat;
        }

        let check_mask = self.movegen.check_mask(&self.position);
        let mut picker = QMovePicker::new(Move::none(), check_mask);

        while let Some(mv) = picker.next(&self.position, &self.movegen) {
            let move_type = mv.type_of();

            // Delta Pruning: skip captures where even capturing the piece + a 200cp margin
            // can't raise the score above alpha (the capture can't possibly help).
            if move_type != MoveTypes::PROMOTION {
                let captured_piece = if move_type == MoveTypes::EN_PASSANT {
                    PieceType::PAWN
                } else {
                    type_of_piece(self.position.board[mv.to_sq()])
                };
                let delta = SEE_VALUES[captured_piece] + 200;
                if stand_pat + delta < alpha {
                    continue;
                }
            }

            // SEE pruning in quiescence
            if move_type != MoveTypes::PROMOTION && see(&self.position, mv) < 0 {
                continue;
            }

            self.do_move_nnue(mv);
            let score = self.quiescence(-beta, -alpha, ply + 1).map(|s| -s);
            self.undo_move_nnue(mv);

            match score {
                Some(score) => {
                    if score >= beta {
                        return Some(beta);
                    }
                    if score > alpha {
                        alpha = score;
                    }
                }
                None => {
                    return None;
                }
            }
        }

        Some(alpha)
    }
}

// ─── Static Exchange Evaluation (SEE) ──────────────────────────────────────

/// Evaluate a capture exchange on a square by simulating all recaptures.
/// Returns the material gain/loss from the attacker's perspective.
///
/// Free function so it's accessible from both search and move_picker.
fn see(position: &Position, mv: Move) -> i16 {
    let from = mv.from_sq();
    let to = mv.to_sq();
    let move_type = mv.type_of();

    // Determine the initial captured piece value
    let mut gain = [0i16; 32];
    gain[0] = if move_type == MoveTypes::EN_PASSANT {
        SEE_VALUES[PieceType::PAWN]
    } else if move_type == MoveTypes::PROMOTION {
        SEE_VALUES[type_of_piece(position.board[to])] + SEE_VALUES[PieceType::QUEEN] - SEE_VALUES[PieceType::PAWN]
    } else {
        SEE_VALUES[type_of_piece(position.board[to])]
    };

    let mut side_to_move = color_of_piece(position.board[from]);
    let mut occupied = position.by_color_bb[Sides::BOTH];

    // Remove the initial attacker from occupied
    occupied ^= square_bb(from);

    // For en passant, also remove the captured pawn
    if move_type == MoveTypes::EN_PASSANT {
        let ep_sq = (to as isize - pawn_push(side_to_move)) as usize;
        occupied ^= square_bb(ep_sq);
    }

    let mut attackers = position.attackers_to(to, occupied);
    let mut attacker_piece_type = if move_type == MoveTypes::PROMOTION {
        PieceType::QUEEN
    } else {
        type_of_piece(position.board[from])
    };

    let mut depth = 0usize;
    side_to_move ^= 1;

    loop {
        depth += 1;
        if depth >= 32 {
            break;
        }

        // Speculative store: assume current attacker gets captured next
        gain[depth] = SEE_VALUES[attacker_piece_type] - gain[depth - 1];

        // Pruning: if neither side can improve, stop early
        if (-gain[depth]).max(gain[depth - 1]) < 0 {
            break;
        }

        // Find least valuable attacker for current side
        let my_attackers = attackers & position.by_color_bb[side_to_move];
        if my_attackers == EMPTY {
            break;
        }

        // Find least valuable piece type
        attacker_piece_type = PieceType::NONE;
        let mut from_bb = EMPTY;
        for pt in [
            PieceType::PAWN,
            PieceType::KNIGHT,
            PieceType::BISHOP,
            PieceType::ROOK,
            PieceType::QUEEN,
            PieceType::KING,
        ] {
            from_bb = my_attackers & position.by_type_bb[side_to_move][pt];
            if from_bb != EMPTY {
                attacker_piece_type = pt;
                break;
            }
        }

        if attacker_piece_type == PieceType::NONE {
            break;
        }

        // Remove attacker from occupied to reveal x-ray attackers behind it
        let attacker_sq = bits::lsb(from_bb);
        occupied ^= square_bb(attacker_sq);

        // Recompute sliding attackers through the now-empty square
        if attacker_piece_type == PieceType::PAWN
            || attacker_piece_type == PieceType::BISHOP
            || attacker_piece_type == PieceType::QUEEN
        {
            attackers |= position.attack_bb(make_piece(0, PieceType::BISHOP), to, occupied)
                & (position.by_type_bb[Sides::BOTH][PieceType::BISHOP]
                    | position.by_type_bb[Sides::BOTH][PieceType::QUEEN]);
        }
        if attacker_piece_type == PieceType::ROOK || attacker_piece_type == PieceType::QUEEN {
            attackers |= position.attack_bb(make_piece(0, PieceType::ROOK), to, occupied)
                & (position.by_type_bb[Sides::BOTH][PieceType::ROOK]
                    | position.by_type_bb[Sides::BOTH][PieceType::QUEEN]);
        }

        // Remove used attacker from attackers set
        attackers &= occupied;

        side_to_move ^= 1;
    }

    // Minimax walk-back: each side can choose to stop the exchange
    while depth > 1 {
        depth -= 1;
        gain[depth - 1] = -((-gain[depth]).max(gain[depth - 1]));
    }

    gain[0]
}
