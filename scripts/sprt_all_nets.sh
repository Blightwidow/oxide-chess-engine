#!/usr/bin/env bash
# Run SPRT tests for every .nnue file in nets/ against a base engine binary.
# Skips the committed (embedded) net. Results are logged to nets/<hash>.sprt.log.
# Expects to be run from the repo root.
#
# Usage: ./scripts/sprt_all_nets.sh [base_engine]
#   base_engine: path to baseline engine binary (default: ./base/release/oxid)
#
# Flags:
#   --summary    Print summary of previous results and exit

set -euo pipefail

NETS_DIR="nets"
ENGINE="./target/release/oxid"
FASTCHESS="./bin/fastchess"
OPENINGS="data/openings.pgn"
CONCURRENCY=6
ROUNDS=15000
TC="8+0.08"

# SPRT bounds: H0 = no regression (elo0=0), H1 = elo gain (elo1=5)
ELO0=0
ELO1=5
ALPHA=0.05
BETA=0.05

# Detect the committed (embedded) net from .gitignore
committed_net=""
if [ -f ".gitignore" ]; then
    committed_net=$(grep '!nets/' .gitignore 2>/dev/null | sed 's/.*!nets\///' | head -1 || true)
fi

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
RESET='\033[0m'

# Parse arguments: handle --summary before positional args
BASE_ENGINE="./base/release/oxid"
for arg in "$@"; do
    case "$arg" in
        --summary) ;;  # handled below
        *) BASE_ENGINE="$arg" ;;
    esac
done

# Print summary of all previously tested nets and exit
print_summary() {
    echo "=========================================="
    echo "  SPRT Test Summary (base: $(basename "$BASE_ENGINE"))"
    echo "=========================================="
    echo ""

    local p=0 f=0 inc=0

    for logfile in "$NETS_DIR"/*.sprt.log; do
        [ -f "$logfile" ] || continue

        local net_name
        net_name=$(basename "${logfile%.sprt.log}.nnue")

        # Extract the last results block from the log
        local elo games result_label
        elo=$(grep "^Elo:" "$logfile" | tail -1 | sed 's/Elo: //')
        games=$(grep "^Games:" "$logfile" | tail -1 | sed 's/Games: \([0-9]*\).*/\1/')

        if grep -q "H1 was accepted" "$logfile"; then
            result_label="PASSED"
            p=$((p + 1))
        elif grep -q "H0 was accepted" "$logfile"; then
            result_label="FAILED"
            f=$((f + 1))
        else
            result_label="INCOMP"
            inc=$((inc + 1))
        fi

        printf "  %-8s  %-30s  %5s games  Elo: %s\n" "$result_label" "$net_name" "$games" "$elo"
    done

    echo ""
    echo "------------------------------------------"
    echo "  Totals: $p passed, $f failed, $inc incomplete"
    echo "=========================================="
}

# If --summary is passed, just print summary and exit
for arg in "$@"; do
    if [[ "$arg" == "--summary" ]]; then
        print_summary
        exit 0
    fi
done

if [ ! -f "$BASE_ENGINE" ]; then
    echo "Error: base engine not found at $BASE_ENGINE."
    echo "Build it first: cargo build -r --target-dir=base"
    exit 1
fi

if [ ! -f "$FASTCHESS" ]; then
    echo "Error: fastchess not found at $FASTCHESS."
    exit 1
fi

# Build candidate engine if needed
if [ ! -f "$ENGINE" ]; then
    echo "Building engine..."
    cargo build -r
fi

# Print summary of previous results before starting new tests
print_summary
echo ""

passed=0
failed=0
skipped=0

for net in "$NETS_DIR"/*.nnue; do
    [ -f "$net" ] || continue
    net_name=$(basename "$net")

    # Skip the committed net (likely incompatible architecture version)
    if [ "$net_name" = "$committed_net" ]; then
        printf "${YELLOW}SKIP${RESET} %s (committed net)\n" "$net_name"
        skipped=$((skipped + 1))
        continue
    fi

    logfile="${net%.nnue}.sprt.log"

    if [ -f "$logfile" ]; then
        echo "Skipping $net_name — already tested (see $logfile)"
        skipped=$((skipped + 1))
        continue
    fi

    echo "=========================================="
    echo "Testing $net_name vs $(basename "$BASE_ENGINE")"
    echo "=========================================="

    openings_args=()
    if [ -f "$OPENINGS" ]; then
        openings_args=(-openings file="$OPENINGS" format=pgn order=random)
    fi

    abs_net="$(cd "$(dirname "$net")" && pwd)/$(basename "$net")"

    "$FASTCHESS" \
        -engine cmd="$ENGINE" name=candidate "option.EvalFile=$abs_net" \
        -engine cmd="$BASE_ENGINE" name=base \
        -each tc="$TC" \
        "${openings_args[@]}" \
        -rounds "$ROUNDS" -repeat -concurrency "$CONCURRENCY" -recover \
        -sprt elo0="$ELO0" elo1="$ELO1" alpha="$ALPHA" beta="$BETA" \
        2>&1 | tee "$logfile"

    # Check SPRT result
    if grep -q "H1 was accepted" "$logfile"; then
        printf "${GREEN}PASSED${RESET}: %s\n" "$net_name"
        passed=$((passed + 1))
    elif grep -q "H0 was accepted" "$logfile"; then
        printf "${RED}FAILED${RESET}: %s\n" "$net_name"
        failed=$((failed + 1))
    else
        printf "${YELLOW}INCONCLUSIVE${RESET}: %s (check %s)\n" "$net_name" "$logfile"
    fi

    echo ""
done

echo "=========================================="
echo "Summary: $passed passed, $failed failed, $skipped skipped"
echo "=========================================="
