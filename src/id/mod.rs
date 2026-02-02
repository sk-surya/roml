mod arena;


/// Generation counter for detecting stale IDs.
/// 
/// When an entity is deleted, its slot's generation is incremented.
/// Any ID with a mismatched generation is considered stale.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Generation(u32);

impl Generation {
    /// Create a new generation starting at 0.
    pub const fn new() -> Self {
        Self(0)
    }

    /// Increment the generation (called on deletion).
    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    /// Get the raw generation value.
    pub fn value(self) -> u32 {
        self.0
    }
}