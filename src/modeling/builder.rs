//! Mixed constant + parametric row lowering plan (MIR-03, IR-22).
//!
//! [`RowBatch`] accumulates rows and produces a [`RowBlockPlan`] that owns the
//! complete L1→L2 commit metadata: local row topology, the numeric (constant)
//! packed cell stream, the parametric [`ParamDepLayout`], and the derived
//! canonical p-base offsets. A future core mixed-row commit consumes the plan
//! directly without rereading or reinterpreting raw [`Term`]s, and the model
//! layer never independently rediscovers canonical collisions.
//!
//! Collisions (a constant and a parametric contribution, or two parametric
//! contributions, reaching one `(row, variable)` cell) are detected by the
//! conservative variable-span disjointness proof and fall back to `General`.
//! A numeric-only batch stays representable by the numeric stream rather than
//! being mislabeled as an unsupported parametric case.

use std::collections::HashSet;

use crate::bulk::ParamDepLayout;
use crate::modeling::eligibility::{try_param_block_layout, SinkMap, TargetRun};
use crate::modeling::{CoeffView, LinArray, Term, ViewError};
use crate::ModelInstanceId;

/// One local row in a plan.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalRow {
    /// Local row index within the batch (canonical target ordinal).
    pub row: u32,
    /// Row lower bound.
    pub lower: f64,
    /// Row upper bound.
    pub upper: f64,
}

/// One numeric (constant) packed cell contribution.
#[derive(Clone, Debug, PartialEq)]
pub struct NumericCell {
    /// Local row index.
    pub row: u32,
    /// Canonical variable ordinal.
    pub var: u32,
    /// Numeric coefficient.
    pub value: f64,
}

/// A complete L1→L2 row lowering plan.
#[derive(Clone, Debug, PartialEq)]
pub struct RowBlockPlan {
    owner: ModelInstanceId,
    rows: Vec<LocalRow>,
    numeric: Vec<NumericCell>,
    parametric: ParamDepLayout,
}

impl RowBlockPlan {
    /// The owning model.
    pub fn owner(&self) -> ModelInstanceId {
        self.owner
    }

    /// Local row topology, in allocation order.
    pub fn rows(&self) -> &[LocalRow] {
        &self.rows
    }

    /// The numeric (constant) packed cell stream.
    pub fn numeric(&self) -> &[NumericCell] {
        &self.numeric
    }

    /// The parametric dependency layout (derived canonical p-base offsets).
    pub fn parametric(&self) -> &ParamDepLayout {
        &self.parametric
    }
}

/// The decision for one accumulated row batch.
#[derive(Clone, Debug, PartialEq)]
pub enum RowBatchPlan {
    /// A complete mixed or numeric row plan.
    Planned(RowBlockPlan),
    /// At least one row must use the general symbolic path.
    General,
}

/// An accumulator for constant and parametric rows under one model.
#[derive(Clone, Debug)]
pub struct RowBatch {
    owner: ModelInstanceId,
    rows: Vec<(LocalRow, LinArray)>,
    seen: HashSet<u32>,
}

impl RowBatch {
    /// A new empty batch for `owner`.
    pub fn new(owner: ModelInstanceId) -> Self {
        Self {
            owner,
            rows: Vec::new(),
            seen: HashSet::new(),
        }
    }

    /// Number of accumulated rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Accumulate one row. Rejects a foreign array or a repeated row target.
    pub fn push(&mut self, row: LocalRow, array: LinArray) -> Result<(), ViewError> {
        if array.owner() != self.owner {
            return Err(ViewError::CrossModel {
                left: self.owner,
                right: array.owner(),
            });
        }
        if !self.seen.insert(row.row) {
            return Err(ViewError::Unsupported("duplicate row target in batch"));
        }
        self.rows.push((row, array));
        Ok(())
    }

