# TASK-0730: `scripts/horner_distributed_demo.sh` — N-container Horner orchestrator

**Spec:** none (operator tooling; relies on SPEC-07 coordinator/worker + SPEC-12 binary format)
**Bundle:** D-017
**Priority:** P0 (the user-visible deliverable)
**Status:** TODO
**Depends on:** TASK-0728 (encode-only), TASK-0729 (decode subcommand)
**Blocked by:** TASK-0728, TASK-0729
**Estimated complexity:** M (~150–200 LoC bash)

## Context

The current "live demo" (`scripts/horner_live_demo.sh`) runs `docker compose --profile bench-tcp run --rm bench-tcp compute --codec horner --workers N` — a **single container** with N threads internally (the in-process distributed path via `compute --workers`). The operator wants to see **N separate worker containers** each with its own persisted log, so the audience can `docker logs relativist-worker-1` and watch the BSP cycle in any of them post-hoc.

The infrastructure already exists in `docker-compose.yml` (`coordinator` + `worker` services, lines 1–33). The coordinator loads `/data/input.bin`, the workers connect via `coordinator:9000`, and `deploy.replicas: ${NUM_WORKERS:-2}` scales the worker pool. The only missing pieces are:
1. Pre-stage encode (TASK-0728) → place `input.bin` in the bind-mounted `./data/`.
2. Post-stage decode (TASK-0729) → read `output.bin` and print the numeric result.

## Acceptance Criteria

- [ ] `bash scripts/horner_distributed_demo.sh` runs end-to-end with default polynomial and exits 0.
- [ ] Supports `--workers N` (default 4), `--input <JSON>` (default matches `horner_live_demo.sh`), `--keep-running` (skip `docker compose stop`).
- [ ] Pre-flight checks: `docker` in PATH, `docker compose version` succeeds, `target/release/relativist[.exe]` exists.
- [ ] Pipeline order: (1) `relativist compute --codec horner --input <JSON> --encode-only --output ./data/horner_input.bin`; (2) `docker compose up -d coordinator` (background); (3) `NUM_WORKERS=N docker compose up -d --scale worker=N worker` (background); (4) **wait** for coordinator process to exit (poll via `docker compose ps` or `wait` semantics — see Notes); (5) `relativist decode --codec horner --input ./data/horner_output.bin` (printed to stdout).
- [ ] Sets `MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'` for every `docker compose` call (D-014 Phase 2 lesson).
- [ ] On success, prints a "Inspect logs:" footer with the exact `docker logs <container>` commands the operator can run for each worker + the coordinator.
- [ ] On exit (success or fail), calls `docker compose stop` (NOT `down`) — containers are kept around for log inspection. `--keep-running` skips this so live audiences can `docker logs -f` mid-talk.
- [ ] Non-zero exit on: encode failure, coordinator timeout, worker startup failure, missing output.bin, decode mismatch vs. an in-process reference run (compute the reference once at top, assert string-equal).

## Files to Create/Modify

- `scripts/horner_distributed_demo.sh` — **new file**, executable (`chmod +x`).
- `data/.gitignore` — append `horner_input.bin` and `horner_output.bin` if not already covered (likely already wildcarded; verify).

## Key Behaviour Sketch

