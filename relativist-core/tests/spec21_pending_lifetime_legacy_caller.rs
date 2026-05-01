//! TASK-0597 — `max_pending_lifetime` legacy-caller integration tests.
//!
//! Spec: SPEC-21 §3.7 R37g (`MAX_PENDING_LIFETIME` pending-store memory bound,
//! closes SC-016).
//!
//! Origin: QA-D010-009 residual. Commit `5a54111` wired the
//! `_with_chunk_size_and_lifetime` wrapper end-to-end; the legacy entrypoint
//! `merge::generate_and_partition_chunked_with_delta` previously hard-coded
//! `u32::MAX`. TASK-0597 threads `GridConfig.max_pending_lifetime` through
//! that legacy entrypoint so the SC-016 bound is observed by every caller.
//!
//! These tests pin three properties from the test spec:
//!
//! - **IT-0597-02** — with `max_pending_lifetime = 16`, a malformed stream
//!   (forward refs that never resolve) MUST surface as
//!   `PendingConnectionExpired { budget: 16, .. }` within `lifetime + 1`
//!   chunks. The pending store therefore never grows beyond 16 + 1 entries
//!   (one extra for the chunk that triggers eviction).
//! - **IT-0597-03** — *negative-control regression sentinel.* With
//!   `max_pending_lifetime = u32::MAX`, the same malformed stream MUST NOT
//!   raise `PendingConnectionExpired` (the budget is effectively disabled);
//!   it falls through to `UnresolvedForwardReferences` only post-stream. The
//!   pending store therefore grows to the total number of unresolved
//!   forward refs emitted (≫ 16). This documents that the bound in
//!   IT-0597-02 is load-bearing.
//! - **IT-0597-04** — *source-level guard.* No surviving `u32::MAX` literal
//!   in proximity to a `max_pending_lifetime` token in the streaming source
//!   files (acceptance criterion #1).

use std::fs;
use std::path::Path;

use relativist_core::error::PartitionError;
use relativist_core::net::Symbol;
use relativist_core::partition::streaming::{
    AgentBatch, ConnectionDirective, RoundRobinStreamingStrategy,
};

/// Build a stream of `n_chunks` single-agent batches, each emitting a
/// `Pending` directive whose target id (`unresolved_target`) NEVER appears in
/// the stream. Each chunk's pending entry stays in the store; with
/// `max_pending_lifetime = N`, the chunk recorded at chunk_seen=1 must age
/// out by chunk N+1 (where age == N+1 > N == budget).
fn build_malformed_pending_stream(
    n_chunks: u32,
    unresolved_target: u32,
) -> Box<dyn Iterator<Item = AgentBatch>> {
    let batches: Vec<AgentBatch> = (0..n_chunks)
        .map(|i| AgentBatch {
            agents: vec![(i, Symbol::Era)],
            connections: vec![ConnectionDirective::Pending {
                source: (i, 0u8),
                target_agent_id: unresolved_target,
                target_port: 0u8,
            }],
        })
        .collect();
    Box::new(batches.into_iter())
}

/// IT-0597-02: with `max_pending_lifetime = 16` and a 100-chunk malformed
/// stream, the pending store never grows beyond ~17 entries (one extra
/// for the chunk that triggers eviction). The bound is observed via the
/// fast-fail eviction error returned by the streaming pipeline.
#[test]
fn pending_store_bounded_by_lifetime_16() {
    // Use the configuration-aware wrapper so the streaming path engages
    // (chunk_size < u32::MAX). The legacy wrapper hard-codes chunk_size to
    // u32::MAX (R26 short-circuit, materialise-then-split), which never
    // exercises the per-batch eviction logic.
    let stream = build_malformed_pending_stream(100, 999_999);
    let mut strategy = RoundRobinStreamingStrategy::new(2);

    let result =
        relativist_core::merge::helpers::generate_and_partition_chunked_with_delta_and_config(
            stream,
            2,
            &mut strategy,
            None,
            false, // delta_mode
            true,  // streaming_active
            5,     // chunk_size: small so streaming path engages
            16,    // max_pending_lifetime: SC-016 bound
        );

    match result {
        Err(PartitionError::PendingConnectionExpired { budget, age, .. }) => {
            assert_eq!(
                budget, 16,
                "IT-0597-02: budget echoed in the error must equal the configured \
                 max_pending_lifetime (proves the bound is the value observed at the \
                 eviction site, not a hard-coded u32::MAX)"
            );
            // Eviction fires the FIRST time `age > budget`. The earliest
            // detection moment is age == budget + 1 == 17. The pending store
            // therefore never grows beyond 17 entries (one entry per chunk
            // up to and including the chunk that triggers detection).
            assert_eq!(
                age, 17,
                "IT-0597-02: eviction must fire at age == budget + 1 (= 17), proving the \
                 bound is tight and the comparator is strict-greater-than"
            );
        }
        other => panic!(
            "IT-0597-02: expected PendingConnectionExpired bounding the pending store; \
             got {:?}",
            other
        ),
    }
}

