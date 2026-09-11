//! Shared model-owned ordinal array IR (MIR-03, D-019).
//!
//! L1 layer: strided views over trusted block spans. [`view::View`] carries
//! `span + shape + signed strides + offset`; [`view::VarView`] and
//! [`view::ParamView`] attach model ownership so cross-model composition is a
//! typed error before member IDs are reconstructed.
//!
//! This module is intentionally free of Python types and of the L2 persisted
//! dependency descriptor: `roml::modeling` produces an L2 witness, and core
//! revalidates it after canonicalization.

mod view;

pub use view::{ParamView, VarView, View, ViewError};
