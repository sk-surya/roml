//! Sink-aware parametric eligibility proof (MIR-03, IR-21).
//!
//! `roml::modeling` lowering presents a sink as `r -> (target(r), var_j(r))`:
//! ordinal `r` writes canonical cell `(target(r), var_j(r))` for each term `j`.
//! [`try_param_block_layout`] proves from **metadata only** that the parametric
//! terms admit L2 [`ParamDepBlockWitness`](crate::bulk::ParamDepBlockWitness)
//! families instead of per-cell `param_positions`; core revalidates the witness
//! against post-canonical storage before accepting it (IR-12).
//!
//! The proof is algebraic: it reasons over the sink target runs, each term's
//! `VarView` map, the parameter maps and the packed layout bases. It never
//! enumerates `Vec<CanonicalCell>` over the array. False positive = defect;
//! false negative = conservative fallback.
//!
//! Initial conservative conditions:
//! - every term's variable span is pairwise disjoint from the others (so no two
//!   terms can reach one canonical cell);
//! - each run's per-term packed ranges are pairwise disjoint (no interleaving);
//! - for a run of length > 1, the term's variable view and parameter view are
//!   contiguous dense (row-major canonical strides, hence injective) with
//!   strictly positive parameter strides;
//! - a run of length 1 is trivially injective and admits a broadcast (zero
//!   stride) variable or parameter.

use std::collections::HashSet;
use std::sync::Arc;

use crate::bulk::{ParamDepBlockWitness, ParamDepLayout, StridedMap};
use crate::modeling::{CoeffView, Term, ViewError};

/// One contiguous slice of array ordinals sharing one canonical sink target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetRun {
    /// Canonical target ordinal (a row ordinal, or the objective marker for an
    /// objective run).
    pub target: u32,
    /// Whether this run is an objective target (`row = None` in the witness) as
    /// opposed to a row target (`row = Some(target)`).
    pub objective: bool,
    /// First array ordinal covered by the run.
    pub start: usize,
    /// Number of array ordinals in the run.
    pub len: usize,
    /// Packed-run base of each term's family cells within this run, in `terms`
    /// order. `bases[j]` is the offset of term `j`'s first cell in the packed
    /// coefficient run.
    pub bases: Vec<u32>,
}

/// Sink metadata consumed by the eligibility proof.
///
/// The target map is compact: a partition of the array ordinals into target
/// runs, not a per-cell `target` vector, and never per-cell canonical cells.
#[derive(Clone, Debug)]
pub struct SinkMap {
    shape: Arc<[usize]>,
    runs: Vec<TargetRun>,
}

impl SinkMap {
    /// Build a sink map from a row-major shape and an ordered, exact cover of
    /// the ordinal range by target runs.
    pub fn new(shape: impl Into<Arc<[usize]>>, runs: Vec<TargetRun>) -> Result<Self, ViewError> {
        let shape: Arc<[usize]> = shape.into();
        let total = shape
            .iter()
            .try_fold(1usize, |acc, &dim| acc.checked_mul(dim))
            .ok_or(ViewError::ShapeOverflow)?;
        let mut cursor = 0usize;
        let mut row_targets: HashSet<u32> = HashSet::new();
        let mut objective_runs = 0usize;
        for run in &runs {
            if run.start != cursor {
                return Err(ViewError::Unsupported(
                    "sink runs must be contiguous and ordered",
                ));
            }
            cursor = cursor
                .checked_add(run.len)
                .ok_or(ViewError::ShapeOverflow)?;
            if run.objective {
                objective_runs += 1;
            } else if !row_targets.insert(run.target) {
                return Err(ViewError::Unsupported("duplicate row target in sink map"));
            }
        }
        if cursor != total {
            return Err(ViewError::Unsupported(
                "sink runs must cover the array exactly",
            ));
        }
        if objective_runs > 1 {
            return Err(ViewError::Unsupported("at most one objective run"));
        }
        Ok(Self { shape, runs })
    }

    /// The sink array shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// The target runs, in ordinal order.
    pub fn runs(&self) -> &[TargetRun] {
        &self.runs
    }
}

/// Whether a shape/stride view is contiguous dense under the row-major
/// convention (last dimension fastest), hence injective over any ordinal range.
fn is_contiguous_dense(shape: &[usize], strides: &[isize]) -> bool {
    if shape.len() != strides.len() || shape.is_empty() {
        return false;
    }
    let mut expected: isize = 1;
    for dim in (0..shape.len()).rev() {
        if strides[dim] != expected {
            return false;
        }
        expected = match expected.checked_mul(shape[dim] as isize) {
            Some(value) => value,
            None => return false,
        };
    }
    true
}

fn overlaps(a: (usize, usize), b: (usize, usize)) -> bool {
    a.0 < b.1 && b.0 < a.1
}

