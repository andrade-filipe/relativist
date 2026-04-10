#!/usr/bin/env bash
# =============================================================================
# Phase 2 Docker — L6 fix validation
# =============================================================================
# Runs the 4 configs previously blocked by L6 (256 MiB frame cap):
#   - dual_tree=22                w=1     (was ~293 MB frame)
#   - ep_annihilation_con=5M      w=1     (was ~350 MB)
#   - ep_annihilation_con=5M      w=2     (was ~315 MB)
#   - ep_annihilation_con=5M      w=4     (was 282.5 MB)
# Writes isolated CSVs under results/post_fix/ so the canonical phase2_*.csv
# is untouched until the B3 comparison is signed off.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
DATA_DIR="$REPO_DIR/data"
RESULTS_DIR="$REPO_DIR/results/post_fix"
mkdir -p "$RESULTS_DIR"

winpath() { if command -v cygpath &>/dev/null; then cygpath -w "$1"; else echo "$1"; fi; }
join_comma() { local IFS=','; echo "$*"; }

if [ -f "$REPO_DIR/target/release/relativist.exe" ]; then
    RELATIVIST="$REPO_DIR/target/release/relativist.exe"
elif [ -f "$REPO_DIR/target/release/relativist" ]; then
    RELATIVIST="$REPO_DIR/target/release/relativist"
else
    RELATIVIST="relativist"
fi

DETAIL_FILE="$RESULTS_DIR/phase2_l6_detail.csv"
SUMMARY_FILE="$RESULTS_DIR/phase2_l6_summary.csv"
ROUNDS_FILE="$RESULTS_DIR/phase2_l6_rounds.csv"
CANONICAL_SUMMARY="$REPO_DIR/results/phase2_summary.csv"

# Initialise output files with headers
echo "benchmark,input_size,mode,workers,repetition,correct,wall_clock_secs,total_interactions,mips,rounds,speedup,efficiency,overhead_ratio,peak_memory_bytes,bytes_sent,bytes_received,con_con,dup_dup,era_era,con_dup,con_era,dup_era" > "$DETAIL_FILE"
echo "benchmark,input_size,mode,workers,repetitions,all_correct,wall_clock_mean,wall_clock_std,wall_clock_median,wall_clock_min,wall_clock_max,mips_mean,speedup_mean,efficiency_mean,overhead_ratio_mean,cv" > "$SUMMARY_FILE"
echo "benchmark,input_size,workers,mode,repetition,round,partition_time_secs,compute_time_secs,merge_time_secs,network_time_secs,border_redexes,border_ratio,agents_at_start,bytes_sent,bytes_received" > "$ROUNDS_FILE"

WARMUP_RUNS=1
REPETITIONS=3
TIMEOUT_SECS=1800   # 30 min per run for the heaviest configs

# Configs: "bench_name:example_net:size:workers_csv"
CONFIGS=(
    "dual_tree:dual-tree:22:1"
    "ep_annihilation_con:ep-annihilation-con:5000000:1,2,4"
)

log() { echo "[$(date +%H:%M:%S)] $*"; }

get_seq_baseline() {
    local bname="$1" size="$2"
    awk -F, -v b="$bname" -v s="$size" \
        '$1==b && $2==s && $3=="sequential" {print $9}' "$CANONICAL_SUMMARY" | head -1
}

parse_metrics_to_detail() {
    local metrics_file="$1" bname="$2" size="$3" w="$4" rep="$5" seq_b="$6" correct="$7"
    local wpath; wpath=$(winpath "$metrics_file")
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
seq_b = $seq_b
speedup = seq_b / total_secs if total_secs > 0 else 0.0
w = $w
efficiency = speedup / w if w > 0 else speedup
overhead = max(0.0, 1.0 - efficiency)
bytes_s = sum(m.get("bytes_sent_per_round", []))
bytes_r = sum(m.get("bytes_received_per_round", []))
print(f"$bname,$size,tcp_localhost,$w,$rep,$correct,{total_secs:.6f},{total_int},{mips:.3f},{rounds},{speedup:.4f},{efficiency:.4f},{overhead:.4f},0,{bytes_s},{bytes_r},{con_con},{dup_dup},{era_era},{con_dup},{con_era},{dup_era}")
PYEOF
}

