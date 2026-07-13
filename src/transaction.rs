//! Atomic transaction system that produces `DeltaBatch` values from model operations.
//!
//! A `StagingTransaction` collects [`ModelOp`] values as the user mutates the model.
//! On [`commit`](StagingTransaction::commit), the operations are validated, formed
//! into a [`DeltaBatch`], and recorded in the [`SyncCoordinator`]'s journal. The
//! model revision advances atomically.
//!
//! # Design
//!
//! - Operations are staged in order and committed as a single batch.
//! - On error, the coordinator state (journal, revision) is unchanged.
//! - `rollback()` discards staged operations without side effects.
//!
//! # Example
//!
//! ```ignore
//! use roml::sync::SyncCoordinator;
//! use roml::transaction::{StagingTransaction, TransactionError};
//! use roml::delta::ModelOp;
//! use roml::revision::ModelRevision;
//!
//! let mut coordinator = SyncCoordinator::new();
//! let mut tx = StagingTransaction::new(coordinator.revision());
//!
//! // tx.stage(ModelOp::AddVariable { ... });
//!
//! let batch = tx.commit(&mut coordinator);
//! # Ok::<(), TransactionError>(())
//! ```

use crate::delta::{DeltaBatch, ModelOp};
use crate::revision::{ModelRevision, RevisionError};
use crate::sync::SyncCoordinator;

/// Errors that can occur during transaction operations.
///
/// # Variants
///
/// * `EmptyTransaction` — commit was called with no staged operations.
/// * `ValidationFailed` — one or more operations are invalid for the current
///   model state. The contained string describes the first problem.
/// * `RevisionOverflow` — the revision counter would overflow `u64::MAX`.
/// * `RevisionError` — an error from the revision/journal subsystem.
#[derive(Clone, Debug, PartialEq)]
pub enum TransactionError {
    /// The transaction has no staged operations.
    EmptyTransaction,
    /// Validation of staged operations failed.
    ValidationFailed(String),
    /// The revision counter would overflow.
    RevisionOverflow,
    /// An error from the revision system.
    RevisionError(RevisionError),
}

impl std::fmt::Display for TransactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTransaction => write!(f, "cannot commit an empty transaction"),
            Self::ValidationFailed(msg) => write!(f, "transaction validation failed: {msg}"),
            Self::RevisionOverflow => write!(f, "revision counter overflow"),
            Self::RevisionError(e) => write!(f, "revision error: {e}"),
        }
    }
}

impl std::error::Error for TransactionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RevisionError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<RevisionError> for TransactionError {
    fn from(e: RevisionError) -> Self {
        match e {
            RevisionError::Overflow => Self::RevisionOverflow,
            other => Self::RevisionError(other),
        }
    }
}

/// An atomic, staged transaction that collects [`ModelOp`] values.
///
/// Operations are collected in the order they are staged. On [`commit`](Self::commit),
/// they are validated, formed into a [`DeltaBatch`], and recorded in the
/// [`SyncCoordinator`]'s journal. If validation fails, nothing is committed and
/// the transaction can be discarded.
///
/// # Invariants
///
/// * The `from_revision` is set at creation and never changes.
/// * Operations are stored in insertion order.
/// * `rollback()` drops the operations without side effects (the same as
///   dropping the transaction).
#[derive(Clone, Debug)]
pub struct StagingTransaction {
    /// The revision at which this transaction was created.
    from_revision: ModelRevision,
    /// Staged model operations, in order.
    ops: Vec<ModelOp>,
}

impl StagingTransaction {
    /// Create a new staging transaction at the given revision.
    ///
    /// All committed operations will advance the model from `from_revision`
    /// to `from_revision.next()`.
    pub fn new(from: ModelRevision) -> Self {
        Self {
            from_revision: from,
            ops: Vec::new(),
        }
    }

    /// Add an operation to the transaction.
    ///
    /// Operations are stored in the order they are staged and will be
    /// committed in this order.
    pub fn stage(&mut self, op: ModelOp) {
        self.ops.push(op);
    }

