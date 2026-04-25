# TASK-0418: Extend `Message` enum with 5 elastic-grid variants + supporting payload types

**Spec:** SPEC-20 §3.5 R21 (`LeaveKind`), R35 (discriminants 12-16), R35a (ack semantics), R36 (serde+rkyv), R35-cross-spec-version-shape (NF-009).
**Requirements:** R21 (`LeaveKind`), R35 (variants: `JoinRequest`, `JoinAck`, `LeaveRequest`, `LeaveAck`, `JoinNack`), R35a (JoinNackReason 4 cases), R36, NF-009 payload alignment with `RegisterNack`.
**Priority:** P0 (wire protocol; blocker for every join/leave runtime path).
**Status:** TODO
**Depends on:** TASK-0417 (version bump must be visible), TASK-0411 *(reuses `PartitionError` pattern for validation; not a hard block)*.
**Blocked by:** TASK-0417.
**Estimated complexity:** M (~120-180 LoC production + ~80 LoC bincode/rkyv round-trip tests)
**Bundle:** SPEC-20 Elastic Grid — wire protocol foundations.

## Context

SPEC-20 appends 5 new `Message` variants in the order shown at R35, preserving discriminant stability with SPEC-19's discriminants 7-11. `LeaveKind` is a supporting enum for `LeaveRequest`; `JoinNackReason` has 4 cases including `ProtocolVersionMismatch { coordinator: u32, worker: u32 }` whose payload shape MUST match the SPEC-19 R37 `RegisterNack` version-rejection path per NF-009.

## Acceptance Criteria

- [ ] Add `LeaveKind { AfterResult, Urgent }` enum with `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`.
- [ ] Add `WorkerCapabilities { /* empty */ }` placeholder struct with `Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize`.
- [ ] Add `JoinNackReason` enum with 4 variants:
  - `ProtocolVersionMismatch { coordinator: u32, worker: u32 }` (NF-009 shape — MUST align with `RegisterNack` version-mismatch payload).
  - `ElasticJoinDisabled`.
  - `WorkerIdSpaceExhausted` (SC-023).
  - `AuthenticationFailed` (SPEC-10 link).
- [ ] Extend `Message` enum with discriminants 12-16 per R35:
  - `12: JoinRequest { protocol_version: u32, auth_token: Option<[u8; 32]>, worker_capabilities: WorkerCapabilities }`
  - `13: JoinAck { assigned_worker_id: WorkerId, partition_index: u32, next_round_number: u32 }`
  - `14: LeaveRequest { kind: LeaveKind }`
  - `15: LeaveAck` (unit variant)
  - `16: JoinNack { reason: JoinNackReason }`
- [ ] R35a ack semantics documented on `LeaveAck` / `LeaveRequest`: the coordinator sends `LeaveAck` BEFORE closing the TCP stream; workers MUST NOT close their connection before receiving `LeaveAck`.
- [ ] `Shutdown` (SPEC-06) remains reserved for coordinator-initiated termination ONLY; docstring updated to cross-reference R35a / SC-017.
- [ ] All new types derive serde Serialize/Deserialize.
- [ ] Under `#[cfg(feature = "zero-copy")]` all new types derive `rkyv::{Archive, Serialize, Deserialize}` + bytecheck (R36).
- [ ] bincode round-trip tests for each variant (6 tests: 4 messages + `LeaveKind` + `JoinNackReason`).
- [ ] rkyv round-trip tests mirrored under `--features zero-copy`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/types.rs` *(or `message.rs`)* | modify | Append 5 new variants + 3 supporting types (LeaveKind, WorkerCapabilities, JoinNackReason). |
| `relativist-core/src/protocol/error.rs` | modify *(if needed)* | Add any `JoinRequest` decode-error variants. |

## Key Types / Signatures

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub enum LeaveKind { AfterResult, Urgent }

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct WorkerCapabilities { /* reserved v3 */ }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub enum JoinNackReason {
    ProtocolVersionMismatch { coordinator: u32, worker: u32 },
    ElasticJoinDisabled,
    WorkerIdSpaceExhausted,
    AuthenticationFailed,
}

pub enum Message {
    // ... existing 0-11 ...
    // 12:
    JoinRequest { protocol_version: u32, auth_token: Option<[u8; 32]>, worker_capabilities: WorkerCapabilities },
    // 13:
    JoinAck { assigned_worker_id: WorkerId, partition_index: u32, next_round_number: u32 },
    // 14:
    LeaveRequest { kind: LeaveKind },
    // 15:
    LeaveAck,
    // 16:
    JoinNack { reason: JoinNackReason },
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0418 formalizes UT-0418-01..08:

- bincode round-trip for each of the 5 new variants.
- `LeaveKind`, `JoinNackReason` round-trip.
- rkyv round-trip under `--features zero-copy` for each.
- Discriminant stability: assert numeric discriminant values 12-16 are stable.

Also: EG-U14 (WorkerId exhaustion → `JoinNack { WorkerIdSpaceExhausted }`), EG-U15a/b (version mismatch), EG-U19 (`LeaveAck` before close).

## Invariants Touched

- None directly (wire-surface change); feeds into D6 (Protocol Termination) via R35a ack semantics.

## Notes

- **NF-009 alignment**: the `JoinNackReason::ProtocolVersionMismatch { coordinator, worker }` shape MUST align bit-exactly with the `RegisterNack::ProtocolVersionMismatch` shape that SPEC-19 R37's next revision will adopt. If the two diverge, a v3 worker hitting a v4 coordinator via the two different rejection paths would observe different error shapes — forbidden by R35-cross-spec-version-shape.
- **Discriminant stability**: do NOT reorder any existing variants; append only.

## DAG Links

- **Predecessors:** TASK-0417.
- **Successors:** TASK-0419 (handshake branch), TASK-0432 (JoinRequest/Ack flow), TASK-0441 (LeaveRequest/Ack flow), EG-U14/15a/15b/19.
