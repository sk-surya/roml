//! Typed delta batches for revisioned synchronization.
//!
//! A `DeltaBatch` is an immutable, self-contained set of model
//! operations that transforms the model from one revision to the next.
//! Each batch carries an explicit `from -> to` revision pair and an
//! ordered list of typed operations.

use crate::id::{ConId, ObjId, ParamId, VarId};
use crate::model::coefficient::CellKey;
use crate::model::{Bounds, ConstraintBounds, Sense, VarType};
use crate::revision::ModelRevision;
use crate::value_expr::ValueExpr;

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
        var: VarId,
        bounds: Bounds,
        var_type: VarType,
    },

    /// Remove a variable and all associated cells.
    RemoveVariable { var: VarId },

    /// Change variable bounds.
    SetVariableBounds { var: VarId, bounds: Bounds },

    /// Change variable activity.
    SetVariableActive { var: VarId, active: bool },

    /// Change variable type.
    SetVariableType { var: VarId, var_type: VarType },

    /// Add a new constraint.
    AddConstraint {
        con: ConId,
        bounds: ConstraintBounds,
    },

    /// Remove a constraint and all associated cells.
    RemoveConstraint { con: ConId },

    /// Change constraint bounds.
    SetConstraintBounds {
        con: ConId,
        bounds: ConstraintBounds,
    },

    /// Change constraint activity.
    SetConstraintActive { con: ConId, active: bool },

    /// Add or update a coefficient cell.
    SetCell {
        cell_key: CellKey,
        value_expr: ValueExpr,
        evaluated_value: f64,
    },

    /// Remove a coefficient cell.
    RemoveCell { cell_key: CellKey },

    /// Add a new objective.
    AddObjective { obj: ObjId, sense: Sense },

    /// Remove an objective.
    RemoveObjective { obj: ObjId },

    /// Set the active objective.
    SetActiveObjective { obj: Option<ObjId> },

    /// Update objective coefficient cell.
    SetObjectiveCell {
        cell_key: CellKey,
        value_expr: ValueExpr,
        evaluated_value: f64,
        /// Objective constant.
        constant: f64,
    },

    /// Set a parameter value (for solvers that need to know parameters).
    SetParameter { param: ParamId, value: f64 },
}

/// An immutable batch of operations transforming from one revision to another.
///
/// # Invariants
/// - `from < to` (the batch always advances the revision)
/// - Operations are ordered and deterministic
/// - The batch is self-contained (adapters need no model access)
#[derive(Clone, Debug, PartialEq)]
pub struct DeltaBatch {
    /// The revision before this batch is applied.
    pub from: ModelRevision,

    /// The revision after this batch is applied.
    pub to: ModelRevision,

    /// Ordered operations in this batch.
    pub operations: Vec<ModelOp>,
}

impl DeltaBatch {
    /// Create a new delta batch.
    ///
    /// Returns `None` if `from >= to`.
    pub fn new(from: ModelRevision, to: ModelRevision, operations: Vec<ModelOp>) -> Option<Self> {
        if from >= to {
            return None;
        }
        Some(Self {
            from,
            to,
            operations,
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
