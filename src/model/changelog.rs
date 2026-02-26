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
use crate::model::objective::Sense;
use crate::model::variable::{Bounds, VarType};
use crate::model::constraint::ConstraintBounds;


/// A single atomic change to the model.
/// 
/// Changes store both old and new values where applicable, enabling:
/// - Incremental updates to solver state (Smart solver delta computation)
/// - Debugging and auditing
#[derive(Clone, Debug)]
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
    ConstraintAdded {
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
    CoefficientAdded {
        coeff: CoeffId,
        var: VarId,
        target: CoefficientTarget,
        /// Resolved value at time of addition.
        value: f64,
    },


    /// A coefficient was removed.
    CoefficientRemoved {
        coeff: CoeffId,
        var: VarId,
        target: CoefficientTarget,
    },

    /// A coefficient's value changed (due to parameter propagation or direct modification).
    CoefficientValueChanged {
        coeff: CoeffId,
        var: VarId,
        target: CoefficientTarget,
        old: f64,
        new: f64,
    },

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
pub struct ChangeLog {
    changes: Vec<Change>,
    /// Monotonically increasing sequence number.
    sequence: u64,
}

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
    pub fn clear(&mut self) {
        self.changes.clear();
        // Should we reset sequence on clear? Probably not, as it represents total changes since creation.
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
