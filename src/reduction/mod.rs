//! Reduction engine for Interaction Combinators (SPEC-03).
//!
//! Implements the 6 interaction rules (CON-CON, DUP-DUP, ERA-ERA,
//! CON-DUP, CON-ERA, DUP-ERA), dispatch, and the reduction loop
//! (reduce_step, reduce_all, reduce_n).

pub mod dispatch;
pub mod engine;
pub mod rules;
