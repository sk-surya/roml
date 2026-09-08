//! Canonical model snapshots.
//!
//! A `ModelSnapshot` captures the complete solver-relevant state of a
//! model at a specific revision. Snapshots are used for:
//! - deterministic rebuild when incremental application fails
//! - verification that incremental projection equals snapshot rebuild
//! - compaction anchor points for the journal

use std::collections::HashMap;

use crate::construct::ConstructEntry;
use crate::expr::{LinExpr, TermCoeff};
use crate::function::{FunctionEntry, ScalarFunction, ScalarSet};
use crate::id::{ConId, ObjId, ParamId, VarId};
use crate::model::coefficient::{CellKey, CoefficientTarget};
use crate::model::{Bounds, ConstraintBounds, Sense, VarType, VariableFixing};
use crate::revision::ModelRevision;
use crate::value_expr::ValueExpr;

/// A read-only snapshot of model state at a specific revision.
///
/// Contains all active entities and their solver-relevant attributes.
/// Snapshots are deterministic — two snapshots from the same model at
/// the same revision produce identical projections.
///
/// P25 (SM-01.4): the snapshot also carries the canonical semantic
/// function-in-set entries ([`functions`](Self::functions)). These are always
/// *reconstructed* from the authoritative coefficient cells and constraint
/// bounds (the single coefficient authority) — never stored independently.
/// The transitional legacy `constraint`/`cell` fields remain and every one is
/// guarded by an invariant check against the reconstructed function/set.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelSnapshot {
    /// The revision this snapshot was taken at.
    pub revision: ModelRevision,

    /// All variables with their current bounds, type, and activity.
    pub variables: Vec<VariableEntry>,

    /// All constraints with their current bounds and activity.
    pub constraints: Vec<ConstraintEntry>,

    /// All objectives with their sense and activation status.
    pub objectives: Vec<ObjectiveEntry>,

    /// All parameters with their current values.
    pub parameters: Vec<ParameterEntry>,

    /// All coefficient cells with their evaluated values.
    pub cells: Vec<CellEntry>,

    /// Canonical semantic function-in-set entries, reconstructed from the
    /// coefficient cells and constraint bounds (P25 Task 3, SM-01.4).
    pub functions: Vec<FunctionEntry>,

    /// Canonical semantic construct entries (design §7, P25 Task 4, SM-01.4).
    ///
    /// Populated by [`Model::take_snapshot`](crate::Model::take_snapshot) from
    /// the construct arena; the low-level [`take_snapshot`] projection starts
    /// empty because it receives no construct data.
    ///
    /// P25 (F3): `ConstructEntry` is crate-private until P32, so this field is
    /// `#[doc(hidden)]` and its elements are unusable by external consumers
    /// (they cannot name `ConstructEntry`). It is kept public only so external
    /// crates can build `ModelSnapshot` struct literals (a `pub(crate)` field
    /// would forbid struct-update construction entirely).
    #[doc(hidden)]
    pub constructs: Vec<ConstructEntry>,
}

/// A variable in a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableEntry {
    /// The variable's unique identifier.
    pub id: VarId,
    /// **Declared** bounds for this variable (P27 Task 8, SM-05.1).
    ///
    /// The solver-facing effective bounds fold any persistent fixing
    /// ([`fixing`](Self::fixing)) into `[value, value]`; the identity compiler
    /// performs that fold (SM-05.3). `bounds` remains the declared view so a
    /// rebuild can reconstruct both declared and effective state.
    pub bounds: Bounds,
    /// Variable type (Continuous, Integer, or Binary).
    pub var_type: VarType,
    /// Whether this variable is active in the model.
    pub active: bool,
    /// Semi-continuous lower bound, if set.
    pub semicontinuous_lower: Option<f64>,
    /// Optional persistent fixing (P27 Task 8, SM-05.1). Carried so the
    /// fixing survives `commit` → snapshot → rebuild (the phase gate).
    pub fixing: Option<VariableFixing>,
}

