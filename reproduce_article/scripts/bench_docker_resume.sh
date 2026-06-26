#!/usr/bin/env bash
# Resume: complete condup_expansion 5000 workers=4 (reps 6-9) and workers=8 (reps 0-9)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
DATA_DIR="$REPO_DIR/data"
RESULTS_DIR="$REPO_DIR/reproduce_article/results"
RELATIVIST="$REPO_DIR/target/release/relativist.exe"

winpath() {
    if command -v cygpath &>/dev/null; then cygpath -w "$1"; else echo "$1"; fi
}

join_comma() { local IFS=','; echo "$*"; }

DETAIL_FILE="$RESULTS_DIR/phase2_detail.csv"
SUMMARY_FILE="$RESULTS_DIR/phase2_summary.csv"
ROUNDS_FILE="$RESULTS_DIR/phase2_rounds.csv"

BENCH_NAME="condup_expansion"
INPUT_SIZE=5000
INPUT_FILE="$DATA_DIR/bench_condup_expansion_5000.bin"
SEQ_OUTPUT="$DATA_DIR/seq_condup_expansion_5000.bin"
SEQ_BASELINE=0.267651  # median from completed sequential runs

WARMUP_RUNS=2
TIMEOUT_SECS=600

parse_detail() {
    local mf="$1" w="$2" r="$3" correct="$4"
    local wp=$(winpath "$mf")
    python3 << PYEOF
import json
with open(r"$wp") as f:
    m = json.load(f)
rounds = m["rounds"]
total_int = m["total_interactions"]
t = m["total_time"]["secs"] + m["total_time"]["nanos"] / 1e9
mips = total_int / t / 1e6 if t > 0 else 0
rules = m["total_interactions_by_rule"]
seq = $SEQ_BASELINE
sp = seq / t if t > 0 else 0
eff = sp / $w if $w > 0 else sp
oh = max(0, 1 - eff)
bs = sum(m.get("bytes_sent_per_round", []))
br = sum(m.get("bytes_received_per_round", []))
print(f"$BENCH_NAME,$INPUT_SIZE,tcp_localhost,$w,$r,$correct,{t:.6f},{total_int},{mips:.3f},{rounds},{sp:.4f},{eff:.4f},{oh:.4f},0,{bs},{br},{rules[0]},{rules[3]},{rules[5]},{rules[1]},{rules[2]},{rules[4]}")
PYEOF
}

parse_rounds() {
    local mf="$1" w="$2" r="$3"
    local wp=$(winpath "$mf")
    python3 << PYEOF
import json
with open(r"$wp") as f:
    m = json.load(f)
for rnd in range(m["rounds"]):
    def dur(f):
        v=m.get(f,[]);return v[rnd]["secs"]+v[rnd]["nanos"]/1e9 if rnd<len(v) else 0
    def iv(f):
        v=m.get(f,[]);return v[rnd] if rnd<len(v) else 0
    pt=dur("partition_time_per_round");ct=dur("compute_time_per_round")
    mt=dur("merge_time_per_round")
    nt=dur("network_send_time_per_round")+dur("network_recv_time_per_round")
    br2=iv("border_redexes_per_round");ag=iv("agents_per_round")
    li=iv("local_interactions_per_round");bi=iv("border_interactions_per_round")
    ti=li+bi;brat=bi/ti if ti>0 else 0
    bs=iv("bytes_sent_per_round");brc=iv("bytes_received_per_round")
    print(f"$BENCH_NAME,$INPUT_SIZE,$w,tcp_localhost,$r,{rnd},{pt:.6f},{ct:.6f},{mt:.6f},{nt:.6f},{br2},{brat:.6f},{ag},{bs},{brc}")
PYEOF
}

extract_wc() {
    local wp=$(winpath "$1")
    python3 -c "
import json
with open(r'$wp') as f:
    m=json.load(f)
print(f'{m[\"total_time\"][\"secs\"]+m[\"total_time\"][\"nanos\"]/1e9:.6f}')
"
}

run_docker() {
    local w="$1"
    (cd "$REPO_DIR" && docker compose down --remove-orphans 2>/dev/null) || true
    (cd "$REPO_DIR" && NUM_WORKERS="$w" timeout "$TIMEOUT_SECS" \
        docker compose up --abort-on-container-exit --exit-code-from coordinator \
        --scale worker="$w" 2>&1 | tail -15)
    local rc=$?
    (cd "$REPO_DIR" && docker compose down --remove-orphans 2>/dev/null) || true
    return $rc
}

verify_g1() {
    local s="$1" d="$2"
    [ ! -f "$s" ] || [ ! -f "$d" ] && { echo "false"; return; }
    local si=$("$RELATIVIST" inspect -i "$s" 2>/dev/null | grep -E "^(Agents|Redexes):" | tr '\n' '|')
    local di=$("$RELATIVIST" inspect -i "$d" 2>/dev/null | grep -E "^(Agents|Redexes):" | tr '\n' '|')
    [ "$si" = "$di" ] && echo "true" || echo "false"
}

