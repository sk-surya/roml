//! Identity compiler — canonical snapshots/deltas into backend IR
//! (design §8, §18, §19; P26 Task 7).
//!
//! [`CompilationSession`] is the per-solver identity compiler: it lowers a
//! canonical [`ModelSnapshot`] one-to-one into a [`BackendSnapshot`] (dense
//! compiled ids distinct from user handles — SM-02.4), and a primitive linear
//! [`DeltaBatch`] into a [`BackendDeltaBatch`] with exact from/to
//! [`CompilationId`]s (B2, D28). Any delta op the identity compiler cannot
//! prove incrementally equivalent selects a deterministic rebuild (design
//! §18, D22; the Task 0 acceptance record F-B1): no compiled delta is
//! emitted.
//!
//! # A31-aware delta consumption
//!
//! [`DeltaBatch.functions`](DeltaBatch::functions) is the view of constraints
//! **added** by a batch with final folded bounds, minus removed constraints.
//! Updates to pre-existing functions ride the ops
//! (`SetCell`/`SetConstraintBounds`/`RemoveCell`). This compiler consumes the
//! ops for updates and the ops' carried evaluated values for added entities —
//! it never treats `functions` as exhaustive for pre-existing constraints.
//!
//! # Origin completeness
//!
//! Snapshot compilation records every compiled entity's [`EntityOrigin`] in
//! the snapshot's [`OriginMap`] (D5, SM-02.5). Delta compilation records the
//! origins of entities **added** by the batch in
//! [`BackendDeltaBatch.origin_additions`](BackendDeltaBatch::origin_additions),
//! so the target compiled state remains origin-complete.

use std::collections::HashMap;

use crate::compiler::backend_ir::{
    BackendDeltaBatch, BackendOp, BackendSnapshot, BackendSnapshotBuilder, CompilationId,
    CompiledConstraintId, CompiledLinearRow, CompiledObjective, CompiledObjectiveId,
    CompiledObjectivePolicy, CompiledVariable, CompiledVariableId, RecipeFingerprint,
};
use crate::compiler::capability::{BackendCapabilitySet, BackendFeature, CompilationPolicy};
use crate::compiler::origin::{EntityOrigin, OriginMap};
use crate::compiler::CompileError;
use crate::delta::{DeltaBatch, ModelOp};
use crate::id::{ConId, ObjId, VarId};
use crate::identity::ModelInstanceId;
use crate::model::coefficient::CoefficientTarget;
use crate::model::{Bounds, ConstraintBounds, VarType};
use crate::revision::ModelRevision;
use crate::snapshot::ModelSnapshot;

/// Per-family checked atomic counter state for one compiled state.
#[derive(Clone)]
struct CurrentCompilation {
    /// Exact compiled id of the current backend state.
    compilation_id: CompilationId,
    /// Canonical revision of the current backend state.
    revision: ModelRevision,
    /// User variable -> compiled variable id.
    variable_ids: HashMap<VarId, CompiledVariableId>,
    /// User variable -> activity (WR-02). `SetVariableActive` is not
    /// incrementally compilable (it forces `RebuildRequired`), so activity is
    /// FIXED between snapshot rebuilds — tracking it lets `compile_delta` fold
    /// activity identically for bounds/fixing ops instead of silently diverging
    /// from `compile_snapshot`'s `[0,0]` fold for an inactive variable.
    variable_activity: HashMap<VarId, bool>,
    /// User constraint -> compiled row id.
    row_ids: HashMap<ConId, CompiledConstraintId>,
    /// User objective -> compiled objective id.
    objective_ids: HashMap<ObjId, CompiledObjectiveId>,
    /// Compiled variable id -> user variable.
    compiled_to_variable: HashMap<CompiledVariableId, VarId>,
    /// Compiled row id -> user constraint.
    compiled_to_row: HashMap<CompiledConstraintId, ConId>,
    /// Compiled objective id -> user objective.
    compiled_to_objective: HashMap<CompiledObjectiveId, ObjId>,
    /// Compiled objective id -> evaluated coefficient vector (WR-03). The
    /// authoritative coefficients the backend holds; the overlay compiler
    /// resolves temporary rows from these instead of re-deriving from the
    /// canonical symbolic expression.
    compiled_objective_coefficients: HashMap<CompiledObjectiveId, Vec<(CompiledVariableId, f64)>>,
    /// Compiled objective id -> evaluated constant (WR-03).
    compiled_objective_constants: HashMap<CompiledObjectiveId, f64>,
    /// Next dense compiled variable index.
    next_variable_index: u32,
    /// Next dense compiled row index.
    next_row_index: u32,
    /// Next dense compiled objective index.
    next_objective_index: u32,
    /// The active compiled objective policy of the current compiled state
    /// (A32). Tracked so a `RemoveObjective` of the currently-active objective
    /// can emit the `SetObjectivePolicy(None)` transition at the compile
    /// boundary — the batch must be self-contained (A31, CR-02).
    objective_policy: CompiledObjectivePolicy,
}

