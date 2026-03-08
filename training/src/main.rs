use bullet::{
    game::{
        formats::sfbinpack::{
            TrainingDataEntry,
            chess::{r#move::MoveType, piecetype::PieceType},
        },
        inputs::Chess768,
    },
    nn::optimiser::AdamW,
    trainer::{
        save::SavedFormat,
        schedule::{TrainingSchedule, TrainingSteps, lr::StepLR, wdl::ConstantWDL},
        settings::{LocalSettings, TestDataset},
    },
    value::{ValueTrainerBuilder, loader::SfBinpackLoader},
};

const HIDDEN_SIZE: usize = 256;
const L1_SIZE: usize = 32;
const QA: i16 = 255;
const QB: i16 = 64;
const THREADS: usize = 8;

fn main() {
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

    let mut trainer = ValueTrainerBuilder::default()
        .use_threads(THREADS)
        .dual_perspective()
        .optimiser(AdamW)
        .inputs(Chess768)
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
            let l0 = builder.new_affine("l0", 768, HIDDEN_SIZE);
            let l1 = builder.new_affine("l1", HIDDEN_SIZE * 2, L1_SIZE);
            let l2 = builder.new_affine("l2", L1_SIZE, 1);

            let stm_out = l0.forward(stm).screlu();
            let ntm_out = l0.forward(ntm).screlu();
            let hidden = stm_out.concat(ntm_out);
            let l1_out = l1.forward(hidden).screlu();
            l2.forward(l1_out)
        });

    let schedule = TrainingSchedule {
        net_id: "oxide".to_string(),
        eval_scale: 400.0,
        steps: TrainingSteps {
            batch_size: 16384,
            batches_per_superbatch: 6104,
            start_superbatch: 1,
            end_superbatch: 40,
        },
        wdl_scheduler: ConstantWDL { value: 0.75 },
        lr_scheduler: StepLR { start: 0.001, gamma: 0.1, step: 18 },
        save_rate: 10,
    };

    let settings = LocalSettings {
        threads: THREADS,
        test_set: Some(TestDataset::at("data/test/test.binpack")),
        output_directory: "checkpoints",
        batch_queue_size: 32,
    };

    fn filter(entry: &TrainingDataEntry) -> bool {
        entry.ply >= 16
            && !entry.pos.is_checked(entry.pos.side_to_move())
            && entry.score.unsigned_abs() <= 10000
            && entry.mv.mtype() == MoveType::Normal
            && entry.pos.piece_at(entry.mv.to()).piece_type() == PieceType::None
    }

    let loader = SfBinpackLoader::new_concat_multiple(&data_refs, 1024, 4, filter);

    trainer.run(&schedule, &settings, &loader);
}
