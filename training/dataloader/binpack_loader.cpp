/*
 * Binpack streaming data loader for Oxide NNUE training.
 *
 * Reads Stockfish .binpack files directly, applies the same filters as the old
 * Rust preprocessor, extracts NNUE features, and returns batches via a C ABI.
 *
 * Feature encoding matches training/pytorch/data.py exactly:
 *   index = bucket * 768 + color_relative * 384 + piece_type * 64 + square
 * where:
 *   - bucket   = king_rank (0-7), with files e-h horizontally mirrored
 *   - color_relative = 0 for STM piece, 1 for NTM piece (STM perspective)
 *                      reversed (1/0) for NTM perspective
 *   - square is XOR'd with the mirror value (7 or 0) for horizontal flip
 *   - NTM perspective additionally flips squares vertically (XOR 56)
 */

#include "nnue_training_data_formats.h"
#include "nnue_training_data_stream.h"

#include <algorithm>
#include <cassert>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <functional>
#include <ios>
#include <memory>
#include <optional>
#include <random>
#include <string>
#include <vector>

namespace fs = std::filesystem;

extern "C" {

struct SparseBatch {
    int64_t* stm_indices;
    int64_t* ntm_indices;
    int64_t* stm_offsets;
    int64_t* ntm_offsets;
    float*   targets;
    int      batch_size;
    int      total_features;
};

}  // extern "C"

// ---------------------------------------------------------------------------
// Filtering (mirrors preprocess.rs)
// ---------------------------------------------------------------------------

static bool passes_filter(const binpack::TrainingDataEntry& entry) {
    if (entry.ply < 16) return false;
    if (entry.isInCheck()) return false;
    if (std::abs(entry.score) > 10000) return false;
    if (entry.move.type != chess::MoveType::Normal) return false;
    if (entry.pos.pieceAt(entry.move.to) != chess::Piece::none()) return false;
    return true;
}

// ---------------------------------------------------------------------------
// Feature extraction
// ---------------------------------------------------------------------------

// Extract STM and NTM feature indices for one position.
// Appends to stm_out and ntm_out (which must be pre-reserved).
static void extract_features(
    const binpack::TrainingDataEntry& entry,
    std::vector<int64_t>& stm_out,
    std::vector<int64_t>& ntm_out
) {
    const auto& pos = entry.pos;
    const chess::Color stm = pos.sideToMove();
    const bool stm_is_black = (stm == chess::Color::Black);

    // STM king bucket: flip vertically for black perspective, then horizontal mirror
    int stm_ksq_idx = static_cast<int>(pos.kingSquare(stm));
    if (stm_is_black) stm_ksq_idx ^= 56;
    const bool stm_needs_mirror = (stm_ksq_idx & 7) > 3;
    if (stm_needs_mirror) stm_ksq_idx ^= 7;
    const int stm_bucket_offset = (stm_ksq_idx >> 3) * 768;
    const int stm_mirror = stm_needs_mirror ? 7 : 0;

    // NTM king bucket: flip vertically for black perspective, then horizontal mirror
    int ntm_ksq_idx = static_cast<int>(pos.kingSquare(!stm));
    if (!stm_is_black) ntm_ksq_idx ^= 56;  // NTM is black when STM is white
    const bool ntm_needs_mirror = (ntm_ksq_idx & 7) > 3;
    if (ntm_needs_mirror) ntm_ksq_idx ^= 7;
    const int ntm_bucket_offset = (ntm_ksq_idx >> 3) * 768;
    const int ntm_mirror = ntm_needs_mirror ? 7 : 0;

    // Iterate all occupied squares
    chess::Bitboard all_pieces = pos.piecesBB();
    while (all_pieces.any()) {
        const chess::Square sq = all_pieces.first();
        all_pieces.popFirst();

        const chess::Piece piece = pos.pieceAt(sq);
        const int sq_idx    = static_cast<int>(sq);
        const int type      = static_cast<int>(chess::ordinal(piece.type()));
        // color_relative: 0 = our piece (STM), 1 = their piece (NTM)
        const int color_rel = (piece.color() == stm) ? 0 : 1;

        // STM perspective: our pieces at offset 0, their pieces at 384
        // Flip square vertically for black perspective; horizontal mirror if king on files e-h
        const int stm_sq = stm_is_black ? (sq_idx ^ 56) : sq_idx;
        const int stm_base = color_rel * 384 + type * 64 + stm_sq;
        stm_out.push_back(static_cast<int64_t>(stm_bucket_offset + (stm_base ^ stm_mirror)));

        // NTM perspective: their pieces at offset 0, our pieces at 384
        // Flip square vertically for black perspective (NTM is black when STM is white)
        const int ntm_sq = stm_is_black ? sq_idx : (sq_idx ^ 56);
        const int ntm_base = (1 - color_rel) * 384 + type * 64 + ntm_sq;
        ntm_out.push_back(static_cast<int64_t>(ntm_bucket_offset + (ntm_base ^ ntm_mirror)));
    }
}

