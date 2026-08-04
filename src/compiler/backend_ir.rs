//! Backend IR and exact compilation identity (design §8.3–8.5, §4; P26 Task 5).
//!
//! The compiler projects canonical semantic state into this solver-neutral
//! backend IR: dense compiled IDs distinct from user handles (SM-02.4), a
//! unique checked [`CompilationId`] per compiled state (D28), a mandatory
//! [`OriginMap`], a structured [`CompilationReport`], and a deterministic
//! [`RecipeFingerprint`] used for evidence/cache only — never stale-state
//! authority (SM-03.9).
//!
//! Every generated compiled entity must carry an origin: builder finalization
//! rejects any [`CompiledVariable`]/[`CompiledLinearRow`]/[`CompiledObjective`]
//! that has no `OriginMap` entry, returning a typed [`CompileError`] (D5,
//! SM-02.5). This is the Task 5 stopping condition.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::identity::{IdentityOverflow, ModelInstanceId};
use crate::model::{Bounds, ConstraintBounds, Sense, VarType};
use crate::revision::ModelRevision;

use super::origin::OriginMap;
use super::report::{CompilationReport, FormulationDecision};
use super::CompileError;

/// Allocate the next id from `counter`, returning a typed error on overflow.
///
/// Mirrors `src/identity.rs`: `fetch_update` returns the pre-update value, so
/// the first issued id is 1 (value 0 is reserved and never issued), and the
/// counter saturates at `u64::MAX` — a saturated counter keeps reporting
/// [`IdentityOverflow`] instead of wrapping to 0 and re-issuing id 1.
fn allocate_id(counter: &AtomicU64) -> Result<u64, IdentityOverflow> {
    match counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |pre| {
        if pre == u64::MAX {
            None // saturated: do not advance; report overflow
        } else {
            Some(pre + 1)
        }
    }) {
        Ok(pre) => Ok(pre + 1),
        Err(_) => Err(IdentityOverflow),
    }
}

/// Exact opaque identity of one compiled backend state (design §4.3, D28).
///
/// Every `BackendSnapshot` and `BackendDeltaBatch` carries a `CompilationId`.
/// Stale-state safety compares exact `CompilationId` values — never a
/// fingerprint or revision. Ids are allocated through a checked atomic
/// counter: zero is reserved (the first issued id is 1), and counter
/// exhaustion returns a typed error instead of wrapping (ids are never
/// reused).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CompilationId(u64);

/// Per-family checked atomic counter (zero reserved; 0 never issued).
static COMPILATION_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

impl CompilationId {
    /// Allocate a fresh opaque compilation id. The first issued id is 1; zero
    /// is reserved and never issued. Returns [`IdentityOverflow`] on counter
    /// exhaustion instead of wrapping.
    pub(crate) fn allocate() -> Result<Self, IdentityOverflow> {
        allocate_id(&COMPILATION_ID_COUNTER).map(Self)
    }
}

/// Deterministic recipe fingerprint (design §3.2, §4.3; D28).
///
/// An opaque 32-byte digest of the compiled recipe (variables, rows,
/// objectives, objective policy). Equal compiled recipes produce equal
/// fingerprints; the fingerprint is deterministic evidence/cache support only
/// and is **never** stale-state authority — exact [`CompilationId`] is the
/// comparison key (SM-03.9, must-have truth 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RecipeFingerprint([u8; 32]);

impl RecipeFingerprint {
    /// Compute a deterministic 32-byte fingerprint over the compiled recipe.
    ///
    /// The digest is a non-cryptographic, stable integer construction
    /// (four FNV-1a passes with distinct offset bases packed into 32 bytes)
    /// used for evidence and cache comparisons only.
    pub(crate) fn compute(
        variables: &[CompiledVariable],
        linear_rows: &[CompiledLinearRow],
        objectives: &[CompiledObjective],
        objective_policy: &CompiledObjectivePolicy,
    ) -> Self {
        let mut enc = RecipeEncoder::new();
        // Recipe format version: bump when the encoded recipe changes so
        // fingerprints of identical-older states stay distinguishable.
        enc.push_u8(1);
        // Native constraints are not representable in P26 (F-G); encode the
        // count (0) for forward stability when P32/P33 add payloads.
        enc.push_u64(0);

        enc.push_u64(variables.len() as u64);
        for variable in variables {
            variable.encode(&mut enc);
        }

        enc.push_u64(linear_rows.len() as u64);
        for row in linear_rows {
            row.encode(&mut enc);
        }

        enc.push_u64(objectives.len() as u64);
        for objective in objectives {
            objective.encode(&mut enc);
        }

        objective_policy.encode(&mut enc);

        Self(digest(&enc.buf))
    }

    /// Compute a deterministic fingerprint over a delta batch's operations.
    ///
    /// The compiled delta carries no full target state (only the operations
    /// that transform the from-state into it), so the batch fingerprint is a
    /// deterministic digest of the ordered operations. It is evidence/cache
    /// support only and is **never** stale-state authority (D28, SM-03.9).
    pub(crate) fn for_operations(operations: &[BackendOp]) -> Self {
        let mut enc = RecipeEncoder::new();
        enc.push_u8(2); // delta-recipe format version
        enc.push_u64(operations.len() as u64);
        for op in operations {
            op.encode(&mut enc);
        }
        Self(digest(&enc.buf))
    }
}

/// A dense compiled variable id, distinct from the user [`VarId`](crate::VarId)
/// handle (SM-02.4). `0` is a valid dense index (compiled ids are dense, not
/// opaque counters).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CompiledVariableId(pub u32);

/// A dense compiled constraint/row id, distinct from the user
/// [`ConId`](crate::ConId) handle (SM-02.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CompiledConstraintId(pub u32);

/// A dense compiled objective id, distinct from the user
/// [`ObjId`](crate::ObjId) handle (SM-02.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CompiledObjectiveId(pub u32);

/// Reference to a compiled entity, used by origin completeness validation and
/// error/inventory reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompiledEntityRef {
    /// A compiled variable.
    Variable(CompiledVariableId),
    /// A compiled linear row (constraint).
    Constraint(CompiledConstraintId),
    /// A compiled objective.
    Objective(CompiledObjectiveId),
}

/// A compiled variable (design §8.3).
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledVariable {
    /// Dense compiled id.
    pub id: CompiledVariableId,
    /// Compiled bounds.
    pub bounds: Bounds,
    /// Compiled variable type.
    pub var_type: VarType,
    /// Optional name (source name preserved for diagnostics).
    pub name: Option<String>,
}

/// A compiled linear row (constraint) (design §8.3).
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledLinearRow {
    /// Dense compiled row id.
    pub id: CompiledConstraintId,
    /// Compiled row bounds.
    pub bounds: ConstraintBounds,
    /// Sparse coefficients in deterministic (id, value) order.
    pub coefficients: Vec<(CompiledVariableId, f64)>,
    /// Optional name.
    pub name: Option<String>,
}

