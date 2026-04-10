//! Reduction engine for Interaction Combinators (SPEC-03).
//!
//! Implements the 6 interaction rules (CON-CON, DUP-DUP, ERA-ERA,
//! CON-DUP, CON-ERA, DUP-ERA), dispatch, and the reduction loop
//! (reduce_step, reduce_all, reduce_n).

pub mod dispatch;
pub mod engine;
pub mod rules;

// Re-exports: convenience access via `crate::reduction::*`
pub use dispatch::{get_rule, get_specific_rule, normalize_pair, Rule, SpecificRule};
pub use engine::{reduce_all, reduce_border_once, reduce_n, reduce_step, ReductionStats, StepResult};
pub use rules::{interact_anni, interact_comm, interact_eras, interact_void};
