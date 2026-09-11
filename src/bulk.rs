//! Language-independent L2 block descriptors (D-019, DESIGN §2–3).
//!
//! A [`VarSpan`]/[`ParamSpan`] is a **trusted block identity**. It can only be
//! produced by core block allocation; there is no public `(start, len)`
//! constructor. All members of a fresh block share the arena's fresh
//! generation, and because arena indices are never reused, a member can be
//! reconstructed from `(span, offset)` and revalidated against the store:
//! deleting one member makes only that member stale, and there is no
//! span-wide epoch.
//!
//! The descriptors here are deliberately free of any L1/`roml::modeling` or
//! Python view type. Views over blocks are a later-phase concept; the span
//! is the persisted, ordinal anchor they build on.

use std::sync::Arc;

use crate::id::{Generation, ParamId, VarId};
use crate::model::variable::{Bounds, VarType};

/// Bounds input to the `Model::add_variable_block` block API (MIR-01).
///
/// The caller may supply one bounds value for the whole block or exactly one
/// entry per variable.
#[derive(Clone, Copy, Debug)]
pub enum BlockBounds<'a> {
    /// Every variable in the block is allocated with these bounds.
    Uniform(Bounds),
    /// One bounds entry per variable, in block order.
    PerElement(&'a [Bounds]),
}

/// Owned, self-contained bounds payload for a variable block.
///
/// Converted from [`BlockBounds`] at allocation time so the resulting journal
/// entry / delta op never borrows caller input and never queries live model
/// state.
#[derive(Clone, Debug, PartialEq)]
pub enum BlockBoundsOwned {
    /// Every variable shares these bounds.
    Uniform(Bounds),
    /// One entry per variable, in block order.
    PerElement(Arc<[Bounds]>),
}

impl BlockBoundsOwned {
    /// Resolve the bounds of `offset`, or `None` when out of range.
    ///
    /// `PerElement` entries are indexed; a uniform payload answers every
    /// in-range offset with the same bounds.
    pub fn get(&self, offset: usize, len: usize) -> Option<Bounds> {
        match self {
            BlockBoundsOwned::Uniform(bounds) => (offset < len).then_some(*bounds),
            BlockBoundsOwned::PerElement(bounds) => bounds.get(offset).copied(),
        }
    }

    /// Convert validated caller input into an owned, self-contained payload.
    #[allow(dead_code)] // consumed by MIR-01 Task 3 (packed variable-block op)
    pub(crate) fn from_input(input: BlockBounds<'_>, n: usize) -> Self {
        match input {
            BlockBounds::Uniform(bounds) => BlockBoundsOwned::Uniform(bounds),
            BlockBounds::PerElement(bounds) => {
                debug_assert_eq!(bounds.len(), n);
                BlockBoundsOwned::PerElement(Arc::from(bounds))
            }
        }
    }
}

/// A contiguous, trusted span of variable ordinals.
///
/// The fields are private and there is no public constructor: a caller cannot
/// forge a span from `(start, len)`.
///
/// ```compile_fail
/// // Private fields plus no public constructor make this fail to compile.
/// let _ = roml::bulk::VarSpan { start: 0, len: 3, generation: Default::default() };
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VarSpan {
    start: u32,
    len: u32,
    generation: Generation,
}

impl VarSpan {
    /// Number of variables in the span.
    #[inline]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether the span is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Trusted constructor; crate-private so no caller can forge a span.
    pub(crate) fn from_parts(start: u32, len: u32, generation: Generation) -> Self {
        Self {
            start,
            len,
            generation,
        }
    }

    /// Reconstruct the `offset`-th member identity.
    ///
    /// Returns `None` for an out-of-range offset. Liveness/generation of the
    /// returned id is validated by the owning store, so a deleted member
    /// fails validation while its siblings remain valid.
    #[allow(dead_code)] // consumed by MIR-01 Task 3 (packed variable-block op)
    pub(crate) fn id_at(&self, offset: usize) -> Option<VarId> {
        if offset >= self.len as usize {
            return None;
        }
        Some(VarId::new(self.start + offset as u32, self.generation))
    }

    /// First arena index covered by the span.
    #[inline]
    #[allow(dead_code)] // consumed by MIR-01 Task 3 (packed variable-block op)
    pub(crate) fn start(&self) -> u32 {
        self.start
    }

