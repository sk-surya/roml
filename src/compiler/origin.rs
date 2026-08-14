//! Compiler origin mapping (design §4.4, §5; D5; SM-02.5).
//!
//! Every generated compiled entity must trace to a user entity, construct, or
//! solve overlay through [`EntityOrigin`]. [`OriginMap`] provides bidirectional
//! queries (compiled → origin and origin → compiled) and a completeness
//! validator used by [`BackendSnapshotBuilder`](crate::compiler::backend_ir::BackendSnapshotBuilder)
//! finalization: no compiled entity without an origin can be finalized.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::construct::Construct;
use crate::identity::IdentityOverflow;
use crate::model::{Constraint, Objective, Variable};
use crate::solver::infeasibility::{
    CompiledRestrictionRef, ConflictAtomId, InfeasibilityError, SemanticConflictUniverse,
    SemanticRestrictionAtom,
};

use super::backend_ir::{
    CompiledConstraintId, CompiledEntityRef, CompiledLinearRow, CompiledObjective,
    CompiledObjectiveId, CompiledVariable, CompiledVariableId,
};

/// Opaque overlay identity (design §4.4).
///
/// Solve overlays are solve-scoped and never advance canonical revision; their
/// generated entities carry a `SolveOverlay` origin with a distinct overlay id.
/// P27 introduces the overlay lifecycle; the id is declared here as the
/// origin-mapping foundation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OverlayId(u64);

/// Per-family checked atomic counter (zero reserved; 0 never issued).
static OVERLAY_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

impl OverlayId {
    /// Allocate a fresh opaque overlay id. The first issued id is 1; zero is
    /// reserved. Returns [`IdentityOverflow`] on counter exhaustion instead of
    /// wrapping. Used by [`SolveOverlay::new`](crate::solver::overlay::SolveOverlay::new)
    /// (P27) and referenced by every overlay-generated entity and receipt.
    pub(crate) fn allocate() -> Result<Self, IdentityOverflow> {
        match OVERLAY_ID_COUNTER.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |pre| {
            if pre == u64::MAX {
                None // saturated: do not advance; report overflow
            } else {
                Some(pre + 1)
            }
        }) {
            Ok(pre) => Ok(Self(pre + 1)),
            Err(_) => Err(IdentityOverflow),
        }
    }
}

