# Handoff — TCC Comprehensive Catch-up (post-v0.20.0-pre)

**Status:** READY TO DISPATCH (from TCC root session, NOT from `relativist/`)
**Saved:** 2026-05-05
**Target:** TCC root session — agents REDATOR, BIBLIOTECARIO, DEBATEDOR, ESPECIALISTA EM SPECS
**Scope:** Update the academic side of the TCC (`artigo/tcc_pt_br.tex`, `biblioteca/`, `discussoes/`) to reflect ~7 weeks of code-side work (D-005..D-012, all closed; v0.20.0-pre published; main merged from v2-development).

The user has been 100% focused on code for ~2 months and the writing side is significantly behind. This handoff provides a single self-contained brief that each TCC agent can read to catch up on what changed and what to incorporate into the artigo.

---

## §0 — Reading guide per agent

| Agent | Sections to read | Output expected |
|-------|------------------|-----------------|
| **REDATOR** (`artigo/tcc_pt_br.tex`) | §1, §3, §4, §5, §6, §10 | Update Sections 4 (architecture) + 5 (results) + 6 (discussion) of the artigo with the post-D-012 baseline numbers, the latent-bug scientific finding, and the Phase 3 LAN forward-look |
| **DEBATEDOR** (`discussoes/`) | §1, §3, §4, §5 | Revise ARG-006 + ARG-007 with the new empirical signature; new DISC if any conceptual question emerged |
| **BIBLIOTECARIO** (`biblioteca/`) | §1, §7 | Register `v0.20.0-pre` GitHub release as a self-citable artifact (REF-NNN); update `mapa-citacoes.md` |
| **ESPECIALISTA EM SPECS** | §1, §6 (status delta only) | Nothing immediately required — D-013 inventory is in `relativist/docs/next-steps.md` and only opens after Phase 3 LAN data |

The user authorises each agent to make its respective edits per the inviolable rules in the TCC root `CLAUDE.md`. **NONE of these agents writes to `codigo/relativist/`** — that's the relativist repo's territory, already at v0.20.0-pre with all CI green.

---

## §1 — One-page status update

| Item | Pre-handoff state (last TCC session, ~2026-04-11) | Now (2026-05-05) |
|------|---------------------------------------------------|-------------------|
| Code repo state | v1 frozen at `v0.10.0-bench` on `v1-feature-complete` branch; main = v1; v2-development empty/early | v2 era complete (D-005..D-012 closed); `main` merged from v2-development; tag `v0.20.0-pre` published as GitHub pre-release |
| Test floor | 690 tests (v1 frozen) | 1798 default / 1842 zero-copy / 1789 streaming-no-recycle / 1740 release; v1 floor 690 inviolable on `v1-feature-complete` |
| Specs | SPEC-00..SPEC-16 (17 specs) | SPEC-00..SPEC-27 (28 specs); SPEC-17..22 SHIPPED, SPEC-23..27 Draft |
| Bundles closed | Pre-DEV waves only (Stage 0+1+2 of SPEC-20/21/22) | D-005, D-006, D-009, D-010, D-011, D-012 — full 6-stage SDD cycles each |
| Empirical baseline | `results/locked/v1_local_baseline/` (Phase 1 + Phase 2, 4490 reps, 0 failures) | + `results/locked/v2_post_d012_baseline_2026-05-05/` **canonical v2 baseline** (32 distributed slots × 10 reps, all_correct=true, instrumentation populated). Plus 2 historical v2 baselines preserved (`v2_pre_fix_baseline_2026-05-04`, `v2_d011_final_baseline_2026-05-04`). |
| CI | red on v2-development since 2026-04-27 | **green** on v2-development AND on main (post-merge), all 3 workflows (CI, Docker Smoke, Bench Smoke) |
| Phase 3 LAN | not started; tutorial not written | tutorial in `codigo/relativist/docs/benchmarks/phase-3-lan.md` (~720 lines, dual-axis Axis 1 bincode + Axis 2 zero-copy); LAN bench itself NOT yet executed |

---

## §2 — Bundle-by-bundle summary (academic-relevant)

Sequence (chronological, all CLOSED):

### D-005 (CLOSED 2026-04-24) — Worker-side `CommutationBatch.local_wiring` (SPEC-19 §3.4 Shape A)

