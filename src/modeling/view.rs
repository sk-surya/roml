//! Model-owned ordinal view metadata (MIR-03, D-019).
//!
//! [`View`] is the shared L1 representation of a strided slice over a trusted
//! block span: `span + shape + signed strides + offset`. Slicing, transposing
//! and reversing are **metadata-only** (O(1) in cells; no per-cell allocation),
//! and the row-major ordinal convention is exactly the one frozen by
//! [`StridedMap`](crate::bulk::StridedMap) (last dimension fastest).
//!
//! Symbolic views ([`VarView`], [`ParamView`]) retain the owning
//! [`ModelInstanceId`]; composition across models is a typed error **before**
//! member IDs are reconstructed.

use std::sync::Arc;

use crate::bulk::{ParamSpan, StridedMap, VarSpan};
use crate::id::{ParamId, VarId};
use crate::ModelInstanceId;

/// Errors from constructing or composing model-owned views.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// Shape rank and stride rank differ.
    RankMismatch {
        /// Shape rank.
        shape: usize,
        /// Stride rank.
        strides: usize,
    },
    /// The shape product does not fit in `usize`.
    ShapeOverflow,
    /// An operation referenced an axis outside the shape rank.
    AxisOutOfRange {
        /// Requested axis.
        axis: usize,
        /// Shape rank.
        rank: usize,
    },
    /// A slice range did not fit within the axis dimension.
    SliceOutOfRange {
        /// Axis being sliced.
        axis: usize,
        /// Requested start offset.
        start: usize,
        /// Requested slice length.
        len: usize,
        /// Axis dimension.
        dim: usize,
    },
    /// Two array operands have different shapes.
    ShapeMismatch {
        /// Left shape.
        left: Arc<[usize]>,
        /// Right shape.
        right: Arc<[usize]>,
    },
    /// The composition is outside the initial conservative IR and must use the
    /// general symbolic path.
    Unsupported(&'static str),
    /// A checked metadata transformation overflowed `isize`/`usize`.
    IndexOverflow,
    /// A view maps a member offset outside its underlying span.
    SpanOutOfRange,
    /// Two symbolic arrays belong to different model instances.
    CrossModel {
        /// Left operand owner.
        left: ModelInstanceId,
        /// Right operand owner.
        right: ModelInstanceId,
    },
}

impl std::fmt::Display for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RankMismatch { shape, strides } => {
                write!(f, "shape rank {shape} does not match stride rank {strides}")
            }
            Self::ShapeOverflow => write!(f, "view shape product does not fit in usize"),
            Self::AxisOutOfRange { axis, rank } => {
                write!(f, "view axis {axis} is outside rank {rank}")
            }
            Self::SliceOutOfRange {
                axis,
                start,
                len,
                dim,
            } => write!(
                f,
                "view slice {start}..{} exceeds axis {axis} dimension {dim}",
                start.saturating_add(*len)
            ),
            Self::ShapeMismatch { left, right } => {
                write!(f, "array shape mismatch: {left:?} vs {right:?}")
            }
            Self::Unsupported(what) => {
                write!(f, "unsupported IR composition (general fallback): {what}")
            }
            Self::IndexOverflow => write!(f, "view metadata transformation overflowed"),
            Self::SpanOutOfRange => write!(f, "view maps a member outside its span"),
            Self::CrossModel { left, right } => {
                write!(f, "cross-model view composition: {left:?} vs {right:?}")
            }
        }
    }
}

impl std::error::Error for ViewError {}

/// The inclusive range of member offsets a strided view can map, or `None` when
/// a dimension does not fit `isize` or the arithmetic overflows. Computed in
/// O(rank) from metadata only.
pub(crate) fn mapped_range(
    shape: &[usize],
    strides: &[isize],
    offset: isize,
) -> Option<(isize, isize)> {
    if shape.len() != strides.len() {
        return None;
    }
    let mut low = offset;
    let mut high = offset;
    for (dim, stride) in shape.iter().zip(strides.iter()) {
        if *dim == 0 {
            continue;
        }
        let last = isize::try_from(dim.checked_sub(1)?).ok()?;
        let span = last.checked_mul(*stride)?;
        if span >= 0 {
            high = high.checked_add(span)?;
        } else {
            low = low.checked_add(span)?;
        }
    }
    Some((low, high))
}

