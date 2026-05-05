# `bench_docker_v2.sh` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Adapt `scripts/bench_docker.sh` v1 into `scripts/bench_docker_v2.sh` to run the D-011 Phase F-2 docker TCP rodada — same 8 configs as v1's frozen `phase2_*` baseline, with Tier 3 active in the coordinator path.

**Architecture:** Single bash + Python 3 script orchestrating `docker compose up coordinator worker --scale worker=N`. Phase A runs sequential baselines natively (outside docker). Phase B loops (config, workers, rep) running one docker cycle per measurement, parsing `metrics.json` into v1 22-column CSVs. Resume capability mandatory — re-invocation reads existing CSVs and skips completed `(bench, size, workers)` tuples with ≥10 reps. Output to `results/v2_tcp_baseline/`.

**Tech Stack:** Bash 4+, Python 3 (statistics, json), Docker Compose v2, `relativist` Rust binary (`coordinator`/`worker`/`generate`/`reduce`/`inspect` subcommands).

**Reference spec:** `docs/plans/2026-05-02-bench-docker-v2-tcp-rodada-design.md`

**Reference v1 script:** `scripts/bench_docker.sh` (520 lines, fully functional — produced `results/locked/v1_local_baseline/phase2_*.csv`)

---

## Task 1: Update `docker-compose.yml` to pass Tier 3 flags to coordinator

**Files:**
- Modify: `docker-compose.yml:6-18` (coordinator service `command:` block)

- [ ] **Step 1: Add `--chunk-size` and `--max-pending-lifetime` to coordinator command**

Edit `docker-compose.yml`. In the `coordinator:` service `command:` array, append two new entries after `--metrics=/data/metrics.json`:

```yaml
  coordinator:
    build: .
    command:
      - coordinator
      - --workers=${NUM_WORKERS:-2}
      - --bind=0.0.0.0:9000
      - --input=/data/input.bin
      - --output=/data/output.bin
      - --metrics=/data/metrics.json
      - --chunk-size=${CHUNK_SIZE:-10000}
      - --max-pending-lifetime=${MAX_PENDING_LIFETIME:-16}
    ports:
      - "9000:9000"
    volumes:
      - ./data:/data
```

- [ ] **Step 2: Verify compose file still parses**

Run:
```powershell
docker compose config --services
```
Expected: prints `coordinator`, `worker`, `bench-tcp`, `bench-tcp-eager` (one per line, no errors).

- [ ] **Step 3: Verify the new flags reach the coordinator**

Run:
```powershell
docker compose run --rm --entrypoint /bin/sh coordinator -c "echo 'noop'"
```
This should NOT actually start the coordinator (no `input.bin` exists yet). Expected: prints `noop`, exits 0. If it errors with "input file not found" — that means the `coordinator` subcommand was invoked, which is wrong (we asked for `/bin/sh -c noop`). Recheck the command override.

- [ ] **Step 4: Commit**

```bash
git add docker-compose.yml
git commit -m "chore(d-011): wire Tier 3 flags into compose coordinator service"
```

---

## Task 2: Pre-flight smoke #1 — verify coordinator+worker work together with Tier 3

**Goal:** Validate the docker-compose hybrid coordinator + N workers actually completes one reduction successfully before writing the orchestrator script. If this fails, the script can't possibly work.

**Files:** none modified. This is a manual validation.

- [ ] **Step 1: Generate a small test input**

Run:
```powershell
.\target\release\relativist.exe generate ep-annihilation -n 1000 -o data\input.bin
```
Expected: prints `=== Relativist Generate ===` and similar, exit 0. File `data\input.bin` should exist (~10-50 KB).

- [ ] **Step 2: Run coordinator + 2 workers via compose, abort-on-container-exit**

Run:
```powershell
Remove-Item .\data\output.bin, .\data\metrics.json -ErrorAction SilentlyContinue
$env:NUM_WORKERS=2
docker compose up coordinator worker --scale worker=2 --abort-on-container-exit --exit-code-from coordinator
```
Expected: workers connect, coordinator runs reduction, prints summary, all containers exit. Compose returns exit code 0.

- [ ] **Step 3: Verify outputs were produced**

Run:
```powershell
Get-ChildItem .\data\output.bin, .\data\metrics.json | Select-Object Name, Length
```
Expected: both files exist; `output.bin` is non-empty (~1-50 KB), `metrics.json` is ~1-10 KB.

- [ ] **Step 4: Verify `metrics.json` has the fields the script will parse**

Run:
```powershell
Get-Content .\data\metrics.json | python -c "import json, sys; m=json.load(sys.stdin); print('rounds:', m['rounds']); print('total_int:', m['total_interactions']); print('total_time keys:', list(m['total_time'].keys())); print('rules:', m['total_interactions_by_rule']); print('per_round arrays present:', all(k in m for k in ['partition_time_per_round','compute_time_per_round','merge_time_per_round','agents_per_round','bytes_sent_per_round','bytes_received_per_round']))"
```
Expected output:
```
rounds: <some int>
total_int: 1000
total_time keys: ['secs', 'nanos']
rules: [<6 ints>]
per_round arrays present: True
```

If any field is missing → STOP. The metrics.json schema changed v1→v2 in a way the parser won't handle. Adapt the parser before continuing.

- [ ] **Step 5: Cleanup**

Run:
```powershell
docker compose down --remove-orphans
```

---

## Task 3: Write `scripts/bench_docker_v2.sh` (full script, in one shot)

**Files:**
- Create: `scripts/bench_docker_v2.sh`

**Rationale:** the script is closely modeled on v1's `bench_docker.sh` (which is functional). Writing it function-by-function would leave intermediate states where the script doesn't run. We write the full ~700-line file, then validate via 4 smoke tests.

- [ ] **Step 1: Create the full script file**

Create `scripts/bench_docker_v2.sh` with the content below. **Every line shown is the actual content** — copy verbatim. The script:
1. Declares config (BENCHMARKS_AXIS1, WORKER_COUNTS, REPETITIONS, etc.)
2. Defines library functions (parsers, summary writer, G1, docker cycle, resume checkpoint)
3. Phase A: native sequential baselines
4. Phase B: docker TCP rodada with resume support
5. Final report

