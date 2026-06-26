#!/usr/bin/env bash
#
# horner_demo.sh — D-016 + TASK-0727 reproducer for the HornerCodec demo set.
#
# Runs every Horner demo from the working envelope (post BUG-001 fix) through
# TWO arms:
#   - in-process: target/release/relativist[.exe] compute --encoder horner ...
#   - docker:     docker compose --profile bench-tcp run --rm bench-tcp
#                 compute --encoder horner ... (ENTRYPOINT=relativist)
# and cross-checks G1 (same value across arms, every workers count).
#
# Each demo is also swept across a configurable list of worker counts
# (default: 1,2,4,8). Wall-clock, interactions, value, and per-row G1
# match are emitted as CSV to results/horner_demo_YYYY-MM-DD.csv (or a
# user-supplied path).
#
# Usage (from repo root):
#   cargo build --release --bin relativist
#   bash scripts/horner_demo.sh                       # in-process + docker, workers 1,2,4,8
#   bash scripts/horner_demo.sh --in-process-only     # legacy (no docker)
#   bash scripts/horner_demo.sh --docker              # docker-only
#   bash scripts/horner_demo.sh --workers 1,2         # custom worker sweep
#   bash scripts/horner_demo.sh --csv path/to.csv     # custom CSV output path
#
# Exit code: 0 on full success (all expected values match AND every G1 cross-arm
# pair agrees), non-zero with a summary line on any mismatch or failure.
#
# Notes on the Docker arm:
#   * The bench-tcp service has ENTRYPOINT=/usr/local/bin/relativist (see
#     Dockerfile), so the override args after `run --rm bench-tcp` are passed
#     directly to the binary (replacing the default `bench ...` command).
#   * MSYS_NO_PATHCONV=1 + MSYS2_ARG_CONV_EXCL='*' guard against Git-Bash on
#     Windows mangling the JSON `--input` string (D-014 Phase 2 lesson).
#   * The credential helper used by `docker compose build` lives outside the
#     MSYS2 PATH on Windows; if a rebuild is required, prepend
#     /c/Program\ Files/Docker/Docker/resources/bin to PATH first.
#
# Working envelope (post D-016 BUG-001 fix, decoder returns Err outside):
#   * Single-iter (coeffs.len() == 2): [c0, c1] with c0 in [0,10000] and
#     c1 in [0,1025]; x in [0,10000].
#   * Degree-2 (coeffs.len() == 3): [c0, c1, 1] (c2 must be 1) with c0 in
#     [0,10000]; x in [0,10000].
#   * Constants (coeffs.len() == 1): trivially safe.
#
# All ten demos below were hand-verified against the in-process binary on
# v2-development @ 0bf67e2 before this script was committed.

set -euo pipefail

# ----------------------------------------------------------------------------
# Constants and CLI parsing
# ----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# Pick up either ELF or .exe (Git-Bash on Windows ships both possible names).
if [[ -x "${REPO_ROOT}/target/release/relativist.exe" ]]; then
    RELATIVIST="${REPO_ROOT}/target/release/relativist.exe"
elif [[ -x "${REPO_ROOT}/target/release/relativist" ]]; then
    RELATIVIST="${REPO_ROOT}/target/release/relativist"
else
    echo "ERROR: relativist binary not found under ${REPO_ROOT}/target/release/" >&2
    echo "       Run: cargo build --release --bin relativist" >&2
    exit 2
fi

# Locate `docker` (Git-Bash on Windows does not put it on PATH by default).
DOCKER_BIN="${DOCKER_BIN:-}"
if [[ -z "${DOCKER_BIN}" ]]; then
    if command -v docker >/dev/null 2>&1; then
        DOCKER_BIN="$(command -v docker)"
    elif [[ -x "/c/Program Files/Docker/Docker/resources/bin/docker.exe" ]]; then
        DOCKER_BIN="/c/Program Files/Docker/Docker/resources/bin/docker.exe"
    fi
