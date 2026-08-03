//! Core model layer (solver-agnostic).
//!
//! The Model owns all modeling entities and is completely solver-agnostic.
//! It supports:
//! - Adding/removing/modifying variables, constraints, objectives, parameters
//! - Coefficient management with automatic parameter propagation
//! - Change tracking for incremental solver updates
//! - Transaction-based parameter batching

pub mod changelog;
pub mod coefficient;
pub mod constraint;
pub mod objective;
pub mod parameter;
pub mod transaction;
pub mod validation;
pub mod variable;

pub use changelog::Change;
pub(crate) use changelog::ChangeLog;
pub(crate) use coefficient::CoefficientData;
pub(crate) use coefficient::CoefficientIndex;
pub use coefficient::CoefficientTarget;
pub use constraint::ConstraintBounds;
pub(crate) use constraint::ConstraintStore;
pub(crate) use objective::ObjectiveStore;
pub use objective::Sense;
pub(crate) use parameter::ParameterStore;
pub use parameter::{parameter, ParameterDef};
pub(crate) use transaction::Transaction;
pub(crate) use variable::VariableStore;
pub use variable::{
    binary, continuous, integer, Bounds, FixingProvenance, SemiDomain, VarType, VariableDef,
    VariableDomain, VariableFixing,
};

/// Semantic alias for a variable handle (D8). A plain type alias of [`VarId`].
pub type Variable = crate::id::VarId;
/// Semantic alias for a constraint handle (D8). A plain type alias of [`ConId`].
pub type Constraint = crate::id::ConId;
/// Semantic alias for an objective handle (D8). A plain type alias of [`ObjId`].
pub type Objective = crate::id::ObjId;
/// Semantic alias for a parameter handle (D8). A plain type alias of [`ParamId`].
pub type Parameter = crate::id::ParamId;

#[cfg(test)]
use crate::construct::FixturePayload;
use crate::construct::{
    derive_parameter_dependencies, AbsoluteValueConstraint, AbsoluteValueVariant,
    BinaryProductConstraint, BooleanKind, CardinalityKind, Construct, ConstructEntry,
    ConstructKind, ConstructStore, FormulationPreference, IndicatorConstraint, IndicatorDirection,
    MinMaxConstraint, MinMaxRelation, MinMaxSense, ProductOperand, ReificationConstraint,
};
use crate::delta::{DeltaBatch, ModelOp};
use crate::expr::{LinExpr, TermCoeff};
use crate::function::{FunctionConstraint, ScalarFunction, ScalarSet};
use crate::id::{CoeffId, ConId, ObjId, ParamId, VarId};
use crate::identity::{ModelInstanceId, ModelLineageId};
use crate::metadata::{EntityMetadata, EntityRef};
use crate::revision::ModelRevision;
use crate::snapshot::ModelSnapshot;
use crate::solution::Solution;
// Options are now supplied via SolveRequest at the BackendSession boundary.
// Legacy import removed.

use crate::value_expr::ValueExpr;

use std::collections::HashSet;

use log::warn;

/// Error type for model operations.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelError {
    /// The specified variable was not found.
    VariableNotFound(VarId),
    /// The specified constraint was not found.
    ConstraintNotFound(ConId),
    /// The specified objective was not found.
    ObjectiveNotFound(ObjId),
    /// The specified parameter was not found.
    ParameterNotFound(ParamId),
    /// The specified coefficient was not found.
    CoefficientNotFound(CoeffId),
    /// The specified construct was not found (stale/removed construct id).
    ConstructNotFound(Construct),
    /// Invalid bounds (lower > upper).
    InvalidBounds,
    /// Binary bounds must lie within `[0, 1]`.
    InvalidBinaryBounds,
    /// A numeric value was not finite (NaN or infinite).
    NonFiniteValue(&'static str),
    /// A value was NaN.
    NaNValue(&'static str),
    /// A logical construct required a binary variable but received a
    /// non-binary one (continuous or integer) (SM-12.2).
    NonBinaryVariable(VarId),
    /// A cardinality construct received the same binary variable more than
    /// once (SM-12.5).
    DuplicateCardinalityVariable(VarId),
    /// A cardinality construct received an invalid `k` (negative, non-integral,
    /// or greater than the input length) (SM-12.5).
    InvalidCardinalityK {
        /// The offending `k` value.
        k: f64,
        /// Why the value is invalid.
        reason: &'static str,
    },
    /// Continuous exact reification without an explicit separation tolerance
    /// (SM-12.2, D14).
    ContinuousReificationWithoutSeparation,
    /// A reification separation tolerance was non-finite or non-positive
    /// (SM-12.2, D14).
    InvalidReificationSeparation(f64),
    /// Reification currently supports `le`/`ge` threshold relations only;
    /// equality/interval relations need a disjunctive complement not in the
    /// P32 two-implication contract.
    UnsupportedReificationSet,
    /// A proven-integer reification with the inferred unit gap requires an
    /// integral set threshold: `f > rhs ⟺ f >= rhs + 1` is exact only for an
    /// integer `rhs`, so a fractional threshold on a proven-integer expression
    /// is a typed build-time rejection (SM-12.2, D14; CR-01).
    NonIntegralReificationThreshold(f64),
    /// A construct input list was empty (Boolean any/all, cardinality).
    EmptyConstructInput,
    /// A min/max construct requires at least two operands (SM-12.3).
    MinMaxTooFewOperands,
    /// A min-epigraph / max-hypograph relation is trivially satisfiable
    /// (SM-12.3) — a min epigraph (`output >= min`) and a max hypograph
    /// (`output <= max`) impose no constraint.
    TriviallySatisfiableMinMax,
    /// An absolute-value-family expression is unbounded (free variable or
    /// unbounded parameter) — the bounded exact bridge cannot be built
    /// (SM-12.4).
    UnboundedConstructExpression,
    /// A clamp variant has invalid bounds (`lower > upper` or non-finite)
    /// (SM-12.4).
    InvalidClampBounds {
        /// The offending lower bound.
        lower: f64,
        /// The offending upper bound.
        upper: f64,
    },
    /// A product of two continuous operands is not exact MILP — no exact path
    /// exists and no relaxation is emitted (SM-12.7, D23).
    ContinuousTimesContinuousProduct,
    /// A fix value lies outside the variable's declared bounds (SM-05.5).
    ValueOutOfBounds {
        /// The variable being fixed.
        variable: VarId,
        /// The rejected fix value.
        value: f64,
        /// The declared bounds the value must lie inside.
        bounds: Bounds,
    },
    /// A fix value on an integer/binary variable is not integral beyond the
    /// named integrality tolerance (SM-05.5).
    NonIntegralValue {
        /// The variable being fixed.
        variable: VarId,
        /// The rejected non-integral fix value.
        value: f64,
        /// The integrality tolerance used for the check.
        tolerance: f64,
    },
    /// Declared-bound changes that exclude an active fixing fail atomically
    /// (SM-05.6): the fixing value is outside the requested bounds.
    BoundsExcludeFixing {
        /// The variable with the active fixing.
        variable: VarId,
        /// The active fixing value.
        value: f64,
        /// The requested bounds that exclude the fixing value.
        bounds: Bounds,
    },
    /// The integrality tolerance must be finite and non-negative.
    InvalidIntegralityTolerance(f64),
    /// Revision counter overflow.
    RevisionOverflow,
    /// An opaque identity counter was exhausted (ids never wrap).
    IdentityOverflow,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VariableNotFound(id) => write!(f, "Variable not found: {:?}", id),
            Self::ConstraintNotFound(id) => write!(f, "Constraint not found: {:?}", id),
            Self::ObjectiveNotFound(id) => write!(f, "Objective not found: {:?}", id),
            Self::ParameterNotFound(id) => write!(f, "Parameter not found: {:?}", id),
            Self::CoefficientNotFound(id) => write!(f, "Coefficient not found: {:?}", id),
            Self::ConstructNotFound(id) => {
                write!(f, "Construct not found (stale or removed): {id:?}")
            }
            Self::InvalidBounds => write!(f, "Invalid bounds: lower > upper"),
            Self::InvalidBinaryBounds => {
                write!(
                    f,
                    "Invalid binary bounds: binary variable bounds must lie within [0, 1]"
                )
            }
            Self::NonFiniteValue(label) => write!(f, "Value must be finite: {label}"),
            Self::NaNValue(label) => write!(f, "Value must not be NaN: {label}"),
            Self::NonBinaryVariable(id) => {
                write!(
                    f,
                    "a binary variable is required here, got non-binary {id:?}"
                )
            }
            Self::DuplicateCardinalityVariable(id) => {
                write!(f, "cardinality input contains a duplicate variable {id:?}")
            }
            Self::InvalidCardinalityK { k, reason } => {
                write!(f, "invalid cardinality k = {k}: {reason}")
            }
            Self::ContinuousReificationWithoutSeparation => write!(
                f,
                "continuous exact reification requires an explicit separation tolerance \
                 (D14); the unit gap is inferred only for proven-integer expressions"
            ),
            Self::InvalidReificationSeparation(s) => {
                write!(
                    f,
                    "invalid reification separation tolerance {s}: must be finite and > 0"
                )
            }
            Self::UnsupportedReificationSet => write!(
                f,
                "reification currently supports le/ge threshold relations only (P32 \
                 two-implication contract)"
            ),
            Self::NonIntegralReificationThreshold(v) => write!(
                f,
                "reification with the inferred unit gap requires an integral set threshold \
                 (D14, CR-01); got {v}: the unit gap `f > rhs ⟺ f >= rhs + 1` is exact only \
                 for an integer rhs — pass an explicit separation tolerance instead"
            ),
            Self::EmptyConstructInput => write!(f, "construct input list must not be empty"),
            Self::MinMaxTooFewOperands => {
                write!(f, "a min/max construct requires at least two operands")
            }
            Self::TriviallySatisfiableMinMax => write!(
                f,
                "a min-epigraph / max-hypograph min/max relation is trivially satisfiable \
                 (D13): choose exact or the complementary one-sided relation"
            ),
            Self::UnboundedConstructExpression => write!(
                f,
                "an absolute-value-family construct requires a bounded expression (free \
                 variables or unbounded parameters are a typed rejection; the bounded exact \
                 bridge cannot be built)"
            ),
            Self::InvalidClampBounds { lower, upper } => {
                write!(
                    f,
                    "invalid clamp bounds [{lower}, {upper}]: lower must be finite and <= upper"
                )
            }
            Self::ContinuousTimesContinuousProduct => write!(
                f,
                "a continuous-times-continuous product is not exact MILP (D23, SM-12.7); \
                 exact products cover binary-binary and binary-times-bounded-linear only"
            ),
            Self::ValueOutOfBounds {
                variable,
                value,
                bounds,
            } => write!(
                f,
                "fix value {value} for variable {variable:?} lies outside declared bounds {bounds:?}"
            ),
            Self::NonIntegralValue {
                variable,
                value,
                tolerance,
            } => write!(
                f,
                "fix value {value} for integer variable {variable:?} is not integral within tolerance {tolerance}"
            ),
            Self::BoundsExcludeFixing {
                variable,
                value,
                bounds,
            } => write!(
                f,
                "declared bounds {bounds:?} for variable {variable:?} exclude the active fixing value {value}"
            ),
            Self::InvalidIntegralityTolerance(tolerance) => {
                write!(f, "integrality tolerance must be >= 0 and finite, got {tolerance}")
            }
            Self::RevisionOverflow => write!(f, "revision counter overflow"),
            Self::IdentityOverflow => {
                write!(f, "identity counter exhausted (ids never wrap)")
            }
        }
    }
}

impl std::error::Error for ModelError {}

/// The core MILP model - solver-agnostic representation.
///
/// # Architecture
///
/// The model maintains:
/// - Variables with bounds, type, and activity
/// - Constraints with bounds and activity
/// - Objectives with sense and activity (only one active)
/// - Parameters as coefficient value sources
/// - Coefficients linking variables to constraints/objectives
/// - A changelog for incremental solver updates
/// - A transaction for batched parameter changes
///
/// # Invariants
///
/// - IDs are never reused (stable identity)
/// - Only one objective can be active at a time
/// - Parameter changes propagate to all dependent coefficients
/// - All mutations are logged for solver consumption
///
/// # Identity and metadata (P25)
///
/// Every model carries an opaque [`ModelLineageId`] and [`ModelInstanceId`]
/// (design §4). `Clone` preserves lineage but allocates a new instance id
/// (SM-02.7). Entity metadata is keyed by [`EntityRef`] and is canonical but
/// non-solver-affecting: metadata changes never advance the revision.
#[derive(Debug)]
pub struct Model {
    /// Variable storage.
    pub(crate) variables: VariableStore,
    /// Constraint storage.
    pub(crate) constraints: ConstraintStore,
    /// Objective storage.
    pub(crate) objectives: ObjectiveStore,
    /// Parameter storage.
    pub(crate) parameters: ParameterStore,
    /// Coefficient storage with multi-indexing.
    pub(crate) coefficients: CoefficientIndex,
    /// Change tracking for solver sync.
    pub(crate) changelog: ChangeLog,
    /// Transaction for batched parameter updates.
    pub(crate) transaction: Transaction,
    /// Optional model name.
    pub name: Option<String>,
    /// Model constants (e.g., tolerances).
    pub constants: ModelConstants,
    /// Tracks semi-continuous lower bounds per variable.
    /// A variable with an entry in this map must be 0 or ≥ the stored value.
    pub(crate) semicontinuous_lower: std::collections::HashMap<VarId, f64>,

    /// Synchronization coordinator for revisioned delta batch management.
    pub(crate) coordinator: crate::sync::SyncCoordinator,

    /// The lineage identity of this model (design §4.1). Shared by clones.
    lineage: ModelLineageId,
    /// The instance identity of this live model object (design §4.2).
    instance: ModelInstanceId,
    /// Canonical but non-solver-affecting entity metadata (design §5).
    metadata: std::collections::HashMap<EntityRef, EntityMetadata>,
    /// The generation-safe construct arena (design §7, P25 Task 4).
    constructs: ConstructStore,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            variables: VariableStore::default(),
            constraints: ConstraintStore::default(),
            objectives: ObjectiveStore::default(),
            parameters: ParameterStore::default(),
            coefficients: CoefficientIndex::default(),
            changelog: ChangeLog::default(),
            transaction: Transaction::default(),
            name: None,
            constants: ModelConstants::default(),
            semicontinuous_lower: std::collections::HashMap::new(),
            coordinator: crate::sync::SyncCoordinator::default(),
            // A fresh model allocates fresh lineage and instance ids. Zero is
            // reserved, so `Model::new` never collides with a sentinel.
            lineage: ModelLineageId::allocate().expect("model lineage counter exhausted"),
            instance: ModelInstanceId::allocate().expect("model instance counter exhausted"),
            metadata: std::collections::HashMap::new(),
            constructs: ConstructStore::new(),
        }
    }
}

impl Clone for Model {
    fn clone(&self) -> Self {
        Self {
            variables: self.variables.clone(),
            constraints: self.constraints.clone(),
            objectives: self.objectives.clone(),
            parameters: self.parameters.clone(),
            coefficients: self.coefficients.clone(),
            changelog: self.changelog.clone(),
            transaction: self.transaction.clone(),
            name: self.name.clone(),
            constants: self.constants.clone(),
            semicontinuous_lower: self.semicontinuous_lower.clone(),
            coordinator: self.coordinator.clone(),
            // Clone preserves lineage but allocates a NEW instance id
            // (SM-02.7, D28: a derived Clone would silently copy the instance).
            lineage: self.lineage,
            instance: ModelInstanceId::allocate().expect("model instance counter exhausted"),
            metadata: self.metadata.clone(),
            // Constructs survive clone with the same ids, kinds, and activity.
            constructs: self.constructs.clone(),
        }
    }
}

/// Model-level constants used by algebraic introspection (slack and violation
/// checks) and fixing validation (SM-05.5).
#[derive(Clone, Debug)]
pub struct ModelConstants {
    /// Tolerance for considering a constraint violated (negative slack).
    pub feasibility_tolerance: f64,
    /// Named integrality tolerance used by fix validation on integer/binary
    /// variables (SM-05.5). Default is consistent with the feasibility
    /// tolerance convention (1e-9).
    pub integrality_tolerance: f64,
}

impl Default for ModelConstants {
    fn default() -> Self {
        // default tolerance is a small epsilon used in slack/violation checks.
        Self {
            feasibility_tolerance: 1e-9,
            integrality_tolerance: 1e-9,
        }
    }
}

impl ModelConstants {
    /// Create model constants with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Helper to build a constants struct with a custom tolerance.
    pub fn set_feas_tol(feasibility_tolerance: f64) -> Self {
        Self {
            feasibility_tolerance,
            integrality_tolerance: 1e-9,
        }
    }
}

/// Map a distinct [`BoundAnalyzer`](crate::compiler::bounds::BoundAnalyzer)
/// failure to the matching [`ModelError`] variant so the specific cause is
/// preserved (IN-02) instead of collapsing every failure into one generic
/// `NonFiniteValue`.
fn bound_error_to_model_error(err: crate::compiler::bounds::BoundError) -> ModelError {
    use crate::compiler::bounds::BoundError;
    match err {
        BoundError::NonFiniteCoefficient { .. } => {
            ModelError::NonFiniteValue("expression coefficient")
        }
        BoundError::NonFiniteBound { .. } => ModelError::NaNValue("expression variable bound"),
        BoundError::InvalidBounds { .. } => ModelError::InvalidBounds,
        BoundError::NonFiniteConstant => ModelError::NonFiniteValue("expression constant"),
        BoundError::NonFiniteParameterValue { .. } => {
            ModelError::NonFiniteValue("expression parameter value")
        }
        BoundError::MissingParameter { .. } => {
            ModelError::NonFiniteValue("expression missing parameter")
        }
        BoundError::ArithmeticNan => ModelError::NaNValue("expression interval arithmetic"),
        BoundError::UnsupportedFunctionKind => {
            ModelError::NonFiniteValue("unsupported function kind")
        }
    }
}

