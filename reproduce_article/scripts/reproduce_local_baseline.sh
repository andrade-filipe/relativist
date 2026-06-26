#!/usr/bin/env bash
# =============================================================================
# Reproduce v1_local_baseline on a different machine
# =============================================================================
# Runs Phase 1 and Phase 2 locked campaigns on the tagged binary and
# compares the reproduction outputs against the frozen v1_local_baseline.
#
# Usage:
#   bash scripts/reproduce_local_baseline.sh [--dry-run] [--phase 1|2|both]
#
# Outputs:
#   results/reproduction/<YYYY-MM-DD>/phase1_*.csv
#   results/reproduction/<YYYY-MM-DD>/phase2_*.csv
#   results/reproduction/<YYYY-MM-DD>/comparison.md
#
# Comparison policy:
#   - Row counts: MUST match exactly (across all CSVs).
#   - correct=true ratio: MUST match exactly (100% expected in both).
#   - Wall-clock columns: WILL differ by hardware — reported but not an
#     error condition.
#   - Rounds / bytes_sent / bytes_received: MUST match exactly (these
#     are structural, not temporal).
#
# Pre-conditions:
#   - Clean working copy at tag v0.10.0-bench.
#   - cargo build --release succeeded.
#   - Docker Desktop running (for Phase 2).
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

LOCKED_DIR="$REPO_DIR/reproduce_article/results/locked/v1_local_baseline"
REPRO_ROOT="$REPO_DIR/reproduce_article/results/reproduction"
REPRO_DATE="$(date +%Y-%m-%d)"
REPRO_DIR="$REPRO_ROOT/$REPRO_DATE"

DRY_RUN=false
PHASE="both"

for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        --phase)   shift; PHASE="${1:-both}" ;;
        --phase=*) PHASE="${arg#--phase=}" ;;
        *) echo "Unknown arg: $arg"; exit 1 ;;
    esac
done

log() { echo "[$(date +%H:%M:%S)] $*"; }

# Sanity: the frozen baseline must exist to compare against.
if [ ! -d "$LOCKED_DIR" ]; then
    echo "ERROR: $LOCKED_DIR does not exist. Cannot reproduce without a reference." >&2
    exit 1
fi

EXPECTED_TAG="v0.10.0-bench"
CURRENT_TAG="$(cd "$REPO_DIR" && git describe --tags --exact-match 2>/dev/null || echo "(no tag on HEAD)")"
if [ "$CURRENT_TAG" != "$EXPECTED_TAG" ]; then
    log "WARNING: HEAD is at '$CURRENT_TAG', expected '$EXPECTED_TAG'."
    log "         Wall-clock values will diverge; structural columns must still match."
fi

mkdir -p "$REPRO_DIR"

log "=== Reproduce v1_local_baseline ==="
log "Reference:     $LOCKED_DIR"
log "Reproduction:  $REPRO_DIR"
log "Date:          $REPRO_DATE"
log "Tag on HEAD:   $CURRENT_TAG"
log "Phase:         $PHASE"
echo ""

if [ "$DRY_RUN" = true ]; then
    log "[DRY RUN] Would run:"
    [ "$PHASE" = "1" ] || [ "$PHASE" = "both" ] && log "  scripts/bench_phase1_locked.sh -> $REPRO_DIR/phase1_*.csv"
    [ "$PHASE" = "2" ] || [ "$PHASE" = "both" ] && log "  scripts/bench_phase2_locked.sh -> $REPRO_DIR/phase2_*.csv"
    log "  comparison report -> $REPRO_DIR/comparison.md"
    exit 0
fi

# The locked scripts write to results/locked/v1_local_baseline/ by design.
# For reproduction we want the same filenames but under reproduction/<date>/.
# Strategy: run the scripts, then *move* the output files into REPRO_DIR and
# restore the locked dir if it was populated by the operator beforehand.
#
# Simpler approach: the locked scripts both use a hard-coded output path,
# so for reproduction we run them and immediately copy+delete. This assumes
# no other process is writing to $LOCKED_DIR concurrently.

run_phase1() {
    log ""
    log "--- Phase 1 reproduction ---"

    # Snapshot anything the operator might have in LOCKED_DIR already
    # (we don't want to clobber the frozen reference).
    local locked_backup=""
    if [ -f "$LOCKED_DIR/phase1_lenient_detail.csv" ]; then
        locked_backup="$LOCKED_DIR/.phase1_backup.$$"
        mkdir -p "$locked_backup"
        cp "$LOCKED_DIR"/phase1_*.csv "$locked_backup/" 2>/dev/null || true
        if [ -d "$LOCKED_DIR/raw/phase1" ]; then
            cp -r "$LOCKED_DIR/raw/phase1" "$locked_backup/raw_phase1" 2>/dev/null || true
        fi
    fi

    bash "$SCRIPT_DIR/bench_phase1_locked.sh"

    # Move freshly generated Phase 1 outputs into the reproduction dir.
    mkdir -p "$REPRO_DIR/raw"
    mv "$LOCKED_DIR"/phase1_*.csv "$REPRO_DIR/"
    if [ -d "$LOCKED_DIR/raw/phase1" ]; then
        mv "$LOCKED_DIR/raw/phase1" "$REPRO_DIR/raw/phase1"
    fi

    # Restore the frozen reference if we displaced it.
    if [ -n "$locked_backup" ]; then
        cp "$locked_backup"/phase1_*.csv "$LOCKED_DIR/" 2>/dev/null || true
        if [ -d "$locked_backup/raw_phase1" ]; then
            mkdir -p "$LOCKED_DIR/raw"
            cp -r "$locked_backup/raw_phase1" "$LOCKED_DIR/raw/phase1"
        fi
        rm -rf "$locked_backup"
    fi
}

