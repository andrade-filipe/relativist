#!/usr/bin/env bash
# =============================================================================
# Phase 3 Locked Baseline — v1_lan_baseline (Tailscale / TcpNetwork)
# =============================================================================
# Runs the full Phase 3 LAN benchmark campaign using real distributed workers
# connected via Tailscale. Emits frozen CSVs under
# `results/locked/v1_lan_baseline/`.
#
# Campaign layout:
#   - 8 benchmark configurations (ep_annihilation_con × 500k/1M/5M,
#     dual_tree × 18/20/22, condup_expansion × 1000/5000).
#   - 4 worker counts (1, 2, 4, 8) per config → 32 distributed datapoints.
#   - 8 native sequential baselines (one per bench × size) → 40 total.
#   - 10 repetitions each, 2 warmup runs.
#
# Pre-conditions:
#   - `cargo build --release` must succeed prior to invocation.
#   - Tailscale running on all machines (coordinator + workers).
#   - Worker daemons started on N machines BEFORE running this script:
#       relativist worker --coordinator <TAILSCALE_IP>:9000 \
#           --token "<TOKEN>" --daemon
#   - Coordinator machine (running this script) can ping all workers
#     via Tailscale.
#
# Token handling:
#   - Set RELATIVIST_TOKEN env var to a fixed base64 token, or let the
#     script generate one with --token auto (default). Workers must use
#     the SAME token.
#
# On error the script stops immediately (set -e). Partial CSVs remain in
# place under results/locked/v1_lan_baseline/ for forensic review; raw
# per-run metrics.json files persist in raw/phase3/.
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
DATA_DIR="$REPO_DIR/data"
LOCKED_DIR="$REPO_DIR/results/locked/v1_lan_baseline"
RAW_DIR="$LOCKED_DIR/raw/phase3"

mkdir -p "$DATA_DIR" "$LOCKED_DIR" "$RAW_DIR"

# On Windows (Git Bash / MSYS2), convert paths for Python compatibility.
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

