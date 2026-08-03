//! Deterministic interval bound analysis and Big-M safety (design §9, §19;
//! D12; SM-13).
//!
//! [`BoundAnalyzer`] computes deterministic intervals for linear scalar
//! functions over declared variable bounds and evaluated parameter values,
//! handling coefficient signs, constant terms, fixed/equal-bound variables,
//! infinite (free) bounds, and evaluated parameters, and rejecting NaN input
//! with a typed [`BoundError`] (SM-13.1, SM-13.6). No auxiliary LP is ever
//! solved for bound tightening (SM-13.6) — the interval is propagated from
//! declared bounds and coefficients only.
//!
//! The one-sided Big-M helpers ([`bound_big_m_implied`],
//! [`validated_explicit_big_m`]) derive a finite M from the analyzed interval
//! or surface the construct-aware [`CompileError::UnboundedBigM`] marker — a
//! finite M exists only as a finite derived value or an explicit validated
//! user value, never an arbitrary default constant (SM-13.2/13.3, D12).

use crate::construct::Construct;
use crate::expr::{LinExpr, TermCoeff};
use crate::function::ScalarFunction;
use crate::id::{ParamId, VarId};
use crate::model::Bounds;
use crate::snapshot::ModelSnapshot;
use crate::value_expr::ValueExpr;

use super::CompileError;

/// A bounded numeric interval `[lower, upper]` (design §9).
///
/// Either endpoint may be infinite for unbounded intervals; a NaN endpoint
/// never appears — [`BoundAnalyzer`] rejects non-finite input with a typed
/// [`BoundError`] (SM-13.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    /// Lower endpoint (`f64::NEG_INFINITY` when unbounded below).
    pub lower: f64,
    /// Upper endpoint (`f64::INFINITY` when unbounded above).
    pub upper: f64,
}

impl Interval {
    /// A degenerate interval holding exactly `value`.
    pub fn exact(value: f64) -> Self {
        Self {
            lower: value,
            upper: value,
        }
    }

    /// Whether both endpoints are finite.
    pub fn is_bounded(&self) -> bool {
        self.lower.is_finite() && self.upper.is_finite()
    }

    /// Whether `value` lies within the interval (inclusive).
    pub fn contains(&self, value: f64) -> bool {
        self.lower <= value && value <= self.upper
    }
}

/// A provenance marker recording where an interval bound came from
/// (design §9 gloss).
///
/// An implementation-detail marker used by [`BoundTrace`]; not part of the
/// ordinary public surface (kept reachable because `BoundTrace.sources` is a
/// public field).
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BoundSource {
    /// The interval was derived from a variable's declared bounds.
    DeclaredVariableBounds(VarId),
    /// The interval was derived from a fixed variable (equal lower/upper
    /// bounds — the fixing representation, D6).
    FixedValue(VarId),
    /// The interval was derived from an evaluated parameter value.
    ParameterValue(ParamId),
    /// The interval component is a constant term.
    Constant,
}

/// The result of one interval analysis: the provenance trace and the computed
/// interval (design §9).
#[derive(Clone, Debug, PartialEq)]
pub struct BoundTrace {
    /// Provenance markers in analysis order (deterministic).
    pub sources: Vec<BoundSource>,
    /// The computed interval.
    pub result: Interval,
}

/// A typed bound-analysis failure (SM-13.1).
///
/// NaN/invalid input is rejected, never propagated silently.
#[derive(Clone, Debug, PartialEq)]
pub enum BoundError {
    /// A coefficient (constant or parameter-evaluated) is NaN or infinite.
    NonFiniteCoefficient {
        /// The variable whose coefficient is non-finite.
        variable: VarId,
    },
    /// A variable bound is NaN.
    NonFiniteBound {
        /// The variable with a NaN bound.
        variable: VarId,
    },
    /// A variable has inverted bounds (`lower > upper`).
    InvalidBounds {
        /// The variable with inverted bounds.
        variable: VarId,
    },
    /// The constant term is NaN.
    NonFiniteConstant,
    /// A parameter value used by a bare-parameter coefficient is NaN or
    /// infinite.
    NonFiniteParameterValue {
        /// The parameter whose value is non-finite.
        parameter: ParamId,
    },
    /// Interval arithmetic produced NaN from finite inputs.
    ArithmeticNan,
    /// The function kind has no interval analysis (M3 implements linear only).
    UnsupportedFunctionKind,
}

