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

# Find relativist binary (host-native, used for sequential baselines + generate + inspect)
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

# Find a Python 3 interpreter. Git Bash on Windows often has none on PATH;
# fall back to common installation locations (Anaconda, system Python).
if command -v python3 &>/dev/null; then
    PYTHON=python3
elif command -v python &>/dev/null; then
    PYTHON=python
elif [ -f "$USERPROFILE/anaconda3/python.exe" ]; then
    PYTHON="$USERPROFILE/anaconda3/python.exe"
elif [ -f "$HOME/anaconda3/python.exe" ]; then
    PYTHON="$HOME/anaconda3/python.exe"
elif [ -f "/c/Users/$USER/anaconda3/python.exe" ]; then
    PYTHON="/c/Users/$USER/anaconda3/python.exe"
elif [ -f "/c/Users/$USERNAME/anaconda3/python.exe" ]; then
    PYTHON="/c/Users/$USERNAME/anaconda3/python.exe"
else
    echo "ERROR: Python 3 not found. Install python3 or set \$PYTHON to its path."
    exit 1
fi

# Find Docker. On Git Bash for Windows the Docker Desktop binary is in
# Program Files but not on PATH by default.
if command -v docker &>/dev/null; then
    DOCKER=docker
elif [ -f "/c/Program Files/Docker/Docker/resources/bin/docker.exe" ]; then
    DOCKER="/c/Program Files/Docker/Docker/resources/bin/docker.exe"
else
    echo "ERROR: docker not found. Start Docker Desktop or install Docker."
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

# Parse metrics.json -> detail CSV row (22 cols, v1 schema)
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
    "$PYTHON" << PYEOF
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

# Parse metrics.json -> rounds CSV rows (one per round)
parse_metrics_to_rounds() {
    local metrics_file="$1"
    local bench_name="$2"
    local input_size="$3"
    local workers="$4"
    local rep="$5"

    local wpath
    wpath=$(winpath "$metrics_file")
    "$PYTHON" << PYEOF
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
    "$PYTHON" -c "
import json
with open(r'$wpath') as f:
    m = json.load(f)
t = m['total_time']['secs'] + m['total_time']['nanos']/1e9
print(f'{t:.6f}')
"
}

# QA-D012-002 (D-012 REFACTOR, 2026-05-05): extract total_interactions
# (integer) from metrics.json so the summary writer can recompute
# `mips_mean` from the same per-rep counters that detail.csv::mips uses.
# Falls back to 0 on missing/zero. Caller threads the values as a
# comma-separated list to `write_summary_row`.
extract_total_interactions() {
    local wpath
    wpath=$(winpath "$1")
    "$PYTHON" -c "
import json
with open(r'$wpath') as f:
    m = json.load(f)
print(int(m.get('total_interactions', 0)))
"
}

# Compute summary stats from a list of wall_clock values
#
# QA-D012-002 (D-012 REFACTOR, 2026-05-05): the historical version of this
# function hardcoded `mips = 0.0` because the summary writer was originally
# disconnected from the per-rep `total_interactions` counter. The result
# was that every row of `summary.csv::mips_mean` produced by this script
# read 0.000, even though the matching rows of `detail.csv::mips` read the
# real value (the detail writer at parse_metrics_to_detail recomputes mips
# from `total_interactions / wall_clock`).
#
# After this refactor the summary recomputes `mips_mean` from the same
# inputs the detail rows use. The 8th positional arg is the comma-separated
# list of `total_interactions` values (one per rep, paired with the
# wall_clock values that follow); when no values are passed the column
# falls back to 0.0 with a deterministic note in run.log.
write_summary_row() {
    local bench_name="$1"
    local input_size="$2"
    local mode="$3"
    local workers="$4"
    local reps="$5"
    local all_correct="$6"
    local seq_baseline="$7"
    local total_interactions_csv="$8"
    shift 8
    # remaining args are wall_clock values

    "$PYTHON" << PYEOF
import statistics

values = [float(v) for v in "$*".split()]
n = len(values)
if n == 0:
    exit(0)

ti_raw = "$total_interactions_csv"
ti_values = [int(v) for v in ti_raw.split(",") if v.strip()]

mean_v = statistics.mean(values)
std_v = statistics.pstdev(values) if n > 1 else 0.0
median_v = statistics.median(values)
min_v = min(values)
max_v = max(values)
cv = std_v / mean_v if mean_v > 0 else 0.0

seq_b = $seq_baseline
w = int("$workers")

# QA-D012-002 (D-012 REFACTOR): mips_mean now derives from the per-rep
# (total_interactions, wall_clock) pairs, mirroring detail.csv::mips. If
# `ti_values` is shorter than `values` (caller forgot to thread the new
# arg through), fall back to 0.0 to preserve the legacy behavior rather
# than crashing.
if ti_values and len(ti_values) == len(values):
    per_rep_mips = [
        (ti / wc / 1e6) if wc > 0 else 0.0
        for ti, wc in zip(ti_values, values)
    ]
    mips = statistics.mean(per_rep_mips)
else:
    mips = 0.0  # legacy fallback — see QA-D012-002

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
    (cd "$REPO_DIR" && "$DOCKER" compose down --remove-orphans 2>/dev/null) || true

    # Run with abort-on-container-exit: stops all when coordinator exits
    local exit_code=0
    (cd "$REPO_DIR" && \
        NUM_WORKERS="$workers" \
        CHUNK_SIZE=10000 \
        MAX_PENDING_LIFETIME=16 \
        timeout "$timeout" \
        "$DOCKER" compose up coordinator worker \
            --abort-on-container-exit \
            --exit-code-from coordinator \
            --scale worker="$workers" \
            2>&1 | tail -30
    ) || exit_code=$?

    # Always clean up
    (cd "$REPO_DIR" && "$DOCKER" compose down --remove-orphans 2>/dev/null) || true

    return $exit_code
}