#[cfg(test)]
mod bound_error_mapping_tests {
    use super::*;
    use crate::compiler::bounds::BoundError;

    /// IN-02: each distinct `BoundAnalyzer` failure maps to the matching
    /// `ModelError` variant — the specific cause is preserved, never collapsed
    /// into one generic `NonFiniteValue("expression bounds")`. (The distinct
    /// causes are defensive: `validate_expression_entities` and the interval
    /// clamping preempt most of them through the public builders, but the
    /// mapping keeps the analyzer reason observable wherever one surfaces.)
    #[test]
    fn expression_interval_maps_distinct_bound_error_causes() {
        let v = VarId::new(0, crate::id::Generation::new());
        let p = ParamId::new(0, crate::id::Generation::new());
        assert!(matches!(
            bound_error_to_model_error(BoundError::NonFiniteCoefficient { variable: v }),
            ModelError::NonFiniteValue("expression coefficient")
        ));
        assert!(matches!(
            bound_error_to_model_error(BoundError::NonFiniteBound { variable: v }),
            ModelError::NaNValue("expression variable bound")
        ));
        assert!(matches!(
            bound_error_to_model_error(BoundError::InvalidBounds { variable: v }),
            ModelError::InvalidBounds
        ));
        assert!(matches!(
            bound_error_to_model_error(BoundError::NonFiniteConstant),
            ModelError::NonFiniteValue("expression constant")
        ));
        assert!(matches!(
            bound_error_to_model_error(BoundError::NonFiniteParameterValue { parameter: p }),
            ModelError::NonFiniteValue("expression parameter value")
        ));
        assert!(matches!(
            bound_error_to_model_error(BoundError::ArithmeticNan),
            ModelError::NaNValue("expression interval arithmetic")
        ));
        assert!(matches!(
            bound_error_to_model_error(BoundError::UnsupportedFunctionKind),
            ModelError::NonFiniteValue("unsupported function kind")
        ));
    }
}

