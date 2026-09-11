//! Mixed constant + parametric row lowering plan (MIR-03, IR-22).
//!
//! [`RowBatch`] accumulates rows and produces a [`RowBlockPlan`] that owns the
//! complete L1→L2 commit metadata: local row topology (with constants folded
//! into the bounds), the numeric cell stream, the compact parametric family
//! stream, and the derived dependency layout. A future core mixed-row commit
//! consumes the plan alone — no `LinArray`/`Term` rereading — and the model
//! layer never independently rediscovers canonical collisions.
//!
//! Constant semantics (`LinArray::constant`) are preserved: a `Zero` constant
//! leaves the bounds unchanged, a `Scalar` (or a single-cell `Dense`) shifts
//! both bounds, and anything else falls back to the general path rather than
//! silently dropping a constant.
//!
//! Identity: numeric cells carry a [`VarId`] (index + generation) and the
//! parametric stream carries `VarSpan`/`ParamSpan` slices, so a plan can never
//! silently retarget a deleted or reallocated variable.

use std::collections::HashSet;

use crate::bulk::{ParamDepLayout, ParamSpan, StridedMap, VarSpan};
use crate::id::{ParamId, VarId};
use crate::modeling::eligibility::{try_param_block_layout, SinkMap, TargetRun};
use crate::modeling::{CoeffView, ConstantView, LinArray, Term, View, ViewError};
use crate::ModelInstanceId;

/// One local row in a plan, with constants already folded into the bounds.
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericCell {
    /// Local row index.
    pub row: u32,
    /// Materialized variable identity (index + generation).
    pub var: VarId,
    /// Numeric coefficient.
    pub value: f64,
}

/// A compact variable slice: trusted span + strided ordinal map.
#[derive(Clone, Debug, PartialEq)]
pub struct VarSlice {
    /// Trusted variable span (carries the fresh generation).
    pub span: VarSpan,
    /// Ordinal -> member offset within the span.
    pub map: StridedMap,
}

impl VarSlice {
    /// Number of mapped members.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the slice covers no members.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Reconstruct the member variable for a row-major ordinal.
    pub fn member(&self, ordinal: usize) -> Option<VarId> {
        let offset = usize::try_from(self.map.get(ordinal)?).ok()?;
        self.span.id_at(offset)
    }
}

/// A compact parameter slice: trusted span + strided ordinal map.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamSlice {
    /// Trusted parameter span.
    pub span: ParamSpan,
    /// Ordinal -> member offset within the span.
    pub map: StridedMap,
}

impl ParamSlice {
    /// Number of mapped members.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the slice covers no members.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Reconstruct the member parameter for a row-major ordinal.
    pub fn member(&self, ordinal: usize) -> Option<ParamId> {
        let offset = usize::try_from(self.map.get(ordinal)?).ok()?;
        self.span.id_at(offset)
    }
}

/// One compact parametric construction family: `row`, variable slice,
/// parameter slice and `scale`. Core builds `scale * params[r]` cells at
/// `(row, vars[r])` for each family ordinal.
#[derive(Clone, Debug, PartialEq)]
pub struct ParametricFamily {
    /// Local row target, or `None` for the objective.
    pub row: Option<u32>,
    /// Variables the family multiplies.
    pub vars: VarSlice,
    /// Parameters the family reads.
    pub params: ParamSlice,
    /// Coefficient scale.
    pub scale: f64,
}

/// A complete, self-contained L1→L2 row lowering plan.
#[derive(Clone, Debug, PartialEq)]
pub struct RowBlockPlan {
    owner: ModelInstanceId,
    rows: Vec<LocalRow>,
    numeric: Vec<NumericCell>,
    families: Vec<ParametricFamily>,
    parametric: ParamDepLayout,
}

impl RowBlockPlan {
    /// The owning model.
    pub fn owner(&self) -> ModelInstanceId {
        self.owner
    }

    /// Local row topology, with constants folded into the bounds.
    pub fn rows(&self) -> &[LocalRow] {
        &self.rows
    }

    /// The numeric (constant) packed cell stream.
    pub fn numeric(&self) -> &[NumericCell] {
        &self.numeric
    }

    /// The compact parametric construction stream.
    pub fn families(&self) -> &[ParametricFamily] {
        &self.families
    }

    /// The parametric dependency witness (derived canonical p-base offsets).
    pub fn parametric(&self) -> &ParamDepLayout {
        &self.parametric
    }
}

