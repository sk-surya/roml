//! Canonical scalar functions and sets (design §6).
//!
//! Primitive constraints use a constrained function-in-set core:
//! [`FunctionConstraint`] pairs a [`ScalarFunction`] with a [`ScalarSet`].
//! The ordinary M2 `LinExpr` and `.le`/`.ge`/`.eq`/`.between` builders remain
//! the canonical user path (SM-01.5); users do not construct these enums
//! directly, but the canonical model state and snapshot/delta entries carry
//! them (SM-01.1, SM-01.4).

pub mod scalar;
pub mod set;

pub use scalar::ScalarFunction;
pub use set::ScalarSet;

use crate::id::{ConId, ParamId};

/// A constrained function-in-set primitive constraint (design §6).
///
/// `function` is the left-hand scalar function; `set` constrains it. For
/// linear rows the coefficient index remains the single coefficient
/// authority: the linear function is reconstructed deterministically from it
/// (P25 Task 3).
///
/// # Symbolic view (F1)
///
/// The symbolic expression belongs INSIDE the scalar function: the canonical
/// reconstruction builds `ScalarFunction::Linear(LinExpr)` whose terms carry
/// `TermCoeff::Expr(ValueExpr)` sourced from the coefficient index's
/// `value_expr`, so a parameterized coefficient `p*x` keeps its symbolic form.
/// Parameter dependencies are DERIVED — see [`Self::parameter_dependencies`] —
/// never stored as a parallel field (a single authority for each function).
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionConstraint {
    /// The scalar function on the left-hand side.
    pub function: ScalarFunction,
    /// The set constraining the function.
    pub set: ScalarSet,
}

impl FunctionConstraint {
    /// Derive the parameter dependencies of the left-hand function (F1).
    ///
    /// Dependencies are DERIVED from the symbolic `ValueExpr` coefficients
    /// inside the function, never stored.
    pub fn parameter_dependencies(&self) -> Vec<ParamId> {
        self.function.parameter_dependencies()
    }
}

/// Conversion into a canonical scalar function.
///
/// Implemented for [`LinExpr`](crate::expr::LinExpr) in M3.
pub trait IntoScalarFunction {
    /// Convert this value into a canonical [`ScalarFunction`].
    fn into_scalar_function(self) -> ScalarFunction;
}

impl IntoScalarFunction for crate::expr::LinExpr {
    fn into_scalar_function(self) -> ScalarFunction {
        ScalarFunction::Linear(self)
    }
}

/// A semantic function-in-set entry carried by a snapshot or delta batch.
///
/// The entry is always *reconstructed* from the authoritative coefficient
/// index / legacy operations — it is a derived semantic view, never a second
/// coefficient authority (P25 Task 3, SM-01.1).
///
/// # Symbolic view (F1)
///
/// The reconstructed [`function`](Self::function) carries the symbolic
/// expression inside itself: `ScalarFunction::Linear(LinExpr)` with
/// `TermCoeff::Expr(ValueExpr)` coefficients sourced from the authoritative
/// cells, so P26's compiler can rebuild a parameterized row without re-joining
/// legacy cells. Parameter dependencies are DERIVED from the function via
/// [`ScalarFunction::parameter_dependencies`], never stored as a parallel
/// field.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionEntry {
    /// The constraint this function-in-set entry describes.
    pub constraint: ConId,
    /// The reconstructed scalar function.
    pub function: ScalarFunction,
    /// The reconstructed scalar set.
    pub set: ScalarSet,
}