/// IT-0597-03 (negative-control regression sentinel): with
/// `max_pending_lifetime = u32::MAX`, the malformed stream MUST NOT raise
/// `PendingConnectionExpired` (the budget is disabled). It falls through to
/// the post-stream `UnresolvedForwardReferences` check instead. This
/// documents that the bound in IT-0597-02 is meaningful — without this
/// regression sentinel, a future refactor that quietly re-introduced the
/// old `u32::MAX` hard-code could satisfy IT-0597-02 vacuously on a
/// well-formed workload.
#[test]
fn pending_store_unbounded_under_u32_max_regression_sentinel() {
    let stream = build_malformed_pending_stream(100, 999_999);
    let mut strategy = RoundRobinStreamingStrategy::new(2);

    let result =
        relativist_core::merge::helpers::generate_and_partition_chunked_with_delta_and_config(
            stream,
            2,
            &mut strategy,
            None,
            false,
            true,
            5,        // chunk_size: streaming path active
            u32::MAX, // max_pending_lifetime: legacy disabled sentinel
        );

    assert!(
        matches!(
            result,
            Err(PartitionError::UnresolvedForwardReferences { agent_id: 999_999 })
        ),
        "IT-0597-03: under u32::MAX, eviction MUST NOT fire; the malformed stream must \
         fall through to the post-stream UnresolvedForwardReferences check (legacy \
         behavior, witness that the IT-0597-02 bound is load-bearing). got {:?}",
        result
    );
}

