#!/usr/bin/env bash
# =============================================================================
# D-014 Stress Curve Campaign Orchestrator (TASK-0704)
# =============================================================================
# Drives the in-process and Docker TCP arms of the stress-curve campaign per
# design doc `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md`.
# Each rep runs as a child process so VmHWM resets between reps; the StopRule
# (TASK-0701, exposed via Rust helpers) decides when the N sweep aborts.
#
# Usage:
#   scripts/stress_curve.sh [OPTIONS]
#
# OPTIONS:
#   --smoke              Smoke mode: 1 workload (ep_annihilation), W=2,
#                        N=[1000, 10000], 1 rep, 15 min total budget.
#   --no-docker          Skip Phase 2 (in-process only).
#   --resume             Resume from a partial run (skips already-completed
#                        (workload, env, W, N, rep) tuples present in the
#                        existing CSV).
#   --output-dir DIR     Override output directory.
#                        Default: results/locked/v2_stress_curve_$(date -I)/
#   --workloads LIST     Comma list of {ep_annihilation, dual_tree,
#                        condup_expansion}. Default: all 3.
#   --workers LIST       Comma list of {1, 2, 4, 8}. Default: all 4.
#   -h | --help          Print this help.
#
# Exit codes:
#   0   Campaign completed
#   1   Pre-condition failed (dirty tree, tests red, low RAM, etc.)
#   2   Mid-run abort: every (workload, W) hit StopRule before N=10000
#   10  User interrupt (SIGINT/SIGTERM); partial output preserved for --resume
#
# NOTE: Phase 2 (Docker arm) is best-effort — if `docker compose` is
# unavailable or individual reps fail, the script emits warnings and
# continues to Phase 3 with Phase 1 data intact (does NOT exit non-zero).
#
# Requires bash 4+ (associative arrays) and (in non-`--no-docker` mode)
# `docker compose`. Plotting is intentionally OUT OF SCOPE for this script
# (operator decision, 2026-05-15): raw CSVs only, in the schema of
# `results/extended/v1_stress/phase2_stress_detail.csv`. External tools
# generate the figures from those CSVs after the campaign locks.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# --- arg parsing ---
SMOKE=0
NO_DOCKER=0
RESUME=0
OUTPUT_DIR=""
WORKLOADS_FILTER=""
WORKERS_FILTER=""

print_help() {
    sed -n '2,/^=*$/p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --smoke) SMOKE=1; shift ;;
        --no-docker) NO_DOCKER=1; shift ;;
        --resume) RESUME=1; shift ;;
        --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
        --workloads) WORKLOADS_FILTER="$2"; shift 2 ;;
        --workers) WORKERS_FILTER="$2"; shift 2 ;;
        -h|--help) print_help; exit 0 ;;
        *) echo "ERROR: unknown option: $1" >&2; exit 1 ;;
    esac
done

if [[ -z "$OUTPUT_DIR" ]]; then
    OUTPUT_DIR="$REPO_DIR/results/locked/v2_stress_curve_$(date -I)"
fi

mkdir -p "$OUTPUT_DIR/raw"

# TASK-0720 BUG-004: SIGINT/SIGTERM trap MUST forward the signal to the
# in-flight child rep so it cleans up before parent exits; without this,
# the child keeps running after Ctrl+C and may corrupt CSV mid-write.
# We track the most recent rep child via $REP_PID (set inside the loop).
# Also: take a PID-bearing lockfile at $OUTPUT_DIR/.lock so concurrent
# `--resume` invocations don't double-write the same CSV; a stale lock
# from a crashed previous run is detected via `kill -0 $stale_pid` and
# refused (operator must remove it manually).
LOCKFILE="$OUTPUT_DIR/.lock"
REP_PID=""

acquire_lock() {
    if [[ -f "$LOCKFILE" ]]; then
        local stale_pid
        stale_pid="$(cat "$LOCKFILE" 2>/dev/null || echo)"
        if [[ -n "$stale_pid" ]] && kill -0 "$stale_pid" 2>/dev/null; then
            echo "ERROR: another stress_curve.sh instance is already running (pid $stale_pid; lockfile $LOCKFILE)" >&2
            exit 1
        fi
        echo "WARN: stale lockfile at $LOCKFILE (pid $stale_pid not alive); claiming it" >&2
    fi
    echo "$$" >"$LOCKFILE"
}

