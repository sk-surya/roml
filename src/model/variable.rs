

use crate::id::{IdArena, VarId};

/// Variable type (continuous, integer, or binary).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[derive(Default)]
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
        let tolerance = match tolerance {
            Some(tol) => tol,
            None => f64::EPSILON,
        };
        (self.upper - self.lower).abs() <= 2.0 * tolerance
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
pub struct VariableData {
    /// Variable bounds.
    pub bounds: Bounds,
    /// Variable type.
    pub var_type: VarType,
    /// Whether this variable is active in the model.
    pub active: bool,
    /// Optional name for debugging/printing.
    pub name: Option<String>,
}

/// Storage for all variables in the model.
#[derive(Clone, Debug, Default)]
pub struct VariableStore {
    arena: IdArena<VariableData>,
}