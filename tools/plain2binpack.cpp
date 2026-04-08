// Convert semicolon-delimited plain text training data to Stockfish binpack format.
// Input format: FEN;UCI_MOVE;SCORE;PLY;RESULT
// Build: clang++ -O2 -std=c++17 -o plain2binpack plain2binpack.cpp
//        (header-only: nnue_training_data_formats.h must be findable via -I)

#include <cstdlib>
#include <fstream>
#include <iostream>
#include <sstream>
#include <string>

#include "../training/dataloader/nnue_training_data_formats.h"

using namespace binpack;

int main(int argc, char* argv[]) {
    if (argc < 3) {
        std::cerr << "Usage: plain2binpack <input.txt> <output.binpack>\n";
        return 1;
    }

    const std::string input_path = argv[1];
    const std::string output_path = argv[2];

    std::ifstream input_file(input_path);
    if (!input_file) {
        std::cerr << "Error: cannot open " << input_path << "\n";
        return 1;
    }

    CompressedTrainingDataEntryWriter writer(output_path, std::ios_base::out);

    std::string line;
    std::size_t total_entries = 0;
    std::size_t skipped_entries = 0;

    while (std::getline(input_file, line)) {
        if (line.empty()) continue;

        // Parse: FEN;MOVE;SCORE;PLY;RESULT
        std::istringstream stream(line);
        std::string fen_str, move_str, score_str, ply_str, result_str;

        if (!std::getline(stream, fen_str, ';') ||
            !std::getline(stream, move_str, ';') ||
            !std::getline(stream, score_str, ';') ||
            !std::getline(stream, ply_str, ';') ||
            !std::getline(stream, result_str, ';')) {
            // Try without trailing semicolon for result
            if (result_str.empty()) {
                std::istringstream retry(line);
                std::getline(retry, fen_str, ';');
                std::getline(retry, move_str, ';');
                std::getline(retry, score_str, ';');
                std::getline(retry, ply_str, ';');
                std::getline(retry, result_str);
            }
            if (result_str.empty()) {
                ++skipped_entries;
                continue;
            }
        }

        TrainingDataEntry entry{};
        entry.pos = chess::Position::fromFen(fen_str.c_str());
        entry.move = chess::uci::uciToMove(entry.pos, move_str);
        entry.score = static_cast<std::int16_t>(std::stoi(score_str));
        entry.ply = static_cast<std::uint16_t>(std::stoi(ply_str));
        entry.result = static_cast<std::int16_t>(std::stoi(result_str));

        writer.addTrainingDataEntry(entry);
        ++total_entries;

        if (total_entries % 1000000 == 0) {
            std::cout << "Converted " << total_entries << " entries...\n";
        }
    }

    std::cout << "Done: " << total_entries << " entries written to " << output_path;
    if (skipped_entries > 0) {
        std::cout << " (" << skipped_entries << " skipped)";
    }
    std::cout << "\n";

    return 0;
}
