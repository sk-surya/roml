//! Typed delta batches for revisioned synchronization.
//!
//! A `DeltaBatch` is an immutable, self-contained set of model
//! operations that transforms the model from one revision to the next.
//! Each batch carries an explicit `from -> to` revision pair and an
//! ordered list of typed operations.

use crate::construct::{Construct, ConstructEntry, ConstructKind, FormulationPreference};
use crate::expr::{LinExpr, TermCoeff};
use crate::function::{FunctionEntry, ScalarFunction, ScalarSet};
use crate::id::{ConId, ObjId, ParamId, VarId};
use crate::model::coefficient::{CellKey, CoefficientTarget};
use crate::model::{Bounds, ConstraintBounds, Sense, VarType, VariableFixing};
use crate::revision::ModelRevision;
use crate::value_expr::ValueExpr;
use std::sync::Arc;

/// A packed block of constant linear constraint rows.
///
/// Canonical journal/delta payload for bulk row insertion (P1A): row
/// `r` owns `vars[row_ptr[r]..row_ptr[r+1]]` with the parallel `values`
/// slice, all canonical (sorted, unique, zero-dropped, finite — exactly
/// what the scalar row path stores). One `Arc` shares the whole block
/// across changelog, delta batch, and cursor fan-out.
#[derive(Clone, Debug, PartialEq)]
pub struct LinearRowBlock {
    /// Row identities in block order.
    pub constraints: Vec<ConId>,
    /// Final bounds per row.
    pub bounds: Vec<ConstraintBounds>,
    /// CSR row pointers over `vars`/`values` (`len == constraints.len()+1`).
    pub row_ptr: Vec<u32>,
    /// Canonical variable runs, concatenated.
    pub vars: Vec<VarId>,
    /// Evaluated constant coefficients, parallel to `vars`.
    pub values: Vec<f64>,
}

/// A packed block of parameterized linear constraint rows (MIR-02).
///
/// Canonical journal/delta payload for bulk parametric row insertion: row `r`
/// owns `vars[row_ptr[r]..row_ptr[r+1]]` with parallel `params`/`scales` and
/// `bounds[r]`; `values[r]` is the evaluated cache at insertion. Cells are
/// `scale * param`, exactly the canonical packed form.
#[derive(Clone, Debug, PartialEq)]
pub struct ParametricRowBlock {
    /// Row identities in block order.
    pub constraints: Vec<ConId>,
    /// Final bounds per row.
    pub bounds: Vec<ConstraintBounds>,
    /// CSR row pointers over the parallel arrays (`len == rows + 1`).
    pub row_ptr: Vec<u32>,
    /// Canonical variable runs, concatenated.
    pub vars: Vec<VarId>,
    /// Parameter run parallel to `vars`.
    pub params: Vec<ParamId>,
    /// Scale run parallel to `vars`.
    pub scales: Vec<f64>,
    /// Evaluated `scale * param` values parallel to `vars`.
    pub values: Vec<f64>,
}

/// A packed block of mixed constant + parametric constraint rows (MIR-03).
///
/// One constraint allocation and one semantic operation carry both the numeric
/// and the parametric canonical cells of the same rows, plus the derived L2
/// dependency layout over the parametric cells. Replay consumes this payload
/// alone: it never reruns L1 planning, the eligibility proof, or `RowBlockPlan`.
#[derive(Clone, Debug, PartialEq)]
pub struct MixedRowBlock {
    /// Row identities in block order (allocated once).
    pub constraints: Vec<ConId>,
    /// Final bounds per row (constants folded in).
    pub bounds: Vec<ConstraintBounds>,
    /// Numeric cells: CSR row pointers (`len == rows + 1`).
    pub numeric_ptr: Vec<u32>,
    /// Numeric canonical variables, concatenated.
    pub numeric_vars: Vec<VarId>,
    /// Numeric canonical values, parallel to `numeric_vars`.
    pub numeric_values: Vec<f64>,
    /// Parametric cells: CSR row pointers (`len == rows + 1`).
    pub parametric_ptr: Vec<u32>,
    /// Parametric canonical variables, concatenated.
    pub parametric_vars: Vec<VarId>,
    /// Parametric parameters, parallel to `parametric_vars`.
    pub parametric_params: Vec<ParamId>,
    /// Parametric scales, parallel to `parametric_vars`.
    pub parametric_scales: Vec<f64>,
    /// Evaluated `scale * param` cache, parallel to `parametric_vars`.
    pub parametric_values: Vec<f64>,
    /// L2 dependency layout over the parametric cells of these rows.
    pub layout: crate::bulk::ParamDepLayout,
}

