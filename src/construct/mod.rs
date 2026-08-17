//! Canonical semantic constructs (design §7).
//!
//! High-level constructs remain canonical entities and are not eagerly erased
//! into backend rows. One generation-safe construct arena owns lifecycle —
//! ROML does not add a separate side map for every feature.
//!
//! Design §7 declares nine construct kinds (`Indicator`, `Reification`,
//! `MinMax`, `AbsoluteValue`, `Boolean`, `Cardinality`, `BinaryProduct`,
//! `PiecewiseLinear`, `SoftConstraint`) as the extension surface. P25 declared
//! [`ConstructKind`] and its `#[non_exhaustive]` extension boundary and stored
//! only the crate-private `FixturePayload`. P32 Task 16 activates the four
//! logical-construct variants (`Indicator`, `Reification`, `Boolean`,
//! `Cardinality`) with exact semantic payloads; the remaining variants land in
//! P30/P32/P33 follow-up plans.
//!
//! # Public exports (A30, P32)
//!
//! P32 Task 16 activates the real per-construct variants, so the module and
//! [`ConstructKind`]/[`ConstructEntry`] become **public** exports (A30). The
//! `Fixture` variant and `FixturePayload` are `#[cfg(test)]`-gated test-only
//! scaffolding: they exist solely for the in-crate construct lifecycle tests
//! and are ABSENT from the public API surface in non-test builds (external code
//! can never name `ConstructKind::Fixture` or `FixturePayload`). The
//! `#[non_exhaustive]` extension boundary on [`ConstructKind`] stays (A30).

pub mod absolute;
pub mod boolean;
pub mod cardinality;
pub mod indicator;
pub mod minmax;
pub mod piecewise_linear;
pub mod product;
pub mod reification;
pub mod soft_constraint;

use std::collections::HashMap;

use crate::expr::LinExpr;
use crate::function::ScalarFunction;
use crate::id::{ParamId, VarId};
use crate::identity::{ConstructId, IdentityOverflow};

pub use absolute::{AbsoluteValueConstraint, AbsoluteValueVariant};
pub use boolean::{BooleanConstraint, BooleanKind};
pub use cardinality::{CardinalityConstraint, CardinalityKind};
pub use indicator::{IndicatorConstraint, IndicatorDirection};
pub use minmax::{MinMaxConstraint, MinMaxRelation, MinMaxSense};
pub(crate) use piecewise_linear::classify_curvature_from_slopes;
pub use piecewise_linear::{
    ExtrapolationPolicy, PiecewiseLinearConstraint, PwlCurvature, PwlEvalError, PwlPoint,
    PwlRelation,
};
pub use product::{BinaryProductConstraint, ProductOperand};
pub use reification::ReificationConstraint;
pub use soft_constraint::{
    PenaltyPolicy, PenaltyTarget, SoftConstraint, SoftConstraintConstraint, ViolationPolicy,
    ViolationRole, ViolationSide,
};

/// A canonical semantic construct handle (design §7).
pub type Construct = ConstructId;

