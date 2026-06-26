#!/usr/bin/env bash
# =============================================================================
# Phase 2 Stress Campaign — v1_stress (Docker / TcpLocalhost)
# =============================================================================
# Runs the Phase 2 Docker stress benchmark campaign on the tagged binary and
# emits frozen CSVs under `results/extended/v1_stress/`.
#
# Purpose: extend the Docker TcpLocalhost measurements beyond v1_local_baseline
# (which capped at ep_con=5M and dual_tree=22) to document how the transport
# layer behaves at stress sizes. Primary evidence for the ROADMAP 2.22-2.26
# network optimisation items: the current `tcp_localhost / sequential` ratio
# is 2.02x at 5M and 3.48x at 20M, so pushing to 50M on ep_con and d=25 on
# dual_tree should expose whether the ratio keeps worsening with scale.
#
# Campaign layout:
#   - ep_annihilation_con × {10M, 20M} × workers {1, 2, 4, 8}
#   - ep_annihilation_con × {50M} × workers {4, 8} only (frame cap hedge:
#     at w=1, partition is ~1.8 GiB, exceeds the 1 GiB frame cap; at w=2
#     ~900 MiB, still borderline given bincode v1 overhead. Only w>=4 is
#     safe for 50M on the current code.)
#   - dual_tree × {23, 24, 25} × workers {1, 2, 4, 8}
#   - native sequential baseline: one per bench × size
#   - 5 repetitions per config, 2 warmup runs
#
# CRITICAL: shutdown strategy fix.
# The original bench_phase2_locked.sh uses `docker compose up
# --abort-on-container-exit --exit-code-from coordinator`. At stress sizes
# (observed in the 20M smoke on 2026-04-11) the coordinator takes longer
# to flush metrics.json than the workers take to exit, so abort-on-exit
# SIGKILLs the coordinator mid-flush and metrics.json never reaches disk.
# This script uses `docker compose up -d` + `docker wait coordinator` +
# then `docker compose down`, which lets the coordinator exit naturally
# and flush its metrics.json before teardown.
#
# Pre-conditions:
#   - Clean working copy at tag v0.10.0-bench.
#   - `cargo build --release` succeeded.
#   - Docker Desktop running, WSL2 VM alive, compose build succeeded.
#   - Environment hygiene matches v1 (Ultimate Performance power plan, IDE
#     closed, browsers closed, Windows Update paused).
#
# On error the script stops immediately (set -e). Partial CSVs remain in
# place under results/extended/v1_stress/ for forensic review; raw
# per-run metrics.json files persist in raw/phase2/.
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
DATA_DIR="$REPO_DIR/data"
STRESS_DIR="$REPO_DIR/reproduce_article/results/extended/v1_stress"
RAW_DIR="$STRESS_DIR/raw/phase2"

mkdir -p "$DATA_DIR" "$STRESS_DIR" "$RAW_DIR"

winpath() {
    if command -v cygpath &>/dev/null; then
        cygpath -w "$1"
    else
        echo "$1"
    fi
}

join_comma() {
    local IFS=','
    echo "$*"
}

if [ -f "$REPO_DIR/target/release/relativist.exe" ]; then
    RELATIVIST="$REPO_DIR/target/release/relativist.exe"
elif [ -f "$REPO_DIR/target/release/relativist" ]; then
    RELATIVIST="$REPO_DIR/target/release/relativist"
elif command -v relativist &>/dev/null; then
    RELATIVIST="relativist"
else
    echo "ERROR: relativist binary not found. Run 'cargo build --release' first." >&2
    exit 1
fi

# Benchmark configurations: BENCHMARK_NAME:EXAMPLE_NET:SIZE:WORKER_LIST
# WORKER_LIST is a comma-separated subset of {1,2,4,8}; used to skip
# worker counts that are known to exceed the 1 GiB frame cap under the
# current bincode v1 + CompactSubnet encoding.
BENCHMARKS=(
    "ep_annihilation_con:ep-annihilation-con:10000000:1,2,4,8"
    "ep_annihilation_con:ep-annihilation-con:20000000:1,2,4,8"
    "ep_annihilation_con:ep-annihilation-con:50000000:4,8"
    "dual_tree:dual-tree:23:1,2,4,8"
    "dual_tree:dual-tree:24:1,2,4,8"
    "dual_tree:dual-tree:25:1,2,4,8"
)

