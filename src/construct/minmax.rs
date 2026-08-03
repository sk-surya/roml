//! Min/max construct payload (design §7, §16.3; D13; SM-12.3).
//!
//! Exact equality and the one-sided epigraph/hypograph relations are distinct
//! semantic choices the user selects explicitly (D13) — exactness is never
//! inferred from objective context. `MinMaxConstraint` carries an explicit
//! [`MinMaxRelation`]: `Exact` compiles to a bounded selector formulation with
//! binaries; `Max`+`Epigraph` and `Min`+`Hypograph` compile to zero-binary
//! one-sided rows. The payload stores the exact semantic content only; the
//! per-construct formulation preference lives in the
//! [`ConstructEntry`](crate::construct::ConstructEntry) (A29).

use crate::expr::LinExpr;
use crate::id::{ParamId, VarId};

/// The min/max sense of a [`MinMaxConstraint`] (design §16.3).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinMaxSense {
    /// `output = min(operands)` (or `output <= min(operands)` hypograph).
    Min,
    /// `output = max(operands)` (or `output >= max(operands)` epigraph).
    Max,
}

/// The relation a [`MinMaxConstraint`] declares (design §16.3; D13).
///
/// `Exact` and the one-sided relations are distinct semantics — a one-sided
/// relation is never labeled exact and never implies exactness (D13).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinMaxRelation {
    /// `output = max(operands)` / `output = min(operands)` — exact equality,
    /// compiled as a bounded selector formulation (binaries).
    Exact,
    /// `output >= max(operands)` — the max epigraph (zero-binary rows
    /// `x_i <= output`).
    Epigraph,
    /// `output <= min(operands)` — the min hypograph (zero-binary rows
    /// `output <= x_i`).
    Hypograph,
}

/// The exact semantic payload of a min/max construct (design §7, §16.3).
///
/// `output` is the variable holding the min/max result; the builder creates it
/// and stores it here so the construct is self-contained and its origins are
/// top-level (the output is the construct's canonical result, never a bare
/// compiler-internal auxiliary). `operands` must contain at least two finite
/// linear expressions (validated by the builder); the trivially-satisfiable
/// `Min`+`Epigraph` and `Max`+`Hypograph` combinations are rejected.
#[derive(Clone, Debug, PartialEq)]
pub struct MinMaxConstraint {
    /// The min/max operands (at least two, finite linear expressions).
    pub operands: Vec<LinExpr>,
    /// The variable holding the min/max result (created by the builder).
    pub output: VarId,
    /// The min/max sense.
    pub sense: MinMaxSense,
    /// The declared relation (exact vs one-sided).
    pub relation: MinMaxRelation,
}

impl MinMaxConstraint {
    /// Derive the parameter dependencies across all operand expressions (F1).
    pub fn parameter_dependencies(&self) -> Vec<ParamId> {
        let mut deps: Vec<ParamId> = Vec::new();
        for expr in &self.operands {
            for p in expr.parameter_dependencies() {
                if !deps.contains(&p) {
                    deps.push(p);
                }
            }
        }
        deps
    }
}
