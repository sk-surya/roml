//! L1 coefficient and linear-array IR (MIR-03, IR-19/IR-20).
//!
//! `LinArray = Σ Term{VarView, CoeffView} + ConstantView` over one model
//! instance. The initial coefficient families are `One`, `Scalar`, scaled
//! `Dense`, and scaled `ScaledParam`. Scalar scaling folds into the stored
//! scale and never copies a numeric buffer.
//!
//! `ParamView * LinArray` stays on the fast IR only under the conservative
//! rule in `DESIGN.md` §5: every variable term coefficient is `One`/`Scalar`
//! and the constant is `Zero`/`Scalar`. Anything else returns `Ok(None)` so the
//! caller uses the general symbolic path.

use std::sync::Arc;

use crate::modeling::{ParamView, VarView, View, ViewError};
use crate::ModelInstanceId;

fn product(shape: &[usize]) -> Option<usize> {
    shape
        .iter()
        .try_fold(1usize, |acc, &dim| acc.checked_mul(dim))
}

/// A numeric buffer with a validated strided view over it.
#[derive(Clone, Debug, PartialEq)]
pub struct NumView {
    values: Arc<[f64]>,
    view: View<()>,
}

/// The inclusive range of buffer offsets a strided view can map, or `None` on
/// overflow. Computed in O(rank) from metadata only.
fn mapped_range(shape: &[usize], strides: &[isize], offset: isize) -> Option<(isize, isize)> {
    let mut low = offset;
    let mut high = offset;
    for (dim, stride) in shape.iter().zip(strides.iter()) {
        if *dim == 0 {
            continue;
        }
        let span = (*dim as isize - 1).checked_mul(*stride)?;
        if span >= 0 {
            high = high.checked_add(span)?;
        } else {
            low = low.checked_add(span)?;
        }
    }
    Some((low, high))
}

impl NumView {
    /// A validated strided numeric view over a shared buffer.
    ///
    /// Every mapped offset must index the buffer; out-of-range metadata is a
    /// typed [`ViewError::Unsupported`] rather than a later panic.
    pub fn new(
        values: Arc<[f64]>,
        shape: impl Into<Arc<[usize]>>,
        strides: impl Into<Arc<[isize]>>,
        offset: isize,
    ) -> Result<Self, ViewError> {
        let view = View::new((), shape, strides, offset)?;
        let (low, high) = mapped_range(view.shape(), view.strides(), view.offset())
            .ok_or(ViewError::IndexOverflow)?;
        if !view.is_empty() && (low < 0 || high >= values.len() as isize) {
            return Err(ViewError::Unsupported(
                "numeric view offset out of buffer range",
            ));
        }
        Ok(Self { values, view })
    }

    /// A contiguous numeric view over a freshly owned buffer.
    pub fn from_vec(values: Vec<f64>) -> Self {
        let values: Arc<[f64]> = Arc::from(values);
        let len = values.len();
        Self {
            values,
            view: View::contiguous((), len),
        }
    }

    /// A contiguous numeric view over a shared buffer.
    pub fn from_shared(values: Arc<[f64]>) -> Self {
        let len = values.len();
        Self {
            values,
            view: View::contiguous((), len),
        }
    }

    /// Shape of the numeric view.
    pub fn shape(&self) -> &[usize] {
        self.view.shape()
    }

    /// Signed strides.
    pub fn strides(&self) -> &[isize] {
        self.view.strides()
    }

    /// Signed offset into the buffer.
    pub fn offset(&self) -> isize {
        self.view.offset()
    }

    /// Number of mapped values.
    pub fn len(&self) -> usize {
        self.view.len()
    }

    /// Whether the view covers no values.
    pub fn is_empty(&self) -> bool {
        self.view.is_empty()
    }

    /// Resolve a row-major ordinal to its numeric value.
    pub fn get(&self, ordinal: usize) -> Option<f64> {
        let offset = usize::try_from(self.view.get(ordinal)?).ok()?;
        self.values.get(offset).copied()
    }

