# TASK-0726 — D-016 doc cleanup + `scripts/horner_demo.sh` placeholder

**Spec:** SPEC-27 v3 (`specs/SPEC-27-encoder-decoder-api.md`) — public-facing narrative
**Requirements:** N/A (doc + tooling housekeeping only)
**Priority:** P1 (cosmetic but important: keeps the public demo doc honest after the fixes ship)
**Status:** TODO
**Depends on:** TASK-0723 + TASK-0724 + TASK-0725 (the fixes MUST land + be verified before the doc is rewritten)
**Blocked by:** TASK-0725
**Estimated complexity:** S (~40 LoC doc delta + ~30 LoC bash skeleton; 0 LoC production)
**Bundle:** D-016 — HornerCodec decoder extension

---

## Context

After TASK-0723, TASK-0724, and TASK-0725 land, the "Limitações
conhecidas" section in `docs/demos/horner-g1-demonstration.md` (lines
214-239 of HEAD) is OUTDATED — the 3 failing cases it documents
(`[3,5]@4`, `[1,1,1]@2`, `[1,0,1]@3`) now decode correctly. The
section needs to be either:

- **Path A (preferred):** rewritten as a "Working envelope" section
  describing the full readable subset (degree 1..=N, coefficients in
  `0..=MAX_CHURCH_NAT`), with a note that the Mackie/Pinto-style
  optimization in SPEC-27 §5.1 Future Work remains a v2.1+ scope item
  for performance (NOT correctness) reasons.
- **Path B:** struck entirely, with the limitation narrative replaced by
  the new working envelope inline in the introduction.

Additionally, the **next bundle** will want a `scripts/horner_demo.sh`
that reproduces all 7 demos (plus the 3 newly-fixed cases) end-to-end
both in-process and in Docker — to be lifted to a CI smoke gate. This
TASK creates the **placeholder script** and the BACKLOG entry for the
follow-up implementation TASK; it does NOT implement the Docker side.

## Acceptance Criteria

- [ ] `docs/demos/horner-g1-demonstration.md` "Limitações conhecidas" section is either rewritten (Path A) or removed (Path B). The new content MUST:
  - Cite TASK-0723 + TASK-0724 as the closure references for the previous 3 limitations.
  - Add 3 new "Demo 8 / 9 / 10" entries running the previously-failing inputs (`[3,5]@4 → 23`, `[1,1,1]@2 → 7`, `[1,0,1]@3 → 10`), formatted identically to existing Demo 2-6 entries (CLI command + expected output block + ✅ confirmation).
  - Update the "Síntese" table to reference G1 evidence at degree ≥ 2 (point at PT-0724 and UT-0725-E).
- [ ] `docs/demos/horner-g1-demonstration.md` "Reprodutibilidade" section is updated to:
  - Replace `git rev-parse HEAD` reference `d35a784` and tag `v0.20.0` with the post-D-016 closing commit + tag (e.g., `v0.21.0` if D-016 cuts a tag — coordinate with sdd-pipeline).
  - Mention `scripts/horner_demo.sh` (created by this TASK) as the one-shot reproducer.
- [ ] New file `scripts/horner_demo.sh` exists and:
  - Runs all 10 demos in-process (`target/release/relativist compute --codec horner --input '<JSON>' [--workers N]`) via Bash.
  - Asserts each output's `"value"` field matches the expected via simple grep/jq.
  - Includes a `# TODO(D-017+): Docker arm` comment block delineating where the future Docker invocation will live.
  - Has a `set -euo pipefail` header and a 30-second per-demo timeout via `timeout 30`.
  - Returns exit 0 on full success, non-zero with a summary line on failure.
  - ~30-50 LoC of Bash.
- [ ] BACKLOG.md is updated to add a "deferred" follow-up TASK entry (TASK-0727) for the Docker arm of `horner_demo.sh`. NO TASK file is created in this TASK — just the BACKLOG entry. The actual TASK-0727 file will be written by the next bundle's task-splitter.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `docs/demos/horner-g1-demonstration.md` | modify | Rewrite "Limitações conhecidas" (Path A or B); add Demo 8/9/10 entries; update Síntese table + Reprodutibilidade section. ~40-60 LoC delta. |
| `scripts/horner_demo.sh` | **CREATE.** | Bash one-shot reproducer for all 10 demos with timeout + value assertions. ~30-50 LoC. |
| `docs/backlog/BACKLOG.md` | modify | Add TASK-0727 entry in the "Deferred / next bundle" section: "TASK-0727 — scripts/horner_demo.sh Docker arm (D-017 candidate)". ~3 lines. |

## Key Types / Signatures

N/A — doc + Bash only.

## Test Expectations (for Stage 2 test-generator)

The test-generator may SKIP this TASK entirely (doc + script) OR
optionally add one CI smoke test that invokes `scripts/horner_demo.sh`
in a `#[ignore]`-by-default test fixture:

- **IT-0726-01** (`#[ignore]`) — `tests/horner_demo_script_smoke.rs`: invokes `bash scripts/horner_demo.sh` via `std::process::Command`, asserts exit code 0. Gated `#[ignore]` so it runs on-demand (`cargo test -- --ignored`) — full demo run is ~30-60s.

No required automated tests; manual verification by the operator
running `bash scripts/horner_demo.sh` is sufficient acceptance for this
TASK.

## Dependencies Context

- TASK-0723/0724 fixes MUST be in HEAD (otherwise the new Demo 8/9/10
  examples in the doc would fail).
- `target/release/relativist` binary built (`cargo build --release`).
- Bash + standard POSIX tools (`grep`, `timeout`); jq is OPTIONAL — if
  used, the script MUST `command -v jq` and fall back to grep on the
  raw `--input` JSON output.

## Notes

- The 3 newly-fixed demos in the Limitações section are also covered by
  the TASK-0723/0724 unit tests. The doc is the public-facing surface;
  the unit tests are the regression dam.
- The `scripts/horner_demo.sh` Docker arm is OUT of scope — the TASK-0727
  follow-up (to be split by the next bundle's task-splitter) will use the
  existing Docker infrastructure pattern from `scripts/stress_curve.sh`
  Phase 2 (which has Docker arm landed via commit `c77d7fc`). The TASK-0727
  scope is a port of that pattern to the Horner demo workflow.
- DO NOT include any LaTeX or BibTeX edits in this TASK — the artigo's
  citations of the demo doc point at the file path, not at section
  numbers, so the rewrite is invisible from `artigo/tcc_pt_br.tex`.
- This TASK closes D-016. After it lands + the sdd-pipeline moves the
  bundle's TASK files to `archive/`, the BACKLOG.md "Active" section
  empties (unless TASK-0727 or a new bundle has been queued by then).

## Sequencing within D-016

LAST. Pure doc + script housekeeping. Can be done in the same dispatch
as the closing commit of TASK-0725 if desired.
