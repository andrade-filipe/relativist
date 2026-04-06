//! Core types for Interaction Combinator networks (SPEC-02).
//!
//! Defines Symbol, Agent, PortRef, Net, and all operations on the
//! interaction net data structure. This is the foundation of Relativist.

pub mod core;
pub mod debug;
pub mod types;

#[allow(unused_imports)]
pub use core::*;
#[allow(unused_imports)]
pub use types::*;