impl std::fmt::Display for BoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFiniteCoefficient { variable } => {
                write!(f, "non-finite coefficient on variable {variable:?}")
            }
            Self::NonFiniteBound { variable } => {
                write!(f, "NaN bound on variable {variable:?}")
            }
            Self::InvalidBounds { variable } => {
                write!(f, "inverted bounds on variable {variable:?}")
            }
            Self::NonFiniteConstant => write!(f, "non-finite constant term"),
            Self::NonFiniteParameterValue { parameter } => {
                write!(f, "non-finite value for parameter {parameter:?}")
            }
            Self::ArithmeticNan => write!(f, "interval arithmetic produced NaN"),
            Self::UnsupportedFunctionKind => {
                write!(f, "no interval analysis for this function kind")
            }
        }
    }
}

impl std::error::Error for BoundError {}

/// Deterministic linear interval propagation (design §9; SM-13.1, SM-13.6).
///
/// Stateless: each [`interval_of`](Self::interval_of) call propagates the
/// interval of a linear scalar function over the supplied variable-bounds and
/// parameter-value lookups. Terms are processed in sorted variable order, so
/// the result is deterministic regardless of input term order. Coefficient
/// signs flip the contribution, constant terms offset the interval, fixed
/// variables (equal lower/upper bounds) contribute exact values, and infinite
/// bounds propagate to unbounded endpoints. NaN anywhere is a typed
/// [`BoundError`]. No auxiliary optimization problem is ever run (SM-13.6).
#[derive(Clone, Copy, Debug, Default)]
pub struct BoundAnalyzer;

impl BoundAnalyzer {
    /// Create an analyzer.
    pub fn new() -> Self {
        Self
    }

    /// Analyze the interval of `function` using the given variable-bounds and
    /// parameter-value lookups.
    ///
    /// `variable_bounds` resolves each variable's declared bounds (the fixing
    /// representation is equal lower/upper bounds, D6); `parameter_values`
    /// resolves each parameter's current value for coefficient evaluation.
    pub fn interval_of<F, G>(
        &self,
        function: &ScalarFunction,
        variable_bounds: F,
        parameter_values: G,
    ) -> Result<BoundTrace, BoundError>
    where
        F: Fn(VarId) -> Bounds,
        G: Fn(ParamId) -> f64,
    {
        // M3 implements only `ScalarFunction::Linear`; the enum is
        // `#[non_exhaustive]` for external consumers, so an in-crate exhaustive
        // match is correct and a future non-linear variant is handled here.
        match function {
            ScalarFunction::Linear(expr) => {
                self.interval_of_linear(expr, variable_bounds, parameter_values)
            }
        }
    }

    /// Convenience over a canonical snapshot's declared variable bounds and
    /// evaluated parameter values.
    ///
    /// A variable absent from the snapshot is treated as free (conservative:
    /// an unbounded interval can only produce an `UnboundedBigM`, never an
    /// unsafe finite M); a parameter absent from the snapshot evaluates to
    /// zero.
    pub fn interval_of_snapshot(
        &self,
        function: &ScalarFunction,
        snapshot: &ModelSnapshot,
    ) -> Result<BoundTrace, BoundError> {
        let variable_bounds = |v: VarId| {
            snapshot
                .variables
                .iter()
                .find(|e| e.id == v)
                .map(|e| e.bounds)
                .unwrap_or(Bounds::UNBOUNDED)
        };
        let parameter_values = |p: ParamId| {
            snapshot
                .parameters
                .iter()
                .find(|e| e.id == p)
                .map(|e| e.value)
                .unwrap_or(0.0)
        };
        self.interval_of(function, variable_bounds, parameter_values)
    }

