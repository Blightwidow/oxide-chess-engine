pub mod defs;

use self::defs::*;
use crate::defs::*;

#[derive(Clone)]
pub struct Bitboards {
    king: [Bitboard; NrOf::SQUARES],
    knight: [Bitboard; NrOf::SQUARES],
    pawn_attacks: [[Bitboard; NrOf::SQUARES]; NrOf::SIDES],
    rook: Vec<Bitboard>,
    bishop: Vec<Bitboard>,
    rook_magics: [Magic; NrOf::SQUARES],
    bishop_magics: [Magic; NrOf::SQUARES],
    pub line_bb: [[Bitboard; NrOf::SQUARES]; NrOf::SQUARES],
    pub between_bb: [[Bitboard; NrOf::SQUARES]; NrOf::SQUARES],
}

// extern Bitboard BetweenBB[SQUARE_NB][SQUARE_NB];
// extern Bitboard LineBB[SQUARE_NB][SQUARE_NB];

impl Bitboards {
    pub fn new() -> Self {
        let mut mg: Bitboards = Self {
            king: [EMPTY; NrOf::SQUARES],
            knight: [EMPTY; NrOf::SQUARES],
            pawn_attacks: [[EMPTY; NrOf::SQUARES]; NrOf::SIDES],
            rook: vec![EMPTY; 0x19000],
            bishop: vec![EMPTY; 0x1480],
            rook_magics: [Magic::default(); NrOf::SQUARES],
            bishop_magics: [Magic::default(); NrOf::SQUARES],
            line_bb: [[EMPTY; NrOf::SQUARES]; NrOf::SQUARES],
            between_bb: [[EMPTY; NrOf::SQUARES]; NrOf::SQUARES],
        };
        Bitboards::init_magics(PieceType::ROOK, &mut mg.rook_magics, &mut mg.rook);
        Bitboards::init_magics(PieceType::BISHOP, &mut mg.bishop_magics, &mut mg.bishop);

        for from in RangeOf::SQUARES {
            for step in [-9, -8, -7, -1, 1, 7, 8, 9] {
                mg.king[from] |= Bitboards::safe_destination(from, step);
            }

            for step in [-17, -15, -10, -6, 6, 10, 15, 17] {
                mg.knight[from] |= Bitboards::safe_destination(from, step);
            }

            for step in [7, 9] {
                mg.pawn_attacks[Sides::WHITE][from] |= Bitboards::safe_destination(from, step);
            }

            for step in [-7, -9] {
                mg.pawn_attacks[Sides::BLACK][from] |= Bitboards::safe_destination(from, step);
            }

            for piece in [PieceType::ROOK, PieceType::BISHOP] {
                for to in RangeOf::SQUARES {
                    if Bitboards::sliding_attack(piece, from, EMPTY) & square_bb(to) == EMPTY  {
                        continue;
                    }

                    mg.line_bb[from][to] = (Bitboards::sliding_attack(piece, from, EMPTY)
                        & Bitboards::sliding_attack(piece, to, EMPTY))
                        | square_bb(from)
                        | square_bb(to);

                    if from == to {
                        continue;
                    }

                    mg.between_bb[from][to] = Bitboards::sliding_attack(piece, from, square_bb(to))
                        & Bitboards::sliding_attack(piece, to, square_bb(from));
                }
            }
        }

        mg
    }

    pub fn attack_bb(&self, piece: Piece, square: Square, occupied: Bitboard) -> Bitboard {
        match type_of_piece(piece) {
            PieceType::PAWN => self.pawn_attacks[color_of_piece(piece)][square],
            PieceType::KING => self.king[square],
            PieceType::KNIGHT => self.knight[square],
            PieceType::BISHOP => self.bishop[self.bishop_magics[square].get_index(occupied)],
            PieceType::ROOK => self.rook[self.rook_magics[square].get_index(occupied)],
            PieceType::QUEEN => {
                self.bishop[self.bishop_magics[square].get_index(occupied)]
                    | self.rook[self.rook_magics[square].get_index(occupied)]
            }
            piece => panic!("Invalid piece {}.", piece),
        }
    }

