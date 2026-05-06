# TEST-SPEC-0706: Tests for TASK-0706 — Methodology docs page

**Task:** TASK-0706
**Spec:** none
**Bundle:** D-014 (Stress Curve Campaign)
**Requirements covered:** Acceptance criteria 1-10 from TASK-0706
**Test IDs:** none — docs-only TASK; **+0 test floor delta**

---

## Scope

TASK-0706 is documentation-only. It produces:
- `docs/benchmarks/campaigns/stress-curve.md` (~250 lines markdown)
- ~2-line additions to `docs/benchmarks/README.md`
- ~2-line additions to `docs/INDEX.md`

There is **no Rust code change** and therefore no `cargo test` delta. TASK-0706 acceptance criterion 10 explicitly says "cargo test floors unchanged from TASK-0705's cumulative numbers (≥ 1813 default, etc.)".

This TEST-SPEC documents the **review checklist** that REVIEWER (Stage 4) will use to validate the docs page, since there are no automated tests to write. It is included in this directory for inventory parity (8 TEST-SPECs for 8 dev-bearing TASKs out of 9, matching the user's directive — TASK-0708 is overnight-run so has no TEST-SPEC).

## Test category & location

| # | Name | Category | Where |
|---|------|----------|-------|
| RC-0706-01 | Methodology page review checklist | manual review | applied at Stage 4 (REVIEWER) |

## Test floor delta

**+0** — docs only. All cumulative floors carry over from TASK-0705:
- default ≥ 1813
- zero-copy ≥ 1857
- streaming-no-recycle ≥ 1804
- release ≥ 1755

---

## Manual review checklist

The reviewer applies these checks against `docs/benchmarks/campaigns/stress-curve.md` (and the two index touch-ups). Each check maps to a TASK-0706 acceptance criterion.

### RC-0706-01: Page structure (AC-1)

- [ ] File exists at `docs/benchmarks/campaigns/stress-curve.md`.
- [ ] Length ≥ 200 lines markdown.
- [ ] Sections appear in the exact order specified by TASK-0706:
      1. Research question
      2. Scope
      3. Components
      4. CSV schema
      5. How to reproduce (5.1 Pre-conditions, 5.2 Smoke run, 5.3 Full overnight run)
      6. Lock procedure
      7. Failure modes
      8. Sanity checks
      9. Limitations
      10. Cross-references

### RC-0706-02: Component table (AC-2)

- [ ] Section 3 lists all 7 components from design doc §4.2 with their TASK numbers (TASK-0700 through TASK-0706).
- [ ] Each row links to the component's source file path.
- [ ] No "TBD" placeholders.

### RC-0706-03: CSV schema documentation (AC-3)

- [ ] Section 4 documents all 4 new TASK-0703 columns with type + source:
      - `vmrss_peak_mb` (f64, MiB) — `MemoryProbe::peak_bytes / 1_048_576`
      - `vmrss_current_end_mb` (f64, MiB) — `MemoryProbe::current_bytes / 1_048_576`
      - `stop_reason` (String, `""` for normal) — `StopReason` serde
      - `cv_above_gate` (bool) — `(stddev/mean) > 0.05`
- [ ] States the columns are append-only at the END of the row.
- [ ] Notes the SPEC-09 R18a column `peak_memory_at_construction_mb` is preserved (different semantics).

### RC-0706-04: Reproduction commands (AC-4)

- [ ] Section 5.2 contains exactly: `scripts/stress_curve.sh --smoke`
- [ ] Section 5.3 contains exactly: `scripts/stress_curve.sh` (no flags)
- [ ] Both commands match TASK-0704's CLI verbatim (no aspirational flags).
- [ ] Pre-condition list in Section 5.1 mirrors TASK-0704's gate checklist (12 items).

### RC-0706-05: Index touch-ups (AC-5, AC-6)

- [ ] `docs/benchmarks/README.md` gains a row pointing to the new page in the campaigns index table.
- [ ] `docs/INDEX.md` gains a "Benchmark Results > Stress Curve (v2)" entry pointing to the new page.

### RC-0706-06: Markdown lint (AC-7)

- [ ] `mdformat --check docs/benchmarks/campaigns/stress-curve.md` exit 0 (or `markdownlint` if that's what the project uses — check `package.json` / pre-commit hooks).

### RC-0706-07: Cross-reference style (AC-8)

- [ ] All TASK references use the form `TASK-NNNN` consistently (no `task-0700`, no `Task 700`).

### RC-0706-08: No placeholders (AC-9)

- [ ] Page contains zero of: `TBD`, `TODO`, `XXX`, `FIXME`, `???`, `[placeholder]`. (`grep -E "TBD|TODO|XXX|FIXME|\?\?\?|\[placeholder\]" docs/benchmarks/campaigns/stress-curve.md` returns no matches.)

### RC-0706-09: Failure-modes coverage (Implementation hint #3)

- [ ] Section 7 explicitly mentions `--resume` semantics:
      - interrupted runs can resume via `--resume`
      - malformed (mid-row truncated) CSV is detected; `--resume` refuses with exit 1.

### RC-0706-10: MANIFEST template (Implementation hint #4)

- [ ] Section 5.3 lists exact MANIFEST.md fields:
      - git rev SHA
      - rustc version
      - `/proc/meminfo` snapshot (Linux) / `systeminfo` (Windows)
      - `/proc/cpuinfo` model name (Linux) / `wmic cpu` (Windows)
      - total reps (count)
      - total wall (HH:MM:SS)
      - median CV across reps
      - stop_reason histogram (counts per variant)

### RC-0706-11: Limitations lead sentence (Implementation hint #5)

- [ ] Section 9's lead sentence is: "the campaign characterizes the wall, it does not remove it" (or strict paraphrase preserving meaning).

### RC-0706-12: Floor invariants (AC-10)

- [ ] No `cargo test`, `cargo clippy`, or `cargo fmt` runs are needed because the change is markdown-only. Floor invariants from TASK-0705 carry over unchanged. REVIEWER spot-checks by running `cargo test` once and confirming it still passes ≥ 1813.

---

## Out of scope

- Automated tests (none possible / required for docs-only).
- LaTeX article integration — REDATOR territory; out of scope here.
- Translation to PT-BR — out of scope; `docs/benchmarks/campaigns/` is English-only.

## Cross-references

- TASK-0706 (this TEST-SPEC's parent).
- Design doc: `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md`.
- Sibling pages: `docs/benchmarks/campaigns/{v1-local-baseline.md, v1-stress.md, church-sum-of-squares.md}`.
- Floor table: `docs/backlog/BACKLOG.md` Cumulative test floors section.