```bash
#!/usr/bin/env bash
# =============================================================================
# bench_docker_v2.sh — D-011 Phase F-2 docker TCP rodada (Axis 1)
# =============================================================================
#
# Adapts v1 bench_docker.sh for v2-development. Runs the same 8 configs as
# v1's frozen baseline (results/locked/v1_local_baseline/phase2_*.csv) with
# Tier 3 active in the coordinator path:
#   - --chunk-size=10000 (SPEC-21 R37)
#   - --max-pending-lifetime=16 (SPEC-21 R37g)
#   - free-list recycling (SPEC-22, runtime-resident)
#   - CompactSubnet wire fix (D-011 B-1)
#
# Usage:
#   bash scripts/bench_docker_v2.sh [--dry-run] [--skip-build] [--skip-sequential] [--no-resume]
#
# Resume: by default, on each invocation the script reads existing CSVs and
# skips (benchmark, size, workers) tuples that already have >=10 reps. Pass
# --no-resume to force re-run of all tuples (overwrites existing CSVs).
#
# Output:
#   results/v2_tcp_baseline/detail.csv      (22-col v1 schema)
#   results/v2_tcp_baseline/summary.csv     (16-col)
#   results/v2_tcp_baseline/rounds.csv      (15-col, per-round breakdown)
#   results/v2_tcp_baseline/per_rep_metrics/  (forensic dumps of metrics.json)
#   results/v2_tcp_baseline/seq_outputs/      (sequential reduction outputs for G1)
#   results/v2_tcp_baseline/run.log
#
# G1 verification: count-based via `relativist inspect`. v1-equivalent gate.
# Topological correctness independently validated by local rodada (360
# datapoints all_correct=true) and `cargo test` (1619 tests).
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
DATA_DIR="$REPO_DIR/data"
OUTPUT_DIR="$REPO_DIR/results/v2_tcp_baseline"
PER_REP_DIR="$OUTPUT_DIR/per_rep_metrics"
SEQ_OUT_DIR="$OUTPUT_DIR/seq_outputs"

# Find relativist binary
if [ -f "$REPO_DIR/target/release/relativist.exe" ]; then
    RELATIVIST="$REPO_DIR/target/release/relativist.exe"
elif [ -f "$REPO_DIR/target/release/relativist" ]; then
    RELATIVIST="$REPO_DIR/target/release/relativist"
elif command -v relativist &>/dev/null; then
    RELATIVIST="relativist"
else
    echo "ERROR: relativist binary not found. Run 'cargo build --release' first."
    exit 1
fi

# Axis 1 configs (mirror v1 phase2 docker rodada exactly)
BENCHMARKS_AXIS1=(
    "ep_annihilation_con:ep-annihilation-con:500000"
    "ep_annihilation_con:ep-annihilation-con:1000000"
    "ep_annihilation_con:ep-annihilation-con:5000000"
    "dual_tree:dual-tree:18"
    "dual_tree:dual-tree:20"
    "dual_tree:dual-tree:22"
    "condup_expansion:con-dup-expansion:1000"
    "condup_expansion:con-dup-expansion:5000"
)

WORKER_COUNTS=(1 2 4 8)
WARMUP_RUNS=2
REPETITIONS=10
TIMEOUT_SECS=600  # 10 minutes per docker cycle

# Flags
DRY_RUN=false
SKIP_BUILD=false
SKIP_SEQUENTIAL=false
RESUME=true  # default ON; disable with --no-resume

for arg in "$@"; do
    case "$arg" in
        --dry-run)         DRY_RUN=true ;;
        --skip-build)      SKIP_BUILD=true ;;
        --skip-sequential) SKIP_SEQUENTIAL=true ;;
        --no-resume)       RESUME=false ;;
        *) echo "Unknown arg: $arg"; exit 1 ;;
    esac
done

# ---------------------------------------------------------------------------
# CSV schema (22-column v1)
# ---------------------------------------------------------------------------

DETAIL_HEADER="benchmark,input_size,mode,workers,repetition,correct,wall_clock_secs,total_interactions,mips,rounds,speedup,efficiency,overhead_ratio,peak_memory_bytes,bytes_sent,bytes_received,con_con,dup_dup,era_era,con_dup,con_era,dup_era"
SUMMARY_HEADER="benchmark,input_size,mode,workers,repetitions,all_correct,wall_clock_mean,wall_clock_std,wall_clock_median,wall_clock_min,wall_clock_max,mips_mean,speedup_mean,efficiency_mean,overhead_ratio_mean,cv"
ROUNDS_HEADER="benchmark,input_size,workers,mode,repetition,round,partition_time_secs,compute_time_secs,merge_time_secs,network_time_secs,border_redexes,border_ratio,agents_at_start,bytes_sent,bytes_received"

DETAIL_FILE="$OUTPUT_DIR/detail.csv"
SUMMARY_FILE="$OUTPUT_DIR/summary.csv"
ROUNDS_FILE="$OUTPUT_DIR/rounds.csv"
LOG_FILE="$OUTPUT_DIR/run.log"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

log() { echo "[$(date +%H:%M:%S)] $*" | tee -a "$LOG_FILE"; }

# On Windows (Git Bash / MSYS2), convert paths for Python compatibility
winpath() {
    if command -v cygpath &>/dev/null; then
        cygpath -w "$1"
    else
        echo "$1"
    fi
}

# Join array elements with commas (for Python list literals)
join_comma() {
    local IFS=','
    echo "$*"
}

# Parse metrics.json → detail CSV row (22 cols, v1 schema)
parse_metrics_to_detail() {
    local metrics_file="$1"
    local bench_name="$2"
    local input_size="$3"
    local workers="$4"
    local rep="$5"
    local seq_baseline="$6"
    local is_correct="$7"

    local wpath
    wpath=$(winpath "$metrics_file")
    python3 << PYEOF
import json

with open(r"$wpath") as f:
    m = json.load(f)

rounds = m["rounds"]
total_int = m["total_interactions"]
total_secs = m["total_time"]["secs"] + m["total_time"]["nanos"] / 1e9
mips = total_int / total_secs / 1e6 if total_secs > 0 else 0.0

rules = m["total_interactions_by_rule"]
con_con, con_dup, con_era, dup_dup, dup_era, era_era = rules

seq_base = $seq_baseline
speedup = seq_base / total_secs if total_secs > 0 else 0.0
w = $workers
efficiency = speedup / w if w > 0 else speedup
overhead = max(0.0, 1.0 - efficiency)

bytes_s = sum(m.get("bytes_sent_per_round", []))
bytes_r = sum(m.get("bytes_received_per_round", []))

print(f"$bench_name,$input_size,tcp_localhost,$workers,$rep,$is_correct,{total_secs:.6f},{total_int},{mips:.3f},{rounds},{speedup:.4f},{efficiency:.4f},{overhead:.4f},0,{bytes_s},{bytes_r},{con_con},{dup_dup},{era_era},{con_dup},{con_era},{dup_era}")
PYEOF
}

# Parse metrics.json → rounds CSV rows (one per round)
parse_metrics_to_rounds() {
    local metrics_file="$1"
    local bench_name="$2"
    local input_size="$3"
    local workers="$4"
    local rep="$5"

    local wpath
    wpath=$(winpath "$metrics_file")
    python3 << PYEOF
import json

with open(r"$wpath") as f:
    m = json.load(f)

for r in range(m["rounds"]):
    def dur(field):
        vals = m.get(field, [])
        if r < len(vals):
            return vals[r]["secs"] + vals[r]["nanos"] / 1e9
        return 0.0

    def ival(field):
        vals = m.get(field, [])
        return vals[r] if r < len(vals) else 0

    part_t = dur("partition_time_per_round")
    comp_t = dur("compute_time_per_round")
    merge_t = dur("merge_time_per_round")
    net_t = dur("network_send_time_per_round") + dur("network_recv_time_per_round")

    b_redex = ival("border_redexes_per_round")
    agents = ival("agents_per_round")
    local_i = ival("local_interactions_per_round")
    border_i = ival("border_interactions_per_round")
    total_i = local_i + border_i
    b_ratio = border_i / total_i if total_i > 0 else 0.0

    bs = ival("bytes_sent_per_round")
    br = ival("bytes_received_per_round")

    print(f"$bench_name,$input_size,$workers,tcp_localhost,$rep,{r},{part_t:.6f},{comp_t:.6f},{merge_t:.6f},{net_t:.6f},{b_redex},{b_ratio:.6f},{agents},{bs},{br}")
PYEOF
}

# Extract just wall_clock_secs from metrics.json
extract_wall_clock() {
    local wpath
    wpath=$(winpath "$1")
    python3 -c "
import json
with open(r'$wpath') as f:
    m = json.load(f)
t = m['total_time']['secs'] + m['total_time']['nanos']/1e9
print(f'{t:.6f}')
"
}

# Compute summary stats from a list of wall_clock values
write_summary_row() {
    local bench_name="$1"
    local input_size="$2"
    local mode="$3"
    local workers="$4"
    local reps="$5"
    local all_correct="$6"
    local seq_baseline="$7"
    shift 7
    # remaining args are wall_clock values

    python3 << PYEOF
import statistics

values = [float(v) for v in "$*".split()]
n = len(values)
if n == 0:
    exit(0)

mean_v = statistics.mean(values)
std_v = statistics.pstdev(values) if n > 1 else 0.0
median_v = statistics.median(values)
min_v = min(values)
max_v = max(values)
cv = std_v / mean_v if mean_v > 0 else 0.0

seq_b = $seq_baseline
w = int("$workers")
mips = 0.0  # populated from detail rows; summary doesn't recompute
speedup = seq_b / mean_v if mean_v > 0 else 0.0
efficiency = speedup / w if w > 0 else speedup
overhead = max(0.0, 1.0 - efficiency)

print(f"$bench_name,$input_size,$mode,$workers,$reps,$all_correct,{mean_v:.6f},{std_v:.6f},{median_v:.6f},{min_v:.6f},{max_v:.6f},{mips:.3f},{speedup:.4f},{efficiency:.4f},{overhead:.4f},{cv:.4f}")
PYEOF
}

# G1 verification: compare Agents+Redexes counts via `relativist inspect`
# Returns "true" or "false" on stdout.
verify_g1() {
    local seq_output="$1"
    local dist_output="$2"

    if [ ! -f "$seq_output" ] || [ ! -f "$dist_output" ]; then
        echo "false"
        return
    fi

    local seq_info dist_info
    seq_info=$("$RELATIVIST" inspect -i "$seq_output" 2>/dev/null | grep -E "^(Agents|Redexes):" | tr '\n' '|' || echo "ERR")
    dist_info=$("$RELATIVIST" inspect -i "$dist_output" 2>/dev/null | grep -E "^(Agents|Redexes):" | tr '\n' '|' || echo "ERR2")

    if [ "$seq_info" = "$dist_info" ]; then
        echo "true"
    else
        echo "false"
    fi
}

# Run a single docker compose cycle. Returns docker exit code.
run_docker_cycle() {
    local workers="$1"
    local timeout="$2"

    # Clean previous state
    (cd "$REPO_DIR" && docker compose down --remove-orphans 2>/dev/null) || true

    # Run with abort-on-container-exit: stops all when coordinator exits
    local exit_code=0
    (cd "$REPO_DIR" && \
        NUM_WORKERS="$workers" \
        CHUNK_SIZE=10000 \
        MAX_PENDING_LIFETIME=16 \
        timeout "$timeout" \
        docker compose up coordinator worker \
            --abort-on-container-exit \
            --exit-code-from coordinator \
            --scale worker="$workers" \
            2>&1 | tail -30
    ) || exit_code=$?

    # Always clean up
    (cd "$REPO_DIR" && docker compose down --remove-orphans 2>/dev/null) || true

    return $exit_code
}

# Read existing detail.csv (if present) and emit "bench:size:mode:workers" lines
# for tuples with >=REPETITIONS distinct repetitions. Used for resume.
checkpoint_completed_set() {
    if [ ! -f "$DETAIL_FILE" ] || [ "$RESUME" = "false" ]; then
        return  # empty set
    fi
    python3 << PYEOF
import csv
from collections import defaultdict
counts = defaultdict(set)  # (bench, size, mode, workers) -> set of repetitions
with open(r"$(winpath "$DETAIL_FILE")", newline="") as f:
    rd = csv.DictReader(f)
    for row in rd:
        key = (row["benchmark"], row["input_size"], row["mode"], row["workers"])
        counts[key].add(row["repetition"])
threshold = $REPETITIONS
for key, reps in counts.items():
    if len(reps) >= threshold:
        print(":".join(key))
PYEOF
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
    mkdir -p "$DATA_DIR" "$OUTPUT_DIR" "$PER_REP_DIR" "$SEQ_OUT_DIR"

    # Initialize log file (append mode — survives resume)
    echo "" >> "$LOG_FILE"
    log "================================================================"
    log "=== bench_docker_v2.sh — D-011 Phase F-2 (Axis 1) ==="
    log "================================================================"
    log "Configs:     ${#BENCHMARKS_AXIS1[@]}"
    log "Workers:     ${WORKER_COUNTS[*]}"
    log "Warmup:      $WARMUP_RUNS"
    log "Repetitions: $REPETITIONS"
    log "Timeout:     ${TIMEOUT_SECS}s per docker cycle"
    log "Resume:      $RESUME"
    log "Output:      $OUTPUT_DIR"
    log ""

    # Build Docker image once
    if [ "$SKIP_BUILD" = false ]; then
        log "Building Docker image..."
        if [ "$DRY_RUN" = true ]; then
            log "[DRY RUN] Would build Docker image"
        else
            (cd "$REPO_DIR" && docker compose build coordinator 2>&1 | tail -3)
            log "Docker image built."
        fi
    fi

    # Initialize CSV files: only write headers if (a) file doesn't exist OR (b) --no-resume
    if [ ! -f "$DETAIL_FILE" ] || [ "$RESUME" = "false" ]; then
        echo "$DETAIL_HEADER" > "$DETAIL_FILE"
    fi
    if [ ! -f "$SUMMARY_FILE" ] || [ "$RESUME" = "false" ]; then
        echo "$SUMMARY_HEADER" > "$SUMMARY_FILE"
    fi
    if [ ! -f "$ROUNDS_FILE" ] || [ "$RESUME" = "false" ]; then
        echo "$ROUNDS_HEADER" > "$ROUNDS_FILE"
    fi

    # Build resume set
    local completed_set=""
    if [ "$RESUME" = "true" ]; then
        completed_set=$(checkpoint_completed_set)
        if [ -n "$completed_set" ]; then
            local n=$(echo "$completed_set" | wc -l)
            log "Resume: $n completed tuples found in detail.csv"
        else
            log "Resume: no prior progress detected"
        fi
    fi

    # Helper: check if tuple is already in completed set
    is_completed() {
        local key="$1"
        echo "$completed_set" | grep -qxF "$key"
    }

    # Sequential baselines: associative array bench:size → median_secs
    declare -A SEQ_BASELINES

    # ===== PHASE A: Sequential baselines (native, not Docker) =====
    if [ "$SKIP_SEQUENTIAL" = false ]; then
        log ""
        log "--- Phase A: Sequential Baselines (native) ---"

        for bench_spec in "${BENCHMARKS_AXIS1[@]}"; do
            IFS=':' read -r bench_name example_net input_size <<< "$bench_spec"

            local seq_key="${bench_name}:${input_size}:sequential:0"
            local input_file="$DATA_DIR/bench_${bench_name}_${input_size}.bin"
            local seq_output="$SEQ_OUT_DIR/${bench_name}_${input_size}.bin"

            if [ "$DRY_RUN" = true ]; then
                log "[DRY RUN] Sequential: $bench_name size=$input_size"
                SEQ_BASELINES["${bench_name}:${input_size}"]="1.0"
                continue
            fi

            # Skip if already complete (resume)
            if is_completed "$seq_key"; then
                log "Sequential: $bench_name size=$input_size [SKIP — already complete]"
                # Recover median from existing detail rows for SEQ_BASELINES
                local median
                median=$(python3 << PYEOF
import csv, statistics
vals = []
with open(r"$(winpath "$DETAIL_FILE")", newline="") as f:
    rd = csv.DictReader(f)
    for row in rd:
        if (row["benchmark"]=="$bench_name" and row["input_size"]=="$input_size"
            and row["mode"]=="sequential" and row["workers"]=="0"):
            vals.append(float(row["wall_clock_secs"]))
print(f"{statistics.median(vals):.6f}" if vals else "1.0")
PYEOF
)
                SEQ_BASELINES["${bench_name}:${input_size}"]="$median"
                continue
            fi

            log "Sequential: $bench_name size=$input_size"

            # Generate input net
            "$RELATIVIST" generate "$example_net" -n "$input_size" -o "$input_file" 2>/dev/null

            # Warmup
            for ((w=0; w<WARMUP_RUNS; w++)); do
                "$RELATIVIST" reduce -i "$input_file" -o /dev/null 2>/dev/null || true
            done

            # Measure
            local wall_clocks=()
            for ((rep=0; rep<REPETITIONS; rep++)); do
                local t0 t1 elapsed
                t0=$(python3 -c "import time; print(f'{time.perf_counter():.9f}')")
                "$RELATIVIST" reduce -i "$input_file" -o "$seq_output" 2>/dev/null
                t1=$(python3 -c "import time; print(f'{time.perf_counter():.9f}')")
                elapsed=$(python3 -c "print(f'{$t1 - $t0:.6f}')")
                wall_clocks+=("$elapsed")

                echo "$bench_name,$input_size,sequential,0,$rep,true,$elapsed,0,0.0,0,1.0000,1.0000,0.0000,0,0,0,0,0,0,0,0,0" >> "$DETAIL_FILE"
            done

            # Compute median baseline
            local median
            median=$(python3 -c "
import statistics
vals = [$(join_comma "${wall_clocks[@]}")]
print(f'{statistics.median(vals):.6f}')
")
            SEQ_BASELINES["${bench_name}:${input_size}"]="$median"

            # Write summary row
            write_summary_row "$bench_name" "$input_size" "sequential" "0" \
                "$REPETITIONS" "true" "$median" "${wall_clocks[@]}" >> "$SUMMARY_FILE"

            log "  Baseline: ${median}s (median of $REPETITIONS)"
        done
    else
        log "Skipping sequential baselines (--skip-sequential)"
        for bench_spec in "${BENCHMARKS_AXIS1[@]}"; do
            IFS=':' read -r bench_name _ input_size <<< "$bench_spec"
            SEQ_BASELINES["${bench_name}:${input_size}"]="1.0"
        done
    fi

    # ===== PHASE B: Docker (TcpLocalhost) benchmarks =====
    log ""
    log "--- Phase B: Docker (TcpLocalhost) Benchmarks ---"

    local total_configs=$(( ${#BENCHMARKS_AXIS1[@]} * ${#WORKER_COUNTS[@]} ))
    local config_num=0

    for bench_spec in "${BENCHMARKS_AXIS1[@]}"; do
        IFS=':' read -r bench_name example_net input_size <<< "$bench_spec"

        local input_file="$DATA_DIR/bench_${bench_name}_${input_size}.bin"
        local seq_output="$SEQ_OUT_DIR/${bench_name}_${input_size}.bin"
        local seq_baseline="${SEQ_BASELINES["${bench_name}:${input_size}"]}"

        # Ensure input file exists (may have been deleted between sessions)
        if [ ! -f "$input_file" ] && [ "$DRY_RUN" = false ]; then
            "$RELATIVIST" generate "$example_net" -n "$input_size" -o "$input_file" 2>/dev/null
        fi

        for workers in "${WORKER_COUNTS[@]}"; do
            config_num=$((config_num + 1))
            local key="${bench_name}:${input_size}:tcp_localhost:${workers}"

            log ""
            log "[$config_num/$total_configs] $bench_name size=$input_size workers=$workers"

            if [ "$DRY_RUN" = true ]; then
                log "  [DRY RUN] Skipping"
                continue
            fi

            # Resume: skip if already complete
            if is_completed "$key"; then
                log "  [SKIP — already complete]"
                continue
            fi

            local wall_clocks=()
            local all_correct="true"
            local total_runs=$((WARMUP_RUNS + REPETITIONS))

            for ((run=0; run < total_runs; run++)); do
                local is_warmup=false
                local rep=$((run - WARMUP_RUNS))
                if [ "$run" -lt "$WARMUP_RUNS" ]; then
                    is_warmup=true
                fi

                # Prepare: copy input to data/input.bin (docker volume target)
                cp "$input_file" "$DATA_DIR/input.bin"
                rm -f "$DATA_DIR/output.bin" "$DATA_DIR/metrics.json"

                # Run docker compose cycle
                local exit_code=0
                run_docker_cycle "$workers" "$TIMEOUT_SECS" || exit_code=$?

                if [ "$is_warmup" = true ]; then
                    log "  Warmup $((run+1))/$WARMUP_RUNS"
                    continue
                fi

                # Check if metrics were produced
                if [ "$exit_code" -ne 0 ] || [ ! -f "$DATA_DIR/metrics.json" ]; then
                    log "  Rep $((rep+1)): FAILED (exit=$exit_code)"
                    all_correct="false"
                    echo "$bench_name,$input_size,tcp_localhost,$workers,$rep,false,0,0,0,0,0,0,1.0,0,0,0,0,0,0,0,0,0" >> "$DETAIL_FILE"
                    continue
                fi

                # Save metrics for forensics
                local metrics_file="$PER_REP_DIR/${bench_name}_${input_size}_w${workers}_r${rep}.json"
                cp "$DATA_DIR/metrics.json" "$metrics_file"

                # G1 verification
                local correct="true"
                if [ -f "$DATA_DIR/output.bin" ] && [ -f "$seq_output" ]; then
                    correct=$(verify_g1 "$seq_output" "$DATA_DIR/output.bin")
                fi
                if [ "$correct" = "false" ]; then
                    all_correct="false"
                    log "  Rep $((rep+1)): G1 FAILED!"
                fi

                # Extract wall clock + write CSV rows
                local wc
                wc=$(extract_wall_clock "$metrics_file")
                wall_clocks+=("$wc")

                parse_metrics_to_detail "$metrics_file" "$bench_name" "$input_size" \
                    "$workers" "$rep" "$seq_baseline" "$correct" >> "$DETAIL_FILE"

                parse_metrics_to_rounds "$metrics_file" "$bench_name" "$input_size" \
                    "$workers" "$rep" >> "$ROUNDS_FILE"

                log "  Rep $((rep+1))/$REPETITIONS: ${wc}s correct=$correct"
            done

            # Write summary row
            if [ ${#wall_clocks[@]} -gt 0 ]; then
                write_summary_row "$bench_name" "$input_size" "tcp_localhost" "$workers" \
                    "${#wall_clocks[@]}" "$all_correct" "$seq_baseline" "${wall_clocks[@]}" >> "$SUMMARY_FILE"

                local median_wc
                median_wc=$(python3 -c "
import statistics
vals = [$(join_comma "${wall_clocks[@]}")]
print(f'{statistics.median(vals):.6f}')
")
                local speedup
                speedup=$(python3 -c "print(f'{$seq_baseline / $median_wc:.4f}' if $median_wc > 0 else '0')")
                log "  Summary: median=${median_wc}s speedup=${speedup}x correct=$all_correct"
            fi
        done
    done

    # ===== Report =====
    log ""
    log "=========================================="
    log "  bench_docker_v2 Complete"
    log "=========================================="
    log "Output files:"
    log "  $DETAIL_FILE"
    log "  $SUMMARY_FILE"
    log "  $ROUNDS_FILE"
    log "  $LOG_FILE"

    local detail_rows
    detail_rows=$(( $(wc -l < "$DETAIL_FILE") - 1 ))
    log "Total datapoints: $detail_rows"
}

main "$@"
```

