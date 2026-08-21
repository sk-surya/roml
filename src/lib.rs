//! ROML - Rust Optimization Modeling Library
//!
//! A pre-release, incremental MILP modeling library that:
//! - Supports efficient model mutation
//! - Cleanly separates model and solver concerns
//! - Supports multiple solver backends
//! - Stores and reasons about solutions
//! - Allows algebraic introspection (slack, infeasibility, evaluation)
//!
//! The public surface is split into a curated [`prelude`] for ordinary model
//! authors and the [`advanced`] namespace for framework/backend authors
//! (API-07). See `MODELING_API.md` for the modeling guide.

#![warn(missing_docs)]

pub mod advanced;
pub mod assignment;
pub mod compiler;
// A30 (P32): the real per-construct variants land in P32 Task 16, so the
// construct module and `ConstructKind`/`ConstructEntry` become PUBLIC exports.
// The `Fixture` variant, `FixturePayload`, and `add_construct_fixture` are
// `#[cfg(test)]`-gated test-only scaffolding (absent from the public API
// surface in non-test builds); the `#[non_exhaustive]` boundary stays.
pub mod construct;
pub mod delta;
pub mod expr;
pub mod function;
pub mod id;
pub mod identity;
/// Solver-free import and export boundaries for supported file formats.
pub mod io;
pub(crate) mod journal;
pub mod metadata;
pub mod model;
pub mod objective_policy;
pub mod revision;
pub mod snapshot;
pub mod solution;
pub mod solver;
pub mod sync;
pub(crate) mod transaction;
pub mod value_expr;

// Re-export commonly used types for public API
pub use assignment::{
    AssignmentError, ContinuousLock, LockSelector, PrimalAssignment, SolutionLock,
};
pub use construct::{
    AbsoluteValueConstraint, AbsoluteValueVariant, BinaryProductConstraint, BooleanConstraint,
    BooleanKind, CardinalityConstraint, CardinalityKind, Construct, ConstructEntry, ConstructKind,
    ExtrapolationPolicy, FormulationPreference, IndicatorConstraint, IndicatorDirection,
    MinMaxConstraint, MinMaxRelation, MinMaxSense, PenaltyPolicy, PenaltyTarget,
    PiecewiseLinearConstraint, ProductOperand, PwlCurvature, PwlPoint, PwlRelation,
    ReificationConstraint, SoftConstraint, SoftConstraintConstraint, ViolationPolicy,
    ViolationRole, ViolationSide,
};
pub use delta::{DeltaBatch, ModelOp};
pub use expr::{ConstraintExprExt, ConstraintSpec, LinExpr, ObjectiveExprExt, ObjectiveSpec};
pub use function::{
    FunctionConstraint, FunctionEntry, IntoScalarFunction, ScalarFunction, ScalarSet,
};
pub use id::{CoeffId, ConId, ObjId, ParamId, VarId};
pub use identity::{ConstructId, IdentityOverflow, ModelInstanceId, ModelLineageId};
pub use metadata::{EntityMetadata, EntityRef, ModelSource};
pub use model::changelog::Change;
pub use model::{
    binary, continuous, integer, parameter, Bounds, Constraint, ConstraintBounds, FixingProvenance,
    Model, ModelError, Objective, Parameter, ParameterDef, SemiDomain, Sense, VarType, Variable,
    VariableDef, VariableDomain, VariableFixing,
};
pub use objective_policy::{
    LexicographicObjectives, ObjectiveExecutionProvider, ObjectivePolicyError, ObjectivePriority,
    ObjectiveProviderPolicy, StageContinuation, WeightedObjective, WeightedObjectiveLevel,
    WeightedObjectives,
};
pub use revision::ModelRevision;
pub use snapshot::ModelSnapshot;
pub use solution::{
    metadata::SolveMetadata, metadata::SynchronizationMode, ConstraintViolation, SignedCorrection,
    Solution, SolutionBuilder, SolutionStore, ViolationError, ViolationPresentation,
};
pub use solver::backend::{
    BackendCapabilities, BackendError, ErrorCategory, HealthEffect, TerminationStatus,
};
pub use solver::effective_plan::{
    AppliedFeature, EffectiveSolvePlan, ObjectiveStageResult, PlanAdjustment, PlanRejection,
};
pub use solver::infeasibility::BoundSide;
pub use solver::infeasibility::ConflictAtomId;
pub use solver::infeasibility::{
    AnalysisBudget, AnalysisCompletion, AnalysisNumericalPolicy, AnalysisProviderRecord,
    AnalysisWarning, CandidateUniverseSummary, CompiledRestrictionEvidence,
    ConflictDeclarationSnapshot, ConflictGrouping, ConflictGuarantee, ConflictMember,
    FeasibilityProofStrength, InfeasibilityMode, InfeasibilityOutcome, InfeasibilityPlan,
    InfeasibilityReport, InfeasibilityScope, MarkdownInfeasibilityReport, NativeBoundStatus,
    NativeConflictEvidence, NativeConflictMember, NativeMembership, ReductionPolicy, SeedPolicy,
    TextInfeasibilityReport,
};
pub use solver::overlay::{
    CutoffDirection, ObjectiveCutoff, ObjectiveLock, OverlayError, SolveOverlay,
};
pub use solver::plan::{
    HintPriority, LexStagePolicy, MipStart, ObjectivePolicy, PlanError, RepairPolicy, SolvePlan,
    UnsupportedFeaturePolicy, VariableHint, VariableHints,
};
pub use solver::relaxation::{
    map_p29_members, FeasibilityRelaxationError, FeasibilityRelaxationPlan,
    FeasibilityRelaxationReport, RelaxationAcceptance, RelaxationExecutionProvider,
    RelaxationMappedRestriction, RelaxationMetadata, RelaxationNumerics, RelaxationObjective,
    RelaxationOutcome, RelaxationProviderPolicy, RelaxationRestriction, RelaxationScope,
    RelaxationUnknownReason, RelaxedRestriction,
};
pub use solver::request::{
    ConfigAdjustment, ConfigRejection, EffectiveConfig, SolveRequest, SolveResult, SolveSolution,
};
pub use solver::session::{
    BackendMetadata, BackendSession, CallbackSession, SessionHealth, SolutionView, SyncReceipt,
    Synchronization,
};
pub use solver::{
    classify_feasibility, FeasibilityOutcome, InfeasibilityError, LpAlgorithm, SolveError,
    SolveOptions, SolveStatus, SolverError, SolverSession, SolverStatus,
};
pub use sync::{AdapterCursor, AdapterHealth, ApplyOutcome};
pub use value_expr::ValueExpr;

