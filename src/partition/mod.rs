//! Net partitioning for distributed reduction (SPEC-04).
//!
//! Splits a Net into K partitions with disjoint agent sets,
//! boundary FreePort markers, and ID space partitioning.

pub mod helpers;
pub mod split;
pub mod strategy;
pub mod types;
