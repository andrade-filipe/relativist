#!/usr/bin/env bash
#
# horner_distributed_demo.sh — D-017 / TASK-0730
#
# End-to-end orchestrator for the multi-container Horner G1 demo:
#
#   1. Encode locally  : `relativist compute --codec horner --encode-only ...`
#   2. Spin up coordinator container (reads the encoded .bin from ./data/)
#   3. Scale to N worker containers (BSP over TCP)
#   4. Wait for coordinator to exit (full reduction)
#   5. Decode the reduced .bin locally
#   6. G1 cross-check vs in-process reference
#   7. Print docker logs invocations + `docker compose stop` (NOT down — keeps
#      containers around so the operator can inspect logs post-talk)
#
# Differs from `scripts/horner_live_demo.sh`: that script uses the
# `bench-tcp` profile which runs one container with N internal threads
# (in-process distributed via `compute --workers N`). This script uses the
# default profile with N separate worker containers, each with its own
# persistent log — what an audience asks for when they want to see "atrás
# das cortinas".
#
# Usage:
#   bash scripts/horner_distributed_demo.sh
#   bash scripts/horner_distributed_demo.sh --workers 2
#   bash scripts/horner_distributed_demo.sh --input '{"coeffs":[42],"x":7}'
#   bash scripts/horner_distributed_demo.sh --keep-running   # skip teardown
#
# Pre-flight:
#   * `docker ps` works (Docker Desktop running).
#   * `docker compose version` works.
#   * `cargo build --release --bin relativist` recent.
#
# Exit codes:
#   0  full pipeline OK + decoded value matches the in-process reference (G1).
#   1  precondition / arg failure.
#   2  encode failure.
#   3  docker compose up failure.
#   4  coordinator exited non-zero, or timed out before producing output.bin.
#   5  decode failure.
#   6  G1 mismatch: distributed value != in-process value (THIS IS THE BUG).
#
# Envelope (same rules as horner_live_demo.sh):
#   * Single-iter   (coeffs.len()==2): [c0, c1], c0 in [0,10000], c1 in [0,1025]
#   * Degree-2      (coeffs.len()==3): [c0, c1, 1] (c2 must be 1)
#   * Constants     (coeffs.len()==1): trivially safe

set -euo pipefail

# ----------------------------------------------------------------------------
# arg parsing
# ----------------------------------------------------------------------------

WORKERS="${WORKERS:-4}"
INPUT_JSON='{"coeffs":[10000,500,1],"x":100}'   # default = horner_live_demo.sh
KEEP=0
WAIT_TIMEOUT_SECS="${WAIT_TIMEOUT_SECS:-600}"   # 10 minutes default

while [[ $# -gt 0 ]]; do
    case "$1" in
        --workers)
            WORKERS="$2"
            shift 2
            ;;
        --input)
            INPUT_JSON="$2"
            shift 2
            ;;
        --keep-running|--keep)
            KEEP=1
            shift
            ;;
        --wait-timeout)
            WAIT_TIMEOUT_SECS="$2"
            shift 2
            ;;
        -h|--help)
            sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "ERROR: unknown arg: $1" >&2
            exit 1
            ;;
    esac
done

if ! [[ "$WORKERS" =~ ^[1-9][0-9]*$ ]]; then
    echo "ERROR: --workers must be a positive integer, got: $WORKERS" >&2
    exit 1
fi

# ----------------------------------------------------------------------------
# locate binary
# ----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

if [[ -x "$REPO_DIR/target/release/relativist.exe" ]]; then
    RELATIVIST_BIN="$REPO_DIR/target/release/relativist.exe"
elif [[ -x "$REPO_DIR/target/release/relativist" ]]; then
    RELATIVIST_BIN="$REPO_DIR/target/release/relativist"
else
    echo "ERROR: target/release/relativist not built. Run \`cargo build --release --bin relativist\` first." >&2
    exit 1
