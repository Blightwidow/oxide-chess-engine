pub mod defs;
mod test;

use std::cmp;

use crate::{
    evaluate::{defs::PAWN_UNIT, Eval},
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
            self.iterative_deepening(limits);
        } else {
            let score = self.eval.evaluate(&self.position);
            println!("info depth 0 score cp {}", score);
        }
    }

    fn iterative_deepening(&mut self, limits: SearchLimits) {
        let mut movelist = self
            .movegen
            .legal_moves(&self.position)
            .iter()
            .map(|&mv| (mv, 0i64))
            .collect::<arrayvec::ArrayVec<(Move, i64), 256>>();

        if movelist.is_empty() {
            println!("bestmove 0000");
            return;
        } else if movelist.len() == 1 {
            println!("bestmove {:?}", movelist[0]);
            return;
        }

        let mut last_score: i16 = 0;

        for depth in 1u8..limits.depth + 1 {
            if self.time.should_stop() {
                break;
            }

            if let Some(best_score) = self.aspiration_window(last_score, &mut movelist, depth) {
                last_score = best_score;
                movelist[1..].sort_by_key(|&(_, subtree_size)| -subtree_size)
            }

            println!("info depth {} score cp {} pv {:?}", depth, last_score, movelist[0].0);
        }

        println!("bestmove {:?}", movelist[0].0);
    }

    fn aspiration_window(&mut self, last_score: i16, moves: &mut [(Move, i64)], depth: u8) -> Option<i16> {
        let mut delta = PAWN_UNIT / 2;
        let mut alpha = cmp::max(last_score - delta, -VALUE_MATE);
        let mut beta = cmp::min(last_score + delta, VALUE_MATE);

        loop {
            let (score, index) = self.search_root(moves, alpha, beta, depth)?;
            moves[0..index + 1].rotate_right(1);

            delta += delta / 3;

            if score >= beta {
                beta = cmp::min(VALUE_MATE, score + delta);
            } else if score <= alpha {
                alpha = cmp::max(score - delta, -VALUE_MATE);
            } else {
                return Some(score);
            }
        }
    }

    fn search_root(&mut self, moves: &mut [(Move, i64)], alpha: i16, beta: i16, depth: u8) -> Option<(i16, usize)> {
        let mut alpha = alpha;
        let mut best_score = -VALUE_INFINITE;
        let mut best_move_index = 0;
        let mut increased_alpha = false;

        for (i, &mut (mv, ref mut subtree_size)) in moves.iter_mut().enumerate() {
            self.position.do_move(mv);
            let mut score = Some(VALUE_INFINITE);

            if i > 0 {
                score = self.search(-alpha - 1, -alpha, depth).map(|v| -v);
            }

            if Some(alpha) < score {
                score = self.search(-beta, -alpha, depth).map(|v| -v);
            }

            self.position.undo_move(mv);

            match score {
                None => {
                    if increased_alpha {
                        return Some((best_score, best_move_index));
                    } else {
                        return None;
                    }
                }
                Some(value) => {
                    if value > best_score {
                        best_score = value;
                        best_move_index = i;
                    }

                    if value > alpha {
                        alpha = value;
                        increased_alpha = true;
                        // self.add_pv_move(mov, 0);
                    }

                    if value >= beta {
                        break;
                    }
                }
            }
        }

        Some((best_score, best_move_index))
    }

    fn search(&mut self, alpha: i16, beta: i16, depth: u8) -> Option<i16> {
        if self.time.should_stop() {
            return None;
        }

        if depth == 0 {
            return Some(self.eval.evaluate(&self.position));
        }

        let is_pv = alpha + 1 != beta;

        // TODO: Add check for Draw  and 50 move rule ?

        let movelist = self.movegen.legal_moves(&self.position);
        let mut alpha = alpha;
        let mut best_score = -VALUE_MATE;
        let mut num_moves_searched = 0;

        for mv in movelist {
            self.position.do_move(mv);
            let mut score: Option<i16> = Some(VALUE_MATE);

            if !(is_pv && num_moves_searched == 0) {
                score = self.search(-alpha - 1, -alpha, depth - 1).map(|v| -v);
            }

            num_moves_searched += 1;

            if Some(alpha) < score && is_pv {
                score = self.search(-beta, -alpha, depth - 1).map(|v| -v);
            }

            self.position.undo_move(mv);

            match score {
                None => {
                    return None;
                }
                Some(value) => {
                    if value > best_score {
                        best_score = value;
                    }

                    if value > alpha {
                        alpha = value;
                    }

                    if value >= beta {
                        break;
                    }
                }
            }
        }

        Some(best_score)
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
}
