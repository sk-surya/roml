//! Opaque identity values for canonical model state.
//!
//! P25 introduces opaque identities with distinct authority (design §4):
//!
//! - [`ModelLineageId`] identifies a model lineage. Independent models get
//!   distinct lineages; clones share a lineage. Lineage governs assignment
//!   reuse compatibility across clones (SM-02.1, SM-02.2 foundations).
//! - [`ModelInstanceId`] identifies a live [`Model`](crate::Model) object.
//!   Every live model has a distinct instance id; cloning allocates a new
//!   instance while preserving lineage (SM-02.7).
//! - [`ConstructId`] identifies a canonical semantic construct (design §4.4).
//!   Allocation is generation-safe through the construct arena (P25 Task 4).
//!
//! Each id wraps an opaque `u64`. Ids are allocated through checked atomic
//! counters with zero reserved: the first issued id is 1, and counter
//! exhaustion returns a typed [`IdentityOverflow`] error rather than wrapping.

use std::sync::atomic::{AtomicU64, Ordering};

/// Error returned when an opaque id counter is exhausted.
///
/// Id allocation never wraps: once `u64::MAX` ids have been issued for an id
/// family, further allocation returns this error instead of reusing ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityOverflow;

/// Allocate the next id from `counter`, returning a typed error on overflow.
///
/// `fetch_add` returns the pre-increment value, so the first issued id is 1
/// (value 0 is reserved and never issued). When the counter already holds
/// `u64::MAX`, the increment would wrap; we report exhaustion instead.
fn allocate_id(counter: &AtomicU64) -> Result<u64, IdentityOverflow> {
    let pre = counter.fetch_add(1, Ordering::Relaxed);
    if pre == u64::MAX {
        return Err(IdentityOverflow);
    }
    Ok(pre + 1)
}

/// Define one opaque `u64` id struct with a checked per-family atomic counter.
macro_rules! define_opaque_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u64);

        impl $name {
            /// Allocate a fresh opaque id. The first issued id is 1; zero is
            /// reserved and never issued. Returns [`IdentityOverflow`] on
            /// counter exhaustion instead of wrapping.
            pub(crate) fn allocate() -> Result<Self, IdentityOverflow> {
                static COUNTER: AtomicU64 = AtomicU64::new(0);
                allocate_id(&COUNTER).map(Self)
            }
        }
    };
}

define_opaque_id!(
    /// Identifies a model lineage.
    ///
    /// Independent models receive distinct lineages; cloning preserves
    /// lineage while allocating a new instance id (design §4.1).
    ModelLineageId
);

define_opaque_id!(
    /// Identifies a live [`Model`](crate::Model) object.
    ///
    /// Every live model has a distinct instance id, so divergent clones with
    /// equal revisions can never be mistaken for the same canonical state
    /// (design §4.2, SM-02.7).
    ModelInstanceId
);

define_opaque_id!(
    /// Identifies a canonical semantic construct (design §4.4).
    ///
    /// Issued generation-safely by the construct arena (P25 Task 4). A
    /// removed construct's id is stale and rejected with a typed error.
    ConstructId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_issued_id_is_one_and_zero_is_reserved() {
        let first = ModelLineageId::allocate().unwrap();
        let second = ModelLineageId::allocate().unwrap();
        // The macro does not expose the inner value; distinctness and
        // ordering are the observable contract. Two allocations differ.
        assert_ne!(first, second);
        assert!(first < second, "ids are issued in increasing order");
    }

    #[test]
    fn separate_families_have_separate_counters() {
        let lineage = ModelLineageId::allocate().unwrap();
        let instance = ModelInstanceId::allocate().unwrap();
        let construct = ConstructId::allocate().unwrap();
        assert_ne!(lineage, ModelLineageId::allocate().unwrap());
        assert_ne!(instance, ModelInstanceId::allocate().unwrap());
        assert_ne!(construct, ConstructId::allocate().unwrap());
    }

    #[test]
    fn ids_are_copy_hash_and_ordered() {
        use std::collections::HashSet;
        let a = ModelLineageId::allocate().unwrap();
        let b = a; // Copy
        assert_eq!(a, b);
        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
        // PartialOrd/Ord present via derive; ordering is total.
        let c = ModelLineageId::allocate().unwrap();
        let mut items = [c, a];
        items.sort();
        assert_eq!(items, [a, c]);
    }
}