    /// Build the complete batch plan, or fall back to the general path.
    pub fn plan(&self) -> RowBatchPlan {
        let mut numeric = Vec::new();
        let mut blocks = Vec::new();
        for (row, array) in &self.rows {
            let len = array.len();
            if len == 0 {
                return RowBatchPlan::General;
            }
            let sink = match SinkMap::new(
                array.shape().to_vec(),
                vec![TargetRun {
                    target: row.row,
                    objective: false,
                    start: 0,
                    len,
                }],
            ) {
                Ok(sink) => sink,
                Err(_) => return RowBatchPlan::General,
            };
            match try_param_block_layout(&sink, array.terms()) {
                Some(layout) => blocks.extend(layout.blocks),
                None => {
                    if array.terms().iter().any(|term| term.coeff.is_parametric()) {
                        return RowBatchPlan::General;
                    }
                }
            }
            if emit_numeric(row.row, array.terms(), &mut numeric).is_none() {
                return RowBatchPlan::General;
            }
        }
        RowBatchPlan::Planned(RowBlockPlan {
            owner: self.owner,
            rows: self.rows.iter().map(|(row, _)| row.clone()).collect(),
            numeric,
            parametric: ParamDepLayout { blocks },
        })
    }
}

/// Emit the numeric packed stream for a row's non-parametric terms.
fn emit_numeric(row: u32, terms: &[Term], out: &mut Vec<NumericCell>) -> Option<()> {
    for term in terms {
        if term.coeff.is_parametric() {
            continue;
        }
        let view = term.vars.view();
        for ordinal in 0..view.len() {
            let offset = view.get(ordinal)?;
            let offset = u32::try_from(offset).ok()?;
            let var = view.span().start().checked_add(offset)?;
            let value = match &term.coeff {
                CoeffView::One => 1.0,
                CoeffView::Scalar(value) => *value,
                CoeffView::Dense { scale, values } => scale * values.get(ordinal)?,
                CoeffView::ScaledParam { .. } => continue,
            };
            out.push(NumericCell { row, var, value });
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bulk::{ParamSpan, VarSpan};
    use crate::id::Generation;
    use crate::modeling::{ParamView, VarView, View};

    fn new_owner() -> ModelInstanceId {
        ModelInstanceId::allocate().expect("owner")
    }

    fn var(owner: ModelInstanceId, start: u32, len: usize) -> VarView {
        VarView::new(
            owner,
            View::contiguous(
                VarSpan::from_parts(start, len as u32, Generation::new()),
                len,
            ),
        )
        .expect("var view")
    }

    fn param(owner: ModelInstanceId, len: usize) -> ParamView {
        ParamView::new(
            owner,
            View::contiguous(ParamSpan::from_parts(0, len as u32, Generation::new()), len),
        )
        .expect("param view")
    }

    fn numeric_term(owner: ModelInstanceId, start: u32, len: usize, value: f64) -> Term {
        Term {
            vars: var(owner, start, len),
            coeff: CoeffView::Scalar(value),
        }
    }

    fn param_term(owner: ModelInstanceId, start: u32, len: usize, scale: f64) -> Term {
        Term {
            vars: var(owner, start, len),
            coeff: CoeffView::ScaledParam {
                scale,
                params: param(owner, len),
            },
        }
    }

    fn row(index: u32) -> LocalRow {
        LocalRow {
            row: index,
            lower: 0.0,
            upper: 1.0,
        }
    }

    fn array(owner: ModelInstanceId, len: usize, terms: Vec<Term>) -> LinArray {
        LinArray::new(owner, [len], terms, crate::modeling::ConstantView::Zero).expect("array")
    }

    #[test]
    fn mixed_constant_and_parametric_row_plans_once() {
        let owner = new_owner();
        let n = 2usize;
        let mut batch = RowBatch::new(owner);
        let terms = vec![
            numeric_term(owner, 0, n, 2.0),
            param_term(owner, n as u32, n, 1.0),
        ];
        batch.push(row(0), array(owner, n, terms)).expect("row");
        match batch.plan() {
            RowBatchPlan::Planned(plan) => {
                assert_eq!(plan.rows().len(), 1);
                assert_eq!(plan.numeric().len(), n);
                assert_eq!(plan.parametric().blocks.len(), 1);
                assert_eq!(plan.parametric().blocks[0].row, Some(0));
                assert_eq!(plan.parametric().blocks[0].cell_offset, 0);
            }
            other => panic!("expected planned mixed batch, got {other:?}"),
        }
    }

    #[test]
    fn multiple_mixed_rows_allocate_each_target_once() {
        let owner = new_owner();
        let n = 2usize;
        let mut batch = RowBatch::new(owner);
        batch
            .push(
                row(0),
                array(
                    owner,
                    n,
                    vec![
                        numeric_term(owner, 0, n, 1.0),
                        param_term(owner, n as u32, n, 1.0),
                    ],
                ),
            )
            .expect("row 0");
        batch
            .push(
                row(1),
                array(
                    owner,
                    n,
                    vec![numeric_term(owner, 2, n, 3.0), param_term(owner, 4, n, 1.0)],
                ),
            )
            .expect("row 1");
        match batch.plan() {
            RowBatchPlan::Planned(plan) => {
                assert_eq!(plan.rows().len(), 2);
                assert_eq!(plan.numeric().len(), 2 * n);
                assert_eq!(plan.parametric().blocks.len(), 2);
                let rows: HashSet<u32> = plan.rows().iter().map(|r| r.row).collect();
                assert_eq!(rows, HashSet::from([0, 1]));
            }
            other => panic!("expected planned batch, got {other:?}"),
        }
    }

    #[test]
    fn constant_and_parametric_collision_falls_back() {
        let owner = new_owner();
        let n = 2usize;
        let mut batch = RowBatch::new(owner);
        // Both terms use the same variable span -> one canonical cell.
        let terms = vec![numeric_term(owner, 0, n, 2.0), param_term(owner, 0, n, 1.0)];
        batch.push(row(0), array(owner, n, terms)).expect("row");
        assert_eq!(batch.plan(), RowBatchPlan::General);
    }

    #[test]
    fn two_parametric_collision_falls_back() {
        let owner = new_owner();
        let n = 2usize;
        let mut batch = RowBatch::new(owner);
        let terms = vec![param_term(owner, 0, n, 1.0), param_term(owner, 0, n, 2.0)];
        batch.push(row(0), array(owner, n, terms)).expect("row");
        assert_eq!(batch.plan(), RowBatchPlan::General);
    }

    #[test]
    fn numeric_only_batch_uses_the_numeric_stream() {
        let owner = new_owner();
        let n = 2usize;
        let mut batch = RowBatch::new(owner);
        batch
            .push(
                row(0),
                array(owner, n, vec![numeric_term(owner, 0, n, 5.0)]),
            )
            .expect("row");
        match batch.plan() {
            RowBatchPlan::Planned(plan) => {
                assert!(plan.parametric().blocks.is_empty());
                assert_eq!(plan.numeric().len(), n);
                assert!(plan.numeric().iter().all(|cell| cell.value == 5.0));
            }
            other => panic!("expected numeric plan, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_row_target_is_rejected() {
        let owner = new_owner();
        let mut batch = RowBatch::new(owner);
        batch
            .push(
                row(0),
                array(owner, 1, vec![numeric_term(owner, 0, 1, 1.0)]),
            )
            .expect("row");
        assert!(batch
            .push(
                row(0),
                array(owner, 1, vec![numeric_term(owner, 1, 1, 1.0)])
            )
            .is_err());
    }

    #[test]
    fn foreign_array_is_rejected() {
        let owner = new_owner();
        let mut batch = RowBatch::new(owner);
        let other = new_owner();
        assert!(batch
            .push(
                row(0),
                array(other, 1, vec![numeric_term(other, 0, 1, 1.0)]),
            )
            .is_err());
    }
}