/// The kind of a canonical semantic construct (design §7).
///
/// The enum is `#[non_exhaustive]`: the design §7 variants
/// (`Indicator`, `Reification`, `MinMax`, `AbsoluteValue`, `Boolean`,
/// `Cardinality`, `BinaryProduct`, `PiecewiseLinear`, `SoftConstraint`) are
/// the declared extension surface and land with the per-construct modules in
/// P30/P32/P33. P32 Task 16 activates the four logical-construct variants; the
/// `Fixture` variant remains crate-private (A30) and pre-implements no
/// formulation.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ConstructKind {
    /// Indicator: a one-way implication over a binary activator (design §16.1).
    Indicator(IndicatorConstraint),
    /// Reification: `b = 1 ⟺ function ∈ set` (design §16.2).
    Reification(ReificationConstraint),
    /// Boolean: implication/equivalence/any/all over binary variables (design §16.4).
    Boolean(BooleanConstraint),
    /// Cardinality: exactly/at-most/at-least-k over binary variables (design §16.4).
    Cardinality(CardinalityConstraint),
    /// Min/max: exact or one-sided epigraph/hypograph over linear operands
    /// (design §16.3, P32 Task 17a).
    MinMax(MinMaxConstraint),
    /// Absolute value / positive part / clamp (design §16.3, P32 Task 17b).
    AbsoluteValue(AbsoluteValueConstraint),
    /// Binary product: binary-binary or binary-times-bounded-linear (design
    /// §16.5, P32 Task 17c).
    BinaryProduct(BinaryProductConstraint),
    /// Piecewise-linear: epigraph/hypograph/exact-graph over finite strictly
    /// increasing breakpoints (design §17, P33 Task 1).
    PiecewiseLinear(PiecewiseLinearConstraint),
    /// Persistent canonical softening of one primitive constraint.
    SoftConstraint(SoftConstraintConstraint),
    /// Test-only crate-private fixture payload used by the in-crate construct
    /// lifecycle tests (A30 — `#[cfg(test)]`-gated, so the variant is ABSENT
    /// from the public API surface in non-test builds).
    #[doc(hidden)]
    #[cfg(test)]
    Fixture(FixturePayload),
}

/// Test-only crate-private fixture payload for the in-crate construct lifecycle
/// tests (A30).
///
/// Minimal and intentionally solver-free; carries no formulation. The type is
/// `#[cfg(test)]`-gated and `#[doc(hidden)]`: it exists only in test builds and
/// is ABSENT from the public API surface in non-test builds, so external code
/// can never name or construct the fixture scaffolding (A30).
#[doc(hidden)]
#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub struct FixturePayload {
    /// A distinguishing key (crate-visible only — A30).
    pub(crate) key: String,
    /// A numeric value (crate-visible only — A30).
    pub(crate) value: f64,
}

#[cfg(test)]
impl FixturePayload {
    /// Build a fixture payload (crate-private — A30).
    pub(crate) fn new(key: String, value: f64) -> Self {
        Self { key, value }
    }
}

/// A construct entry in canonical state (design §7).
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructEntry {
    /// The stable generation-safe construct identity.
    pub id: Construct,
    /// The exact semantic construct type.
    pub kind: ConstructKind,
    /// Whether the construct is active in the model.
    pub active: bool,
    /// Per-construct formulation preference (F4).
    ///
    /// Threaded through `Change::ConstructAdded`/`ModelOp::AddConstruct` and
    /// the snapshot/delta reconstruction paths so P26 can honor
    /// Auto/Portable/NativeRequired from canonical snapshots/deltas. The
    /// construct arena reads preference exclusively from this entry
    /// (single authority).
    pub preference: FormulationPreference,
}

/// Per-construct formulation preference (design §7, §8.1).
///
/// Narrows the global compilation policy but can never weaken exactness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormulationPreference {
    /// Prefer a qualified exact native primitive, otherwise an exact portable
    /// bridge.
    Auto,
    /// Force deterministic ROML formulations.
    Portable,
    /// Reject when the backend lacks exact native support.
    NativeRequired,
}

/// Internal construct data held by the arena.
///
/// Crate-private (A30): the construct arena is internal scaffolding; external
/// consumers reach constructs through the public builders and the
/// [`ConstructEntry`] in canonical snapshots/deltas.
#[derive(Clone, Debug)]
pub(crate) struct ConstructData {
    /// The canonical construct entry (single authority for kind, activity, and
    /// formulation preference — F4).
    pub entry: ConstructEntry,
    /// Derived parameter dependencies of the payload.
    pub parameter_dependencies: Vec<ParamId>,
}

/// The generation-safe construct arena (design §7).
///
/// Ids are issued by the checked atomic counter — never reused, zero
/// reserved — and the store invalidates removed ids: any operation on a
/// removed id is rejected with a typed error.
#[derive(Clone, Debug, Default)]
pub(crate) struct ConstructStore {
    arena: HashMap<Construct, ConstructData>,
}