/// One parameter value change inside a packed block update (MIR-02).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParameterValueChange {
    /// The affected parameter.
    pub param: ParamId,
    /// Previous value.
    pub old: f64,
    /// New value.
    pub new: f64,
}

/// One self-contained coefficient patch for an eligible dependency cell
/// (MIR-02). The patch carries the canonical target/variable and the new
/// evaluated value, so an adapter applies it with no live-model p-base
/// access.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoefficientPatch {
    /// The canonical cell target (constraint or objective).
    pub target: CoefficientTarget,
    /// The variable the coefficient multiplies.
    pub var: VarId,
    /// Previous evaluated value.
    pub old: f64,
    /// New evaluated value.
    pub new: f64,
}

/// One packed parameterized objective cell (P1C-2): the coefficient is
/// `scale * param`, evaluated at insertion and re-evaluated on parameter
/// updates through the packed reverse index (no per-cell expression).
/// `value` is the evaluated cache at the batch's revision, so backend
/// projection expands the block without parameter lookups — exactly like
/// [`ModelOp::SetObjectiveCells`] carries evaluated constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamCoeffCell {
    /// The variable this coefficient multiplies.
    pub var: VarId,
    /// Finite multiplier applied to the parameter.
    pub scale: f64,
    /// The parameter providing the value.
    pub param: ParamId,
    /// Evaluated `scale * parameter` at insertion.
    pub value: f64,
}

