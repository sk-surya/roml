//! Variable storage and operations.

use crate::id::{IdArena, VarId};

/// Variable type (continuous, integer, or binary).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum VarType {
    /// Continuous variable (can take any value in bounds).
    #[default]
    Continuous,
    /// Integer variable (must be integer in bounds).
    Integer,
    /// Binary variable (0 or 1).
    Binary,
}

/// Variable bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// Lower bound (f64::NEG_INFINITY for unbounded below).
    pub lower: f64,
    /// Upper bound (f64::INFINITY for unbounded above).
    pub upper: f64,
}

impl Bounds {
    /// Unbounded in both directions.
    pub const UNBOUNDED: Self = Self {
        lower: f64::NEG_INFINITY,
        upper: f64::INFINITY,
    };

    /// Non-negative: [0, +inf).
    pub const NON_NEGATIVE: Self = Self {
        lower: 0.0,
        upper: f64::INFINITY,
    };

    /// Binary bounds: [0, 1].
    pub const BINARY: Self = Self {
        lower: 0.0,
        upper: 1.0,
    };

    /// Create bounds with given lower and upper.
    pub const fn new(lower: f64, upper: f64) -> Self {
        Self { lower, upper }
    }

    /// Create a fixed value (lower == upper). Optionally with tolerance.
    pub const fn fixed(value: f64, tolerance: Option<f64>) -> Self {
        let tolerance = match tolerance {
            Some(tol) => tol,
            None => f64::EPSILON,
        };
        Self {
            lower: value - tolerance,
            upper: value + tolerance,
        }
    }

    /// Check if this is a fixed value. Optionally with tolerance.
    pub fn is_fixed(&self, tolerance: Option<f64>) -> bool {
        let tolerance = tolerance.unwrap_or(f64::EPSILON);
        (self.upper - self.lower).abs() <= 2.0 * tolerance + f64::EPSILON
    }

    /// Check if bounds are valid (lower <= upper).
    pub fn is_valid(&self) -> bool {
        self.lower <= self.upper
    }
}

impl Default for Bounds {
    fn default() -> Self {
        Self::NON_NEGATIVE
    }
}

/// Internal data for a variable.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct VariableData {
    /// Variable bounds.
    pub bounds: Bounds,
    /// Variable type.
    pub var_type: VarType,
    /// Whether this variable is active in the model.
    pub active: bool,
    /// Optional name for debugging/printing.
    pub name: Option<String>,
}

impl VariableData {
    /// Create a new variable with default settings.
    pub fn new(bounds: Bounds, var_type: VarType) -> Self {
        Self {
            bounds,
            var_type,
            active: true,
            name: None,
        }
    }
}

/// Storage for all variables in the model.
#[derive(Clone, Debug, Default)]
pub(crate) struct VariableStore {
    arena: IdArena<VariableData>,
}

/// Methods used by Model.
#[allow(dead_code)]
impl VariableStore {
    /// Create an empty variable store.
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

    /// Add a new variable and return its ID.
    pub fn add(&mut self, bounds: Bounds, var_type: VarType) -> VarId {
        let data = VariableData::new(bounds, var_type);
        let (index, generation) = self.arena.allocate(data);
        VarId::new(index, generation)
    }

    /// Add a new variable with a name.
    pub fn add_named(&mut self, bounds: Bounds, var_type: VarType, name: String) -> VarId {
        let mut data = VariableData::new(bounds, var_type);
        data.name = Some(name);
        let (index, generation) = self.arena.allocate(data);
        VarId::new(index, generation)
    }

    /// Remove a variable. Returns the data if it existed.
    pub fn remove(&mut self, id: VarId) -> Option<VariableData> {
        self.arena.remove(id.index(), id.generation())
    }

    /// Get variable data by ID.
    pub fn get(&self, id: VarId) -> Option<&VariableData> {
        self.arena.get(id.index(), id.generation())
    }

    /// Get mutable variable data by ID.
    pub fn get_mut(&mut self, id: VarId) -> Option<&mut VariableData> {
        self.arena.get_mut(id.index(), id.generation())
    }

    /// Check if a variable ID is valid.
    pub fn contains(&self, id: VarId) -> bool {
        self.arena.contains(id.index(), id.generation())
    }

    /// Get the number of variables.
    pub fn len(&self) -> usize {
        self.arena.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    /// Iterate over all variables.
    pub fn iter(&self) -> impl Iterator<Item = (VarId, &VariableData)> {
        self.arena
            .iter()
            .map(|(idx, gen, data)| (VarId::new(idx, gen), data))
    }

    /// Iterate over active variables only.
    pub fn iter_active(&self) -> impl Iterator<Item = (VarId, &VariableData)> {
        self.iter().filter(|(_, data)| data.active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_validation() {
        assert!(Bounds::new(0.0, 10.0).is_valid());
        assert!(Bounds::new(5.0, 5.0).is_valid());
        assert!(!Bounds::new(10.0, 0.0).is_valid());
        assert!(!Bounds::new(10.0, 0.0).is_valid());
        assert!(Bounds::fixed(3.0, None).is_fixed(None));
        assert!(Bounds::fixed(3.0, Some(0.00001)).is_fixed(Some(0.00001)));
        assert!(!Bounds::fixed(3.0, Some(0.00001)).is_fixed(Some(0.000001)));
        assert!(Bounds::fixed(3.0, Some(0.00001)).is_fixed(Some(0.0001)));
    }

    #[test]
    fn add_and_get() {
        let mut store = VariableStore::new();
        let id = store.add(Bounds::NON_NEGATIVE, VarType::Continuous);

        let data = store.get(id).unwrap();
        assert_eq!(data.bounds, Bounds::NON_NEGATIVE);
        assert_eq!(data.var_type, VarType::Continuous);
        assert!(data.active);
    }

    #[test]
    fn remove_invalidates() {
        let mut store = VariableStore::new();
        let id = store.add(Bounds::NON_NEGATIVE, VarType::Continuous);

        assert!(store.arena.len() == 1);
        assert!(store.arena.capacity_used() == 1);
        let removed = store.remove(id);
        assert!(removed.is_some());
        assert!(store.arena.is_empty());
        assert!(store.arena.capacity_used() == 1);
        assert!(store.get(id).is_none());
        assert!(!store.contains(id));
    }

    #[test]
    fn active_filtering() {
        let mut store = VariableStore::new();
        let id1 = store.add(Bounds::NON_NEGATIVE, VarType::Continuous);
        let id2 = store.add(Bounds::NON_NEGATIVE, VarType::Continuous);

        let active: Vec<_> = store.iter_active().map(|(id, _)| id).collect();
        assert_eq!(active.len(), 2);
        // Deactivate first variable
        store.get_mut(id1).unwrap().active = false;

        let active: Vec<_> = store.iter_active().map(|(id, _)| id).collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0], id2);
    }
}
