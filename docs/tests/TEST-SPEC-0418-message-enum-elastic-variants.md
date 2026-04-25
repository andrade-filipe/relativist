# TEST-SPEC-0418: 5 new `Message` variants + supporting types (R21, R35, R36)

**SPEC-20 §7 ID:** none direct (wire variants; functional flow tested by EG-U10/U10a-c, EG-U14, EG-U15a-b, EG-U19).
**Owning task:** TASK-0418.
**Parent spec:** SPEC-20 §3.5 R21 (`LeaveKind`), R35 (variants 12-16), R35a (ack semantics), R36 (serde+rkyv), R35-cross-spec-version-shape (NF-009).
**Type:** unit (wire round-trip + discriminant stability).

---

## Inputs / Fixtures

- One representative payload per new variant:
  - `JoinRequest { protocol_version: 4, auth_token: None, worker_capabilities: WorkerCapabilities::default() }`
  - `JoinAck { assigned_worker_id: WorkerId(7), partition_index: 2, next_round_number: 13 }`
  - `LeaveRequest { kind: LeaveKind::AfterResult }` and `LeaveRequest { kind: LeaveKind::Urgent }`
  - `LeaveAck`
  - `JoinNack { reason: JoinNackReason::ProtocolVersionMismatch { coordinator: 4, worker: 3 } }` (4 reasons total)
- For NF-009: a corresponding `RegisterNack { reason: RegisterNackReason::ProtocolVersionMismatch { coordinator: 4, worker: 3 } }` payload built from the SPEC-19-side enum, used for byte-shape comparison.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0418-01 | `message_join_request_bincode_roundtrip` | `JoinRequest { protocol_version: 4, auth_token: Some([0x42; 32]), worker_capabilities: WorkerCapabilities::default() }` | bincode serialize → deserialize | `m_out == m_in`. |
| UT-0418-02 | `message_join_ack_bincode_roundtrip` | `JoinAck { assigned_worker_id: WorkerId(7), partition_index: 2, next_round_number: 13 }` | same | `m_out == m_in`. |
| UT-0418-03 | `message_leave_request_bincode_roundtrip` | one `LeaveRequest` per `LeaveKind` variant (2 cases) | same | `m_out == m_in` for both. |
| UT-0418-04 | `message_leave_ack_bincode_roundtrip` | `LeaveAck` | same | `m_out == m_in`. |
| UT-0418-05 | `message_join_nack_bincode_roundtrip` | one `JoinNack` per `JoinNackReason` variant (4 cases incl. `ProtocolVersionMismatch`, `ElasticJoinDisabled`, `WorkerIdSpaceExhausted`, `AuthenticationFailed`) | same | `m_out == m_in` for all four. |
| UT-0418-06 | `discriminant_stability_12_to_16` | enumerate the bincode discriminant byte for each new variant | inspect serialized bytes | The first variant byte encodes 12, 13, 14, 15, 16 respectively (or matches the bincode encoding of the in-source discriminant; assert via `bincode::serialize(&JoinRequest{..})[0..N]` reading the variant tag). |
| UT-0418-07 | `existing_variants_0_through_11_unchanged` | construct one of each existing variant 0..11 | round-trip | All succeed; no discriminant shifted (regression canary). |
| UT-0418-08 | `nf_009_protocol_version_mismatch_payload_shape_aligned` | `RegisterNackReason::ProtocolVersionMismatch { coordinator: 4, worker: 3 }` and `JoinNackReason::ProtocolVersionMismatch { coordinator: 4, worker: 3 }` | bincode serialize each, then compare the field-encoding suffix (skipping the outer enum-discriminant byte) | The two byte-payload tails are identical. NF-009: a v3 worker observes the same payload structure on both rejection paths. |
| UT-0418-09 *(zero-copy)* | `message_new_variants_rkyv_roundtrip` `#[cfg(feature = "zero-copy")]` | each of the 5 new variants | rkyv `to_bytes` → `access` → `deserialize` | All variants round-trip; archived view exposes fields as expected. |
| UT-0418-10 *(zero-copy)* | `leave_kind_and_join_nack_reason_rkyv_roundtrip` `#[cfg(feature = "zero-copy")]` | each variant of both helper enums | same | All round-trip. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `JoinRequest.auth_token = None` and `auth_token = Some([0u8; 32])` | Both round-trip; `Option<[u8; 32]>` encodes a discriminant byte. |
| EC-2 | `WorkerCapabilities` is currently empty | Round-trip OK; zero-byte payload. |
| EC-3 | `JoinAck.assigned_worker_id = WorkerId(0)` | Round-trip OK (WorkerId 0 is reserved for the self-partition; wire is indifferent). |
| EC-4 | `JoinAck.next_round_number = u32::MAX` | Round-trip OK. |

## Invariants asserted

None directly (wire surface).

## ARG/DISC/REF citation

None.

## Determinism notes

Pure synchronous serde / rkyv; deterministic.

## Cross-test dependencies

- UT-0418-08 is the NF-009 closure anchor — must remain green for both the SPEC-19 and SPEC-20 rejection paths. EG-U15a / EG-U15b consume this guarantee.
- UT-0418-07 prevents an accidental reorder of pre-existing variants when adding the new ones.