echo "=== Resume: condup_expansion 5000 ==="
echo ""

# Config 1: workers=4, reps 6-9 (6 already done)
for workers in 4 8; do
    if [ "$workers" = "4" ]; then
        START_REP=6; END_REP=9; WARMUPS=0
    else
        START_REP=0; END_REP=9; WARMUPS=$WARMUP_RUNS
    fi

    echo "--- workers=$workers, reps $START_REP-$END_REP ---"

    wall_clocks=()
    all_correct="true"

    # Warmup (only for workers=8)
    for ((w=0; w<WARMUPS; w++)); do
        cp "$INPUT_FILE" "$DATA_DIR/input.bin"
        rm -f "$DATA_DIR/output.bin" "$DATA_DIR/metrics.json"
        run_docker "$workers" >/dev/null 2>&1 || true
        echo "  Warmup $((w+1))/$WARMUPS"
    done

    for ((rep=START_REP; rep<=END_REP; rep++)); do
        cp "$INPUT_FILE" "$DATA_DIR/input.bin"
        rm -f "$DATA_DIR/output.bin" "$DATA_DIR/metrics.json"

        ec=0
        run_docker "$workers" || ec=$?

        if [ "$ec" -ne 0 ] || [ ! -f "$DATA_DIR/metrics.json" ]; then
            echo "  Rep $rep: FAILED"
            all_correct="false"
            echo "$BENCH_NAME,$INPUT_SIZE,tcp_localhost,$workers,$rep,false,0,0,0,0,0,0,1.0,0,0,0,0,0,0,0,0,0" >> "$DETAIL_FILE"
            continue
        fi

        mf="$DATA_DIR/metrics_resume_w${workers}_r${rep}.json"
        cp "$DATA_DIR/metrics.json" "$mf"

        correct=$(verify_g1 "$SEQ_OUTPUT" "$DATA_DIR/output.bin")
        [ "$correct" = "false" ] && all_correct="false"

        wc=$(extract_wc "$mf")
        wall_clocks+=("$wc")

        parse_detail "$mf" "$workers" "$rep" "$correct" >> "$DETAIL_FILE"
        parse_rounds "$mf" "$workers" "$rep" >> "$ROUNDS_FILE"

        echo "  Rep $rep: ${wc}s correct=$correct"
    done

    # Summary for workers=8 (full 10 reps)
    if [ "$workers" = "8" ] && [ ${#wall_clocks[@]} -gt 0 ]; then
        python3 << PYEOF
import statistics
values = [$(join_comma "${wall_clocks[@]}")]
n = len(values)
mean_v = statistics.mean(values)
std_v = statistics.pstdev(values)
median_v = statistics.median(values)
min_v = min(values)
max_v = max(values)
cv = std_v / mean_v if mean_v > 0 else 0
seq = $SEQ_BASELINE
sp = seq / mean_v if mean_v > 0 else 0
eff = sp / 8
oh = max(0, 1 - eff)
print(f"$BENCH_NAME,$INPUT_SIZE,tcp_localhost,8,{n},${all_correct},{mean_v:.6f},{std_v:.6f},{median_v:.6f},{min_v:.6f},{max_v:.6f},0.000,{sp:.4f},{eff:.4f},{oh:.4f},{cv:.4f}")
PYEOF
    fi

    # Summary for workers=4 (merge existing 6 reps + new 4 reps)
    if [ "$workers" = "4" ] && [ ${#wall_clocks[@]} -gt 0 ]; then
        # Get existing 6 reps from detail CSV
        existing=$(awk -F, '$1=="condup_expansion" && $2=="5000" && $3=="tcp_localhost" && $4=="4" {print $7}' "$DETAIL_FILE" | head -6)
        all_wc="$existing"$'\n'"$(printf '%s\n' "${wall_clocks[@]}")"

        python3 << PYEOF
import statistics
values = [float(v) for v in """$all_wc""".strip().split('\n') if v.strip()]
n = len(values)
mean_v = statistics.mean(values)
std_v = statistics.pstdev(values)
median_v = statistics.median(values)
min_v = min(values)
max_v = max(values)
cv = std_v / mean_v if mean_v > 0 else 0
seq = $SEQ_BASELINE
sp = seq / mean_v if mean_v > 0 else 0
eff = sp / 4
oh = max(0, 1 - eff)
print(f"$BENCH_NAME,$INPUT_SIZE,tcp_localhost,4,{n},${all_correct},{mean_v:.6f},{std_v:.6f},{median_v:.6f},{min_v:.6f},{max_v:.6f},0.000,{sp:.4f},{eff:.4f},{oh:.4f},{cv:.4f}")
PYEOF
    fi
done >> "$SUMMARY_FILE"

echo ""
echo "=== Resume Complete ==="
echo "Detail rows: $(wc -l < "$DETAIL_FILE")"
echo "Summary rows: $(wc -l < "$SUMMARY_FILE")"
