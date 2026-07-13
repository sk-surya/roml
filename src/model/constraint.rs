//! Constraint storage and operations.

use crate::id::{ConId, IdArena};

/// Constraint bounds
///
/// A constraint `lower <= expr <= upper` is represented by these bounds, where:
/// - `lower = f64::NEG_INFINITY` means no lower bound (expr <= upper)
/// - `upper = f64::INFINITY` means no upper bound (lower <= expr)
/// - `lower == upper` means an equality constraint (expr == value)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstraintBounds {
    /// Lower bound of the constraint aka LHS.
    pub lower: f64,
    /// Upper bound of the constraint aka RHS.
    pub upper: f64,
}

impl ConstraintBounds {
    /// Create an equality constraint (expr == rhs)
    pub const fn eq(rhs: f64) -> Self {
        Self {
            lower: rhs,
            upper: rhs,
        }
    }

    // Create a less-than-or-equal constraint (expr <= upper)
    pub const fn le(upper: f64) -> Self {
        Self {
            lower: f64::NEG_INFINITY,
            upper,
        }
    }

    /// Create a greater-than-or-equal constraint (expr >= lower)
    pub const fn ge(lower: f64) -> Self {
        Self {
            lower,
            upper: f64::INFINITY,
        }
    }

    /// Create a ranged constraint (lower <= expr <= upper)
    pub const fn range(lower: f64, upper: f64) -> Self {
        Self { lower, upper }
    }

    /// Check if this is an equality constraint.
    pub const fn is_equality(&self) -> bool {
        (self.upper - self.lower).abs() <= f64::EPSILON
    }

    /// Check if this has a finite lower bound.
    pub const fn has_lower(&self) -> bool {
        self.lower.is_finite()
    }

    /// Check if this has a finite upper bound.
    pub const fn has_upper(&self) -> bool {
        self.upper.is_finite()
    }

    // Check if bounds are valid (lower <= upper).
    pub fn is_valid(&self) -> bool {
        self.lower <= self.upper
    }
}

impl Default for ConstraintBounds {
    fn default() -> Self {
        Self::eq(0.0)
    }
}

/// Internal data for a constraint.
/// (which will be stored in ConstraintStore, handled by Arena, just like any other Entity Data).
/// Intentionally non-copiable for move semantics.
#[derive(Clone, Debug)]
pub(crate) struct ConstraintData {
    /// Constraint bounds.
    pub bounds: ConstraintBounds,
    /// Whether this constraint is active in the model.
    pub active: bool,
    /// Optional name for debugging/printing.
    pub name: Option<String>,
}

impl ConstraintData {
    /// Create a new ConstraintData with default settings.
    pub fn new(bounds: ConstraintBounds) -> Self {
        Self {
            bounds,
            active: true,
            name: None,
        }
    }
}

/// Storage for all constraints in the model.
#[derive(Clone, Debug, Default)]
pub(crate) struct ConstraintStore {
    arena: IdArena<ConstraintData>,
}

impl ConstraintStore {
    /// Create an empty constraint store.
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

    /// Add a new constraint and return its ID.
    pub fn add(&mut self, bounds: ConstraintBounds) -> ConId {
        let data = ConstraintData::new(bounds);
        let (index, generation) = self.arena.allocate(data);
        ConId::new(index, generation)
    }

    /// Add a new constraint with a name.
    pub fn add_named(&mut self, bounds: ConstraintBounds, name: String) -> ConId {
        let mut data = ConstraintData::new(bounds);
        data.name = Some(name);
        let (index, generation) = self.arena.allocate(data);
        ConId::new(index, generation)
    }

    /// Remove a constraint. Returns the data if it existed.
    pub fn remove(&mut self, id: ConId) -> Option<ConstraintData> {
        self.arena.remove(id.index(), id.generation())
    }

    /// Get constraint data by ID.
    pub fn get(&self, id: ConId) -> Option<&ConstraintData> {
        self.arena.get(id.index(), id.generation())
    }

    /// Get mutable constraint data by ID.
    pub fn get_mut(&mut self, id: ConId) -> Option<&mut ConstraintData> {
        self.arena.get_mut(id.index(), id.generation())
    }

    /// Check if a constraint ID is valid.
    pub fn contains(&self, id: ConId) -> bool {
        self.arena.contains(id.index(), id.generation())
    }

    /// Get the number of constraints
    pub fn len(&self) -> usize {
        self.arena.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    /// Iterate over all constraints.
    pub fn iter(&self) -> impl Iterator<Item = (ConId, &ConstraintData)> {
        self.arena
            .iter()
            .map(|(idx, generation, data)| (ConId::new(idx, generation), data))
    }

    /// Iterate over active constraints only.
    pub fn iter_active(&self) -> impl Iterator<Item = (ConId, &ConstraintData)> {
        self.iter().filter(|(_, data)| data.active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constraint_types() {
        let eq = ConstraintBounds::eq(5.0);
        assert!(eq.is_equality());
        assert_eq!(eq.lower, 5.0);
        assert_eq!(eq.upper, 5.0);

        let le = ConstraintBounds::le(10.0);
        assert!(!le.is_equality());
        assert!(!le.has_lower());
        assert!(le.has_upper());
        assert_eq!(le.lower, f64::NEG_INFINITY);
        assert_eq!(le.upper, 10.0);

        let ge = ConstraintBounds::ge(0.3);
        assert!(!ge.is_equality());
        assert!(ge.has_lower());
        assert!(!ge.has_upper());
        assert_eq!(ge.lower, 0.3);
        assert_eq!(ge.upper, f64::INFINITY);

        let range = ConstraintBounds::range(1.0, 10.0);
        assert!(range.has_lower());
        assert!(range.has_upper());
        assert!(!range.is_equality());
        assert_eq!(range.lower, 1.0);
        assert_eq!(range.upper, 10.0);
    }

    #[test]
    fn add_and_get() {
        let mut store = ConstraintStore::new();
        let id = store.add(ConstraintBounds::le(100.0));

        let data = store.get(id);
        assert!(data.is_some());
        let data = data.unwrap();
        assert_eq!(data.bounds, ConstraintBounds::le(100.0));
        assert!(data.active);
        assert_eq!(store.len(), 1);

        // add one more constraint
        store.add(ConstraintBounds::ge(5.0));
        for (i, (_, con_data)) in store.iter().enumerate() {
            if i == 0 {
                assert_eq!(con_data.bounds, ConstraintBounds::le(100.0));
            } else {
                assert_eq!(con_data.bounds, ConstraintBounds::ge(5.0));
            }
        }
    }
}
