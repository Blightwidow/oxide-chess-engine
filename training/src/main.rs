use bullet::{
    game::{
        formats::sfbinpack::{
            chess::{piecetype::PieceType, r#move::MoveType},
            TrainingDataEntry,
        },
        inputs::ChessBucketsMirrored,
    },
    nn::optimiser::AdamW,
    trainer::{
        save::SavedFormat,
        schedule::{lr::StepLR, wdl::ConstantWDL, TrainingSchedule, TrainingSteps},
        settings::LocalSettings,
    },
    value::{loader::SfBinpackLoader, ValueTrainerBuilder},
};

use bullet::acyclib::graph::like::GraphLike;
use bullet::acyclib::trainer::dataloader::PreparedBatchDevice;
use bullet::value::loader::DataLoader;

const HIDDEN_SIZE: usize = 384;
const L1_SIZE: usize = 32;
const NUM_BUCKETS: usize = 8;
const QA: i16 = 255;
const QB: i16 = 64;
const THREADS: usize = 8;
const VAL_BATCHES: usize = 64;

struct TrainingArgs {
    resume_checkpoint: Option<String>,
    end_superbatch: usize,
}

fn parse_args() -> TrainingArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut resume_checkpoint = None;
    let mut end_superbatch = 100;
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--resume" => {
                index += 1;
                if index >= args.len() {
                    eprintln!("Error: --resume requires a checkpoint path");
                    std::process::exit(1);
                }
                resume_checkpoint = Some(args[index].clone());
            }
            "--end" => {
                index += 1;
                if index >= args.len() {
                    eprintln!("Error: --end requires a superbatch number");
                    std::process::exit(1);
                }
                end_superbatch = args[index].parse().unwrap_or_else(|_| {
                    eprintln!("Error: --end must be a number");
                    std::process::exit(1);
                });
            }
            "--help" | "-h" => {
                println!("Usage: train [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --resume <path>  Resume from checkpoint (e.g. checkpoints/oxid-100)");
                println!("  --end <N>        End superbatch (default: 100)");
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown argument: {other}");
                eprintln!("Run with --help for usage");
                std::process::exit(1);
            }
        }
        index += 1;
    }

    TrainingArgs {
        resume_checkpoint,
        end_superbatch,
    }
}

/// Extract the superbatch number from a checkpoint path like "checkpoints/oxid-60".
fn superbatch_from_checkpoint(path: &str) -> usize {
    let directory_name = std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    // Parse "oxid-60" -> 60
    directory_name
        .rsplit('-')
        .next()
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| {
            eprintln!("Error: cannot parse superbatch number from checkpoint path: {path}");
            eprintln!("Expected format: checkpoints/oxid-<N>");
            std::process::exit(1);
        })
}

fn delete_old_checkpoints(directory: &str) {
    let path = std::path::Path::new(directory);
    if !path.exists() {
        return;
    }
    let entries: Vec<_> = std::fs::read_dir(path)
        .ok()
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .collect()
        })
        .unwrap_or_default();
    if entries.is_empty() {
        return;
    }
    println!(
        "Found {} existing checkpoint(s) in {}/",
        entries.len(),
        directory
    );
    print!("Delete old checkpoints before starting? [y/N] ");
    use std::io::Write;
    std::io::stdout().flush().ok();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).ok();
    if answer.trim().eq_ignore_ascii_case("y") {
        std::fs::remove_dir_all(path).ok();
        std::fs::create_dir_all(path).ok();
        println!("Deleted old checkpoints.");
    }
}

