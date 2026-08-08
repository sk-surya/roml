//! Solver-free file-format interfaces.
//!
//! Formats live outside [`crate::Model`] so reading or writing a file never
//! becomes a responsibility of canonical model state.

/// MPS import and, in a later phase, MPS export interfaces.
pub mod mps;