/// A compiled objective (design §8.3).
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledObjective {
    /// Dense compiled objective id.
    pub id: CompiledObjectiveId,
    /// Optimization sense.
    pub sense: Sense,
    /// Sparse coefficients in deterministic (id, value) order.
    pub coefficients: Vec<(CompiledVariableId, f64)>,
    /// Objective constant offset.
    pub constant: f64,
    /// Optional name.
    pub name: Option<String>,
}

/// The compiled objective policy: which optimization problem is active
/// (design §8.4; A32).
///
/// [`None`](Self::None) represents the M2 no-active-objective case (A32 /
/// backend-contract point B1): objective-less canonical state compiles to
/// `None`, and `SetActiveObjective { obj: None }` compiles to
/// `BackendOp::SetObjectivePolicy(CompiledObjectivePolicy::None)`. This
/// preserves the M2 reference-backend objective-less solve behavior through
/// the compiled path.
#[derive(Clone, Debug, PartialEq)]
pub enum CompiledObjectivePolicy {
    /// No active objective.
    None,
    /// A single active objective.
    Single(CompiledObjectiveId),
    /// A weighted combination of objectives (P31 canonical `ObjectivePolicy`).
    Weighted(Vec<CompiledWeightedObjective>),
    /// A lexicographic priority list of objectives (P31 canonical
    /// `ObjectivePolicy`).
    Lexicographic(Vec<CompiledObjectiveLevel>),
}

/// One weighted objective in a weighted policy (design §15.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompiledWeightedObjective {
    /// The compiled objective.
    pub objective: CompiledObjectiveId,
    /// Nonnegative weight.
    pub weight: f64,
}

/// One lexicographic priority level (design §15.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompiledObjectiveLevel {
    /// The compiled objective.
    pub objective: CompiledObjectiveId,
    /// Absolute degradation tolerance.
    pub absolute_tolerance: f64,
    /// Relative degradation tolerance.
    pub relative_tolerance: f64,
}

/// Normalized native-primitive extension surface (design §8.3, F-G).
///
/// The normalized-native-primitive payloads (`Indicator`/`Sos1`/`Sos2`/
/// `PiecewiseLinear`) are declared by the packet but land with the P32/P33
/// bridge tasks, mirroring the P25 `ConstructKind`/A30 pattern. In P26 this
/// enum is empty and `BackendSnapshot.native_constraints` is always empty.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum BackendConstraint {}

/// Whether the backend IR can carry a real native-primitive payload (F4).
///
/// In P32 [`BackendConstraint`] has NO variants, so no construct feature can
/// be selected through a qualified native primitive — the exact portable
/// bridge is the only representable path. When a real native payload variant
/// lands (P32/P33 bridge tasks), this function MUST be updated alongside
/// `BackendConstraint` so a backend's native declaration becomes selectable
/// again. Until then, `NativeRequired` rejects every construct feature (a
/// bridge-only path is not native) and `Auto` never reports a native label.
pub(crate) const fn native_payloads_available() -> bool {
    false
}

/// A compiled backend snapshot (design §8.3).
///
/// The full compiled state produced by compiling one canonical
/// [`ModelSnapshot`](crate::ModelSnapshot). `compilation_id` is the exact
/// opaque identity of this compiled state; `source_instance`/`source_revision`
/// tie it back to exact canonical state `(ModelInstanceId, ModelRevision)`
/// (D28).
#[derive(Clone, Debug, PartialEq)]
pub struct BackendSnapshot {
    /// Exact opaque identity of this compiled state.
    pub compilation_id: CompilationId,
    /// The source model instance the snapshot was compiled from.
    pub source_instance: ModelInstanceId,
    /// The source model revision the snapshot was compiled from.
    pub source_revision: ModelRevision,
    /// Compiled variables.
    pub variables: Vec<CompiledVariable>,
    /// Compiled linear rows.
    pub linear_rows: Vec<CompiledLinearRow>,
    /// Normalized native constraints (always empty in P26).
    pub native_constraints: Vec<BackendConstraint>,
    /// Compiled objectives.
    pub objectives: Vec<CompiledObjective>,
    /// The active compiled objective policy.
    pub objective_policy: CompiledObjectivePolicy,
    /// Mandatory origin map (every compiled entity has an origin).
    pub origin_map: OriginMap,
    /// Structured compilation report (fingerprint, inventory, decisions).
    pub report: CompilationReport,
    /// Deterministic recipe fingerprint (evidence/cache only, never authority).
    pub recipe_fingerprint: RecipeFingerprint,
}

/// A compiled delta batch (design §8.3, B2).
///
/// Every batch carries exact from/to `CompilationId` and `ModelRevision`
/// pairs. The compiler allocates a fresh `CompilationId` per target state;
/// divergent clones with equal `ModelRevision` never share a `CompilationId`
/// (D28).
///
/// # Origin completeness (SM-02.5 / truth 5)
///
/// A delta adds generated compiled entities (e.g. `AddVariable`); the
/// [`origin_additions`](Self::origin_additions) map records the
/// `EntityOrigin` of every entity ADDED by this batch, so the target
/// compiled state remains origin-complete and the backend can map compiled
/// solution values back to user entities. Removals and updates reference
/// compiled ids already present in the from-state.
#[derive(Clone, Debug, PartialEq)]
pub struct BackendDeltaBatch {
    /// The exact compiled state this batch applies on top of.
    pub from_compilation: CompilationId,
    /// The exact compiled state this batch produces.
    pub to_compilation: CompilationId,
    /// The canonical revision before application.
    pub from_revision: ModelRevision,
    /// The canonical revision after application.
    pub to_revision: ModelRevision,
    /// Ordered backend operations.
    pub operations: Vec<BackendOp>,
    /// Origins of the entities added by this batch (SM-02.5).
    pub origin_additions: OriginMap,
    /// Deterministic fingerprint over the batch's operations (evidence only).
    pub recipe_fingerprint: RecipeFingerprint,
}