// ---------------------------------------------------------------------------
// Loader struct
// ---------------------------------------------------------------------------

static constexpr int SHUFFLE_BUFFER_POSITIONS = 1'000'000;

struct BinpackLoader {
    std::vector<std::string> file_paths;
    size_t                   current_file_idx = 0;
    std::unique_ptr<binpack::CompressedTrainingDataEntryReader> reader;

    std::vector<binpack::TrainingDataEntry> shuffle_buffer;
    size_t buffer_pos = 0;

    int   batch_size;
    float wdl_blend;
    float eval_scale;

    std::mt19937 rng;

    BinpackLoader(
        const char* data_dir,
        int   batch_size,
        float wdl_blend,
        float eval_scale
    ) : batch_size(batch_size), wdl_blend(wdl_blend), eval_scale(eval_scale),
        rng(std::random_device{}())
    {
        for (const auto& dir_entry : fs::directory_iterator(data_dir)) {
            const auto& path = dir_entry.path();
            if (path.extension() == ".binpack") {
                file_paths.push_back(path.string());
            }
        }
        std::sort(file_paths.begin(), file_paths.end());
        std::shuffle(file_paths.begin(), file_paths.end(), rng);

        if (file_paths.empty()) {
            return;
        }

        open_file(0);
        refill_buffer();
    }

    void open_file(size_t idx) {
        current_file_idx = idx;
        reader = std::make_unique<binpack::CompressedTrainingDataEntryReader>(
            file_paths[idx], std::ios::in | std::ios::binary
        );
    }

    // Advance to the next file, cycling back to the start when all are done.
    void advance_file() {
        const size_t next = (current_file_idx + 1) % file_paths.size();
        if (next == 0) {
            // All files exhausted: reshuffle order before restarting
            std::shuffle(file_paths.begin(), file_paths.end(), rng);
        }
        open_file(next);
    }

    // Pull the next filtered entry from the current stream, cycling files as needed.
    std::optional<binpack::TrainingDataEntry> next_filtered() {
        // Guard against infinite loop on empty/all-filtered data
        size_t attempts = 0;
        while (true) {
            if (reader && reader->hasNext()) {
                auto entry = reader->next();
                if (passes_filter(entry)) {
                    return entry;
                }
                attempts = 0;  // reset on progress
                continue;
            }
            // Current file exhausted
            if (file_paths.empty()) return std::nullopt;
            advance_file();
            ++attempts;
            if (attempts > file_paths.size() * 2) {
                // All files are empty or filtered — avoid infinite loop
                return std::nullopt;
            }
        }
    }

    void refill_buffer() {
        shuffle_buffer.clear();
        shuffle_buffer.reserve(SHUFFLE_BUFFER_POSITIONS);

        while (static_cast<int>(shuffle_buffer.size()) < SHUFFLE_BUFFER_POSITIONS) {
            auto entry = next_filtered();
            if (!entry.has_value()) break;
            shuffle_buffer.push_back(std::move(*entry));
        }

        std::shuffle(shuffle_buffer.begin(), shuffle_buffer.end(), rng);
        buffer_pos = 0;
    }

