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
#[derive(Clone, Debug, Default)]
pub struct Journal {
    /// Batches indexed by their `from` revision, in order.
    batches: BTreeMap<ModelRevision, DeltaBatch>,

    /// The latest revision committed to this journal.
    latest_revision: ModelRevision,
}

/// Methods used by sync coordinator and tests.
#[allow(dead_code)]
impl Journal {
    /// Create an empty journal.
    pub fn new() -> Self {
        Self {
            batches: BTreeMap::new(),
            latest_revision: ModelRevision::ZERO,
        }
    }

    /// Record a committed delta batch.
    ///
    /// The batch's `from` revision must equal the journal's latest revision
    /// (no gaps). Returns an error if there's a gap.
    pub fn record(&mut self, batch: DeltaBatch) -> Result<(), RevisionError> {
        if batch.from != self.latest_revision {
            return Err(RevisionError::FutureRevision {
                requested: batch.from,
                current: self.latest_revision,
            });
        }

        self.latest_revision = batch.to;
        self.batches.insert(batch.from, batch);
        Ok(())
    }

    /// Return all batches with `from` revision >= `since`, in order.
    ///
    /// Returns an error if `since` references a compacted revision
    /// (not yet implemented — compaction is future work).
    pub fn deltas_since(&self, since: ModelRevision) -> Result<Vec<&DeltaBatch>, RevisionError> {
        if since > self.latest_revision {
            return Err(RevisionError::FutureRevision {
                requested: since,
                current: self.latest_revision,
            });
        }

        Ok(self.batches.range(since..).map(|(_, b)| b).collect())
    }

    /// The latest revision in the journal.
    pub fn latest_revision(&self) -> ModelRevision {
        self.latest_revision
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
}