- [ ] **Step 2: Make the script executable**

Run:
```bash
chmod +x scripts/bench_docker_v2.sh
```

- [ ] **Step 3: Bash syntax check (no execution)**

Run:
```bash
bash -n scripts/bench_docker_v2.sh
```
Expected: no output, exit 0. Any syntax error stops here — fix the heredoc/quoting/etc. immediately.

---

## Task 4: Pre-flight smoke #2 — `--dry-run` validates orchestration without executing

**Files:** none modified.

- [ ] **Step 1: Run with `--dry-run`**

Run:
```bash
bash scripts/bench_docker_v2.sh --dry-run
```
Expected output (excerpt):
```
[HH:MM:SS] === bench_docker_v2.sh — D-011 Phase F-2 (Axis 1) ===
[HH:MM:SS] Configs:     8
[HH:MM:SS] Workers:     1 2 4 8
...
[HH:MM:SS] [DRY RUN] Would build Docker image
...
[HH:MM:SS] --- Phase A: Sequential Baselines (native) ---
[HH:MM:SS] [DRY RUN] Sequential: ep_annihilation_con size=500000
... (8 lines for 8 configs)
[HH:MM:SS] --- Phase B: Docker (TcpLocalhost) Benchmarks ---
[HH:MM:SS] [1/32] ep_annihilation_con size=500000 workers=1
[HH:MM:SS]   [DRY RUN] Skipping
... (32 lines for 8 configs × 4 workers)
[HH:MM:SS] Total datapoints: 0
```