/// The identity compiler for one solver/backend (design §8).
///
/// A `CompilationSession` tracks the exact compiled state it has produced so
/// that `compile_delta` can (a) allocate dense compiled ids for added
/// entities and (b) reject stale from-compilations (D28). The facade owns one
/// session per backend.
#[derive(Default)]
pub struct CompilationSession {
    source_instance: Option<ModelInstanceId>,
    current: Option<CurrentCompilation>,
}

/// SM-04.4 / F3(d): require `feature` be available for compilation under
/// `policy`.
///
/// `Auto` accepts exact native support (the only path in P26 — no portable
/// bridges exist); `NativeRequired` rejects a non-native feature; `Portable`
/// requires a portable bridge formulation, which does not exist in P26
/// (bridge declarations land with P32), so it rejects a non-native feature
/// too. An unqualified feature is a typed `CompileError::UnsupportedFeature`,
/// never silently ignored. For `Auto` the payload is the exact feature name
/// (e.g. `"IncrementalBounds"`); the policy variants carry a descriptive
/// message.
fn require_feature(
    capabilities: &BackendCapabilitySet,
    policy: &CompilationPolicy,
    feature: BackendFeature,
    context: &str,
) -> Result<(), CompileError> {
    if capabilities.supports(feature) {
        return Ok(());
    }
    let feature_name = format!("{feature:?}");
    let message = match policy {
        CompilationPolicy::Auto => feature_name,
        CompilationPolicy::NativeRequired => format!(
            "{feature_name} requires exact native support which this backend lacks \
             ({context}; NativeRequired policy)"
        ),
        CompilationPolicy::Portable => format!(
            "{feature_name} has no portable bridge in P26 ({context}; Portable policy) \
             — bridges land with P32"
        ),
    };
    Err(CompileError::UnsupportedFeature(message))
}

impl CompilationSession {
    /// Create a fresh identity compiler with no compiled state.
    pub fn new() -> Self {
        Self::default()
    }

    /// The exact compiled id of the most recently compiled state, if any.
    pub fn current_compilation(&self) -> Option<CompilationId> {
        self.current.as_ref().map(|c| c.compilation_id)
    }

    /// The model instance the compiled base belongs to, if any (P27 Task 9
    /// overlay compiler: a stale/cross-model base is rejected before any
    /// overlay op).
    pub(crate) fn source_instance(&self) -> Option<ModelInstanceId> {
        self.source_instance
    }

    /// Resolve a user variable to its compiled variable id in the current
    /// compiled base (P27 Task 9 overlay compiler; additive, no behavior
    /// change).
    pub(crate) fn compiled_variable_id(&self, v: VarId) -> Option<CompiledVariableId> {
        self.current
            .as_ref()
            .and_then(|c| c.variable_ids.get(&v).copied())
    }

    /// Resolve a user objective to its compiled objective id in the current
    /// compiled base (P27 Task 9 overlay compiler; additive, no behavior
    /// change).
    pub(crate) fn compiled_objective_id(&self, o: ObjId) -> Option<CompiledObjectiveId> {
        self.current
            .as_ref()
            .and_then(|c| c.objective_ids.get(&o).copied())
    }

    /// The compiled objective's evaluated coefficient vector and constant from
    /// the compiled base (WR-03; P27 Task 9 overlay compiler). `None` only when
    /// the objective is genuinely absent from the compiled base — the overlay
    /// compiler resolves temporary rows from these authoritative evaluated
    /// values, never by re-deriving from the canonical symbolic expression
    /// (`as_constant()` returns `None` for parameterized/composite
    /// coefficients).
    pub(crate) fn compiled_objective_terms(
        &self,
        o: ObjId,
    ) -> Option<(Vec<(CompiledVariableId, f64)>, f64)> {
        let current = self.current.as_ref()?;
        let cid = *current.objective_ids.get(&o)?;
        let coefficients = current.compiled_objective_coefficients.get(&cid)?.clone();
        let constant = *current.compiled_objective_constants.get(&cid)?;
        Some((coefficients, constant))
    }

    /// The next dense compiled row index in the current compiled base (the
    /// starting point for overlay temporary-row ids; additive, no behavior
    /// change).
    pub(crate) fn next_row_index(&self) -> Option<u32> {
        self.current.as_ref().map(|c| c.next_row_index)
    }

    /// The user variable for a compiled variable id, if known.
    pub fn user_variable(&self, compiled: CompiledVariableId) -> Option<VarId> {
        self.current
            .as_ref()
            .and_then(|c| c.compiled_to_variable.get(&compiled).copied())
    }

    /// The user constraint for a compiled row id, if known.
    pub fn user_constraint(&self, compiled: CompiledConstraintId) -> Option<ConId> {
        self.current
            .as_ref()
            .and_then(|c| c.compiled_to_row.get(&compiled).copied())
    }