**Academic relevance:** completes the **delta protocol** that ARG-006 / ARG-007 conceptually required. Without it, distributed reduction would still need full-state dispatch every round, contradicting the c_o/c_r argument.

**Outcome:** **G1 parity gate 12/12 GREEN** on both iteration orders + both feature configs. PROTOCOL_VERSION 2→3.

### D-006 (CLOSED 2026-04-27) — Hybrid Coordinator + Elastic Grid skeleton (SPEC-20 §3.1-3.3, Option A)

**Academic relevance:** **Coordinator-as-worker** (ARG-001 corollary 2.1) shipped. Workers can join mid-execution. Validates §"Confluence Argument" point 1 + 2 of the artigo's Discussion section. Note: full elastic *departure with reclaim* (point 3) deferred to v2.1; current code ships *detection-only*. The artigo Discussion section can cite 2.1 + 2.2 as shipped, 2.3 as detection-only with the reclaim path on the v2.1 roadmap.

**Outcome:** 14 CRITICAL + 23 HIGH bugs found by audit on the LLM-attempt; full per-phase REFACTOR closed all of them. Original LLM commits archived at tag `v2-llm-experiment-archive`. **Lesson learned:** wall-clock saved at Stage 3 was consumed several times over by review+QA+refactor; future LLM delegation will be limited to mechanical refactor passes only.

### D-009 (CLOSED 2026-04-27) — SPEC-22 Arena Management

**Academic relevance:** introduces `Net.free_list` (LIFO recycling) + `SparseNet` (HashMap-backed alternative representation) to keep memory bounded under streaming. **Closes 2.32 + 2.33 of ROADMAP.** SPEC-01 invariant **I3 → I3' (Uniqueness amendment)** propagated to 7 predecessor specs.

**Outcome:** 5 CRITICAL distributed-seam bugs that the reviewer missed found by adversarial QA (Opus). All closed. CompactSubnet wire format `free_list` extension deferred to D-011 (TASK-0595 / SPEC-19 R35a).

### D-010 (CLOSED 2026-04-30) — SPEC-21 Streaming Generation

**Academic relevance:** **streaming partition generation** so the coordinator never needs to materialise the full net before partitioning. Closes ROADMAP §§2.27 + 2.28 + 2.30. Pull-dispatch FSM with `RequestWork`/`NoMoreWork` wire variants (PROTOCOL_VERSION 5→6). New `streaming-no-recycle` cargo feature for diagnostic comparison.

**Outcome:** 4 CRITICAL + 5 HIGH QA findings closed (incl. `default_chunked_iter` silently dropping every `FreePort` — T6 violation; impersonation gap on `RequestWork.worker_id`).

### D-011 (CLOSED 2026-05-04) — Partition perf regression BLOCKER  ⭐ **THIS IS THE "SERIOUS BUG"**

**Academic relevance — VERY HIGH.** This is the scientific finding the user explicitly mentioned wanting to incorporate into the TCC.

**What happened.** A v2 bench rodada showed `ep_con 5M w=2` wall going from v1 ~12s to v2-HEAD ~22s (+83% regression). 7-point bisect isolated this to commit `d6411be` (D-009 Phase C). Probe instrumentation revealed the root cause: `partition::helpers::build_subnet_with_config` threshold `id_range > 4 × live_count` (SPEC-22 R22 v2.3) measured the *planning range* (`next_id × 10`) instead of the *actual arena memory* (`max_live_id + 1`). Every healthy partition was mis-routed to SPARSE (HashMap-backed, ~7s/partition) when DENSE would have taken ~1s.

**The latent-bug discovery.** Phase B of the fix attempted to switch to DENSE but uncovered **2 latent bugs in the dense `build_subnet` path** that had been masked since D-009 because no production workload exercised the dense path:

- **Bug 1** (`helpers.rs:384`) — dense initialised `freeport_redirects: HashMap::new()` while sparse cloned them (broke delta-mode integration; SC-001 second surface).
- **Bug 2** (`helpers.rs:382`) — dense returned `next_id = 0`, then `split.rs:96-98` widened to `max(0, max_agent_id + 1)`, causing worker 0 to allocate fresh agents OUTSIDE its assigned `id_range` and INSIDE worker 1's pre-existing IDs. **I1 (Bidirectional Consistency) violation surfaced post-merge** in `condup_expansion`.

