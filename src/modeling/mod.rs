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

pub(crate) mod array;
pub(crate) mod builder;
mod coeff;
pub(crate) mod eligibility;
mod labeled;
#[allow(clippy::op_ref)]
mod ops;
mod view;

pub use array::{ParamArray, Shape, VarArray};
pub use coeff::{CoeffView, ConstantView, LinArray, NumView, RowBlockSpec, RowSpec, Term};
pub use labeled::{Axis, LabelError, Labeled, Shaped};
pub use view::{ParamView, VarView, View, ViewError};
