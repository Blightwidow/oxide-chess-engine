#!/usr/bin/env bash
# Builds the browser package: oxid.wasm + JS glue in pkg/, plus the NNUE nets
# the page has to fetch at runtime (they are deliberately not embedded).
#
# Requires: rustup target add wasm32-unknown-unknown
#           cargo install wasm-bindgen-cli --version <matching Cargo.lock>
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

TARGET_DIR="target/wasm32-unknown-unknown/release"
OUTPUT_DIR="pkg"

# Keep the CLI and the wasm-bindgen crate on the same version — a mismatch fails
# at bindgen time with a confusing schema error.
CRATE_VERSION="$(sed -n '/^name = "wasm-bindgen"$/,/^version = /s/^version = "\(.*\)"/\1/p' Cargo.lock | head -1)"
CLI_VERSION="$(wasm-bindgen --version | awk '{print $2}')"
if [ "$CRATE_VERSION" != "$CLI_VERSION" ]; then
  echo "wasm-bindgen version mismatch: crate $CRATE_VERSION, CLI $CLI_VERSION" >&2
  echo "Fix with: cargo install wasm-bindgen-cli --version $CRATE_VERSION --locked" >&2
  exit 1
fi

echo "Building oxid for wasm32-unknown-unknown"
cargo build --release --target wasm32-unknown-unknown --no-default-features --features wasm

echo "Generating JS bindings into $OUTPUT_DIR/"
rm -rf "$OUTPUT_DIR"
wasm-bindgen "$TARGET_DIR/oxid.wasm" --out-dir "$OUTPUT_DIR" --target web --no-typescript

# The wasm blob carries no net, so ship the nets alongside it.
mkdir -p "$OUTPUT_DIR/nets"
cp nets/*.nnue "$OUTPUT_DIR/nets/"

echo "Done. Contents of $OUTPUT_DIR:"
ls -la "$OUTPUT_DIR" "$OUTPUT_DIR/nets"