    fn interval_of_linear<F, G>(
        &self,
        expr: &LinExpr,
        variable_bounds: F,
        parameter_values: G,
    ) -> Result<BoundTrace, BoundError>
    where
        F: Fn(VarId) -> Bounds,
        G: Fn(ParamId) -> f64,
    {
        if !expr.constant.is_finite() {
            return Err(BoundError::NonFiniteConstant);
        }
        let mut sources = vec![BoundSource::Constant];
        let mut interval = Interval::exact(expr.constant);

        // Deterministic: process terms in sorted variable order regardless of
        // the input term order (SM-13.1 determinism).
        let mut terms: Vec<_> = expr.terms.iter().collect();
        terms.sort_by_key(|term| term.var);

        for term in terms {
            let mut parameter_source = None;
            let coeff = match &term.coeff {
                TermCoeff::Constant(value) => *value,
                TermCoeff::Expr(value_expr) => {
                    // Attribute a NaN/infinite bare-parameter value to the
                    // parameter; any other expression evaluating non-finite is
                    // attributed to the coefficient.
                    if let ValueExpr::Param(pid) = value_expr {
                        let value = parameter_values(*pid);
                        if !value.is_finite() {
                            return Err(BoundError::NonFiniteParameterValue { parameter: *pid });
                        }
                        parameter_source = Some(*pid);
                        value
                    } else {
                        let value = value_expr.eval(&parameter_values);
                        if !value.is_finite() {
                            return Err(BoundError::NonFiniteCoefficient { variable: term.var });
                        }
                        value
                    }
                }
            };
            if !coeff.is_finite() {
                return Err(BoundError::NonFiniteCoefficient { variable: term.var });
            }
            if coeff == 0.0 {
                continue;
            }

            let bounds = variable_bounds(term.var);
            if bounds.lower.is_nan() || bounds.upper.is_nan() {
                return Err(BoundError::NonFiniteBound { variable: term.var });
            }
            if bounds.lower > bounds.upper {
                return Err(BoundError::InvalidBounds { variable: term.var });
            }

            let contribution = term_contribution(coeff, bounds);
            interval = add_intervals(interval, contribution)?;
            if let Some(pid) = parameter_source {
                sources.push(BoundSource::ParameterValue(pid));
            }
            sources.push(if bounds.lower == bounds.upper {
                BoundSource::FixedValue(term.var)
            } else {
                BoundSource::DeclaredVariableBounds(term.var)
            });
        }

        Ok(BoundTrace {
            sources,
            result: interval,
        })
    }
}

/// Contribution of one term `coeff * x` over `bounds` (coeff != 0).
///
/// Coefficient signs flip the endpoints; infinite bounds propagate to
/// unbounded endpoints; overflow of finite arithmetic is conservative
/// (treated as the corresponding infinity, which can only yield an
/// `UnboundedBigM` downstream — never an unsafe finite M).
fn term_contribution(coeff: f64, bounds: Bounds) -> Interval {
    if bounds.lower == bounds.upper {
        // Fixed variable (the fixing representation — D6): exact value.
        Interval::exact(coeff * bounds.lower)
    } else if coeff > 0.0 {
        Interval {
            lower: scale_positive_lower(coeff, bounds.lower),
            upper: scale_positive_upper(coeff, bounds.upper),
        }
    } else {
        Interval {
            lower: scale_negative_lower(coeff, bounds.upper),
            upper: scale_negative_upper(coeff, bounds.lower),
        }
    }
}

fn scale_positive_lower(coeff: f64, lower: f64) -> f64 {
    if lower == f64::NEG_INFINITY {
        f64::NEG_INFINITY
    } else {
        coeff * lower
    }
}

fn scale_positive_upper(coeff: f64, upper: f64) -> f64 {
    if upper == f64::INFINITY {
        f64::INFINITY
    } else {
        coeff * upper
    }
}

