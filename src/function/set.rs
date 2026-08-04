//! Canonical scalar sets (design §6).
//!
//! A scalar set constrains a scalar function. The four variants below mirror
//! the ordinary M2 constraint-bound forms (`le`/`ge`/`eq`/`between`). The
//! enum is `#[non_exhaustive]` so later milestones may add conic or
//! complementarity sets additively.

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
