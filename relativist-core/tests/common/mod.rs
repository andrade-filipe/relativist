//! Shared test helpers for `relativist-core` integration tests.
//!
//! Each integration test that needs a helper declares `mod common;` at the
//! top of its file and uses `common::<helper>` from this module. Cargo's
//! integration-test mode treats each `tests/*.rs` as its own crate, so
//! sharing through this `common/mod.rs` pattern is the canonical idiom
//! ([Rust by Example, "Submodules in integration tests"]).

#![allow(dead_code)]

pub mod d014_helpers;