impl ConstructStore {
    /// Create an empty construct store.
    pub fn new() -> Self {
        Self {
            arena: HashMap::new(),
        }
    }

    /// Allocate a fresh construct id and insert the entry. Active by default.
    ///
    /// P25 (F3): crate-private scaffolding exercised by the in-crate construct
    /// lifecycle tests via `Model::add_construct_fixture`; the real per-kind
    /// builder APIs land in P32.
    #[allow(dead_code)]
    pub fn add(
        &mut self,
        kind: ConstructKind,
        preference: FormulationPreference,
    ) -> Result<Construct, IdentityOverflow> {
        let id = ConstructId::allocate()?;
        self.add_with_id(id, kind, preference)
    }

    /// Insert a construct with a PRE-ALLOCATED id (IN-03 atomic builders).
    ///
    /// The caller reserves the id first (e.g. before creating the construct's
    /// output/activator variable) so a construct-id failure cannot leave an
    /// orphaned variable in the arena/changelog. Returns the pre-allocated id.
    pub fn add_with_id(
        &mut self,
        id: Construct,
        kind: ConstructKind,
        preference: FormulationPreference,
    ) -> Result<Construct, IdentityOverflow> {
        let parameter_dependencies = derive_parameter_dependencies(&kind);
        self.arena.insert(
            id,
            ConstructData {
                entry: ConstructEntry {
                    id,
                    kind,
                    active: true,
                    preference,
                },
                parameter_dependencies,
            },
        );
        Ok(id)
    }

    /// Read construct data by id (stale/removed ids return `None`).
    pub fn get(&self, id: Construct) -> Option<&ConstructData> {
        self.arena.get(&id)
    }

    /// Mutate construct data by id (stale/removed ids return `None`).
    pub fn get_mut(&mut self, id: Construct) -> Option<&mut ConstructData> {
        self.arena.get_mut(&id)
    }

    /// Whether a live construct with this id exists.
    pub fn contains(&self, id: Construct) -> bool {
        self.arena.contains_key(&id)
    }

    /// Remove a construct, invalidating its id. Returns the removed data.
    pub fn remove(&mut self, id: Construct) -> Option<ConstructData> {
        self.arena.remove(&id)
    }

    /// Number of live constructs.
    pub fn len(&self) -> usize {
        self.arena.len()
    }

    /// Iterate over live constructs in arbitrary (deterministic per-store)
    /// order; callers sort for deterministic output.
    pub fn iter(&self) -> impl Iterator<Item = (Construct, &ConstructData)> {
        self.arena.iter().map(|(id, data)| (*id, data))
    }
}

/// Collect the distinct variable ids referenced by a linear expression.
fn lin_expr_variables(expr: &LinExpr) -> Vec<VarId> {
    let mut vars: Vec<VarId> = expr.terms.iter().map(|t| t.var).collect();
    vars.sort();
    vars.dedup();
    vars
}

/// Collect the distinct variable ids referenced by a scalar function.
fn scalar_function_variables(function: &ScalarFunction) -> Vec<VarId> {
    match function {
        ScalarFunction::Linear(expr) => lin_expr_variables(expr),
    }
}