/// A typed model operation for solver synchronization.
///
/// Unlike the raw `Change` enum (which captures fine-grained events),
/// `ModelOp` values are self-contained — they carry all information
/// an adapter needs to apply the operation without consulting
/// adjacent events or model state.
///
/// # Variants
///
/// Each variant represents one atomic mutation that a solver adapter
/// can apply. Variants carry all data needed for the operation,
/// including both old and new values for change operations.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelOp {
    /// Add a new variable.
    AddVariable {
        /// The added variable.
        var: VarId,
        /// Bounds of the added variable.
        bounds: Bounds,
        /// Domain type (continuous, integer, or binary).
        var_type: VarType,
    },

    /// Add a packed block of variables (MIR-01).
    ///
    /// Compiled from `Change::VariableBlockAdded`; the shared payload is
    /// self-contained, so adapters expand it with no live-model access and the
    /// canonical delta stays packed.
    AddVariableBlock {
        /// The packed variable block (shared).
        block: Arc<crate::bulk::VariableBlock>,
    },

    /// Remove a variable and all associated cells.
    RemoveVariable {
        /// The removed variable.
        var: VarId,
    },

    /// Change variable bounds.
    SetVariableBounds {
        /// The affected variable.
        var: VarId,
        /// New bounds.
        bounds: Bounds,
    },

    /// Change a variable's persistent fixing (P27 Task 8, SM-05.2).
    ///
    /// Self-contained (mirrors [`Self::SetConstraintBounds`]): carries the
    /// `effective_bounds` the backend must apply. `Some(fixing)` is the
    /// equal-bound representation `[value, value]` (SM-05.3); `None` is an
    /// unfix that restores the **current** declared bounds (SM-05.4). The
    /// identity compiler lowers this to `BackendOp::SetVariableBounds` when
    /// the backend declares `BackendFeature::IncrementalBounds`, else selects
    /// a deterministic rebuild (D22, SM-05.7).
    SetVariableFixing {
        /// The affected variable.
        var: VarId,
        /// The new fixing, or `None` when the variable was unfixed.
        fixing: Option<VariableFixing>,
        /// The effective bounds to apply (equal bounds for a fix; the current
        /// declared bounds for an unfix).
        effective_bounds: Bounds,
    },

    /// Change variable activity.
    SetVariableActive {
        /// The affected variable.
        var: VarId,
        /// Whether the variable is now active.
        active: bool,
    },

    /// Change variable type.
    SetVariableType {
        /// The affected variable.
        var: VarId,
        /// New domain type.
        var_type: VarType,
    },

    /// Add a new constraint.
    AddConstraint {
        /// The added constraint.
        con: ConId,
        /// Bounds of the added constraint.
        bounds: ConstraintBounds,
    },

    /// Remove a constraint and all associated cells.
    RemoveConstraint {
        /// The removed constraint.
        con: ConId,
    },

    /// Change constraint bounds.
    SetConstraintBounds {
        /// The affected constraint.
        con: ConId,
        /// New bounds.
        bounds: ConstraintBounds,
    },

    /// Change constraint activity.
    SetConstraintActive {
        /// The affected constraint.
        con: ConId,
        /// Whether the constraint is now active.
        active: bool,
    },

    /// Add or update a coefficient cell.
    SetCell {
        /// The canonical cell coordinate.
        cell_key: CellKey,
        /// The cell's value expression (possibly parameter-dependent).
        value_expr: ValueExpr,
        /// Evaluated value at the batch's `to` revision.
        evaluated_value: f64,
    },

    /// Remove a coefficient cell.
    RemoveCell {
        /// The canonical cell coordinate.
        cell_key: CellKey,
    },

    /// Insert a packed block of constant linear rows at once.
    ///
    /// P1A bulk path: compiles from `Change::BulkLinearRows` so a
    /// hundred-thousand-row block journals and replays as one packed
    /// operation instead of per-row `AddConstraint` plus per-cell
    /// `SetCell` ops. Adapters expand it into at most one backend row op
    /// per row over already-evaluated values.
    AddLinearRows {
        /// The packed row block (shared).
        block: Arc<LinearRowBlock>,
    },

    /// Insert a packed block of parameterized linear rows at once (MIR-02).
    ///
    /// Cells are `scale * param`; adapters expand to at most one backend row
    /// op per row using the evaluated `values`.
    AddParametricRows {
        /// The packed parametric row block (shared).
        block: Arc<ParametricRowBlock>,
    },

    /// Insert a packed block of mixed constant + parametric linear rows at once
    /// (MIR-03, IR-22).
    ///
    /// One semantic operation on one row set: the numeric and parametric cells
    /// share the same allocated constraints and bounds. Adapters may expand it
    /// internally (add each row once, then install numeric/parametric
    /// coefficients) but must not create two independent logical row additions.
    AddMixedRows {
        /// The packed mixed row block (shared).
        block: Arc<MixedRowBlock>,
    },

    /// Add a new objective.
    AddObjective {
        /// The added objective.
        obj: ObjId,
        /// Minimize/maximize sense.
        sense: Sense,
    },

    /// Remove an objective.
    RemoveObjective {
        /// The removed objective.
        obj: ObjId,
    },

    /// Set the active objective.
    SetActiveObjective {
        /// The newly active objective, if any.
        obj: Option<ObjId>,
    },

    /// Update objective coefficient cell.
    SetObjectiveCell {
        /// The canonical cell coordinate.
        cell_key: CellKey,
        /// The cell's value expression (possibly parameter-dependent).
        value_expr: ValueExpr,
        /// Evaluated value at the batch's `to` revision.
        evaluated_value: f64,
        /// Objective constant (reported exactly once, API-03.5).
        constant: f64,
    },

    /// Insert a block of constant objective coefficient cells at once.
    ///
    /// P0 bulk path: compiles from `Change::BulkObjectiveCoefficients` so a
    /// million-cell objective journals and replays as one packed operation
    /// instead of a million `SetCell` ops. The payload is shared (`Arc`), so
    /// batch construction, journaling, and cursor fan-out clone a refcount
    /// rather than the block. Adapters expand it into per-cell backend
    /// operations in one tight loop over already-evaluated values.
    SetObjectiveCells {
        /// The objective the block belongs to.
        obj: ObjId,
        /// Packed `(variable, constant value)` cells in insertion order.
        cells: Arc<[(VarId, f64)]>,
    },

    /// Insert a block of parameterized objective coefficient cells at once.
    ///
    /// P1C-2 bulk path: compiles from
    /// `Change::BulkObjectiveParamCoefficients`. Each cell is
    /// `scale * parameter`; adapters expand the block like
    /// [`ModelOp::SetObjectiveCells`].
    SetObjectiveParamCells {
        /// The objective the block belongs to.
        obj: ObjId,
        /// Packed cells in insertion order.
        cells: Arc<[ParamCoeffCell]>,
    },

    /// Set the optimization sense of an objective.
    SetObjectiveSense {
        /// The affected objective.
        obj: ObjId,
        /// New minimize/maximize sense.
        sense: Sense,
    },

    /// Set the constant offset of an objective.
    ///
    /// Propagated on the incremental path so objective constants reach the
    /// backend exactly once (API-03.5); the rebuild path carries them in the
    /// snapshot's objective entries.
    SetObjectiveConstant {
        /// The affected objective.
        obj: ObjId,
        /// New constant value.
        constant: f64,
    },

    /// Set a parameter value (for solvers that need to know parameters).
    SetParameter {
        /// The affected parameter.
        param: ParamId,
        /// New value.
        value: f64,
    },

    /// Set a packed block of parameter values at once (MIR-02).
    ///
    /// One packed parameter-value change for a committed parameter block.
    SetParametersBulk {
        /// The packed parameter changes (shared).
        changes: Arc<[ParameterValueChange]>,
    },

    /// Apply one packed coefficient-patch batch (MIR-02).
    ///
    /// Emitted once per committed parameter block; the batch may contain
    /// patches from several eligible dependency families (for example charge
    /// and discharge). Every patch is self-contained.
    SetCoefficientPatchBatch {
        /// The packed coefficient patches (shared).
        patches: Arc<[CoefficientPatch]>,
    },

    /// Mark a variable as semi-continuous with the given lower bound.
    SetSemiContinuousBound {
        /// The affected variable.
        var: VarId,
        /// The semi-continuous lower bound.
        lower: f64,
    },

    /// Add a canonical semantic construct (design §7, P25 Task 4).
    ///
    /// Constructs are canonical entities, not backend rows; M3 v1 adapters
    /// treat this as a no-op (SM-01.6: no backend index/handle enters
    /// canonical state).
    AddConstruct {
        /// The added construct's stable identity.
        construct: Construct,
        /// The construct's exact semantic type.
        ///
        /// P25 (F3): `ConstructKind` is crate-private scaffolding; the field
        /// is hidden from the public docs and unusable by external consumers
        /// (they cannot name the type). It becomes a public export in P32.
        #[doc(hidden)]
        kind: ConstructKind,
        /// Per-construct formulation preference (F4).
        preference: FormulationPreference,
        /// Whether the construct is active.
        active: bool,
    },

    /// Remove a canonical semantic construct, invalidating its id.
    RemoveConstruct {
        /// The removed construct's identity.
        construct: Construct,
    },

    /// Toggle a construct's activity.
    SetConstructActive {
        /// The affected construct.
        construct: Construct,
        /// Whether the construct is now active.
        active: bool,
    },
}