/// A constraint in a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstraintEntry {
    /// The constraint's unique identifier.
    pub id: ConId,
    /// Current bounds for this constraint.
    pub bounds: ConstraintBounds,
    /// Whether this constraint is active in the model.
    pub active: bool,
}

/// An objective in a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectiveEntry {
    /// The objective's unique identifier.
    pub id: ObjId,
    /// Optimization sense (minimize or maximize).
    pub sense: Sense,
    /// Whether this objective is currently active.
    pub active: bool,
    /// Objective constant term (the constant part of the expression).
    pub constant: f64,
}

/// A parameter in a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterEntry {
    /// The parameter's unique identifier.
    pub id: ParamId,
    /// Current numeric value of this parameter.
    pub value: f64,
}

/// A coefficient cell in a snapshot.
///
/// Each cell is the canonical (target, variable) pair with its
/// evaluated coefficient value.
#[derive(Clone, Debug, PartialEq)]
pub struct CellEntry {
    /// Canonical (target, variable) pair identifying this cell.
    pub cell_key: CellKey,
    /// The value expression (may depend on parameters).
    pub value_expr: ValueExpr,
    /// Pre-evaluated coefficient value at snapshot time.
    pub evaluated_value: f64,
    /// Parameter IDs this cell's expression depends on.
    pub dependencies: Vec<ParamId>,
}

impl ModelSnapshot {
    /// Create an empty snapshot at the given revision.
    pub fn empty(revision: ModelRevision) -> Self {
        Self {
            revision,
            variables: Vec::new(),
            constraints: Vec::new(),
            objectives: Vec::new(),
            parameters: Vec::new(),
            cells: Vec::new(),
            functions: Vec::new(),
            constructs: Vec::new(),
        }
    }

    /// True if the snapshot contains no entities.
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
            && self.constraints.is_empty()
            && self.objectives.is_empty()
            && self.parameters.is_empty()
            && self.cells.is_empty()
            && self.functions.is_empty()
            && self.constructs.is_empty()
    }

    /// Count of all entities in the snapshot.
    pub fn entity_count(&self) -> usize {
        self.variables.len()
            + self.constraints.len()
            + self.objectives.len()
            + self.parameters.len()
            + self.cells.len()
            + self.functions.len()
            + self.constructs.len()
    }
}

/// Reconstruct one semantic function-in-set entry from grouped cells.
///
/// Helper for [`take_snapshot`]: `terms` arrives in cell-slice encounter
/// order for exactly one constraint with already-resolved `bounds`. Sorting
/// here (stable, by var) reproduces the deterministic order the former
/// per-constraint full scan produced (WR-01).
fn build_function_entry(
    con: ConId,
    bounds: ConstraintBounds,
    mut terms: Vec<(VarId, ValueExpr)>,
) -> FunctionEntry {
    // F1: reconstruct the linear function SYMBOLICALLY — each term carries
    // `TermCoeff::Expr(ValueExpr)` sourced from the cell's `value_expr`, so a
    // parameterized coefficient keeps its symbolic form inside the function
    // (design §6). Dependencies are DERIVED from the function, never stored.
    terms.sort_by_key(|(var, _)| *var);
    let mut expr = LinExpr::new();
    for (var, value_expr) in terms {
        expr = expr.term(TermCoeff::Expr(value_expr), var);
    }
    FunctionEntry {
        constraint: con,
        function: ScalarFunction::Linear(expr),
        set: ScalarSet::from(bounds),
    }
}

/// A snapshot's per-variable record (P27 Task 8, SM-05.1):
/// `(declared bounds, type, active, semi-continuous lower, fixing)`.
///
/// The declared bounds and the optional persistent fixing are carried
/// separately so a rebuild can reconstruct both declared and effective state.
pub type SnapshotVariableRecord = (Bounds, VarType, bool, Option<f64>, Option<VariableFixing>);