If output looks wrong (e.g., missing configs, wrong totals), inspect script. Total configs MUST be 8, total cycles MUST be 32.

- [ ] **Step 2: Verify CSVs created with headers but no data rows**

Run:
```powershell
Get-Content .\results\v2_tcp_baseline\detail.csv | Select-Object -First 1
(Get-Content .\results\v2_tcp_baseline\detail.csv).Count
```
Expected: header line printed; line count = 1.

---

## Task 5: Pre-flight smoke #3 — 1 config end-to-end (smallest, fastest)

**Goal:** Validate the full pipeline (sequential baseline → docker cycle → metrics parse → CSV write → G1) with **one** config that's quick (~30s total). This is the most likely place to find bugs because all moving parts are exercised once.

**Files:**
- Temporarily edit: `scripts/bench_docker_v2.sh` (revert in step 5)

- [ ] **Step 1: Reduce script scope to 1 config + 1 worker + 1 rep + 0 warmup**

Edit `scripts/bench_docker_v2.sh`. Find the BENCHMARKS_AXIS1 array and replace it:

Old:
```bash
BENCHMARKS_AXIS1=(
    "ep_annihilation_con:ep-annihilation-con:500000"
    "ep_annihilation_con:ep-annihilation-con:1000000"
    "ep_annihilation_con:ep-annihilation-con:5000000"
    "dual_tree:dual-tree:18"
    "dual_tree:dual-tree:20"
    "dual_tree:dual-tree:22"
    "condup_expansion:con-dup-expansion:1000"
    "condup_expansion:con-dup-expansion:5000"
)

WORKER_COUNTS=(1 2 4 8)
WARMUP_RUNS=2
REPETITIONS=10
```

