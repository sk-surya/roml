//! Labeled array boundary metadata (MIR-04, IR-24).
//!
//! [`Labeled<A>`] attaches per-axis names and per-index labels to a structured
//! array. Labels are **boundary metadata**: alignment is checked once (with
//! typed errors), labels never enter [`LinArray`]/`ValueExpr` nodes, and they
//! are never stored in the canonical model, so relabeling cannot change the
//! ordinal IR.

use std::sync::Arc;

use crate::modeling::{LinArray, ParamArray, VarArray};

/// Anything with a row-major shape that can be labeled.
pub trait Shaped {
    /// Row-major shape.
    fn shape(&self) -> &[usize];
}

impl Shaped for VarArray {
    fn shape(&self) -> &[usize] {
        VarArray::shape(self)
    }
}

impl Shaped for ParamArray {
    fn shape(&self) -> &[usize] {
        ParamArray::shape(self)
    }
}

impl Shaped for LinArray {
    fn shape(&self) -> &[usize] {
        LinArray::shape(self)
    }
}

/// One labeled axis: an optional axis name plus per-index labels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Axis {
    name: Option<String>,
    labels: Arc<[String]>,
}

impl Axis {
    /// A labeled axis.
    pub fn new(name: Option<String>, labels: Vec<String>) -> Self {
        Self {
            name,
            labels: Arc::from(labels),
        }
    }

    /// The axis name, if any.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The per-index labels.
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// Number of labels (the axis width).
    pub fn len(&self) -> usize {
        self.labels.len()
    }

    /// Whether the axis has no labels.
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }
}

/// A typed label-boundary error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LabelError {
    /// The number of axes does not match the array rank.
    RankMismatch {
        /// Supplied axis count.
        axes: usize,
        /// Array rank.
        rank: usize,
    },
    /// An axis label count does not match the shape dimension.
    WidthMismatch {
        /// Axis index.
        axis: usize,
        /// Supplied label count.
        labels: usize,
        /// Shape dimension.
        dim: usize,
    },
    /// The requested axis does not exist.
    AxisNotFound {
        /// Axis index.
        axis: usize,
    },
    /// Two arrays disagree on an axis name or its labels.
    AxisMismatch {
        /// Axis index.
        axis: usize,
        /// Left-hand description.
        left: String,
        /// Right-hand description.
        right: String,
    },
}

impl std::fmt::Display for LabelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RankMismatch { axes, rank } => {
                write!(f, "rank mismatch: {axes} axes for rank {rank} array")
            }
            Self::WidthMismatch { axis, labels, dim } => {
                write!(
                    f,
                    "axis {axis} width mismatch: {labels} labels for dim {dim}"
                )
            }
            Self::AxisNotFound { axis } => write!(f, "axis {axis} not found"),
            Self::AxisMismatch { axis, left, right } => {
                write!(f, "axis {axis} mismatch: left {left} vs right {right}")
            }
        }
    }
}

impl std::error::Error for LabelError {}

/// A structured array with per-axis labels (boundary metadata only).
#[derive(Clone, Debug)]
pub struct Labeled<A> {
    inner: A,
    axes: Vec<Axis>,
}

impl<A: Shaped> Labeled<A> {
    /// Attach labels, validating the rank and each axis width.
    pub fn new(inner: A, axes: Vec<Axis>) -> Result<Self, LabelError> {
        let shape = inner.shape();
        if axes.len() != shape.len() {
            return Err(LabelError::RankMismatch {
                axes: axes.len(),
                rank: shape.len(),
            });
        }
        for (axis, (a, &dim)) in axes.iter().zip(shape).enumerate() {
            if a.len() != dim {
                return Err(LabelError::WidthMismatch {
                    axis,
                    labels: a.len(),
                    dim,
                });
            }
        }
        Ok(Self { inner, axes })
    }

    /// The underlying ordinal array.
    pub fn inner(&self) -> &A {
        &self.inner
    }

    /// Consume the wrapper, yielding the ordinal array (labels dropped).
    pub fn into_inner(self) -> A {
        self.inner
    }

    /// The array shape.
    pub fn shape(&self) -> &[usize] {
        self.inner.shape()
    }

    /// Number of axes.
    pub fn rank(&self) -> usize {
        self.axes.len()
    }

    /// The axes.
    pub fn axes(&self) -> &[Axis] {
        &self.axes
    }

    /// One axis, if present.
    pub fn axis(&self, axis: usize) -> Option<&Axis> {
        self.axes.get(axis)
    }

    /// Check alignment on one axis (axis name and labels).
    pub fn align_axis(&self, other: &Self, axis: usize) -> Result<(), LabelError> {
        let left = self
            .axes
            .get(axis)
            .ok_or(LabelError::AxisNotFound { axis })?;
        let right = other
            .axes
            .get(axis)
            .ok_or(LabelError::AxisNotFound { axis })?;
        if left.name != right.name {
            return Err(LabelError::AxisMismatch {
                axis,
                left: left.name.clone().unwrap_or_default(),
                right: right.name.clone().unwrap_or_default(),
            });
        }
        if left.labels != right.labels {
            return Err(LabelError::AxisMismatch {
                axis,
                left: left.labels.join(","),
                right: right.labels.join(","),
            });
        }
        Ok(())
    }

    /// Check alignment on every axis (the boundary check).
    pub fn align(&self, other: &Self) -> Result<(), LabelError> {
        if self.rank() != other.rank() {
            return Err(LabelError::RankMismatch {
                axes: self.rank(),
                rank: other.rank(),
            });
        }
        for axis in 0..self.rank() {
            self.align_axis(other, axis)?;
        }
        Ok(())
    }

    /// Rename an axis (metadata only).
    pub fn rename_axis(&mut self, axis: usize, name: Option<String>) -> Result<(), LabelError> {
        self.axes
            .get_mut(axis)
            .ok_or(LabelError::AxisNotFound { axis })?
            .name = name;
        Ok(())
    }

    /// Replace an axis's labels (metadata only), validating the width.
    pub fn set_axis_labels(&mut self, axis: usize, labels: Vec<String>) -> Result<(), LabelError> {
        let dim = *self
            .inner
            .shape()
            .get(axis)
            .ok_or(LabelError::AxisNotFound { axis })?;
        if labels.len() != dim {
            return Err(LabelError::WidthMismatch {
                axis,
                labels: labels.len(),
                dim,
            });
        }
        self.axes
            .get_mut(axis)
            .ok_or(LabelError::AxisNotFound { axis })?
            .labels = Arc::from(labels);
        Ok(())
    }
}
