//! Monotonic model revisions.
//!
//! Each committed model mutation advances the revision counter.
//! Revisions are opaque, ordered, and never reused. Overflow
//! produces a defined error rather than wrapping silently.

/// A monotonically-increasing model revision number.
///
/// Each committed change (single operation or batched transaction)
/// increments the revision. Revisions are used by the journal for
/// delta replay and by adapter cursors for tracking applied state.
///
/// Revisions implement `Ord` — a later revision (higher value)
/// represents a newer state than an earlier revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModelRevision(u64);

impl ModelRevision {
    /// The initial revision (zero, before any mutations).
    pub const ZERO: Self = ModelRevision(0);

    /// Return the next revision after this one.
    ///
    /// Returns `None` if the counter would overflow `u64::MAX`.
    pub fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(ModelRevision)
    }

    /// Return the raw revision number.
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// True if `self` is strictly before `other`.
    pub fn is_before(self, other: ModelRevision) -> bool {
        self < other
    }

    /// True if the revision is the initial (zero) revision.
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Display for ModelRevision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "r{}", self.0)
    }
}

impl Default for ModelRevision {
    fn default() -> Self {
        Self::ZERO
    }
}

/// Error returned when a revision operation fails.
#[derive(Clone, Debug, PartialEq)]
pub enum RevisionError {
    /// The revision counter would overflow.
    Overflow,
    /// The requested revision has been compacted (no longer in journal).
    Compacted { revision: ModelRevision },
    /// The requested revision is in the future.
    FutureRevision {
        requested: ModelRevision,
        current: ModelRevision,
    },
}

impl std::fmt::Display for RevisionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Overflow => write!(f, "revision counter overflow"),
            Self::Compacted { revision } => write!(f, "revision {revision} has been compacted"),
            Self::FutureRevision { requested, current } => {
                write!(
                    f,
                    "revision {requested} is in the future (current: {current})"
                )
            }
        }
    }
}

impl std::error::Error for RevisionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_initial() {
        assert!(ModelRevision::ZERO.is_zero());
        assert_eq!(ModelRevision::ZERO.as_u64(), 0);
    }

    #[test]
    fn next_increments() {
        let r0 = ModelRevision::ZERO;
        let r1 = r0.next().unwrap();
        assert_eq!(r1.as_u64(), 1);
        assert!(r0 < r1);
        assert!(r0.is_before(r1));
    }

    #[test]
    fn ordering() {
        let r0 = ModelRevision::ZERO;
        let r1 = r0.next().unwrap();
        let r2 = r1.next().unwrap();
        let r3 = r2.next().unwrap();

        assert!(r0 < r1);
        assert!(r1 < r2);
        assert!(r0 < r3);
        assert!(!r2.is_before(r1));
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", ModelRevision::ZERO), "r0");
        assert_eq!(format!("{}", ModelRevision(42)), "r42");
    }

    #[test]
    fn overflow_returns_none() {
        let r = ModelRevision(u64::MAX);
        assert!(r.next().is_none());
    }
}
