//! Piecewise-linear construct payload (design §17; SM-14; D13, D24).
//!
//! PWL declarations specify finite strictly increasing breakpoints, an
//! explicit relation (`epigraph`, `hypograph`, or `exact graph`), an explicit
//! extrapolation policy, and an optional per-construct formulation preference
//! (stored on the [`ConstructEntry`](crate::construct::ConstructEntry), A29).
//! Curvature is classified deterministically from segment slopes (SM-14.2).
//!
//! Exactness is a semantic choice the user selects explicitly (D13): the exact
//! graph and the one-sided relations are distinct semantics, and the compiler
//! never infers exactness from objective context. The payload stores the exact
//! semantic content only — `points`, `relation`, `extrapolation`, `argument`,
//! and the `output` variable created by the builder (top-level construct
//! origin).
//!
//! The direct evaluator ([`PiecewiseLinearConstraint::evaluate`]) performs
//! linear interpolation between breakpoints and extrapolation per the explicit
//! [`ExtrapolationPolicy`]. Point values are `ValueExpr`s (parameter-dependent
//! values are supported and evaluated at compile time by the bridge); the
//! direct evaluator resolves constant point values.

use crate::expr::LinExpr;
use crate::id::{ParamId, VarId};
use crate::value_expr::ValueExpr;

/// The relation a [`PiecewiseLinearConstraint`] declares (design §17; D13,
/// D24).
///
/// The relation determines the formulation (D24): `Epigraph` on a convex PWL
/// and `Hypograph` on a concave PWL compile to zero-binary supporting-inequality
/// rows; `ExactGraph` compiles to the exact representation (deterministic exact
/// segment binaries in M3 — never a convex relaxation, SM-14.4/SM-14.5).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PwlRelation {
    /// `output >= f(argument)` — the epigraph (zero-binary rows for a convex
    /// PWL).
    Epigraph,
    /// `output <= f(argument)` — the hypograph (zero-binary rows for a concave
    /// PWL).
    Hypograph,
    /// `output = f(argument)` — the exact graph (exact representation).
    ExactGraph,
}

/// The explicit extrapolation policy of a [`PiecewiseLinearConstraint`]
/// (SM-14.1): how the function extends outside the breakpoint range.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtrapolationPolicy {
    /// Clamp: the function is constant outside the breakpoint range (the value
    /// at the nearest end breakpoint).
    Constant,
    /// The end segment slope continues linearly outside the breakpoint range.
    Linear,
}

/// The deterministic curvature class of a [`PiecewiseLinearConstraint`]
/// (SM-14.2), classified from segment slopes.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PwlCurvature {
    /// All segment slopes are equal (the function is linear).
    Affine,
    /// Segment slopes are non-decreasing.
    Convex,
    /// Segment slopes are non-increasing.
    Concave,
    /// Neither convex nor concave (a slope sign change).
    NonConvex,
}

/// One breakpoint of a [`PiecewiseLinearConstraint`] (SM-14.1).
///
/// `x` must be finite and strictly increasing across the point list; `value` is
/// the function value at the breakpoint (a `ValueExpr`, so point values may
/// depend on model parameters).
#[derive(Clone, Debug, PartialEq)]
pub struct PwlPoint {
    /// The finite breakpoint abscissa.
    pub x: f64,
    /// The function value at the breakpoint (parameter-dependent allowed).
    pub value: ValueExpr,
}

impl From<(f64, f64)> for PwlPoint {
    /// Build a constant-valued point from `(breakpoint, value)`.
    fn from((x, value): (f64, f64)) -> Self {
        Self {
            x,
            value: ValueExpr::constant(value),
        }
    }
}

/// The exact semantic payload of a piecewise-linear construct (design §17).
///
/// `output` is the variable holding the PWL result; the builder creates it and
/// stores it here so the construct is self-contained and its origins are
/// top-level. `points` must contain at least two finite strictly increasing
/// breakpoints (validated by the builder — SM-14.1).
#[derive(Clone, Debug, PartialEq)]
pub struct PiecewiseLinearConstraint {
    /// The breakpoints and values (at least two, finite, strictly increasing).
    pub points: Vec<PwlPoint>,
    /// The declared relation (epigraph / hypograph / exact graph).
    pub relation: PwlRelation,
    /// The explicit extrapolation policy.
    pub extrapolation: ExtrapolationPolicy,
    /// The scalar linear argument expression the PWL function is applied to.
    pub argument: LinExpr,
    /// The variable holding the PWL result (created by the builder).
    pub output: VarId,
}