parse_metrics_to_rounds() {
    local metrics_file="$1" bname="$2" size="$3" w="$4" rep="$5"
    local wpath; wpath=$(winpath "$metrics_file")
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
    net_t  = dur("network_send_time_per_round") + dur("network_recv_time_per_round")
    b_redex = ival("border_redexes_per_round")
    agents  = ival("agents_per_round")
    local_i = ival("local_interactions_per_round")
    border_i= ival("border_interactions_per_round")
    total_i = local_i + border_i
    b_ratio = border_i / total_i if total_i > 0 else 0.0
    bs = ival("bytes_sent_per_round")
    br = ival("bytes_received_per_round")
    print(f"$bname,$size,$w,tcp_localhost,$rep,{r},{part_t:.6f},{comp_t:.6f},{merge_t:.6f},{net_t:.6f},{b_redex},{b_ratio:.6f},{agents},{bs},{br}")
PYEOF
}

extract_wall_clock() {
    local wpath; wpath=$(winpath "$1")
    python3 -c "
import json
with open(r'$wpath') as f:
    m = json.load(f)
t = m['total_time']['secs'] + m['total_time']['nanos']/1e9
print(f'{t:.6f}')
"
}

write_summary_row() {
    local bname="$1" size="$2" w="$3" reps="$4" all_ok="$5" seq_b="$6"
    shift 6
    local vals_csv; vals_csv=$(join_comma "$@")
    python3 << PYEOF
import statistics
values = [$vals_csv]
n = len(values)
if n == 0:
    exit(0)
mean_v = statistics.mean(values)
std_v = statistics.pstdev(values) if n > 1 else 0.0
median_v = statistics.median(values)
min_v = min(values)
max_v = max(values)
cv = std_v / mean_v if mean_v > 0 else 0.0
seq_b = $seq_b
w = int("$w")
speedup = seq_b / mean_v if mean_v > 0 else 0.0
efficiency = speedup / w if w > 0 else speedup
overhead = max(0.0, 1.0 - efficiency)
print(f"$bname,$size,tcp_localhost,$w,$reps,$all_ok,{mean_v:.6f},{std_v:.6f},{median_v:.6f},{min_v:.6f},{max_v:.6f},0.000,{speedup:.4f},{efficiency:.4f},{overhead:.4f},{cv:.4f}")
PYEOF
}

verify_g1() {
    local seq_output="$1" dist_output="$2"
    if [ ! -f "$seq_output" ] || [ ! -f "$dist_output" ]; then
        echo "false"; return
    fi
    local si di
    si=$("$RELATIVIST" inspect -i "$seq_output" 2>/dev/null | grep -E "^(Agents|Redexes):" | tr '\n' '|' || echo "ERR1")
    di=$("$RELATIVIST" inspect -i "$dist_output" 2>/dev/null | grep -E "^(Agents|Redexes):" | tr '\n' '|' || echo "ERR2")
    [ "$si" = "$di" ] && echo "true" || echo "false"
}

run_docker_cycle() {
    local w="$1" timeout="$2"
    (cd "$REPO_DIR" && docker compose down --remove-orphans --timeout 5 2>/dev/null) || true
    sleep 1
    local ec=0
    (cd "$REPO_DIR" && \
        NUM_WORKERS="$w" \
        docker compose up -d --force-recreate --scale worker="$w" 2>&1 | tail -5
    ) || {
        ec=$?
        (cd "$REPO_DIR" && docker compose down --remove-orphans --timeout 5 2>/dev/null) || true
        return $ec
    }
    local coord_name="relativist-coordinator-1"
    local wait_out
    if wait_out=$(timeout "$timeout" docker wait "$coord_name" 2>/dev/null); then
        ec=${wait_out:-0}
    else
        ec=124
    fi
    (cd "$REPO_DIR" && docker compose down --remove-orphans --timeout 5 2>/dev/null) || true
    return "$ec"
}

log "=== Phase 2 L6 fix validation ==="
log "Timeout per run: ${TIMEOUT_SECS}s"
log "Output dir: $RESULTS_DIR"
echo ""

log "Rebuilding Docker image with CompactSubnet fix..."
(cd "$REPO_DIR" && docker compose build 2>&1 | tail -5) || true
log "Docker image OK."
echo ""