fn main() {
    let args = parse_args();

    let start_superbatch = match &args.resume_checkpoint {
        Some(checkpoint_path) => {
            let superbatch = superbatch_from_checkpoint(checkpoint_path);
            println!("Resuming from {} (superbatch {})", checkpoint_path, superbatch);
            superbatch + 1
        }
        None => {
            delete_old_checkpoints("checkpoints");
            1
        }
    };

    if start_superbatch > args.end_superbatch {
        println!(
            "Already at superbatch {} (end: {}), nothing to do.",
            start_superbatch - 1,
            args.end_superbatch
        );
        return;
    }

    // Glob all binpack files from data/
    let mut data_file_paths: Vec<String> = std::fs::read_dir("data")
        .expect("data/ directory not found — run from training/ directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".binpack"))
        .collect();
    data_file_paths.sort();
    println!("Loading {} binpack files", data_file_paths.len());

    let data_refs: Vec<&str> = data_file_paths.iter().map(|s| s.as_str()).collect();

    // Check for validation data
    let val_file_paths: Vec<String> = std::fs::read_dir("data/validation")
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path().to_string_lossy().to_string())
                .filter(|p| p.ends_with(".binpack"))
                .collect()
        })
        .unwrap_or_default();
    let has_validation = !val_file_paths.is_empty();
    if has_validation {
        println!("Found {} validation binpack files", val_file_paths.len());
    } else {
        println!("No validation data found in data/validation/ — skipping validation loss");
    }

    // 8 king buckets by rank with horizontal mirroring (files e-h mapped to d-a)
    let buckets: [usize; 32] = {
        let mut b = [0usize; 32];
        for i in 0..32 {
            b[i] = i / 4; // rank = bucket
        }
        b
    };

    let mut trainer = ValueTrainerBuilder::default()
        .use_threads(THREADS)
        .dual_perspective()
        .optimiser(AdamW)
        .inputs(ChessBucketsMirrored::new(buckets))
        .save_format(&[
            // Feature transform: bullet column-major matches our [feature][hidden] layout
            SavedFormat::id("l0w").round().quantise::<i16>(QA),
            SavedFormat::id("l0b").round().quantise::<i16>(QA),
            // L1 weights (QB) and biases (QA*QB to match accumulated scale after /QA)
            SavedFormat::id("l1w").round().quantise::<i16>(QB),
            SavedFormat::id("l1b").round().quantise::<i16>(QA * QB),
            // L2 weights (QB) and bias (QA*QB)
            SavedFormat::id("l2w").round().quantise::<i16>(QB),
            SavedFormat::id("l2b").round().quantise::<i16>(QA * QB),
        ])
        .loss_fn(|output, target| output.sigmoid().squared_error(target))
        .build(|builder, stm, ntm| {
            let l0 = builder.new_affine("l0", 768 * NUM_BUCKETS, HIDDEN_SIZE);
            let l1 = builder.new_affine("l1", HIDDEN_SIZE * 2, L1_SIZE);
            let l2 = builder.new_affine("l2", L1_SIZE, 1);

            let stm_out = l0.forward(stm).screlu();
            let ntm_out = l0.forward(ntm).screlu();
            let hidden = stm_out.concat(ntm_out);
            let l1_out = l1.forward(hidden).screlu();
            l2.forward(l1_out)
        });

    // Load checkpoint if resuming
    if let Some(checkpoint_path) = &args.resume_checkpoint {
        trainer.load_from_checkpoint(checkpoint_path);
        println!("Checkpoint loaded.");
    }

    fn filter(entry: &TrainingDataEntry) -> bool {
        entry.ply >= 16
            && !entry.pos.is_checked(entry.pos.side_to_move())
            && entry.score.unsigned_abs() <= 10000
            && entry.mv.mtype() == MoveType::Normal
            && entry.pos.piece_at(entry.mv.to()).piece_type() == PieceType::None
    }

    let loader = SfBinpackLoader::new_concat_multiple(&data_refs, 1024, 4, filter);

    let batch_size = 16384;
    let save_rate = 20;
    let end_superbatch = args.end_superbatch;

    let settings = LocalSettings {
        threads: THREADS,
        test_set: None,
        output_directory: "checkpoints",
        batch_queue_size: 32,
    };

    if !has_validation {
        // No validation data — run normally
        let schedule = TrainingSchedule {
            net_id: "oxid".to_string(),
            eval_scale: 400.0,
            steps: TrainingSteps {
                batch_size,
                batches_per_superbatch: 6104,
                start_superbatch,
                end_superbatch,
            },
            wdl_scheduler: ConstantWDL { value: 0.75 },
            lr_scheduler: StepLR {
                start: 0.001,
                gamma: 0.3,
                step: 15,
            },
            save_rate,
        };
        trainer.run(&schedule, &settings, &loader);
        return;
    }

    // Segmented training with validation loss
    let val_refs: Vec<&str> = val_file_paths.iter().map(|s| s.as_str()).collect();
    let val_loader = SfBinpackLoader::new_concat_multiple(&val_refs, 256, 4, filter);
    let blend = 0.75; // ConstantWDL value
    let scale = 400.0; // eval_scale

    let mut val_log = Vec::new();

    for segment_start in (start_superbatch..=end_superbatch).step_by(save_rate) {
        let segment_end = (segment_start + save_rate - 1).min(end_superbatch);

        let schedule = TrainingSchedule {
            net_id: "oxid".to_string(),
            eval_scale: scale,
            steps: TrainingSteps {
                batch_size,
                batches_per_superbatch: 6104,
                start_superbatch: segment_start,
                end_superbatch: segment_end,
            },
            wdl_scheduler: ConstantWDL { value: blend },
            lr_scheduler: StepLR {
                start: 0.001,
                gamma: 0.3,
                step: 15,
            },
            save_rate,
        };

        trainer.run(&schedule, &settings, &loader);

        // Compute validation loss via forward-only passes
        let mut val_batches: Vec<Vec<_>> = Vec::new();
        val_loader.map_batches(0, batch_size, |batch| {
            val_batches.push(batch.to_vec());
            val_batches.len() >= VAL_BATCHES
        });

        let mut total_loss = 0.0;
        let num_batches = val_batches.len();

        for batch in &val_batches {
            let host = trainer.state.prepare(batch, THREADS, blend, scale);

            let graph = trainer.optimiser.graph.primary_mut();

            let mut dev = PreparedBatchDevice::new(graph.devices(), &host).unwrap();
            dev.load_into_graph(graph).unwrap();
            graph.synchronise().unwrap();
            let loss = graph.forward().unwrap();
            total_loss += loss / batch.len() as f32;
        }

        let avg_val_loss = if num_batches > 0 {
            total_loss / num_batches as f32
        } else {
            0.0
        };
        println!(
            "[Validation] Superbatch {} | val_loss = {:.5}",
            segment_end, avg_val_loss
        );

        val_log.push(format!("superbatch {} | val_loss = {:.5}", segment_end, avg_val_loss));

        // Write validation log to checkpoint directory
        let log_path = format!("{}/val_log.txt", settings.output_directory);
        std::fs::write(&log_path, val_log.join("\n") + "\n").ok();
    }
}
