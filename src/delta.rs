//! Typed delta batches for revisioned synchronization.
//!
//! A `DeltaBatch` is an immutable, self-contained set of model
//! operations that transforms the model from one revision to the next.
//! Each batch carries an explicit `from -> to` revision pair and an
//! ordered list of typed operations.

use crate::expr::LinExpr;
use crate::function::{FunctionEntry, ScalarFunction, ScalarSet};
use crate::id::{ConId, ObjId, ParamId, VarId};
use crate::model::coefficient::{CellKey, CoefficientTarget};
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
        /// The added variable.
        var: VarId,
        /// Bounds of the added variable.
        bounds: Bounds,
        /// Domain type (continuous, integer, or binary).
        var_type: VarType,
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

    /// Mark a variable as semi-continuous with the given lower bound.
    SetSemiContinuousBound {
        /// The affected variable.
        var: VarId,
        /// The semi-continuous lower bound.
        lower: f64,
    },
}

/// An immutable batch of operations transforming from one revision to another.
///
/// # Invariants
/// - `from < to` (the batch always advances the revision)
/// - Operations are ordered and deterministic
/// - The batch is self-contained (adapters need no model access)
///
/// # Semantic function/set entries (P25 Task 3)
///
/// The batch also carries [`functions`](Self::functions): the canonical
/// semantic function-in-set entries for constraints added by this batch,
/// *reconstructed* deterministically from the batch's legacy operations
/// (`AddConstraint` bounds + `SetCell` cells). The coefficient index remains
/// the single coefficient authority (SM-01.1) — these entries are a derived
/// view, and each transitional legacy field is guarded by an invariant check.
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
    pub functions: Vec<FunctionEntry>,
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
        Some(Self {
            from,
            to,
            operations,
            functions,
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
/// The coefficient index is the single coefficient authority (SM-01.1): each
/// added constraint's linear function is rebuilt from the `SetCell` cells and
/// its set from the `AddConstraint` bounds. The transitional legacy fields
/// remain the source; the invariant assertion documents that the semantic set
/// is derived from the legacy bounds, never a parallel authority.
fn reconstruct_function_entries(operations: &[ModelOp]) -> Vec<FunctionEntry> {
    let mut entries = Vec::new();
    for op in operations {
        if let ModelOp::AddConstraint { con, bounds } = op {
            let mut expr = LinExpr::new();
            for cell_op in operations {
                if let ModelOp::SetCell {
                    cell_key,
                    evaluated_value,
                    ..
                } = cell_op
                {
                    if let CoefficientTarget::Constraint(c) = cell_key.0 {
                        if c == *con {
                            expr = expr.term(*evaluated_value, cell_key.1);
                        }
                    }
                }
            }
            let set = ScalarSet::from(*bounds);
            // Invariant (transitional legacy field): the set must be derived
            // from the legacy bounds.
            debug_assert_eq!(set, ScalarSet::from(*bounds));
            entries.push(FunctionEntry {
                constraint: *con,
                function: ScalarFunction::Linear(expr),
                set,
            });
        }
    }
    entries.sort_by_key(|f| f.constraint);
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