fn scale_negative_lower(coeff: f64, upper: f64) -> f64 {
    if upper == f64::INFINITY {
        f64::NEG_INFINITY
    } else {
        coeff * upper
    }
}

fn scale_negative_upper(coeff: f64, lower: f64) -> f64 {
    if lower == f64::NEG_INFINITY {
        f64::INFINITY
    } else {
        coeff * lower
    }
}

/// Add two intervals, clamping unbounded sides conservatively so that
/// `+inf + -inf` yields the unbounded result rather than NaN.
fn add_intervals(a: Interval, b: Interval) -> Result<Interval, BoundError> {
    let lower = if a.lower == f64::NEG_INFINITY || b.lower == f64::NEG_INFINITY {
        f64::NEG_INFINITY
    } else {
        a.lower + b.lower
    };
    let upper = if a.upper == f64::INFINITY || b.upper == f64::INFINITY {
        f64::INFINITY
    } else {
        a.upper + b.upper
    };
    if lower.is_nan() || upper.is_nan() {
        return Err(BoundError::ArithmeticNan);
    }
    Ok(Interval { lower, upper })
}

// ===========================================================================
// One-sided Big-M helpers (design §9, SM-13.2/13.3, D12)
// ===========================================================================

/// The direction of the one-sided implication for which a Big-M is derived
/// (design §9 rule 1: the M is derived for the exact one-sided implication
/// being relaxed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BigMImplication {
    /// The implication `f(x) <= rhs` (upper-bounded): `M = max(0, max f - rhs)`.
    Upper,
    /// The implication `f(x) >= rhs` (lower-bounded): `M = max(0, rhs - min f)`.
    Lower,
}

/// The construct-aware marker that no finite Big-M exists for a construct's
/// bounds (SM-13.2/13.4, D12).
///
/// An implementation-detail marker: a bridge converts it into the public
/// [`CompileError::UnboundedBigM`] (via `From`) rather than substituting a
/// default constant. The type is crate-private — callers observe the
/// construct-aware error, never a silent number.
pub(crate) struct UnboundedBigM {
    /// The construct that requires the Big-M.
    pub(crate) construct: Construct,
    /// The expression the Big-M was being derived for.
    pub(crate) expression: String,
}

impl From<UnboundedBigM> for CompileError {
    fn from(value: UnboundedBigM) -> Self {
        CompileError::UnboundedBigM {
            construct: value.construct,
            expression: value.expression,
        }
    }
}

fn unbounded_error(construct: Construct, expression: &str) -> CompileError {
    UnboundedBigM {
        construct,
        expression: expression.to_string(),
    }
    .into()
}

/// The one-sided Big-M request context (design §9, SM-13.2/13.3).
///
/// Carries everything a Big-M derivation needs: the construct and a
/// human-readable expression (for the construct-aware error naming, SM-13.4),
/// the scalar function being relaxed, the implication direction, the rhs, and
/// the variable-bounds / parameter-value lookups for the interval analysis.
#[derive(Clone, Copy, Debug)]
pub struct BigMRequest<'a, F, G> {
    /// The construct the Big-M is for.
    pub construct: Construct,
    /// Human-readable expression the Big-M is derived for (naming in errors).
    pub expression: &'a str,
    /// The scalar function whose interval bounds the implication relaxes.
    pub function: &'a ScalarFunction,
    /// The implication direction being relaxed.
    pub side: BigMImplication,
    /// The rhs of the one-sided implication.
    pub rhs: f64,
    /// Resolves each variable's declared bounds.
    pub variable_bounds: F,
    /// Resolves each parameter's current value.
    pub parameter_values: G,
}

