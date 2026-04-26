//! Typed timer identifiers (SPEC-20 §4.1.3; closes SC-022 and NF-008).

/// Typed timer identifiers (SPEC-20 §4.1.3; closes SC-022).
///
/// `TimerId` values are derived via `TimerKind as u32` so that the cast is
/// portable and log tooling can decode `TimerId → TimerKind` without
/// per-build metadata.
///
/// # Import path for observability consumers
///
/// Observability crates and log-decoding tooling should import this type as
/// `relativist_core::protocol::TimerKind`.
///
/// # ABI stability
///
/// Explicit `#[repr(u32)]` discriminants 0–3 are normative per SPEC-20 §4.1.3
/// (NF-008). Log-decoding tooling depends on these values. A future variant
/// insertion that shifts these values is a wire-decoder break. Do NOT reorder
/// or insert variants before position 4.
///
/// `#[non_exhaustive]` allows new variants to be added in future SPEC revisions
/// without breaking downstream matches.
#[repr(u32)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum TimerKind {
    /// Timer for the initial worker-connection wait (R6).
    InitialWait = 0,
    /// Minimum join-window duration (R10, R10a).
    JoinWindowMin = 1,
    /// Maximum join-window duration (R10a).
    JoinWindowMax = 2,
    /// Per-round collect timeout (SPEC-06 R30, R18).
    Collect = 3,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_kind_discriminants_are_stable() {
        assert_eq!(TimerKind::InitialWait as u32, 0);
        assert_eq!(TimerKind::JoinWindowMin as u32, 1);
        assert_eq!(TimerKind::JoinWindowMax as u32, 2);
        assert_eq!(TimerKind::Collect as u32, 3);
    }
}
