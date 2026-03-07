pub mod defs;
mod test;

use crate::{
    evaluate::{
        transposition::{HashData, NodeType},
        Eval,
    },
    movegen::{defs::Move, Movegen},
    position::Position,
    time::TimeManager,
};

use self::defs::*;

pub struct Search {
    pub position: Position,
    pub movegen: Movegen,
    pub eval: Eval,
    pub nodes_searched: usize,
    time: TimeManager,
}

impl Search {
    pub fn new(position: Position, movegen: Movegen, eval: Eval) -> Self {
        let mut search = Self {
            position,
            movegen,
            nodes_searched: 0,
            eval,
            time: TimeManager::default(),
        };
        search.position.set(FEN_START_POSITION.to_string());

        search
    }

    pub fn run(&mut self, limits: SearchLimits) {
        self.nodes_searched = 0;

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
            let best_move = self.search(limits.depth);
            if let Some(mv) = best_move {
                println!("bestmove {:?}", mv);
            } else {
                println!("bestmove 0000");
            }
        } else {
            let score = self.eval.evaluate(&self.position);
            println!("info depth 0 score cp {}", score);
        }
    }

    fn perft(&mut self, depth: u8, root: bool) -> u128 {
        let mut count: u128;
        let mut nodes: u128 = 0;
        let leaf: bool = depth == 2;
        let moves: Vec<Move> = self.movegen.legal_moves(&self.position);

        for mv in moves.iter() {
            if depth <= 1 {
                count = 1;
                nodes += 1;
            } else {
                self.position.do_move(*mv);
                count = match leaf {
                    true => self.movegen.legal_moves(&self.position).len() as u128,
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

    fn search(&mut self, max_depth: u8) -> Option<Move> {
        let moves = self.movegen.legal_moves(&self.position);

        if moves.is_empty() {
            return None;
        }

        // Store move scores from previous iteration for move ordering
        let mut move_scores: Vec<(Move, Option<i16>)> = Vec::new();
        let mut best_move: Option<Move> = None;

        // Iterative deepening: search from depth 1 to max_depth
        for current_depth in 1..=max_depth {
            if self.time.should_stop() {
                break;
            }

            let mut best_score = -VALUE_INFINITE;
            let mut current_best_move: Option<Move> = None;

            // Sort moves by score from previous iteration (best moves first)
            let sorted_moves: Vec<Move> = if move_scores.is_empty() {
                // First iteration: no previous scores, use all moves as-is
                moves.clone()
            } else {
                // Sort by score (descending) - best moves first
                move_scores.sort_by(|a, b| b.1.cmp(&a.1));
                let sorted: Vec<Move> = move_scores.iter().map(|(mv, _)| *mv).collect();
                // Clear move scores for this iteration
                move_scores.clear();
                sorted
            };

            // Search each move at current depth
            for &mv in sorted_moves.iter() {
                self.position.do_move(mv);

                let score = self.alpha_beta(current_depth - 1, -VALUE_INFINITE, VALUE_INFINITE).map(|score| -score);

                self.position.undo_move(mv);

                // Store score for move ordering in next iteration
                move_scores.push((mv, score));

                match score {
                    Some(score) => {
                        if score >= best_score {
                            best_score = score;
                            current_best_move = Some(mv);
                            best_move = Some(mv);
                        }
                    }
                    None => {
                        break;
                    }
                }
            }

            // Print search info after completing each depth
            if let Some(mv) = current_best_move {
                println!(
                    "info depth {} score cp {} nodes {} pv {:?}",
                    current_depth, best_score, self.nodes_searched, mv
                );
            }
        }

        best_move
    }

    fn alpha_beta(&mut self, depth: u8, mut alpha: i16, beta: i16) -> Option<i16> {
        if self.time.should_stop() {
            return None;
        }
        self.nodes_searched += 1;

        // TT probe
        let zobrist = self.position.zobrist;
        if let Some(entry) = self.eval.transposition_table.probe(zobrist) {
            if entry.depth >= depth {
                match entry.node_type {
                    NodeType::EXACT => return Some(entry.value),
                    NodeType::LOWERBOUND => {
                        if entry.value >= beta {
                            return Some(entry.value);
                        }
                    }
                    NodeType::UPPERBOUND => {
                        if entry.value <= alpha {
                            return Some(entry.value);
                        }
                    }
                }
            }
        }

        if depth == 0 {
            return Some(self.eval.evaluate(&self.position));
        }

        let moves = self.movegen.legal_moves(&self.position);

        // Check for terminal positions
        if moves.is_empty() {
            let checkers = self.position.checkers(self.position.side_to_move);
            if !checkers.is_empty() {
                return Some(-VALUE_MATE + (depth as i16));
            } else {
                return Some(VALUE_DRAW);
            }
        }

        let original_alpha = alpha;
        let mut best_move = Move::none();

        // Search all moves
        for &mv in moves.iter() {
            self.position.do_move(mv);
            let score = self.alpha_beta(depth - 1, -beta, -alpha).map(|score| -score);
            self.position.undo_move(mv);

            match score {
                Some(score) => {
                    if score >= beta {
                        self.eval.transposition_table.store(
                            zobrist,
                            HashData {
                                depth,
                                value: beta,
                                best_move: mv,
                                node_type: NodeType::LOWERBOUND,
                            },
                        );
                        return Some(beta);
                    }
                    if score > alpha {
                        alpha = score;
                        best_move = mv;
                    }
                }
                None => {
                    return None;
                }
            }
        }

        // Store in TT
        let node_type = if alpha > original_alpha {
            NodeType::EXACT
        } else {
            NodeType::UPPERBOUND
        };
        self.eval.transposition_table.store(
            zobrist,
            HashData {
                depth,
                value: alpha,
                best_move,
                node_type,
            },
        );

        Some(alpha)
    }
}
