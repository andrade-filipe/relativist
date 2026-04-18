# TEST-SPEC-0372: `border_resolver.rs` skeleton + `materialize_agent` helper + module wiring

**Task:** TASK-0372
**Spec:** SPEC-19 §3.2 R13 (pre-condition for border-redex resolution), R14
  (dispatch to `interact_*` topology — scaffolding only), R19 (pure-core
  module); SPEC-19 §3.3 R48 (AgentId allocation coordination; reserved
  range `u32::MAX - 10_000 .. u32::MAX`); 2.26-B spec-critic DC-B1, DC-B2,
  DC-B4 (2026-04-17).
**Generated:** 2026-04-17
**Baseline before this task:** 991 lib (default) / 1031 lib
  (`--features zero-copy`) — post-2.26-A close (TEST-SPEC-0371 final
  count per 2.26-A trajectory 968 → 972 → 973 → 981 → 988 → 990 → 991).
**Cumulative target after this task:** 996 lib (default) / 1036 lib
  (`--features zero-copy`) — **+5** new `#[test]` fns inside
  `relativist-core/src/merge/border_resolver.rs::tests`.

---

## Scope note

TASK-0372 lands the pure-core skeleton of the coordinator-side border-
redex resolver: the NEW file `relativist-core/src/merge/border_resolver.rs`,
a one-line `pub mod border_resolver;` in `merge/mod.rs`, and the single
helper `materialize_agent(partition: &Partition, port: PortRef) ->
Option<(AgentId, Symbol)>`.

Three contracts under test:

1. **Helper correctness.** `materialize_agent` returns `Some((id, sym))`
   for principal-port `AgentPort(id, 0)` pointing at a live agent, and
   `None` for every non-principal-agent input (non-principal port slot,
   `FreePort`, `DISCONNECTED`, vacated agent slot). No panic path.
2. **Pure-core module wiring.** `border_resolver` is reachable from
   `crate::merge::border_resolver::materialize_agent` (pub(crate)) and
   the file imports NONE of `tokio`, `async_trait`, `crate::protocol`.
   This TEST-SPEC ships a compile-time identity fixture; the programmatic
   import guard is TEST-SPEC-0377's territory.
3. **Module-level `//!` doc naming the invariants.** T5 below grep-asserts
   the `//!` block names SPEC-19 §3.2 R13-R15 parts 1-2, §3.3 item 2.26,
   R19 pure-core, DC-B1 (cache), DC-B2 (panic policy reference), DC-B4
   (pinning reference). The doc-presence assertion is the "stability
   lock-in" for DC-B1 / DC-B2 / DC-B4 amendments from the 2.26-B
   spec-critic verdict.

Downstream tasks extend the same `#[cfg(test)] mod tests` block; this
TEST-SPEC seeds its shape.

**Out of scope:**
- `resolve_border_redex` dispatcher body → TEST-SPEC-0373.
- The `assert_agent` helper + caller-side panic path → TEST-SPEC-0373
  (per DC-B2, the `None` from `materialize_agent` is handled at the
  resolver call site, not in the helper).
- Programmatic grep guard → TEST-SPEC-0377.
- Cache maintenance in the coordinator → 2.26-C memo (see verdict
  DC-B1 TASK IMPACT bullet).

---

## Test target file paths

- `relativist-core/src/merge/border_resolver.rs` — **NEW FILE**; inline
  `#[cfg(test)] mod tests` with 5 new `#[test]` fns.
