//! Solution storage and introspection.
//!
//! Solutions are immutable once stored and contain:
//! - Variable values
//! - Objective value(s)
//! - Solver status
//! - Optional duals and reduced costs
//! - Solve metadata (backend, revision, effective configuration)
//!
//! # Design
//!
//! Solutions are stored separately from the model. Multiple solutions can
//! be kept (latest, named snapshots, etc.). Expression evaluation against
//! solutions does not require solver access.

use std::collections::{BTreeMap, HashMap};

pub mod metadata;

use crate::assignment::PrimalAssignment;
use crate::compiler::backend_ir::CompilationId;
use crate::compiler::origin::OverlayId;
use crate::construct::SoftConstraint;
use crate::id::{ConId, ObjId, VarId};
use crate::model::Model;
use crate::solver::SolveStatus;

pub use metadata::{SolveMetadata, SynchronizationMode};

/// Typed failures returned by original-constraint violation accessors.
#[derive(Clone, Debug, PartialEq)]
pub enum ViolationError {
    /// The solution belongs to another model instance.
    ModelInstanceMismatch {
        /// The instance expected by the model.
        expected: crate::identity::ModelInstanceId,
        /// The instance recorded by the solution.
        actual: crate::identity::ModelInstanceId,
    },
    /// The solution belongs to an older/newer model revision.
    StaleRevision {
        /// The revision expected by the model.
        expected: crate::revision::ModelRevision,
        /// The revision recorded by the solution.
        actual: crate::revision::ModelRevision,
    },
    /// The referenced original constraint is absent.
    ConstraintNotFound(ConId),
    /// A referenced persistent soft constraint is stale or not a soft
    /// construct in the supplied model.
    SoftConstraintNotFound(SoftConstraint),
    /// A tolerance or evaluated value was not finite/nonnegative.
    InvalidTolerance(f64),
    /// The solution did not contain a value for a variable used by the
    /// requested expression.
    MissingVariableValue(VarId),
    /// A candidate variable value was not finite.
    NonFiniteVariableValue {
        /// The variable with the invalid value.
        variable: VarId,
        /// The invalid value.
        value: f64,
    },
    /// The evaluated constraint expression was not finite.
    NonFiniteEvaluation(f64),
    /// The solution was not produced for the exact compiled state requested
    /// by a solver-derived violation accessor.
    CompilationMismatch {
        /// Exact compiled state requested by the caller.
        expected: Option<CompilationId>,
        /// Exact compiled state recorded by the solution.
        actual: Option<CompilationId>,
    },
    /// The solution was not produced under the exact overlay requested by a
    /// solver-derived violation accessor.
    OverlayMismatch {
        /// Exact overlay requested by the caller.
        expected: Option<OverlayId>,
        /// Exact overlay recorded by the solution.
        actual: Option<OverlayId>,
    },
}

/// Raw lower/upper violation magnitudes for one original constraint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstraintViolation {
    /// Distance below the finite lower side, or zero when satisfied/absent.
    pub lower: f64,
    /// Distance above the finite upper side, or zero when satisfied/absent.
    pub upper: f64,
}

impl ConstraintViolation {
    /// Sum of the independent lower and upper violation magnitudes.
    pub const fn total(self) -> f64 {
        self.lower + self.upper
    }
}

/// Raw and tolerance-adjusted presentation of a violation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViolationPresentation {
    /// Mathematical raw values; never tolerance-adjusted.
    pub raw: ConstraintViolation,
    /// Values clipped by the requested presentation tolerance.
    pub adjusted: ConstraintViolation,
    /// Tolerance used for the presentation only.
    pub tolerance: f64,
}

/// Explicit signed correction parts. This type is independent of persistent
/// soft-constraint violation variables and is never extracted from them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SignedCorrection {
    /// Upward correction required to reach a finite lower side.
    pub positive: f64,
    /// Downward correction required to reach a finite upper side.
    pub negative: f64,
}

