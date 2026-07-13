//! Typed validation for model inputs.
//!
//! Every value that enters canonical model storage is validated.
//! Invalid inputs are rejected with a typed error; the model is
//! never left in a partially-mutated state by a failed operation.

/// A finite scalar value (not NaN, not infinite).
///
/// This is the default coefficient/parameter value type.
/// Use [`BoundValue`] for bounds that may be ±∞.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct FiniteScalar(f64);

impl FiniteScalar {
    /// The zero value.
    pub const ZERO: Self = FiniteScalar(0.0);
    /// The value 1.0.
    pub const ONE: Self = FiniteScalar(1.0);

    /// Create a `FiniteScalar`, returning `None` if the value is NaN or infinite.
    pub fn new(value: f64) -> Option<Self> {
        if value.is_finite() {
            Some(FiniteScalar(value))
        } else {
            None
        }
    }

    /// Return the inner `f64`.
    pub fn get(self) -> f64 {
        self.0
    }
}

impl From<FiniteScalar> for f64 {
    fn from(v: FiniteScalar) -> f64 {
        v.0
    }
}

impl std::fmt::Display for FiniteScalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// Arithmetic for ergonomic expression use
impl std::ops::Add for FiniteScalar {
    type Output = f64;
    fn add(self, rhs: Self) -> f64 {
        self.0 + rhs.0
    }
}

impl std::ops::Mul for FiniteScalar {
    type Output = f64;
    fn mul(self, rhs: Self) -> f64 {
        self.0 * rhs.0
    }
}

impl std::ops::Neg for FiniteScalar {
    type Output = f64;
    fn neg(self) -> f64 {
        -self.0
    }
}

/// A bound value: must not be NaN, but may be ±∞.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct BoundValue(f64);

impl BoundValue {
    /// Negative infinity.
    pub const NEG_INF: Self = BoundValue(f64::NEG_INFINITY);
    /// Positive infinity.
    pub const INF: Self = BoundValue(f64::INFINITY);
    /// Zero.
    pub const ZERO: Self = BoundValue(0.0);

    /// Create a `BoundValue`, returning `None` if NaN.
    pub fn new(value: f64) -> Option<Self> {
        if value.is_nan() {
            None
        } else {
            Some(BoundValue(value))
        }
    }

    /// Return the inner `f64`.
    pub fn get(self) -> f64 {
        self.0
    }

    /// True if the value is finite.
    pub fn is_finite(self) -> bool {
        self.0.is_finite()
    }
}

impl From<BoundValue> for f64 {
    fn from(v: BoundValue) -> f64 {
        v.0
    }
}

/// A tolerance value: must be finite and non-negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Tolerance(f64);

impl Tolerance {
    /// Default feasibility tolerance (1e-9).
    pub const DEFAULT: Self = Tolerance(1e-9);

    /// Create a `Tolerance`, returning `None` if negative, NaN, or infinite.
    pub fn new(value: f64) -> Option<Self> {
        if value.is_finite() && value >= 0.0 {
            Some(Tolerance(value))
        } else {
            None
        }
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

/// Validates that a `f64` is finite. Used inline in mutators.
#[inline]
#[allow(dead_code)]
pub(crate) fn require_finite(value: f64, label: &'static str) -> Result<f64, ValidationError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ValidationError::NonFinite { label })
    }
}

/// Validates that a `f64` is not NaN. Used for bounds.
#[inline]
#[allow(dead_code)]
pub(crate) fn require_not_nan(value: f64, label: &'static str) -> Result<f64, ValidationError> {
    if value.is_nan() {
        Err(ValidationError::NaN { label })
    } else {
        Ok(value)
    }
}

/// Validates that a pair of bounds (lower, upper) is valid.
#[inline]
#[allow(dead_code)]
pub(crate) fn require_valid_bounds(lower: f64, upper: f64) -> Result<(), ValidationError> {
    if lower.is_nan() || upper.is_nan() {
        return Err(ValidationError::NaN { label: "bounds" });
    }
    if lower > upper {
        return Err(ValidationError::InvalidBounds { lower, upper });
    }
    Ok(())
}

/// Error returned when a value fails validation.
#[derive(Clone, Debug, PartialEq)]
pub enum ValidationError {
    /// Value was not finite (NaN or ±∞).
    NonFinite { label: &'static str },
    /// Value was NaN.
    NaN { label: &'static str },
    /// Lower bound exceeds upper bound.
    InvalidBounds { lower: f64, upper: f64 },
    /// Tolerance must be ≥ 0 and finite.
    InvalidTolerance { value: f64 },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFinite { label } => write!(f, "value must be finite: {label}"),
            Self::NaN { label } => write!(f, "value must not be NaN: {label}"),
            Self::InvalidBounds { lower, upper } => {
                write!(f, "invalid bounds: lower ({lower}) > upper ({upper})")
            }
            Self::InvalidTolerance { value } => {
                write!(f, "tolerance must be >= 0 and finite, got {value}")
            }
        }
    }
}

impl std::error::Error for ValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_scalar_rejects_nan() {
        assert!(FiniteScalar::new(f64::NAN).is_none());
    }

