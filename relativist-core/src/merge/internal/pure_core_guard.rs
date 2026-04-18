//! Programmatic R19 pure-core import-discipline guard (SPEC-19 §3.2
//! R19; SPEC-13 R6-R8). Any file in `merge/` that claims pure-core
//! status opts in with a single `#[test]` fn that hands its own
//! source (`include_str!("self.rs")`) to
//! [`assert_no_forbidden_imports`]. The helper scans `use` lines
//! against [`FORBIDDEN_USE_PREFIXES`] and panics loudly on
//! violations — the panic message cites both the offending prefix
//! and the "R19 violation" tag so CI failure output points to the
//! invariant immediately.
//!
//! **DC-B8 (spec-critic verdict 2026-04-17, option c):** factor the
//! scan into a shared helper so each future pure-core file adopts
//! the invariant with a one-liner test instead of copy-pasting the
//! scan logic. As of 2.26-B the only opt-in site is
//! `border_resolver.rs`; when future `merge/*.rs` pure-core files
//! land (e.g. under 2.26-C or later), add a mirror test there.
//!
//! **DC-B9 (spec-critic verdict 2026-04-17):** the forbidden list
//! contains FIVE entries — three direct violations (`use tokio`,
//! `use async_trait`, `use crate::protocol`) plus two
//! transitive-leak closures (`use crate::coordinator`,
//! `use crate::worker`). The last two cover the case where a
//! developer pulls in a `coordinator` or `worker` type that
//! itself depends on async + protocol, smuggling the dependency
//! into `merge/` without a direct `tokio` use.

/// Forbidden `use` prefixes for any pure-core file in `merge/`.
/// Authoritatively five entries per DC-B9 (2026-04-17).
pub(crate) const FORBIDDEN_USE_PREFIXES: &[&str] = &[
    "use tokio",
    "use async_trait",
    "use crate::protocol",
    "use crate::coordinator",
    "use crate::worker",
];

/// Scans `src` line-by-line. For each line starting (after trimming
/// leading whitespace) with `"use "`, verifies the line does NOT
/// begin with any entry in [`FORBIDDEN_USE_PREFIXES`]. Panics on
/// the first match with a message citing `label`, the offending
/// prefix, and "R19 violation".
///
/// Called from each pure-core file's own `#[test]` block as a
/// one-liner.
pub(crate) fn assert_no_forbidden_imports(src: &str, label: &str) {
    for line in src.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("use ") {
            continue;
        }
        for prefix in FORBIDDEN_USE_PREFIXES {
            assert!(
                !trimmed.starts_with(prefix),
                "R19 violation: {label} imports {prefix:?} \
                 (pure-core files must not depend on tokio, \
                 async_trait, crate::protocol, crate::coordinator, \
                 or crate::worker — see SPEC-19 §3.2 R19 and \
                 DC-B9 spec-critic verdict 2026-04-17)"
            );
        }
    }
}