    /// Stage multiple operations at once.
    pub fn stage_all(&mut self, ops: impl IntoIterator<Item = ModelOp>) {
        self.ops.extend(ops);
    }

    /// True if no operations have been staged.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Number of staged operations.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// The revision at which this transaction was created.
    pub fn from_revision(&self) -> ModelRevision {
        self.from_revision
    }

    /// Commit the staged operations, producing a [`DeltaBatch`].
    ///
    /// # Steps
    ///
    /// 1. Checks that the transaction is not empty.
    /// 2. Computes the next revision (checking for overflow).
    /// 3. Creates a [`DeltaBatch`] from the staged operations.
    /// 4. Records the batch in the coordinator's journal.
    ///
    /// On success, the coordinator's revision advances. On failure, the
    /// coordinator state is unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`TransactionError::EmptyTransaction`] if no operations have
    /// been staged. Returns [`TransactionError::RevisionOverflow`] if the
    /// revision counter is at maximum. Returns
    /// [`TransactionError::RevisionError`] if the journal rejects the batch.
    pub fn commit(self, coordinator: &mut SyncCoordinator) -> Result<DeltaBatch, TransactionError> {
        if self.ops.is_empty() {
            return Err(TransactionError::EmptyTransaction);
        }

        let to_revision = self
            .from_revision
            .next()
            .ok_or(TransactionError::RevisionOverflow)?;

        let batch = DeltaBatch::new(self.from_revision, to_revision, self.ops)
            .expect("from < to is guaranteed by next() check above");

        coordinator.commit_batch(batch.clone())?;

        Ok(batch)
    }

    /// Discard all staged operations without committing.
    ///
    /// This is a semantic no-op; dropping the transaction also discards all
    /// staged operations without side effects.
    pub fn rollback(self) {
        // `self` is dropped, which drops `self.ops` — all staged operations
        // are discarded without side effects.
    }

    /// Get a reference to the staged operations.
    pub fn operations(&self) -> &[ModelOp] {
        &self.ops
    }
}

/// A convenience wrapper that pairs a [`StagingTransaction`] with a
/// [`SyncCoordinator`].
///
/// This is useful when working with a single model that owns its own
/// coordinator. All operations delegate to the inner `StagingTransaction`.
///
/// # Example
///
/// ```ignore
/// use roml::sync::SyncCoordinator;
/// use roml::transaction::ModelTransaction;
/// use roml::delta::ModelOp;
///
/// let coordinator = SyncCoordinator::new();
/// let mut tx = ModelTransaction::new(coordinator);
///
/// // tx.stage(ModelOp::AddVariable { ... });
///
/// match tx.commit() {
///     Ok(batch) => { /* batch is in the journal */ }
///     Err(e) => { /* transaction discarded */ }
/// }
/// ```
#[derive(Clone, Debug)]
pub struct ModelTransaction {
    staging: StagingTransaction,
    coordinator: SyncCoordinator,
}

impl ModelTransaction {
    /// Create a new model transaction at the current coordinator revision.
    pub fn new(coordinator: SyncCoordinator) -> Self {
        let revision = coordinator.revision();
        Self {
            staging: StagingTransaction::new(revision),
            coordinator,
        }
    }

    /// Create a new model transaction at a specific revision.
    ///
    /// The coordinator's current revision is ignored; the transaction uses
    /// `from` as its base revision.
    pub fn new_at(from: ModelRevision, coordinator: SyncCoordinator) -> Self {
        Self {
            staging: StagingTransaction::new(from),
            coordinator,
        }
    }

    /// Stage an operation in the transaction.
    pub fn stage(&mut self, op: ModelOp) {
        self.staging.stage(op);
    }

    /// Stage multiple operations at once.
    pub fn stage_all(&mut self, ops: impl IntoIterator<Item = ModelOp>) {
        self.staging.stage_all(ops);
    }

