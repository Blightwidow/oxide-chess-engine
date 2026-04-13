use std::sync::OnceLock;

use pyrrhic_rs::{self, EngineAdapter, TableBases, WdlProbeResult};

use crate::{
    bitboards::{defs::EMPTY, Bitboards},
    defs::*,
    movegen::defs::{Move, MoveTypes},
    position::Position,
    search::defs::{VALUE_DRAW, VALUE_MATE},
};

// ─── Score Constants ────────────────────────────────────────────────────────

/// Tablebase win score: clearly winning but distinct from checkmate.
/// VALUE_MATE - MAX_PLY - 1 so mate scores always outrank TB wins.
pub const VALUE_TB_WIN: i16 = VALUE_MATE - 129;
pub const VALUE_TB_LOSS: i16 = -VALUE_TB_WIN;

/// Maximum search ply (mirrors search::MAX_PLY)
const MAX_PLY: usize = 128;

// ─── Static Attack Tables ───────────────────────────────────────────────────

/// Global bitboards instance used by the EngineAdapter associated functions.
/// Initialized once via `init_attack_tables()` before any tablebase probing.
static ATTACK_TABLES: OnceLock<Bitboards> = OnceLock::new();

/// Initialize the static attack tables for tablebase probing.
/// Must be called once at startup before creating any `Tablebase` instance.
pub fn init_attack_tables(bitboards: &Bitboards) {
    ATTACK_TABLES.get_or_init(|| bitboards.clone());
}

// ─── Engine Adapter ─────────────────────────────────────────────────────────

/// Adapter bridging pyrrhic-rs to our attack generation.
///
/// pyrrhic-rs Color: Black=0, White=1
/// Our engine Sides: WHITE=0, BLACK=1
#[derive(Clone)]
struct OxidAdapter;

impl EngineAdapter for OxidAdapter {
    fn pawn_attacks(color: pyrrhic_rs::Color, square: u64) -> u64 {
        let bitboards = ATTACK_TABLES.get().expect("attack tables not initialized");
        // pyrrhic: Black=0, White=1 → our engine: WHITE=0, BLACK=1
        let side = match color {
            pyrrhic_rs::Color::White => Sides::WHITE,
            pyrrhic_rs::Color::Black => Sides::BLACK,
        };
        bitboards.attack_bb(make_piece(side, PieceType::PAWN), square as usize, EMPTY)
    }

    fn knight_attacks(square: u64) -> u64 {
        let bitboards = ATTACK_TABLES.get().expect("attack tables not initialized");
        bitboards.attack_bb(PieceType::KNIGHT, square as usize, EMPTY)
    }

    fn bishop_attacks(square: u64, occupied: u64) -> u64 {
        let bitboards = ATTACK_TABLES.get().expect("attack tables not initialized");
        bitboards.attack_bb(PieceType::BISHOP, square as usize, occupied)
    }

    fn rook_attacks(square: u64, occupied: u64) -> u64 {
        let bitboards = ATTACK_TABLES.get().expect("attack tables not initialized");
        bitboards.attack_bb(PieceType::ROOK, square as usize, occupied)
    }

    fn queen_attacks(square: u64, occupied: u64) -> u64 {
        let bitboards = ATTACK_TABLES.get().expect("attack tables not initialized");
        bitboards.attack_bb(PieceType::QUEEN, square as usize, occupied)
    }

    fn king_attacks(square: u64) -> u64 {
        let bitboards = ATTACK_TABLES.get().expect("attack tables not initialized");
        bitboards.attack_bb(PieceType::KING, square as usize, EMPTY)
    }
}

// ─── Tablebase Wrapper ──────────────────────────────────────────────────────

pub struct Tablebase {
    inner: TableBases<OxidAdapter>,
    max_pieces: u32,
}

impl Tablebase {
    /// Load Syzygy tablebases from a colon-separated path.
    pub fn new(path: &str) -> Result<Self, String> {
        let inner = TableBases::<OxidAdapter>::new(path).map_err(|error| format!("{:?}", error))?;
        let max_pieces = inner.max_pieces();
        Ok(Self { inner, max_pieces })
    }

    /// Maximum number of pieces (including kings) the loaded tables support.
    pub fn max_pieces(&self) -> u32 {
        self.max_pieces
    }

    /// Probe WDL tables for the given position.
    /// Returns None if the position is outside tablebase range or has castling rights.
    pub fn probe_wdl(&self, position: &Position) -> Option<WdlProbeResult> {
        if !self.can_probe(position) {
            return None;
        }

        let state = position.states.last().unwrap();
        let (white, black, kings, queens, rooks, bishops, knights, pawns) = extract_bitboards(position);
        let en_passant = if state.en_passant_square == NONE_SQUARE {
            0
        } else {
            state.en_passant_square as u32
        };
        let white_to_move = position.side_to_move == Sides::WHITE;

        self.inner
            .probe_wdl(
                white,
                black,
                kings,
                queens,
                rooks,
                bishops,
                knights,
                pawns,
                en_passant,
                white_to_move,
            )
            .ok()
    }