#[cfg(test)]
impl RowBlockPlan {
    /// Test-only: corrupt the derived dependency witness so core revalidation
    /// must reject it (proves atomic rejection before mutation).
    pub(crate) fn corrupt_layout_for_test(&mut self) {
        for block in &mut self.parametric.blocks {
            block.cell_offset = block.cell_offset.wrapping_add(1000);
        }
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
        let mut rows = Vec::with_capacity(self.rows.len());
        let mut numeric = Vec::new();
        let mut families = Vec::new();
        let mut blocks = Vec::new();
        for (row, array) in &self.rows {
            let len = array.len();
            if len == 0 {
                return RowBatchPlan::General;
            }
            // Fold the constant into the bounds; any unsupported/non-finite
            // constant is an atomic fallback.
            let Some((lower, upper)) = shift_bounds(row.lower, row.upper, array.constant()) else {
                return RowBatchPlan::General;
            };
            rows.push(LocalRow {
                row: row.row,
                lower,
                upper,
            });
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
            for term in array.terms() {
                let (scale, params) = match &term.coeff {
                    CoeffView::ScaledParam { scale, params } => (*scale, params),
                    _ => continue,
                };
                families.push(ParametricFamily {
                    row: Some(row.row),
                    vars: var_slice(term.vars.view()),
                    params: param_slice(params.view()),
                    scale,
                });
            }
            if emit_numeric(row.row, array.terms(), &mut numeric).is_none() {
                return RowBatchPlan::General;
            }
        }
        if canonicalize_numeric_rows(&mut numeric).is_none() {
            return RowBatchPlan::General;
        }
        RowBatchPlan::Planned(RowBlockPlan {
            owner: self.owner,
            rows,
            numeric,
            families,
            parametric: ParamDepLayout { blocks },
        })
    }
}

/// Sort numeric cells by `(row, variable)`, merge duplicates, reject a
/// non-finite merged coefficient, and drop near-zero totals — mirroring the
/// canonical constant-row rule the packed append requires.
fn canonicalize_numeric_rows(cells: &mut Vec<NumericCell>) -> Option<()> {
    cells.sort_by(|a, b| a.row.cmp(&b.row).then_with(|| a.var.cmp(&b.var)));
    let mut merged: Vec<NumericCell> = Vec::with_capacity(cells.len());
    for cell in cells.iter() {
        if let Some(last) = merged.last_mut() {
            if last.row == cell.row && last.var == cell.var {
                last.value += cell.value;
                if !last.value.is_finite() {
                    return None;
                }
                continue;
            }
        }
        merged.push(*cell);
    }
    merged.retain(|cell| cell.value.abs() >= f64::EPSILON);
    *cells = merged;
    Some(())
}

/// Build a cell-wise row plan from a **leading-axis row array** (MIR-04 L1).
///
/// The leading axis of `array` is the row set: leading entry `r` becomes one
/// constraint with `bounds[r]`, and the remaining axes are that row's
/// coefficients (row-major). A one-row array reuses the full single-row
/// planner, including parametric families.
///
/// Conservative for the initial subset: a multi-row array with any parametric
/// term returns [`RowBatchPlan::General`] so the caller uses the general
/// symbolic path rather than a half-specified per-row family plan.
pub fn plan_row_block(
    owner: ModelInstanceId,
    bounds: &[(f64, f64)],
    array: LinArray,
) -> RowBatchPlan {
    let total = array.len();
    if total == 0 || total != bounds.len() {
        return RowBatchPlan::General;
    }
    // A single-cell array reuses the full single-row planner (parametric
    // families included).
    if total == 1 {
        let mut batch = RowBatch::new(owner);
        if batch
            .push(
                LocalRow {
                    row: 0,
                    lower: bounds[0].0,
                    upper: bounds[0].1,
                },
                array,
            )
            .is_err()
        {
            return RowBatchPlan::General;
        }
        return batch.plan();
    }
    // Multi-cell conservative subset: numeric coefficients only. Per-cell
    // parametric rows keep the general symbolic path (per-cell family slicing
    // lands with the general L1 fallback).
    if array.terms().iter().any(|term| term.coeff.is_parametric()) {
        return RowBatchPlan::General;
    }
    // One constraint per cell: each ordinal is its own sink run.
    let runs: Vec<TargetRun> = (0..total)
        .map(|ordinal| TargetRun {
            target: ordinal as u32,
            objective: false,
            start: ordinal,
            len: 1,
        })
        .collect();
    let sink = match SinkMap::new(array.shape().to_vec(), runs) {
        Ok(sink) => sink,
        Err(_) => return RowBatchPlan::General,
    };
    // The proof is not applicable to an all-numeric block; if it nonetheless
    // claims a layout the conservative path declines.
    if try_param_block_layout(&sink, array.terms()).is_some() {
        return RowBatchPlan::General;
    }
    let mut rows = Vec::with_capacity(total);
    for (ordinal, &(lower, upper)) in bounds.iter().enumerate() {
        let Some(shift) = constant_at(array.constant(), ordinal) else {
            return RowBatchPlan::General;
        };
        let shifted_lower = lower - shift;
        let shifted_upper = upper - shift;
        if shifted_lower.is_nan() || shifted_upper.is_nan() {
            return RowBatchPlan::General;
        }
        rows.push(LocalRow {
            row: ordinal as u32,
            lower: shifted_lower,
            upper: shifted_upper,
        });
    }
    let mut numeric = Vec::new();
    if emit_numeric_cells(array.terms(), &mut numeric).is_none() {
        return RowBatchPlan::General;
    }
    if canonicalize_numeric_rows(&mut numeric).is_none() {
        return RowBatchPlan::General;
    }
    RowBatchPlan::Planned(RowBlockPlan {
        owner,
        rows,
        numeric,
        families: Vec::new(),
        parametric: ParamDepLayout { blocks: Vec::new() },
    })
}

