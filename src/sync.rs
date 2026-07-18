//! Adapter cursors and synchronization coordinator.
//!
//! Each solver adapter maintains an independent `AdapterCursor` that
//! tracks which revision it has applied. The cursor supports:
//! - catching up to later revisions via delta replay
//! - detecting when a rebuild is required
//! - recording terminal failures
//!
//! The synchronization coordinator provides model-owned entry points
//! for adapters without granting mutable access to model internals.

use crate::delta::DeltaBatch;
use crate::journal::Journal;
use crate::revision::{ModelRevision, RevisionError};

/// Health status of an adapter session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterHealth {
    /// Adapter is synchronized and ready to accept deltas.
    Ready,
    /// Adapter needs a full rebuild from snapshot.
    RequiresRebuild,
    /// Adapter session is terminally broken (recreate adapter).
    Terminal,
}

/// An independent cursor tracking one adapter's synchronization progress.
///
/// Each adapter has its own cursor. Multiple cursors can independently
/// advance through the journal without interfering with each other.
#[derive(Clone, Debug)]
pub struct AdapterCursor {
    /// The last revision successfully applied.
    pub applied_revision: ModelRevision,

    /// Current health of this adapter session.
    pub health: AdapterHealth,
}

/// Methods used by sync coordinator and adapters.
impl AdapterCursor {
    /// Create a new cursor at the initial revision.
    pub fn new() -> Self {
        Self {
            applied_revision: ModelRevision::ZERO,
            health: AdapterHealth::Ready,
        }
    }

    /// Advance the cursor after successfully applying a batch.
    ///
    /// The batch's `to` revision must equal the cursor's current
    /// revision + 1 (sequential advancement).
    pub fn advance(&mut self, batch: &DeltaBatch) -> Result<(), ApplyError> {
        if batch.from != self.applied_revision {
            return Err(ApplyError::RevisionMismatch {
                expected: self.applied_revision,
                got: batch.from,
            });
        }
        self.applied_revision = batch.to;
        self.health = AdapterHealth::Ready;
        Ok(())
    }

    /// Mark the cursor as needing a rebuild.
    pub fn mark_rebuild(&mut self) {
        self.health = AdapterHealth::RequiresRebuild;
    }

    /// Mark the cursor as terminally failed.
    pub fn mark_terminal(&mut self) {
        self.health = AdapterHealth::Terminal;
    }

    /// Mark the cursor as ready after a successful rebuild.
    pub fn mark_ready(&mut self, revision: ModelRevision) {
        self.applied_revision = revision;
        self.health = AdapterHealth::Ready;
    }

    /// True if the cursor is in a healthy state.
    pub fn is_ready(&self) -> bool {
        self.health == AdapterHealth::Ready
    }

    /// True if this adapter needs a rebuild.
    pub fn needs_rebuild(&self) -> bool {
        self.health == AdapterHealth::RequiresRebuild
    }
}

impl Default for AdapterCursor {
    fn default() -> Self {
        Self::new()
    }
}

/// Outcome of applying a delta batch to an adapter.
#[derive(Clone, Debug, PartialEq)]
pub enum ApplyOutcome {
    /// All operations applied successfully, cursor advanced.
    Applied {
        /// The revision after a successful application.
        new_revision: ModelRevision,
    },
    /// One or more operations are not incrementally supported.
    /// The adapter needs a full rebuild.
    RequiresRebuild {
        /// Index of the first operation that could not be applied.
        failed_at_op: usize,
        /// Human-readable explanation of why incremental apply failed.
        reason: String,
    },
    /// A recoverable failure occurred; adapter state is unchanged.
    RecoverableFailure {
        /// Description of the failure condition.
        reason: String,
    },
    /// A partial/dirty failure occurred; adapter must be rebuilt.
    DirtyFailure {
        /// Description of the dirty failure.
        reason: String,
    },
}

/// Error during apply or cursor advancement.
#[derive(Clone, Debug, PartialEq)]
pub enum ApplyError {
    /// The batch's `from` revision doesn't match the cursor.
    RevisionMismatch {
        expected: ModelRevision,
        got: ModelRevision,
    },
    /// The requested revision is not in the journal.
    RevisionNotFound(ModelRevision),
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RevisionMismatch { expected, got } => {
                write!(f, "revision mismatch: expected {expected}, got {got}")
            }
            Self::RevisionNotFound(rev) => write!(f, "revision not found in journal: {rev}"),
        }
    }
}

