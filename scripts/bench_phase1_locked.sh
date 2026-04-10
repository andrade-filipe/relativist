#!/usr/bin/env bash
# =============================================================================
# Phase 1 Locked Baseline — v1_local_baseline
# =============================================================================
# Runs the full Phase 1 local benchmark campaign on a tagged binary and
# emits frozen CSVs under `results/locked/v1_local_baseline/`.
#
# Campaign layout:
#   - Lenient (default) pass: all 12 benchmarks, 10 repetitions each, mode
#     local, workers 1,2,4,8 (plus sequential baseline auto-added by the
#     suite). Per-benchmark default sizes from `Benchmark::default_sizes()`.
#   - condup_expansion at 10_000 / 50_000 uses `--skip-g1` (weak count
#     check) — abordagem A, the documented default. See USAGE_GUIDE.md
#     Section 11.5 for the optional abordagem B overnight commands.
#   - Strict BSP pass: cascade_cross (all sizes) and dual_tree (subset)
#     re-run with `--strict-bsp` so the multi-round BSP data required by
#     Phase 3 LAN is captured in the frozen snapshot.
#
# All CSVs are concatenated per stream (detail / rounds / summary) and
# written to the locked directory. Raw per-benchmark files land under
# `raw/phase1/` alongside the captured stdout/stderr logs.
#
# Pre-conditions:
#   - Run from a clean working copy at tag v0.10.0-bench (or newer).
#   - `cargo build --release` must succeed prior to invocation.
#   - Nothing else using significant CPU/RAM on the machine.
#
# On error the script stops immediately (set -e). Partial CSVs remain in
# place under raw/ for forensic review.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
LOCKED_DIR="$REPO_DIR/results/locked/v1_local_baseline"
RAW_DIR="$LOCKED_DIR/raw/phase1"

mkdir -p "$LOCKED_DIR" "$RAW_DIR"

if [ -f "$REPO_DIR/target/release/relativist.exe" ]; then
    RELATIVIST="$REPO_DIR/target/release/relativist.exe"
elif [ -f "$REPO_DIR/target/release/relativist" ]; then
    RELATIVIST="$REPO_DIR/target/release/relativist"
else
    echo "ERROR: relativist release binary not found. Run 'cargo build --release' first." >&2
    exit 1
fi

log() { echo "[$(date +%H:%M:%S)] $*"; }

# ----------------------------------------------------------------------------
# Lenient pass — default grid loop
# ----------------------------------------------------------------------------
REPETITIONS=10
WARMUP=2
WORKERS="1,2,4,8"

# Benchmarks that run at their full default size set, lenient + full G1.
LENIENT_BENCHMARKS=(
    "ep_annihilation"
    "ep_annihilation_con"
    "ep_annihilation_dup"
    "dual_tree"
    "tree_sum"
    "tree_sum_balanced"
    "mixed_net"
    "erasure_propagation"
    "church_add"
    "church_mul"
    "cascade_cross"
)

# condup_expansion splits into: small sizes with G1 full, large sizes weak.
CONDUP_SMALL_SIZES="100,500,1000,5000"
CONDUP_LARGE_SIZES="10000,50000"

# Strict pass benchmarks + size overrides (smaller than lenient defaults to
# keep the round-by-round data manageable).
STRICT_CASCADE_SIZES="10,50,100,500,1000"
STRICT_DUAL_TREE_SIZES="6,10,14"

run_bench_lenient() {
    local bench="$1"
    local sizes_opt="${2:-}"
    local skip_g1="${3:-false}"

    local tag="${bench}"
    if [ -n "$sizes_opt" ]; then
        tag="${bench}_s$(echo "$sizes_opt" | tr ',' '_')"
    fi

    local detail="$RAW_DIR/${tag}_detail.csv"
    local rounds="$RAW_DIR/${tag}_rounds.csv"
    local summary="$RAW_DIR/${tag}_summary.csv"
    local log_file="$RAW_DIR/${tag}.log"

    log "LENIENT  $tag (workers=$WORKERS reps=$REPETITIONS)"

    local cmd=(
        "$RELATIVIST" bench
        --benchmark "$bench"
        --workers "$WORKERS"
        --repetitions "$REPETITIONS"
        --warmup "$WARMUP"
        --mode local
        --csv-detail "$detail"
        --csv-rounds "$rounds"
        --csv-summary "$summary"
    )
    if [ -n "$sizes_opt" ]; then
        cmd+=(--sizes "$sizes_opt")
    fi
    if [ "$skip_g1" = "true" ]; then
        cmd+=(--skip-g1)
    fi

    "${cmd[@]}" >"$log_file" 2>&1
}

