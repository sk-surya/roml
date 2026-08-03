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

use std::sync::atomic::{AtomicU64, Ordering};

use crate::identity::{IdentityOverflow, ModelInstanceId};
use crate::model::{Bounds, ConstraintBounds, Sense, VarType};
use crate::revision::ModelRevision;

use super::origin::OriginMap;
use super::report::CompilationReport;
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
}