/// Derive the tightest finite one-sided Big-M for `f(x) <= rhs`
/// ([`BigMImplication::Upper`]) or `f(x) >= rhs` ([`BigMImplication::Lower`]).
///
/// `M = max(0, max f - rhs)` for `Upper`; `M = max(0, rhs - min f)` for
/// `Lower`. The relevant endpoint comes from a deterministic interval analysis
/// over the construct's declared bounds. Returns the finite derived M, or the
/// construct-aware [`CompileError::UnboundedBigM`] when the relevant endpoint
/// is infinite — never a silent default constant (SM-13.2, D12). Non-finite
/// input (NaN coefficients/bounds/constants/parameters) is a typed
/// [`CompileError::InvalidBigM`] (SM-13.1).
pub fn bound_big_m_implied<F, G>(
    analyzer: &BoundAnalyzer,
    request: BigMRequest<'_, F, G>,
) -> Result<f64, CompileError>
where
    F: Fn(VarId) -> Bounds,
    G: Fn(ParamId) -> f64,
{
    let BigMRequest {
        construct,
        expression,
        function,
        side,
        rhs,
        variable_bounds,
        parameter_values,
    } = request;
    if !rhs.is_finite() {
        return Err(CompileError::InvalidBigM {
            construct,
            expression: expression.to_string(),
            reason: "non-finite rhs".to_string(),
        });
    }
    let trace = analyzer
        .interval_of(function, variable_bounds, parameter_values)
        .map_err(|e| CompileError::InvalidBigM {
            construct,
            expression: expression.to_string(),
            reason: e.to_string(),
        })?;
    derived_big_m(construct, expression, &trace.result, side, rhs)
}

/// Derive the finite M for a pre-analyzed interval (shared by the helpers).
fn derived_big_m(
    construct: Construct,
    expression: &str,
    interval: &Interval,
    side: BigMImplication,
    rhs: f64,
) -> Result<f64, CompileError> {
    match side {
        BigMImplication::Upper => {
            if interval.upper.is_infinite() {
                return Err(unbounded_error(construct, expression));
            }
            let m = (interval.upper - rhs).max(0.0);
            if m.is_infinite() {
                // `upper - rhs` overflowed: no finite M is derivable (the
                // true minimum is astronomically large) — fail closed.
                return Err(unbounded_error(construct, expression));
            }
            Ok(m)
        }
        BigMImplication::Lower => {
            if interval.lower.is_infinite() {
                return Err(unbounded_error(construct, expression));
            }
            let m = (rhs - interval.lower).max(0.0);
            if m.is_infinite() {
                return Err(unbounded_error(construct, expression));
            }
            Ok(m)
        }
    }
}

/// Validate an explicit user-supplied Big-M against the known bounds where
/// possible (SM-13.3), returning it when consistent.
///
/// When the derived minimum M is finite, an explicit M smaller than that
/// minimum is rejected as inconsistent. When the relevant endpoint is
/// infinite (no finite derived bound exists), the explicit finite value is
/// accepted — that is the D12 explicit-user-value contract. A non-finite,
/// non-positive, or inconsistent explicit M is a typed
/// [`CompileError::InvalidBigM`]; an unbounded derivation with no explicit
/// substitution is surfaced through [`bound_big_m_implied`] instead.
pub fn validated_explicit_big_m<F, G>(
    analyzer: &BoundAnalyzer,
    request: BigMRequest<'_, F, G>,
    proposed: f64,
) -> Result<f64, CompileError>
where
    F: Fn(VarId) -> Bounds,
    G: Fn(ParamId) -> f64,
{
    let BigMRequest {
        construct,
        expression,
        function,
        side,
        rhs,
        variable_bounds,
        parameter_values,
    } = request;
    if !rhs.is_finite() {
        return Err(CompileError::InvalidBigM {
            construct,
            expression: expression.to_string(),
            reason: "non-finite rhs".to_string(),
        });
    }
    if !proposed.is_finite() || proposed <= 0.0 {
        return Err(CompileError::InvalidBigM {
            construct,
            expression: expression.to_string(),
            reason: format!("explicit M must be finite and positive, got {proposed}"),
        });
    }
    let trace = analyzer
        .interval_of(function, variable_bounds, parameter_values)
        .map_err(|e| CompileError::InvalidBigM {
            construct,
            expression: expression.to_string(),
            reason: e.to_string(),
        })?;
    let derived_minimum = match side {
        BigMImplication::Upper => {
            if trace.result.upper.is_infinite() {
                None
            } else {
                Some((trace.result.upper - rhs).max(0.0))
            }
        }
        BigMImplication::Lower => {
            if trace.result.lower.is_infinite() {
                None
            } else {
                Some((rhs - trace.result.lower).max(0.0))
            }
        }
    };
    match derived_minimum {
        Some(min) if proposed < min => Err(CompileError::InvalidBigM {
            construct,
            expression: expression.to_string(),
            reason: format!("explicit M {proposed} is smaller than the derived minimum {min}"),
        }),
        _ => Ok(proposed),
    }
}