/// Build a plan where the **leading axis is the row set** (MIR-04 L1): entry
/// `r` becomes one constraint with `bounds[r]`, and the remaining axes are that
/// row's coefficients (row-major). Used for reduction rows such as
/// `Σ_j x[i, j] == supply[i]`.
///
/// Conservative: a parametric leading row (beyond a single row) keeps the
/// general symbolic path.
pub fn plan_leading_row_block(
    owner: ModelInstanceId,
    bounds: &[(f64, f64)],
    array: LinArray,
) -> RowBatchPlan {
    let shape = array.shape();
    if shape.is_empty() {
        return RowBatchPlan::General;
    }
    let nrows = shape[0];
    if nrows == 0 || nrows != bounds.len() {
        return RowBatchPlan::General;
    }
    if nrows == 1 {
        let mut batch = RowBatch::new(owner);
        if batch
            .push(
                LocalRow {
                    row: 0,
                    lower: bounds[0].0,
                    upper: bounds[0].1,
                },
                array,
            )
            .is_err()
        {
            return RowBatchPlan::General;
        }
        return batch.plan();
    }
    if array.terms().iter().any(|term| term.coeff.is_parametric()) {
        return RowBatchPlan::General;
    }
    let row_len = match shape[1..]
        .iter()
        .try_fold(1usize, |acc, &dim| acc.checked_mul(dim))
    {
        Some(len) if len > 0 => len,
        _ => return RowBatchPlan::General,
    };
    let mut numeric = Vec::new();
    if emit_numeric_leading(row_len, array.terms(), &mut numeric).is_none() {
        return RowBatchPlan::General;
    }
    if canonicalize_numeric_rows(&mut numeric).is_none() {
        return RowBatchPlan::General;
    }
    // A per-cell constant varies within a row and has no scalar bounds form.
    let shift = match array.constant() {
        ConstantView::Zero => 0.0,
        ConstantView::Scalar(value) => *value,
        ConstantView::Dense { .. } | ConstantView::ScaledParam { .. } => {
            return RowBatchPlan::General
        }
    };
    if !shift.is_finite() {
        return RowBatchPlan::General;
    }
    let mut rows = Vec::with_capacity(nrows);
    for (r, &(lower, upper)) in bounds.iter().enumerate() {
        rows.push(LocalRow {
            row: r as u32,
            lower: lower - shift,
            upper: upper - shift,
        });
    }
    RowBatchPlan::Planned(RowBlockPlan {
        owner,
        rows,
        numeric,
        families: Vec::new(),
        parametric: ParamDepLayout { blocks: Vec::new() },
    })
}

