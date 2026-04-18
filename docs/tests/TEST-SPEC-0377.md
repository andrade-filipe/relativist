# TEST-SPEC-0377: Pure-core compliance guard — `border_resolver.rs` import-discipline lock-in

**See also:** [docs/backlog/SPEC-19-section-3.3-coordinator-dispatch-tasks.md](../backlog/SPEC-19-section-3.3-coordinator-dispatch-tasks.md)
  — 2.26-B bundle index (DAG, DC verdict table, acceptance gate).

**Task:** TASK-0377
**Spec:** SPEC-19 §3.2 R19 (pure-core module); SPEC-13 R6-R8 (layer
  boundaries).
**Spec-critic verdicts consumed:**
  - **DC-B8 (resolution option c):** the guard is implemented as a
    shared helper `merge::internal::pure_core_guard::assert_no_forbidden_imports`
    that takes a file's source (via `include_str!`) plus a
    human-readable label, runs the forbidden-prefix scan, and panics
    loudly on violation. Each pure-core file opts in with a
    one-liner `#[test]`. For 2.26-B the only opt-in site is
    `border_resolver.rs`; future pure-core files can adopt the same
    helper with a single extra test fn — no duplicated scan logic.
  - **DC-B9 (resolution: extend list):** the forbidden-prefix list is
    authoritatively FIVE entries:
    1. `use tokio`
    2. `use async_trait`
    3. `use crate::protocol`
    4. `use crate::coordinator`
    5. `use crate::worker`
    Entries 4 and 5 cover the transitive-leak case DC-B9 flagged:
    `coordinator.rs` / `worker.rs` re-export / depend on async +
    protocol types, so any future re-export under `merge/` would
    smuggle async into the pure core even without a direct `tokio`
    `use`.
**Generated:** 2026-04-17
**Baseline before this task:** 1020 lib (default) / 1060 lib
  (`--features zero-copy`) — post TASK-0376 per cumulative trajectory
  991 (2.26-A exit) → 996 → 1002 → 1008 → 1013 → 1020.
**Cumulative target after this task:** 1021 lib / 1061 lib — **+1** new
  `#[test]` fn in `border_resolver.rs`.

---

## Scope note

This TEST-SPEC locks in the R19 pure-core invariant for
`border_resolver.rs` by an in-source programmatic check, mirroring the
manual grep guard used by the §3.2 bundle for `border_graph.rs`.

**Key difference vs. the TASK-0377 default plan:** TASK-0377 shows the
scan logic inlined inside a single `#[test]` fn. Per DC-B8 (option c)
spec-critic ruling, the scan logic is instead factored into a shared
helper `merge::internal::pure_core_guard::assert_no_forbidden_imports`,
and `border_resolver.rs` invokes it via a one-liner `#[test]`. This
keeps the 2.26-B per-file surface to a single test fn while paying
forward a reusable hook for future pure-core opt-ins (e.g. if 2.26-C
adds another `merge/*.rs` file, that file gets the same one-liner
test and the invariant scales without copy-paste).

**Per DC-B9,** the shared helper's forbidden-prefix list has FIVE
entries. The test below asserts presence of all five so that a future
edit that narrows the list (e.g. drops `crate::worker`) fires a loud
red test BEFORE the narrowed helper can silently let a real regression
through.

---

## Test target file paths

- `relativist-core/src/merge/internal/pure_core_guard.rs` — **new**
  `pub(crate)` module containing:
  ```rust
  pub(crate) const FORBIDDEN_USE_PREFIXES: &[&str] = &[
      "use tokio",
      "use async_trait",
      "use crate::protocol",
      "use crate::coordinator",
      "use crate::worker",
  ];

  pub(crate) fn assert_no_forbidden_imports(src: &str, label: &str);
  ```
  The function walks `src.lines()`, trims leading whitespace, skips
  lines not starting with `"use "`, and panics if any remaining line
  begins with any entry in `FORBIDDEN_USE_PREFIXES`. The panic message
  MUST include `label`, the offending prefix, and cite "R19 violation".
- `relativist-core/src/merge/border_resolver.rs` — `#[cfg(test)] mod
  tests` block. ONE new `#[test]` fn
  (`border_resolver_pure_core_no_forbidden_imports`) that invokes the
  shared helper.