impl Model {
    /// Create a new empty model.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new model with a name.
    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Self::default()
        }
    }

    /// Create a new named model (P22 target constructor).
    pub fn named(name: impl Into<String>) -> Self {
        Self::with_name(name)
    }

    // ========== Lineage, Instance, and Metadata ==========

    /// The lineage identity of this model (design §4.1, SM-02.1).
    ///
    /// Independent models receive distinct lineages; [`Clone`](Self::clone)
    /// preserves the lineage. Lineage governs assignment reuse compatibility
    /// across clones.
    pub fn lineage(&self) -> ModelLineageId {
        self.lineage
    }

    /// The instance identity of this live model object (design §4.2, SM-02.7).
    ///
    /// Every live model has a distinct instance id; cloning allocates a new
    /// instance while preserving the lineage, so divergent clones with equal
    /// revisions are never confused.
    pub fn instance(&self) -> ModelInstanceId {
        self.instance
    }

    /// Attach metadata to an entity.
    ///
    /// Metadata is canonical but non-solver-affecting: it never advances the
    /// model revision or emits a solver-facing change (EXECUTION.md
    /// "Incremental semantics", design §5).
    ///
    /// Fallible (D10, WR-05): a stale/removed entity id is rejected with the
    /// entity's typed `*NotFound` error — metadata is never stored against a
    /// dead entity. The entity stores are the liveness authority.
    pub fn set_metadata(
        &mut self,
        entity: EntityRef,
        metadata: EntityMetadata,
    ) -> Result<(), ModelError> {
        match entity {
            EntityRef::Variable(var) if !self.variables.contains(var) => {
                return Err(ModelError::VariableNotFound(var));
            }
            EntityRef::Constraint(con) if !self.constraints.contains(con) => {
                return Err(ModelError::ConstraintNotFound(con));
            }
            EntityRef::Objective(obj) if !self.objectives.contains(obj) => {
                return Err(ModelError::ObjectiveNotFound(obj));
            }
            EntityRef::Parameter(param) if !self.parameters.contains(param) => {
                return Err(ModelError::ParameterNotFound(param));
            }
            EntityRef::Construct(construct) if !self.constructs.contains(construct) => {
                return Err(ModelError::ConstructNotFound(construct));
            }
            _ => {}
        }
        self.metadata.insert(entity, metadata);
        Ok(())
    }

    /// Read the metadata attached to an entity, if any.
    pub fn metadata(&self, entity: EntityRef) -> Option<&EntityMetadata> {
        self.metadata.get(&entity)
    }

    /// Remove the metadata attached to an entity, returning it if present.
    pub fn remove_metadata(&mut self, entity: EntityRef) -> Option<EntityMetadata> {
        self.metadata.remove(&entity)
    }

    /// Reconstruct the canonical function-in-set form of a constraint
    /// (design §6, P25 Task 3).
    ///
    /// The coefficient index is the single coefficient authority (SM-01.1):
    /// the linear function is rebuilt deterministically from the constraint's
    /// coefficient cells and the set from its bounds. The ordinary M2
    /// `LinExpr` path stays canonical (SM-01.5); this is the semantic IR
    /// view used by snapshots, deltas, and later compilers.
    ///
    /// F1: the linear function is reconstructed SYMBOLICALLY — its terms carry
    /// `TermCoeff::Expr(ValueExpr)` sourced from the coefficient index's
    /// `value_expr`, so a parameterized coefficient `p*x` keeps its symbolic
    /// form inside the function (design §6) rather than a parallel
    /// `terms`/`dependencies` view. Dependencies are DERIVED from the function
    /// ([`ScalarFunction::parameter_dependencies`]), never stored.
    pub fn constraint_function(&self, con: ConId) -> Result<FunctionConstraint, ModelError> {
        if !self.constraints.contains(con) {
            return Err(ModelError::ConstraintNotFound(con));
        }
        let bounds = self
            .constraint_bounds(con)
            .ok_or(ModelError::ConstraintNotFound(con))?;
        let expr = self.constraint_symbolic_expression(con)?;
        Ok(FunctionConstraint {
            function: ScalarFunction::Linear(expr),
            set: ScalarSet::from(bounds),
        })
    }

    // ========== Construct Operations ==========

    /// Add a canonical semantic construct from a P25 fixture payload
    /// (design §7, P25 Task 4).
    ///
    /// Crate-private (F3): the fixture payload is P25-internal scaffolding.
    /// The public per-construct builder APIs (`add_indicator`, `add_minmax`,
    /// ...) land with the per-construct modules in P30/P32/P33, at which point
    /// [`ConstructKind`]/[`ConstructEntry`] become public exports. The
    /// returned [`Construct`] is stable and generation-safe; removal
    /// invalidates it.
    ///
    /// Fallible (D10): a construct id counter exhaustion returns a typed
    /// error rather than wrapping.
    ///
    /// P25 (F3): exercised only by the in-crate construct lifecycle tests.
    ///
    /// Test-only (A30): the `Fixture` variant and [`FixturePayload`] are
    /// `#[cfg(test)]`-gated, so this method exists only in test builds and is
    /// absent from the public API surface in non-test builds.
    #[cfg(test)]
    pub(crate) fn add_construct_fixture(
        &mut self,
        payload: FixturePayload,
        preference: FormulationPreference,
    ) -> Result<Construct, ModelError> {
        self.add_construct(ConstructKind::Fixture(payload), Some(preference))
    }

    /// Add an indicator construct (design §16.1, packet Task 16).
    ///
    /// `direction` selects the one-way implication: `WhenOne` means
    /// `activator = 1 ⇒ relation`, `WhenZero` means `activator = 0 ⇒ relation`.
    /// The activator must be a binary variable (a continuous or integer
    /// activator is a typed [`ModelError::NonBinaryVariable`]). The optional
    /// per-construct [`FormulationPreference`] narrows the global compilation
    /// policy (A29 single authority — stored on the [`ConstructEntry`]).
    pub fn add_indicator(
        &mut self,
        activator: VarId,
        direction: IndicatorDirection,
        relation: impl Into<FunctionConstraint>,
        preference: Option<FormulationPreference>,
    ) -> Result<Construct, ModelError> {
        self.require_binary(activator)?;
        let fc = relation.into();
        let payload = IndicatorConstraint {
            activator,
            direction,
            function: fc.function,
            set: fc.set,
        };
        self.add_construct(ConstructKind::Indicator(payload), preference)
    }

    /// Add a reification construct (design §16.2, packet Task 16).
    ///
    /// `relation` is reified to a binary `b = 1 ⟺ relation`. Continuous exact
    /// reification requires an explicit `separation` tolerance (D14); when
    /// `separation` is `None` the unit gap is inferred only when the expression
    /// is proven integer-valued (all variables integer/binary with integral
    /// constant coefficients). The optional per-construct
    /// [`FormulationPreference`] narrows the global compilation policy.
    pub fn add_reify(
        &mut self,
        relation: impl Into<FunctionConstraint>,
        separation: Option<f64>,
        preference: Option<FormulationPreference>,
    ) -> Result<Construct, ModelError> {
        if let Some(tol) = separation {
            if !tol.is_finite() || tol <= 0.0 {
                return Err(ModelError::InvalidReificationSeparation(tol));
            }
        }
        let fc = relation.into();
        let proven = self.expression_is_proven_integral(&fc.function);
        if separation.is_none() && !proven {
            return Err(ModelError::ContinuousReificationWithoutSeparation);
        }
        // P32 reification is the two-implication threshold contract (le/ge).
        // Equality/interval reification needs a disjunctive complement — a
        // typed build-time rejection, never a silent relaxation.
        match &fc.set {
            ScalarSet::LessEqual(_) | ScalarSet::GreaterEqual(_) => {}
            ScalarSet::EqualTo(_) | ScalarSet::Interval { .. } => {
                return Err(ModelError::UnsupportedReificationSet);
            }
        }
        // CR-01: the inferred unit gap `f > rhs ⟺ f >= rhs + 1` is exact only
        // when the set threshold is integral (an integer-valued f separated by
        // a non-integral rhs silently excludes the integer just above rhs from
        // both b=0 and b=1). Validate every threshold for integrality at build
        // time; a fractional threshold on a proven-integer expression is a
        // typed rejection (an explicit separation tolerance is the exact path
        // for fractional thresholds, D14).
        if separation.is_none() && proven {
            let eval = |e: &ValueExpr| e.eval(|p| self.parameters.get_value(p).unwrap_or(0.0));
            let integral = |v: f64| v.is_finite() && (v - v.round()).abs() < 1e-9;
            let check = |v: f64| -> Result<(), ModelError> {
                if integral(v) {
                    Ok(())
                } else {
                    Err(ModelError::NonIntegralReificationThreshold(v))
                }
            };
            match &fc.set {
                ScalarSet::LessEqual(upper) => check(eval(upper))?,
                ScalarSet::GreaterEqual(lower) => check(eval(lower))?,
                ScalarSet::EqualTo(_) | ScalarSet::Interval { .. } => unreachable!(
                    "equality/interval reification is rejected above before the integrality check"
                ),
            }
        }
        // The reification result is a fresh binary variable the construct owns
        // (design §16.2: `reify` creates and returns the indicator variable).
        // IN-03 atomicity: reserve the construct id BEFORE creating the
        // activator so a construct-id failure cannot leave an orphaned
        // variable in the arena/changelog.
        let id =
            crate::identity::ConstructId::allocate().map_err(|_| ModelError::IdentityOverflow)?;
        let activator = self.add_variable_internal(Bounds::BINARY, VarType::Binary, None);
        let payload = ReificationConstraint {
            activator,
            function: fc.function,
            set: fc.set,
            separation_tolerance: separation,
            proven_integrality: proven,
        };
        self.add_construct_allocated(id, ConstructKind::Reification(payload), preference)
    }

    /// Add a Boolean construct (design §16.4, packet Task 16).
    ///
    /// `kind` selects implication, equivalence, any (at-least-one), or all
    /// (all-ones) over binary variables. Every referenced variable must be
    /// binary. The optional per-construct [`FormulationPreference`] narrows the
    /// global compilation policy.
    pub fn add_boolean(
        &mut self,
        kind: BooleanKind,
        preference: Option<FormulationPreference>,
    ) -> Result<Construct, ModelError> {
        match &kind {
            BooleanKind::Implication {
                antecedent,
                consequent,
            } => {
                self.require_binary(*antecedent)?;
                self.require_binary(*consequent)?;
            }
            BooleanKind::Equivalence { left, right } => {
                self.require_binary(*left)?;
                self.require_binary(*right)?;
            }
            BooleanKind::Any { variables } | BooleanKind::All { variables } => {
                if variables.is_empty() {
                    return Err(ModelError::EmptyConstructInput);
                }
                for &v in variables {
                    self.require_binary(v)?;
                }
            }
        }
        self.add_construct(
            ConstructKind::Boolean(crate::construct::BooleanConstraint { kind }),
            preference,
        )
    }

    /// Add a cardinality construct (design §16.4, packet Task 16).
    ///
    /// Exactly/at-most/at-least `k` of `variables` are `1`. `k` is validated:
    /// it must be finite, non-negative, integral, and no greater than the input
    /// length (typed [`ModelError::InvalidCardinalityK`] otherwise); the input
    /// list must be non-empty, all-binary, and duplicate-free (typed errors).
    /// The optional per-construct [`FormulationPreference`] narrows the global
    /// compilation policy.
    pub fn add_cardinality(
        &mut self,
        variables: impl IntoIterator<Item = VarId>,
        kind: CardinalityKind,
        k: f64,
        preference: Option<FormulationPreference>,
    ) -> Result<Construct, ModelError> {
        let variables: Vec<VarId> = variables.into_iter().collect();
        if variables.is_empty() {
            return Err(ModelError::EmptyConstructInput);
        }
        let mut seen = HashSet::new();
        for &v in &variables {
            if !seen.insert(v) {
                return Err(ModelError::DuplicateCardinalityVariable(v));
            }
            self.require_binary(v)?;
        }
        if !k.is_finite() {
            return Err(ModelError::InvalidCardinalityK {
                k,
                reason: "k must be finite",
            });
        }
        if k < 0.0 {
            return Err(ModelError::InvalidCardinalityK {
                k,
                reason: "k must be non-negative",
            });
        }
        if (k - k.round()).abs() > 1e-9 {
            return Err(ModelError::InvalidCardinalityK {
                k,
                reason: "k must be an integer",
            });
        }
        let kk = k.round() as usize;
        if kk > variables.len() {
            return Err(ModelError::InvalidCardinalityK {
                k,
                reason: "k exceeds the input length",
            });
        }
        let payload = crate::construct::CardinalityConstraint {
            variables,
            kind,
            k: kk,
        };
        self.add_construct(ConstructKind::Cardinality(payload), preference)
    }

    /// Add a min/max construct (design §16.3, packet Task 17a; SM-12.3, D13).
    ///
    /// `operands` must contain at least two finite linear expressions.
    /// `relation` selects the exact equality or the one-sided epigraph/
    /// hypograph relation — these are distinct semantics and exactness is never
    /// inferred from objective context (D13). The trivially-satisfiable
    /// `Min`+`Epigraph` and `Max`+`Hypograph` combinations are typed rejections.
    /// The builder creates the output variable (the construct's canonical
    /// result) and returns it alongside the stable [`Construct`] handle
    /// (SM-12.8). The optional per-construct [`FormulationPreference`] narrows
    /// the global compilation policy (A29).
    pub fn add_minmax(
        &mut self,
        operands: Vec<LinExpr>,
        sense: MinMaxSense,
        relation: MinMaxRelation,
        preference: Option<FormulationPreference>,
    ) -> Result<(Construct, VarId), ModelError> {
        if operands.len() < 2 {
            return Err(ModelError::MinMaxTooFewOperands);
        }
        // A min epigraph (output >= min) and a max hypograph (output <= max)
        // are trivially satisfiable — reject them (SM-12.3, D13).
        if matches!(sense, MinMaxSense::Min) && relation == MinMaxRelation::Epigraph {
            return Err(ModelError::TriviallySatisfiableMinMax);
        }
        if matches!(sense, MinMaxSense::Max) && relation == MinMaxRelation::Hypograph {
            return Err(ModelError::TriviallySatisfiableMinMax);
        }
        for expr in &operands {
            // Reject non-finite constants/coefficients and stale entities
            // before any mutation (API-06.5).
            self.validate_expression_entities(expr)?;
            // F2: the operand interval is validated (finite endpoints, no NaN)
            // but is NOT encoded as the output variable's declared bounds — the
            // interval is mutable (variable bounds / parameter values change),
            // so a frozen build-time bound would over-restrict a full rebuild.
            self.expression_interval(expr)?;
        }
        // The output variable's declared bounds are a parameter/domain-
        // independent conservative domain: the exact selector / one-sided rows
        // enforce the relation from the CURRENT operand intervals at compile
        // time (F2). An unbounded operand still fails at compile time with
        // `UnboundedBigM` for the exact relation.
        let output_bounds = match relation {
            MinMaxRelation::Exact => Bounds::UNBOUNDED,
            MinMaxRelation::Epigraph => Bounds::UNBOUNDED,
            MinMaxRelation::Hypograph => Bounds::UNBOUNDED,
        };
        // IN-03 atomicity: reserve the construct id BEFORE creating the output
        // variable so a construct-id failure cannot leave an orphaned variable
        // in the arena/changelog.
        let id =
            crate::identity::ConstructId::allocate().map_err(|_| ModelError::IdentityOverflow)?;
        let output = self.add_variable_internal(output_bounds, VarType::Continuous, None);
        let payload = MinMaxConstraint {
            operands,
            output,
            sense,
            relation,
        };
        let construct =
            self.add_construct_allocated(id, ConstructKind::MinMax(payload), preference)?;
        Ok((construct, output))
    }

    /// Add an absolute-value-family construct (design §16.3, packet Task 17b;
    /// SM-12.4).
    ///
    /// `expression` must be bounded (a finite `BoundAnalyzer` interval) — a free
    /// variable or unbounded parameter is a typed [`ModelError`] because the
    /// exact bridge requires finite derived bounds (never an arbitrary Big-M,
    /// D12). `Clamp` requires finite `lower <= upper`. The builder creates the
    /// output variable (the construct's canonical result, preserved as a
    /// top-level construct origin) and returns it alongside the stable
    /// [`Construct`] handle (SM-12.8).
    pub fn add_absolute_value(
        &mut self,
        expression: LinExpr,
        variant: AbsoluteValueVariant,
        preference: Option<FormulationPreference>,
    ) -> Result<(Construct, VarId), ModelError> {
        self.validate_expression_entities(&expression)?;
        let interval = self.expression_interval(&expression)?;
        if !interval.is_bounded() {
            return Err(ModelError::UnboundedConstructExpression);
        }
        // F2: the expression interval is validated (and required bounded) at
        // build time, but the output variable's declared bounds are NOT derived
        // from it — the interval is mutable (variable bounds / parameter values
        // change), so a frozen build-time bound would over-restrict a full
        // rebuild. Only static sign/clamp facts are encoded; the exact bridge
        // rows enforce the relationship from the CURRENT interval at compile
        // time.
        let output_bounds = match variant {
            // z = |x|: the static sign fact is z >= 0; the upper bound is
            // derived by the bridge at compile time.
            AbsoluteValueVariant::Absolute => Bounds::new(0.0, f64::INFINITY),
            // z = max(x, 0): the static sign fact is z >= 0.
            AbsoluteValueVariant::PositivePart => Bounds::new(0.0, f64::INFINITY),
            // Clamp bounds are fixed constants — a parameter-independent static
            // fact that IS retained.
            AbsoluteValueVariant::Clamp { lower, upper } => {
                if !lower.is_finite() || !upper.is_finite() || lower > upper {
                    return Err(ModelError::InvalidClampBounds { lower, upper });
                }
                Bounds::new(lower, upper)
            }
        };
        // IN-03 atomicity: reserve the construct id BEFORE creating the output
        // variable so a construct-id failure cannot leave an orphaned variable
        // in the arena/changelog.
        let id =
            crate::identity::ConstructId::allocate().map_err(|_| ModelError::IdentityOverflow)?;
        let output = self.add_variable_internal(output_bounds, VarType::Continuous, None);
        let payload = AbsoluteValueConstraint {
            expression,
            output,
            variant,
        };
        let construct =
            self.add_construct_allocated(id, ConstructKind::AbsoluteValue(payload), preference)?;
        Ok((construct, output))
    }

    /// Add a binary product construct (design §16.5, packet Task 17c; SM-12.6,
    /// SM-12.7, D23).
    ///
    /// The operand combination must be exactly one of Binary×Binary,
    /// Binary×Linear, or Linear×Binary. A continuous×continuous request is a
    /// typed rejection (SM-12.7) and produces no compiled entities; a non-binary
    /// variable in a `Binary` operand is a typed rejection (SM-12.6). The
    /// builder creates the output variable (the construct's canonical result)
    /// and returns it alongside the stable [`Construct`] handle (SM-12.8).
    pub fn add_binary_product(
        &mut self,
        left: ProductOperand,
        right: ProductOperand,
        preference: Option<FormulationPreference>,
    ) -> Result<(Construct, VarId), ModelError> {
        let (left, right) = self.validate_product_operands(left, right)?;
        // F2: the output variable's declared bounds are a conservative static
        // domain — never the build-time linear-operand interval, which is
        // mutable (variable bounds / parameter values change). The exact
        // product rows enforce the relationship from the CURRENT interval at
        // compile time.
        let output_bounds = match (&left, &right) {
            // w = a·b with a,b binary → w ∈ {0,1} is a static binary fact.
            (ProductOperand::Binary(_), ProductOperand::Binary(_)) => Bounds::new(0.0, 1.0),
            // w = b·f: the reachable set {0} ∪ [L,U] depends on f's CURRENT
            // interval — the bridge derives L/U at compile time.
            (ProductOperand::Binary(_), ProductOperand::Linear(_))
            | (ProductOperand::Linear(_), ProductOperand::Binary(_)) => Bounds::UNBOUNDED,
            _ => unreachable!("builder validates exactly one binary operand"),
        };
        // IN-03 atomicity: reserve the construct id BEFORE creating the output
        // variable so a construct-id failure cannot leave an orphaned variable
        // in the arena/changelog.
        let id =
            crate::identity::ConstructId::allocate().map_err(|_| ModelError::IdentityOverflow)?;
        let output = self.add_variable_internal(output_bounds, VarType::Continuous, None);
        let payload = BinaryProductConstraint {
            left,
            right,
            output,
        };
        let construct =
            self.add_construct_allocated(id, ConstructKind::BinaryProduct(payload), preference)?;
        Ok((construct, output))
    }

    /// Convenience builder: `output = binary * expression` (binary-times-
    /// bounded-linear, design §16.5; SM-12.6).
    ///
    /// Equivalent to [`Self::add_binary_product`] with
    /// `ProductOperand::Binary(binary)` × `ProductOperand::Linear(expression)`.
    pub fn add_binary_times_linear(
        &mut self,
        binary: VarId,
        expression: LinExpr,
        preference: Option<FormulationPreference>,
    ) -> Result<(Construct, VarId), ModelError> {
        self.add_binary_product(
            ProductOperand::Binary(binary),
            ProductOperand::Linear(expression),
            preference,
        )
    }

    /// Validate the two product operands: exactly one binary operand, and any
    /// binary operand must be a true binary variable (SM-12.6).
    fn validate_product_operands(
        &mut self,
        left: ProductOperand,
        right: ProductOperand,
    ) -> Result<(ProductOperand, ProductOperand), ModelError> {
        let left_binary = matches!(left, ProductOperand::Binary(_));
        let right_binary = matches!(right, ProductOperand::Binary(_));
        if !left_binary && !right_binary {
            // Two continuous/linear operands: no exact MILP path exists.
            return Err(ModelError::ContinuousTimesContinuousProduct);
        }
        if let ProductOperand::Binary(var) = &left {
            self.require_binary(*var)?;
        }
        if let ProductOperand::Binary(var) = &right {
            self.require_binary(*var)?;
        }
        if let ProductOperand::Linear(expr) = &left {
            self.validate_expression_entities(expr)?;
        }
        if let ProductOperand::Linear(expr) = &right {
            self.validate_expression_entities(expr)?;
        }
        Ok((left, right))
    }

    /// Compute the deterministic interval of a linear expression over the
    /// model's declared variable bounds and evaluated parameter values, using
    /// the compiler's [`BoundAnalyzer`](crate::compiler::bounds::BoundAnalyzer)
    /// (the single interval semantics — SM-13.1).
    fn expression_interval(
        &self,
        expr: &LinExpr,
    ) -> Result<crate::compiler::bounds::Interval, ModelError> {
        let analyzer = crate::compiler::bounds::BoundAnalyzer::new();
        let variable_bounds = |v: VarId| {
            self.variables
                .get(v)
                .map(|d| d.domain.bounds)
                .unwrap_or(Bounds::UNBOUNDED)
        };
        let parameter_values = |p: ParamId| self.parameters.get_value(p).unwrap_or(0.0);
        analyzer
            .interval_of(
                &crate::function::ScalarFunction::Linear(expr.clone()),
                variable_bounds,
                parameter_values,
            )
            .map(|trace| trace.result)
            .map_err(bound_error_to_model_error)
    }

    /// Shared construct-add: allocate the arena entry, record the
    /// self-contained `Change::ConstructAdded` (A29: payload + preference single
    /// authority), and return the stable generation-safe handle.
    fn add_construct(
        &mut self,
        kind: ConstructKind,
        preference: Option<FormulationPreference>,
    ) -> Result<Construct, ModelError> {
        let preference = preference.unwrap_or(FormulationPreference::Auto);
        let construct = self
            .constructs
            .add(kind.clone(), preference)
            .map_err(|_| ModelError::IdentityOverflow)?;
        // Constructs start active (design §7).
        self.changelog.push(Change::ConstructAdded {
            construct,
            kind,
            preference,
            active: true,
        });
        Ok(construct)
    }

    /// Atomic construct-add with a PRE-ALLOCATED id (IN-03).
    ///
    /// A builder that creates a variable (output/activator) reserves the
    /// construct id FIRST via [`crate::identity::ConstructId::allocate`], then
    /// creates the variable, then calls this helper. Nothing fallible remains
    /// after the id is reserved, so a construct-add failure can never leave an
    /// orphaned variable in the arena/changelog.
    fn add_construct_allocated(
        &mut self,
        id: Construct,
        kind: ConstructKind,
        preference: Option<FormulationPreference>,
    ) -> Result<Construct, ModelError> {
        let preference = preference.unwrap_or(FormulationPreference::Auto);
        self.constructs
            .add_with_id(id, kind.clone(), preference)
            .map_err(|_| ModelError::IdentityOverflow)?;
        // Constructs start active (design §7).
        self.changelog.push(Change::ConstructAdded {
            construct: id,
            kind,
            preference,
            active: true,
        });
        Ok(id)
    }

    /// Require `var` to exist and be a binary variable (SM-12.2).
    fn require_binary(&self, var: VarId) -> Result<(), ModelError> {
        match self.variables.get(var).map(|d| d.domain.var_type) {
            Some(VarType::Binary) => Ok(()),
            Some(_) => Err(ModelError::NonBinaryVariable(var)),
            None => Err(ModelError::VariableNotFound(var)),
        }
    }

    /// Whether `function` is proven integer-valued over its domain (D14).
    ///
    /// All referenced variables must be binary/integer and every coefficient
    /// (including the constant term) must be an integral constant. A
    /// parameterized coefficient is conservatively NOT proven integral.
    fn expression_is_proven_integral(&self, function: &ScalarFunction) -> bool {
        match function {
            ScalarFunction::Linear(expr) => {
                let constant_ok = expr.constant.is_finite()
                    && (expr.constant - expr.constant.round()).abs() < 1e-9;
                constant_ok
                    && expr
                        .terms
                        .iter()
                        .all(|term| match term.coeff.as_constant() {
                            Some(v) => v.is_finite() && (v - v.round()).abs() < 1e-9,
                            None => false,
                        })
                    && expr.terms.iter().all(|term| {
                        matches!(
                            self.variables.get(term.var).map(|d| d.domain.var_type),
                            Some(VarType::Binary) | Some(VarType::Integer)
                        )
                    })
            }
        }
    }

    /// Read a construct entry by id.
    ///
    /// Crate-private (F3): `ConstructEntry` is not part of the public surface
    /// until P32. Returns a typed error for a stale/removed id (D10).
    ///
    /// P25 (F3): exercised only by the in-crate construct lifecycle tests.
    #[allow(dead_code)]
    pub(crate) fn construct(&self, id: Construct) -> Result<&ConstructEntry, ModelError> {
        self.constructs
            .get(id)
            .map(|d| &d.entry)
            .ok_or(ModelError::ConstructNotFound(id))
    }

    /// Set a construct's activity.
    ///
    /// Fallible (D10): a stale/removed construct id is rejected.
    pub fn set_construct_active(&mut self, id: Construct, active: bool) -> Result<(), ModelError> {
        let data = self
            .constructs
            .get_mut(id)
            .ok_or(ModelError::ConstructNotFound(id))?;
        if data.entry.active != active {
            data.entry.active = active;
            self.changelog.push(Change::ConstructActivityChanged {
                construct: id,
                active,
            });
        }
        Ok(())
    }

    /// Remove a construct, invalidating its id (design §7).
    ///
    /// Fallible (D10): a stale/removed construct id is rejected; removing an
    /// already-removed id fails rather than being a no-op.
    pub fn remove_construct(&mut self, id: Construct) -> Result<(), ModelError> {
        if !self.constructs.contains(id) {
            return Err(ModelError::ConstructNotFound(id));
        }
        self.constructs.remove(id);
        // WR-06: cascade construct metadata so the valid attach-metadata-then-
        // remove sequence does not trip `validate_invariants` with an orphaned
        // construct-metadata entry.
        self.metadata.remove(&EntityRef::Construct(id));
        self.changelog
            .push(Change::ConstructRemoved { construct: id });
        Ok(())
    }

    /// The number of live constructs.
    pub fn num_constructs(&self) -> usize {
        self.constructs.len()
    }

    /// Parameter dependencies of a construct, derived from its payload.
    ///
    /// The stored cache is invariant-checked against the payload derivation in
    /// `validate_invariants`.
    pub fn construct_parameter_dependencies(
        &self,
        id: Construct,
    ) -> Result<&[ParamId], ModelError> {
        self.constructs
            .get(id)
            .map(|d| d.parameter_dependencies.as_slice())
            .ok_or(ModelError::ConstructNotFound(id))
    }

    // ========== Variable Operations ==========

    /// Add a variable from a validated [`VariableDef`] (D7).
    ///
    /// Fallible (D10): invalid bounds are rejected before any mutation.
    /// Returns the semantic [`Variable`] handle.
    pub fn add_variable(&mut self, def: VariableDef) -> Result<Variable, ModelError> {
        let (bounds, var_type, name) = def.into_parts();
        if !bounds.is_valid() {
            return Err(ModelError::InvalidBounds);
        }
        if !bounds.lower.is_finite() && bounds.lower != f64::NEG_INFINITY {
            return Err(ModelError::NonFiniteValue("variable lower bound"));
        }
        if !bounds.upper.is_finite() && bounds.upper != f64::INFINITY {
            return Err(ModelError::NonFiniteValue("variable upper bound"));
        }
        if var_type == VarType::Binary && (bounds.lower < 0.0 || bounds.upper > 1.0) {
            return Err(ModelError::InvalidBinaryBounds);
        }
        Ok(self.add_variable_internal(bounds, var_type, name))
    }

    /// Internal infallible variable insertion (arena + changelog).
    fn add_variable_internal(
        &mut self,
        bounds: Bounds,
        var_type: VarType,
        name: Option<String>,
    ) -> VarId {
        let id = match name {
            Some(name) => self.variables.add_named(bounds, var_type, name),
            None => self.variables.add(bounds, var_type),
        };
        self.changelog.push(Change::VariableAdded {
            var: id,
            bounds,
            var_type,
        });
        id
    }

    /// Add a new continuous variable with non-negative bounds.
    ///
    /// Deprecated in P23: replaced by [`Self::add_variable`] with the
    /// [`continuous()`](crate::continuous) definition builder (D7). Kept for
    /// the pre-1.0 compatibility window and remains tested (API-08.3). See
    /// `MIGRATION.md` → "Variable and parameter creation".
    #[deprecated(
        since = "0.1.0",
        note = "use `Model::add_variable(continuous())` (D7); see MIGRATION.md -> Variable and parameter creation"
    )]
    pub fn add_var(&mut self) -> VarId {
        self.add_variable_internal(Bounds::NON_NEGATIVE, VarType::Continuous, None)
    }

    /// Add a new binary variable.
    ///
    /// Deprecated in P23: replaced by [`Self::add_variable`] with the
    /// [`binary()`](crate::binary) definition builder (D7). Kept for the
    /// pre-1.0 compatibility window and remains tested (API-08.3). See
    /// `MIGRATION.md` → "Variable and parameter creation".
    #[deprecated(
        since = "0.1.0",
        note = "use `Model::add_variable(binary())` (D7); see MIGRATION.md -> Variable and parameter creation"
    )]
    pub fn add_binary(&mut self) -> VarId {
        self.add_variable_internal(Bounds::BINARY, VarType::Binary, None)
    }

    /// Add a new integer variable with the given bounds.
    ///
    /// Fallible (D10, API-06.1/06.4): invalid or non-finite bounds are
    /// rejected before any mutation — the compatibility wrapper for
    /// `add_variable(integer().bounds(...))` (see Signature-collision
    /// migration in the P20 disposition).
    ///
    /// Deprecated in P23: replaced by
    /// [`Self::add_variable`](Self::add_variable) with the
    /// [`integer()`](crate::integer) definition builder plus `.bounds(...)`
    /// (D7). Kept for the pre-1.0 compatibility window and remains tested
    /// (API-08.3). See `MIGRATION.md` → "Variable and parameter creation".
    #[deprecated(
        since = "0.1.0",
        note = "use `Model::add_variable(integer().bounds(lower, upper))` (D7); see MIGRATION.md -> Variable and parameter creation"
    )]
    pub fn add_integer(&mut self, bounds: Bounds) -> Result<VarId, ModelError> {
        if !bounds.is_valid() {
            return Err(ModelError::InvalidBounds);
        }
        if !bounds.lower.is_finite() && bounds.lower != f64::NEG_INFINITY {
            return Err(ModelError::NonFiniteValue("variable lower bound"));
        }
        if !bounds.upper.is_finite() && bounds.upper != f64::INFINITY {
            return Err(ModelError::NonFiniteValue("variable upper bound"));
        }
        Ok(self.add_variable_internal(bounds, VarType::Integer, None))
    }

    /// Remove a variable and all its coefficients.
    pub fn remove_variable(&mut self, var: VarId) -> Result<(), ModelError> {
        if !self.variables.contains(var) {
            return Err(ModelError::VariableNotFound(var));
        }

        // Remove all coefficients for this variable
        let coeffs: Vec<_> = self.coefficients.for_var(var).collect();
        for coeff_id in coeffs {
            self.remove_coefficient_internal(coeff_id);
        }

        self.variables.remove(var);
        // WR-05: cascade metadata so add/remove churn leaves no orphaned entry.
        self.metadata.remove(&EntityRef::Variable(var));
        self.changelog.push(Change::VariableRemoved { var });
        Ok(())
    }

    /// Get variable bounds.
    ///
    /// This is the **declared** bound view (SM-05.1). Use
    /// [`Self::effective_bounds`] for the bounds the solver actually applies
    /// (declared ∩ active fixing).
    pub fn variable_bounds(&self, var: VarId) -> Option<Bounds> {
        self.variables.get(var).map(|d| d.domain.bounds)
    }

    /// The declared domain of a variable (design §10, SM-05.1).
    ///
    /// Returns `None` for a stale/removed variable. The declared domain
    /// separates declared bounds/type/semi from the optional persistent
    /// fixing; see [`Self::effective_bounds`] for the solver-facing bounds.
    pub fn variable_domain(&self, var: VarId) -> Option<VariableDomain> {
        self.variables.get(var).map(|d| d.domain)
    }

    /// The declared bounds of a variable (SM-05.1).
    ///
    /// The declared bounds are independent of any persistent fixing — they are
    /// what `unfix` restores. Returns `None` for a stale/removed variable.
    pub fn declared_bounds(&self, var: VarId) -> Option<Bounds> {
        self.variables.get(var).map(|d| d.domain.bounds)
    }

    /// The effective bounds of a variable (SM-05.1).
    ///
    /// The effective bounds are `declared ∩ fixing`: for a fixed variable the
    /// effective bounds equal `[value, value]` (D6: fixing compiles as bound
    /// tightening, SM-05.3). For an unfixed variable the effective bounds
    /// equal the declared bounds. Returns `None` for a stale/removed variable.
    pub fn effective_bounds(&self, var: VarId) -> Option<Bounds> {
        let data = self.variables.get(var)?;
        // WR-02: the solver-facing bounds fold the fixing FIRST (SM-05.3),
        // THEN the activity — an inactive variable's solver-facing bounds are
        // `[0,0]` regardless of its fixing, matching `compile_snapshot`'s
        // fold (the model API, `compile_snapshot`, and `compile_delta` must
        // agree).
        if !data.active {
            return Some(Bounds::new(0.0, 0.0));
        }
        match &data.fixing {
            Some(fixing) => Some(Bounds {
                lower: data.domain.bounds.lower.max(fixing.value),
                upper: data.domain.bounds.upper.min(fixing.value),
            }),
            None => Some(data.domain.bounds),
        }
    }

    /// The named integrality tolerance used by fix validation on integer and
    /// binary variables (SM-05.5).
    pub fn integrality_tolerance(&self) -> f64 {
        self.constants.integrality_tolerance
    }

    /// Set the named integrality tolerance used by fix validation (SM-05.5).
    ///
    /// Fallible (D10): a negative, NaN, or infinite tolerance is rejected
    /// before any state change.
    pub fn set_integrality_tolerance(&mut self, tolerance: f64) -> Result<(), ModelError> {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(ModelError::InvalidIntegralityTolerance(tolerance));
        }
        self.constants.integrality_tolerance = tolerance;
        Ok(())
    }

    /// Fix a variable to a value (SM-05.2, design §10).
    ///
    /// A typed atomic canonical mutation: validates finiteness, in-domain,
    /// and (for integer/binary variables) integrality within the named
    /// integrality tolerance (SM-05.5), then records a
    /// [`VariableFixing`] with [`FixingProvenance::User`]. Fixing is
    /// represented as bound tightening — a fixed variable's effective bounds
    /// become `[value, value]` (D6, SM-05.3) — and emits
    /// [`Change::VariableFixingChanged`] (compiled to
    /// `ModelOp::SetVariableFixing`). `commit` advances the canonical
    /// revision exactly once.
    ///
    /// A failed validation leaves the model fully unchanged (no pending
    /// change, no revision advance).
    pub fn fix(&mut self, var: VarId, value: f64) -> Result<(), ModelError> {
        let data = self
            .variables
            .get(var)
            .ok_or(ModelError::VariableNotFound(var))?;
        if !value.is_finite() {
            return Err(ModelError::NonFiniteValue("variable fixing value"));
        }
        let bounds = data.domain.bounds;
        if value < bounds.lower || value > bounds.upper {
            return Err(ModelError::ValueOutOfBounds {
                variable: var,
                value,
                bounds,
            });
        }
        let tolerance = self.constants.integrality_tolerance;
        if matches!(data.domain.var_type, VarType::Integer | VarType::Binary) {
            let nearest = value.round();
            if (value - nearest).abs() > tolerance {
                return Err(ModelError::NonIntegralValue {
                    variable: var,
                    value,
                    tolerance,
                });
            }
        }

        let fixing = VariableFixing {
            value,
            provenance: FixingProvenance::User,
        };
        self.variables
            .get_mut(var)
            .expect("variable liveness verified above")
            .fixing = Some(fixing.clone());
        self.changelog.push(Change::VariableFixingChanged {
            var,
            fixing: Some(fixing),
            effective_bounds: Bounds::new(value, value),
        });
        Ok(())
    }

    /// Unfix a variable, restoring the **current** declared bounds (SM-05.4,
    /// design §10).
    ///
    /// A typed atomic canonical mutation. The effective bounds after `unfix`
    /// equal the declared bounds at the time of the call, not the bounds at
    /// the time the variable was fixed. Unfixing an already-unfixed variable
    /// records no change.
    pub fn unfix(&mut self, var: VarId) -> Result<(), ModelError> {
        let data = self
            .variables
            .get(var)
            .ok_or(ModelError::VariableNotFound(var))?;
        if data.fixing.is_none() {
            return Ok(());
        }
        let declared = data.domain.bounds;
        self.variables
            .get_mut(var)
            .expect("variable liveness verified above")
            .fixing = None;
        self.changelog.push(Change::VariableFixingChanged {
            var,
            fixing: None,
            effective_bounds: declared,
        });
        Ok(())
    }

    /// Get a variable's name (D6/API-05.5).
    ///
    /// Returns `Ok(Some(name))` for a named variable, `Ok(None)` for a valid
    /// unnamed variable, and a typed stale-ID error if the variable was
    /// removed (D10/API-06.3).
    pub fn variable_name(&self, var: VarId) -> Result<Option<&str>, ModelError> {
        self.variables
            .get(var)
            .map(|d| d.name.as_deref())
            .ok_or(ModelError::VariableNotFound(var))
    }

    /// Set variable bounds.
    ///
    /// Fallible (D10/API-06.1/06.2/06.4): inverted or NaN bounds are rejected
    /// before any mutation, ±inf misuse (a `+inf` lower or `-inf` upper) is
    /// rejected, and a binary variable must stay inside `[0, 1]`.
    ///
    /// Atomicity guard (SM-05.6): the requested bounds are validated against
    /// any **active fixing** — bounds that exclude the fixing value return a
    /// typed [`ModelError::BoundsExcludeFixing`] with no state change (the
    /// fixing value must always lie inside the declared bounds).
    pub fn set_variable_bounds(&mut self, var: VarId, bounds: Bounds) -> Result<(), ModelError> {
        let data = self
            .variables
            .get(var)
            .ok_or(ModelError::VariableNotFound(var))?;
        if !bounds.is_valid() {
            return Err(ModelError::InvalidBounds);
        }
        if !bounds.lower.is_finite() && bounds.lower != f64::NEG_INFINITY {
            return Err(ModelError::NonFiniteValue("variable lower bound"));
        }
        if !bounds.upper.is_finite() && bounds.upper != f64::INFINITY {
            return Err(ModelError::NonFiniteValue("variable upper bound"));
        }
        if data.domain.var_type == VarType::Binary && (bounds.lower < 0.0 || bounds.upper > 1.0) {
            return Err(ModelError::InvalidBinaryBounds);
        }
        // SM-05.6: a declared-bound change that excludes the active fixing
        // value fails atomically — validate before any mutation.
        if let Some(fixing) = &data.fixing {
            if fixing.value < bounds.lower || fixing.value > bounds.upper {
                return Err(ModelError::BoundsExcludeFixing {
                    variable: var,
                    value: fixing.value,
                    bounds,
                });
            }
        }
        let data = self
            .variables
            .get_mut(var)
            .expect("variable liveness verified above");
        let old = data.domain.bounds;
        if old != bounds {
            data.domain.bounds = bounds;
            self.changelog.push(Change::VariableBoundsChanged {
                var,
                old,
                new: bounds,
            });
        }
        Ok(())
    }

    /// Set variable activity.
    pub fn set_variable_active(&mut self, var: VarId, active: bool) -> Result<(), ModelError> {
        let data = self
            .variables
            .get_mut(var)
            .ok_or(ModelError::VariableNotFound(var))?;
        if data.active != active {
            data.active = active;
            self.changelog
                .push(Change::VariableActivityChanged { var, active });
        }
        Ok(())
    }

    /// Change a variable's type (Continuous, Integer, Binary).
    ///
    /// Produces a `Change::VariableTypeChanged` which the solver adapter
    /// applies on the next `sync_model` / `apply_changes` call.
    pub fn set_variable_type(&mut self, var: VarId, var_type: VarType) -> Result<(), ModelError> {
        let data = self
            .variables
            .get_mut(var)
            .ok_or(ModelError::VariableNotFound(var))?;
        let old = data.domain.var_type;
        if old != var_type {
            data.domain.var_type = var_type;
            self.changelog.push(Change::VariableTypeChanged {
                var,
                old,
                new: var_type,
            });
        }
        Ok(())
    }

    /// Convenience: set variable to binary `[0,1]`.
    pub fn set_binary(&mut self, var: VarId) -> Result<(), ModelError> {
        self.set_variable_type(var, VarType::Binary)?;
        self.set_variable_bounds(var, Bounds::new(0.0, 1.0))?;
        Ok(())
    }

    /// Mark a variable as semi-continuous with the given lower bound.
    ///
    /// A semi-continuous variable can take value 0 or any value between
    /// `lower` and its current upper bound. This tightens the LP relaxation
    /// (the variable cannot be fractionally below `lower`) while remaining
    /// feasible for all integer solutions.
    ///
    /// If `lower` exceeds the current lower bound, the lower bound is raised.
    pub fn set_semicontinuous(&mut self, var: VarId, lower: f64) -> Result<(), ModelError> {
        let bounds = self
            .variable_bounds(var)
            .ok_or(ModelError::VariableNotFound(var))?;
        if !lower.is_finite() {
            return Err(ModelError::NonFiniteValue("semi-continuous lower bound"));
        }
        if lower > bounds.upper {
            return Err(ModelError::InvalidBounds);
        }
        if lower > bounds.lower {
            self.set_variable_bounds(var, Bounds::new(lower, bounds.upper))?;
        }
        // Record the declared semi-continuous domain (design §10) alongside
        // the legacy `semicontinuous_lower` map (drives the snapshot and the
        // compile-boundary rejection, P26 behavior unchanged).
        if let Some(data) = self.variables.get_mut(var) {
            let semi = match data.domain.var_type {
                VarType::Integer => SemiDomain::Integer {
                    nonzero_lower: lower,
                },
                VarType::Continuous | VarType::Binary => SemiDomain::Continuous {
                    nonzero_lower: lower,
                },
            };
            data.domain.semi = Some(semi);
        }
        self.semicontinuous_lower.insert(var, lower);
        self.changelog
            .push(Change::SemiContinuousBoundChanged { var, lower });
        Ok(())
    }

    /// Get the number of variables.
    pub fn num_variables(&self) -> usize {
        self.variables.len()
    }

    // ========== Constraint Operations ==========

    /// Add a constraint from a [`crate::expr::ConstraintSpec`] (API-04.1, D1).
    ///
    /// Accepts any type that converts into a [`crate::expr::ConstraintSpec`],
    /// including raw [`ConstraintBounds`] (input-shape compatibility bridge).
    /// Fallible (D10).
    pub fn add_constraint<S>(&mut self, spec: S) -> Result<ConId, ModelError>
    where
        S: Into<crate::expr::ConstraintSpec>,
    {
        let spec = spec.into();
        self.add_constraint_spec_impl(spec)
    }

    /// Core constraint insertion shared by the spec API and the fluent
    /// expression path: arena insert (with optional name) + changelog, then
    /// expression compilation and bounds adjustment.
    /// Validate every entity referenced by a linear expression BEFORE any
    /// mutation (API-06.5): a stale variable or parameter must fail
    /// atomically instead of leaving a dangling row, objective, or changelog
    /// event behind (PR #22 review round 1).
    pub(crate) fn validate_expression_entities(&self, expr: &LinExpr) -> Result<(), ModelError> {
        for term in &expr.terms {
            let value_expr = term.coeff.clone().into_value_expr();
            // Reject non-finite coefficient values before any mutation
            // (API-06.2, deferred item 5). Checking here — before the row is
            // inserted — also catches NaN term coefficients that `simplify`
            // would otherwise silently drop, preserving atomicity (API-06.5).
            if !value_expr.eval(self.parameters.as_lookup()).is_finite() {
                return Err(ModelError::NonFiniteValue("coefficient value"));
            }
            if !self.variables.contains(term.var) {
                return Err(ModelError::VariableNotFound(term.var));
            }
            self.validate_value_expr_parameters(&value_expr)?;
        }
        if !expr.constant.is_finite() {
            return Err(ModelError::NonFiniteValue("expression constant"));
        }
        Ok(())
    }

    /// Validate every parameter referenced by a [`ValueExpr`] BEFORE any
    /// mutation — shared by the raw coefficient mutators and the expression
    /// paths (PR #23 review). A stale parameter must fail with
    /// `ModelError::ParameterNotFound` instead of being stored as a
    /// zero-valued coefficient: `ParameterStore::as_lookup()` returns 0.0
    /// for a missing parameter, so the dependency must be checked explicitly.
    pub(crate) fn validate_value_expr_parameters(
        &self,
        value_expr: &ValueExpr,
    ) -> Result<(), ModelError> {
        for param in value_expr.dependencies() {
            if !self.parameters.contains(param) {
                return Err(ModelError::ParameterNotFound(param));
            }
        }
        Ok(())
    }

    fn add_constraint_spec_impl(
        &mut self,
        spec: crate::expr::ConstraintSpec,
    ) -> Result<ConId, ModelError> {
        let crate::expr::ConstraintSpec { expr, bounds, name } = spec;
        // Atomicity + validation (API-06.5): reject invalid bounds and stale
        // expression entities before inserting the row, so a NaN/inverted
        // bound or a stale variable/parameter cannot leave a dangling
        // constraint or changelog event behind.
        validate_constraint_bounds(bounds)?;
        self.validate_expression_entities(&expr)?;
        let con = self.add_empty_constraint_internal(bounds, name);
        let constant = expr.compile_for_constraint(self, con)?;
        if constant.abs() >= f64::EPSILON {
            let adjusted_bounds = ConstraintBounds {
                lower: bounds.lower - constant,
                upper: bounds.upper - constant,
            };
            self.set_constraint_bounds(con, adjusted_bounds)?;
        }
        Ok(con)
    }

    /// Advanced: insert an empty constraint row with the given bounds.
    ///
    /// This is the raw bounds-only row creation primitive (D11-adjacent low-level
    /// mutation). The canonical path is [`Self::add_constraint`] with a spec.
    /// No coefficients are created; fill the row with the sparse cell APIs.
    pub fn add_empty_constraint(&mut self, bounds: ConstraintBounds) -> ConId {
        self.add_empty_constraint_internal(bounds, None)
    }

    /// Private primitive: insert an empty constraint with the given bounds and
    /// optional name, pushing the changelog event.
    pub(crate) fn add_empty_constraint_internal(
        &mut self,
        bounds: ConstraintBounds,
        name: Option<String>,
    ) -> ConId {
        let id = match name {
            Some(name) => self.constraints.add_named(bounds, name),
            None => self.constraints.add(bounds),
        };
        self.changelog
            .push(Change::ConstraintAdded { con: id, bounds });
        id
    }

    /// Get the bounds of a constraint, if it exists.
    pub fn constraint_bounds(&self, con: ConId) -> Option<ConstraintBounds> {
        self.constraints.get(con).map(|data| data.bounds)
    }

    /// Get a constraint's name (D6/API-05.5).
    ///
    /// Returns `Ok(Some(name))` for a named constraint, `Ok(None)` for a valid
    /// unnamed constraint, and a typed stale-ID error if the constraint was
    /// removed (D10/API-06.3).
    pub fn constraint_name(&self, con: ConId) -> Result<Option<&str>, ModelError> {
        self.constraints
            .get(con)
            .map(|d| d.name.as_deref())
            .ok_or(ModelError::ConstraintNotFound(con))
    }

    /// Remove a constraint and all its coefficients.
    pub fn remove_constraint(&mut self, con: ConId) -> Result<(), ModelError> {
        if !self.constraints.contains(con) {
            return Err(ModelError::ConstraintNotFound(con));
        }

        // Remove all coefficients for this constraint
        let coeffs: Vec<_> = self.coefficients.for_constraint(con).collect();
        for coeff_id in coeffs {
            self.remove_coefficient_internal(coeff_id);
        }

        self.constraints.remove(con);
        // WR-05: cascade metadata so add/remove churn leaves no orphaned entry.
        self.metadata.remove(&EntityRef::Constraint(con));
        self.changelog.push(Change::ConstraintRemoved { con });
        Ok(())
    }

    /// Set constraint bounds.
    ///
    /// Fallible (D10/API-06.1/06.2): NaN bounds are rejected before any
    /// mutation. Infinite sides remain valid (`le`/`ge` forms).
    pub fn set_constraint_bounds(
        &mut self,
        con: ConId,
        bounds: ConstraintBounds,
    ) -> Result<(), ModelError> {
        let data = self
            .constraints
            .get_mut(con)
            .ok_or(ModelError::ConstraintNotFound(con))?;
        validate_constraint_bounds(bounds)?;
        let old = data.bounds;
        if old != bounds {
            data.bounds = bounds;
            self.changelog.push(Change::ConstraintBoundsChanged {
                con,
                old,
                new: bounds,
            });
        }
        Ok(())
    }

    /// Set constraint activity.
    pub fn set_constraint_active(&mut self, con: ConId, active: bool) -> Result<(), ModelError> {
        let data = self
            .constraints
            .get_mut(con)
            .ok_or(ModelError::ConstraintNotFound(con))?;
        let old = data.active;
        if old != active {
            data.active = active;
            self.changelog
                .push(Change::ConstraintActivityChanged { con, active });
        }
        Ok(())
    }

    /// Get the number of constraints.
    pub fn num_constraints(&self) -> usize {
        self.constraints.len()
    }

    // ========== Objective Operations ==========

    /// Add a new objective with the given sense.
    pub fn add_objective(&mut self, sense: Sense) -> ObjId {
        self.add_objective_internal(sense, None)
    }

    /// Advanced: create an inactive objective with an explicit name (D6).
    ///
    /// The returned objective is inactive; activate it with
    /// [`Self::set_active_objective`] and populate it with
    /// [`Self::set_objective_expr`]. The canonical path is
    /// [`Self::minimize`] / [`Self::maximize`].
    pub fn add_objective_named(&mut self, sense: Sense, name: impl Into<String>) -> ObjId {
        self.add_objective_internal(sense, Some(name.into()))
    }

    /// Private primitive: insert an objective (optionally named), pushing the
    /// changelog event. The new objective is inactive by default.
    pub(crate) fn add_objective_internal(&mut self, sense: Sense, name: Option<String>) -> ObjId {
        let id = match name {
            Some(name) => self.objectives.add_named(sense, name),
            None => self.objectives.add(sense),
        };
        self.changelog
            .push(Change::ObjectiveAdded { obj: id, sense });
        id
    }

    /// Remove an objective and all its coefficients.
    pub fn remove_objective(&mut self, obj: ObjId) -> Result<(), ModelError> {
        if !self.objectives.contains(obj) {
            return Err(ModelError::ObjectiveNotFound(obj));
        }

        // Remove all coefficients for this objective
        let coeffs: Vec<_> = self.coefficients.for_objective(obj).collect();
        for coeff_id in coeffs {
            self.remove_coefficient_internal(coeff_id);
        }

        self.objectives.remove(obj);
        // WR-05: cascade metadata so add/remove churn leaves no orphaned entry.
        self.metadata.remove(&EntityRef::Objective(obj));
        self.changelog.push(Change::ObjectiveRemoved { obj });
        Ok(())
    }

    /// Set the active objective.
    pub fn set_active_objective(&mut self, obj: ObjId) -> Result<(), ModelError> {
        if !self.objectives.contains(obj) {
            return Err(ModelError::ObjectiveNotFound(obj));
        }
        let old = self.objectives.active();
        if old != Some(obj) {
            self.objectives.set_active(obj);
            self.changelog.push(Change::ActiveObjectiveChanged {
                old,
                new: Some(obj),
            });
        }
        Ok(())
    }

    /// Clear the active objective.
    pub fn clear_active_objective(&mut self) {
        let old = self.objectives.active();
        if old.is_some() {
            self.objectives.clear_active();
            self.changelog
                .push(Change::ActiveObjectiveChanged { old, new: None });
        }
    }

    /// Get the active objective.
    pub fn active_objective(&self) -> Option<ObjId> {
        self.objectives.active()
    }

    /// Get an objective's optimization sense (P27 Task 9 overlay compiler:
    /// objective-lock degradation rows follow the objective's sense, design
    /// §15.2). Returns `None` for a stale/removed objective.
    pub fn objective_sense(&self, obj: ObjId) -> Option<Sense> {
        self.objectives.get(obj).map(|data| data.sense)
    }

    /// Get the constant offset for an objective.
    pub fn objective_constant(&self, obj: ObjId) -> Option<f64> {
        self.objectives.get(obj).map(|data| data.constant)
    }

    /// Get an objective's name (D6/API-05.5).
    ///
    /// Returns `Ok(Some(name))` for a named objective, `Ok(None)` for a valid
    /// unnamed objective, and a typed stale-ID error if the objective was
    /// removed (D10/API-06.3).
    pub fn objective_name(&self, obj: ObjId) -> Result<Option<&str>, ModelError> {
        self.objectives
            .get(obj)
            .map(|d| d.name.as_deref())
            .ok_or(ModelError::ObjectiveNotFound(obj))
    }

    /// Set an objective's constant offset, journaling the change when it
    /// differs (API-03.5: the delta path propagates constants to backends).
    pub(crate) fn set_objective_constant_internal(&mut self, obj: ObjId, constant: f64) {
        if let Some(data) = self.objectives.get_mut(obj) {
            let old = data.constant;
            if (old - constant).abs() >= f64::EPSILON {
                data.constant = constant;
                self.changelog.push(Change::ObjectiveConstantChanged {
                    obj,
                    old,
                    new: constant,
                });
            }
        }
    }

    /// Get the constant offset for the active objective.
    pub fn active_objective_constant(&self) -> Option<f64> {
        self.active_objective()
            .and_then(|obj| self.objective_constant(obj))
    }

    /// Get the number of objectives.
    pub fn num_objectives(&self) -> usize {
        self.objectives.len()
    }

    // ========== Parameter Operations ==========

    /// Add a parameter from a validated [`ParameterDef`] (D7).
    ///
    /// Plain `f64` values convert through [`From<f64>`] so the current
    /// `add_parameter(value)` call shape keeps compiling. Fallible (D10):
    /// non-finite values are rejected before mutation. Returns the semantic
    /// [`Parameter`] handle.
    pub fn add_parameter<P>(&mut self, def: P) -> Result<Parameter, ModelError>
    where
        P: Into<ParameterDef>,
    {
        let def = def.into();
        if !def.value.is_finite() {
            return Err(ModelError::NonFiniteValue("parameter value"));
        }
        let id = match def.name {
            Some(name) => self.parameters.add_named(def.value, name),
            None => self.parameters.add(def.value),
        };
        Ok(id)
    }

    /// Get a parameter value.
    pub fn parameter_value(&self, param: ParamId) -> Option<f64> {
        self.parameters.get_value(param)
    }

    /// Get a parameter's name (D6/API-05.5).
    ///
    /// Returns `Ok(Some(name))` for a named parameter, `Ok(None)` for a valid
    /// unnamed parameter, and a typed stale-ID error if the parameter was
    /// removed (D10/API-06.3).
    pub fn parameter_name(&self, param: ParamId) -> Result<Option<&str>, ModelError> {
        self.parameters
            .get(param)
            .map(|d| d.name.as_deref())
            .ok_or(ModelError::ParameterNotFound(param))
    }

    /// Queue a parameter change in the current transaction.
    ///
    /// The change is not applied until `commit()` is called.
    ///
    /// Fallible (D10/API-06.3): a stale parameter id or a non-finite value is
    /// rejected before any state change.
    pub fn set_parameter(&mut self, param: Parameter, value: f64) -> Result<(), ModelError> {
        if !self.parameters.contains(param) {
            return Err(ModelError::ParameterNotFound(param));
        }
        if !value.is_finite() {
            return Err(ModelError::NonFiniteValue("parameter value"));
        }
        self.transaction.set_param(param, value);
        Ok(())
    }

    /// Check if there are uncommitted parameter changes.
    pub fn has_uncommitted(&self) -> bool {
        self.transaction.has_pending()
    }

    /// Commit all pending parameter changes.
    ///
    /// This:
    /// 1. Applies all queued parameter value changes
    /// 2. Propagates changes to dependent coefficients
    /// 3. Logs all changes to the changelog
    fn commit_parameters(&mut self) {
        for (param, new_value) in self.transaction.take_pending() {
            self.apply_parameter_change(param, new_value);
        }
    }

    /// Commit all pending changes and produce a revisioned delta batch.
    ///
    /// This:
    /// 1. Applies all queued parameter value changes (same as the old parameter commit)
    /// 2. Compiles all pending changes into ModelOps
    /// 3. Records a DeltaBatch through the coordinator
    /// 4. Returns the new revision
    pub fn commit(&mut self) -> Result<ModelRevision, ModelError> {
        // Step 1: Commit pending parameters (this produces Change entries)
        self.commit_parameters();

        // Step 2: Snapshot changes before draining (atomicity: restore on failure)
        let saved: Vec<Change> = self.changelog.changes().to_vec();
        if saved.is_empty() {
            return Ok(self.coordinator.revision());
        }

        // Step 3: Compute revision before fallible operations
        let from = self.coordinator.revision();
        let to = from.next().ok_or(ModelError::RevisionOverflow)?;

        // Step 4: Compile changes to ModelOps (fallible -- pre-validate)
        let ops: Vec<ModelOp> = saved
            .iter()
            .map(|c| compile_change(c.clone()))
            .collect::<Result<_, _>>()?;

        // Step 5: Create DeltaBatch (safe: from < to since to = from.next() succeeded)
        let batch =
            DeltaBatch::new(from, to, ops).expect("DeltaBatch::new should succeed when from < to");

        // Step 6: Record through coordinator (last fallible step)
        self.coordinator
            .commit_batch(batch)
            .map_err(|_| ModelError::RevisionOverflow)?;

        // Step 7: All fallible operations succeeded -- safely drain
        self.changelog.drain();

        Ok(to)
    }

    /// Apply a single parameter change and propagate to coefficients.
    fn apply_parameter_change(&mut self, param: ParamId, new_value: f64) {
        let old_value = match self.parameters.set_value(param, new_value) {
            Some(v) => v,
            None => return, // Parameter doesn't exist
        };

        if (old_value - new_value).abs() < f64::EPSILON {
            return; // No change
        }

        // Log the parameter change
        self.changelog.push(Change::ParameterValueChanged {
            param,
            old: old_value,
            new: new_value,
        });

        // Propagate to dependent coefficients
        let affected: Vec<_> = self.coefficients.for_param(param).collect();
        let lookup = self.parameters.as_lookup();

        for coeff_id in affected {
            if let Some(data) = self.coefficients.get_mut(coeff_id) {
                let old_cached = data.cached_value;
                let new_cached = data.value_expr.eval(&lookup);

                if (old_cached - new_cached).abs() >= f64::EPSILON {
                    data.cached_value = new_cached;
                    self.changelog.push(Change::CoefficientValueChanged {
                        coeff: coeff_id,
                        var: data.var,
                        target: data.target,
                        value_expr: data.value_expr.clone(),
                        old: old_cached,
                        new: new_cached,
                    });
                }
            }
        }
    }

    /// Rollback uncommitted parameter changes.
    pub fn rollback(&mut self) {
        self.transaction.rollback();
    }

    /// Get the number of parameters.
    pub fn num_parameters(&self) -> usize {
        self.parameters.len()
    }

    // ========== Coefficient Operations ==========

    /// Add a coefficient to a constraint.
    ///
    /// If a coefficient already exists for this (constraint, variable) pair,
    /// the expressions are algebraically combined and the existing coefficient
    /// is updated in place. The returned `CoeffId` will be the existing ID.
    pub fn add_constraint_coefficient<E>(
        &mut self,
        con: ConId,
        var: VarId,
        value_expr: E,
    ) -> Result<CoeffId, ModelError>
    where
        E: Into<ValueExpr>,
    {
        if !self.constraints.contains(con) {
            return Err(ModelError::ConstraintNotFound(con));
        }
        if !self.variables.contains(var) {
            return Err(ModelError::VariableNotFound(var));
        }

        let value_expr = value_expr.into();
        let target = CoefficientTarget::Constraint(con);
        // Reject stale parameter dependencies before any mutation (PR #23
        // review): `as_lookup` returns 0.0 for a missing parameter, so the
        // dependency must be checked explicitly.
        self.validate_value_expr_parameters(&value_expr)?;
        let initial_value = value_expr.eval(self.parameters.as_lookup());

        // Reject non-finite coefficient values before any mutation
        // (API-06.2, deferred item 5).
        if !initial_value.is_finite() {
            return Err(ModelError::NonFiniteValue("coefficient value"));
        }

        // Check if this cell already exists (for correct changelog event)
        let existing = self.coefficients.for_cell(target, var);
        let old_value = existing
            .and_then(|id| self.coefficients.get(id))
            .map(|d| d.cached_value);

        let id = self
            .coefficients
            .add(var, target, value_expr, initial_value);

        if let Some(old) = old_value {
            // Combined with existing cell — emit value change
            let new_value = self
                .coefficients
                .get(id)
                .map(|d| d.cached_value)
                .unwrap_or(initial_value);
            if (old - new_value).abs() >= f64::EPSILON {
                let value_expr = self
                    .coefficients
                    .get(id)
                    .map(|d| d.value_expr.clone())
                    .unwrap_or_else(|| ValueExpr::constant(new_value));
                self.changelog.push(Change::CoefficientValueChanged {
                    coeff: id,
                    var,
                    target,
                    value_expr,
                    old,
                    new: new_value,
                });
            }
        } else {
            // New cell
            let value_expr = self
                .coefficients
                .get(id)
                .map(|d| d.value_expr.clone())
                .unwrap_or_else(|| ValueExpr::constant(initial_value));
            self.changelog.push(Change::CoefficientAdded {
                coeff: id,
                var,
                target,
                value_expr,
                value: initial_value,
            });
        }

        Ok(id)
    }

    /// Add a coefficient to an objective.
    ///
    /// If a coefficient already exists for this (objective, variable) pair,
    /// the expressions are algebraically combined and the existing coefficient
    /// is updated in place.
    pub fn add_objective_coefficient<E>(
        &mut self,
        obj: ObjId,
        var: VarId,
        value_expr: E,
    ) -> Result<CoeffId, ModelError>
    where
        E: Into<ValueExpr>,
    {
        if !self.objectives.contains(obj) {
            return Err(ModelError::ObjectiveNotFound(obj));
        }
        if !self.variables.contains(var) {
            return Err(ModelError::VariableNotFound(var));
        }

        let value_expr = value_expr.into();
        let target = CoefficientTarget::Objective(obj);
        // Reject stale parameter dependencies before any mutation (PR #23
        // review): `as_lookup` returns 0.0 for a missing parameter, so the
        // dependency must be checked explicitly.
        self.validate_value_expr_parameters(&value_expr)?;
        let initial_value = value_expr.eval(self.parameters.as_lookup());

        // Reject non-finite coefficient values before any mutation
        // (API-06.2, deferred item 5).
        if !initial_value.is_finite() {
            return Err(ModelError::NonFiniteValue("coefficient value"));
        }

        // Check if this cell already exists
        let existing = self.coefficients.for_cell(target, var);
        let old_value = existing
            .and_then(|id| self.coefficients.get(id))
            .map(|d| d.cached_value);

        let id = self
            .coefficients
            .add(var, target, value_expr, initial_value);

        // Look up the canonical expression from the combined cell.
        let combined_expr = self
            .coefficients
            .get(id)
            .map(|d| d.value_expr.clone())
            .unwrap_or_else(|| ValueExpr::constant(initial_value));
        let combined_value = self
            .coefficients
            .get(id)
            .map(|d| d.cached_value)
            .unwrap_or(initial_value);

        if let Some(old) = old_value {
            if (old - combined_value).abs() >= f64::EPSILON {
                self.changelog.push(Change::CoefficientValueChanged {
                    coeff: id,
                    var,
                    target,
                    value_expr: combined_expr,
                    old,
                    new: combined_value,
                });
            }
        } else {
            self.changelog.push(Change::CoefficientAdded {
                coeff: id,
                var,
                target,
                value_expr: combined_expr,
                value: initial_value,
            });
        }

        Ok(id)
    }

    /// Add a constant coefficient to a constraint.
    pub fn add_coeff(&mut self, con: ConId, var: VarId, value: f64) -> Result<CoeffId, ModelError> {
        self.add_constraint_coefficient(con, var, ValueExpr::constant(value))
    }

    /// Add a constant coefficient to an objective.
    pub fn add_objective_coeff(
        &mut self,
        obj: ObjId,
        var: VarId,
        value: f64,
    ) -> Result<CoeffId, ModelError> {
        self.add_objective_coefficient(obj, var, value)
    }

    /// Advanced: set the coefficient cell at `(target, variable)` by
    /// coordinate, replacing any existing canonical cell with `value` (D11).
    ///
    /// The scalar `value` becomes the cell's constant expression; any parameter
    /// dependency of a prior expression at this cell is dropped. The
    /// canonical-cell invariant (one cell per `(target, variable)`) is
    /// preserved. Raw [`CoeffId`] operations remain in the advanced surface.
    ///
    /// Fallible (D10): stale entities and non-finite values are rejected before
    /// any mutation.
    pub fn set_coefficient(
        &mut self,
        target: CoefficientTarget,
        var: VarId,
        value: f64,
    ) -> Result<(), ModelError> {
        if !value.is_finite() {
            return Err(ModelError::NonFiniteValue("coefficient value"));
        }
        match target {
            CoefficientTarget::Constraint(con) => {
                if !self.constraints.contains(con) {
                    return Err(ModelError::ConstraintNotFound(con));
                }
            }
            CoefficientTarget::Objective(obj) => {
                if !self.objectives.contains(obj) {
                    return Err(ModelError::ObjectiveNotFound(obj));
                }
            }
        }
        if !self.variables.contains(var) {
            return Err(ModelError::VariableNotFound(var));
        }

        let value_expr = ValueExpr::constant(value);
        if let Some(existing_id) = self.coefficients.for_cell(target, var) {
            // Replacement compares EXPRESSION semantics, not cached evaluated
            // values (PR #22 review round 1): only a prior CONSTANT
            // expression equal to the requested value is a semantic no-op. A
            // parameter-dependent expression must be replaced even when its
            // current evaluated value coincides with the requested constant —
            // otherwise the dependency survives and a later parameter update
            // silently changes the supposedly replaced coefficient.
            let existing = self.coefficients.get(existing_id);
            let old = existing.map(|d| d.cached_value).unwrap_or(value);
            if let Some(ValueExpr::Constant(c)) = existing.map(|d| &d.value_expr) {
                if (c - value).abs() < f64::EPSILON {
                    return Ok(());
                }
            }
            self.coefficients
                .set_expr(existing_id, value_expr.clone(), value);
            self.changelog.push(Change::CoefficientValueChanged {
                coeff: existing_id,
                var,
                target,
                value_expr,
                old,
                new: value,
            });
        } else {
            let id = self
                .coefficients
                .add(var, target, value_expr.clone(), value);
            self.changelog.push(Change::CoefficientAdded {
                coeff: id,
                var,
                target,
                value_expr,
                value,
            });
        }
        Ok(())
    }

    /// Advanced: algebraically add `value` to the canonical coefficient cell at
    /// `(target, variable)`, creating it when absent (D11).
    ///
    /// Repeated additions keep one canonical cell whose value is the running
    /// sum. Fallible (D10): stale entities and non-finite values are rejected
    /// before any mutation.
    pub fn add_to_coefficient(
        &mut self,
        target: CoefficientTarget,
        var: VarId,
        value: f64,
    ) -> Result<(), ModelError> {
        if !value.is_finite() {
            return Err(ModelError::NonFiniteValue("coefficient value"));
        }
        match target {
            CoefficientTarget::Constraint(con) => {
                self.add_constraint_coefficient(con, var, ValueExpr::constant(value))?;
            }
            CoefficientTarget::Objective(obj) => {
                self.add_objective_coefficient(obj, var, ValueExpr::constant(value))?;
            }
        }
        Ok(())
    }

    /// Advanced: remove the canonical coefficient cell at `(target, variable)`
    /// by coordinate (D11).
    ///
    /// Removing a coordinate with no cell is a no-op. Fallible (D10): stale
    /// target entities and variables are rejected.
    pub fn remove_coefficient_at(
        &mut self,
        target: CoefficientTarget,
        var: VarId,
    ) -> Result<(), ModelError> {
        match target {
            CoefficientTarget::Constraint(con) => {
                if !self.constraints.contains(con) {
                    return Err(ModelError::ConstraintNotFound(con));
                }
            }
            CoefficientTarget::Objective(obj) => {
                if !self.objectives.contains(obj) {
                    return Err(ModelError::ObjectiveNotFound(obj));
                }
            }
        }
        if !self.variables.contains(var) {
            return Err(ModelError::VariableNotFound(var));
        }
        if let Some(existing_id) = self.coefficients.for_cell(target, var) {
            self.remove_coefficient_internal(existing_id);
        }
        Ok(())
    }

    /// Remove a coefficient.
    pub fn remove_coefficient(&mut self, coeff: CoeffId) -> Result<(), ModelError> {
        if !self.coefficients.contains(coeff) {
            return Err(ModelError::CoefficientNotFound(coeff));
        }
        self.remove_coefficient_internal(coeff);
        Ok(())
    }

    /// Internal coefficient removal (no validation).
    fn remove_coefficient_internal(&mut self, coeff: CoeffId) {
        if let Some(data) = self.coefficients.remove(coeff) {
            self.changelog.push(Change::CoefficientRemoved {
                coeff,
                var: data.var,
                target: data.target,
            });
        }
    }

    /// Get coefficient data.
    pub fn coefficient(&self, coeff: CoeffId) -> Option<&CoefficientData> {
        self.coefficients.get(coeff)
    }

    /// Get the number of coefficients.
    pub fn num_coefficients(&self) -> usize {
        self.coefficients.len()
    }

    // ========== Changelog Operations ==========

    /// Check if there are pending changes for the solver.
    pub fn has_pending_changes(&self) -> bool {
        !self.changelog.is_empty()
    }

    /// Drain all pending changes.
    ///
    /// If there are uncommitted parameter changes, this will:
    /// 1. Log a warning
    /// 2. Auto-commit the changes
    #[deprecated(
        since = "0.1.0",
        note = "use commit() which returns DeltaBatch through BackendSession"
    )]
    pub fn drain_changes(&mut self) -> Vec<Change> {
        if self.has_uncommitted() {
            warn!("Uncommitted parameter changes detected, auto-committing");
            self.commit_parameters();
        }
        #[allow(deprecated)]
        self.changelog.drain()
    }

    /// Get the changelog sequence number.
    pub fn changelog_sequence(&self) -> u64 {
        self.changelog.sequence()
    }

    /// Get the current model revision.
    pub fn current_revision(&self) -> ModelRevision {
        self.coordinator.revision()
    }

    /// Take a snapshot of the complete model state at the current revision.
    ///
    /// The snapshot captures all variables, constraints, objectives, parameters,
    /// and coefficient cells in a deterministic, revisioned record.
    pub fn take_snapshot(&self) -> Result<ModelSnapshot, ModelError> {
        use std::collections::HashMap;

        let mut variables = HashMap::new();
        for (id, data) in self.variables.iter() {
            let sc_lower = self.semicontinuous_lower.get(&id).copied();
            variables.insert(
                id,
                (
                    data.domain.bounds,
                    data.domain.var_type,
                    data.active,
                    sc_lower,
                    data.fixing.clone(),
                ),
            );
        }

        let mut constraints = HashMap::new();
        for (id, data) in self.constraints.iter() {
            constraints.insert(id, (data.bounds, data.active));
        }

        let mut objectives = HashMap::new();
        for (id, data) in self.objectives.iter() {
            objectives.insert(id, (data.sense, data.active, data.constant));
        }

        let mut parameters = HashMap::new();
        for (id, data) in self.parameters.iter() {
            parameters.insert(id, data.value);
        }

        let cells: Vec<(
            crate::model::coefficient::CellKey,
            ValueExpr,
            f64,
            Vec<ParamId>,
        )> = self
            .coefficients
            .iter()
            .map(|(_, data)| {
                let cell_key = (data.target, data.var);
                let deps: Vec<ParamId> = data.value_expr.dependencies().into_iter().collect();
                (cell_key, data.value_expr.clone(), data.cached_value, deps)
            })
            .collect();

        let revision = self.coordinator.revision();
        let mut snapshot = crate::snapshot::take_snapshot(
            revision,
            &variables,
            &constraints,
            &objectives,
            &parameters,
            &cells,
        );

        // Invariant (SM-01.1, P25 Task 3): the snapshot's reconstructed
        // semantic function/set entries must agree with the model's canonical
        // reconstruction from the coefficient index. Every transitional
        // legacy field is guarded by this check.
        for entry in &snapshot.functions {
            if let Ok(fc) = self.constraint_function(entry.constraint) {
                debug_assert_eq!(
                    entry.set, fc.set,
                    "snapshot semantic set diverges from the coefficient index"
                );
                // WR-01/WR-02: the reconstructed function expression must also
                // agree with the canonical reconstruction. Both sides are now
                // var-ordered deterministic rebuilds of the same coefficient
                // index, so divergence here would indicate a real
                // second-coefficient-authority bug (e.g. CR-01's pre-adjusted
                // bounds), not a benign ordering difference.
                debug_assert_eq!(
                    entry.function, fc.function,
                    "snapshot semantic function diverges from the coefficient index"
                );
            }
        }

        // Canonical semantic construct entries from the arena (design §7,
        // P25 Task 4). Sorted by id for deterministic output.
        let mut constructs: Vec<ConstructEntry> = self
            .constructs
            .iter()
            .map(|(_, data)| data.entry.clone())
            .collect();
        constructs.sort_by_key(|c| c.id);
        snapshot.constructs = constructs;

        Ok(snapshot)
    }

    /// Validate all model invariants. Returns a list of violations.
    ///
    /// This is a debug/test helper. It checks:
    /// - Every coefficient cell references live variables, constraints, objectives
    /// - Every reverse index (by_var, by_constraint, by_objective, by_param) is
    ///   consistent with forward storage
    /// - No more than one objective is active
    /// - Cached coefficient values match fresh expression evaluation
    /// - No duplicate cell keys
    ///
    /// Returns `Ok(())` if all invariants hold, or `Err(violations)` with
    /// human-readable descriptions of each violated invariant.
    pub fn validate_invariants(&self) -> Result<(), Vec<String>> {
        let mut violations: Vec<String> = Vec::new();
        let lookup = self.parameters.as_lookup();

        // 1. At most one active objective
        let active_count = self.objectives.active_count();
        if active_count > 1 {
            violations.push(format!("multiple active objectives: {active_count}"));
        }

        // 2. Every coefficient references live entities
        for (coeff_id, data) in self.coefficients.iter() {
            if !self.variables.contains(data.var) {
                violations.push(format!(
                    "coefficient {coeff_id:?} references dead variable {:?}",
                    data.var
                ));
            }
            match data.target {
                CoefficientTarget::Constraint(con) => {
                    if !self.constraints.contains(con) {
                        violations.push(format!(
                            "coefficient {coeff_id:?} references dead constraint {con:?}"
                        ));
                    }
                }
                CoefficientTarget::Objective(obj) => {
                    if !self.objectives.contains(obj) {
                        violations.push(format!(
                            "coefficient {coeff_id:?} references dead objective {obj:?}"
                        ));
                    }
                }
            }

            // 3. Cached value matches fresh evaluation
            let fresh = data.value_expr.eval(&lookup);
            if (data.cached_value - fresh).abs() > self.constants.feasibility_tolerance {
                violations.push(format!(
                    "coefficient {coeff_id:?} cached value {} != fresh eval {fresh}",
                    data.cached_value
                ));
            }
        }

        // 4. by_var index consistency
        for (var_id, coeff_set) in self.coefficients.by_var_iter() {
            if !self.variables.contains(*var_id) {
                violations.push(format!("by_var index references dead variable {var_id:?}"));
            }
            for &coeff_id in coeff_set {
                if !self.coefficients.contains(coeff_id) {
                    violations.push(format!(
                        "by_var index has dead coefficient {coeff_id:?} for var {var_id:?}"
                    ));
                }
            }
        }

        // 5. by_constraint index consistency
        for (con_id, coeff_set) in self.coefficients.by_constraint_iter() {
            if !self.constraints.contains(*con_id) {
                violations.push(format!(
                    "by_constraint index references dead constraint {con_id:?}"
                ));
            }
            for &coeff_id in coeff_set {
                if !self.coefficients.contains(coeff_id) {
                    violations.push(format!(
                        "by_constraint index has dead coefficient {coeff_id:?}"
                    ));
                }
            }
        }

        // 6. by_objective index consistency
        for (obj_id, coeff_set) in self.coefficients.by_objective_iter() {
            if !self.objectives.contains(*obj_id) {
                violations.push(format!(
                    "by_objective index references dead objective {obj_id:?}"
                ));
            }
            for &coeff_id in coeff_set {
                if !self.coefficients.contains(coeff_id) {
                    violations.push(format!(
                        "by_objective index has dead coefficient {coeff_id:?}"
                    ));
                }
            }
        }

        // 7. by_param index consistency
        for (param_id, coeff_set) in self.coefficients.by_param_iter() {
            if !self.parameters.contains(*param_id) {
                violations.push(format!(
                    "by_param index references dead parameter {param_id:?}"
                ));
            }
            for &coeff_id in coeff_set {
                if !self.coefficients.contains(coeff_id) {
                    violations.push(format!("by_param index has dead coefficient {coeff_id:?}"));
                }
            }
        }

        // 8. Function-in-set consistency (P25 Task 3, SM-01.1): the
        // transitional legacy constraint bounds must convert to the same
        // ScalarSet as the canonical function-in-set view reconstructed from
        // the coefficient index — there is no second coefficient authority.
        for (con, data) in self.constraints.iter() {
            let legacy_set = ScalarSet::from(data.bounds);
            if let Ok(fc) = self.constraint_function(con) {
                if legacy_set != fc.set {
                    violations.push(format!(
                        "constraint {con:?} legacy bounds diverge from semantic set (bounds {data:?}, function {fc:?})"
                    ));
                }
            }
        }

        // 9. Construct store integrity (P25 Task 4, design §7):
        //    - every construct metadata entry references a live construct;
        //    - the cached parameter-dependency list equals a re-derivation
        //      from the payload (invariant proving cache correctness).
        for entity in self.metadata.keys() {
            if let EntityRef::Construct(construct) = entity {
                if !self.constructs.contains(*construct) {
                    violations.push(format!(
                        "construct metadata references dead construct {construct:?}"
                    ));
                }
            }
        }
        for (id, data) in self.constructs.iter() {
            let derived = derive_parameter_dependencies(&data.entry.kind);
            if derived != data.parameter_dependencies {
                violations.push(format!(
                    "construct {id:?} parameter-dependency cache diverges from payload derivation"
                ));
            }
        }

        // 10. Fixing invariant (SM-05.5, P27 Task 8): every live variable with
        // an active fixing satisfies `declared.lower <= fixing.value <=
        // declared.upper`. `fix` and `set_variable_bounds` enforce this at
        // mutation time; this re-checks the stored state.
        for (var, data) in self.variables.iter() {
            if !data.active {
                continue;
            }
            let bounds = data.domain.bounds;
            if !self::validation::fixing_within_declared(data.fixing.as_ref(), bounds) {
                let value = data.fixing.as_ref().map(|f| f.value).unwrap_or(f64::NAN);
                violations.push(format!(
                    "variable {var:?} fixing value {value} lies outside declared bounds {bounds:?}"
                ));
            }
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }
}

