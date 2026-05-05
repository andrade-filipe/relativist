# Phase B/C/D/E Audit Summary — Non-Claude LLM DEV Output

**Date:** 2026-04-27
**Subject:** Stage 4 + 5 audit of 12 commits (`4fb77bc..a84cb37`) implementing TASK-0415..0452 (SPEC-20 Elastic Grid bundle D-006), authored by a non-Claude LLM during Stage 3 DEV.
**Question being answered:** "Did the alternative-LLM DEV pass survive an adversarial review?"

## Top-line answer

**No.** The bundle compiles, `cargo test` passes (1256 lib / 1299 zero-copy), `cargo clippy -D warnings` is clean — but it harbors **14 CRITICAL + 23 HIGH bugs** distributed across all four phases. Two of the four phases received explicit **REJECT** verdicts at Stage 4. The break-even-analysis benchmarks Phase E added are scientifically invalid (`merge_time_per_round` polluted with join-window time). The same `tokio::select! { ...accept... else => reduce_n(...) }` trap was independently discovered in two phases (B QA-001 and C MF-002). The same `IdRange { start: 0, end: 100_000 }` placeholder was committed unchanged by both Phase B (QA-004) and Phase D (MF-003).

## Tally

| Phase | Stage 4 verdict | Review (MF/SF) | QA (CRIT/HIGH/MED/LOW) | Total CRIT+HIGH |
|-------|------------------|----------------|-------------------------|-----------------|
| **B** (Foundations, TASK-0415..0426) | ACCEPT_WITH_FIXES | 4 / 6 | 4 / 5 / 4 / 3 | **9** |
| **C** (Joining, TASK-0430..0435) | **REJECT_WITH_FIXES** | 4 (CRIT) + 3 (HIGH) + 5 (MED) / 4 (LOW) | 3 / 6 / 7 / 4 | **16** |
| **D** (Departure, TASK-0438..0443) | **REJECT** | 3 (CRIT) + 3 (HIGH) + 3 (MED) / 6 | 5 / 6 / 5 / 3 | **17** |
| **E** (Observability, TASK-0450..0452) | ACCEPT_WITH_FIXES | 6 (MED) / 4 (LOW) | 2 / 3 / 5 / 4 | **5** |
| **TOTALS** | 2× ACCEPT_WITH_FIXES + 2× REJECT | **17 MF + 20 SF (review)** | **14 CRIT + 20 HIGH + 21 MED + 14 LOW (QA)** | **47** |

## Highest-impact findings (selected, full details in per-phase artifacts)

| ID | Phase | Severity | One-liner |
|----|-------|----------|-----------|
| QA-001 (B) | B | CRITICAL | `tokio::select! { ...accept... else => reduce_n(...) }` else-arm is unreachable while `accept()` is unguarded → `reduce_solo_batch` is dead code, SoloReducing hangs forever |
| QA-002 (B) | B | CRITICAL | `recv_frame` in join-window drain has NO timeout → one slow client stalls grid past `join_window_max` |
| QA-003 (B) | B | CRITICAL | `transport.accept()` missing from collect-phase select → mid-round connections wait until next join window |
| QA-004 (B) | B | CRITICAL | Hardcoded `IdRange { start: 0, end: 100_000 }` placeholder in departure recovery (also QA-003 D and MF-003 D) — surviving workers' AgentIds collide with reclaimed |
| MF-001 (C) | C | CRITICAL | `WaitingForResults`/Partitioning/Dispatching/Merging do NOT call `transport.accept()` concurrently — TCP arrivals dropped (same trap pattern as QA-003 B) |
| MF-002 (C) | C | CRITICAL | `SoloReducing` `tokio::select!` uses `else =>` for `reduce_n` → reduction NEVER runs (same trap as QA-001 B) |
| MF-003 (C) | C | CRITICAL | `WorkerIdSpaceExhausted` aborts coordinator with `Err(...)` instead of `JoinNack` |
| MF-004 (C) | C | CRITICAL | `WorkerJoined` event silently absorbed by wildcard FSM arm (recurrence of Phase A QA-001 pattern) |
| QA-001 (C) | C | CRITICAL | Off-by-one: joiner told `next_round_number = N+2`, coordinator does `N+1` |
| QA-002 (C) | C | CRITICAL | `worker_streams: Vec` uses position as identity → collides post-departure |
| QA-003 (C) | C | CRITICAL | Phase D stub-`Err` silently drops `pending_connections_queue` (D6 violation) |
| MF-001 (D) | D | CRITICAL | Successful reclaim ends with unconditional `return Err(ProtocolError::Fatal("…TASK-0443 follow-up"))` — SPEC-20 R18 violated |
| MF-002 (D) | D | CRITICAL | R26a hybrid branch logs "falling back to SoloReducing" then `return Err(...)` — both arms abort |
| MF-003 (D) | D | CRITICAL | All departed workers mapped to same `IdRange` → ID-range collisions |
| QA-001 (D) | D | CRITICAL | `RetainedStateRegistry` has no on-disk persistence; debug_assert masks release-mode silent state loss |
| QA-002 (D) | D | CRITICAL | `D >= k_eff` hybrid branch is algebraically unreachable code |
| QA-003 (D) | D | CRITICAL | `IdRange{0,100_000}` collides with **surviving** partitions, not just other reclaimed (concrete witness redex sequence in QA report) |
| QA-005 (D) | D | CRITICAL | `departing_worker_ids: Vec` has no dedup → double-detection corrupts `materialize_reclaimed_partitions` |
| QA-001 (E) | E | CRITICAL | `merge_time_per_round` polluted with join-window wall-clock → SPEC-09 break-even analysis scientifically invalid |
| QA-002 (E) | E | CRITICAL | D5 `debug_assert!` panics on mid-session joiner because seeding is round-0-only |