/// Prove from metadata that every member offset a view maps is within `span_len`.
fn validate_within_span<S>(view: &View<S>, span_len: usize) -> Result<(), ViewError> {
    if view.is_empty() {
        return Ok(());
    }
    let (low, high) = view.mapped_range().ok_or(ViewError::IndexOverflow)?;
    let len = isize::try_from(span_len).map_err(|_| ViewError::IndexOverflow)?;
    if low < 0 || high >= len {
        return Err(ViewError::SpanOutOfRange);
    }
    Ok(())
}

/// A model-owned strided view over a trusted block span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View<S> {
    span: S,
    map: StridedMap,
}

impl<S> View<S> {
    /// Build a validated view from span metadata.
    ///
    /// Rejects a shape/stride rank mismatch and a shape product that does not
    /// fit in `usize`; the ordinal mapping itself stays checked in [`Self::get`].
    pub fn new(
        span: S,
        shape: impl Into<Arc<[usize]>>,
        strides: impl Into<Arc<[isize]>>,
        offset: isize,
    ) -> Result<Self, ViewError> {
        let shape: Arc<[usize]> = shape.into();
        let strides: Arc<[isize]> = strides.into();
        if shape.len() != strides.len() {
            return Err(ViewError::RankMismatch {
                shape: shape.len(),
                strides: strides.len(),
            });
        }
        let map = StridedMap::new(shape, strides, offset);
        if !map.is_well_formed() {
            return Err(ViewError::ShapeOverflow);
        }
        Ok(Self { span, map })
    }

    /// A one-dimensional contiguous view `0..len` over the span.
    pub fn contiguous(span: S, len: usize) -> Self {
        Self {
            span,
            map: StridedMap::contiguous(len),
        }
    }

    /// The underlying trusted span.
    pub fn span(&self) -> &S {
        &self.span
    }

    /// Shape dimensions (row-major).
    pub fn shape(&self) -> &[usize] {
        self.map.shape()
    }

    /// Signed strides.
    pub fn strides(&self) -> &[isize] {
        self.map.strides()
    }

    /// Signed offset into the span.
    pub fn offset(&self) -> isize {
        self.map.offset()
    }

    /// Inclusive range of member offsets the view maps, or `None` on overflow.
    pub(crate) fn mapped_range(&self) -> Option<(isize, isize)> {
        mapped_range(self.shape(), self.strides(), self.offset())
    }

    /// Number of mapped ordinals (product of the shape).
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the view covers no ordinals.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Resolve a row-major ordinal to its signed member offset within the
    /// span, or `None` when out of bounds or the arithmetic overflows.
    pub fn get(&self, ordinal: usize) -> Option<isize> {
        self.map.get(ordinal)
    }

    /// Metadata-only slice along `axis`: `[start, start + len)`.
    pub fn slice(&self, axis: usize, start: usize, len: usize) -> Result<Self, ViewError>
    where
        S: Clone,
    {
        let rank = self.map.shape().len();
        if axis >= rank {
            return Err(ViewError::AxisOutOfRange { axis, rank });
        }
        let dim = self.map.shape()[axis];
        if start.checked_add(len).is_none_or(|end| end > dim) {
            return Err(ViewError::SliceOutOfRange {
                axis,
                start,
                len,
                dim,
            });
        }
        let mut shape = self.map.shape().to_vec();
        shape[axis] = len;
        let strides = self.map.strides().to_vec();
        let start_isize = isize::try_from(start).map_err(|_| ViewError::IndexOverflow)?;
        let delta = start_isize
            .checked_mul(strides[axis])
            .ok_or(ViewError::IndexOverflow)?;
        let offset = self
            .map
            .offset()
            .checked_add(delta)
            .ok_or(ViewError::IndexOverflow)?;
        Ok(Self {
            span: self.span.clone(),
            map: StridedMap::new(shape, strides, offset),
        })
    }

    /// Metadata-only reversal (negative step) along `axis`.
    ///
    /// Reversing a zero-length axis is a no-op: the offset is not shifted and
    /// the stride is not negated. All arithmetic is checked; overflow is a typed
    /// [`ViewError::IndexOverflow`], never a wrap or debug panic.
    pub fn reverse(&self, axis: usize) -> Result<Self, ViewError>
    where
        S: Clone,
    {
        let rank = self.map.shape().len();
        if axis >= rank {
            return Err(ViewError::AxisOutOfRange { axis, rank });
        }
        let dim = self.map.shape()[axis];
        if dim == 0 {
            return Ok(self.clone());
        }
        let mut strides = self.map.strides().to_vec();
        let last = isize::try_from(dim - 1).map_err(|_| ViewError::IndexOverflow)?;
        let delta = last
            .checked_mul(strides[axis])
            .ok_or(ViewError::IndexOverflow)?;
        let offset = self
            .map
            .offset()
            .checked_add(delta)
            .ok_or(ViewError::IndexOverflow)?;
        strides[axis] = strides[axis]
            .checked_neg()
            .ok_or(ViewError::IndexOverflow)?;
        Ok(Self {
            span: self.span.clone(),
            map: StridedMap::new(self.map.shape().to_vec(), strides, offset),
        })
    }