    /// The user objective for a compiled objective id, if known.
    pub fn user_objective(&self, compiled: CompiledObjectiveId) -> Option<ObjId> {
        self.current
            .as_ref()
            .and_then(|c| c.compiled_to_objective.get(&compiled).copied())
    }

    /// Compile a canonical snapshot one-to-one into a [`BackendSnapshot`].
    ///
    /// Every variable -> one `CompiledVariable`, every constraint -> one
    /// `CompiledLinearRow`, every objective -> one `CompiledObjective`, all
    /// with dense deterministic compiled ids (SM-02.4). The active objective
    /// compiles into [`CompiledObjectivePolicy::Single`]; a snapshot with no
    /// active objective compiles to [`CompiledObjectivePolicy::None`] (A32 /
    /// the Task 0 acceptance record B1). Inactive variables/rows fold their
    /// activity into bounds (`[0,0]` / unbounded), matching the M2 projection
    /// semantics.
    ///
    /// # Capability gating (SM-04.4)
    ///
    /// A snapshot containing integer/binary variables is rejected with
    /// [`CompileError::UnsupportedFeature`] against a backend that does not
    /// declare `BackendFeature::Mip`.
    pub fn compile_snapshot(
        &mut self,
        source_instance: ModelInstanceId,
        snapshot: &ModelSnapshot,
        policy: &CompilationPolicy,
        capabilities: &BackendCapabilitySet,
    ) -> Result<BackendSnapshot, CompileError> {
        // WR-4 (D28): the session's compiled base belongs to ONE model instance.
        // Reusing the session across a DIFFERENT model must be rejected, never
        // silently miscompiled against the recorded instance's dense ids.
        if let Some(recorded) = self.source_instance {
            if recorded != source_instance {
                return Err(CompileError::RebuildRequired(format!(
                    "source instance {source_instance:?} != recorded compiled-base \
                     instance {recorded:?}; cross-model session reuse requires a rebuild"
                )));
            }
        }

        // SM-04.4 / F3: an unqualified feature is rejected, never silently
        // ignored. `Lp` is always required (a backend without LP cannot solve
        // any snapshot); `Mip` is required when integers are present. The
        // `CompilationPolicy` is enforced by `require_feature` (no longer
        // ignored — F3(d)).
        require_feature(
            capabilities,
            policy,
            BackendFeature::Lp,
            "linear snapshot compilation",
        )?;
        let has_integer = snapshot
            .variables
            .iter()
            .any(|v| !matches!(v.var_type, VarType::Continuous));
        if has_integer {
            require_feature(
                capabilities,
                policy,
                BackendFeature::Mip,
                "integer/binary variables",
            )?;
        }

        // The compiled IR has no semi-continuous representation (the compiled
        // variable carries bounds + type only), so a snapshot with a
        // semi-continuous variable cannot be compiled faithfully. Reject it
        // (M1R-H7 preserved at the compile boundary) rather than silently
        // dropping the flag.
        if snapshot
            .variables
            .iter()
            .any(|v| v.semicontinuous_lower.is_some())
        {
            return Err(CompileError::UnsupportedFeature(
                "semi-continuous variables".into(),
            ));
        }

        // ── Variables ────────────────────────────────────────────────────
        let mut origin_map = OriginMap::new();
        let mut variables = Vec::with_capacity(snapshot.variables.len());
        let mut variable_ids = HashMap::new();
        let mut variable_activity = HashMap::new();
        let mut compiled_to_variable = HashMap::new();
        for (index, v) in snapshot.variables.iter().enumerate() {
            let id = CompiledVariableId(index as u32);
            // SM-05.3: the compiled representation of a fixing is equal
            // lower/upper bounds — a fixed variable compiles with effective
            // bounds `[value, value]` (D6: fixing compiles as bound
            // tightening). `VariableEntry.bounds` is the DECLARED view;
            // `fixing` folds on top. Activity is then folded into bounds:
            // inactive -> fixed [0, 0].
            let effective = match &v.fixing {
                Some(fixing) => Bounds::new(fixing.value, fixing.value),
                None => v.bounds,
            };
            let bounds = if v.active {
                effective
            } else {
                Bounds::new(0.0, 0.0)
            };
            variables.push(CompiledVariable {
                id,
                bounds,
                var_type: v.var_type,
                name: None,
            });
            variable_ids.insert(v.id, id);
            variable_activity.insert(v.id, v.active);
            compiled_to_variable.insert(id, v.id);
            origin_map.insert_variable(id, EntityOrigin::UserVariable(v.id));
        }

        // ── Rows ─────────────────────────────────────────────────────────
        let mut rows = Vec::with_capacity(snapshot.constraints.len());
        let mut row_ids = HashMap::new();
        let mut compiled_to_row = HashMap::new();
        for (index, c) in snapshot.constraints.iter().enumerate() {
            let id = CompiledConstraintId(index as u32);
            // Activity is folded into bounds: inactive -> unbounded.
            let bounds = if c.active {
                c.bounds
            } else {
                ConstraintBounds::range(f64::NEG_INFINITY, f64::INFINITY)
            };
            let coefficients: Vec<(CompiledVariableId, f64)> = snapshot
                .cells
                .iter()
                .filter_map(|cell| match cell.cell_key.0 {
                    CoefficientTarget::Constraint(con) if con == c.id => variable_ids
                        .get(&cell.cell_key.1)
                        .map(|&vid| (vid, cell.evaluated_value)),
                    _ => None,
                })
                .collect();
            rows.push(CompiledLinearRow {
                id,
                bounds,
                coefficients,
                name: None,
            });
            row_ids.insert(c.id, id);
            compiled_to_row.insert(id, c.id);
            origin_map.insert_constraint(id, EntityOrigin::UserConstraint(c.id));
        }

        // ── Objectives ───────────────────────────────────────────────────
        let mut objectives = Vec::with_capacity(snapshot.objectives.len());
        let mut objective_ids = HashMap::new();
        let mut compiled_to_objective = HashMap::new();
        let mut compiled_objective_coefficients = HashMap::new();
        let mut compiled_objective_constants = HashMap::new();
        for (index, o) in snapshot.objectives.iter().enumerate() {
            let id = CompiledObjectiveId(index as u32);
            let coefficients: Vec<(CompiledVariableId, f64)> = snapshot
                .cells
                .iter()
                .filter_map(|cell| match cell.cell_key.0 {
                    CoefficientTarget::Objective(obj) if obj == o.id => variable_ids
                        .get(&cell.cell_key.1)
                        .map(|&vid| (vid, cell.evaluated_value)),
                    _ => None,
                })
                .collect();
            // WR-03: record the evaluated coefficients/constant of the compiled
            // objective (the authoritative values the backend holds) so the
            // overlay compiler can resolve temporary rows without re-deriving
            // from the canonical symbolic expression.
            compiled_objective_coefficients.insert(id, coefficients.clone());
            compiled_objective_constants.insert(id, o.constant);
            objectives.push(CompiledObjective {
                id,
                sense: o.sense,
                coefficients,
                constant: o.constant,
                name: None,
            });
            objective_ids.insert(o.id, id);
            compiled_to_objective.insert(id, o.id);
            origin_map.insert_objective(id, EntityOrigin::UserObjective(o.id));
        }

        // ── Objective policy (A32) ───────────────────────────────────────
        let objective_policy = snapshot
            .objectives
            .iter()
            .find(|o| o.active)
            .and_then(|o| objective_ids.get(&o.id).copied())
            .map(CompiledObjectivePolicy::Single)
            .unwrap_or(CompiledObjectivePolicy::None);

        let mut builder = BackendSnapshotBuilder::new(source_instance, snapshot.revision)
            .origin_map(origin_map)
            .objective_policy(objective_policy.clone());
        for v in variables {
            builder = builder.add_variable(v);
        }
        for r in rows {
            builder = builder.add_linear_row(r);
        }
        for o in objectives {
            builder = builder.add_objective(o);
        }
        let compiled = builder.finalize()?;

        self.source_instance = Some(source_instance);
        self.current = Some(CurrentCompilation {
            compilation_id: compiled.compilation_id,
            revision: compiled.source_revision,
            variable_ids,
            variable_activity,
            row_ids,
            objective_ids,
            compiled_to_variable,
            compiled_to_row,
            compiled_to_objective,
            compiled_objective_coefficients,
            compiled_objective_constants,
            next_variable_index: compiled.variables.len() as u32,
            next_row_index: compiled.linear_rows.len() as u32,
            next_objective_index: compiled.objectives.len() as u32,
            objective_policy,
        });

        Ok(compiled)
    }

