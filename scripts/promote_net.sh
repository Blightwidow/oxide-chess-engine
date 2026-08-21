#!/usr/bin/env bash
# Promote a .nnue net file as the active embedded net.
#
# Usage: scripts/promote_net.sh path/to/file.nnue
#
# This script:
#   1. Verifies the file exists and has a valid OXNN header
#   2. Computes SHA256, renames to nn-{hash12}.nnue if needed
#   3. Updates DEFAULT_EVAL_FILE and include_bytes! path in src/lib.rs
#   4. Updates .gitignore: swaps the exception line
#   5. git rm the old net, git add the new one
#   6. Prints a summary (user commits manually)

set -euo pipefail

if [ $# -ne 1 ]; then
    echo "Usage: $0 <path-to-nnue-file>"
    exit 1
fi

INPUT="$1"

if [ ! -f "$INPUT" ]; then
    echo "Error: file not found: $INPUT"
    exit 1
fi

# Verify OXNN magic header
MAGIC=$(head -c 4 "$INPUT" | xxd -p)
if [ "$MAGIC" != "4f584e4e" ]; then
    echo "Error: invalid NNUE file (expected OXNN magic header, got $MAGIC)"
    exit 1
fi

# Keep the existing name if the net already lives in nets/ under an nn-<hash> name.
# convert_checkpoints.sh names by the hash of quantised.bin, which does not match the hash
# of the wrapped .nnue file — recomputing here would duplicate the net under a second name
# and orphan its .sprt.log.
if [[ "$INPUT" == nets/nn-*.nnue ]]; then
    NEW_NAME=$(basename "$INPUT")
    NEW_PATH="$INPUT"
else
    HASH=$(shasum -a 256 "$INPUT" | cut -c1-12)
    NEW_NAME="nn-${HASH}.nnue"
    NEW_PATH="nets/${NEW_NAME}"
fi

# Find current promoted net from src/lib.rs
OLD_NAME=$(grep 'pub const DEFAULT_EVAL_FILE' src/lib.rs | sed 's/.*"\(.*\)".*/\1/')

if [ "$NEW_NAME" = "$OLD_NAME" ]; then
    echo "Net is already the active net: $NEW_NAME"
    exit 0
fi

# Move/copy the file into nets/
if [ "$INPUT" != "$NEW_PATH" ]; then
    cp "$INPUT" "$NEW_PATH"
fi

# Update src/lib.rs
sed -i '' "s|$OLD_NAME|$NEW_NAME|g" src/lib.rs

# Update .gitignore: swap the exception line
sed -i '' "s|!nets/$OLD_NAME|!nets/$NEW_NAME|" .gitignore

# Git operations
OLD_PATH="nets/${OLD_NAME}"
if [ -f "$OLD_PATH" ] && git ls-files --error-unmatch "$OLD_PATH" >/dev/null 2>&1; then
    # -f is needed when the outgoing net was staged but never committed (promoted twice
    # between commits). Its content is reproducible from the training checkpoint.
    git rm --quiet "$OLD_PATH" 2>/dev/null || {
        echo "Note: $OLD_NAME was staged but never committed — forcing removal."
        git rm --quiet -f "$OLD_PATH"
    }
fi
git add "$NEW_PATH"
git add src/lib.rs .gitignore

echo ""
echo "=== Net promoted ==="
echo "Old: $OLD_NAME"
echo "New: $NEW_NAME"
echo ""
echo "Review and commit when ready."