impl SignedCorrection {
    /// Signed net correction (`positive - negative`).
    pub const fn net(self) -> f64 {
        self.positive - self.negative
    }
}

/// A solution to the optimization problem.
///
/// Contains variable values, objective value, solver status, and solve
/// metadata. Solutions are immutable once created.
#[derive(Clone, Debug, PartialEq)]
pub struct Solution {
    /// Variable values.
    values: HashMap<VarId, f64>,
    /// Objective value (if solved successfully).
    objective_value: Option<f64>,
    /// Which objective this solution is solution for.
    objective_id: Option<ObjId>,
    /// Solver status
    status: SolveStatus,
    /// Dual values for constraints (if available).
    duals: Option<HashMap<ConId, f64>>,
    /// Reduced costs for variables (if available).
    reduced_costs: Option<HashMap<VarId, f64>>,
    /// Metadata describing how this solution was produced.
    metadata: SolveMetadata,
}

impl Solution {
    /// Create a new solution with the given status.
    pub fn new(status: SolveStatus) -> Self {
        Self {
            values: HashMap::new(),
            objective_value: None,
            objective_id: None,
            status,
            duals: None,
            reduced_costs: None,
            metadata: SolveMetadata::default(),
        }
    }

    /// Create a solution from variable values.
    pub fn from_values(values: HashMap<VarId, f64>, status: SolveStatus) -> Self {
        Self {
            values,
            objective_value: None,
            objective_id: None,
            status,
            duals: None,
            reduced_costs: None,
            metadata: SolveMetadata::default(),
        }
    }

    /// Get the solver status.
    pub fn status(&self) -> SolveStatus {
        self.status
    }

    /// Get the metadata describing how this solution was produced.
    pub fn metadata(&self) -> &SolveMetadata {
        &self.metadata
    }

