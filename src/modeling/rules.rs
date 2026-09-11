//! Rule-builder CSR accumulator (MIR-05, IR-26).
//!
//! Rule syntax is sugar over the frozen MIR-04 L1, not a second modeling
//! representation. A closure may execute once per index to *construct* row
//! expressions and push them here, but nothing is committed to the model until
//! [`Model::add_rules`](crate::Model::add_rules) turns the whole batch into one
//! self-contained [`RowBlockPlan`](crate::modeling::builder::RowBlockPlan) and
//! one packed `AddMixedRows` change. Per-index core insertion is a defect.

use crate::modeling::builder::{LocalRow, RowBatch, RowBatchPlan, RowBlockPlan};
use crate::modeling::{LinArray, ViewError};
use crate::ModelInstanceId;

/// A model-owned accumulator for rule-built rows.
///
/// Rows are assigned local ordinals in accumulation order. The batch borrows
/// no model state and performs no mutation; all work happens in the single
/// commit driven by `Model::add_rules`.
#[derive(Clone, Debug)]
pub struct RuleBatch {
    batch: RowBatch,
    next_row: u32,
}

impl RuleBatch {
    /// A new empty batch for `owner`.
    pub fn new(owner: ModelInstanceId) -> Self {
        Self {
            batch: RowBatch::new(owner),
            next_row: 0,
        }
    }

    /// Number of accumulated rows.
    pub fn len(&self) -> usize {
        self.batch.len()
    }

    /// Whether the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.batch.is_empty()
    }

    /// Accumulate one row: `Σ cells(coeffs)` between `lower` and `upper`.
    pub fn add_row(
        &mut self,
        coeffs: impl Into<LinArray>,
        lower: f64,
        upper: f64,
    ) -> Result<(), ViewError> {
        let row = self.next_row;
        let next = self
            .next_row
            .checked_add(1)
            .ok_or(ViewError::ShapeOverflow)?;
        self.batch
            .push(LocalRow { row, lower, upper }, coeffs.into())?;
        self.next_row = next;
        Ok(())
    }

    /// Accumulate one equality row.
    pub fn add_eq(&mut self, coeffs: impl Into<LinArray>, bound: f64) -> Result<(), ViewError> {
        self.add_row(coeffs, bound, bound)
    }

    /// Accumulate one `≤` row.
    pub fn add_le(&mut self, coeffs: impl Into<LinArray>, bound: f64) -> Result<(), ViewError> {
        self.add_row(coeffs, f64::NEG_INFINITY, bound)
    }

    /// Accumulate one `≥` row.
    pub fn add_ge(&mut self, coeffs: impl Into<LinArray>, bound: f64) -> Result<(), ViewError> {
        self.add_row(coeffs, bound, f64::INFINITY)
    }

    /// The single self-contained plan, or `None` when the batch is not
    /// packable and must use the general L1 path.
    pub(crate) fn into_plan(self) -> Option<RowBlockPlan> {
        match self.batch.plan() {
            RowBatchPlan::Planned(plan) => Some(plan),
            RowBatchPlan::General => None,
        }
    }
}
