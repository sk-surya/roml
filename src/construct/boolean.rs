//! Boolean construct payload (design §7, §16.4).
//!
//! Implication, equivalence, any (at-least-one), and all (all-ones) over binary
//! variables, through exact linear rows. The payload stores the exact semantic
//! relation only; the per-construct formulation preference lives in the
//! [`ConstructEntry`](crate::construct::ConstructEntry) (A29).

use crate::id::VarId;

/// The Boolean relation being constrained (design §16.4).
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum BooleanKind {
    /// `antecedent ⇒ consequent`.
    Implication {
        /// The antecedent binary variable.
        antecedent: VarId,
        /// The consequent binary variable.
        consequent: VarId,
    },
    /// `left ⟺ right`.
    Equivalence {
        /// The left binary variable.
        left: VarId,
        /// The right binary variable.
        right: VarId,
    },
    /// At least one of the variables is `1`.
    Any {
        /// The binary variables (non-empty, all binary).
        variables: Vec<VarId>,
    },
    /// All of the variables are `1`.
    All {
        /// The binary variables (non-empty, all binary).
        variables: Vec<VarId>,
    },
}

/// The exact semantic payload of a Boolean construct (design §7, §16.4).
#[derive(Clone, Debug, PartialEq)]
pub struct BooleanConstraint {
    /// The Boolean relation.
    pub kind: BooleanKind,
}
