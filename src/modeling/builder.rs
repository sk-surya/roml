//! Mixed constant + parametric row batching (MIR-03, IR-22).
//!
//! [`RowBatch`] accumulates sink maps and their terms, then decides from
//! metadata whether the whole batch commits as one packed parametric row batch
//! or uses the general symbolic path. Cross-row canonical-cell disjointness is
//! the lowering's responsibility; core revalidation remains authoritative.

use crate::modeling::eligibility::{try_param_block_layout, SinkMap};
use crate::modeling::{CoeffView, Term, ViewError};
use crate::ModelInstanceId;

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
    rows: Vec<(SinkMap, Vec<Term>)>,
}

impl RowBatch {
    /// A new empty batch for `owner`.
    pub fn new(owner: ModelInstanceId) -> Self {
        Self {
            owner,
            rows: Vec::new(),
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

    /// Accumulate one sink. Rejects a variable or parameter coefficient owned
    /// by another model before any proof or ID reconstruction.
    pub fn push(&mut self, sink: SinkMap, terms: Vec<Term>) -> Result<(), ViewError> {
        for term in &terms {
            if term.vars.owner() != self.owner {
                return Err(ViewError::CrossModel {
                    left: self.owner,
                    right: term.vars.owner(),
                });
            }
            match &term.coeff {
                CoeffView::ScaledParam { params, .. } if params.owner() != self.owner => {
                    return Err(ViewError::CrossModel {
                        left: self.owner,
                        right: params.owner(),
                    });
                }
                _ => {}
            }
        }
        self.rows.push((sink, terms));
        Ok(())
    }

    /// Decide whether the whole batch can commit as one packed parametric batch.
    pub fn plan(&self) -> RowBatchPlan {
        let mut blocks = Vec::new();
        for (sink, terms) in &self.rows {
            match try_param_block_layout(sink, terms) {
                Some(layout) => blocks.extend(layout.blocks),
                None => {
                    if terms.iter().any(|term| term.coeff.is_parametric()) {
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
    use crate::id::Generation;
    use crate::modeling::eligibility::TargetRun;
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
    }

    fn param(owner: ModelInstanceId, len: usize) -> ParamView {
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
                params: param(owner, len),
            },
        }
    }

    fn sink(len: usize, row: Option<u32>) -> SinkMap {
        SinkMap::new(
            [len],
            vec![TargetRun {
                target: row.unwrap_or(0),
                objective: row.is_none(),
                start: 0,
                len,
                bases: vec![0],
            }],
        )
        .expect("sink")
    }

    #[test]
    fn disjoint_parametric_rows_commit_as_one_batch() {
        let owner = new_owner();
        let mut batch = RowBatch::new(owner);
        batch
            .push(sink(2, Some(0)), vec![param_term(owner, 0, 2)])
            .expect("row");
        batch
            .push(sink(2, Some(1)), vec![param_term(owner, 2, 2)])
            .expect("row");
        match batch.plan() {
            RowBatchPlan::Parametric(layout) => {
                assert_eq!(layout.blocks.len(), 2);
                assert_eq!(layout.blocks[0].row, Some(0));
                assert_eq!(layout.blocks[1].row, Some(1));
            }
            other => panic!("expected parametric batch, got {other:?}"),
        }
    }

    #[test]
    fn non_parametric_only_batch_is_general() {
        let owner = new_owner();
        let mut batch = RowBatch::new(owner);
        batch
            .push(
                sink(2, None),
                vec![Term {
                    vars: var(owner, 0, 2),
                    coeff: CoeffView::Scalar(1.0),
                }],
            )
            .expect("row");
        assert_eq!(batch.plan(), RowBatchPlan::General);
    }

    #[test]
    fn cross_model_term_is_rejected() {
        let owner = new_owner();
        let mut batch = RowBatch::new(owner);
        assert!(batch
            .push(sink(1, None), vec![param_term(new_owner(), 0, 1)])
            .is_err());
    }

    #[test]
    fn cross_model_parameter_coefficient_is_rejected() {
        let owner = new_owner();
        let mut batch = RowBatch::new(owner);
        // Variable belongs to `owner` but the parameter view belongs to another.
        let term = Term {
            vars: var(owner, 0, 1),
            coeff: CoeffView::ScaledParam {
                scale: 1.0,
                params: param(new_owner(), 1),
            },
        };
        assert!(batch.push(sink(1, None), vec![term]).is_err());
    }
}