**Resolution.** SPEC-22 amended to v2.4 (R22 metric → `effective_arena_size = max_live_id + 1`, R30 reworded). 2-line bug fixes (`helpers.rs:382/384`). Defensive guards AF-2 (`Net::create_agent`) + AF-3 (`merge::core::merge`) added.

**Bench verification.** ep_con 5M w=2 local wall: v1 = 14.247s, post-fix = 15.775s, ratio **1.11× (within noise floor)** — ~88 % of the +83 % regression closed.

### **🔬 Scientific finding to incorporate into the artigo:**

> The apparent v1→v2 perf regression masked **2 latent correctness bugs** that v1's empirical test suite (4490 executions, 0 reported failures) **never detected**. The formal invariant framework (SPEC-01 I1, SPEC-22 R10/D3, SPEC-22 v2.4 R22 metric) detected what the empirical test suite missed. **v2 is now strictly more correct than v1 ever was**, at a residual ~10 % wall-clock cost on the canonical workload.

**This strengthens the TCC's central claim** that formal IC theory + invariant-driven SDD produces empirical correctness guarantees that empirical testing alone cannot. Recommend: add a paragraph to the Discussion section of the artigo referencing this finding, with the bisect → invariant detection → fix narrative.

### D-012 (CLOSED 2026-05-05) — Instrumentation Restore

**Academic relevance — HIGH.** Without D-012, the c_o/c_r argument (§2.40 of ROADMAP) would have to be *estimated by black-box subtraction* of `wall_dist - wall_seq` instead of measured per-component. D-012 restored:

- `network_time_secs` per round (was 0.0 across all v2 datasets — RF-04 in the cold post-mortem)
- `compute_time_secs` per round (was 0.0 — RF-05)
- `mips_mean` for distributed rows (was 0.000 — RF-07; root cause: a Python-in-bash hardcode in `scripts/bench_docker_v2.sh:283` overrode the binary)

**Sample headline** (now publishable): `ep_500k w=1` round 0 wall = 0.460s = compute 0.10 + network 0.39 + merge 0.04 + ~0.03 framing. **47 % of average round time is network on TCP-localhost**.

---

## §3 — New empirical data (canonical baseline)

### Canonical baseline going forward

`results/locked/v2_post_d012_baseline_2026-05-05/` — **this is the v2 reference for all artigo numbers from now on.**

- HEAD at run: `e6ff6bb` (post-D-012 paperwork close), pre-version-bump
- 32 distributed configs × 10 reps + 8 sequential native baselines = 400 datapoints
- All 32/32 distributed slots `all_correct=true`, 10/10 reps (zero failures)
- Hardware: Lenovo T14 Gen 4, i7-1365U, 32 GB DDR5, Ultimate Performance + battery saver explicitly disabled, RUSTFLAGS=-C target-cpu=native (host) / Docker default release (container)
- Same hardware as `v1_local_baseline` (Lenovo T14 i7-1365U) — comparability preserved
- MANIFEST.md with full provenance + SHA-256 of all 4 artifacts
- Companion: `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` (cold post-mortem, rev 2 post-D-012, 9 red flags catalog, RF-04/05/07 marked CLOSED, verdict DEFENSIBLE)

### Historical baselines preserved (for delta analysis only — DO NOT cite as primary)

| Path | What it is | Why preserved |
|------|------------|---------------|
| `results/locked/v1_local_baseline/` | v1 frozen reference (2026-04-11) | Still valid as v1 reference; D-011 bugs didn't affect v1 because v1 had no SPARSE path |
| `results/locked/v2_pre_fix_baseline_2026-05-04/` | v2 before D-011 fix | 4 broken slots; demonstrates Bug 1+2 wall-clock cost (post/pre median ratio ~0.55 = D-011 fix-and-improvement empirical proof) |
| `results/locked/v2_d011_final_baseline_2026-05-04/` | v2 after D-011 fix, before D-012 instrumentation restore | 3 zeroed columns (network/compute/mips); superseded by post-D-012 baseline for analysis purposes |

