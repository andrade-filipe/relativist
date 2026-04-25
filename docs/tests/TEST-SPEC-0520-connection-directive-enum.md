# TEST-SPEC-0520: ConnectionDirective enum (Resolved + Pending variants)

**SPEC-21 §7 ID:** plumbing (T2 partial — AgentBatch construction depends on this).
**Owning task:** TASK-0520.
**Parent spec:** SPEC-21 §4.1 (ConnectionDirective enum); R14 (forward references via Pending).
**Type:** unit (struct construction + serde + variant invariants).
**Theory anchor:** None direct (data type).

---

## Inputs / Fixtures

- A `ConnectionDirective::Resolved { from: AgentPort(0, 0), to: AgentPort(1, 1) }` instance.
- A `ConnectionDirective::Pending { from: AgentPort(0, 0), target_agent: AgentId(50), target_port: PortId(2) }` instance.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0520-01 | `resolved_variant_constructible` | the enum | construct `Resolved { from, to }` | constructs without panic; structural fields readable. |
| UT-0520-02 | `pending_variant_constructible` | the enum | construct `Pending { from, target_agent, target_port }` | constructs; fields readable. |
| UT-0520-03 | `resolved_serde_round_trip` | the Resolved fixture | bincode encode → decode | decoded `== original` (`PartialEq`). |
| UT-0520-04 | `pending_serde_round_trip` | the Pending fixture | bincode encode → decode | decoded `== original`. |
| UT-0520-05 | `pending_target_port_accepts_zero_one_two` | construct `Pending` with `target_port: PortId(0)`, then `(1)`, then `(2)` | each construction | succeeds; the enum does NOT validate port range — that is the caller's responsibility (per task acceptance criteria). |
| UT-0520-06 | `derives_present` | the enum definition | grep `#[derive(...)]` | contains at minimum `Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize`, `Deserialize` (per SPEC-21 §4.1 derive set + project coding standards). |
| UT-0520-07 | `variants_distinguishable` | one Resolved and one Pending fixture (with the same `from` field) | `==` comparison | `false` (different variants are unequal regardless of shared field values). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `Pending { target_agent: AgentId(u32::MAX), .. }` | constructs OK; serde round-trip preserves the value. |
| EC-2 | `Pending { target_port: PortId(255) }` (out-of-range value) | constructs OK at the enum level (no validation); a downstream consumer (TASK-0553 install_connection) is responsible for surfacing this as an error. The enum-level test does NOT assert error semantics. |
| EC-3 | A future amendment adding a third variant | UT-0520-06 derive list MUST be propagated to the new variant; UT-0520-07 MUST be extended to cover the new variant pair. |

## Invariants asserted

- SPEC-21 §4.1 derive set.
- R14 (forward-reference via `Pending`).
- T2 partial (AgentBatch construction depends on this).

## ARG/DISC/REF citation

- None at type level.

## Determinism notes

Pure synchronous, no tokio, no RNG. Bincode 1.x serde format (consistent with v1 wire format).

## Cross-test dependencies

- TEST-SPEC-0521 (AgentBatch struct) — depends on this enum being constructible.
- TEST-SPEC-T2 (AgentBatch construction behavioral) — extends this with batch-level monotonicity.
- TEST-SPEC-0553 (install_connection helper) — consumes both variants and resolves Pending → Resolved at runtime.