/// A backend delta operation (design §8.3; pinned 15-variant enumeration,
/// backend-contract point B3).
///
/// The enumeration is review-gated with the Task 7 implementation plan and
/// pinned in the P26 Task 0 acceptance record. Ops that the identity compiler
/// cannot prove incrementally equivalent force a deterministic rebuild
/// (design §18, F-B1).
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum BackendOp {
    /// Add a compiled variable.
    AddVariable(CompiledVariable),
    /// Remove a compiled variable.
    RemoveVariable(CompiledVariableId),
    /// Set compiled variable bounds.
    SetVariableBounds {
        /// The affected compiled variable.
        variable: CompiledVariableId,
        /// New bounds.
        bounds: Bounds,
    },
    /// Add a compiled linear row.
    AddLinearRow(CompiledLinearRow),
    /// Remove a compiled linear row.
    RemoveLinearRow(CompiledConstraintId),
    /// Set compiled row bounds.
    SetLinearRowBounds {
        /// The affected compiled row.
        constraint: CompiledConstraintId,
        /// New bounds.
        bounds: ConstraintBounds,
    },
    /// Set a linear coefficient cell (upsert).
    SetLinearCoefficient {
        /// The affected compiled row.
        constraint: CompiledConstraintId,
        /// The compiled variable.
        variable: CompiledVariableId,
        /// New coefficient value.
        value: f64,
    },
    /// Remove a linear coefficient cell.
    RemoveLinearCoefficient {
        /// The affected compiled row.
        constraint: CompiledConstraintId,
        /// The compiled variable.
        variable: CompiledVariableId,
    },
    /// Add a compiled objective.
    AddObjective(CompiledObjective),
    /// Remove a compiled objective.
    RemoveObjective(CompiledObjectiveId),
    /// Set an objective coefficient cell (upsert).
    SetObjectiveCoefficient {
        /// The affected compiled objective.
        objective: CompiledObjectiveId,
        /// The compiled variable.
        variable: CompiledVariableId,
        /// New coefficient value.
        value: f64,
    },
    /// Remove an objective coefficient cell.
    RemoveObjectiveCoefficient {
        /// The affected compiled objective.
        objective: CompiledObjectiveId,
        /// The compiled variable.
        variable: CompiledVariableId,
    },
    /// Set an objective constant offset (API-03.5: reported exactly once).
    SetObjectiveConstant {
        /// The affected compiled objective.
        objective: CompiledObjectiveId,
        /// New constant value.
        value: f64,
    },
    /// Set an objective sense.
    SetObjectiveSense {
        /// The affected compiled objective.
        objective: CompiledObjectiveId,
        /// New minimize/maximize sense.
        sense: Sense,
    },
    /// Set the active compiled objective policy (A32 includes `None`).
    SetObjectivePolicy(CompiledObjectivePolicy),
}

// ===========================================================================
// Preflight reference validation (F5)
// ===========================================================================

/// The set of compiled entities a [`BackendDeltaBatch`] may reference (F5).
///
/// A backend constructs this from the compiled state it currently holds before
/// validating an incoming batch; `Add*` ops inside the batch extend the set.
/// Malformed batches whose ops reference entities outside this set are rejected
/// with a typed [`CompileError::InvalidReference`] before any op is applied.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompiledEntityRegistry {
    /// Compiled variables present in the target state.
    pub variables: BTreeSet<CompiledVariableId>,
    /// Compiled rows present in the target state.
    pub rows: BTreeSet<CompiledConstraintId>,
    /// Compiled objectives present in the target state.
    pub objectives: BTreeSet<CompiledObjectiveId>,
}

impl BackendSnapshot {
    /// Validate a snapshot's structural integrity (F2/F5):
    ///
    /// - (F2) every compiled-id family is UNIQUE and DENSE (`0..len`) — a
    ///   duplicate id is a typed [`CompileError::DuplicateEntity`], a gap or an
    ///   id beyond the count a [`CompileError::NonDenseCompilation`];
    /// - (F2) every variable/row/objective has a recorded origin (D5, SM-02.5)
    ///   — otherwise [`CompileError::MissingOrigin`];
    /// - (F5) every row and objective coefficient references a compiled
    ///   variable present in this snapshot, and the objective policy
    ///   references only compiled objectives present in this snapshot —
    ///   otherwise a typed [`CompileError::InvalidReference`].
    ///
    /// Malformed backend IR is rejected, never silently skipped. `BackendSnapshot`
    /// has all-`pub` fields and can be constructed directly (bypassing
    /// [`BackendSnapshotBuilder::finalize`]), so backends run this before
    /// reconstructing native state from a snapshot.
    pub fn validate(&self) -> Result<(), CompileError> {
        // F2(a): compiled ids must be UNIQUE within each family — dense
        // deterministic allocation (SM-02.4) never reuses an id.
        // F2(b): compiled ids must be DENSE (`0..len` per family) — a gap or
        // an id beyond the count is malformed backend IR.
        let variable_ids: Vec<u32> = self.variables.iter().map(|v| v.id.0).collect();
        let row_ids: Vec<u32> = self.linear_rows.iter().map(|r| r.id.0).collect();
        let objective_ids: Vec<u32> = self.objectives.iter().map(|o| o.id.0).collect();
        validate_id_family(&variable_ids, |id| {
            CompiledEntityRef::Variable(CompiledVariableId(id))
        })?;
        validate_id_family(&row_ids, |id| {
            CompiledEntityRef::Constraint(CompiledConstraintId(id))
        })?;
        validate_id_family(&objective_ids, |id| {
            CompiledEntityRef::Objective(CompiledObjectiveId(id))
        })?;

        // F2(c): every compiled entity must carry an origin (D5, SM-02.5) —
        // re-checked here because `BackendSnapshot` has all-`pub` fields and
        // can be constructed directly, bypassing builder finalization.
        if let Some(entity) = self
            .origin_map
            .missing_origins(&self.variables, &self.linear_rows, &self.objectives)
            .first()
        {
            return Err(CompileError::MissingOrigin { entity: *entity });
        }

        let variables: BTreeSet<CompiledVariableId> = self.variables.iter().map(|v| v.id).collect();

        for row in &self.linear_rows {
            for (vid, _) in &row.coefficients {
                if !variables.contains(vid) {
                    return Err(CompileError::InvalidReference {
                        entity: CompiledEntityRef::Variable(*vid),
                    });
                }
            }
        }
        for objective in &self.objectives {
            for (vid, _) in &objective.coefficients {
                if !variables.contains(vid) {
                    return Err(CompileError::InvalidReference {
                        entity: CompiledEntityRef::Variable(*vid),
                    });
                }
            }
        }
        // Policy references must exist (mirrors builder finalization, re-checked
        // for directly-constructed snapshots).
        if let Some(id) = dangling_policy_objective(&self.objective_policy, &self.objectives) {
            return Err(CompileError::InvalidReference {
                entity: CompiledEntityRef::Objective(id),
            });
        }
        Ok(())
    }
}

/// F2: verify one compiled-id family is unique and dense (`0..len`).
///
/// Deterministic: the ids are sorted, then checked for adjacent duplicates (a
/// duplicate is a typed [`CompileError::DuplicateEntity`]) and for the exact
/// `0..len` sequence (a gap or an id beyond the count is a typed
/// [`CompileError::NonDenseCompilation`]).
fn validate_id_family(
    ids: &[u32],
    to_entity: impl Fn(u32) -> CompiledEntityRef,
) -> Result<(), CompileError> {
    let mut sorted: Vec<u32> = ids.to_vec();
    sorted.sort_unstable();
    for pair in sorted.windows(2) {
        if pair[0] == pair[1] {
            return Err(CompileError::DuplicateEntity {
                entity: to_entity(pair[0]),
            });
        }
    }
    for (i, id) in sorted.iter().enumerate() {
        if *id as usize != i {
            return Err(CompileError::NonDenseCompilation {
                entity: to_entity(*id),
            });
        }
    }
    Ok(())
}