# Benchmark configurations: BENCHMARK_NAME:EXAMPLE_NET:SIZE
BENCHMARKS=(
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
TIMEOUT_SECS=1800   # 30 minutes per run

# Token: use env var or generate one for the campaign
TOKEN="${RELATIVIST_TOKEN:-}"
BIND="${RELATIVIST_BIND:-tailscale}"

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

# ---------------------------------------------------------------------------
# CSV Headers (matching Phase 1/2 format for direct comparison)
# ---------------------------------------------------------------------------

DETAIL_HEADER="benchmark,input_size,mode,workers,repetition,correct,wall_clock_secs,total_interactions,mips,rounds,speedup,efficiency,overhead_ratio,peak_memory_bytes,bytes_sent,bytes_received,con_con,dup_dup,era_era,con_dup,con_era,dup_era"
SUMMARY_HEADER="benchmark,input_size,mode,workers,repetitions,all_correct,wall_clock_mean,wall_clock_std,wall_clock_median,wall_clock_min,wall_clock_max,mips_mean,speedup_mean,efficiency_mean,overhead_ratio_mean,cv"
ROUNDS_HEADER="benchmark,input_size,workers,mode,repetition,round,partition_time_secs,compute_time_secs,merge_time_secs,network_time_secs,border_redexes,border_ratio,agents_at_start,bytes_sent,bytes_received"

DETAIL_FILE="$LOCKED_DIR/phase3_detail.csv"
SUMMARY_FILE="$LOCKED_DIR/phase3_summary.csv"
ROUNDS_FILE="$LOCKED_DIR/phase3_rounds.csv"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

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

print(f"$bench_name,$input_size,tcp_network,$workers,$rep,$is_correct,{total_secs:.6f},{total_int},{mips:.3f},{rounds},{speedup:.4f},{efficiency:.4f},{overhead:.4f},0,{bytes_s},{bytes_r},{con_con},{dup_dup},{era_era},{con_dup},{con_era},{dup_era}")
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

    print(f"$bench_name,$input_size,$workers,tcp_network,$rep,{r},{part_t:.6f},{comp_t:.6f},{merge_t:.6f},{net_t:.6f},{b_redex},{b_ratio:.6f},{agents},{bs},{br}")
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

# G1 verification via structural check.
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

run_tailscale_coordinator() {
    local workers="$1"
    local input_file="$2"
    local output_file="$3"
    local metrics_file="$4"
    local timeout_val="$5"

    local exit_code=0
    timeout "$timeout_val" "$RELATIVIST" coordinator \
        -w "$workers" \
        -i "$input_file" \
        -o "$output_file" \
        -m "$metrics_file" \
        --bind "$BIND" \
        --token "$TOKEN" \
        2>&1 | tail -30 || exit_code=$?

    return $exit_code
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
    log "=== Phase 3 Locked Baseline — v1_lan_baseline (Tailscale) ==="
    log "Binary:      $RELATIVIST"
    log "Output:      $LOCKED_DIR"
    log "Benchmarks:  ${#BENCHMARKS[@]} configurations"
    log "Workers:     ${WORKER_COUNTS[*]}"
    log "Warmup:      $WARMUP_RUNS"
    log "Repetitions: $REPETITIONS"
    log "Timeout:     ${TIMEOUT_SECS}s per run"
    log "Bind:        $BIND"

    # Generate or display campaign token
    if [ -z "$TOKEN" ]; then
        TOKEN=$(python3 -c "import secrets, base64; print(base64.b64encode(secrets.token_bytes(32)).decode())")
        log "Generated campaign token (use this for all workers):"
        log "  TOKEN=$TOKEN"
    else
        log "Using provided token from RELATIVIST_TOKEN"
    fi

    echo ""
    log "--- Worker daemon command (run on each worker machine) ---"
    log "  relativist worker --coordinator <THIS_MACHINE_TAILSCALE_IP>:9000 --token \"$TOKEN\" --daemon"
    echo ""

    if [ "$DRY_RUN" = true ]; then
        log "[DRY RUN] Would run ${#BENCHMARKS[@]} × ${#WORKER_COUNTS[@]} = $(( ${#BENCHMARKS[@]} * ${#WORKER_COUNTS[@]} )) configurations"
        log "[DRY RUN] Exiting."
        return
    fi

    if [ "$SKIP_BUILD" = false ]; then
        log "Building release binary..."
        (cd "$REPO_DIR" && cargo build --release 2>&1 | tail -3)
        log "Build complete."
    fi

    echo "$DETAIL_HEADER" > "$DETAIL_FILE"
    echo "$SUMMARY_HEADER" > "$SUMMARY_FILE"
    echo "$ROUNDS_HEADER" > "$ROUNDS_FILE"

    declare -A SEQ_BASELINES

    # ===== PHASE A: Sequential baselines (native, no network) =====
    if [ "$SKIP_SEQUENTIAL" = false ]; then
        log ""
        log "--- Phase A: Sequential Baselines (native) ---"

        for bench_spec in "${BENCHMARKS[@]}"; do
            IFS=':' read -r bench_name example_net input_size <<< "$bench_spec"

            local input_file="$DATA_DIR/bench_${bench_name}_${input_size}.bin"

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
            SEQ_BASELINES["${bench_name}:${input_size}"]="$median"

            write_summary_row "$bench_name" "$input_size" "sequential" "0" \
                "$REPETITIONS" "true" "$median" "${wall_clocks[@]}" >> "$SUMMARY_FILE"

            log "  Baseline: ${median}s (median of $REPETITIONS)"
        done
    else
        log "Skipping sequential baselines (--skip-sequential)"
        for bench_spec in "${BENCHMARKS[@]}"; do
            IFS=':' read -r bench_name _ input_size <<< "$bench_spec"
            SEQ_BASELINES["${bench_name}:${input_size}"]="1.0"
        done
    fi

    # ===== PHASE B: Tailscale (TcpNetwork) benchmarks =====
    log ""
    log "--- Phase B: Tailscale (TcpNetwork) Benchmarks ---"
    log "Ensure worker daemons are running on remote machines."
    echo ""

    local total_configs=$(( ${#BENCHMARKS[@]} * ${#WORKER_COUNTS[@]} ))
    local config_num=0

    for bench_spec in "${BENCHMARKS[@]}"; do
        IFS=':' read -r bench_name example_net input_size <<< "$bench_spec"

        local input_file="$DATA_DIR/bench_${bench_name}_${input_size}.bin"
        local seq_output="$DATA_DIR/seq_${bench_name}_${input_size}.bin"
        local seq_baseline="${SEQ_BASELINES["${bench_name}:${input_size}"]}"

        if [ ! -f "$input_file" ]; then
            "$RELATIVIST" generate "$example_net" -n "$input_size" -o "$input_file" 2>/dev/null
        fi

        for workers in "${WORKER_COUNTS[@]}"; do
            config_num=$((config_num + 1))
            log ""
            log "[$config_num/$total_configs] $bench_name size=$input_size workers=$workers"

            local wall_clocks=()
            local all_correct="true"
            local total_runs=$((WARMUP_RUNS + REPETITIONS))

            for ((run=0; run < total_runs; run++)); do
                local is_warmup=false
                local rep=$((run - WARMUP_RUNS))
                if [ "$run" -lt "$WARMUP_RUNS" ]; then
                    is_warmup=true
                fi

                local dist_output="$DATA_DIR/dist_output.bin"
                local dist_metrics="$DATA_DIR/dist_metrics.json"
                rm -f "$dist_output" "$dist_metrics"

                local exit_code=0
                run_tailscale_coordinator "$workers" "$input_file" "$dist_output" "$dist_metrics" "$TIMEOUT_SECS" || exit_code=$?

                if [ "$is_warmup" = true ]; then
                    log "  Warmup $((run+1))/$WARMUP_RUNS"
                    continue
                fi

                if [ "$exit_code" -ne 0 ] || [ ! -f "$dist_metrics" ]; then
                    log "  Rep $((rep+1)): FAILED (exit=$exit_code)"
                    all_correct="false"
                    echo "$bench_name,$input_size,tcp_network,$workers,$rep,false,0,0,0,0,0,0,1.0,0,0,0,0,0,0,0,0,0" >> "$DETAIL_FILE"
                    continue
                fi

                # Persist raw metrics
                local metrics_file="$RAW_DIR/metrics_${bench_name}_${input_size}_w${workers}_r${rep}.json"
                cp "$dist_metrics" "$metrics_file"

                local correct="true"
                if [ -f "$dist_output" ] && [ -f "$seq_output" ]; then
                    correct=$(verify_g1 "$seq_output" "$dist_output")
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
                write_summary_row "$bench_name" "$input_size" "tcp_network" "$workers" \
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

    log ""
    log "=========================================="
    log "  Phase 3 Locked Baseline Complete"
    log "=========================================="
    log "Output files:"
    log "  $DETAIL_FILE  ($(wc -l < "$DETAIL_FILE") rows)"
    log "  $SUMMARY_FILE ($(wc -l < "$SUMMARY_FILE") rows)"
    log "  $ROUNDS_FILE  ($(wc -l < "$ROUNDS_FILE") rows)"
    log "Raw metrics in: $RAW_DIR"
}

main "$@"
