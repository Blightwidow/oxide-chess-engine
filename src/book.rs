mod keys;

use crate::defs::*;
use crate::movegen::defs::{CastlingRights, Move, MoveTypes};
use crate::movegen::Movegen;
use crate::position::Position;

use self::keys::POLYGLOT_KEYS;

/// Parsed 16-byte Polyglot book entry.
struct BookEntry {
    key: u64,
    raw_move: u16,
    weight: u16,
}

pub struct OpeningBook {
    entries: Vec<BookEntry>,
}

impl OpeningBook {
    /// Load a Polyglot `.bin` book from disk. Returns `None` on any I/O or format error.
    pub fn load(path: &str) -> Option<Self> {
        let data = std::fs::read(path).ok()?;
        if data.len() % 16 != 0 {
            return None;
        }

        let count = data.len() / 16;
        let mut entries = Vec::with_capacity(count);

        for index in 0..count {
            let offset = index * 16;
            let key = u64::from_be_bytes(data[offset..offset + 8].try_into().unwrap());
            let raw_move = u16::from_be_bytes(data[offset + 8..offset + 10].try_into().unwrap());
            let weight = u16::from_be_bytes(data[offset + 10..offset + 12].try_into().unwrap());
            // bytes 12..16 are the learn field (ignored)

            if raw_move != 0 {
                entries.push(BookEntry { key, raw_move, weight });
            }
        }

        println!("info string Book loaded: {} entries from {}", entries.len(), path);
        Some(Self { entries })
    }

    /// Probe the book for the current position. Returns a move selected by weighted random
    /// from all matching entries, validated against the legal move list.
    pub fn probe(&self, position: &Position, movegen: &Movegen) -> Option<Move> {
        let hash = polyglot_hash(position);

        // Binary search to find the first entry with this key
        let start = self.entries.partition_point(|entry| entry.key < hash);

        // Collect all entries matching this hash
        let mut total_weight: u32 = 0;
        let mut candidates: Vec<(u16, u16)> = Vec::new();
        for entry in &self.entries[start..] {
            if entry.key != hash {
                break;
            }
            candidates.push((entry.raw_move, entry.weight));
            total_weight += entry.weight as u32;
        }

        if candidates.is_empty() || total_weight == 0 {
            return None;
        }

        // Weighted random selection
        let legal_moves = movegen.legal_moves(position);
        let mut random_value = xorshift_seed() % total_weight;
        for (raw_move, weight) in &candidates {
            if let Some(engine_move) = poly_move_to_engine(*raw_move, position, &legal_moves) {
                if random_value < *weight as u32 {
                    return Some(engine_move);
                }
                random_value = random_value.saturating_sub(*weight as u32);
            }
        }

        // Fallback: return first valid candidate (in case rounding ate the selection)
        for (raw_move, _weight) in &candidates {
            if let Some(engine_move) = poly_move_to_engine(*raw_move, position, &legal_moves) {
                return Some(engine_move);
            }
        }

        None
    }
}

/// Convert a Polyglot move encoding to an engine `Move` by matching against the legal move list.
///
/// Polyglot move bits: 0-2 to_file, 3-5 to_rank, 6-8 from_file, 9-11 from_rank, 12-14 promotion.
/// Castling is encoded as king→rook (same as engine internal encoding).
fn poly_move_to_engine(raw_move: u16, _position: &Position, legal_moves: &[Move]) -> Option<Move> {
    let to_file = (raw_move & 0x7) as usize;
    let to_rank = ((raw_move >> 3) & 0x7) as usize;
    let from_file = ((raw_move >> 6) & 0x7) as usize;
    let from_rank = ((raw_move >> 9) & 0x7) as usize;
    let promotion = ((raw_move >> 12) & 0x7) as usize;

    let from_square = square_of(from_file, from_rank);
    let to_square = square_of(to_file, to_rank);

    // Map Polyglot promotion encoding (1=N, 2=B, 3=R, 4=Q) to engine PieceType
    let promotion_piece = match promotion {
        1 => PieceType::KNIGHT,
        2 => PieceType::BISHOP,
        3 => PieceType::ROOK,
        4 => PieceType::QUEEN,
        _ => PieceType::NONE,
    };

    // Find the matching legal move. Both engine and Polyglot use king→rook for castling.
    for &legal_move in legal_moves {
        if legal_move.from_sq() != from_square || legal_move.to_sq() != to_square {
            continue;
        }

        // For promotions, also match the promotion piece
        if legal_move.type_of() == MoveTypes::PROMOTION {
            if legal_move.promotion_type() == promotion_piece {
                return Some(legal_move);
            }
        } else if promotion_piece == PieceType::NONE {
            return Some(legal_move);
        }
    }

    // No matching legal move found (hash collision or corrupt entry)
    None
}