impl BackendDeltaBatch {
    /// All-or-nothing preflight validation of this batch against `existing`
    /// (the backend's current compiled state) using EXACT op semantics in
    /// operation order (F1):
    ///
    /// - `AddVariable`/`AddLinearRow`/`AddObjective` INSERT the entity into a
    ///   working registry clone and REJECT a duplicate id (a compiled id is
    ///   added exactly once per target state) as a typed
    ///   [`CompileError::DuplicateEntity`];
    /// - `RemoveVariable`/`RemoveLinearRow`/`RemoveObjective` DELETE the id
    ///   from the working clone, so a later `Set*`/coefficient op on a removed
    ///   id correctly fails ([`CompileError::InvalidReference`]);
    /// - every `Add*` op must carry a corresponding origin in this batch's
    ///   [`origin_additions`](Self::origin_additions) map — the target state
    ///   stays origin-complete (D5, SM-02.5), otherwise
    ///   [`CompileError::MissingOrigin`];
    /// - update/remove ops reference entities present in the evolving clone,
    ///   and the objective policy references an existing objective.
    ///
    /// Before the op simulation the ENVELOPE is validated (fifth review): the
    /// batch must advance both the exact compiled identity
    /// (`from_compilation != to_compilation`, D28) and the canonical model
    /// revision (`from_revision < to_revision`), otherwise a typed
    /// [`CompileError::InvalidDeltaEnvelope`] is returned with no registry work.
    ///
    /// Backends run this BEFORE applying any op, so a malformed batch — even
    /// one whose ops are self-consistent only when reordered — never partially
    /// mutates native state.
    pub fn validate(&self, existing: &CompiledEntityRegistry) -> Result<(), CompileError> {
        // Envelope validation runs BEFORE any op simulation (fifth review): a
        // batch must advance BOTH the exact compiled identity (D28) and the
        // canonical model revision. A batch whose `from_compilation ==
        // to_compilation` would mutate state while retaining the old exact
        // identity, and a batch whose `from_revision >= to_revision` would not
        // advance the model, are malformed — rejected without any registry
        // work, so a malformed envelope never reaches a backend's native state.
        if self.from_compilation == self.to_compilation {
            return Err(CompileError::InvalidDeltaEnvelope {
                reason: "identical from/to compilation ids".to_string(),
            });
        }
        if self.from_revision >= self.to_revision {
            return Err(CompileError::InvalidDeltaEnvelope {
                reason: format!(
                    "non-advancing revisions: from {} >= to {}",
                    self.from_revision, self.to_revision
                ),
            });
        }

        let mut present = existing.clone();
        for op in &self.operations {
            match op {
                // F1: `Add*` inserts and REJECTS an id that is already present
                // (whether from the from-state or an earlier op in this batch);
                // `Remove*` DELETES from the working set, so a later `Set*` on a
                // removed id correctly fails. Order is preserved — the batch is
                // simulated sequentially against a working registry clone.
                BackendOp::AddVariable(v) => {
                    if !present.variables.insert(v.id) {
                        return Err(CompileError::DuplicateEntity {
                            entity: CompiledEntityRef::Variable(v.id),
                        });
                    }
                    // F1 (SM-02.5): every entity ADDED by the batch must carry
                    // an origin in the batch's origin map.
                    if self.origin_additions.variable_origin(v.id).is_none() {
                        return Err(CompileError::MissingOrigin {
                            entity: CompiledEntityRef::Variable(v.id),
                        });
                    }
                }
                BackendOp::RemoveVariable(id) => {
                    if !present.variables.remove(id) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Variable(*id),
                        });
                    }
                }
                BackendOp::SetVariableBounds { variable, .. } => {
                    if !present.variables.contains(variable) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Variable(*variable),
                        });
                    }
                }
                BackendOp::AddLinearRow(r) => {
                    if !present.rows.insert(r.id) {
                        return Err(CompileError::DuplicateEntity {
                            entity: CompiledEntityRef::Constraint(r.id),
                        });
                    }
                    if self.origin_additions.constraint_origin(r.id).is_none() {
                        return Err(CompileError::MissingOrigin {
                            entity: CompiledEntityRef::Constraint(r.id),
                        });
                    }
                    for (cid, _) in &r.coefficients {
                        if !present.variables.contains(cid) {
                            return Err(CompileError::InvalidReference {
                                entity: CompiledEntityRef::Variable(*cid),
                            });
                        }
                    }
                }
                BackendOp::RemoveLinearRow(id) => {
                    if !present.rows.remove(id) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Constraint(*id),
                        });
                    }
                }
                BackendOp::SetLinearRowBounds { constraint, .. } => {
                    if !present.rows.contains(constraint) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Constraint(*constraint),
                        });
                    }
                }
                BackendOp::SetLinearCoefficient {
                    constraint,
                    variable,
                    ..
                } => {
                    if !present.rows.contains(constraint) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Constraint(*constraint),
                        });
                    }
                    if !present.variables.contains(variable) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Variable(*variable),
                        });
                    }
                }
                BackendOp::RemoveLinearCoefficient {
                    constraint,
                    variable,
                } => {
                    if !present.rows.contains(constraint) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Constraint(*constraint),
                        });
                    }
                    if !present.variables.contains(variable) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Variable(*variable),
                        });
                    }
                }
                BackendOp::AddObjective(o) => {
                    if !present.objectives.insert(o.id) {
                        return Err(CompileError::DuplicateEntity {
                            entity: CompiledEntityRef::Objective(o.id),
                        });
                    }
                    if self.origin_additions.objective_origin(o.id).is_none() {
                        return Err(CompileError::MissingOrigin {
                            entity: CompiledEntityRef::Objective(o.id),
                        });
                    }
                    for (cid, _) in &o.coefficients {
                        if !present.variables.contains(cid) {
                            return Err(CompileError::InvalidReference {
                                entity: CompiledEntityRef::Variable(*cid),
                            });
                        }
                    }
                }
                BackendOp::RemoveObjective(id) => {
                    if !present.objectives.remove(id) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Objective(*id),
                        });
                    }
                }
                BackendOp::SetObjectiveCoefficient {
                    objective,
                    variable,
                    ..
                } => {
                    if !present.objectives.contains(objective) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Objective(*objective),
                        });
                    }
                    if !present.variables.contains(variable) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Variable(*variable),
                        });
                    }
                }
                BackendOp::RemoveObjectiveCoefficient {
                    objective,
                    variable,
                } => {
                    if !present.objectives.contains(objective) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Objective(*objective),
                        });
                    }
                    if !present.variables.contains(variable) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Variable(*variable),
                        });
                    }
                }
                BackendOp::SetObjectiveConstant { objective, .. } => {
                    if !present.objectives.contains(objective) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Objective(*objective),
                        });
                    }
                }
                BackendOp::SetObjectiveSense { objective, .. } => {
                    if !present.objectives.contains(objective) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Objective(*objective),
                        });
                    }
                }
                BackendOp::SetObjectivePolicy(policy) => match policy {
                    CompiledObjectivePolicy::None => {}
                    CompiledObjectivePolicy::Single(id) => {
                        if !present.objectives.contains(id) {
                            return Err(CompileError::InvalidReference {
                                entity: CompiledEntityRef::Objective(*id),
                            });
                        }
                    }
                    CompiledObjectivePolicy::Weighted(items) => {
                        for item in items {
                            if !present.objectives.contains(&item.objective) {
                                return Err(CompileError::InvalidReference {
                                    entity: CompiledEntityRef::Objective(item.objective),
                                });
                            }
                        }
                    }
                    CompiledObjectivePolicy::Lexicographic(items) => {
                        for item in items {
                            if !present.objectives.contains(&item.objective) {
                                return Err(CompileError::InvalidReference {
                                    entity: CompiledEntityRef::Objective(item.objective),
                                });
                            }
                        }
                    }
                },
            }
        }
        Ok(())
    }
}