// ── Introspection helpers ────────────────────────────────────────────────────

fn format_bound(v: f64) -> String {
    if v == f64::NEG_INFINITY {
        "-inf".to_string()
    } else if v == f64::INFINITY {
        "+inf".to_string()
    } else {
        format!("{v}")
    }
}

/// Render a variable's diagnostic label: its name when present, else a stable
/// `x[N]` debug handle (D6: names are diagnostics with an index fallback).
fn var_label(model: &Model, var: VarId) -> String {
    model
        .variables
        .get(var)
        .and_then(|d| d.name.clone())
        .unwrap_or_else(|| format!("x[{}]", var.index()))
}

/// Render an entity header label, e.g. `c[0] "capacity"` (named) or `c[0]`.
fn entity_label(prefix: &str, index: u32, name: &Option<String>) -> String {
    match name {
        Some(name) => format!("{prefix}[{index}] \"{name}\""),
        None => format!("{prefix}[{index}]"),
    }
}

fn format_lin_expr(model: &Model, expr: &LinExpr) -> String {
    let terms = expr.terms();
    let constant = expr.get_constant();

    if terms.is_empty() && constant == 0.0 {
        return "0".to_string();
    }

    let mut out = String::new();
    for (i, term) in terms.iter().enumerate() {
        let coeff = match &term.coeff {
            TermCoeff::Constant(v) => *v,
            TermCoeff::Expr(e) => e.as_constant().unwrap_or(f64::NAN),
        };
        let abs_coeff = coeff.abs();
        let negative = coeff < 0.0;
        let label = var_label(model, term.var);

        if i == 0 {
            if (coeff - 1.0).abs() < f64::EPSILON {
                out.push_str(&label);
            } else if (coeff + 1.0).abs() < f64::EPSILON {
                out.push_str(&format!("-{label}"));
            } else {
                out.push_str(&format!("{coeff}*{label}"));
            }
        } else if negative {
            out.push_str(" - ");
            if (abs_coeff - 1.0).abs() < f64::EPSILON {
                out.push_str(&label);
            } else {
                out.push_str(&format!("{abs_coeff}*{label}"));
            }
        } else {
            out.push_str(" + ");
            if (abs_coeff - 1.0).abs() < f64::EPSILON {
                out.push_str(&label);
            } else {
                out.push_str(&format!("{abs_coeff}*{label}"));
            }
        }
    }

    if constant.abs() > f64::EPSILON {
        if out.is_empty() {
            out.push_str(&format!("{constant}"));
        } else if constant < 0.0 {
            out.push_str(&format!(" - {}", constant.abs()));
        } else {
            out.push_str(&format!(" + {constant}"));
        }
    }

    if out.is_empty() {
        "0".to_string()
    } else {
        out
    }
}