- `relativist-core/src/merge/internal/mod.rs` — **modify**: add
  `pub(crate) mod pure_core_guard;`. If `internal/` does not yet exist
  (the 2.26-B bundle is the first consumer of `merge/internal/`),
  create `relativist-core/src/merge/internal/mod.rs` plus the module
  declaration in `merge/mod.rs` (`pub(crate) mod internal;`).

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Unit Tests

### T1: `border_resolver_pure_core_no_forbidden_imports`

**Purpose:** Programmatic in-source guard that `border_resolver.rs`
imports nothing from `tokio`, `async_trait`, `crate::protocol`,
`crate::coordinator`, or `crate::worker`. This is the R19 regression
guard — a future 2.26-C developer who accidentally writes
`use tokio::time::sleep;` or `use crate::coordinator::Coordinator;` at
the top of `border_resolver.rs` will see a red test immediately.

**Target file:** `merge/border_resolver.rs::tests`.

**Given:** `include_str!("border_resolver.rs")` — the file's own
source at compile time (stable Rust macro; no external crates; no
`grep` dependency; works on Windows).

**When:** The test calls
`crate::merge::internal::pure_core_guard::assert_no_forbidden_imports(src, "border_resolver.rs")`.

**Then:**
```rust
#[test]
fn border_resolver_pure_core_no_forbidden_imports() {
    // SPEC-19 R19 (pure-core module): border_resolver.rs MUST NOT
    // import tokio, async_trait, crate::protocol, crate::coordinator,
    // or crate::worker. The first three are direct violations; the
    // last two cover the transitive-leak case flagged by DC-B9
    // (2026-04-17 spec-critic) — coordinator.rs and worker.rs
    // themselves depend on protocol + async, so re-exporting their
    // types under merge/ would smuggle async into the pure core.
    //
    // DC-B8 (2026-04-17 spec-critic) shared-helper resolution: the
    // scan logic lives in merge::internal::pure_core_guard so future
    // pure-core files can opt in with a one-liner test.
    //
    // TODO: when a new pure-core file is added to merge/, add a
    // mirror test in that file invoking the same helper. Do NOT
    // inline the scan logic — keep it in the shared helper.

    let src = include_str!("border_resolver.rs");

    // Sanity: helper's forbidden-prefix list is the authoritative
    // DC-B9 set of FIVE entries. If this assertion fires, someone
    // narrowed the list and this test is the canary.
    assert_eq!(
        crate::merge::internal::pure_core_guard::FORBIDDEN_USE_PREFIXES.len(),
        5,
        "DC-B9: forbidden-prefix list must contain exactly 5 entries \
         (use tokio, use async_trait, use crate::protocol, \
         use crate::coordinator, use crate::worker); adjust this \
         assertion ONLY if DC-B9 is formally revised by a new \
         spec-critic verdict"
    );
    for prefix in ["use tokio", "use async_trait", "use crate::protocol",
                   "use crate::coordinator", "use crate::worker"] {
        assert!(
            crate::merge::internal::pure_core_guard::FORBIDDEN_USE_PREFIXES
                .contains(&prefix),
            "DC-B9: expected forbidden prefix {:?} in the guard's \
             prefix list — drift between test and helper means the \
             guard is silently weaker than spec",
            prefix
        );
    }

    // Main guard: panic loudly (with label + prefix + "R19 violation")
    // if any forbidden import is present.
    crate::merge::internal::pure_core_guard::assert_no_forbidden_imports(
        src,
        "border_resolver.rs",
    );
}
```

**Assertions:**
- 1 × cardinality of `FORBIDDEN_USE_PREFIXES == 5` — canary against
  helper drift (list narrowed without a spec-critic verdict).
- 5 × membership check — each of the five DC-B9 prefixes is present
  in the helper's list.
- 1 × main guard invocation — the helper itself panics with a
  descriptive message if `border_resolver.rs` contains any forbidden
  `use` line. On a clean file the helper is a silent pass; the test
  is green because no assertion inside the helper fires.