    /// True if no operations have been staged.
    pub fn is_empty(&self) -> bool {
        self.staging.is_empty()
    }

    /// Number of staged operations.
    pub fn len(&self) -> usize {
        self.staging.len()
    }

    /// The revision at which this transaction was created.
    pub fn from_revision(&self) -> ModelRevision {
        self.staging.from_revision()
    }

    /// Commit the staged operations.
    ///
    /// Consumes the transaction. On success, returns the committed
    /// [`DeltaBatch`] and the coordinator's revision advances. On failure,
    /// the coordinator state is unchanged.
    pub fn commit(self) -> Result<DeltaBatch, TransactionError> {
        let Self {
            staging,
            mut coordinator,
        } = self;
        staging.commit(&mut coordinator)
    }

    /// Discard all staged operations without committing.
    pub fn rollback(self) {
        self.staging.rollback();
    }

    /// Get a reference to the staged operations.
    pub fn operations(&self) -> &[ModelOp] {
        self.staging.operations()
    }

    /// Get a reference to the coordinator.
    pub fn coordinator(&self) -> &SyncCoordinator {
        &self.coordinator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{ConId, Generation, ObjId, VarId};
    use crate::model::{Bounds, ConstraintBounds, Sense, VarType};

    // ── helpers ──────────────────────────────────────────────────────────

    fn var_id(index: u32) -> VarId {
        VarId::new(index, Generation::new())
    }

    fn con_id(index: u32) -> ConId {
        ConId::new(index, Generation::new())
    }

    fn obj_id(index: u32) -> ObjId {
        ObjId::new(index, Generation::new())
    }

    fn sample_add_var() -> ModelOp {
        ModelOp::AddVariable {
            var: var_id(0),
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }
    }

    fn sample_add_con() -> ModelOp {
        ModelOp::AddConstraint {
            con: con_id(0),
            bounds: ConstraintBounds::le(100.0),
        }
    }

    fn sample_add_obj() -> ModelOp {
        ModelOp::AddObjective {
            obj: obj_id(0),
            sense: Sense::Minimize,
        }
    }

    // ── StagingTransaction tests ─────────────────────────────────────────

    #[test]
    fn empty_transaction_is_rejected() {
        let mut coordinator = SyncCoordinator::new();
        let tx = StagingTransaction::new(ModelRevision::ZERO);
        let result = tx.commit(&mut coordinator);
        assert_eq!(result, Err(TransactionError::EmptyTransaction));
    }

    #[test]
    fn basic_commit_produces_batch() {
        let mut coordinator = SyncCoordinator::new();
        let mut tx = StagingTransaction::new(ModelRevision::ZERO);

        tx.stage(sample_add_var());
        tx.stage(sample_add_con());

        let batch = tx.commit(&mut coordinator).unwrap();
        assert_eq!(batch.from, ModelRevision::ZERO);
        assert_eq!(batch.to, ModelRevision::from_u64(1));
        assert_eq!(batch.len(), 2);
        assert_eq!(coordinator.revision(), ModelRevision::from_u64(1));
        assert_eq!(coordinator.journal.len(), 1);
    }

    #[test]
    fn commit_advances_revision_sequentially() {
        let mut coordinator = SyncCoordinator::new();

        // First commit: r0 → r1
        let mut tx1 = StagingTransaction::new(ModelRevision::ZERO);
        tx1.stage(sample_add_var());
        let batch1 = tx1.commit(&mut coordinator).unwrap();
        assert_eq!(batch1.to, ModelRevision::from_u64(1));

        // Second commit: r1 → r2
        let mut tx2 = StagingTransaction::new(coordinator.revision());
        tx2.stage(sample_add_con());
        let batch2 = tx2.commit(&mut coordinator).unwrap();
        assert_eq!(batch2.to, ModelRevision::from_u64(2));

        assert_eq!(coordinator.revision(), ModelRevision::from_u64(2));
        assert!(batch2.follows(&batch1));
    }

    #[test]
    fn rollback_discards_operations() {
        let mut coordinator = SyncCoordinator::new();
        let r0 = coordinator.revision();

        let mut tx = StagingTransaction::new(r0);
        tx.stage(sample_add_var());
        assert!(!tx.is_empty());
        assert_eq!(tx.len(), 1);

        tx.rollback(); // discard

        // Coordinator is unchanged
        assert_eq!(coordinator.revision(), r0);
        assert!(coordinator.journal.is_empty());

        // A new transaction at the same revision works independently
        let mut tx2 = StagingTransaction::new(r0);
        tx2.stage(sample_add_con());
        let batch = tx2.commit(&mut coordinator).unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(coordinator.revision(), ModelRevision::from_u64(1));
    }

    #[test]
    fn dropping_transaction_discards_ops() {
        let coordinator = SyncCoordinator::new();
        let r0 = coordinator.revision();

        {
            let mut tx = StagingTransaction::new(r0);
            tx.stage(sample_add_var());
            // tx drops here without commit
        }

        // No change to coordinator
        assert_eq!(coordinator.revision(), r0);
        assert!(coordinator.journal.is_empty());
    }

    #[test]
    fn multiple_operations_preserve_order() {
        let mut coordinator = SyncCoordinator::new();
        let mut tx = StagingTransaction::new(ModelRevision::ZERO);

        tx.stage(sample_add_var());
        tx.stage(sample_add_con());
        tx.stage(sample_add_obj());

        let batch = tx.commit(&mut coordinator).unwrap();
        assert_eq!(batch.len(), 3);

        // Check order is preserved
        assert_eq!(batch.operations.len(), 3);
        assert!(matches!(batch.operations[0], ModelOp::AddVariable { .. }));
        assert!(matches!(batch.operations[1], ModelOp::AddConstraint { .. }));
        assert!(matches!(batch.operations[2], ModelOp::AddObjective { .. }));
    }

    #[test]
    fn is_empty_and_len() {
        let mut tx = StagingTransaction::new(ModelRevision::ZERO);
        assert!(tx.is_empty());
        assert_eq!(tx.len(), 0);

        tx.stage(sample_add_var());
        assert!(!tx.is_empty());
        assert_eq!(tx.len(), 1);

        tx.stage(sample_add_con());
        assert_eq!(tx.len(), 2);
    }

    #[test]
    fn from_revision_tracking() {
        let r5 = ModelRevision::from_u64(5);
        let tx = StagingTransaction::new(r5);
        assert_eq!(tx.from_revision(), r5);
    }

    #[test]
    fn operations_reference() {
        let mut tx = StagingTransaction::new(ModelRevision::ZERO);
        tx.stage(sample_add_var());
        tx.stage(sample_add_con());

        let ops = tx.operations();
        assert_eq!(ops.len(), 2);
        assert!(matches!(ops[0], ModelOp::AddVariable { .. }));
    }

    #[test]
    fn stage_all_bulk_adds() {
        let mut tx = StagingTransaction::new(ModelRevision::ZERO);
        tx.stage_all(vec![sample_add_var(), sample_add_con(), sample_add_obj()]);
        assert_eq!(tx.len(), 3);
    }

    #[test]
    fn revision_overflow_error() {
        let mut coordinator = SyncCoordinator::new();
        let max_revision = ModelRevision::from_u64(u64::MAX);
        let mut tx = StagingTransaction::new(max_revision);
        tx.stage(sample_add_var());

        let result = tx.commit(&mut coordinator);
        assert_eq!(result, Err(TransactionError::RevisionOverflow));
    }

    // ── TransactionError tests ──────────────────────────────────────────

    #[test]
    fn error_display() {
        assert_eq!(
            format!("{}", TransactionError::EmptyTransaction),
            "cannot commit an empty transaction"
        );

        let val_err = TransactionError::ValidationFailed("bad op".to_string());
        assert!(format!("{}", val_err).contains("bad op"));

        assert_eq!(
            format!("{}", TransactionError::RevisionOverflow),
            "revision counter overflow"
        );

        let rev_err = TransactionError::RevisionError(RevisionError::Overflow);
        assert!(format!("{}", rev_err).contains("overflow"));
    }

    #[test]
    fn error_source() {
        use std::error::Error;
        let rev_err = TransactionError::RevisionError(RevisionError::Compacted {
            revision: ModelRevision::from_u64(0),
        });
        assert!(rev_err.source().is_some());

        assert!(TransactionError::EmptyTransaction.source().is_none());
        assert!(TransactionError::RevisionOverflow.source().is_none());
    }

    #[test]
    fn from_revision_error_overflow() {
        let err: TransactionError = RevisionError::Overflow.into();
        assert_eq!(err, TransactionError::RevisionOverflow);
    }

    #[test]
    fn from_revision_error_other() {
        let re = RevisionError::FutureRevision {
            requested: ModelRevision::from_u64(5),
            current: ModelRevision::ZERO,
        };
        let err: TransactionError = re.clone().into();
        assert_eq!(err, TransactionError::RevisionError(re));
    }

    // ── ModelTransaction tests ──────────────────────────────────────────

    #[test]
    fn model_transaction_commit() {
        let coordinator = SyncCoordinator::new();
        let mut tx = ModelTransaction::new(coordinator);

        tx.stage(sample_add_var());
        tx.stage(sample_add_con());

        let batch = tx.commit().unwrap();
        assert_eq!(batch.len(), 2);
        assert_eq!(batch.to, ModelRevision::from_u64(1));
        // coordinator is consumed inside commit, but we verify batch
    }

    #[test]
    fn model_transaction_empty_is_rejected() {
        let coordinator = SyncCoordinator::new();
        let tx = ModelTransaction::new(coordinator);
        let result = tx.commit();
        assert_eq!(result, Err(TransactionError::EmptyTransaction));
    }

    #[test]
    fn model_transaction_rollback() {
        let coordinator = SyncCoordinator::new();
        let mut tx = ModelTransaction::new(coordinator);
        tx.stage(sample_add_var());
        assert_eq!(tx.len(), 1);
        tx.rollback(); // no-op, staging dropped
                       // After rollback, tx is consumed — no further checks needed
    }

    #[test]
    fn model_transaction_new_at_specific_revision() {
        let r5 = ModelRevision::from_u64(5);
        let coordinator = SyncCoordinator::new();
        let tx = ModelTransaction::new_at(r5, coordinator);
        assert_eq!(tx.from_revision(), r5);
    }

    #[test]
    fn model_transaction_stage_all() {
        let coordinator = SyncCoordinator::new();
        let mut tx = ModelTransaction::new(coordinator);
        tx.stage_all(vec![sample_add_var(), sample_add_con()]);
        assert_eq!(tx.len(), 2);
    }

    #[test]
    fn model_transaction_is_empty_and_len() {
        let coordinator = SyncCoordinator::new();
        let mut tx = ModelTransaction::new(coordinator);
        assert!(tx.is_empty());
        assert_eq!(tx.len(), 0);

        tx.stage(sample_add_var());
        assert!(!tx.is_empty());
        assert_eq!(tx.len(), 1);
    }

    #[test]
    fn model_transaction_operations_and_coordinator() {
        let coordinator = SyncCoordinator::new();
        let mut tx = ModelTransaction::new(coordinator);
        tx.stage(sample_add_var());

        let ops = tx.operations();
        assert_eq!(ops.len(), 1);

        let coord = tx.coordinator();
        assert_eq!(coord.revision(), ModelRevision::ZERO);
    }

    #[test]
    fn model_transaction_clone_and_debug() {
        let coordinator = SyncCoordinator::new();
        let mut tx = ModelTransaction::new(coordinator);
        tx.stage(sample_add_var());

        let tx2 = tx.clone();
        assert_eq!(tx2.len(), 1);

        let debug_str = format!("{:?}", tx);
        assert!(!debug_str.is_empty());
    }

    // ── Integration tests ───────────────────────────────────────────────

    #[test]
    fn sequential_transactions_with_coordinator() {
        let mut coordinator = SyncCoordinator::new();

        // Transaction 1: add variable and constraint
        let mut tx1 = StagingTransaction::new(coordinator.revision());
        tx1.stage(sample_add_var());
        tx1.stage(sample_add_con());
        let batch1 = tx1.commit(&mut coordinator).unwrap();
        assert_eq!(batch1.to, ModelRevision::from_u64(1));

        // Transaction 2: add objective
        let mut tx2 = StagingTransaction::new(coordinator.revision());
        tx2.stage(sample_add_obj());
        let batch2 = tx2.commit(&mut coordinator).unwrap();
        assert_eq!(batch2.to, ModelRevision::from_u64(2));

        // Coordinator journal has both batches
        assert_eq!(coordinator.journal.len(), 2);

        // Batches are sequential
        assert!(batch2.follows(&batch1));

        // Adapter cursor can replay all
        let mut cursor = crate::sync::AdapterCursor::new();
        let batches = coordinator.batches_for_cursor(&cursor).unwrap();
        assert_eq!(batches.len(), 2);
        for b in &batches {
            cursor.advance(b).unwrap();
        }
        assert_eq!(cursor.applied_revision, ModelRevision::from_u64(2));
    }

    #[test]
    fn rollback_preserves_coordinator_state() {
        let mut coordinator = SyncCoordinator::new();

        // Commit one batch
        let mut tx1 = StagingTransaction::new(coordinator.revision());
        tx1.stage(sample_add_var());
        tx1.commit(&mut coordinator).unwrap();
        assert_eq!(coordinator.revision(), ModelRevision::from_u64(1));

        // Rollback another
        let mut tx2 = StagingTransaction::new(coordinator.revision());
        tx2.stage(sample_add_con());
        let ops = tx2.operations().to_vec();
        assert_eq!(ops.len(), 1);
        tx2.rollback();

        // Coordinator state is still at r1
        assert_eq!(coordinator.revision(), ModelRevision::from_u64(1));
        assert_eq!(coordinator.journal.len(), 1);
    }

    #[test]
    fn transaction_staged_at_wrong_revision_is_rejected() {
        let mut coordinator = SyncCoordinator::new();

        // Commit first batch: r0 → r1
        let mut tx1 = StagingTransaction::new(coordinator.revision());
        tx1.stage(sample_add_var());
        tx1.commit(&mut coordinator).unwrap();

        // Create another transaction claiming to be at r0 (wrong — coordinator is at r1)
        let mut tx2 = StagingTransaction::new(ModelRevision::ZERO);
        tx2.stage(sample_add_con());

        // This should fail because the journal expects the batch at r1, not r0
        let result = tx2.commit(&mut coordinator);
        assert!(result.is_err());
        // Coordinator state is unchanged
        assert_eq!(coordinator.revision(), ModelRevision::from_u64(1));
    }

    #[test]
    fn transaction_revision_must_match_coordinator() {
        let mut coordinator = SyncCoordinator::new();

        // Commit: r0 → r1
        let mut tx1 = StagingTransaction::new(ModelRevision::ZERO);
        tx1.stage(sample_add_var());
        tx1.commit(&mut coordinator).unwrap();

        // Try to commit another batch from r0 (should fail — journal expects r1)
        let mut tx2 = StagingTransaction::new(ModelRevision::ZERO);
        tx2.stage(sample_add_con());
        match tx2.commit(&mut coordinator) {
            Err(TransactionError::RevisionError(RevisionError::FutureRevision {
                requested,
                current,
            })) => {
                assert_eq!(requested, ModelRevision::ZERO);
                assert_eq!(current, ModelRevision::from_u64(1));
            }
            other => panic!("expected FutureRevision error, got {other:?}"),
        }

        // Coordinator is unchanged
        assert_eq!(coordinator.revision(), ModelRevision::from_u64(1));
    }
}