/// Builder for [`BackendSnapshot`].
///
/// The canonical construction path: accumulate compiled entities and an
/// [`OriginMap`], then [`finalize`](Self::finalize). Finalization allocates a
/// fresh checked [`CompilationId`], validates that every compiled entity has a
/// recorded origin (D5, SM-02.5) and that the objective policy references
/// compiled objectives (design §8.4), then computes the deterministic recipe
/// fingerprint and structured report.
pub struct BackendSnapshotBuilder {
    source_instance: ModelInstanceId,
    source_revision: ModelRevision,
    variables: Vec<CompiledVariable>,
    linear_rows: Vec<CompiledLinearRow>,
    objectives: Vec<CompiledObjective>,
    objective_policy: Option<CompiledObjectivePolicy>,
    origin_map: OriginMap,
    formulation_decisions: Vec<FormulationDecision>,
}

impl BackendSnapshotBuilder {
    /// Begin building a snapshot compiled from the given source identity.
    #[allow(clippy::new_without_default)]
    pub fn new(source_instance: ModelInstanceId, source_revision: ModelRevision) -> Self {
        Self {
            source_instance,
            source_revision,
            variables: Vec::new(),
            linear_rows: Vec::new(),
            objectives: Vec::new(),
            objective_policy: None,
            origin_map: OriginMap::new(),
            formulation_decisions: Vec::new(),
        }
    }

    /// Provide the origin map that records every compiled entity's origin.
    pub fn origin_map(mut self, origin_map: OriginMap) -> Self {
        self.origin_map = origin_map;
        self
    }

    /// Add a compiled variable.
    pub fn add_variable(mut self, variable: CompiledVariable) -> Self {
        self.variables.push(variable);
        self
    }

    /// Add a compiled linear row.
    pub fn add_linear_row(mut self, row: CompiledLinearRow) -> Self {
        self.linear_rows.push(row);
        self
    }

    /// Add a compiled objective.
    pub fn add_objective(mut self, objective: CompiledObjective) -> Self {
        self.objectives.push(objective);
        self
    }

    /// Set the compiled objective policy.
    ///
    /// When unset, the snapshot finalizes with
    /// [`CompiledObjectivePolicy::None`] (A32: the M2 no-active-objective
    /// default).
    pub fn objective_policy(mut self, policy: CompiledObjectivePolicy) -> Self {
        self.objective_policy = Some(policy);
        self
    }

    /// Append extra formulation decisions (P32/P33 bridge per-construct
    /// decisions) to the compiled report.
    pub fn add_formulation_decisions(mut self, decisions: Vec<FormulationDecision>) -> Self {
        self.formulation_decisions.extend(decisions);
        self
    }

    /// Finalize the snapshot, rejecting any generated entity without an origin.
    ///
    /// On success a fresh checked [`CompilationId`] is allocated for this
    /// compiled state. On failure a typed [`CompileError`] is returned and no
    /// state is produced.
    pub fn finalize(self) -> Result<BackendSnapshot, CompileError> {
        let objective_policy = self
            .objective_policy
            .unwrap_or(CompiledObjectivePolicy::None);

        // D5 / SM-02.5 — the Task 5 stopping condition: no generated entity
        // can be finalized without a recorded origin.
        let missing =
            self.origin_map
                .missing_origins(&self.variables, &self.linear_rows, &self.objectives);
        if let Some(entity) = missing.first() {
            return Err(CompileError::MissingOrigin { entity: *entity });
        }

        // Design §8.4: the active objective policy must reference a compiled
        // objective that exists in this snapshot.
        if let Some(id) = dangling_policy_objective(&objective_policy, &self.objectives) {
            return Err(CompileError::InvalidObjectivePolicy(id));
        }

        let compilation_id =
            CompilationId::allocate().map_err(|_| CompileError::IdentityOverflow)?;
        let recipe_fingerprint = RecipeFingerprint::compute(
            &self.variables,
            &self.linear_rows,
            &self.objectives,
            &objective_policy,
        );
        let report = CompilationReport::new(
            recipe_fingerprint,
            &self.variables,
            &self.linear_rows,
            &self.objectives,
            &objective_policy,
            self.formulation_decisions,
        );

        Ok(BackendSnapshot {
            compilation_id,
            source_instance: self.source_instance,
            source_revision: self.source_revision,
            variables: self.variables,
            linear_rows: self.linear_rows,
            native_constraints: Vec::new(),
            objectives: self.objectives,
            objective_policy,
            origin_map: self.origin_map,
            report,
            recipe_fingerprint,
        })
    }
}

/// Return the first compiled objective id referenced by `policy` that does not
/// appear in `objectives`, if any.
fn dangling_policy_objective(
    policy: &CompiledObjectivePolicy,
    objectives: &[CompiledObjective],
) -> Option<CompiledObjectiveId> {
    let compiled: Vec<CompiledObjectiveId> = objectives.iter().map(|o| o.id).collect();
    match policy {
        CompiledObjectivePolicy::None => None,
        CompiledObjectivePolicy::Single(id) => {
            if compiled.contains(id) {
                None
            } else {
                Some(*id)
            }
        }
        CompiledObjectivePolicy::Weighted(items) => items.iter().find_map(|item| {
            if compiled.contains(&item.objective) {
                None
            } else {
                Some(item.objective)
            }
        }),
        CompiledObjectivePolicy::Lexicographic(items) => items.iter().find_map(|item| {
            if compiled.contains(&item.objective) {
                None
            } else {
                Some(item.objective)
            }
        }),
    }
}

// ===========================================================================
// Deterministic recipe encoding + digest
// ===========================================================================

/// Deterministic byte encoder for the compiled recipe (fingerprint input).
#[derive(Default)]
struct RecipeEncoder {
    buf: Vec<u8>,
}

impl RecipeEncoder {
    fn new() -> Self {
        Self { buf: Vec::new() }
    }

    fn push_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    fn push_u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    fn push_u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    fn push_f64(&mut self, v: f64) {
        self.buf.extend_from_slice(&v.to_bits().to_le_bytes());
    }

    fn push_opt_str(&mut self, s: &Option<String>) {
        match s {
            Some(s) => {
                self.buf.push(1);
                self.push_str(s);
            }
            None => self.buf.push(0),
        }
    }

    fn push_str(&mut self, s: &str) {
        self.push_u64(s.len() as u64);
        self.buf.extend_from_slice(s.as_bytes());
    }
}

fn var_type_tag(t: VarType) -> u8 {
    match t {
        VarType::Continuous => 0,
        VarType::Integer => 1,
        VarType::Binary => 2,
    }
}

fn sense_tag(s: Sense) -> u8 {
    match s {
        Sense::Minimize => 0,
        Sense::Maximize => 1,
    }
}

