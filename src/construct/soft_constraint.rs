//! Canonical persistent soft-constraint semantics (P30).
//!
//! A soft constraint is a canonical construct attached to one existing
//! primitive constraint.  The construct records the two semantic violation
//! roles even when the corresponding side is unbounded; compilation decides
//! which finite sides emit rows.  This keeps the roles stable across
//! revisions and prevents temporary solve overlays from being mistaken for
//! persistent softening.

use crate::id::{ConId, ParamId};
use crate::identity::ConstructId;
use crate::model::Objective;
use crate::value_expr::ValueExpr;

/// Stable handle for one persistent soft constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SoftConstraint {
    construct: ConstructId,
    original_constraint: ConId,
}

impl SoftConstraint {
    /// Return the canonical construct identity backing this soft constraint.
    pub const fn construct(self) -> ConstructId {
        self.construct
    }

    /// Return the original primitive constraint being softened.
    pub const fn original_constraint(self) -> ConId {
        self.original_constraint
    }

    /// Return the stable lower-side violation role.
    pub const fn lower_violation(self) -> ViolationRole {
        ViolationRole {
            soft_constraint: self.construct,
            side: ViolationSide::Lower,
        }
    }

    /// Return the stable upper-side violation role.
    pub const fn upper_violation(self) -> ViolationRole {
        ViolationRole {
            soft_constraint: self.construct,
            side: ViolationSide::Upper,
        }
    }

    pub(crate) const fn new(construct: ConstructId, original_constraint: ConId) -> Self {
        Self {
            construct,
            original_constraint,
        }
    }
}

/// Which side of the original constraint a generated violation represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ViolationSide {
    /// `f(x) + v_lo >= l`.
    Lower,
    /// `f(x) - v_up <= u`.
    Upper,
}

/// Stable role metadata for one generated violation variable/row pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViolationRole {
    soft_constraint: ConstructId,
    side: ViolationSide,
}

impl ViolationRole {
    /// Return the owning persistent soft-constraint construct.
    pub const fn soft_constraint(self) -> ConstructId {
        self.soft_constraint
    }

    /// Return the side represented by this role.
    pub const fn side(self) -> ViolationSide {
        self.side
    }
}

/// Validation policy for generated violation variables.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViolationPolicy {
    /// Optional finite non-negative upper bound for each generated violation.
    pub max_violation: Option<f64>,
}

/// Where a persistent violation penalty is placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PenaltyTarget {
    /// Do not add a persistent penalty objective term.
    None,
    /// Add the penalty to this canonical objective.
    Objective(Objective),
}

/// Penalty weight and target for persistent softening.
#[derive(Clone, Debug, PartialEq)]
pub struct PenaltyPolicy {
    /// A finite non-negative weight expression.
    pub weight: ValueExpr,
    /// The canonical target receiving the weighted violation.
    pub target: PenaltyTarget,
}

impl Default for PenaltyPolicy {
    fn default() -> Self {
        Self {
            weight: ValueExpr::constant(1.0),
            target: PenaltyTarget::None,
        }
    }
}

/// Canonical payload stored in the construct arena.
#[derive(Clone, Debug, PartialEq)]
pub struct SoftConstraintConstraint {
    /// Stable handle for the owning soft constraint.
    pub handle: SoftConstraint,
    /// Original primitive constraint.
    pub original_constraint: ConId,
    /// Violation cap policy.
    pub violation: ViolationPolicy,
    /// Persistent penalty policy.
    pub penalty: PenaltyPolicy,
}

impl SoftConstraintConstraint {
    /// Return the stable lower-side role.
    pub const fn lower_role(&self) -> ViolationRole {
        self.handle.lower_violation()
    }

    /// Return the stable upper-side role.
    pub const fn upper_role(&self) -> ViolationRole {
        self.handle.upper_violation()
    }

    /// Derive parameter dependencies from the persistent penalty weight.
    pub fn parameter_dependencies(&self) -> Vec<ParamId> {
        let mut dependencies: Vec<_> = self.penalty.weight.dependencies().into_iter().collect();
        dependencies.sort();
        dependencies
    }
}