/// An immutable batch of operations transforming from one revision to another.
///
/// # Invariants
/// - `from < to` (the batch always advances the revision)
/// - Operations are ordered and deterministic
/// - The batch is self-contained (adapters need no model access)
///
/// # Semantic function/set entries (P25 Task 3, F2 contract)
///
/// The batch also carries [`functions`](Self::functions): the canonical
/// semantic function-in-set entries for constraints **added** by this batch
/// with their final folded bounds (`SetConstraintBounds` folds, per CR-01),
/// **minus** constraints removed by this batch. Updates to pre-existing
/// functions ride the underlying ops; full before/after semantic entries for
/// pre-existing functions are deferred until recipe-level incremental
/// equivalence is proven (design §8). The coefficient index remains the single
/// coefficient authority (SM-01.1) — these entries are a derived view, and
/// each transitional legacy field is guarded by an invariant check.
#[derive(Clone, Debug, PartialEq)]
pub struct DeltaBatch {
    /// The revision before this batch is applied.
    pub from: ModelRevision,

    /// The revision after this batch is applied.
    pub to: ModelRevision,

    /// Ordered operations in this batch.
    pub operations: Vec<ModelOp>,

    /// Canonical semantic function-in-set entries reconstructed from the
    /// batch's `AddConstraint`/`SetCell` operations (P25 Task 3, SM-01.4).
    ///
    /// # Contract (F2)
    ///
    /// The view of constraints ADDED by this batch with their final folded
    /// bounds (`SetConstraintBounds` folds, per CR-01), minus constraints
    /// removed by this batch. Updates to pre-existing functions ride the
    /// underlying ops; full before/after semantic entries for pre-existing
    /// functions are deferred until recipe-level incremental equivalence is
    /// proven (design §8).
    pub functions: Vec<FunctionEntry>,

    /// Canonical semantic construct entries reconstructed from the batch's
    /// `AddConstruct`/`SetConstructActive` operations (P25 Task 4, SM-01.4).
    ///
    /// Crate-private (F3): `ConstructEntry` is not part of the public surface
    /// until P32.
    pub(crate) constructs: Vec<ConstructEntry>,
}