/// The role a generated entity plays for its originating construct or overlay
/// (design §5).
///
/// An implementation-detail marker refined with the bridge tasks (P32/P33)
/// and the overlay tasks (P27). The enum is `#[non_exhaustive]`; P26 declared
/// it empty (no construct/overlay generated entities yet). P32 Task 15 added
/// the generic [`Bridge`](Self::Bridge) role; P32 Task 16 adds the
/// per-construct role variants for the logical constructs (indicator rows,
/// reification rows, Boolean rows, cardinality rows). P27 adds the
/// solve-overlay row roles; the enum stays `#[non_exhaustive]` so both
/// families can extend it without breaking match arms.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GeneratedRole {
    /// A generated bridge entity whose precise role is refined by the
    /// per-construct bridge modules. Task 15's `BridgeFinalizer` uses this
    /// generic role for every entity it generates.
    Bridge,
    /// An indicator implication row (the exact one-way Big-M row; also the
    /// exact row emitted when a qualified native indicator is selected — the
    /// P32 backend IR has no native-constraint representation, so the native
    /// selection is recorded as a formulation decision and the emitted exact
    /// row carries this role via `IndicatorNative`).
    IndicatorImplicationRow,
    /// A row generated under a qualified native `BackendFeature::Indicator`
    /// selection (distinct role so the selection is observable in the origin
    /// map).
    IndicatorNative,
    /// A reification implication row (`b = 1 ⇒ relation`).
    ReificationImplicationRow,
    /// A reification complement row (`b = 0 ⇒ relation complement`, honoring
    /// the separation tolerance).
    ReificationComplement,
    /// A Boolean implication row (`a ⇒ b`).
    BooleanImplicationRow,
    /// A Boolean equivalence row (`a ⟺ b`).
    BooleanEquivalenceRow,
    /// A Boolean any (at-least-one) row.
    BooleanAnyRow,
    /// A Boolean all (all-ones) row.
    BooleanAllRow,
    /// A cardinality row (exactly/at-most/at-least `k`).
    CardinalityRow,
    /// A max-epigraph row (`x_i <= y`, zero binaries — P32 Task 17a).
    MinMaxEpigraphRow,
    /// A min-hypograph row (`x_i >= y`, zero binaries — P32 Task 17a).
    MinMaxHypographRow,
    /// An exact min/max selector row (P32 Task 17a).
    MinMaxSelectorRow,
    /// An exact min/max selector binary (one per operand, sum = 1 — P32 Task 17a).
    MinMaxSelectorBinary,
    /// An exact absolute-value decomposition row (`z = p + n`, `p - n = x`).
    AbsoluteValueDecompositionRow,
    /// The exact absolute-value/positive-part nonnegativity row (`p <= M_p·b`).
    AbsoluteValuePositivePartRow,
    /// The exact absolute-value/positive-part negative-part row (`n <= M_n·(1-b)`).
    AbsoluteValueNegativePartRow,
    /// The exact absolute-value selector binary (`b`).
    AbsoluteValueSelectorBinary,
    /// A clamp inner max-selector row (P32 Task 17b).
    ClampInnerSelectorRow,
    /// A clamp inner max-selector binary (P32 Task 17b).
    ClampInnerSelectorBinary,
    /// A clamp outer min-selector row (P32 Task 17b).
    ClampOuterSelectorRow,
    /// A clamp outer min-selector binary (P32 Task 17b).
    ClampOuterSelectorBinary,
    /// A PWL convex-epigraph supporting-inequality row
    /// (`output >= v_i + s_i*(argument - x_i)`, zero binaries — P33 Task 2).
    PwlEpigraphRow,
    /// A PWL concave-hypograph supporting-inequality row
    /// (`output <= v_i + s_i*(argument - x_i)`, zero binaries — P33 Task 2).
    PwlHypographRow,
    /// A PWL exact-graph row (adjacency / convex-combination / selector rows —
    /// P33 Task 3).
    PwlExactGraphRow,
    /// A PWL exact-graph segment-adjacency binary (`z_k`, one per segment,
    /// `sum z = 1` — P33 Task 3).
    PwlSegmentBinary,
    /// A PWL exact-graph weight (convex-combination) variable (`lambda_i >= 0`,
    /// `sum lambda = 1` — P33 Task 3).
    PwlWeightVariable,
    /// A binary-binary product row (P32 Task 17c).
    BinaryProductRow,
    /// A binary-times-bounded-linear product row (P32 Task 17c).
    BinaryProductLinearRow,
    /// The binary-times-linear product's bound row (`w >= L·b` / `w <= U·b`,
    /// P32 Task 17c).
    BinaryProductBoundRow,
    /// A persistent soft constraint's lower-side nonnegative violation.
    SoftConstraintLowerViolationVariable,
    /// The exact lower-side soft constraint row (`f(x) + v_lo >= l`).
    SoftConstraintLowerViolationRow,
    /// A persistent soft constraint's upper-side nonnegative violation.
    SoftConstraintUpperViolationVariable,
    /// The exact upper-side soft constraint row (`f(x) - v_up <= u`).
    SoftConstraintUpperViolationRow,
    /// A temporary row added for an [`ObjectiveLock`](crate::solver::overlay::ObjectiveLock)
    /// (P27 Task 9).
    ObjectiveLockRow,
    /// A temporary row added for an [`ObjectiveCutoff`](crate::solver::overlay::ObjectiveCutoff)
    /// (P27 Task 9).
    CutoffRow,
}

/// The origin of a generated compiled entity (design §4.4, §5; D5).
///
/// Every compiled entity maps to exactly one origin: a user variable /
/// constraint / objective, a semantic construct (with a generated role), or a
/// solve overlay (with a generated role).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EntityOrigin {
    /// The compiled entity is a one-to-one projection of a user variable.
    UserVariable(Variable),
    /// The compiled entity is a one-to-one projection of a user constraint.
    UserConstraint(Constraint),
    /// The compiled entity is a one-to-one projection of a user objective.
    UserObjective(Objective),
    /// The compiled entity was generated for a semantic construct.
    Construct {
        /// The originating canonical construct.
        construct: Construct,
        /// The role the generated entity plays for that construct.
        role: GeneratedRole,
    },
    /// The compiled entity was generated for a solve-scoped overlay.
    SolveOverlay {
        /// The originating overlay.
        overlay: OverlayId,
        /// The role the generated entity plays for that overlay.
        role: GeneratedRole,
    },
}