/// Derive the variable dependencies of a construct payload (F1).
///
/// The variables whose domain/removal can change the construct's generated
/// bridge artifact: every user variable the construct's formulation references
/// (activator/output, function/operand variables, binary operands). The
/// compiler persists these as [`crate::compiler::bridge::BridgeDependency`]s
/// and forces a rebuild when a dependency-affecting delta touches one.
pub(crate) fn derive_variable_dependencies(kind: &ConstructKind) -> Vec<VarId> {
    let mut vars: Vec<VarId> = match kind {
        ConstructKind::Indicator(payload) => {
            let mut v = vec![payload.activator];
            v.extend(scalar_function_variables(&payload.function));
            v
        }
        ConstructKind::Reification(payload) => {
            let mut v = vec![payload.activator];
            v.extend(scalar_function_variables(&payload.function));
            v
        }
        ConstructKind::Boolean(payload) => match &payload.kind {
            BooleanKind::Implication {
                antecedent,
                consequent,
            } => vec![*antecedent, *consequent],
            BooleanKind::Equivalence { left, right } => vec![*left, *right],
            BooleanKind::Any { variables } | BooleanKind::All { variables } => variables.clone(),
        },
        ConstructKind::Cardinality(payload) => payload.variables.clone(),
        ConstructKind::MinMax(payload) => {
            let mut v = vec![payload.output];
            for op in &payload.operands {
                v.extend(lin_expr_variables(op));
            }
            v
        }
        ConstructKind::AbsoluteValue(payload) => {
            let mut v = vec![payload.output];
            v.extend(lin_expr_variables(&payload.expression));
            v
        }
        ConstructKind::BinaryProduct(payload) => {
            let mut v = vec![payload.output];
            for op in [&payload.left, &payload.right] {
                match op {
                    ProductOperand::Binary(var) => v.push(*var),
                    ProductOperand::Linear(expr) => v.extend(lin_expr_variables(expr)),
                }
            }
            v
        }
        ConstructKind::PiecewiseLinear(payload) => {
            let mut v = vec![payload.output];
            v.extend(lin_expr_variables(&payload.argument));
            v
        }
        // The referenced constraint's cells are snapshot-dependent rather
        // than payload-owned. The soft bridge captures those constraint,
        // variable, and parameter edges from the snapshot when it compiles.
        ConstructKind::SoftConstraint(_) => Vec::new(),
        #[cfg(test)]
        ConstructKind::Fixture(_) => Vec::new(),
    };
    vars.sort();
    vars.dedup();
    vars
}

/// Derive the parameter dependencies of a construct payload (design §7).
///
/// The P25 fixture payload carries no parameters. Later per-construct modules
/// derive their dependencies from their payloads; any cached dependency list
/// is invariant-checked against this derivation (P25 Task 4).
pub(crate) fn derive_parameter_dependencies(kind: &ConstructKind) -> Vec<ParamId> {
    match kind {
        ConstructKind::Indicator(payload) => payload.parameter_dependencies(),
        ConstructKind::Reification(payload) => payload.parameter_dependencies(),
        ConstructKind::Boolean(_) => Vec::new(),
        ConstructKind::Cardinality(_) => Vec::new(),
        ConstructKind::MinMax(payload) => payload.parameter_dependencies(),
        ConstructKind::AbsoluteValue(payload) => payload.parameter_dependencies(),
        ConstructKind::BinaryProduct(payload) => payload.parameter_dependencies(),
        ConstructKind::PiecewiseLinear(payload) => payload.parameter_dependencies(),
        ConstructKind::SoftConstraint(payload) => payload.parameter_dependencies(),
        #[cfg(test)]
        ConstructKind::Fixture(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::ConstructId;

    /// IN-03: a construct can be inserted with a PRE-ALLOCATED id, so a builder
    /// can reserve the construct id BEFORE creating its output/activator
    /// variable — a construct-id failure then cannot leave an orphaned variable
    /// in the arena/changelog.
    #[test]
    fn store_add_with_id_inserts_pre_allocated_construct() {
        let mut store = ConstructStore::new();
        let id = ConstructId::allocate().expect("construct id allocation");
        let kind = ConstructKind::Fixture(FixturePayload::new("atomic".to_string(), 1.0));
        let returned = store
            .add_with_id(id, kind.clone(), FormulationPreference::Auto)
            .expect("pre-allocated insert must succeed");
        assert_eq!(returned, id, "add_with_id returns the pre-allocated id");
        let data = store.get(id).expect("construct present after add_with_id");
        assert_eq!(data.entry.kind, kind);
        assert!(data.entry.active);
        assert_eq!(store.len(), 1);
    }
}
