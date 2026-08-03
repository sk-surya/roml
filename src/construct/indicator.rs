//! Indicator construct payload (design §7, §16.1).
//!
//! A one-way implication over a binary activator: when the activator takes the
//! indicated value, a scalar-function-in-set relation must hold. The exact
//! semantics are `direction == WhenOne ⇒ activator = 1 ⇒ relation` and
//! `direction == WhenZero ⇒ activator = 0 ⇒ relation`. Compilation (Task 16)
//! selects a qualified native indicator or an exact finite-bound bridge (design
//! §8.1); the payload stores the exact semantic content only (A29 single
//! authority — the preference lives in the [`ConstructEntry`](crate::construct::ConstructEntry)).

use crate::function::{ScalarFunction, ScalarSet};
use crate::id::{ParamId, VarId};

/// The implication direction of an [`IndicatorConstraint`] (design §16.1).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndicatorDirection {
    /// The relation holds when the activator is `1`.
    WhenOne,
    /// The relation holds when the activator is `0`.
    WhenZero,
}

/// The exact semantic payload of an indicator construct (design §7, §16.1).
///
/// `activator` must be a binary variable (validated by the builder); when it
/// takes the `direction` value, `function ∈ set` is enforced. Compilation is a
/// one-way implication — no equivalence is implied.
#[derive(Clone, Debug, PartialEq)]
pub struct IndicatorConstraint {
    /// The binary activator variable.
    pub activator: VarId,
    /// The one-way implication direction.
    pub direction: IndicatorDirection,
    /// The scalar function constrained when the activator is active.
    pub function: ScalarFunction,
    /// The set constraining the function when the activator is active.
    pub set: ScalarSet,
}

impl IndicatorConstraint {
    /// Derive the parameter dependencies of the constrained function (F1).
    pub fn parameter_dependencies(&self) -> Vec<ParamId> {
        self.function.parameter_dependencies()
    }
}