    /// Set the metadata on a solution (builder style).
    pub fn with_metadata(mut self, metadata: SolveMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Check if the solution is optimal.
    pub fn is_optimal(&self) -> bool {
        self.status == SolveStatus::Optimal
    }

    /// Check if the solution has variable values.
    pub fn has_values(&self) -> bool {
        !self.values.is_empty()
    }

    /// Get a variable's value.
    pub fn value(&self, var: VarId) -> Option<f64> {
        self.values.get(&var).copied()
    }

    /// Get a variable's value, defaulting to 0.0 if not found.
    pub fn value_or_zero(&self, var: VarId) -> f64 {
        self.values.get(&var).copied().unwrap_or(0.0)
    }

    /// Get all variable values.
    pub fn values(&self) -> &HashMap<VarId, f64> {
        &self.values
    }

    /// Produce a lineage-bound [`PrimalAssignment`] of this solution's
    /// user-variable values (SM-06.2, design §11.1).
    ///
    /// Binds the SOLVED model's real lineage/instance/revision from the
    /// metadata (CR-02 pattern: real solved identity, never fresh
    /// [`SolveMetadata::default()`] counter ids). Compiler-only variables are
    /// excluded structurally at extraction — solution values are keyed by user
    /// [`VarId`], so the produced assignment never fabricates a value for a
    /// generated entity. The assignment makes no feasibility/optimality claim.
    pub fn primal_assignment(&self) -> PrimalAssignment {
        PrimalAssignment {
            lineage: self.metadata.model_lineage,
            source_instance: Some(self.metadata.model_instance),
            source_revision: Some(self.metadata.model_revision),
            values: self
                .values
                .iter()
                .map(|(var, value)| (*var, *value))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    /// Get the objective value.
    pub fn objective_value(&self) -> Option<f64> {
        self.objective_value
    }

    /// Get which objective this solution is for.
    pub fn objective_id(&self) -> Option<ObjId> {
        self.objective_id
    }

    /// Get the dual value for a constraint (if available).
    pub fn dual(&self, con: ConId) -> Option<f64> {
        self.duals.as_ref()?.get(&con).copied()
    }

    /// Check if dual values are available.
    pub fn has_duals(&self) -> bool {
        self.duals.is_some() && !self.duals().unwrap().is_empty()
    }

    /// Get all dual values.
    pub fn duals(&self) -> Option<&HashMap<ConId, f64>> {
        self.duals.as_ref()
    }

    /// Get the reduced cost for a variable (if available).
    pub fn reduced_cost(&self, var: VarId) -> Option<f64> {
        self.reduced_costs.as_ref()?.get(&var).copied()
    }

    /// Check if reduced costs are available.
    pub fn has_reduced_costs(&self) -> bool {
        self.reduced_costs.is_some() && !self.reduced_costs().unwrap().is_empty()
    }

    /// Get all reduced costs.
    pub fn reduced_costs(&self) -> Option<&HashMap<VarId, f64>> {
        self.reduced_costs.as_ref()
    }

    /// Create a lookup function for variable values.
    ///
    /// Useful for expression evaluation.
    pub fn as_var_lookup(&self) -> impl Fn(VarId) -> f64 + '_ {
        move |var| self.value_or_zero(var)
    }

    /// Return raw lower/upper violation magnitudes for an original constraint.
    /// The solution's model instance and revision are checked before any
    /// expression evaluation.
    pub fn constraint_violation(
        &self,
        model: &Model,
        constraint: ConId,
    ) -> Result<ConstraintViolation, ViolationError> {
        self.validate_model_identity(model)?;
        self.evaluate_constraint_violation(model, constraint)
    }

    /// Return a violation only when this solution carries the exact compiled
    /// state and overlay identity supplied by the solver session.
    pub fn constraint_violation_with_identity(
        &self,
        model: &Model,
        constraint: ConId,
        compilation_id: CompilationId,
        overlay_id: Option<OverlayId>,
    ) -> Result<ConstraintViolation, ViolationError> {
        self.validate_model_identity(model)?;
        self.validate_solver_identity(compilation_id, overlay_id)?;
        self.evaluate_constraint_violation(model, constraint)
    }

    fn evaluate_constraint_violation(
        &self,
        model: &Model,
        constraint: ConId,
    ) -> Result<ConstraintViolation, ViolationError> {
        let function = model
            .constraint_function(constraint)
            .map_err(|_| ViolationError::ConstraintNotFound(constraint))?;
        let lhs = match function.function {
            crate::function::ScalarFunction::Linear(expression) => {
                for term in expression.terms() {
                    let value = self
                        .value(term.var)
                        .ok_or(ViolationError::MissingVariableValue(term.var))?;
                    if !value.is_finite() {
                        return Err(ViolationError::NonFiniteVariableValue {
                            variable: term.var,
                            value,
                        });
                    }
                }
                let lhs = expression.evaluate(
                    |variable| self.value(variable).unwrap_or(0.0),
                    |parameter| model.parameter_value(parameter).unwrap_or(f64::NAN),
                );
                if !lhs.is_finite() {
                    return Err(ViolationError::NonFiniteEvaluation(lhs));
                }
                lhs
            }
        };
        let bounds = model
            .constraint_bounds(constraint)
            .ok_or(ViolationError::ConstraintNotFound(constraint))?;
        Ok(ConstraintViolation {
            lower: if bounds.lower.is_finite() {
                (bounds.lower - lhs).max(0.0)
            } else {
                0.0
            },
            upper: if bounds.upper.is_finite() {
                (lhs - bounds.upper).max(0.0)
            } else {
                0.0
            },
        })
    }

    /// Return raw violations through a persistent soft-constraint handle.
    pub fn soft_constraint_violation(
        &self,
        model: &Model,
        soft: SoftConstraint,
    ) -> Result<ConstraintViolation, ViolationError> {
        let payload = model
            .soft_constraint(soft)
            .map_err(|_| ViolationError::SoftConstraintNotFound(soft))?;
        self.constraint_violation(model, payload.original_constraint)
    }

    /// Return a persistent soft-constraint violation with exact solver
    /// compilation and overlay provenance checks.
    pub fn soft_constraint_violation_with_identity(
        &self,
        model: &Model,
        soft: SoftConstraint,
        compilation_id: CompilationId,
        overlay_id: Option<OverlayId>,
    ) -> Result<ConstraintViolation, ViolationError> {
        let payload = model
            .soft_constraint(soft)
            .map_err(|_| ViolationError::SoftConstraintNotFound(soft))?;
        self.constraint_violation_with_identity(
            model,
            payload.original_constraint,
            compilation_id,
            overlay_id,
        )
    }

    /// Return raw values plus a separate tolerance-adjusted presentation.
    pub fn constraint_violation_with_tolerance(
        &self,
        model: &Model,
        constraint: ConId,
        tolerance: f64,
    ) -> Result<ViolationPresentation, ViolationError> {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(ViolationError::InvalidTolerance(tolerance));
        }
        let raw = self.constraint_violation(model, constraint)?;
        Ok(ViolationPresentation {
            raw,
            adjusted: ConstraintViolation {
                lower: (raw.lower - tolerance).max(0.0),
                upper: (raw.upper - tolerance).max(0.0),
            },
            tolerance,
        })
    }

    fn validate_model_identity(&self, model: &Model) -> Result<(), ViolationError> {
        if self.metadata.model_instance != model.instance() {
            return Err(ViolationError::ModelInstanceMismatch {
                expected: model.instance(),
                actual: self.metadata.model_instance,
            });
        }
        if self.metadata.model_revision != model.current_revision() {
            return Err(ViolationError::StaleRevision {
                expected: model.current_revision(),
                actual: self.metadata.model_revision,
            });
        }
        Ok(())
    }

    fn validate_solver_identity(
        &self,
        compilation_id: CompilationId,
        overlay_id: Option<OverlayId>,
    ) -> Result<(), ViolationError> {
        if self.metadata.compilation_id != Some(compilation_id) {
            return Err(ViolationError::CompilationMismatch {
                expected: Some(compilation_id),
                actual: self.metadata.compilation_id,
            });
        }
        if self.metadata.overlay_id != overlay_id {
            return Err(ViolationError::OverlayMismatch {
                expected: overlay_id,
                actual: self.metadata.overlay_id,
            });
        }
        Ok(())
    }
}

/// Builder for constructing solutions.
#[derive(Clone, Debug, Default)]
pub struct SolutionBuilder {
    values: HashMap<VarId, f64>,
    objective_value: Option<f64>,
    objective_id: Option<ObjId>,
    status: SolveStatus,
    duals: Option<HashMap<ConId, f64>>,
    reduced_costs: Option<HashMap<VarId, f64>>,
    metadata: SolveMetadata,
}

impl SolutionBuilder {
    /// Create a new builder with NotSolved status.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the solver status.
    pub fn status(mut self, status: SolveStatus) -> Self {
        self.status = status;
        self
    }