    /// Whether two views share the same backing buffer (pointer identity).
    pub fn shares_buffer(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.values, &other.values)
    }

    /// Metadata-only slice.
    pub fn slice(&self, axis: usize, start: usize, len: usize) -> Result<Self, ViewError> {
        Ok(Self {
            values: self.values.clone(),
            view: self.view.slice(axis, start, len)?,
        })
    }

    /// Metadata-only reversal.
    pub fn reverse(&self, axis: usize) -> Result<Self, ViewError> {
        Ok(Self {
            values: self.values.clone(),
            view: self.view.reverse(axis)?,
        })
    }

    /// Metadata-only transpose.
    pub fn transpose(&self, a: usize, b: usize) -> Result<Self, ViewError> {
        Ok(Self {
            values: self.values.clone(),
            view: self.view.transpose(a, b)?,
        })
    }
}

/// A variable linear-expression coefficient family.
#[derive(Clone, Debug, PartialEq)]
pub enum CoeffView {
    /// The coefficient `1`.
    One,
    /// A scalar coefficient.
    Scalar(f64),
    /// A scaled strided numeric buffer (dense coefficients).
    Dense {
        /// Scalar multiplier.
        scale: f64,
        /// Numeric buffer and view.
        values: NumView,
    },
    /// `scale × ParamView`.
    ScaledParam {
        /// Scalar multiplier.
        scale: f64,
        /// Parameter view.
        params: ParamView,
    },
}

impl CoeffView {
    /// Scale the coefficient, folding into the stored scalar (zero-copy for
    /// `Dense`/`ScaledParam`).
    pub fn scaled(self, alpha: f64) -> Self {
        match self {
            Self::One => Self::Scalar(alpha),
            Self::Scalar(value) => Self::Scalar(alpha * value),
            Self::Dense { scale, values } => Self::Dense {
                scale: scale * alpha,
                values,
            },
            Self::ScaledParam { scale, params } => Self::ScaledParam {
                scale: scale * alpha,
                params,
            },
        }
    }

    /// Whether the coefficient carries a parameter dependency.
    pub fn is_parametric(&self) -> bool {
        matches!(self, Self::ScaledParam { .. })
    }

    /// Whether the coefficient is `One`/`Scalar` (the conservative fast subset).
    pub fn is_broadcastable(&self) -> bool {
        matches!(self, Self::One | Self::Scalar(_))
    }
}

/// A constant coefficient family.
#[derive(Clone, Debug, PartialEq)]
pub enum ConstantView {
    /// The constant `0`.
    Zero,
    /// A scalar constant.
    Scalar(f64),
    /// A scaled strided numeric constant.
    Dense {
        /// Scalar multiplier.
        scale: f64,
        /// Numeric buffer and view.
        values: NumView,
    },
    /// `scale × ParamView`.
    ScaledParam {
        /// Scalar multiplier.
        scale: f64,
        /// Parameter view.
        params: ParamView,
    },
}

impl ConstantView {
    /// Scale the constant, folding into the stored scalar.
    pub fn scaled(self, alpha: f64) -> Self {
        match self {
            Self::Zero => Self::Zero,
            Self::Scalar(value) => Self::Scalar(alpha * value),
            Self::Dense { scale, values } => Self::Dense {
                scale: scale * alpha,
                values,
            },
            Self::ScaledParam { scale, params } => Self::ScaledParam {
                scale: scale * alpha,
                params,
            },
        }
    }

    /// Whether the constant is exactly zero.
    pub fn is_zero(&self) -> bool {
        matches!(self, Self::Zero)
    }

    /// Whether the constant is `Zero`/`Scalar` (the conservative fast subset).
    pub fn is_broadcastable(&self) -> bool {
        matches!(self, Self::Zero | Self::Scalar(_))
    }