/// Build a snapshot from a model by extracting canonical state.
///
/// This is the reference implementation. The projection must be
/// deterministic — given the same model state, the same snapshot
/// is produced every time.
pub fn take_snapshot(
    revision: ModelRevision,
    variables: &HashMap<VarId, SnapshotVariableRecord>,
    constraints: &HashMap<ConId, (ConstraintBounds, bool)>,
    objectives: &HashMap<ObjId, (Sense, bool, f64)>,
    parameters: &HashMap<ParamId, f64>,
    cells: &[(CellKey, ValueExpr, f64, Vec<ParamId>)],
) -> ModelSnapshot {
    let mut vars: Vec<_> = variables
        .iter()
        .map(
            |(&id, &(bounds, var_type, active, semicontinuous_lower, ref fixing))| VariableEntry {
                id,
                bounds,
                var_type,
                active,
                semicontinuous_lower,
                fixing: fixing.clone(),
            },
        )
        .collect();
    vars.sort_by_key(|v| v.id);

    let mut cons: Vec<_> = constraints
        .iter()
        .map(|(&id, &(bounds, active))| ConstraintEntry { id, bounds, active })
        .collect();
    cons.sort_by_key(|c| c.id);

    let mut objs: Vec<_> = objectives
        .iter()
        .map(|(&id, &(sense, active, constant))| ObjectiveEntry {
            id,
            sense,
            active,
            constant,
        })
        .collect();
    objs.sort_by_key(|o| o.id);

    let mut params: Vec<_> = parameters
        .iter()
        .map(|(&id, &value)| ParameterEntry { id, value })
        .collect();
    params.sort_by_key(|p| p.id);

    let mut c: Vec<_> = cells
        .iter()
        .map(
            |(cell_key, value_expr, evaluated_value, dependencies)| CellEntry {
                cell_key: *cell_key,
                value_expr: value_expr.clone(),
                evaluated_value: *evaluated_value,
                dependencies: dependencies.clone(),
            },
        )
        .collect();
    c.sort_by_key(|ce| ce.cell_key);

    // Reconstruct the canonical semantic function-in-set entries from the
    // authoritative legacy fields (constraint bounds + coefficient cells).
    // Deterministic: sorted by constraint id, and each row's linear function
    // term order is var-sorted (WR-01).
    //
    // P0.5: one linear grouping scan replaces the former per-constraint full
    // scan over all cells (`O(constraints × cells)`). Constraint-target
    // cells are grouped once in cell-slice encounter order; each row is then
    // built from its own group with the same stable var sort, so output is
    // bit-identical (see `reconstruction_lock_tests`). No cache is kept:
    // the groups are local temporaries, never a second authority.
    let mut grouped: HashMap<ConId, Vec<(VarId, ValueExpr)>> = HashMap::new();
    for (cell_key, value_expr, _, _) in cells.iter() {
        if let CoefficientTarget::Constraint(c) = cell_key.0 {
            grouped
                .entry(c)
                .or_default()
                .push((cell_key.1, value_expr.clone()));
        }
    }
    let mut functions: Vec<FunctionEntry> = constraints
        .iter()
        .map(|(&id, &(bounds, _))| {
            build_function_entry(id, bounds, grouped.remove(&id).unwrap_or_default())
        })
        .collect();
    functions.sort_by_key(|f| f.constraint);

    ModelSnapshot {
        revision,
        variables: vars,
        constraints: cons,
        objectives: objs,
        parameters: params,
        cells: c,
        functions,
        // The low-level projection receives no construct data; the canonical
        // Model::take_snapshot populates this from the construct arena.
        constructs: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Generation;
    use crate::model::coefficient::CoefficientTarget;

    fn make_var(index: u32) -> VarId {
        VarId::new(index, Generation::new())
    }
    fn make_con(index: u32) -> ConId {
        ConId::new(index, Generation::new())
    }

    #[test]
    fn empty_snapshot() {
        let snap = ModelSnapshot::empty(ModelRevision::ZERO);
        assert!(snap.is_empty());
        assert_eq!(snap.entity_count(), 0);
        assert_eq!(snap.revision, ModelRevision::ZERO);
    }

    #[test]
    fn snapshot_with_entities() {
        let var = make_var(0);
        let con = make_con(0);

        let mut variables = HashMap::new();
        variables.insert(
            var,
            (Bounds::NON_NEGATIVE, VarType::Continuous, true, None, None),
        );

        let mut constraints = HashMap::new();
        constraints.insert(con, (ConstraintBounds::le(10.0), true));

        let objectives = HashMap::new();
        let parameters = HashMap::new();

        let cells: Vec<(CellKey, ValueExpr, f64, Vec<ParamId>)> = vec![(
            (CoefficientTarget::Constraint(con), var),
            ValueExpr::constant(2.0),
            2.0,
            vec![],
        )];

        let snap = take_snapshot(
            ModelRevision::ZERO.next().unwrap(),
            &variables,
            &constraints,
            &objectives,
            &parameters,
            &cells,
        );

        assert!(!snap.is_empty());
        assert_eq!(snap.variables.len(), 1);
        assert_eq!(snap.constraints.len(), 1);
        assert_eq!(snap.cells.len(), 1);
        assert_eq!(snap.variables[0].bounds, Bounds::NON_NEGATIVE);
    }
}

#[cfg(test)]
mod reconstruction_lock_tests {
    //! P0.5 output locks for snapshot function reconstruction: exact entries
    //! the linearization must preserve bit-for-bit.
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

    #[test]
    fn snapshot_functions_group_sort_and_fold_exactly() {
        use crate::expr::TermCoeff;
        let (v9, v50, v2) = (var(9), var(50), var(2));
        let (c100, c3) = (con(100), con(3));
        let p = param(5);
        let coeff = ValueExpr::param(p) + 1.0;
        let mut constraints = HashMap::new();
        constraints.insert(c100, (ConstraintBounds::le(4.0), true));
        constraints.insert(c3, (ConstraintBounds::ge(1.0), true));
        // Cells arrive out of order across rows and within rows.
        let cells: Vec<(CellKey, ValueExpr, f64, Vec<ParamId>)> = vec![
            (
                (CoefficientTarget::Constraint(c100), v50),
                coeff.clone(),
                3.0,
                vec![p],
            ),
            (
                (CoefficientTarget::Constraint(c3), v2),
                ValueExpr::constant(2.0),
                2.0,
                vec![],
            ),
            (
                (CoefficientTarget::Constraint(c100), v9),
                ValueExpr::constant(1.0),
                1.0,
                vec![],
            ),
            // Objective-target cells never enter constraint functions.
            (
                (
                    CoefficientTarget::Objective(ObjId::new(0, Generation::new())),
                    v2,
                ),
                ValueExpr::constant(9.0),
                9.0,
                vec![],
            ),
        ];
        let snap = take_snapshot(
            ModelRevision::ZERO,
            &HashMap::new(),
            &constraints,
            &HashMap::new(),
            &HashMap::new(),
            &cells,
        );
        assert_eq!(snap.functions.len(), 2);
        assert_eq!(snap.functions[0].constraint, c3);
        assert_eq!(snap.functions[1].constraint, c100);
        // Parameterized symbolic form preserved, terms var-sorted.
        let expected = crate::expr::LinExpr::new()
            .term(TermCoeff::Expr(ValueExpr::constant(1.0)), v9)
            .term(TermCoeff::Expr(coeff), v50);
        assert_eq!(snap.functions[1].function, ScalarFunction::Linear(expected));
        assert_eq!(
            snap.functions[1].set,
            ScalarSet::from(ConstraintBounds::le(4.0))
        );
        // Legacy cells sorted by cell key.
        let keys: Vec<CellKey> = snap.cells.iter().map(|c| c.cell_key).collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
        let _ = v2;
    }
}
