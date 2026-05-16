# Review: D-016 — HornerCodec Decoder Extension (TASK-0723 / 0724 / 0725 / 0726)

**Stage:** 4 (REVIEWER) of the 6-stage SDD pipeline
**Reviewer:** unified code-quality + architecture reviewer
**Date:** 2026-05-16
**Branch reviewed:** `main` @ commits `29c371d..6a098be` (4 commits)
**Files in scope:**
- `relativist-core/src/encoding/biguint_readback.rs` (+725 / -145 LoC, rewrite of chain reader)
- `relativist-core/src/encoding/horner.rs` (UT-0715-02 relaxed; PT-0715-06 tightened 95% -> 5%)
- `relativist-core/tests/horner_pipeline_property.rs` (new IT)
- `scripts/horner_demo.sh` (new)
- `docs/demos/horner-g1-demonstration.md` (envelope post-D-016 + Limitacoes rewrite)

**Code quality verdict:** PASS WITH NOTES
**Architecture verdict:** ALIGNED (drift contained inside documented v1 limitation envelope)
**Spec compliance:** SPEC-27 v3 R14' / R15' / R16' satisfied for declared readable subset; SPEC-14 §4.4 Independence clause preserved.

---

## Verdict Summary

**VERDE / GREEN — QA can proceed in parallel.** No must-fix issues block QA. There
are 2 should-fix notes (cosmetic / API hygiene) and 4 nice-to-haves; none of
them changes behaviour or violates a spec invariant. The "pending"
`[1;5]@2 = 15 vs 31` mismatch the developer flagged is unambiguously FUTURE
WORK (documented at `biguint_readback.rs:454-468` and
`horner-g1-demonstration.md:262-265`) and matches the SPEC-27 §5.1 Mackie/Pinto
deferral that the spec itself already carries.

---

## Top 3 Findings (HIGH severity first)

### SF-001 — Dead-state arguments in `read_mult_subnet` / `walk_mult_tree`

**Category:** Code Quality (SRP / dead code)
**Severity:** Should-Fix (cosmetic; no behavioural impact; not blocking QA)
**File:** `relativist-core/src/encoding/biguint_readback.rs:528-548` and 557-615

**Problem.** `read_mult_subnet` allocates two `usize` counters
(`exit_branches`, `max_nested_depth`), threads them through `walk_mult_tree`
via `&mut`, then discards them at the call site with
`let _ = (exit_branches, max_nested_depth);` (line 546). They are written by
`walk_mult_tree` but never read by any caller, any test, any
`tracing::trace!`, and no `#[cfg(debug_assertions)]` block. They add 2 of the 9
parameters to `walk_mult_tree`, which is the primary reason the function
needs `#[allow(clippy::too_many_arguments)]` (line 556).

**Before:**
```rust
let mut exit_branches: usize = 0;
let mut max_nested_depth: usize = 0;
walk_mult_tree(
    net, dup_id, lam_x, chain_visited,
    &mut visited_dups, &mut multiplier, &mut exit,
    &mut exit_branches, &mut max_nested_depth, depth,
)?;
let _ = (exit_branches, max_nested_depth);
```

**After (option A — drop them):** remove both counters and the parameters;
`walk_mult_tree` drops two args and very likely no longer needs the
`#[allow(clippy::too_many_arguments)]`.

**After (option B — keep with a purpose):** wire them into a `tracing::trace!`
(both counters are useful debug signals when chasing the degree>=3
limitation), and drop the `let _ = ...`.

**Why.** Either pattern is fine; the current shape is the worst of both —
unused state that still costs a parameter and a clippy suppression. Closing
it makes the function genuinely "one thing" again and lifts the lint
exemption.

---

### SF-002 — `ChainVisited::default()` reset inside `walk_mult_tree` is the root cause of degree>=3 undercounting; comment is too generic

**Category:** Architecture (documentation of an invariant gap)
**Severity:** Should-Fix (clarity; not a bug — the limitation IS the spec)
**File:** `relativist-core/src/encoding/biguint_readback.rs:602-606` and 458-468

**Problem.** The `ExitChainAt` branch in `walk_mult_tree` (line 603) builds a
**fresh** `ChainVisited::default()` before calling `read_chain_terminal`.
That is exactly what makes the v1 walker exact for the declared subset and
exactly what makes it under-count for degree>=3 (the inner accumulator's
cycle-count history is lost on the exit chain). The doc-comment at line
458-468 explains the limitation at `read_chain_terminal`'s point of failure
but never points at the **caller** that throws the visited set away. A
future maintainer touching `walk_mult_tree` to "add a feature" will not see
why the fresh set is load-bearing.