    pub fn sliding_attack(piece: Piece, square: Square, occupied: Bitboard) -> Bitboard {
        let piece_type = type_of_piece(piece);
        assert!(
            piece_type == PieceType::ROOK || piece_type == PieceType::BISHOP,
            "Invalid piece {}",
            piece_type
        );

        let mut attack_bb: Bitboard = EMPTY;

        let directions: [Direction; 4] = match piece_type {
            PieceType::ROOK => [Directions::UP, Directions::DOWN, Directions::LEFT, Directions::RIGHT],
            PieceType::BISHOP => [
                Directions::UP_LEFT,
                Directions::UP_RIGHT,
                Directions::DOWN_LEFT,
                Directions::DOWN_RIGHT,
            ],
            _ => panic!("Invalid piece."),
        };

        for direction in directions {
            let mut s: isize = square as isize;
            let mut dest_bb = Bitboards::safe_destination(s as usize, direction);

            while dest_bb > 0 {
                let met = occupied & dest_bb != 0;
                attack_bb |= dest_bb;
                s += direction;
                dest_bb = Bitboards::safe_destination(s as usize, direction);

                if met {
                    break;
                }
            }
        }

        attack_bb
    }

    // Returns the bitboard of target square for the given step
    // from the given square. If the step is off the board, returns empty bitboard.
    fn safe_destination(square: Square, step: Direction) -> Bitboard {
        let to = square as isize + step;

        match to {
            to if !(0..=63).contains(&to) || distance(square, to as usize) > 2 => EMPTY,
            _ => 1u64 << to,
        }
    }

    pub fn init_magics(piece_type: Piece, magics: &mut [Magic; NrOf::SQUARES], table: &mut [Bitboard]) {
        assert!(
            piece_type == PieceType::ROOK || piece_type == PieceType::BISHOP,
            "Invalid piece."
        );

        let mut offset: u64 = 0;

        for square in RangeOf::SQUARES {
            // Board edges are not considered in the relevant occupancies
            let edges: Bitboard =
                ((RANK_1BB | RANK_8BB) & !rank_bb(square)) | ((FILE_ABB | FILE_HBB) & !file_bb(square));
            let mask = Bitboards::sliding_attack(piece_type, square, EMPTY) & !edges;
            let mut occupancy: [Bitboard; 4096] = [EMPTY; 4096];
            let mut reference: [Bitboard; 4096] = [EMPTY; 4096];

            let magic = &mut magics[square];
            magic.mask = mask;
            magic.shift = 64 - mask.count_ones() as u8;
            magic.offset = offset;
            magic.number = match piece_type {
                PieceType::ROOK => ROOK_MAGIC_NUMBERS[square],
                PieceType::BISHOP => BISHOP_MAGIC_NUMBERS[square],
                _ => panic!("Invalid piece."),
            };

            // Let's init the occupancy and reference arrays
            let mut occupied: Bitboard = EMPTY;
            let mut count: usize = 0;
            loop {
                occupancy[count] = occupied;
                reference[count] = Bitboards::sliding_attack(piece_type, square, occupied);

                count += 1;
                occupied = occupied.wrapping_sub(mask) & mask;

                // If occupancy is 0, we have reached the end of the permutations.
                if occupied == 0 {
                    break;
                }
            }

            let end = offset + (count - 1) as u64;
            for i in 0..count {
                let index = magic.get_index(occupancy[i]);
                // If the table at this index is empty...
                if table[index] == EMPTY {
                    // Check if we're within the expected range
                    let fail_low = index < offset as usize;
                    let fail_high = index > end as usize;
                    assert!(!fail_low && !fail_high, "Indexing error.");

                    // We found a working magic.
                    table[index] = reference[i];
                } else {
                    panic!("Invalid magic number.");
                }
            }

            offset += count as u64;
        }
    }

    pub fn aligned(&self, a: Square, b: Square, c: Square) -> bool {
        self.line_bb[a][b] & square_bb(c) != EMPTY
    }

    #[allow(dead_code)]
    pub fn pretty(bitboard: Bitboard) -> String {
        let mut output = "  A   B   C   D   E   F   G   H  \n+---+---+---+---+---+---+---+---+\n".to_owned();

        for rank in RangeOf::RANKS.rev() {
            for file in RangeOf::FILES {
                let square = square_of(file, rank);
                output += match bitboard & (1u64 << square) {
                    0 => "|   ",
                    _ => "| X ",
                };
            }
            output += format!("| {}\n+---+---+---+---+---+---+---+---+\n", rank + 1).as_str();
        }

        output
    }
}