fi

# ----------------------------------------------------------------------------
# pre-flight
# ----------------------------------------------------------------------------

if ! command -v docker >/dev/null 2>&1; then
    echo "ERROR: docker not in PATH. Start Docker Desktop and retry." >&2
    exit 1
fi
if ! docker compose version >/dev/null 2>&1; then
    echo "ERROR: docker compose not available." >&2
    exit 1
fi

# ----------------------------------------------------------------------------
# paths (per-run, timestamped, to avoid collisions with concurrent demos)
# ----------------------------------------------------------------------------

TS="$(date +%Y%m%d_%H%M%S)"
mkdir -p "$REPO_DIR/data"
INPUT_REL="data/horner_input_${TS}.bin"
OUTPUT_REL="data/horner_output_${TS}.bin"
METRICS_REL="data/horner_metrics_${TS}.json"
INPUT_HOST="$REPO_DIR/$INPUT_REL"
OUTPUT_HOST="$REPO_DIR/$OUTPUT_REL"

INPUT_CONT="/data/horner_input_${TS}.bin"
OUTPUT_CONT="/data/horner_output_${TS}.bin"
METRICS_CONT="/data/horner_metrics_${TS}.json"

# ----------------------------------------------------------------------------
# cleanup trap — runs even on early exits / Ctrl-C
# ----------------------------------------------------------------------------

cleanup() {
    local rc=$?
    if [[ "$KEEP" -eq 0 ]]; then
        echo ""
        echo "[teardown] docker compose stop (containers preserved for log inspection)"
        MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
            docker compose stop >/dev/null 2>&1 || true
    else
        echo ""
        echo "[teardown] --keep-running set; leaving containers up. Stop manually with:"
        echo "  docker compose stop"
    fi
    exit $rc
}
trap cleanup EXIT INT TERM

# ----------------------------------------------------------------------------
# stage 1 — local encode (host)
# ----------------------------------------------------------------------------

echo "[1/6] Encoding locally: $INPUT_JSON  ->  $INPUT_REL"
if ! "$RELATIVIST_BIN" compute --codec horner \
    --input "$INPUT_JSON" \
    --encode-only \
    --output "$INPUT_HOST" >/dev/null; then
    echo "ERROR: local encode failed." >&2
    exit 2
fi
if [[ ! -s "$INPUT_HOST" ]]; then
    echo "ERROR: encoded .bin is empty: $INPUT_HOST" >&2
    exit 2
fi

# ----------------------------------------------------------------------------
# stage 2 — compute in-process reference (for G1 cross-check)
# ----------------------------------------------------------------------------

echo "[2/6] Computing in-process reference (for G1 cross-check)"
REF_FULL="$("$RELATIVIST_BIN" compute --codec horner --input "$INPUT_JSON")"
# Extract the JSON block printed by `compute` after `Result:`. The pretty-
# printed JSON spans multiple lines: opens with `{` on the same line as
# `Result:`, body is indented, closes with `}` at column 0. We track brace
# depth (incrementing on `{`, decrementing on `}`) and stop after the
# outer `}`.
REF_JSON="$(printf '%s\n' "$REF_FULL" | awk '
    BEGIN { depth=0; found=0 }
    /^Result:/ {
        line=$0
        sub(/^Result:[[:space:]]*/, "", line)
        # Count braces in this line.
        n=gsub(/\{/, "{", line); depth+=n
        n=gsub(/\}/, "}", line); depth-=n
        print line
        found=1
        if (depth==0) exit
        next
    }
    found {
        n=gsub(/\{/, "{", $0); depth+=n
        n=gsub(/\}/, "}", $0); depth-=n
        print
        if (depth==0) exit
    }
')"
if [[ -z "$REF_JSON" ]]; then
    echo "ERROR: could not extract in-process reference JSON from compute output:" >&2
    echo "$REF_FULL" >&2
    exit 2
