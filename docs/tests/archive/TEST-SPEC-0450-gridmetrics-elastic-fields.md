# TEST-SPEC-0450: `GridMetrics` 7 elastic fields + R45 disjointness audit (R38, R38a, R38b)

**SPEC-20 §7 ID:** none direct (consumed by EG-B1, EG-B2, EG-B3).
**Owning task:** TASK-0450.
**Parent spec:** SPEC-20 §3.6 R38, R38a (NF-004 audit), R38b (closes SC-027).
**Type:** unit + sentinel.

---

## Inputs / Fixtures

- A `GridMetrics` instance built from a stub round (1 worker, 0 joins, 0 departures) — useful for default-state assertions.
- Hand-built coordinator round transitions that trigger each lifecycle hook (join → joined++, depart → departed++, re-split → redispatched++, etc.).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0450-01 | `grid_metrics_has_all_seven_elastic_fields` | `let m: GridMetrics = Default::default()` | inspect via reflection or hand-coded field assertions | All 7 new vec fields exist and are empty: `workers_joined_per_round`, `workers_departed_per_round`, `effective_slots_per_round`, `partitions_redispatched_per_round`, `retained_initial_reclaims_per_round`, `retained_last_acked_reclaims_per_round`, `join_round_overhead_ms_per_round`. |
| UT-0450-02 | `worker_round_stats_has_is_coordinator_self` | `let s: WorkerRoundStats = ...` | inspect | `s.is_coordinator_self: bool` field exists; default is `false`. |
| UT-0450-03 | `workers_joined_recorded_on_membership_window_close` | a stub coordinator transition that joins 2 workers in a round; emit `AcceptingMembershipChanges → ...` | drive 1 round | `m.workers_joined_per_round.last() == Some(&2)`. |
| UT-0450-04 | `workers_departed_recorded_on_membership_window_close` | stub transition with 1 departure | drive 1 round | `m.workers_departed_per_round.last() == Some(&1)`. |
| UT-0450-05 | `effective_slots_recorded_at_partitioning_entry` | stub round with K=3 remote + hybrid (K_eff=4) | drive | `m.effective_slots_per_round.last() == Some(&4)`. |
| UT-0450-06 | `retained_initial_reclaims_increment_on_r24a` | stub round where a never-completed worker departs | drive | `m.retained_initial_reclaims_per_round.last() == Some(&1)`; `retained_last_acked_reclaims_per_round.last() == Some(&0)`. |
| UT-0450-07 | `retained_last_acked_reclaims_increment_on_r24b` | stub round where a worker with N≥1 successful rounds departs | drive | `retained_last_acked_reclaims_per_round.last() == Some(&1)`; `retained_initial_reclaims_per_round.last() == Some(&0)`. |
| UT-0450-08 | `partitions_redispatched_increment_on_resplit` | stub round triggering re-split (departure scenario) | drive | `m.partitions_redispatched_per_round.last() == Some(&K_eff_new)`. |
| UT-0450-09 | `join_round_overhead_recorded_in_delta_mode` | stub delta-mode rejoin with measurable wall-clock cost | drive 1 round | `m.join_round_overhead_ms_per_round.last() == Some(&x)` with `x > 0` (pure-CPU stub may be 0; in that case relax to `>= 0` and assert presence rather than positivity). |
| UT-0450-10 | `nf_004_field_name_disjointness_with_spec19_r45` | enumerate the field names of `GridMetrics` from SPEC-19 R45 (8 fields) and SPEC-20 R38 (7 fields) | check for collision via a hand-coded `assert_fields_disjoint!` macro OR a runtime enumeration test that lists both sets | The two sets are disjoint. Specifically: SPEC-19 R45's prefixes (whatever they are) do NOT overlap with the SPEC-20 R38 prefixes `workers_`, `effective_`, `partitions_`, `retained_`, `join_round_`. **The test fails loudly if a future field is added on either side that collides.** |
| UT-0450-11 | `grid_metrics_serde_roundtrip` | `m` with at least one entry per new vec | `bincode::serialize` → deserialize | `m_out == m_in`. |
| UT-0450-12 | `worker_round_stats_is_coordinator_self_true_for_self_partition` | a stub run with `hybrid_coordinator=true`; capture self-partition's `WorkerRoundStats` | inspect | `s.is_coordinator_self == true`. |
| UT-0450-13 | `worker_round_stats_is_coordinator_self_false_for_remote` | same run; capture remote worker's stats | inspect | `s.is_coordinator_self == false`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A round with 0 joins and 0 departures | The vecs grow by one element each: `workers_joined_per_round.push(0)`, `workers_departed_per_round.push(0)`. (The vec length tracks rounds, not membership-change events.) |
| EC-2 | A round with simultaneous join + departure | Both per-round counters reflect the per-round counts; UT-0450-03 + UT-0450-04 hold simultaneously. |

## Invariants asserted

None.

## ARG/DISC/REF citation

None directly. Underpins ROADMAP §2.40 break-even analysis.

## Determinism notes

Stub-driven tests are pure synchronous transitions. UT-0450-09 (timing measurement) is the only test with a wall-clock dependency; relaxed to "presence, not value" if measurement is sub-microsecond.

## Cross-test dependencies

- UT-0450-10 is the NF-004 closure anchor; it MUST remain green forever. Any new metric field added on either SPEC-19 or SPEC-20 side must be added to its enumerated list.
- EG-B1 / EG-B2 / EG-B3 read these fields as their measurement surface.