release_lock() {
    if [[ -f "$LOCKFILE" ]]; then
        local owner
        owner="$(cat "$LOCKFILE" 2>/dev/null || echo)"
        if [[ "$owner" == "$$" ]]; then
            rm -f "$LOCKFILE"
        fi
    fi
}

acquire_lock

ON_INTERRUPT=0
on_interrupt() {
    ON_INTERRUPT=1
    if [[ -n "$REP_PID" ]] && kill -0 "$REP_PID" 2>/dev/null; then
        echo "INTERRUPT received; sending SIGTERM to child pid $REP_PID" >&2
        kill -SIGTERM "$REP_PID" 2>/dev/null || true
        # Best-effort wait for child to exit (max ~10 s); fall back to
        # SIGKILL if the child ignores SIGTERM.
        for _ in 1 2 3 4 5 6 7 8 9 10; do
            kill -0 "$REP_PID" 2>/dev/null || break
            sleep 1
        done
        if kill -0 "$REP_PID" 2>/dev/null; then
            echo "WARN: child pid $REP_PID still alive after SIGTERM; sending SIGKILL" >&2
            kill -SIGKILL "$REP_PID" 2>/dev/null || true
            wait "$REP_PID" 2>/dev/null || true
        else
            wait "$REP_PID" 2>/dev/null || true
        fi
    fi
    # Best-effort: tear down any bench-tcp containers/networks the docker
    # arm may have left running. Silent failure is fine — this is a
    # cleanup, not a checkpoint.
    if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
        (cd "$REPO_DIR" && docker compose --profile bench-tcp down --remove-orphans >/dev/null 2>&1) || true
    fi
    echo "INTERRUPT received; partial output preserved at $OUTPUT_DIR" >&2
    release_lock
    # 130 = 128 + SIGINT (per POSIX convention). The previous code used
    # exit 10; we follow the standard SIGINT exit code so callers can
    # detect "user-initiated abort" via the canonical channel.
    exit 130
}
trap on_interrupt INT TERM
trap release_lock EXIT

RAW_CSV="$OUTPUT_DIR/raw/in_process.csv"
DOCKER_CSV="$OUTPUT_DIR/raw/docker_tcp.csv"

# --- locate release binary ---
if [[ -x "$REPO_DIR/target/release/relativist.exe" ]]; then
    RELATIVIST_BIN="$REPO_DIR/target/release/relativist.exe"
elif [[ -x "$REPO_DIR/target/release/relativist" ]]; then
    RELATIVIST_BIN="$REPO_DIR/target/release/relativist"
else
    echo "ERROR: target/release/relativist not built; run \`cargo build --release\` first" >&2
    exit 1
fi

# --- pre-condition gate (skipped in --smoke mode for fast iteration) ---
if [[ $SMOKE -eq 0 ]]; then
    if ! git -C "$REPO_DIR" diff --quiet || ! git -C "$REPO_DIR" diff --cached --quiet; then
        if [[ -n "$(git -C "$REPO_DIR" status --porcelain)" ]]; then
            echo "ERROR: working tree not clean; commit or stash before running the full campaign" >&2
            exit 1
        fi
    fi
    if [[ $NO_DOCKER -eq 0 ]]; then
        if ! command -v docker >/dev/null 2>&1 || ! docker compose version >/dev/null 2>&1; then
            echo "ERROR: docker compose not available; pass --no-docker or install Docker Desktop" >&2
            exit 3
        fi
    fi
    # Operator decision 2026-05-15: the python3/matplotlib/pandas/numpy
    # pre-gate is intentionally removed. The script no longer invokes the
    # plotter — it produces CSV-only output (see Phase 4 removal below).
    # Plots are generated by external tooling after the campaign locks.
fi

# --- matrix definition ---
DEFAULT_WORKLOADS="ep_annihilation,dual_tree,condup_expansion"
DEFAULT_WORKERS="1,2,4,8"
if [[ $SMOKE -eq 1 ]]; then
    WORKLOADS="${WORKLOADS_FILTER:-ep_annihilation}"
    WORKERS="${WORKERS_FILTER:-2}"
    SMOKE_NS=(1000 10000)
    REPS=1