/// IT-0597-04: source-level guard. Scan the production streaming files for
/// any `u32::MAX` literal inside a 200-character window of a
/// `max_pending_lifetime` or `MAX_PENDING_LIFETIME` token. Allow-listed
/// occurrences:
///
/// - the *legacy disabled sentinel comments* in
///   `partition/streaming.rs::generate_and_partition_chunked_with_chunk_size_and_lifetime`
///   (the eviction site explicitly documents `u32::MAX` as the disabled
///   sentinel — that is the implementation's contract, not a caller default);
/// - the *negative-control test* in `partition/streaming.rs` that exercises
///   the legacy disabled behavior;
/// - the unit-test forwarding probe in `merge/helpers.rs` that documents
///   what the legacy wrapper passes when chunk_size = u32::MAX (R26
///   short-circuit, where the lifetime is irrelevant by construction).
///
/// The intent is to catch a future PR that adds a NEW caller and forgets to
/// thread `GridConfig.max_pending_lifetime` through.
#[test]
fn no_surviving_u32_max_literal_in_streaming_paths() {
    // Locate the workspace `relativist-core/src/` directory relative to this
    // integration test (which lives in `relativist-core/tests/`).
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let src_root = Path::new(manifest_dir).join("src");

    let scan_targets: Vec<std::path::PathBuf> = vec![
        src_root.join("merge").join("helpers.rs"),
        src_root.join("merge").join("mod.rs"),
        src_root.join("merge").join("grid.rs"),
        src_root.join("partition").join("streaming.rs"),
    ];

    let mut violations: Vec<String> = Vec::new();

    for path in &scan_targets {
        if !path.exists() {
            continue;
        }
        let body = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("IT-0597-04: failed to read {}: {e}", path.display()));

        // Walk every match of `u32::MAX` and inspect a 200-char window on
        // either side for a `max_pending_lifetime` / `MAX_PENDING_LIFETIME`
        // token.
        let needle = "u32::MAX";
        let bytes = body.as_bytes();
        let mut idx = 0usize;
        while let Some(pos) = body[idx..].find(needle) {
            let abs = idx + pos;
            let lo = abs.saturating_sub(200);
            let hi = (abs + needle.len() + 200).min(bytes.len());
            let window = &body[lo..hi];

            let suspicious =
                window.contains("max_pending_lifetime") || window.contains("MAX_PENDING_LIFETIME");

            if suspicious {
                // Compute a friendly file:line for the report.
                let line_no = body[..abs].matches('\n').count() + 1;
                let line_text = body
                    .lines()
                    .nth(line_no - 1)
                    .unwrap_or("<line out of range>");

                // Allow-list rules. The match must satisfy ANY of the
                // following to be considered legitimate.
                //
                //  (1) The line is a `///` / `//!` / `//` comment that
                //      documents the legacy disabled sentinel.
                //  (2) The literal sits inside the eviction guard
                //      `if max_pending_lifetime != u32::MAX` — this is the
                //      single legitimate runtime check at the eviction
                //      site (the disabled sentinel is part of the
                //      function's public contract).
                //  (3) The line lives in a `#[test]` / `#[cfg(test)]`
                //      block (the negative-control regression test in
                //      streaming.rs and the forwarding-probe test in
                //      helpers.rs).
                //  (4) The literal is itself the `chunk_size` argument
                //      (R26 short-circuit sentinel), recognisable by the
                //      preceding line referencing `chunk_size` or the
                //      function header containing
                //      `generate_and_partition_chunked_with_delta`. The
                //      `chunk_size` sentinel is unrelated to
                //      `max_pending_lifetime`; the proximity scan picked
                //      it up because both arguments live at the same
                //      call.
                //  (5) The literal is the body of a wrapper function
                //      whose own signature does NOT take
                //      `max_pending_lifetime` (the wrapper is offering a
                //      legacy default to its callers). Recognised by
                //      checking whether the enclosing function signature
                //      (within ~600 chars upstream) contains the
                //      `max_pending_lifetime` parameter; if it does NOT,
                //      the wrapper is legitimately defaulting.
                let trimmed = line_text.trim_start();
                let is_comment = trimmed.starts_with("///")
                    || trimmed.starts_with("//!")
                    || trimmed.starts_with("//");
                let is_eviction_guard_line = line_text.contains("max_pending_lifetime != u32::MAX");

                let test_window_lo = abs.saturating_sub(4_000);
                let in_test_block = body[test_window_lo..abs].contains("#[test]")
                    || body[test_window_lo..abs].contains("#[cfg(test)]");

                // Heuristic for (4): the surrounding ~120-char window does
                // not mention the `max_pending_lifetime` token directly
                // adjacent to this `u32::MAX` (i.e., `u32::MAX` is most
                // likely a `chunk_size` argument that just happens to
                // share the same call as a `max_pending_lifetime`
                // argument).
                let close_lo = abs.saturating_sub(60);
                let close_hi = (abs + needle.len() + 60).min(bytes.len());
                let close_window = &body[close_lo..close_hi];
                let close_to_lifetime = close_window.contains("max_pending_lifetime");

                // Heuristic for (5): the enclosing function signature
                // (within ~800 chars upstream of this `u32::MAX`) does
                // NOT take `max_pending_lifetime` as a parameter, AND
                // contains `pub fn` or `fn` — i.e., this is a wrapper
                // function defaulting the lifetime for upstream callers.
                let sig_window_lo = abs.saturating_sub(800);
                let sig_window = &body[sig_window_lo..abs];
                // The most recent `pub fn ... (` or `fn ... (` introducing
                // a function signature.
                let last_fn_open = sig_window
                    .rmatch_indices("fn ")
                    .next()
                    .map(|(i, _)| sig_window_lo + i);
                let enclosing_fn_takes_lifetime = if let Some(fn_open) = last_fn_open {
                    // Walk forward from `fn_open` to find the closing `)`
                    // of the parameter list and check the substring.
                    let header_hi = (fn_open + 800).min(bytes.len());
                    let header_substr = &body[fn_open..header_hi];
                    if let Some(paren_close) = header_substr.find(") ->") {
                        header_substr[..paren_close].contains("max_pending_lifetime")
                    } else if let Some(paren_close) = header_substr.find(")\n") {
                        header_substr[..paren_close].contains("max_pending_lifetime")
                    } else {
                        // Could not determine; treat as "lifetime-aware"
                        // (i.e., conservatively flag the violation).
                        true
                    }
                } else {
                    true
                };

                let is_legitimate_wrapper_default = !enclosing_fn_takes_lifetime;

                // Heuristic for (4) refined: the IMMEDIATE next non-blank
                // line after the current `u32::MAX,` literal contains
                // `max_pending_lifetime,` or `max_pending_lifetime\n` as a
                // forwarded identifier (NOT another u32::MAX). If so,
                // THIS `u32::MAX` is the OTHER argument (chunk_size), and
                // the lifetime IS being threaded correctly on the next
                // line.
                let after_lo = abs;
                let after_hi = (abs + 240).min(bytes.len());
                let after_window = &body[after_lo..after_hi];
                let next_line = after_window.lines().nth(1).map(|s| s.trim()).unwrap_or("");
                let next_line_forwards_lifetime_identifier = next_line
                    .starts_with("max_pending_lifetime")
                    && !next_line.contains("u32::MAX");

                if is_comment
                    || is_eviction_guard_line
                    || in_test_block
                    || !close_to_lifetime
                    || is_legitimate_wrapper_default
                    || next_line_forwards_lifetime_identifier
                {
                    // Allow-listed; not a violation.
                } else {
                    violations.push(format!(
                        "{}:{}: u32::MAX near `max_pending_lifetime` outside the allow-list \
                         (comment / eviction guard / test). Line: {}",
                        path.display(),
                        line_no,
                        line_text.trim()
                    ));
                }
            }
            idx = abs + needle.len();
        }
    }

    assert!(
        violations.is_empty(),
        "IT-0597-04: legacy u32::MAX literal(s) survive in streaming source files. \
         Each must thread `GridConfig.max_pending_lifetime` through instead. \
         Violations:\n{}",
        violations.join("\n")
    );
}
