//! L1 array handles (MIR-04, IR-24).
//!
//! `VarArray`/`ParamArray` are the ordinary user-facing handles for structured
//! variable and parameter blocks. They wrap a model-owned [`VarView`]/
//! [`ParamView`] (owner + trusted span + strided shape) plus a boundary name,
//! and support metadata-only slicing/transposition (no per-cell allocation).
//!
//! Names are boundary metadata: they never enter expression nodes or the
//! canonical ordinal IR.

use std::sync::Arc;

use crate::id::{ParamId, VarId};
use crate::modeling::{ParamView, VarView, ViewError};
use crate::ModelInstanceId;

/// A validated, immutable array shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape(Arc<[usize]>);

impl Shape {
    /// Shape dimensions (row-major).
    pub fn dims(&self) -> &[usize] {
        &self.0
    }

    /// Number of dimensions.
    pub fn rank(&self) -> usize {
        self.0.len()
    }

    /// Number of cells (product of dimensions), or `None` on overflow.
    pub fn product(&self) -> Option<usize> {
        self.0
            .iter()
            .try_fold(1usize, |acc, &dim| acc.checked_mul(dim))
    }
}

impl From<usize> for Shape {
    fn from(len: usize) -> Self {
        Self(Arc::from([len]))
    }
}

impl<const N: usize> From<[usize; N]> for Shape {
    fn from(dims: [usize; N]) -> Self {
        Self(Arc::from(dims))
    }
}

impl From<Vec<usize>> for Shape {
    fn from(dims: Vec<usize>) -> Self {
        Self(Arc::from(dims))
    }
}

impl From<&[usize]> for Shape {
    fn from(dims: &[usize]) -> Self {
        Self(Arc::from(dims))
    }
}

/// Contiguous row-major strides for a shape (last dimension fastest).
pub(crate) fn row_major_strides(shape: &[usize]) -> Option<Vec<isize>> {
    let mut strides = vec![1isize; shape.len()];
    let mut acc: isize = 1;
    for dim in (0..shape.len()).rev() {
        strides[dim] = acc;
        acc = acc.checked_mul(isize::try_from(shape[dim]).ok()?)?;
    }
    Some(strides)
}

/// A model-owned multidimensional variable array handle.
#[derive(Clone, Debug)]
pub struct VarArray {
    name: Arc<str>,
    view: VarView,
}

impl VarArray {
    pub(crate) fn new(name: impl Into<Arc<str>>, view: VarView) -> Self {
        Self {
            name: name.into(),
            view,
        }
    }

    /// Boundary name (metadata only; never part of expression nodes).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Owning model.
    pub fn owner(&self) -> ModelInstanceId {
        self.view.owner()
    }

    /// Array shape.
    pub fn shape(&self) -> &[usize] {
        self.view.view().shape()
    }

    /// Number of cells.
    pub fn len(&self) -> usize {
        self.view.view().len()
    }

    /// Whether the array covers no cells.
    pub fn is_empty(&self) -> bool {
        self.view.view().is_empty()
    }

    /// The variable at a row-major ordinal.
    pub fn get(&self, ordinal: usize) -> Option<VarId> {
        self.view.member(ordinal)
    }

    /// The underlying trusted symbolic view.
    pub fn view(&self) -> &VarView {
        &self.view
    }

    /// A linear expression over this array with unit coefficients.
    pub fn expr(&self) -> Result<crate::modeling::LinArray, ViewError> {
        use crate::modeling::{CoeffView, ConstantView, LinArray, Term};
        LinArray::new(
            self.owner(),
            self.shape().to_vec(),
            vec![Term {
                vars: self.view.clone(),
                coeff: CoeffView::One,
            }],
            ConstantView::Zero,
        )
    }

    /// The linear form used by operator algebra (infallible for a validated
    /// array).
    pub(crate) fn lin(&self) -> crate::modeling::LinArray {
        self.expr()
            .expect("a validated variable array is a valid LinArray")
    }

    /// Metadata-only slice along `axis` (`[start, start + len)`).
    pub fn slice(&self, axis: usize, start: usize, len: usize) -> Result<Self, ViewError> {
        Ok(Self {
            name: self.name.clone(),
            view: self.view.slice(axis, start, len)?,
        })
    }

    /// Metadata-only reversal along `axis`.
    pub fn reverse(&self, axis: usize) -> Result<Self, ViewError> {
        Ok(Self {
            name: self.name.clone(),
            view: self.view.reverse(axis)?,
        })
    }

    /// Metadata-only transpose (swap two axes).
    pub fn transpose(&self, a: usize, b: usize) -> Result<Self, ViewError> {
        Ok(Self {
            name: self.name.clone(),
            view: self.view.transpose(a, b)?,
        })
    }

