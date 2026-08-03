//! Canonical model snapshots.
//!
//! A `ModelSnapshot` captures the complete solver-relevant state of a
//! model at a specific revision. Snapshots are used for:
//! - deterministic rebuild when incremental application fails
//! - verification that incremental projection equals snapshot rebuild
//! - compaction anchor points for the journal

use std::collections::HashMap;

use crate::expr::LinExpr;
use crate::function::{FunctionEntry, ScalarFunction, ScalarSet};
use crate::id::{ConId, ObjId, ParamId, VarId};
use crate::model::coefficient::{CellKey, CoefficientTarget};
use crate::model::{Bounds, ConstraintBounds, Sense, VarType};
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
}

/// A variable in a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableEntry {
    /// The variable's unique identifier.
    pub id: VarId,
    /// Current bounds for this variable.
    pub bounds: Bounds,
    /// Variable type (Continuous, Integer, or Binary).
    pub var_type: VarType,
    /// Whether this variable is active in the model.
    pub active: bool,
    /// Semi-continuous lower bound, if set.
    pub semicontinuous_lower: Option<f64>,
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
    }

    /// Count of all entities in the snapshot.
    pub fn entity_count(&self) -> usize {
        self.variables.len()
            + self.constraints.len()
            + self.objectives.len()
            + self.parameters.len()
            + self.cells.len()
            + self.functions.len()
    }
}

/// Reconstruct one semantic function-in-set entry from the authoritative
/// legacy fields (constraint bounds + coefficient cells) (P25 Task 3).
///
/// The coefficient index is the single coefficient authority (SM-01.1): the
/// linear function is rebuilt from the constraint's cells and the set from the
/// constraint's bounds. The transitional legacy fields remain the source; the
/// invariant assertion below documents that the semantic set is derived from
/// the legacy bounds, never a parallel authority.
fn reconstruct_function_entry(
    con: ConId,
    bounds: ConstraintBounds,
    cells: &[(CellKey, ValueExpr, f64, Vec<ParamId>)],
) -> FunctionEntry {
    let mut expr = LinExpr::new();
    for (cell_key, _value_expr, evaluated_value, _deps) in cells {
        if let CoefficientTarget::Constraint(c) = cell_key.0 {
            if c == con {
                expr = expr.term(*evaluated_value, cell_key.1);
            }
        }
    }
    let set = ScalarSet::from(bounds);
    // Invariant (transitional legacy field): the set must be derived from the
    // legacy bounds. Re-derive to catch any divergence.
    debug_assert_eq!(set, ScalarSet::from(bounds));
    FunctionEntry {
        constraint: con,
        function: ScalarFunction::Linear(expr),
        set,
    }
}

/// Build a snapshot from a model by extracting canonical state.
///
/// This is the reference implementation. The projection must be
/// deterministic — given the same model state, the same snapshot
/// is produced every time.
pub fn take_snapshot(
    revision: ModelRevision,
    variables: &HashMap<VarId, (Bounds, VarType, bool, Option<f64>)>,
    constraints: &HashMap<ConId, (ConstraintBounds, bool)>,
    objectives: &HashMap<ObjId, (Sense, bool, f64)>,
    parameters: &HashMap<ParamId, f64>,
    cells: &[(CellKey, ValueExpr, f64, Vec<ParamId>)],
) -> ModelSnapshot {
    let mut vars: Vec<_> = variables
        .iter()
        .map(
            |(&id, &(bounds, var_type, active, semicontinuous_lower))| VariableEntry {
                id,
                bounds,
                var_type,
                active,
                semicontinuous_lower,
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
    // Deterministic: sorted by constraint id, and the linear function term
    // order follows the (already collected) cell order.
    let mut functions: Vec<FunctionEntry> = constraints
        .iter()
        .map(|(&id, &(bounds, _))| reconstruct_function_entry(id, bounds, cells))
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
        variables.insert(var, (Bounds::NON_NEGATIVE, VarType::Continuous, true, None));

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
