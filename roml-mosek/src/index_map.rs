//! Map from a typed ID to a MOSEK column/row index.
//!
//! MOSEK uses dense integer indices for columns and rows. When an element
//! is deleted at index `k`, MOSEK closes the gap: everything at index > k
//! shifts down by 1. `reindex_after_delete` handles this maintenance.

use std::collections::HashMap;
use std::hash::Hash;

pub struct IndexMap<Id: Hash + Eq + Copy> {
    id_to_idx: HashMap<Id, i32>,
}

impl<Id: Hash + Eq + Copy> IndexMap<Id> {
    pub fn new() -> Self {
        Self {
            id_to_idx: HashMap::new(),
        }
    }

    pub fn insert(&mut self, id: Id, idx: i32) {
        self.id_to_idx.insert(id, idx);
    }

    pub fn get(&self, id: Id) -> Option<i32> {
        self.id_to_idx.get(&id).copied()
    }

    pub fn remove(&mut self, id: Id) -> Option<i32> {
        self.id_to_idx.remove(&id)
    }

    pub fn len(&self) -> usize {
        self.id_to_idx.len()
    }

    /// After MOSEK deletes the element at `deleted`, decrement every stored
    /// index that is strictly greater than `deleted` by 1.
    pub fn reindex_after_delete(&mut self, deleted: i32) {
        for idx in self.id_to_idx.values_mut() {
            if *idx > deleted {
                *idx -= 1;
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (Id, i32)> + '_ {
        self.id_to_idx.iter().map(|(id, idx)| (*id, *idx))
    }

    /// Build a reverse map: MOSEK index → Id.
    pub fn reverse_map(&self) -> HashMap<i32, Id> {
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
        let rev = m.reverse_map();
        assert_eq!(rev.len(), 2);
        assert_eq!(rev.get(&0), Some(&10u32));
        assert_eq!(rev.get(&1), Some(&20u32));
    }

    #[test]
    fn reindex_after_delete() {
        let mut m: IndexMap<u32> = IndexMap::new();
        m.insert(0u32, 0);
        m.insert(1u32, 1);
        m.insert(2u32, 2);
        m.insert(3u32, 3);

        m.remove(1u32);
        m.reindex_after_delete(1);

        assert_eq!(m.get(0u32), Some(0));
        assert_eq!(m.get(2u32), Some(1));
        assert_eq!(m.get(3u32), Some(2));
    }
}
