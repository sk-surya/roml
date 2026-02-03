//! Objective storage and operations.

use crate::id::{IdArena, ObjId};

/// Optimization sense (minimize or maximize).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[derive(Default)]
pub enum Sense {
    /// Minimize the objective.
    #[default]
    Minimize,
    /// Maximize the objective.
    Maximize,
}

/// internal data for an objective.
#[derive(Clone, Debug)]
pub struct ObjectiveData {
    /// Optimization sense.
    pub sense: Sense,
    /// Whether this objective is active (only one can be active at a time).
    pub active: bool,
    /// Optional name for debugging/printing.
    pub name: Option<String>,
}

impl ObjectiveData {
    /// Create a new objective with default settings.
    pub fn new(sense: Sense) -> Self {
        Self {
            sense,
            active: false,  // Inactive by default, user must activate explicitly.
            name: None,
        }
    }
}

/// Storage for all objectives in the model.
/// 
/// # Invariant
/// 
/// Only one objective can be active at time. The store enforces this
/// by deactivating the current active objective when a new one is activated.
#[derive(Clone, Debug, Default)]
pub struct ObjectiveStore {
    arena: IdArena<ObjectiveData>,
    /// Currently active objective, if any.
    active_objective: Option<ObjId>,
}

impl ObjectiveStore {
    /// Create a new empty objective store.
    pub fn new() -> Self {
        Self {
            arena: IdArena::new(),
            active_objective: None,
        }
    }

    /// Add a new objective and return its ID.
    /// 
    /// The new objective is inactive by default.
    pub fn add(&mut self, sense: Sense) -> ObjId {
        let data = ObjectiveData::new(sense);
        let (index, generation) = self.arena.allocate(data);
        ObjId::new(index, generation)
    }

    /// Add a new objective with a name.
    pub fn add_named(&mut self, sense: Sense, name: String) -> ObjId {
        let mut data = ObjectiveData::new(sense);
        data.name = Some(name);
        let (index, generation) = self.arena.allocate(data);
        ObjId::new(index, generation)
    }

    /// Remove an objective by its ID. Returns the data if it existed.
    /// 
    /// If this was the active objective, then no objective will be active.
    pub fn remove(&mut self, id: ObjId) -> Option<ObjectiveData> {
        if self.active_objective == Some(id) {
            self.active_objective = None;
        }
        self.arena.remove(id.index(), id.generation())
    }

    /// Get objective data by its ID.
    pub fn get(&self, id: ObjId) -> Option<&ObjectiveData> {
        self.arena.get(id.index(), id.generation())
    }

    /// Get mutable objective by its ID.
    pub fn get_mut(&mut self, id: ObjId) -> Option<&mut ObjectiveData> {
        self.arena.get_mut(id.index(), id.generation())
    }

    /// Get the currently active Objective ID.
    pub fn active(&self) -> Option<ObjId> {
        self.active_objective
    }

    /// Set the given objective as active.
    /// 
    /// Deactivates the previous objective and activates the new one.
    /// Returns the previously active objective (if any).
    pub fn set_active(&mut self, id: ObjId) -> Option<ObjId> {
        let previous = self.clear_active();
        
        if let Some(data) = self.get_mut(id) {
            data.active = true;
            self.active_objective = Some(id);
        }

        previous
    }

    /// Clear the active objective (none will be active) and return its id
    pub fn clear_active(&mut self) -> Option<ObjId> {
        let previous = self.active_objective;

        if let Some(prev_id) = previous {
            if let Some(data) = self.arena.get_mut(prev_id.index(), prev_id.generation()) {
                data.active = false;
            }
        }

        self.active_objective = None;
        previous
    }

    /// Check if an objective ID is valid.
    pub fn contains(&self, id: ObjId) -> bool {
        self.arena.contains(id.index(), id.generation())
    }

    /// Get the number of objectives.
    pub fn len(&self) -> usize {
        self.arena.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    /// Iterate over all objectives.
    pub fn iter(&self) -> impl Iterator<Item = (ObjId, &ObjectiveData)> {
        self.arena
            .iter()
            .map(|(idx, gen, data)| (ObjId::new(idx, gen), data))
    }
}