Plus brand-new findings not on any reviewer's radar:
- **QA-010 (D)** — `release_worker` is **never called** by the coordinator → retained state grows unboundedly across rounds.
- **QA-008 (D)** — 4 GiB `RetainedLastAcked::DeltaLight { placeholder: String }` DoS surface on the wire.
- **QA-007 (B)** — Hidden `JoinAck.next_round_number` off-by-one (cousin of QA-001 C).
- **QA-006 (D)** — Concrete reproduction sequence for `reconstruct(border_graph, evolved_survivors, round_0_reclaimed)` invariant violation.

## Pattern observations

1. **The `tokio::select! else =>` trap appears in two distinct phases** (B Wave 4 and C Wave 3). Two different waves, same anti-pattern — suggests the LLM has a systematic blind spot for `tokio::select!` semantics.
2. **`Err(...)` after a "succeeded" log** appears in three places (B, C MF-003, D MF-001/MF-002). The LLM seems to reach for `return Err(...)` as a TODO-marker, treating "this works but stream-management isn't done" as a fatal error.
3. **Position-as-identity in `Vec<TransportStream>`** (B + C QA-002) — the LLM did not internalize the implication of departure on a position-indexed worker registry.
4. **Hardcoded magic placeholders** committed unchanged (B QA-004 + D MF-003 + D QA-003) — `IdRange { 0, 100_000 }` is the same constant in two phases, neither flagged as TODO.
5. **Missing tests across structural waves** (B SF, C SF-001, D MF-006). The LLM frequently committed waves with zero new test coverage despite task contracts explicitly specifying test LoC budgets.

## Test-coverage gap quantified

- TEST-SPEC-04{15..52} prescribes ~700 LoC of new tests across the 27 tasks. Actual delta in `relativist-core/tests/` post-Phase-A is much smaller; many EG-* scenarios from the test specs are unverifiable by current code.
- Reviewer Phase C SF-001: zero new tests in 3 structural waves.
- Reviewer Phase D MF-006: zero unit tests in either new module (`retained.rs`, `departure_recovery.rs`); zero integration tests for the 19 EG-* test specs.
- Reviewer Phase E SF-003: no test asserts any of the 7 new `GridMetrics` fields.

## Stage 6 path forward

The plan (`C:\Users\Filipe\.claude\plans\transient-exploring-popcorn.md`) calls for sequential per-phase developer-agent refactor with cargo gates after each commit. Order chosen:

1. **Phase E first** (smallest scope, ~14 fixes, mostly mechanical) — refactor in flight as of 2026-04-27.
2. **Phase B next** (foundational; cascades into C, D).
3. **Phase C** (fixes intersect heavily with Phase B fixes).
4. **Phase D** (REJECTed — may need partial revert + rework rather than patch).

Each phase is one new `refactor(elastic-grid): apply Stage 4+5 findings — Phase X` commit. The original 12 commits are preserved (per `WORKFLOWS.md` §3 git rules — never amend, always new commit).

## Verdict

The user's framing question — *"will valeu a pena?"* (was outsourcing Stage 3 DEV worth it?) — is answered by the audit:

- **Wall-clock saved at Stage 3:** ~6-12h of Claude DEV work (estimate based on Phase A's pace).
- **Wall-clock cost at Stage 6:** estimated 5-7 days for Phase D rework alone (per Phase D reviewer); 2-4 days for Phases B+C+E refactor combined. Total: ~8-11 days.
- **Net:** the alternative-LLM DEV pass produced compilable, test-green Rust that hides 14 CRITICAL bugs. The token economy gained at Stage 3 is consumed several times over by the downstream review + QA + refactor cycle. **The experiment confirmed the value of Claude's Stage 3 discipline; outsourcing structural Rust DEV was not net-positive on this bundle.**

The audit artifacts themselves are valuable. They give a precise lesson catalog of where the alternative LLM falls short, which can become a rubric for future delegation experiments (e.g., "delegate ONLY mechanical refactor passes, not new structural code").
