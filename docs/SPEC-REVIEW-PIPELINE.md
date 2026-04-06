# Spec Review Pipeline

**Last updated:** 2026-04-05
**Status:** Ready — agents created, awaiting first review cycle

---

## Overview

Adversarial review process for Relativist specifications, adapted from the TCC-layer DISC debate pipeline. Each spec undergoes a structured 3-stage review before implementation begins for its phase.

**Goal:** Catch consistency issues, incomplete designs, untestable requirements, and invariant violations BEFORE implementation — when fixes are cheap.

---

## Pipeline Stages

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Spec Exists │────>│   Round 1:   │────>│   Round 2:   │────>│    Round 3:  │
│  (Draft v1)  │     │  Spec Critic │     │   Defender   │     │ Task Updater │
└──────────────┘     └──────────────┘     └──────────────┘     └──────────────┘
                           │                     │                     │
                           v                     v                     v
                     SPEC-NN-round1-     Revised SPEC-NN +     Updated tasks +
                     critic.md           SPEC-NN-round2-       SPEC-NN-task-
                                         defender.md           impact.md
                                                                      │
                                                                      v
                                                              ┌──────────────┐
                                                              │    Human     │
                                                              │   Approval   │
                                                              └──────────────┘
```

---

## Stage Details

### Stage 1: Spec Critic (adversarial review)

**Agent:** `spec-critic` (in `.claude/agents/spec-critic.md`)
**Input:** Target spec + all predecessor specs + SPEC-00 (glossary) + SPEC-01 (invariants)
**Output:** `docs/spec-reviews/SPEC-NN-round1-critic.md`

**Review axes (4):**
1. **Consistency** — Does this spec contradict predecessors? Are types/terms used consistently?
2. **Testability** — Can every MUST requirement be verified by a concrete test?
3. **Completeness** — Are all edge cases covered? Is pseudocode implementable without guessing?
4. **Invariant Preservation** — Does any design choice risk violating T1-T7, D1-D6, I1-I4, or G1?

**Severity levels:** CRITICAL (blocks implementation) / HIGH (likely bug) / MEDIUM (improvement) / LOW (nit)

### Stage 2: Defender Response

**Agent:** `especialista-specs` (existing TCC-layer agent)
**Input:** Round 1 critic review + original spec
**Output:** Revised spec in `specs/SPEC-NN-*.md` + `docs/spec-reviews/SPEC-NN-round2-defender.md`

**Per-issue response format:**
- **ACCEPTED** — issue is valid, spec revised
- **PARTIALLY ACCEPTED** — issue is valid but fix differs from suggestion
- **NOT ADDRESSED** — issue rejected with justification

### Stage 3: Task Updater

**Agent:** `task-updater` (in `.claude/agents/task-updater.md`)
**Input:** Revised spec + existing tasks in `docs/backlog/`
**Output:** Updated task files + `docs/spec-reviews/SPEC-NN-task-impact.md`

**Actions:**
- Update tasks whose referenced requirements changed
- Create new tasks for new requirements
- Mark tasks as obsolete if requirements were removed
- Update BACKLOG.md index

### Stage 4: Human Approval

The user reviews the revised spec, defender response, and task impact report. Approves or requests another round.

---

## Review Queue

| Spec | Current Status | Round 1 | Round 2 | Task Update | Final Status |
|------|---------------|---------|---------|-------------|-------------|
| SPEC-13 (System Architecture) | Revised v2 | DONE (NEEDS REVISION) | DONE (12A/3P/1N) | TODO | — |
| SPEC-14 (Arithmetic Encoding) | Revised v2 | DONE (MAJOR REVISION) | DONE (14A/2P/2N) | TODO | — |
| SPEC-10 (Security) | Revised v2 | DONE (NEEDS REVISION) | DONE (10A/4P/0N) | TODO | — |
| SPEC-11 (Observability) | Revised v2 | DONE (NEEDS REVISION) | DONE (13A/3P/0N) | TODO | — |
| SPEC-12 (User I/O) | Revised v2 | DONE (NEEDS REVISION) | DONE (17A/3P/0N) | TODO | — |

### Execution Order (based on dependency chain)

| Batch | Specs | Can Parallelize? | Depends On |
|-------|-------|-----------------|------------|
| **A** | SPEC-13 + SPEC-14 | Yes (parallel) | Only Revised v2 specs (00-09) |
| **B** | SPEC-10 + SPEC-11 | Yes (parallel) | SPEC-13 revised |
| **C** | SPEC-12 | Alone | SPEC-13 + SPEC-14 revised |

---

## Timing

Spec reviews happen **before implementation of their phase**, not all at once:

- **Before Phase 6:** Review SPEC-13 (System Architecture)
- **Before Phase 7:** Review SPEC-10 (Security)
- **Before Phase 8:** Review SPEC-11 (Observability)
- **Before Phase 9:** Review SPEC-12 (User I/O) + SPEC-14 (Arithmetic Encoding)
- **Before Phase 10:** No new review needed (SPEC-09 already at Revised v2)

However, Batches A-C can be front-loaded if time permits.

---

## Output Directory

All review artifacts go to `docs/spec-reviews/`:

```
docs/spec-reviews/
├── SPEC-10-round1-critic.md
├── SPEC-10-round2-defender.md
├── SPEC-10-task-impact.md
├── SPEC-11-round1-critic.md
├── ...
└── SPEC-14-task-impact.md
```

---

## Differences from TCC-layer DISC Pipeline

| Aspect | DISC (TCC layer) | Spec Review (Relativist) |
|--------|-----------------|-------------------------|
| **Scope** | Conceptual gray zones (Z1-Z7) | Technical correctness of specs |
| **Focus** | Multiple perspectives, evidence | Consistency, testability, completeness |
| **Audience** | Thesis committee | Implementation team |
| **Output** | ARG files → thesis narrative | Revised specs + updated tasks → code |
| **Rounds** | 2 debate rounds | 1 critic + 1 defender + 1 task update |
| **Extra stage** | None | Task Updater (re-aligns backlog) |
