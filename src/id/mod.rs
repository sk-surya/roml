//! Typed identifiers for model entities.
//!
//! All entities in the model have stable, opaque IDs that are never reused.
//! Each ID consists of:
//! - A `u32` index for O(1) slot-based lookup
//! - A `Generation` counter for detecting stale IDs
//!
//! # Invariants
//!
//! - IDs are never reused, even after deletion
//! - Generation is bumped when a slot is freed (for stale detection)
//! - Indices are internal details; users should treat IDs as opaque

mod arena;

pub use arena::IdArena;

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

/// Macro for defining typed ID structs consistently.
///
/// Each ID type has:
/// - `index`: u32 for slot lookup
/// - `generation`: Generation for staleness detection
macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub struct $name {
            index: u32,
            generation: Generation,
        }

        impl $name {
            /// Create a new ID. Internal use only.
            #[doc(hidden)]
            pub const fn new(index: u32, generation: Generation) -> Self {
                Self { index, generation }
            }

            /// Get the internal index for storage lookup.
            ///
            /// # Warning
            ///
            /// This is NOT stable across serialization or sessions.
            /// Only use for internal slot-based storage access.
            #[inline]
            pub const fn index(self) -> u32 {
                self.index
            }

            /// Get the generation for staleness checking.
            #[inline]
            pub const fn generation(self) -> Generation {
                self.generation
            }
        }
    };
}

define_id!(
    /// Identifier for a decision variable.
    ///
    /// Variables can be continuous, integer, or binary, with bounds and an active flag.
    VarId
);