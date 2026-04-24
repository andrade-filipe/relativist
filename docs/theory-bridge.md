# Theory Bridge — TCC ↔ Relativist

> **Purpose:** Relativist agents (`spec-critic`, `especialista-specs`, `task-splitter`, `test-generator`, `developer`, `reviewer`, `qa`, etc.) live in `codigo/relativist/.claude/agents/` and by design do not traverse upward to read TCC-root academic artifacts. This file is the local index of every formal ARG (argument), DISC (discussion) and REF (reference) that Relativist specs cite. Summaries here are deliberately short — when an agent needs full content, follow the absolute path back to the TCC source.
>
> **Maintained by:** TCC-root `pesquisador` or a `general-purpose` agent invoked from the TCC root. Relativist agents MAY READ this file but MUST NOT edit it. Edits propagate from TCC root → this bridge, never the reverse.
>
> **Last updated:** 2026-04-24 (adds ARG-005, ARG-006; DISC-009 v2, DISC-011 v2, DISC-013; TCC IDs frozen at 6 ARGs / 13 DISCs / 18 REFs / 15 ACs).

---

## Arguments (ARG-NNN)

Located at: `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\argumentos\`

Six formal arguments built on the DISCs. Each ARG records a tese, explicit premises, a reasoning chain, and a strength classification. Relativist specs cite ARGs to discharge proof obligations.

### ARG-001 — Strong Confluence Preserves Determinism under Distribution
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\argumentos\ARG-001-confluencia-preserva-determinismo.md`
- **Date / Strength:** 2026-03-25 · Moderate-Strong (theorem-backed Layer 1, engineering-verifiable Layer 2)
- **Premises:** P1 (Lafont conditions → strong confluence), P2 (split/reduce/remap/merge correctness), P3 (border completeness), P4 (ID consistency), P5 (termination), P6 (terminating-nets scope)
- **Thesis (condensed):** For terminating IC networks, distributed reduction with a structure-preserving split/reduce/remap/merge protocol yields the same result as local sequential reduction, for any worker count and processing order.
- **Specs that depend:** SPEC-01 G1 (global determinism); SPEC-04 (split correctness); SPEC-05 (merge identity); SPEC-19 R38 (via ARG-005); SPEC-20 G1-elastic (via ARG-006).
- **Basis:** DISC-001 v2, DISC-003 v2.
- **Limitations:** Terminating nets only; empirical coverage dominated by ERA-ERA rule in v1 artifacts.

### ARG-002 — Partitioning Preserves Structure
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\argumentos\ARG-002-particionamento-preserva-estrutura.md`
- **Date / Strength:** 2026-03-25 · Moderate (solid for split/merge without reduction; implementation-dependent for full cycle)
- **Premises:** Q1 (IC-net definition), Q2 (σ allocation function), Q3 (bidirectional FreePort mechanism), Q4 (linearity at borders), Q5/C1-C3 (correctness conditions: agent coverage, wire coverage, border bijection), Q6 (interaction locality), Q7 (isomorphism)
- **Thesis (condensed):** Partitioning an IC net via an allocation function σ with border wires as paired FreePorts preserves structure under C1-C3: `merge(split(μ)) = μ`; local reduction of internal redexes equals global reduction; partition quality affects performance only, never correctness.
- **Specs that depend:** SPEC-04 (partitioning), SPEC-05 (merge); underpins ARG-001 P2.
- **Basis:** DISC-004 v2 (+ DISC-003 v2).

### ARG-003 — Centralized Merge Protocol Guarantees Border Completeness (v1 exhaustive)
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\argumentos\ARG-003-protocolo-completude-fronteira.md`
- **Date / Strength:** 2026-03-25 · Moderate (strong by construction; empirical coverage of 1/6 rules in v1)
- **Premises:** R1 (border redex anatomy), R2 (two origins: cut + emergent via CON-DUP), R3 (FreePort as wire-reference, not agent-reference), R4 (C1-C3 from ARG-002), R5 (strong confluence), R6 (split/merge/remap correctness), R7 (multi-round cycle)
- **Thesis (condensed):** Bidirectional FreePort + exhaustive `findBorderRedexes` + sequential coordinator resolution + round cycle guarantees (a) no existing border redex is lost, (b) emergent border redexes from CON-DUP are captured next round, (c) the cycle terminates at the unique normal form.
- **Specs that depend:** SPEC-06/SPEC-13 (BSP coordinator), SPEC-05 (merge); underpins ARG-001 P3.
- **Basis:** DISC-005 v2 (+ DISC-003 v2, DISC-004 v2).