fi

# ----------------------------------------------------------------------------
# stage 3 — bring up coordinator + workers
# ----------------------------------------------------------------------------

echo "[3/6] Bringing up coordinator (workers=$WORKERS) ..."
cd "$REPO_DIR"

# Override input/output/metrics via env vars — backwards-compat: defaults
# in docker-compose.yml restore the prior `/data/input.bin` etc. literals.
export NUM_WORKERS="$WORKERS"
export INPUT_PATH="$INPUT_CONT"
export OUTPUT_PATH="$OUTPUT_CONT"
export METRICS_PATH="$METRICS_CONT"

if ! MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
        docker compose up -d coordinator >/dev/null 2>&1; then
    echo "ERROR: docker compose up coordinator failed." >&2
    exit 3
fi

if ! MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
        docker compose up -d --scale "worker=$WORKERS" worker >/dev/null 2>&1; then
    echo "ERROR: docker compose up workers (scale=$WORKERS) failed." >&2
    exit 3
fi

# ----------------------------------------------------------------------------
# stage 4 — wait for coordinator to finish
# ----------------------------------------------------------------------------

echo "[4/6] Waiting for coordinator to finish (timeout: ${WAIT_TIMEOUT_SECS}s) ..."
COORD_CONTAINER=""
# Try the Compose v2 `wait` primitive first; fall back to a polling loop.
if MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
        timeout "$WAIT_TIMEOUT_SECS" docker compose wait coordinator >/dev/null 2>&1; then
    :
else
    # Fallback: find the coordinator container ID and poll its state.
    COORD_CONTAINER="$(MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
        docker compose ps -q coordinator | head -n1 || true)"
    if [[ -z "$COORD_CONTAINER" ]]; then
        echo "ERROR: could not locate coordinator container." >&2
        exit 4
    fi
    waited=0
    while : ; do
        state="$(docker inspect -f '{{.State.Status}}' "$COORD_CONTAINER" 2>/dev/null || echo unknown)"
        if [[ "$state" == "exited" ]]; then
            break
        fi
        if (( waited >= WAIT_TIMEOUT_SECS )); then
            echo "ERROR: coordinator did not exit within ${WAIT_TIMEOUT_SECS}s." >&2
            exit 4
        fi
        sleep 2
        waited=$((waited + 2))
    done
fi

# Confirm output exists.
if [[ ! -s "$OUTPUT_HOST" ]]; then
    echo "ERROR: coordinator did not produce output .bin at $OUTPUT_HOST" >&2
    exit 4
fi

# ----------------------------------------------------------------------------
# stage 5 — local decode
# ----------------------------------------------------------------------------

echo "[5/6] Decoding $OUTPUT_REL locally"
if ! DIST_JSON="$("$RELATIVIST_BIN" decode --codec horner --input "$OUTPUT_HOST")"; then
    echo "ERROR: local decode failed." >&2
    exit 5
fi
echo "$DIST_JSON"

# ----------------------------------------------------------------------------
# stage 6 — G1 cross-check
# ----------------------------------------------------------------------------

echo "[6/6] G1 cross-check: distributed vs in-process reference"
if [[ "$DIST_JSON" != "$REF_JSON" ]]; then
    echo "G1 MISMATCH (this is a BUG; please file an issue)" >&2
    echo "--- distributed ---" >&2
    echo "$DIST_JSON" >&2
    echo "--- in-process reference ---" >&2
    echo "$REF_JSON" >&2
    exit 6
fi
echo "G1 OK — distributed value matches in-process reference."

# ----------------------------------------------------------------------------
# footer — inspect logs
# ----------------------------------------------------------------------------

echo ""
echo "Inspect logs:"
echo "  docker logs relativist-coordinator-1"
for i in $(seq 1 "$WORKERS"); do
    echo "  docker logs relativist-worker-$i"
done

# trap will run cleanup() with rc=0 here
