//! Canonical model snapshots.
//!
//! A `ModelSnapshot` captures the complete solver-relevant state of a
//! model at a specific revision. Snapshots are used for:
//! - deterministic rebuild when incremental application fails
//! - verification that incremental projection equals snapshot rebuild
//! - compaction anchor points for the journal

use std::collections::HashMap;

use crate::id::{ConId, ObjId, ParamId, VarId};
use crate::model::coefficient::CellKey;
use crate::model::{Bounds, ConstraintBounds, Sense, VarType};
use crate::revision::ModelRevision;
use crate::value_expr::ValueExpr;

/// A read-only snapshot of model state at a specific revision.
///
/// Contains all active entities and their solver-relevant attributes.
/// Snapshots are deterministic — two snapshots from the same model at
/// the same revision produce identical projections.
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
}

/// A variable in a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableEntry {
    pub id: VarId,
    pub bounds: Bounds,
    pub var_type: VarType,
    pub active: bool,
    /// Semi-continuous lower bound, if set.
    pub semicontinuous_lower: Option<f64>,
}

/// A constraint in a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstraintEntry {
    pub id: ConId,
    pub bounds: ConstraintBounds,
    pub active: bool,
}

/// An objective in a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectiveEntry {
    pub id: ObjId,
    pub sense: Sense,
    pub active: bool,
    /// Objective constant term (the constant part of the expression).
    pub constant: f64,
}

/// A parameter in a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterEntry {
    pub id: ParamId,
    pub value: f64,
}

/// A coefficient cell in a snapshot.
///
/// Each cell is the canonical (target, variable) pair with its
/// evaluated coefficient value.
#[derive(Clone, Debug, PartialEq)]
pub struct CellEntry {
    pub cell_key: CellKey,
    pub value_expr: ValueExpr,
    pub evaluated_value: f64,
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
        }
    }

    /// True if the snapshot contains no entities.
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
            && self.constraints.is_empty()
            && self.objectives.is_empty()
            && self.parameters.is_empty()
            && self.cells.is_empty()
    }

    /// Count of all entities in the snapshot.
    pub fn entity_count(&self) -> usize {
        self.variables.len()
            + self.constraints.len()
            + self.objectives.len()
            + self.parameters.len()
            + self.cells.len()
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

    ModelSnapshot {
        revision,
        variables: vars,
        constraints: cons,
        objectives: objs,
        parameters: params,
        cells: c,
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