/// Build a [`ConstraintSpec`] from math-like tokens.
///
/// Supports infix `<=`, `>=`, and `==` forms, plus an explicit ranged form:
///
/// ```ignore
/// use roml::{constraint, Model};
///
/// let mut model = Model::new();
/// let x = model.add_var();
/// let y = model.add_var();
///
/// let cap = constraint!(2.0 * x + y <= 10.0);
/// let floor = constraint!(x >= 1.0);
/// let band = constraint!(between: 0.0, x + y, 5.0);
/// ```
#[macro_export]
macro_rules! constraint {
	(between: $lower:expr, $expr:expr, $upper:expr) => {
		$crate::ConstraintSpec::new($expr, $crate::ConstraintBounds::range($lower, $upper))
	};
	(@scan [$($lhs:tt)+] <= $rhs:expr) => {
		$crate::ConstraintSpec::new(
			$crate::constraint!(@expr $($lhs)+),
			$crate::ConstraintBounds::le($rhs),
		)
	};
	(@scan [$($lhs:tt)+] >= $rhs:expr) => {
		$crate::ConstraintSpec::new(
			$crate::constraint!(@expr $($lhs)+),
			$crate::ConstraintBounds::ge($rhs),
		)
	};
	(@scan [$($lhs:tt)+] == $rhs:expr) => {
		$crate::ConstraintSpec::new(
			$crate::constraint!(@expr $($lhs)+),
			$crate::ConstraintBounds::eq($rhs),
		)
	};
	(@scan [$($lhs:tt)*] $next:tt $($rest:tt)*) => {
		$crate::constraint!(@scan [$($lhs)* $next] $($rest)*)
	};
	(@scan [$($lhs:tt)*]) => {
		compile_error!(
			"constraint! expects `expr <= rhs`, `expr >= rhs`, `expr == rhs`, or `between: lower, expr, upper`",
		)
	};
	(@expr $expr:expr) => {
		$expr
	};
	($($tokens:tt)+) => {
		$crate::constraint!(@scan [] $($tokens)+)
	};
}