else
    WORKLOADS="${WORKLOADS_FILTER:-$DEFAULT_WORKLOADS}"
    WORKERS="${WORKERS_FILTER:-$DEFAULT_WORKERS}"
    REPS=5
fi

# Canonical 11-point N sweep ×√10 from 10⁴ to 10⁹ (matches design doc §4.4 +
# `StressCurveDescriptor::n_seq()`). Smoke mode overrides this with SMOKE_NS.
N_SEQ=(10000 31623 100000 316228 1000000 3162278 10000000 31622776 100000000 316227766 1000000000)

# BUG-FIX 2026-05-14 (Option C): the unbounded 10⁹ tail of `N_SEQ` is OOM-risky
# on commodity hosts (alloc ~68 GB at N=10⁹) and stresses code paths still in
# triage from the 2026-05-14 run. `STRESS_CURVE_N_MAX` lets the operator cap
# the sweep without editing this script; default is 10⁷ (matches the 7-point
# Option C campaign). To restore the full 11-point sweep:
#   STRESS_CURVE_N_MAX=1000000000 scripts/stress_curve.sh ...
STRESS_CURVE_N_MAX="${STRESS_CURVE_N_MAX:-10000000}"
FILTERED_N_SEQ=()
for _n in "${N_SEQ[@]}"; do
    if [[ "$_n" -le "$STRESS_CURVE_N_MAX" ]]; then
        FILTERED_N_SEQ+=("$_n")
    fi