**SPEC-19 R covered:** R19 (pure-core module, applied to
`border_resolver.rs`).
**SPEC-13 R covered:** R6-R8 (layer boundaries: `merge/` is pure, does
not reach sideways into `protocol/` / `coordinator` / `worker`, and
does not depend on async runtime).
**DC verdicts covered:** DC-B8 (shared-helper factoring), DC-B9
(forbidden-list extension to 5 entries).

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| SPEC-19 R19 — `border_resolver.rs` imports no tokio / async_trait / crate::protocol | T1 main guard invocation |
| SPEC-19 R19 — transitive-leak closure via coordinator / worker re-exports | T1 main guard invocation (DC-B9 list items 4-5) |
| DC-B8 — scan logic factored into `merge::internal::pure_core_guard` shared helper | T1 call site is a one-liner; scan logic not inlined |
| DC-B9 — forbidden-prefix list is authoritatively 5 entries | T1 cardinality + membership assertions (6 total pre-guard assertions) |
| Future pure-core files can opt in without duplicating logic | Implicit: T1 is the reference one-liner shape for future files |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0377-A | A future 2.26-C developer adds `use tokio::time::sleep;` to `border_resolver.rs` | Helper panics with "R19 violation: border_resolver.rs imports use tokio..."; T1 fires red. Happy path. |
| QA-0377-B | Developer adds `use crate::coordinator::Coordinator;` to pull in a helper type | Helper panics on the `use crate::coordinator` prefix; T1 fires red. DC-B9 transitive-leak protection works. |
| QA-0377-C | Developer adds `use async_trait::async_trait;` to decorate a trait inside `merge/` | Helper panics on `use async_trait` prefix; T1 fires red. |
| QA-0377-D | Developer adds `use crate::protocol::types::Message;` to peek at protocol payloads from inside `merge/` | Helper panics on `use crate::protocol` prefix; T1 fires red. |
| QA-0377-E | Developer adds `use crate::worker::WorkerState;` | Helper panics on `use crate::worker` prefix; T1 fires red. |
| QA-0377-F | Developer re-exports a protocol type indirectly: `pub use crate::protocol::types::Message as Msg;` (leading `pub use`, not `use`) | T1 does NOT catch this — the helper filters on `starts_with("use ")` after trimming, and `pub use` does not match. **Flagged for QA / future hardening.** Propose extending the helper to also scan `pub use` in a follow-up task. |
| QA-0377-G | Developer writes `use\ttokio::...` (tab instead of space after `use`) | Helper trims leading whitespace on the LINE but matches prefix `"use "` (space). Tab after `use` would evade the prefix check. Rustc rejects `use\ttokio` in practice (syntax requires whitespace-separated path), so this is a theoretical edge; **note for QA**. |
| QA-0377-H | Developer writes the import inside a `#[cfg(test)]` block | Helper scans ALL lines of the file, including test-only `use` statements. Intentional: R19 says pure-core files must not depend on async or protocol even in tests. Confirm with reviewer. |
| QA-0377-I | A spec revision legitimately widens R19 to forbid a 6th prefix (e.g. `use reqwest`) | The helper's `FORBIDDEN_USE_PREFIXES` constant is updated to 6 entries, but T1's `cardinality == 5` assertion fires red. Intentional canary — forces the TEST-SPEC to be updated alongside the spec. |
| QA-0377-J | A spec revision legitimately narrows R19 (e.g. `crate::worker` is reclassified as pure) | Helper list drops to 4 entries; T1 cardinality fires. Forces explicit spec-critic sign-off via a new verdict, not a silent narrowing. |
| QA-0377-K | Helper is deleted or module path renamed | `border_resolver_pure_core_no_forbidden_imports` fails to compile — path resolution error. Good fail-fast. |
| QA-0377-L | Helper becomes a no-op stub (e.g. function body returns early unconditionally) | T1 passes on a clean file but would also pass on a dirty file. **Flagged for QA.** Mitigation: ADD a helper-level unit test inside `pure_core_guard.rs` itself that feeds in a synthetic `"use tokio::time;\n"` string and asserts `std::panic::catch_unwind` catches a panic. This would be TASK-0378-level scope; **deferred**. |
| QA-0377-M | `include_str!` loads a stale snapshot (Rust caches) | `include_str!` is re-evaluated on every rebuild when the source file's mtime changes. cargo's incremental build tracks this. Green. |
| QA-0377-N | Future refactor moves `border_resolver.rs` to a subdirectory (e.g. `merge/resolver/border_resolver.rs`) | `include_str!("border_resolver.rs")` resolves relative to the file that calls the macro, so as long as the test fn lives INSIDE `border_resolver.rs`, the path stays correct. If someone factors the test out to a sibling file, the path breaks. Good fail-fast. |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 1020 → **1021** (+1 new
   `#[test]` fn).
