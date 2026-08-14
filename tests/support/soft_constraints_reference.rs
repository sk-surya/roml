//! Independent mathematical reference rows for persistent soft constraints.

use roml::{ConstraintBounds, ViolationSide};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReferenceSide {
    pub side: ViolationSide,
    pub bounds: ConstraintBounds,
    pub violation_bounds: (f64, f64),
    pub expression_sign: f64,
}

pub fn sides(bounds: ConstraintBounds, cap: Option<f64>) -> Vec<ReferenceSide> {
    let upper = cap.unwrap_or(f64::INFINITY);
    let mut result = Vec::new();
    if bounds.lower.is_finite() {
        result.push(ReferenceSide {
            side: ViolationSide::Lower,
            bounds: ConstraintBounds::ge(bounds.lower),
            violation_bounds: (0.0, upper),
            expression_sign: 1.0,
        });
    }
    if bounds.upper.is_finite() {
        result.push(ReferenceSide {
            side: ViolationSide::Upper,
            bounds: ConstraintBounds::le(bounds.upper),
            violation_bounds: (0.0, upper),
            expression_sign: -1.0,
        });
    }
    result
}

pub fn raw_violation(value: f64, bounds: ConstraintBounds) -> (f64, f64) {
    (
        (bounds.lower - value).max(0.0),
        (value - bounds.upper).max(0.0),
    )
}
