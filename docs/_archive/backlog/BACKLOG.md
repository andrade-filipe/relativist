# Relativist Implementation Backlog

**Last updated:** 2026-05-16 (D-017 Multi-container Horner distribution demo; +5 tasks TASK-0728..0732 opens the bundle).

**Status:** 32 active TASKs across five bundles:
- **D-014 (Stress Curve Campaign):** TASK-0700..0708 (9 tasks; Topic 1).
- **SPEC-27 v3 (Encoder/Decoder API + HornerCodec):** TASK-0709..0719 (11 tasks; Topic 2). Stage 1 splitting completed 2026-05-06 from `specs/SPEC-27-encoder-decoder-api.md` Revised v3 (Round 2 spec-critic response closed all 13 issues).
- **D-015 follow-ups:** TASK-0720..0722 (3 tasks; refactor + benchmark data).
- **D-016 (HornerCodec decoder extension):** TASK-0723..0726 (4 tasks; Stage 1 splitting completed 2026-05-16 from `docs/demos/horner-g1-demonstration.md` "Limitações conhecidas" gap analysis).
- **D-017 (Multi-container Horner distribution demo):** TASK-0728..0732 (5 tasks; Stage 1 splitting completed 2026-05-16 from operator request "N containers separados, cada um com seu log"). TASK-0727 number reserved (deferred placeholder retired into D-016 scope).

The full inventory of D-005..D-012 atomic tasks (TASK-0001..TASK-0618 with intentional gaps) is preserved at `archive/`. Numbering gap 0619-0699 reserved for any intermediate bundles between D-012 and D-014.

**Pipeline:** See `../WORKFLOWS.md` (§1 Development Pipeline) for the 6-stage SDD process.

---

## Active

### D-014 — Stress Curve Campaign (Topic 1)

| ID | Title | Priority | Status | Depends | Complexity | Bundle |
|----|-------|----------|--------|---------|------------|--------|
| TASK-0700 | Cross-platform `MemoryProbe` (current + peak + RAM fraction) | P0 | DONE (DEV) | none | S–M (~180 LoC) | D-014 |
| TASK-0701 | `StopRule` (wall-time / RAM / OOM sequence aborter) | P0 | DONE (DEV) | TASK-0700 | S–M (~170 LoC) | D-014 |
| TASK-0702 | `stress-curve` campaign descriptor in `bench/suite.rs` | P0 | DONE (DEV) | TASK-0700, TASK-0701 | S–M (~170 LoC) | D-014 |
| TASK-0703 | CSV schema extension (`vmrss_*`, `stop_reason`, `cv_above_gate`) | P1 | DONE (DEV) | TASK-0700, TASK-0701 | S (~60 LoC) | D-014 |
| TASK-0704 | `scripts/stress_curve.sh` Phase 1 + Phase 2 orchestrator | P0 | DONE (DEV) | TASK-0700..0703 | M (~230 LoC) | D-014 |
| TASK-0705 | `scripts/plot_stress_curve.py` (9 PDFs + summary) | P1 | DONE (DEV) | TASK-0703 | M (~230 LoC) | D-014 |
| TASK-0706 | `docs/benchmarks/campaigns/stress-curve.md` methodology page | P1 | DONE (DEV) | TASK-0700..0705 | S–M (~250 lines md, 0 LoC) | D-014 |
| TASK-0707 | 6 integration tests for stress_curve_*.rs | P0 | DONE (DEV) | TASK-0700..0703 | M (~200 LoC) | D-014 |
| TASK-0708 | Full campaign overnight + lock dir + INDEX/ROADMAP/CHANGELOG updates | P0 | SENTINEL (awaiting operator overnight run) | TASK-0700..0707 | L (0 LoC; 7-8h wall) | D-014 |

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

### D-016 — HornerCodec decoder extension

Closes the 3 known decoder failures documented in `docs/demos/horner-g1-demonstration.md` "Limitações conhecidas" section (post-v0.20.0 audit, 2026-05-16). The encoder + reducer are correct in all cases per Lafont confluence (G1); the `biguint_readback` decoder mishandles (a) Church multiplication output for coefficient `c_i >= 2` and (b) nested Horner composition for degree `>= 2`. Bundle target: full readable subset over `MAX_CHURCH_NAT = 10_000` bounds with empirical G1 cross-checks at degree ≥ 2.

| ID | Title | Priority | Status | Depends | Complexity | Bundle |
|----|-------|----------|--------|---------|------------|--------|
| TASK-0723 | biguint_readback: handle Church mul output for c_i ≥ 2 | P0 | TODO | none (extends TASK-0712 in HEAD) | M (~120 LoC prod) | D-016 |
| TASK-0724 | biguint_readback: handle nested Horner composition (degree ≥ 2) | P0 | TODO | TASK-0723 | M (~140 LoC prod) | D-016 |
| TASK-0725 | Horner pipeline property tests at full MAX_CHURCH_NAT bounds | P1 | TODO | TASK-0723, TASK-0724 | S–M (~80 LoC test) | D-016 |
| TASK-0726 | Doc cleanup of demo "Limitações" section + `scripts/horner_demo.sh` placeholder | P1 | TODO | TASK-0725 | S (~40 LoC doc + ~30 LoC bash) | D-016 |

