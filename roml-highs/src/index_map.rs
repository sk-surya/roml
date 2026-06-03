//! Map from a typed ID to a HiGHS column/row index.
//!
//! HiGHS uses dense integer indices for columns and rows. When an element
//! is deleted at index `k`, HiGHS closes the gap: everything at index > k
//! shifts down by 1. `reindex_after_delete` handles this maintenance.

use std::collections::HashMap;
use std::hash::Hash;

/// Maps typed IDs (VarId, ConId, …) to HiGHS integer indices.
pub struct IndexMap<Id: Hash + Eq + Copy> {
    id_to_idx: HashMap<Id, i32>,
}

impl<Id: Hash + Eq + Copy> IndexMap<Id> {
    pub fn new() -> Self {
        Self {
            id_to_idx: HashMap::new(),
        }
    }

    /// Insert or overwrite the index for `id`.
    pub fn insert(&mut self, id: Id, idx: i32) {
        self.id_to_idx.insert(id, idx);
    }

    /// Look up the HiGHS index for `id`.
    pub fn get(&self, id: Id) -> Option<i32> {
        self.id_to_idx.get(&id).copied()
    }

    /// Remove `id` from the map, returning its old index if present.
    pub fn remove(&mut self, id: Id) -> Option<i32> {
        self.id_to_idx.remove(&id)
    }

    pub fn contains(&self, id: Id) -> bool {
        self.id_to_idx.contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.id_to_idx.len()
    }

    /// After HiGHS deletes the element that was at index `deleted`, decrement
    /// every stored index that is strictly greater than `deleted` by 1.
    ///
    /// This keeps the map in sync with HiGHS's dense indexing.
    pub fn reindex_after_delete(&mut self, deleted: i32) {
        for idx in self.id_to_idx.values_mut() {
            if *idx > deleted {
                *idx -= 1;
            }
        }
    }

    /// Iterate over all (id, index) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (Id, i32)> + '_ {
        self.id_to_idx.iter().map(|(id, idx)| (*id, *idx))
    }

    /// Build a reverse map: HiGHS index → Id.
    ///
    /// Useful for translating HiGHS column/row indices back to roml IDs
    /// inside callback handlers.
    pub fn reverse_map(&self) -> std::collections::HashMap<i32, Id> {
        self.iter().map(|(id, idx)| (idx, id)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_insert_get_remove() {
        let mut m: IndexMap<u32> = IndexMap::new();
        m.insert(10u32, 0);
        m.insert(20u32, 1);
        m.insert(30u32, 2);

        assert_eq!(m.get(10u32), Some(0));
        assert_eq!(m.get(20u32), Some(1));
        assert_eq!(m.get(30u32), Some(2));

        assert_eq!(m.remove(20u32), Some(1));
        assert_eq!(m.get(20u32), None);
    }

    #[test]
    fn reverse_map_works() {
        let mut m: IndexMap<u32> = IndexMap::new();
        m.insert(10u32, 0);
        m.insert(20u32, 1);
        m.insert(30u32, 2);

        let rev = m.reverse_map();
        assert_eq!(rev.len(), 3);
        assert_eq!(rev.get(&0), Some(&10u32));
        assert_eq!(rev.get(&1), Some(&20u32));
        assert_eq!(rev.get(&2), Some(&30u32));
    }

    #[test]
    fn reindex_after_delete() {
        let mut m: IndexMap<u32> = IndexMap::new();
        // indices 0, 1, 2, 3
        m.insert(0u32, 0);
        m.insert(1u32, 1);
        m.insert(2u32, 2);
        m.insert(3u32, 3);

        // Delete the element at index 1 (id=1).
        m.remove(1u32);
        m.reindex_after_delete(1);

        // Indices > 1 shift down.
        assert_eq!(m.get(0u32), Some(0)); // unchanged
        assert_eq!(m.get(2u32), Some(1)); // was 2, now 1
        assert_eq!(m.get(3u32), Some(2)); // was 3, now 2
    }
}
