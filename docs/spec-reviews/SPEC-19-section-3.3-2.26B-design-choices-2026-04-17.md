# SPEC-19 §3.3 (item 2.26-B) — Design Choices Verdict (Stage 1.5 spec-critic)

**Date:** 2026-04-17
**Reviewer:** spec-critic (adversarial)
**Bundle:** SPEC-19 §3.3 item 2.26-B — Coordinator-side border-redex
  resolution (R13, R14, R15 parts 1-2). R13-R15 live in §3.2 of the
  spec file; §3.3 is the surrounding delta-protocol context. The
  2.26-B sub-bundle label follows the ROADMAP item-level decomposition,
  not the spec section number.
**Predecessors consulted:** SPEC-19 §3.2 R8-R19 (BorderGraph data
  structure — already shipped by item 2.35 bundle), SPEC-19 §3.3
  R20-R37 (delta-protocol wire contract — 2.26-A territory), SPEC-19
  §3.5 R38-R40 (invariant amendments — 2.26-D territory), SPEC-03
  (6 interaction rules).
**Source consulted:**
  `relativist-core/src/merge/border_graph.rs` (the §3.2 deliverable —
  `BorderGraph`, `BorderState`, `BorderDelta`, `AddBorderEntry`),
  `relativist-core/src/reduction/rules.rs` (`interact_void`,
  `interact_anni`, `interact_comm`, `interact_eras`),
  `relativist-core/src/partition/types.rs` (`Partition`, `IdRange`,
  `WorkerId`, `free_port_index: HashMap<u32, PortRef>`),
  `relativist-core/src/net/types.rs` (`PortRef`, `DISCONNECTED`,
  compact wire encoding),
  `docs/backlog/TASK-0372.md` .. `TASK-0377.md`,
  `docs/backlog/SPEC-19-section-3.3-coordinator-dispatch-tasks.md`.
**Precedent consulted:**
  `docs/spec-reviews/SPEC-19-section-3.2-design-choices-2026-04-17.md`
  (format + verdict style; directly sibling to this review).

---

## Overall Assessment

The 2.26-B SPLITTING bundle is structurally sound. Nine design
choices were flagged; five have a single defensible option once the
spec text and existing code are fully read (DC-B1, DC-B3, DC-B4,
DC-B5, DC-B6), three are taxonomy cleanups with clear winners (DC-B7,
DC-B8, DC-B9), and one is a genuine policy question with a
conservative answer (DC-B2).

**Two amendments have non-trivial cross-bundle coupling:**

1. **DC-B1** ratifies Option (A) — coordinator keeps a shadow
   `Partition` cache — but this requires the coordinator to apply
   `BorderDelta`s to its OWN cache at the same time it applies them to
   the `BorderGraph`. That mirror-update logic lives in 2.26-C's BSP
   loop, not in this bundle. The bundle ships the pure-core resolver
   with the cache as an input (`&[Partition]`); cache maintenance is a
   2.26-C concern.
2. **DC-B5** ratifies Option (B) — workers allocate new agent IDs
   locally and echo them in `RoundResult`. This adds a new field
   (`minted_agents: Vec<(AgentId, Symbol)>`) to the wire variant
   `Message::RoundResult` owned by 2.26-A, AND a new field
   (`pending_commutations: Vec<PendingCommutation>`) to
   `Message::RoundStart` also owned by 2.26-A. Cross-bundle
   coordination required. See DC-B5 TASK IMPACT for the exact fields
   and 2.26-A memo text.

Other amendments (DC-B2, DC-B3, DC-B6, DC-B7, DC-B8, DC-B9) are
internal to 2.26-B task files.

**Verdict:** APPROVED WITH AMENDMENTS to **task files only** (no spec
edits to `specs/SPEC-19-delta-protocol.md`). Stage 2 TESTS unblocked
once task-updater applies the amendments listed per-DC below, plus
the cross-bundle memos to 2.26-A (DC-B5 adds two `Message` fields),
2.26-C (DC-B1 cache maintenance, DC-B4 ordering obligation, DC-B5
two-phase round), 2.26-D (DC-B4 may touch G1 amendment text — see
verdict).

---

## Verdicts (9 design choices)

### DC-B1: Agent data sourcing

**PICK:** **Option (A)** — coordinator holds a READ-ONLY CACHED
`Partition` per worker, refreshed by applying incoming `BorderDelta`s
+ `minted_agents` reports from each `RoundResult`. The resolver takes
`&[Partition]` (one entry per worker, indexed by `WorkerId as usize`)
and does NOT mutate.

**WHY:** The three options are not equivalent on wire cost or
correctness:

- **Option (A) — cache.** The coordinator already receives the full
  partition once at round 0 via `Message::InitialPartition` (SPEC-19
  R31, discriminant 7). That payload is the BSP-worker's starting
  state; the coordinator has no reason to discard it — keeping it
  costs one `Partition` clone per worker (one-time, round 0) plus
  per-round delta application (already O(|deltas|) work the
  coordinator does anyway when updating `BorderGraph`). **Zero extra
  wire traffic.**
- **Option (B) — on-demand request.** Adds a new C->W round trip per
  border redex: `AgentRequest(wid, aid) -> AgentResponse(agent)`.
  At break-even analysis (ROADMAP 2.40), the current c_o/c_r = 2.2
  figure already fails the break-even for w=2; adding 1-2 ms of
  network latency PER BORDER REDEX pushes the ratio further in the
  wrong direction. Every CON-DUP resolution would need 2 agent
  fetches; every round with k border redexes adds 2k latencies
  serially (the coordinator cannot resolve redex K+1 before it knows
  agent K's auxiliary port targets, because CON-DUP commutation may
  have moved them).
- **Option (C) — pre-shipped candidates.** Worker includes the agents
  in `RoundResult` whenever `has_border_activity == true`. This
  duplicates data: for every round where worker A has ≥1 principal
  border port, the entire worker-A agent set adjacent to borders is
  shipped — even for borders that worker A's side is principal but
  worker B's side is auxiliary (no redex). Wire cost is a function of
  border-adjacency fan-out, which for a high-connectivity workload is
  unbounded. The coordinator then receives data it mostly does not
  need.

**Option (A) wins on wire cost, latency, and absence of redundancy.**
It does require the coordinator to shadow each worker's partition;
that mirror is R11 semantics (`apply_deltas` against the local
cache, parallel to the graph update) plus one extension to register
newly-minted agents from DC-B5. The additional coordinator memory is
bounded by the sum of worker partition sizes — which the coordinator
already holds transiently inside `merge()` in v1, so it is not new
memory pressure in absolute terms, only in duration.

**Correctness against stale cache:** strict BSP (SPEC-19 §3.5 R40a
lenient mode; bundle 2.26-D owns the invariant amendment) ensures the
coordinator's cache is synchronized with the worker's true state
**at round boundaries**. The resolver operates ONLY between rounds
(coordinator holds the round-result from every worker before
dispatching round N+1), so the cache is never read mid-round. DC-B4
addresses the subtle race path in detail.

**AMENDMENT NEEDED?** No spec amendment. **TASK-0372 amendment
required** — lock Option (A) in the file's `//!` doc and in the
`Notes` section. **Cross-bundle memo to 2.26-C** — the coordinator
BSP loop owns the cache's maintenance.

**TASK IMPACT:**
- **TASK-0372** "Context" — add a sentence after the third paragraph:
  "Per spec-critic DC-B1, the resolver consumes a read-only slice of
  the coordinator's cached partitions (`&[Partition]`, indexed by
  `WorkerId as usize`). Cache maintenance — applying `BorderDelta`s
  and `minted_agents` reports to each worker's cached partition — is
  2.26-C territory; this bundle's resolver is a pure function of the
  cache state at round boundaries."
- **TASK-0372** "Notes" — delete the DC-B1 bullet under "Design
  Choices Flagged" (moved to the resolved-verdict citation below);
  replace the `materialize_agent` signature discussion with:
  "Per DC-B1, `materialize_agent(partition: &Partition, port: PortRef)`
  stays as specified — the caller supplies the partition slice;
  `materialize_agent` is pure."