// ── Model introspection methods ─────────────────────────────────────────────

impl Model {
    /// Return a human-readable string representation of the model.
    ///
    /// Output format is deterministic (sorted by internal index) and suitable
    /// for debugging and diffing. Similar to Pyomo's `.pprint()`.
    pub fn pprint(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();

        let name = self.name.as_deref().unwrap_or("unnamed");
        writeln!(out, "Model: {name}").unwrap();

        // Variables
        writeln!(out, "  Variables ({}):", self.variables.len()).unwrap();
        let mut vars: Vec<_> = self.variables.iter().collect();
        vars.sort_by_key(|(id, _)| id.index());
        for (id, data) in &vars {
            let lb = format_bound(data.domain.bounds.lower);
            let ub = format_bound(data.domain.bounds.upper);
            let type_s = match data.domain.var_type {
                VarType::Continuous => "Continuous",
                VarType::Integer => "Integer",
                VarType::Binary => "Binary",
            };
            let inactive = if !data.active { " [inactive]" } else { "" };
            let fixing_s = match &data.fixing {
                Some(fixing) => format!(" fixed={}", fixing.value),
                None => String::new(),
            };
            let label = entity_label("x", id.index(), &data.name);
            writeln!(
                out,
                "    {label}: [{lb}, {ub}] {type_s}{fixing_s}{inactive}"
            )
            .unwrap();
        }

        // Parameters
        writeln!(out, "  Parameters ({}):", self.parameters.len()).unwrap();
        let mut params: Vec<_> = self.parameters.iter().collect();
        params.sort_by_key(|(id, _)| id.index());
        for (id, data) in &params {
            let label = entity_label("p", id.index(), &data.name);
            writeln!(out, "    {label}: {}", data.value).unwrap();
        }

        // Constraints
        writeln!(out, "  Constraints ({}):", self.constraints.len()).unwrap();
        let mut cons: Vec<_> = self.constraints.iter().collect();
        cons.sort_by_key(|(id, _)| id.index());
        for (id, data) in &cons {
            let lb = format_bound(data.bounds.lower);
            let ub = format_bound(data.bounds.upper);
            let inactive = if !data.active { " [inactive]" } else { "" };
            let expr_s = self
                .constraint_expression(*id)
                .map(|e| format_lin_expr(self, &e))
                .unwrap_or_else(|_| "?".to_string());
            let label = entity_label("c", id.index(), &data.name);
            writeln!(out, "    {label}: {lb} <= {expr_s} <= {ub}{inactive}").unwrap();
        }

        // Objectives
        writeln!(out, "  Objectives ({}):", self.objectives.len()).unwrap();
        let mut objs: Vec<_> = self.objectives.iter().collect();
        objs.sort_by_key(|(id, _)| id.index());
        for (id, data) in &objs {
            let sense = match data.sense {
                Sense::Minimize => "Minimize",
                Sense::Maximize => "Maximize",
            };
            let active = if data.active { " [active]" } else { "" };
            let expr_s = self
                .objective_expression(*id)
                .map(|e| format_lin_expr(self, &e))
                .unwrap_or_else(|_| "?".to_string());
            let label = entity_label("obj", id.index(), &data.name);
            writeln!(out, "    {label}: {sense} {expr_s}{active}").unwrap();
        }

        out
    }