    /// Set a variable value.
    pub fn value(mut self, var: VarId, value: f64) -> Self {
        self.values.insert(var, value);
        self
    }

    /// Set all variable values.
    pub fn values(mut self, values: HashMap<VarId, f64>) -> Self {
        self.values = values;
        self
    }

    /// Set the objective value.
    pub fn objective_value(mut self, value: f64) -> Self {
        self.objective_value = Some(value);
        self
    }

    /// Set which objective this solution is for.
    pub fn objective_id(mut self, obj: ObjId) -> Self {
        self.objective_id = Some(obj);
        self
    }

    /// Set a dual value for a constraint.
    pub fn dual(mut self, con: ConId, value: f64) -> Self {
        self.duals
            .get_or_insert_with(HashMap::new)
            .insert(con, value);
        self
    }

    /// Set all dual values.
    pub fn duals(mut self, duals: HashMap<ConId, f64>) -> Self {
        self.duals = Some(duals);
        self
    }

    /// Set a reduced cost for a variable.
    pub fn reduced_cost(mut self, var: VarId, value: f64) -> Self {
        self.reduced_costs
            .get_or_insert_with(HashMap::new)
            .insert(var, value);
        self
    }

    /// Set all reduced costs.
    pub fn reduced_costs(mut self, costs: HashMap<VarId, f64>) -> Self {
        self.reduced_costs = Some(costs);
        self
    }

