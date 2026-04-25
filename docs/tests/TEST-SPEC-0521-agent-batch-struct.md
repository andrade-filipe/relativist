# TEST-SPEC-0521: AgentBatch struct (agents + connection_directives + base_agent_id)

**SPEC-21 §7 ID:** T2 (partial).
**Owning task:** TASK-0521.
**Parent spec:** SPEC-21 §4.1 (AgentBatch struct); R14 (Pending entries supported).
**Type:** unit.
**Theory anchor:** None direct.

---

## Inputs / Fixtures

- A 3-agent batch: `AgentBatch { base_agent_id: AgentId(0), agents: [Agent::new(CON), Agent::new(DUP), Agent::new(ERA)], connection_directives: [Resolved { (0,0), (1,1) }, Pending { (2,0), AgentId(10), PortId(0) }] }`.
- A second batch starting at `base_agent_id: AgentId(3)` with 2 agents.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0521-01 | `batch_constructible_with_agents_and_directives` | the 3-agent fixture | construct via struct literal | succeeds; fields readable. |
| UT-0521-02 | `batch_serde_round_trip` | the 3-agent fixture | bincode encode → decode | decoded `== original`. |
| UT-0521-03 | `monotonic_id_assignment_within_batch` | a batch with `base_agent_id = 5`, agents `[CON, DUP]` | inspect resolved AgentIds (5, 6) | both IDs are in the half-open range `[base, base + agents.len())`; strictly increasing. |
| UT-0521-04 | `monotonic_id_assignment_across_batches` | batch 1 `base=0, len=3`; batch 2 `base=3, len=2` | concatenate → effective IDs `[0, 1, 2, 3, 4]` | strictly monotone. |
| UT-0521-05 | `connection_directives_classify_resolved_vs_pending` | the 3-agent fixture (1 Resolved, 1 Pending) | iterate `connection_directives` and match | exactly 1 `Resolved` variant and exactly 1 `Pending` variant. |
| UT-0521-06 | `empty_batch_constructible` | `AgentBatch { base_agent_id: AgentId(0), agents: vec![], connection_directives: vec![] }` | construct + serde round-trip | succeeds; `agents.is_empty() == true`. |
| UT-0521-07 | `derives_present` | struct definition | grep `#[derive(...)]` | contains `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A batch with `agents.len() = 0` but `connection_directives` non-empty | structurally allowed at the type level; downstream pipeline (TASK-0553) MUST treat this as error or degenerate (test deferred to install_connection). |
| EC-2 | `base_agent_id = u32::MAX - 5` with `agents.len() = 10` | construction succeeds; downstream R15 monotonicity will catch arithmetic overflow as a validation error in a later batch (TASK-0544 enforces). |
| EC-3 | Agents with internal connections referencing IDs in the SAME batch | the `Resolved` directive uses `AgentPort` tuples; both endpoints ARE allowed within the batch. |
| EC-4 | A batch carrying a `Pending` directive whose `target_agent` is in a PREVIOUS batch (already processed) | structurally allowed; the install_connection helper MUST treat this as a Resolved-on-arrival case (target is already known). The type does not enforce this; TEST-SPEC-0553 covers the runtime semantics. |

## Invariants asserted

- §4.1 AgentBatch derive set.
- R14 (Pending entries supported via the `connection_directives` field).
- R15 (generator-phase monotonicity) — type-level: NONE; the type does not enforce; enforcement is in TASK-0544 / TEST-SPEC-0544.
- I3' (Uniqueness of AgentIds) — preserved trivially under R15.
- T2 (AgentBatch construction).

## ARG/DISC/REF citation

- None at type level.

## Determinism notes

Pure synchronous, no tokio. Bincode 1.x serde format. Deterministic by construction.

## Cross-test dependencies

- TEST-SPEC-0520 (ConnectionDirective enum) — prerequisite.
- TEST-SPEC-T2 (AgentBatch construction) — this TEST-SPEC IS T2 partial; the spec-catalog T2 extends this with cross-batch verification at the pipeline level.
- TEST-SPEC-0541 (ep_annihilation streaming override) — produces `AgentBatch` instances; UT-0541-* tests behavior at the generator level.
- TEST-SPEC-0544 (R15 monotonicity discipline) — enforces what this struct does NOT enforce.
