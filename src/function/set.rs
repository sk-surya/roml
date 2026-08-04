//! Canonical scalar sets (design §6).
//!
//! A scalar set constrains a scalar function. The four variants below mirror
//! the ordinary M2 constraint-bound forms (`le`/`ge`/`eq`/`between`). The
//! enum is `#[non_exhaustive]` so later milestones may add conic or
//! complementarity sets additively.

use std::collections::HashSet;

use crate::id::ParamId;
use crate::value_expr::ValueExpr;

/// A canonical scalar set.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ScalarSet {
    /// `f(x) <= bound`.
    LessEqual(ValueExpr),
    /// `f(x) >= bound`.
    GreaterEqual(ValueExpr),
    /// `f(x) == bound`.
    EqualTo(ValueExpr),
    /// `lower <= f(x) <= upper`.
    Interval {
        /// Lower bound of the interval.
        lower: ValueExpr,
        /// Upper bound of the interval.
        upper: ValueExpr,
    },
}

impl ScalarSet {
    /// The parameter dependencies of every set threshold (F1).
    ///
    /// A threshold `ValueExpr` can reference a parameter (e.g. a
    /// `LessEqual(ValueExpr::param(p))` set bound), which construct bridges
    /// evaluate at compile time. Construct payloads derive their parameter
    /// dependencies from both the constrained function AND the set thresholds
    /// (WR-03).
    pub fn dependencies(&self) -> HashSet<ParamId> {
        let mut deps = HashSet::new();
        match self {
            Self::LessEqual(upper) => deps.extend(upper.dependencies()),
            Self::GreaterEqual(lower) => deps.extend(lower.dependencies()),
            Self::EqualTo(value) => deps.extend(value.dependencies()),
            Self::Interval { lower, upper } => {
                deps.extend(lower.dependencies());
                deps.extend(upper.dependencies());
            }
        }
        deps
    }
}
