# TEST-SPEC-0365: Module-level `//!` doc + R19 pure-core invariant guard + R13/R14/R15 scaffolding placeholders

**Task:** TASK-0365
**Spec:** SPEC-19 §3.2 (R13, R14, R15 parts 1 and 2 — scaffolding only; R19)
**Generated:** 2026-04-17
**Baseline before this task:** 938+ lib (post-TASK-0364)
**Cumulative target after this task:** 941+ lib (≥ +3 new tests)

---

## Scope note

TASK-0365 is a **documentation-only task** in terms of production
code: no new `impl` blocks, no new types, no new behavior. The task's
only code deliverable is the expanded `//!` module-level doc comment
at the top of `border_graph.rs` (lifecycle, R19 invariant, out-of-scope
pointers — see TASK-0365 Key Types block for the mandated text).

However, a small set of tests is warranted to **lock the R19 pure-core
invariant as a positive contract** and to pin the doc-comment's
stability. Without these tests, a future patch could silently introduce
`use tokio::...;` into `border_graph.rs` and regress the pure-core
guarantee — the grep CI guard in the task acceptance criteria catches
this, but a test inside the workspace (running on every `cargo test`)
provides a second defense.

**No scaffolding code for R13/R14/R15 parts 1-2:** per the bundle
scope decision, coordinator-side dispatch and `interact_*` callsites
ship under item 2.26. This test-spec does NOT require any placeholder
functions, stubs, or `todo!()` marker code. The "scaffolding placeholder
assertions" referenced in the test-generator brief are satisfied by:
- The `cargo doc` no-warnings gate (intra-doc links to
  `detect_border_redexes`, `apply_deltas`, `remove_border`,
  `add_border_states` resolve).
- The documentation-presence tests below that verify the `//!` block
  contains the expected section headings.

**DC-4 cascade baked in:** the doc-presence test (UT-0365-02) asserts
the `//!` block names `AddBorderEntry` as the input type to
`add_border_states`, per the TASK-0365 spec-critic amendment table.

---

## Test target file paths

- `relativist-core/src/merge/border_graph.rs` — extend inline
  `#[cfg(test)] mod tests` block with three new tests.

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Unit Tests

### UT-0365-01: `border_graph_source_file_respects_r19_pure_core_invariant`

**Purpose:** Positive contract that `border_graph.rs` does NOT import
`tokio`, `async_trait`, or anything from `crate::protocol::*`. The
grep CI guard is the primary enforcement (runs at merge time); this
test is a secondary in-workspace canary that catches regressions on
every `cargo test`.

**Target file:** `merge/border_graph.rs::tests`

**Mechanism:** `include_str!("border_graph.rs")` at compile time, then
substring-scan for forbidden use-statements. The test is line-oriented
so `use crate::partition::types::WorkerId;` (legitimate) does not
trigger.

**Given:** The current source file contents, embedded via
`include_str!`.

**When:** Scan each line of the file for forbidden prefixes.

**Then:**
```rust
#[test]
fn border_graph_source_respects_r19_pure_core_invariant() {
    let source: &str = include_str!("border_graph.rs");

    for (line_number, line) in source.lines().enumerate() {
        // Skip comments, doc-comments, blank lines — only actual `use`
        // items should be checked.
        let trimmed = line.trim_start();
        if !trimmed.starts_with("use ") {
            continue;
        }
        // Forbidden prefixes per R19 + TASK-0365 acceptance criterion.
        for forbidden in &[
            "use tokio",
            "use async_trait",
            "use crate::protocol",
        ] {
            assert!(
                !trimmed.starts_with(forbidden),
                "R19 pure-core invariant violated at line {}: `{}` (border_graph.rs MUST NOT import tokio / async_trait / crate::protocol)",
                line_number + 1,
                trimmed,
            );
        }
    }
}
```

**Assertions:**
- No `use` line in `border_graph.rs` begins with `use tokio`,
  `use async_trait`, or `use crate::protocol`.
- The test is robust to comment content (won't false-positive on a
  doc-comment that mentions `tokio` — the scan only considers lines
  starting with `use `).

