//! Binary product construct payload (design §7, §16.5; D23; SM-12.6, SM-12.7).
//!
//! Exact product support is limited to binary-binary and binary-times-bounded-
//! linear (D23). Continuous-times-continuous exact equality is not exposed as
//! exact MILP — the builder rejects it with a typed error (SM-12.7) and no
//! relaxation is ever labeled exact. The payload stores the exact semantic
//! content only; the per-construct formulation preference lives in the
//! [`ConstructEntry`](crate::construct::ConstructEntry) (A29).

use crate::expr::LinExpr;
use crate::id::{ParamId, VarId};

/// One operand of a [`BinaryProductConstraint`] (design §16.5).
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ProductOperand {
    /// A binary variable operand (must be a true binary variable).
    Binary(VarId),
    /// A linear scalar-function operand (must be bounded at compile time).
    Linear(LinExpr),
}

/// The exact semantic payload of a binary product construct (design §7, §16.5).
///
/// The operand combination is validated by the builder: exactly one of
/// Binary×Binary, Binary×Linear, or Linear×Binary is accepted; two continuous
/// operands are a typed rejection (SM-12.7). `output` is the variable holding
/// the product result, created by the builder and stored here so the construct
/// is self-contained with a top-level construct origin.
#[derive(Clone, Debug, PartialEq)]
pub struct BinaryProductConstraint {
    /// The left product operand.
    pub left: ProductOperand,
    /// The right product operand.
    pub right: ProductOperand,
    /// The variable holding the product result (created by the builder).
    pub output: VarId,
}

impl BinaryProductConstraint {
    /// Derive the parameter dependencies across any linear operand (F1).
    pub fn parameter_dependencies(&self) -> Vec<ParamId> {
        let mut deps: Vec<ParamId> = Vec::new();
        for operand in [&self.left, &self.right] {
            if let ProductOperand::Linear(expr) = operand {
                for p in expr.parameter_dependencies() {
                    if !deps.contains(&p) {
                        deps.push(p);
                    }
                }
            }
        }
        deps
    }
}