impl CompiledVariable {
    fn encode(&self, enc: &mut RecipeEncoder) {
        enc.push_u32(self.id.0);
        enc.push_f64(self.bounds.lower);
        enc.push_f64(self.bounds.upper);
        enc.push_u8(var_type_tag(self.var_type));
        enc.push_opt_str(&self.name);
    }
}

impl CompiledLinearRow {
    fn encode(&self, enc: &mut RecipeEncoder) {
        enc.push_u32(self.id.0);
        enc.push_f64(self.bounds.lower);
        enc.push_f64(self.bounds.upper);
        enc.push_u64(self.coefficients.len() as u64);
        for (var, value) in &self.coefficients {
            enc.push_u32(var.0);
            enc.push_f64(*value);
        }
        enc.push_opt_str(&self.name);
    }
}

impl CompiledObjective {
    fn encode(&self, enc: &mut RecipeEncoder) {
        enc.push_u32(self.id.0);
        enc.push_u8(sense_tag(self.sense));
        enc.push_u64(self.coefficients.len() as u64);
        for (var, value) in &self.coefficients {
            enc.push_u32(var.0);
            enc.push_f64(*value);
        }
        enc.push_f64(self.constant);
        enc.push_opt_str(&self.name);
    }
}

impl CompiledObjectivePolicy {
    fn encode(&self, enc: &mut RecipeEncoder) {
        match self {
            CompiledObjectivePolicy::None => enc.push_u8(0),
            CompiledObjectivePolicy::Single(id) => {
                enc.push_u8(1);
                enc.push_u32(id.0);
            }
            CompiledObjectivePolicy::Weighted(items) => {
                enc.push_u8(2);
                enc.push_u64(items.len() as u64);
                for item in items {
                    enc.push_u32(item.objective.0);
                    enc.push_f64(item.weight);
                }
            }
            CompiledObjectivePolicy::Lexicographic(items) => {
                enc.push_u8(3);
                enc.push_u64(items.len() as u64);
                for item in items {
                    enc.push_u32(item.objective.0);
                    enc.push_f64(item.absolute_tolerance);
                    enc.push_f64(item.relative_tolerance);
                }
            }
        }
    }
}

impl BackendOp {
    /// Deterministic encoding of one backend op for the delta fingerprint.
    fn encode(&self, enc: &mut RecipeEncoder) {
        match self {
            BackendOp::AddVariable(v) => {
                enc.push_u8(0);
                v.encode(enc);
            }
            BackendOp::RemoveVariable(id) => {
                enc.push_u8(1);
                enc.push_u32(id.0);
            }
            BackendOp::SetVariableBounds { variable, bounds } => {
                enc.push_u8(2);
                enc.push_u32(variable.0);
                enc.push_f64(bounds.lower);
                enc.push_f64(bounds.upper);
            }
            BackendOp::AddLinearRow(r) => {
                enc.push_u8(3);
                r.encode(enc);
            }
            BackendOp::RemoveLinearRow(id) => {
                enc.push_u8(4);
                enc.push_u32(id.0);
            }
            BackendOp::SetLinearRowBounds { constraint, bounds } => {
                enc.push_u8(5);
                enc.push_u32(constraint.0);
                enc.push_f64(bounds.lower);
                enc.push_f64(bounds.upper);
            }
            BackendOp::SetLinearCoefficient {
                constraint,
                variable,
                value,
            } => {
                enc.push_u8(6);
                enc.push_u32(constraint.0);
                enc.push_u32(variable.0);
                enc.push_f64(*value);
            }
            BackendOp::RemoveLinearCoefficient {
                constraint,
                variable,
            } => {
                enc.push_u8(7);
                enc.push_u32(constraint.0);
                enc.push_u32(variable.0);
            }
            BackendOp::AddObjective(o) => {
                enc.push_u8(8);
                o.encode(enc);
            }
            BackendOp::RemoveObjective(id) => {
                enc.push_u8(9);
                enc.push_u32(id.0);
            }
            BackendOp::SetObjectiveCoefficient {
                objective,
                variable,
                value,
            } => {
                enc.push_u8(10);
                enc.push_u32(objective.0);
                enc.push_u32(variable.0);
                enc.push_f64(*value);
            }
            BackendOp::RemoveObjectiveCoefficient {
                objective,
                variable,
            } => {
                enc.push_u8(11);
                enc.push_u32(objective.0);
                enc.push_u32(variable.0);
            }
            BackendOp::SetObjectiveConstant { objective, value } => {
                enc.push_u8(12);
                enc.push_u32(objective.0);
                enc.push_f64(*value);
            }
            BackendOp::SetObjectiveSense { objective, sense } => {
                enc.push_u8(13);
                enc.push_u32(objective.0);
                enc.push_u8(sense_tag(*sense));
            }
            BackendOp::SetObjectivePolicy(policy) => {
                enc.push_u8(14);
                policy.encode(enc);
            }
        }
    }
}