- **TASK-0372 cross-reference** — add a new sentence in "Dependencies
  on Sibling Bundles" (create the subsection if absent):
  "Consumed by 2.26-C: cache maintenance in `coordinator.rs`
  (`apply_deltas` mirror on per-worker cached `Partition`s, minted-
  agent registration from `RoundResult.minted_agents` per DC-B5)
  lives outside this bundle."
- **Cross-bundle memo to 2.26-C** (task-updater delivers when 2.26-C
  enters Stage 1): "DC-B1 ratified Option (A). 2.26-C must add a
  `coordinator_partition_cache: Vec<Partition>` field to the
  coordinator state and two update paths — (i) on each `RoundResult`,
  apply `border_deltas` to `cache[worker_id]` via `apply_deltas` on
  both the `BorderGraph` and the cached partition; (ii) on each
  `RoundResult.minted_agents` (DC-B5), insert the new agents into
  `cache[worker_id].subnet`. The resolver call
  `resolve_border_redex(&mut graph, &cache, bid)` at round boundary
  reads from the cache."

---

### DC-B2: `materialize_agent` missing-agent error strategy

**PICK:** **Option (A)** — **panic** with a diagnostic message.
BUT wrap the panic in a named helper (`border_resolver_assert_agent`)
so the panic site is grep-able and the error message is uniform
across all three resolver callers (`resolve_con_con`,
`resolve_con_dup`, `resolve_con_era`, …).

**WHY:** The resolver is called AT the coordinator in between BSP
rounds. Per DC-B4 (see below), strict BSP + the cache consistency
invariant (DC-B1) MUST guarantee that if a border's `is_redex == true`
then both sides' agents exist in the coordinator's cache. A `None`
return from `materialize_agent` at this site means the cache is
desynchronized from the `BorderGraph` — i.e., **a coordinator
invariant has been violated**, not a recoverable error.

Option (B) — typed `Result<_, BorderResolverError::AgentMissing>` —
has two problems:

1. **Error-value bleed.** The resolver's caller (the 2.26-C BSP
   loop) receives `Err(AgentMissing)` and must decide what to do.
   Continuing means entering round N+1 with a desynced cache (the
   bug gets worse). Aborting means shutting the coordinator down
   with a non-panic error. The non-panic error path adds code at
   every resolver call site for zero semantic benefit — the only
   correct response is "abort".
2. **Diagnostic weakness.** A `Result::Err` collapses to a string in
   the caller's log; a `panic!` with `"border_resolver: agent
   missing for border {bid} on worker {wid} side {a_or_b} — cache
   desync; check DC-B1 maintenance in coordinator.rs"` gives the
   stack trace + line number + suggestion in one step.

The v1 codebase already uses the panic discipline for invariant
violations: `reduce_all`'s validity checks (`SPEC-03 R12`),
`Net::remove_agent` on a vacant slot, the `is_principal_pair` guard
in `merge/core.rs` all panic. This is consistent with the
`.unwrap()` ban in coding standards (CLAUDE.md Relativist line
"No `unwrap()` in production code") because **explicit `panic!` with
a message is not `unwrap()`** — it's a typed assertion.

**Edge case worth fixing:** the `materialize_agent` signature
currently returns `Option<(AgentId, Symbol)>`. That's the right shape
for the HELPER (which must be defensive; callers other than the
resolver may ask about ports that are legitimately not agent-ports —
e.g. future tests); the `panic!` belongs at the **CALLER** side, i.e.
inside `resolve_border_redex` after `materialize_agent` returns
`None`. Keep the helper's `Option` return.

**AMENDMENT NEEDED?** No spec amendment. **TASK-0372 amendment
minor** (wording only — the `//!` doc should explicitly name the
caller-side panic policy). **TASK-0373 amendment required** (pin the
panic message format; add a `border_resolver_assert_agent` private
helper).

**TASK IMPACT:**
- **TASK-0372** "Acceptance Criteria" — the bullet describing
  `materialize_agent`'s `Option` return is unchanged. Add a sentence
  after the last bullet: "Caller-side `None` handling is resolved by
  DC-B2 Option (A): `resolve_border_redex` panics on `None` with the
  message format fixed by TASK-0373."
- **TASK-0373** "Acceptance Criteria" — replace the bullet
  "Defensive-panic path when `materialize_agent` returns `None`: per
  DC-B2 (flagged in TASK-0372), panic with …" with:
  ```
  - Private helper `fn assert_agent(
        maybe: Option<(AgentId, Symbol)>,
        bid: u32,
        wid: WorkerId,
        side_label: &'static str,  // "side_a" or "side_b"
    ) -> (AgentId, Symbol)` that unwraps `maybe` via `expect` with
    the diagnostic message:
    "border_resolver: agent missing for border {bid} on worker {wid}
    side {side_label} — cache desync (DC-B1); check
    coordinator.rs::apply_worker_delta_to_cache (DC-B1 maintenance)".
  ```
- **TASK-0373** "Notes" — delete the DC-B2 bullet; replace with
  one-line citation to this verdict.

---

### DC-B3: Local reconnection delta type

**PICK:** **Option (A)** — **extend `BorderDelta` is the WRONG
answer; extend the NEW wire variant `Message::RoundStart` to carry a
separate `local_reconnections` field** on TOP of `border_deltas`,
`resolved_borders`, `new_borders`. The pure-core mirror
(`RoundStartDispatch`) gets the matching field. `BorderDelta` itself
stays untouched (its wire form is frozen by R33, 2 fields).

**WHY:** A CON-CON or DUP-DUP resolution at the coordinator re-
targets auxiliary ports. Some of those targets are "other side of
another border" (existing `BorderDelta` covers this via `border_id`
+ `new_target`); some are "a local agent in worker A's partition"
(pointing to `AgentPort(some_local_id, slot)`). The latter is NOT a
border — worker A has no `FreePort(bid)` to rewrite. It must be told:
"take the local agent at `PortRef::AgentPort(x, p)` and re-point its
port `(x, p)` to `PortRef::AgentPort(y, q)`".

The three sub-options of DC-B3 as flagged in the bundle index:

- **(A) extend `BorderDelta` with `local_reconnection`.** This
  CONTAMINATES the wire type — `BorderDelta`'s compact encoding
  (TASK-0344 lineage) depends on the 2-field shape. Adding a third
  optional field breaks the discriminant, forces a new compact
  variant, and bleeds local-reconnection semantics into a struct
  named for border connectivity. The §3.2 bundle's DC-1 rationale
  (keep `BorderDelta` at 2 fields for 1-byte DISCONNECTED wire
  economy) would be overturned.
- **(B) new `LocalReconnection` delta type ALONGSIDE `BorderDelta`
  in `Message::RoundStart`.** Clean separation: `border_deltas`
  carries only border updates; `local_reconnections` carries local
  port rewrites. Both are `Vec<...>`, both travel in the same wire
  variant. Worker applies `border_deltas` to `free_port_index` +
  local port array, and `local_reconnections` to the local port
  array only.
- **(C) virtual borders.** Encode a local reconnection as a border
  with both sides in the same worker. Requires worker to filter out
  "self-borders" at apply time; leaks into `BorderGraph` shape (the
  graph would have to carry same-worker entries that never produce
  a redex). Type abuse.

**(B) is the correct answer.** It has a wire cost (new field in
`Message::RoundStart`, owned by 2.26-A) but the cost is intrinsic:
the information has to cross the wire somehow, and packing it into a
dedicated vector is cleaner than smuggling it through `BorderDelta`.

The pure-core mirror (`RoundStartDispatch` in TASK-0375) gains a
parallel `local_reconnections: Vec<(PortRef, PortRef)>` field. The
per-worker `BorderResolution.worker_deltas` entry becomes richer: each
worker's "deltas" split into two vectors (border + local). The
simplest struct reshape is to rename the `worker_deltas` tuple type
from `(WorkerId, Vec<BorderDelta>)` to:

```rust
pub(crate) struct WorkerDeltas {
    pub(crate) border_deltas: Vec<BorderDelta>,
    pub(crate) local_reconnections: Vec<(PortRef, PortRef)>,
}
// BorderResolution.worker_deltas: Vec<(WorkerId, WorkerDeltas)>
```

Unequal auxiliary-target residence is the common case for CON-CON /
DUP-DUP / CON-DUP border resolutions: the target of a border-agent's
auxiliary port is ALMOST ALWAYS a local agent (otherwise the wire
was itself a border, which would have been reported separately in
the worker's `free_port_index`). So `local_reconnections` is load-
bearing on the common path, not a corner case.

**Cross-bundle impact:** 2.26-A's `Message::RoundStart` (discriminant
8) was specified in SPEC-19 R31 with 3 payload fields (`round`,
`border_deltas`, `resolved_borders`, `new_borders`). DC-B3 adds a
FOURTH field `local_reconnections: Vec<(PortRef, PortRef)>`. This is
an R31 spec text change, not a task change — **SPEC-19 AMENDMENT
REQUIRED** to R31 (discriminant 8 payload). Propagates to 2.26-D's
R38 invariant amendment if the invariant text references the field
list.

**AMENDMENT NEEDED?** **SPEC-19 R31 amendment required** (+1 field
on `RoundStart`). **TASK-0373 amendment required** (new
`WorkerDeltas` struct or equivalent; body produces both vectors).
**TASK-0375 amendment required** (`RoundStartDispatch` gains the
parallel field). **Cross-bundle memo to 2.26-A** (add the field to
`Message::RoundStart` and its serde + compact encoding).

**TASK IMPACT:**
- **SPEC-19 R31** — **this is the ONLY spec edit in this verdict.**
  The row for discriminant 8 (`RoundStart`) payload becomes:
  `round: u32, border_deltas: Vec<BorderDelta>, local_reconnections: Vec<(PortRef, PortRef)>, resolved_borders: Vec<u32>, new_borders: Vec<(u32, PortRef)>`
  — 5 fields. Task-updater performs the spec edit under the
  spec-critic's authority since the edit is a pure amendment, not a
  new requirement. Order: `local_reconnections` sits between
  `border_deltas` and `resolved_borders` for grouping
  (delta-application-order: apply border deltas first, then local
  rewrites, then remove resolved-border `FreePort` entries, then add
  new-border `FreePort` entries).
- **TASK-0373** "Key Types / Signatures" — replace the
  `BorderResolution` struct definition with:
  ```rust
  #[derive(Debug, Clone, Default)]
  pub(crate) struct WorkerDeltas {
      pub(crate) border_deltas: Vec<BorderDelta>,
      pub(crate) local_reconnections: Vec<(PortRef, PortRef)>,
  }

  #[derive(Debug, Clone, Default)]
  pub(crate) struct BorderResolution {
      pub(crate) worker_deltas: Vec<(WorkerId, WorkerDeltas)>,
      pub(crate) resolved_borders: Vec<(u32, WorkerId, WorkerId)>,  // see DC-B7
      pub(crate) new_borders: Vec<AddBorderEntry>,
  }
  ```
- **TASK-0373** "Acceptance Criteria" — the bullet for CON-CON /
  DUP-DUP dispatch now explicitly asserts that the resolver:
  "(i) for each auxiliary-port target of the consumed agent, checks
  whether the target is `AgentPort(local_id, _)` in the SAME
  partition as the consumed agent — if so, emits a
  `local_reconnections` entry `(PortRef, PortRef)` pointing to the
  other consumed agent's symmetric auxiliary-target (post-rule);
  otherwise, emits a `BorderDelta` updating the relevant border's
  endpoint on the consumed agent's worker side."
- **TASK-0373** "Notes" — delete the DC-B3 bullet; replace with
  citation to this verdict.
- **TASK-0375** "Key Types / Signatures" — `RoundStartDispatch`
  gains the new field `local_reconnections: Vec<(PortRef, PortRef)>`;
  `package_resolutions` folds `WorkerDeltas.local_reconnections` into
  `per_worker[w].local_reconnections`.
- **TASK-0375** "Acceptance Criteria" — add a new test:
  `package_resolutions_fans_local_reconnections_to_correct_worker` —
  a `BorderResolution` with one `(w=0, local_reconnections=[(p1, p2)])`
  entry lands exactly in `result[0].local_reconnections`; worker 1's
  output has an empty vector.
- **TASK-0376** "Acceptance Criteria" — the CON-CON / DUP-DUP test
  bullets now assert that the `package_resolutions` output's
  `local_reconnections` field has the expected entries when the
  auxiliary-target is local to each worker (fixture needs the helper
  to create 4 local agents, 2 of which are the border-principal
  agents and 2 of which are their auxiliary-target neighbours).
- **Cross-bundle memo to 2.26-A** — add the `local_reconnections`
  field to `Message::RoundStart` (SPEC-06 R4 serde + compact
  encoding per SPEC-18). The payload size grows by `vec_len + 2 *
  PortRef_compact * num_entries` per `RoundStart`; for the typical
  border redex the vector is `≤ 4` entries (CON-CON has at most 2
  auxiliary-port reconnections per worker; DUP-DUP same; CON-DUP
  up to 4 per worker).
- **Cross-bundle memo to 2.26-C** — worker-side `apply_deltas` in
  `run_grid_delta` must apply `local_reconnections` AFTER
  `border_deltas` and BEFORE `resolved_borders` / `new_borders`.
  Application is `net.connect(p1, p2)` for each `(p1, p2)` entry.

---

### DC-B4: Local/coordinator race window

**PICK:** **Option (B)** — worker guarantees by construction that a
border wire's agent is either **consumed** OR **exported as a
principal-port endpoint** in the same round, NEVER both. The worker
enforces this via a **pre-reduction border-pinning** pass:
before running `reduce_all` at round N, the worker marks every
local agent `A` such that `FreePort(border_id)` appears as `A`'s
principal-port (`A.ports[0] == FreePort(border_id)` for some `bid`)
as **non-reducible for round N** — those agents may only participate
in border redexes (resolved at the coordinator), never in local
redexes. Local `reduce_all` skips them.

**WHY:** The DC-B4 window is real: if the worker locally reduces
agent `A` at round N (where `A`'s principal port connects to
`FreePort(bid)`), then `A` is consumed, and `bid`'s worker-A side
becomes `DISCONNECTED`. In the SAME round, if worker A also reported
`has_border_activity = true` for `bid` in round N-1's result (because
`A`'s principal port was there), the coordinator has already
DETECTED the border redex via `detect_border_redexes` and is about
to call `resolve_border_redex(&mut graph, &cache, bid)` at round N's
boundary — at which point `cache[worker_A]` still shows `A` (because
the coordinator hasn't applied round N's deltas yet), but the "true"
post-round-N state has `A` consumed.

The `materialize_agent` call will return `Some((A, A.symbol))` from
the cache (which is CORRECT for the round N-1 snapshot the
coordinator is operating on), and the resolver will emit deltas that
ASSUME `A` is consumed by the coordinator's resolution. But `A` is
ALREADY consumed by the worker's local reduction. The worker
receives the deltas and applies them to... an agent that doesn't
exist. Undefined behaviour.

The three flagged options:

- **(A) coordinator detects double-consumption and drops.** Requires
  the coordinator to distinguish "agent consumed by me" from "agent
  consumed by worker", which means an extra round-trip ack. Adds
  latency and protocol surface.
- **(B) worker guarantees by construction — pre-reduction pinning.**
  The worker knows which of its agents are border-principal
  (trivial: scan `free_port_index` values; any value
  `PortRef::AgentPort(id, 0)` pins agent `id`). Pinned agents are
  excluded from the local redex queue for that round. Worker's
  `reduce_all` becomes "reduce all local redexes whose neither
  agent is pinned". When the coordinator resolves the border redex
  at round N boundary, the worker's state STILL has `A` at round
  N's end (not consumed). Cache is consistent. Deltas apply cleanly.
- **(C) undefined behaviour.** Rejected — leaking UB into the spec
  is not acceptable for a formal protocol.

