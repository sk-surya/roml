//! Sink-aware parametric eligibility proof (MIR-03, IR-21).
//!
//! [`try_param_block_layout`] is a **metadata-only, conservative** proof that a
//! parametric term can be stored as L2 [`ParamDepBlock`](crate::bulk::ParamDepBlockWitness)s
//! instead of per-cell `param_positions`. It produces a witness; core
//! revalidates it against post-canonical storage before accepting it (IR-12).
//!
//! The proof requires, for every array ordinal:
//! - the ordinal resolves to a canonical `(target, variable)` cell;
//! - no two terms (and no pre-existing contribution) reach the same cell;
//! - for each parametric term, the canonical cell run is contiguous in
//!   ordinal order and the parameter view strides are strictly positive.
//!
//! Anything uncertain returns `None` for the general symbolic path. Stride sign
//! and bounding ranges are never used as eligibility proofs on their own.

use std::collections::HashMap;

use crate::bulk::{ParamDepBlockWitness, ParamDepLayout, StridedMap};
use crate::id::VarId;
use crate::modeling::{CoeffView, Term};

/// One canonical coefficient cell: a sink target ordinal plus a variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CanonicalCell {
    /// Sink target ordinal (a row ordinal, or `0` for a single-target sink
    /// such as the objective).
    pub target: u32,
    /// Canonical variable.
    pub var: VarId,
}

/// Sink metadata the eligibility proof may consult (read-only).
pub trait SinkCells {
    /// Number of array ordinals in the sink.
    fn len(&self) -> usize;

    /// Whether the sink covers no ordinals.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Canonical cell written at array `ordinal`, if the ordinal is in the
    /// sink's canonical space.
    fn cell(&self, ordinal: usize) -> Option<CanonicalCell>;

    /// Canonical packed-run index for a cell, if the cell is retained.
    fn cell_index(&self, cell: CanonicalCell) -> Option<u32>;

    /// Whether the cell already has a contribution from outside `terms`.
    fn is_occupied(&self, _cell: CanonicalCell) -> bool {
        false
    }

    /// Row ordinal for a row-batch sink, or `None` for the objective.
    fn row(&self) -> Option<u32> {
        None
    }
}