**Suggested execution order** (linear DAG, no parallelism):

1. TASK-0723 (cofactor c_i ≥ 2 traversal — Demo 2 case `[3,5]@4` → 23).
2. TASK-0724 (nested Horner traversal — Demos 4/5 + T9 BigUint witness).
3. TASK-0725 (property tests; promotes TASK-0723/0724 from "demos pass" to "100% readable subset audited").
4. TASK-0726 (doc + script housekeeping; opens TASK-0727 entry for next bundle).

**Deferred next-bundle entry (placeholder, no TASK file yet):**

- **TASK-0727** — `scripts/horner_demo.sh` Docker arm (D-017 candidate). Port the Phase 2 Docker pattern from `scripts/stress_curve.sh` (commit `c77d7fc`) to the Horner demo workflow. Owner: next bundle's task-splitter. **STATUS 2026-05-16:** Subsumed by D-017 below; the Docker arm landed inline in `horner_demo.sh` ahead of this bundle (see commit `c77d7fc`), so the number is retired and D-017 starts at TASK-0728.

### D-017 — Multi-container Horner distribution demo

Operator request (2026-05-16): the current `horner_live_demo.sh` runs N reduction workers inside a **single** container via `compute --workers N` (in-process distributed). For the TCC defesa, the audience wants to see N **separate** worker containers, each with its own preserved log (`docker logs relativist-worker-{1..N}`), exercising the real `coordinator` + `worker` TCP services from `docker-compose.yml`. The infrastructure already exists; the missing glue is (a) a way to encode a HornerCodec input to a `.bin` on disk without reducing, and (b) a way to decode the reduced `.bin` after the coordinator finishes — connecting the encoder registry to the existing coordinator/worker pipeline.

| ID | Title | Priority | Status | Depends | Complexity | Bundle |
|----|-------|----------|--------|---------|------------|--------|
| TASK-0728 | `compute --encode-only --output <path>` — emit `.bin` without reducing | P0 | DONE (commit `35aaef4`) | none | S (~80–120 LoC) | D-017 |
| TASK-0729 | `decode` subcommand — read reduced `.bin`, run codec decoder, print JSON | P0 | DONE (commit `47a21c8`) | none | S (~50–80 LoC) | D-017 |
| TASK-0730 | `scripts/horner_distributed_demo.sh` — N-container orchestrator + coordinator compose env-var override | P0 | DONE (commit `439c7c6`) | TASK-0728, TASK-0729 | M (~150–200 LoC bash) | D-017 |
| TASK-0731 | Integration tests — encode-only / decode roundtrip + ignored multi-container smoke | P1 | DONE (commits `8934ea0` + fmt `1d53692`) | TASK-0728, TASK-0729 | S–M (~80 LoC tests) | D-017 |
| TASK-0732 | Doc + live-demo updates — multi-container Horner variant (`docs/demos/live_demo.md` + optional `horner_live_distributed_demo.sh`) | P1 | DONE (commit `4f1e2cd`) | TASK-0730 | S (~50 LoC bash + ~80 lines md) | D-017 |

**D-017 Stage 3 (DEV) closure — 2026-05-16:** all 5 TASKs implemented, committed, and verified GREEN.
- `cargo test --release`: **1890** passed / 0 failed / 7 ignored (above the >1881 post-D-017 expected threshold; v1 floor 690).
- `cargo test` (default): **1948** passed / 0 failed / 7 ignored (above the 1918 pre-D-017 floor).
- `cargo clippy --all-features -- -D warnings`: clean.
- `cargo fmt --check`: clean.
- Three new D-017 test files all GREEN: `tests/compute_encode_only.rs` (6/6), `tests/decode_subcommand.rs` (7/7), `tests/horner_encode_decode_roundtrip.rs` (8 passed + 2 `#[ignore]` distributed smokes).
- `bash -n` clean on `scripts/horner_distributed_demo.sh` and `scripts/horner_live_demo.sh`.
- `docker-compose.yml` coordinator service parametrised via `INPUT_PATH` / `OUTPUT_PATH` / `METRICS_PATH` env vars (defaults match the prior literals — backwards-compat preserved for stress-curve / bench-tcp callers).

**Suggested execution order for D-017 bundle** (DAG topological sort):
1. TASK-0728 + TASK-0729 (parallel — both foundational CLI surfaces; no inter-dependencies; both reuse existing `EncoderRegistry` + `io::binary` helpers).
2. TASK-0731 (consumes TASK-0728 + TASK-0729 at compile time; the `#[ignore]` Docker smoke also consumes TASK-0730 but the non-ignored tests do not).
3. TASK-0730 (consumes TASK-0728 + TASK-0729; ALSO patches `docker-compose.yml` `coordinator.command:` block to accept `INPUT_PATH`/`OUTPUT_PATH`/`METRICS_PATH` env-var overrides, backwards-compatible defaults).
4. TASK-0732 (consumes TASK-0730; pure doc + optional live-demo wrapper).

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