    /// Add two constants on the fast IR, or signal general fallback.
    pub fn try_add(self, other: Self) -> Result<Self, ViewError> {
        match (self, other) {
            (Self::Zero, other) | (other, Self::Zero) => Ok(other),
            (Self::Scalar(a), Self::Scalar(b)) => Ok(Self::Scalar(a + b)),
            _ => Err(ViewError::Unsupported("constant addition")),
        }
    }
}

fn validate_shape(actual: &[usize], expected: &[usize]) -> Result<(), ViewError> {
    if actual != expected {
        return Err(ViewError::ShapeMismatch {
            left: Arc::from(expected),
            right: Arc::from(actual),
        });
    }
    Ok(())
}

fn validate_coeff(
    coeff: &CoeffView,
    owner: ModelInstanceId,
    shape: &[usize],
) -> Result<(), ViewError> {
    match coeff {
        CoeffView::One | CoeffView::Scalar(_) => Ok(()),
        CoeffView::Dense { values, .. } => validate_shape(values.shape(), shape),
        CoeffView::ScaledParam { params, .. } => {
            if params.owner() != owner {
                return Err(ViewError::CrossModel {
                    left: owner,
                    right: params.owner(),
                });
            }
            validate_shape(params.view().shape(), shape)
        }
    }
}

fn validate_constant(
    constant: &ConstantView,
    owner: ModelInstanceId,
    shape: &[usize],
) -> Result<(), ViewError> {
    match constant {
        ConstantView::Zero | ConstantView::Scalar(_) => Ok(()),
        ConstantView::Dense { values, .. } => validate_shape(values.shape(), shape),
        ConstantView::ScaledParam { params, .. } => {
            if params.owner() != owner {
                return Err(ViewError::CrossModel {
                    left: owner,
                    right: params.owner(),
                });
            }
            validate_shape(params.view().shape(), shape)
        }
    }
}

/// One `VarView` with its coefficient family.
#[derive(Clone, Debug, PartialEq)]
pub struct Term {
    /// Variables the term applies to.
    pub vars: VarView,
    /// Coefficient family.
    pub coeff: CoeffView,
}

/// A model-owned linear array.
#[derive(Clone, Debug, PartialEq)]
pub struct LinArray {
    owner: ModelInstanceId,
    shape: Arc<[usize]>,
    terms: Vec<Term>,
    constant: ConstantView,
}

