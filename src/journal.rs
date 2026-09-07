//! Revision journal for delta replay.
//!
//! The journal stores committed `DeltaBatch` values indexed by their
//! `from` revision. Each batch is immutable once stored and retained
//! until explicit compaction.
//!
//! # Design
//!
//! - Batches are stored in revision order (by `from` revision).
//! - `deltas_since(revision)` returns all batches whose `from` revision
//!   is >= the requested revision, in order.
//! - The journal does not compact automatically; callers control retention.

use std::collections::BTreeMap;

use crate::delta::DeltaBatch;
use crate::revision::{ModelRevision, RevisionError};

/// A journal of committed delta batches, ordered by revision.
///
/// Each batch is stored at its `from` revision. The journal supports
/// replay queries for adapters that need to catch up.
///
/// # Retention bound
///
/// The journal retains at most `capacity` most recent batches
/// (`DEFAULT_JOURNAL_CAPACITY` unless overridden). Older batches are
/// evicted on record; a replay query for an evicted range fails with
/// `RevisionError::Compacted`, and the caller rebuilds from a snapshot
/// (the facade's existing recovery path). The bound means idle or
/// abandoned sessions can never pin history: retention is global, never
/// per-session. INVARIANT: `batches.len() <= capacity` whenever capacity
/// is `Some` — direct field inserts bypassing `record` violate this.
#[derive(Clone, Debug)]
pub struct Journal {
    /// Batches indexed by their `from` revision, in order.
    batches: BTreeMap<ModelRevision, DeltaBatch>,

    /// The latest revision committed to this journal.
    latest_revision: ModelRevision,

    /// Maximum retained batches (`None` = unbounded, test-only).
    capacity: Option<std::num::NonZeroUsize>,
}

/// Default retention: enough headroom for typical multi-session lag
/// (single digits) with bounded memory on uniform workloads.
pub const DEFAULT_JOURNAL_CAPACITY: usize = 128;

impl Default for Journal {
    /// Default is a bounded empty journal (same as `new`).
    fn default() -> Self {
        Self::new()
    }
}

/// Methods used by sync coordinator and tests.
impl Journal {
    /// Create an empty journal with the default retention bound.
    pub fn new() -> Self {
        Self {
            batches: BTreeMap::new(),
            latest_revision: ModelRevision::ZERO,
            capacity: std::num::NonZeroUsize::new(DEFAULT_JOURNAL_CAPACITY),
        }
    }

    /// Create an empty journal with an explicit retention bound (`None`
    /// = unbounded; intended for tests pinning legacy semantics).
    pub fn with_capacity(capacity: Option<std::num::NonZeroUsize>) -> Self {
        Self {
            batches: BTreeMap::new(),
            latest_revision: ModelRevision::ZERO,
            capacity,
        }
    }

    /// Record a committed delta batch.
    ///
    /// The batch's `from` revision must equal the journal's latest revision
    /// (no gaps). Returns an error if there's a gap. After inserting,
    /// oldest batches beyond capacity are evicted.
    pub fn record(&mut self, batch: DeltaBatch) -> Result<(), RevisionError> {
        if batch.from != self.latest_revision {
            return Err(RevisionError::FutureRevision {
                requested: batch.from,
                current: self.latest_revision,
            });
        }

        self.latest_revision = batch.to;
        self.batches.insert(batch.from, batch);
        if let Some(cap) = self.capacity {
            while self.batches.len() > cap.get() {
                // BTreeMap iterates in key order: pop the oldest first.
                let oldest = *self.batches.keys().next().expect("nonempty after insert");
                self.batches.remove(&oldest);
            }
        }
        Ok(())
    }

    /// Return all batches with `from` revision >= `since`, in order.
    ///
    /// Boundary contract: `since` older than the oldest retained batch
    /// fails with `RevisionError::Compacted` (the caller rebuilds from a
    /// snapshot); `since` within `[oldest_retained, latest]` returns the
    /// exact suffix; `since == latest` (or an empty journal queried at
    /// `ZERO`) returns `Ok(empty)` meaning up-to-date.
    pub fn deltas_since(&self, since: ModelRevision) -> Result<Vec<&DeltaBatch>, RevisionError> {
        if since > self.latest_revision {
            return Err(RevisionError::FutureRevision {
                requested: since,
                current: self.latest_revision,
            });
        }
        // Evicted prefix: the retained suffix would silently miss history.
        if let Some(&oldest) = self.batches.keys().next() {
            if since < oldest {
                return Err(RevisionError::Compacted { revision: since });
            }
        }

        Ok(self.batches.range(since..).map(|(_, b)| b).collect())
    }

    /// The latest revision in the journal.
    pub fn latest_revision(&self) -> ModelRevision {
        self.latest_revision
    }

    /// The oldest retained batch revision, if any. Eviction advances this;
    /// replay queries below it fail with `RevisionError::Compacted`.
    pub fn oldest_retained(&self) -> Option<ModelRevision> {
        self.batches.keys().next().copied()
    }

    /// Number of batches in the journal.
    pub fn len(&self) -> usize {
        self.batches.len()
    }

