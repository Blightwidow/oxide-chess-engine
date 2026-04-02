#!/usr/bin/env bash
# Quick validation of all .nnue files in nets/.
# Runs 2 checks per net: sanity eval + bench.
# Expects to be run from the repo root.

set -euo pipefail

NETS_DIR="nets"
ENGINE="./target/release/oxid"
BENCH_DEPTH=11
BENCH_POSITIONS=5

RED='\033[0;31m'
GREEN='\033[0;32m'
BOLD='\033[1m'
RESET='\033[0m'

pass() { printf "${GREEN}PASS${RESET} %s\n" "$1"; }
fail() { printf "${RED}FAIL${RESET} %s\n" "$1"; }
info() { printf "${BOLD}>>>${RESET} %s\n" "$1"; }

# Build engine
info "Building engine..."
cargo build -r 2>&1 | tail -1

if [ ! -f "$ENGINE" ]; then
    echo "Error: engine not found at $ENGINE"
    exit 1
fi

total=0
passed=0
failed=0

for net in "$NETS_DIR"/*.nnue; do
    [ -f "$net" ] || continue
    net_name=$(basename "$net")
    total=$((total + 1))
    net_ok=true

    echo ""
    echo "=========================================="
    info "Validating $net_name"
    echo "=========================================="

    # ── Step 1: Sanity check — eval startpos ──
    info "Step 1/2: Sanity eval (startpos)"
    eval_output=$(printf "setoption name EvalFile value %s\nposition startpos\neval\nquit\n" "$net" | "$ENGINE" 2>&1)

    if echo "$eval_output" | grep -qi "error\|panic\|failed to load"; then
        fail "Step 1: engine error loading net"
        net_ok=false
    else
        eval_cp=$(echo "$eval_output" | grep -i "evaluation\|cp\|score" | head -1 || true)
        if [ -n "$eval_cp" ]; then
            pass "Step 1: eval output: $eval_cp"
        else
            # No explicit eval command — just check it didn't crash
            pass "Step 1: net loaded without error"
        fi
    fi

    # ── Step 2: Bench — no crashes + performance ──
    info "Step 2/2: Quick bench (depth $BENCH_DEPTH, $BENCH_POSITIONS positions)"
    bench_output=$(printf "setoption name EvalFile value %s\nbench 16 1 %d %d\nquit\n" "$net" "$BENCH_DEPTH" "$BENCH_POSITIONS" | "$ENGINE" 2>&1)

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
echo ""
echo "=========================================="
