# Relativist Implementation Backlog

**Last updated:** 2026-05-06 (D-014 Stress Curve Campaign + SPEC-27 v3 Topic 2 Encoder/HornerCodec; 20 tasks active TASK-0700..0719).

**Status:** 20 active TASKs across two bundles:
- **D-014 (Stress Curve Campaign):** TASK-0700..0708 (9 tasks; Topic 1).
- **SPEC-27 v3 (Encoder/Decoder API + HornerCodec):** TASK-0709..0719 (11 tasks; Topic 2). Stage 1 splitting completed 2026-05-06 from `specs/SPEC-27-encoder-decoder-api.md` Revised v3 (Round 2 spec-critic response closed all 13 issues).

The full inventory of D-005..D-012 atomic tasks (TASK-0001..TASK-0618 with intentional gaps) is preserved at `archive/`. Numbering gap 0619-0699 reserved for any intermediate bundles between D-012 and D-014.

**Pipeline:** See `../WORKFLOWS.md` (§1 Development Pipeline) for the 6-stage SDD process.

---

## Active

### D-014 — Stress Curve Campaign (Topic 1)

| ID | Title | Priority | Status | Depends | Complexity | Bundle |
|----|-------|----------|--------|---------|------------|--------|
| TASK-0700 | Cross-platform `MemoryProbe` (current + peak + RAM fraction) | P0 | TODO | none | S–M (~180 LoC) | D-014 |
| TASK-0701 | `StopRule` (wall-time / RAM / OOM sequence aborter) | P0 | TODO | TASK-0700 | S–M (~170 LoC) | D-014 |
| TASK-0702 | `stress-curve` campaign descriptor in `bench/suite.rs` | P0 | TODO | TASK-0700, TASK-0701 | S–M (~170 LoC) | D-014 |
| TASK-0703 | CSV schema extension (`vmrss_*`, `stop_reason`, `cv_above_gate`) | P1 | TODO | TASK-0700, TASK-0701 | S (~60 LoC) | D-014 |
| TASK-0704 | `scripts/stress_curve.sh` Phase 1 + Phase 2 orchestrator | P0 | TODO | TASK-0700..0703 | M (~230 LoC) | D-014 |
| TASK-0705 | `scripts/plot_stress_curve.py` (9 PDFs + summary) | P1 | TODO | TASK-0703 | M (~230 LoC) | D-014 |
| TASK-0706 | `docs/benchmarks/campaigns/stress-curve.md` methodology page | P1 | TODO | TASK-0700..0705 | S–M (~250 lines md, 0 LoC) | D-014 |
| TASK-0707 | 6 integration tests for stress_curve_*.rs | P0 | TODO | TASK-0700..0703 | M (~200 LoC) | D-014 |
| TASK-0708 | Full campaign overnight + lock dir + INDEX/ROADMAP/CHANGELOG updates | P0 | TODO | TASK-0700..0707 | L (0 LoC; 7-8h wall) | D-014 |

### SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2)

| ID | Title | Priority | Status | Depends | Complexity | Bundle |
|----|-------|----------|--------|---------|------------|--------|
| TASK-0709 | SPEC-27 v3 R4: `NotNormalForm.redexes` valid-pair semantics + I4 prune helper | P0 | TODO | none | S (~70 LoC) | SPEC-27 |
| TASK-0710 | SPEC-27 v3 R7-R9: ChurchArithmeticCodec audit + R8 operand semantics | P1 | TODO | TASK-0709 | S (~70 LoC) | SPEC-27 |
| TASK-0711 | SPEC-27 v3 R13a': `wire_add_into` / `wire_mul_into` obligation validation (Phase 3a promotion) | P0 | TODO | none | S (~80 LoC) | SPEC-27 |
| TASK-0712 | SPEC-27 v3 R14' / R16b': `biguint_readback` module (`decode_biguint`) | P0 | TODO | TASK-0711 | M (~160 LoC) | SPEC-27 |
| TASK-0713 | SPEC-27 v3 R16a': `horner_serial` oracle with `OracleError` | P0 | TODO | none | S (~90 LoC) | SPEC-27 |
| TASK-0714 | SPEC-27 v3 R10'-R13', R16': HornerCodec encoder (Horner recurrence + bounds) | P0 | TODO | TASK-0711, TASK-0713 | M (~190 LoC) | SPEC-27 |
| TASK-0715 | SPEC-27 v3 R14', R15', R16': HornerCodec decoder + `Codec` impl | P0 | TODO | TASK-0712, TASK-0713, TASK-0714 | S–M (~160 LoC) | SPEC-27 |
| TASK-0716 | SPEC-27 v3 R19, R20: default_registry — drop `lambda`, add `horner` | P0 | TODO | TASK-0715 | S (~35 LoC) | SPEC-27 |
| TASK-0717 | SPEC-27 v3 R21, R23: CLI `compute --encoder`/`--codec` with `conflicts_with` | P0 | TODO | TASK-0716 | S–M (~140 LoC) | SPEC-27 |
| TASK-0718 | SPEC-27 v3 R22: `encoders list` (and `codecs list` alias) CLI subcommand | P1 | TODO | TASK-0716 | S (~70 LoC) | SPEC-27 |
| TASK-0719 | SPEC-27 v3 R24-R28: RecipeEncoder generalization audit + AssignRecipe encoder-name field | P1 | TODO | TASK-0716 | M (~130 LoC) | SPEC-27 |