### ARG-004 — Feasibility and Practical Limits of Distributed IC Reduction
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\argumentos\ARG-004-viabilidade-limites-praticos.md`
- **Date / Strength:** 2026-03-25 · Moderate (sub-arguments range Strong–Weak; see its internal summary table)
- **Premises:** V1 (correctness from ARGs 001-003), V2 (decomposable overhead by phase), V3 (O(N²) `findRedexes` is implementation artifact), V4 (Haskell prototype experimental data), V5 (six shared→distributed dimensions), V6 (no-fault scope); profile conditions A/B/C
- **Thesis (condensed):** Distributed IC reduction is viable and efficient under identifiable conditions (per-round local/overhead ratio, round count, border fraction) but not universally advantageous — correctness is unconditional, efficiency is workload-shaped. Defines Profiles A (embarrassingly parallel), B (expansion+collapse, real target), C (sequential dependency — slowdown).
- **Specs that depend:** SPEC-14 (benchmark cenarios), ROADMAP §2.40 (break-even c_o/c_r = 2.2).
- **Basis:** DISC-006 v2, DISC-007 v2, DISC-008 v2.

### ARG-005 — Delta Border Completeness (extension of ARG-001 to the delta protocol)
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\argumentos\ARG-005-delta-border-completeness.md`
- **Date / Strength:** 2026-04-24 · Moderate-Strong (same class as ARG-001/ARG-003)
- **Premises:** P1-P6 inherited from ARG-001; adds P7 (C-DEL1 delta-reporting completeness), P8 (C-DEL2 delta-reporting soundness), P9 (determinism of `reconstruct`). Scope E-FAIL1: stable coordinator during R13-R15.
- **Thesis (condensed):** Under P1-P6 + C-DEL1/C-DEL2, the SPEC-19 delta protocol satisfies (a) R38: `reduce_all(N) ~ extract_result(run_grid_delta(N, n))` up to graph isomorphism; (b) R39: `BorderGraph.detect_border_redexes()` returns the same set as the exhaustive v1 scan; (c) R40: termination in finite rounds. Proof uses an induction-on-rounds invariant **(INV-REC)** that the distributed pair `(B_k, {N_{w,k}})` is isomorphic to a valid intermediate state `μ_k` of sequential reduction.
- **Specs gated:** SPEC-19 §3.5 R38 (G1 reformulated), R39 (D3 incremental ≡ exhaustive), R40 (D6 reformulated); closes SPEC-19 §8 OQ-1.
- **Basis:** DISC-011 v2 (formal pillar), DISC-013 (naming disambiguation).
- **Limitations / open work:** Static worker set only (elastic case is ARG-006). Categorical formalization and Coq/Lean mechanization left as future work. Empirical signature pending: SPEC-19 tests T6-T16.