New:
```bash
BENCHMARKS_AXIS1=(
    "condup_expansion:con-dup-expansion:1000"
)

WORKER_COUNTS=(2)
WARMUP_RUNS=0
REPETITIONS=1
```

- [ ] **Step 2: Clear any prior smoke output**

Run:
```powershell
Remove-Item .\results\v2_tcp_baseline -Recurse -Force -ErrorAction SilentlyContinue
```

- [ ] **Step 3: Run the smoke**

Run:
```bash
bash scripts/bench_docker_v2.sh --skip-build
```
(`--skip-build` because we already validated the docker image works in Task 2.)

Expected: ~30 seconds total. Output ends with:
```
[HH:MM:SS] [1/1] condup_expansion size=1000 workers=2
[HH:MM:SS]   Rep 1/1: 0.001234s correct=true
[HH:MM:SS]   Summary: median=0.001234s speedup=Xx correct=true
[HH:MM:SS] Total datapoints: 2
```

(2 datapoints = 1 sequential baseline rep + 1 TCP rep.)

- [ ] **Step 4: Validate the CSVs**

Run:
```powershell
Write-Host "===DETAIL==="
Get-Content .\results\v2_tcp_baseline\detail.csv
Write-Host "===SUMMARY==="
Get-Content .\results\v2_tcp_baseline\summary.csv
Write-Host "===ROUNDS==="
Get-Content .\results\v2_tcp_baseline\rounds.csv
```

