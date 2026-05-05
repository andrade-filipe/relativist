# TEST-SPEC-0393: `delta_mode` docstring polish + `GridConfig` doctest (R41p)

**Task:** TASK-0393
**Spec:** SPEC-19 §3.6 — R41 (the `delta_mode` field docstring MUST describe the opt-in semantics; this task polishes the docstring landed in TASK-0389 to match the codebase's documentation standard set by `coordinator_free_rounds`).
**Amendment log ref:** `docs/spec-reviews/SPEC-19-section-3.5-3.6-2.26D-design-choices-2026-04-17.md` — no AMB amendment touches TASK-0393 directly. AMB-D-2's behavioural reading of R42 is referenced by the doctest's R42 sanity check (the doctest verifies `Default::default().delta_mode == false`), but the doctest is operational (compile-tested) rather than spec-amendment-driven.
**Generated:** 2026-04-17
**Baseline before this task:** post TEST-SPEC-0392 — cumulative 975 default lib / 1015 `--features zero-copy`. Doctest count baseline at bundle entry is captured by `cargo test --doc --workspace` separately (not counted in the lib total).
**Cumulative target after this task:** **+0** new `#[test]` fns — 975 default lib / 1015 `--features zero-copy` (UNCHANGED). **+1 or +2** new doctest blocks on `GridConfig` (counted separately by `cargo test --doc`).

---

## Scope note

TASK-0393 ships TWO related deliverables on `relativist-core/src/merge/types.rs`:

1. **Docstring polish** — expand the `delta_mode` field doc-comment (currently the minimal version landed by TASK-0389) to match the depth and structure of `coordinator_free_rounds` (~15 lines covering: defaults, the two modes, IC concept, SPEC-19 cross-reference, sibling field cross-reference). User preference `feedback_ic_code_documentation.md` mandates IC concepts be explicitly explained.
2. **Doctest** — add `/// # Examples` block on `GridConfig` (preferred placement per task spec) demonstrating the opt-in builder pattern with `..GridConfig::default()` spread + an R42 default-polarity sanity check.

**Inert field contract.** As of 2.26-D, `GridConfig.delta_mode = true` has no runtime effect (sub-bundle 2.26-C lands the consumer). The doctest MUST therefore avoid asserting any runtime behaviour for the enabled path — it asserts ONLY field settability, default polarity, and that sibling fields retain their defaults. TASK-0393 §Notes explicitly forbids the doctest from claiming any runtime behaviour for `delta_mode = true`.

**No `#[test]` fns added.** Doctests are compile-tested by `cargo test --doc`; they are NOT `#[test]` units in the lib total. The 975 / 1015 lib counts are unchanged. `cargo test --doc --workspace` count increases by +1 or +2 (depending on whether the developer chooses one combined `# Examples` block or two separate ones — both shapes are acceptable per the task spec; this TEST-SPEC recommends two for clearer Stage 4 review).

**Out of scope for this TEST-SPEC:**
- Field presence + default polarity at the type layer → TEST-SPEC-0389 (lib `#[test]` units).
- CLI flag threading → TEST-SPEC-0390.
- Behavioural smoke regression → TEST-SPEC-0391.
- ROADMAP §3.5 narrative → TEST-SPEC-0392.
- IC concept explanation in `coordinator_free_rounds` docstring (already shipped in TASK-0350).
- `cargo doc` site visual review beyond the `cargo test --doc` compile check.

---

## Test target file paths

- `relativist-core/src/merge/types.rs` — sole target. Two edits:
  1. Expand the `delta_mode` field doc-comment in-place (~15 lines).
  2. Append a `/// # Examples` block (with one or two doctest fences) either on the `GridConfig` struct itself (preferred per task spec) or on the `delta_mode` field. This TEST-SPEC recommends placement on the **struct** so that a `cargo doc` page shows the opt-in pattern at the struct level (the natural entry point for new readers).

NO new test files. NO inline `#[cfg(test)] mod tests` additions. Doctests are inline `///` comments only.

---

## Doctest specifications (in lieu of `#[test]` fns)

This task adds two doctest blocks (RECOMMENDED). The task spec (line 78) accepts a single combined block; this TEST-SPEC argues for two because:

- Each block exercises a distinct shape (`delta_mode = true` opt-in vs `Default` polarity check).
- Two short blocks render more cleanly on `docs.rs` / `cargo doc` than one long block with multiple `let` shadowings.
- A single failure (e.g., a future refactor that breaks `Default`) localises to one of the two blocks rather than a long combined block.

If the developer elects the single-block shape, the test count delta is +1 doctest instead of +2; both shapes satisfy the task acceptance criteria.

### DOCTEST-0393-01: Opt-in builder pattern (`delta_mode: true` via spread literal)

**Target:** `GridConfig` struct doc-comment (preferred) OR `delta_mode` field doc-comment.

**Required content:**

```rust
/// # Examples
///
/// Opt into the delta protocol from a builder pattern:
///
/// ```
/// use relativist_core::merge::GridConfig;
///
/// let cfg = GridConfig {
///     num_workers: 4,
///     delta_mode: true,
///     ..GridConfig::default()
/// };
/// assert!(cfg.delta_mode);
/// assert_eq!(cfg.num_workers, 4);
/// // All other fields retain defaults:
/// assert!(!cfg.strict_bsp);
/// assert!(!cfg.coordinator_free_rounds);
/// ```
```

**Compile-test assertions (run by `cargo test --doc`):**

- `relativist_core::merge::GridConfig` is `pub`-importable from a doctest (i.e., the type is reachable from outside the crate).
- The struct-spread literal `GridConfig { delta_mode: true, ..GridConfig::default() }` compiles — proves `delta_mode` is `pub` AND the field exists AND `Default` produces a valid spread base.
- `cfg.delta_mode == true` after construction.
- `cfg.num_workers == 4` (the explicit override survives the spread).
- `cfg.strict_bsp == false` (the spread populates the default).
- `cfg.coordinator_free_rounds == false` (the spread populates the default — note: this matches the *current* `Default` impl; if R43's "default `true` when delta_mode is `true`" is later wired into the `Default` impl, this assertion will need updating).

**Coverage:** R41 (field is `pub` + struct literal spread idiom works), R42 (siblings retain defaults under spread).

**NOTE on R43.** SPEC-19 §3.6 R43 states `coordinator_free_rounds` MUST default to `true` when `delta_mode` is `true`, and SHOULD default to `false` when `delta_mode` is `false`. As of 2.26-D, the `Default` impl unconditionally sets `coordinator_free_rounds: false` (TASK-0350 baseline; R43 is OUT OF SCOPE for sub-bundle 2.26-D — the bundle index restricts scope to R38-R42). The doctest accordingly asserts `!cfg.coordinator_free_rounds` reflecting the current implementation. When R43's conditional default lands (separate task), the doctest assertion will need to be updated; this is a known cross-bundle consideration documented here for the future maintainer.

---

### DOCTEST-0393-02: Default polarity (R42 sanity check)

**Target:** Same as DOCTEST-0393-01 (preferred: same `# Examples` block, second doctest fence).

**Required content:**

```rust
/// Default is v1 (backwards-compatible, R42):
///
/// ```
/// use relativist_core::merge::GridConfig;
///
/// let cfg = GridConfig::default();
/// assert!(!cfg.delta_mode);
/// ```
```

**Compile-test assertions (run by `cargo test --doc`):**

- `GridConfig::default()` is callable from outside the crate (the `Default` derive is `pub`).
- `cfg.delta_mode == false` — R42 default-polarity confirmation at the doc layer.

**Coverage:** R42 (default polarity, doc-layer sanity check duplicating TEST-SPEC-0389 UT-0389-01 at a different layer — defence-in-depth).

**NOTE.** TEST-SPEC-0389 UT-0389-01 is the lib-test version of this assertion. The doctest version is intentional duplication: it (a) makes the public API contract visible on `docs.rs`, (b) catches a `Default` impl regression at the doc layer separately from the lib layer, (c) demonstrates the contract to new readers in the same place they'd look up the type. Spec-critic may flag this as redundant; the TEST-SPEC defends it as the conventional doctest-as-documentation pattern.

---

## Docstring expansion specification (DOC-0393-03)

This is NOT a doctest — it is a doc-comment polish requirement that runs in the same source edit as the doctest blocks.

**Required content for the expanded `delta_mode` doc-comment** (~15 lines, mirroring `coordinator_free_rounds`):

1. **One-line summary:** "Enable the delta-only BSP protocol (stateful workers)."
2. **Defaults paragraph:** "Defaults to `false` (SPEC-19 R42 — v1 backwards compatibility). When `false`, the grid loop runs the v1 full-partition protocol (SPEC-05 R24-R30a). When `true`, the grid loop dispatches `run_grid_delta` (sub-bundle 2.26-C) with stateful workers that retain partitions across BSP rounds and exchange only border deltas via R31-R37 wire variants."
3. **IC concept paragraph (per `feedback_ic_code_documentation.md`):** Explain why retaining partitions across rounds is safe in the IC model — strong confluence (T4) ensures that the order of reductions does not affect the final normal form, so a worker mutating its partition in place (rather than re-receiving it from the coordinator each round) cannot diverge from the v1 baseline. This is counter-intuitive for programmers from imperative or eager-evaluation backgrounds and MUST be explicitly stated.
4. **SPEC-19 cross-reference:** "See SPEC-19 §3.6 R41-R44 for the configuration semantics and SPEC-19 §3.3 for the stateful worker lifecycle."
5. **Sibling field cross-reference:** "See also `coordinator_free_rounds` (SPEC-19 R44) — the two flags are MAY-independent opt-ins; either may be enabled in isolation."

**Compile-test assertion:** none directly. The expansion is verified by Stage 4 reviewer reading the rendered `cargo doc` output. The doctests below run on the same source file so any malformed doc syntax (e.g., unbalanced backticks, broken intra-doc links) breaks `cargo test --doc` immediately.

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R41 — `delta_mode` field is `pub`-constructible from external crates | DOCTEST-0393-01 (doctest runs from outside the crate) |
| R41 — docstring describes opt-in semantics | DOC-0393-03 (expanded doc-comment) |
| R41 — IC concept (counter-intuitive stateful worker invariant) explained | DOC-0393-03 paragraph 3 |
| R41 — sibling field `coordinator_free_rounds` cross-referenced | DOC-0393-03 paragraph 5 |
| R42 — default polarity is `false` (defence-in-depth at doc layer) | DOCTEST-0393-02 |
| R42 — siblings unchanged by `delta_mode = true` spread | DOCTEST-0393-01 (assertions on `strict_bsp`, `coordinator_free_rounds`) |
| R44 — independent opt-in (the two flags are MAY-independent) | DOC-0393-03 paragraph 5 (mention only — runtime independence is out of scope here) |
| R38/R39/R40 invariant amendments | NOT covered — those are documentation-only narrative in TASK-0392, not docstring material on `GridConfig` |
| R43 — conditional default for `coordinator_free_rounds` | OUT OF SCOPE for 2.26-D; doctest-01 NOTE flags this for the future maintainer |

**Proof scaffolding note (no test layer).** TEST-SPEC-0393 carries no `#[ignore]` stubs and no R38/R39 proof-pending hooks. The doctest assertions are operational (compile + run under `cargo test --doc`); the docstring narrative is informational. R38/R39 proof-pending status is documented in TASK-0392's ROADMAP block, not on the `GridConfig` doc-comment (which would clutter the API surface).

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0393-A | Doctest asserts a runtime claim about `delta_mode = true` (e.g., "this enables the delta loop") | Currently FALSE (2.26-C lands the consumer); doctest would lie about the API. Task spec §Notes forbids this. Stage 4 reviewer reads the doctest body. |
| QA-0393-B | Doctest uses `#[doc(hidden)]` shape or a non-public path | `cargo test --doc` would fail; canary |
| QA-0393-C | Future refactor renames `GridConfig` to `GridConfiguration` | Doctest fails to compile; canary |
| QA-0393-D | Future refactor moves `GridConfig` from `merge::` to a different module | Doctest's `use relativist_core::merge::GridConfig;` fails to compile; canary. Stage 4 should consider whether the doctest path is a re-export (more robust to internal moves). |
| QA-0393-E | Doctest's `..GridConfig::default()` spread breaks because `Default` is removed or feature-gated | `cargo test --doc` fires; canary |
| QA-0393-F | Docstring expansion accidentally drops the `R42` reference | Stage 4 reviewer greps for "R42" in the new doc-comment; MUST be present |
| QA-0393-G | Docstring expansion forgets the IC concept paragraph (per `feedback_ic_code_documentation.md`) | Stage 4 reviewer greps for "confluence" or "stateful worker" or equivalent IC-concept phrasing; MUST be present |
| QA-0393-H | Docstring expansion is < 5 lines (insufficiently polished compared to `coordinator_free_rounds`) | Stage 4 reviewer compares line count against the `coordinator_free_rounds` reference docstring (~15 lines per task spec) |
| QA-0393-I | Doctest body modifies the `Default` shape (e.g., uses a builder method that doesn't exist) | `cargo test --doc` fires; the doctest serves as a contract for the builder pattern |
| QA-0393-J | Doctest combined into a single 30-line block instead of two short blocks | Acceptable per task spec but harder to review/render; this TEST-SPEC's recommendation is two blocks. Stage 4 reviewer may approve either. |
| QA-0393-K | A future R43 land (conditional `coordinator_free_rounds` default) breaks DOCTEST-0393-01's `assert!(!cfg.coordinator_free_rounds)` | Doctest fires when R43 lands; this is the **deliberate cross-bundle signal** the doctest's NOTE flags. The R43 PR MUST update this doctest. |

---

## Acceptance gate

This task adds NO `#[test]` lib units. The lib test count is unchanged.

1. `cargo test --workspace --lib` count: 975 → **975** (UNCHANGED).
2. `cargo test --workspace --lib --features zero-copy` count: 1015 → **1015** (UNCHANGED).
3. `cargo test --doc --workspace` count: baseline + **+1 or +2** doctests (depending on combined-block vs split-block choice).
4. `cargo test --doc --workspace` ALL PASS — the new doctests compile AND their assertions hold at runtime.
5. `cargo build --workspace` clean (default features).
6. `cargo build --workspace --features zero-copy` clean.
7. `cargo clippy --workspace --all-targets -- -D warnings` clean (the doctest body is checked under `--all-targets`; some clippy lints apply to doc code).
8. `cargo fmt --check` clean.
9. `cargo doc --workspace --no-deps` builds without warnings (e.g., no broken intra-doc links to `coordinator_free_rounds` or to SPEC-19 if intra-doc links are used).
10. **Per-paragraph reviewer grep checks on the expanded `delta_mode` doc-comment:**
    - Contains "R42" (default polarity reference).
    - Contains "R41" or "SPEC-19 §3.6" (spec cross-reference).
    - Contains "confluence" OR "stateful worker" OR equivalent IC concept phrasing (per `feedback_ic_code_documentation.md`).
    - Contains "coordinator_free_rounds" (sibling cross-reference).
    - Line count ≥ 10 (rough match to `coordinator_free_rounds` reference; not a hard rule).

---

## Out of scope (deferred to later TEST-SPECs in the bundle or future bundles)

- Lib `#[test]` units for field presence / default polarity → TEST-SPEC-0389.
- CLI flag threading → TEST-SPEC-0390.
- R42 behavioural smoke regression → TEST-SPEC-0391.
- ROADMAP §3.5 narrative → TEST-SPEC-0392.
- R43 conditional default for `coordinator_free_rounds` (when `delta_mode = true`) — separate task in a future bundle; the doctest's NOTE flags the deliberate cross-bundle signal.
- Runtime behaviour of `delta_mode = true` (the consumer) → sub-bundle 2.26-C.
- `cargo doc` HTML rendering review beyond the build-clean check → manual reviewer step in Stage 4 if desired.
- IC concept explanation on `coordinator_free_rounds` (already shipped in TASK-0350) — not re-touched here.
