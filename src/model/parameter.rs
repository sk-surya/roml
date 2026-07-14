//! Parameter storage and opertions.

use crate::id::{IdArena, ParamId};

/// Internal data for a parameter.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct ParameterData {
    /// Current parameter value.
    pub value: f64,
    /// Optional name for the debugging/printing.
    pub name: Option<String>,
}

impl ParameterData {
    /// Create a new parameter data with the given value.
    pub fn new(value: f64) -> Self {
        Self { value, name: None }
    }
}

/// Storage for all parameters in the model.
///
/// Parameters are coefficient value sources that can be modified at runtime.
/// When a parameter changes, all dependent coefficients must be updated.
#[derive(Clone, Debug, Default)]
pub(crate) struct ParameterStore {
    arena: IdArena<ParameterData>,
}

/// Methods used by Model.
#[allow(dead_code)]
impl ParameterStore {
    /// Create an empty parameter store.
    pub fn new() -> Self {
        Self {
            arena: IdArena::new(),
        }
    }

    /// Create a store with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            arena: IdArena::with_capacity(capacity),
        }
    }

    /// Add a new parameter and return its ID.
    pub fn add(&mut self, value: f64) -> ParamId {
        let data = ParameterData::new(value);
        let (index, generation) = self.arena.allocate(data);
        ParamId::new(index, generation)
    }

    /// Add a new parameter with a name.
    pub fn add_named(&mut self, value: f64, name: String) -> ParamId {
        let mut data = ParameterData::new(value);
        data.name = Some(name);
        let (index, generation) = self.arena.allocate(data);
        ParamId::new(index, generation)
    }

    /// Remove a parameter. Returns the data if it existed.
    ///
    /// Just keeping signature like other entity stores (I may want to codegen?).
    /// Parameters are rarely removed and I don't expect this to be used - What happens to dependencies?
    // pub fn remove(&mut self, id: ParamId) -> Option<ParameterData> {
    // self.arena.remove(id.index(), id.generation())
    // }
    #[allow(unused)]
    pub fn remove(&mut self, id: ParamId) {
        unimplemented!()
    }

    /// Get parameter data by ID.
    pub fn get(&self, id: ParamId) -> Option<&ParameterData> {
        self.arena.get(id.index(), id.generation())
    }

    /// Get mutable parameter data by ID.
    pub fn get_mut(&mut self, id: ParamId) -> Option<&mut ParameterData> {
        self.arena.get_mut(id.index(), id.generation())
    }

    /// Get the current value of a parameter.
    pub fn get_value(&self, id: ParamId) -> Option<f64> {
        self.get(id).map(|d| d.value)
    }

    /// Set the value of a parameter. Returns the old value if it existed.
    ///
    /// # Note
    ///
    /// This only updates the stored value. Coefficient propagation is handled
    /// by the transaction system in the Model.
    pub fn set_value(&mut self, id: ParamId, value: f64) -> Option<f64> {
        self.get_mut(id).map(|d| {
            let old = d.value;
            d.value = value;
            old
        })
    }

    /// Check if a parameter ID is valid.
    pub fn contains(&self, id: ParamId) -> bool {
        self.arena.contains(id.index(), id.generation())
    }

    /// Get the number of parameters.
    pub fn len(&self) -> usize {
        self.arena.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    /// Iterate over all parameters.
    pub fn iter(&self) -> impl Iterator<Item = (ParamId, &ParameterData)> {
        self.arena
            .iter()
            .map(|(idx, gen, data)| (ParamId::new(idx, gen), data))
    }

    /// Lookup function for ValueExpr evaluation.
    ///
    /// Returns a closure that can be passed to `ValueExpr::eval()`.
    pub fn as_lookup(&self) -> impl Fn(ParamId) -> f64 + '_ {
        move |id| self.get_value(id).unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_get() {
        let mut store = ParameterStore::new();
        let id = store.add(42.0);

        assert_eq!(store.get_value(id), Some(42.0));
    }

    #[test]
    fn set_value() {
        let mut store = ParameterStore::new();
        let id = store.add(1.0);

        let old = store.set_value(id, 2.0);
        assert_eq!(old, Some(1.0));
        assert_eq!(store.get_value(id), Some(2.0));
    }

    #[test]
    fn lookup_function() {
        let mut store = ParameterStore::new();
        let p1 = store.add(10.0);
        let p2 = store.add(20.0);

        let lookup = store.as_lookup();
        assert_eq!(lookup(p1), 10.0);
        assert_eq!(lookup(p2), 20.0);
    }
}