    /// Generation shared by every member of the span.
    #[inline]
    #[allow(dead_code)] // consumed by MIR-01 Task 3 (packed variable-block op)
    pub(crate) fn generation(&self) -> Generation {
        self.generation
    }
}

/// A contiguous, trusted span of parameter ordinals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParamSpan {
    start: u32,
    len: u32,
    generation: Generation,
}

impl ParamSpan {
    /// Number of parameters in the span.
    #[inline]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether the span is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Trusted constructor; crate-private so no caller can forge a span.
    pub(crate) fn from_parts(start: u32, len: u32, generation: Generation) -> Self {
        Self {
            start,
            len,
            generation,
        }
    }

    /// Reconstruct the `offset`-th member identity.
    #[allow(dead_code)] // consumed by MIR-01 Task 3 (packed variable-block op)
    pub(crate) fn id_at(&self, offset: usize) -> Option<ParamId> {
        if offset >= self.len as usize {
            return None;
        }
        Some(ParamId::new(self.start + offset as u32, self.generation))
    }

    /// First arena index covered by the span.
    #[inline]
    #[allow(dead_code)] // consumed by MIR-01 Task 3 (packed variable-block op)
    pub(crate) fn start(&self) -> u32 {
        self.start
    }

    /// Generation shared by every member of the span.
    #[inline]
    #[allow(dead_code)] // consumed by MIR-01 Task 3 (packed variable-block op)
    pub(crate) fn generation(&self) -> Generation {
        self.generation
    }
}

/// A self-contained variable block: trusted span plus its solver-facing
/// domain description. Shared by the packed revision `Change` and its
/// compiled `ModelOp` so neither needs the live model to interpret it.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableBlock {
    span: VarSpan,
    var_type: VarType,
    bounds: BlockBoundsOwned,
}

impl VariableBlock {
    /// Construct a block payload from a trusted span and validated parts.
    #[allow(dead_code)] // consumed by MIR-01 Task 3 (packed variable-block op)
    pub(crate) fn new(span: VarSpan, var_type: VarType, bounds: BlockBoundsOwned) -> Self {
        Self {
            span,
            var_type,
            bounds,
        }
    }

    /// The block's trusted span.
    #[inline]
    pub fn span(&self) -> VarSpan {
        self.span
    }

    /// Number of variables in the block.
    #[inline]
    pub fn len(&self) -> usize {
        self.span.len()
    }

    /// Whether the block is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.span.is_empty()
    }

    /// Declared variable type shared by the block.
    #[inline]
    pub fn var_type(&self) -> VarType {
        self.var_type
    }

    /// The block's bounds payload.
    #[inline]
    pub fn bounds(&self) -> &BlockBoundsOwned {
        &self.bounds
    }

    /// Reconstruct the `offset`-th variable identity.
    #[allow(dead_code)] // consumed by MIR-01 Task 3 (packed variable-block op)
    pub(crate) fn var_at(&self, offset: usize) -> Option<VarId> {
        self.span.id_at(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_bounds_owned_resolves_every_offset() {
        let uniform = BlockBoundsOwned::from_input(BlockBounds::Uniform(Bounds::new(0.0, 5.0)), 3);
        assert_eq!(uniform.get(0, 3), Some(Bounds::new(0.0, 5.0)));
        assert_eq!(uniform.get(2, 3), Some(Bounds::new(0.0, 5.0)));
        assert_eq!(uniform.get(3, 3), None);

        let per = [
            Bounds::new(0.0, 1.0),
            Bounds::new(1.0, 2.0),
            Bounds::new(2.0, 3.0),
        ];
        let owned = BlockBoundsOwned::from_input(BlockBounds::PerElement(&per), 3);
        assert_eq!(owned.get(1, 3), Some(Bounds::new(1.0, 2.0)));
        assert_eq!(owned.get(5, 3), None);
    }

    #[test]
    fn spans_reject_out_of_range_offsets() {
        let span = VarSpan::from_parts(10, 3, Generation::new());
        assert_eq!(span.id_at(0).map(|v| v.index()), Some(10));
        assert_eq!(span.id_at(2).map(|v| v.index()), Some(12));
        assert!(span.id_at(3).is_none());
        assert_eq!(span.len(), 3);

        let pspan = ParamSpan::from_parts(4, 2, Generation::new());
        assert_eq!(pspan.id_at(1).map(|p| p.index()), Some(5));
        assert!(pspan.id_at(2).is_none());
    }
}