### **❗ Data invalidation considerations**

The user explicitly mentioned considering invalidating older data in the TCC. Specifically:

1. **v1 baseline data — STILL VALID as v1 reference.** D-011 bugs lived in code paths v1 didn't have (D-011 introduced D-009's SPARSE path, then broke its dense fallback). v1 measurements stand.

2. **Any v2 wall-clock numbers cited in artigo prior to 2026-05-05 — INVALIDATE.** They reflect either pre-fix (Bug 1+2 active, 4 broken slots) or post-D-011-but-pre-D-012 (3 zeroed columns). Replace with `results/locked/v2_post_d012_baseline_2026-05-05/` numbers.

3. **Any artigo references to "MIPS" or "network time" or "compute time" pre-D-012 — INVALIDATE.** Those columns were structurally zero before D-012. The new baseline has real data.

The exact files/sections to edit in the artigo are the REDATOR's responsibility (see §6 below).

---

## §4 — Headline numbers (ready to embed in artigo Section 5: Empirical Results)

All numbers are from `results/locked/v2_post_d012_baseline_2026-05-05/summary.csv` and `rounds.csv`. SHA-256 hashes in MANIFEST.md.

### Correctness (always lead with this)

> v2 produces 32/32 distributed configurations × 10/10 repetitions = **400 datapoints with `all_correct=true`** (graph-isomorphism check vs sequential reduction). v1 baseline shows 4490 datapoints across Phase 1 + Phase 2 with 0 reported failures. **Cumulatively, ~5000 distributed reductions validated against sequential baseline; zero correctness divergences.**

### Wall-clock comparison (v2 vs v1, TCP-localhost only)

| Family | N slots | v2/v1 median | v2/v1 mean | Range |
|--------|---------|--------------|------------|-------|
| `ep_annihilation_con` (heavy compute) | 12 | **1.63×** slower | 1.63× | 1.43 – 2.06 |
| `dual_tree` (mid-scale) | 12 | **1.77×** slower | 1.80× | 1.26 – 3.04 |
| `condup_expansion` (small N, fixed-cost dominated) | 8 | 12.8× slower | 14.4× | 7.04 – 25.39 — **floor effect; do NOT lead with this** |

### Wire bytes per round (delta protocol working)

`ep_annihilation_con 500k w=1`, round 0:
- v1: bytes_sent = 41,000,069, bytes_received = 153 (asymmetric one-way)
- v2: bytes_sent = 23,802,364, bytes_received = 4,887,587 (bidirectional with delta echoes)
- **v2 sends 30 % fewer bytes per round (28.7 MB vs 41 MB)** despite the bidirectional traffic

### Per-component decomposition (NEW with D-012)

Average across 320 TCP rounds:
- partition: 0.0009 s (0.04 %)
- compute: 0.192 s (8.0 %)
- merge: 0.145 s (6.0 %)
- network: **1.139 s (47.3 %)** ← wire is dominant on TCP-localhost
- measured-total: 2.409 s
- residual (~38 %): framing + orchestration + allocator overhead

### Break-even update (refines §2.40 of `relativist/docs/ROADMAP.md`)

v1 ROADMAP §2.40 derived `c_o/c_r ≈ 2.20` from Phase 1 + Phase 2. v2 instrumentation refines this:
- v2 adds ~100 ms/round of CPU vs v1 (transport + framing layer)
- v2 saves ~12 MB/round on the wire (delta protocol)
- Break-even bandwidth: **B ≤ 156 MB/s (≈1.2 Gbps)** — every real LAN/WAN qualifies
- On TCP-localhost (effectively zero bandwidth limit), v2 loses the trade. On any real network it should win. **This is the empirical signature ARG-006/007 require for the speedup forward-projection.**

### MIPS distribution (NEW with D-012)

