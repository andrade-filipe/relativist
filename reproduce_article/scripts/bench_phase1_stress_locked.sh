#!/usr/bin/env bash
# =============================================================================
# Phase 1 Stress Campaign — v1_stress (in-process)
# =============================================================================
# Runs the Phase 1 stress benchmark campaign on the tagged binary and emits
# frozen CSVs under `results/extended/v1_stress/`.
#
# Purpose: push ep_annihilation_con and dual_tree beyond the sizes covered by
# v1_local_baseline (ep_con up to 5M, dual_tree up to 22) and record where the
# local (in-process) path starts to show cost scaling. This is the "before"
# baseline that the future ROADMAP 2.22-2.26 network optimisations will be
# compared against.
#
# Campaign layout:
#   - ep_annihilation_con × {10M, 20M, 50M} × workers {seq, 1, 2, 4, 8}
#   - dual_tree × {23, 24, 25} × workers {seq, 1, 2, 4, 8}
#   - 5 repetitions (half of v1 due to longer per-rep cost at large N)
#   - 2 warmup runs (same as v1)
#   - lenient BSP mode (strict BSP is a v1_local_baseline concern, not stress)
#   - Full G1 correctness (ep_con and dual_tree are both tractable under G1)
#
# Pre-conditions:
#   - Clean working copy at tag v0.10.0-bench.
#   - `cargo build --release` succeeded.
#   - Environment hygiene matches v1 (Ultimate Performance power plan, IDE
#     closed, browsers closed, Windows Update paused, no other heavy load).
#   - Docker Desktop may be running or closed — Phase 1 is in-process, so
#     Docker state does not matter for these numbers.
#
# On error the script stops immediately (set -e). Partial CSVs remain in
# place under raw/ for forensic review.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
STRESS_DIR="$REPO_DIR/reproduce_article/results/extended/v1_stress"
RAW_DIR="$STRESS_DIR/raw/phase1"

mkdir -p "$STRESS_DIR" "$RAW_DIR"

if [ -f "$REPO_DIR/target/release/relativist.exe" ]; then
    RELATIVIST="$REPO_DIR/target/release/relativist.exe"
elif [ -f "$REPO_DIR/target/release/relativist" ]; then
    RELATIVIST="$REPO_DIR/target/release/relativist"
else
    echo "ERROR: relativist release binary not found. Run 'cargo build --release' first." >&2
    exit 1
fi

log() { echo "[$(date +%H:%M:%S)] $*"; }

REPETITIONS=5
WARMUP=2
WORKERS="1,2,4,8"

EP_CON_SIZES="10000000,20000000,50000000"
DUAL_TREE_SIZES="23,24,25"

run_bench_stress() {
    local bench="$1"
    local sizes_opt="$2"

    local tag="${bench}_stress"
    local detail="$RAW_DIR/${tag}_detail.csv"
    local rounds="$RAW_DIR/${tag}_rounds.csv"
    local summary="$RAW_DIR/${tag}_summary.csv"
    local log_file="$RAW_DIR/${tag}.log"

    log "STRESS   $bench sizes=$sizes_opt (workers=$WORKERS reps=$REPETITIONS)"

    "$RELATIVIST" bench \
        --benchmark "$bench" \
        --sizes "$sizes_opt" \
        --workers "$WORKERS" \
        --repetitions "$REPETITIONS" \
        --warmup "$WARMUP" \
        --mode local \
        --csv-detail "$detail" \
        --csv-rounds "$rounds" \
        --csv-summary "$summary" \
        >"$log_file" 2>&1
}

concat_csvs() {
    local out="$1"
    shift
    local files=("$@")
    : > "$out"
    local first=true
    for f in "${files[@]}"; do
        [ -f "$f" ] || continue
        if $first; then
            cat "$f" >> "$out"
            first=false
        else
            tail -n +2 "$f" >> "$out"
        fi
    done
}

log "=== Phase 1 Stress Campaign — v1_stress ==="
log "Binary: $RELATIVIST"
log "Output: $STRESS_DIR"
log ""

START_TS=$(date '+%Y-%m-%d %H:%M:%S %z')
log "Start: $START_TS"

run_bench_stress "ep_annihilation_con" "$EP_CON_SIZES"
run_bench_stress "dual_tree" "$DUAL_TREE_SIZES"

log ""
log "=== Concatenating CSVs ==="

DETAIL="$STRESS_DIR/phase1_stress_detail.csv"
ROUNDS="$STRESS_DIR/phase1_stress_rounds.csv"
SUMMARY="$STRESS_DIR/phase1_stress_summary.csv"

detail_files=()
rounds_files=()
summary_files=()
for f in "$RAW_DIR"/*_detail.csv; do detail_files+=("$f"); done
for f in "$RAW_DIR"/*_rounds.csv; do rounds_files+=("$f"); done
for f in "$RAW_DIR"/*_summary.csv; do summary_files+=("$f"); done

concat_csvs "$DETAIL"  "${detail_files[@]}"
concat_csvs "$ROUNDS"  "${rounds_files[@]}"
concat_csvs "$SUMMARY" "${summary_files[@]}"

END_TS=$(date '+%Y-%m-%d %H:%M:%S %z')

log ""
log "=== Phase 1 Stress Campaign complete ==="
log "Start:   $START_TS"
log "End:     $END_TS"
log "Detail:  $(wc -l < "$DETAIL") rows -> $DETAIL"
log "Rounds:  $(wc -l < "$ROUNDS") rows -> $ROUNDS"
log "Summary: $(wc -l < "$SUMMARY") rows -> $SUMMARY"
