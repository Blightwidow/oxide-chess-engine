#!/usr/bin/env bash
# Self-play data generation pipeline.
# Usage: ./scripts/generate_data.sh [depth] [num_games] [output_name] [parallel]
#   depth:       search depth per move (default: 8)
#   num_games:   total number of games to play (default: 10000)
#   output_name: base name for output files (default: selfplay)
#   parallel:    number of engine processes to run in parallel (default: 1)

set -euo pipefail

DEPTH="${1:-8}"
NUM_GAMES="${2:-10000}"
OUTPUT_NAME="${3:-selfplay}"
PARALLEL="${4:-1}"

ENGINE="./target/release/oxid"
DATA_DIR="data"
CONVERTER="./tools/plain2binpack"

echo "=== Self-play data generation ==="
echo "Depth: ${DEPTH}, Games: ${NUM_GAMES}, Output: ${OUTPUT_NAME}, Workers: ${PARALLEL}"

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

if [ "${PARALLEL}" -le 1 ]; then
    # Single worker — same as before
    TXT_FILE="${DATA_DIR}/${OUTPUT_NAME}.txt"
    BINPACK_FILE="${DATA_DIR}/${OUTPUT_NAME}.binpack"

    echo "Running datagen..."
    printf "datagen %d %d %s\nquit\n" "${DEPTH}" "${NUM_GAMES}" "${TXT_FILE}" | "${ENGINE}" 2>&1

    echo "Converting to binpack..."
    "${CONVERTER}" "${TXT_FILE}" "${BINPACK_FILE}"

    TXT_SIZE=$(du -h "${TXT_FILE}" | cut -f1)
    BINPACK_SIZE=$(du -h "${BINPACK_FILE}" | cut -f1)
    echo ""
    echo "=== Done ==="
    echo "Plain text: ${TXT_FILE} (${TXT_SIZE})"
    echo "Binpack:    ${BINPACK_FILE} (${BINPACK_SIZE})"
else
    # Split games across workers
    GAMES_PER_WORKER=$(( (NUM_GAMES + PARALLEL - 1) / PARALLEL ))
    PIDS=()

    echo "Spawning ${PARALLEL} workers (${GAMES_PER_WORKER} games each)..."

    for WORKER_INDEX in $(seq 1 "${PARALLEL}"); do
        WORKER_TXT="${DATA_DIR}/${OUTPUT_NAME}_w${WORKER_INDEX}.txt"
        (
            printf "datagen %d %d %s\nquit\n" "${DEPTH}" "${GAMES_PER_WORKER}" "${WORKER_TXT}" \
                | "${ENGINE}" 2>&1 \
                | sed "s/^/[worker ${WORKER_INDEX}] /"
        ) &
        PIDS+=($!)
    done

    # Wait for all workers
    FAILED=0
    for PID in "${PIDS[@]}"; do
        if ! wait "${PID}"; then
            FAILED=$((FAILED + 1))
        fi
    done

    if [ "${FAILED}" -gt 0 ]; then
        echo "ERROR: ${FAILED} worker(s) failed"
        exit 1
    fi

    echo "All workers finished. Converting to binpack..."

    # Convert each worker's output to binpack
    for WORKER_INDEX in $(seq 1 "${PARALLEL}"); do
        WORKER_TXT="${DATA_DIR}/${OUTPUT_NAME}_w${WORKER_INDEX}.txt"
        WORKER_BINPACK="${DATA_DIR}/${OUTPUT_NAME}_w${WORKER_INDEX}.binpack"
        "${CONVERTER}" "${WORKER_TXT}" "${WORKER_BINPACK}"
    done

    # Stats
    echo ""
    echo "=== Done ==="
    TOTAL_POSITIONS=0
    for WORKER_INDEX in $(seq 1 "${PARALLEL}"); do
        WORKER_TXT="${DATA_DIR}/${OUTPUT_NAME}_w${WORKER_INDEX}.txt"
        WORKER_BINPACK="${DATA_DIR}/${OUTPUT_NAME}_w${WORKER_INDEX}.binpack"
        POSITIONS=$(wc -l < "${WORKER_TXT}" | tr -d ' ')
        TOTAL_POSITIONS=$((TOTAL_POSITIONS + POSITIONS))
        BINPACK_SIZE=$(du -h "${WORKER_BINPACK}" | cut -f1)
        echo "  Worker ${WORKER_INDEX}: ${POSITIONS} positions, binpack ${BINPACK_SIZE}"
    done
    echo "Total: ${TOTAL_POSITIONS} positions across ${PARALLEL} binpack files"
fi
