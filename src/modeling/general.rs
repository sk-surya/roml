//! General per-cell symbolic affine arrays (MIR-06, IR-27).
//!
//! [`LinArray`] is the compact fast representation (four coefficient families).
//! [`GeneralLinArray`] is the correct-but-uncompacted fallback: one general
//! affine per cell with arbitrary [`ValueExpr`] coefficients (including forms
//! the compact families do not cover, such as parameter x parameter). It lives
//! in `roml::modeling` so frontends do not own a competing modeling IR.
//!
//! This is the boundary:
//!
//! ```text
//!                 shared roml::modeling
//!            ┌───────────┴───────────┐
//!        LinArray              GeneralLinArray
//!     compact / fast           correct fallback
//!            └───────────┬───────────┘
//!                  frontend wrappers
//! ```
//!
//! Covered expressions stay `LinArray`; only genuinely unsupported forms become
//! `GeneralLinArray` (conservative fallback is a performance finding, not a
//! correctness failure).

use std::sync::Arc;

use crate::id::VarId;
use crate::modeling::{CoeffView, ConstantView, LinArray, ViewError};
use crate::value_expr::ValueExpr;
use crate::ModelInstanceId;

/// One general term: `coeff * var`.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneralTerm {
    /// The variable the coefficient multiplies.
    pub var: VarId,
    /// The coefficient expression.
    pub coeff: ValueExpr,
}

/// One general affine cell: `Σ coeff_j * var_j + constant`.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneralAffine {
    /// Variable terms.
    pub terms: Vec<GeneralTerm>,
    /// Constant expression.
    pub constant: ValueExpr,
}

impl GeneralAffine {
    /// A general affine from terms and a constant.
    pub fn new(terms: Vec<GeneralTerm>, constant: ValueExpr) -> Self {
        Self { terms, constant }
    }

    /// The zero affine.
    pub fn zero() -> Self {
        Self {
            terms: Vec::new(),
            constant: ValueExpr::constant(0.0),
        }
    }

    /// Scale every coefficient and the constant.
    pub fn scaled(mut self, alpha: f64) -> Self {
        for term in &mut self.terms {
            term.coeff = term.coeff.clone() * alpha;
        }
        self.constant = self.constant * alpha;
        self
    }

    /// Add another affine into this one, combining like variables.
    pub fn add_assign(&mut self, other: GeneralAffine) {
        self.constant = self.constant.clone() + other.constant;
        for term in other.terms {
            if let Some(existing) = self.terms.iter_mut().find(|t| t.var == term.var) {
                existing.coeff = existing.coeff.clone() + term.coeff;
            } else {
                self.terms.push(term);
            }
        }
    }
}

/// A general per-cell symbolic affine array over one model instance.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneralLinArray {
    owner: ModelInstanceId,
    shape: Arc<[usize]>,
    cells: Vec<GeneralAffine>,
}

impl GeneralLinArray {
    /// Build a general array, validating the shape product against the cells.
    pub fn new(
        owner: ModelInstanceId,
        shape: impl Into<Arc<[usize]>>,
        cells: Vec<GeneralAffine>,
    ) -> Result<Self, ViewError> {
        let shape: Arc<[usize]> = shape.into();
        let product = shape
            .iter()
            .try_fold(1usize, |acc, &dim| acc.checked_mul(dim))
            .ok_or(ViewError::ShapeOverflow)?;
        if product != cells.len() {
            return Err(ViewError::ShapeMismatch {
                left: shape,
                right: Arc::from([cells.len()]),
            });
        }
        Ok(Self {
            owner,
            shape,
            cells,
        })
    }

    /// The owning model instance.
    pub fn owner(&self) -> ModelInstanceId {
        self.owner
    }

    /// Array shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Number of cells.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether the array covers no cells.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// The per-cell affines.
    pub fn cells(&self) -> &[GeneralAffine] {
        &self.cells
    }

    /// One cell, if present.
    pub fn cell(&self, ordinal: usize) -> Option<&GeneralAffine> {
        self.cells.get(ordinal)
    }

    /// Scale every cell.
    pub fn scaled(mut self, alpha: f64) -> Self {
        for cell in &mut self.cells {
            *cell = cell.clone().scaled(alpha);
        }
        self
    }

    /// Add two general arrays of the same owner and shape (cell-wise).
    pub fn try_add(self, other: Self) -> Result<Self, ViewError> {
        if self.owner != other.owner {
            return Err(ViewError::CrossModel {
                left: self.owner,
                right: other.owner,
            });
        }
        if self.shape != other.shape {
            return Err(ViewError::ShapeMismatch {
                left: self.shape,
                right: other.shape,
            });
        }
        let mut cells = self.cells;
        for (cell, other_cell) in cells.iter_mut().zip(other.cells) {
            cell.add_assign(other_cell);
        }
        Ok(Self {
            owner: self.owner,
            shape: self.shape,
            cells,
        })
    }

    /// Subtract two general arrays of the same owner and shape.
    pub fn try_sub(self, other: Self) -> Result<Self, ViewError> {
        self.try_add(other.scaled(-1.0))
    }
}

impl LinArray {
    /// Expand the compact representation into one general affine per cell.
    ///
    /// Every compact coefficient family (`One`/`Scalar`/`Dense`/`ScaledParam`)
    /// is lowered to a `ValueExpr` coefficient; this is the one-way boundary
    /// from the fast IR to the general fallback.
    pub fn to_general(&self) -> Result<GeneralLinArray, ViewError> {
        let mut cells = Vec::with_capacity(self.len());
        for ordinal in 0..self.len() {
            let mut terms = Vec::with_capacity(self.terms().len());
            for term in self.terms() {
                let var = term.vars.member(ordinal).ok_or(ViewError::SpanOutOfRange)?;
                let coeff = match &term.coeff {
                    CoeffView::One => ValueExpr::constant(1.0),
                    CoeffView::Scalar(value) => ValueExpr::constant(*value),
                    CoeffView::Dense { scale, values } => {
                        let value = values.get(ordinal).ok_or(ViewError::SpanOutOfRange)?;
                        ValueExpr::constant(scale * value)
                    }
                    CoeffView::ScaledParam { scale, params } => {
                        let param = params.member(ordinal).ok_or(ViewError::SpanOutOfRange)?;
                        ValueExpr::scaled_param(*scale, param)
                    }
                };
                terms.push(GeneralTerm { var, coeff });
            }
            let constant = match self.constant() {
                ConstantView::Zero => ValueExpr::constant(0.0),
                ConstantView::Scalar(value) => ValueExpr::constant(*value),
                ConstantView::Dense { scale, values } => {
                    let value = values.get(ordinal).ok_or(ViewError::SpanOutOfRange)?;
                    ValueExpr::constant(scale * value)
                }
                ConstantView::ScaledParam { scale, params } => {
                    let param = params.member(ordinal).ok_or(ViewError::SpanOutOfRange)?;
                    ValueExpr::scaled_param(*scale, param)
                }
            };
            cells.push(GeneralAffine { terms, constant });
        }
        GeneralLinArray::new(self.owner(), self.shape().to_vec(), cells)
    }
}