impl PiecewiseLinearConstraint {
    /// Derive the parameter dependencies across the point values and the
    /// argument expression (F1).
    pub fn parameter_dependencies(&self) -> Vec<ParamId> {
        let mut deps: Vec<ParamId> = Vec::new();
        for point in &self.points {
            for p in point.value.dependencies() {
                if !deps.contains(&p) {
                    deps.push(p);
                }
            }
        }
        for p in self.argument.parameter_dependencies() {
            if !deps.contains(&p) {
                deps.push(p);
            }
        }
        deps
    }

    /// The resolved numeric value at breakpoint `i`.
    ///
    /// The direct evaluator and curvature classification resolve point values
    /// as constants; parameter-dependent point values are evaluated by the
    /// compiler bridge against the snapshot's parameter values (a panic here
    /// is a programming error — direct evaluation requires constant values).
    fn point_value(&self, i: usize) -> f64 {
        self.points[i]
            .value
            .as_constant()
            .expect("direct PWL evaluation/classification requires constant point values")
    }

    /// The segment slopes `s_i = (v_{i+1} - v_i) / (x_{i+1} - x_i)`.
    ///
    /// Deterministic (SM-14.2): breakpoints are strictly increasing, so the
    /// denominator is finite and positive.
    pub fn segment_slopes(&self) -> Vec<f64> {
        (0..self.points.len().saturating_sub(1))
            .map(|i| {
                (self.point_value(i + 1) - self.point_value(i))
                    / (self.points[i + 1].x - self.points[i].x)
            })
            .collect()
    }

    /// Classify the PWL curvature deterministically from segment slopes
    /// (SM-14.2): affine when all slopes are equal, convex when slopes are
    /// non-decreasing, concave when non-increasing, non-convex on a slope sign
    /// change.
    pub fn classify_curvature(&self) -> PwlCurvature {
        let slopes = self.segment_slopes();
        if slopes.len() < 2 {
            return PwlCurvature::Affine;
        }
        let mut non_decreasing = true;
        let mut non_increasing = true;
        for pair in slopes.windows(2) {
            if pair[1] < pair[0] {
                non_decreasing = false;
            }
            if pair[1] > pair[0] {
                non_increasing = false;
            }
        }
        match (non_decreasing, non_increasing) {
            (true, true) => PwlCurvature::Affine,
            (true, false) => PwlCurvature::Convex,
            (false, true) => PwlCurvature::Concave,
            (false, false) => PwlCurvature::NonConvex,
        }
    }

    /// Directly evaluate the PWL function at `x` (SM-14.2/14.7).
    ///
    /// Linear interpolation between breakpoints; extrapolation follows the
    /// explicit [`ExtrapolationPolicy`] outside the breakpoint range (constant
    /// clamps to the end value, linear continues the end segment slope).
    pub fn evaluate(&self, x: f64) -> f64 {
        let n = self.points.len();
        debug_assert!(n >= 2, "builder validates at least two points");
        // Left extrapolation.
        if x <= self.points[0].x {
            return match self.extrapolation {
                ExtrapolationPolicy::Constant => self.point_value(0),
                ExtrapolationPolicy::Linear => {
                    let slope = (self.point_value(1) - self.point_value(0))
                        / (self.points[1].x - self.points[0].x);
                    self.point_value(0) + slope * (x - self.points[0].x)
                }
            };
        }
        // Right extrapolation.
        if x >= self.points[n - 1].x {
            return match self.extrapolation {
                ExtrapolationPolicy::Constant => self.point_value(n - 1),
                ExtrapolationPolicy::Linear => {
                    let slope = (self.point_value(n - 1) - self.point_value(n - 2))
                        / (self.points[n - 1].x - self.points[n - 2].x);
                    self.point_value(n - 1) + slope * (x - self.points[n - 1].x)
                }
            };
        }
        // Interior interpolation over the segment containing `x`.
        for i in 0..n - 1 {
            if x >= self.points[i].x && x <= self.points[i + 1].x {
                let slope = (self.point_value(i + 1) - self.point_value(i))
                    / (self.points[i + 1].x - self.points[i].x);
                return self.point_value(i) + slope * (x - self.points[i].x);
            }
        }
        unreachable!("x lies within the breakpoint range");
    }
}