WARMUP_RUNS=2
REPETITIONS=5
TIMEOUT_SECS=1800

DRY_RUN=false
SKIP_BUILD=false
SKIP_SEQUENTIAL=false

for arg in "$@"; do
    case "$arg" in
        --dry-run)         DRY_RUN=true ;;
        --skip-build)      SKIP_BUILD=true ;;
        --skip-sequential) SKIP_SEQUENTIAL=true ;;
        *) echo "Unknown arg: $arg"; exit 1 ;;
    esac
done

DETAIL_HEADER="benchmark,input_size,mode,workers,repetition,correct,wall_clock_secs,total_interactions,mips,rounds,speedup,efficiency,overhead_ratio,peak_memory_bytes,bytes_sent,bytes_received,con_con,dup_dup,era_era,con_dup,con_era,dup_era"
SUMMARY_HEADER="benchmark,input_size,mode,workers,repetitions,all_correct,wall_clock_mean,wall_clock_std,wall_clock_median,wall_clock_min,wall_clock_max,mips_mean,speedup_mean,efficiency_mean,overhead_ratio_mean,cv"
ROUNDS_HEADER="benchmark,input_size,workers,mode,repetition,round,partition_time_secs,compute_time_secs,merge_time_secs,network_time_secs,border_redexes,border_ratio,agents_at_start,bytes_sent,bytes_received"

DETAIL_FILE="$STRESS_DIR/phase2_stress_detail.csv"
SUMMARY_FILE="$STRESS_DIR/phase2_stress_summary.csv"
ROUNDS_FILE="$STRESS_DIR/phase2_stress_rounds.csv"

log() { echo "[$(date +%H:%M:%S)] $*"; }

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

write_summary_row() {
    local bench_name="$1"
    local input_size="$2"
    local mode="$3"
    local workers="$4"
    local reps="$5"
    local all_correct="$6"
    local seq_baseline="$7"
    shift 7

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
mips = 0.0
speedup = seq_b / mean_v if mean_v > 0 else 0.0
efficiency = speedup / w if w > 0 else speedup
overhead = max(0.0, 1.0 - efficiency)

print(f"$bench_name,$input_size,$mode,$workers,$reps,$all_correct,{mean_v:.6f},{std_v:.6f},{median_v:.6f},{min_v:.6f},{max_v:.6f},{mips:.3f},{speedup:.4f},{efficiency:.4f},{overhead:.4f},{cv:.4f}")
PYEOF
}

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

# =============================================================================
# run_docker_cycle — stress-safe shutdown
# =============================================================================
# Starts the compose stack detached, waits for the coordinator container to
# exit naturally (letting it flush metrics.json to the bind-mounted $DATA_DIR),
# then tears everything down. Uses `timeout` on `docker wait` to protect
# against indefinite hangs. Returns the coordinator's exit code, or 124 if
# the wait itself timed out.
# =============================================================================
run_docker_cycle() {
    local workers="$1"
    local timeout_secs="$2"

    (cd "$REPO_DIR" && docker compose down --remove-orphans --timeout 5 2>/dev/null) || true

    if ! (cd "$REPO_DIR" && \
          NUM_WORKERS="$workers" \
          docker compose up -d \
              --scale worker="$workers" \
              2>&1 | tail -5); then
        (cd "$REPO_DIR" && docker compose down --remove-orphans --timeout 5 2>/dev/null) || true
        return 1
    fi

    local coord_id
    coord_id=$(cd "$REPO_DIR" && docker compose ps -q coordinator 2>/dev/null | head -1)
    if [ -z "$coord_id" ]; then
        log "    ERROR: coordinator container id not resolved"
        (cd "$REPO_DIR" && docker compose down --remove-orphans --timeout 5 2>/dev/null) || true
        return 1
    fi

    local coord_exit
    if ! coord_exit=$(timeout "$timeout_secs" docker wait "$coord_id" 2>/dev/null); then
        log "    ERROR: docker wait timed out after ${timeout_secs}s"
        (cd "$REPO_DIR" && docker compose down --remove-orphans --timeout 10 2>/dev/null) || true
        return 124
    fi
    coord_exit=$(echo "$coord_exit" | tr -d '[:space:]')
    [ -z "$coord_exit" ] && coord_exit=1

    (cd "$REPO_DIR" && docker compose down --remove-orphans --timeout 5 2>/dev/null) || true

    return "$coord_exit"
}