    // Build a SparseBatch from the next batch_size positions in the shuffle buffer.
    SparseBatch* next_batch() {
        if (file_paths.empty()) return nullptr;

        // Refill if we can't fill a complete batch
        if (buffer_pos + static_cast<size_t>(batch_size) > shuffle_buffer.size()) {
            refill_buffer();
        }
        if (shuffle_buffer.empty()) return nullptr;

        const size_t actual_batch = std::min(
            static_cast<size_t>(batch_size),
            shuffle_buffer.size() - buffer_pos
        );

        // Per-position feature vectors
        std::vector<std::vector<int64_t>> stm_feats(actual_batch);
        std::vector<std::vector<int64_t>> ntm_feats(actual_batch);

        for (size_t i = 0; i < actual_batch; ++i) {
            const auto& entry = shuffle_buffer[buffer_pos + i];
            stm_feats[i].reserve(32);
            ntm_feats[i].reserve(32);
            extract_features(entry, stm_feats[i], ntm_feats[i]);
        }

        // Count total features and compute offsets
        size_t total_features = 0;
        for (size_t i = 0; i < actual_batch; ++i) {
            total_features += stm_feats[i].size();
        }

        // Allocate batch
        SparseBatch* batch = new SparseBatch();
        batch->batch_size     = static_cast<int>(actual_batch);
        batch->total_features = static_cast<int>(total_features);
        batch->stm_indices = new int64_t[total_features];
        batch->ntm_indices = new int64_t[total_features];
        batch->stm_offsets = new int64_t[actual_batch];
        batch->ntm_offsets = new int64_t[actual_batch];
        batch->targets     = new float[actual_batch];

        // Fill contiguous feature arrays and compute targets
        size_t feature_cursor = 0;
        for (size_t i = 0; i < actual_batch; ++i) {
            const auto& entry = shuffle_buffer[buffer_pos + i];

            // Offsets (same for STM and NTM — same piece count)
            batch->stm_offsets[i] = static_cast<int64_t>(feature_cursor);
            batch->ntm_offsets[i] = static_cast<int64_t>(feature_cursor);

            // Copy feature indices
            const size_t n = stm_feats[i].size();
            std::memcpy(batch->stm_indices + feature_cursor, stm_feats[i].data(), n * sizeof(int64_t));
            std::memcpy(batch->ntm_indices + feature_cursor, ntm_feats[i].data(), n * sizeof(int64_t));
            feature_cursor += n;

            // Target: wdl_blend * outcome + (1 - wdl_blend) * sigmoid(score / eval_scale)
            // entry.result is -1/0/+1 from STM's perspective
            const float outcome = static_cast<float>(entry.result + 1) / 2.0f;
            const float sigmoid_score = 1.0f / (1.0f + std::exp(-static_cast<float>(entry.score) / eval_scale));
            batch->targets[i] = wdl_blend * outcome + (1.0f - wdl_blend) * sigmoid_score;
        }

        buffer_pos += actual_batch;
        return batch;
    }
};

// ---------------------------------------------------------------------------
// C ABI
// ---------------------------------------------------------------------------

extern "C" {

void* binpack_loader_create(
    const char* data_dir,
    int   batch_size,
    float wdl_blend,
    float eval_scale
) {
    try {
        return new BinpackLoader(data_dir, batch_size, wdl_blend, eval_scale);
    } catch (const std::exception& error) {
        std::fprintf(stderr, "binpack_loader_create error: %s\n", error.what());
        return nullptr;
    }
}

void binpack_loader_destroy(void* loader) {
    delete static_cast<BinpackLoader*>(loader);
}

SparseBatch* binpack_loader_next_batch(void* loader) {
    return static_cast<BinpackLoader*>(loader)->next_batch();
}

void binpack_batch_free(SparseBatch* batch) {
    if (!batch) return;
    delete[] batch->stm_indices;
    delete[] batch->ntm_indices;
    delete[] batch->stm_offsets;
    delete[] batch->ntm_offsets;
    delete[] batch->targets;
    delete batch;
}

}  // extern "C"