### ARG-006 — Mixed-Trace Recoverability (extension of ARG-001 to elastic departure)
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\argumentos\ARG-006-mixed-trace-recoverability.md`
- **Date / Strength:** 2026-04-24 · Moderate-Strong (same class as ARG-001)
- **Premises:** P1-P6 inherited from ARG-001; adds P10 (idempotence of `reduce_all` over terminating intermediates; corollary of P1+P6), P11 (retained-snapshot consistency from SPEC-20 R23 + R31), P12 (mixed-trace recoverability; corollary of P1+P10)
- **Thesis (condensed):** Re-introducing a reclaimed partition from a worker that departed (timeout R18, connection loss R19, or urgent `LeaveRequest` R22b) into a new round under the surviving coordinator yields a final normal form structurally identical to the one that would have resulted had the partition never been reclaimed. For any mixed BSP × departure × reclaim trace within terminating-nets scope, the final NF is invariant. The enabling property is idempotence of reduction on terminating nets (P1+P6) plus SPEC-20 R24c preventing divergent partitions from merging.
- **Specs gated:** SPEC-20 §3.3 R18-R26; §3.3.5 R29/R29a; §3.7 R39 R39-G1-elastic-departure; SPEC-01 G1 under elastic mode. Closed for v1 mode; delta mode CONDITIONAL on ARG-005 (via R24b-delta gated; R24a conservative path covers delta immediately).
- **Basis:** ARG-001 (P1-P6), DISC-007 v2 (fault tolerance + confluence), DISC-011 v2 (INV-REC), DISC-013 (naming).
- **Limitations / open work:** Coordinator replication out of scope. Byzantine extension out of scope. Fairness assumption for termination under adversarial churn is open. Empirical signature pending: SPEC-20 tests EG-I3, EG-I5a, EG-P2, EG-P5.

---

## Discussions (DISC-NNN)

Located at: `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\`

Thirteen discussions. Twelve went through 2-round adversarial ping-pong (Round 1 critico → Round 2 defensor → v2 synthesis); DISC-013 is a single-round decision note. Only the v2 final synthesis paths are indexed here — see the source for Round 1/Round 2 companion files when full adversarial history is needed.

### Block 1 — Foundations

#### DISC-001 v2 — Properties of ICs Relevant to Distribution
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\DISC-001-v2-propriedades-ics-distribuicao.md`
- **Summary:** Establishes F1 (strong confluence), F2 (locality), F3 (finite rule set) as distribution-relevant IC properties; maps each to protocol implications.
- **Informs:** ARG-001 (all premises), SPEC-01 (invariants).

#### DISC-002 v2 — The Literature Gap
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\DISC-002-v2-gap-literatura.md`
- **Summary:** Surveys the state of the art and identifies three sub-gaps where existing work (Mackie, HVM2, DDGrid, DPC) does not cover distributed IC reduction in grid settings.
- **Informs:** TCC §1 (motivation), TCC §3 (related work).

### Block 2 — Central Argument

#### DISC-003 v2 — Strong Local Confluence → Distributed Determinism
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\DISC-003-v2-confluencia-determinismo-distribuido.md`
- **Summary:** Six-perspective analysis (including emergent border redexes in Perspectives 5-6) deriving the engineering-layer proof obligations for ARG-001.
- **Informs:** ARG-001 (all premises), ARG-002, ARG-003.

#### DISC-004 v2 — Formal Partitioning of IC Networks
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\DISC-004-v2-particionamento-formal.md`
- **Summary:** Formalizes σ-allocation, bidirectional FreePort border mechanism, C1-C3 correctness conditions; proves split/merge identity without reduction.
- **Informs:** ARG-002, SPEC-04, SPEC-05.

#### DISC-005 v2 — Cross-Boundary Interaction Protocol
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\DISC-005-v2-protocolo-cross-boundary.md`
- **Summary:** Anatomy of border redexes (pre-existing cut vs CON-DUP emergent); FreePort as wire-reference not agent-reference; 6-dimension comparison HVM2/HVM4/Grid.
- **Informs:** ARG-003, SPEC-06, SPEC-13.

### Block 3 — Feasibility

#### DISC-006 v2 — Communication Overhead and Granularity
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\DISC-006-v2-overhead-comunicacao-granularidade.md`
- **Summary:** Per-phase cost model (6 phases); formal O(N²) derivation for v1 `findRedexes`; payload recalculation with decreasing DualTree; per-level Amdahl replacement.
- **Informs:** ARG-004, SPEC-14, ROADMAP §2.40 (break-even).

#### DISC-007 v2 — Fault Tolerance and Confluence
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\DISC-007-v2-tolerancia-falhas-confluencia.md`
- **Summary:** P1-P5-anchored treatment of fault classes (Class 1 transient, Class 2 permanent); structural implications of strong confluence for recovery.
- **Informs:** ARG-004, ARG-006, SPEC-20 (elastic).

#### DISC-008 v2 — From Shared to Distributed Memory
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\DISC-008-v2-memoria-compartilhada-para-distribuida.md`
- **Summary:** Six dimensions of transition (latency, global state visibility, ID allocation, redex detection, serialization, sync/termination); cites Mackie as prior art.
- **Informs:** ARG-004, SPEC-06, SPEC-08 (serialization).

### Supplementary

#### DISC-009 v2 — Problem Types: Streaming vs Batch, Relativist Candidacy
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\DISC-009-v2-tipos-problema-streaming-batch.md`
- **Summary:** Replaces 1-D A/B/C profile with a 5-axis taxonomy (input size, dependency structure, granularity, bottleneck location, decomposition determinism). Three streaming levels (rule-level, state-protocol, generation-protocol). Five operating modes (centralized batch, streaming generation, recipe-based, delta state streaming, pull-based) × 10-workload matrix with OK/PREF/DEG/NA classification. 5-question yes/no algorithm for classifying new workloads.
- **Informs:** SPEC-14 §8, SPEC-21 Stage 0 (streaming generation), SPEC-25 Stage 0 (recipe generation), ARG-005, ARG-006.

