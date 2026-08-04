//! Cardinality construct payload (design §7, §16.4).
//!
//! Exactly/at-most/at-least-k over a list of binary variables, through exact
//! linear rows. The payload stores the exact semantic content only; the
//! per-construct formulation preference lives in the
//! [`ConstructEntry`](crate::construct::ConstructEntry) (A29).

use crate::id::VarId;

/// The cardinality relation being constrained (design §16.4).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardinalityKind {
    /// Exactly `k` of the variables are `1`.
    Exactly,
    /// At most `k` of the variables are `1`.
    AtMost,
    /// At least `k` of the variables are `1`.
    AtLeast,
}

/// The exact semantic payload of a cardinality construct (design §7, §16.4).
///
/// `variables` must be a non-empty list of binary variables with no duplicates
/// (validated by the builder); `k` is the validated integer count (as
/// `usize` after the builder's `f64`-input validation).
#[derive(Clone, Debug, PartialEq)]
pub struct CardinalityConstraint {
    /// The binary variables the cardinality applies to.
    pub variables: Vec<VarId>,
    /// Whether exactly/at-most/at-least `k`.
    pub kind: CardinalityKind,
    /// The validated cardinality count `0 ≤ k ≤ variables.len()`.
    pub k: usize,
}