2. `cargo test --workspace --lib --features zero-copy` count: 1060 →
   **1061** (+1).
3. `cargo build --workspace` clean.
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean.
6. `cargo fmt --check` clean.
7. No `unwrap()` in production code; test uses `.expect(...)` /
   `panic!` only inside the helper's guard-violation path.
8. `pure_core_guard` module is `pub(crate)` (not `pub`) — the helper
   is an internal reuse surface, not a public API.
9. Per DC-B8 option (c): `border_resolver.rs` invokes the helper in a
   one-liner test; the scan logic is NOT duplicated inline.
10. Per DC-B9: the helper's `FORBIDDEN_USE_PREFIXES` array contains
    exactly 5 entries — `use tokio`, `use async_trait`,
    `use crate::protocol`, `use crate::coordinator`, `use crate::worker`.
11. R19 manual grep (legacy §3.2 bundle check) still passes against
    `border_graph.rs` — this TEST-SPEC does NOT regress the §3.2
    guard; it adds a programmatic guard for the NEW file only (per
    TASK-0377 note: "Mirrors-file guard [for border_graph.rs]
    OPTIONAL — default: skip to keep this task scoped to 2.26-B's
    deliverable").

---

## Notes on interaction with other TEST-SPECs

- **TEST-SPEC-0372** (skeleton + `materialize_agent` helper) establishes
  the `border_resolver.rs` file; T1 here exists because that file
  exists. If the bundle is rolled back to pre-TASK-0372,
  `include_str!("border_resolver.rs")` fails to compile and this test
  is removed alongside the file.
- **TEST-SPEC-0373..0376** add behavioral tests (dispatcher,
  commutation, erasure, packaging, integration). T1 is orthogonal —
  it guards the file's import set, not its behavior.
- **§3.2 bundle's `border_graph.rs` grep guard** (manual, documented
  in the §3.2 bundle acceptance gate): complementary. This TEST-SPEC
  does not extend the in-source helper to `border_graph.rs` per
  TASK-0377's explicit "default: skip" note. A follow-up hardening
  task MAY adopt the helper for `border_graph.rs` by adding a
  one-liner test there — shape is identical to T1.
- **R19 enforcement at CI level:** the `cargo test` invocation in CI
  exercises T1 on every push; no extra workflow step needed.

---

## Out of scope

- **Extending the guard to `border_graph.rs`** — flagged by TASK-0377
  Mirrors-file clause as OPTIONAL and default-skip. Deferred to a
  future hardening task.
- **Scanning `pub use` re-exports** — flagged in QA-0377-F. Deferred:
  a follow-up can extend the helper to also match `pub use ` prefix.
- **Negative-path assertions** (e.g. feeding a synthetic "use tokio;"
  string to the helper and asserting the helper itself panics) —
  TASK-0377 flags this as OPTIONAL ("TEST-SPEC-0377 may add
  negative-path assertions but this is optional"). Deferred to keep
  the 2.26-B scope tight and avoid introducing `catch_unwind`
  complexity. Can be added as a helper-level unit test inside
  `pure_core_guard.rs` in a later hardening pass (QA-0377-L above).
- **Build-time (not test-time) enforcement** — `build.rs` / custom
  lint infra. TASK-0377 notes explicitly rule this out as infra-
  heavy; in-source `#[test]` is the approved mechanism.
- **Cross-workspace enforcement** — this TEST-SPEC only guards
  `border_resolver.rs`. Other crates in the workspace are out of
  scope; each adds its own opt-in test if/when they need R19-style
  isolation.