impl DeltaBatch {
    /// Create a new delta batch.
    ///
    /// Returns `None` if `from >= to`.
    pub fn new(from: ModelRevision, to: ModelRevision, operations: Vec<ModelOp>) -> Option<Self> {
        if from >= to {
            return None;
        }
        let functions = reconstruct_function_entries(&operations);
        let constructs = reconstruct_construct_entries(&operations);
        Some(Self {
            from,
            to,
            operations,
            functions,
            constructs,
        })
    }

    /// True if the batch is empty (no operations).
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Number of operations in the batch.
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// True if this batch is a no-op (same from/to).
    /// Note: from == to is prevented by construction, but this method
    /// exists for ergonomic checks.
    pub fn is_noop(&self) -> bool {
        self.operations.is_empty() && self.from == self.to
    }

    /// Check if this batch follows (immediately after) another batch.
    pub fn follows(&self, prev: &DeltaBatch) -> bool {
        self.from == prev.to
    }
}

/// Reconstruct the canonical semantic function-in-set entries carried by a
/// batch from its legacy `AddConstraint`/`SetCell` operations (P25 Task 3).
///
/// # Delta contract (F2)
///
/// `functions` is the view of constraints **added** by this batch with their
/// final folded bounds (`SetConstraintBounds` folds, per CR-01), **minus**
/// constraints removed by this batch (an `AddConstraint` followed by a
/// `RemoveConstraint` for the same `con` contributes no entry). Updates to
/// pre-existing functions ride the underlying ops
/// (`SetCell`/`SetConstraintBounds`/`RemoveConstraint`); full before/after
/// semantic entries for pre-existing functions are deferred until recipe-level
/// incremental equivalence is proven (design §8).
///
/// The coefficient index is the single coefficient authority (SM-01.1): each
/// added constraint's linear function is rebuilt from the `SetCell` cells and
/// its set from the `AddConstraint` bounds. The transitional legacy fields
/// remain the source; the invariant assertion documents that the semantic set
/// is derived from the legacy bounds, never a parallel authority.
///
/// # Complexity (P0.5)
///
/// One linear scan accumulates per-constraint state (declared bounds in
/// `AddConstraint` order, last-wins folded bounds, removal membership,
/// cells in encounter order); entries are then built per added constraint
/// with a per-row var sort. Total `O(operations + Σ row_terms log row_terms
/// + added log added)` — effectively linear for sparse rows — with
/// bit-identical output to the former per-constraint full scans (stable
/// sorts over the same encounter orders; see `reconstruction_lock_tests`).
fn reconstruct_function_entries(operations: &[ModelOp]) -> Vec<FunctionEntry> {
    use std::collections::{HashMap, HashSet};

    struct Accumulator {
        /// Declared bounds, one record per `AddConstraint` op in op order
        /// (constraint IDs are unique per model, so this is one record per
        /// added constraint; a pathological repeated add yields one entry
        /// per op, exactly as before).
        added: Vec<(ConId, ConstraintBounds)>,
        /// Constraints removed anywhere in this batch.
        removed: HashSet<ConId>,
        /// Last `SetConstraintBounds` per constraint wins (CR-01).
        folded: HashMap<ConId, ConstraintBounds>,
        /// `SetCell` terms per constraint target, in op encounter order.
        cells: HashMap<ConId, Vec<(VarId, ValueExpr)>>,
        /// Packed row blocks, in op encounter order (P1A).
        row_blocks: Vec<Arc<LinearRowBlock>>,
        /// Packed parametric row blocks, in op encounter order (MIR-02).
        param_row_blocks: Vec<Arc<ParametricRowBlock>>,
        /// Packed mixed row blocks, in op encounter order (MIR-03).
        mixed_row_blocks: Vec<Arc<MixedRowBlock>>,
    }
    let mut acc = Accumulator {
        added: Vec::new(),
        removed: HashSet::new(),
        folded: HashMap::new(),
        cells: HashMap::new(),
        row_blocks: Vec::new(),
        param_row_blocks: Vec::new(),
        mixed_row_blocks: Vec::new(),
    };
    for op in operations {
        match op {
            ModelOp::AddConstraint { con, bounds } => {
                acc.added.push((*con, *bounds));
            }
            ModelOp::RemoveConstraint { con } => {
                acc.removed.insert(*con);
            }
            ModelOp::SetConstraintBounds { con, bounds } => {
                acc.folded.insert(*con, *bounds);
            }
            ModelOp::SetCell {
                cell_key,
                value_expr,
                ..
            } => {
                if let CoefficientTarget::Constraint(c) = cell_key.0 {
                    // F1: terms stay symbolic (`TermCoeff::Expr`), so
                    // parameterized coefficients keep their form; the
                    // per-row sort below restores deterministic order.
                    acc.cells
                        .entry(c)
                        .or_default()
                        .push((cell_key.1, value_expr.clone()));
                }
            }
            ModelOp::AddLinearRows { block } => {
                acc.row_blocks.push(block.clone());
            }
            ModelOp::AddParametricRows { block } => {
                acc.param_row_blocks.push(block.clone());
            }
            ModelOp::AddMixedRows { block } => {
                acc.mixed_row_blocks.push(block.clone());
            }
            _ => {}
        }
    }

    let mut entries = Vec::with_capacity(acc.added.len());
    for (con, bounds) in &acc.added {
        // F2 (a): added AND removed within the same batch contributes nothing.
        if acc.removed.contains(con) {
            continue;
        }
        // CR-01: the last same-batch `SetConstraintBounds` is the effective
        // set authority.
        let effective = acc.folded.get(con).copied().unwrap_or(*bounds);
        let set = ScalarSet::from(effective);
        // Cloned per added-constraint record (not drained): a pathological
        // repeated `AddConstraint` for one id rebuilds from the same cells
        // for each occurrence, exactly as before.
        let mut symbolic: Vec<(VarId, ValueExpr)> = acc.cells.get(con).cloned().unwrap_or_default();
        symbolic.sort_by_key(|(var, _)| *var);
        let mut expr = LinExpr::new();
        for (var, value_expr) in symbolic {
            expr = expr.term(TermCoeff::Expr(value_expr), var);
        }
        entries.push(FunctionEntry {
            constraint: *con,
            function: ScalarFunction::Linear(expr),
            set,
        });
    }
    // P1A packed row blocks: one entry per row, honoring the same
    // removal and bounds-folding rules as scalar rows. Block rows are
    // canonical (sorted) by construction; the defensive re-sort keeps
    // bit-identity with the scalar path at negligible cost.
    for block in &acc.row_blocks {
        for r in 0..block.constraints.len() {
            let con = block.constraints[r];
            if acc.removed.contains(&con) {
                continue;
            }
            let effective = acc.folded.get(&con).copied().unwrap_or(block.bounds[r]);
            let set = ScalarSet::from(effective);
            let (s, e) = (block.row_ptr[r] as usize, block.row_ptr[r + 1] as usize);
            let mut symbolic: Vec<(VarId, ValueExpr)> = block.vars[s..e]
                .iter()
                .zip(&block.values[s..e])
                .map(|(var, value)| (*var, ValueExpr::constant(*value)))
                .collect();
            symbolic.sort_by_key(|(var, _)| *var);
            let mut expr = LinExpr::new();
            for (var, value_expr) in symbolic {
                expr = expr.term(TermCoeff::Expr(value_expr), var);
            }
            entries.push(FunctionEntry {
                constraint: con,
                function: ScalarFunction::Linear(expr),
                set,
            });
        }
    }
    // MIR-02 packed parametric rows: symbolic `scale * param` terms.
    for block in &acc.param_row_blocks {
        for r in 0..block.constraints.len() {
            let con = block.constraints[r];
            if acc.removed.contains(&con) {
                continue;
            }
            let effective = acc.folded.get(&con).copied().unwrap_or(block.bounds[r]);
            let set = ScalarSet::from(effective);
            let (s, e) = (block.row_ptr[r] as usize, block.row_ptr[r + 1] as usize);
            let mut symbolic: Vec<(VarId, ValueExpr)> = (s..e)
                .map(|k| {
                    (
                        block.vars[k],
                        ValueExpr::scaled_param(block.scales[k], block.params[k]),
                    )
                })
                .collect();
            symbolic.sort_by_key(|(var, _)| *var);
            let mut expr = LinExpr::new();
            for (var, value_expr) in symbolic {
                expr = expr.term(TermCoeff::Expr(value_expr), var);
            }
            entries.push(FunctionEntry {
                constraint: con,
                function: ScalarFunction::Linear(expr),
                set,
            });
        }
    }
    // MIR-03 packed mixed rows: constant + `scale * param` symbolic terms.
    for block in &acc.mixed_row_blocks {
        for r in 0..block.constraints.len() {
            let con = block.constraints[r];
            if acc.removed.contains(&con) {
                continue;
            }
            let effective = acc.folded.get(&con).copied().unwrap_or(block.bounds[r]);
            let set = ScalarSet::from(effective);
            let mut symbolic: Vec<(VarId, ValueExpr)> = Vec::new();
            let (ns, ne) = (
                block.numeric_ptr[r] as usize,
                block.numeric_ptr[r + 1] as usize,
            );
            for k in ns..ne {
                symbolic.push((
                    block.numeric_vars[k],
                    ValueExpr::constant(block.numeric_values[k]),
                ));
            }
            let (ps, pe) = (
                block.parametric_ptr[r] as usize,
                block.parametric_ptr[r + 1] as usize,
            );
            for k in ps..pe {
                symbolic.push((
                    block.parametric_vars[k],
                    ValueExpr::scaled_param(block.parametric_scales[k], block.parametric_params[k]),
                ));
            }
            symbolic.sort_by_key(|(var, _)| *var);
            let mut expr = LinExpr::new();
            for (var, value_expr) in symbolic {
                expr = expr.term(TermCoeff::Expr(value_expr), var);
            }
            entries.push(FunctionEntry {
                constraint: con,
                function: ScalarFunction::Linear(expr),
                set,
            });
        }
    }
    entries.sort_by_key(|f| f.constraint);
    entries
}

