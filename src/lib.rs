//! Relativist — Distributed reduction of Interaction Combinators on Grid Computing.
//!
//! This crate implements a coordinator-worker grid architecture for reducing
//! Interaction Combinator networks across multiple machines, validating that
//! distributed reduction produces identical results to sequential reduction
//! (the Fundamental Property, SPEC-01 G1).

pub mod error;

// Core types: Symbol, Agent, Net, PortRef (SPEC-02)
pub mod net;

// Reduction engine: 6 interaction rules, reduce_all (SPEC-03)
pub mod reduction;

// Partitioning: split net into worker partitions (SPEC-04)
pub mod partition;

// Merge and grid cycle: reunite partitions, resolve borders (SPEC-05)
pub mod merge;

// Wire protocol: TCP messaging, framing, Transport trait (SPEC-06)
pub mod protocol;

// Configuration and CLI support (SPEC-07, SPEC-13)
pub mod config;

// Security: token auth, optional TLS (SPEC-10)
pub mod security;

// Observability: tracing, metrics (SPEC-11)
pub mod observability;

// User I/O: net formats, generators, examples (SPEC-12)
pub mod io;