    /// Metadata-only transpose (swap two axes).
    pub fn transpose(&self, a: usize, b: usize) -> Result<Self, ViewError>
    where
        S: Clone,
    {
        let rank = self.map.shape().len();
        if a >= rank {
            return Err(ViewError::AxisOutOfRange { axis: a, rank });
        }
        if b >= rank {
            return Err(ViewError::AxisOutOfRange { axis: b, rank });
        }
        let mut shape = self.map.shape().to_vec();
        let mut strides = self.map.strides().to_vec();
        shape.swap(a, b);
        strides.swap(a, b);
        Ok(Self {
            span: self.span.clone(),
            map: StridedMap::new(shape, strides, self.map.offset()),
        })
    }
}

/// A symbolic variable view owned by one model instance.
///
/// Construction is crate-private until the MIR-04 model builders create
/// symbolic views from the owning model, so no public safe path can pair a span
/// from model A with owner B:
///
/// ```compile_fail
/// use roml::bulk::VarSpan;
/// use roml::modeling::{VarView, View};
/// use roml::ModelInstanceId;
/// # fn span() -> VarSpan { unimplemented!() }
/// let span = span();
/// let owner = ModelInstanceId::allocate().unwrap();
/// let _ = VarView::new(owner, View::contiguous(span, 1));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VarView {
    owner: ModelInstanceId,
    view: View<VarSpan>,
}

impl VarView {
    /// Wrap a trusted variable span in its owning model.
    ///
    /// Crate-private until the MIR-04 model builders construct symbolic views
    /// from the owning model: a public constructor would let ownership be
    /// forged. Every mapped member offset is validated against the span.
    pub(crate) fn new(owner: ModelInstanceId, view: View<VarSpan>) -> Result<Self, ViewError> {
        validate_within_span(&view, view.span().len())?;
        Ok(Self { owner, view })
    }

    /// The owning model instance.
    pub fn owner(&self) -> ModelInstanceId {
        self.owner
    }

    /// View metadata.
    pub fn view(&self) -> &View<VarSpan> {
        &self.view
    }

    /// Reconstruct the member variable for a row-major ordinal.
    pub fn member(&self, ordinal: usize) -> Option<VarId> {
        let offset = usize::try_from(self.view.get(ordinal)?).ok()?;
        self.view.span().id_at(offset)
    }

    /// Metadata-only slice.
    pub fn slice(&self, axis: usize, start: usize, len: usize) -> Result<Self, ViewError> {
        Ok(Self {
            owner: self.owner,
            view: self.view.slice(axis, start, len)?,
        })
    }

    /// Metadata-only reversal.
    pub fn reverse(&self, axis: usize) -> Result<Self, ViewError> {
        Ok(Self {
            owner: self.owner,
            view: self.view.reverse(axis)?,
        })
    }

    /// Metadata-only transpose.
    pub fn transpose(&self, a: usize, b: usize) -> Result<Self, ViewError> {
        Ok(Self {
            owner: self.owner,
            view: self.view.transpose(a, b)?,
        })
    }

    /// Reject composition with a view from a different model instance.
    pub fn check_same_model(&self, other: &Self) -> Result<(), ViewError> {
        if self.owner != other.owner {
            return Err(ViewError::CrossModel {
                left: self.owner,
                right: other.owner,
            });
        }
        Ok(())
    }
}

/// A symbolic parameter view owned by one model instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParamView {
    owner: ModelInstanceId,
    view: View<ParamSpan>,
}

impl ParamView {
    /// Wrap a trusted parameter span in its owning model.
    ///
    /// Crate-private until the MIR-04 model builders construct symbolic views
    /// from the owning model. Every mapped member offset is validated against
    /// the span.
    pub(crate) fn new(owner: ModelInstanceId, view: View<ParamSpan>) -> Result<Self, ViewError> {
        validate_within_span(&view, view.span().len())?;
        Ok(Self { owner, view })
    }

    /// The owning model instance.
    pub fn owner(&self) -> ModelInstanceId {
        self.owner
    }

    /// View metadata.
    pub fn view(&self) -> &View<ParamSpan> {
        &self.view
    }