/// Emit the numeric packed stream for a leading-axis row array.
fn emit_numeric_leading(row_len: usize, terms: &[Term], out: &mut Vec<NumericCell>) -> Option<()> {
    if row_len == 0 {
        return None;
    }
    for term in terms {
        if term.coeff.is_parametric() {
            continue;
        }
        let view = term.vars.view();
        for ordinal in 0..view.len() {
            let offset = usize::try_from(view.get(ordinal)?).ok()?;
            let var = view.span().id_at(offset)?;
            let value = match &term.coeff {
                CoeffView::One => 1.0,
                CoeffView::Scalar(value) => *value,
                CoeffView::Dense { scale, values } => scale * values.get(ordinal)?,
                CoeffView::ScaledParam { .. } => continue,
            };
            let row = u32::try_from(ordinal / row_len).ok()?;
            out.push(NumericCell { row, var, value });
        }
    }
    Some(())
}

/// Fold the array constant at one cell into a scalar shift, or `None` when the
/// constant cannot be represented (which must fall back rather than silently
/// disappear).
fn constant_at(constant: &ConstantView, ordinal: usize) -> Option<f64> {
    let shift = match constant {
        ConstantView::Zero => 0.0,
        ConstantView::Scalar(value) => *value,
        ConstantView::Dense { scale, values } => scale * values.get(ordinal)?,
        ConstantView::ScaledParam { .. } => return None,
    };
    shift.is_finite().then_some(shift)
}

fn var_slice(view: &View<VarSpan>) -> VarSlice {
    VarSlice {
        span: *view.span(),
        map: StridedMap::new(
            view.shape().to_vec(),
            view.strides().to_vec(),
            view.offset(),
        ),
    }
}

fn param_slice(view: &View<ParamSpan>) -> ParamSlice {
    ParamSlice {
        span: *view.span(),
        map: StridedMap::new(
            view.shape().to_vec(),
            view.strides().to_vec(),
            view.offset(),
        ),
    }
}

/// Fold a constant into scalar row bounds, or `None` when the constant cannot
/// be represented (which must fall back rather than silently disappear).
fn shift_bounds(lower: f64, upper: f64, constant: &ConstantView) -> Option<(f64, f64)> {
    let shift = match constant {
        ConstantView::Zero => 0.0,
        ConstantView::Scalar(value) => *value,
        // A single-cell dense constant is scalar-equivalent; a per-cell dense
        // constant over a multi-cell row needs a per-cell bounds representation
        // that this initial subset does not have, so it falls back.
        ConstantView::Dense { scale, values } => {
            if values.len() != 1 {
                return None;
            }
            scale * values.get(0)?
        }
        ConstantView::ScaledParam { .. } => return None,
    };
    if !shift.is_finite() {
        return None;
    }
    let shifted_lower = lower - shift;
    let shifted_upper = upper - shift;
    if shifted_lower.is_nan() || shifted_upper.is_nan() {
        return None;
    }
    Some((shifted_lower, shifted_upper))
}

