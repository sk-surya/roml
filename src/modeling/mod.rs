//! Shared model-owned ordinal array IR (MIR-03, D-019).
//!
//! L1 layer: strided views over trusted block spans. [`view::View`] carries
//! `span + shape + signed strides + offset`; [`view::VarView`] and
//! [`view::ParamView`] attach model ownership so cross-model composition is a
//! typed error before member IDs are reconstructed. [`coeff::LinArray`] is
//! `Σ Term{VarView, CoeffView} + ConstantView` over the initial conservative
//! coefficient families.
//!
//! `builder` and `eligibility` are crate-internal L1→L2 planning internals
//! (not public construction surfaces until the MIR-04 model builders create
//! symbolic views from the owning model).

pub(crate) mod builder;
mod coeff;
pub(crate) mod eligibility;
mod view;

pub use coeff::{CoeffView, ConstantView, LinArray, NumView, Term};
pub use view::{ParamView, VarView, View, ViewError};
