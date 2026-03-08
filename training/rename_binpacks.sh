#!/bin/bash
# Renames all .binpack files in training/data/ to random UUIDs.
# Safe to run multiple times.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DATA_DIR="$SCRIPT_DIR/data"

if [ ! -d "$DATA_DIR" ]; then
  echo "No data directory found at $DATA_DIR"
  exit 1
fi

for file in "$DATA_DIR"/*.binpack; do
  [ -e "$file" ] || continue
  uuid=$(uuidgen | tr '[:upper:]' '[:lower:]')
  new_file="$DATA_DIR/${uuid}.binpack"
  echo "$(basename "$file") -> $(basename "$new_file")"
  mv "$file" "$new_file"
done