    /// Compute slack values for a constraint given a solution.
    ///
    /// Returns `(lower_slack, upper_slack)` where:
    /// - `lower_slack = lhs - lower_bound` (positive → lower bound is satisfied)
    /// - `upper_slack = upper_bound - lhs` (positive → upper bound is satisfied)
    pub fn constraint_slack(
        &self,
        con: ConId,
        solution: &Solution,
    ) -> Result<(f64, f64), ModelError> {
        let bounds = self
            .constraints
            .get(con)
            .ok_or(ModelError::ConstraintNotFound(con))?
            .bounds;
        let expr = self.constraint_expression(con)?;
        let lhs = expr.evaluate(solution.as_var_lookup(), self.parameters.as_lookup());
        Ok((lhs - bounds.lower, bounds.upper - lhs))
    }

    /// Iterate over active constraints that are violated by the given solution.
    ///
    /// Yields `(con, lower_slack, upper_slack)` where either slack is negative
    /// (more than a small tolerance).
    pub fn violated_constraints<'a>(
        &'a self,
        solution: &'a Solution,
    ) -> impl Iterator<Item = (ConId, f64, f64)> + 'a {
        self.constraints.iter_active().filter_map(move |(con, _)| {
            let (lower_slack, upper_slack) = self.constraint_slack(con, solution).ok()?;
            if lower_slack < -self.constants.feasibility_tolerance
                || upper_slack < -self.constants.feasibility_tolerance
            {
                Some((con, lower_slack, upper_slack))
            } else {
                None
            }
        })
    }

    /// Iterate over active variables whose solution values violate their bounds.
    ///
    /// Yields `(var, violation)` where `violation` is the distance outside the
    /// feasible region (always positive).
    pub fn bound_violations<'a>(
        &'a self,
        solution: &'a Solution,
    ) -> impl Iterator<Item = (VarId, f64)> + 'a {
        self.variables.iter_active().filter_map(move |(var, data)| {
            let val = solution.value_or_zero(var);
            // The effective (solver-facing) bounds govern feasibility: for a
            // fixed variable this is the equal bound [value, value].
            let bounds = data
                .fixing
                .as_ref()
                .map(|f| Bounds::new(f.value, f.value))
                .unwrap_or(data.domain.bounds);
            let lower_viol = bounds.lower - val; // positive if val < lb
            let upper_viol = val - bounds.upper; // positive if val > ub
            let violation = lower_viol.max(upper_viol);
            if violation > self.constants.feasibility_tolerance {
                Some((var, violation))
            } else {
                None
            }
        })
    }
}

