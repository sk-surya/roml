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

    /// Allocate a new slot and return its index and generation.
    ///
    /// Always allocates from the end (monotonic). Never reuses indices.
    pub fn allocate(&mut self, data: T) -> (u32, Generation) {
        let index = self.slots.len() as u32;
        let generation = Generation::new();
        self.slots.push(Slot {
            data: Some(data),
            generation,
        });
        self.count += 1;
        (index, generation)
    }

    /// Remove an entity by index and generation.
    ///
    /// Returns the data if the ID was valid, None if stale or out of bounds.
    /// Bumps the slot's generation to invalidate any remaining references.
    pub fn remove(&mut self, index: u32, generation: Generation) -> Option<T> {
        let slot = self.slots.get_mut(index as usize)?;
        if slot.generation != generation || slot.data.is_none() {
            return None;
        }
        slot.generation = slot.generation.next();
        self.count -= 1;
        slot.data.take()
    }

    /// Get a reference to the data at the given index and generation.
    ///
    /// Returns None if the ID is stale or out of bounds.
    pub fn get(&self, index: u32, generation: Generation) -> Option<&T> {
        let slot = self.slots.get(index as usize)?;
        if slot.generation != generation {
            return None;
        }
        slot.data.as_ref()
    }

    /// Get a mutable reference to the data at the given index and generation.
    ///
    /// Returns None if the ID is stale or out of bounds.
    pub fn get_mut(&mut self, index: u32, generation: Generation) -> Option<&mut T> {
        let slot = self.slots.get_mut(index as usize)?;
        if slot.generation != generation {
            return None;
        }
        slot.data.as_mut()
    }

    /// Check if an ID is valid (exists and generation matches).
    pub fn contains(&self, index: u32, generation: Generation) -> bool {
        self.get(index, generation).is_some()
    }

    /// Get the number of occupied slots.
    #[inline]
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if the arena is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get the total number of slots (including deleted).
    #[inline]
    pub fn capacity_used(&self) -> usize {
        self.slots.len()
    }
}