Expected:
- `detail.csv`: 3 lines (header + sequential + tcp_localhost), all 22 columns populated, `correct=true` on the TCP row
- `summary.csv`: 3 lines (header + sequential + tcp_localhost), `all_correct=true`, non-zero `wall_clock_mean`
- `rounds.csv`: header + ≥1 round row from the TCP run

If `correct=false` on the TCP row → G1 mismatch. Inspect `per_rep_metrics/condup_expansion_1000_w2_r0.json` and the diff between `seq_outputs/condup_expansion_1000.bin` and `data/output.bin` to debug.

- [ ] **Step 5: Restore the full configuration in the script**

Edit `scripts/bench_docker_v2.sh`, revert the BENCHMARKS_AXIS1 / WORKER_COUNTS / WARMUP_RUNS / REPETITIONS to the original values from Task 3 Step 1.

- [ ] **Step 6: Verify revert with bash syntax check**

Run:
```bash
bash -n scripts/bench_docker_v2.sh
```
Expected: no output, exit 0.

---

## Task 6: Pre-flight smoke #4 — Resume capability

**Goal:** Validate that re-invoking the script after a partial run skips already-completed tuples.

**Files:** none modified (uses output from Task 5).

- [ ] **Step 1: Confirm task 5 left CSVs in place**

Run:
```powershell
Test-Path .\results\v2_tcp_baseline\detail.csv
(Import-Csv .\results\v2_tcp_baseline\detail.csv).Count
```
Expected: `True`, count = 2.

- [ ] **Step 2: Re-run script in dry-run with the smoke config narrowed (test resume detection)**

This step is to verify that `checkpoint_completed_set` correctly identifies the completed tuples. Temporarily edit script as in Task 5 (1 config, 1 worker, 1 rep) and run dry-run:

Edit script: same temp edit as Task 5 step 1.

Run:
```bash
bash scripts/bench_docker_v2.sh --dry-run
```
Expected: log line `Resume: 2 completed tuples found in detail.csv`, then for the seq baseline: `[SKIP — already complete]`, and for tcp_localhost: also `[SKIP — already complete]` (NB: in dry-run mode the SKIP logs may differ slightly because dry-run takes precedence — accept either "SKIP — already complete" or "DRY RUN Skipping" as long as no actual docker invocation is attempted; verify by absence of "Docker image" output and runtime <5s).

- [ ] **Step 3: Restore full configuration**

Same as Task 5 Step 5: revert BENCHMARKS_AXIS1 / WORKER_COUNTS / WARMUP_RUNS / REPETITIONS to original.

- [ ] **Step 4: Final bash syntax check**

