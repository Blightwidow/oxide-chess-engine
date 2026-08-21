#!/usr/bin/env bash
# Build a Stash release for Elo-anchor matches (arm64 macOS).
#
# Stash's Makefile only knows x86 arch profiles, so on Apple Silicon we build
# the generic C sources but still opt into USE_POPCNT (which resolves to
# __builtin_popcountll). That define pulls in <immintrin.h>, which clang does
# not ship for arm64, so scripts/stash-shim/immintrin.h stands in for it.
# Stash guards _mm_prefetch behind USE_POPCNT too, so the shim provides that
# intrinsic via __builtin_prefetch.
set -euo pipefail

version="${1:?usage: build_stash.sh <tag> (e.g. v21.0)}"
script_dir="$(cd "$(dirname "$0")" && pwd)"
root_dir="$(dirname "$script_dir")"
engines_dir="$root_dir/engines"
src_dir="$engines_dir/stash-bot"
output="$engines_dir/stash-${version%%.*}"

mkdir -p "$engines_dir"
if [ ! -d "$src_dir" ]; then
    git clone --quiet https://github.com/mhouppin/stash-bot.git "$src_dir"
fi

git -C "$src_dir" checkout --quiet "$version"
make -C "$src_dir/src" clean >/dev/null 2>&1 || true
make -C "$src_dir/src" -j"$(sysctl -n hw.ncpu)" \
    CC=clang \
    EXT_OFLAGS="-DUSE_POPCNT -I$script_dir/stash-shim -Wno-error" \
    EXE="$output" >/dev/null

echo "built $output"
printf 'uci\nquit\n' | "$output" | head -3