    /// Metadata-only reshape (contiguous dense views only).
    pub fn reshape(&self, shape: impl Into<Shape>) -> Result<Self, ViewError> {
        let shape = shape.into();
        Ok(Self {
            name: self.name.clone(),
            view: self.view.reshape(shape.dims().to_vec())?,
        })
    }

    /// A coefficient array for one leading-axis entry (contiguous dense only):
    /// the leading-axis slice reshaped to the trailing dimensions. A rank-1
    /// array yields a single-cell row.
    pub fn row(&self, index: usize) -> Result<Self, ViewError> {
        let leading = self.shape().first().copied().unwrap_or(0);
        if index >= leading {
            return Err(ViewError::SliceOutOfRange {
                axis: 0,
                start: index,
                len: 1,
                dim: leading,
            });
        }
        let rest = self.shape()[1..].to_vec();
        let target = if rest.is_empty() { vec![1] } else { rest };
        self.slice(0, index, 1)?.reshape(target)
    }
}

/// A model-owned multidimensional parameter array handle.
#[derive(Clone, Debug)]
pub struct ParamArray {
    name: Arc<str>,
    view: ParamView,
}

impl ParamArray {
    pub(crate) fn new(name: impl Into<Arc<str>>, view: ParamView) -> Self {
        Self {
            name: name.into(),
            view,
        }
    }

    /// Boundary name (metadata only).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Owning model.
    pub fn owner(&self) -> ModelInstanceId {
        self.view.owner()
    }

    /// Array shape.
    pub fn shape(&self) -> &[usize] {
        self.view.view().shape()
    }

    /// Number of cells.
    pub fn len(&self) -> usize {
        self.view.view().len()
    }

    /// Whether the array covers no cells.
    pub fn is_empty(&self) -> bool {
        self.view.view().is_empty()
    }

    /// The parameter at a row-major ordinal.
    pub fn get(&self, ordinal: usize) -> Option<ParamId> {
        self.view.member(ordinal)
    }

    /// The underlying trusted symbolic view.
    pub fn view(&self) -> &ParamView {
        &self.view
    }

    /// Multiply this parameter array by a linear expression.
    ///
    /// Returns `Ok(Some(..))` on the conservative fast IR, `Ok(None)` when the
    /// form is not covered (the caller uses the general symbolic path), and a
    /// typed error for cross-model composition or an invalid shape.
    pub fn try_mul(
        &self,
        array: &crate::modeling::LinArray,
    ) -> Result<Option<crate::modeling::LinArray>, ViewError> {
        self.view.mul_linarray(array)
    }

    /// Metadata-only slice along `axis`.
    pub fn slice(&self, axis: usize, start: usize, len: usize) -> Result<Self, ViewError> {
        Ok(Self {
            name: self.name.clone(),
            view: self.view.slice(axis, start, len)?,
        })
    }

    /// Metadata-only reversal along `axis`.
    pub fn reverse(&self, axis: usize) -> Result<Self, ViewError> {
        Ok(Self {
            name: self.name.clone(),
            view: self.view.reverse(axis)?,
        })
    }

    /// Metadata-only transpose (swap two axes).
    pub fn transpose(&self, a: usize, b: usize) -> Result<Self, ViewError> {
        Ok(Self {
            name: self.name.clone(),
            view: self.view.transpose(a, b)?,
        })
    }

    /// Metadata-only reshape (contiguous dense views only).
    pub fn reshape(&self, shape: impl Into<Shape>) -> Result<Self, ViewError> {
        let shape = shape.into();
        Ok(Self {
            name: self.name.clone(),
            view: self.view.reshape(shape.dims().to_vec())?,
        })
    }

    /// A coefficient array for one leading-axis entry (contiguous dense only):
    /// the leading-axis slice reshaped to the trailing dimensions. A rank-1
    /// array yields a single-cell row.
    pub fn row(&self, index: usize) -> Result<Self, ViewError> {
        let leading = self.shape().first().copied().unwrap_or(0);
        if index >= leading {
            return Err(ViewError::SliceOutOfRange {
                axis: 0,
                start: index,
                len: 1,
                dim: leading,
            });
        }
        let rest = self.shape()[1..].to_vec();
        let target = if rest.is_empty() { vec![1] } else { rest };
        self.slice(0, index, 1)?.reshape(target)
    }
}

impl From<VarArray> for crate::modeling::LinArray {
    fn from(array: VarArray) -> Self {
        array.lin()
    }
}

impl From<&VarArray> for crate::modeling::LinArray {
    fn from(array: &VarArray) -> Self {
        array.lin()
    }
}
