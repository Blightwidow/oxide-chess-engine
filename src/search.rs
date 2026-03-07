pub mod defs;
mod test;

use std::time;

use crate::{
    evaluate::{
        tables::PIECE_VALUES_MG,
        transposition::{HashData, NodeType},
        Eval,
    },
    defs::*,
    movegen::{defs::{Move, MoveTypes}, Movegen},
    position::Position,
    time::TimeManager,
};

use self::defs::*;

const MAX_PLY: usize = 128;

pub struct Search {
    pub position: Position,
    pub movegen: Movegen,
    pub eval: Eval,
    pub nodes_searched: usize,
    seldepth: usize,
    time: TimeManager,
    start_time: time::Instant,
    killers: [[Move; 2]; MAX_PLY],
    history: [[i32; 64]; 64],
    lmr_table: [[u8; 64]; 128],
}

impl Search {
    pub fn new(position: Position, movegen: Movegen, eval: Eval) -> Self {
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
            time: TimeManager::default(),
            start_time: time::Instant::now(),
            killers: [[Move::none(); 2]; MAX_PLY],
            history: [[0; 64]; 64],
            lmr_table,
        };
        search.position.set(FEN_START_POSITION.to_string());

        search
    }

    pub fn run(&mut self, limits: SearchLimits) {
        self.nodes_searched = 0;
        self.seldepth = 0;
        self.start_time = time::Instant::now();
        self.killers = [[Move::none(); 2]; MAX_PLY];
        self.history = [[0; 64]; 64];

        if limits.perft > 0 {
            let nodes = self.perft(limits.perft, true);
            println!("\nNodes searched: {}\n", nodes);
            return;
        }

        self.time = TimeManager::new(
            limits,
            self.position.side_to_move,
            self.position.states.last().unwrap().game_ply,
        );

        if limits.depth > 0 {
            let result = self.search(limits.depth);
            if let Some((mv, _score)) = result {
                // Probe TT for ponder move
                self.position.do_move(mv);
                let ponder = self.eval.transposition_table.probe(self.position.zobrist)
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
            let score = self.eval.evaluate(&self.position);
            println!("info depth 0 score cp {}", score);
        }
    }

    fn perft(&mut self, depth: u8, root: bool) -> u64 {
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

    fn search(&mut self, max_depth: u8) -> Option<(Move, i16)> {
        let moves = self.movegen.legal_moves(&self.position);

        if moves.is_empty() {
            return None;
        }

        let mut move_scores: Vec<(Move, Option<i16>)> = Vec::new();
        let mut best_move: Option<Move> = None;
        let mut best_score_overall: i16 = -VALUE_INFINITE;

        for current_depth in 1..=max_depth {
            if self.time.should_stop() {
                break;
            }

            self.seldepth = 0;
            let mut best_score;
            let mut current_best_move: Option<Move>;

            let sorted_moves: Vec<Move> = if move_scores.is_empty() {
                moves.to_vec()
            } else {
                move_scores.sort_by_key(|k| std::cmp::Reverse(k.1));
                let sorted: Vec<Move> = move_scores.iter().map(|(mv, _)| *mv).collect();
                move_scores.clear();
                sorted
            };

            // Aspiration windows
            let (mut alpha, mut beta) = if current_depth >= 4 && best_score_overall.abs() < VALUE_MATE - 100 {
                (best_score_overall.saturating_sub(25), best_score_overall.saturating_add(25))
            } else {
                (-VALUE_INFINITE, VALUE_INFINITE)
            };

            let mut failed = 0u8; // track re-search attempts

            loop {
                best_score = -VALUE_INFINITE;
                current_best_move = None;

                for &mv in sorted_moves.iter() {
                    self.position.do_move(mv);
                    let score = self.alpha_beta(current_depth - 1, -beta, -alpha, 1, true).map(|s| -s);
                    self.position.undo_move(mv);

                    // Only update move_scores on first attempt (or when we have none)
                    if failed == 0 {
                        move_scores.push((mv, score));
                    }

                    match score {
                        Some(score) => {
                            if score > best_score {
                                best_score = score;
                                current_best_move = Some(mv);
                            }
                        }
                        None => {
                            // Time's up
                            break;
                        }
                    }
                }

                // Check if we need to re-search with wider window
                if current_best_move.is_some() && failed < 2 {
                    if best_score <= alpha {
                        // Fail low - widen alpha
                        alpha = if failed == 0 { alpha.saturating_sub(100) } else { -VALUE_INFINITE };
                        failed += 1;
                        if failed == 1 {
                            move_scores.clear();
                        }
                        continue;
                    }
                    if best_score >= beta {
                        // Fail high - widen beta
                        beta = if failed == 0 { beta.saturating_add(100) } else { VALUE_INFINITE };
                        failed += 1;
                        if failed == 1 {
                            move_scores.clear();
                        }
                        continue;
                    }
                }
                break;
            }

            if let Some(mv) = current_best_move {
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

    fn score_move(&self, mv: Move, tt_move: Move, ply: usize) -> i32 {
        // TT move gets highest priority
        if mv == tt_move && tt_move != Move::none() {
            return 1_000_000;
        }

        let to_sq = mv.to_sq();
        let from_sq = mv.from_sq();
        let move_type = mv.type_of();

        // Captures: MVV-LVA
        let is_capture = self.position.board[to_sq] != PieceType::NONE || move_type == MoveTypes::EN_PASSANT;
        if is_capture {
            let victim_value = if move_type == MoveTypes::EN_PASSANT {
                PIECE_VALUES_MG[PieceType::PAWN] as i32
            } else {
                PIECE_VALUES_MG[type_of_piece(self.position.board[to_sq])] as i32
            };
            let attacker_value = PIECE_VALUES_MG[type_of_piece(self.position.board[from_sq])] as i32;
            return 100_000 + victim_value * 100 - attacker_value;
        }

        // Promotions
        if move_type == MoveTypes::PROMOTION {
            return 100_000 + PIECE_VALUES_MG[mv.promotion_type()] as i32 * 100;
        }

        // Killers
        if ply < MAX_PLY {
            if mv == self.killers[ply][0] {
                return 90_000;
            }
            if mv == self.killers[ply][1] {
                return 80_000;
            }
        }

        // History heuristic
        self.history[from_sq][to_sq]
    }

    fn alpha_beta(
        &mut self,
        depth: u8,
        mut alpha: i16,
        beta: i16,
        ply: usize,
        allow_null: bool,
    ) -> Option<i16> {
        if self.time.should_stop() {
            return None;
        }
        self.nodes_searched += 1;
        if ply > self.seldepth {
            self.seldepth = ply;
        }

        // TT probe
        let zobrist = self.position.zobrist;
        let mut tt_move = Move::none();
        if let Some(entry) = self.eval.transposition_table.probe(zobrist) {
            tt_move = entry.best_move;
            if entry.depth >= depth {
                match entry.node_type {
                    NodeType::Exact => return Some(entry.value),
                    NodeType::LowerBound => {
                        if entry.value >= beta {
                            return Some(entry.value);
                        }
                    }
                    NodeType::UpperBound => {
                        if entry.value <= alpha {
                            return Some(entry.value);
                        }
                    }
                }
            }
        }

        // Check extension
        let in_check = self.position.checkers_bb(self.position.side_to_move) != 0;
        let search_depth = if in_check { depth + 1 } else { depth };

        if search_depth == 0 {
            return self.quiescence(alpha, beta, ply);
        }

        // Null Move Pruning
        if allow_null && !in_check && search_depth >= 3 {
            let us = self.position.side_to_move;
            let non_pawn_material = self.position.by_color_bb[us]
                & !self.position.by_type_bb[us][PieceType::PAWN]
                & !self.position.by_type_bb[us][PieceType::KING];
            if non_pawn_material != 0 {
                let r = 3 + (search_depth as usize / 4);
                let reduced_depth = search_depth.saturating_sub(r as u8);

                self.position.do_null_move();
                let score = self
                    .alpha_beta(reduced_depth, -beta, -beta + 1, ply + 1, false)
                    .map(|s| -s);
                self.position.undo_null_move();

                match score {
                    Some(s) if s >= beta => return Some(beta),
                    None => return None,
                    _ => {}
                }
            }
        }

        let moves = self.movegen.legal_moves(&self.position);

        // Check for terminal positions
        if moves.is_empty() {
            if in_check {
                return Some(-VALUE_MATE + (ply as i16));
            } else {
                return Some(VALUE_DRAW);
            }
        }

        // Score moves for ordering
        let mut scored_moves: Vec<(Move, i32)> = moves.iter().map(|&mv| (mv, self.score_move(mv, tt_move, ply))).collect();

        let original_alpha = alpha;
        let mut best_move = Move::none();
        let mut best_score = -VALUE_INFINITE;

        // Incremental selection sort: pick the best move each iteration
        for (moves_searched, i) in (0..scored_moves.len()).enumerate() {
            // Find the best-scored move from i..end
            let mut best_idx = i;
            for j in (i + 1)..scored_moves.len() {
                if scored_moves[j].1 > scored_moves[best_idx].1 {
                    best_idx = j;
                }
            }
            scored_moves.swap(i, best_idx);

            let mv = scored_moves[i].0;
            let move_score = scored_moves[i].1;

            // Check capture/promotion before do_move since board changes
            let to_sq = mv.to_sq();
            let move_type = mv.type_of();
            let is_capture = self.position.board[to_sq] != PieceType::NONE
                || move_type == MoveTypes::EN_PASSANT;
            let is_promotion = move_type == MoveTypes::PROMOTION;
            let is_killer = move_score == 90_000 || move_score == 80_000;

            self.position.do_move(mv);

            let score;
            if moves_searched == 0 {
                // First move: full window search
                score = self.alpha_beta(search_depth - 1, -beta, -alpha, ply + 1, true).map(|s| -s);
            } else {

                // LMR conditions
                let do_lmr = !in_check && !is_capture && !is_promotion && !is_killer
                    && moves_searched >= 3 && search_depth >= 3;

                let reduction = if do_lmr {
                    self.lmr_table[search_depth as usize][moves_searched.min(63)]
                } else {
                    0
                };

                let reduced_depth = (search_depth - 1).saturating_sub(reduction);

                // Null window search (possibly at reduced depth)
                let mut s = self.alpha_beta(reduced_depth, (-alpha).saturating_sub(1), -alpha, ply + 1, true).map(|s| -s);

                // If LMR reduced and failed high, re-search at full depth with null window
                if let Some(val) = s {
                    if val > alpha && reduction > 0 {
                        s = self.alpha_beta(search_depth - 1, (-alpha).saturating_sub(1), -alpha, ply + 1, true).map(|s| -s);
                    }
                }

                // PVS re-search: if null window failed high within bounds, full window
                if let Some(val) = s {
                    if val > alpha && val < beta {
                        s = self.alpha_beta(search_depth - 1, -beta, -alpha, ply + 1, true).map(|s| -s);
                    }
                }

                score = s;
            }

            self.position.undo_move(mv);

            match score {
                Some(score) => {
                    if score > best_score {
                        best_score = score;
                        best_move = mv;
                    }
                    if score >= beta {
                        // Killer move and history: store quiet moves that cause beta cutoffs
                        let is_capture = self.position.board[mv.to_sq()] != PieceType::NONE
                            || mv.type_of() == MoveTypes::EN_PASSANT;
                        if !is_capture && ply < MAX_PLY {
                            self.killers[ply][1] = self.killers[ply][0];
                            self.killers[ply][0] = mv;
                            self.history[mv.from_sq()][mv.to_sq()] +=
                                (search_depth as i32) * (search_depth as i32);
                        }

                        self.eval.transposition_table.store(
                            zobrist,
                            HashData {
                                depth: search_depth,
                                value: beta,
                                best_move: mv,
                                node_type: NodeType::LowerBound,
                            },
                        );
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

        // Store in TT
        let node_type = if alpha > original_alpha {
            NodeType::Exact
        } else {
            NodeType::UpperBound
        };
        self.eval.transposition_table.store(
            zobrist,
            HashData {
                depth: search_depth,
                value: alpha,
                best_move,
                node_type,
            },
        );

        Some(alpha)
    }

    fn quiescence(&mut self, mut alpha: i16, beta: i16, ply: usize) -> Option<i16> {
        if self.time.should_stop() {
            return None;
        }
        self.nodes_searched += 1;
        if ply > self.seldepth {
            self.seldepth = ply;
        }

        // Stand pat
        let stand_pat = self.eval.evaluate(&self.position);
        if stand_pat >= beta {
            return Some(beta);
        }
        if stand_pat > alpha {
            alpha = stand_pat;
        }

        let moves = self.movegen.legal_moves(&self.position);

        // Filter to captures, en passant, and promotions
        let mut scored_moves: Vec<(Move, i32)> = Vec::new();
        for &mv in moves.iter() {
            let to_sq = mv.to_sq();
            let move_type = mv.type_of();
            let is_capture = self.position.board[to_sq] != PieceType::NONE
                || move_type == MoveTypes::EN_PASSANT
                || move_type == MoveTypes::PROMOTION;

            if is_capture {
                let score = self.score_move(mv, Move::none(), ply);
                scored_moves.push((mv, score));
            }
        }

        // Incremental selection sort
        for i in 0..scored_moves.len() {
            let mut best_idx = i;
            for j in (i + 1)..scored_moves.len() {
                if scored_moves[j].1 > scored_moves[best_idx].1 {
                    best_idx = j;
                }
            }
            scored_moves.swap(i, best_idx);

            let mv = scored_moves[i].0;

            self.position.do_move(mv);
            let score = self.quiescence(-beta, -alpha, ply + 1).map(|s| -s);
            self.position.undo_move(mv);

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
