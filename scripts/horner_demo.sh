#!/usr/bin/env bash
#
# horner_demo.sh — D-016 reproducer for the HornerCodec demo set.
#
# Runs every Horner demo from `docs/demos/horner-g1-demonstration.md`
# end-to-end via the in-process binary at `target/release/relativist`,
# greps the JSON result for the expected `"value"` field, and reports
# pass / fail per demo.
#
# Usage (from repo root):
#   cargo build --release --bin relativist
#   bash scripts/horner_demo.sh
#
# Exit code: 0 on full success, non-zero with a summary line on failure.
#
# TODO(D-017+): Docker arm. The TASK-0727 follow-up will add a parallel
# Docker invocation that reproduces the same demo set inside the
# container image used by `scripts/stress_curve.sh` Phase 2 (see commit
# c77d7fc for the Docker-arm pattern). Until then, this script is a
# Bash-only in-process reproducer.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RELATIVIST="${REPO_ROOT}/target/release/relativist"

if [[ ! -x "${RELATIVIST}" ]]; then
    echo "ERROR: ${RELATIVIST} not found. Run: cargo build --release --bin relativist" >&2
    exit 2
fi

# (label, JSON, expected value) tuples. Demos 1-7 are the original set
# from `docs/demos/horner-g1-demonstration.md`; Demos 8-10 are the
# inputs newly unlocked by D-016 (TASK-0723 + TASK-0724).
declare -a DEMOS=(
    "Demo1_const|{\"coeffs\":[42],\"x\":7}|42"
    "Demo2_lin_c1eq1|{\"coeffs\":[1,1],\"x\":2}|3"
    "Demo3_const_x_invariant|{\"coeffs\":[42],\"x\":99}|42"
    "Demo4_smallest_cofactor|{\"coeffs\":[1,2],\"x\":2}|5"
    "Demo5_demo2_unlocked|{\"coeffs\":[3,5],\"x\":4}|23"
    "Demo6_demo4_unlocked|{\"coeffs\":[1,1,1],\"x\":2}|7"
    "Demo7_demo5_unlocked|{\"coeffs\":[1,0,1],\"x\":3}|10"
    "Demo8_high_x|{\"coeffs\":[10,2],\"x\":10000}|20010"
    "Demo9_leading_zero|{\"coeffs\":[0,7],\"x\":3}|21"
    "Demo10_high_cofactor|{\"coeffs\":[3,1000],\"x\":2}|2003"
)

PASS=0
FAIL=0
FAILED_LABELS=()

for entry in "${DEMOS[@]}"; do
    label="${entry%%|*}"
    rest="${entry#*|}"
    json="${rest%|*}"
    expected="${rest##*|}"

    set +e
    output="$(timeout 30 "${RELATIVIST}" compute --codec horner --input "${json}" 2>&1)"
    rc=$?
    set -e

    if [[ ${rc} -ne 0 ]]; then
        echo "FAIL ${label}: relativist exited ${rc}"
        echo "  input: ${json}"
        echo "  output:"
        echo "${output}" | sed 's/^/    /'
        FAIL=$((FAIL + 1))
        FAILED_LABELS+=("${label}")
        continue
    fi

    # The result block contains: "value": "<expected>"
    if echo "${output}" | grep -q "\"value\": \"${expected}\""; then
        echo "PASS ${label}: value=${expected}"
        PASS=$((PASS + 1))
    else
        echo "FAIL ${label}: expected value=${expected} not found in output"
        echo "  input: ${json}"
        echo "  output:"
        echo "${output}" | sed 's/^/    /'
        FAIL=$((FAIL + 1))
        FAILED_LABELS+=("${label}")
    fi
done

echo
echo "=========================================="
echo "Summary: ${PASS} passed, ${FAIL} failed (out of ${#DEMOS[@]})"
echo "=========================================="

if [[ ${FAIL} -gt 0 ]]; then
    echo "Failed demos: ${FAILED_LABELS[*]}"
    exit 1
fi
exit 0