// ── Test helpers ─────────────────────────────────────────────────────────

impl Model {
    /// Get all delta batches since the given revision for testing.
    ///
    /// Returns batches in order. An empty vec means no batches recorded
    /// since the given revision. Errors if `since` is in the future.
    pub fn deltas_since(
        &self,
        since: ModelRevision,
    ) -> Result<Vec<&DeltaBatch>, crate::revision::RevisionError> {
        self.coordinator.journal.deltas_since(since)
    }
}

// ── Constraint bounds validation ──────────────────────────────────────────

/// Validate raw constraint bounds before mutation (D10/API-06.1/06.2).
///
/// Rejects NaN bounds (typed [`ModelError::NonFiniteValue`]) and inverted
/// bounds (`lower > upper`, [`ModelError::InvalidBounds`]). Infinite sides
/// remain valid (`le`/`ge` forms).
pub(crate) fn validate_constraint_bounds(bounds: ConstraintBounds) -> Result<(), ModelError> {
    if bounds.lower.is_nan() || bounds.upper.is_nan() {
        return Err(ModelError::NonFiniteValue("constraint bound"));
    }
    if bounds.lower > bounds.upper {
        return Err(ModelError::InvalidBounds);
    }
    Ok(())
}

// ── Change compilation ─────────────────────────────────────────────────────

/// Compile a `Change` into a self-contained `ModelOp` for DeltaBatch.
///
/// Every Change variant is mapped to exactly one ModelOp variant. No events
/// are silently dropped.
fn compile_change(change: Change) -> Result<ModelOp, ModelError> {
    match change {
        Change::VariableAdded {
            var,
            bounds,
            var_type,
        } => Ok(ModelOp::AddVariable {
            var,
            bounds,
            var_type,
        }),
        Change::VariableRemoved { var } => Ok(ModelOp::RemoveVariable { var }),
        Change::VariableBoundsChanged {
            var, new: bounds, ..
        } => Ok(ModelOp::SetVariableBounds { var, bounds }),
        Change::VariableFixingChanged {
            var,
            fixing,
            effective_bounds,
        } => Ok(ModelOp::SetVariableFixing {
            var,
            fixing,
            effective_bounds,
        }),
        Change::VariableTypeChanged {
            var, new: var_type, ..
        } => Ok(ModelOp::SetVariableType { var, var_type }),
        Change::VariableActivityChanged { var, active } => {
            Ok(ModelOp::SetVariableActive { var, active })
        }
        Change::SemiContinuousBoundChanged { var, lower } => {
            Ok(ModelOp::SetSemiContinuousBound { var, lower })
        }
        Change::ConstraintAdded { con, bounds } => Ok(ModelOp::AddConstraint { con, bounds }),
        Change::ConstraintRemoved { con } => Ok(ModelOp::RemoveConstraint { con }),
        Change::ConstraintBoundsChanged {
            con, new: bounds, ..
        } => Ok(ModelOp::SetConstraintBounds { con, bounds }),
        Change::ConstraintActivityChanged { con, active } => {
            Ok(ModelOp::SetConstraintActive { con, active })
        }
        Change::CoefficientAdded {
            var,
            target,
            value,
            value_expr: expr,
            ..
        } => Ok(ModelOp::SetCell {
            cell_key: (target, var),
            value_expr: expr,
            evaluated_value: value,
        }),
        Change::CoefficientRemoved { var, target, .. } => Ok(ModelOp::RemoveCell {
            cell_key: (target, var),
        }),
        Change::CoefficientValueChanged {
            var,
            target,
            new: evaluated,
            value_expr: expr,
            ..
        } => Ok(ModelOp::SetCell {
            cell_key: (target, var),
            value_expr: expr,
            evaluated_value: evaluated,
        }),
        Change::ObjectiveAdded { obj, sense } => Ok(ModelOp::AddObjective { obj, sense }),
        Change::ObjectiveRemoved { obj } => Ok(ModelOp::RemoveObjective { obj }),
        Change::ObjectiveSenseChanged {
            obj, new: sense, ..
        } => Ok(ModelOp::SetObjectiveSense { obj, sense }),
        Change::ObjectiveConstantChanged {
            obj, new: constant, ..
        } => Ok(ModelOp::SetObjectiveConstant { obj, constant }),
        Change::ActiveObjectiveChanged { new, .. } => Ok(ModelOp::SetActiveObjective { obj: new }),
        Change::ParameterValueChanged { param, new, .. } => {
            Ok(ModelOp::SetParameter { param, value: new })
        }
        Change::ConstructAdded {
            construct,
            kind,
            preference,
            active,
        } => Ok(ModelOp::AddConstruct {
            construct,
            kind,
            preference,
            active,
        }),
        Change::ConstructRemoved { construct } => Ok(ModelOp::RemoveConstruct { construct }),
        Change::ConstructActivityChanged { construct, active } => {
            Ok(ModelOp::SetConstructActive { construct, active })
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(deprecated)] // unit tests exercise the pre-1.0 compatibility surface
    use crate::expr::LinExpr;

    use super::*;

    // helper to initialize logging once per test run. we ignore the result in
    // case the user hasn't provided a config file; most unit tests don't care
    // about logs but they should compile/link when the function exists.

    #[test]
    fn basic_model_operations() {
        let mut model = Model::new();

        // Add variables
        let x = model.add_var();
        let y = model.add_var();

        // Add constraint
        // let c = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();

        // // Add coefficients
        // model.add_coeff(c, x, 2.0).unwrap();
        // model.add_coeff(c, y, 3.0).unwrap();

        let _c = model
            .add_constraint_expr(2.0 * x + 3.0 * y, ConstraintBounds::le(100.0))
            .unwrap();

        assert_eq!(model.num_variables(), 2);
        assert_eq!(model.num_constraints(), 1);
        assert_eq!(model.num_coefficients(), 2);
    }

    #[test]
    fn parameter_propagation() {
        let mut model = Model::new();

        let p = model.add_parameter(10.0).unwrap();
        let x = model.add_var();
        let c = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();

        // Coefficient with parameter dependency: 2 * p
        let coeff_id = model
            .add_constraint_coefficient(
                c,
                x,
                ValueExpr::mul(ValueExpr::constant(2.0), ValueExpr::param(p)),
            )
            .unwrap();

        // Initial value should be 2 * 10 = 20
        assert_eq!(model.coefficient(coeff_id).unwrap().cached_value, 20.0);

        // Change parameter
        model.set_parameter(p, 5.0).unwrap();
        let _ = model.commit();

        // Value should now be 2 * 5 = 10
        assert_eq!(model.coefficient(coeff_id).unwrap().cached_value, 10.0);
    }

    #[test]
    fn coefficient_api_accepts_constants_and_parameters_symmetrically() {
        let mut model = Model::new();

        let p = model.add_parameter(2.5).unwrap();
        let x = model.add_var();
        let y = model.add_var();
        let con = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
        let obj = model.add_objective(Sense::Minimize);

        let constraint_coeff = model.add_constraint_coefficient(con, x, p).unwrap();
        let objective_coeff = model.add_objective_coefficient(obj, x, 1.5).unwrap();
        let objective_shorthand = model.add_objective_coeff(obj, y, 3.0).unwrap();

        assert_eq!(
            model.coefficient(constraint_coeff).unwrap().cached_value,
            2.5
        );
        assert_eq!(
            model.coefficient(objective_coeff).unwrap().cached_value,
            1.5
        );
        assert_eq!(
            model.coefficient(objective_shorthand).unwrap().cached_value,
            3.0
        );
    }

    #[test]
    fn transaction_batching() {
        let mut model = Model::new();

        let p1 = model.add_parameter(1.0).unwrap();
        let p2 = model.add_parameter(2.0).unwrap();
        let x = model.add_var();
        let c = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();

        // Coefficient: p1 * p2
        let coeff_id = model
            .add_constraint_coefficient(
                c,
                x,
                ValueExpr::mul(ValueExpr::param(p1), ValueExpr::param(p2)),
            )
            .unwrap();

        assert_eq!(model.coefficient(coeff_id).unwrap().cached_value, 2.0); // 1 * 2

        // Batch changes
        model.set_parameter(p1, 3.0).unwrap();
        model.set_parameter(p2, 4.0).unwrap();

        // Not committed yet - value unchanged
        assert_eq!(model.coefficient(coeff_id).unwrap().cached_value, 2.0);

        let _ = model.commit();

        // Now it's 3 * 4 = 12
        assert_eq!(model.coefficient(coeff_id).unwrap().cached_value, 12.0);
    }

    #[test]
    #[allow(deprecated)]
    fn changelog_tracking() {
        let mut model = Model::new();

        let x = model.add_var();
        let c = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
        model.add_coeff(c, x, 2.0).unwrap();

        let changes = model.drain_changes();
        assert_eq!(changes.len(), 3); // variable, constraint, coefficient
    }

    #[test]
    fn remove_cascades() {
        let mut model = Model::new();

        let x = model.add_var();
        let c = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
        model.add_coeff(c, x, 2.0).unwrap();

        assert_eq!(model.num_coefficients(), 1);

        // Removing the variable should remove its coefficient
        model.remove_variable(x).unwrap();

        assert_eq!(model.num_coefficients(), 0);
    }

    #[test]
    fn complex_model_flow() {
        // build a model with variables, parameters, constraints, objective
        let mut model = Model::new();
        let x = model.add_variable(continuous()).unwrap();
        let y = model.add_variable(continuous()).unwrap();
        let z = model.add_variable(continuous()).unwrap();

        let p = model.add_parameter(2.0).unwrap();
        let q = model.add_parameter(3.0).unwrap();

        // constraint: 2*x + p*y - q*z <= 100
        let cons_expr: LinExpr = 2.0 * x + p * y - q * z;
        let cons_bounds = ConstraintBounds::le(100.0);
        let con = model.add_constraint_expr(cons_expr, cons_bounds).unwrap();

        // objective: minimize p*x + 3*y + 5
        let obj_expr: LinExpr = p * x + 3.0 * y + 5.0;
        let (obj, offset) = model.add_objective_expr(obj_expr, Sense::Minimize).unwrap();
        assert_eq!(offset, 5.0);
        assert_eq!(model.objective_constant(obj), Some(5.0));

        // record coefficient ids for later
        let con_coeffs: Vec<_> = model.coefficients.for_constraint(con).collect();
        let obj_coeffs: Vec<_> = model.coefficients.for_objective(obj).collect();

        // check initial cached values
        let mut map = std::collections::HashMap::new();
        for cid in &con_coeffs {
            let dat = model.coefficient(*cid).unwrap();
            map.insert(dat.var, dat.cached_value);
        }
        assert_eq!(map.get(&x), Some(&2.0));
        assert_eq!(map.get(&y), Some(&2.0)); // p=2 initial
        assert_eq!(map.get(&z), Some(&-3.0));

        let mut objmap = std::collections::HashMap::new();
        for oid in &obj_coeffs {
            let dat = model.coefficient(*oid).unwrap();
            objmap.insert(dat.var, dat.cached_value);
        }
        assert_eq!(objmap.get(&x), Some(&2.0));
        assert_eq!(objmap.get(&y), Some(&3.0));

        // update parameters and commit
        model.set_parameter(p, 4.0).unwrap();
        model.set_parameter(q, 6.0).unwrap();
        let _ = model.commit();

        // after update, cached values should change
        let mut map2 = std::collections::HashMap::new();
        for cid in &con_coeffs {
            let dat = model.coefficient(*cid).unwrap();
            map2.insert(dat.var, dat.cached_value);
        }
        assert_eq!(map2.get(&y), Some(&4.0));
        assert_eq!(map2.get(&z), Some(&-6.0));

        let mut objmap2 = std::collections::HashMap::new();
        for oid in &obj_coeffs {
            let dat = model.coefficient(*oid).unwrap();
            objmap2.insert(dat.var, dat.cached_value);
        }
        assert_eq!(objmap2.get(&x), Some(&4.0));

        // also reconstruct expressions to ensure they still look right
        let recon = model.constraint_expression(con).unwrap();
        assert_eq!(recon.num_terms(), 3);
        let recon_obj = model.objective_expression(obj).unwrap();
        assert_eq!(recon_obj.num_terms(), 2);
        assert_eq!(recon_obj.get_constant(), 5.0);
    }

    // ── pprint ────────────────────────────────────────────────────────────

    /// A production-style LP:
    ///
    ///   3 variables: x (continuous), y (continuous), z (binary)
    ///   2 parameters: a=2.0, b=5.0
    ///   2 constraints:
    ///     c1: a*x + y <= 10     (resource)
    ///     c2: x + b*z >= 1      (activation)
    ///   1 objective:  minimize 3*x + 2*y + 4*z
    ///
    /// pprint is checked for structural presence of key tokens. Visual review
    /// of the printed output is the primary check.
    #[test]
    fn pprint_medium_model() {
        let mut model = Model::with_name("production_lp");

        let x = model.add_variable(continuous()).unwrap();
        let y = model.add_variable(continuous()).unwrap();
        let z = model.add_variable(binary()).unwrap();

        let a = model.add_parameter(2.0).unwrap();
        let b = model.add_parameter(5.0).unwrap();

        // c1: a*x + y <= 10
        let c1 = model
            .add_constraint_expr(
                LinExpr::new().term(a, x).add_term_with(1.0, y),
                ConstraintBounds::le(10.0),
            )
            .unwrap();

        // c2: x + b*z >= 1
        let _c2 = model
            .add_constraint_expr(
                LinExpr::new().add_term_with(1.0, x).term(b, z),
                ConstraintBounds::ge(1.0),
            )
            .unwrap();

        // objective: minimize 3x + 2y + 4z
        let (obj, _) = model
            .add_objective_expr(3.0 * x + 2.0 * y + 4.0 * z, Sense::Minimize)
            .unwrap();
        model.set_active_objective(obj).unwrap();

        // Deactivate c1 to exercise [inactive] marker
        model.set_constraint_active(c1, false).unwrap();

        let output = model.pprint();
        println!("{output}");

        // Structural checks
        assert!(
            output.contains("Model: production_lp"),
            "missing model header"
        );
        assert!(output.contains("Variables (3):"), "wrong variable count");
        assert!(output.contains("Parameters (2):"), "wrong parameter count");
        assert!(
            output.contains("Constraints (2):"),
            "wrong constraint count"
        );
        assert!(output.contains("Objectives (1):"), "wrong objective count");
        assert!(
            output.contains("[inactive]"),
            "missing inactive marker on c1"
        );
        assert!(
            output.contains("[active]"),
            "missing active marker on objective"
        );
        assert!(output.contains("Binary"), "missing Binary type for z");
        assert!(output.contains("Minimize"), "missing Minimize sense");
        assert!(output.contains("p["), "missing parameter display");
        assert!(output.contains("c["), "missing constraint display");
        assert!(output.contains("obj["), "missing objective display");
    }

    // ── constraint_slack ─────────────────────────────────────────────────

    #[test]
    fn constraint_slack_feasible() {
        let mut model = Model::new();
        let x = model.add_var();
        let y = model.add_var();

        // 2x + 3y <= 12  →  with x=1, y=2: lhs = 2+6 = 8
        let c = model
            .add_constraint_expr(2.0 * x + 3.0 * y, ConstraintBounds::le(12.0))
            .unwrap();

        use crate::solution::SolutionBuilder;
        use crate::solver::SolverStatus;
        let sol = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 1.0)
            .value(y, 2.0)
            .build();

        let (lower_slack, upper_slack) = model.constraint_slack(c, &sol).unwrap();
        // lower bound is -inf → lower_slack = lhs - (-inf) = +inf
        assert!(
            lower_slack.is_infinite() && lower_slack > 0.0,
            "expected +inf lower slack, got {lower_slack}"
        );
        // upper_slack = 12 - 8 = 4
        assert!(
            (upper_slack - 4.0).abs() < model.constants.feasibility_tolerance,
            "expected upper slack = 4, got {upper_slack}"
        );
    }

    #[test]
    fn default_tolerance_is_small_nonzero() {
        // ensure the default value is not zero; it should match the constant
        let m = Model::new();
        assert!(
            m.constants.feasibility_tolerance > 0.0,
            "default tolerance should be positive"
        );
        assert_eq!(m.constants.feasibility_tolerance, 1e-9);
    }

    #[test]
    fn constraint_slack_violated() {
        let mut model = Model::new();
        let x = model.add_var();

        // x >= 5  → with x=2: lhs=2, lower_slack = 2-5 = -3 (violated)
        let c = model
            .add_constraint_expr(LinExpr::from(x), ConstraintBounds::ge(5.0))
            .unwrap();

        use crate::solution::SolutionBuilder;
        use crate::solver::SolverStatus;
        let sol = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 2.0)
            .build();

        let (lower_slack, upper_slack) = model.constraint_slack(c, &sol).unwrap();
        // lower_slack = 2 - 5 = -3
        assert!(
            (lower_slack - (-3.0)).abs() < model.constants.feasibility_tolerance,
            "expected lower slack = -3, got {lower_slack}"
        );
        // upper bound is +inf → upper_slack = inf - 2 = +inf
        assert!(
            upper_slack.is_infinite() && upper_slack > 0.0,
            "expected +inf upper slack, got {upper_slack}"
        );
    }

    // ── violated_constraints ─────────────────────────────────────────────

    #[test]
    fn violated_constraints_finds_violations() {
        let mut model = Model::new();
        let x = model.add_var();
        let y = model.add_var();

        // c1: x <= 3  → satisfied with x=2
        let c1 = model
            .add_constraint_expr(LinExpr::from(x), ConstraintBounds::le(3.0))
            .unwrap();

        // c2: y >= 5  → violated with y=1
        let c2 = model
            .add_constraint_expr(LinExpr::from(y), ConstraintBounds::ge(5.0))
            .unwrap();

        use crate::solution::SolutionBuilder;
        use crate::solver::SolverStatus;
        let sol = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 2.0)
            .value(y, 1.0)
            .build();

        let violations: Vec<_> = model.violated_constraints(&sol).collect();
        assert_eq!(
            violations.len(),
            1,
            "expected exactly 1 violated constraint"
        );
        let (con, lower_slack, _upper_slack) = violations[0];
        assert_eq!(con, c2, "violated constraint should be c2");
        assert!(
            (lower_slack - (-4.0)).abs() < 1e-9,
            "expected lower_slack = -4, got {lower_slack}"
        );

        // c1 should not appear
        assert!(!violations.iter().any(|(c, _, _)| *c == c1));
    }

    // ── bound_violations ─────────────────────────────────────────────────

    #[test]
    fn bound_violations_detects_out_of_bounds() {
        let mut model = Model::new();

        // x in [0, 10]  → solution x=12 violates upper bound
        let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();

        // y in [2, 8]   → solution y=1 violates lower bound
        let y = model.add_variable(continuous().bounds(2.0, 8.0)).unwrap();

        // z in [0, 5]   → solution z=3 is fine
        let z = model.add_variable(continuous().bounds(0.0, 5.0)).unwrap();

        use crate::solution::SolutionBuilder;
        use crate::solver::SolverStatus;
        let sol = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 12.0) // above ub
            .value(y, 1.0) // below lb
            .value(z, 3.0) // feasible
            .build();

        let violations: Vec<_> = model.bound_violations(&sol).collect();
        assert_eq!(violations.len(), 2, "expected 2 bound violations");

        let viol_x = violations.iter().find(|(v, _)| *v == x).map(|(_, d)| *d);
        let viol_y = violations.iter().find(|(v, _)| *v == y).map(|(_, d)| *d);
        assert!(viol_x.is_some(), "x should have a bound violation");
        assert!(
            (viol_x.unwrap() - 2.0).abs() < 1e-9,
            "x violation = 12-10 = 2, got {:?}",
            viol_x
        );
        assert!(viol_y.is_some(), "y should have a bound violation");
        assert!(
            (viol_y.unwrap() - 1.0).abs() < 1e-9,
            "y violation = 2-1 = 1, got {:?}",
            viol_y
        );

        let z_violation = violations.iter().find(|(v, _)| *v == z);
        assert!(z_violation.is_none(), "z should have no bound violation");
    }

    // ── Invariant checker tests ──────────────────────────────────────────

    #[test]
    fn empty_model_passes_invariants() {
        let model = Model::new();
        assert!(model.validate_invariants().is_ok());
    }

    #[test]
    fn simple_model_passes_invariants() {
        let mut model = Model::new();
        let x = model.add_var();
        let con = model.add_constraint(ConstraintBounds::le(10.0)).unwrap();
        model.add_coeff(con, x, 1.0).unwrap();
        assert!(model.validate_invariants().is_ok());
    }

    #[test]
    fn objective_model_passes_invariants() {
        let mut model = Model::new();
        let x = model.add_var();
        let obj = model.add_objective(Sense::Maximize);
        model.set_active_objective(obj).unwrap();
        model
            .add_objective_coefficient(obj, x, ValueExpr::constant(1.0))
            .unwrap();
        assert!(model.validate_invariants().is_ok());
    }

    #[test]
    fn parameter_model_passes_invariants() {
        let mut model = Model::new();
        let x = model.add_var();
        let p = model.add_parameter(5.0).unwrap();
        let con = model.add_constraint(ConstraintBounds::le(10.0)).unwrap();
        model
            .add_constraint_coefficient(con, x, ValueExpr::param(p))
            .unwrap();
        model.set_parameter(p, 3.0).unwrap();
        let _ = model.commit();
        assert!(model.validate_invariants().is_ok());
    }

    #[test]
    fn invariants_after_remove() {
        let mut model = Model::new();
        let x = model.add_var();
        let y = model.add_var();
        let con = model.add_constraint(ConstraintBounds::le(10.0)).unwrap();
        model.add_coeff(con, x, 1.0).unwrap();
        model.add_coeff(con, y, 2.0).unwrap();

        assert!(model.validate_invariants().is_ok());

        model.remove_variable(x).unwrap();
        // x is removed; its coefficients are cascaded. Invariants should still hold.
        assert!(model.validate_invariants().is_ok());
    }

    #[test]
    fn canonical_cell_invariants() {
        // Verify that canonical cell combining doesn't violate invariants
        let mut model = Model::new();
        let x = model.add_var();
        let con = model.add_constraint(ConstraintBounds::le(10.0)).unwrap();

        let id1 = model.add_coeff(con, x, 2.0).unwrap();
        // Adding another term for the same cell should combine
        let id2 = model
            .add_constraint_coefficient(con, x, ValueExpr::constant(3.0))
            .unwrap();

        assert_eq!(id1, id2, "canonical cell: same ID returned on combine");
        assert!(model.validate_invariants().is_ok());

        // combined value: 2.0 + 3.0 = 5.0
        assert!((model.coefficient(id1).unwrap().cached_value - 5.0).abs() < 1e-9);
    }
}

