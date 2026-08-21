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

const HIDDEN_SIZE: usize = 384;
const L1_SIZE: usize = 32;
const NUM_BUCKETS: usize = 8;
const QA: i16 = 255;
const QB: i16 = 64;
const THREADS: usize = 8;

/// Default run length. The dataset in `data/` is ~10.6B filtered positions, i.e. ~106
/// superbatches per epoch, so this is ~7.5 epochs — matching bullet's reference config.
const DEFAULT_END_SUPERBATCH: usize = 800;
/// Superbatches between checkpoints. The final superbatch is always saved regardless.
const SAVE_RATE: usize = 100;
/// Number of learning rate drops to fit into a run, however long it is.
const LR_DROPS: usize = 6;
const LR_START: f32 = 0.001;
const LR_GAMMA: f32 = 0.3;

struct TrainingArgs {
    resume_checkpoint: Option<String>,
    end_superbatch: usize,
}

fn parse_args() -> TrainingArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut resume_checkpoint = None;
    let mut end_superbatch = DEFAULT_END_SUPERBATCH;
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
                println!("  --end <N>        End superbatch (default: {DEFAULT_END_SUPERBATCH})");
                println!();
                println!("The learning rate drops by a factor of {LR_GAMMA} every --end/{LR_DROPS}");
                println!("superbatches, so the schedule always fits {LR_DROPS} drops into the run.");
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
        .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).collect())
        .unwrap_or_default();
    if entries.is_empty() {
        return;
    }
    println!("Found {} existing checkpoint(s) in {}/", entries.len(), directory);
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
    if !val_file_paths.is_empty() {
        println!(
            "Found {} validation binpack files in data/validation/, but validation loss is \
             unsupported: upstream bullet has not reimplemented it since the crate split. \
             Training will proceed without it.",
            val_file_paths.len()
        );
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
    let save_rate = SAVE_RATE;
    let end_superbatch = args.end_superbatch;

    // Scale the LR schedule to the run length: a fixed step would either waste most of a long
    // run at a dead learning rate, or never decay at all on a short one.
    let lr_step = (end_superbatch / LR_DROPS).max(1);
    println!(
        "LR schedule: {LR_START}, x{LR_GAMMA} every {lr_step} superbatches ({} drops by superbatch {end_superbatch})",
        end_superbatch / lr_step
    );

    let settings = LocalSettings {
        threads: THREADS,
        test_set: None,
        output_directory: "checkpoints",
        batch_queue_size: 32,
    };

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
            start: LR_START,
            gamma: LR_GAMMA,
            step: lr_step,
        },
        save_rate,
    };

    trainer.run(&schedule, &settings, &loader);
}