/// Compute the Polyglot Zobrist hash for a position.
///
/// This uses the standard Polyglot random table, which differs from the engine's internal
/// Zobrist keys. Only called at root, so performance is not critical.
pub fn polyglot_hash(position: &Position) -> u64 {
    let mut hash: u64 = 0;

    // Pieces: Polyglot kind = (piece_type - 1) * 2 + (1 - side)
    // Key index = kind * 64 + square
    for square in RangeOf::SQUARES {
        let piece = position.board[square];
        if piece == PieceType::NONE {
            continue;
        }
        let side = color_of_piece(piece);
        let piece_type = type_of_piece(piece);
        let polyglot_kind = (piece_type - 1) * 2 + (1 - side);
        hash ^= POLYGLOT_KEYS[polyglot_kind * 64 + square];
    }

    // Castling rights
    let state = position.states.last().unwrap();
    if state.castling_rights & CastlingRights::WHITE_KINGSIDE != 0 {
        hash ^= POLYGLOT_KEYS[768];
    }
    if state.castling_rights & CastlingRights::WHITE_QUEENSIDE != 0 {
        hash ^= POLYGLOT_KEYS[769];
    }
    if state.castling_rights & CastlingRights::BLACK_KINGSIDE != 0 {
        hash ^= POLYGLOT_KEYS[770];
    }
    if state.castling_rights & CastlingRights::BLACK_QUEENSIDE != 0 {
        hash ^= POLYGLOT_KEYS[771];
    }

    // En passant: only include if a capture is actually possible
    let en_passant_square = state.en_passant_square;
    if en_passant_square != NONE_SQUARE {
        let ep_file = file_of(en_passant_square);
        let ep_rank = rank_of(en_passant_square);
        let capturing_side = position.side_to_move;
        // Capturing pawns sit one rank behind the EP square from the capturer's perspective
        let pawn_rank = if capturing_side == Sides::WHITE {
            ep_rank - 1
        } else {
            ep_rank + 1
        };
        let capturing_pawn = make_piece(capturing_side, PieceType::PAWN);
        let mut can_capture = false;
        if ep_file > 0 {
            can_capture |= position.board[square_of(ep_file - 1, pawn_rank)] == capturing_pawn;
        }
        if ep_file < 7 {
            can_capture |= position.board[square_of(ep_file + 1, pawn_rank)] == capturing_pawn;
        }
        if can_capture {
            hash ^= POLYGLOT_KEYS[772 + ep_file];
        }
    }

    // Turn: XOR when white to move (Polyglot convention)
    if position.side_to_move == Sides::WHITE {
        hash ^= POLYGLOT_KEYS[780];
    }

    hash
}

/// Simple seed from system time for weighted random selection.
fn xorshift_seed() -> u32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    // Mix the bits a little
    let mut state = nanos;
    state ^= state << 13;
    state ^= state >> 17;
    state ^= state << 5;
    if state == 0 {
        1
    } else {
        state
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::bitboards::Bitboards;
    use crate::hash::Hasher;
    use std::rc::Rc;

    fn make_position(fen: &str) -> Position {
        let bitboards = Rc::new(Bitboards::new());
        let hasher = Rc::new(Hasher::new());
        let mut position = Position::new(bitboards, hasher);
        position.set(fen.to_string());
        position
    }

    #[test]
    fn polyglot_hash_startpos() {
        let position = make_position("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(polyglot_hash(&position), 0x463b96181691fc9c);
    }

    #[test]
    fn polyglot_hash_after_e4() {
        let position = make_position("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        assert_eq!(polyglot_hash(&position), 0x823c9b50fd114196);
    }

    #[test]
    fn polyglot_hash_after_e4_d5() {
        let position = make_position("rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2");
        assert_eq!(polyglot_hash(&position), 0x0756b94461c50fb0);
    }

    #[test]
    fn polyglot_hash_ep_excluded_when_not_capturable() {
        // Position with EP square set (h3) but no black pawn on g-file rank 4 to capture.
        // Hash should equal the same position without EP — i.e., EP key must NOT be included.
        let with_ep = make_position("rnbqkbnr/pppppppp/8/8/7P/8/PPPPPPP1/RNBQKBNR b KQkq h3 0 1");
        let without_ep = make_position("rnbqkbnr/pppppppp/8/8/7P/8/PPPPPPP1/RNBQKBNR b KQkq - 0 1");
        assert_eq!(polyglot_hash(&with_ep), polyglot_hash(&without_ep));
    }

    #[test]
    fn polyglot_hash_ep_included_when_capturable() {
        // After 1.e4 d5: EP on d6, white pawn on e5 could capture — but here e4 pawn.
        // The d6 EP after d7-d5 is capturable by e4 pawn (e4xd5 not e.p., but e5xd6 would be).
        // Use a position where EP IS capturable: pawns on adjacent file at correct rank.
        let with_ep = make_position("rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2");
        let without_ep = make_position("rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2");
        // EP IS capturable (white pawn on e5, adjacent to d-file), so hashes must differ
        assert_ne!(polyglot_hash(&with_ep), polyglot_hash(&without_ep));
    }
}