/// FNV-1a 64-bit offset basis.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a 64-bit prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a 64-bit pass over `bytes` with the given offset basis.
fn fnv1a(bytes: &[u8], offset: u64) -> u64 {
    let mut hash = offset;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Deterministic 32-byte digest: four FNV-1a passes with distinct offset
/// bases packed into 32 bytes. Stable integer arithmetic (not a
/// cryptographic hash) — sufficient for evidence/cache fingerprints.
fn digest(bytes: &[u8]) -> [u8; 32] {
    let a = fnv1a(bytes, FNV_OFFSET_BASIS);
    let b = fnv1a(bytes, FNV_OFFSET_BASIS ^ 0x9e37_79b9_7f4a_7c15);
    let c = fnv1a(bytes, FNV_OFFSET_BASIS ^ 0x85eb_ca6b_2b4c_6b53);
    let d = fnv1a(bytes, FNV_OFFSET_BASIS ^ 0xc2b2_ae3d_27d4_eb4f);
    let mut out = [0u8; 32];
    out[0..8].copy_from_slice(&a.to_le_bytes());
    out[8..16].copy_from_slice(&b.to_le_bytes());
    out[16..24].copy_from_slice(&c.to_le_bytes());
    out[24..32].copy_from_slice(&d.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::origin::EntityOrigin;
    use crate::id::{ConId, Generation, VarId};

    /// Test-only id family (mirrors `TestOverflowId` in `src/identity.rs`,
    /// IN-02) so the family-level overflow branch is exercised without racing
    /// the shared `COMPILATION_ID_COUNTER` used by concurrent unit tests.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct TestCompilationId(u64);

    static TEST_COMPILATION_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

    impl TestCompilationId {
        fn allocate() -> Result<Self, IdentityOverflow> {
            allocate_id(&TEST_COMPILATION_ID_COUNTER).map(Self)
        }

        fn seed_for_test(value: u64) {
            TEST_COMPILATION_ID_COUNTER.store(value, Ordering::Relaxed);
        }
    }

    /// The checked allocation helper saturates instead of wrapping (WR-03):
    /// once the counter holds `u64::MAX`, every further allocation is a typed
    /// error and no id is re-issued.
    #[test]
    fn allocate_id_saturates_on_overflow_without_wrapping() {
        let counter = AtomicU64::new(u64::MAX);
        assert_eq!(allocate_id(&counter), Err(IdentityOverflow));
        // The counter stays saturated: a second call is still Err (no wrap,
        // no id reuse).
        assert_eq!(allocate_id(&counter), Err(IdentityOverflow));
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }

    /// Family-level allocation saturates too (IN-02): seeding the test family
    /// counter at `u64::MAX` proves the overflow branch is reached and that a
    /// subsequent call is still `Err`.
    #[test]
    fn family_allocate_saturates_on_overflow_without_reissuing() {
        TestCompilationId::seed_for_test(u64::MAX);
        assert_eq!(TestCompilationId::allocate(), Err(IdentityOverflow));
        assert_eq!(TestCompilationId::allocate(), Err(IdentityOverflow));
    }

    /// Zero is reserved: two allocations from the real family are distinct and
    /// ordered, so the first issued id cannot collide with a zero sentinel.
    #[test]
    fn compilation_id_is_distinct_and_ordered() {
        let a = CompilationId::allocate().unwrap();
        let b = CompilationId::allocate().unwrap();
        assert_ne!(a, b);
        assert!(a < b, "ids are issued in increasing order");
    }

    /// Equal recipe content produces equal fingerprints; differing content
    /// (e.g. a different row bound) produces a different fingerprint.
    #[test]
    fn recipe_fingerprint_is_deterministic_over_content() {
        let variable = |id: u32, lower: f64| CompiledVariable {
            id: CompiledVariableId(id),
            bounds: Bounds::new(lower, f64::INFINITY),
            var_type: VarType::Continuous,
            name: None,
        };
        let vars_a = vec![variable(0, 0.0)];
        let vars_b = vec![variable(0, 1.0)];

        let policy = CompiledObjectivePolicy::None;
        let f_a1 = RecipeFingerprint::compute(&vars_a, &[], &[], &policy);
        let f_a2 = RecipeFingerprint::compute(&vars_a, &[], &[], &policy);
        let f_b = RecipeFingerprint::compute(&vars_b, &[], &[], &policy);

        assert_eq!(f_a1, f_a2, "equal content -> equal fingerprint");
        assert_ne!(f_a1, f_b, "different content -> different fingerprint");
    }

    // ── F1: order-aware, all-or-nothing delta preflight ──────────────────────

    fn test_var(id: u32) -> CompiledVariable {
        CompiledVariable {
            id: CompiledVariableId(id),
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
            name: None,
        }
    }

    fn test_row(id: u32) -> CompiledLinearRow {
        CompiledLinearRow {
            id: CompiledConstraintId(id),
            bounds: ConstraintBounds::le(10.0),
            coefficients: Vec::new(),
            name: None,
        }
    }

    fn test_batch(ops: Vec<BackendOp>, origins: OriginMap) -> BackendDeltaBatch {
        BackendDeltaBatch {
            from_compilation: CompilationId::allocate().unwrap(),
            to_compilation: CompilationId::allocate().unwrap(),
            from_revision: ModelRevision::ZERO,
            to_revision: ModelRevision::from_u64(1),
            operations: ops,
            origin_additions: origins,
            recipe_fingerprint: RecipeFingerprint::for_operations(&[]),
        }
    }

    fn var_origin() -> EntityOrigin {
        EntityOrigin::UserVariable(VarId::new(0, Generation::new()))
    }

    /// F1: a `RemoveVariable` followed by a `SetVariableBounds` on the SAME id
    /// in one batch is rejected at preflight — the removal deletes the id from
    /// the working registry, so the later set correctly fails. Without
    /// order-aware simulation the batch would pass preflight and then partially
    /// mutate (the all-or-nothing guarantee breaks).
    #[test]
    fn delta_validate_rejects_remove_then_set_same_batch() {
        let existing = CompiledEntityRegistry {
            variables: BTreeSet::from([CompiledVariableId(0)]),
            ..CompiledEntityRegistry::default()
        };
        let batch = test_batch(
            vec![
                BackendOp::RemoveVariable(CompiledVariableId(0)),
                BackendOp::SetVariableBounds {
                    variable: CompiledVariableId(0),
                    bounds: Bounds::new(0.0, 5.0),
                },
            ],
            OriginMap::new(),
        );
        assert!(matches!(
            batch.validate(&existing),
            Err(CompileError::InvalidReference {
                entity: CompiledEntityRef::Variable(CompiledVariableId(0))
            })
        ));
    }

    /// F1: adding the SAME compiled id twice in one batch is a typed
    /// `DuplicateEntity` error — a compiled id is added exactly once per target
    /// state.
    #[test]
    fn delta_validate_rejects_duplicate_add_variable() {
        let existing = CompiledEntityRegistry::default();
        let mut origins = OriginMap::new();
        origins.insert_variable(CompiledVariableId(0), var_origin());
        let batch = test_batch(
            vec![
                BackendOp::AddVariable(test_var(0)),
                BackendOp::AddVariable(test_var(0)),
            ],
            origins,
        );
        assert!(matches!(
            batch.validate(&existing),
            Err(CompileError::DuplicateEntity {
                entity: CompiledEntityRef::Variable(CompiledVariableId(0))
            })
        ));
    }

    /// F1 (SM-02.5): an `AddLinearRow` op MUST carry a corresponding origin in
    /// the batch's `origin_additions` — the target compiled state stays
    /// origin-complete. A missing origin is a typed error.
    #[test]
    fn delta_validate_rejects_add_without_origin() {
        let existing = CompiledEntityRegistry::default();
        let mut origins = OriginMap::new();
        origins.insert_variable(CompiledVariableId(0), var_origin());
        let batch = test_batch(
            vec![
                BackendOp::AddVariable(test_var(0)),
                BackendOp::AddLinearRow(test_row(0)),
            ],
            // origin for the variable, but NOT for the added row.
            origins,
        );
        assert!(matches!(
            batch.validate(&existing),
            Err(CompileError::MissingOrigin {
                entity: CompiledEntityRef::Constraint(CompiledConstraintId(0))
            })
        ));
    }

    /// F1: a valid ordered batch (add -> set -> remove -> add) still passes
    /// preflight — removal deletes the id so a later re-add is not a duplicate,
    /// and every add carries an origin.
    #[test]
    fn delta_validate_accepts_valid_ordered_batch() {
        let existing = CompiledEntityRegistry::default();
        let mut origins = OriginMap::new();
        origins.insert_variable(CompiledVariableId(0), var_origin());
        let batch = test_batch(
            vec![
                BackendOp::AddVariable(test_var(0)),
                BackendOp::SetVariableBounds {
                    variable: CompiledVariableId(0),
                    bounds: Bounds::new(0.0, 5.0),
                },
                BackendOp::RemoveVariable(CompiledVariableId(0)),
                BackendOp::AddVariable(test_var(0)),
            ],
            origins,
        );
        assert!(batch.validate(&existing).is_ok());
    }

    // ── Envelope validation (fifth review) ───────────────────────────────────

    /// Fifth review: a batch whose `from_compilation == to_compilation` is
    /// rejected with a typed [`CompileError::InvalidDeltaEnvelope`] BEFORE any
    /// op simulation — a batch that mutates state while retaining the old exact
    /// identity (D28) is malformed.
    #[test]
    fn delta_validate_rejects_identical_from_to_compilation() {
        let existing = CompiledEntityRegistry::default();
        let from = CompilationId::allocate().unwrap();
        let batch = BackendDeltaBatch {
            from_compilation: from,
            to_compilation: from,
            from_revision: ModelRevision::ZERO,
            to_revision: ModelRevision::from_u64(1),
            operations: vec![BackendOp::AddVariable(test_var(0))],
            origin_additions: {
                let mut origins = OriginMap::new();
                origins.insert_variable(CompiledVariableId(0), var_origin());
                origins
            },
            recipe_fingerprint: RecipeFingerprint::for_operations(&[]),
        };
        assert!(
            matches!(
                batch.validate(&existing),
                Err(CompileError::InvalidDeltaEnvelope { reason }) if reason.contains("identical")
            ),
            "expected InvalidDeltaEnvelope naming the identical ids, got {:?}",
            batch.validate(&existing)
        );
    }

    /// Fifth review: a batch whose `from_revision >= to_revision` is rejected
    /// with a typed [`CompileError::InvalidDeltaEnvelope`] — a batch must
    /// advance the canonical model revision.
    #[test]
    fn delta_validate_rejects_non_advancing_revision() {
        let existing = CompiledEntityRegistry::default();
        let batch = BackendDeltaBatch {
            from_compilation: CompilationId::allocate().unwrap(),
            to_compilation: CompilationId::allocate().unwrap(),
            // from == to (and would equally hold for from > to): no advance.
            from_revision: ModelRevision::from_u64(2),
            to_revision: ModelRevision::from_u64(2),
            operations: vec![BackendOp::AddVariable(test_var(0))],
            origin_additions: {
                let mut origins = OriginMap::new();
                origins.insert_variable(CompiledVariableId(0), var_origin());
                origins
            },
            recipe_fingerprint: RecipeFingerprint::for_operations(&[]),
        };
        assert!(
            matches!(
                batch.validate(&existing),
                Err(CompileError::InvalidDeltaEnvelope { reason }) if reason.contains("revision")
            ),
            "expected InvalidDeltaEnvelope naming the non-advancing revisions, got {:?}",
            batch.validate(&existing)
        );
    }

    /// Fifth review: a batch whose envelope ADVANCES both the compilation id
    /// (distinct from/to) and the revision (from < to) passes the envelope
    /// checks; the op-level F1 checks still apply on top.
    #[test]
    fn delta_validate_accepts_advancing_envelope() {
        let existing = CompiledEntityRegistry::default();
        let mut origins = OriginMap::new();
        origins.insert_variable(CompiledVariableId(0), var_origin());
        let batch = test_batch(vec![BackendOp::AddVariable(test_var(0))], origins);
        assert!(
            batch.validate(&existing).is_ok(),
            "an advancing envelope plus valid ops must pass preflight"
        );
    }

    // ── F2: unique dense ids + origin completeness in snapshot validation ────

    /// F2(a): a snapshot with a duplicate compiled variable id is rejected —
    /// dense compiled ids are unique by construction.
    #[test]
    fn snapshot_validate_rejects_duplicate_variable_ids() {
        let instance = crate::identity::ModelInstanceId::allocate().unwrap();
        let mut origins = OriginMap::new();
        origins.insert_variable(CompiledVariableId(0), var_origin());
        let snapshot = BackendSnapshotBuilder::new(instance, ModelRevision::ZERO)
            .origin_map(origins)
            .objective_policy(CompiledObjectivePolicy::None)
            .add_variable(test_var(0))
            .add_variable(test_var(0))
            .finalize()
            .expect("builder finalization does not check id uniqueness");
        assert!(matches!(
            snapshot.validate(),
            Err(CompileError::DuplicateEntity {
                entity: CompiledEntityRef::Variable(CompiledVariableId(0))
            })
        ));
    }

    /// F2(b): a snapshot with a gap in its compiled ids (0 and 2, missing 1) is
    /// rejected — compiled ids are deterministically dense (0..len per family).
    #[test]
    fn snapshot_validate_rejects_non_dense_ids() {
        let instance = crate::identity::ModelInstanceId::allocate().unwrap();
        let mut origins = OriginMap::new();
        origins.insert_variable(CompiledVariableId(0), var_origin());
        origins.insert_variable(CompiledVariableId(2), var_origin());
        let snapshot = BackendSnapshotBuilder::new(instance, ModelRevision::ZERO)
            .origin_map(origins)
            .objective_policy(CompiledObjectivePolicy::None)
            .add_variable(test_var(0))
            .add_variable(test_var(2))
            .finalize()
            .expect("builder finalization does not check density");
        assert!(matches!(
            snapshot.validate(),
            Err(CompileError::NonDenseCompilation {
                entity: CompiledEntityRef::Variable(CompiledVariableId(2))
            })
        ));
    }

    /// F2(c): a snapshot whose origin map is missing an entry for a compiled
    /// entity is rejected (D5/SM-02.5) — re-checked for directly-constructed
    /// snapshots.
    #[test]
    fn snapshot_validate_rejects_missing_origin() {
        let instance = crate::identity::ModelInstanceId::allocate().unwrap();
        let mut origins = OriginMap::new();
        origins.insert_variable(CompiledVariableId(0), var_origin());
        let mut snapshot = BackendSnapshotBuilder::new(instance, ModelRevision::ZERO)
            .origin_map(origins)
            .objective_policy(CompiledObjectivePolicy::None)
            .add_variable(test_var(0))
            .finalize()
            .expect("valid snapshot must build");
        // Strip the origin map to simulate a directly-constructed snapshot.
        snapshot.origin_map = OriginMap::new();
        assert!(matches!(
            snapshot.validate(),
            Err(CompileError::MissingOrigin {
                entity: CompiledEntityRef::Variable(CompiledVariableId(0))
            })
        ));
    }

    /// F2: a valid snapshot (unique dense ids, complete origins, no dangling
    /// references) passes validation — the compiler's own snapshots stay green.
    #[test]
    fn snapshot_validate_accepts_valid_compiler_snapshot() {
        let instance = crate::identity::ModelInstanceId::allocate().unwrap();
        let mut origins = OriginMap::new();
        origins.insert_variable(CompiledVariableId(0), var_origin());
        origins.insert_constraint(
            CompiledConstraintId(0),
            EntityOrigin::UserConstraint(ConId::new(0, Generation::new())),
        );
        let snapshot = BackendSnapshotBuilder::new(instance, ModelRevision::ZERO)
            .origin_map(origins)
            .objective_policy(CompiledObjectivePolicy::None)
            .add_variable(test_var(0))
            .add_linear_row(CompiledLinearRow {
                id: CompiledConstraintId(0),
                bounds: ConstraintBounds::le(10.0),
                coefficients: vec![(CompiledVariableId(0), 1.0)],
                name: None,
            })
            .finalize()
            .expect("valid snapshot must build");
        assert!(snapshot.validate().is_ok());
    }
}
