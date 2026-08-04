//! Absolute value / positive part / clamp construct payload (design §7,
//! §16.3; D13, D14; SM-12.4).
//!
//! `AbsoluteValueConstraint` declares an exact absolute-value-family relation
//! over one bounded linear expression: `|expression|`, `max(expression, 0)`,
//! or `clamp(expression, lower, upper)`. Each variant compiles to a bounded
//! exact bridge (Task 17b) that preserves the top-level construct origin — the
//! output variable is the construct's canonical result, never a bare
//! compiler-internal auxiliary. The payload stores the exact semantic content
//! only; the per-construct formulation preference lives in the
//! [`ConstructEntry`](crate::construct::ConstructEntry) (A29).

use crate::expr::LinExpr;
use crate::id::{ParamId, VarId};

/// The absolute-value-family variant of an [`AbsoluteValueConstraint`]
/// (design §16.3).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AbsoluteValueVariant {
    /// `output = |expression|` — exact absolute value.
    Absolute,
    /// `output = max(expression, 0)` — exact positive part.
    PositivePart,
    /// `output = clamp(expression, lower, upper)` — exact clamp with finite
    /// `lower <= upper`.
    Clamp {
        /// The clamp lower bound (finite, `<= upper`).
        lower: f64,
        /// The clamp upper bound (finite, `>= lower`).
        upper: f64,
    },
}

/// The exact semantic payload of an absolute-value-family construct
/// (design §7, §16.3; SM-12.4).
///
/// `expression` must be a bounded (finite-interval) linear expression
/// (validated by the builder); `output` is the variable holding the
/// absolute/positive-part/clamp result, created by the builder and stored here
/// so the construct is self-contained with a top-level construct origin.
#[derive(Clone, Debug, PartialEq)]
pub struct AbsoluteValueConstraint {
    /// The bounded linear expression the variant is applied to.
    pub expression: LinExpr,
    /// The variable holding the result (created by the builder).
    pub output: VarId,
    /// The exact absolute-value-family variant.
    pub variant: AbsoluteValueVariant,
}

impl AbsoluteValueConstraint {
    /// Derive the parameter dependencies of the expression (F1).
    pub fn parameter_dependencies(&self) -> Vec<ParamId> {
        self.expression.parameter_dependencies()
    }
}