#### DISC-010 — Real-World Applications of Distributed IC Reduction
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\DISC-010-aplicacoes-reais-ic-distribuido.md`
- **Summary:** Six candidate domains (functional evaluation, formal verification, symbolic computation, data transformation, smart contracts, compilation); six adequacy criteria (C1-C6); counter-examples (TSP, ML, CFD); differential is correctness-by-construction. (Single round v1, no ping-pong.)
- **Informs:** TCC §5 (Discussion).

#### DISC-011 v2 — Distributed State Decomposition (delta protocol)
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\DISC-011-v2-distributed-state-decomposition.md`
- **Summary:** Formalizes (B, {N_w}) ~ N via Q-DEC1 (structural isomorphism at convergence); preservation under reduction via Theorem **(INV-REC)** by induction on rounds, conditional on C-DEL1 (delta-reporting completeness) and C-DEL2 (soundness). Five sub-cases of CON-DUP treated individually (§4.2-bis). Sanity-check n=1 (§4.4). Uniqueness up to isomorphism given 3 fixings (σ, bid, orient).
- **Informs:** ARG-005, SPEC-19 §3.5, SPEC-19 §8 OQ-1.

#### DISC-012 v2 — Job Submission, Problem Encoding, Result Decoding
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\DISC-012-v2-job-submission-encoding-decoding.md`
- **Summary:** 3 pain points (giant net, no decode, only Church); 8 options (A-H including HVM as encoder); encode→reduce→decode contract; HVM/Bend viability investigation; v2 recommendation in 5 layers (~1900 LoC); pure Lambda Calculus PoC (REF-005).
- **Informs:** SPEC-22 (Job submission), SPEC-23 (Encoding pipeline), SPEC-25 (Recipe generation).

#### DISC-013 — ARG-005 vs ARG-006 Disambiguation (single-round decision note)
- **Source path:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\DISC-013-arg-005-disambiguation.md`
- **Summary:** Pragmatic decision (no ping-pong): ARG-005 stays with SPEC-19 (delta border completeness, prerequisite DISC-011); SPEC-20's argument is renamed to ARG-006 (mixed-trace recoverability, gates R29a/R39). Cleanup task: 15 mentions of "ARG-005" in SPEC-20 must be substituted to "ARG-006" + §11 change log entry. SPEC-19 unchanged.
- **Informs:** ARG-005, ARG-006, SPEC-19, SPEC-20, ESPECIALISTA-SPECS cleanup task.

---

## References (REF-NNN)