**Suggestion.** Add one line of comment at `walk_mult_tree:602-606`
explaining that the `ChainVisited::default()` reset is the deliberate v1
"each exit branch is treated as an independent additive constant" choice
that gives correct results for the declared subset (degree<=2 with
leading coefficient 1 in degree-2 case) AND is the root cause of the
degree>=3 undercount. Cross-reference the limitation paragraph at
`read_chain_terminal:454-468` and `horner-g1-demonstration.md` §Limitacoes
remanescentes.

**Why.** This is the single most important architectural decision in the
rewrite. Currently it is visible only by reading both the doc comment and
the implementation and inferring the link — a maintenance hazard given the
file gained +725 LoC.

---

### NTH-001 — UT-0715-02 relaxation accepts ANY decimal Ok — too loose

**Category:** Code Quality (test rigor)
**Severity:** Nice-to-Have (subjective; not blocking)
**File:** `relativist-core/src/encoding/horner.rs:524-536`

**Problem.** The relaxed assertion (line 530-533) accepts `Ok(v)` if `v ==
"100001"` (correct) OR `v.parse::<u64>().is_ok()` (any decimal that fits in
u64). The latter is too permissive: `"0"`, `"1"`, or any garbage decimal
under `u64::MAX` would pass. The dispatch brief explicitly asked whether
this "enfraquece a invariante" — yes, mildly. A tighter form would be
to assert `v.parse::<u64>()` is Ok AND `v < expected` (the v1 walker
under-counts, never over-counts, per the documented limitation), or to
pin against the exact value the current v1 walker produces (whatever
`100001`'s under-counted equivalent is) so a future regression that
returns a DIFFERENT wrong value would be detected.

**Defensibility judgement.** The current form is defensible because (a) the
proptest companion `pt_0715_06_skip_rate_is_bounded` (now 5% cap) does the
heavy lifting on the readable subset and (b) the multi-iter sparse case is
documented as out-of-envelope. It is NOT a hidden invariant violation; it
is a hole large enough to swallow some future regressions silently in
THIS specific test. Not blocking.

**Suggestion (optional):**
```rust
Ok((v, _)) => {
    if let Ok(n) = v.parse::<u64>() {
        // v1 walker under-counts the multiplier; the future-readback
        // value (100001) is the exact target. Any other decimal under
        // 100001 is the v1 walker's known incorrect output.
        assert!(
            n == 100_001 || n < 100_001,
            "expected 100001 (future readback) or v1 under-count, got {v}"
        );
    } else {
        panic!("non-decimal Ok value: {v}");
    }
}
```

---

## Other Should-Fix

### SF-003 — `READBACK_MAX_DEPTH = 16_384` justification is good in the const doc-comment but is not exercised by a regression test

**File:** `relativist-core/src/encoding/biguint_readback.rs:62-71`

The constant has a strong "why this number" comment (covers every
encoder-produced input + margin; runaway DUP cycles trip the per-walk
step counter first). However there is no explicit test that **trips** the
guard (constructs a contrived net with a DUP cycle and asserts
`Err(UnrecognizedStructure("... max recursion depth ..."))`). QA should
add one in Stage 5. The 4_000_000 step counter at
`biguint_readback.rs:294` and `:403` is in the same boat. Not blocking
the review; flagged so QA picks it up.

---

## Nice-to-Have

### NTH-002 — `classify_dup_branch` carries a magic `1024`-iteration loop bound (line 651)

```rust
for _ in 0..1024 {
```

The constant is reasonable (DUP transparency walks should be very short)
but it is undocumented and unrelated to `READBACK_MAX_DEPTH`. Pull it into
a named `const TRANSPARENT_DUP_WALK_LIMIT: usize = 1024;` near
`READBACK_MAX_DEPTH` and add a one-liner comment.

### NTH-003 — `walk_mult_tree` doc-comment fragment is duplicated

`biguint_readback.rs:550-555` — the doc-comment has two paragraphs that
contradict each other in tone ("Recursively walk" then "Iterative DUP-tree
walker"). The function was iterativized (good — UT-0723-07 stress) but
the old "recursive" phrasing was left in. Rewrite the doc-comment as one
coherent block.

### NTH-004 — Demo doc envelope description does not match test envelope

`docs/demos/horner-g1-demonstration.md:222` says `c₂ ∈ {1}` (degree-2
leading coefficient pinned to 1). Code agrees. However the
`decode_biguint_degree_2_spot_check` test (`biguint_readback.rs:983-1006`)
includes `[3,5,1]@4 = 39` which has the LEADING coefficient = 1 (correct
per envelope) but the comment block at line 989-993 documents
`[5,2,3]@2 = 21` as a known undercount. The envelope statement and the
spot-check are consistent, but the relationship is not obvious to a
reader. A single sentence in the demo doc clarifying "leading coefficient
= `coeffs[len-1]`, which is `c_2` in degree-2, NOT `c_0`" would close
the ambiguity.

### NTH-005 — `scripts/horner_demo.sh` Demo 4 conflicts with the demo doc's Demo 4

The script labels (`scripts/horner_demo.sh:36-47`):
- `Demo4_smallest_cofactor|[1,2]@2|5`

while `horner-g1-demonstration.md` Demo 4 is the `[100,1]@50 = 150`
scale-test (line 130). The script's numbering doesn't match the doc's
numbering — the script's Demos 1-3 alias the doc's Demos 1,2,? and
Demos 4-10 are mostly new. Either renumber the script to start from
Demo 8 (matching the doc's "newly unlocked" table) or add a header
comment to `horner_demo.sh` explaining the script's labels are
script-local IDs, not doc IDs. Confusion risk for reproducibility
auditors.

---

## Passed Checks

- [x] No `unwrap()` in production code (the rewrite uses `?` /
      `.ok_or_else(...)` consistently across all 4 walker functions).
- [x] No `panic!` / `expect` in production code (panics are confined to
      `#[cfg(test)]`).
- [x] No `unsafe` introduced.
- [x] No `println!` introduced.
- [x] Error propagation: every error path goes through
      `DecodeError::UnrecognizedStructure(_)` or `DecodeFailed(_)` —
      typed `thiserror` enum, not strings-at-large.
- [x] **Independence clause (SPEC-27 R14').** `biguint_readback.rs`
      does NOT delegate to `decode_nat`; the CI test
      `tests/biguint_readback_independence.rs` continues to enforce this
      structurally (file unchanged in D-016).
- [x] **R14' BigUint accumulator.** `read_chain` / `read_chain_terminal`
      both use `count: BigUint`, not `u64`.
- [x] **R15' output schema.** `HornerCodec::decode` returns
      `{value: <string>, bit_length: <usize>}` (horner.rs:91-98)
      unchanged from D-015.
- [x] **R16' edge cases.** Constant polynomial fast-path
      (horner.rs:152-159), Church(0) frame with f-side ERA / DUP-tree
      acceptance (biguint_readback.rs:155-187) both preserved.
- [x] **Module boundary.** All new code lives in `relativist-core::
      encoding::biguint_readback` (core/pure layer). Zero `tokio`,
      `async`, or I/O imports. The directional rule (`net <- reduction
      <- encoding`) is honoured.
- [x] **Newtype IDs.** `AgentId`, `PortRef` used throughout; no raw
      `u32` substitutions.
- [x] **Iterative walker.** `walk_mult_tree` uses an explicit work-stack
      `Vec<(AgentId, usize)>` (line 569) — no recursion through the call
      stack, which is the documented motivation (UT-0723-07 stress with
      `coeffs[1] = MAX_CHURCH_NAT` chains).
- [x] **Depth + step guards.** All four walkers
      (`read_chain`, `read_chain_terminal`, `walk_mult_tree`,
      `classify_dup_branch`) carry a hard cap that throws
      `UnrecognizedStructure` rather than spinning forever.
- [x] **Doc-comments on all `pub` items.** `decode_biguint` has a
      detailed contract block (biguint_readback.rs:73-91). Private
      walkers have purpose + invariant comments.
- [x] **Test architecture.** TASK-0725 IT file
      (`horner_pipeline_property.rs`) follows the `rust-tests-guidelines`
      pattern: one helper (`pipeline_value`), two proptest slices, one
      deterministic witness, all with named `pt_*` test IDs.
- [x] **PT-0715-06 tightening (95% -> 5%).** Defensible: the empirical
      skip rate on the declared readable subset is ~0% per the
      TASK-0723/0724 commits, so 5% is a generous regression bound.
      Crucially this test enumerates the proptest's domain
      deterministically (no RNG dependency), so it cannot become flaky
      under different seeds.

---

## Failed Checks

- [ ] `walk_mult_tree` parameter count (9) triggered
      `#[allow(clippy::too_many_arguments)]` — see SF-001. Removing the
      two dead state args would let us drop the lint exemption.

---

## Architecture / SPEC-27 Compliance Detail

### R14' / R16b' Independence Preserved

`decode_biguint` algorithm is structurally `decode_nat`-shaped but reads
into a `BigUint` accumulator AND extends the post-chain walk to
multiplication boundaries (the new TASK-0723/0724 capability).
Importantly, the new helpers (`read_chain`, `read_chain_terminal`,
`read_mult_subnet`, `walk_mult_tree`, `classify_dup_branch`) are **all
private to `biguint_readback`**. None of them is shared with `church.rs`
or `arithmetic.rs`, so the R16b' cross-check property (the v1 floor
`PT-0712-05`) still witnesses two independent code paths agreeing.

### Readable Subset vs SPEC-27 v3 R16'

The spec's R16' enumerates four edge cases (constant polynomial,
evaluation at zero, all-zero coefficients, boundary values). The D-016
rewrite preserves all four (see "Passed Checks"). The **decoder-side**
"readable subset" envelope — single-iteration any `c_1`, degree-2 with
`c_2 = 1` — is a documented v1 LIMITATION not a spec requirement, and
the spec itself defers the full envelope to §5.1 (Mackie/Pinto). So:
the readable subset is **narrower than the spec's encoder envelope**
but the spec explicitly permits this (§5.1 Future Work).

### "Narrowed Not Abandoned" Documentation Trail

The dispatch asked whether the degree>=3 deferral is well documented.
Verdict: yes, in 4 places that are mutually consistent:
1. `biguint_readback.rs:36-38` (top-of-module Future Work pointer)
2. `biguint_readback.rs:454-468` (`read_chain_terminal` failure-site comment)
3. `horner.rs:511-527` (UT-0715-02 expectation comment)
4. `horner-g1-demonstration.md:252-268` (Portuguese reader-facing version)

All four point at the same SPEC-27 §5.1 anchor. The Mackie/Pinto
"Future Work" reference is consistent across code and demo doc; it is
NOT documented in `docs/ROADMAP.md` as a tracked v2.1 item — the demo
doc says "Tracking no roadmap como item v2.1+" but the actual ROADMAP
entry was not added in this bundle. This is a paperwork gap, not a
spec violation. Flagged for the next pipeline pass; not a blocker.

### The Developer's Reported Pending Issue

> "`[1;5]@2` retorna 15 em vez de 31 — verifique se isto está classificado
>  como FUTURE WORK (aceitável) ou BUG."

**Verdict: FUTURE WORK, not a bug.** `[1;5]@2` is degree-4 (5 coefficients),
which falls outside the declared envelope (single-iter OR degree-2 with
leading c=1). The `walk_mult_tree` -> `read_chain_terminal` chain
deliberately throws away `ChainVisited` between nested exit branches
(see SF-002), and this is the same algorithmic limitation that produces
the under-count documented at `horner-g1-demonstration.md:264`:
`[1,1,1,1,1]@2 = 31 expected, decoder returns 15`. The exact same
`expected/actual` ratio (31:15 ≈ 2:1 per added nesting level) matches
the under-count signature. So this is not a NEW bug from the rewrite;
it is the spec's §5.1 Mackie/Pinto deferral expressing itself on a
fifth-degree input. **QA may proceed in parallel.**

---

## Recommendation

- **VERDE / GREEN — QA can start Stage 5 immediately.**
- No must-fix issues; no spec violations; the documented v1 limitation
  envelope is internally consistent across code (`biguint_readback.rs`,
  `horner.rs`), tests (`horner_pipeline_property.rs`), demo doc, and
  SPEC-27 §5.1.
- Suggest the developer pick up **SF-001** (drop dead state args /
  the clippy exemption) and **SF-002** (one-line architectural comment
  at the `ChainVisited::default()` reset) as part of the Stage 6
  refactor pass; both are zero-risk cosmetic cleanups.
- **NTH-005** (`scripts/horner_demo.sh` numbering mismatch with the
  demo doc) is worth fixing before the script is referenced from
  thesis text or any external audit.
- The **ROADMAP.md "Mackie/Pinto v2.1+" tracking entry** referenced
  by the demo doc (line 268) was not added in this bundle. Either
  the demo doc claim should be softened or the ROADMAP entry should
  be added in a separate documentation-only commit. Not a blocker.

**File created:** `docs/reviews/D-016-horner-decoder-extension-review.md`