impl LinArray {
    /// Build a linear array, checking that every term belongs to `owner`, has
    /// the array shape, and carries coefficient metadata whose owner and shape
    /// agree with the array.
    pub fn new(
        owner: ModelInstanceId,
        shape: impl Into<Arc<[usize]>>,
        terms: Vec<Term>,
        constant: ConstantView,
    ) -> Result<Self, ViewError> {
        let shape: Arc<[usize]> = shape.into();
        if product(&shape).is_none() {
            return Err(ViewError::ShapeOverflow);
        }
        for term in &terms {
            if term.vars.owner() != owner {
                return Err(ViewError::CrossModel {
                    left: owner,
                    right: term.vars.owner(),
                });
            }
            if term.vars.view().shape() != &shape[..] {
                return Err(ViewError::ShapeMismatch {
                    left: shape.clone(),
                    right: Arc::from(term.vars.view().shape()),
                });
            }
            validate_coeff(&term.coeff, owner, &shape)?;
        }
        validate_constant(&constant, owner, &shape)?;
        Ok(Self {
            owner,
            shape,
            terms,
            constant,
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

    /// Variable terms.
    pub fn terms(&self) -> &[Term] {
        &self.terms
    }

    /// Constant family.
    pub fn constant(&self) -> &ConstantView {
        &self.constant
    }

    /// Number of array cells.
    pub fn len(&self) -> usize {
        product(&self.shape).unwrap_or(0)
    }

    /// Whether the array covers no cells.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Scale every term and the constant, folding into each stored scale.
    pub fn scaled(mut self, alpha: f64) -> Self {
        for term in &mut self.terms {
            term.coeff = term.coeff.clone().scaled(alpha);
        }
        self.constant = self.constant.scaled(alpha);
        self
    }

    /// Add two arrays of the same owner and shape, concatenating terms.
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
        let constant = self.constant.try_add(other.constant)?;
        let mut terms = self.terms;
        terms.extend(other.terms);
        Ok(Self {
            owner: self.owner,
            shape: self.shape,
            terms,
            constant,
        })
    }

    /// Subtract two arrays of the same owner and shape.
    pub fn try_sub(self, other: Self) -> Result<Self, ViewError> {
        self.try_add(other.scaled(-1.0))
    }
}

impl ParamView {
    /// Conservative `ParamView * LinArray`.
    ///
    /// Stays on the fast IR only when every variable term coefficient is
    /// `One`/`Scalar` and the constant is `Zero`/`Scalar`, producing a
    /// `ScaledParam` coefficient/constant. Everything else returns `Ok(None)`
    /// for the general symbolic path. Cross-model composition is a typed error
    /// before member reconstruction; a shape mismatch is a fallback.
    pub fn mul_linarray(&self, array: &LinArray) -> Result<Option<LinArray>, ViewError> {
        if self.owner() != array.owner() {
            return Err(ViewError::CrossModel {
                left: self.owner(),
                right: array.owner(),
            });
        }
        if self.view().shape() != array.shape() {
            return Ok(None);
        }
        let mut terms = Vec::with_capacity(array.terms.len());
        for term in &array.terms {
            let scale = match term.coeff {
                CoeffView::One => 1.0,
                CoeffView::Scalar(value) => value,
                _ => return Ok(None),
            };
            terms.push(Term {
                vars: term.vars.clone(),
                coeff: CoeffView::ScaledParam {
                    scale,
                    params: self.clone(),
                },
            });
        }
        let constant = match array.constant {
            ConstantView::Zero => ConstantView::Zero,
            ConstantView::Scalar(value) => ConstantView::ScaledParam {
                scale: value,
                params: self.clone(),
            },
            _ => return Ok(None),
        };
        Ok(Some(LinArray::new(
            array.owner(),
            array.shape.clone(),
            terms,
            constant,
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bulk::{ParamSpan, VarSpan};
    use crate::id::Generation;
    use crate::modeling::View;

    fn var_view(owner: ModelInstanceId, len: usize) -> VarView {
        VarView::new(
            owner,
            View::contiguous(VarSpan::from_parts(0, len as u32, Generation::new()), len),
        )
    }

    fn param_view(owner: ModelInstanceId, len: usize) -> ParamView {
        ParamView::new(
            owner,
            View::contiguous(ParamSpan::from_parts(0, len as u32, Generation::new()), len),
        )
    }

    #[test]
    fn scaling_dense_folds_scale_without_copying_the_buffer() {
        let dense = NumView::from_vec(vec![1.0, 2.0, 3.0, 4.0]);
        let coeff = CoeffView::Dense {
            scale: 2.0,
            values: dense.clone(),
        };
        let scaled = coeff.scaled(3.0);
        match scaled {
            CoeffView::Dense { scale, values } => {
                assert_eq!(scale, 6.0);
                assert!(
                    values.shares_buffer(&dense),
                    "dense buffer must not be copied"
                );
            }
            other => panic!("expected Dense, got {other:?}"),
        }
    }

    #[test]
    fn param_times_one_or_scalar_terms_stays_on_the_fast_ir() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let vars = var_view(owner, 3);
        let params = param_view(owner, 3);
        let array = LinArray::new(
            owner,
            [3usize],
            vec![
                Term {
                    vars: vars.clone(),
                    coeff: CoeffView::One,
                },
                Term {
                    vars: vars.clone(),
                    coeff: CoeffView::Scalar(-2.5),
                },
            ],
            ConstantView::Scalar(4.0),
        )
        .expect("array");

        let product = params
            .mul_linarray(&array)
            .expect("fast path")
            .expect("representable");
        assert_eq!(product.terms().len(), 2);
        match &product.terms()[0].coeff {
            CoeffView::ScaledParam { scale, params: p } => {
                assert_eq!(*scale, 1.0);
                assert_eq!(p.owner(), owner);
            }
            other => panic!("expected ScaledParam, got {other:?}"),
        }
        match &product.terms()[1].coeff {
            CoeffView::ScaledParam { scale, .. } => assert_eq!(*scale, -2.5),
            other => panic!("expected ScaledParam, got {other:?}"),
        }
        match product.constant() {
            ConstantView::ScaledParam { scale, .. } => assert_eq!(*scale, 4.0),
            other => panic!("expected ScaledParam constant, got {other:?}"),
        }
        assert!(!product.constant().is_zero());
    }

    #[test]
    fn param_times_uncovered_forms_fall_back() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let vars = var_view(owner, 2);
        let params = param_view(owner, 2);

        // Dense variable coefficients are not in the initial fast subset.
        let dense_terms = LinArray::new(
            owner,
            [2usize],
            vec![Term {
                vars: vars.clone(),
                coeff: CoeffView::Dense {
                    scale: 1.0,
                    values: NumView::from_vec(vec![1.0, 2.0]),
                },
            }],
            ConstantView::Zero,
        )
        .expect("array");
        assert!(params.mul_linarray(&dense_terms).expect("ok").is_none());

        // A dense/numeric constant is not in the initial fast subset.
        let dense_constant = LinArray::new(
            owner,
            [2usize],
            vec![Term {
                vars: vars.clone(),
                coeff: CoeffView::One,
            }],
            ConstantView::Dense {
                scale: 1.0,
                values: NumView::from_vec(vec![5.0, 6.0]),
            },
        )
        .expect("array");
        assert!(params.mul_linarray(&dense_constant).expect("ok").is_none());

        // Cross-model product is a typed error before ID reconstruction.
        let foreign = param_view(ModelInstanceId::allocate().expect("owner"), 2);
        assert!(matches!(
            foreign.mul_linarray(&dense_constant),
            Err(ViewError::CrossModel { .. })
        ));
    }

    #[test]
    fn linarray_add_checks_owner_shape_and_constants() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let vars = var_view(owner, 2);
        let term = Term {
            vars: vars.clone(),
            coeff: CoeffView::Scalar(2.0),
        };
        let left = LinArray::new(
            owner,
            [2usize],
            vec![term.clone()],
            ConstantView::Scalar(1.0),
        )
        .expect("left");
        let right =
            LinArray::new(owner, [2usize], vec![term.clone()], ConstantView::Zero).expect("right");
        let sum = left.try_add(right).expect("sum");
        assert_eq!(sum.terms().len(), 2);
        assert_eq!(*sum.constant(), ConstantView::Scalar(1.0));

        // Shape mismatch is a typed error.
        let wide = LinArray::new(owner, [3usize], vec![], ConstantView::Zero).expect("wide");
        let legacy = LinArray::new(owner, [2usize], vec![], ConstantView::Zero).expect("legacy");
        assert!(matches!(
            legacy.try_add(wide),
            Err(ViewError::ShapeMismatch { .. })
        ));

        // Dense constant addition is a general fallback.
        let dense = LinArray::new(
            owner,
            [2usize],
            vec![],
            ConstantView::Dense {
                scale: 1.0,
                values: NumView::from_vec(vec![1.0, 1.0]),
            },
        )
        .expect("dense");
        let scalar =
            LinArray::new(owner, [2usize], vec![], ConstantView::Scalar(1.0)).expect("scalar");
        assert!(matches!(
            scalar.try_add(dense),
            Err(ViewError::Unsupported(_))
        ));
    }

    #[test]
    fn scaling_a_linarray_folds_every_coefficient() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let vars = var_view(owner, 2);
        let array = LinArray::new(
            owner,
            [2usize],
            vec![Term {
                vars,
                coeff: CoeffView::Dense {
                    scale: 1.0,
                    values: NumView::from_vec(vec![1.0, 2.0]),
                },
            }],
            ConstantView::Scalar(2.0),
        )
        .expect("array");
        let scaled = array.scaled(2.0);
        match &scaled.terms()[0].coeff {
            CoeffView::Dense { scale, .. } => assert_eq!(*scale, 2.0),
            other => panic!("expected Dense, got {other:?}"),
        }
        assert_eq!(*scaled.constant(), ConstantView::Scalar(4.0));
    }

    #[test]
    fn linarray_rejects_foreign_and_misshaped_coefficient_metadata() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let foreign = ModelInstanceId::allocate().expect("owner");
        let vars = var_view(owner, 3);

        // Foreign parameter owner in a coefficient.
        let foreign_coeff = LinArray::new(
            owner,
            [3usize],
            vec![Term {
                vars: vars.clone(),
                coeff: CoeffView::ScaledParam {
                    scale: 1.0,
                    params: param_view(foreign, 3),
                },
            }],
            ConstantView::Zero,
        );
        assert!(matches!(foreign_coeff, Err(ViewError::CrossModel { .. })));

        // Foreign parameter owner in a constant.
        let foreign_constant = LinArray::new(
            owner,
            [3usize],
            vec![Term {
                vars: vars.clone(),
                coeff: CoeffView::One,
            }],
            ConstantView::ScaledParam {
                scale: 1.0,
                params: param_view(foreign, 3),
            },
        );
        assert!(matches!(
            foreign_constant,
            Err(ViewError::CrossModel { .. })
        ));

        // Dense coefficient shape not matching the array.
        let dense_shape = LinArray::new(
            owner,
            [3usize],
            vec![Term {
                vars: vars.clone(),
                coeff: CoeffView::Dense {
                    scale: 1.0,
                    values: NumView::from_vec(vec![1.0, 2.0]),
                },
            }],
            ConstantView::Zero,
        );
        assert!(matches!(dense_shape, Err(ViewError::ShapeMismatch { .. })));

        // Parameter coefficient shape not matching the array.
        let param_shape = LinArray::new(
            owner,
            [3usize],
            vec![Term {
                vars: vars.clone(),
                coeff: CoeffView::ScaledParam {
                    scale: 1.0,
                    params: param_view(owner, 2),
                },
            }],
            ConstantView::Zero,
        );
        assert!(matches!(param_shape, Err(ViewError::ShapeMismatch { .. })));

        // Dense constant shape not matching the array.
        let constant_shape = LinArray::new(
            owner,
            [3usize],
            vec![],
            ConstantView::Dense {
                scale: 1.0,
                values: NumView::from_vec(vec![1.0, 2.0]),
            },
        );
        assert!(matches!(
            constant_shape,
            Err(ViewError::ShapeMismatch { .. })
        ));
    }

    #[test]
    fn numview_is_validated_and_transforms_are_metadata_only() {
        let values: Arc<[f64]> = Arc::from(vec![10.0, 20.0, 30.0, 40.0]);
        let view = NumView::new(values, [2usize, 2], [2isize, 1], 0).expect("view");
        assert_eq!(view.get(0), Some(10.0));
        assert_eq!(view.get(3), Some(40.0));

        let transposed = view.transpose(0, 1).expect("transpose");
        let mapped: Vec<f64> = (0..4).filter_map(|i| transposed.get(i)).collect();
        assert_eq!(mapped, vec![10.0, 30.0, 20.0, 40.0]);

        let reversed = view.reverse(0).expect("reverse");
        let mapped: Vec<f64> = (0..4).filter_map(|i| reversed.get(i)).collect();
        assert_eq!(mapped, vec![30.0, 40.0, 10.0, 20.0]);
        // Metadata transforms never copy the buffer.
        assert!(view.shares_buffer(&transposed));
        assert!(view.shares_buffer(&reversed));

        // Metadata whose offsets leave the buffer is rejected.
        assert!(matches!(
            NumView::new(Arc::from(vec![1.0, 2.0]), [3usize], [1isize], 0),
            Err(ViewError::Unsupported(_))
        ));
    }
}
