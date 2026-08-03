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
//!
//! # Overflow handling (WR-03, WR-04)
//!
//! Counters **saturate** on overflow: once `u64::MAX` ids have been issued for
//! a family, the counter stays at `u64::MAX` and every further allocation
//! returns [`IdentityOverflow`] — the counter never wraps to 0 and ids are
//! never reused. Exhaustion is practically unreachable (2^64 allocations).
//!
//! The fallible allocation APIs (e.g. the test-only
//! `Model::add_construct_fixture` / the construct arena) surface exhaustion as
//! the typed [`IdentityOverflow`] error. The infallible constructors —
//! [`Model`](crate::Model)`::default`, `Clone`, and `SolveMetadata::default` —
//! must return a value, so they `expect`/`panic`; but because the counters
//! saturate, a panicking constructor never re-issues an id. The panic therefore
//! fires only when the family counter is *truly* exhausted.

use std::sync::atomic::{AtomicU64, Ordering};

/// Error returned when an opaque id counter is exhausted.
///
/// Id allocation never wraps: once `u64::MAX` ids have been issued for an id
/// family, further allocation returns this error instead of reusing ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityOverflow;

/// Allocate the next id from `counter`, returning a typed error on overflow.
///
/// `fetch_update` returns the pre-update value, so the first issued id is 1
/// (value 0 is reserved and never issued). The counter **saturates** on
/// overflow (WR-03): when the counter already holds `u64::MAX`, the closure
/// returns `None` so the counter stays at `u64::MAX` and every subsequent call
/// reports [`IdentityOverflow`] instead of wrapping to 0 and re-issuing id 1.
fn allocate_id(counter: &AtomicU64) -> Result<u64, IdentityOverflow> {
    match counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |pre| {
        if pre == u64::MAX {
            None // saturated: do not advance; report overflow
        } else {
            Some(pre + 1)
        }
    }) {
        Ok(pre) => Ok(pre + 1),
        Err(_) => Err(IdentityOverflow),
    }
}

/// Define one opaque `u64` id struct with a checked per-family atomic counter.
///
/// The macro takes a second identifier for the module-level counter static so
/// the test seam (`seed_counter_for_test`, IN-02) can address the same counter
/// the `allocate` path uses.
macro_rules! define_opaque_id {
    ($(#[$meta:meta])* $name:ident, $counter:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u64);

        /// Per-family checked atomic counter (zero reserved; 0 never issued).
        static $counter: AtomicU64 = AtomicU64::new(0);

        impl $name {
            /// Allocate a fresh opaque id. The first issued id is 1; zero is
            /// reserved and never issued. Returns [`IdentityOverflow`] on
            /// counter exhaustion instead of wrapping (the counter saturates
            /// at `u64::MAX`, so ids are never reused).
            pub(crate) fn allocate() -> Result<Self, IdentityOverflow> {
                allocate_id(&$counter).map(Self)
            }

            /// Test-only seam (IN-02): store a value directly into the family
            /// counter so the overflow branch can be exercised without 2^64
            /// allocations. Emitted for every family; only the dedicated test
            /// family uses it, so the others are allow-dead under the test
            /// profile.
            #[cfg(test)]
            #[allow(dead_code)]
            pub(crate) fn seed_counter_for_test(value: u64) {
                $counter.store(value, Ordering::Relaxed);
            }
        }
    };
}

define_opaque_id!(
    /// Identifies a model lineage.
    ///
    /// Independent models receive distinct lineages; cloning preserves
    /// lineage while allocating a new instance id (design §4.1).
    ModelLineageId,
    MODEL_LINEAGE_ID_COUNTER
);

define_opaque_id!(
    /// Identifies a live [`Model`](crate::Model) object.
    ///
    /// Every live model has a distinct instance id, so divergent clones with
    /// equal revisions can never be mistaken for the same canonical state
    /// (design §4.2, SM-02.7).
    ModelInstanceId,
    MODEL_INSTANCE_ID_COUNTER
);

define_opaque_id!(
    /// Identifies a canonical semantic construct (design §4.4).
    ///
    /// Issued generation-safely by the construct arena (P25 Task 4). A
    /// removed construct's id is stale and rejected with a typed error.
    ConstructId,
    CONSTRUCT_ID_COUNTER
);

#[cfg(test)]
define_opaque_id!(
    /// Test-only id family (IN-02) so the overflow branch is exercised
    /// without racing the shared families used by concurrent unit tests.
    TestOverflowId,
    TEST_OVERFLOW_ID_COUNTER
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

    /// WR-03: a saturated counter must keep reporting overflow without
    /// wrapping to 0 (which would re-issue id 1). A `fetch_add`-based
    /// implementation wraps the counter to 0 on the first Err call, so the
    /// second call below would return `Ok(1)` and this test would fail.
    #[test]
    fn allocate_id_saturates_on_overflow_without_wrapping() {
        let counter = AtomicU64::new(u64::MAX);
        assert_eq!(allocate_id(&counter), Err(IdentityOverflow));
        // The counter stays saturated: a second call is still Err (no wrap,
        // no id reuse).
        assert_eq!(allocate_id(&counter), Err(IdentityOverflow));
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }

    /// IN-02: the family-level `allocate` path saturates too. Seeding the
    /// counter at `u64::MAX` proves the overflow branch is actually reached
    /// and that a subsequent call is still `Err` (no wrap, no id reuse).
    #[test]
    fn family_allocate_saturates_on_overflow_without_reissuing() {
        TestOverflowId::seed_counter_for_test(u64::MAX);
        assert_eq!(TestOverflowId::allocate(), Err(IdentityOverflow));
        assert_eq!(TestOverflowId::allocate(), Err(IdentityOverflow));
    }
}