    /// Probe DTZ tables at the root position.
    /// Returns the best move and its WDL result, or None if probe fails.
    pub fn probe_root(&self, position: &Position) -> Option<(Move, WdlProbeResult)> {
        if !self.can_probe(position) {
            return None;
        }

        let state = position.states.last().unwrap();
        let (white, black, kings, queens, rooks, bishops, knights, pawns) = extract_bitboards(position);
        let en_passant = if state.en_passant_square == NONE_SQUARE {
            0
        } else {
            state.en_passant_square as u32
        };
        let white_to_move = position.side_to_move == Sides::WHITE;

        let result = self
            .inner
            .probe_root(
                white,
                black,
                kings,
                queens,
                rooks,
                bishops,
                knights,
                pawns,
                state.rule50 as u32,
                en_passant,
                white_to_move,
            )
            .ok()?;

        // Extract the root's best move from the DTZ result
        match result.root {
            pyrrhic_rs::DtzProbeValue::DtzResult(dtz) => {
                let mv = dtz_result_to_move(&dtz, position);
                Some((mv, dtz.wdl))
            }
            _ => None,
        }
    }

    /// Check if position is eligible for tablebase probing.
    fn can_probe(&self, position: &Position) -> bool {
        let piece_count = position.by_color_bb[Sides::BOTH].count_ones();
        let state = position.states.last().unwrap();
        piece_count <= self.max_pieces && state.castling_rights == 0
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Extract per-piece-type bitboards (combined both sides) for pyrrhic-rs probe calls.
fn extract_bitboards(position: &Position) -> (u64, u64, u64, u64, u64, u64, u64, u64) {
    let white = position.by_color_bb[Sides::WHITE];
    let black = position.by_color_bb[Sides::BLACK];
    let kings = position.by_type_bb[Sides::WHITE][PieceType::KING] | position.by_type_bb[Sides::BLACK][PieceType::KING];
    let queens =
        position.by_type_bb[Sides::WHITE][PieceType::QUEEN] | position.by_type_bb[Sides::BLACK][PieceType::QUEEN];
    let rooks = position.by_type_bb[Sides::WHITE][PieceType::ROOK] | position.by_type_bb[Sides::BLACK][PieceType::ROOK];
    let bishops =
        position.by_type_bb[Sides::WHITE][PieceType::BISHOP] | position.by_type_bb[Sides::BLACK][PieceType::BISHOP];
    let knights =
        position.by_type_bb[Sides::WHITE][PieceType::KNIGHT] | position.by_type_bb[Sides::BLACK][PieceType::KNIGHT];
    let pawns = position.by_type_bb[Sides::WHITE][PieceType::PAWN] | position.by_type_bb[Sides::BLACK][PieceType::PAWN];
    (white, black, kings, queens, rooks, bishops, knights, pawns)
}

/// Convert a pyrrhic-rs DTZ result into our Move type.
fn dtz_result_to_move(dtz: &pyrrhic_rs::DtzResult, position: &Position) -> Move {
    let from = dtz.from_square as usize;
    let to = dtz.to_square as usize;

    // Detect en passant
    if dtz.ep {
        return Move::make(from, to, PieceType::NONE, MoveTypes::EN_PASSANT);
    }

    // Detect promotion
    let promotion_piece = match dtz.promotion {
        pyrrhic_rs::Piece::Queen => PieceType::QUEEN,
        pyrrhic_rs::Piece::Rook => PieceType::ROOK,
        pyrrhic_rs::Piece::Bishop => PieceType::BISHOP,
        pyrrhic_rs::Piece::Knight => PieceType::KNIGHT,
        _ => PieceType::NONE,
    };
    if promotion_piece != PieceType::NONE {
        return Move::make(from, to, promotion_piece, MoveTypes::PROMOTION);
    }

    // Detect castling: king moves 2+ squares horizontally
    let piece_type = type_of_piece(position.board[from]);
    if piece_type == PieceType::KING && distance(from, to) > 1 {
        // pyrrhic gives king destination square, but our engine encodes castling as king→rook
        let rook_square = if to > from {
            // Kingside: rook is at h1/h8
            from - (from % 8) + 7
        } else {
            // Queenside: rook is at a1/a8
            from - (from % 8)
        };
        return Move::make(from, rook_square, PieceType::NONE, MoveTypes::CASTLING);
    }

    Move::with_from_to(from, to)
}

/// Convert a WDL probe result to a centipawn score, adjusted by ply.
/// Closer TB wins score higher; closer TB losses score lower (worse).
pub fn wdl_to_score(wdl: WdlProbeResult, ply: usize) -> i16 {
    match wdl {
        WdlProbeResult::Win => VALUE_TB_WIN - ply as i16,
        WdlProbeResult::Loss => VALUE_TB_LOSS + ply as i16,
        WdlProbeResult::Draw | WdlProbeResult::CursedWin | WdlProbeResult::BlessedLoss => VALUE_DRAW,
    }
}

/// Adjust a TB score for TT storage: convert ply-relative to root-relative.
pub fn adjust_tb_score_to_tt(score: i16, ply: usize) -> i16 {
    if score > VALUE_TB_WIN - MAX_PLY as i16 {
        score + ply as i16
    } else if score < VALUE_TB_LOSS + MAX_PLY as i16 {
        score - ply as i16
    } else {
        score
    }
}