fi

WORKERS_LIST="1,2,4,8"
MODE="both"   # both | in-process-only | docker-only
CSV_OUT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --docker)
            MODE="docker-only"
            shift
            ;;
        --in-process-only)
            MODE="in-process-only"
            shift
            ;;
        --workers)
            WORKERS_LIST="$2"
            shift 2
            ;;
        --workers=*)
            WORKERS_LIST="${1#*=}"
            shift
            ;;
        --csv)
            CSV_OUT="$2"
            shift 2
            ;;
        --csv=*)
            CSV_OUT="${1#*=}"
            shift
            ;;
        -h|--help)
            sed -n '2,40p' "${BASH_SOURCE[0]}" | sed 's/^# \?//'
            exit 0
            ;;
        *)
            echo "ERROR: unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

if [[ -z "${CSV_OUT}" ]]; then
    mkdir -p "${REPO_ROOT}/reproduce_article/results"
    CSV_OUT="${REPO_ROOT}/reproduce_article/results/horner_demo_$(date -I).csv"
fi

# Validate Docker availability when needed.
if [[ "${MODE}" != "in-process-only" ]]; then
    if [[ -z "${DOCKER_BIN}" ]]; then
        echo "ERROR: docker not found on PATH and DOCKER_BIN not set." >&2
        echo "       Set DOCKER_BIN=/path/to/docker.exe or pass --in-process-only." >&2
        exit 2
    fi
    if ! MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' "${DOCKER_BIN}" compose version >/dev/null 2>&1; then
        echo "ERROR: docker compose unavailable via ${DOCKER_BIN}." >&2
        exit 2
    fi
fi

# ----------------------------------------------------------------------------
# Demo set — (label, JSON input, expected value)
#
# Selection rationale (TASK-0727): bias toward inputs that drive more
# interactions (degree-2 + max-scale cofactors) so the Docker arm exercises
# a non-trivial reduction load, not just a degenerate constant.
# ----------------------------------------------------------------------------

declare -a DEMOS=(
    "Demo01_const_baseline|{\"coeffs\":[42],\"x\":99}|42"
    "Demo02_lin_baseline|{\"coeffs\":[1,1],\"x\":5}|6"
    "Demo03_lin_mid_scale|{\"coeffs\":[100,1],\"x\":50}|150"
    "Demo04_lin_max_x|{\"coeffs\":[42,1],\"x\":10000}|10042"
    "Demo05_single_iter_c1_max|{\"coeffs\":[1,1025],\"x\":10000}|10250001"
    "Demo06_single_iter_c0c1_max|{\"coeffs\":[10000,1025],\"x\":10000}|10260000"
    "Demo07_deg2_baseline|{\"coeffs\":[1,1,1],\"x\":2}|7"
    "Demo08_deg2_zero_mid|{\"coeffs\":[1,0,1],\"x\":3}|10"
    "Demo09_lin_c1_nontrivial|{\"coeffs\":[3,5],\"x\":4}|23"
    "Demo10_deg2_max_scale|{\"coeffs\":[10000,500,1],\"x\":100}|70000"
)

# ----------------------------------------------------------------------------
# Helpers
# ----------------------------------------------------------------------------

