//! Internal helpers for the `merge` module. NOT part of the public
//! API — consumers outside `merge/` must NOT `use` anything from
//! here. The sub-modules are reserved for cross-file contracts and
//! test-time invariants that should not leak into the crate's public
//! surface.

#[cfg(test)]
pub(crate) mod pure_core_guard;