impl std::error::Error for ApplyError {}

/// Synchronization coordinator — bridges model journal to adapter cursors.
///
/// This is the model-owned side of the synchronization protocol.
/// Adapters interact with this coordinator rather than directly
/// accessing model internals.
#[derive(Clone, Debug, Default)]
pub struct SyncCoordinator {
    /// The journal of committed delta batches.
    pub journal: Journal,
    /// The current model revision.
    pub current_revision: ModelRevision,
}

/// Methods used by model and adapter sessions.
impl SyncCoordinator {
    /// Create a new coordinator.
    pub fn new() -> Self {
        Self {
            journal: Journal::new(),
            current_revision: ModelRevision::ZERO,
        }
    }

    /// Record a new delta batch and advance the model revision.
    pub fn commit_batch(&mut self, batch: DeltaBatch) -> Result<(), RevisionError> {
        self.journal.record(batch)?;
        self.current_revision = self.journal.latest_revision();
        Ok(())
    }

    /// Get the batches an adapter needs to catch up from its cursor.
    pub fn batches_for_cursor(
        &self,
        cursor: &AdapterCursor,
    ) -> Result<Vec<&DeltaBatch>, RevisionError> {
        self.journal.deltas_since(cursor.applied_revision)
    }

    /// Get the current model revision.
    pub fn revision(&self) -> ModelRevision {
        self.current_revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_batch(from: ModelRevision, to: ModelRevision) -> DeltaBatch {
        DeltaBatch::new(from, to, vec![]).unwrap()
    }

    #[test]
    fn cursor_advances_sequentially() {
        let mut cursor = AdapterCursor::new();
        let r0 = ModelRevision::ZERO;
        let r1 = r0.next().unwrap();

        let batch = make_batch(r0, r1);
        cursor.advance(&batch).unwrap();
        assert_eq!(cursor.applied_revision, r1);
        assert!(cursor.is_ready());
    }

    #[test]
    fn cursor_rejects_non_sequential() {
        let mut cursor = AdapterCursor::new();
        let r0 = ModelRevision::ZERO;
        let r1 = r0.next().unwrap();
        let r2 = r1.next().unwrap();

        // Try to apply batch r1→r2 on cursor at r0
        let batch = make_batch(r1, r2);
        assert!(cursor.advance(&batch).is_err());
    }

    #[test]
    fn cursor_rebuild_cycle() {
        let mut cursor = AdapterCursor::new();
        assert!(cursor.is_ready());

        cursor.mark_rebuild();
        assert!(cursor.needs_rebuild());
        assert!(!cursor.is_ready());

        cursor.mark_ready(ModelRevision::from_u64(5));
        assert!(cursor.is_ready());
        assert_eq!(cursor.applied_revision, ModelRevision::from_u64(5));
    }

    #[test]
    fn two_independent_cursors() {
        let mut coordinator = SyncCoordinator::new();
        let r0 = ModelRevision::ZERO;
        let r1 = r0.next().unwrap();
        let r2 = r1.next().unwrap();

        coordinator.commit_batch(make_batch(r0, r1)).unwrap();
        coordinator.commit_batch(make_batch(r1, r2)).unwrap();

        let mut cursor_a = AdapterCursor::new();
        let mut cursor_b = AdapterCursor::new();

        // Cursor A advances fully
        let batches_a = coordinator.batches_for_cursor(&cursor_a).unwrap();
        for batch in batches_a {
            cursor_a.advance(batch).unwrap();
        }
        assert_eq!(cursor_a.applied_revision, r2);

        // Cursor B is still at r0
        assert_eq!(cursor_b.applied_revision, r0);

        // Cursor B catches up
        let batches_b = coordinator.batches_for_cursor(&cursor_b).unwrap();
        for batch in batches_b {
            cursor_b.advance(batch).unwrap();
        }
        assert_eq!(cursor_b.applied_revision, r2);
    }

    #[test]
    fn coordinator_tracks_revision() {
        let mut coordinator = SyncCoordinator::new();
        assert_eq!(coordinator.revision(), ModelRevision::ZERO);

        let r0 = ModelRevision::ZERO;
        let r1 = r0.next().unwrap();

        coordinator.commit_batch(make_batch(r0, r1)).unwrap();
        assert_eq!(coordinator.revision(), r1);
    }
}
