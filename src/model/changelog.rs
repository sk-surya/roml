//! Change tracking for incremental solver updates.
//! 
//! The model maintains an explicit ChangeLog capturing all mutations.
//! Solver adapters consume this log to apply incremental updates.
//! 
//! # Invariant
//! 
//! The model never mutates solver state directly. All changes go through the ChangeLog.

use crate::id::{CoeffId, ConId, ObjId, ParamId, VarId};
use crate::model::objective::Sense;
use crate::model::variable::{Bounds, VarType};
use crate::model::constraint::ConstraintBounds;


/// A single atomic change to the model.
/// 
/// Changes store both old and new values where applicable, enabling:
/// - Incremental updates to solver state (Smart solver delta computation)
/// - Debugging and auditing
pub enum Change {
    // ========== Variable Changes ==========
    // Variable was added.
    VariableAdded {
        var: VarId,
        bounds: Bounds,
        var_type: VarType,
    },

    // Variable was removed.
    VariableRemoved { var: VarId },

    // Variable bounds was changed.
    VariableBoundsChanged {
        var: VarId,
        old: Bounds,
        new: Bounds,
    },

    // Variable type was changed.
    VariableTypeChanged {
        var: VarId,
        old: VarType,
        new: VarType,
    },

    // Variable activity was toggled.
    VariableActivityChanged { var: VarId, active: bool },
    
    // ========== Constraint Changes ==========
    /// Constraint was added.
    ConstrainAdded {
        con: ConId,
        bounds: ConstraintBounds,
    },

    /// Constraint was removed.
    ConstraintRemoved { con: ConId },

    /// Constraint bounds were changed.
    ConstraintBoundsChanged {
        con: ConId,
        old: ConstraintBounds,
        new: ConstraintBounds,
    },

    /// Constraint activity was toggled.
    ConstraintActivityChanged { con: ConId, active: bool },

    // ========== Coefficient Changes ==========
    /// A coefficient was added.

    /// A coefficient was removed.

    /// A coefficient's value changed (due to parameter propagation or direct modification).

    // ========== Objective Changes ==========
    /// An objective was added.
    ObjectiveAdded { obj: ObjId, sense: Sense },

    /// An objective was removed.
    ObjectiveRemoved { obj: ObjId },

    /// Objective sense was changed.
    ObjectiveSenseChanged {
        obj: ObjId,
        old: Sense,
        new: Sense,
    },

    /// The active objective was changed.
    ActiveObjectiveChanged { 
        old: Option<ObjId>, 
        new: Option<ObjId> 
    },

    // ========== Parameter Changes ==========
    /// A parameter value was changed.
    ParameterValueChanged {
        param: ParamId,
        old: f64,
        new: f64,
    },
}


#[derive(Clone, Debug, Default)]
pub struct ChangeLog {
}