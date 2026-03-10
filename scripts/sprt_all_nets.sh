#!/usr/bin/env bash
# Run SPRT tests for every .nnue file in nets/ against the currently embedded net.
# Skips the baseline net itself. Results are logged to nets/<hash>.sprt.log.
# Expects to be run from the repo root.

set -euo pipefail

NETS_DIR="nets"
BASE_NAME=$(grep 'pub const DEFAULT_EVAL_FILE' src/main.rs | sed 's/.*"\(.*\)".*/\1/')
BASE_NET="nets/${BASE_NAME}"
FASTCHESS="./bin/fastchess"
ENGINE="./target/release/oxide"
OPENINGS="data/openings.pgn"
CONCURRENCY=6
ROUNDS=15000
TC="8+0.08"

# SPRT bounds: H0 = no regression (elo0=0), H1 = elo gain (elo1=5)
ELO0=0
ELO1=5
ALPHA=0.05
BETA=0.05

if [ ! -f "$BASE_NET" ]; then
    echo "Error: baseline net $BASE_NET not found."
    exit 1
fi

if [ ! -f "$FASTCHESS" ]; then
    echo "Error: fastchess not found at $FASTCHESS."
    exit 1
fi

# Build engine if needed
if [ ! -f "$ENGINE" ]; then
    echo "Building engine..."
    cargo build -r
fi

passed=0
failed=0
skipped=0

for net in "$NETS_DIR"/*.nnue; do
    [ -f "$net" ] || continue

    # Skip the baseline
    if [ "$(realpath "$net")" = "$(realpath "$BASE_NET")" ]; then
        continue
    fi

    logfile="${net%.nnue}.sprt.log"

    if [ -f "$logfile" ]; then
        echo "Skipping $(basename "$net") — already tested (see $logfile)"
        skipped=$((skipped + 1))
        continue
    fi

    echo "=========================================="
    echo "Testing $(basename "$net") vs $(basename "$BASE_NET")"
    echo "=========================================="

    openings_args=()
    if [ -f "$OPENINGS" ]; then
        openings_args=(-openings file="$OPENINGS" format=pgn order=random)
    fi

    "$FASTCHESS" \
        -engine cmd="$ENGINE" name=candidate "option.EvalFile=$net" \
        -engine cmd="$ENGINE" name=base "option.EvalFile=$BASE_NET" \
        -each tc="$TC" \
        "${openings_args[@]}" \
        -rounds "$ROUNDS" -repeat -concurrency "$CONCURRENCY" -recover \
        -sprt elo0="$ELO0" elo1="$ELO1" alpha="$ALPHA" beta="$BETA" \
        2>&1 | tee "$logfile"

    # Check SPRT result
    if grep -q "H1 was accepted" "$logfile"; then
        echo "PASSED: $(basename "$net")"
        passed=$((passed + 1))
    elif grep -q "H0 was accepted" "$logfile"; then
        echo "FAILED: $(basename "$net")"
        failed=$((failed + 1))
    else
        echo "INCONCLUSIVE: $(basename "$net") (check $logfile)"
    fi

    echo ""
done

echo "=========================================="
echo "Summary: $passed passed, $failed failed, $skipped skipped"
echo "=========================================="