```bash
#!/usr/bin/env bash
set -euo pipefail

WORKERS="${WORKERS:-4}"
INPUT_JSON='{"coeffs":[10000,500,1],"x":100}'   # default = horner_live_demo.sh default
KEEP=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --workers) WORKERS="$2"; shift 2 ;;
        --input)   INPUT_JSON="$2"; shift 2 ;;
        --keep-running) KEEP=1; shift ;;
        *) echo "ERROR: unknown arg: $1" >&2; exit 2 ;;
    esac
done

cleanup() {
    if [[ "$KEEP" -eq 0 ]]; then
        MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' docker compose stop || true
    fi
}
trap cleanup EXIT

# Stage 1: encode (host)
"$RELATIVIST_BIN" compute --codec horner --input "$INPUT_JSON" \
    --encode-only --output ./data/horner_input.bin

# Stage 2a: reference (in-process) — to verify against the distributed result
REF_JSON="$("$RELATIVIST_BIN" compute --codec horner --input "$INPUT_JSON" | grep -A100 '^Result:' | tail -n +1)"

# Stage 2b: spin up coordinator (in-place: it reads /data/horner_input.bin)
#   NOTE: the coordinator service currently has `--input=/data/input.bin` hardcoded
#   in docker-compose.yml command:. We override via:
#     INPUT_PATH=/data/horner_input.bin OUTPUT_PATH=/data/horner_output.bin \
#     docker compose up -d coordinator
#   This requires TASK-0730 to ALSO patch docker-compose.yml `coordinator` command:
#   block to use `${INPUT_PATH:-/data/input.bin}` and `${OUTPUT_PATH:-/data/output.bin}`.
#   See "Coordinator compose env-var override" below.

NUM_WORKERS="$WORKERS" \
INPUT_PATH=/data/horner_input.bin \
OUTPUT_PATH=/data/horner_output.bin \
MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
    docker compose up -d coordinator

# Stage 3: scale workers
NUM_WORKERS="$WORKERS" \
MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
    docker compose up -d --scale worker="$WORKERS" worker

# Stage 4: wait for coordinator to finish (it exits when grid completes).
echo "Waiting for coordinator..."
MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
    docker compose wait coordinator    # exits with the coordinator's exit code

# Stage 5: decode (host)
RESULT_JSON="$("$RELATIVIST_BIN" decode --codec horner --input ./data/horner_output.bin)"
echo "$RESULT_JSON"

# Stage 6: cross-check vs reference
[[ "$RESULT_JSON" == "$REF_JSON" ]] || { echo "G1 MISMATCH"; exit 1; }

# Stage 7: footer
echo
echo "Inspect logs:"
echo "  docker logs relativist-coordinator-1"
for i in $(seq 1 "$WORKERS"); do
    echo "  docker logs relativist-worker-$i"
done
```

## Coordinator compose env-var override (part of this task)

The current `docker-compose.yml` hardcodes `--input=/data/input.bin` and `--output=/data/output.bin` in the `coordinator.command:` block. To let this script point at `/data/horner_input.bin` (avoiding name collision with stress-curve / other demos), patch:

```yaml
coordinator:
  command:
    - coordinator
    - --workers=${NUM_WORKERS:-2}
    - --bind=0.0.0.0:9000
    - --input=${INPUT_PATH:-/data/input.bin}     # NEW
    - --output=${OUTPUT_PATH:-/data/output.bin}  # NEW
    - --metrics=${METRICS_PATH:-/data/metrics.json}  # NEW (consistent)
    # ... chunk-size, max-pending-lifetime unchanged
```

This is **backwards-compatible** (defaults match the current literals). Confirm via `docker compose config` that the default-case rendering equals the current file.

## Test Expectations (for test-generator)

- Script lint: `shellcheck scripts/horner_distributed_demo.sh` (no errors; warnings OK).
- Integration test (Rust, ignored by default — requires Docker): TASK-0731 (separate).
- Smoke harness: a `dry-run` flag could be useful but is OUT OF SCOPE for this task; defer.

## Dependencies Context

- `data/` is bind-mounted into both services (`./data:/data`).
- `docker compose wait <service>` blocks until the container exits and propagates the exit code (Docker Compose v2 feature; verify available — fall back to a polling loop on `docker compose ps --format json` if not).
- `docker logs` is name-resolved via the project's auto-generated container name `relativist-<service>-<N>`. The project name defaults to the compose-file's parent directory name (`relativist`); confirm by running `docker compose ps` once during dev.

## Notes

- **Wait semantics**: `docker compose wait` is the cleanest primitive but is relatively new. If unavailable, poll `docker inspect -f '{{.State.Status}}' relativist-coordinator-1` until it equals `exited`, with a 10-minute timeout. Document the fallback in a script comment.
- **Race**: if workers don't start before the coordinator opens its socket, the coordinator's `--initial-wait-timeout=30` (default) gives them a 30-second window. With `up -d coordinator` followed immediately by `up -d --scale worker=N`, the worker containers usually attach within 2–3 seconds, so this is safe. If flakey, swap to `up -d --scale worker=N coordinator worker` (single call, lets Compose start coordinator first via `depends_on`).
- **`docker compose stop` vs `down`**: the operator wants `docker logs` to keep working post-demo. `stop` preserves the container; `down` removes it (and the logs). `--keep-running` is a stronger flag for live talks where the operator wants `docker logs -f` mid-narration.
- The "reference vs distributed" cross-check is the G1 confluence punchline (Lafont 1997) — same numerical result via two different reduction strategies. This is the same property `horner_demo.sh` checks today across in-process vs single-container; this task extends it to **distributed multi-container**.
