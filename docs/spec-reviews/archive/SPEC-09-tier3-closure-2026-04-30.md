# SPEC-09 Tier 3 Measurement Protocol — Round 2 Closure Log

**Date:** 2026-04-30
**Bundle:** D-011 Phase F-1
**Spec:** `specs/SPEC-09-benchmarks.md` (now v3.2)
**Round 1 review:** `docs/spec-reviews/SPEC-09-tier3-round1-2026-04-30.md`
**Defender:** especialista-specs (self-review adversarial mode, Round 2)
**Pipeline gate:** Phase C (developer wires bench harness) and Phase D (developer wires SparseNet micro-bench) unblocked after this commit.

---

## Disposition of Round 1 findings

| Finding | Severity | Disposition | Edit location |
|---------|----------|-------------|---------------|
| F1 — R18a measurement-point ambiguity | HIGH | **CLOSED** by Round 2 patch: R18a now defines a per-path bullet list pinning the sample to (eager) post-`make_net`, (streaming) post-`generate_and_partition_chunked_*`, (sparse) post-`to_dense`. Sampling at the iterator-exhaustion point on the streaming path is explicitly FORBIDDEN. | SPEC-09 §3.3.X R18a |
| F2 — R37c reference-net poisons R18a | HIGH | **CLOSED** by Round 2 patch: R37c now contains an explicit 6-step sequencing constraint (build Tier 3 net → sample R18a → build reference → run isomorphism → discard reference → run reduction). R18a is frozen for the row at step 2; reference net's allocation in step 3 cannot influence R18a or R18b. The "separate verification run" alternative is preserved as MAY. | SPEC-09 §3.6 R37c |
| F3 — R18c ill-defined for Sparse + streaming | MEDIUM | **CLOSED** by Round 2 patch: R18c rewrites the discipline as a numbered first-match-wins dispatch list with Sparse priority over streaming. This preserves orthogonality of the two axes and avoids the streaming-sum vs. SparseNet-len divergence on forward references. | SPEC-09 §3.3.X R18c |
| F4 — CSV column-order duplicated | MEDIUM | **CLOSED** by Round 2 patch: the joinability gate block no longer duplicates the v1 column list verbatim; instead it cites R39a as the source of truth. R39a was extended to include the 7 new columns explicitly, with a clarifying paragraph stating that the leftmost 22 columns are frozen by `v1_local_baseline/phase2_detail.csv`. | SPEC-09 §3.3.X joinability gate; SPEC-09 §3.7 R39a |
| F5 — RecyclePolicy ownership ambiguity | LOW | **CLOSED** by Round 2 patch: R18g now states that the Rust enum is FIRST DEFINED HERE in SPEC-09 §4.1 for cross-module use, while the SEMANTICS of the variants are governed by SPEC-22 R10b/R10c. Wire-format / persistence concerns defer to SPEC-22. | SPEC-09 §3.3.X R18g |
| F6 — `max_pending_lifetime` ignored-case underspecified | LOW | **CLOSED** by Round 2 patch: §4.9.1 now states explicitly that when `chunk_size == None`, the `max_pending_lifetime` field MUST be ignored (not propagated, not validated, not altering eager-path behavior). | SPEC-09 §4.9.1 |
| F7 — EAGER gate procedurally underspecified | LOW | **CLOSED** by Round 2 patch: §4.9.2 EAGER gate now contains an explicit footnote: if v1 summary lacks CI columns, post-processing MUST recompute them from `phase2_detail.csv` per R32a's offline-bootstrap escape hatch (10,000 resamples on the median). | SPEC-09 §4.9.2 |
| F8 — Brief vs. SPEC-22 §3.6 mismatch | INFO | **DOCUMENTED, no spec change.** SPEC-22 has no §3.6; the cross-reference was placed at the end of §3.1 (free-list, after R12), where SPEC-22's free-list lifecycle requirements live. The brief's "§3.6" was a typo for "§3.1". | SPEC-22 §3.1 (after R12) |

All 8 findings dispatched; 7 patched, 1 (F8) documented as a brief-side artifact. No findings carried over to a future round.

---

## Verification of Round 2 patches

I re-read each patched section in SPEC-09 against its corresponding Round 1 finding:

- **F1 verification:** R18a's bullet list now contains three orthogonal program points keyed on `chunk_size` and `representation`. The streaming bullet pins the sample to "AFTER `merge::generate_and_partition_chunked_with_chunk_size_and_lifetime` returns AND BEFORE the first `AssignPartition` is dispatched", which is point (b) per the Round 1 review's terminology. Sampling at point (a) is explicitly named and FORBIDDEN. PASS.
- **F2 verification:** R37c's 6-step sequence places R18a sampling (step 2) before reference allocation (step 3). The remaining sentence about R18b notes that the reference net's `VmHWM` contribution lives in legacy `peak_memory_bytes` but NOT in R18b unless reduction itself rises above it, which is monotonically true. PASS.
- **F3 verification:** R18c's three numbered rules with first-match-wins dispatch eliminate the Sparse+streaming ambiguity. The Sparse rule (rule 1) takes priority over the streaming rule (rule 2). The double-counting concern around forward references is addressed by the explicit "the sum is over distinct AgentIds emitted by the generator iterator" phrase. PASS.
- **F4 verification:** The joinability gate block no longer contains the verbatim 22-column list; R39a now contains the canonical 22+7 list with a frozen-by-v1 anchor. A future amendment to R39a's column order will update one place, not two. PASS.
- **F5 verification:** R18g now states the type/semantics ownership division explicitly. PASS.
- **F6 verification:** §4.9.1 closes with an explicit "ignore on eager path" sentence. PASS.
- **F7 verification:** §4.9.2 EAGER gate carries a footnote explaining the offline-bootstrap escape hatch. PASS.

All 7 patches verified.

---

## Change inventory (final)

### `specs/SPEC-09-benchmarks.md` (v3.1.2 → v3.2)

1. **Status header** updated to v3.2.
2. **§3.3 (after R18 BenchmarkResult struct):** new subsection `3.3.X Streaming Representation Metrics` introducing R18a–R18g (7 new metrics) plus a backward-compatibility joinability-gate block.
3. **§3.4 (after R29):** three new subsections — §3.4.5 Chunk size (R29a), §3.4.6 Recycle policy (R29b), §3.4.7 Representation (R29c).
4. **§3.6 (between R37b and R38):** new requirement R37c for construction-phase isomorphism with explicit 6-step sequencing.
5. **§3.7 R39a:** schema block extended with 7 new columns and an anchor paragraph.
6. **§4.1 Types:** new enums `NetRepresentation` (Dense/Sparse) and `RecyclePolicy` (DisableUnderDelta/BorderClean); `BenchmarkSuiteConfig` extended with 4 new fields (`chunk_size`, `recycle_policy`, `representation`, `max_pending_lifetime`).
7. **§4.9 (new subsection):** `Streaming Architecture (Tier 3 measurement protocol)` covering EAGER/STREAMING path selection, recycle policy, representation, the `max_pending_lifetime` budget (§4.9.1), three acceptance gates (§4.9.2: EAGER vs. baseline, STREAMING memory-scaling, SPARSE micro-bench), and provenance discipline (§4.9.3).

### `specs/SPEC-21-streaming-generation.md`

1. **§3.5 (end of section, after R29b's closing R15↔I3' note):** one-line cross-reference to SPEC-09 §4.9 for the bench-harness measurement protocol.

### `specs/SPEC-22-arena-management.md`

1. **§3.1 (end of section, after R12, before §3.2):** one-line cross-reference to SPEC-09 §3.3.X (R18c–R18g) and §4.9 for the BenchmarkResult provenance fields and SparseNet micro-bench acceptance gate.

---

## Out of scope (confirmed)

- No code edits under `relativist-core/src/`. `BenchmarkResult` struct extension lives in Phase C developer scope.
- No edits to `relativist-core/src/bench/memory.rs`. `get_peak_memory_at_construction_complete` lives in Phase C developer scope.
- No edits to `docker-compose.yml` / `Dockerfile`. Phase E.
- No edits to `DATA-COLLECTION-PLAN.md`. The brief notes this should be created in Phase F-2 closure if absent; this closure log records that the file does not exist on `v2-development` as of 2026-04-30 and recommends creating it as part of the Phase F-2 closure.

---

## Gate state

- Phase F-1 spec amendment: **CLOSED**.
- Phase A (SPEC-19 R35a): independent of F-1; status unchanged.
- Phase B-2 (`PartitionError::PendingLifetimeExceeded`): referenced by §4.9.1; remains a developer task.
- Phase C (developer wires bench harness with R18a–R18g + R29a–R29c + R37c + §4.9): **UNBLOCKED**.
- Phase D (developer wires SparseNet micro-bench, dual_tree only, with §4.9.2 SPARSE gate): **UNBLOCKED**.
- Phase E (Dockerfile / docker-compose for Tier 3 rodada): independent of F-1.
- Phase F-2 (DATA-COLLECTION-PLAN authoring): not started; recommended next.

---

## Commit

Single commit on `v2-development`:

```
spec(d-011): amend SPEC-09 — Tier 3 measurement protocol (R18a–R18g, R37c, §4.9)
```

Files in the commit:

- `codigo/relativist/specs/SPEC-09-benchmarks.md`
- `codigo/relativist/specs/SPEC-21-streaming-generation.md`
- `codigo/relativist/specs/SPEC-22-arena-management.md`
- `codigo/relativist/docs/spec-reviews/SPEC-09-tier3-round1-2026-04-30.md`
- `codigo/relativist/docs/spec-reviews/SPEC-09-tier3-closure-2026-04-30.md`

Do NOT push (per dispatch brief).