    /// Compile a canonical delta into a [`BackendDeltaBatch`].
    ///
    /// The caller passes the exact `from_compilation` the batch must apply on
    /// top of (the backend's current compiled state id) and the incoming
    /// model's `source_instance`. Every emitted batch carries exact from/to
    /// compilation ids and revisions (B2), and allocates a fresh
    /// `CompilationId` for the target state (D28).
    ///
    /// # Source-instance guard (WR-4, D28)
    ///
    /// The compiled base belongs to ONE model instance. A delta whose
    /// `source_instance` differs from the recorded one is rejected with
    /// [`CompileError::RebuildRequired`], so a `SolverSession` reused across two
    /// different models can never silently compile the second model's deltas
    /// against the first model's compiled base (cross-model session reuse must
    /// not miscompile).
    ///
    /// Any op the identity compiler cannot prove incrementally equivalent
    /// returns [`CompileError::RebuildRequired`] and **no** `BackendDeltaBatch`
    /// is emitted (design §18, D22): the caller performs one deterministic
    /// snapshot rebuild instead.
    pub fn compile_delta(
        &mut self,
        delta: &DeltaBatch,
        from_compilation: CompilationId,
        source_instance: ModelInstanceId,
        policy: &CompilationPolicy,
        capabilities: &BackendCapabilitySet,
    ) -> Result<BackendDeltaBatch, CompileError> {
        // WR-4 (D28): the session's compiled base belongs to ONE model instance.
        // Reusing the session across a DIFFERENT model must be rejected, never
        // silently miscompiled against the recorded instance's dense ids.
        if let Some(recorded) = self.source_instance {
            if recorded != source_instance {
                return Err(CompileError::RebuildRequired(format!(
                    "source instance {source_instance:?} != recorded compiled-base \
                     instance {recorded:?}; cross-model session reuse requires a rebuild"
                )));
            }
        }

        let current = self
            .current
            .as_ref()
            .ok_or_else(|| CompileError::RebuildRequired("no compiled base state".into()))?;

        if current.compilation_id != from_compilation {
            return Err(CompileError::StaleCompilation {
                expected: from_compilation,
                actual: current.compilation_id,
            });
        }
        if current.revision != delta.from {
            return Err(CompileError::RebuildRequired(format!(
                "delta from revision {} != compiled base revision {}",
                delta.from, current.revision
            )));
        }

        // Work on a copy; the session's current state is committed only on
        // full success so a failed delta never advances the compiler.
        let mut w = current.clone();
        let mut operations: Vec<BackendOp> = Vec::new();
        let mut origin_additions = OriginMap::new();

        for op in &delta.operations {
            match op {
                ModelOp::AddVariable {
                    var,
                    bounds,
                    var_type,
                } => {
                    require_feature(
                        capabilities,
                        policy,
                        BackendFeature::IncrementalRows,
                        "incremental variable addition",
                    )?;
                    let id = CompiledVariableId(w.next_variable_index);
                    w.next_variable_index += 1;
                    operations.push(BackendOp::AddVariable(CompiledVariable {
                        id,
                        bounds: *bounds,
                        var_type: *var_type,
                        name: None,
                    }));
                    w.variable_ids.insert(*var, id);
                    // WR-02: a newly added variable is active (the canonical
                    // `add_variable` has no inactive state).
                    w.variable_activity.insert(*var, true);
                    w.compiled_to_variable.insert(id, *var);
                    origin_additions.insert_variable(id, EntityOrigin::UserVariable(*var));
                }

                ModelOp::RemoveVariable { var } => {
                    let id = *w.variable_ids.get(var).ok_or_else(|| {
                        CompileError::RebuildRequired(format!(
                            "RemoveVariable for unknown compiled variable ({var:?})"
                        ))
                    })?;
                    operations.push(BackendOp::RemoveVariable(id));
                    w.variable_ids.remove(var);
                    w.compiled_to_variable.remove(&id);
                }

                ModelOp::SetVariableBounds { var, bounds } => {
                    // SM-04.4 (WR-3): an unqualified feature is rejected, never
                    // silently compiled.
                    require_feature(
                        capabilities,
                        policy,
                        BackendFeature::IncrementalBounds,
                        "incremental variable bounds",
                    )?;
                    let id = *w.variable_ids.get(var).ok_or_else(|| {
                        CompileError::RebuildRequired(format!(
                            "SetVariableBounds for unknown compiled variable ({var:?})"
                        ))
                    })?;
                    // WR-02: a bounds change on an INACTIVE variable must not
                    // silently diverge from `compile_snapshot` (which folds the
                    // inactive variable to `[0,0]`). The delta carries the raw
                    // declared bounds; force a rebuild so the snapshot fold
                    // wins deterministically.
                    if w.variable_activity.get(var) == Some(&false) {
                        return Err(CompileError::RebuildRequired(format!(
                            "SetVariableBounds on inactive variable ({var:?}) must fold to [0, 0]; \
                             rebuild required"
                        )));
                    }
                    operations.push(BackendOp::SetVariableBounds {
                        variable: id,
                        bounds: *bounds,
                    });
                }

                // P27 Task 8 (SM-05.7): a persistent fixing change IS an
                // effective-bound delta. The op is self-contained — it carries
                // the effective bounds to apply (equal `[value, value]` for a
                // fix; the current declared bounds for an unfix). Under
                // `IncrementalBounds` it lowers to `BackendOp::SetVariableBounds`
                // (D6: fixing compiles as bound tightening). A backend without
                // `IncrementalBounds` selects a deterministic rebuild (D22).
                ModelOp::SetVariableFixing {
                    var,
                    fixing: _,
                    effective_bounds,
                } => {
                    require_feature(
                        capabilities,
                        policy,
                        BackendFeature::IncrementalBounds,
                        "incremental variable fixing (effective bounds)",
                    )?;
                    let id = *w.variable_ids.get(var).ok_or_else(|| {
                        CompileError::RebuildRequired(format!(
                            "SetVariableFixing for unknown compiled variable ({var:?})"
                        ))
                    })?;
                    // WR-02: a fixing change on an INACTIVE variable must fold
                    // identically to `compile_snapshot`'s `[0,0]`. The delta
                    // carries the raw effective bounds (e.g. the declared
                    // `[0,10]` after an unfix), which would diverge from the
                    // rebuild; force a deterministic snapshot rebuild instead.
                    if w.variable_activity.get(var) == Some(&false) {
                        return Err(CompileError::RebuildRequired(format!(
                            "SetVariableFixing on inactive variable ({var:?}) must fold to [0, 0]; \
                             rebuild required"
                        )));
                    }
                    operations.push(BackendOp::SetVariableBounds {
                        variable: id,
                        bounds: *effective_bounds,
                    });
                }

                // F-B1: these ops have no backend-IR equivalent; uncertainty
                // selects rebuild (design §18).
                ModelOp::SetVariableActive { .. } => {
                    return Err(CompileError::RebuildRequired(
                        "SetVariableActive is not incrementally compilable".into(),
                    ));
                }
                ModelOp::SetVariableType { .. } => {
                    return Err(CompileError::RebuildRequired(
                        "SetVariableType is not incrementally compilable".into(),
                    ));
                }

                ModelOp::AddConstraint { con, bounds } => {
                    require_feature(
                        capabilities,
                        policy,
                        BackendFeature::IncrementalRows,
                        "incremental constraint addition",
                    )?;
                    let id = CompiledConstraintId(w.next_row_index);
                    w.next_row_index += 1;
                    operations.push(BackendOp::AddLinearRow(CompiledLinearRow {
                        id,
                        bounds: *bounds,
                        coefficients: Vec::new(),
                        name: None,
                    }));
                    w.row_ids.insert(*con, id);
                    w.compiled_to_row.insert(id, *con);
                    origin_additions.insert_constraint(id, EntityOrigin::UserConstraint(*con));
                }

                ModelOp::RemoveConstraint { con } => {
                    let id = *w.row_ids.get(con).ok_or_else(|| {
                        CompileError::RebuildRequired(format!(
                            "RemoveConstraint for unknown compiled row ({con:?})"
                        ))
                    })?;
                    operations.push(BackendOp::RemoveLinearRow(id));
                    w.row_ids.remove(con);
                    w.compiled_to_row.remove(&id);
                }

                ModelOp::SetConstraintBounds { con, bounds } => {
                    // SM-04.4 (WR-3): an unqualified feature is rejected, never
                    // silently compiled.
                    require_feature(
                        capabilities,
                        policy,
                        BackendFeature::IncrementalBounds,
                        "incremental constraint bounds",
                    )?;
                    let id = *w.row_ids.get(con).ok_or_else(|| {
                        CompileError::RebuildRequired(format!(
                            "SetConstraintBounds for unknown compiled row ({con:?})"
                        ))
                    })?;
                    operations.push(BackendOp::SetLinearRowBounds {
                        constraint: id,
                        bounds: *bounds,
                    });
                }

                ModelOp::SetConstraintActive { .. } => {
                    return Err(CompileError::RebuildRequired(
                        "SetConstraintActive is not incrementally compilable".into(),
                    ));
                }

                // A31: updates to pre-existing functions ride the ops. The
                // cell's evaluated value at the batch's `to` revision is the
                // exact coefficient to apply (SM-01.1).
                ModelOp::SetCell {
                    cell_key,
                    evaluated_value,
                    ..
                } => {
                    // SM-04.4 (WR-3): coefficient changes gate on
                    // `IncrementalCoefficients`, never silently compiled.
                    require_feature(
                        capabilities,
                        policy,
                        BackendFeature::IncrementalCoefficients,
                        "coefficient cell changes",
                    )?;
                    let (target, var) = *cell_key;
                    let vid = *w.variable_ids.get(&var).ok_or_else(|| {
                        CompileError::RebuildRequired(format!(
                            "SetCell for unknown compiled variable ({var:?})"
                        ))
                    })?;
                    match target {
                        CoefficientTarget::Constraint(con) => {
                            let rid = *w.row_ids.get(&con).ok_or_else(|| {
                                CompileError::RebuildRequired(format!(
                                    "SetCell for unknown compiled row ({con:?})"
                                ))
                            })?;
                            operations.push(BackendOp::SetLinearCoefficient {
                                constraint: rid,
                                variable: vid,
                                value: *evaluated_value,
                            });
                        }
                        CoefficientTarget::Objective(obj) => {
                            let oid = *w.objective_ids.get(&obj).ok_or_else(|| {
                                CompileError::RebuildRequired(format!(
                                    "SetCell for unknown compiled objective ({obj:?})"
                                ))
                            })?;
                            operations.push(BackendOp::SetObjectiveCoefficient {
                                objective: oid,
                                variable: vid,
                                value: *evaluated_value,
                            });
                        }
                    }
                }

                ModelOp::RemoveCell { cell_key } => {
                    // SM-04.4 (WR-3): coefficient changes gate on
                    // `IncrementalCoefficients`, never silently compiled.
                    require_feature(
                        capabilities,
                        policy,
                        BackendFeature::IncrementalCoefficients,
                        "coefficient cell removal",
                    )?;
                    let (target, var) = *cell_key;
                    let vid = *w.variable_ids.get(&var).ok_or_else(|| {
                        CompileError::RebuildRequired(format!(
                            "RemoveCell for unknown compiled variable ({var:?})"
                        ))
                    })?;
                    match target {
                        CoefficientTarget::Constraint(con) => {
                            let rid = *w.row_ids.get(&con).ok_or_else(|| {
                                CompileError::RebuildRequired(format!(
                                    "RemoveCell for unknown compiled row ({con:?})"
                                ))
                            })?;
                            operations.push(BackendOp::RemoveLinearCoefficient {
                                constraint: rid,
                                variable: vid,
                            });
                        }
                        CoefficientTarget::Objective(obj) => {
                            let oid = *w.objective_ids.get(&obj).ok_or_else(|| {
                                CompileError::RebuildRequired(format!(
                                    "RemoveCell for unknown compiled objective ({obj:?})"
                                ))
                            })?;
                            operations.push(BackendOp::RemoveObjectiveCoefficient {
                                objective: oid,
                                variable: vid,
                            });
                        }
                    }
                }

                ModelOp::AddObjective { obj, sense } => {
                    let id = CompiledObjectiveId(w.next_objective_index);
                    w.next_objective_index += 1;
                    operations.push(BackendOp::AddObjective(CompiledObjective {
                        id,
                        sense: *sense,
                        coefficients: Vec::new(),
                        constant: 0.0,
                        name: None,
                    }));
                    w.objective_ids.insert(*obj, id);
                    w.compiled_to_objective.insert(id, *obj);
                    // WR-03: keep the compiled objective coefficient/constant
                    // tracking in sync across delta batches.
                    w.compiled_objective_coefficients.insert(id, Vec::new());
                    w.compiled_objective_constants.insert(id, 0.0);
                    origin_additions.insert_objective(id, EntityOrigin::UserObjective(*obj));
                }

                ModelOp::RemoveObjective { obj } => {
                    let id = *w.objective_ids.get(obj).ok_or_else(|| {
                        CompileError::RebuildRequired(format!(
                            "RemoveObjective for unknown compiled objective ({obj:?})"
                        ))
                    })?;
                    // CR-02: removing the ACTIVE objective must ALSO emit the
                    // `SetObjectivePolicy(None)` transition, so the batch is
                    // self-contained (A31) and the target compiled state never
                    // carries a dangling `Single(removed_id)` policy (the
                    // policy the snapshot builder would reject).
                    let was_active = w.objective_policy == CompiledObjectivePolicy::Single(id);
                    operations.push(BackendOp::RemoveObjective(id));
                    if was_active {
                        operations
                            .push(BackendOp::SetObjectivePolicy(CompiledObjectivePolicy::None));
                        w.objective_policy = CompiledObjectivePolicy::None;
                    }
                    w.objective_ids.remove(obj);
                    w.compiled_to_objective.remove(&id);
                    // WR-03: drop the removed objective's coefficient tracking.
                    w.compiled_objective_coefficients.remove(&id);
                    w.compiled_objective_constants.remove(&id);
                }

                // A32: `SetActiveObjective { obj: None }` compiles to
                // `SetObjectivePolicy(CompiledObjectivePolicy::None)`.
                ModelOp::SetActiveObjective { obj } => {
                    let policy = match obj {
                        Some(obj) => {
                            let id = *w.objective_ids.get(obj).ok_or_else(|| {
                                CompileError::RebuildRequired(format!(
                                    "SetActiveObjective for unknown compiled objective ({obj:?})"
                                ))
                            })?;
                            CompiledObjectivePolicy::Single(id)
                        }
                        None => CompiledObjectivePolicy::None,
                    };
                    // Track the working policy so a later `RemoveObjective` of
                    // the now-active objective emits the None transition
                    // (CR-02).
                    w.objective_policy = policy.clone();
                    operations.push(BackendOp::SetObjectivePolicy(policy));
                }

                ModelOp::SetObjectiveCell {
                    cell_key,
                    evaluated_value,
                    constant,
                    ..
                } => {
                    // SM-04.4 (WR-3): objective coefficient changes gate on
                    // `IncrementalCoefficients`, never silently compiled.
                    require_feature(
                        capabilities,
                        policy,
                        BackendFeature::IncrementalCoefficients,
                        "objective coefficient changes",
                    )?;
                    let (target, var) = *cell_key;
                    let obj = match target {
                        CoefficientTarget::Objective(o) => o,
                        CoefficientTarget::Constraint(_) => {
                            return Err(CompileError::RebuildRequired(
                                "SetObjectiveCell with a Constraint target".into(),
                            ));
                        }
                    };
                    let oid = *w.objective_ids.get(&obj).ok_or_else(|| {
                        CompileError::RebuildRequired(format!(
                            "SetObjectiveCell for unknown compiled objective ({obj:?})"
                        ))
                    })?;
                    let vid = *w.variable_ids.get(&var).ok_or_else(|| {
                        CompileError::RebuildRequired(format!(
                            "SetObjectiveCell for unknown compiled variable ({var:?})"
                        ))
                    })?;
                    operations.push(BackendOp::SetObjectiveCoefficient {
                        objective: oid,
                        variable: vid,
                        value: *evaluated_value,
                    });
                    // API-03.5: the constant is reported exactly once.
                    operations.push(BackendOp::SetObjectiveConstant {
                        objective: oid,
                        value: *constant,
                    });
                    // WR-03: keep the compiled objective coefficient/constant
                    // tracking in sync (replace the coefficient cell, preserve
                    // deterministic compiled order).
                    if let Some(cells) = w.compiled_objective_coefficients.get_mut(&oid) {
                        cells.retain(|(cid, _)| *cid != vid);
                        cells.push((vid, *evaluated_value));
                        cells.sort_by_key(|(cid, _)| *cid);
                    }
                    w.compiled_objective_constants.insert(oid, *constant);
                }

                ModelOp::SetObjectiveSense { obj, sense } => {
                    let id = *w.objective_ids.get(obj).ok_or_else(|| {
                        CompileError::RebuildRequired(format!(
                            "SetObjectiveSense for unknown compiled objective ({obj:?})"
                        ))
                    })?;
                    operations.push(BackendOp::SetObjectiveSense {
                        objective: id,
                        sense: *sense,
                    });
                }

                ModelOp::SetObjectiveConstant { obj, constant } => {
                    let id = *w.objective_ids.get(obj).ok_or_else(|| {
                        CompileError::RebuildRequired(format!(
                            "SetObjectiveConstant for unknown compiled objective ({obj:?})"
                        ))
                    })?;
                    operations.push(BackendOp::SetObjectiveConstant {
                        objective: id,
                        value: *constant,
                    });
                    // WR-03: keep the compiled objective constant tracking in
                    // sync.
                    w.compiled_objective_constants.insert(id, *constant);
                }

                // `SetParameter` is a provable NO-OP on backend IR: no compiled
                // entity carries a parameter, and the coefficient index is the
                // single coefficient authority (SM-01.1) — a parameter change
                // re-emits `SetCell` ops for every dependent cell with the new
                // evaluated value, which the compiled delta carries. Skipping
                // the parameter op preserves the M2 incremental parameter
                // behavior through the compiled path (the F-B1 conservative
                // rebuild list is narrowed here by this provable equivalence).
                ModelOp::SetParameter { .. } => {}
                ModelOp::SetSemiContinuousBound { .. } => {
                    return Err(CompileError::RebuildRequired(
                        "SetSemiContinuousBound is not incrementally compilable".into(),
                    ));
                }
                ModelOp::AddConstruct { .. }
                | ModelOp::RemoveConstruct { .. }
                | ModelOp::SetConstructActive { .. } => {
                    // D22: semantic construct changes rebuild first.
                    return Err(CompileError::RebuildRequired(
                        "semantic construct change is not incrementally compilable in P26".into(),
                    ));
                }
            }
        }

        let to_compilation =
            CompilationId::allocate().map_err(|_| CompileError::IdentityOverflow)?;
        let recipe_fingerprint = RecipeFingerprint::for_operations(&operations);

        let batch = BackendDeltaBatch {
            from_compilation: current.compilation_id,
            to_compilation,
            from_revision: delta.from,
            to_revision: delta.to,
            operations,
            origin_additions,
            recipe_fingerprint,
        };

        self.current = Some(CurrentCompilation {
            compilation_id: to_compilation,
            revision: delta.to,
            variable_ids: w.variable_ids,
            variable_activity: w.variable_activity,
            row_ids: w.row_ids,
            objective_ids: w.objective_ids,
            compiled_to_variable: w.compiled_to_variable,
            compiled_to_row: w.compiled_to_row,
            compiled_to_objective: w.compiled_to_objective,
            compiled_objective_coefficients: w.compiled_objective_coefficients,
            compiled_objective_constants: w.compiled_objective_constants,
            next_variable_index: w.next_variable_index,
            next_row_index: w.next_row_index,
            next_objective_index: w.next_objective_index,
            objective_policy: w.objective_policy,
        });

        Ok(batch)
    }
}
