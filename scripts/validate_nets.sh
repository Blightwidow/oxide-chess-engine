#!/usr/bin/env bash
# Validate .nnue files in nets/ against a base engine.
# Skips the committed (embedded) net since it may be from a different architecture version.
#
# Usage: ./scripts/validate_nets.sh [base_engine]
#   base_engine: path to baseline engine binary for SPRT (default: ./base/release/oxid)
#
# Steps per net:
#   1. Sanity eval (startpos)
#   2. Quick bench (depth 11, 5 positions)
#   3. ERET tactical accuracy (1M nodes)
#   4. SPRT vs base engine (if base_engine exists)

set -euo pipefail

BASE_ENGINE="${1:-./base/release/oxid}"
NETS_DIR="nets"
ENGINE="./target/release/oxid"
BENCH_DEPTH=11
BENCH_POSITIONS=5
SPRT_ROUNDS=15000
SPRT_TC="8+0.08"
SPRT_CONCURRENCY=6
FASTCHESS="./bin/fastchess"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
RESET='\033[0m'

pass() { printf "${GREEN}PASS${RESET} %s\n" "$1"; }
fail() { printf "${RED}FAIL${RESET} %s\n" "$1"; }
skip() { printf "${YELLOW}SKIP${RESET} %s\n" "$1"; }
info() { printf "${BOLD}>>>${RESET} %s\n" "$1"; }

# Detect the committed (embedded) net from .gitignore
committed_net=""
if [ -f ".gitignore" ]; then
    committed_net=$(grep '!nets/' .gitignore 2>/dev/null | sed 's/.*!nets\///' | head -1 || true)
fi

# Build engine
info "Building engine..."
cargo build -r 2>&1 | tail -1

if [ ! -f "$ENGINE" ]; then
    echo "Error: engine not found at $ENGINE"
    exit 1
fi

# Check for base engine and fastchess
has_sprt=false
if [ -f "$BASE_ENGINE" ] && [ -f "$FASTCHESS" ]; then
    has_sprt=true
    info "SPRT enabled: $ENGINE vs $BASE_ENGINE"
elif [ -f "$BASE_ENGINE" ]; then
    info "SPRT disabled: fastchess not found at $FASTCHESS"
else
    info "SPRT disabled: base engine not found at $BASE_ENGINE"
fi

total=0
passed=0
failed=0
skipped=0

for net in "$NETS_DIR"/*.nnue; do
    [ -f "$net" ] || continue
    net_name=$(basename "$net")

    # Skip the committed net (likely incompatible architecture version)
    if [ "$net_name" = "$committed_net" ]; then
        skipped=$((skipped + 1))
        echo ""
        skip "Skipping committed net: $net_name"
        continue
    fi

    total=$((total + 1))
    net_ok=true
    abs_net="$(cd "$(dirname "$net")" && pwd)/$(basename "$net")"

    echo ""
    echo "=========================================="
    info "Validating $net_name"
    echo "=========================================="

    # ── Step 1: Sanity check — eval startpos ──
    info "Step 1/4: Sanity eval (startpos)"
    eval_output=$(printf "setoption name EvalFile value %s\nposition startpos\neval\nquit\n" "$abs_net" | "$ENGINE" 2>&1)

    if echo "$eval_output" | grep -qi "error\|panic\|failed to load"; then
        fail "Step 1: engine error loading net"
        net_ok=false
    else
        eval_cp=$(echo "$eval_output" | grep -i "evaluation\|cp\|score" | head -1 || true)
        if [ -n "$eval_cp" ]; then
            pass "Step 1: eval output: $eval_cp"
        else
            pass "Step 1: net loaded without error"
        fi
    fi

    # ── Step 2: Bench — no crashes + performance ──
    info "Step 2/4: Quick bench (depth $BENCH_DEPTH, $BENCH_POSITIONS positions)"
    bench_output=$(printf "setoption name EvalFile value %s\nbench 16 1 %d %d\nquit\n" "$abs_net" "$BENCH_DEPTH" "$BENCH_POSITIONS" | "$ENGINE" 2>&1)

    if echo "$bench_output" | grep -qi "panic\|error\|thread.*panicked"; then
        fail "Step 2: bench crashed"
        net_ok=false
    else
        bench_nodes=$(echo "$bench_output" | grep "Nodes searched" | awk '{print $NF}')
        bench_nps=$(echo "$bench_output" | grep "Nodes/second" | awk '{print $NF}')
        if [ -n "$bench_nps" ]; then
            pass "Step 2: $bench_nodes nodes, $bench_nps nps"
        else
            fail "Step 2: no bench output"
            net_ok=false
        fi
    fi

    # ── Step 3: ERET — tactical accuracy ──
    info "Step 3/4: ERET (1M nodes per position)"
    eret_output=$(printf "setoption name EvalFile value %s\neret nodes 1000000\nquit\n" "$abs_net" | "$ENGINE" 2>&1)

    if echo "$eret_output" | grep -qi "panic\|error\|thread.*panicked"; then
        fail "Step 3: ERET crashed"
        net_ok=false
    else
        eret_score=$(echo "$eret_output" | grep "ERET Score" | awk '{print $NF}')
        if [ -n "$eret_score" ]; then
            pass "Step 3: ERET Score $eret_score"
        else
            fail "Step 3: no ERET output"
            net_ok=false
        fi
    fi

    # ── Step 4: SPRT vs base engine ──
    if $has_sprt && $net_ok; then
        info "Step 4/4: SPRT vs base ($SPRT_ROUNDS rounds, tc=$SPRT_TC)"
        sprt_output=$("$FASTCHESS" \
            -engine cmd="$ENGINE" name="oxid_new" "option.EvalFile=$abs_net" \
            -engine cmd="$BASE_ENGINE" name="oxid_base" \
            -each tc="$SPRT_TC" \
            -rounds "$SPRT_ROUNDS" -repeat -concurrency "$SPRT_CONCURRENCY" -recover \
            -sprt elo0=0 elo1=5 alpha=0.05 beta=0.05 2>&1)

        if echo "$sprt_output" | grep -qi "H1\|accepted"; then
            elo_line=$(echo "$sprt_output" | grep -i "elo" | tail -1 || true)
            pass "Step 4: SPRT passed — $elo_line"
        elif echo "$sprt_output" | grep -qi "H0\|rejected"; then
            elo_line=$(echo "$sprt_output" | grep -i "elo" | tail -1 || true)
            fail "Step 4: SPRT failed — $elo_line"
            net_ok=false
        else
            skip "Step 4: SPRT inconclusive"
        fi
    elif ! $has_sprt; then
        skip "Step 4: SPRT (no base engine or fastchess)"
    else
        skip "Step 4: SPRT (skipped — earlier steps failed)"
    fi

    # ── Net result ──
    if $net_ok; then
        passed=$((passed + 1))
    else
        failed=$((failed + 1))
    fi
done

echo ""
echo "=========================================="
printf "${BOLD}Results: %d/%d passed${RESET}" "$passed" "$total"
if [ "$failed" -gt 0 ]; then
    printf ", ${RED}%d failed${RESET}" "$failed"
fi
if [ "$skipped" -gt 0 ]; then
    printf ", ${YELLOW}%d skipped${RESET}" "$skipped"
fi
echo ""
echo "=========================================="
