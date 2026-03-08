//! Reads bullet's quantised.bin and wraps it with our OXNN header.
use std::{env, fs};

const FEATURE_SIZE: u32 = 768;
const HIDDEN_SIZE: u32 = 256;
const L1_SIZE: u32 = 32;
const NET_VERSION: u32 = 1;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: convert <quantised.bin> <output.nnue>");
        std::process::exit(1);
    }
    let input = &args[1];
    let output = &args[2];

    let weights = fs::read(input).expect("Failed to read input");

    let mut out = Vec::new();
    // OXNN header (20 bytes)
    out.extend_from_slice(b"OXNN");
    out.extend_from_slice(&NET_VERSION.to_le_bytes());
    out.extend_from_slice(&FEATURE_SIZE.to_le_bytes());
    out.extend_from_slice(&HIDDEN_SIZE.to_le_bytes());
    out.extend_from_slice(&L1_SIZE.to_le_bytes());
    // Weights (already in correct order from SavedFormat)
    out.extend_from_slice(&weights);

    fs::write(output, &out).expect("Failed to write output");
    println!("Wrote {} bytes to {}", out.len(), output);
}
