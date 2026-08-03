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

use crate::id::{ConId, ParamId, VarId};
use crate::value_expr::ValueExpr;

/// A constrained function-in-set primitive constraint.
///
/// `function` is the left-hand scalar function; `set` constrains it. For
/// linear rows the coefficient index remains the single coefficient
/// authority: the linear function is reconstructed deterministically from it
/// (P25 Task 3).
///
/// # Symbolic view (F1)
///
/// [`terms`](Self::terms) and [`dependencies`](Self::dependencies) are the
/// *symbolic* view of the left-hand side: each `(VarId, ValueExpr)` preserves
/// the parameter-dependent coefficient expression (not just its evaluated
/// number), and `dependencies` lists every parameter the row references. They
/// are populated by the canonical [`Model::constraint_function`](crate::Model::constraint_function)
/// reconstruction from the coefficient index; the design §6 evaluated
/// `function`/`set` remain the primary form.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionConstraint {
    /// The scalar function on the left-hand side.
    pub function: ScalarFunction,
    /// The set constraining the function.
    pub set: ScalarSet,
    /// The symbolic left-hand-side terms: variable and its coefficient
    /// `ValueExpr` (F1). Empty for a raw pre-coefficient-index
    /// [`ConstraintSpec`](crate::expr::ConstraintSpec); populated by the
    /// canonical reconstruction.
    pub terms: Vec<(VarId, ValueExpr)>,
    /// Parameter dependencies of the left-hand side, sorted and deduplicated
    /// (F1). Empty when the row references no parameters.
    pub dependencies: Vec<ParamId>,
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
/// [`terms`](Self::terms) and [`dependencies`](Self::dependencies) mirror the
/// [`FunctionConstraint`] symbolic view: each term preserves the
/// parameter-dependent `ValueExpr`, so P26's compiler can rebuild the
/// parameterized row without re-joining legacy cells. The evaluated
/// `function`/`set` remain the design §6 form.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionEntry {
    /// The constraint this function-in-set entry describes.
    pub constraint: ConId,
    /// The reconstructed scalar function.
    pub function: ScalarFunction,
    /// The reconstructed scalar set.
    pub set: ScalarSet,
    /// The symbolic left-hand-side terms: variable and its coefficient
    /// `ValueExpr` (F1).
    pub terms: Vec<(VarId, ValueExpr)>,
    /// Parameter dependencies of the left-hand side, sorted and deduplicated
    /// (F1).
    pub dependencies: Vec<ParamId>,
}
