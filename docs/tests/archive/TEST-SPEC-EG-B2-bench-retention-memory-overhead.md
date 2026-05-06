# TEST-SPEC EG-B2: bench retention memory overhead (R31, R23)

**SPEC-20 §7.4 ID:** EG-B2
**Owning task(s):** TASK-0450.
**Type:** benchmark.
**Bench name:** `bench_retention_memory_overhead`.

---

## Inputs / Fixtures

- A large terminating net (e.g., `dual_tree(depth=8)` or `ep_annihilation_con(N=256)`) — chosen to exercise retention non-trivially.
- K_remote ∈ `{4, 8}`.
- Two configurations:
  - `retain_partitions = false` (v1 baseline; no retention; not compatible with `elastic_departure=true`).
  - `retain_partitions = true` (v2 elastic-departure-ready).
- Two reduction modes: v1 and delta.

## Metrics measured

- **Peak resident memory** during the run (use `jemalloc` stats or platform RSS via instrumentation; existing v1 bench infra may already provide this).
- `metrics.retained_*_reclaims_per_round` to confirm retention is exercised (these are 0 if no departure occurs; assert at least the structures are allocated).
- `effective_slots_per_round` to confirm K_eff is correct.

## Pass / fail criteria

Comparative; reports peak memory for both configs. The expected overhead bound (per R31): retention is O(K_eff active workers × partition size) for `retained_last_acked` plus O(sum partition sizes for ever-active workers) for `retained_initial`, capped by the net size.

The bench:
- Reports both peak memories.
- **Asserts** that `retain_partitions = true` peak memory is `<= 2.5x` the `retain_partitions = false` peak (a sanity ceiling; document as a soft threshold to detect a leak, not a tight bound).
- **Asserts** no monotonic memory growth across rounds when no membership changes occur (per R31's atomic-refresh guarantee — retained slots are released on round advance, not accumulated).

## Assertions

| # | Assertion |
|---|-----------|
| A1 | Both configs produce `canonicalise(final) == canonicalise(reduce_all(net))`. |
| A2 | `peak_mem(retain=true) <= 2.5 * peak_mem(retain=false)` (sanity; soft). |
| A3 | Memory at round 5 is not significantly higher than memory at round 2 in the no-membership-change run (slope ≈ 0; assert via linear-fit slope < threshold). |
| A4 | Total interactions match between configs (correctness sanity). |

## Edge / negative cases

- EC: `retain_partitions = true` + `delta_mode = true` — retention payload is `(BorderGraph snapshot, deltas)` not full Partition; expected to be smaller; document the difference.
- EC: `K_remote = 0 + hybrid` — no remote workers; retention overhead is bounded by the self-partition size only.

## Invariants asserted

- R31 atomic refresh discipline (peak memory bounded over rounds with stable membership).

## ARG/DISC/REF citation

None.

## Determinism notes

Wall-clock not deterministic; memory values reported with median + max over 5 repetitions. Allocator flushed between runs to remove pre-warming bias.

## Cross-test dependencies

- TASK-0450 metric fields.
- EG-U13 (atomic release at the unit level — this benchmark is the macro counterpart).
