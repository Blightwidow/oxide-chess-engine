#!/usr/bin/env bash
# Convert all PyTorch training checkpoints into .nnue files with hash names.
# Expects to be run from the repo root.
# Skips checkpoints whose output net already exists in nets/ (hash of model.pt).

set -euo pipefail

CHECKPOINTS_DIR="training/checkpoints"
NETS_DIR="nets"

if [ ! -d "$CHECKPOINTS_DIR" ]; then
    echo "Error: $CHECKPOINTS_DIR not found. Run from the repo root."
    exit 1
fi

mkdir -p "$NETS_DIR"

converted=0
skipped=0

for model_pt in "$CHECKPOINTS_DIR"/*/model.pt; do
    [ -f "$model_pt" ] || continue

    hash=$(shasum -a 256 "$model_pt" | cut -c1-12)
    output="$NETS_DIR/nn-${hash}.nnue"

    if [ -f "$output" ]; then
        skipped=$((skipped + 1))
        continue
    fi

    echo "Converting $(basename "$(dirname "$model_pt")")..."
    (cd training && uv run python export.py "../$model_pt" "../$output")
    echo "  -> $output"

    converted=$((converted + 1))
done

echo "Done: $converted converted, $skipped already done."
