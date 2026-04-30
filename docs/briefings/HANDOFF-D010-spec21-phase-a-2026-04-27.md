# Handoff Brief — D-010 Phase A (SPEC-21 Amendments A1..A8)

**Date:** 2026-04-27
**Target session:** TCC root (`C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing`)
**Target agent:** `especialista-em-specs` (Tier 1, root `.claude/agents/`)
**Why root, not subdir:** Rule 7 — only ESPECIALISTA EM SPECS edits `codigo/relativist/specs/`. The relativist subdir session has no Tier 1 spec specialist; the root session does.

## Mission

Land 8 amendments (A1..A8) on the predecessor specs that SPEC-21 (Streaming Generation) depends on, exactly as specified in `codigo/relativist/specs/SPEC-21-streaming-generation.md` §3.8. The amendments unlock D-010 Phase B (developer implementation in `relativist-core/`).

## Context — what just shipped (D-009 closed, D-010 next)

- Bundle **D-009 / SPEC-22 Arena Management** closed on `v2-development` (commit `d2e8e60` is the close-out). 9 commits, full SDD cycle, test floor 1308 → **1464 default / 1507 zero-copy**.
- D-010 / SPEC-21 Stages 1+2 (SPLITTING + TESTS) were already closed in commit `131ca26` (Pre-DEV Wave 2 second half, 2026-04-25): 36 atomic tasks `TASK-0510..0517` (Phase A) + `0520..0524` (Phase B foundation) + `0530..0531` (Phase C strategies) + `0540..0544` (Phase D benchmarks) + `0550..0554` (Phase E accumulator) + Phase F polish — and 49 TEST-SPECs.
- Phase A amendments A1..A8 are listed in SPEC-21 §3.8 but have NOT been landed in the predecessor specs yet. **Subdir session verified:** `grep -c "RequestWork\|NoMoreWork" codigo/relativist/specs/SPEC-06-wire-protocol.md` returns 0 — A2 is missing.

## Deliverables

### 1. Amend predecessor specs (8 edits, ~S each)

| Amendment | Target spec | What | Source-of-truth |
|---|---|---|---|
| **A1** | `codigo/relativist/specs/SPEC-04-partition.md` | New R12 (border-id allocation for streaming pipeline) | SPEC-21 §3.8 A1 |
| **A2** | `codigo/relativist/specs/SPEC-06-wire-protocol.md` | `Message` enum gains `RequestWork` + `NoMoreWork` variants; `PROTOCOL_VERSION` bump using **defensive `PREVIOUS_LIVE_VERSION + 1`** language (NOT a hardcoded integer — D-009 just bumped to 5; this lands relative-to-current) | SPEC-21 §3.8 A2 |
| **A3** | `codigo/relativist/specs/SPEC-07-deployment.md` | `GridConfig` gains `chunk_size: usize`, `streaming_strategy: StreamingStrategy`, `dispatch_mode: DispatchMode` (+ optional `max_pending_lifetime` per R37g) | SPEC-21 §3.8 A3 |
| **A4** | `codigo/relativist/specs/SPEC-09-benchmarks.md` | `Benchmark` trait gains `make_net_stream(...) -> impl Iterator<Item = AgentBatch>` with default impl that wraps `make_net()` (R10/R11) | SPEC-21 §3.8 A4 |
| **A5** | `codigo/relativist/specs/SPEC-13-system-architecture.md` | Coordinator FSM gains 5 streaming states; worker FSM gains 2 pull-only states | SPEC-21 §3.8 A5 |
| **A6** | `codigo/relativist/specs/SPEC-22-arena-management.md` | R10b broadening — gate becomes `(delta_mode \|\| streaming_active)` (closes SC-007) | SPEC-21 §3.8 A6 |
| **A7** | `codigo/relativist/specs/SPEC-19-delta-protocol.md` | `BorderGraph` gains `extend_with_chunk_borders(...)` method signature | SPEC-21 §3.8 A7 |
| **A8** | `codigo/relativist/specs/SPEC-04-partition.md` §4.5 | Clarification — `split()` unchanged; chunked pipeline is purely additive | SPEC-21 §3.8 A8 |

### 2. Closure log

Write `codigo/relativist/docs/spec-reviews/CLOSURE-D010-amendments-A1A8-2026-04-27.md` documenting per-amendment: target spec, line(s) edited, R-numbers landed, brief justification.

### 3. Status bumps

In each amended predecessor spec, update the `**Status:**` header line to add a marker like `+ A8 amend (D-010, 2026-04-27)` or similar — match whatever convention SPEC-22 used after D-009 Phase A (commit `01184f1`).

## Constraints / non-negotiable rules

1. **DO NOT edit code** in `relativist-core/src/` or `tests/` — Phase B onwards is the developer agent's territory.
2. **DO NOT bump PROTOCOL_VERSION to a hardcoded integer.** D-009 just bumped it to 5. Use defensive `PREVIOUS_LIVE_VERSION + 1` language exactly as TEST-SPEC-0511 / TEST-SPEC-0576 already write defensively. Spec text should say something like "PROTOCOL_VERSION ← PREVIOUS_LIVE_VERSION + 1 (= 6 at the time of writing, given current value 5 from D-009 Phase A)."
3. **DO NOT relax** any existing R-numbers; only add (R12 in SPEC-04, R10b broadening in SPEC-22 etc).
4. **DO NOT edit** `codigo/relativist/specs/SPEC-21-streaming-generation.md` itself — the source of truth is unchanged; you're propagating its §3.8 amendments to predecessor specs.
5. **Match the formatting convention** SPEC-22 amendments used (commit `01184f1`) — same header style, same in-line `<!-- D-009 -->` tag style if present.

## Reference — D-009 Phase A as template

- Commit: `01184f1` — "docs(spec): land SPEC-22 amendments A1..A10 — D-009 Phase A"
- 7 spec files modified (SPEC-01/02/03/04/05/18/19) with A1..A10
- Closure log: `codigo/relativist/docs/spec-reviews/CLOSURE-D009-amendments-A1A10-2026-04-27.md`
- Use `git show 01184f1 --stat` and `git show 01184f1 -- specs/SPEC-18-wire-format-v2.md` to see exact format style

## Success criteria

- [ ] 8 amendments landed verbatim per SPEC-21 §3.8.
- [ ] Closure log written with line-level citations for each amendment.
- [ ] No code touched. No tests touched.
- [ ] Single commit (or 2 if you prefer one for amendments + one for closure log) with message `docs(spec): land SPEC-21 amendments A1..A8 — D-010 Phase A`.
- [ ] After commit, the relativist subdir session can run `git log --oneline | head` and see the new commit, then dispatch the developer agent for Phase B.

## After Phase A lands — what happens next in the subdir

The subdir session will pick up `git pull` (or just `git log` since same working tree) and dispatch:
1. **Stage 3 DEV — developer Sonnet** for D-010 Phase B (`TASK-0520..0524` foundation types: `ConnectionDirective`, `AgentBatch`, `StreamingPartitionStats`, `ChunkedPartitionResult`, `StreamingPartitionStrategy` trait), then C/D/E/F.
2. **Stages 4-6** — reviewer / qa / refactor as usual.

## TASK-0510..0517 backlog files

Already exist in `codigo/relativist/docs/backlog/`. Read them for per-amendment task scope before editing the spec file. They map 1:1 to A1..A8.

---

**End of brief.** Paste this entire file (or its mission/deliverables/constraints sections) into the TCC root session and invoke `especialista-em-specs`.
