//! Arena allocator for ID generation.
//!
//! Provides monotonic ID allocation with generation tracking for staleness detection.
//! IDs are never reused - when an entity is deleted, its slot's generation is bumped.

use super::Generation;

/// Slot state in the arena.
#[derive(Clone, Debug)]
pub struct Slot<T> {
    /// The data stored in this slot, if occupied.
    pub data: Option<T>,
    /// Current generation of this slot.
    pub generation: Generation,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self {
            data: None,
            generation: Generation::new(),
        }
    }
}

/// Arena allocator for managing entities with stable typed IDs.
///
/// # Design
///
/// - Uses monotonic allocation (always allocates from the end)
/// - Never reuses indices (deleted slots stay with bumped generation)
/// - O(1) allocation and lookup
/// - Generation tracking for stale ID detection

#[derive(Clone, Debug)]
pub struct IdArena<T> {
    slots: Vec<Slot<T>>,
    /// Count of occupied slots (for len())
    count: usize,
}

impl<T> Default for IdArena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> IdArena<T> {
    /// Create a new empty arena.
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            count: 0,
        }
    }

    /// Create an arena with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            count: 0,
        }
    }
}