# QA Report — D-010 SPEC-21 Streaming Generation — Phase B+C+D+E+F

**Date:** 2026-04-28
**Phases audited:** B (foundation types), C (strategies), D (bench streaming), E (accumulator + orchestrator), F W1-W9 (CLI flags, R26 short-circuit, wire protocol, FSMs, BorderGraph extension, Strategy A/B counters, `streaming-no-recycle` feature gate)
**Commits in scope:** `df80fe1..61e86a1` (14 commits)
**Reviewer findings already known:** MF-001, MF-002, SF-001..SF-004, NTH-001..NTH-003 — see `docs/reviews/REVIEW-PHASE-D010-spec21-streaming-2026-04-28.md`. **NOT re-reported here.**
**Mindset:** Adversarial — try to break the code; cite reviewer only when extending a separate dimension.

## Summary

- **Total findings:** 16 (CRITICAL: 4, HIGH: 5, MEDIUM: 4, LOW: 3)
- **Top-3 most dangerous:**
  1. **QA-D010-001** [CRITICAL] — `enter_streaming_mode` / `exit_streaming_mode` use `is_in_delta_round` as proxy. Under the `delta_mode && streaming_active` conjunction, `exit_streaming_mode` (called on FinalReduction → Done) **silently disarms `is_in_delta_round` mid-run**, killing every SPEC-22 R10b/R10c protection (Strategy A/B gate, R10c protected-tombstone guard, R10 id_range guard) for the remainder of the worker's lifetime. The reviewer's MF-002 says these helpers are not called in production; that masks a worse failure mode that surfaces the moment the wiring lands.
  2. **QA-D010-002** [CRITICAL] — `Message::RequestWork { worker_id }` carries the worker's claimed identity in the message body. Nothing in the FSM (`PullCoordinatorEvent::RequestWorkReceived { worker_id }`) or in the protocol handler validates that this `worker_id` matches the connection it arrived on. A misbehaving / malicious worker can issue `RequestWork { worker_id: <victim_id> }` and the coordinator's pull dispatcher will route the next chunk to the WRONG worker (impersonation / chunk theft, denial of service to victim).
  3. **QA-D010-003** [CRITICAL] — `default_chunked_iter` (the R10 default-impl path used by every benchmark that does NOT override `make_net_stream`) **silently drops every `FreePort` connection from the materialized net** (`bench/streaming.rs:90-93`). The doc-comment on those lines claims "the accumulator path reconstructs from the port array" — false. The accumulator only sees what the iterator yields. Any benchmark with a non-empty interface (Lafont root or any agent connected to a FreePort) loses those wires after streaming, breaking T6 isomorphism and producing a different normal form.

The bundle has the same shape D-009 had: well-tested *primitives* (FSM transitions, individual strategy methods, individual gates) but **integration seams** that silently corrupt or DoS. Combined with reviewer's MF-001/MF-002, the streaming pipeline ships safe ONLY when (a) `delta_mode == false`, (b) the operator never enables `enter_streaming_mode`, (c) every benchmark overrides `make_net_stream`, and (d) every worker is non-malicious. Stage 6 REFACTOR must address at least the four CRITICAL findings before the conjunction `delta_mode && streaming_active` can be considered safe.

---

## Findings

### QA-D010-001 — `exit_streaming_mode` silently destroys `delta_mode` runtime state under the `delta_mode && streaming_active` conjunction [CRITICAL]
**File:** `relativist-core/src/worker.rs:926-940` (helpers); intended call-site `WorkerPullContext` post-`FinalReductionComplete`
**Severity:** CRITICAL
**Category:** Logic / Spec Violation / State Aliasing

**Reproducer (conceptual once MF-002 is fixed):**
```rust
let mut net = /* a worker subnet entering a delta-mode round */;
net.is_in_delta_round = true;          // SPEC-19: real delta round
net.recycle_policy = RecyclePolicy::DisableUnderDelta;

// Coordinator dispatches first streaming chunk under delta_mode + streaming.
enter_streaming_mode(&mut net);        // sets is_in_delta_round = true (already was)
// ... worker reduces chunk(s), pop suppression works ...

// Worker exits streaming on FinalReduction:
exit_streaming_mode(&mut net);         // sets is_in_delta_round = FALSE.
                                       // BUT the DELTA round is still active!
                                       // SPEC-22 R10b/R10c protections are now OFF,
                                       // SPEC-19 R12a invariants now silently violated,
                                       // border IDs may be popped before reconstruct().
```

**Why it breaks:** The two helpers blindly set/clear `is_in_delta_round` as a single bit. SPEC-21 §3.7 R37b broadens the gate to `delta_mode || streaming_active` — semantically a UNION, requiring two independent state bits with `OR` evaluation. The implementation reuses the same bit for both, so the LAST writer wins. Under the conjunction the writes alternate destructively: `enter_streaming_mode` is idempotent (true → true) but `exit_streaming_mode` is destructive (true → false). Once streaming exits, R10b is disarmed even though the delta round has not exited. Subsequent `create_agent` calls during the same delta round happily pop border-protected IDs from the free-list, **directly enabling the G1 violation R10b was designed to prevent**.

