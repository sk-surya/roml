//! Mixed constant + parametric row batching (MIR-03, IR-22).
//!
//! [`RowBatch`] accumulates rows and decides, from metadata only, whether the
//! whole batch can commit as one packed parametric row batch or must use the
//! general symbolic path. Rows with disjoint canonical cells stay fast; a
//! collision into one canonical cell (across rows, or with a cell already
//! committed) falls back without creating duplicate physical cells.

use std::collections::{HashMap, HashSet};

use crate::modeling::eligibility::{try_param_block_layout, CanonicalCell, SinkCells};
use crate::modeling::{Term, ViewError};
use crate::ModelInstanceId;

/// One row's canonical cells, in array-ordinal order.
#[derive(Clone, Debug)]
pub struct RowSink {
    row: u32,
    cells: Vec<CanonicalCell>,
    indices: HashMap<CanonicalCell, u32>,
}

impl RowSink {
    /// Build a row sink from `(cell, packed index)` pairs in ordinal order.
    ///
    /// Rejects a repeated cell (a row may not target one canonical cell twice).
    pub fn new(row: u32, cells: Vec<(CanonicalCell, u32)>) -> Result<Self, ViewError> {
        let mut ordered = Vec::with_capacity(cells.len());
        let mut indices = HashMap::with_capacity(cells.len());
        for (cell, index) in cells {
            if indices.insert(cell, index).is_some() {
                return Err(ViewError::Unsupported(
                    "duplicate canonical cell in one row",
                ));
            }
            ordered.push(cell);
        }
        Ok(Self {
            row,
            cells: ordered,
            indices,
        })
    }
}

impl SinkCells for RowSink {
    fn len(&self) -> usize {
        self.cells.len()
    }
    fn cell(&self, ordinal: usize) -> Option<CanonicalCell> {
        self.cells.get(ordinal).copied()
    }
    fn cell_index(&self, cell: CanonicalCell) -> Option<u32> {
        self.indices.get(&cell).copied()
    }
    fn row(&self) -> Option<u32> {
        Some(self.row)
    }
}

/// The decision for one accumulated row batch.
#[derive(Clone, Debug, PartialEq)]
pub enum RowBatchPlan {
    /// The batch commits as one packed parametric row batch.
    Parametric(crate::bulk::ParamDepLayout),
    /// At least one row must use the general symbolic path.
    General,
}

/// An accumulator for constant and parametric rows under one model.
#[derive(Clone, Debug)]
pub struct RowBatch {
    owner: ModelInstanceId,
    occupied: HashSet<CanonicalCell>,
    rows: Vec<(RowSink, Vec<Term>)>,
}

impl RowBatch {
    /// A new empty batch for `owner`.
    pub fn new(owner: ModelInstanceId) -> Self {
        Self {
            owner,
            occupied: HashSet::new(),
            rows: Vec::new(),
        }
    }

    /// Mark a canonical cell as already committed outside this batch.
    pub fn mark_occupied(&mut self, cell: CanonicalCell) {
        self.occupied.insert(cell);
    }

    /// Number of accumulated rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Accumulate one row. Rejects a term owned by another model immediately.
    pub fn push(&mut self, sink: RowSink, terms: Vec<Term>) -> Result<(), ViewError> {
        for term in &terms {
            if term.vars.owner() != self.owner {
                return Err(ViewError::CrossModel {
                    left: self.owner,
                    right: term.vars.owner(),
                });
            }
        }
        self.rows.push((sink, terms));
        Ok(())
    }