/// Bidirectional compiled-entity ↔ origin mapping (design §5; D5).
///
/// Forward queries map a compiled id to its [`EntityOrigin`]; reverse queries
/// map an origin back to the compiled id(s) it produced. The completeness
/// validator flags any compiled entity missing an origin — the check the
/// snapshot builder uses at finalization.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OriginMap {
    variables: HashMap<CompiledVariableId, EntityOrigin>,
    constraints: HashMap<CompiledConstraintId, EntityOrigin>,
    objectives: HashMap<CompiledObjectiveId, EntityOrigin>,
}

impl OriginMap {
    /// Create an empty origin map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of recorded origin entries.
    pub fn len(&self) -> usize {
        self.variables.len() + self.constraints.len() + self.objectives.len()
    }

    /// Merge `other`'s origins into this map (P32 construct compilation
    /// merges each bridge's origin map into the session's).
    pub fn merge(&mut self, other: OriginMap) {
        self.variables.extend(other.variables);
        self.constraints.extend(other.constraints);
        self.objectives.extend(other.objectives);
    }

    /// True when no origin has been recorded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Record the origin of a compiled variable.
    pub fn insert_variable(&mut self, id: CompiledVariableId, origin: EntityOrigin) {
        self.variables.insert(id, origin);
    }

    /// Record the origin of a compiled row.
    pub fn insert_constraint(&mut self, id: CompiledConstraintId, origin: EntityOrigin) {
        self.constraints.insert(id, origin);
    }

    /// Record the origin of a compiled objective.
    pub fn insert_objective(&mut self, id: CompiledObjectiveId, origin: EntityOrigin) {
        self.objectives.insert(id, origin);
    }

    /// Look up the origin of a compiled variable (compiled → origin).
    pub fn variable_origin(&self, id: CompiledVariableId) -> Option<&EntityOrigin> {
        self.variables.get(&id)
    }

    /// Look up the origin of a compiled row (compiled → origin).
    pub fn constraint_origin(&self, id: CompiledConstraintId) -> Option<&EntityOrigin> {
        self.constraints.get(&id)
    }

    /// Look up the origin of a compiled objective (compiled → origin).
    pub fn objective_origin(&self, id: CompiledObjectiveId) -> Option<&EntityOrigin> {
        self.objectives.get(&id)
    }

    /// Look up the origin of a compiled entity by its reference (compiled →
    /// origin).
    pub fn origin_for(&self, entity: CompiledEntityRef) -> Option<&EntityOrigin> {
        match entity {
            CompiledEntityRef::Variable(id) => self.variable_origin(id),
            CompiledEntityRef::Constraint(id) => self.constraint_origin(id),
            CompiledEntityRef::Objective(id) => self.objective_origin(id),
        }
    }

    /// Compiled variables that trace to `origin` (origin → compiled).
    ///
    /// Deterministic: results are sorted by compiled id.
    pub fn variables_for_origin(&self, origin: &EntityOrigin) -> Vec<CompiledVariableId> {
        let mut out: Vec<_> = self
            .variables
            .iter()
            .filter(|(_, o)| *o == origin)
            .map(|(id, _)| *id)
            .collect();
        out.sort();
        out
    }

    /// Compiled rows that trace to `origin` (origin → compiled).
    ///
    /// Deterministic: results are sorted by compiled id.
    pub fn constraints_for_origin(&self, origin: &EntityOrigin) -> Vec<CompiledConstraintId> {
        let mut out: Vec<_> = self
            .constraints
            .iter()
            .filter(|(_, o)| *o == origin)
            .map(|(id, _)| *id)
            .collect();
        out.sort();
        out
    }

    /// Compiled objectives that trace to `origin` (origin → compiled).
    ///
    /// Deterministic: results are sorted by compiled id.
    pub fn objectives_for_origin(&self, origin: &EntityOrigin) -> Vec<CompiledObjectiveId> {
        let mut out: Vec<_> = self
            .objectives
            .iter()
            .filter(|(_, o)| *o == origin)
            .map(|(id, _)| *id)
            .collect();
        out.sort();
        out
    }