/// Prove a conservative L2 dependency layout for `terms`, or `None` to use the
/// general symbolic path.
pub fn try_param_block_layout(sink: &impl SinkCells, terms: &[Term]) -> Option<ParamDepLayout> {
    let len = sink.len();
    if len == 0 {
        return None;
    }

    // Pass 1: every term must cover exactly the sink, resolve every ordinal to a
    // canonical cell, and no cell may be reached twice.
    let mut seen: HashMap<CanonicalCell, ()> = HashMap::new();
    for term in terms {
        if term.vars.view().len() != len {
            return None;
        }
        for ordinal in 0..len {
            let cell = sink.cell(ordinal)?;
            if sink.is_occupied(cell) {
                return None;
            }
            if seen.insert(cell, ()).is_some() {
                // Two contributions to one canonical cell: not packable.
                return None;
            }
        }
    }

    // Pass 2: one strided witness per parametric term.
    let mut blocks = Vec::new();
    for term in terms {
        let (scale, params) = match &term.coeff {
            CoeffView::ScaledParam { scale, params } => (*scale, params),
            _ => continue,
        };
        if params.view().len() != len {
            return None;
        }
        // Signed-strided parameter views are not proven by this conservative
        // proof; fall back rather than guess.
        if params.view().strides().iter().any(|stride| *stride <= 0) {
            return None;
        }
        // The canonical cell run must be contiguous in ordinal order so it maps
        // to `cell_map = contiguous(len)` at `cell_offset = first`.
        let first = sink.cell_index(sink.cell(0)?)?;
        for ordinal in 0..len {
            let index = sink.cell_index(sink.cell(ordinal)?)?;
            if index != first.checked_add(ordinal as u32)? {
                return None;
            }
        }
        let view = params.view();
        blocks.push(ParamDepBlockWitness {
            params: *view.span(),
            param_map: StridedMap::new(
                view.shape().to_vec(),
                view.strides().to_vec(),
                view.offset(),
            ),
            cell_offset: first,
            cell_map: StridedMap::contiguous(len),
            scale,
            row: sink.row(),
        });
    }

    if blocks.is_empty() {
        return None;
    }
    Some(ParamDepLayout { blocks })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bulk::{ParamSpan, VarSpan};
    use crate::id::Generation;
    use crate::modeling::{ParamView, VarView, View};
    use crate::ModelInstanceId;

    struct TestSink {
        cells: Vec<Option<CanonicalCell>>,
        indices: HashMap<CanonicalCell, u32>,
        occupied: Vec<CanonicalCell>,
        row: Option<u32>,
    }

    impl SinkCells for TestSink {
        fn len(&self) -> usize {
            self.cells.len()
        }
        fn cell(&self, ordinal: usize) -> Option<CanonicalCell> {
            self.cells.get(ordinal).copied().flatten()
        }
        fn cell_index(&self, cell: CanonicalCell) -> Option<u32> {
            self.indices.get(&cell).copied()
        }
        fn is_occupied(&self, cell: CanonicalCell) -> bool {
            self.occupied.contains(&cell)
        }
        fn row(&self) -> Option<u32> {
            self.row
        }
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

    fn params(owner: ModelInstanceId, len: usize) -> ParamView {
        ParamView::new(
            owner,
            View::contiguous(ParamSpan::from_parts(0, len as u32, Generation::new()), len),
        )
    }

    fn scalarm_term(owner: ModelInstanceId, vars: VarView, param_len: usize, scale: f64) -> Term {
        Term {
            vars,
            coeff: CoeffView::ScaledParam {
                scale,
                params: params(owner, param_len),
            },
        }
    }

    fn build_sink(cells: Vec<(u32, u32, u32)>) -> TestSink {
        // (target, var index, packed index)
        let mut out = Vec::new();
        let mut indices = HashMap::new();
        for (target, var_index, index) in cells {
            let cell = CanonicalCell {
                target,
                var: VarId::new(var_index, Generation::new()),
            };
            out.push(Some(cell));
            indices.insert(cell, index);
        }
        TestSink {
            cells: out,
            indices,
            occupied: Vec::new(),
            row: None,
        }
    }

    #[test]
    fn broadcast_over_rows_is_eligible() {
        let owner = ModelInstanceId::allocate().expect("owner");
        // 4 ordinals, each a distinct (row, var) cell, packed contiguously.
        let sink = build_sink(vec![(0, 0, 0), (1, 0, 1), (2, 0, 2), (3, 0, 3)]);
        let term = scalarm_term(owner, var(owner, 0, 4), 4, 2.5);
        let layout = try_param_block_layout(&sink, &[term]).expect("eligible");
        assert_eq!(layout.blocks.len(), 1);
        let block = &layout.blocks[0];
        assert_eq!(block.cell_offset, 0);
        assert_eq!(block.cell_map.len(), 4);
        assert_eq!(block.scale, 2.5);
        assert_eq!(block.row, None);
    }

    #[test]
    fn broadcast_into_one_objective_cell_is_ineligible() {
        let owner = ModelInstanceId::allocate().expect("owner");
        // Same (target, var) reached at every ordinal: one canonical cell.
        let sink = build_sink(vec![(0, 7, 0), (0, 7, 0), (0, 7, 0), (0, 7, 0)]);
        let term = scalarm_term(owner, var(owner, 7, 4), 4, 1.0);
        assert!(try_param_block_layout(&sink, &[term]).is_none());
    }

    #[test]
    fn two_params_one_cell_is_ineligible() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let sink = build_sink(vec![(0, 0, 0), (0, 1, 1)]);
        let first = scalarm_term(owner, var(owner, 0, 2), 2, 1.0);
        let mut second = scalarm_term(owner, var(owner, 0, 2), 2, 1.0);
        // Same ordinal -> same cell for both terms.
        second.vars = var(owner, 0, 2);
        assert!(try_param_block_layout(&sink, &[first, second]).is_none());
    }

    #[test]
    fn pre_occupied_cell_is_ineligible() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let mut sink = build_sink(vec![(0, 0, 0), (0, 1, 1)]);
        let cell = sink.cells[1].expect("cell");
        sink.occupied.push(cell);
        let term = scalarm_term(owner, var(owner, 0, 2), 2, 1.0);
        assert!(try_param_block_layout(&sink, &[term]).is_none());
    }

    #[test]
    fn non_monotone_parameter_stride_falls_back() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let sink = build_sink(vec![(0, 0, 0), (0, 1, 1), (0, 2, 2), (0, 3, 3)]);
        // Reversed parameter view -> negative stride -> conservative fallback.
        let base = params(owner, 4);
        let reversed = base.reverse(0).expect("reverse");
        let term = Term {
            vars: var(owner, 0, 4),
            coeff: CoeffView::ScaledParam {
                scale: 1.0,
                params: reversed,
            },
        };
        assert!(try_param_block_layout(&sink, &[term]).is_none());
    }

    #[test]
    fn interleaved_cell_indices_fall_back_conservatively() {
        let owner = ModelInstanceId::allocate().expect("owner");
        // Disjoint but interleaved packed indices are not a contiguous run here.
        let sink = build_sink(vec![(0, 0, 0), (0, 1, 2), (0, 2, 4), (0, 3, 6)]);
        let term = scalarm_term(owner, var(owner, 0, 4), 4, 1.0);
        assert!(try_param_block_layout(&sink, &[term]).is_none());
    }

    #[test]
    fn disjoint_contiguous_family_and_nonparametric_term() {
        let owner = ModelInstanceId::allocate().expect("owner");
        // A family packed at run offset 2..4.
        let sink = build_sink(vec![(1, 2, 2), (1, 3, 3)]);
        let family = scalarm_term(owner, var(owner, 2, 2), 2, 3.0);
        let layout = try_param_block_layout(&sink, &[family]).expect("family");
        assert_eq!(layout.blocks.len(), 1);
        assert_eq!(layout.blocks[0].cell_offset, 2);
        assert_eq!(layout.blocks[0].scale, 3.0);

        // A term with no parameter dependency is never a dependency block.
        let constant = Term {
            vars: var(owner, 0, 2),
            coeff: CoeffView::Scalar(1.0),
        };
        assert!(try_param_block_layout(&sink, &[constant]).is_none());
    }
}