Run:
```bash
bash -n scripts/bench_docker_v2.sh
```
Expected: no output.

- [ ] **Step 5: Clean smoke output (we're about to do the real rodada)**

Run:
```powershell
Remove-Item .\results\v2_tcp_baseline -Recurse -Force -ErrorAction SilentlyContinue
```

---

## Task 7: Commit the script + design doc updates

**Files:**
- Add: `scripts/bench_docker_v2.sh`
- Modified: `docs/plans/2026-05-02-bench-docker-v2-tcp-rodada-design.md` (Axis 2 dropped)
- Add: `docs/plans/2026-05-02-bench-docker-v2-tcp-rodada-implementation.md` (this file)

- [ ] **Step 1: Stage the files**

Run:
```bash
git add scripts/bench_docker_v2.sh \
        docs/plans/2026-05-02-bench-docker-v2-tcp-rodada-design.md \
        docs/plans/2026-05-02-bench-docker-v2-tcp-rodada-implementation.md
git status --short
```

Expected: `M docs/plans/2026-05-02-bench-docker-v2-tcp-rodada-design.md`, `?? scripts/bench_docker_v2.sh`, `?? docs/plans/2026-05-02-bench-docker-v2-tcp-rodada-implementation.md`.

- [ ] **Step 2: Commit**

Run:
```bash
git commit -m "$(cat <<'EOF'
feat(scripts): bench_docker_v2 — D-011 Phase F-2 docker TCP rodada

Adapts v1 bench_docker.sh for v2 with Tier 3 active in coordinator path.
Same 8 configs as v1's frozen phase2 baseline (results/locked/v1_local_baseline)
so v1-TCP vs v2-TCP comparison is direct, on identical workload.

Tier 3 in coord+worker path:
  - --chunk-size=10000 / --max-pending-lifetime=16 (SPEC-21)
  - free-list recycling (D-009 SPEC-22, runtime-resident)
  - CompactSubnet wire fix (D-011 B-1, QA-D009-001)
  - streaming chunked partitioning N/A (coord loads net from disk)

Resume capability mandatory: re-invocation reads detail.csv, skips tuples
with >=10 reps. Validated by 4 pre-flight smokes (compose Tier 3 wiring,
--dry-run orchestration, 1-config E2E with G1 gate, resume detection).

Schema: v1 22-column CSV. The 7 extra v2 columns (peak_memory_during_*,
agent_count_at_construction_complete, live_agent_count_watermark,
representation, chunk_size, recycle_policy) are partially N/A in coord
path (no construction phase, metrics.json doesn't expose post-reduction
VmHWM) — kept off-schema rather than zero-padded.

Axis 2 (v2-local vs v2-TCP) dropped from this bundle: v2-local rodada uses
sizes <=100K with sequential <=38ms, where docker startup overhead (~5s)
would dominate signal 100-1000x. Deferred to D-013 candidate (see design
doc §10.1) which would require a v2-local-expanded rodada at v1's size scale.

EOF
)"
```

- [ ] **Step 3: Verify commit landed**

Run:
```bash
git log --oneline -3
```
Expected: top commit is `feat(scripts): bench_docker_v2 — D-011 Phase F-2 docker TCP rodada`.

---

## Task 8: Execute the full Axis 1 rodada (~8h, background)

**Goal:** Run the full 8-config × 4-worker × 12-rep matrix to produce the v2-TCP baseline.

**Files:** none modified. Output to `results/v2_tcp_baseline/`.

- [ ] **Step 1: Verify clean state**

Run:
```powershell
Test-Path .\results\v2_tcp_baseline
docker compose ps
```

Expected: first command prints `False`. Second prints empty list (no running containers).

- [ ] **Step 2: Verify cargo release build is current**

Run:
```powershell
.\target\release\relativist.exe --version
```
Expected: prints `relativist 0.11.0`. If file missing, run `cargo build --release` first (~3 min).

- [ ] **Step 3: Start the rodada (background, with tee to log)**

Run from Git Bash (not PowerShell — script uses bash syntax):
```bash
bash scripts/bench_docker_v2.sh 2>&1 | tee results/v2_tcp_baseline_invocation.log
```

This runs synchronously in your terminal. Estimated wall-clock 8h. You can:
- Leave the terminal open and let it run.
- Interrupt with Ctrl+C at any time; resume by re-invoking the same command (resume is default ON).
- If laptop sleeps, pending docker timeout (600s) eventually fires and script moves on.

- [ ] **Step 4: Monitor periodically**

In a separate terminal, periodically check progress:
```powershell
Write-Host "===PROGRESS==="
(Get-Content .\results\v2_tcp_baseline\detail.csv).Count - 1  # rows = datapoints so far
Write-Host "===LATEST==="
Get-Content .\results\v2_tcp_baseline\run.log -Tail 10
```

Expected target: `detail.csv` row count grows by 1 every ~30-300s (depending on config size). Final target: ≥384 rows for TCP rows + 80 rows for sequential = 464 datapoints minimum.

- [ ] **Step 5: Verify rodada complete**

When the script exits cleanly, run:
```powershell
Write-Host "===FINAL ROW COUNTS==="
Write-Host "detail:  $((Import-Csv .\results\v2_tcp_baseline\detail.csv).Count)"
Write-Host "summary: $((Import-Csv .\results\v2_tcp_baseline\summary.csv).Count)"
Write-Host "rounds:  $((Import-Csv .\results\v2_tcp_baseline\rounds.csv).Count)"
Write-Host "===CORRECTNESS==="
Import-Csv .\results\v2_tcp_baseline\summary.csv | Where-Object { $_.mode -eq 'tcp_localhost' } | Group-Object all_correct | Select-Object Name, Count
```

Acceptance criteria from design doc §9:
- detail rows ≥ 464 (80 sequential + 384 TCP minimum)
- summary rows ≥ 40 (8 sequential + 32 TCP)
- TCP rows with `all_correct=true` ratio ≥ 0.95 (i.e., ≥ 30 of 32)
- All summary rows have non-zero `wall_clock_mean` and finite `cv`

If criteria fail, document which configs failed in the close-out and decide whether to (a) accept partial data, (b) re-run failed configs with `--no-resume` after fixing, or (c) escalate.

---

## Task 9: F-3 close-out — promote baseline to locked, update progress.md

**Files:**
- Modify/Add: `results/locked/v2_tcp_baseline/` (copies of `results/v2_tcp_baseline/{detail,summary,rounds}.csv`)
- Modify: `docs/progress.md`
- Modify: `docs/next-steps.md`

- [ ] **Step 1: Copy CSVs to locked dir (treat as immutable from this point)**

Run:
```powershell
New-Item -ItemType Directory -Path .\results\locked\v2_tcp_baseline -Force | Out-Null
Copy-Item .\results\v2_tcp_baseline\detail.csv  .\results\locked\v2_tcp_baseline\detail.csv
Copy-Item .\results\v2_tcp_baseline\summary.csv .\results\locked\v2_tcp_baseline\summary.csv
Copy-Item .\results\v2_tcp_baseline\rounds.csv  .\results\locked\v2_tcp_baseline\rounds.csv
Get-ChildItem .\results\locked\v2_tcp_baseline | Format-Table Name, Length
```

- [ ] **Step 2: Update `docs/progress.md`** with a closing entry

Read current `docs/progress.md` head, then append a new section. Template:

```markdown
## D-011 close — 2026-05-02

**Phase F-2 (BENCH RODADA) and F-3 (CLOSE-OUT) shipped.**

### What landed
- Local rodada `v2_local_full_baseline` (commit 00f9ce8): 360 datapoints, all_correct=true
- Docker TCP rodada `v2_tcp_baseline` (commit <SHA from Task 8>): N datapoints, all_correct ratio X
- 3 docker-path bug fixes (commit 4efd344): stub binary cache, socket2 features, GLIBC pin
- Design + implementation plans in `docs/plans/2026-05-02-bench-docker-v2-tcp-rodada-{design,implementation}.md`

### Tier 3 final state (v2)
| Optimization | Active in local | Active in TCP coord+worker |
|---|---|---|
| Free-list recycling (SPEC-22) | Yes | Yes |
| CompactSubnet wire fix (B-1) | Yes | Yes |
| Streaming generation (SPEC-21) | Yes | N/A (coord loads net from disk) |
| max_pending_lifetime | Yes | Yes |
| Sparse representation | Yes (micro-bench only) | N/A |

### Deferred to D-013 candidate
- v2-local-expanded rodada at v1's size scale + matching TCP (Axis 2 of original design)
- TCP dispatch inside `bench` subcommand
- Strong G1 (`nets_graph_isomorphic`) via `relativist verify`
- Phase 3 LAN rodada
```

(Replace `<SHA from Task 8>` and `N`, `X` with actual values once Task 8 completes.)

- [ ] **Step 3: Update `docs/next-steps.md`**

Mark D-011 closed and record the deferred follow-ups. Locate the section about D-011 in `next-steps.md` and update its status. If unsure of the format, follow the existing pattern from the file.

- [ ] **Step 4: Commit close-out**

Run:
```bash
git add results/locked/v2_tcp_baseline/ docs/progress.md docs/next-steps.md
git commit -m "$(cat <<'EOF'
chore(d-011): close Tier 3 hardening + bench enablement

D-011 complete. Phase F-2 produced two frozen baselines:
  - results/locked/v2_local_full_baseline (commit 00f9ce8) — 360 datapoints
  - results/locked/v2_tcp_baseline (commit <Task 8 SHA>) — N datapoints

Both pass all_correct gate at >=0.95 ratio. v1-TCP vs v2-TCP comparison on
identical 8-config workload now possible against
results/locked/v1_local_baseline/phase2_*.csv.

Deferred to D-013 candidate: v2-local-expanded rodada at v1's size scale
to enable v2-local vs v2-TCP cross-axis comparison (see design §10.1).
EOF
)"
```

- [ ] **Step 5: Final state verification**

Run:
```bash
git log --oneline -10
git status
```
Expected: `git status` clean. Top 5 commits include the docker-path fixes, design, implementation plan, bench data, and close-out.

---

## Self-review

**Spec coverage:**
- Goal "Produce TCP rodada CSVs comparable to phase2_*" → Task 8 (rodada execution)
- Goal "Tier 3 active in coord path" → Task 1 (compose update)
- Goal "Resume capability mandatory" → Task 3 (script `checkpoint_completed_set`), Task 6 (smoke validates)
- Goal "Single user-runnable script" → Task 3 (full script written)
- Pre-flight smokes 1-4 → Tasks 2, 4, 5, 6 respectively
- Output layout → Task 3 (script creates `OUTPUT_DIR`, `PER_REP_DIR`, `SEQ_OUT_DIR`)
- Schema 22-col → Task 3 (DETAIL_HEADER literal)
- G1 verification → Task 3 (`verify_g1` function), Task 5 (validates real call)
- Acceptance criteria → Task 8 step 5 (validates against design §9)
- F-3 close-out → Task 9

**Placeholder scan:** Replaced `<SHA from Task 8>` and `N`, `X` are explicit "fill in after Task 8 runs" — these are values that genuinely cannot be known until execution. Acceptable as fill-in fields, not placeholders for missing logic.

**Type/identifier consistency:**
- `BENCHMARKS_AXIS1`, `WORKER_COUNTS`, `WARMUP_RUNS`, `REPETITIONS`, `TIMEOUT_SECS` — used consistently between Task 3 (definition), Task 5 (override + revert), Task 6 (override + revert).
- `DETAIL_FILE`, `SUMMARY_FILE`, `ROUNDS_FILE`, `LOG_FILE` — paths relative to `OUTPUT_DIR=results/v2_tcp_baseline`, used consistently.
- `is_completed`, `checkpoint_completed_set` — function names match between definition and call sites.
- `SEQ_BASELINES` associative array key format `"${bench_name}:${input_size}"` — used consistently in Phase A (assignment) and Phase B (lookup).

No issues found.

---

## Execution handoff

After completing Tasks 1-7 (script written, smokes pass, committed), Task 8 (rodada execution) takes ~8h wall-clock during which active user time is ~5min (start, occasional check, finish). Task 9 (close-out) takes ~30min after rodada completes.

The plan is structured for inline execution by Claude with user oversight at smoke checkpoints (Tasks 2, 4, 5, 6 each have a clear pass/fail criterion the user can confirm). Task 8 is user-driven (the rodada must run in the user's terminal because docker daemon credentials live there). Task 9 is back to Claude.