run_phase2() {
    log ""
    log "--- Phase 2 reproduction ---"

    local locked_backup=""
    if [ -f "$LOCKED_DIR/phase2_detail.csv" ]; then
        locked_backup="$LOCKED_DIR/.phase2_backup.$$"
        mkdir -p "$locked_backup"
        cp "$LOCKED_DIR"/phase2_*.csv "$locked_backup/" 2>/dev/null || true
        if [ -d "$LOCKED_DIR/raw/phase2" ]; then
            cp -r "$LOCKED_DIR/raw/phase2" "$locked_backup/raw_phase2" 2>/dev/null || true
        fi
    fi

    bash "$SCRIPT_DIR/bench_phase2_locked.sh"

    mkdir -p "$REPRO_DIR/raw"
    mv "$LOCKED_DIR"/phase2_*.csv "$REPRO_DIR/"
    if [ -d "$LOCKED_DIR/raw/phase2" ]; then
        mv "$LOCKED_DIR/raw/phase2" "$REPRO_DIR/raw/phase2"
    fi

    if [ -n "$locked_backup" ]; then
        cp "$locked_backup"/phase2_*.csv "$LOCKED_DIR/" 2>/dev/null || true
        if [ -d "$locked_backup/raw_phase2" ]; then
            mkdir -p "$LOCKED_DIR/raw"
            cp -r "$locked_backup/raw_phase2" "$LOCKED_DIR/raw/phase2"
        fi
        rm -rf "$locked_backup"
    fi
}

compare_csvs() {
    log ""
    log "--- Comparison ---"

    local report="$REPRO_DIR/comparison.md"
    {
        echo "# Reproduction comparison — $REPRO_DATE"
        echo ""
        echo "- **Reference:** \`$LOCKED_DIR\`"
        echo "- **Reproduction:** \`$REPRO_DIR\`"
        echo "- **Tag on HEAD during reproduction:** $CURRENT_TAG"
        echo ""
        echo "## Structural match (row counts, correctness)"
        echo ""
        echo "| File | ref rows | repro rows | match | ref correct=true | repro correct=true | match |"
        echo "|---|---|---|---|---|---|---|"
    } > "$report"

    local csv_files=()
    [ "$PHASE" = "1" ] || [ "$PHASE" = "both" ] && csv_files+=(phase1_lenient_detail.csv phase1_strict_detail.csv)
    [ "$PHASE" = "2" ] || [ "$PHASE" = "both" ] && csv_files+=(phase2_detail.csv)

    for csv in "${csv_files[@]}"; do
        local ref="$LOCKED_DIR/$csv"
        local rep="$REPRO_DIR/$csv"
        if [ ! -f "$ref" ] || [ ! -f "$rep" ]; then
            echo "| \`$csv\` | - | - | MISSING | - | - | - |" >> "$report"
            continue
        fi

        local ref_rows rep_rows
        ref_rows=$(( $(wc -l < "$ref") - 1 ))
        rep_rows=$(( $(wc -l < "$rep") - 1 ))

        local rows_match="OK"
        [ "$ref_rows" != "$rep_rows" ] && rows_match="DIFF"

        local ref_correct rep_correct
        ref_correct=$(awk -F, 'NR>1 && $6=="true"' "$ref" | wc -l)
        rep_correct=$(awk -F, 'NR>1 && $6=="true"' "$rep" | wc -l)

        local correct_match="OK"
        [ "$ref_correct" != "$rep_correct" ] && correct_match="DIFF"

        echo "| \`$csv\` | $ref_rows | $rep_rows | $rows_match | $ref_correct | $rep_correct | $correct_match |" >> "$report"
    done

    {
        echo ""
        echo "## Notes"
        echo ""
        echo "- Wall-clock values (\`wall_clock_secs\`, \`mean\`, \`median\`, \`cv\`) are expected to differ"
        echo "  between machines and are not compared here."
        echo "- Row counts and \`correct=true\` counts MUST match; any DIFF is a blocker."
        echo "- Structural columns (\`rounds\`, \`bytes_sent\`, \`bytes_received\`, interaction counts) should"
        echo "  match exactly. Compare manually via \`diff <(cut -d, -f1-5,8,10,15,16,17-22 ref.csv) <(cut ...)\`"
        echo "  if any DIFF appears above."
    } >> "$report"

    log "Comparison report: $report"
    cat "$report"
}

if [ "$PHASE" = "1" ] || [ "$PHASE" = "both" ]; then
    run_phase1
fi

if [ "$PHASE" = "2" ] || [ "$PHASE" = "both" ]; then
    run_phase2
fi

compare_csvs

log ""
log "=== Reproduction complete ==="
log "Artifacts: $REPRO_DIR"