**(B) is correct.** The cost is negligible: pinning is O(|borders
per worker|) work per round — same complexity as `free_port_index`
iteration, which the worker does anyway. The pinning is a
**deterministic, local-only** computation; no new messages, no new
fields, no new round trips.

**Cascading concern on CON-DUP commutation emergence:** if worker A
performs a CON-DUP commutation locally in round N on agents `B` (Con)
and `C` (Dup) where `C` had its principal port `C.0` connected to a
local auxiliary port `X.p`, and the commutation creates 4 new agents
`B', B'', C', C''` — one of which (say `B'`) gets its principal port
connected to `FreePort(some_border_id)` (because one of `B`'s
auxiliary ports had pointed to that FreePort). **This creates an
EMERGENT border-principal agent mid-round**, which must then be
treated as pinned for any FUTURE round that re-examines the border.

The spec SPEC-19 §3.5 R39 D3c already handles this: "Emergent border
redexes created by CON-DUP commutation during local reduction MUST
be captured by the worker's border delta report." The worker reports
the new endpoint in its `RoundResult.border_deltas`; the coordinator
updates `BorderGraph` in the following round. **No same-round
double-consumption is possible** because `B'` (the emergent agent)
cannot itself be a redex in round N (it was just created; the
commutation emits `reduce_all` to re-consider the queue, but `B'`'s
principal port is now a FreePort, not another agent — so no local
redex is formed). `B'` becomes a candidate border-principal agent
for round N+1; pinning in round N+1 covers it.

**Pinning scheme formal amendment:** SPEC-19 §3.5 should gain a new
invariant (or amend D6) to document the border-pinning discipline.
The amendment is:

```
R40c (NEW). In delta mode, a worker's local `reduce_all` at round
N MUST exclude any agent whose principal port is connected to
`FreePort(bid)` for some `bid` in `self.free_port_index`. These
agents are "border-pinned": their only reducible role at round N is
as one side of a border redex, resolved at the coordinator. The
worker's border delta report MUST NOT report a "consumed" event
for a pinned agent unless the resolution delta from round N-1 told
the worker to remove it.
```

This is a D6 amendment (progress guarantee). It does not break v1
(v1 has no `free_port_index`-based pinning because v1 has no
stateful worker; v1's `run_grid` does full merge + full redex
scan).

**AMENDMENT NEEDED?** **SPEC-19 AMENDMENT REQUIRED** — new R40c in
§3.5 invariant amendments. This edit belongs to bundle 2.26-D (which
owns §3.5 R38-R40 per the bundle-index scope), NOT this bundle.
**Cross-bundle memo to 2.26-D** specifying the R40c text. **Cross-
bundle memo to 2.26-C** specifying the pinning logic in
`run_grid_delta`. **TASK-0372 Context update** to reference the
pinning invariant.

**TASK IMPACT:**
- **TASK-0372** "Context" — add after the new DC-B1 sentence:
  "Per spec-critic DC-B4 + SPEC-19 §3.5 R40c (amendment owned by
  2.26-D), the resolver assumes border-adjacent agents are pinned
  in the worker's local `reduce_all` for the round in which they
  are border-principal. No race window exists; `materialize_agent`
  finding `None` is a cache-maintenance bug (DC-B1), not a race
  artefact."
- **TASK-0373** "Design Choices Flagged" — delete the DC-B4 bullet;
  replace with one-line citation to this verdict.
- **Cross-bundle memo to 2.26-D**: add R40c to SPEC-19 §3.5 (text
  above); update the G1 amendment (R38) if its correctness
  argument lists the specific progress-guarantee discipline
  (border-pinning is a new mechanism to cite).
- **Cross-bundle memo to 2.26-C**: in `run_grid_delta` worker loop,
  before calling `reduce_all`, filter the redex queue by pinning
  (iterate `partition.free_port_index`, collect principal-agent
  IDs into `pinned: HashSet<AgentId>`, pass to `reduce_all` as a
  skip-set OR pre-filter the redex queue). Implementation hint:
  add `pub fn reduce_all_with_skip(&mut self, skip: &HashSet<AgentId>)`
  as a variant; v1's `reduce_all` stays unchanged (called with an
  empty skip-set or by a trivial wrapper).

---

### DC-B5: New AgentId allocation for CON-DUP expansion

**PICK:** **Option (B)** — workers allocate new agent IDs locally
from their own `IdRange` after receiving a "create agents" directive.
Two-phase protocol: coordinator proposes the commutation (via a new
field `pending_commutations: Vec<PendingCommutation>` in
`Message::RoundStart`); worker allocates 2 agent IDs from its
`IdRange`, creates the agents locally, reports the IDs back in
`Message::RoundResult.minted_agents: Vec<(AgentId, Symbol)>`. The
coordinator finalizes the border-graph updates using the reported IDs
in the FOLLOWING round's `RoundStart` (`border_deltas` and
`new_borders` reference the concrete IDs).

**WHY:** Three options:

- **(A) coordinator picks from a reserved high range.** Violates the
  SPEC-04 invariant that `Partition.id_range` owns the agent-ID
  allocation space for each worker. The coordinator injecting IDs
  like `u32::MAX - N` breaks the partition-agent-ID dense layout
  assumption and contaminates the arena (`Net.agents` is a `Vec<_>`
  indexed by agent ID as `usize`; `u32::MAX - N` as an index creates
  a 16 GB arena).
- **(B) workers allocate locally, echo back (2-phase).** Respects
  `id_range`. Cost: one extra round of back-and-forth for every
  CON-DUP resolution. The coordinator cannot finalize the CON-DUP
  border graph in round N; it finalizes in round N+1. This is
  slightly worse than single-phase (A) but matches reality of
  distributed ID allocation.
- **(C) coordinator uses a shared "coordinator-reserved" range
  outside any worker's range.** Same problem as (A) for Rust arena
  layout; additionally requires worker-side arena extension
  ("create agent X at slot Y not in your `id_range`") — breaks the
  `id_range` invariant that every worker's arena slot ∈ its range.

**(B) is correct.** The two-phase cost is not as bad as it sounds:
CON-DUP resolutions are rare (they require two cross-partition
agents whose principal ports connect AND one is Con while the other
is Dup). For the Church-numeral benchmarks (Profile B: expansion +
collapse), CON-DUP commutations dominate inside the workers but are
vanishingly rare at the borders (borders are typically the "skeleton"
of the parallelization, and skeleton redexes are Anni/Void). So the
2-phase cost is a log factor on a marginal case — acceptable.

**Concrete 2-phase flow:**

1. **Round N (detection):** coordinator calls
   `resolve_border_redex` for CON-DUP border `bid`. Resolver returns
   a `BorderResolution` with:
   - `new_borders: Vec<AddBorderEntry>` still included, BUT with
     `side_a` / `side_b` populated with placeholder tokens
     (e.g. `PortRef::AgentPort(AGENT_ID_PENDING_A1, 0)`) —
     resolved next round.
   - A new field `pending_commutations: Vec<PendingCommutation>` on
     `BorderResolution` (and propagated through `RoundStartDispatch`)
     that tells each of the two workers: "create `k` agents of
     these symbols; report back their IDs in your
     `RoundResult.minted_agents`".
   - `resolved_borders: [(bid, worker_a, worker_b)]` — the original
     border is resolved (removed from the graph) at round N.
2. **Round N+1 (workers create agents):** each involved worker
   receives `pending_commutations` in `RoundStart`, allocates IDs
   from its `id_range`, creates the agents, assembles the local
   wiring (local auxiliary-port connections), and echoes the newly-
   minted agent IDs in `RoundResult.minted_agents`. The worker does
   NOT treat these agents as reducible in round N+1 if their
   principal port is a `FreePort(new_bid)` (pinning, per DC-B4).
3. **Round N+2 (coordinator finalizes border graph):** coordinator
   reads `minted_agents` from both workers' `RoundResult` (collected
   at round N+1's boundary), constructs the real `AddBorderEntry`
   values (with concrete `AgentId`s), calls
   `graph.add_border_states(entries)`. The new borders are now live
   and eligible for redex detection in round N+2 onward.