/// Prove a conservative L2 dependency layout for `terms`, or `None` to use the
/// general symbolic path.
pub fn try_param_block_layout(sink: &SinkMap, terms: &[Term]) -> Option<ParamDepLayout> {
    if terms.is_empty() {
        return None;
    }
    let total = sink
        .shape()
        .iter()
        .try_fold(1usize, |acc, &dim| acc.checked_mul(dim))?;

    // Every term must cover the whole sink and have a variable span disjoint
    // from every other term (conservative no-collision proof).
    let spans: Vec<(usize, usize)> = terms
        .iter()
        .map(|term| {
            let view = term.vars.view();
            if view.len() != total {
                return None;
            }
            let span = view.span();
            let start = span.start() as usize;
            let end = start.checked_add(span.len())?;
            Some((start, end))
        })
        .collect::<Option<_>>()?;
    for i in 0..spans.len() {
        for j in (i + 1)..spans.len() {
            if overlaps(spans[i], spans[j]) {
                return None;
            }
        }
    }

    let mut blocks = Vec::new();
    for run in sink.runs() {
        if run.bases.len() != terms.len() {
            return None;
        }
        let run_end = run.start.checked_add(run.len)?;
        // Per-term packed ranges within this run must be pairwise disjoint.
        for i in 0..terms.len() {
            for j in (i + 1)..terms.len() {
                let ri = (
                    run.bases[i] as usize,
                    (run.bases[i] as usize).checked_add(run.len)?,
                );
                let rj = (
                    run.bases[j] as usize,
                    (run.bases[j] as usize).checked_add(run.len)?,
                );
                if overlaps(ri, rj) {
                    return None;
                }
            }
        }

        for (index, term) in terms.iter().enumerate() {
            let (scale, params) = match &term.coeff {
                CoeffView::ScaledParam { scale, params } => (*scale, params),
                _ => continue,
            };
            let vars = term.vars.view();
            let pview = params.view();
            if run_end > vars.len() || pview.len() != vars.len() {
                return None;
            }
            let param_map = if run.len <= 1 {
                let offset = pview.get(run.start)?;
                if offset < 0 {
                    return None;
                }
                StridedMap::new([1usize], [1isize], offset)
            } else {
                // Injectivity over the run: contiguous dense variable and
                // parameter views with strictly positive strides.
                if !is_contiguous_dense(vars.shape(), vars.strides())
                    || !is_contiguous_dense(pview.shape(), pview.strides())
                    || pview.strides().iter().any(|stride| *stride <= 0)
                {
                    return None;
                }
                let offset = pview.offset().checked_add(run.start as isize)?;
                StridedMap::new([run.len], [1isize], offset)
            };
            blocks.push(ParamDepBlockWitness {
                params: *pview.span(),
                param_map,
                cell_offset: run.bases[index],
                cell_map: StridedMap::contiguous(run.len),
                scale,
                row: if run.objective {
                    None
                } else {
                    Some(run.target)
                },
            });
        }
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

    fn var(owner: ModelInstanceId, start: u32, len: usize) -> VarView {
        VarView::new(
            owner,
            View::contiguous(
                VarSpan::from_parts(start, len as u32, Generation::new()),
                len,
            ),
        )
    }

    fn param(owner: ModelInstanceId, start: u32, len: usize) -> ParamView {
        ParamView::new(
            owner,
            View::contiguous(
                ParamSpan::from_parts(start, len as u32, Generation::new()),
                len,
            ),
        )
    }

    fn param_term(vars: VarView, params: ParamView, scale: f64) -> Term {
        Term {
            vars,
            coeff: CoeffView::ScaledParam { scale, params },
        }
    }

    fn objective_run(len: usize, bases: Vec<u32>) -> TargetRun {
        TargetRun {
            target: 0,
            objective: true,
            start: 0,
            len,
            bases,
        }
    }

    #[test]
    fn bess_objective_two_terms_produce_two_families() {
        let owner = ModelInstanceId::allocate().expect("owner");
        // one objective target, disjoint charge/discharge views, same param view.
        let n = 4;
        let charge = var(owner, 0, n);
        let discharge = var(owner, n as u32, n);
        let price = param(owner, 0, n);
        let terms = vec![
            param_term(charge, price.clone(), -1.0),
            param_term(discharge, price, 1.0),
        ];
        let sink = SinkMap::new([n], vec![objective_run(n, vec![0, n as u32])]).expect("sink");
        let layout = try_param_block_layout(&sink, &terms).expect("eligible");
        assert_eq!(layout.blocks.len(), 2);
        assert_eq!(layout.blocks[0].cell_offset, 0);
        assert_eq!(layout.blocks[0].row, None);
        assert_eq!(layout.blocks[1].cell_offset, n as u32);
        assert_eq!(layout.blocks[1].scale, 1.0);
    }

    #[test]
    fn broadcast_over_rows_uses_honest_row_targets() {
        let owner = ModelInstanceId::allocate().expect("owner");
        // A broadcast (zero-stride) variable used once per distinct row target.
        let rows = 3usize;
        let broadcast = VarView::new(
            owner,
            View::new(
                VarSpan::from_parts(0, 1, Generation::new()),
                [rows],
                [0isize],
                0,
            )
            .expect("broadcast view"),
        );
        let p = param(owner, 0, rows);
        let terms = vec![param_term(broadcast, p, 2.0)];
        let runs: Vec<TargetRun> = (0..rows)
            .map(|b| TargetRun {
                target: b as u32,
                objective: false,
                start: b,
                len: 1,
                bases: vec![b as u32],
            })
            .collect();
        let sink = SinkMap::new([rows], runs).expect("sink");
        let layout = try_param_block_layout(&sink, &terms).expect("eligible");
        assert_eq!(layout.blocks.len(), rows);
        for (b, block) in layout.blocks.iter().enumerate() {
            assert_eq!(block.row, Some(b as u32));
            assert_eq!(block.cell_offset, b as u32);
            assert_eq!(block.cell_map.len(), 1);
        }
    }

    #[test]
    fn broadcast_into_one_objective_cell_is_ineligible() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let n = 3usize;
        let broadcast = VarView::new(
            owner,
            View::new(
                VarSpan::from_parts(0, 1, Generation::new()),
                [n],
                [0isize],
                0,
            )
            .expect("broadcast view"),
        );
        let terms = vec![param_term(broadcast, param(owner, 0, n), 1.0)];
        let sink = SinkMap::new([n], vec![objective_run(n, vec![0])]).expect("sink");
        assert!(try_param_block_layout(&sink, &terms).is_none());
    }

    #[test]
    fn overlapping_two_term_spans_are_ineligible() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let n = 3usize;
        let first = var(owner, 0, n);
        let second = var(owner, 1, n); // overlaps first
        let terms = vec![
            param_term(first, param(owner, 0, n), 1.0),
            param_term(second, param(owner, 0, n), 1.0),
        ];
        let sink = SinkMap::new([n], vec![objective_run(n, vec![0, n as u32])]).expect("sink");
        assert!(try_param_block_layout(&sink, &terms).is_none());
    }

    #[test]
    fn interleaved_or_overlapping_packed_bases_fall_back() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let n = 3usize;
        let terms = vec![
            param_term(var(owner, 0, n), param(owner, 0, n), 1.0),
            param_term(var(owner, n as u32, n), param(owner, 0, n), 1.0),
        ];
        // Both families claim the same packed base: overlapping ranges.
        let sink = SinkMap::new([n], vec![objective_run(n, vec![0, 0])]).expect("sink");
        assert!(try_param_block_layout(&sink, &terms).is_none());
    }

    #[test]
    fn non_monotone_parameter_stride_falls_back() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let n = 4usize;
        let reversed = param(owner, 0, n).reverse(0).expect("reverse");
        let terms = vec![param_term(var(owner, 0, n), reversed, 1.0)];
        let sink = SinkMap::new([n], vec![objective_run(n, vec![0])]).expect("sink");
        assert!(try_param_block_layout(&sink, &terms).is_none());
    }

    #[test]
    fn zero_stride_over_a_multi_ordinal_run_falls_back() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let n = 3usize;
        let broadcast_param = ParamView::new(
            owner,
            View::new(
                ParamSpan::from_parts(0, 1, Generation::new()),
                [n],
                [0isize],
                0,
            )
            .expect("broadcast param"),
        );
        let terms = vec![param_term(var(owner, 0, n), broadcast_param, 1.0)];
        let sink = SinkMap::new([n], vec![objective_run(n, vec![0])]).expect("sink");
        assert!(try_param_block_layout(&sink, &terms).is_none());
    }

    #[test]
    fn sink_map_rejects_bad_run_covers_and_duplicate_rows() {
        assert!(SinkMap::new([4usize], vec![]).is_err());
        assert!(SinkMap::new(
            [4usize],
            vec![TargetRun {
                target: 0,
                objective: false,
                start: 0,
                len: 3,
                bases: vec![0],
            }]
        )
        .is_err());
        let dup = vec![
            TargetRun {
                target: 0,
                objective: false,
                start: 0,
                len: 2,
                bases: vec![0],
            },
            TargetRun {
                target: 0,
                objective: false,
                start: 2,
                len: 2,
                bases: vec![2],
            },
        ];
        assert!(SinkMap::new([4usize], dup).is_err());
    }

    #[test]
    fn non_parametric_only_terms_have_no_layout() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let n = 2usize;
        let term = Term {
            vars: var(owner, 0, n),
            coeff: CoeffView::Scalar(1.0),
        };
        let sink = SinkMap::new([n], vec![objective_run(n, vec![0])]).expect("sink");
        assert!(try_param_block_layout(&sink, &[term]).is_none());
    }
}
