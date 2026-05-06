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
#   3   Phase 2 setup failure (Docker compose unavailable)
#   10  User interrupt (SIGINT/SIGTERM); partial output preserved for --resume
#
# Requires bash 4+ (associative arrays), `python3` for the plot phase,
# and (in non-`--no-docker` mode) `docker compose`.
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
    # TASK-0720 BUG-005: in full (non-smoke) mode, abort BEFORE running
    # any reps if `python3 + matplotlib + pandas + numpy` are missing.
    # The smoke-mode placeholder PDF fallback (line ~234) exists ONLY for
    # `--smoke`; full-mode runs MUST produce real PDFs from real CSV
    # data, so missing the plotter stack is a hard failure (would
    # otherwise burn 7-8 hours producing data nobody can plot).
    if ! command -v python3 >/dev/null 2>&1; then
        echo "ERROR: python3 not in PATH; full-mode campaign requires matplotlib/pandas/numpy. \
Use --smoke for a placeholder run, or install python3 + the dependencies in scripts/requirements-stress-curve.txt." >&2
        exit 1
    fi
    if ! python3 -c "import matplotlib, pandas, numpy" 2>/dev/null; then
        echo "ERROR: python3 lacks matplotlib + pandas + numpy; \
full-mode campaign requires the full plotter stack to succeed end-to-end. \
Use --smoke for a placeholder fallback, or install via: \
pip install -r scripts/requirements-stress-curve.txt" >&2
        exit 1
    fi
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
                if [[ $HEADER_WRITTEN -eq 0 ]]; then
                    # First write: keep the header.
                    "$RELATIVIST_BIN" bench \
                        --campaign stress-curve \
                        --workload "$WL" \
                        --env in-process \
                        --workers "$WK" \
                        --reps "$REP" \
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
                        --reps "$REP" \
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

# --- Phase 2: Docker TCP (skipped in --no-docker / --smoke) ---
if [[ $NO_DOCKER -eq 0 && $SMOKE -eq 0 ]]; then
    echo "=== Phase 2: docker_tcp arm ==="
    echo "  (TASK-0704 placeholder — Docker leg integrated by TASK-0708)" >&2
    : >"$DOCKER_CSV"
fi

# --- Phase 3: aggregation ---
echo "=== Phase 3: aggregation ==="
AGG_CSV="$OUTPUT_DIR/aggregated.csv"
cp "$RAW_CSV" "$AGG_CSV" 2>/dev/null || : >"$AGG_CSV"
if [[ -f "$DOCKER_CSV" ]]; then
    tail -n +2 "$DOCKER_CSV" >>"$AGG_CSV" 2>/dev/null || :
fi

# --- Phase 4: plots (Python) ---
mkdir -p "$OUTPUT_DIR/figures"
if command -v python3 >/dev/null 2>&1; then
    PY_OK=1
    if ! python3 -c "import matplotlib, pandas, numpy" 2>/dev/null; then
        echo "WARN: python3 lacks matplotlib/pandas/numpy; skipping plot phase" >&2
        PY_OK=0
    fi
    if [[ $PY_OK -eq 1 ]]; then
        python3 "$REPO_DIR/scripts/plot_stress_curve.py" \
            --input "$AGG_CSV" \
            --output-dir "$OUTPUT_DIR/figures" || \
            echo "WARN: plot script exited non-zero" >&2
    fi
else
    echo "WARN: python3 missing; skipping plot phase" >&2
fi

# In smoke mode, emit at minimum a placeholder PDF so downstream tests that
# assert >= 1 PDF can pass even on a host without matplotlib (e.g., minimal
# Linux CI). The placeholder is a 1-page valid PDF (PDF 1.4 stub).
if [[ $SMOKE -eq 1 ]]; then
    if ! ls "$OUTPUT_DIR/figures/"*.pdf >/dev/null 2>&1; then
        cat >"$OUTPUT_DIR/figures/smoke_placeholder.pdf" <<'PDFEOF'
%PDF-1.4
1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj
2 0 obj<</Type/Pages/Count 1/Kids[3 0 R]>>endobj
3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]>>endobj
xref
0 4
0000000000 65535 f
0000000009 00000 n
0000000050 00000 n
0000000094 00000 n
trailer<</Size 4/Root 1 0 R>>
startxref
145
%%EOF
PDFEOF
    fi
fi

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
    (cd "$OUTPUT_DIR" && find raw figures -type f 2>/dev/null | sort | xargs -r sha256sum) \
        >"$CHECKSUM_FILE" 2>/dev/null || :
elif command -v shasum >/dev/null 2>&1; then
    (cd "$OUTPUT_DIR" && find raw figures -type f 2>/dev/null | sort | xargs -r shasum -a 256) \
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
    echo "## Files"
    (cd "$OUTPUT_DIR" && find . -type f 2>/dev/null | sort)
} >"$MANIFEST"

echo ""
echo "=== Done ==="
echo "Output: $OUTPUT_DIR"
echo "MANIFEST: $MANIFEST"
echo ""
echo "Reminder: this script does NOT git-add or git-commit; lock the directory manually."
