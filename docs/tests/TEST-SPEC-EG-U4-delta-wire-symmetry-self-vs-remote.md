# TEST-SPEC EG-U4-delta-wire-symmetry: self-worker delta payload symmetry with remote (R4-delta-self-symmetry, NF-003)

**SPEC-20 §7.1 ID:** EG-U4-delta-wire-symmetry
**Owning task(s):** TASK-0437.
**Type:** unit (instrumented; cross-transport).
**Test name:** `test_self_worker_delta_round_result_shape_matches_remote`.

**Closes:** NF-003 (self-worker delta loop must match remote-worker delta loop bit-for-bit on the wire).

---

## Inputs / Fixtures

- A single deterministic partition `p` (chosen for non-trivial border interaction; e.g. a CON-DUP cascade boundary).
- `BorderGraph bg` snapshot.
- Two transports:
  - `(a)` `ChannelTransport` connecting the in-process self-worker to the coordinator (the shortcut path).
  - `(b)` an instrumented "loopback transport" — a real `tokio::io::duplex` (or a real `TcpStream` on `127.0.0.1`) carrying the same partition through the worker binary code path.

Both runs use the same seed/RNG so that any internal nondeterminism is pinned.

## Expected behaviour

Both transports produce a `RoundResult { border_deltas, .. }`. The two `border_deltas` payloads must contain the **same set of `(border_id, delta_kind)` entries** in the **same canonical order**.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `result_self.border_deltas.iter().collect::<BTreeSet<_>>() == result_remote.border_deltas.iter().collect::<BTreeSet<_>>()` (set equality). |
| A2 | After canonical sort by `border_id`, `result_self.border_deltas == result_remote.border_deltas` (sequence equality). |
| A3 | `result_self.partition_after_reduce` and `result_remote.partition_after_reduce` have the same canonicalised form. |
| A4 | If `result_self` is observed via the wire byte-stream (rkyv/bincode), the byte representation matches `result_remote` modulo any framing prefix that ChannelTransport elides. |

## Edge / negative cases

- EC-1: partition with **zero** redexes — both deltas are empty; result_self.border_deltas == result_remote.border_deltas == [].
- EC-2: partition reaches normal form within a single round — both transports emit identical "no further rounds needed" signal.
- EC-3: partition emits an emergent border redex (CON-DUP between two crossings) — both transports report the same emergent redex on the same border.

## Invariants asserted

- D3 (Border Completeness) — both transports report the same border deltas.
- G1 conditional via ARG-005 — wire symmetry is an *additional* discipline that prevents a regression where the self-worker shortcut diverges silently.

## ARG/DISC/REF citation

None directly. NF-003 closure anchor.

## Determinism notes

**Critical.** Two independent runs of a tokio-driven worker code path must agree byte-for-byte. Strategy:
- Both runs use `#[tokio::test(flavor = "current_thread", start_paused = true)]`.
- The reduction within the partition is purely synchronous (no async inside the inner loop).
- `border_deltas` ordering is captured via instrumentation (sort by `border_id` ascending before assertion); ANY non-canonical ordering at the source is flagged as a separate bug.
- RNG, if any, uses a fixed seed.
- The "remote" worker is a same-process tokio task connected via duplex/loopback so no network jitter.

## Cross-test dependencies

- EG-U4-delta uses the same partition fixture but only checks the apply-deltas path.
- EG-I1-delta is the end-to-end integration check.