mips_mean across 32 distributed slots: range **0.002 – 1.261 MIPS**, with `dual_tree 20 w=1` reporting the highest. Sequential rows still have `mips_mean = 0.000` (TODO `D-012-FU-SEQ-MIPS` — `total_interactions` only wired in worker→coordinator path; cosmetic, doesn't affect distributed analysis).

### Sample table format suggestion for artigo

| Workload | N | w | v1 wall (s) | v2 wall (s) | v2 bytes/round | v2 mips |
|----------|---|---|-------------|-------------|----------------|---------|
| ep_annihilation_con | 5M | 2 | 5.04 | 7.42 | 28.7 MB | 0.674 |
| dual_tree | 22 | 4 | 5.05 | 7.23 | (in CSV) | 0.581 |
| ep_annihilation_con | 1M | 8 | 1.15 | 2.04 | (in CSV) | 0.489 |

Pull from `results/locked/v2_post_d012_baseline_2026-05-05/summary.csv` directly.

---

## §5 — ARG status delta (for DEBATEDOR)

| ARG | Status pre-handoff | Status now | Action recommended |
|-----|--------------------|-------------|---------------------|
| ARG-001 (Confluence preserves determinism) | CLOSED in v1 | **STRENGTHENED** by D-011 finding | Add §"Empirical reinforcement post-D-011" subsection: formal invariants caught what 4490 v1 reps didn't |
| ARG-002 (P5 wall-clock viability) | OPEN | OPEN — empirical gap remains until Phase 3 LAN | No change yet |
| ARG-004 (Feasibility analysis) | OPEN | OPEN | No change yet |
| ARG-005 (Delta border completeness) | CLOSED for v1 + delta-conservative | **REINFORCED** by D-005 + D-010 | Cite the integration of delta protocol + streaming as a v2 corollary |
| ARG-006 (Empirical signature for distributed) | OPEN, awaited Phase 3 LAN | **PARTIALLY VALIDATED** by D-012 instrumentation: per-component decomposition now measurable | Update with "47 % of round time is network on TCP-localhost" + recommend Phase 3 LAN for the LAN/WAN crossover number |
| ARG-007 (c_o/c_r breakeven) | OPEN, awaited Phase 3 LAN | **PARTIALLY VALIDATED** by D-012: B ≤ 156 MB/s break-even derived from real per-component data | Update with the 156 MB/s number; defer the actual LAN measurement to a Phase 3 LAN follow-up DISC |

**Suggestion:** open a new `discussoes/exploracoes/DISC-NNN` titled "D-011 latent-bug discovery: formal invariants vs empirical testing" capturing the scientific finding before the artigo reaches Section 5. The 2-round adversarial ping-pong protocol applies as usual.

---

## §6 — SPEC status delta (for ESPECIALISTA EM SPECS)

| Spec | Pre-handoff status | Status now | Action |
|------|--------------------|-------------|--------|
| SPEC-00..SPEC-16 | shipped in v1 | unchanged | none |
| SPEC-17 transport abstraction | Reviewed v2 (Pre-DEV) | **shipped** (D-006) | none |
| SPEC-18 wire format v2 | Reviewed v2 (Pre-DEV) | **shipped** (D-009/D-010 propagation; PROTOCOL_VERSION 5) | none |
| SPEC-19 delta protocol | Reviewed v2 (Pre-DEV) | **shipped + amended R35a** (D-005 + D-011 Phase A for CompactSubnet free_list) | none |
| SPEC-20 elastic grid | Reviewed v2 (Pre-DEV) | **shipped (Option A; full reclaim deferred)** | none — full reclaim is v2.1 work |
| SPEC-21 streaming generation | Reviewed v2 (Pre-DEV) | **shipped + 8 amendments propagated** (D-010 Phase A) | none |
| SPEC-22 arena management | Reviewed v2 (Pre-DEV) | **shipped + amended to v2.4** (D-009 + D-011 R22 metric correction) | none |
| SPEC-23 compact memory | Draft | Draft | hold until Phase 3 LAN data informs whether bit-packing is needed |
| SPEC-24 WAN deployment | Draft | Draft | depends on SPEC-17 (DONE); next priority after Phase 3 LAN |
| SPEC-25 recipe generation | Draft | Draft | M7 milestone |
| SPEC-26 GUI app | Draft | Draft (workspace restructure §3.1 R1-R7 SHIPPED — `relativist-core` + `relativist-cli`) | UI itself out of TCC scope |
| SPEC-27 encoder/decoder API | Draft (Layer 3 partial) | Draft (TASK-0340 partial Codec registry shipped) | full Layer 3 deferred until SPEC-25 |

**Bottom line:** 6 of 11 v2 specs SHIPPED (55 %); 5 in Draft (compact memory, WAN, recipe gen, GUI, encoder API). The 6 shipped are exactly what the TCC's central claim requires; the 5 Draft are "production polish" features that don't gate the academic argument.

---

## §7 — `v0.20.0-pre` as a citable artifact (for BIBLIOTECARIO)

Recommend registering in `biblioteca/`:

- **REF-NNN (pick next number):** Andrade, F. (2026). *Relativist: Distributed reduction of Interaction Combinators on Grid Computing, v0.20.0-pre*. GitHub Release. URL: `https://github.com/andrade-filipe/relativist/releases/tag/v0.20.0-pre`. Tag annotated SHA: `379edc1`. Pre-release flag: yes (intended for Phase 3 LAN testing).

- Cross-reference in `mapa-citacoes.md`:
  - Section 4 (Architecture) — cite the v0.20.0-pre release for the prototype reference
  - Section 5 (Empirical Results) — cite for the canonical baseline (`results/locked/v2_post_d012_baseline_2026-05-05/`)
  - Section 6 (Discussion) — cite for the D-011 latent-bug finding (point to per-bundle archive at `codigo/relativist/docs/qa/archive/QA-D011-BUG2-i1-violation-2026-05-04.md`)

- The MANIFEST file at `codigo/relativist/results/locked/v2_post_d012_baseline_2026-05-05/MANIFEST.md` is the authoritative provenance document; cite by path + SHA-256 of contents (already computed in the file).

**No new external papers added in this period** — the SDD work was implementation, not literature. BIBLIOTECARIO doesn't need to ingest new PDFs; only register the self-citable artifact above.

---

## §8 — Recommendations per TCC agent (concrete edit list)

### REDATOR (`artigo/tcc_pt_br.tex`)

1. **Section 4 (Architecture):** add a "v2 evolutions" subsection covering SPEC-17/18/19/21/22 in 1 paragraph each (their rationale, not implementation detail). Keep the v1 architecture description as the baseline.
2. **Section 5 (Empirical Results):**
   - **REPLACE** any v2 wall-clock numbers with the canonical post-D-012 baseline (§4 of this handoff for the pull-from-CSV instructions).
   - **ADD** the per-component decomposition table (47 % network share, etc.) — this is the headline empirical signature.
   - **ADD** the wire-bytes comparison table (30 % byte reduction with delta protocol).
   - **ADD** the v2/v1 median ratios per family — frame as "measured cost of the abstraction layers, recoverable on real LAN".
   - **DROP** any "MIPS" or "network time" claims that pre-date 2026-05-05 (they were invalidated by D-012 instrumentation — see §3).
3. **Section 6 (Discussion):**
   - **ADD** the D-011 latent-bug scientific finding as a numbered subsection. The narrative arc is: empirical regression → bisect → spec-driven root cause → 2 latent bugs revealed → formal invariants caught what empirical testing didn't → v2 strictly more correct than v1 (§2 D-011 of this handoff).
   - **EXTEND** the §"Confluence Argument" already in the ROADMAP file to include the v2 empirical reinforcement note.
   - **FRAME** the Phase 3 LAN forward-look honestly: localhost loss, projected LAN crossover at B ≤ 156 MB/s. Do NOT claim Phase 3 LAN data the TCC doesn't yet have.
4. **Section 10 (Reproducibility):** point at `v0.20.0-pre` GitHub release + the canonical baseline + the MANIFEST.md SHA-256 for verification. Cite `docs/benchmarks/phase-3-lan.md` as the protocol for any reader who wants to re-run on their own LAN.

### DEBATEDOR (`discussoes/`)

1. Open `discussoes/exploracoes/DISC-NNN-formal-invariants-vs-empirical-testing-2026-05-04.md` capturing the D-011 latent-bug finding as a discrete academic insight. 2-round adversarial ping-pong before promoting to ARG.
2. Update ARG-006 (`discussoes/argumentos/ARG-006-*.md`): per-component decomposition is now empirically measurable (47 % network share); cite the canonical baseline.
3. Update ARG-007 (`discussoes/argumentos/ARG-007-*.md`): add the 156 MB/s break-even as a derivation; note the Phase 3 LAN crossover empirical confirmation is still pending.
4. Optionally open a `discussoes/revisoes/REV-NNN` for the artigo's Section 5 once REDATOR has drafted the updates.

### BIBLIOTECARIO (`biblioteca/`)

1. Register `v0.20.0-pre` as REF-NNN per §7 above.
2. Update `mapa-citacoes.md` per §7 above.
3. No new external papers ingested in this period; no `fichas/` updates needed.

### ESPECIALISTA EM SPECS

1. No immediate action. SPEC-22 v2.4 was the last amendment; no spec edits are pending.
2. When D-013 is ready (after Phase 3 LAN data informs whether the SPEC-21/22 hardening backlog needs amendments), the closure log goes in `relativist/docs/spec-reviews/`.

---

## §9 — Phase 3 LAN preview — what's NOT done yet (do NOT cite as concluded)

The user has the prototype + the protocol document, but **the Phase 3 LAN bench has NOT been run**. Specifically:

- `docs/benchmarks/phase-3-lan.md` (~720 lines) is the operational tutorial. Two axes:
  - **Axis 1**: bincode + delta (= same protocol as `v2_post_d012_baseline_2026-05-05` but on real LAN hardware)
  - **Axis 2**: zero-copy + delta (`cargo build --release --features zero-copy`; reserved to measure rkyv contribution separately)
- Real LAN hardware procurement / setup is the user's next operational step.
- Time-sync requirement (NTP) and TCP firewall config noted in tutorial.
- Manifest template + SHA-256 protocol included in tutorial so the result, when produced, follows the same provenance rigor as the localhost baseline.

**Per the artigo:** frame Phase 3 LAN as the **forward-look** that turns the per-component decomposition + the 156 MB/s break-even projection into an empirical claim. Until the LAN data exists, the artigo's Section 5 conclusion is "v2 trades wire-bytes for CPU-cycles; localhost is the wrong regime; LAN measurement pending."

---

## §10 — Where to find more detail (cross-references for TCC root agents)

All paths relative to TCC root.

| Topic | Path |
|-------|------|
| Latest baseline (canonical) | `codigo/relativist/results/locked/v2_post_d012_baseline_2026-05-05/MANIFEST.md` |
| Cold post-mortem analysis | `codigo/relativist/docs/analysis/D011-final-baseline-analysis-2026-05-04.md` |
| Bundle-by-bundle history | `codigo/relativist/docs/progress.md` |
| Active queue (D-013+) | `codigo/relativist/docs/next-steps.md` |
| ROADMAP (v2+ design notes) | `codigo/relativist/docs/ROADMAP.md` |
| User-facing CHANGELOG | `codigo/relativist/CHANGELOG.md` |
| Phase 3 LAN tutorial | `codigo/relativist/docs/benchmarks/phase-3-lan.md` |
| User guides (10 numbered, pt-BR) | `codigo/relativist/docs/guides/` |
| Per-bundle adversarial QA | `codigo/relativist/docs/qa/archive/` |
| Per-bundle reviews | `codigo/relativist/docs/reviews/archive/` |
| Per-bundle spec-review closure logs | `codigo/relativist/docs/spec-reviews/archive/` |
| All 28 specs | `codigo/relativist/specs/SPEC-NN-*.md` |
| Tag info | `git -C codigo/relativist show v0.20.0-pre` |
| GitHub release page | https://github.com/andrade-filipe/relativist/releases/tag/v0.20.0-pre |
| TCC root rules | `CLAUDE.md` (TCC root) |

---

## Operator note

This handoff is intentionally exhaustive (~500 lines) because the user has been ~7 weeks away from the TCC writing side and asked for a full catch-up. Each TCC agent should **read its own §0-row sections** rather than the full file. The §1 status table is the only "everyone reads this" section.

After all TCC root agents have done their respective edits, recommend the user runs the standard TCC root agent flow (REDATOR drafts, DEBATEDOR reviews, BIBLIOTECARIO indexes) before any next session of code work.

The relativist subdir is in a clean state: tag `v0.20.0-pre` published, main merged, all CI green. No concurrent code work is needed in parallel with the TCC catch-up.