/// Derive the one-sided Big-M for `function` against `rhs` directly from a
/// canonical snapshot's declared variable bounds and evaluated parameter
/// values (a crate-internal convenience the P32 bridges use). Returns the
/// finite M together with the [`BoundTrace`] provenance sources so a bridge can
/// record the SM-13.5 bound-evidence report entry.
pub(crate) fn bound_big_m_implied_snapshot(
    analyzer: &BoundAnalyzer,
    construct: Construct,
    expression: &str,
    function: &ScalarFunction,
    side: BigMImplication,
    rhs: f64,
    snapshot: &ModelSnapshot,
) -> Result<(f64, Vec<BoundSource>), CompileError> {
    if !rhs.is_finite() {
        return Err(CompileError::InvalidBigM {
            construct,
            expression: expression.to_string(),
            reason: "non-finite rhs".to_string(),
        });
    }
    let variable_bounds = |v: VarId| {
        snapshot
            .variables
            .iter()
            .find(|e| e.id == v)
            .map(|e| e.bounds)
            .unwrap_or(Bounds::UNBOUNDED)
    };
    let parameter_values = |p: ParamId| {
        snapshot
            .parameters
            .iter()
            .find(|e| e.id == p)
            .map(|e| e.value)
            .unwrap_or(0.0)
    };
    let trace = analyzer
        .interval_of(function, variable_bounds, parameter_values)
        .map_err(|e| CompileError::InvalidBigM {
            construct,
            expression: expression.to_string(),
            reason: e.to_string(),
        })?;
    let m = derived_big_m(construct, expression, &trace.result, side, rhs)?;
    Ok((m, trace.sources))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::ConstructId;

    fn var(index: u32) -> VarId {
        VarId::new(index, crate::id::Generation::new())
    }

    fn construct() -> Construct {
        ConstructId::allocate().expect("construct id allocation")
    }

    fn no_params(_: ParamId) -> f64 {
        0.0
    }

    fn expr_linear(expr: LinExpr) -> ScalarFunction {
        ScalarFunction::Linear(expr)
    }

    /// Build a `BigMRequest` for the tests (no parameter dependencies).
    fn request<'a, F>(
        c: Construct,
        expression: &'a str,
        f: &'a ScalarFunction,
        bounds: F,
        side: BigMImplication,
        rhs: f64,
    ) -> BigMRequest<'a, F, fn(ParamId) -> f64>
    where
        F: Fn(VarId) -> Bounds,
    {
        BigMRequest {
            construct: c,
            expression,
            function: f,
            side,
            rhs,
            variable_bounds: bounds,
            parameter_values: no_params,
        }
    }

    // ── One-sided Big-M: finite-bound derivation ───────────────────────────

    /// `f = 2x`, `x in [0, 10]`, `Upper` with `rhs = 5`:
    /// `max f = 20`, so `M = max(0, 20 - 5) = 15`.
    #[test]
    fn bound_big_m_implied_derives_finite_m_from_bounded_upper() {
        let analyzer = BoundAnalyzer::new();
        let x = var(0);
        let f = expr_linear(LinExpr::new().term(2.0, x));
        let c = construct();
        let bounds = |v: VarId| {
            if v == x {
                Bounds::new(0.0, 10.0)
            } else {
                Bounds::UNBOUNDED
            }
        };
        let m = bound_big_m_implied(
            &analyzer,
            request(c, "2x <= 5", &f, bounds, BigMImplication::Upper, 5.0),
        )
        .expect("bounded implication must derive a finite M");
        assert_eq!(m, 15.0);
    }

    /// `f = 2x`, `x in [0, 10]`, `Lower` with `rhs = 25`:
    /// `min f = 0`, so `M = max(0, 25 - 0) = 25`.
    #[test]
    fn bound_big_m_implied_derives_finite_m_from_bounded_lower() {
        let analyzer = BoundAnalyzer::new();
        let x = var(0);
        let f = expr_linear(LinExpr::new().term(2.0, x));
        let c = construct();
        let bounds = |v: VarId| {
            if v == x {
                Bounds::new(0.0, 10.0)
            } else {
                Bounds::UNBOUNDED
            }
        };
        let m = bound_big_m_implied(
            &analyzer,
            request(c, "2x >= 25", &f, bounds, BigMImplication::Lower, 25.0),
        )
        .expect("bounded implication must derive a finite M");
        assert_eq!(m, 25.0);
    }

    // ── One-sided Big-M: construct-aware UnboundedBigM (never a constant) ──

    /// A free variable on the relevant side returns the construct-aware
    /// `UnboundedBigM` marker — never a silent default constant (D12).
    #[test]
    fn bound_big_m_implied_returns_construct_aware_marker_for_free_variable() {
        let analyzer = BoundAnalyzer::new();
        let x = var(0);
        let f = expr_linear(LinExpr::new().term(1.0, x));
        let c = construct();
        let err = bound_big_m_implied(
            &analyzer,
            request(
                c,
                "free_expr <= 5",
                &f,
                |_| Bounds::UNBOUNDED,
                BigMImplication::Upper,
                5.0,
            ),
        )
        .expect_err("a free variable on the upper side must be unbounded");
        assert!(
            matches!(
                &err,
                CompileError::UnboundedBigM { construct, expression }
                    if *construct == c && expression.as_str() == "free_expr <= 5"
            ),
            "marker must name the construct and expression, got {err:?}"
        );
    }

    /// The relevant side alone determines unboundedness: `f = x` free in one
    /// direction but bounded on the relevant side still derives a finite M.
    #[test]
    fn bound_big_m_implied_uses_only_the_relevant_side() {
        let analyzer = BoundAnalyzer::new();
        let x = var(0);
        // x in [0, +inf): upper endpoint +inf, lower endpoint 0.
        let f = expr_linear(LinExpr::new().term(1.0, x));
        let c = construct();
        let bounds = |v: VarId| {
            if v == x {
                Bounds::new(0.0, f64::INFINITY)
            } else {
                Bounds::UNBOUNDED
            }
        };
        // Upper implication is unbounded (max f = +inf)...
        assert!(bound_big_m_implied(
            &analyzer,
            request(c, "x <= 5", &f, bounds, BigMImplication::Upper, 5.0),
        )
        .is_err());
        // ...but the Lower implication is finite: M = max(0, 5 - 0) = 5.
        let m = bound_big_m_implied(
            &analyzer,
            request(c, "x >= 5", &f, bounds, BigMImplication::Lower, 5.0),
        )
        .expect("bounded lower side must derive a finite M");
        assert_eq!(m, 5.0);
    }

    // ── Explicit M validation (SM-13.3) ────────────────────────────────────

    /// `f = 2x`, `x in [0, 10]`, `Upper` `rhs = 5` -> derived minimum M = 15.
    /// An explicit M = 10 is inconsistent and rejected.
    #[test]
    fn validated_explicit_big_m_rejects_inconsistent_value() {
        let analyzer = BoundAnalyzer::new();
        let x = var(0);
        let f = expr_linear(LinExpr::new().term(2.0, x));
        let c = construct();
        let bounds = |v: VarId| {
            if v == x {
                Bounds::new(0.0, 10.0)
            } else {
                Bounds::UNBOUNDED
            }
        };
        let err = validated_explicit_big_m(
            &analyzer,
            request(c, "2x <= 5", &f, bounds, BigMImplication::Upper, 5.0),
            10.0,
        )
        .expect_err("an explicit M below the derived minimum is inconsistent");
        assert!(
            matches!(&err, CompileError::InvalidBigM { construct, expression, .. } if *construct == c && expression.as_str() == "2x <= 5"),
            "inconsistent explicit M must be a typed InvalidBigM, got {err:?}"
        );
    }

    /// An explicit M at or above the derived minimum is accepted.
    #[test]
    fn validated_explicit_big_m_accepts_consistent_value() {
        let analyzer = BoundAnalyzer::new();
        let x = var(0);
        let f = expr_linear(LinExpr::new().term(2.0, x));
        let c = construct();
        let bounds = |v: VarId| {
            if v == x {
                Bounds::new(0.0, 10.0)
            } else {
                Bounds::UNBOUNDED
            }
        };
        let m = validated_explicit_big_m(
            &analyzer,
            request(c, "2x <= 5", &f, bounds, BigMImplication::Upper, 5.0),
            20.0,
        )
        .expect("an explicit M at or above the derived minimum is accepted");
        assert_eq!(m, 20.0);
    }

    /// When the derived bound is infinite (free variable), an explicit finite
    /// user M is accepted — the D12 explicit-value contract.
    #[test]
    fn validated_explicit_big_m_accepts_finite_value_when_bounds_unbounded() {
        let analyzer = BoundAnalyzer::new();
        let x = var(0);
        let f = expr_linear(LinExpr::new().term(1.0, x));
        let c = construct();
        let m = validated_explicit_big_m(
            &analyzer,
            request(
                c,
                "free_expr <= 5",
                &f,
                |_| Bounds::UNBOUNDED,
                BigMImplication::Upper,
                5.0,
            ),
            1_000.0,
        )
        .expect("the explicit user value substitutes for the missing finite proof");
        assert_eq!(m, 1_000.0);
    }

    /// A non-finite or non-positive explicit M is rejected as invalid.
    #[test]
    fn validated_explicit_big_m_rejects_non_positive_value() {
        let analyzer = BoundAnalyzer::new();
        let x = var(0);
        let f = expr_linear(LinExpr::new().term(1.0, x));
        let c = construct();
        let err = validated_explicit_big_m(
            &analyzer,
            request(
                c,
                "x <= 5",
                &f,
                |v| {
                    if v == x {
                        Bounds::new(0.0, 10.0)
                    } else {
                        Bounds::UNBOUNDED
                    }
                },
                BigMImplication::Upper,
                5.0,
            ),
            -3.0,
        )
        .expect_err("a non-positive explicit M is invalid");
        assert!(matches!(err, CompileError::InvalidBigM { .. }));
    }

    // ── NaN surfacing through the helpers (SM-13.1) ─────────────────────────

    /// A NaN coefficient during Big-M derivation is a typed InvalidBigM.
    #[test]
    fn bound_big_m_implied_surfaces_nan_analysis_as_typed_error() {
        let analyzer = BoundAnalyzer::new();
        let x = var(0);
        let f = expr_linear(LinExpr::new().term(f64::NAN, x));
        let c = construct();
        let err = bound_big_m_implied(
            &analyzer,
            request(
                c,
                "nan_expr <= 5",
                &f,
                |v| {
                    if v == x {
                        Bounds::new(0.0, 10.0)
                    } else {
                        Bounds::UNBOUNDED
                    }
                },
                BigMImplication::Upper,
                5.0,
            ),
        )
        .expect_err("NaN input must be a typed error, never a silent M");
        assert!(
            matches!(&err, CompileError::InvalidBigM { construct, expression, .. } if *construct == c && expression.as_str() == "nan_expr <= 5")
        );
    }
}
