//! TASK-0600 — Collapse parallel `Pull*` / `PullCoordinatorState` types
//! (QA-D010-013).
//!
//! `PullCoordinatorState` is the canonical per-FSM pull-state type. The
//! `CoordinatorState::Pull*` variants are kept for ABI stability (UT-0577-18)
//! but are constructed *only* via the `From<PullCoordinatorState>` projection
//! defined in `relativist-core/src/coordinator.rs`. Production code MUST NOT
//! assign the projected variants directly.
//!
//! IT-0600-03 below is the structural regression fence: it grep-scans the
//! production sources under `src/protocol/{coordinator,worker}.rs` and the
//! coordinator FSM (`src/coordinator.rs`) for any assignment of the form
//! `= CoordinatorState::Pull*` (which would bypass the canonical projection).
//! If any production line outside the `From` impl matches, the test fails
//! with a clear message naming the offending file + line.
//!
//! References:
//! - SPEC-13 §x — coordinator FSM state catalog.
//! - SPEC-21 §3.8 A5 — pull-only FSM extensions (5 coordinator + 1 terminal).
//! - TASK-0600 dispatch brief (2026-04-30): `PullCoordinatorState` chosen
//!   as canonical (the more specific name; the richer state set).

/// IT-0600-03 — `no_control_flow_reads_both_types`.
///
/// Source-text inspection: production code in `src/coordinator.rs` and
/// `src/protocol/{coordinator,worker}.rs` MUST NOT directly construct
/// `CoordinatorState::Pull*` — the only allowed producer is the
/// `From<PullCoordinatorState> for CoordinatorState` impl. The test scans
/// for the assignment pattern `= CoordinatorState::Pull` and fails if any
/// match is found outside the canonical `From` impl block.
#[test]
fn it_0600_03_no_control_flow_reads_both_types() {
    let coordinator_rs = include_str!("../src/coordinator.rs");
    let proto_coord_rs = include_str!("../src/protocol/coordinator.rs");
    let proto_worker_rs = include_str!("../src/protocol/worker.rs");

    // Exempt source: the `From<PullCoordinatorState>` impl in coordinator.rs
    // is the canonical projection — its body legitimately produces
    // `CoordinatorState::Pull*` variants. We exclude this region from the
    // scan by anchoring on a unique marker inside the impl block.
    let canonical_marker = "impl From<PullCoordinatorState> for CoordinatorState";

    // ----- coordinator.rs scan -----
    // Split on the canonical impl marker to exclude its body from the scan.
    let (before_canonical, after_marker) = coordinator_rs
        .split_once(canonical_marker)
        .expect("coordinator.rs must contain the canonical From<PullCoordinatorState> impl");
    // The impl block extends until the next `}` at column 0 followed by a
    // top-level item — pragmatic heuristic: skip the next ~600 characters
    // (the impl body is small) and resume scanning after.
    // Safer: find the next "// /// Events that drive the pull-dispatch FSM"
    // sentinel, which is the comment immediately following the impl.
    let resume_marker = "/// Events that drive the pull-dispatch FSM";
    let after_canonical = after_marker
        .split_once(resume_marker)
        .map(|(_, rest)| rest)
        .unwrap_or(after_marker);

    let scannable = format!("{before_canonical}{after_canonical}");

    // Match any line that constructs `CoordinatorState::Pull` via assignment
    // (`= CoordinatorState::Pull...`) — this is the production-code drift
    // pattern we are fencing against. Doc-comments, test code, and the
    // canonical impl are NOT counted (test code is in `mod tests` blocks
    // gated by `#[cfg(test)]`, and we excluded the canonical impl above).
    //
    // The pattern excludes test code by also excluding lines that appear
    // inside a `#[cfg(test)]` module. We use a coarse approximation: lines
    // inside `mod tests { ... }` blocks. Since the entire tests module is
    // at the bottom of the file, we truncate at the test marker.
    let test_marker = "#[cfg(test)]";
    let production_only = scannable
        .split_once(test_marker)
        .map(|(prod, _)| prod)
        .unwrap_or(&scannable);

    let offending: Vec<_> = production_only
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("= CoordinatorState::Pull"))
        .map(|(i, line)| format!("  coordinator.rs:line~{}: {}", i + 1, line.trim()))
        .collect();

    assert!(
        offending.is_empty(),
        "IT-0600-03: production code in coordinator.rs MUST NOT assign \
         CoordinatorState::Pull* directly — use From<PullCoordinatorState>. \
         Offenders:\n{}",
        offending.join("\n")
    );

    // ----- protocol/coordinator.rs scan -----
    let proto_coord_offending: Vec<_> = proto_coord_rs
        .split_once(test_marker)
        .map(|(prod, _)| prod)
        .unwrap_or(proto_coord_rs)
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("= CoordinatorState::Pull"))
        .map(|(i, line)| format!("  protocol/coordinator.rs:line~{}: {}", i + 1, line.trim()))
        .collect();

    assert!(
        proto_coord_offending.is_empty(),
        "IT-0600-03: protocol/coordinator.rs MUST NOT assign \
         CoordinatorState::Pull* directly. Offenders:\n{}",
        proto_coord_offending.join("\n")
    );

    // ----- protocol/worker.rs scan (defensive — workers should not touch
    // coordinator-side state, but we scan anyway as a drift fence). -----
    let proto_worker_offending: Vec<_> = proto_worker_rs
        .split_once(test_marker)
        .map(|(prod, _)| prod)
        .unwrap_or(proto_worker_rs)
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("= CoordinatorState::Pull"))
        .map(|(i, line)| format!("  protocol/worker.rs:line~{}: {}", i + 1, line.trim()))
        .collect();

    assert!(
        proto_worker_offending.is_empty(),
        "IT-0600-03: protocol/worker.rs MUST NOT touch CoordinatorState::Pull*. \
         Offenders:\n{}",
        proto_worker_offending.join("\n")
    );
}
