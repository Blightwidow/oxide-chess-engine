#!/usr/bin/env bash
# Promote a .nnue net file as the active embedded net.
#
# Usage: scripts/promote_net.sh path/to/file.nnue
#
# This script:
#   1. Verifies the file exists and has a valid OXNN header
#   2. Computes SHA256, renames to nn-{hash12}.nnue if needed
#   3. Updates DEFAULT_EVAL_FILE and include_bytes! path in src/main.rs
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

# Compute SHA256-based name
HASH=$(shasum -a 256 "$INPUT" | cut -c1-12)
NEW_NAME="nn-${HASH}.nnue"
NEW_PATH="nets/${NEW_NAME}"

# Find current promoted net from src/main.rs
OLD_NAME=$(grep 'pub const DEFAULT_EVAL_FILE' src/main.rs | sed 's/.*"\(.*\)".*/\1/')

if [ "$NEW_NAME" = "$OLD_NAME" ]; then
    echo "Net is already the active net: $NEW_NAME"
    exit 0
fi

# Move/copy the file into nets/
if [ "$INPUT" != "$NEW_PATH" ]; then
    cp "$INPUT" "$NEW_PATH"
fi

# Update src/main.rs
sed -i '' "s|$OLD_NAME|$NEW_NAME|g" src/main.rs

# Update .gitignore: swap the exception line
sed -i '' "s|!nets/$OLD_NAME|!nets/$NEW_NAME|" .gitignore

# Git operations
OLD_PATH="nets/${OLD_NAME}"
if [ -f "$OLD_PATH" ] && git ls-files --error-unmatch "$OLD_PATH" >/dev/null 2>&1; then
    git rm "$OLD_PATH"
fi
git add "$NEW_PATH"
git add src/main.rs .gitignore

echo ""
echo "=== Net promoted ==="
echo "Old: $OLD_NAME"
echo "New: $NEW_NAME"
echo ""
echo "Review and commit when ready."