    /// All compiled entities (any kind) that trace to `origin` (origin →
    /// compiled).
    ///
    /// Deterministic: variables, then rows, then objectives, each sorted.
    pub fn compiled_for_origin(&self, origin: &EntityOrigin) -> Vec<CompiledEntityRef> {
        let mut out = Vec::new();
        out.extend(
            self.variables_for_origin(origin)
                .into_iter()
                .map(CompiledEntityRef::Variable),
        );
        out.extend(
            self.constraints_for_origin(origin)
                .into_iter()
                .map(CompiledEntityRef::Constraint),
        );
        out.extend(
            self.objectives_for_origin(origin)
                .into_iter()
                .map(CompiledEntityRef::Objective),
        );
        out
    }

    /// Completeness validator: return the compiled entities among
    /// `variables`/`linear_rows`/`objectives` that have no recorded origin.
    ///
    /// Used by builder finalization to enforce D5 — no generated entity
    /// without an origin. Deterministic: variables, then rows, then objectives,
    /// in declaration order.
    pub fn missing_origins(
        &self,
        variables: &[CompiledVariable],
        linear_rows: &[CompiledLinearRow],
        objectives: &[CompiledObjective],
    ) -> Vec<CompiledEntityRef> {
        let mut missing = Vec::new();
        for variable in variables {
            if !self.variables.contains_key(&variable.id) {
                missing.push(CompiledEntityRef::Variable(variable.id));
            }
        }
        for row in linear_rows {
            if !self.constraints.contains_key(&row.id) {
                missing.push(CompiledEntityRef::Constraint(row.id));
            }
        }
        for objective in objectives {
            if !self.objectives.contains_key(&objective.id) {
                missing.push(CompiledEntityRef::Objective(objective.id));
            }
        }
        missing
    }
}

/// Restriction-level origin map keyed by one exact compiled state.
#[derive(Clone, Debug, PartialEq)]
pub struct RestrictionOriginMap {
    compilation_id: crate::compiler::backend_ir::CompilationId,
    atoms: HashMap<ConflictAtomId, SemanticRestrictionAtom>,
    compiled: HashMap<CompiledRestrictionRef, Vec<ConflictAtomId>>,
}

impl RestrictionOriginMap {
    /// Build a restriction map from a complete semantic universe.
    pub fn new(universe: &SemanticConflictUniverse) -> Result<Self, InfeasibilityError> {
        let mut atoms = HashMap::new();
        let mut compiled: HashMap<CompiledRestrictionRef, Vec<ConflictAtomId>> = HashMap::new();
        for atom in &universe.atoms {
            if atoms.insert(atom.id, atom.clone()).is_some() {
                return Err(InfeasibilityError::InvalidUniverse {
                    reason: format!("duplicate conflict atom {:?}", atom.id),
                });
            }
            for member in &atom.compiled_restrictions {
                compiled.entry(*member).or_default().push(atom.id);
            }
        }
        Ok(Self {
            compilation_id: universe.compilation_id,
            atoms,
            compiled,
        })
    }

    /// Exact compilation identity guarded by this map.
    pub fn compilation_id(&self) -> crate::compiler::backend_ir::CompilationId {
        self.compilation_id
    }

    /// Map compiled membership to all semantic contribution atoms after an
    /// exact identity check. Multiple atoms are valid when a later bound layer
    /// (for example a persistent fixing) contributes to the same compiled
    /// lower/upper bound as its declared predecessor.
    pub fn map_compiled(
        &self,
        compilation_id: crate::compiler::backend_ir::CompilationId,
        member: CompiledRestrictionRef,
    ) -> Result<Vec<ConflictAtomId>, InfeasibilityError> {
        self.require_compilation(compilation_id)?;
        self.compiled
            .get(&member)
            .cloned()
            .ok_or_else(|| InfeasibilityError::InvalidUniverse {
                reason: format!("compiled restriction {:?} is not mapped", member),
            })
    }

    /// Resolve a semantic atom after an exact identity check.
    pub fn atom(
        &self,
        compilation_id: crate::compiler::backend_ir::CompilationId,
        id: ConflictAtomId,
    ) -> Result<&SemanticRestrictionAtom, InfeasibilityError> {
        self.require_compilation(compilation_id)?;
        self.atoms
            .get(&id)
            .ok_or_else(|| InfeasibilityError::InvalidUniverse {
                reason: format!("unknown conflict atom {:?}", id),
            })
    }

    fn require_compilation(
        &self,
        actual: crate::compiler::backend_ir::CompilationId,
    ) -> Result<(), InfeasibilityError> {
        if actual != self.compilation_id {
            return Err(InfeasibilityError::CompilationMismatch {
                expected: self.compilation_id,
                actual,
            });
        }
        Ok(())
    }
}