- `relativist-core/src/merge/mod.rs` — one-line `pub mod border_resolver;`
  (exercised implicitly by every other test's import path).

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Unit Tests

| Test ID | Name | Reqs covered | File | Preconditions | Assertions | Expected outcome |
|---------|------|--------------|------|---------------|------------|------------------|
| UT-0372-01 | `materialize_agent_returns_symbol_for_principal_port_of_live_agent` | R13, DC-B1 | `merge/border_resolver.rs::tests` | Build a 1-agent `Partition` with a single `Con` agent at `AgentId(0)`; port `PortRef::AgentPort(0, 0)`. | `materialize_agent(&partition, AgentPort(0, 0)) == Some((0, Symbol::Con))`. | Happy path — helper returns `(id, sym)` exactly. |
| UT-0372-02 | `materialize_agent_returns_none_for_non_principal_port_slot` | R13 (defensive), DC-B2 | `merge/border_resolver.rs::tests` | Same 1-agent fixture; port `PortRef::AgentPort(0, 1)` (auxiliary left) and `AgentPort(0, 2)` (auxiliary right). | Both calls return `None`. | Non-principal port slot yields `None` — callers must only ask about principal ports; helper does not panic. |
| UT-0372-03 | `materialize_agent_returns_none_for_free_port_and_disconnected` | R13 (defensive), DC-B1 (DISCONNECTED handling) | `merge/border_resolver.rs::tests` | Any valid `Partition`; call with `PortRef::FreePort(42)` and `crate::net::DISCONNECTED` (`FreePort(u32::MAX)`). | Both calls return `None`. | `FreePort` variant is never an agent; DISCONNECTED sentinel yields `None` without touching `agents` arena. |
| UT-0372-04 | `materialize_agent_returns_none_for_vacated_agent_slot` | R13 (defensive), DC-B1 | `merge/border_resolver.rs::tests` | Build a `Partition` whose `subnet.agents` has `None` in slot 3 (e.g. via `remove_agent`) and call with `AgentPort(3, 0)`. Also call with `AgentPort(99, 0)` where 99 is past `agents.len()`. | Both calls return `None`. | Defensive: helper tolerates vacated slots and out-of-range IDs; returns `None` instead of panicking or indexing OOB. |
| UT-0372-05 | `border_resolver_module_doc_cites_spec_sections_and_dc_rulings` | R19, DC-B1, DC-B2, DC-B4 (doc presence) | `merge/border_resolver.rs::tests` | Read the module source via `include_str!("border_resolver.rs")`; look at the leading `//!` block only (lines until the first non-`//!` non-blank line). | The `//!` block text contains ALL the substrings: `"SPEC-19 §3.2"`, `"R13"`, `"R14"`, `"R15"`, `"§3.3"`, `"2.26"`, `"R19"`, `"DC-B1"`, `"DC-B2"`, `"DC-B4"`, `"pure-core"`, `"tokio"`, `"protocol"`. | Module-doc stability — future edits that strip the invariant references trip the assertion. |

### Detailed assertions per test

**UT-0372-01** — happy path.
```text
Given:
  let mut net = Net::new();
  let agent_id = net.create_agent(Symbol::Con);
  let partition = Partition {
      subnet: net,
      worker_id: 0,
      free_port_index: HashMap::new(),
      id_range: IdRange { start: 0, end: 1 },
      border_id_start: 0,
      border_id_end: 0,
  };
When:
  let result = materialize_agent(&partition, PortRef::AgentPort(agent_id, 0));
Then:
  assert_eq!(result, Some((agent_id, Symbol::Con)));
```

**UT-0372-02** — non-principal port slot.
```text
Given: same 1-Con-agent partition.
When:
  let r1 = materialize_agent(&partition, PortRef::AgentPort(0, 1));
  let r2 = materialize_agent(&partition, PortRef::AgentPort(0, 2));
Then:
  assert_eq!(r1, None);
  assert_eq!(r2, None);
```

**UT-0372-03** — FreePort + DISCONNECTED.
```text
Given: any valid partition (e.g. empty net).
When:
  let r1 = materialize_agent(&partition, PortRef::FreePort(42));
  let r2 = materialize_agent(&partition, crate::net::DISCONNECTED);
Then:
  assert_eq!(r1, None);
  assert_eq!(r2, None);
```

**UT-0372-04** — vacated slot + out-of-range.
```text
Given:
  let mut net = Net::new();
  let a = net.create_agent(Symbol::Con);
  let b = net.create_agent(Symbol::Dup);
  let c = net.create_agent(Symbol::Era);
  net.remove_agent(b);  // agents[b] is now None
  let partition = Partition { subnet: net, worker_id: 0, ... };
When:
  let r_vacant = materialize_agent(&partition, PortRef::AgentPort(b, 0));
  let r_oor = materialize_agent(&partition, PortRef::AgentPort(999, 0));
Then:
  assert_eq!(r_vacant, None);  // vacated slot
  assert_eq!(r_oor, None);     // past arena
```

**UT-0372-05** — module-doc stability.
```text
Given:
  const SRC: &str = include_str!("border_resolver.rs");
When:
  // Extract the leading //! block by taking lines from the top while
  // they start with "//!" or are blank.
  let doc_block: String = SRC.lines()
      .take_while(|l| {
          let t = l.trim_start();
          t.is_empty() || t.starts_with("//!")
      })
      .collect::<Vec<_>>()
      .join("\n");
Then:
  for needle in [
      "SPEC-19 §3.2", "R13", "R14", "R15",
      "§3.3", "2.26",
      "R19", "pure-core",
      "DC-B1", "DC-B2", "DC-B4",
      "tokio", "protocol",
  ] {
      assert!(
          doc_block.contains(needle),
          "border_resolver.rs //! block missing {needle:?}"
      );
  }
```

---

## Adversarial / QA coverage map

| Requirement / DC | Covered by |
|---|---|
| R13 — precondition for resolution (principal-port materialization) | UT-0372-01 |
| R13 / DC-B2 — defensive `None` on invalid inputs (no panic at helper level) | UT-0372-02, UT-0372-03, UT-0372-04 |
| R14 — scaffolding (the helper is reachable for the dispatcher TASK-0373 to call) | UT-0372-01 (compile-time identity via public path) |
| R19 — pure-core module-doc declares the invariant | UT-0372-05 |
| R48 — doc cites the DC-B5 AgentId reserved range context (via "R48" + "2.26" section mentions) | UT-0372-05 (via the `//!` block's SPEC-19 §3.3 reference — DC-B5 is NOT yet exercised at this task; T0374 adds the pending_commutations path) |
| DC-B1 — coordinator-cache reasoning (resolver takes `&[Partition]`) | UT-0372-01 through UT-0372-04 exercise the `&Partition` surface; UT-0372-05 locks the doc citation |
| DC-B2 — helper keeps `Option` return; caller-side panic lives in TASK-0373 | UT-0372-02, UT-0372-03, UT-0372-04 (all `None` returns, no panics) |
| DC-B4 — border-pinning invariant referenced in doc | UT-0372-05 |

### QA adversarial angles (Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0372-A | Helper changes return type to `(AgentId, Symbol)` (panics on `None`) | UT-0372-02/03/04 fail at compile — helper must keep `Option` |
| QA-0372-B | A future `materialize_agent` implementation accidentally accepts `AgentPort(id, pid)` with `pid != 0` and returns `Some(sym)` | UT-0372-02 fires (assert_eq! fails) |
| QA-0372-C | Future edit removes `DISCONNECTED` handling; calls `agents.get(u32::MAX as usize)` and OOBs | UT-0372-03 catches at runtime |
| QA-0372-D | Future edit strips DC-B1/DC-B2/DC-B4 comments | UT-0372-05 fires |
| QA-0372-E | `pub mod border_resolver;` forgotten in `merge/mod.rs` | ALL other tests fail to compile (import path does not resolve) |
| QA-0372-F | `tokio` import accidentally added in a downstream amendment | UT-0372-05 will not catch (doc still cites "tokio" as forbidden prose); TEST-SPEC-0377's programmatic guard fires |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 991 → **996** (+5 new `#[test]`
   fns in `merge::border_resolver::tests`).
2. `cargo test --workspace --lib --features zero-copy` count: 1031 →
   **1036** (+5).
3. `cargo build --workspace` clean (default features).
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean both
   with and without `--features zero-copy`.
6. `cargo fmt --check` clean.
7. `cargo doc --workspace --no-deps` exits 0 with no broken-link
   warnings on the new `//!` block.
8. Manual grep guard still passes:
   `grep -E '^use\s+(tokio|crate::protocol|async_trait)' relativist-core/src/merge/border_resolver.rs`
   returns zero matches (programmatic lock-in lands in TEST-SPEC-0377).

---

## Resolved ambiguities

- **`None` at caller site, per DC-B2.** The helper returns `Option`
  (the skeleton this task ships); the `assert_agent` shim that panics
  on `None` is TASK-0373's deliverable. No test in this TEST-SPEC
  exercises a panic path.
- **DISCONNECTED handling explicit.** UT-0372-03 exercises the
  `FreePort(u32::MAX)` sentinel to pin the "non-agent port" semantics
  per DC-B1 cache-consistency reasoning (a DISCONNECTED cache entry
  must not yield a spurious agent).
- **Doc-block extraction technique** (UT-0372-05). Take leading
  `//!` lines (blank lines OK) until the first non-doc non-blank line.
  This is deliberately more permissive than the TEST-SPEC-0365
  heading-match pattern because the `//!` block text can grow; the
  test only asserts PRESENCE of named anchors, not ordering or surrounding
  prose.

---

## Test count delta

**+5 tests** (default + zero-copy). Running total after this task:
996 lib / 1036 lib.