    /// Set the solve metadata.
    pub fn metadata(mut self, metadata: SolveMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Build the solution.
    pub fn build(self) -> Solution {
        Solution {
            values: self.values,
            objective_value: self.objective_value,
            objective_id: self.objective_id,
            status: self.status,
            duals: self.duals,
            reduced_costs: self.reduced_costs,
            metadata: self.metadata,
        }
    }
}

/// Storage for multiple solutions (latest, named, etc.).
#[derive(Clone, Debug, Default)]
pub struct SolutionStore {
    /// The most recent solution.
    latest: Option<Solution>,
    /// Named solution snapshots.
    named: HashMap<String, Solution>,
}

impl SolutionStore {
    /// Create an empty solution store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a solution as the latest.
    pub fn set_latest(&mut self, solution: Solution) {
        self.latest = Some(solution);
    }

    /// Get the latest solution.
    pub fn latest(&self) -> Option<&Solution> {
        self.latest.as_ref()
    }

    /// Take the latest solution (removing it from the store).
    pub fn take_latest(&mut self) -> Option<Solution> {
        self.latest.take()
    }

    /// Save the latest solution with a name.
    pub fn save_as(&mut self, name: impl Into<String>) -> bool {
        if let Some(solution) = &self.latest {
            self.named.insert(name.into(), solution.clone());
            true
        } else {
            false
        }
    }

    /// Store a named solution.
    pub fn set_named(&mut self, name: impl Into<String>, solution: Solution) {
        self.named.insert(name.into(), solution);
    }

    /// Get a named solution.
    pub fn get_named(&self, name: &str) -> Option<&Solution> {
        self.named.get(name)
    }

    /// Remove a named solution.
    pub fn remove_named(&mut self, name: &str) -> Option<Solution> {
        self.named.remove(name)
    }

    /// List all named solution names.
    pub fn named_solutions(&self) -> impl Iterator<Item = &str> {
        self.named.keys().map(|s| s.as_str())
    }

    /// Clear all solutions.
    pub fn clear(&mut self) {
        self.latest = None;
        self.named.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::ConstraintExprExt;
    use crate::id::Generation;
    use crate::model::{continuous, Model};
    use crate::SolverStatus;

    fn make_var(index: u32) -> VarId {
        VarId::new(index, Generation::new())
    }

    fn make_con(index: u32) -> ConId {
        ConId::new(index, Generation::new())
    }

    #[test]
    fn solution_builder() {
        let x = make_var(0);
        let y = make_var(1);

        let solution = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 1.0)
            .value(y, 2.0)
            .objective_value(10.0)
            .build();

        assert!(solution.is_optimal());
        assert_eq!(solution.value(x), Some(1.0));
        assert_eq!(solution.value(y), Some(2.0));
        assert_eq!(solution.objective_value(), Some(10.0));
    }

    #[test]
    fn solution_store() {
        let x = make_var(0);

        let mut store = SolutionStore::new();

        let sol1 = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 1.0)
            .build();

        store.set_latest(sol1);
        assert!(store.latest().is_some());

        store.save_as("first");
        assert!(store.get_named("first").is_some());

        let sol2 = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 2.0)
            .build();

        store.set_latest(sol2);
        assert_eq!(store.latest().unwrap().value(x), Some(2.0));
        assert_eq!(store.get_named("first").unwrap().value(x), Some(1.0));
    }

    #[test]
    fn var_lookup() {
        let x = make_var(0);
        let y = make_var(1);
        let z = make_var(2);

        let solution = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 1.0)
            .value(y, 2.0)
            .build();