The reviewer's MF-002 notes the helpers aren't called in production — that's the *only* reason this hasn't blown up yet. The moment Stage 6 wires them up (per reviewer's "Posture" §1-2), the wiring MUST not call `exit_streaming_mode` on a Net whose `is_in_delta_round` was set by an outer delta-mode round.

**Fix sketch:**
1. Add a dedicated `streaming_active: bool` field on `Net` (NOT reuse `is_in_delta_round`).
2. Update the SPEC-22 gates from `is_in_delta_round && policy == DisableUnderDelta` to `(is_in_delta_round || streaming_active) && policy == DisableUnderDelta`. Same for BorderClean.
3. `enter_streaming_mode` writes `streaming_active = true`. `exit_streaming_mode` writes `streaming_active = false`. Neither touches `is_in_delta_round`.
4. Add a regression test: enter delta round → enter streaming → exit streaming → assert delta-round protections still active.

---

### QA-D010-002 — `RequestWork.worker_id` is not validated against the connection's authenticated WorkerId — peer impersonation surface [CRITICAL]
**File:** `relativist-core/src/protocol/types.rs:272-275` (wire), `relativist-core/src/coordinator.rs:874` (FSM event), `relativist-core/src/protocol/coordinator.rs` (handler — no validation)
**Severity:** CRITICAL
**Category:** Wire Protocol / Spec Violation / Security

**Reproducer:**
```rust
// Worker A (auth'd as WorkerId 5) crafts a frame with someone else's id:
let evil = Message::RequestWork { worker_id: 7 };
send_frame(&mut conn_a, &evil).await?;
// Coordinator dispatches the next chunk to worker_id=7 (according to the FSM
// event payload), but the chunk is delivered over conn_a. Worker A has either
// stolen 7's chunk (steady-state work theft) OR 7 receives nothing (DoS).
// In hybrid push/pull configurations under contention, the chunks-allocated
// counter and the actual delivery target diverge.
```

**Why it breaks:** `Message::RequestWork { worker_id: WorkerId }` ships the worker's identity in the payload, not in the framing layer. The PullCoordinatorEvent::RequestWorkReceived { worker_id: u32 } arm at `coordinator.rs:1000-1007` matches on `..` (ignores worker_id entirely) for the state transition. The actual dispatch logic — wherever the chunk's destination is computed — has no access to the connection's authenticated WorkerId from the Register handshake (SPEC-10), so it MUST use the `worker_id` field of the message at face value. SPEC-21 R32 says nothing about whether this field is normative or informational; SPEC-06 R5 enforces discriminant stability but not field-trust posture. A malicious worker (or one with a state bug) can silently misroute chunks.

The reviewer's "Passed Checks" line "RequestWork { worker_id } carries WorkerId as a newtype (not raw u32)" is not the same property: newtype prevents type confusion, not impersonation. WorkerId is a transparent `u32` newtype, no authentication content.

**Fix sketch:**
1. At the protocol handler, drop the `worker_id` field from the wire (the connection already knows who the peer is from Register/JoinAck) OR
2. Compare `incoming_msg.worker_id` against `conn.authenticated_worker_id` and `Err(ProtocolError::WorkerIdMismatch)` on disagreement.
3. Add a regression test: forge a `RequestWork { worker_id: <other> }` over a registered connection; assert coordinator drops it.
4. SPEC-21 R32 needs a normative line: "the `worker_id` in `RequestWork` MUST equal the connection's authenticated WorkerId; coordinator MUST reject mismatches."

---

### QA-D010-003 — `default_chunked_iter` silently drops all FreePort connections — T6 isomorphism violated for any net with an interface [CRITICAL]
**File:** `relativist-core/src/bench/streaming.rs:55-103` (specifically the `match net.ports[idx] { … FreePort(_) => {} }` arm at lines 90-93)
**Severity:** CRITICAL
**Category:** Logic / Spec Violation / Wire Loss

**Reproducer:**
```rust
let mut net = Net::new();
let con = net.create_agent(Symbol::Con);
net.connect(PortRef::AgentPort(con, 0), PortRef::FreePort(0));   // Lafont root
net.connect(PortRef::AgentPort(con, 1), PortRef::FreePort(1));   // aux interface
net.connect(PortRef::AgentPort(con, 2), PortRef::FreePort(2));   // aux interface

let stream = default_chunked_iter(net.clone());
let batch = stream.next().unwrap();
assert_eq!(batch.connections.len(), 0);   // !!! all 3 wires LOST
// The streamed result has 1 agent, 0 connections. The original had 1 agent,
// 3 wires. Reduction proceeds normally on both, but normal forms differ:
// the streamed net has dangling ports, the original has FreePorts (Lafont
// interface preserved).
```

**Why it breaks:** Lines 90-93:
```rust
PortRef::FreePort(_) => {
    // FreePort (Lafont interface or border) — not emitted here;
    // the accumulator path reconstructs these from the port array.
}
```
The doc-comment is **factually wrong**. The accumulator (`PartitionAccumulator`) only sees `ConnectionDirective`s emitted by the iterator; it has no access to the original `Net.ports` array. Any wire whose port array entry is a `FreePort` is omitted from the iterator and therefore from the partitioned result.

This breaks SPEC-21 R26 isomorphism (T6 oracle). The R26 short-circuit path (chunk_size == u32::MAX) at `streaming.rs:967-1001` *also* skips `FreePortInterface` directives ("not reachable on well-formed full-stream materialisation" comment at line 988) — but `default_chunked_iter` is not a "well-formed full-stream materialisation"; it IS the materialisation, and it's missing the FreePort wires upstream of the short-circuit path's filter.

The integration test `default_impl_path_equivalence_ep_annihilation` only checks `agents.len()`, not connections. Same for `default_impl_path_equivalence_dual_tree`. Neither test would catch this: ep_annihilation has no FreePort interface (pairs are agent-to-agent), and dual_tree's native override (`dual_tree_stream`) bypasses `default_chunked_iter` entirely. CascadeCross *might* exercise this path but the test only asserts `batches.len() == 1`, not connection count.

Any benchmark whose `make_net` produces FreePort interface wires AND does not override `make_net_stream` (i.e. `CascadeCross`, future custom nets) silently produces a torn streamed copy.

**Fix sketch:**
1. Emit a `ConnectionDirective::FreePortInterface { agent_port, free_port_id }` for every `(id, port) → FreePort(fp_id)` in the port array, with a "lower id wins" deduplication rule similar to the AgentPort case.
2. Add a regression test that creates a Net with a Lafont root + 2 FreePort interface wires, runs it through `default_chunked_iter`, materialises the batches via the accumulator, and asserts the resulting partition has 3 FreePort wires.
3. Update the doc-comment that claims "the accumulator path reconstructs from the port array" — the accumulator does no such thing.

---

### QA-D010-004 — `border_id_counter` starts at 0; first border ID collides with `FreePortInterface { free_port_id: 0 }` and silently overwrites the border map entry [CRITICAL]
**File:** `relativist-core/src/partition/streaming.rs:1008` (counter init), `streaming.rs:837` (border_map insert), `streaming.rs:889` (the doc lies)
**Severity:** CRITICAL
**Category:** Logic / Wire Aliasing / Spec Violation

**Reproducer:**
```rust
// Generator emits a FreePortInterface with free_port_id = 0
// (e.g. a custom benchmark that uses 0-based FreePort IDs):
let batch_0 = AgentBatch {
    agents: vec![(0, Symbol::Con)],
    connections: vec![ConnectionDirective::FreePortInterface {
        agent_port: (0, 1),
        free_port_id: 0,
    }],
};
// Pipeline installs accumulator[w0].connect(AgentPort(0,1), FreePort(0)).
// free_port_index[0] = AgentPort(0,1).

// Next batch issues a cross-worker wire allocating bid = 0 (counter starts at 0):
let batch_1 = AgentBatch {
    agents: vec![(1, Symbol::Era)],
    connections: vec![ConnectionDirective::Resolved {
        source: (0, 2),     // worker w0
        target: (1, 0),     // worker w1
    }],
};
// install_connection allocates bid = 0 (counter), border_map.insert(0, ...).
// Both workers now have FreePort(0) wires, but the border map only records
// the LAST insertion. The Lafont interface wire is now indistinguishable
// from the border wire on the receiver side — merge will treat them as the
// same wire, splicing two unrelated agents together.
```

**Why it breaks:** Line 1008 — `let mut border_id_counter: u32 = 0;`. The doc comment at lines 887-892 claims:
> If the first batch contains any `FreePort` IDs in its connections (Lafont interface ports), `border_id_counter` is initialized to the maximum such ID + 1 to avoid collision. Otherwise it starts at 0.

This is a **lie**. The code **never** scans batch connections for FreePort IDs. The counter is hard-coded to 0. The directive's documentation at `streaming.rs:88-92` correctly states "MUST NOT collide with border IDs allocated by `install_connection`" but the streaming pipeline does not enforce or even check this contract. If any generator's `FreePortInterface` uses an ID `< border_count_for_this_run`, the resulting net has TWO wires sharing the same `FreePort(bid)` — silent D5/D6 (port bijection) violation.

Furthermore, even when the doc *worked*, scanning only the FIRST batch is insufficient: a Pending → Resolved sequence whose target carries a FreePortInterface in batch 7 would also need the counter pre-seeded. The "scan first batch" comment is both unimplemented and architecturally wrong.

**Fix sketch:**
1. Pre-scan ALL batches' `FreePortInterface` directives before main loop, compute `max_fpid`, init counter to `max_fpid + 1`. (Requires a 2-pass orchestrator OR a streaming pre-allocation contract — generators announce a `reserved_freeport_range` upfront.)
2. OR — preferably — keep border IDs and FreePort IDs in **disjoint disjoint namespaces** (e.g. border IDs are `u32::MAX/2..u32::MAX`, FreePorts are `0..u32::MAX/2`); enforce in a `debug_assert!` on every directive.
3. Add a regression test: generator emits FreePortInterface(0) and a cross-worker resolved wire; assert border_map and free_port_index have non-overlapping keys.
4. Either delete the doc-comment lines 887-892 or implement them. As-is they're dead metadata that misleads future maintainers.

---

### QA-D010-005 — `RoundRobinStreamingStrategy::allocate_batch` panics on `num_workers == 0`; FENNEL same; neither has a debug_assert [HIGH]
**File:** `relativist-core/src/partition/streaming.rs:380` (RoundRobin), `streaming.rs:484-545` (FENNEL)
**Severity:** HIGH
**Category:** Panic Path

**Reproducer:**
```rust
let mut s = RoundRobinStreamingStrategy::new(0);
let batch = AgentBatch {
    agents: vec![(0, Symbol::Era)],
    connections: vec![],
};
let _ = s.allocate_batch(&batch, 0);   // PANIC: integer division by zero
                                       // at `self.counter % num_workers as u64`
```

**Why it breaks:** Both strategies divide / mod by `num_workers` without a guard. The doc on `RoundRobinStreamingStrategy::new` at line 358 says "Callers MUST ensure `num_workers >= 1`", but no `debug_assert!` enforces it at `new()`, and `allocate_batch` re-takes `num_workers` (a separate parameter from construction time — see QA-D010-006), so even a careful caller can be foiled by a coordinator that drops to 0 workers mid-run (SPEC-20 elastic departure).

**Fix sketch:**
1. Add `assert!(num_workers >= 1, "...")` (release-mode panic with message, not silent UB) in both `new()` and `allocate_batch`.
2. SPEC-21 should add a normative R: "Strategies are unspecified for num_workers == 0; coordinator MUST short-circuit before invoking allocate_batch."
3. Coordinator (SPEC-20 elastic departure path): when `active_workers == 0`, transition to `SoloReducing` or `Done` BEFORE calling any streaming strategy.

---

### QA-D010-006 — `allocate_batch(num_workers)` parameter is independent of construction-time `num_workers` — index OOB on `per_worker_counts` if they disagree [HIGH]
**File:** `relativist-core/src/partition/streaming.rs:375-386` (RoundRobin), `streaming.rs:484-554` (FENNEL)
**Severity:** HIGH
**Category:** Panic Path / API Hazard

**Reproducer:**
```rust
let mut s = RoundRobinStreamingStrategy::new(4);   // per_worker_counts.len() == 4
let batch = AgentBatch {
    agents: vec![(0, Symbol::Era), (1, Symbol::Era)],
    connections: vec![],
};
let _ = s.allocate_batch(&batch, 8);
// counter = 0, 1 → workers 0, 1 (within bounds, no panic THIS call)
// counter = 4, 5 → workers 4, 5 → per_worker_counts[4..=5] OOB → PANIC
```

**Why it breaks:** Both strategies maintain `per_worker_counts: Vec<u64>` sized at construction. `allocate_batch` receives `num_workers` as a parameter and uses it for the modulus / iteration bounds. If those two values disagree (a caller bug, an elastic-grid resize, etc.) the strategy produces `worker_id >= self.per_worker_counts.len()` and panics on `self.per_worker_counts[worker as usize]`.

This is *also* the scenario triggered by SPEC-20 elastic join/leave: the coordinator can grow `num_workers` mid-run, but the strategy was built with the prior count.

**Fix sketch:**
1. Drop the `num_workers` parameter from `allocate_batch` (it's already in `&self`); strategies use only the construction-time value.
2. OR re-size `per_worker_counts` to `max(self.per_worker_counts.len(), num_workers as usize)` at the top of every `allocate_batch`.
3. Either way, `debug_assert_eq!(num_workers as usize, self.per_worker_counts.len())` during transitional period.
4. SPEC-21 R8 says "deterministic given num_workers" — the spec needs a clarification that the strategy's construction-time `num_workers` is the canonical value.

---

### QA-D010-007 — FENNEL strategy with `alpha = NaN` violates R8 determinism (best_worker becomes the LAST worker checked, not the lowest WorkerId) [HIGH]
**File:** `relativist-core/src/partition/streaming.rs:535-545`
**Severity:** HIGH
**Category:** Logic / Spec Violation

**Reproducer:**
```rust
let mut s = FennelStreamingStrategy::new(4, f64::NAN);
let batch = AgentBatch {
    agents: vec![(0, Symbol::Era)],
    connections: vec![],
};
let assignments = s.allocate_batch(&batch, 4);
// score = 0 - NaN * 0 = NaN for every worker.
// In the comparison loop:
//   w=0: score=NaN; best_score=NEG_INFINITY; best_score.is_nan()=false
//        → branch "score > best_score" is `NaN > NEG_INFINITY` = false (IEEE).
//        → branch "best_score.is_nan()" = false. Result: best_worker stays 0,
//        best_score stays NEG_INFINITY.
//   w=1: score=NaN; best_score=NEG_INFINITY → same branches → no update.
//   w=2,3: same.
// Final: best_worker = 0. OK, determinism preserved? Let's check w=0 first iteration:
//   score=NaN, best_score=NEG_INFINITY, w<best_worker is 0<0 (false), score==best_score
//   is `NaN==NEG_INFINITY` (false), best_score.is_nan() is false → no update.
//   best_score stays NEG_INFINITY, best_worker stays 0.
// Ultimately, best_worker is always 0, but for the WRONG reason (no branch fires;
// initial value carries through). And the doc comment says alpha=NaN "is handled
// by falling back to capacity-only scoring (tiebreak by lowest WorkerId)".
// Capacity-only (degree) is NOT computed — the actual fallback is "always w=0",
// which is incidentally correct for round 1 but is NOT degree-aware.
```

Wait — closer inspection: when `alpha=NaN` and degree=0, `NaN*0 = NaN`, `0-NaN = NaN`. The condition order in the code is:
```rust
if score > best_score
    || (score == best_score && w < best_worker)
    || best_score.is_nan()
```
- `NaN > NEG_INFINITY` = `false`.
- `NaN == NEG_INFINITY` = `false`.
- `best_score.is_nan()` checks BEST_SCORE, which starts at `NEG_INFINITY` (not NaN), so `false`.

So the `if` branches all fire `false` for every worker. `best_worker` stays at the initialiser `0`. **OK — accidentally correct for this case**.

BUT consider when `alpha` is finite and a real score gets stored, then we hit a NaN later (e.g., neighbor count infected by alpha later). Once `best_score = NaN`, the `best_score.is_nan()` arm fires for every subsequent worker, **overwriting `best_worker` on every iteration**. Final `best_worker = num_workers - 1` regardless of the workers' merits. This **violates R8 determinism's intent** ("tiebreak = lowest WorkerId") for any score sequence that admits a NaN partway through.

**Fix sketch:**
1. Use `f64::total_cmp` (mentioned in the doc comment at line 536 but NOT actually used in the code at lines 538-545):
```rust
match score.total_cmp(&best_score) {
    Ordering::Greater => { best_score = score; best_worker = w; }
    Ordering::Equal => { if w < best_worker { best_worker = w; } }
    Ordering::Less => {}
}
```
This is total-order, NaN-safe, and matches the doc claim.
2. Add a regression test with `alpha = NaN` AND a non-trivial score history; assert determinism (lowest WorkerId).

---

### QA-D010-008 — `generate_and_partition_chunked_with_chunk_size` panics on bogus strategy (`worker_id >= num_workers`) and on missing `agent_id` in batch.agents [HIGH]
**File:** `relativist-core/src/partition/streaming.rs:1026-1029`
**Severity:** HIGH
**Category:** Panic Path / Trust Boundary

**Reproducer:**
```rust
struct Evil;
impl StreamingPartitionStrategy for Evil {
    fn allocate_batch(&mut self, batch: &AgentBatch, num_workers: u32) -> Vec<(AgentId, WorkerId)> {
        // Returns out-of-bounds worker_id:
        batch.agents.iter().map(|(id, _)| (*id, num_workers + 100)).collect()
    }
    fn finalize(&self) -> StreamingPartitionStats { /* ... */ }
}
let stream: Box<dyn Iterator<Item = AgentBatch>> = /* ... */;
generate_and_partition_chunked_with_chunk_size(stream, 4, &mut Evil, 1000);
// At streaming.rs:1029: accumulators[*worker_id as usize].add_agent(...)
// → vec index 104 OOB → PANIC

struct EvilTwo;
impl StreamingPartitionStrategy for EvilTwo {
    fn allocate_batch(&mut self, batch: &AgentBatch, num_workers: u32) -> Vec<(AgentId, WorkerId)> {
        // Returns assignment for an agent NOT in the batch:
        vec![(99999, 0)]
    }
    fn finalize(&self) -> StreamingPartitionStats { /* ... */ }
}
// At streaming.rs:1028: let symbol = symbol_lookup[agent_id];
// → HashMap index access for missing key → PANIC
```

**Why it breaks:** The orchestrator trusts the strategy to return only `agent_id` values present in `batch.agents` and only `worker_id` values in `[0, num_workers)`. There's no validation. SPEC-21 R7 says "Every WorkerId is in [0, num_workers)" but as a contract on the strategy, not enforced by the pipeline. Production strategies (RoundRobin, FENNEL) are well-behaved, but the trait is `pub`; downstream crates can implement it. A buggy strategy panics the coordinator thread (which in async code may abort the whole tokio runtime).

**Fix sketch:**
```rust
for (agent_id, worker_id) in &assignments {
    if *worker_id as usize >= accumulators.len() {
        return Err(PartitionError::StrategyReturnedInvalidWorker {
            worker_id: *worker_id,
            num_workers: num_workers,
        });
    }
    let symbol = match symbol_lookup.get(agent_id) {
        Some(s) => *s,
        None => return Err(PartitionError::StrategyReturnedUnknownAgent { agent_id: *agent_id }),
    };
    agent_owner.insert(*agent_id, *worker_id);
    accumulators[*worker_id as usize].add_agent(*agent_id, symbol);
}
```

---

### QA-D010-009 — `max_pending_lifetime` is declared in CLI args + GridConfig + serde, but `generate_and_partition_chunked_with_chunk_size` never reads it; pending HashMap grows unboundedly [HIGH]
**File:** `relativist-core/src/partition/streaming.rs:1011, 1067-1084` (HashMap insert without lifetime check), `relativist-core/src/merge/types.rs:680` (declaration)
**Severity:** HIGH
**Category:** DoS / Spec Violation / Resource Exhaustion

**Reproducer:**
```rust
// Stream that emits Pending directives forever, never the targets:
let stream = (0..u32::MAX).map(|i| {
    AgentBatch {
        agents: vec![(i, Symbol::Era)],
        connections: vec![ConnectionDirective::Pending {
            source: (i, 0),
            target_agent_id: u32::MAX,   // never emitted
            target_port: 0,
        }],
    }
});
let mut s = RoundRobinStreamingStrategy::new(2);
let _ = generate_and_partition_chunked_with_chunk_size(Box::new(stream), 2, &mut s, 1);
// pending: HashMap<AgentId, Vec<PendingConnection>> grows without bound.
// Each iteration: pending.entry(u32::MAX).or_default().push(pc).
// After 100M iterations: ~3 GiB resident.
```

**Why it breaks:** Lines 1067-1084 unconditionally `pending.entry(target).or_default().push(pc)`. SPEC-21 §3.7 R37g (cited in the field's doc-comment at `merge/types.rs:677`) says "A `Pending` directive that remains unresolved after more than `max_pending_lifetime` batches indicates a malformed stream". The lifetime is **declared, serialized, even passed through CLI** but **never consulted** by the orchestrator. R37g is unenforced. A malicious or buggy generator monopolizes coordinator memory; even a benign generator hits this if a target is mis-typed.

The post-loop check at `streaming.rs:1105-1107` returns `UnresolvedForwardReferences` AFTER the stream ends. If the stream never ends (long-running generator, pull-mode dispatch), the loop runs forever and the check never fires.

**Fix sketch:**
1. Add a per-PendingConnection `birth_chunk: u64` field; on every iteration scan pending entries with `chunks_seen - birth_chunk > max_pending_lifetime` and return `Err(PartitionError::PendingConnectionExpired)`.
2. Pass `max_pending_lifetime` into `generate_and_partition_chunked_with_chunk_size` (currently absent from the signature — that itself is a sign the contract was forgotten).
3. Add a regression test: pending target never resolved, max_pending_lifetime=4; assert error after 5 chunks.

---

### QA-D010-010 — `extend_with_chunk_borders` stores `worker_a: 0, worker_b: 0` placeholders; if `is_redex == true` the placeholder gets pushed to `active_redexes` and the coordinator believes worker 0 owns BOTH endpoints [MEDIUM]
**File:** `relativist-core/src/merge/border_graph.rs:653-670`
**Severity:** MEDIUM
**Category:** Logic / Latent (activates when MF-001 is wired)
**Cross-ref:** Reviewer MF-001 noted the call-site is dead. This finding extends MF-001 with a *content* hazard that lives independently in the method's body.

**Reproducer (post-MF-001-fix):**
```rust
let mut graph = BorderGraph::default();
let mut new_borders = HashMap::new();
new_borders.insert(0, (
    PortRef::AgentPort(5, 0),   // principal port of worker A's agent 5
    PortRef::AgentPort(7, 0),   // principal port of worker B's agent 7
));
graph.extend_with_chunk_borders(&new_borders);
// is_redex = is_principal_pair(...) = true → active_redexes.insert(0).
// borders[0] = BorderState { worker_a: 0, worker_b: 0, is_redex: true, ... }
// Coordinator detects redex, asks "who owns side_a?" → worker 0.
// "Who owns side_b?" → worker 0. Sends commutation BATCH to worker 0
// for a wire that actually crosses worker 5 ↔ worker 7. Wrong dispatch.
```

**Why it breaks:** Lines 658-660 store `worker_a: 0, worker_b: 0` as "placeholder" values. The comment claims this is safe because "downstream delta-mode resolution uses border_id + side_a/side_b only". This is incorrect for the active-redex path: SPEC-19 R12-R13 dispatch the commutation to the worker(s) owning the principal endpoints. With both worker IDs hardcoded to 0, **all chunk-extension borders that are also redexes get routed to worker 0**, regardless of which workers actually own the endpoints.

The reviewer correctly flagged the call-site as dead in MF-001. Once that's fixed, the placeholder content becomes a live hazard — masking under MF-001 isn't a fix.

**Fix sketch:**
1. Change `extend_with_chunk_borders` signature to take `&HashMap<u32, ChunkBorderEntry>` where `ChunkBorderEntry = (PortRef, WorkerId, PortRef, WorkerId)` — caller MUST supply worker IDs (the caller knows them; the streaming pipeline sets them via the accumulator).
2. Alternatively, REJECT borders without known workers: return an error if any side's owner is unknown.
3. The "placeholder" pattern is a code smell — never store sentinel values in fields that get consumed downstream.

---

### QA-D010-011 — `streaming-no-recycle` feature gate's early-return path bypasses every SPEC-22 safety `debug_assert!` (R10c protected_tombstones, R10 id_range, R27 family-3, R4(b) slot is None) [MEDIUM]
**File:** `relativist-core/src/net/core.rs:261-283`
**Severity:** MEDIUM
**Category:** Logic / Defense in Depth / Test Coverage Loss

**Reproducer (with `cargo test --features streaming-no-recycle` in DEBUG):**
```rust
// Inject a Net into an inconsistent state (debug builds normally catch this):
let mut net = Net::new();
let alive = net.create_agent(Symbol::Con);   // alive=0, next_id=1
// Simulate state corruption: next_id rewinds (could be a synthesis fixture,
// a remap bug, a peer-supplied state):
net.next_id = 0;   // CORRUPTED: agents[0] is Some, but next_id points at it.

net.is_in_delta_round = true;   // streaming active
let new_id = net.create_agent(Symbol::Era);
// Feature-on path executes:
//   let fresh_id = self.next_id;        // = 0
//   self.next_id += 1;                  // = 1
//   self.agents[0] = Some(Agent::Era);  // SILENTLY OVERWRITES the live Con.
// In feature-off path, the protected_tombstones / R4(b) / R27 family-3
// debug_asserts would have caught this. Feature-on path skips ALL of them.
```

**Why it breaks:** The feature-on early-return at lines 261-283 has only ONE assertion: `next_id < u32::MAX` (line 264). It does NOT have:
- `debug_assert!(self.agents[fresh_id].is_none(), "R4(b)")`.
- `debug_assert!(!self.protected_tombstones.as_ref().is_some_and(|s| s.contains(&fresh_id)), "R10c")`.
- `debug_assert!(self.id_range.as_ref().map_or(true, |r| r.contains(&fresh_id)), "R10")`.
- The R27 family-3 invariants.
- The `free_list_pops` counter increment (the spec is explicit that this is "successful pops" — but the feature-on path also calls `create_agent` while bypassing the free_list, so by spec it doesn't pop. OK for that one.)

Feature-on is supposed to be a "safety simplification" per R37b alternative closure. In practice it is **less safe** than the runtime gates because it removes the debug-mode invariant checks. A user enabling the feature trusts that they're getting MORE safety; they're getting LESS.

**Fix sketch:**
1. Restructure the feature-on path to flow through the same fresh-allocation code path as the runtime-gate path (just the additional `if self.is_in_delta_round { skip_free_list }` guard at the top, then call into shared fresh-alloc with all asserts).
2. OR copy all the debug_asserts from the runtime-gate path into the feature-on path.
3. Add a regression test: feature-on, inconsistent Net state, assert debug build panics with the SAME assertion message as feature-off.

---

### QA-D010-012 — IT-0591-01/02 ("cross-feature isomorphism") tests are vacuous: they run the same workload twice in the same binary, never comparing across feature states [MEDIUM]
**File:** `relativist-core/tests/spec22_streaming_no_recycle.rs:174-244`
**Severity:** MEDIUM
**Category:** Test Coverage Gap / False Confidence

**Reproducer (just read the test):**
```rust
fn it_0591_01_cross_feature_isomorphism_strategy_a() {
    fn run_strategy_a() -> Net { /* ... */ }

    let result_1 = run_strategy_a();
    let result_2 = run_strategy_a();   // SAME binary, SAME feature state.

    assert!(nets_isomorphic(&result_1, &result_2), "...");
    // Trivially true: identical input, identical code, identical output.
}
```

**Why it breaks:** The test name promises cross-feature isomorphism. The body verifies *self-consistency* (idempotence of the reduction in a single feature state). It does NOT verify that the result is the same with the feature ON vs. OFF — that would require a separate test process / build artifact. The doc-comment at line 170-173 even acknowledges "Since both feature states (in the same test binary) ultimately reduce the same CON-CON net to the empty normal form, isomorphism trivially holds." That admits the test is structurally incapable of catching a feature-induced divergence.

A real bug in the feature-on path that produces a NON-empty normal form, or a different agent count, would NOT be caught by IT-0591-01/02.

**Fix sketch:**
1. Make the test a snapshot comparison: Strategy A workload produces a fixture serialized in a known shape; the test under feature-on deserializes the fixture and asserts isomorphism with `nets_isomorphic`. Then under feature-off, same fixture, same assertion. The fixture is the bridge.
2. OR reframe the test as "feature-state self-consistency" and add a new acceptance test (CI-only) that runs two `cargo test` invocations and diff-compares debug output.
3. Acknowledge in the test comment that it's a determinism check, NOT cross-feature.

---

### QA-D010-013 — `CoordinatorPullContext` and `CoordinatorState::Pull*` are two parallel state representations with no synchronization layer [MEDIUM]
**File:** `relativist-core/src/coordinator.rs:151-181` (main enum), `coordinator.rs:840-866` (PullCoordinatorState)
**Severity:** MEDIUM
**Category:** Architecture / State Aliasing

**Reproducer:**
```rust
// In a real coordinator loop, the main FSM and the pull-FSM are stepped
// independently. Suppose:
let mut ctx = CoordinatorContext::new(...);
let mut pull = CoordinatorPullContext::new(4);

// Main FSM transitions to Dispatching:
ctx.state = CoordinatorState::PullAwaitingResults;

// Pull FSM is in DispatchingFirst (constructed at start; no caller has stepped it yet):
assert_eq!(pull.state, PullCoordinatorState::DispatchingFirst);

// Two FSMs disagree on the same logical state. The coordinator loop must
// step BOTH on every event; if any step is missed, the divergence persists
// silently. There's no test for "main_state must equal pull_state.into()".
```

**Why it breaks:** Two enums with overlapping semantics + no mapping function = guaranteed drift over time. The reviewer's W4 spot-check at `coordinator.rs:151` shows the main enum gained 5 `Pull*` variants. The W4 commit also added 5 variants in `PullCoordinatorState`. There's no `From` / `Into` / parity assertion connecting them. Future refactors that touch one will silently desynchronize from the other.

**Fix sketch:**
1. Pick one. Either main-FSM-only (delete `PullCoordinatorState`) or pull-FSM-only (`CoordinatorState::PullDispatchActive { sub_state: PullCoordinatorState }`).
2. Add a parity test that exercises every transition of both and asserts equivalence.

---

### QA-D010-014 — `Net.free_list_pops`, `free_list_pops_border`, `free_list_pops_non_border` fields are `#[cfg(debug_assertions)]` — debug and release `Net` have different layouts; mixed debug/release peer pairs cannot share `Net` via shared memory or FFI [MEDIUM]
**File:** `relativist-core/src/net/core.rs:115-143`
**Severity:** MEDIUM
**Category:** ABI / Cross-Build Compatibility

**Reproducer (conceptual):**
```rust
// Build coordinator in debug, worker in release. Assume both use the same
// Net struct via crate dependency. The debug coordinator passes a `Net` over
// a shared-memory transport (or unsafe FFI); the release worker reads it.
// Layouts differ: debug has 3 extra u64 fields and possibly a HashSet field
// (protected_tombstones). The release worker mis-interprets bytes following
// is_in_delta_round.
```

**Why it breaks:** `#[cfg(debug_assertions)]` on **fields** changes the struct's layout based on build profile. While serde paths are guarded by `#[serde(skip)]` and rkyv by `rkyv::with::Skip`, ANY in-memory cross-crate sharing — direct `Box<Net>` over `unsafe { transmute }`, `Pin` -based async transfers across crate boundaries with mixed profiles, future shared-memory transport — sees mismatched memory representations.

For now there's no production code path that does this, so this is MEDIUM not CRITICAL. But adding instrumentation fields under `cfg(debug_assertions)` to a public type is a long-term ABI hazard. Most crates use `#[cfg(any(test, debug_assertions))]` AND apply the cfg to a wrapper / instrumentation submodule, not to public struct fields directly.

**Fix sketch:**
1. Move the three counter fields into a separate `NetTelemetry` struct that the production `Net` holds via `Option<Box<NetTelemetry>>` (None in release, Some in debug). Layout becomes invariant.
2. OR keep the fields always-on and gate only the increment + counter assertions.

---

### QA-D010-015 — `r15_monotonicity_checked` allows empty batches to slip past without consuming `last_max_id` budget; an attacker can interleave empty batches with monotone-violating batches if they rotate state [LOW]
**File:** `relativist-core/src/bench/streaming.rs:487-511`
**Severity:** LOW
**Category:** Logic / Defense in Depth

**Reproducer:**
The current code at line 492 does `if !batch.agents.is_empty()`. An empty batch is a no-op for `last_max_id`. So a stream like `[batch{0..10}, batch{}, batch{5..15}]` produces:
- batch 1: last_max_id = 9.
- batch 2: empty, no update.
- batch 3: min = 5, prev = 9. `5 > 9` = false. **assertion fires correctly.**

Therefore this is NOT actually a violation — empty batches don't help an attacker bypass the check. **Downgrading to LOW**.

But there's a related issue: the assertion uses `min_id > prev_max` (strict `>`), so a batch where `min_id == prev_max + 1` passes and where `min_id == prev_max` fails. **However** if a batch has `[(prev_max, ...)]` (a duplicate of the previous batch's max), it triggers the assertion correctly. So the strict comparison is right.

**Fix sketch:** Add a `debug_assert_eq!(batch.agents.iter().map(|(id,_)|*id).collect::<HashSet<_>>().len(), batch.agents.len(), "no duplicate IDs within a single batch")` for completeness.

---

### QA-D010-016 — Strategy B BorderClean recycling efficiency is at most "LIFO top is non-protected" — if the top is protected, the entire pop+re-push happens once and the (potentially many) non-protected IDs deeper in the stack are never recycled this call [LOW]
**File:** `relativist-core/src/net/core.rs:293-304`
**Severity:** LOW
**Category:** Performance / Spec Drift

**Reproducer:**
```rust
let mut net = Net::new();
for _ in 0..200 { net.create_agent(Symbol::Era); }
// Free 100 non-border IDs first (push order: 0..100):
for id in 0..100 { net.remove_agent(id); }
// Now free 1 border ID at the TOP:
net.remove_agent(150);
let mut shadow = HashSet::new();
shadow.insert(150);
net.border_entries_shadow = Some(shadow);
net.recycle_policy = RecyclePolicy::BorderClean;
net.is_in_delta_round = true;

// Next create_agent:
let id = net.create_agent(Symbol::Era);
// Pops 150 (LIFO top), border-protected → push back, fall through to fresh.
// id = 200 (fresh). free_list still has [0..100, 150] - 0 IDs recycled.
// 100 perfectly-recyclable non-border IDs were never inspected.
```

**Why it breaks:** The spec text at `RecyclePolicy::BorderClean` doc claims "workers MAY pop from the free-list only if the popped ID is NOT in the partition's `border_entries_shadow`". The implementation reads this as "ONLY THE LIFO TOP is inspected". Spec intent (per the "precision recycling" name) implies search-then-skip: scan free-list for a non-protected ID, recycle it.

The reviewer's SF-001 noted the counter-naming issue in this same code block. This finding is the recycling-efficiency consequence of the same `pop+test` pattern: it's a single LIFO probe, not a search.

For the M5 break-even budget (where SPEC-22 R10b is supposed to recover memory), single-probe recycling is essentially equivalent to "no recycling" whenever the pop top is border-protected — which is common during a delta round.

**Fix sketch:**
1. Convert `Vec<AgentId>` free_list to `(Vec<AgentId>, Vec<AgentId>)` — a (non_border, border) split — with O(1) append/pop on each.
2. OR scan free_list linearly from end to front for a non-protected ID.
3. SPEC-22 R10b should be amended to clarify whether "MAY pop" means "MAY scan for a non-border ID" or "MAY pop only if LIFO top is non-border". Current implementation matches the literal text; spec likely intends the former.

---

## What was checked but no issue found

- **`PROTOCOL_VERSION = 6` and `PREVIOUS_LIVE_VERSION = 5`** are pinned by `const _: () = assert!(...)` at coordinator.rs:213-216 — discriminant +1 contract enforced at compile time. Reviewer's "Passed Checks" verified.
- **`Message::RequestWork` and `Message::NoMoreWork` discriminants 17, 18** are byte-pinned by the `test_message_discriminant_stability` test (types.rs:1218-1351). Cardinality assertion at line 1346-1349 (`assert_eq!(cases.len(), 19)`) catches future appends without test updates.
- **`R15MonotonicityChecker` strict `>` comparison** is correct for monotonicity (see QA-D010-015 — initially flagged but downgraded after analysis).
- **`PartitionAccumulator::add_agent` updates `min_assigned_id`/`max_assigned_id`** correctly with `map_or(id, ...)`. No off-by-one.
- **`From<PartitionPlan> for ChunkedPartitionResult`** at streaming.rs:891-925 — `chunks_processed: 1` for short-circuit path is consistent with "the whole net is one chunk".
- **`ChunkedPartitionResult` band-based id_range computation** at streaming.rs:1133-1145 satisfies D3 disjointness.
- **`CHUNK_SIZE_MAX_SENTINEL = u32::MAX`** is exported and used consistently.
- **`#[serde(skip)]` and `rkyv::with::Skip` for new debug-only counter fields** at all `Net` literal sites — reviewer "Passed Checks" verified, not regressed.
- **CI matrix gains a `streaming-no-recycle` column** at `.github/workflows/ci.yml` — verified via UT-0591-04/05/`it_0591_04_ci_matrix_includes_feature_column`.

---

## Cross-cutting observations

1. **The reviewer's MF-001 (extend_with_chunk_borders dead) and MF-002 (enter/exit_streaming_mode dead) compose with this report's QA-D010-001 (exit_streaming_mode is destructive on conjunction) and QA-D010-010 (extend_with_chunk_borders worker placeholders).** Once Stage 6 wires up the call-sites, BOTH content bugs go live simultaneously. Any Stage 6 work to "fix MF-001/MF-002" MUST do the content fixes here in the same patch — fixing the call-site without fixing the content surfaces the bugs immediately.
2. **The trust posture on the `StreamingPartitionStrategy` trait is too generous.** Strategy implementations are user-pluggable (the trait is `pub`), but the orchestrator panics on bad output (QA-D010-008). Either (a) tighten the trait contract to `Result<Vec<...>, ...>` so strategies surface their own validation, or (b) wrap every public-trait call in a defensive validation pass. Currently the panic surface is exactly the spot where downstream crates get to inject their own bugs into the coordinator's runtime.
3. **`#[cfg(debug_assertions)]` on struct fields is a long-term hazard.** D-009 added `protected_tombstones`. D-010 adds three more. Each one widens the debug-vs-release ABI gap. A future zero-copy / shared-memory transport (SPEC-31?) will hit this; recommend Stage 6 or a follow-up bundle moves all instrumentation into a `NetTelemetry` sidecar.
4. **Three of the four CRITICAL findings (QA-D010-001, -002, -003) are essentially "the doc-comment lies"** — code says one thing, the doc says another, and tests assert the doc. SF-002 (reviewer) has the same flavor. There's a documentation-vs-implementation drift across this bundle that suggests doc-driven development outran the implementation.

---

## Severity-ordered fix queue (for Stage 6 REFACTOR)

| # | Finding | Severity | Owner | Blocking |
|---|---------|----------|-------|----------|
| 1 | QA-D010-001 (exit_streaming_mode disarms delta-mode protections) | CRITICAL | DEVELOPER + ESPECIALISTA EM SPECS | Yes (paired with reviewer MF-002) |
| 2 | QA-D010-002 (RequestWork worker_id impersonation) | CRITICAL | DEVELOPER + ESPECIALISTA EM SPECS | Yes (security) |
| 3 | QA-D010-003 (default_chunked_iter drops FreePort wires) | CRITICAL | DEVELOPER | Yes (T6 isomorphism) |
| 4 | QA-D010-004 (border_id_counter starts at 0; doc lies) | CRITICAL | DEVELOPER | Yes |
| 5 | QA-D010-005 (num_workers=0 panic) | HIGH | DEVELOPER | Should fix |
| 6 | QA-D010-006 (allocate_batch num_workers OOB) | HIGH | DEVELOPER | Should fix |
| 7 | QA-D010-007 (FENNEL alpha=NaN R8 violation) | HIGH | DEVELOPER | Should fix |
| 8 | QA-D010-008 (orchestrator panics on bad strategy output) | HIGH | DEVELOPER | Should fix |
| 9 | QA-D010-009 (max_pending_lifetime unenforced; DoS) | HIGH | DEVELOPER | Should fix |
| 10 | QA-D010-010 (extend_with_chunk_borders worker placeholders) | MEDIUM | DEVELOPER | Yes (paired with reviewer MF-001) |
| 11 | QA-D010-011 (streaming-no-recycle bypasses safety asserts) | MEDIUM | DEVELOPER | Should fix |
| 12 | QA-D010-012 (IT-0591-01/02 vacuous) | MEDIUM | DEVELOPER + CICD | Discretionary |
| 13 | QA-D010-013 (parallel state representations) | MEDIUM | DEVELOPER | Discretionary |
| 14 | QA-D010-014 (debug-only fields ABI drift) | MEDIUM | DEVELOPER | Discretionary |
| 15 | QA-D010-015 (monotonicity checker — actually fine) | LOW | — | No |
| 16 | QA-D010-016 (BorderClean LIFO-only recycling) | LOW | DEVELOPER + ESPECIALISTA EM SPECS | Discretionary |

---

## Posture

**Safe to advance to Stage 6 REFACTOR as-is?** No.

The reviewer's verdict `ACCEPT_WITH_FIXES` accurately reads the *streaming-only* surface (chunk_size != u32::MAX, delta_mode = false, no malicious peers) — that surface is genuinely safe. But the bundle ships:

- A wire variant (RequestWork) with a forged-identity surface (QA-D010-002).
- A default benchmark path (default_chunked_iter) that silently drops wires (QA-D010-003).
- A doc-driven contract (border_id_counter init from FreePort scan) that's never implemented (QA-D010-004).
- A free-list gate (enter/exit_streaming_mode) that, the moment MF-002 is wired up, will silently corrupt delta-mode state (QA-D010-001).

The four CRITICAL findings are independent of MF-001/MF-002 — fixing the call-sites without fixing these surfaces simply *exposes* the bugs faster. Stage 6 MUST address at least the four CRITICALs before the bundle is merged into v2-development's main streaming-correct baseline.

The eight HIGH/MEDIUM findings are deferrable to a follow-up "D-010 hardening" bundle, but several (QA-D010-005, -006, -008, -009) are panic surfaces that any adversarial test suite will hit immediately. The recommended Stage 6 close-out covers items 1-9 in the queue above; items 10-16 may slip to D-011.

— QA Stage 5