**The coordinator does NOT block on the echo.** The BSP loop
continues; round N+1 happens as usual; the `add_border_states` call
just happens at round N+2's start. The only cost is a 1-round
latency on "when can this newly created border contribute to a
redex", which is bounded and acceptable.

**Cross-bundle impact:**

- **2.26-A R31/R32**: both `Message::RoundStart` (add
  `pending_commutations: Vec<PendingCommutation>`) and
  `Message::RoundResult` (add `minted_agents: Vec<(AgentId, Symbol)>`)
  gain one field each. New type `PendingCommutation` defined in
  `protocol/types.rs`:
  ```rust
  pub struct PendingCommutation {
      pub commutation_id: u64,  // correlation handle C->W
      pub target_agents: Vec<Symbol>,  // usually Con-Con or Dup-Dup (2 entries per worker)
      pub local_wiring: Vec<(u8, PortRef)>,  // hint: how to wire the new agents' auxiliary ports
  }
  ```
- **2.26-D R38 (G1 amendment)**: the amendment text must
  acknowledge that border-graph finalization is NOT immediate on
  CON-DUP resolutions — it is deferred by 1 round. The
  "recoverability" property (R38 text) holds modulo this 1-round
  delay; the proof obligation grows slightly (DISC-011 / ARG-005
  work).

- **SPEC-19 §3.2 R15 part 3**: the `add_border_states` primitive
  already accepts `Vec<AddBorderEntry>` (shipped by §3.2 bundle's
  TASK-0364 per DC-4). This bundle's resolver does NOT call
  `add_border_states` directly on CON-DUP — the deferred
  finalization (round N+2) is the call site. That call site lives
  in 2.26-C's coordinator BSP loop, not in this bundle. **The
  resolver's `new_borders` field is renamed to
  `pending_new_borders` and carries placeholder IDs until
  finalization.**

**AMENDMENT NEEDED?** **NO spec amendment** to the text of R15 part
3 (semantics unchanged). **SPEC-19 AMENDMENT REQUIRED** to R31 /
R32 (new fields on `Message::RoundStart` and `Message::RoundResult`).
**TASK-0374 amendment required** (`pending_commutations` emission;
`pending_new_borders` placeholder). **Cross-bundle memo to 2.26-A**
(new message fields), **to 2.26-C** (2-phase finalization in
`run_grid_delta`), **to 2.26-D** (G1 amendment note).

**TASK IMPACT:**
- **SPEC-19 R31 row for `RoundStart`** — add
  `pending_commutations: Vec<PendingCommutation>` between
  `new_borders` and end. The row becomes 6 fields (including the
  DC-B3 `local_reconnections` field added above).
- **SPEC-19 R32 row for `RoundResult`** — add
  `minted_agents: Vec<(AgentId, Symbol)>` between
  `has_border_activity` and end.
- **TASK-0374** "Acceptance Criteria" — replace the bullet "New
  `BorderIdAllocator` struct … seeded from `max(existing_border_ids)
  + 1`" with:
  ```
  - CON-DUP resolution does NOT allocate concrete AgentIds or
    concrete new-border IDs at coordinator resolution time. Instead,
    the resolver returns:
      * `pending_commutations: Vec<PendingCommutation>` — one entry
        per involved worker (usually 2 — one for worker_a, one for
        worker_b), each listing the symbols to mint (usually 2 per
        worker) and the local wiring hints for the new agents'
        auxiliary ports.
      * `pending_new_borders: Vec<PendingNewBorder>` — one entry per
        new cross-partition wire, carrying placeholder token handles
        (`CommutationId` + `agent_slot_in_commutation`) that resolve
        to concrete `(AgentId, Symbol)` pairs in round N+2 after
        `minted_agents` is reported.
    The `BorderIdAllocator` (for new border IDs, u32) IS still in
    scope — border IDs are coordinator-owned and can be allocated
    synchronously.
  ```
- **TASK-0374** "Key Types / Signatures" — add new types:
  ```rust
  pub(crate) struct PendingCommutation {
      pub(crate) commutation_id: u64,
      pub(crate) worker: WorkerId,
      pub(crate) target_symbols: Vec<Symbol>,
      pub(crate) local_wiring: Vec<(u8, PortRef)>,
  }
  pub(crate) struct PendingNewBorder {
      pub(crate) border_id: u32,
      pub(crate) side_a: PendingPortRef,
      pub(crate) side_b: PendingPortRef,
      pub(crate) worker_a: WorkerId,
      pub(crate) worker_b: WorkerId,
  }
  pub(crate) enum PendingPortRef {
      Concrete(PortRef),
      Pending { commutation_id: u64, agent_slot: u8, port_slot: u8 },
  }
  ```
- **TASK-0374** "Notes" — delete the DC-B5 bullet; replace with
  citation to this verdict.
- **TASK-0375** — `RoundStartDispatch` gains
  `pending_commutations: Vec<PendingCommutation>`; `package_resolutions`
  folds by worker.
- **TASK-0376** "Acceptance Criteria" — the `test_con_dup_border_redex_creates_new_borders`
  assertion now checks:
  (i) `resolution.pending_commutations.len() == 2` (one per worker);
  (ii) `resolution.new_borders.is_empty()` (concrete new borders not
  yet finalized);
  (iii) `resolution.pending_new_borders.len() <= 4` (up to 4 new
  cross-partition wires).
  The round-N+2 finalization is NOT tested here — that's 2.26-C's
  BSP-loop integration-test territory.
- **Cross-bundle memo to 2.26-A**: add `PendingCommutation` to
  `protocol/types.rs` with serde + compact encoding; add the new
  fields to `Message::RoundStart` and `Message::RoundResult`.
- **Cross-bundle memo to 2.26-C**: add coordinator-side 2-phase
  finalization in `run_grid_delta`; store pending commutations in
  `HashMap<u64, PendingCommutation>`; on `minted_agents` receipt,
  look up by `commutation_id`, resolve `PendingPortRef`s, call
  `graph.add_border_states(real_entries)`.
- **Cross-bundle memo to 2.26-D**: the G1 amendment proof
  obligation now includes the 1-round deferral on CON-DUP border
  finalization. DISC-011 / ARG-005 work should cite this.

---

### DC-B6: CON-ERA / DUP-ERA border preservation

**PICK:** **Option (A)** — **preserve** the border. The
`FreePort(bid)` entry stays; its endpoint is re-pointed from the
consumed agent's auxiliary-port to the new ERA's principal port.
Implementation: `graph.apply_deltas(worker_id, &[(bid, new_target)])`.
No new border ID needed.

**WHY:** SPEC-03 §4.1.4 / §4.1.5 (CON-ERA, DUP-ERA) says: "ERA
annihilates the principal port of the other agent; on each auxiliary
port of the non-ERA agent, a NEW ERA is connected." The new ERAs are
real agents with principal ports that replace the former auxiliary-
port connections. If the former auxiliary-port target was a FreePort
(i.e. a border), the border is preserved — the FreePort still exists
on the same worker, but its remote-side pointer (the coordinator's
view) now points to the new ERA's principal port instead of the old
agent's auxiliary port.