    /// Decide whether the whole batch can commit as one packed parametric batch.
    pub fn plan(&self) -> RowBatchPlan {
        let mut seen: HashSet<CanonicalCell> = HashSet::new();
        for (sink, _) in &self.rows {
            for ordinal in 0..sink.len() {
                let Some(cell) = sink.cell(ordinal) else {
                    return RowBatchPlan::General;
                };
                if self.occupied.contains(&cell) || !seen.insert(cell) {
                    return RowBatchPlan::General;
                }
            }
        }

        let mut blocks = Vec::new();
        for (sink, terms) in &self.rows {
            match try_param_block_layout(sink, terms) {
                Some(layout) => blocks.extend(layout.blocks),
                None => {
                    if terms.iter().any(|term| term.coeff.is_parametric()) {
                        // A parametric row that failed the metadata proof.
                        return RowBatchPlan::General;
                    }
                }
            }
        }
        if blocks.is_empty() {
            return RowBatchPlan::General;
        }
        RowBatchPlan::Parametric(crate::bulk::ParamDepLayout { blocks })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bulk::{ParamSpan, VarSpan};
    use crate::id::{Generation, VarId};
    use crate::modeling::{CoeffView, ParamView, VarView, View};

    fn var(owner: ModelInstanceId, start: u32, len: usize) -> VarView {
        VarView::new(
            owner,
            View::contiguous(
                VarSpan::from_parts(start, len as u32, Generation::new()),
                len,
            ),
        )
    }

    fn params(owner: ModelInstanceId, len: usize) -> ParamView {
        ParamView::new(
            owner,
            View::contiguous(ParamSpan::from_parts(0, len as u32, Generation::new()), len),
        )
    }

    fn param_term(owner: ModelInstanceId, start: u32, len: usize) -> Term {
        Term {
            vars: var(owner, start, len),
            coeff: CoeffView::ScaledParam {
                scale: 1.0,
                params: params(owner, len),
            },
        }
    }

    fn constant_term(owner: ModelInstanceId, start: u32, len: usize) -> Term {
        Term {
            vars: var(owner, start, len),
            coeff: CoeffView::Scalar(2.0),
        }
    }

    fn cell(row: u32, index: u32) -> CanonicalCell {
        CanonicalCell {
            target: row,
            var: VarId::new(index, Generation::new()),
        }
    }

    #[test]
    fn disjoint_parametric_rows_commit_as_one_batch() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let mut batch = RowBatch::new(owner);
        batch
            .push(
                RowSink::new(0, vec![(cell(0, 0), 0), (cell(0, 1), 1)]).expect("sink"),
                vec![param_term(owner, 0, 2)],
            )
            .expect("row");
        batch
            .push(
                RowSink::new(1, vec![(cell(1, 2), 2), (cell(1, 3), 3)]).expect("sink"),
                vec![param_term(owner, 2, 2)],
            )
            .expect("row");
        match batch.plan() {
            RowBatchPlan::Parametric(layout) => assert_eq!(layout.blocks.len(), 2),
            other => panic!("expected parametric batch, got {other:?}"),
        }
    }

    #[test]
    fn mixed_constant_and_parametric_rows_stay_fast() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let mut batch = RowBatch::new(owner);
        batch
            .push(
                RowSink::new(0, vec![(cell(0, 0), 0)]).expect("sink"),
                vec![constant_term(owner, 0, 1)],
            )
            .expect("row");
        batch
            .push(
                RowSink::new(1, vec![(cell(1, 1), 1)]).expect("sink"),
                vec![param_term(owner, 1, 1)],
            )
            .expect("row");
        match batch.plan() {
            RowBatchPlan::Parametric(layout) => assert_eq!(layout.blocks.len(), 1),
            other => panic!("expected parametric batch, got {other:?}"),
        }
    }

    #[test]
    fn collision_with_a_committed_cell_falls_back() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let mut batch = RowBatch::new(owner);
        batch.mark_occupied(cell(0, 0));
        batch
            .push(
                RowSink::new(0, vec![(cell(0, 0), 0)]).expect("sink"),
                vec![param_term(owner, 0, 1)],
            )
            .expect("row");
        assert_eq!(batch.plan(), RowBatchPlan::General);
    }

    #[test]
    fn repeated_cell_across_rows_falls_back() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let mut batch = RowBatch::new(owner);
        batch
            .push(
                RowSink::new(0, vec![(cell(9, 0), 0)]).expect("sink"),
                vec![param_term(owner, 0, 1)],
            )
            .expect("row");
        batch
            .push(
                RowSink::new(1, vec![(cell(9, 0), 0)]).expect("sink"),
                vec![param_term(owner, 1, 1)],
            )
            .expect("row");
        assert_eq!(batch.plan(), RowBatchPlan::General);
    }

    #[test]
    fn duplicate_cell_in_one_row_is_rejected() {
        let cell = cell(0, 0);
        assert!(RowSink::new(0, vec![(cell, 0), (cell, 1)]).is_err());
    }

    #[test]
    fn cross_model_row_is_rejected() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let other = ModelInstanceId::allocate().expect("owner");
        let mut batch = RowBatch::new(owner);
        assert!(batch
            .push(
                RowSink::new(0, vec![(cell(0, 0), 0)]).expect("sink"),
                vec![param_term(other, 0, 1)],
            )
            .is_err());
    }
}