        let lookup = solution.as_var_lookup();
        assert_eq!(lookup(x), 1.0);
        assert_eq!(lookup(y), 2.0);
        assert_eq!(lookup(z), 0.0); // Not in solution, defaults to 0
    }

    #[test]
    fn duals_and_reduced_costs() {
        let x = make_var(0);
        let c = make_con(0);

        let solution = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 1.0)
            .dual(c, 0.5)
            .reduced_cost(x, 0.0)
            .build();

        assert!(solution.has_duals());
        assert_eq!(solution.dual(c), Some(0.5));
        assert!(solution.has_reduced_costs());
        assert_eq!(solution.reduced_cost(x), Some(0.0));
    }

    #[test]
    fn solution_constructors_and_accessors() {
        let x = make_var(0);
        let y = make_var(1);

        // Solution::new(status) — empty, non-optimal.
        let s = Solution::new(SolverStatus::Infeasible);
        assert_eq!(s.status(), SolverStatus::Infeasible);
        assert!(!s.is_optimal());
        assert!(!s.has_values());
        assert!(s.values().is_empty());
        assert_eq!(s.objective_value(), None);
        assert_eq!(s.objective_id(), None);

        // Solution::from_values preserves the value map and status.
        let mut map = HashMap::new();
        map.insert(x, 1.0);
        map.insert(y, 2.0);
        let s2 = Solution::from_values(map.clone(), SolverStatus::Optimal);
        assert!(s2.is_optimal());
        assert!(s2.has_values());
        assert_eq!(s2.values(), &map);
        assert_eq!(s2.value(x), Some(1.0));
        assert_eq!(s2.value(y), Some(2.0));
    }

    /// F5: a synthetic `Solution::from_values` (no real solve) carries
    /// `compilation_id == None` — it must not fabricate a compilation identity.
    #[test]
    fn synthetic_solution_has_no_compilation_id() {
        let mut map = HashMap::new();
        map.insert(make_var(0), 1.0);
        let s = Solution::from_values(map, SolverStatus::Optimal);
        assert_eq!(s.metadata().compilation_id, None);
    }

    #[test]
    fn constraint_violation_rejects_missing_candidate_value() {
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
        let constraint = model.add_constraint(x.ge(1.0)).unwrap();
        let solution = SolutionBuilder::new()
            .metadata(SolveMetadata {
                model_instance: model.instance(),
                model_revision: model.current_revision(),
                ..SolveMetadata::default()
            })
            .build();

        assert!(matches!(
            solution.constraint_violation(&model, constraint),
            Err(ViolationError::MissingVariableValue(variable)) if variable == x
        ));
    }

    #[test]
    fn constraint_violation_rejects_nonfinite_candidate_value() {
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
        let constraint = model.add_constraint(x.ge(1.0)).unwrap();
        let solution = SolutionBuilder::new()
            .value(x, f64::NAN)
            .metadata(SolveMetadata {
                model_instance: model.instance(),
                model_revision: model.current_revision(),
                ..SolveMetadata::default()
            })
            .build();

        assert!(matches!(
            solution.constraint_violation(&model, constraint),
            Err(ViolationError::NonFiniteVariableValue { variable, .. }) if variable == x
        ));
    }

    #[test]
    fn solver_violation_accessor_requires_exact_compilation_and_overlay_identity() {
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
        let constraint = model.add_constraint(x.ge(1.0)).unwrap();
        let compilation_id = CompilationId::allocate().unwrap();
        let overlay_id = OverlayId::allocate().unwrap();
        let solution = SolutionBuilder::new()
            .value(x, 1.0)
            .metadata(SolveMetadata {
                model_instance: model.instance(),
                model_revision: model.current_revision(),
                compilation_id: Some(compilation_id),
                overlay_id: Some(overlay_id),
                ..SolveMetadata::default()
            })
            .build();

        assert!(matches!(
            solution.constraint_violation_with_identity(
                &model,
                constraint,
                CompilationId::allocate().unwrap(),
                Some(overlay_id),
            ),
            Err(ViolationError::CompilationMismatch { .. })
        ));
        assert!(matches!(
            solution.constraint_violation_with_identity(
                &model,
                constraint,
                compilation_id,
                Some(OverlayId::allocate().unwrap()),
            ),
            Err(ViolationError::OverlayMismatch { .. })
        ));
        assert_eq!(
            solution
                .constraint_violation_with_identity(
                    &model,
                    constraint,
                    compilation_id,
                    Some(overlay_id),
                )
                .unwrap()
                .total(),
            0.0
        );
    }

    #[test]
    fn builder_values_replaces_and_objective_id() {
        let x = make_var(0);
        let o = ObjId::new(0, Generation::new());

        let mut map = HashMap::new();
        map.insert(x, 1.0);
        let sol = SolutionBuilder::new()
            .value(x, 99.0) // overwritten by .values(map)
            .values(map.clone())
            .objective_id(o)
            .build();
        assert_eq!(sol.values(), &map);
        assert_eq!(sol.objective_id(), Some(o));

        // objective_id is None when never set.
        let sol2 = SolutionBuilder::new().build();
        assert_eq!(sol2.objective_id(), None);
    }

    #[test]
    fn builder_dual_and_reduced_cost_maps() {
        let x = make_var(0);
        let c = make_con(0);

        // Per-entry setters (lazy-init map insertion).
        let sol = SolutionBuilder::new()
            .dual(c, 0.5)
            .reduced_cost(x, 0.25)
            .build();
        assert!(sol.has_duals());
        assert_eq!(sol.dual(c), Some(0.5));
        assert!(sol.has_reduced_costs());
        assert_eq!(sol.reduced_cost(x), Some(0.25));

        // Whole-map setters round-trip through build().
        let mut dmap = HashMap::new();
        dmap.insert(c, 1.5);
        let mut rmap = HashMap::new();
        rmap.insert(x, 2.5);
        let sol2 = SolutionBuilder::new()
            .duals(dmap.clone())
            .reduced_costs(rmap.clone())
            .build();
        assert_eq!(sol2.duals(), Some(&dmap));
        assert_eq!(sol2.reduced_costs(), Some(&rmap));
        assert_eq!(sol2.dual(c), Some(1.5));
        assert_eq!(sol2.reduced_cost(x), Some(2.5));
    }

    #[test]
    fn solution_store_lifecycle() {
        let x = make_var(0);
        let mut store = SolutionStore::new();

        // save_as with no latest solution returns false.
        assert!(!store.save_as("a"));
        assert!(store.get_named("a").is_none());

        let sol = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 1.0)
            .build();
        store.set_latest(sol);
        assert!(store.save_as("a"));
        assert!(store.get_named("a").is_some());

        // set_named overwrites an existing name.
        let sol2 = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 2.0)
            .build();
        store.set_named("a", sol2);
        assert_eq!(store.get_named("a").unwrap().value(x), Some(2.0));

        // remove_named returns the stored solution and drops the entry.
        let removed = store.remove_named("a");
        assert!(removed.is_some());
        assert!(store.get_named("a").is_none());

        // named_solutions yields stored names; clear empties everything.
        store.set_named("b", SolutionBuilder::new().build());
        store.set_named("c", SolutionBuilder::new().build());
        let names: Vec<&str> = store.named_solutions().collect();
        assert!(names.contains(&"b"));
        assert!(names.contains(&"c"));
        store.clear();
        assert!(store.latest().is_none());
        assert_eq!(store.named_solutions().count(), 0);

        // take_latest removes and returns the latest solution.
        store.set_latest(
            SolutionBuilder::new()
                .status(SolverStatus::Optimal)
                .value(x, 3.0)
                .build(),
        );
        let taken = store.take_latest();
        assert_eq!(taken.unwrap().value(x), Some(3.0));
        assert!(store.latest().is_none());
        assert!(store.take_latest().is_none());
    }
}