**Suggested execution order for SPEC-27 bundle** (DAG topological sort):
1. TASK-0709 + TASK-0711 + TASK-0713 (parallel — all foundational, no inter-dependencies).
2. TASK-0710 (consumes TASK-0709) + TASK-0712 (consumes TASK-0711) — parallel.
3. TASK-0714 (consumes TASK-0711 + TASK-0713) — encoder.
4. TASK-0715 (consumes TASK-0712 + TASK-0713 + TASK-0714) — decoder + Codec impl.
5. TASK-0716 (consumes TASK-0715) — registry swap.
6. TASK-0717 + TASK-0718 + TASK-0719 (parallel — all consume TASK-0716).

**Suggested execution order** (DAG topological sort):
1. TASK-0700 (foundational; no deps)
2. TASK-0701 (consumes TASK-0700)
3. TASK-0702 + TASK-0703 (parallel; both consume TASK-0700+0701)
4. TASK-0707 (integration tests; needs TASK-0700..0703)
5. TASK-0704 + TASK-0705 (parallel; TASK-0704 needs TASK-0700..0703; TASK-0705 needs TASK-0703)
6. TASK-0706 (docs; needs TASK-0700..0705)
7. TASK-0708 (campaign run; needs everything green)

When the bundle closes, TASK files move to `archive/` and this section clears per the existing housekeeping pattern.

---

## Cumulative bundles delivered (per `progress.md`)

| Bundle | TASKs | Tasks archive | Closure narrative |
|--------|-------|---------------|--------------------|
| Phase 1..11 (v1) | TASK-0001..TASK-0399 (~270 done) | `archive/` | `progress.md` "Local Benchmark Phase" |
| D-005 | TASK-0400..0403 (4) | `archive/` | `progress.md` D-005 entry |
| D-006 (SPEC-20 elastic, Option A) | TASK-0410..0455 (~46) | `archive/` | `progress.md` D-006 entry |
| D-009 (SPEC-22 arena) | TASK-0460..0500 (~36) | `archive/` | `progress.md` D-009 entry |
| D-010 (SPEC-21 streaming) | TASK-0510..0591 (~40) | `archive/` | `progress.md` D-010 entry |
| D-011 (BLOCKER perf regression) | TASK-0595..0614 (~10) | `archive/` | `progress.md` D-011 entry |
| D-012 (instrumentation restore) | TASK-0615..0618 (4) | `archive/` | `progress.md` D-012 entry |

**Total tasks shipped across bundles:** ~410 atomic tasks across SPEC-02..SPEC-22, all archived. Per-task definitions in `archive/TASK-NNNN-*.md`. Full per-bundle narratives in `progress.md`.

---

## How to repopulate this file (D-013+ workflow)

1. The next bundle's inventory lives in `docs/next-steps.md` (e.g., D-013 hardening backlog inherited from D-011).
2. Run `task-splitter` from the relativist subdir against the relevant SPEC + inventory items. The agent writes TASK files directly into `docs/backlog/` (NOT into `archive/`) and updates this file's "Active" section + per-spec coverage matrix.
3. When the bundle closes, the next housekeeping commit moves the TASK files into `docs/backlog/archive/` and clears the "Active" section.

This pattern keeps the **active backlog small enough to read at a glance** while preserving every historical task definition for audit/reproducibility.