    #[test]
    fn finite_scalar_rejects_infinity() {
        assert!(FiniteScalar::new(f64::INFINITY).is_none());
        assert!(FiniteScalar::new(f64::NEG_INFINITY).is_none());
    }

    #[test]
    fn finite_scalar_accepts_normal() {
        assert!(FiniteScalar::new(0.0).is_some());
        assert!(FiniteScalar::new(-42.5).is_some());
        assert!(FiniteScalar::new(1e308).is_some());
    }

    #[test]
    fn bound_value_rejects_nan() {
        assert!(BoundValue::new(f64::NAN).is_none());
    }

    #[test]
    fn bound_value_accepts_infinity() {
        assert!(BoundValue::new(f64::INFINITY).is_some());
        assert!(BoundValue::new(f64::NEG_INFINITY).is_some());
    }

    #[test]
    fn bound_value_accepts_finite() {
        assert!(BoundValue::new(0.0).is_some());
    }

    #[test]
    fn tolerance_rejects_negative() {
        assert!(Tolerance::new(-0.001).is_none());
    }

    #[test]
    fn tolerance_rejects_nan() {
        assert!(Tolerance::new(f64::NAN).is_none());
    }

    #[test]
    fn tolerance_rejects_infinite() {
        assert!(Tolerance::new(f64::INFINITY).is_none());
    }

    #[test]
    fn tolerance_accepts_zero_and_positive() {
        assert!(Tolerance::new(0.0).is_some());
        assert!(Tolerance::new(1e-9).is_some());
        assert!(Tolerance::new(1.0).is_some());
    }

    #[test]
    fn require_finite_ok() {
        assert_eq!(require_finite(42.0, "test").unwrap(), 42.0);
    }

    #[test]
    fn require_finite_rejects_nan() {
        assert!(require_finite(f64::NAN, "test").is_err());
    }

    #[test]
    fn require_not_nan_ok() {
        assert_eq!(require_not_nan(0.0, "test").unwrap(), 0.0);
    }

    #[test]
    fn require_not_nan_rejects_nan() {
        assert!(require_not_nan(f64::NAN, "test").is_err());
    }

    #[test]
    fn require_valid_bounds_ok() {
        assert!(require_valid_bounds(0.0, 1.0).is_ok());
        assert!(require_valid_bounds(1.0, 1.0).is_ok());
        assert!(require_valid_bounds(f64::NEG_INFINITY, f64::INFINITY).is_ok());
    }

    #[test]
    fn require_valid_bounds_rejects_inverted() {
        assert!(require_valid_bounds(2.0, 1.0).is_err());
    }

    #[test]
    fn require_valid_bounds_rejects_nan_lower() {
        assert!(require_valid_bounds(f64::NAN, 1.0).is_err());
    }

    #[test]
    fn require_valid_bounds_rejects_nan_upper() {
        assert!(require_valid_bounds(0.0, f64::NAN).is_err());
    }
}
