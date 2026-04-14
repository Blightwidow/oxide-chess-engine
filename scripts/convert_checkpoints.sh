#!/usr/bin/env bash
# Convert all bullet training checkpoints into .nnue files with hash names.
# Expects to be run from the repo root.
# Skips checkpoints whose output net already exists in nets/ (hash of quantised.bin).

set -euo pipefail

CHECKPOINTS_DIR="training/checkpoints"
NETS_DIR="nets"

if [ ! -d "$CHECKPOINTS_DIR" ]; then
    echo "Error: $CHECKPOINTS_DIR not found. Run from the repo root."
    exit 1
fi

mkdir -p "$NETS_DIR"

# Build the convert binary if needed
if [ ! -f "training/target/release/convert" ]; then
    echo "Building convert binary..."
    (cd training && cargo build --release --bin convert)
fi

converted=0
skipped=0

for quantised_bin in "$CHECKPOINTS_DIR"/*/quantised.bin; do
    [ -f "$quantised_bin" ] || continue

    hash=$(shasum -a 256 "$quantised_bin" | cut -c1-12)
    output="$NETS_DIR/nn-${hash}.nnue"

    if [ -f "$output" ]; then
        skipped=$((skipped + 1))
        continue
    fi

    echo "Converting $(basename "$(dirname "$quantised_bin")")..."
    training/target/release/convert "$quantised_bin" "$output"
    echo "  -> $output"

    converted=$((converted + 1))
done

echo "Done: $converted converted, $skipped already done."
