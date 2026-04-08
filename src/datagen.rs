use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::time;

use crate::defs::*;
use crate::search::defs::{SearchLimits, FEN_START_POSITION};
use crate::search::Search;

// ─── Configuration ──────────────────────────────────────────────────────────

const DEFAULT_RANDOM_PLIES: usize = 8;
const MAX_GAME_PLY: usize = 400;
const WIN_ADJUDICATION_THRESHOLD: i16 = 3000;
const WIN_ADJUDICATION_COUNT: usize = 5;
const DRAW_ADJUDICATION_THRESHOLD: i16 = 5;
const DRAW_ADJUDICATION_COUNT: usize = 10;
const REPORT_INTERVAL: usize = 100;

pub struct DatagenConfig {
    pub depth: u8,
    pub num_games: usize,
    pub output_path: String,
}

// ─── Simple xorshift64 RNG ─────────────────────────────────────────────────

pub(crate) struct Rng {
    state: u64,
}

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0xDEAD_BEEF_CAFE_BABE } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    pub(crate) fn gen_range(&mut self, range_len: usize) -> usize {
        (self.next_u64() as usize) % range_len
    }
}

// ─── Training entry ─────────────────────────────────────────────────────────

pub(crate) struct TrainingEntry {
    pub(crate) fen: String,
    pub(crate) bestmove: String,
    pub(crate) score: i16,
    pub(crate) ply: usize,
    pub(crate) side_to_move: Side,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum GameResult {
    WhiteWin,
    BlackWin,
    Draw,
}

impl GameResult {
    fn wdl_for_side(self, side_to_move: Side) -> i16 {
        match self {
            GameResult::Draw => 0,
            GameResult::WhiteWin => {
                if side_to_move == Sides::WHITE {
                    1
                } else {
                    -1
                }
            }
            GameResult::BlackWin => {
                if side_to_move == Sides::BLACK {
                    1
                } else {
                    -1
                }
            }
        }
    }
}

// ─── Game playing ───────────────────────────────────────────────────────────

pub(crate) fn play_game(search: &mut Search, depth: u8, rng: &mut Rng) -> (Vec<TrainingEntry>, GameResult) {
    let mut entries: Vec<TrainingEntry> = Vec::with_capacity(200);

    // Reset to starting position
    search.position.set(FEN_START_POSITION.to_string());
    search.nnue.refresh(&search.position);

    // Play random opening moves for diversity
    for _ in 0..DEFAULT_RANDOM_PLIES {
        let moves = search.movegen.legal_moves(&search.position);
        if moves.is_empty() {
            break;
        }
        let index = rng.gen_range(moves.len());
        let random_move = moves[index];
        search.position.do_move(random_move);
        search.nnue.refresh(&search.position);
    }

    let mut ply: usize = 0;
    let mut win_streak: usize = 0;
    let mut draw_streak: usize = 0;

    let game_result = loop {
        let moves = search.movegen.legal_moves(&search.position);

        // Checkmate or stalemate
        if moves.is_empty() {
            let in_check = search.position.checkers_bb(search.position.side_to_move) != 0;
            break if in_check {
                if search.position.side_to_move == Sides::WHITE {
                    GameResult::BlackWin
                } else {
                    GameResult::WhiteWin
                }
            } else {
                GameResult::Draw
            };
        }

        // 50-move rule
        if search.position.states.last().unwrap().rule50 >= 100 {
            break GameResult::Draw;
        }

        // Max ply
        if ply >= MAX_GAME_PLY {
            break GameResult::Draw;
        }

        // 3-fold repetition
        if is_repetition(&search.position) {
            break GameResult::Draw;
        }

        // Search
        let mut limits = SearchLimits::default();
        limits.depth = depth;
        let (bestmove, score) = match search.run_and_return(limits) {
            Some(result) => result,
            None => break GameResult::Draw,
        };

        // Record the position
        let mut move_string = String::new();
        write!(move_string, "{:?}", bestmove).unwrap();
        entries.push(TrainingEntry {
            fen: search.position.fen(),
            bestmove: move_string,
            score,
            ply,
            side_to_move: search.position.side_to_move,
        });

        // Win adjudication
        if score.unsigned_abs() >= WIN_ADJUDICATION_THRESHOLD as u16 {
            win_streak += 1;
            if win_streak >= WIN_ADJUDICATION_COUNT {
                let stm_winning = score > 0;
                break if (search.position.side_to_move == Sides::WHITE) == stm_winning {
                    GameResult::WhiteWin
                } else {
                    GameResult::BlackWin
                };
            }
        } else {
            win_streak = 0;
        }

        // Draw adjudication
        if score.unsigned_abs() <= DRAW_ADJUDICATION_THRESHOLD as u16 {
            draw_streak += 1;
            if draw_streak >= DRAW_ADJUDICATION_COUNT {
                break GameResult::Draw;
            }
        } else {
            draw_streak = 0;
        }

        // Make the move
        search.position.do_move(bestmove);
        search.nnue.refresh(&search.position);
        ply += 1;
    };

    (entries, game_result)
}

/// Check for 3-fold repetition by scanning the position's state history.
/// State layout: states[i].zobrist stores the zobrist *before* move i was made.
/// The current position's zobrist is position.zobrist. We compare at even
/// intervals (same side to move) going backwards within the rule50 window.
pub(crate) fn is_repetition(position: &crate::position::Position) -> bool {
    let states = &position.states;
    let rule50 = match states.last() {
        Some(state) => state.rule50,
        None => return false,
    };
    if rule50 < 4 {
        return false;
    }

    let current_zobrist = position.zobrist;
    let len = states.len();
    let mut count = 0;
    let mut k = 2usize;
    while k <= rule50 && k < len {
        if states[len - k].zobrist == current_zobrist {
            count += 1;
            if count >= 2 {
                return true;
            }
        }
        k += 2;
    }
    false
}

// ─── Orchestrator ───────────────────────────────────────────────────────────

pub fn run_datagen(search: &mut Search, config: DatagenConfig) {
    let mut rng = Rng::new(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64,
    );

    let file = std::fs::File::create(&config.output_path);
    let mut writer = match file {
        Ok(file) => std::io::BufWriter::new(file),
        Err(error) => {
            println!(
                "info string datagen error: cannot create {}: {}",
                config.output_path, error
            );
            return;
        }
    };

    search.silent = true;
    let mut total_positions: usize = 0;
    let start = time::Instant::now();

    for game_index in 0..config.num_games {
        let (entries, result) = play_game(search, config.depth, &mut rng);

        for entry in &entries {
            let wdl = result.wdl_for_side(entry.side_to_move);
            let _ = writeln!(
                writer,
                "{};{};{};{};{}",
                entry.fen, entry.bestmove, entry.score, entry.ply, wdl
            );
        }

        total_positions += entries.len();

        if (game_index + 1) % REPORT_INTERVAL == 0 {
            let elapsed = start.elapsed().as_secs_f64();
            let positions_per_second = total_positions as f64 / elapsed;
            println!(
                "info string datagen: {} games, {} positions, {:.0} pos/s",
                game_index + 1,
                total_positions,
                positions_per_second
            );
        }
    }

    let _ = writer.flush();
    search.silent = false;

    let elapsed = start.elapsed().as_secs_f64();
    println!(
        "info string datagen complete: {} games, {} positions in {:.1}s, written to {}",
        config.num_games, total_positions, elapsed, config.output_path
    );
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::{
        bitboards::Bitboards, evaluate::Eval, hash::Hasher, movegen::Movegen, nnue::NnueEval, position::Position,
    };

    fn make_search() -> Search {
        let bitboards = Rc::new(Bitboards::new());
        let hasher = Rc::new(Hasher::new());
        let movegen = Movegen::new(Rc::clone(&bitboards));
        let position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));
        let nnue = NnueEval::from_bytes(crate::EMBEDDED_NET).unwrap_or_else(|| NnueEval::zero());
        Search::new(position, movegen, Eval::new(), nnue)
    }

    // ─── RNG ────────────────────────────────────────────────────────────────

    #[test]
    fn rng_produces_different_values() {
        let mut rng = Rng::new(42);
        let first = rng.next_u64();
        let second = rng.next_u64();
        assert_ne!(first, second);
    }

    #[test]
    fn rng_gen_range_within_bounds() {
        let mut rng = Rng::new(123);
        for _ in 0..1000 {
            let value = rng.gen_range(10);
            assert!(value < 10);
        }
    }

    #[test]
    fn rng_zero_seed_uses_fallback() {
        let mut rng = Rng::new(0);
        assert_ne!(rng.next_u64(), 0);
    }

    // ─── GameResult WDL ─────────────────────────────────────────────────────

    #[test]
    fn wdl_draw_is_zero() {
        assert_eq!(GameResult::Draw.wdl_for_side(Sides::WHITE), 0);
        assert_eq!(GameResult::Draw.wdl_for_side(Sides::BLACK), 0);
    }

    #[test]
    fn wdl_white_win_perspectives() {
        assert_eq!(GameResult::WhiteWin.wdl_for_side(Sides::WHITE), 1);
        assert_eq!(GameResult::WhiteWin.wdl_for_side(Sides::BLACK), -1);
    }

    #[test]
    fn wdl_black_win_perspectives() {
        assert_eq!(GameResult::BlackWin.wdl_for_side(Sides::BLACK), 1);
        assert_eq!(GameResult::BlackWin.wdl_for_side(Sides::WHITE), -1);
    }

    // ─── Repetition detection ───────────────────────────────────────────────

    #[test]
    fn no_repetition_at_startpos() {
        let search = make_search();
        assert!(!is_repetition(&search.position));
    }

    #[test]
    fn no_repetition_after_single_cycle() {
        let mut search = make_search();
        // One Ng1-f3, Ng8-f6, Nf3-g1, Nf6-g8 cycle = 2-fold, not 3-fold
        let moves = ["g1f3", "g8f6", "f3g1", "f6g8"];
        for uci_str in &moves {
            let legal = search.movegen.legal_moves(&search.position);
            let mv = legal.iter().find(|m| format!("{:?}", m) == *uci_str).copied().unwrap();
            search.position.do_move(mv);
        }
        assert!(!is_repetition(&search.position));
    }

    #[test]
    fn detects_threefold_repetition() {
        let mut search = make_search();
        // Two full cycles = 3-fold
        let moves = ["g1f3", "g8f6", "f3g1", "f6g8", "g1f3", "g8f6", "f3g1", "f6g8"];
        for uci_str in &moves {
            let legal = search.movegen.legal_moves(&search.position);
            let mv = legal.iter().find(|m| format!("{:?}", m) == *uci_str).copied().unwrap();
            search.position.do_move(mv);
        }
        assert!(is_repetition(&search.position));
    }

    // ─── Game playing ───────────────────────────────────────────────────────

    #[test]
    fn play_game_produces_entries() {
        let mut search = make_search();
        search.silent = true;
        let mut rng = Rng::new(42);
        let (entries, _) = play_game(&mut search, 1, &mut rng);
        assert!(!entries.is_empty(), "Game should produce training entries");
    }

    #[test]
    fn play_game_entries_have_valid_fens() {
        let mut search = make_search();
        search.silent = true;
        let mut rng = Rng::new(42);
        let (entries, _) = play_game(&mut search, 1, &mut rng);

        for entry in &entries {
            assert!(entry.fen.contains(' '), "FEN should be valid: {}", entry.fen);
            assert!(
                entry.bestmove.len() >= 4 && entry.bestmove.len() <= 5,
                "Move string should be 4-5 chars: {}",
                entry.bestmove
            );
        }
    }

    #[test]
    fn play_game_plies_are_sequential() {
        let mut search = make_search();
        search.silent = true;
        let mut rng = Rng::new(42);
        let (entries, _) = play_game(&mut search, 1, &mut rng);

        for (index, entry) in entries.iter().enumerate() {
            assert_eq!(entry.ply, index);
        }
    }

    #[test]
    fn play_game_respects_max_ply() {
        let mut search = make_search();
        search.silent = true;
        let mut rng = Rng::new(42);
        let (entries, _) = play_game(&mut search, 1, &mut rng);

        for entry in &entries {
            assert!(entry.ply < MAX_GAME_PLY, "Ply {} exceeds max", entry.ply);
        }
    }

    #[test]
    fn play_game_different_seeds_different_openings() {
        let mut search = make_search();
        search.silent = true;

        let mut rng1 = Rng::new(1);
        let (entries1, _) = play_game(&mut search, 1, &mut rng1);

        let mut rng2 = Rng::new(999);
        let (entries2, _) = play_game(&mut search, 1, &mut rng2);

        assert_ne!(
            entries1[0].fen, entries2[0].fen,
            "Different seeds should produce different openings"
        );
    }

    #[test]
    fn play_game_sides_alternate() {
        let mut search = make_search();
        search.silent = true;
        let mut rng = Rng::new(42);
        let (entries, _) = play_game(&mut search, 1, &mut rng);

        // After random plies, sides should still alternate each ply
        for window in entries.windows(2) {
            assert_ne!(
                window[0].side_to_move, window[1].side_to_move,
                "Consecutive entries should have opposite sides to move"
            );
        }
    }
}
