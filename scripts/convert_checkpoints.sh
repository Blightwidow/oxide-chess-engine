#!/usr/bin/env bash
# Convert all bullet training checkpoints into .nnue files with random hash names.
# Expects to be run from the repo root.
# Skips checkpoints that have already been converted (tracked via .converted marker).

set -euo pipefail

CHECKPOINTS_DIR="training/checkpoints"
NETS_DIR="nets"
CONVERT_BIN="training/target/release/convert"

if [ ! -d "$CHECKPOINTS_DIR" ]; then
    echo "Error: $CHECKPOINTS_DIR not found. Run from the repo root."
    exit 1
fi

# Build the convert binary if needed
if [ ! -f "$CONVERT_BIN" ]; then
    echo "Building convert binary..."
    (cd training && cargo build --release --bin convert)
fi

mkdir -p "$NETS_DIR"

converted=0
skipped=0

for quantised in "$CHECKPOINTS_DIR"/*/quantised.bin; do
    [ -f "$quantised" ] || continue

    checkpoint_dir="$(dirname "$quantised")"
    marker="$checkpoint_dir/.converted"

    if [ -f "$marker" ]; then
        skipped=$((skipped + 1))
        continue
    fi

    tmp_output="$NETS_DIR/_tmp_convert.nnue"

    echo "Converting $(basename "$checkpoint_dir")..."
    "$CONVERT_BIN" "$quantised" "$tmp_output"

    hash=$(shasum -a 256 "$tmp_output" | cut -c1-12)
    output="$NETS_DIR/nn-${hash}.nnue"
    mv "$tmp_output" "$output"
    echo "  -> $output"

    # Mark as converted so we don't redo it
    echo "$output" > "$marker"
    converted=$((converted + 1))
done

echo "Done: $converted converted, $skipped already done."