Option (B) — remove the border — would be correct only if the
former connection involved the CONSUMED agent's PRINCIPAL port. But
the principal port of the non-ERA agent is what was consumed BY the
ERA rule; its auxiliary-port connections survive in the new ERAs.
The border in question (DC-B6's subject) is on an AUXILIARY port of
the consumed agent, and the wire survives the reduction.

Semantic truth (SPEC-03 R14 reduction preserves connectivity):

- Consumed non-ERA agent `N` has auxiliary port `N.p` (p ∈ {1, 2})
  connected to `FreePort(bid)`. The border's other side is on some
  other worker; the BorderGraph sees it as `AgentPort(N, p)`-vs-
  (other worker's side).
- Post-reduction: new ERA `E_p` has principal port `E_p.0` connected
  to whatever `N.p` was connected to — i.e. to `FreePort(bid)`.
- Wire `bid` is unchanged. The coordinator's BorderGraph side for
  this worker must now read `AgentPort(E_p, 0)` instead of
  `AgentPort(N, p)`.
- `is_redex` recomputes: the OTHER side's principal-port status may
  or may not create a new border redex (if the other side's agent
  was also a principal, yes; otherwise no).

This is **exactly** what `apply_deltas((bid, AgentPort(E_p, 0)))`
does. The primitive is already shipped (§3.2 R11, TASK-0362). No
new border ID, no `add_border_states` call.

**(A) is correct.** Option (B) (remove the border) would delete a
wire that still exists, violating D3 (border completeness).

**AMENDMENT NEEDED?** No spec amendment. **TASK-0374 amendment
minor** (pin the CON-ERA / DUP-ERA behaviour in the acceptance
criteria; delete the DC-B6 ambiguity bullet).

**TASK IMPACT:**
- **TASK-0374** "Acceptance Criteria" — CON-ERA / DUP-ERA bullet
  currently reads "CON-ERA / DUP-ERA bodies: similar but only 2 new
  ERA agents, no new borders …". Replace with:
  "CON-ERA / DUP-ERA bodies: produce 2 new ERA agents (via
  `pending_commutations` per DC-B5 — the worker mints them locally).
  Each new ERA's principal port inherits the non-ERA agent's former
  auxiliary-port target. If the former target was `FreePort(bid)`
  (i.e. border `bid`), the resolver emits
  `BorderDelta { border_id: bid, new_target: AgentPort(new_era_id, 0) }`
  on the relevant worker's side (via `apply_deltas` semantics).
  **NO new border ID is allocated** for these — the existing border
  survives. If both auxiliary-target connections were borders
  (rare), both are updated. The resolver does NOT call
  `graph.add_border_states` for CON-ERA / DUP-ERA under any
  circumstance. The R15 part 3 `add_border_states` primitive
  applies ONLY to CON-DUP commutation."
- **TASK-0374** "Notes" — delete the DC-B6 bullet; replace with
  citation to this verdict.
- **TASK-0376** "Acceptance Criteria" — the `test_con_era_border_redex_removes_border`
  test name is misleading (the border may NOT be removed if the
  ERA inherits a border target). Rename to
  `test_con_era_border_redex_resolves_principal_border`, and have
  it assert:
  (i) `graph.borders.contains_key(&0) == false` — the ORIGINAL
      border (the one that was principal-principal and triggered
      resolution) IS removed.
  (ii) Any auxiliary-port-borders of the consumed CON/DUP agent
      remain in `graph.borders` with updated endpoints.
  Same for `test_dup_era_border_redex_resolves_principal_border`.

---

### DC-B7: `resolved_borders` worker attribution

**PICK:** **Option (A)** — embed `(bid, worker_a, worker_b)` triples
in `BorderResolution.resolved_borders`. The `package_resolutions`
function (TASK-0375) fans the triple into both workers' output.
No extra `BorderGraph` reference needed.

**WHY:** Three options:

- **(A) triples in `resolved_borders`.** Clean, self-contained,
  single pass. 8 bytes per entry (u32 + u32 + u32 padded) vs 4
  bytes (u32) — negligible memory cost.
- **(B) pass `BorderGraph` to `package_resolutions`.** Requires
  the graph to be accessible at packaging time, but `remove_border`
  has already been called in the resolver, so `worker_a` / `worker_b`
  info is GONE from the graph. A "snapshot before removal" would
  require another data structure. Inefficient and error-prone.
- **(C) embed in `worker_deltas` and filter by appearance.** Relies
  on the invariant that both workers appear in `worker_deltas` for
  every resolved border. For CON-CON / DUP-DUP, this is true
  (both workers' sides get deltas). For ERA-ERA, it is FALSE — ERA
  has no auxiliary ports, so no deltas are emitted to either
  worker, but both workers must still remove their FreePort
  entries. Option (C) fails on ERA-ERA.

**(A) is correct.** The `BorderState` already carries `worker_a` /
`worker_b` (§3.2 R9), so the resolver has the info locally when it
calls `graph.remove_border(bid)`; it just needs to pass it into the
`BorderResolution`.

**AMENDMENT NEEDED?** No spec amendment. **TASK-0373 amendment
required** (change `resolved_borders: Vec<u32>` →
`Vec<(u32, WorkerId, WorkerId)>`). **TASK-0375 amendment required**
(`package_resolutions` fans the triple into both per-worker
`resolved_borders` buckets; pure-core
`RoundStartDispatch.resolved_borders: Vec<u32>` drops the worker
fields — each per-worker output only needs the u32 IDs).

**TASK IMPACT:**
- **TASK-0373** "Key Types / Signatures" — `BorderResolution.resolved_borders`
  type changes from `Vec<u32>` to `Vec<(u32, WorkerId, WorkerId)>`.
  (Already reflected in the DC-B3 struct example above.)
- **TASK-0373** "Acceptance Criteria" — the bullet "Invariant
  check: after resolving …" gains a sub-bullet:
  "for every `(bid, worker_a, worker_b)` in `resolution.resolved_borders`,
  the triple matches `graph.borders[bid].worker_a / worker_b` AT
  THE MOMENT the resolver computes it (i.e. just before
  `graph.remove_border(bid)`)."
- **TASK-0373** "Notes" — delete the DC-B7 bullet (it's currently
  in TASK-0375; deletion applies there); add a reference line.
- **TASK-0375** "Acceptance Criteria" — the bullet
  "`BorderResolution.resolved_borders` → folded into the
  `resolved_borders` field of EACH worker whose side was consumed"
  now reads:
  "`BorderResolution.resolved_borders: Vec<(u32, WorkerId, WorkerId)>`
  → fan into both workers' `RoundStartDispatch.resolved_borders:
  Vec<u32>`. For each triple `(bid, wa, wb)`,
  `per_worker[wa].resolved_borders.push(bid)` AND
  `per_worker[wb].resolved_borders.push(bid)`. If `wa == wb` (a
  worker's two sides of the same logical wire — not a typical
  border but theoretically possible), push once."
- **TASK-0375** "Notes" — delete the DC-B7 bullet; replace with
  citation to this verdict.

---

### DC-B8: Pure-core guard scope

**PICK:** **Option (C)** — **shared helper** in a new
`merge/internal/pure_core_guard.rs` (test-only module) that any
pure-core file can reference via a one-line
`#[cfg(test)] mod pure_core_guard_test` test. This bundle ships the
helper AND the `border_resolver.rs` test; `border_graph.rs`'s
existing test is ALSO migrated to use the shared helper.

**WHY:** Three options:

- **(A) only `border_resolver.rs`.** Minimal scope. But DC-B9's
  forbidden-imports list is now complex enough (3+ prefixes + the
  DC-B9 additions) that duplicating the check per file is
  unmaintainable — a future pure-core file would need 30 LoC of
  test-copy each.
- **(B) extend the guard to every `merge/*.rs` file.** Correct in
  intent but requires enumerating files either by compile-time
  `include_str!(…)` incantations (one line per file) or by a
  `build.rs` scan (bundle-level refactor). Bundle scope is TIGHT
  per 2.26-B forbidden-files list — `build.rs` changes are out of
  scope.
- **(C) shared helper + opt-in per file.** Ships the
  infrastructure now; each pure-core file adds ~3 LoC to opt in.
  `border_graph.rs` already has a similar pattern per §3.2 bundle
  — the shared helper unifies both and pays back immediately.

**(C) wins on long-term cost.** Initial LoC is slightly higher
(~20 LoC for the helper + ~3 LoC per adopting file vs ~15 LoC
per file standalone) but subsequent files drop to ~3 LoC.

The helper signature:

```rust
// merge/internal/pure_core_guard.rs (cfg(test) only)
#[cfg(test)]
pub(crate) fn assert_no_forbidden_imports(src: &str, file_name: &str) {
    const FORBIDDEN_PREFIXES: &[&str] = &[
        "use tokio",
        "use async_trait",
        "use crate::protocol",
        "use crate::coordinator",   // DC-B9
        "use crate::worker",         // DC-B9
    ];
    for line in src.lines() {
        let trimmed = line.trim_start();
        for prefix in FORBIDDEN_PREFIXES {
            assert!(
                !trimmed.starts_with(prefix),
                "R19 violation: {file_name} imports {prefix}",
            );
        }
    }
}
```

Each pure-core file then has:

```rust
#[cfg(test)]
mod pure_core_guard_test {
    #[test]
    fn pure_core_no_forbidden_imports() {
        crate::merge::internal::pure_core_guard::assert_no_forbidden_imports(
            include_str!("border_resolver.rs"),
            "border_resolver.rs",
        );
    }
}
```

**Forbidden-files list concern:** the bundle's forbidden-files list
(TASK-0377 Files section) restricts this bundle to
`border_resolver.rs` only. Adding `merge/internal/pure_core_guard.rs`
is a NEW file, not a forbidden modification — allowed. Adding a
one-line-per-file opt-in to `border_graph.rs` IS a forbidden
modification (this bundle does not own the §3.2 file). Fix: 2.26-B
ships the helper + adopts it in `border_resolver.rs` only; a
follow-up task under item 2.38 (a new ROADMAP entry: "pure-core
guard consolidation") migrates `border_graph.rs`.

**AMENDMENT NEEDED?** No spec amendment. **TASK-0377 amendment
required** (new helper file + opt-in pattern). **ROADMAP addition
needed** (new item 2.43: "pure-core guard consolidation") — owner
task-splitter or orchestrator at convenience.

**TASK IMPACT:**
- **TASK-0377** "Files to Create/Modify" — add:
  ```
  - `relativist-core/src/merge/internal/mod.rs` — **create** (new
    internal submodule; contains the guard helper).
  - `relativist-core/src/merge/internal/pure_core_guard.rs` —
    **create** (~20 LoC, #[cfg(test)] only; the `assert_no_forbidden_imports`
    function).
  - `relativist-core/src/merge/mod.rs` — **modify**: add
    `#[cfg(test)] pub(crate) mod internal;` (1 line).
  ```
- **TASK-0377** "Key Types / Signatures" — replace the inline
  `#[test] fn pure_core_no_forbidden_imports()` body with the
  shared-helper opt-in pattern shown above.
- **TASK-0377** "Notes" — delete the DC-B8 bullet; replace with
  citation to this verdict. Add a bullet: "Follow-up: a future item
  (ROADMAP 2.43 — pure-core guard consolidation) migrates
  `border_graph.rs` and any future pure-core files to use the
  shared helper."
- **TASK-0377** "Dependencies Context" — the helper has NO
  external dependencies; `#[cfg(test)]` ensures it does not enter
  the release binary.
- **ROADMAP memo to orchestrator** — append item 2.43 to
  `docs/ROADMAP.md` v2 section:
  ```
  ### 2.43 Pure-core import-discipline guard consolidation
  Migrate all pure-core files in `merge/` (and any future pure-core
  modules) to use the shared `pure_core_guard::assert_no_forbidden_imports`
  helper shipped in 2.26-B TASK-0377. Per DC-B8 of
  `docs/spec-reviews/SPEC-19-section-3.3-2.26B-design-choices-2026-04-17.md`.
  Out of scope for 2.26-B (forbidden-files list).
  ```

---

### DC-B9: Pure-core transitive import forbid

**PICK:** **Option (A)** — forbid `use crate::coordinator` AND
`use crate::worker` from `border_resolver.rs` (and all other pure-
core files that opt into the helper per DC-B8).

**WHY:** R19's intent is "no I/O dependency, no async dependency, no
wire-protocol dependency". The direct blockers are `tokio`,
`async_trait`, `crate::protocol`. But the `crate::coordinator` and
`crate::worker` modules both IMPORT `crate::protocol::*` and use
`tokio` extensively — so a pure-core file that imports from them
**transitively** acquires the forbidden dependencies.

Option (B) — "no, it's conceptually OK since the coordinator CALLS
INTO this module, not the other way around" — is wrong **because the
static import graph is what matters for the compilation-unit async-
dependency contract**, not the runtime call direction. If
`border_resolver.rs` does `use crate::coordinator::CoordinatorState;`
to reference a type defined in `coordinator.rs`, Rust compiles the
`merge` module with a visibility edge to `coordinator.rs`, and
`rustc`'s async-runtime pollution analysis (and the `cargo-deny`
future-lint sensibility) treats the pure-core file as part of the
async subgraph. The R19 invariant breaks in practice.

The concern is NOT theoretical: at the time 2.26-C ships, it will
be tempting for a developer to write
`use crate::coordinator::delta_state::WorkerDeltaState;` in
`border_resolver.rs` to reference a shared type. The R19 intent is
that such a type should live in `merge/` (or be a pure type in
`coordinator.rs` that's re-exported from `merge/`), never the
reverse. The guard enforces the dependency direction.

**(A) is correct.** The `FORBIDDEN_PREFIXES` list grows to 5:
`use tokio`, `use async_trait`, `use crate::protocol`, `use crate::coordinator`,
`use crate::worker`. The `merge/internal/pure_core_guard.rs` helper
per DC-B8 encodes this list once; every pure-core file benefits.

**AMENDMENT NEEDED?** No spec amendment (R19's `MUST NOT depend on
tokio, async, or I/O` already covers the transitive case in spirit).
**TASK-0377 amendment required** (forbidden-prefix list extended).

**TASK IMPACT:**
- **TASK-0377** "Acceptance Criteria" — the shared helper's
  `FORBIDDEN_PREFIXES` array contains 5 entries (already shown under
  DC-B8).
- **TASK-0377** "Notes" — delete the DC-B9 bullet; replace with
  citation to this verdict.

---

## Summary table (compact)

| DC | Pick | Spec edit? | Task files touched | Cross-bundle |
|----|------|-----------|---------------------|--------------|
| B1 | (A) coordinator cache, resolver takes `&[Partition]` | No | TASK-0372 | 2.26-C (cache maintenance) |
| B2 | (A) caller-side panic via `assert_agent` helper; `materialize_agent` keeps `Option` return | No | TASK-0372, TASK-0373 | None |
| B3 | (B, renamed) new `local_reconnections` field on `RoundStart`; `BorderDelta` unchanged | **R31 amended (new field)** | TASK-0373, TASK-0375, TASK-0376 | 2.26-A (new field), 2.26-C (apply order) |
| B4 | (B) worker border-pinning; no race window | **R40c (new)** in §3.5 (owned by 2.26-D) | TASK-0372, TASK-0373 | 2.26-D (R40c), 2.26-C (pinning logic) |
| B5 | (B) workers allocate AgentIds, 2-phase echo; new `pending_commutations` + `minted_agents` fields | **R31, R32 amended** | TASK-0374, TASK-0375, TASK-0376 | 2.26-A (new fields + types), 2.26-C (2-phase finalization), 2.26-D (G1 amendment note) |
| B6 | (A) preserve auxiliary-border via `apply_deltas`; NO new border ID for CON-ERA / DUP-ERA | No | TASK-0374, TASK-0376 | None |
| B7 | (A) `(bid, worker_a, worker_b)` triples in `BorderResolution.resolved_borders` | No | TASK-0373, TASK-0375 | None |
| B8 | (C) shared helper in `merge/internal/pure_core_guard.rs`; opt-in per file | No | TASK-0377; new item 2.43 in ROADMAP | None |
| B9 | (A) forbid `crate::coordinator` and `crate::worker` too | No | TASK-0377 | None |

## SPEC-19 amendments required before TESTS

**Count: 3.** (Plus 1 invariant amendment owned by 2.26-D.)

1. **R31 `RoundStart` payload**: add `local_reconnections: Vec<(PortRef, PortRef)>` (DC-B3)
   and `pending_commutations: Vec<PendingCommutation>` (DC-B5).
2. **R32 `RoundResult` payload**: add `minted_agents: Vec<(AgentId, Symbol)>` (DC-B5).
3. **(2.26-D territory, but specified here)** Add R40c to §3.5: border-pinning invariant (DC-B4).

Spec-critic does NOT edit spec files directly per local convention
(only task-splitter / task-updater edit `docs/backlog/`; spec edits
go through the 2.26-A / 2.26-D owners). The task-updater dispatched
after this review should record the R31 / R32 edits as
cross-bundle memos to 2.26-A (NOT apply them — 2.26-A's
spec-critic stage will re-ratify); and the R40c edit as a
cross-bundle memo to 2.26-D.

## Stage 1.5 verdict

**Stage 1.5 spec-critic complete.** TESTS stage is **unblocked in
principle**, but the task-updater MUST apply:

1. TASK-0372 amendments (DC-B1 context + Notes; DC-B2 caller-side
   panic reference; DC-B4 pinning reference).
2. TASK-0373 amendments (DC-B3 struct reshape +
   `local_reconnections` emission; DC-B2 `assert_agent` helper;
   DC-B7 triple in `resolved_borders`).
3. TASK-0374 amendments (DC-B5 `pending_commutations` /
   `pending_new_borders` emission, remove `BorderIdAllocator` for
   agent IDs; DC-B6 CON-ERA / DUP-ERA `apply_deltas` path).
4. TASK-0375 amendments (DC-B3 `RoundStartDispatch` new field;
   DC-B5 `pending_commutations` fanning; DC-B7 triple fanning).
5. TASK-0376 amendments (DC-B3 + DC-B5 + DC-B6 assertion changes;
   one test rename).
6. TASK-0377 amendments (DC-B8 shared helper; DC-B9 forbidden
   prefixes extended).
7. Bundle-index amendments (`SPEC-19-section-3.3-coordinator-dispatch-tasks.md`):
   update each DC-Bx line with the spec-critic verdict; advance
   Stage 1.5 to complete; reference this spec-review file.

**Cross-bundle memos** (task-updater emits; target bundle's
spec-critic re-ratifies):

- **2.26-A**: add two new fields to `Message::RoundStart`
  (`local_reconnections`, `pending_commutations`), one new field to
  `Message::RoundResult` (`minted_agents`), plus new type
  `PendingCommutation` in `protocol/types.rs`. Serde + compact
  encoding required.
- **2.26-C**: add cache maintenance (DC-B1), border-pinning in
  `reduce_all` (DC-B4), 2-phase CON-DUP finalization (DC-B5),
  delta application ordering (DC-B3).
- **2.26-D**: add R40c invariant to SPEC-19 §3.5 (DC-B4
  border-pinning); update G1 amendment proof obligation to
  acknowledge 1-round deferral on CON-DUP finalization (DC-B5).

After task-updater applies the 7 in-bundle amendments and emits the
3 cross-bundle memos, test-generator may proceed with
TEST-SPEC-0372..0377 for Stage 2.

### Additional observations (not DC verdicts but flagged here)

1. **`BorderIdAllocator` downgrade:** DC-B5 Option (B) means
   CON-DUP's new-agent IDs are worker-allocated, NOT coordinator-
   allocated. But the NEW BORDER IDs (u32) remain coordinator-
   allocated (the coordinator is the single source of truth for
   `border_id` uniqueness). TASK-0374's `BorderIdAllocator` shrinks
   in scope: it allocates u32 border IDs only, not agent IDs. The
   task title and Notes should be rewritten to reflect this.
2. **TASK-0372's `materialize_agent`:** the current spec text says
   "agents[id as usize]" — note that `AgentId` is a type alias
   (`pub type AgentId = u32;` in `net/types.rs`). The cast is safe.
   The TASK-0372 inline test should verify `agents.get(id as usize)`
   handles the case where `id as usize` exceeds
   `agents.len()` — that's a `None` return, not a panic.
3. **`RoundResult.stats` gap:** SPEC-19 R26 says `RoundResult`
   contains `stats: WorkerRoundStats`. With DC-B5's `minted_agents`
   addition, the stats structure should gain a counter
   `minted_agents_count: u32` for observability. Non-blocking;
   orchestrator to file as a follow-up ticket on 2.26-A.
4. **Determinism of `PendingCommutation.commutation_id`:** DC-B5's
   2-phase flow correlates by `u64` handle. The coordinator MUST
   generate these deterministically (e.g. `round_number << 32 |
   sequence_within_round`) so that a replay of the protocol yields
   identical handles. Non-blocking for 2.26-B (the resolver just
   needs to emit unique handles per run); worth documenting in
   2.26-C's handle-assignment logic.

Observations 1-4 are non-blocking for Stage 2 TESTS; each is a
flag for 2.26-C / 2.26-A / orchestrator follow-up.

## Checklist

### Consistency
- [x] All terms match SPEC-00 / SPEC-01 / SPEC-19 / SPEC-03
      definitions (no new terms introduced except
      `PendingCommutation`, which is a 2.26-A protocol type).
- [x] Type signatures compatible with predecessor specs (`PortRef`,
      `WorkerId`, `AgentId`, `Symbol`, `BorderState`, `BorderDelta`,
      `AddBorderEntry` per SPEC-19 §3.2).
- [x] No contradictions with predecessor requirements — DC-B3 / B5
      add fields, they do not override existing ones; DC-B4's R40c
      strengthens D6 rather than amending it; DC-B6 aligns with
      SPEC-03 R14.
- [x] Data flow assumptions match predecessor outputs — the §3.2
      BorderGraph primitives (`apply_deltas`, `remove_border`,
      `add_border_states`) suffice for all 6 rules; no new BorderGraph
      method is required.

### Testability
- [x] DC-B1 verifiable by signature check on `materialize_agent`
      (`&Partition` input).
- [x] DC-B2 verifiable by panic-message regex on test:
      `#[should_panic(expected = "border_resolver: agent missing")]`.
- [x] DC-B3 verifiable by struct-shape test on `WorkerDeltas` +
      `RoundStartDispatch` fields.
- [x] DC-B4 verifiable by end-to-end test in 2.26-C (pinning
      prevents double-consumption on a 2-partition fixture with a
      border-principal agent that has a locally-adjacent redex).
      Out of scope for 2.26-B's Stage 2.
- [x] DC-B5 verifiable by struct-shape tests in TASK-0374; end-to-
      end verification is 2.26-C's integration test.
- [x] DC-B6 verifiable by the renamed
      `test_con_era_border_redex_resolves_principal_border` assertion.
- [x] DC-B7 verifiable by triple-struct check on
      `BorderResolution.resolved_borders`.
- [x] DC-B8 + DC-B9 verifiable by the shared-helper test invocation
      + attempted `use crate::coordinator` import returns assertion
      failure (can be tested by a compile-fail harness, optional).

### Completeness
- [x] All 9 flagged choices have a verdict.
- [x] All cascading TASK impacts documented.
- [x] Cross-bundle memos enumerated (2.26-A: 2 new fields + 1 new
      type; 2.26-C: 4 distinct additions; 2.26-D: R40c invariant).
- [x] No undefined terms in the verdict.

### Invariant Preservation
- [x] T1-T7 unaffected (coordinator-side metadata, not net
      structure).
- [x] D1-D2, D4-D5 unaffected (no partitioning semantics changed).
- [x] D3 (Border Completeness) unchanged in spirit, STRENGTHENED in
      practice by DC-B6 Option (A) (auxiliary-port borders are
      preserved across CON-ERA / DUP-ERA).
- [x] D6 (Protocol Termination) — DC-B4's R40c amendment
      introduces border-pinning; progress guarantee is preserved
      because pinned agents become reducible via coordinator
      resolution (next round) or are consumed by another local
      reduction that does not involve their principal port (never
      happens — principal-port is how reduction is triggered).
      Termination bound unchanged.
- [x] I1-I5 unaffected (no interaction rules changed; the resolver
      MIRRORS the existing rules' topology without modifying them).
- [x] G1 — DC-B5 introduces 1-round deferral on CON-DUP
      finalization; the recoverability property holds modulo this
      delay (proof obligation owned by 2.26-D + DISC-011 /
      ARG-005).
- [x] R18 complexity bounds preserved — resolver is O(1) per border
      redex; batching via `package_resolutions` is O(B) per round.
- [x] R19 pure-core invariant — strengthened by DC-B8 + DC-B9 (new
      shared helper + broader forbidden-prefix list).