/// Build an [`ObjectiveSpec`] from a sense and expression.
///
/// ```ignore
/// use roml::{objective, Model};
///
/// let mut model = Model::new();
/// let x = model.add_var();
/// let y = model.add_var();
///
/// let max_profit = objective!(maximize: x + 2.0 * y);
/// let min_cost = objective!(minimize: 3.0 * x + y);
/// ```
#[macro_export]
macro_rules! objective {
    (minimize: $expr:expr) => {
        $crate::ObjectiveSpec::new($crate::Sense::Minimize, $expr)
    };
    (maximize: $expr:expr) => {
        $crate::ObjectiveSpec::new($crate::Sense::Maximize, $expr)
    };
    ($($tokens:tt)*) => {
        compile_error!("objective! expects `minimize: expr` or `maximize: expr`")
    };
}

/// Add a constraint to a model from math-like tokens.
///
/// ```ignore
/// use roml::{constrain, Model};
///
/// let mut model = Model::new();
/// let x = model.add_var();
/// let y = model.add_var();
///
/// constrain!(model, x + y <= 4.0)?;
/// constrain!(model, between: 0.0, x, 3.0)?;
/// ```
///
/// **Deprecated (P23):** this macro is effectful (D1) — it mutates the model
/// directly. Use the canonical method-first mutation instead:
///
/// ```ignore
/// model.add_constraint(constraint!(x + y <= 4.0))?;
/// model.add_constraint((x + y).le(4.0))?;
/// ```
///
/// See `MIGRATION.md` → "Constraints".
#[deprecated(
    since = "0.1.0",
    note = "effectful; use `model.add_constraint(constraint!(...))` or `model.add_constraint((expr).le/ge/eq/between(...))`; see MIGRATION.md -> Constraints"
)]
#[macro_export]
macro_rules! constrain {
	($model:expr, between: $lower:expr, $expr:expr, $upper:expr) => {
		$model.constrain($crate::constraint!(between: $lower, $expr, $upper))
	};
	($model:expr, $($tokens:tt)+) => {
		$model.constrain($crate::constraint!($($tokens)+))
	};
}

/// Add and activate an objective on a model from a sense and expression.
///
/// ```ignore
/// use roml::{set_objective, Model};
///
/// let mut model = Model::new();
/// let x = model.add_var();
/// let y = model.add_var();
///
/// let obj = set_objective!(model, maximize: x + 2.0 * y + 3.0)?;
/// assert_eq!(model.objective_constant(obj), Some(3.0));
/// ```
///
/// **Deprecated (P23):** this macro is effectful (D1) — it mutates the model
/// directly. Use the canonical method-first mutations instead:
///
/// ```ignore
/// model.maximize(x + 2.0 * y + 3.0)?;
/// model.minimize(x + 2.0 * y + 3.0)?;
/// ```
///
/// See `MIGRATION.md` → "Objectives".
#[deprecated(
    since = "0.1.0",
    note = "effectful; use `model.maximize(expr)` / `model.minimize(expr)`; see MIGRATION.md -> Objectives"
)]
#[macro_export]
macro_rules! set_objective {
	($model:expr, minimize: $expr:expr) => {
		$model.set_objective($crate::objective!(minimize: $expr))
	};
	($model:expr, maximize: $expr:expr) => {
		$model.set_objective($crate::objective!(maximize: $expr))
	};
	($model:expr, $spec:expr) => {
		$model.set_objective($spec)
	};
}

/// Common imports for the fluent modeling API (P23 curated default surface).
///
/// This is the intentional default for ordinary model authors (API-07.1): it
/// covers model, expression, definition, solver, solution, and error types.
/// Protocol and backend-extension types are deliberately ABSENT from this
/// prelude (API-07.2); framework and backend authors reach them through
/// [`advanced`].
///
/// ```compile_fail
/// use roml::prelude::*;
///
/// // API-07.2: these protocol/backend-extension types must NOT resolve from
/// // the default prelude. If any of them compile here, the negative
/// // inventory has regressed and this doctest (correctly) fails.
/// fn _absent(
///     _: DeltaBatch,
///     _: ModelOp,
///     _: ModelRevision,
///     _: ModelSnapshot,
///     _: Change,
///     _: CoeffId,
///     _: AdapterCursor,
///     _: AdapterHealth,
///     _: Synchronization,
///     _: BackendSession,
///     _: SyncReceipt,
/// ) {
/// }
/// ```
pub mod prelude {
    pub use crate::{
        binary, constraint, continuous, integer, parameter, Bounds, Constraint, ConstraintExprExt,
        ConstraintSpec, LinExpr, Model, ModelError, Objective, ObjectiveExprExt, ObjectiveSpec,
        Parameter, ParameterDef, Sense, Solution, SolveError, SolveOptions, SolveStatus, VarType,
        Variable, VariableDef,
    };
}