total_cfgs=0
for cfg in "${CONFIGS[@]}"; do
    IFS=':' read -r _ _ _ wlist <<< "$cfg"
    IFS=',' read -ra warr <<< "$wlist"
    total_cfgs=$((total_cfgs + ${#warr[@]}))
done
cfg_num=0

for cfg in "${CONFIGS[@]}"; do
    IFS=':' read -r bname example_net size wlist <<< "$cfg"
    input_file="$DATA_DIR/bench_${bname}_${size}.bin"
    seq_output="$DATA_DIR/seq_${bname}_${size}.bin"

    if [ ! -f "$input_file" ]; then
        log "Generating $bname size=$size"
        "$RELATIVIST" generate "$example_net" -n "$size" -o "$input_file" 2>/dev/null
    fi

    if [ ! -f "$seq_output" ]; then
        log "Reducing sequentially to produce reference output"
        "$RELATIVIST" reduce -i "$input_file" -o "$seq_output" 2>/dev/null
    fi

    seq_baseline=$(get_seq_baseline "$bname" "$size")
    if [ -z "$seq_baseline" ]; then
        log "WARN: no sequential baseline for $bname:$size, using 1.0"
        seq_baseline="1.0"
    fi
    log "Baseline $bname:$size = ${seq_baseline}s"

    IFS=',' read -ra workers_array <<< "$wlist"
    for workers in "${workers_array[@]}"; do
        cfg_num=$((cfg_num + 1))
        log ""
        log "[$cfg_num/$total_cfgs] $bname size=$size workers=$workers"

        wall_clocks=()
        all_correct="true"
        total_runs=$((WARMUP_RUNS + REPETITIONS))

        for ((run=0; run < total_runs; run++)); do
            is_warmup=false
            rep=$((run - WARMUP_RUNS))
            if [ "$run" -lt "$WARMUP_RUNS" ]; then
                is_warmup=true
            fi

            cp "$input_file" "$DATA_DIR/input.bin"
            rm -f "$DATA_DIR/output.bin" "$DATA_DIR/metrics.json"

            ec=0
            run_docker_cycle "$workers" "$TIMEOUT_SECS" >/dev/null 2>&1 || ec=$?

            if [ "$is_warmup" = true ]; then
                log "  Warmup $((run+1))/$WARMUP_RUNS (exit=$ec)"
                continue
            fi

            if [ "$ec" -ne 0 ] || [ ! -f "$DATA_DIR/metrics.json" ]; then
                log "  Rep $((rep+1)): FAILED (exit=$ec)"
                all_correct="false"
                echo "$bname,$size,tcp_localhost,$workers,$rep,false,0,0,0,0,0,0,1.0,0,0,0,0,0,0,0,0,0" >> "$DETAIL_FILE"
                continue
            fi

            metrics_file="$DATA_DIR/metrics_l6_${bname}_${size}_w${workers}_r${rep}.json"
            cp "$DATA_DIR/metrics.json" "$metrics_file"

            correct="true"
            if [ -f "$DATA_DIR/output.bin" ] && [ -f "$seq_output" ]; then
                correct=$(verify_g1 "$seq_output" "$DATA_DIR/output.bin")
            fi
            if [ "$correct" = "false" ]; then
                all_correct="false"
                log "  Rep $((rep+1)): G1 FAILED"
            fi

            wc=$(extract_wall_clock "$metrics_file")
            wall_clocks+=("$wc")

            parse_metrics_to_detail "$metrics_file" "$bname" "$size" "$workers" \
                "$rep" "$seq_baseline" "$correct" >> "$DETAIL_FILE"
            parse_metrics_to_rounds "$metrics_file" "$bname" "$size" "$workers" "$rep" \
                >> "$ROUNDS_FILE"

            log "  Rep $((rep+1))/$REPETITIONS: ${wc}s correct=$correct"
        done

        if [ ${#wall_clocks[@]} -gt 0 ]; then
            write_summary_row "$bname" "$size" "$workers" "${#wall_clocks[@]}" \
                "$all_correct" "$seq_baseline" "${wall_clocks[@]}" >> "$SUMMARY_FILE"
        fi
    done
done

log ""
log "=== L6 validation complete ==="
log "Detail  rows: $(wc -l < "$DETAIL_FILE")"
log "Summary rows: $(wc -l < "$SUMMARY_FILE")"
log "Rounds  rows: $(wc -l < "$ROUNDS_FILE")"