Located at: `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\biblioteca\fichas\` (fichas); BibTeX at `biblioteca\referencias.bib`; PDFs at `references\` (TCC root).

Eighteen references catalogued (8 essential, 8 useful, 2 marginal). Per-reference reading cards (`fichas/REF-NNN_*.md`) contain the full analytical reading; here we list only 1-line summaries and primary Relativist citations.

### Foundations

- **REF-001 — Lafont 1990 — Interaction Nets.** Foundational paper: linearity, binary interaction, locality. *Cited by:* SPEC-00, SPEC-01, most foundational prose.
- **REF-002 — Lafont 1997 — Interaction Combinators (FUNDAMENTAL).** Proposition 1 p.73 (strong confluence with auto-interaction); Theorem 1 (universality of γ/δ/ε). *Cited by:* ALL specs (foundational); ARG-001, ARG-002, ARG-003, ARG-005, ARG-006.
- **REF-005 — Mackie & Pinto 2002 — Encoding Linear Logic with IC.** Cut elimination; multiplexing/packing nets; Lemma 2.1 (normal-form uniqueness in terminating nets). *Cited by:* ARG-001 P5, DISC-012 v2.
- **REF-006 — Mazza & Ross 2012 — Full Abstraction for Set-Based Models.** Semantic completeness of IC. *Cited by:* TCC §2.1.
- **REF-018 — Arrighi et al. 2024 — Space-Time Deterministic Graph Rewriting.** Argues confluence alone insufficient for non-terminating distributed computations — motivates scope P6. *Cited by:* ARG-001 P6, TCC §5.

### Implementation / Technique

- **REF-003 — Taelin 2024 — HVM2.** Massively parallel IC evaluator; extended IC set. *Cited by:* All cross-cutting technique specs (AC-006–AC-008).
- **REF-013 — Mackie 1997 — Static Analysis of INets for Distributed Impl.** Configuration as pair (A, W); abstract interpretation for initial distribution. *Cited by:* SPEC-04, SPEC-08.
- **REF-014 — Kahl 2015 — Simple Parallel Implementation of INets in Haskell.** MVar-based fine-grain parallelism; polarity model. *Cited by:* Prototype reference analyses.
- **REF-015 — Mackie & Sato 2015 — Parallel Evaluation of INets: Case Studies.** Rule-level streaming reference. *Cited by:* DISC-009 v2 (streaming level 1), SPEC-14 §8.
- **REF-016 — Pinto 2000 — Sequential/Concurrent Abstract Machines for INets.** *Cited by:* Reference analyses.

### Distributed / Grid

- **REF-004 — Andersen & Sergey 2021 — DPC (Protocol Combinators).** Session-typed distributed protocol modeling. *Cited by:* TCC §2.3.
- **REF-007 — Casanova 2002 — Distributed Computing Issues in Grid Computing.** *Cited by:* TCC §2.4, grid challenges.
- **REF-008 — Montanari & Rossi c.1997 — Distributed Systems with Sync (Graph Rewriting).** Binary port synchronization. *Cited by:* DISC-008 v2, TCC §5.
- **REF-009 — Degano & Montanari 1987 — A Model for Distributed Systems (Graph).** Partial causal ordering. *Cited by:* TCC §5.
- **REF-011 — Wang et al. 2008 — DDGrid: Concurrency and Fault Tolerance.** Master-worker + SEDA. *Cited by:* DISC-007 v2, ROADMAP.
- **REF-017 — Foster, Kesselman & Tuecke 2001 — The Anatomy of the Grid.** Virtual organizations; layered architecture. *Cited by:* TCC §2.4.

### Marginal / Supplementary

- **REF-010 — Blostein et al. 1995 — Graph Rewriting Issues.** *Cited by:* TCC §5.
- **REF-012 — Saraph & Herlihy 2019 — Smart Contracts (Speculative Concurrency).** Semantic commutativity. *Cited by:* DISC-010.

---

## Code Analyses (AC-NNN)

Located at: `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\biblioteca\analise-codigo\`

Fifteen technical analyses of reference implementations. Each AC documents extracted techniques and flags applicability to Relativist (SIM / NAO / PARCIAL / AVALIAR). Full technique catalogue and Haskell→Rust type/pattern mapping live in `biblioteca/analise-codigo/INDICE.md`.

### Haskell Prototype (historical reference; `codigo/grid_computing_interaction_combinators_prototype_v1/`)
- **AC-001 — IC.Core** — reduction rules, redex bag, FreePort, budget.
- **AC-002 — IC.Partition** — σ allocation, border map, split/merge.
- **AC-003 — IC.Protocol + IC.Network** — TLV codec, TCP coordinator loop, ID remap.
- **AC-004 — IC.Grid + IC.TreeMapReduce** — 5-phase coordinator, effective-workers, CoV balance metric.
- **AC-005 — IC.Benchmark + Experimental Results** — BenchDef record, sequential baseline, EP/DualTree/Expansion generators.

### HVM2 (`codigo/higher_order_co/HVM2/`)
- **AC-006 — Types + Memory** — port encoding, arena, RBag, GNet, TMem.
- **AC-007 — Reduction Engine** — 8×8 dispatch table, atomic link with ownership, on-the-fly redex detection.
- **AC-008 — AST + Compilation** — FIDs, template+relocation injection.

### HVM4 (`codigo/higher_order_co/HVM4/`)
- **AC-009 — Term + Heap** — 64-bit term encoding (SUB+TAG+EXT+VAL), unified heap, bump allocation, cache-line padding.
- **AC-010 — WNF Evaluation + Interactions** — goto enter/apply state machine, frame reuse, 1-file-per-interaction.
- **AC-011 — Threading + Work-Stealing** — Chase-Lev deque, fork-join, static heap partitioning.

### Optiscope (`codigo/higher_order_co/optiscope/`)
- **AC-012 — Optimal Reduction** — pool allocator, native-pointer ports, Graphviz debug, Lévy-optimal.

### Bend (`codigo/higher_order_co/Bend/`)
- **AC-013 — Compilation to Nets** — binary-tree encoding, port-directed readback, vicious-cycle detection.

### Bench (`codigo/higher_order_co/bench/`)
- **AC-014 — Benchmark Methodology** — hrtime wall-clock, discover-configure-execute-display runner, timeline viz.

### Cross-Cutting
- **AC-015 — Cross-Cutting Synthesis** — comparison across 8 axes for all above implementations.

---

## Open Theoretical Debts

| Debt | Status | Owner | Source |
|---|---|---|---|
| ARG-005 categorical formalization (pushout-based perspective) | OPEN (future work) | DEBATEDOR | DISC-011 v2 §1 (Perspective 1 demoted to motivation) |
| ARG-005 Coq/Lean mechanization | OPEN (future work) | n/a | ARG-005 §Limitations |
| ARG-006 coordinator replication | OUT OF SCOPE | n/a | ARG-006 §Limitations |
| ARG-006 Byzantine extension | OUT OF SCOPE | n/a | ARG-006 §Limitations |
| ARG-006 fairness assumption for termination under adversarial churn | OPEN (future work) | n/a | ARG-006 §Limitations |
| Empirical signature of ARG-005 (SPEC-19 tests T6-T16) | PENDING — gated on SPEC-19 test execution | DEVELOPER (future) | SPEC-19 §8, ARG-005 §Closing |
| Empirical signature of ARG-006 (SPEC-20 EG-I3, EG-I5a, EG-P2, EG-P5) | PENDING — gated on SPEC-20 test execution | DEVELOPER (future) | SPEC-20 §4, ARG-006 §Closing |
| ARG-003 broader empirical coverage (beyond ERA-ERA, the only rule exercised in v1 distributed) | PARTIAL — noted as strength caveat | DEVELOPER (future benchmarks) | ARG-003 §Forca |

---

## Usage Pattern for Relativist Agents

**spec-critic:** Before flagging *"ARG-XXX is pending"* or *"spec X cites unknown ARG"*, consult this file. If the ARG exists here with status CLOSED (ARG-001 through ARG-006 all CLOSED at last update), the spec citation is valid — demand only that the spec link to the absolute path listed here. If an ARG ID not in this file is cited, flag as unresolved. Same rule for DISC/REF references.

**especialista-specs:** When revising a spec to cite an ARG/DISC/REF, copy the absolute path from this file into the spec's frontmatter under `Arguments consumed:`, `Discussions consumed:`, or `References consumed:`. Never invent paths. When a new ARG/DISC/REF is needed but does not yet exist, record the gap in the spec's open-questions section and surface it via the TCC-root `debatedor` workflow — do not author the missing artifact from within Relativist.

**task-splitter / test-generator:** When a task or test verifies an invariant defended by an ARG, cite the ARG ID and absolute path in the task/test description. For empirical-signature tests, consult "Open Theoretical Debts" above for the canonical list (T6-T16 for ARG-005; EG-I3, EG-I5a, EG-P2, EG-P5 for ARG-006).

**developer / reviewer / qa:** Do not follow ARG/DISC/REF paths during normal coding — the 1-paragraph summary here is canonical for *"what does the theory say?"*. Only follow absolute paths when you need to cite a specific premise (e.g. ARG-001 P4) in code comments or PR descriptions. Code comments explaining IC concepts (per user preference) should cite ARG/REF IDs plus this bridge, not raw paths.

**sdd-pipeline (Relativist orchestrator):** When validating a spec's readiness, check that all cited ARG/DISC/REF IDs appear in this file. An ID absent from this bridge is a hard block — request an update (run from TCC root) before allowing the spec to advance stages.

**All agents:** If this file appears stale (date older than what a spec you are reading claims to consume), report the drift rather than proceeding — this is an integrity boundary between the TCC academic layer and the Relativist implementation layer.