/// Emit the numeric packed stream for a row's non-parametric terms.
fn emit_numeric(row: u32, terms: &[Term], out: &mut Vec<NumericCell>) -> Option<()> {
    for term in terms {
        if term.coeff.is_parametric() {
            continue;
        }
        let view = term.vars.view();
        for ordinal in 0..view.len() {
            let offset = usize::try_from(view.get(ordinal)?).ok()?;
            let var = view.span().id_at(offset)?;
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

/// Emit the numeric packed stream for a cell-wise row array: each cell is its
/// own row, so the row ordinal equals the cell ordinal.
fn emit_numeric_cells(terms: &[Term], out: &mut Vec<NumericCell>) -> Option<()> {
    for term in terms {
        if term.coeff.is_parametric() {
            continue;
        }
        let view = term.vars.view();
        for ordinal in 0..view.len() {
            let offset = usize::try_from(view.get(ordinal)?).ok()?;
            let var = view.span().id_at(offset)?;
            let value = match &term.coeff {
                CoeffView::One => 1.0,
                CoeffView::Scalar(value) => *value,
                CoeffView::Dense { scale, values } => scale * values.get(ordinal)?,
                CoeffView::ScaledParam { .. } => continue,
            };
            let row = u32::try_from(ordinal).ok()?;
            out.push(NumericCell { row, var, value });
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Generation;
    use crate::modeling::{ParamView, VarView};

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

    fn row(index: u32, lower: f64, upper: f64) -> LocalRow {
        LocalRow {
            row: index,
            lower,
            upper,
        }
    }

    fn array(
        owner: ModelInstanceId,
        len: usize,
        terms: Vec<Term>,
        constant: ConstantView,
    ) -> LinArray {
        LinArray::new(owner, [len], terms, constant).expect("array")
    }

    fn plan_of(owner: ModelInstanceId, row: LocalRow, array: LinArray) -> RowBatchPlan {
        let mut batch = RowBatch::new(owner);
        batch.push(row, array).expect("row");
        batch.plan()
    }

    // A1: x + 5 <= 10 lowers the same as x <= 5.
    #[test]
    fn scalar_constant_shifts_bounds_equivalently() {
        let owner = new_owner();
        let shifted = plan_of(
            owner,
            row(0, f64::NEG_INFINITY, 10.0),
            array(
                owner,
                1,
                vec![numeric_term(owner, 0, 1, 1.0)],
                ConstantView::Scalar(5.0),
            ),
        );
        let base = plan_of(
            owner,
            row(0, f64::NEG_INFINITY, 5.0),
            array(
                owner,
                1,
                vec![numeric_term(owner, 0, 1, 1.0)],
                ConstantView::Zero,
            ),
        );
        match (shifted, base) {
            (RowBatchPlan::Planned(a), RowBatchPlan::Planned(b)) => {
                assert_eq!(a.rows(), b.rows());
                assert_eq!(a.rows()[0].upper, 5.0);
                assert_eq!(a.numeric(), b.numeric());
            }
            other => panic!("expected two planned rows, got {other:?}"),
        }
    }

    // A2: a single-cell dense constant shifts bounds where representable.
    #[test]
    fn dense_single_cell_constant_shifts_bounds() {
        let owner = new_owner();
        let plan = plan_of(
            owner,
            row(0, 0.0, 10.0),
            array(
                owner,
                1,
                vec![numeric_term(owner, 0, 1, 1.0)],
                ConstantView::Dense {
                    scale: 2.0,
                    values: crate::modeling::NumView::from_vec(vec![3.0]),
                },
            ),
        );
        match plan {
            RowBatchPlan::Planned(plan) => assert_eq!(plan.rows()[0].upper, 4.0),
            other => panic!("expected planned row, got {other:?}"),
        }
    }

    // A2b: a multi-cell dense constant is not representable with scalar bounds.
    #[test]
    fn dense_multi_cell_constant_falls_back() {
        let owner = new_owner();
        let plan = plan_of(
            owner,
            row(0, 0.0, 10.0),
            array(
                owner,
                2,
                vec![numeric_term(owner, 0, 2, 1.0)],
                ConstantView::Dense {
                    scale: 1.0,
                    values: crate::modeling::NumView::from_vec(vec![1.0, 2.0]),
                },
            ),
        );
        assert_eq!(plan, RowBatchPlan::General);
    }

    // A3: a parameterized constant is never silently dropped.
    #[test]
    fn scaled_param_constant_falls_back() {
        let owner = new_owner();
        let plan = plan_of(
            owner,
            row(0, 0.0, 10.0),
            array(
                owner,
                1,
                vec![numeric_term(owner, 0, 1, 1.0)],
                ConstantView::ScaledParam {
                    scale: 1.0,
                    params: param(owner, 1),
                },
            ),
        );
        assert_eq!(plan, RowBatchPlan::General);
    }

    // A4: non-finite constants fall back atomically.
    #[test]
    fn non_finite_constant_falls_back() {
        let owner = new_owner();
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let plan = plan_of(
                owner,
                row(0, 0.0, 10.0),
                array(
                    owner,
                    1,
                    vec![numeric_term(owner, 0, 1, 1.0)],
                    ConstantView::Scalar(value),
                ),
            );
            assert_eq!(plan, RowBatchPlan::General, "constant {value}");
        }
    }

    // B: the plan is self-contained after the source LinArray is dropped.
    #[test]
    fn plan_is_self_contained_after_source_is_dropped() {
        let owner = new_owner();
        let n = 2usize;
        let plan = {
            let mut batch = RowBatch::new(owner);
            batch
                .push(
                    row(0, 0.0, 10.0),
                    array(
                        owner,
                        n,
                        vec![
                            numeric_term(owner, 0, n, 2.0),
                            param_term(owner, n as u32, n, 1.5),
                        ],
                        ConstantView::Zero,
                    ),
                )
                .expect("row");
            match batch.plan() {
                RowBatchPlan::Planned(plan) => plan,
                other => panic!("expected planned batch, got {other:?}"),
            }
        };

        // Reconstruct the expected payload from the plan alone.
        assert_eq!(plan.numeric().len(), n);
        assert!(plan.numeric().iter().all(|cell| cell.value == 2.0));
        assert_eq!(plan.families().len(), 1);
        let family = &plan.families()[0];
        assert_eq!(family.row, Some(0));
        assert_eq!(family.scale, 1.5);
        let vars: Vec<VarId> = (0..family.vars.len())
            .filter_map(|i| family.vars.member(i))
            .collect();
        let params: Vec<ParamId> = (0..family.params.len())
            .filter_map(|i| family.params.member(i))
            .collect();
        assert_eq!(vars.len(), n);
        assert_eq!(params.len(), n);
        assert_eq!(plan.parametric().blocks.len(), 1);
    }

    // C: variable identity (generation) is preserved, never a bare index.
    #[test]
    fn plan_preserves_variable_generation_identity() {
        let owner = new_owner();
        let generation = Generation::new();
        let view = VarView::new(
            owner,
            View::contiguous(VarSpan::from_parts(7, 1, generation), 1),
        )
        .expect("var view");
        let term = Term {
            vars: view,
            coeff: CoeffView::Scalar(1.0),
        };
        let plan = {
            let mut batch = RowBatch::new(owner);
            batch
                .push(
                    row(0, 0.0, 1.0),
                    LinArray::new(owner, [1usize], vec![term], ConstantView::Zero).expect("array"),
                )
                .expect("row");
            match batch.plan() {
                RowBatchPlan::Planned(plan) => plan,
                other => panic!("expected planned batch, got {other:?}"),
            }
        };
        let cell = plan.numeric()[0];
        assert_eq!(cell.var.index(), 7);
        assert_eq!(cell.var.generation(), generation);

        // A generation-mismatched variable is a *different* identity, so a plan
        // cannot silently retarget a deleted/reallocated slot.
        let stale = VarId::new(7, generation.next());
        assert_ne!(cell.var, stale);
    }

    #[test]
    fn mixed_and_numeric_and_collision_cases() {
        let owner = new_owner();
        let n = 2usize;
        // Mixed row -> mixed plan.
        match plan_of(
            owner,
            row(0, 0.0, 10.0),
            array(
                owner,
                n,
                vec![
                    numeric_term(owner, 0, n, 1.0),
                    param_term(owner, n as u32, n, 1.0),
                ],
                ConstantView::Zero,
            ),
        ) {
            RowBatchPlan::Planned(plan) => {
                assert_eq!(plan.numeric().len(), n);
                assert_eq!(plan.families().len(), 1);
            }
            other => panic!("expected mixed plan, got {other:?}"),
        }
        // Constant + parametric collision -> General.
        assert_eq!(
            plan_of(
                owner,
                row(0, 0.0, 10.0),
                array(
                    owner,
                    n,
                    vec![numeric_term(owner, 0, n, 1.0), param_term(owner, 0, n, 1.0)],
                    ConstantView::Zero,
                ),
            ),
            RowBatchPlan::General
        );
        // Two-parametric collision -> General.
        assert_eq!(
            plan_of(
                owner,
                row(0, 0.0, 10.0),
                array(
                    owner,
                    n,
                    vec![param_term(owner, 0, n, 1.0), param_term(owner, 0, n, 2.0)],
                    ConstantView::Zero,
                ),
            ),
            RowBatchPlan::General
        );
    }

    #[test]
    fn duplicate_row_target_and_foreign_array_are_rejected() {
        let owner = new_owner();
        let mut batch = RowBatch::new(owner);
        batch
            .push(
                row(0, 0.0, 1.0),
                array(
                    owner,
                    1,
                    vec![numeric_term(owner, 0, 1, 1.0)],
                    ConstantView::Zero,
                ),
            )
            .expect("row");
        assert!(batch
            .push(
                row(0, 0.0, 1.0),
                array(
                    owner,
                    1,
                    vec![numeric_term(owner, 1, 1, 1.0)],
                    ConstantView::Zero
                ),
            )
            .is_err());
        let other = new_owner();
        assert!(batch
            .push(
                row(1, 0.0, 1.0),
                array(
                    other,
                    1,
                    vec![numeric_term(other, 0, 1, 1.0)],
                    ConstantView::Zero
                ),
            )
            .is_err());
    }
}