**Spec traceability:** TASK-0365 Acceptance Criteria line 84 ("R19
grep guard still passes"). This test is the workspace-level mirror of
the CI grep.

**SPEC-19 R covered:** R19 (pure-core).

---

### UT-0365-02: `border_graph_module_doc_references_documented_coordinator_lifecycle`

**Purpose:** The `//!` block MUST describe the coordinator-side
lifecycle (TASK-0365 Acceptance Criteria line 42 — bullet 2), naming
all five primitives that this bundle ships PLUS the `AddBorderEntry`
input (DC-4 cascade). This test pins the doc-comment's stability so
a refactor that removes a primitive reference (e.g., "do we still need
`remove_border`?") is caught.

**Target file:** `merge/border_graph.rs::tests`

**Mechanism:** `include_str!` again, substring-scan for required terms.

**Given:** `include_str!("border_graph.rs")`.

**When:** Check the string contains the five primitive names + the
lifecycle section heading + the `AddBorderEntry` type name.

**Then:**
```rust
#[test]
fn border_graph_module_doc_references_coordinator_lifecycle() {
    let source: &str = include_str!("border_graph.rs");

    // Section heading (TASK-0365 Acceptance Criteria bullet 2).
    assert!(
        source.contains("Coordinator-side lifecycle"),
        "module doc MUST contain the heading `Coordinator-side lifecycle`"
    );
    // R19 section heading.
    assert!(
        source.contains("Pure-core invariant"),
        "module doc MUST contain the heading `Pure-core invariant`"
    );
    // Out-of-scope heading (guards against silent scope-creep).
    assert!(
        source.contains("Out of scope"),
        "module doc MUST contain the heading `Out of scope`"
    );

    // Five primitive names (intra-doc links in the actual doc).
    for primitive in &[
        "detect_border_redexes",
        "apply_deltas",
        "remove_border",
        "add_border_states",
        "from_partition_plan",
    ] {
        assert!(
            source.contains(primitive),
            "module doc MUST reference primitive `{primitive}`"
        );
    }

    // DC-4 cascade: the AddBorderEntry input type MUST be named in the
    // coordinator-lifecycle bullet.
    assert!(
        source.contains("AddBorderEntry"),
        "module doc MUST reference `AddBorderEntry` (DC-4 cascade — \
         the add_border_states input struct)"
    );
}
```

**Assertions:**
- All three section headings present in the doc text.
- All five primitive names present (either in the lifecycle bullets
  or as intra-doc links).
- `AddBorderEntry` is named (DC-4 cascade — enforces that the doc
  reflects the post-amendment signature, not the deleted Option A
  `Vec<BorderState>` form).

**Spec traceability:** TASK-0365 Acceptance Criteria line 42 + DC-4
cascade amendment table.

**SPEC-19 R covered:** R13/R14/R15 documentation presence (the doc
records the future coordinator integration without shipping the code).

---

### UT-0365-03: `border_graph_is_sync_and_send`

**Purpose:** Positive contract that `BorderGraph` is `Send + Sync`.
This is an R19 consequence: a pure-core data structure of standard
collections (`HashMap`, `HashSet`, `Vec<Vec<u32>>`, `BorderState`
fields are all `Copy` or primitive) MUST trivially satisfy both auto
traits. The test locks the property so a future refactor that inserts
an `Rc<T>` or a `*const T` (breaking `Send`/`Sync`) is rejected at
compile time.

**Target file:** `merge/border_graph.rs::tests`

**Given:** `BorderGraph`, `BorderState`, `BorderDelta`, `AddBorderEntry`
types.

**When:** Compile-time `Send + Sync` bound check via a helper.

**Then:**
```rust
#[test]
fn border_graph_and_friends_are_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<BorderGraph>();
    assert_send_sync::<BorderState>();
    assert_send_sync::<BorderDelta>();
    assert_send_sync::<AddBorderEntry>();
}
```

**Assertions:**
- The function body compiles. Any refactor inserting a non-`Send` or
  non-`Sync` field fails the build.

**Spec traceability:** implicit from R19 (pure-core data lives on
either side of the coordinator / protocol boundary and must be
freely shareable) and the SPEC-13 layer-boundary guidance. Not
directly in TASK-0365 Acceptance Criteria but a natural fit for the
"scaffolding placeholder assertions" the test-generator brief
requests.

**SPEC-19 R covered:** R19 (pure-core, trivially `Send + Sync`).

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R19 pure-core invariant (no `tokio`, no `async_trait`, no `crate::protocol`) | UT-0365-01 (source scan) |
| R19 pure-core invariant (`Send + Sync` consequence) | UT-0365-03 |
| R13/R14 scaffolding: coordinator-side lifecycle documented | UT-0365-02 (doc presence) |
| R15 part 1 scaffolding: coordinator-side `apply_deltas` + `remove_border` dispatch documented | UT-0365-02 (references `apply_deltas`, `remove_border`) |
| R15 part 2 scaffolding: coordinator-side graph update documented | UT-0365-02 (via lifecycle section) |
| DC-4 cascade: `AddBorderEntry` referenced in doc block | UT-0365-02 (asserts `AddBorderEntry` literal substring) |
| "Out of scope" scope-creep guard | UT-0365-02 (asserts "Out of scope" heading) |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0365-A | A PR adds `use tokio::sync::Mutex;` inside a function body (not a top-level `use`) | UT-0365-01 checks lines starting with `use `; an inner `use` would match too. But a FULLY-QUALIFIED `tokio::sync::Mutex::new(...)` without a `use` statement would bypass the check. CI grep should complement with a full-file regex on `tokio::`. QA should confirm |
| QA-0365-B | The doc-block is reformatted in a way that breaks a section heading (e.g., rewording "Pure-core invariant" to "Pure core invariant") | UT-0365-02 catches the specific string mismatch. Adversarial test: a PR author who rewords the heading must update the test OR the heading — forces a deliberate choice |
| QA-0365-C | `BorderGraph` gains an `Arc<Mutex<T>>` field (breaks `Send + Sync` only if `T: !Send`) | UT-0365-03 would still pass if T is Send. QA probes with a non-Send `T` to confirm the canary fires |
| QA-0365-D | `cargo doc --workspace --no-deps` fails because a new intra-doc link is broken | Acceptance gate — not a `#[test]`. QA runs this as a build-level guard |
| QA-0365-E | A `#[cfg(not(test))]` branch imports `tokio` in `border_graph.rs` | UT-0365-01's `include_str!` sees the file as-is (including both cfg branches); so the test would still catch it. Good |
| QA-0365-F | A developer adds a `use crate::protocol::types::PartitionResult;` to import a type for a doc-link resolution | Forbidden per R19 — UT-0365-01 catches. Doc links can use fully-qualified paths in the comment without the `use` |
| QA-0365-G | The `//!` block gains an example that references a type from `crate::protocol` via a fully-qualified doctest import | `rustdoc` doctest execution would pull `crate::protocol`, indirectly depending on `tokio`. Document in the doc-block's example that examples MUST use only `crate::merge` + `crate::net` types |

---

## Acceptance gate

1. `cargo test --workspace` count: 938 → **941+** (≥ +3: UT-0365-01,
   UT-0365-02, UT-0365-03).
2. Same +3 under `--features zero-copy` (985 → 988 post-0364).
3. All previously passing tests still pass (no regression).
4. `cargo clippy --workspace --all-targets -- -D warnings` clean.
5. `cargo fmt --check` clean.
6. `cargo doc --workspace --no-deps` exits 0 with no broken intra-doc
   link warnings against `border_graph.rs`.
7. Grep guard (external): `grep -E '^use\s+(tokio|crate::protocol|async_trait)'
   relativist-core/src/merge/border_graph.rs` returns zero matches —
   redundant with UT-0365-01 but runs at CI level.

---

## Out of scope (deferred to item 2.26)

- Coordinator-side dispatch loop (R13).
- Coordinator-side `interact_*` call site (R14).
- `Message::RoundStart` / `Message::RoundResult` wire-format
  extensions (R20-R36).
- `GridConfig.delta_mode` flag (R20).
- Worker-side delta emission and stateful-worker lifecycle
  (R20-R30).
- The `run_grid_delta` BSP loop (SPEC-19 §3.3, §4.3).

The TEST-SPECs for the above are a future bundle's responsibility
(item 2.26).

---

## Design-choice verdict traceability (bundle-level)

This TEST-SPEC, together with TEST-SPEC-0360..0364, completes the
SPEC-19 §3.2 bundle. All four design-choice verdicts from
`docs/spec-reviews/SPEC-19-section-3.2-design-choices-2026-04-17.md`
are baked into the Stage-2 contracts:

| DC | Verdict | Test-spec anchor |
|---|---|---|
| DC-1 | DISCONNECTED = `crate::net::DISCONNECTED` (`PortRef::FreePort(u32::MAX)`); no `BorderTarget` enum | TEST-SPEC-0362 UT-0362-05, UT-0362-06, UT-0362-11 (all use the named constant) |
| DC-2 | Ship `worker_borders: Vec<Vec<u32>>` now; doc-comment locks it to item 2.26 R23 consumer | TEST-SPEC-0360 UT-0360-05 (field present in struct shape); TEST-SPEC-0361 UT-0361-02, UT-0361-04 (worker_borders populated by `from_partition_plan`); TEST-SPEC-0364 UT-0364-07 (updated by `add_border_states`) |
| DC-3 | `detect_border_redexes` → owned `Vec<(u32, BorderState)>` | TEST-SPEC-0363 UT-0363-02, UT-0363-03 (explicit owned type annotation) |
| DC-4 | `add_border_states` takes `Vec<AddBorderEntry>`; graph computes `is_redex` | TEST-SPEC-0364 UT-0364-05..08, UT-0364-11, and the DC-4-mandated UT-0364-12 (`add_border_states_enforces_is_redex_invariant`) |