// ── Construct lifecycle (P25 Task 4, design §7) ────────────────────────────
//
// Moved IN-CRATE from tests/semantic_ir.rs (F3): these exercise the
// `#[cfg(test)]`-gated fixture scaffolding (`FixturePayload`,
// `ConstructKind::Fixture`, `add_construct_fixture`, `Model::construct`, and
// the pub(crate) snapshot / delta `.constructs` fields), which is test-only
// and absent from the public API surface (A30).
#[cfg(test)]
mod construct_tests {
    // `ConstructKind` is single-variant (`Fixture`) in P25. The tests keep the
    // defensive `if let ... else panic!` shape for P32's variant expansion, so
    // the pattern is currently irrefutable.
    #![allow(irrefutable_let_patterns)]

    use super::*;

    fn fixture(key: &str, value: f64) -> FixturePayload {
        FixturePayload::new(key.to_string(), value)
    }

    #[test]
    fn construct_add_returns_stable_id_and_payload_round_trips() {
        let mut model = Model::new();
        let k = model
            .add_construct_fixture(fixture("cap", 100.0), FormulationPreference::Auto)
            .unwrap();

        let entry = model.construct(k).unwrap();
        assert_eq!(entry.id, k, "add returns the stable construct id");
        assert!(entry.active, "constructs start active");
        if let ConstructKind::Fixture(p) = &entry.kind {
            assert_eq!(p.key, "cap");
            assert_eq!(p.value, 100.0);
        } else {
            panic!("expected fixture payload");
        }
    }

    #[test]
    fn construct_clone_preserves_ids_and_activity() {
        let mut model = Model::new();
        let k = model
            .add_construct_fixture(fixture("cap", 50.0), FormulationPreference::Portable)
            .unwrap();
        model.set_construct_active(k, false).unwrap();

        let cloned = model.clone();
        let entry = cloned.construct(k).unwrap();
        assert_eq!(entry.id, k, "clone preserves the construct id");
        assert!(!entry.active, "clone preserves activity");
        if let ConstructKind::Fixture(p) = &entry.kind {
            assert_eq!(p.value, 50.0);
        }
        assert_eq!(cloned.num_constructs(), model.num_constructs());
    }

    #[test]
    fn construct_snapshot_and_delta_round_trip() {
        let mut model = Model::new();
        let k = model
            .add_construct_fixture(fixture("on", 1.0), FormulationPreference::Auto)
            .unwrap();
        let r1 = model.commit().unwrap();

        // Snapshot carries every construct entry.
        let snap = model.take_snapshot().unwrap();
        assert_eq!(snap.constructs.len(), 1);
        assert_eq!(snap.constructs[0].id, k);
        assert!(snap.constructs[0].active);

        // Delta carries the added construct entry.
        let batches = model.deltas_since(ModelRevision::ZERO).unwrap();
        let batch = batches
            .iter()
            .find(|b| b.to == r1)
            .expect("construct-add batch present");
        assert_eq!(batch.constructs.len(), 1);
        assert_eq!(batch.constructs[0].id, k);

        // Deterministic snapshot round-trip.
        assert_eq!(snap, model.take_snapshot().unwrap());
    }

    #[test]
    fn construct_activity_toggling_reflected_in_snapshot() {
        let mut model = Model::new();
        let k = model
            .add_construct_fixture(fixture("t", 2.0), FormulationPreference::NativeRequired)
            .unwrap();
        model.set_construct_active(k, false).unwrap();

        let snap = model.take_snapshot().unwrap();
        assert!(
            !snap.constructs[0].active,
            "inactive construct reflected in snapshot"
        );
    }

    #[test]
    fn construct_remove_invalidates_id_and_stale_ids_rejected() {
        let mut model = Model::new();
        let k = model
            .add_construct_fixture(fixture("gone", 7.0), FormulationPreference::Auto)
            .unwrap();
        assert_eq!(model.num_constructs(), 1);

        model.remove_construct(k).unwrap();
        assert_eq!(model.num_constructs(), 0);

        // Stale id is rejected with a typed error.
        match model.construct(k) {
            Err(ModelError::ConstructNotFound(id)) => assert_eq!(id, k),
            other => panic!("expected ConstructNotFound, got {other:?}"),
        }
        assert!(model.set_construct_active(k, true).is_err());
        assert!(model.remove_construct(k).is_err());
    }

    #[test]
    fn construct_store_survives_rebuild() {
        let mut model = Model::new();
        model
            .add_construct_fixture(fixture("a", 1.0), FormulationPreference::Auto)
            .unwrap();
        let k2 = model
            .add_construct_fixture(fixture("b", 2.0), FormulationPreference::Portable)
            .unwrap();
        model.set_construct_active(k2, false).unwrap();

        // Snapshot captures the construct store.
        let snap = model.take_snapshot().unwrap();
        assert_eq!(snap.constructs.len(), 2);

        // Rebuild: a fresh empty model restored from the snapshot carries the
        // same construct content (kind + activity), with fresh ids.
        let mut rebuilt = Model::new();
        // Track each rebuilt entry's fresh id so the reconstruction can be
        // looked up by id instead of relying on order coincidence (IN-03).
        let mut rebuilt_ids: Vec<(ConstructEntry, Construct)> = Vec::new();
        for entry in &snap.constructs {
            let payload = if let ConstructKind::Fixture(p) = &entry.kind {
                p.clone()
            } else {
                panic!("expected fixture payload");
            };
            let id = rebuilt
                .add_construct_fixture(payload, FormulationPreference::Auto)
                .unwrap();
            if !entry.active {
                rebuilt.set_construct_active(id, false).unwrap();
            }
            rebuilt_ids.push((entry.clone(), id));
        }
        assert_eq!(rebuilt.num_constructs(), 2);

        // Rebuilding from the same snapshot reproduces equal construct content:
        // look each rebuilt entry up by its fresh id and assert the full
        // `ConstructEntry` (kind + activity) matches the original (IN-03).
        let rebuilt_snap = rebuilt.take_snapshot().unwrap();
        assert_eq!(rebuilt_snap.constructs.len(), snap.constructs.len());
        for (original, new_id) in &rebuilt_ids {
            let rebuilt_entry = rebuilt_snap
                .constructs
                .iter()
                .find(|e| e.id == *new_id)
                .expect("rebuilt construct present in snapshot");
            assert_eq!(
                rebuilt_entry.kind, original.kind,
                "rebuilt construct kind must match the original"
            );
            assert_eq!(
                rebuilt_entry.active, original.active,
                "rebuilt construct activity must match the original"
            );
        }
    }

    #[test]
    fn construct_metadata_usable_via_entity_ref() {
        let mut model = Model::new();
        let k = model
            .add_construct_fixture(fixture("meta", 1.0), FormulationPreference::Auto)
            .unwrap();

        let meta = EntityMetadata {
            description: Some("a construct".to_string()),
            ..EntityMetadata::default()
        };
        model
            .set_metadata(EntityRef::Construct(k), meta.clone())
            .unwrap();
        assert_eq!(
            model.metadata(EntityRef::Construct(k)),
            Some(&meta),
            "EntityRef::Construct is usable now (design §4.4)"
        );
        assert!(model.validate_invariants().is_ok());
    }

    /// F4: `FormulationPreference` must round-trip through the construct entry,
    /// the snapshot, and the delta so P26 can honor Auto/Portable/NativeRequired
    /// from canonical snapshots/deltas.
    #[test]
    fn construct_preference_round_trips_through_snapshot_and_delta() {
        let mut model = Model::new();
        let k = model
            .add_construct_fixture(fixture("pref", 1.0), FormulationPreference::NativeRequired)
            .unwrap();
        let r1 = model.commit().unwrap();

        // The canonical entry carries the preference.
        let entry = model.construct(k).unwrap();
        assert_eq!(
            entry.preference,
            FormulationPreference::NativeRequired,
            "entry must carry the preference"
        );

        // Snapshot carries it.
        let snap = model.take_snapshot().unwrap();
        let snap_entry = snap
            .constructs
            .iter()
            .find(|e| e.id == k)
            .expect("snapshot carries the construct entry");
        assert_eq!(
            snap_entry.preference,
            FormulationPreference::NativeRequired,
            "snapshot entry must carry the preference"
        );

        // Delta carries it.
        let batches = model.deltas_since(ModelRevision::ZERO).unwrap();
        let batch = batches
            .iter()
            .find(|b| b.to == r1)
            .expect("construct-add batch present");
        let delta_entry = batch
            .constructs
            .iter()
            .find(|e| e.id == k)
            .expect("delta carries the construct entry");
        assert_eq!(
            delta_entry.preference,
            FormulationPreference::NativeRequired,
            "delta entry must carry the preference"
        );

        // Rebuild from the snapshot preserves it.
        let mut rebuilt = Model::new();
        for entry in &snap.constructs {
            let payload = if let ConstructKind::Fixture(p) = &entry.kind {
                p.clone()
            } else {
                panic!("expected fixture payload");
            };
            let id = rebuilt
                .add_construct_fixture(payload, entry.preference)
                .unwrap();
            if !entry.active {
                rebuilt.set_construct_active(id, false).unwrap();
            }
        }
        let rebuilt_snap = rebuilt.take_snapshot().unwrap();
        let rebuilt_entry = rebuilt_snap
            .constructs
            .iter()
            .find(|e| e.id != k)
            .expect("rebuilt construct present in snapshot");
        assert_eq!(
            rebuilt_entry.preference,
            FormulationPreference::NativeRequired,
            "rebuild from snapshot preserves the preference"
        );
    }

    /// WR-06: removing a construct must cascade its metadata, so the valid
    /// attach-metadata-then-remove sequence does not trip `validate_invariants`
    /// with an orphaned construct-metadata entry.
    #[test]
    fn construct_remove_cascades_metadata_and_invariants_pass() {
        let mut model = Model::new();
        let k = model
            .add_construct_fixture(fixture("meta", 1.0), FormulationPreference::Auto)
            .unwrap();
        model
            .set_metadata(
                EntityRef::Construct(k),
                EntityMetadata {
                    description: Some("doomed".to_string()),
                    ..EntityMetadata::default()
                },
            )
            .unwrap();

        model.remove_construct(k).unwrap();
        assert!(
            model.metadata(EntityRef::Construct(k)).is_none(),
            "construct metadata cascaded on removal"
        );
        assert!(
            model.validate_invariants().is_ok(),
            "no orphaned construct metadata after removal"
        );
    }
}
