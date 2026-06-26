# TASK-0706 — D-014-DOCS: `docs/benchmarks/campaigns/stress-curve.md` methodology page

**Phase:** D-014 (Stress Curve Campaign) — Stage 3 DEV scope
**Bundle:** D-014 — Stress Curve Campaign
**Status:** TODO
**Priority:** P1 (must ship before campaign run; reviewers consult this for reproducibility).
**Spec:** none.
**Depends on:** TASK-0700, TASK-0701, TASK-0702, TASK-0703, TASK-0704, TASK-0705 (all components named in the doc must already exist when the page lands).
**Estimated complexity:** S–M (~250 lines markdown, no production code).

---

## Context

The campaign needs a self-contained methodology page that the user (and any future reviewer) can read to:
1. Understand WHY the campaign exists (research question, scope).
2. Re-run it from a clean checkout (exact commands).
3. Audit the locked output (column meanings, sentinel rows, MANIFEST contents).
4. Distinguish it from the existing `v1-local-baseline.md` and `v1-stress.md` campaign pages.

This page goes alongside the existing `docs/benchmarks/campaigns/{v1-local-baseline,v1-stress,church-sum-of-squares}.md` files and follows their structural template.

## Files in scope

| File | Change |
|------|--------|
| `docs/benchmarks/campaigns/stress-curve.md` | **CREATE.** ~250 lines markdown. |
| `docs/benchmarks/README.md` | **MODIFY.** Add a row for the new campaign in the campaigns index table. ~2 lines. |
| `docs/INDEX.md` | **MODIFY.** Add an entry under "Benchmark Results > Stress Curve (v2)" pointing to the new page (and post-campaign, to the locked dir). ~2 lines. |

## Files explicitly OUT of scope

- `docs/ROADMAP.md` §2.16 — updated by TASK-0708 after the campaign actually runs and characterizes the wall.
- `CHANGELOG.md` — updated by TASK-0708.
- `docs/next-steps.md` — updated by TASK-0708.
- `progress.md` (project root) — author updates per CLAUDE.md rule #2 once the bundle closes.
- `OBJETIVO_TCC.md` — never edited per CLAUDE.md rule #8.
- The TCC article — REDATOR territory.

## Required content (section structure)

```markdown
# Stress Curve Campaign — v2-development

## 1. Research question
<verbatim from design doc §1>

## 2. Scope
<3 workloads, 2 envs, 4 W, 11 N, 5 reps; 7-8h overnight>

## 3. Components
| Component | Path | Owner TASK |
| Memory probe | `relativist-core/src/bench/memory_probe.rs` | TASK-0700 |
| Stop rule | `relativist-core/src/bench/stop_rule.rs` | TASK-0701 |
| ...

## 4. CSV schema
<full column list with types and meanings; reference TASK-0703>

## 5. How to reproduce
### 5.1 Pre-conditions
<full list from TASK-0704 acceptance criteria>

### 5.2 Smoke run
\`\`\`bash
scripts/stress_curve.sh --smoke
\`\`\`
<expected outputs>

### 5.3 Full overnight run
\`\`\`bash
scripts/stress_curve.sh
\`\`\`
<expected wall ~7-8h, MANIFEST contents>

## 6. Lock procedure
<sha256 generation, MANIFEST template, no auto-commit>

## 7. Failure modes
<RAM exhausted, OOM, smoke-failed-don't-run-overnight, --resume>

## 8. Sanity checks (post-aggregation)
<verbatim from design doc §8>

## 9. Limitations
<verbatim from design doc §7 limitations 1-7>

## 10. Cross-references
- Design doc: `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md`
- Locked output (post-run): `results/locked/v2_stress_curve_<YYYY-MM-DD>/`
- ROADMAP item: §2.16 streaming reduction (this campaign characterizes the wall it leaves)
- Predecessor campaigns: `docs/benchmarks/campaigns/{v1-local-baseline,v1-stress}.md`
```

## Acceptance criteria

1. The page is ≥ 200 lines markdown, with exactly the section structure above.
2. All 7 components from design doc §4.2 are listed with their TASK numbers (TASK-0700..0706).
3. All 4 new CSV columns from TASK-0703 are documented with type and source.
4. The "How to reproduce" section's commands match TASK-0704's `--smoke` and full-run invocations exactly (no aspirational flags that don't exist in the script).
5. `docs/benchmarks/README.md` index gains a row pointing to the new page.
6. `docs/INDEX.md` gains a "Benchmark Results > Stress Curve (v2)" entry.
7. `mdformat` or `markdownlint` (whatever the project uses) reports clean.
8. Cross-references to TASK files use the form `TASK-NNNN` consistently.
9. The page contains zero placeholder text — every TBD must be filled or removed before this task is marked DONE.
10. No code changes; `cargo test` floors unchanged from TASK-0705's cumulative numbers (≥ 1813 default, etc.).

## Test floor delta

**0** (docs-only). All cumulative floors carried over from TASK-0705.

## Implementation hints

1. Read `docs/benchmarks/campaigns/v1-local-baseline.md` first as the structural template; this page mirrors its sections.
2. Don't repeat the design doc verbatim — link to it. The methodology page is the "user-facing" cut; the design doc is the "decision-record" cut.
3. The "Failure modes" section MUST mention `--resume` semantics from TASK-0704 (interrupted runs can resume, malformed CSV refuses).
4. For section 5.3, list the exact MANIFEST.md fields the campaign locks (git SHA, rustc version, /proc/meminfo, /proc/cpuinfo, total reps, total wall, median CV, stop_reason histogram).
5. Section 9 (limitations) reuses design doc §7's wording but tightens for a reviewer audience: "the campaign characterizes the wall, it does not remove it" — make this the section's lead sentence.

## Estimated LoC / lines

- Markdown: ~250 lines.
- Code: 0.
- Total: ~250 lines.

## Cross-references

- Design doc: `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md` (full).
- Sibling pages: `docs/benchmarks/campaigns/{v1-local-baseline,v1-stress,church-sum-of-squares}.md`.
- Consumed by: TASK-0708 (campaign run validates the page's commands work end-to-end).
