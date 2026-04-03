#!/usr/bin/env bash
# Self-play data generation pipeline.
# Usage: ./scripts/generate_data.sh [depth] [num_games] [output_name]
#   depth:       search depth per move (default: 8)
#   num_games:   number of games to play (default: 10000)
#   output_name: base name for output files (default: selfplay)

set -euo pipefail

DEPTH="${1:-8}"
NUM_GAMES="${2:-10000}"
OUTPUT_NAME="${3:-selfplay}"

ENGINE="./target/release/oxid"
DATA_DIR="data"
TXT_FILE="${DATA_DIR}/${OUTPUT_NAME}.txt"
BINPACK_FILE="${DATA_DIR}/${OUTPUT_NAME}.binpack"
CONVERTER="./tools/plain2binpack"

echo "=== Self-play data generation ==="
echo "Depth: ${DEPTH}, Games: ${NUM_GAMES}, Output: ${OUTPUT_NAME}"

# Build engine
echo "Building engine (release)..."
cargo build -r

# Build converter if needed
if [ ! -f "${CONVERTER}" ]; then
    echo "Building plain2binpack converter..."
    clang++ -O2 -std=c++20 -o "${CONVERTER}" tools/plain2binpack.cpp
fi

# Create data directory
mkdir -p "${DATA_DIR}"

# Run datagen
echo "Running datagen..."
printf "datagen %d %d %s\n" "${DEPTH}" "${NUM_GAMES}" "${TXT_FILE}" | "${ENGINE}" 2>&1

# Convert to binpack
echo "Converting to binpack..."
"${CONVERTER}" "${TXT_FILE}" "${BINPACK_FILE}"

# Stats
TXT_SIZE=$(du -h "${TXT_FILE}" | cut -f1)
BINPACK_SIZE=$(du -h "${BINPACK_FILE}" | cut -f1)
echo ""
echo "=== Done ==="
echo "Plain text: ${TXT_FILE} (${TXT_SIZE})"
echo "Binpack:    ${BINPACK_FILE} (${BINPACK_SIZE})"