/// Reconstruct the canonical semantic construct entries carried by a batch
/// from its `AddConstruct`/`SetConstructActive`/`RemoveConstruct` operations
/// (P25 Task 4, design §7).
///
/// Constructs added by this batch are carried with their final activity after
/// any later `SetConstructActive` op; constructs removed within the batch are
/// excluded. Removed constructs are represented by the `RemoveConstruct` op.
fn reconstruct_construct_entries(operations: &[ModelOp]) -> Vec<ConstructEntry> {
    let mut entries = Vec::new();
    for op in operations {
        if let ModelOp::AddConstruct {
            construct,
            kind,
            preference,
            active,
        } = op
        {
            // A construct removed within the same batch is not carried.
            if operations
                .iter()
                .any(|o| matches!(o, ModelOp::RemoveConstruct { construct: c } if c == construct))
            {
                continue;
            }
            let mut final_active = *active;
            for later in operations {
                if let ModelOp::SetConstructActive {
                    construct: c,
                    active,
                } = later
                {
                    if c == construct {
                        final_active = *active;
                    }
                }
            }
            entries.push(ConstructEntry {
                id: *construct,
                kind: kind.clone(),
                active: final_active,
                // F4: the op carries the preference so the delta's construct
                // entry round-trips it.
                preference: *preference,
            });
        }
    }
    entries.sort_by_key(|e| e.id);
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_construction() {
        let r0 = ModelRevision::ZERO;
        let r1 = r0.next().unwrap();

        let batch = DeltaBatch::new(r0, r1, vec![]).unwrap();
        assert_eq!(batch.from, r0);
        assert_eq!(batch.to, r1);
        assert!(batch.is_empty());
    }

    #[test]
    fn batch_rejects_invalid_revisions() {
        let r0 = ModelRevision::ZERO;
        assert!(DeltaBatch::new(r0, r0, vec![]).is_none());
        let r1 = r0.next().unwrap();
        assert!(DeltaBatch::new(r1, r0, vec![]).is_none());
    }

    #[test]
    fn follows_detection() {
        let r0 = ModelRevision::ZERO;
        let r1 = r0.next().unwrap();
        let r2 = r1.next().unwrap();

        let b1 = DeltaBatch::new(r0, r1, vec![]).unwrap();
        let b2 = DeltaBatch::new(r1, r2, vec![]).unwrap();

        assert!(b2.follows(&b1));
        assert!(!b1.follows(&b2));
    }
}