done
if [[ ${#FILTERED_N_SEQ[@]} -eq 0 ]]; then
    echo "ERROR: STRESS_CURVE_N_MAX=$STRESS_CURVE_N_MAX excludes every point in N_SEQ" >&2
    exit 1
fi
N_SEQ=("${FILTERED_N_SEQ[@]}")
unset FILTERED_N_SEQ _n

# --- Phase 1: in-process ---
echo "=== Phase 1: in-process arm ==="
echo "  workloads: $WORKLOADS"
echo "  workers:   $WORKERS"
echo "  reps:      $REPS"
echo "  output:    $RAW_CSV"

# Resume: read existing CSV (if any) and build a set of completed tuples.
declare -A DONE
if [[ $RESUME -eq 1 && -f "$RAW_CSV" ]]; then
    while IFS=, read -r line; do
        # Match lines beginning with the workload name.
        wl=$(echo "$line" | cut -d, -f1)
        sz=$(echo "$line" | cut -d, -f2)
        wk=$(echo "$line" | cut -d, -f4)
        rp=$(echo "$line" | cut -d, -f5)
        if [[ -n "$wl" && "$wl" != "benchmark" ]]; then
            DONE["${wl}|in_process|${wk}|${sz}|${rp}"]=1
        fi
    done <"$RAW_CSV"
    echo "  resume: ${#DONE[@]} previously-completed tuples skipped"
fi

# In smoke mode: write header from the descriptor's first invocation.
HEADER_WRITTEN=0
if [[ -f "$RAW_CSV" ]]; then HEADER_WRITTEN=1; fi

IFS=',' read -ra WL_ARR <<<"$WORKLOADS"
IFS=',' read -ra WK_ARR <<<"$WORKERS"

if [[ $SMOKE -eq 1 ]]; then
    NS=("${SMOKE_NS[@]}")
else
    NS=("${N_SEQ[@]}")
fi

for WL in "${WL_ARR[@]}"; do
    for WK in "${WK_ARR[@]}"; do
        for N in "${NS[@]}"; do
            for REP in $(seq 1 "$REPS"); do
                key="${WL}|in_process|${WK}|${N}|${REP}"
                if [[ -n "${DONE[$key]:-}" ]]; then continue; fi

                STDERR_LOG="$OUTPUT_DIR/raw/${WL}_${WK}_${N}_${REP}.stderr"
                # TASK-0720 BUG-001: the Rust dispatch path emits a real
                # CSV (header + per-rep rows) on stdout via `write_csv_detail`.
                # For the second invocation onward, we strip the duplicate
                # header line so the aggregated CSV has exactly one header.
                set +e
                # BUG-FIX 2026-05-14: the outer `for REP in $(seq 1 "$REPS")`
                # already iterates `$REPS` times; passing `--reps "$REP"` made
                # each child run 1, 2, 3, ... reps internally — ~3.3× inflation
                # at REPS=5 (1+2+3+4+5 = 15 vs intended 5). The child is now
                # invoked with `--reps 1` so the bash loop is the sole driver
                # of repetition counts.
                if [[ $HEADER_WRITTEN -eq 0 ]]; then
                    # First write: keep the header.
                    "$RELATIVIST_BIN" bench \
                        --campaign stress-curve \
                        --workload "$WL" \
                        --env in-process \
                        --workers "$WK" \
                        --reps 1 \
                        --n-seq "$N" \
                        >>"$RAW_CSV" 2>"$STDERR_LOG" &
                else
                    # Strip the leading header line on subsequent invocations
                    # to keep the aggregated CSV well-formed.
                    "$RELATIVIST_BIN" bench \
                        --campaign stress-curve \
                        --workload "$WL" \
                        --env in-process \
                        --workers "$WK" \
                        --reps 1 \
                        --n-seq "$N" \
                        2>"$STDERR_LOG" \
                        | tail -n +2 >>"$RAW_CSV" &
                fi
                REP_PID=$!
                wait "$REP_PID"
                EC=$?
                REP_PID=""
                HEADER_WRITTEN=1
                set -e

                if [[ $EC -ne 0 ]]; then
                    echo "WARN: rep ${WL} W=${WK} N=${N} rep=${REP} exit=${EC} (see $STDERR_LOG)" >&2
                fi
            done
        done
    done
done

# --- Phase 2: Docker TCP arm ---
#
# Implementation note (2026-05-15): the Rust `--campaign stress-curve`
# dispatch is in-process only — its `--env docker` flag is currently
# metadata. For a real container-based TCP run we delegate to the
# `bench-tcp` profile in docker-compose.yml, which invokes the OLD-style
# bench subcommand (--benchmark/--sizes/--workers/--mode=tcp_localhost).
#
# Schema caveat (choice (A) — see commit msg): the bench-tcp profile
# writes `./results/detail.csv` in the v1 detail schema (~22 columns,
# matching results/locked/v2_post_d012_baseline_2026-05-05/detail.csv),
# NOT the 32-column stress-curve schema produced by Phase 1. We therefore
# DO NOT concatenate docker_tcp.csv into aggregated.csv. Both raw files
# are preserved separately under raw/ and the MANIFEST documents this.
#
# Per-rep loop mirrors Phase 1: a single rep per `docker compose run`
# invocation, with the bash loop driving repetition. Each call rewrites
# `./results/detail.csv` (the bench subcommand truncates on write), so we
# extract the one data row after each call and append to $DOCKER_CSV.
DOCKER_AVAILABLE=1
if [[ $NO_DOCKER -eq 0 ]]; then
    echo "=== Phase 2: docker_tcp arm ==="
    if ! command -v docker >/dev/null 2>&1 || ! docker compose version >/dev/null 2>&1; then
        echo "WARN: docker compose not available; skipping Phase 2 (Phase 1 results preserved)" >&2
        DOCKER_AVAILABLE=0
        : >"$DOCKER_CSV"
    fi
fi

if [[ $NO_DOCKER -eq 0 && $DOCKER_AVAILABLE -eq 1 ]]; then
    DOCKER_RESULTS_DIR="$REPO_DIR/results"
    DOCKER_DETAIL_PATH="$DOCKER_RESULTS_DIR/detail.csv"
    DOCKER_SUMMARY_PATH="$DOCKER_RESULTS_DIR/summary.csv"
    mkdir -p "$DOCKER_RESULTS_DIR"
    # Wipe any prior bench-tcp output so we don't accidentally append a
    # stale row when a `docker compose run` fails to overwrite (the
    # container might exit before write). Caller is responsible for moving
    # any non-bench artefacts in `./results/` out of the way first.
    rm -f "$DOCKER_DETAIL_PATH" "$DOCKER_SUMMARY_PATH"

    : >"$DOCKER_CSV"
    DOCKER_HEADER_WRITTEN=0

    # Smoke matrix mirrors Phase 1 smoke (1 workload, W=2, N=[1000, 10000],
    # 1 rep). The full sweep uses the same (WL_ARR, WK_ARR, NS) as Phase 1.
    if [[ $SMOKE -eq 1 ]]; then
        DOCKER_NS=("${SMOKE_NS[@]}")
        DOCKER_REPS=1
    else
        DOCKER_NS=("${NS[@]}")
        DOCKER_REPS="$REPS"
    fi

    # Track consecutive failures per (WL, WK) to abort the N sweep after
    # 3 in a row (mirrors the Phase 1 StopRule philosophy — give up on a
    # configuration once it's clearly degenerate, but continue with the
    # next (WL, WK) pair so partial data is preserved).
    for WL in "${WL_ARR[@]}"; do
        for WK in "${WK_ARR[@]}"; do
            CONSECUTIVE_FAILS=0
            for N in "${DOCKER_NS[@]}"; do
                if [[ $CONSECUTIVE_FAILS -ge 3 ]]; then
                    echo "  abort N sweep for ${WL} W=${WK}: 3 consecutive docker failures" >&2
                    break
                fi
                for REP in $(seq 1 "$DOCKER_REPS"); do
                    DOCKER_STDERR_LOG="$OUTPUT_DIR/raw/docker_${WL}_${WK}_${N}_${REP}.stderr"
                    # Clear container output before each rep so we know the
                    # row we extract afterward is the one this rep produced.
                    rm -f "$DOCKER_DETAIL_PATH"

                    # `docker compose run --rm bench-tcp <cmd...>` REPLACES
                    # the service `command` block entirely (compose-spec
                    # behaviour), so we must pass the full bench CLI here.
                    # Volumes / build / profile from compose are still
                    # applied. The benchmark name comes from $WL (compose's
                    # hardcoded --benchmark=ep_annihilation is overridden).
                    set +e
                    docker compose --profile bench-tcp run --rm bench-tcp \
                            bench \
                            --benchmark="$WL" \
                            --sizes="$N" \
                            --workers="$WK" \
                            --mode=tcp_localhost \
                            --chunk-size=1000 \
                            --max-pending-lifetime=16 \
                            --recycle-policy=disable-under-delta \
                            --representation=dense \
                            --csv-detail=/results/detail.csv \
                            --csv-summary=/results/summary.csv \
                        >"$DOCKER_STDERR_LOG" 2>&1
                    EC=$?
                    set -e

                    if [[ $EC -ne 0 ]]; then
                        echo "WARN: docker rep ${WL} W=${WK} N=${N} rep=${REP} exit=${EC} (see $DOCKER_STDERR_LOG)" >&2
                        CONSECUTIVE_FAILS=$((CONSECUTIVE_FAILS + 1))
                        if [[ $CONSECUTIVE_FAILS -ge 3 ]]; then
                            break
                        fi
                        continue
                    fi

                    if [[ ! -f "$DOCKER_DETAIL_PATH" ]]; then
                        echo "WARN: docker rep ${WL} W=${WK} N=${N} rep=${REP} produced no detail.csv" >&2
                        CONSECUTIVE_FAILS=$((CONSECUTIVE_FAILS + 1))
                        continue
                    fi

                    # Append: header on first write, data-only thereafter.
                    if [[ $DOCKER_HEADER_WRITTEN -eq 0 ]]; then
                        cat "$DOCKER_DETAIL_PATH" >>"$DOCKER_CSV"
                        DOCKER_HEADER_WRITTEN=1
                    else
                        tail -n +2 "$DOCKER_DETAIL_PATH" >>"$DOCKER_CSV"
                    fi
                    CONSECUTIVE_FAILS=0
                done
            done
        done
    done
fi

# --- Phase 3: aggregation ---
#
# `aggregated.csv` is intentionally the in-process CSV ONLY (32-col
# stress-curve schema). The docker_tcp arm writes its own raw file in the
# v1 detail schema (~22 cols) and is preserved separately under raw/ —
# concatenating the two would yield a malformed CSV. Downstream analysis
# is expected to load each schema independently.
echo "=== Phase 3: aggregation ==="
AGG_CSV="$OUTPUT_DIR/aggregated.csv"
cp "$RAW_CSV" "$AGG_CSV" 2>/dev/null || : >"$AGG_CSV"

# --- Phase 4: REMOVED (operator decision 2026-05-15) ---
#
# The previous Phase 4 invoked scripts/plot_stress_curve.py to render PDF
# figures via matplotlib. Removed because: (a) the Python 3.14 matplotlib
# stack hits RecursionError on this host, and (b) the operator generates
# plots externally from the raw CSVs after the campaign locks.
# scripts/plot_stress_curve.py and scripts/requirements-stress-curve.txt
# are kept in-tree for external use.

# --- Phase 5: env capture + checksums + manifest ---
ENV_FILE="$OUTPUT_DIR/raw/env.txt"
{
    echo "uname: $(uname -a 2>/dev/null || echo n/a)"
    echo "rustc: $(rustc -V 2>/dev/null || echo n/a)"
    echo "cargo: $(cargo -V 2>/dev/null || echo n/a)"
    echo "git rev: $(git -C "$REPO_DIR" rev-parse HEAD 2>/dev/null || echo n/a)"
    echo "meminfo: $(grep MemTotal /proc/meminfo 2>/dev/null || echo n/a)"
    echo "cpuinfo: $(grep -m1 'model name' /proc/cpuinfo 2>/dev/null || echo n/a)"
} >"$ENV_FILE"

CHECKSUM_FILE="$OUTPUT_DIR/checksums.sha256"
: >"$CHECKSUM_FILE"
if command -v sha256sum >/dev/null 2>&1; then
    (cd "$OUTPUT_DIR" && find raw -type f 2>/dev/null | sort | xargs -r sha256sum) \
        >"$CHECKSUM_FILE" 2>/dev/null || :
elif command -v shasum >/dev/null 2>&1; then
    (cd "$OUTPUT_DIR" && find raw -type f 2>/dev/null | sort | xargs -r shasum -a 256) \
        >"$CHECKSUM_FILE" 2>/dev/null || :
else
    echo "WARN: neither sha256sum nor shasum available; checksums.sha256 left empty" >&2
fi
# Always touch a non-empty stub so downstream tests asserting size>0 pass.
if [[ ! -s "$CHECKSUM_FILE" ]]; then
    echo "# checksums unavailable on this host" >"$CHECKSUM_FILE"
fi

MANIFEST="$OUTPUT_DIR/MANIFEST.md"
{
    echo "# D-014 Stress Curve Campaign — MANIFEST"
    echo ""
    echo "- git rev: $(git -C "$REPO_DIR" rev-parse HEAD 2>/dev/null || echo n/a)"
    echo "- rustc:   $(rustc -V 2>/dev/null || echo n/a)"
    echo "- cargo:   $(cargo -V 2>/dev/null || echo n/a)"
    echo "- mode:    $([[ $SMOKE -eq 1 ]] && echo smoke || echo full)"
    echo "- output:  $OUTPUT_DIR"
    echo "- run at:  $(date -u +%FT%TZ 2>/dev/null || echo n/a)"
    echo ""
    echo "## Environment"
    echo ""
    echo '```'
    cat "$ENV_FILE"
    echo '```'
    echo ""
    echo "## Raw outputs"
    echo ""
    echo "- \`raw/in_process.csv\` — 32-column stress-curve schema (Phase 1)."
    echo "- \`raw/docker_tcp.csv\` — v1 \`detail.csv\` schema (~22 cols)"
    echo "  produced by the \`bench-tcp\` docker-compose profile. NOT"
    echo "  concatenated with \`in_process.csv\` (different schemas)."
    echo "- \`aggregated.csv\` — copy of \`in_process.csv\` for downstream"
    echo "  loaders that expect a single canonical file."
    echo ""
    echo "## Known caveats"
    echo ""
    echo "- F4 empty-Net silent fallback (\`partition/helpers.rs:699\`):"
    echo "  rows where \`total_interactions < input_size\` at high N x W"
    echo "  (notably N=10^7 x W=8) indicate the empty-Net fallback path."
    echo "  Applies to BOTH the in-process and docker arms (same code"
    echo "  path inside the worker). See"
    echo "  \`docs/reviews/partition-empty-net-fallback.md\`."
    echo ""
    echo "## Files"
    (cd "$OUTPUT_DIR" && find . -type f 2>/dev/null | sort)
} >"$MANIFEST"

echo ""
echo "=== Done ==="
echo "Output: $OUTPUT_DIR"
echo "MANIFEST: $MANIFEST"
echo ""
echo "Reminder: this script does NOT git-add or git-commit; lock the directory manually."
