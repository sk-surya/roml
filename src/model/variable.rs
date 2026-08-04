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

/// The declared domain of a variable (design §10, SM-05.1).
///
/// Separates the declared domain (bounds, type, optional semi-continuous
/// lower bound) from the optional persistent [`VariableFixing`]. The compiled
/// effective bounds of a fixed variable are `[value, value]` (D6: fixing
/// compiles as bound tightening, never a separate equality row).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VariableDomain {
    /// The declared variable bounds.
    pub bounds: Bounds,
    /// The declared variable type (continuous, integer, or binary).
    pub var_type: VarType,
    /// Optional semi-continuous domain (declared canonical state only; the
    /// compiled IR has no semi-continuous representation, so it is rejected
    /// at the compile boundary — P26 behavior unchanged).
    pub semi: Option<SemiDomain>,
}

/// A semi-continuous domain (design §10): the variable is zero or at least
/// `nonzero_lower` in magnitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SemiDomain {
    /// Semi-continuous continuous variable: 0 or `≥ nonzero_lower`.
    Continuous {
        /// The non-zero lower bound.
        nonzero_lower: f64,
    },
    /// Semi-continuous integer variable: 0 or `≥ nonzero_lower`.
    Integer {
        /// The non-zero lower bound.
        nonzero_lower: f64,
    },
}

/// The provenance of a persistent [`VariableFixing`] (design §10, SM-05.5).
#[derive(Clone, Debug, PartialEq)]
pub enum FixingProvenance {
    /// The fixing was made by the user through [`Model::fix`](crate::Model::fix).
    User,
    /// The fixing was imported from an external source.
    Imported {
        /// A diagnostic label describing the import source.
        source: String,
    },
}

/// A persistent variable fixing (design §10, SM-05.1).
///
/// Fixing is represented as bound tightening: a fixed variable's effective
/// bounds equal `[value, value]`. The fixing is stored separately from the
/// declared domain so `unfix` can restore the *current* declared bounds
/// (SM-05.4).
#[derive(Clone, Debug, PartialEq)]
pub struct VariableFixing {
    /// The value the variable is fixed to.
    pub value: f64,
    /// Where this fixing came from (diagnostics and provenance).
    pub provenance: FixingProvenance,
}

/// Internal data for a variable.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct VariableData {
    /// The declared variable domain.
    pub domain: VariableDomain,
    /// Optional persistent fixing (SM-05.1).
    pub fixing: Option<VariableFixing>,
    /// Whether this variable is active in the model.
    pub active: bool,
    /// Optional name for debugging/printing.
    pub name: Option<String>,
}

impl VariableData {
    /// Create a new variable with default settings.
    pub fn new(bounds: Bounds, var_type: VarType) -> Self {
        Self {
            domain: VariableDomain {
                bounds,
                var_type,
                semi: None,
            },
            fixing: None,
            active: true,
            name: None,
        }
    }
}

/// A validated variable definition (D7).
///
/// Built by [`continuous`], [`integer`], and [`binary`] and consumed by
/// [`Model::add_variable`](crate::Model::add_variable). Supports optional
/// bounds and a name.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableDef {
    pub(crate) bounds: Bounds,
    pub(crate) var_type: VarType,
    pub(crate) name: Option<String>,
}

impl VariableDef {
    /// Override the bounds of this definition.
    pub fn bounds(mut self, lower: f64, upper: f64) -> Self {
        self.bounds = Bounds::new(lower, upper);
        self
    }

    /// Override only the lower bound, preserving the current upper bound
    /// (default `+inf` for continuous/integer definitions, `1.0` for binary).
    pub fn lower_bound(mut self, lower: f64) -> Self {
        self.bounds.lower = lower;
        self
    }

    /// Override only the upper bound, preserving the current lower bound
    /// (default `0.0`).
    pub fn upper_bound(mut self, upper: f64) -> Self {
        self.bounds.upper = upper;
        self
    }

    /// Attach a name to this definition.
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub(crate) fn into_parts(self) -> (Bounds, VarType, Option<String>) {
        (self.bounds, self.var_type, self.name)
    }
}

/// A validated continuous variable definition with non-negative bounds.
pub fn continuous() -> VariableDef {
    VariableDef {
        bounds: Bounds::NON_NEGATIVE,
        var_type: VarType::Continuous,
        name: None,
    }
}

/// A validated integer variable definition with non-negative bounds.
pub fn integer() -> VariableDef {
    VariableDef {
        bounds: Bounds::NON_NEGATIVE,
        var_type: VarType::Integer,
        name: None,
    }
}

/// A validated binary variable definition with `[0, 1]` bounds.
pub fn binary() -> VariableDef {
    VariableDef {
        bounds: Bounds::BINARY,
        var_type: VarType::Binary,
        name: None,
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
        assert_eq!(data.domain.bounds, Bounds::NON_NEGATIVE);
        assert_eq!(data.domain.var_type, VarType::Continuous);
        assert!(data.fixing.is_none());
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