#[cfg(test)]
mod reconstruction_lock_tests {
    //! P0.5 output locks: exact `functions` reconstruction semantics that the
    //! linearization must preserve bit-for-bit. These pass on the current
    //! quadratic implementation and must pass unchanged after.
    use super::*;
    use crate::id::Generation;

    fn var(i: u32) -> VarId {
        VarId::new(i, Generation::new())
    }
    fn con(i: u32) -> ConId {
        ConId::new(i, Generation::new())
    }
    fn param(i: u32) -> ParamId {
        ParamId::new(i, Generation::new())
    }
    fn rev_pair() -> (ModelRevision, ModelRevision) {
        let r0 = ModelRevision::ZERO;
        (r0, r0.next().unwrap())
    }

    #[test]
    fn parameterized_cells_keep_symbolic_form() {
        let (r0, r1) = rev_pair();
        let (v, p, c) = (var(7), param(3), con(41));
        let coeff = ValueExpr::param(p) * 2.0;
        let ops = vec![
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(10.0),
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: coeff.clone(),
                evaluated_value: 6.0,
            },
        ];
        let batch = DeltaBatch::new(r0, r1, ops).unwrap();
        assert_eq!(batch.functions.len(), 1);
        let f = &batch.functions[0];
        assert_eq!(f.constraint, c);
        assert_eq!(f.set, ScalarSet::from(ConstraintBounds::le(10.0)));
        let expected = LinExpr::new().term(TermCoeff::Expr(coeff), v);
        assert_eq!(f.function, ScalarFunction::Linear(expected));
    }

    #[test]
    fn add_update_remove_folds_and_excludes_exactly() {
        let (r0, r1) = rev_pair();
        let (v1, v2, v5, v9) = (var(1), var(2), var(5), var(9));
        let (c0, c1, c2) = (con(11), con(12), con(13));
        let ops = vec![
            ModelOp::AddConstraint {
                con: c1,
                bounds: ConstraintBounds::le(10.0),
            },
            // Cells arrive out of var order; output must be var-sorted.
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c1), v2),
                value_expr: ValueExpr::constant(2.0),
                evaluated_value: 2.0,
            },
            ModelOp::SetConstraintBounds {
                con: c1,
                bounds: ConstraintBounds::le(7.0),
            },
            ModelOp::AddConstraint {
                con: c2,
                bounds: ConstraintBounds::le(5.0),
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c2), v9),
                value_expr: ValueExpr::constant(9.0),
                evaluated_value: 9.0,
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c1), v1),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
            },
            // Update to a pre-existing constraint rides the ops only.
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c0), v5),
                value_expr: ValueExpr::constant(3.0),
                evaluated_value: 3.0,
            },
            // Added-then-removed constraint contributes no entry.
            ModelOp::RemoveConstraint { con: c2 },
        ];
        let batch = DeltaBatch::new(r0, r1, ops).unwrap();
        assert_eq!(batch.functions.len(), 1, "only c1 survives");
        let f = &batch.functions[0];
        assert_eq!(f.constraint, c1);
        assert_eq!(f.set, ScalarSet::from(ConstraintBounds::le(7.0)));
        let expected = LinExpr::new()
            .term(TermCoeff::Expr(ValueExpr::constant(1.0)), v1)
            .term(TermCoeff::Expr(ValueExpr::constant(2.0)), v2);
        assert_eq!(f.function, ScalarFunction::Linear(expected));
    }

    #[test]
    fn sparse_ids_reconstruct_exactly() {
        let (r0, r1) = rev_pair();
        let (v_a, v_b) = (var(1001), var(57));
        let (c_a, c_b) = (con(900), con(7));
        let ops = vec![
            ModelOp::AddConstraint {
                con: c_b,
                bounds: ConstraintBounds::eq(0.0),
            },
            ModelOp::AddConstraint {
                con: c_a,
                bounds: ConstraintBounds::ge(-3.0),
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c_a), v_a),
                value_expr: ValueExpr::constant(1.5),
                evaluated_value: 1.5,
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c_a), v_b),
                value_expr: ValueExpr::constant(-1.5),
                evaluated_value: -1.5,
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c_b), v_b),
                value_expr: ValueExpr::constant(4.0),
                evaluated_value: 4.0,
            },
        ];
        let batch = DeltaBatch::new(r0, r1, ops).unwrap();
        assert_eq!(batch.functions.len(), 2);
        // Entries sorted by constraint id regardless of op order.
        assert_eq!(batch.functions[0].constraint, c_b);
        assert_eq!(batch.functions[1].constraint, c_a);
        let expected_a = LinExpr::new()
            .term(TermCoeff::Expr(ValueExpr::constant(-1.5)), v_b)
            .term(TermCoeff::Expr(ValueExpr::constant(1.5)), v_a);
        assert_eq!(
            batch.functions[1].function,
            ScalarFunction::Linear(expected_a)
        );
        assert_eq!(
            batch.functions[1].set,
            ScalarSet::from(ConstraintBounds::ge(-3.0))
        );
    }
}