# Read existing detail.csv (if present) and emit "bench:size:mode:workers" lines
# for tuples with >=REPETITIONS distinct repetitions. Used for resume.
checkpoint_completed_set() {
    if [ ! -f "$DETAIL_FILE" ] || [ "$RESUME" = "false" ]; then
        return  # empty set
    fi
    "$PYTHON" << PYEOF
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

    # Initialize log file (append mode -- survives resume)
    echo "" >> "$LOG_FILE"
    log "================================================================"
    log "=== bench_docker_v2.sh -- D-011 Phase F-2 (Axis 1) ==="
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
            (cd "$REPO_DIR" && "$DOCKER" compose build coordinator 2>&1 | tail -3)
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
            local n
            n=$(echo "$completed_set" | wc -l)
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

    # Sequential baselines: associative array bench:size -> median_secs
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
                log "Sequential: $bench_name size=$input_size [SKIP -- already complete]"
                # Recover median from existing detail rows for SEQ_BASELINES
                local median
                median=$("$PYTHON" << PYEOF
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
                t0=$("$PYTHON" -c "import time; print(f'{time.perf_counter():.9f}')")
                "$RELATIVIST" reduce -i "$input_file" -o "$seq_output" 2>/dev/null
                t1=$("$PYTHON" -c "import time; print(f'{time.perf_counter():.9f}')")
                elapsed=$("$PYTHON" -c "print(f'{$t1 - $t0:.6f}')")
                wall_clocks+=("$elapsed")

                echo "$bench_name,$input_size,sequential,0,$rep,true,$elapsed,0,0.0,0,1.0000,1.0000,0.0000,0,0,0,0,0,0,0,0,0" >> "$DETAIL_FILE"
            done

            # Compute median baseline
            local median
            median=$("$PYTHON" -c "
import statistics
vals = [$(join_comma "${wall_clocks[@]}")]
print(f'{statistics.median(vals):.6f}')
")
            SEQ_BASELINES["${bench_name}:${input_size}"]="$median"

            # Write summary row
            # QA-D012-002 (D-012 REFACTOR): sequential rows do not have
            # per-rep total_interactions in this script's path (the
            # `relativist reduce` invocation above does not emit metrics.json
            # for sequential runs); pass an empty 8th arg so the summary
            # writer falls back to legacy `mips=0.0`.
            write_summary_row "$bench_name" "$input_size" "sequential" "0" \
                "$REPETITIONS" "true" "$median" "" "${wall_clocks[@]}" >> "$SUMMARY_FILE"

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
                log "  [SKIP -- already complete]"
                continue
            fi

            local wall_clocks=()
            local total_interactions_list=()
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
                local wc ti
                wc=$(extract_wall_clock "$metrics_file")
                ti=$(extract_total_interactions "$metrics_file")
                wall_clocks+=("$wc")
                total_interactions_list+=("$ti")

                parse_metrics_to_detail "$metrics_file" "$bench_name" "$input_size" \
                    "$workers" "$rep" "$seq_baseline" "$correct" >> "$DETAIL_FILE"

                parse_metrics_to_rounds "$metrics_file" "$bench_name" "$input_size" \
                    "$workers" "$rep" >> "$ROUNDS_FILE"

                log "  Rep $((rep+1))/$REPETITIONS: ${wc}s correct=$correct"
            done

            # Write summary row
            if [ ${#wall_clocks[@]} -gt 0 ]; then
                # QA-D012-002 (D-012 REFACTOR): pass the comma-joined per-rep
                # `total_interactions` list as the new 8th positional arg so
                # the summary writer recomputes `mips_mean` instead of
                # hardcoding 0.0.
                local ti_csv
                ti_csv=$(join_comma "${total_interactions_list[@]}")
                write_summary_row "$bench_name" "$input_size" "tcp_localhost" "$workers" \
                    "${#wall_clocks[@]}" "$all_correct" "$seq_baseline" "$ti_csv" \
                    "${wall_clocks[@]}" >> "$SUMMARY_FILE"

                local median_wc
                median_wc=$("$PYTHON" -c "
import statistics
vals = [$(join_comma "${wall_clocks[@]}")]
print(f'{statistics.median(vals):.6f}')
")
                local speedup
                speedup=$("$PYTHON" -c "print(f'{$seq_baseline / $median_wc:.4f}' if $median_wc > 0 else '0')")
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