# Parse one block of `relativist compute ...` stdout/stderr into three CSV
# fields: wall_secs, interactions, value. On parse failure each field is set
# to "NA".
#
# Inputs: $1 = combined stdout+stderr text
# Output via globals: PARSE_WALL, PARSE_INTERACTIONS, PARSE_VALUE
parse_compute_output() {
    local text="$1"
    # Reduction:   1220 interactions in 0.00s (11.37 MIPS)
    local line
    line="$(printf '%s\n' "${text}" | grep -E '^Reduction:' || true)"
    if [[ -n "${line}" ]]; then
        # Pull "<num> interactions" and "in <secs>s"
        PARSE_INTERACTIONS="$(printf '%s' "${line}" | sed -nE 's/.*[[:space:]]([0-9]+)[[:space:]]+interactions.*/\1/p')"
        PARSE_WALL="$(printf '%s' "${line}" | sed -nE 's/.*in[[:space:]]+([0-9.]+)s.*/\1/p')"
    else
        PARSE_INTERACTIONS="NA"
        PARSE_WALL="NA"
    fi
    if [[ -z "${PARSE_INTERACTIONS}" ]]; then PARSE_INTERACTIONS="NA"; fi
    if [[ -z "${PARSE_WALL}" ]]; then PARSE_WALL="NA"; fi

    # "value": "<digits>"
    PARSE_VALUE="$(printf '%s\n' "${text}" | sed -nE 's/.*"value":[[:space:]]*"([0-9]+)".*/\1/p' | head -n1)"
    if [[ -z "${PARSE_VALUE}" ]]; then PARSE_VALUE="NA"; fi
    return 0
}

# Run the in-process arm. Echoes nothing; sets PARSE_* globals + RUN_RC.
run_in_process() {
    local json="$1"
    local workers="$2"
    local output rc
    set +e
    if [[ "${workers}" -ge 1 ]]; then
        output="$(timeout 60 "${RELATIVIST}" compute --encoder horner --input "${json}" --workers "${workers}" 2>&1)"
    else
        output="$(timeout 60 "${RELATIVIST}" compute --encoder horner --input "${json}" 2>&1)"
    fi
    rc=$?
    set -e
    RUN_RC="${rc}"
    RUN_RAW="${output}"
    parse_compute_output "${output}"
}

# Run the docker arm. Echoes nothing; sets PARSE_* globals + RUN_RC.
#
# Uses `docker compose --profile bench-tcp run --rm bench-tcp <args>`: the
# bench-tcp service `command:` block in docker-compose.yml is REPLACED by
# these args (CLI override semantics). ENTRYPOINT is `relativist`, so args
# start with `compute`.
run_docker() {
    local json="$1"
    local workers="$2"
    local output rc
    set +e
    output="$(MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
        timeout 120 "${DOCKER_BIN}" compose --profile bench-tcp run --rm bench-tcp \
        compute --encoder horner --input "${json}" --workers "${workers}" 2>&1)"
    rc=$?
    set -e
    RUN_RC="${rc}"
    RUN_RAW="${output}"
    parse_compute_output "${output}"
}

# ----------------------------------------------------------------------------
# Main sweep
# ----------------------------------------------------------------------------

IFS=',' read -r -a WORKERS_ARR <<< "${WORKERS_LIST}"

# Resolve which arms to run.
ARMS=()
if [[ "${MODE}" != "docker-only"     ]]; then ARMS+=("in-process"); fi
if [[ "${MODE}" != "in-process-only" ]]; then ARMS+=("docker");     fi

echo "=========================================="
echo "horner_demo.sh — TASK-0727 multi-arm sweep"
echo "=========================================="
echo "Demos:      ${#DEMOS[@]}"
echo "Workers:    ${WORKERS_LIST}"
echo "Arms:       ${ARMS[*]}"
echo "CSV out:    ${CSV_OUT}"
echo "Binary:     ${RELATIVIST}"
if [[ "${MODE}" != "in-process-only" ]]; then
    echo "Docker:     ${DOCKER_BIN}"
fi
echo

# Write CSV header.
echo "demo,input,expected,workers,arm,rc,wall_secs,interactions,value,value_ok,g1_match" > "${CSV_OUT}"

TOTAL=0
FAIL_VALUE=0          # arm produced wrong (or NA) value vs expected
FAIL_G1=0             # in-process != docker for the same (demo, workers)
FAIL_RUN=0            # subprocess rc != 0
FAILED_ROWS=()

