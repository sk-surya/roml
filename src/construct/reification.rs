//! Reification construct payload (design §7, §16.2; D14).
//!
//! A bi-implication between a binary variable and a scalar-function-in-set
//! relation: `b = 1 ⟺ function ∈ set`. Continuous exact reification requires an
//! explicit separation tolerance; the unit gap may be inferred only when the
//! expression is proven integer-valued over its domain (D14, SM-12.2). The
//! payload stores the exact semantic content plus the separation contract; the
//! per-construct formulation preference lives in the
//! [`ConstructEntry`](crate::construct::ConstructEntry) (A29).

use crate::function::{ScalarFunction, ScalarSet};
use crate::id::{ParamId, VarId};

/// The exact semantic payload of a reification construct (design §7, §16.2).
///
/// `activator` is the binary variable holding the reification result
/// (`b = 1 ⟺ function ∈ set`); the builder creates it and stores it here so the
/// compiler can emit the two implications. `separation_tolerance = Some(tol)`
/// records an explicit separation (must be finite and positive, validated by
/// the builder). `None` means the compiler uses the unit gap — valid only when
/// `proven_integrality` is `true`, because the unit gap is exact only for
/// integer-valued expressions (D14).
#[derive(Clone, Debug, PartialEq)]
pub struct ReificationConstraint {
    /// The binary variable holding the reification result (created by the
    /// builder).
    pub activator: VarId,
    /// The scalar function whose relation is reified.
    pub function: ScalarFunction,
    /// The set constraining the function (`le`/`ge` thresholds; exact
    /// equality/interval reification is a typed build-time rejection).
    pub set: ScalarSet,
    /// Explicit separation tolerance; `None` means the unit gap (only valid
    /// with `proven_integrality`).
    pub separation_tolerance: Option<f64>,
    /// Whether the expression is proven integer-valued over its domain.
    pub proven_integrality: bool,
}

impl ReificationConstraint {
    /// Derive the parameter dependencies of the constrained function AND its
    /// set thresholds (F1; WR-03).
    ///
    /// The set's `ValueExpr` threshold can reference a parameter (evaluated by
    /// the bridge's `eval_bound` at compile time), so the construct dependency
    /// derivation must attribute it too.
    pub fn parameter_dependencies(&self) -> Vec<ParamId> {
        let mut deps: Vec<ParamId> = self.function.parameter_dependencies();
        for p in self.set.dependencies() {
            if !deps.contains(&p) {
                deps.push(p);
            }
        }
        deps
    }
}
