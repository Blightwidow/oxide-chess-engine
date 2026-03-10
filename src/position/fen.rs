use super::*;

impl Position {
    pub fn set(&mut self, fen: String) {
        let fen_parts: Vec<&str> = fen.split(' ').collect::<Vec<&str>>();

        assert!(fen_parts.len() >= 2);

        self.clear();

        let mut square: usize = 0;
        for c in fen_parts[0].split('/').rev().collect::<Vec<&str>>().join("").chars() {
            if c.is_ascii_digit() {
                square += c.to_digit(10).unwrap() as usize;
            } else {
                let piece_type: Piece = match c.to_ascii_lowercase() {
                    'p' => PieceType::PAWN,
                    'n' => PieceType::KNIGHT,
                    'b' => PieceType::BISHOP,
                    'r' => PieceType::ROOK,
                    'q' => PieceType::QUEEN,
                    'k' => PieceType::KING,
                    _ => panic!("Invalid piece in FEN {}", c),
                };
                let side: Side = match c.is_ascii_lowercase() {
                    true => Sides::BLACK,
                    false => Sides::WHITE,
                };

                self.put_piece(make_piece(side, piece_type), square);
                square += 1;
            }
        }

        self.side_to_move = match fen_parts[1].to_ascii_lowercase().as_str() {
            "w" => Sides::WHITE,
            "b" => Sides::BLACK,
            _ => panic!("Invalid side to move in FEN {}", fen_parts[1]),
        };

        for c in fen_parts[2].chars() {
            match c {
                'K' => self.states.last_mut().unwrap().castling_rights |= CastlingRights::WHITE_KINGSIDE,
                'Q' => self.states.last_mut().unwrap().castling_rights |= CastlingRights::WHITE_QUEENSIDE,
                'k' => self.states.last_mut().unwrap().castling_rights |= CastlingRights::BLACK_KINGSIDE,
                'q' => self.states.last_mut().unwrap().castling_rights |= CastlingRights::BLACK_QUEENSIDE,
                '-' => (),
                _ => panic!("Invalid castling rights in FEN {}", fen_parts[2]),
            }
        }

        if fen_parts[3] != "-" {
            let file = "abcdefgh"
                .chars()
                .position(|c| c == fen_parts[3].chars().nth(0).unwrap())
                .unwrap();
            let rank = "12345678"
                .chars()
                .position(|c| c == fen_parts[3].chars().nth(1).unwrap())
                .unwrap();

            self.states.last_mut().unwrap().en_passant_square = square_of(file, rank);
        }

        if fen_parts.len() > 4 {
            self.states.last_mut().unwrap().rule50 = fen_parts[4].parse::<usize>().unwrap();
        }

        // TODO: Add fullmove number
        // if fen_parts.len() > 5 {
        //     self.states.last_mut().unwrap().fullmove = fen_parts[5].parse::<usize>().unwrap();
        // }

        for side in [Sides::WHITE, Sides::BLACK] {
            self.pinned_bb[side] = self.pinned_bb(side);
        }

        // Compute initial Zobrist hash and pawn hash
        self.zobrist = 0;
        self.pawn_hash = 0;
        for sq in RangeOf::SQUARES {
            let piece = self.board[sq];
            if piece != PieceType::NONE {
                self.zobrist ^= self.hasher.piece_key(color_of_piece(piece), type_of_piece(piece), sq);
                if type_of_piece(piece) == PieceType::PAWN {
                    self.pawn_hash ^= self.hasher.pawn_keys[color_of_piece(piece)][sq];
                }
            }
        }
        if self.side_to_move == Sides::BLACK {
            self.zobrist ^= self.hasher.side_key;
        }
        let state = self.states.last().unwrap();
        self.zobrist ^= self.hasher.castling_keys[state.castling_rights];
        if state.en_passant_square != NONE_SQUARE {
            self.zobrist ^= self.hasher.en_passant_keys[file_of(state.en_passant_square)];
        }
    }

    #[allow(dead_code)]
    pub fn fen(&self) -> String {
        let mut fen: String = String::new();

        for rank in (0..8).rev() {
            let mut empty: usize = 0;

            for file in 0..8 {
                let square: Square = square_of(file, rank);
                let piece: Piece = self.piece_on(square);

                if piece == PieceType::NONE {
                    empty += 1;
                } else {
                    if empty > 0 {
                        fen.push_str(&empty.to_string());
                        empty = 0;
                    }

                    let c: char = match type_of_piece(piece) {
                        PieceType::PAWN => 'p',
                        PieceType::KNIGHT => 'n',
                        PieceType::BISHOP => 'b',
                        PieceType::ROOK => 'r',
                        PieceType::QUEEN => 'q',
                        PieceType::KING => 'k',
                        _ => panic!("Invalid piece"),
                    };

                    if color_of_piece(piece) == Sides::WHITE {
                        fen.push(c.to_ascii_uppercase());
                    } else {
                        fen.push(c);
                    }
                }
            }

            if empty > 0 {
                fen.push_str(&empty.to_string());
            }

            if rank > 0 {
                fen.push('/');
            }
        }

        fen.push(' ');

        fen.push_str(match self.side_to_move {
            Sides::WHITE => "w",
            Sides::BLACK => "b",
            _ => panic!("Invalid side"),
        });

        fen.push(' ');

        if self.states.last().unwrap().castling_rights == CastlingRights::NONE {
            fen.push('-');
        } else {
            if self.states.last().unwrap().castling_rights & CastlingRights::WHITE_KINGSIDE != 0 {
                fen.push('K');
            }
            if self.states.last().unwrap().castling_rights & CastlingRights::WHITE_QUEENSIDE != 0 {
                fen.push('Q');
            }
            if self.states.last().unwrap().castling_rights & CastlingRights::BLACK_KINGSIDE != 0 {
                fen.push('k');
            }
            if self.states.last().unwrap().castling_rights & CastlingRights::BLACK_QUEENSIDE != 0 {
                fen.push('q');
            }
        }

        fen.push(' ');

        if self.states.last().unwrap().en_passant_square == NONE_SQUARE {
            fen.push('-');
        } else {
            fen += &pretty_square(self.states.last().unwrap().en_passant_square);
        }

        fen
    }
}