# Accumulator for G1 cross-check. For each (demo, workers) we record the
# in-process value (if that arm ran) and compare against the docker value
# (if that arm ran). We emit g1_match=true/false on the docker row; for
# rows where only one arm ran, g1_match=NA.
declare -A IP_VALUE

for entry in "${DEMOS[@]}"; do
    label="${entry%%|*}"
    rest="${entry#*|}"
    json="${rest%|*}"
    expected="${rest##*|}"

    for workers in "${WORKERS_ARR[@]}"; do
        for arm in "${ARMS[@]}"; do
            TOTAL=$((TOTAL + 1))

            case "${arm}" in
                in-process) run_in_process "${json}" "${workers}" ;;
                docker)     run_docker     "${json}" "${workers}" ;;
            esac

            # Validate rc.
            rc_ok="true"
            if [[ "${RUN_RC}" -ne 0 ]]; then
                rc_ok="false"
                FAIL_RUN=$((FAIL_RUN + 1))
            fi

            # Validate value against expected.
            value_ok="false"
            if [[ "${PARSE_VALUE}" == "${expected}" ]]; then
                value_ok="true"
            else
                FAIL_VALUE=$((FAIL_VALUE + 1))
                FAILED_ROWS+=("${label} workers=${workers} arm=${arm} value=${PARSE_VALUE} expected=${expected}")
            fi

            # G1 cross-arm bookkeeping.
            g1_match="NA"
            if [[ "${arm}" == "in-process" ]]; then
                IP_VALUE["${label}|${workers}"]="${PARSE_VALUE}"
            elif [[ "${arm}" == "docker" ]]; then
                ip_val="${IP_VALUE["${label}|${workers}"]:-}"
                if [[ -n "${ip_val}" ]]; then
                    if [[ "${ip_val}" == "${PARSE_VALUE}" ]]; then
                        g1_match="true"
                    else
                        g1_match="false"
                        FAIL_G1=$((FAIL_G1 + 1))
                        FAILED_ROWS+=("G1-MISMATCH ${label} workers=${workers}: in-process=${ip_val} docker=${PARSE_VALUE}")
                    fi
                fi
            fi

            # Emit CSV row (quote input to keep commas inside JSON safe).
            printf '%s,"%s",%s,%s,%s,%s,%s,%s,%s,%s,%s\n' \
                "${label}" "${json}" "${expected}" "${workers}" "${arm}" \
                "${RUN_RC}" "${PARSE_WALL}" "${PARSE_INTERACTIONS}" \
                "${PARSE_VALUE}" "${value_ok}" "${g1_match}" >> "${CSV_OUT}"

            # Per-row human progress line.
            printf '[%s] %-32s workers=%-2s arm=%-10s rc=%s wall=%-6s int=%-7s value=%-12s ok=%s g1=%s\n' \
                "${arm}" "${label}" "${workers}" "${arm}" \
                "${RUN_RC}" "${PARSE_WALL}" "${PARSE_INTERACTIONS}" \
                "${PARSE_VALUE}" "${value_ok}" "${g1_match}"

            if [[ "${rc_ok}" != "true" ]]; then
                echo "  raw output (last 10 lines):"
                printf '%s\n' "${RUN_RAW}" | tail -n 10 | sed 's/^/    /'
            fi
        done
    done
done

echo
echo "=========================================="
echo "Summary"
echo "=========================================="
echo "Rows run:        ${TOTAL}"
echo "rc != 0:         ${FAIL_RUN}"
echo "value mismatch:  ${FAIL_VALUE}"
echo "G1 mismatch:     ${FAIL_G1}"
echo "CSV:             ${CSV_OUT}"
echo "=========================================="

if (( FAIL_RUN > 0 || FAIL_VALUE > 0 || FAIL_G1 > 0 )); then
    echo
    echo "Failed rows:"
    for r in "${FAILED_ROWS[@]}"; do
        echo "  - ${r}"
    done
    exit 1
fi
exit 0
