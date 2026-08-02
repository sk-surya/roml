//! Change tracking for incremental solver updates.
//!
//! The model maintains an explicit ChangeLog capturing all mutations.
//! Solver adapters consume this log to apply incremental updates.
//!
//! # Invariant
//!
//! The model never mutates solver state directly. All changes go through the ChangeLog.

use crate::id::{CoeffId, ConId, ObjId, ParamId, VarId};
use crate::model::coefficient::CoefficientTarget;
use crate::model::constraint::ConstraintBounds;
use crate::model::objective::Sense;
use crate::model::variable::{Bounds, VarType};

/// A single atomic change to the model.
///
/// Changes store both old and new values where applicable, enabling:
/// - Incremental updates to solver state (Smart solver delta computation)
/// - Debugging and auditing
#[derive(Clone, Debug)]
pub enum Change {
    // ========== Variable Changes ==========
    /// A variable was added.
    VariableAdded {
        /// The added variable.
        var: VarId,
        /// Bounds of the added variable.
        bounds: Bounds,
        /// Domain type (continuous, integer, or binary).
        var_type: VarType,
    },

    /// A variable was removed.
    VariableRemoved {
        /// The removed variable.
        var: VarId,
    },

    /// A variable's bounds were changed.
    VariableBoundsChanged {
        /// The affected variable.
        var: VarId,
        /// Previous bounds.
        old: Bounds,
        /// New bounds.
        new: Bounds,
    },

    /// A variable's domain type was changed.
    VariableTypeChanged {
        /// The affected variable.
        var: VarId,
        /// Previous domain type.
        old: VarType,
        /// New domain type.
        new: VarType,
    },

    /// A variable's activity was toggled.
    VariableActivityChanged {
        /// The affected variable.
        var: VarId,
        /// Whether the variable is now active.
        active: bool,
    },

    /// A variable was marked semi-continuous with a lower bound.
    ///
    /// When the variable is nonzero, it must be ≥ `lower`; zero is also
    /// feasible.
    SemiContinuousBoundChanged {
        /// The affected variable.
        var: VarId,
        /// The semi-continuous lower bound.
        lower: f64,
    },

    // ========== Constraint Changes ==========
    /// A constraint was added.
    ConstraintAdded {
        /// The added constraint.
        con: ConId,
        /// Bounds of the added constraint.
        bounds: ConstraintBounds,
    },

    /// A constraint was removed.
    ConstraintRemoved {
        /// The removed constraint.
        con: ConId,
    },

    /// A constraint's bounds were changed.
    ConstraintBoundsChanged {
        /// The affected constraint.
        con: ConId,
        /// Previous bounds.
        old: ConstraintBounds,
        /// New bounds.
        new: ConstraintBounds,
    },

    /// A constraint's activity was toggled.
    ConstraintActivityChanged {
        /// The affected constraint.
        con: ConId,
        /// Whether the constraint is now active.
        active: bool,
    },

    // ========== Coefficient Changes ==========
    /// A coefficient was added.
    CoefficientAdded {
        /// The added coefficient cell.
        coeff: CoeffId,
        /// The variable the coefficient multiplies.
        var: VarId,
        /// The target row (constraint or objective).
        target: CoefficientTarget,
        /// The full expression (parameterized).
        value_expr: crate::value_expr::ValueExpr,
        /// Resolved value at time of addition.
        value: f64,
    },

    /// A coefficient was removed.
    CoefficientRemoved {
        /// The removed coefficient cell.
        coeff: CoeffId,
        /// The variable the coefficient multiplied.
        var: VarId,
        /// The target row (constraint or objective).
        target: CoefficientTarget,
    },

    /// A coefficient value changed (due to parameter propagation or direct
    /// modification).
    CoefficientValueChanged {
        /// The affected coefficient cell.
        coeff: CoeffId,
        /// The variable the coefficient multiplies.
        var: VarId,
        /// The target row (constraint or objective).
        target: CoefficientTarget,
        /// The full expression (parameterized).
        value_expr: crate::value_expr::ValueExpr,
        /// Previous resolved value.
        old: f64,
        /// New resolved value.
        new: f64,
    },