run_bench_strict() {
    local bench="$1"
    local sizes_opt="$2"

    local tag="${bench}_strict"
    local detail="$RAW_DIR/${tag}_detail.csv"
    local rounds="$RAW_DIR/${tag}_rounds.csv"
    local summary="$RAW_DIR/${tag}_summary.csv"
    local log_file="$RAW_DIR/${tag}.log"

    log "STRICT   $tag (sizes=$sizes_opt)"

    "$RELATIVIST" bench \
        --benchmark "$bench" \
        --sizes "$sizes_opt" \
        --workers "$WORKERS" \
        --repetitions "$REPETITIONS" \
        --warmup "$WARMUP" \
        --mode local \
        --strict-bsp \
        --csv-detail "$detail" \
        --csv-rounds "$rounds" \
        --csv-summary "$summary" \
        >"$log_file" 2>&1
}

# Concatenate CSV files preserving the header from the first file only.
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

log "=== Phase 1 Locked Baseline — v1_local_baseline ==="
log "Binary: $RELATIVIST"
log "Output: $LOCKED_DIR"
log ""

# --- Lenient main pass -------------------------------------------------------
for bench in "${LENIENT_BENCHMARKS[@]}"; do
    run_bench_lenient "$bench"
done

# --- condup_expansion (abordagem A default) ---------------------------------
run_bench_lenient "condup_expansion" "$CONDUP_SMALL_SIZES" "false"
run_bench_lenient "condup_expansion" "$CONDUP_LARGE_SIZES" "true"

# --- Strict BSP pass --------------------------------------------------------
run_bench_strict "cascade_cross" "$STRICT_CASCADE_SIZES"
run_bench_strict "dual_tree" "$STRICT_DUAL_TREE_SIZES"

log ""
log "=== Concatenating CSVs ==="

# Lenient aggregate
LENIENT_DETAIL="$LOCKED_DIR/phase1_lenient_detail.csv"
LENIENT_ROUNDS="$LOCKED_DIR/phase1_lenient_rounds.csv"
LENIENT_SUMMARY="$LOCKED_DIR/phase1_lenient_summary.csv"

lenient_detail_files=()
lenient_rounds_files=()
lenient_summary_files=()
for f in "$RAW_DIR"/*_detail.csv; do
    case "$f" in
        *_strict_detail.csv) continue ;;
    esac
    lenient_detail_files+=("$f")
done
for f in "$RAW_DIR"/*_rounds.csv; do
    case "$f" in
        *_strict_rounds.csv) continue ;;
    esac
    lenient_rounds_files+=("$f")
done
for f in "$RAW_DIR"/*_summary.csv; do
    case "$f" in
        *_strict_summary.csv) continue ;;
    esac
    lenient_summary_files+=("$f")
done

concat_csvs "$LENIENT_DETAIL"  "${lenient_detail_files[@]}"
concat_csvs "$LENIENT_ROUNDS"  "${lenient_rounds_files[@]}"
concat_csvs "$LENIENT_SUMMARY" "${lenient_summary_files[@]}"

# Strict aggregate
STRICT_DETAIL="$LOCKED_DIR/phase1_strict_detail.csv"
STRICT_ROUNDS="$LOCKED_DIR/phase1_strict_rounds.csv"
STRICT_SUMMARY="$LOCKED_DIR/phase1_strict_summary.csv"

concat_csvs "$STRICT_DETAIL"  "$RAW_DIR"/*_strict_detail.csv
concat_csvs "$STRICT_ROUNDS"  "$RAW_DIR"/*_strict_rounds.csv
concat_csvs "$STRICT_SUMMARY" "$RAW_DIR"/*_strict_summary.csv

log ""
log "=== Phase 1 Locked Baseline complete ==="
log "Lenient detail:  $(wc -l < "$LENIENT_DETAIL") rows -> $LENIENT_DETAIL"
log "Lenient rounds:  $(wc -l < "$LENIENT_ROUNDS") rows -> $LENIENT_ROUNDS"
log "Lenient summary: $(wc -l < "$LENIENT_SUMMARY") rows -> $LENIENT_SUMMARY"
log "Strict  detail:  $(wc -l < "$STRICT_DETAIL") rows -> $STRICT_DETAIL"
log "Strict  rounds:  $(wc -l < "$STRICT_ROUNDS") rows -> $STRICT_ROUNDS"
log "Strict  summary: $(wc -l < "$STRICT_SUMMARY") rows -> $STRICT_SUMMARY"