# =============================================================================
# Main
# =============================================================================

main() {
    log "=== Phase 2 Stress Campaign — v1_stress (Docker) ==="
    log "Binary:      $RELATIVIST"
    log "Output:      $STRESS_DIR"
    log "Benchmarks:  ${#BENCHMARKS[@]} configurations"
    log "Warmup:      $WARMUP_RUNS"
    log "Repetitions: $REPETITIONS"
    log "Timeout:     ${TIMEOUT_SECS}s per run"
    echo ""

    if [ "$SKIP_BUILD" = false ]; then
        log "Building Docker image..."
        if [ "$DRY_RUN" = true ]; then
            log "[DRY RUN] Would build Docker image"
        else
            (cd "$REPO_DIR" && docker compose build 2>&1 | tail -3)
            log "Docker image built."
        fi
    fi

    echo "$DETAIL_HEADER" > "$DETAIL_FILE"
    echo "$SUMMARY_HEADER" > "$SUMMARY_FILE"
    echo "$ROUNDS_HEADER" > "$ROUNDS_FILE"

    declare -A SEQ_BASELINES

    START_TS=$(date '+%Y-%m-%d %H:%M:%S %z')
    log "Start: $START_TS"

    # ===== PHASE A: native sequential baselines =====
    if [ "$SKIP_SEQUENTIAL" = false ]; then
        log ""
        log "--- Phase A: Sequential Baselines (native) ---"

        for bench_spec in "${BENCHMARKS[@]}"; do
            IFS=':' read -r bench_name example_net input_size worker_list <<< "$bench_spec"

            local key="${bench_name}:${input_size}"
            if [ -n "${SEQ_BASELINES[$key]:-}" ]; then
                continue  # already computed (e.g. 50M ep_con appears twice with different worker_lists)
            fi

            local input_file="$DATA_DIR/bench_${bench_name}_${input_size}.bin"

            if [ "$DRY_RUN" = true ]; then
                log "[DRY RUN] Sequential: $bench_name size=$input_size"
                SEQ_BASELINES["$key"]="1.0"
                continue
            fi

            log "Sequential: $bench_name size=$input_size"

            "$RELATIVIST" generate "$example_net" -n "$input_size" -o "$input_file" 2>/dev/null

            for ((w=0; w<WARMUP_RUNS; w++)); do
                "$RELATIVIST" reduce -i "$input_file" -o /dev/null 2>/dev/null || true
            done

            local wall_clocks=()
            local seq_output="$DATA_DIR/seq_${bench_name}_${input_size}.bin"

            for ((rep=0; rep<REPETITIONS; rep++)); do
                local t0 t1 elapsed

                t0=$(python3 -c "import time; print(f'{time.perf_counter():.9f}')")
                "$RELATIVIST" reduce -i "$input_file" -o "$seq_output" 2>/dev/null
                t1=$(python3 -c "import time; print(f'{time.perf_counter():.9f}')")

                elapsed=$(python3 -c "print(f'{$t1 - $t0:.6f}')")
                wall_clocks+=("$elapsed")

                echo "$bench_name,$input_size,sequential,0,$rep,true,$elapsed,0,0.0,0,1.0000,1.0000,0.0000,0,0,0,0,0,0,0,0,0" >> "$DETAIL_FILE"
            done

            local median
            median=$(python3 -c "
import statistics
vals = [$(join_comma "${wall_clocks[@]}")]
print(f'{statistics.median(vals):.6f}')
")
            SEQ_BASELINES["$key"]="$median"

            write_summary_row "$bench_name" "$input_size" "sequential" "0" \
                "$REPETITIONS" "true" "$median" "${wall_clocks[@]}" >> "$SUMMARY_FILE"

            log "  Baseline: ${median}s (median of $REPETITIONS)"
        done
    else
        log "Skipping sequential baselines (--skip-sequential)"
        for bench_spec in "${BENCHMARKS[@]}"; do
            IFS=':' read -r bench_name _ input_size _ <<< "$bench_spec"
            SEQ_BASELINES["${bench_name}:${input_size}"]="1.0"
        done
    fi

    # ===== PHASE B: Docker (TcpLocalhost) =====
    log ""
    log "--- Phase B: Docker (TcpLocalhost) Stress Benchmarks ---"

    # Count total configs across all (bench, size, workers) triples.
    local total_configs=0
    for bench_spec in "${BENCHMARKS[@]}"; do
        IFS=':' read -r _ _ _ worker_list <<< "$bench_spec"
        local wl
        IFS=',' read -ra wl <<< "$worker_list"
        total_configs=$((total_configs + ${#wl[@]}))
    done
    local config_num=0

    for bench_spec in "${BENCHMARKS[@]}"; do
        IFS=':' read -r bench_name example_net input_size worker_list <<< "$bench_spec"

        local input_file="$DATA_DIR/bench_${bench_name}_${input_size}.bin"
        local seq_output="$DATA_DIR/seq_${bench_name}_${input_size}.bin"
        local seq_baseline="${SEQ_BASELINES["${bench_name}:${input_size}"]}"

        if [ ! -f "$input_file" ] && [ "$DRY_RUN" = false ]; then
            "$RELATIVIST" generate "$example_net" -n "$input_size" -o "$input_file" 2>/dev/null
        fi

        IFS=',' read -ra WORKERS_ARR <<< "$worker_list"
        for workers in "${WORKERS_ARR[@]}"; do
            config_num=$((config_num + 1))
            log ""
            log "[$config_num/$total_configs] $bench_name size=$input_size workers=$workers"

            if [ "$DRY_RUN" = true ]; then
                log "  [DRY RUN] Skipping"
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

                cp "$input_file" "$DATA_DIR/input.bin"
                rm -f "$DATA_DIR/output.bin" "$DATA_DIR/metrics.json"

                local exit_code=0
                run_docker_cycle "$workers" "$TIMEOUT_SECS" || exit_code=$?

                if [ "$is_warmup" = true ]; then
                    log "  Warmup $((run+1))/$WARMUP_RUNS (exit=$exit_code)"
                    continue
                fi

                if [ "$exit_code" -ne 0 ] || [ ! -f "$DATA_DIR/metrics.json" ]; then
                    log "  Rep $((rep+1)): FAILED (exit=$exit_code, metrics.json=$([ -f "$DATA_DIR/metrics.json" ] && echo present || echo missing))"
                    all_correct="false"
                    echo "$bench_name,$input_size,tcp_localhost,$workers,$rep,false,0,0,0,0,0,0,1.0,0,0,0,0,0,0,0,0,0" >> "$DETAIL_FILE"
                    continue
                fi

                local metrics_file="$RAW_DIR/metrics_${bench_name}_${input_size}_w${workers}_r${rep}.json"
                cp "$DATA_DIR/metrics.json" "$metrics_file"

                local correct="true"
                if [ -f "$DATA_DIR/output.bin" ] && [ -f "$seq_output" ]; then
                    correct=$(verify_g1 "$seq_output" "$DATA_DIR/output.bin")
                fi
                if [ "$correct" = "false" ]; then
                    all_correct="false"
                    log "  Rep $((rep+1)): G1 FAILED!"
                fi

                local wc
                wc=$(extract_wall_clock "$metrics_file")
                wall_clocks+=("$wc")

                parse_metrics_to_detail "$metrics_file" "$bench_name" "$input_size" \
                    "$workers" "$rep" "$seq_baseline" "$correct" >> "$DETAIL_FILE"

                parse_metrics_to_rounds "$metrics_file" "$bench_name" "$input_size" \
                    "$workers" "$rep" >> "$ROUNDS_FILE"

                log "  Rep $((rep+1))/$REPETITIONS: ${wc}s correct=$correct"
            done

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

    END_TS=$(date '+%Y-%m-%d %H:%M:%S %z')

    log ""
    log "=========================================="
    log "  Phase 2 Stress Campaign Complete"
    log "=========================================="
    log "Start: $START_TS"
    log "End:   $END_TS"
    log "Output files:"
    log "  $DETAIL_FILE  ($(wc -l < "$DETAIL_FILE") rows)"
    log "  $SUMMARY_FILE ($(wc -l < "$SUMMARY_FILE") rows)"
    log "  $ROUNDS_FILE  ($(wc -l < "$ROUNDS_FILE") rows)"
    log "Raw metrics in: $RAW_DIR"
}

main "$@"