    // ========== Objective Changes ==========
    /// An objective was added.
    ObjectiveAdded {
        /// The added objective.
        obj: ObjId,
        /// Minimize/maximize sense.
        sense: Sense,
    },

    /// An objective was removed.
    ObjectiveRemoved {
        /// The removed objective.
        obj: ObjId,
    },

    /// An objective's sense was changed.
    ObjectiveSenseChanged {
        /// The affected objective.
        obj: ObjId,
        /// Previous sense.
        old: Sense,
        /// New sense.
        new: Sense,
    },

    /// The active objective was changed.
    ActiveObjectiveChanged {
        /// Previously active objective, if any.
        old: Option<ObjId>,
        /// Newly active objective, if any.
        new: Option<ObjId>,
    },

    /// The constant offset of an objective changed.
    ///
    /// Journaled so the incremental (delta) path propagates the constant to the
    /// backend exactly once (API-03.5); the snapshot path already carries it.
    ObjectiveConstantChanged {
        /// The affected objective.
        obj: ObjId,
        /// Previous constant.
        old: f64,
        /// New constant.
        new: f64,
    },

    // ========== Parameter Changes ==========
    /// A parameter value was changed.
    ///
    /// Note: This is followed by `CoefficientValueChanged` for each affected
    /// coefficient.
    ParameterValueChanged {
        /// The affected parameter.
        param: ParamId,
        /// Previous value.
        old: f64,
        /// New value.
        new: f64,
    },
}

impl Change {
    /// Check if this change affects solver state.
    ///
    /// Some changes (like parameter value changes) only affect coefficients
    /// and are tracked separately.
    pub fn affects_solver(&self) -> bool {
        !matches!(self, Change::ParameterValueChanged { .. })
    }
}

/// Tracks all changes since last solver sync.
#[derive(Clone, Debug, Default)]
pub(crate) struct ChangeLog {
    changes: Vec<Change>,
    /// Monotonically increasing sequence number.
    sequence: u64,
}

/// Methods used by Model and backend adapters.
#[allow(dead_code)]
impl ChangeLog {
    /// Create an empty changelog.
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
            sequence: 0,
        }
    }

    /// Push a change to the log.
    pub fn push(&mut self, change: Change) {
        self.changes.push(change);
        self.sequence += 1;
    }

    /// Take all changes, clearing the log.
    ///
    /// Used by solver adapters to consume pending changes.
    pub fn drain(&mut self) -> Vec<Change> {
        std::mem::take(&mut self.changes)
    }

    /// Peek at changes without consuming them.
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// Check if there are pending changes.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Get the number of pending changes.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Get the current sequence number.
    ///
    /// This increases with each change, allowing solvers to detect
    /// if they're behind without examining all changes.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Clear all changes without returning them.
    /// Sequence is not reset on clear, as it represents total changes since creation.
    pub fn clear(&mut self) {
        self.changes.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Generation;

    fn make_var(index: u32) -> VarId {
        VarId::new(index, Generation::new())
    }

    #[test]
    fn push_and_drain() {
        let mut log = ChangeLog::new();

        log.push(Change::VariableAdded {
            var: make_var(0),
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        });

        assert_eq!(log.len(), 1);
        assert_eq!(log.sequence(), 1);

        let changes = log.drain();
        assert_eq!(changes.len(), 1);
        assert!(log.is_empty());
        assert_eq!(log.sequence(), 1); // Sequence doesn't reset
    }

    #[test]
    fn sequence_monotonic() {
        let mut log = ChangeLog::new();

        for i in 0..5 {
            log.push(Change::VariableAdded {
                var: make_var(i),
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            });
        }

        assert_eq!(log.sequence(), 5);

        log.drain();

        log.push(Change::VariableRemoved { var: make_var(0) });
        assert_eq!(log.sequence(), 6);
    }
}
