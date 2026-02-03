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
#[derive(Clone, Debug)]
pub struct ConstraintData {
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
#[derive(Debug, Clone, Default)]
pub struct ConstraintStore {
    arena: IdArena<ConstraintData>,                                    
}