pub mod defs;
pub mod tables;
pub mod transposition;

use crate::{
    bitboards::defs::{ADJACENT_FILES_BB, FILE_BB},
    defs::*,
    misc::bits,
    position::Position,
    search::defs::VALUE_INFINITE,
};

use self::{defs::*, tables::*, transposition::TranspositionTable};

pub struct Eval {
    pub transposition_table: TranspositionTable,
    passed_pawn_mask: [[Bitboard; NrOf::SQUARES]; 2],
}

impl Eval {
    pub fn new() -> Self {
        let mut passed_pawn_mask = [[0u64; NrOf::SQUARES]; 2];

        for sq in RangeOf::SQUARES {
            let f = file_of(sq);
            let r = rank_of(sq);

            let file_mask = FILE_BB[f]
                | if f > 0 { FILE_BB[f - 1] } else { 0 }
                | if f < 7 { FILE_BB[f + 1] } else { 0 };

            // White: all ranks above r
            let mut white_mask: Bitboard = 0;
            for rank in (r + 1)..8 {
                white_mask |= 0xFFu64 << (rank * 8);
            }
            passed_pawn_mask[Sides::WHITE][sq] = file_mask & white_mask;

            // Black: all ranks below r
            let mut black_mask: Bitboard = 0;
            for rank in 0..r {
                black_mask |= 0xFFu64 << (rank * 8);
            }
            passed_pawn_mask[Sides::BLACK][sq] = file_mask & black_mask;
        }

        Self {
            transposition_table: TranspositionTable::new(DEFAULT_HASH_SIZE),
            passed_pawn_mask,
        }
    }

    pub fn evaluate(&self, position: &Position) -> i16 {
        let us: Side = position.side_to_move;
        let them: Side = us ^ 1;
        let mut middle_game: [i16; NrOf::SIDES] = [0; NrOf::SIDES];
        let mut eng_game: [i16; NrOf::SIDES] = [0; NrOf::SIDES];
        let mut phase: i16 = 0;

        for square in RangeOf::SQUARES {
            let piece = position.board[square];

            if piece != PieceType::NONE {
                let piece_type: Piece = type_of_piece(piece);
                let color: Side = color_of_piece(piece);
                let piece_index: Square = match color {
                    Sides::WHITE => square ^ 56,
                    _ => square,
                };

                middle_game[color] += PIECE_VALUES_MG[piece_type] + PIECE_SQUARE_MG_TABLES[piece_type][piece_index];
                eng_game[color] += PIECE_VALUES_EG[piece_type] + PIECE_SQUARE_EG_TABLES[piece_type][piece_index];
                phase += GAME_PHASE_INCREMENT[type_of_piece(piece)];
            }
        }

        // Pawn structure, bishop pair, and rook-on-file evaluation
        for side in [Sides::WHITE, Sides::BLACK] {
            let our_pawns = position.by_type_bb[side][PieceType::PAWN];
            let their_pawns = position.by_type_bb[side ^ 1][PieceType::PAWN];

            // Doubled pawns
            for &file in &FILE_BB {
                let pawns_on_file = (our_pawns & file).count_ones() as i16;
                if pawns_on_file > 1 {
                    let extra = pawns_on_file - 1;
                    middle_game[side] += DOUBLED_PAWN_MG * extra;
                    eng_game[side] += DOUBLED_PAWN_EG * extra;
                }
            }

            // Isolated and passed pawns
            let mut pawns = our_pawns;
            while pawns != 0 {
                let sq = bits::pop(&mut pawns);
                let f = file_of(sq);
                let relative_rank = match side {
                    Sides::WHITE => rank_of(sq),
                    _ => 7 - rank_of(sq),
                };

                // Isolated pawn
                if our_pawns & ADJACENT_FILES_BB[f] == 0 {
                    middle_game[side] += ISOLATED_PAWN_MG;
                    eng_game[side] += ISOLATED_PAWN_EG;
                }

                // Passed pawn
                if their_pawns & self.passed_pawn_mask[side][sq] == 0 {
                    middle_game[side] += PASSED_PAWN_MG[relative_rank];
                    eng_game[side] += PASSED_PAWN_EG[relative_rank];
                }
            }

            // Bishop pair
            if position.by_type_bb[side][PieceType::BISHOP].count_ones() >= 2 {
                middle_game[side] += BISHOP_PAIR_MG;
                eng_game[side] += BISHOP_PAIR_EG;
            }

            // Rook on open/semi-open file
            let mut rooks = position.by_type_bb[side][PieceType::ROOK];
            while rooks != 0 {
                let sq = bits::pop(&mut rooks);
                let file = FILE_BB[file_of(sq)];

                if our_pawns & file == 0 {
                    if their_pawns & file == 0 {
                        middle_game[side] += ROOK_OPEN_FILE_MG;
                        eng_game[side] += ROOK_OPEN_FILE_EG;
                    } else {
                        middle_game[side] += ROOK_SEMI_OPEN_FILE_MG;
                        eng_game[side] += ROOK_SEMI_OPEN_FILE_EG;
                    }
                }
            }
        }

        let mg_score: i16 = middle_game[us] - middle_game[them];
        let eg_score: i16 = eng_game[us] - eng_game[them];
        let score: i16 = match phase >= 24 {
            true => mg_score,
            false => ((mg_score as i32 * phase as i32 + eg_score as i32 * (24 - phase) as i32) / 24) as i16,
        };

        score.clamp(-VALUE_INFINITE, VALUE_INFINITE)
    }

    pub fn resize_transposition_table(&mut self, megabytes: usize) {
        self.transposition_table = TranspositionTable::new(megabytes);
    }
}
