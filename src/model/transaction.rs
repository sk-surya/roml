//! Transaction system for batched parameter updates.
//!
//! Parameters can be modified in a transaction to batch changes together.
//! When committed, all affected coefficients are updated atomically.
//!
//! # Design
//!
//! - `set_param()` queues changes in the transaction
//! - `commit()` applies all changes and propagates to coefficients
//! - At solver sync, uncommitted changes trigger a warning and auto-commit

use std::collections::HashMap;

use crate::bulk::ParamSpan;
use crate::id::ParamId;

/// A transaction for batched parameter updates.
///
/// Collects parameter changes and applies them atomically on commit.
#[derive(Clone, Debug, Default)]
pub(crate) struct Transaction {
    /// Pending scalar parameter changes: ParamId -> new value.
    pending: HashMap<ParamId, f64>,
    /// Pending block updates (MIR-02): span plus one value per member. A block
    /// request is retained as a block, not expanded into `n` scalar map
    /// insertions.
    pending_blocks: Vec<(ParamSpan, Vec<f64>)>,
}

/// Methods used by Model.
/// Methods used by Model.
#[allow(dead_code)]
impl Transaction {
    /// Create a new empty transaction.
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            pending_blocks: Vec::new(),
        }
    }

    /// Queue a parameter change.
    ///
    /// If the same parameter is set multiple times, only the last value is kept.
    pub fn set_param(&mut self, param: ParamId, value: f64) {
        self.pending.insert(param, value);
    }

    /// Queue a bulk parameter block update (MIR-02).
    pub fn set_param_block(&mut self, span: ParamSpan, values: Vec<f64>) {
        self.pending_blocks.push((span, values));
    }

    /// Check if there are pending changes.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty() || !self.pending_blocks.is_empty()
    }

    /// Get the number of pending requests (scalars plus blocks).
    pub fn pending_count(&self) -> usize {
        self.pending.len() + self.pending_blocks.len()
    }

    /// Get the pending value for a parameter (if any).
    pub fn get_pending(&self, param: ParamId) -> Option<f64> {
        self.pending.get(&param).copied()
    }

    /// Take all pending changes, clearing the transaction.
    ///
    /// Returns an iterator of (ParamId, new_value) pairs.
    pub fn take_pending(&mut self) -> impl Iterator<Item = (ParamId, f64)> {
        std::mem::take(&mut self.pending).into_iter()
    }

    /// Take all pending block updates, clearing them.
    pub fn take_pending_blocks(&mut self) -> Vec<(ParamSpan, Vec<f64>)> {
        std::mem::take(&mut self.pending_blocks)
    }

    /// Clear all pending changes without applying them.
    pub fn rollback(&mut self) {
        self.pending.clear();
        self.pending_blocks.clear();
    }

    /// Iterate over pending changes without consuming them.
    pub fn iter_pending(&self) -> impl Iterator<Item = (ParamId, f64)> + '_ {
        self.pending.iter().map(|(&k, &v)| (k, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Generation;

    fn make_param(index: u32) -> ParamId {
        ParamId::new(index, Generation::new())
    }

    #[test]
    fn queue_and_take() {
        let mut tx = Transaction::new();
        let p1 = make_param(0);
        let p2 = make_param(1);

        tx.set_param(p1, 10.0);
        tx.set_param(p2, 20.0);

        assert!(tx.has_pending());
        assert_eq!(tx.pending_count(), 2);

        let changes: HashMap<_, _> = tx.take_pending().collect();
        assert_eq!(changes.get(&p1), Some(&10.0));
        assert_eq!(changes.get(&p2), Some(&20.0));

        assert!(!tx.has_pending());
    }

    #[test]
    fn last_value_wins() {
        let mut tx = Transaction::new();
        let p = make_param(0);

        tx.set_param(p, 1.0);
        tx.set_param(p, 2.0);
        tx.set_param(p, 3.0);

        assert_eq!(tx.pending_count(), 1);
        assert_eq!(tx.get_pending(p), Some(3.0));
    }

    #[test]
    fn rollback() {
        let mut tx = Transaction::new();
        tx.set_param(make_param(0), 1.0);

        tx.rollback();

        assert!(!tx.has_pending());
    }

    #[test]
    fn block_pending_lifecycle() {
        let mut tx = Transaction::new();
        let span = ParamSpan::from_parts(0, 2, Generation::new());
        tx.set_param_block(span, vec![1.0, 2.0]);

        assert!(tx.has_pending(), "a block alone is pending");
        assert_eq!(tx.pending_count(), 1);
        let blocks = tx.take_pending_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].1, vec![1.0, 2.0]);
        assert!(!tx.has_pending(), "taking blocks clears pending");

        tx.set_param_block(span, vec![3.0, 4.0]);
        tx.set_param(make_param(0), 8.0);
        assert_eq!(tx.pending_count(), 2, "one block + one scalar");
        assert_eq!(tx.iter_pending().count(), 1, "iter_pending sees scalars");
        tx.rollback();
        assert!(!tx.has_pending());
        assert!(tx.take_pending_blocks().is_empty());
        assert_eq!(tx.take_pending().count(), 0);
    }
}