    /// Reconstruct the member parameter for a row-major ordinal.
    pub fn member(&self, ordinal: usize) -> Option<ParamId> {
        let offset = usize::try_from(self.view.get(ordinal)?).ok()?;
        self.view.span().id_at(offset)
    }

    /// Metadata-only slice.
    pub fn slice(&self, axis: usize, start: usize, len: usize) -> Result<Self, ViewError> {
        Ok(Self {
            owner: self.owner,
            view: self.view.slice(axis, start, len)?,
        })
    }

    /// Metadata-only reversal.
    pub fn reverse(&self, axis: usize) -> Result<Self, ViewError> {
        Ok(Self {
            owner: self.owner,
            view: self.view.reverse(axis)?,
        })
    }

    /// Metadata-only transpose.
    pub fn transpose(&self, a: usize, b: usize) -> Result<Self, ViewError> {
        Ok(Self {
            owner: self.owner,
            view: self.view.transpose(a, b)?,
        })
    }

    /// Reject composition with a view from a different model instance.
    pub fn check_same_model(&self, other: &Self) -> Result<(), ViewError> {
        if self.owner != other.owner {
            return Err(ViewError::CrossModel {
                left: self.owner,
                right: other.owner,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Generation;

    fn var_view(owner: ModelInstanceId, start: u32, len: u32) -> VarView {
        VarView::new(
            owner,
            View::contiguous(
                VarSpan::from_parts(start, len, Generation::new()),
                len as usize,
            ),
        )
        .expect("var view")
    }

    fn param_view(owner: ModelInstanceId, start: u32, len: u32) -> ParamView {
        ParamView::new(
            owner,
            View::contiguous(
                ParamSpan::from_parts(start, len, Generation::new()),
                len as usize,
            ),
        )
        .expect("param view")
    }

    #[test]
    fn view_maps_row_major_ordinals() {
        let view = View::new((), [2usize, 3], [3isize, 1], 0).expect("view");
        let mapped: Vec<isize> = (0..6).filter_map(|i| view.get(i)).collect();
        assert_eq!(mapped, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(view.get(6), None);
    }

    #[test]
    fn slice_transpose_and_reverse_are_metadata_only() {
        let view = View::new((), [2usize, 3], [3isize, 1], 0).expect("view");

        // Slice axis 1 to columns 1..3.
        let sliced = view.slice(1, 1, 2).expect("slice");
        assert_eq!(sliced.shape(), &[2, 2]);
        let mapped: Vec<isize> = (0..4).filter_map(|i| sliced.get(i)).collect();
        assert_eq!(mapped, vec![1, 2, 4, 5]);

        // Reverse axis 1 (negative step).
        let reversed = view.reverse(1).expect("reverse");
        assert_eq!(reversed.shape(), &[2, 3]);
        assert_eq!(reversed.strides(), &[3, -1]);
        let mapped: Vec<isize> = (0..6).filter_map(|i| reversed.get(i)).collect();
        assert_eq!(mapped, vec![2, 1, 0, 5, 4, 3]);

        // Transpose axes: shape/strides swap, same ordinals.
        let transposed = view.transpose(0, 1).expect("transpose");
        assert_eq!(transposed.shape(), &[3, 2]);
        assert_eq!(transposed.strides(), &[1, 3]);
        let mapped: Vec<isize> = (0..6).filter_map(|i| transposed.get(i)).collect();
        assert_eq!(mapped, vec![0, 3, 1, 4, 2, 5]);
    }

    #[test]
    fn malformed_metadata_is_a_typed_error() {
        assert_eq!(
            View::new((), [2usize, 3], [1isize], 0),
            Err(ViewError::RankMismatch {
                shape: 2,
                strides: 1
            })
        );
        assert_eq!(
            View::new((), [usize::MAX, 2], [1isize, 1], 0),
            Err(ViewError::ShapeOverflow)
        );
        let view = View::new((), [2usize, 3], [3isize, 1], 0).expect("view");
        assert_eq!(
            view.slice(2, 0, 1),
            Err(ViewError::AxisOutOfRange { axis: 2, rank: 2 })
        );
        assert_eq!(
            view.slice(1, 2, 2),
            Err(ViewError::SliceOutOfRange {
                axis: 1,
                start: 2,
                len: 2,
                dim: 3
            })
        );
    }

    #[test]
    fn symbolic_views_reconstruct_members_and_reject_cross_model() {
        let owner = ModelInstanceId::allocate().expect("owner");
        let other = ModelInstanceId::allocate().expect("owner");

        let vars = var_view(owner, 10, 4);
        let members: Vec<VarId> = (0..4).filter_map(|i| vars.member(i)).collect();
        assert_eq!(members.len(), 4);
        assert_eq!(members[0].index(), 10);
        assert_eq!(members[3].index(), 13);

        let params = param_view(owner, 20, 3);
        assert_eq!(params.member(2).expect("param").index(), 22);

        let local = var_view(owner, 0, 2);
        assert!(vars.check_same_model(&local).is_ok());
        let foreign = var_view(other, 0, 2);
        assert!(matches!(
            vars.check_same_model(&foreign),
            Err(ViewError::CrossModel { .. })
        ));

        // Sliced/reversed views keep the owner.
        let sliced = vars.slice(0, 1, 2).expect("slice");
        assert_eq!(sliced.owner(), owner);
        assert_eq!(sliced.member(0).expect("member").index(), 11);
    }

    #[test]
    fn slice_and_reverse_overflows_are_typed() {
        // A slice start beyond isize range is a typed overflow, not a wrap.
        let huge = View::new((), [usize::MAX], [1isize], 0).expect("view");
        assert_eq!(
            huge.slice(0, usize::MAX - 1, 1),
            Err(ViewError::IndexOverflow)
        );

        // Stride negation overflow (isize::MIN).
        let min_stride = View::new((), [2usize], [isize::MIN], 0).expect("view");
        assert_eq!(min_stride.reverse(0), Err(ViewError::IndexOverflow));

        // Offset multiplication overflow.
        let big_stride = View::new((), [3usize], [isize::MAX], 0).expect("view");
        assert_eq!(big_stride.reverse(0), Err(ViewError::IndexOverflow));

        // Offset addition overflow.
        let big_offset = View::new((), [3usize], [1isize], isize::MAX).expect("view");
        assert_eq!(big_offset.reverse(0), Err(ViewError::IndexOverflow));

        // Out-of-range slice stays a bounds error.
        let view = View::new((), [2usize, 3], [3isize, 1], 0).expect("view");
        assert!(matches!(
            view.slice(1, 2, 2),
            Err(ViewError::SliceOutOfRange { .. })
        ));
    }

    #[test]
    fn reversing_a_zero_length_axis_does_not_shift_the_offset() {
        let view = View::new((), [0usize], [1isize], 5).expect("view");
        let reversed = view.reverse(0).expect("reverse");
        assert_eq!(reversed.offset(), 5);
        assert_eq!(reversed.shape(), &[0]);
        assert!(reversed.is_empty());
    }

    #[test]
    fn symbolic_view_range_is_validated_against_the_span() {
        let owner = ModelInstanceId::allocate().expect("owner");
        // Offset beyond the span.
        let beyond = VarView::new(
            owner,
            View::new(
                VarSpan::from_parts(0, 3, Generation::new()),
                [2usize],
                [1isize],
                2,
            )
            .expect("view"),
        );
        assert!(matches!(beyond, Err(ViewError::SpanOutOfRange)));

        // Positive-stride range extending past the end.
        let past_end = VarView::new(
            owner,
            View::new(
                VarSpan::from_parts(0, 2, Generation::new()),
                [3usize],
                [1isize],
                0,
            )
            .expect("view"),
        );
        assert!(matches!(past_end, Err(ViewError::SpanOutOfRange)));

        // Negative range below zero.
        let below_zero = ParamView::new(
            owner,
            View::new(
                ParamSpan::from_parts(0, 3, Generation::new()),
                [4usize],
                [-1isize],
                0,
            )
            .expect("view"),
        );
        assert!(matches!(below_zero, Err(ViewError::SpanOutOfRange)));

        // A valid reversed view stays valid and maps the reversed ordinals.
        let base = View::new(
            VarSpan::from_parts(0, 4, Generation::new()),
            [4usize],
            [1isize],
            0,
        )
        .expect("view");
        let reversed = VarView::new(owner, base.reverse(0).expect("reverse")).expect("valid");
        let mapped: Vec<isize> = (0..4).filter_map(|i| reversed.view().get(i)).collect();
        assert_eq!(mapped, vec![3, 2, 1, 0]);
    }

    #[test]
    fn dimensions_above_isize_max_do_not_wrap() {
        // A dimension above isize::MAX: ordinal decomposition is a typed None,
        // never a wrapped coordinate. No buffer of that size is allocated.
        let view = View::new((), [usize::MAX], [1isize], 0).expect("view");
        assert_eq!(view.get(usize::MAX - 1), None);
        assert_eq!(view.get(0), Some(0));
    }
}
