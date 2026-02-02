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