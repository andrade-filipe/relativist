//! Core types for Interaction Combinator networks (SPEC-02).
//!
//! Defines Symbol, Agent, PortRef, Net, and all operations on the
//! interaction net data structure. This is the foundation of Relativist.

pub mod core;
pub mod debug;
pub mod sparse;
pub mod types;

pub use core::*;
pub use sparse::SparseNet;
pub use types::*;