    /// True if the journal is empty.
    pub fn is_empty(&self) -> bool {
        self.batches.is_empty()
    }

    /// Get a specific batch by its `from` revision.
    pub fn get(&self, from: ModelRevision) -> Option<&DeltaBatch> {
        self.batches.get(&from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_batch(from: ModelRevision, to: ModelRevision) -> DeltaBatch {
        DeltaBatch::new(from, to, vec![]).unwrap()
    }

    #[test]
    fn record_sequential_batches() {
        let mut journal = Journal::new();
        let r0 = ModelRevision::ZERO;
        let r1 = r0.next().unwrap();
        let r2 = r1.next().unwrap();

        assert!(journal.record(make_batch(r0, r1)).is_ok());
        assert!(journal.record(make_batch(r1, r2)).is_ok());

        assert_eq!(journal.latest_revision(), r2);
        assert_eq!(journal.len(), 2);
    }

    #[test]
    fn record_rejects_gap() {
        let mut journal = Journal::new();
        let r0 = ModelRevision::ZERO;
        let r1 = r0.next().unwrap();
        let r2 = r1.next().unwrap();
        let r3 = r2.next().unwrap();

        // Record r0→r1, then try r2→r3 (gap at r1)
        journal.record(make_batch(r0, r1)).unwrap();
        assert!(journal.record(make_batch(r2, r3)).is_err());
    }

    #[test]
    fn deltas_since_returns_correct_range() {
        let mut journal = Journal::new();
        let r0 = ModelRevision::ZERO;
        let r1 = r0.next().unwrap();
        let r2 = r1.next().unwrap();
        let r3 = r2.next().unwrap();

        journal.record(make_batch(r0, r1)).unwrap();
        journal.record(make_batch(r1, r2)).unwrap();
        journal.record(make_batch(r2, r3)).unwrap();

        // Request from r1: should get batches r1→r2 and r2→r3
        let batches = journal.deltas_since(r1).unwrap();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].from, r1);
        assert_eq!(batches[1].from, r2);
    }

    #[test]
    fn deltas_since_future_revision_is_error() {
        let journal = Journal::new();
        let r_future = ModelRevision::from_u64(42);
        assert!(journal.deltas_since(r_future).is_err());
    }

    #[test]
    fn empty_journal() {
        let journal = Journal::new();
        assert!(journal.is_empty());
        assert_eq!(journal.latest_revision(), ModelRevision::ZERO);
        assert!(journal
            .deltas_since(ModelRevision::ZERO)
            .unwrap()
            .is_empty());
    }

    fn rev(n: u64) -> ModelRevision {
        ModelRevision::from_u64(n)
    }

    fn fill(journal: &mut Journal, count: u64) {
        for i in 0..count {
            journal
                .record(make_batch(rev(i), rev(i + 1)))
                .expect("sequential record");
        }
    }

    #[test]
    fn record_beyond_capacity_evicts_oldest() {
        use std::num::NonZeroUsize;
        let cap = NonZeroUsize::new(4).unwrap();
        let mut journal = Journal::with_capacity(Some(cap));
        fill(&mut journal, 6);
        assert_eq!(journal.len(), 4);
        assert_eq!(journal.oldest_retained(), Some(rev(2)));
        assert_eq!(journal.latest_revision(), rev(6));
        // Retained window serves exact suffixes.
        let suffix = journal.deltas_since(rev(4)).unwrap();
        assert_eq!(suffix.len(), 2);
        assert_eq!(journal.deltas_since(rev(6)).unwrap().len(), 0);
    }

    #[test]
    fn deltas_since_below_window_is_compacted_not_truncated() {
        use std::num::NonZeroUsize;
        let cap = NonZeroUsize::new(4).unwrap();
        let mut journal = Journal::with_capacity(Some(cap));
        fill(&mut journal, 6);
        // A stale cursor must FAIL, never receive a truncated suffix as Ok.
        match journal.deltas_since(rev(0)) {
            Err(RevisionError::Compacted { revision }) => assert_eq!(revision, rev(0)),
            other => panic!("expected Compacted, got {other:?}"),
        }
        match journal.deltas_since(rev(1)) {
            Err(RevisionError::Compacted { revision }) => assert_eq!(revision, rev(1)),
            other => panic!("expected Compacted, got {other:?}"),
        }
    }

    #[test]
    fn unbounded_capacity_preserves_legacy_semantics() {
        let mut journal = Journal::with_capacity(None);
        fill(&mut journal, 300);
        assert_eq!(journal.len(), 300);
        assert_eq!(journal.oldest_retained(), Some(rev(0)));
        assert_eq!(journal.deltas_since(rev(0)).unwrap().len(), 300);
    }

    #[test]
    fn default_capacity_is_bounded() {
        use std::num::NonZeroUsize;
        let mut journal = Journal::new();
        assert_eq!(
            journal.capacity,
            NonZeroUsize::new(DEFAULT_JOURNAL_CAPACITY)
        );
        fill(&mut journal, DEFAULT_JOURNAL_CAPACITY as u64 + 10);
        assert_eq!(journal.len(), DEFAULT_JOURNAL_CAPACITY);
    }
}
