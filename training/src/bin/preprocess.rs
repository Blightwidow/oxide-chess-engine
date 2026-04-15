//! Preprocessor: reads .binpack files via bullet, applies filters, and writes
//! a flat binary file of ChessBoard structs (32 bytes each) that Python can mmap.
//!
//! Usage: cargo run --release --bin preprocess [output_path]
//!
//! Reads all .binpack files from data/ and writes filtered positions to the output file.
//! Default output: data/preprocessed.bin

use bullet::game::formats::sfbinpack::{
    chess::{piecetype::PieceType, r#move::MoveType},
    TrainingDataEntry,
};
use bullet::value::loader::{DataLoader, SfBinpackLoader};

use std::io::{BufWriter, Write};

fn main() {
    let output_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/preprocessed.bin".to_string());

    let mut data_file_paths: Vec<String> = std::fs::read_dir("data")
        .expect("data/ directory not found — run from training/ directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().to_string_lossy().to_string())
        .filter(|path| path.ends_with(".binpack"))
        .collect();
    data_file_paths.sort();

    if data_file_paths.is_empty() {
        eprintln!("No .binpack files found in data/");
        std::process::exit(1);
    }

    println!("Reading {} binpack files", data_file_paths.len());

    let data_refs: Vec<&str> = data_file_paths.iter().map(|s| s.as_str()).collect();

    fn filter(entry: &TrainingDataEntry) -> bool {
        entry.ply >= 16
            && !entry.pos.is_checked(entry.pos.side_to_move())
            && entry.score.unsigned_abs() <= 10000
            && entry.mv.mtype() == MoveType::Normal
            && entry.pos.piece_at(entry.mv.to()).piece_type() == PieceType::None
    }

    let loader = SfBinpackLoader::new_concat_multiple(&data_refs, 1024, 1, filter);

    let file = std::fs::File::create(&output_path).expect("Failed to create output file");
    let mut writer = BufWriter::with_capacity(64 * 1024 * 1024, file);
    let mut total_positions = 0u64;

    // map_chunks iterates over variable-size chunks from loader
    loader.map_chunks(0, |batch| {
        for board in batch {
            let bytes: &[u8] =
                unsafe { std::slice::from_raw_parts(board as *const _ as *const u8, 32) };
            writer.write_all(bytes).expect("Write failed");
        }
        total_positions += batch.len() as u64;
        if total_positions % 10_000_000 < 65536 {
            println!("  {} million positions written", total_positions / 1_000_000);
        }
        false // don't stop
    });

    writer.flush().expect("Flush failed");
    println!(
        "Done: {} positions written to {} ({:.1} GB)",
        total_positions,
        output_path,
        (total_positions * 32) as f64 / 1e9
    );
}
