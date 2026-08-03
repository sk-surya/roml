//! Primal assignments and solution locks (design §11; D4, D8; SM-06).
//!
//! [`PrimalAssignment`] is a neutral partial value map with lineage plus
//! optional source instance/revision provenance. It makes no feasibility or
//! optimality claim (SM-06.1). [`SolutionLock`]/[`LockSelector`]/
//! [`ContinuousLock`] express solve-scoped feasible-region restrictions
//! (SM-06.3–06.5), distinct from persistent
//! [`VariableFixing`](crate::VariableFixing), MIP starts, and hints (D8).
//!
//! Compatibility is governed by lineage, live entity generation, and
//! value/domain compatibility ONLY ([`PrimalAssignment::validate_for`]);
//! instance/revision are provenance, never compatibility authority (D4,
//! SM-06.6).

use std::collections::{BTreeMap, BTreeSet};

use crate::identity::{ModelInstanceId, ModelLineageId};
use crate::model::{Bounds, Model, VarType};
use crate::revision::ModelRevision;

/// A partial assignment of values to model variables (design §11.1, SM-06.1).
///
/// A neutral partial value map with lineage plus optional source
/// instance/revision provenance. It makes **no feasibility or optimality
/// claim** — it is the raw material for solution locks, MIP starts, and hints
/// (D8), never a statement about solve status.
#[derive(Clone, Debug, PartialEq)]
pub struct PrimalAssignment {
    /// The model lineage this assignment was produced from (D4: the
    /// compatibility authority for reuse).
    pub lineage: ModelLineageId,
    /// The model instance this assignment was produced from. Provenance only —
    /// never a compatibility authority (D4).
    pub source_instance: Option<ModelInstanceId>,
    /// The model revision this assignment was produced from. Provenance only —
    /// never a compatibility authority (D4).
    pub source_revision: Option<ModelRevision>,
    /// The partial variable → value map.
    pub values: BTreeMap<crate::Variable, f64>,
}

impl PrimalAssignment {
    /// Validate this assignment against `model` (SM-02.2, SM-06.6).
    ///
    /// Reuse compatibility is governed by:
    ///
    /// 1. **lineage equality** (D4) — an assignment from an independent model
    ///    is never compatible, even with coincidentally equal generation-safe
    ///    handles;
    /// 2. **live entity generation** — a removed/stale-generation variable in
    ///    the assignment is a typed [`StaleVariable`](AssignmentError::StaleVariable);
    /// 3. **value/domain compatibility** — every assigned value lies inside
    ///    the variable's declared bounds, tolerance-aware for integrality on
    ///    integer/binary variables (a non-integral value is a typed
    ///    [`ValueOutOfBounds`](AssignmentError::ValueOutOfBounds)).
    ///
    /// Instance/revision are **provenance, not authority** (D4): two clones at
    /// the same revision with the same lineage both validate.
    pub fn validate_for(&self, model: &Model) -> Result<(), AssignmentError> {
        if self.lineage != model.lineage() {
            return Err(AssignmentError::LineageMismatch {
                expected: model.lineage(),
                actual: self.lineage,
            });
        }
        for (variable, value) in &self.values {
            let domain =
                model
                    .variable_domain(*variable)
                    .ok_or(AssignmentError::StaleVariable {
                        variable: *variable,
                    })?;
            let bounds = domain.bounds;
            if *value < bounds.lower || *value > bounds.upper {
                return Err(AssignmentError::ValueOutOfBounds {
                    variable: *variable,
                    value: *value,
                    bounds,
                });
            }
            if matches!(domain.var_type, VarType::Integer | VarType::Binary) {
                let nearest = value.round();
                if (*value - nearest).abs() > model.integrality_tolerance() {
                    return Err(AssignmentError::ValueOutOfBounds {
                        variable: *variable,
                        value: *value,
                        bounds,
                    });
                }
            }
        }
        Ok(())
    }

    /// Restrict this assignment's value map to the named variables (SM-06.2).
    ///
    /// Lineage and provenance are preserved; only the value map is filtered.
    pub fn subset(&self, variables: &[crate::Variable]) -> PrimalAssignment {
        PrimalAssignment {
            lineage: self.lineage,
            source_instance: self.source_instance,
            source_revision: self.source_revision,
            values: variables
                .iter()
                .filter_map(|v| self.values.get(v).map(|&value| (*v, value)))
                .collect(),
        }
    }

    /// The assigned value of `variable`, if any.
    pub fn value(&self, variable: crate::Variable) -> Option<f64> {
        self.values.get(&variable).copied()
    }
}

/// Error validating a [`PrimalAssignment`] against a model (design §19,
/// SM-06.6).
#[derive(Clone, Debug, PartialEq)]
pub enum AssignmentError {
    /// The assignment's lineage does not match the model's lineage (D4).
    LineageMismatch {
        /// The model's lineage.
        expected: ModelLineageId,
        /// The assignment's lineage.
        actual: ModelLineageId,
    },
    /// A variable in the assignment is stale: removed, or a dead generation.
    StaleVariable {
        /// The stale variable handle.
        variable: crate::Variable,
    },
    /// An assigned value lies outside the variable's declared domain
    /// (bounds, tolerance-aware for integrality).
    ValueOutOfBounds {
        /// The affected variable.
        variable: crate::Variable,
        /// The assigned value.
        value: f64,
        /// The variable's declared bounds.
        bounds: Bounds,
    },
}

/// A solution lock: a solve-scoped feasible-region restriction (design §11.3,
/// SM-06.3).
///
/// The lock applies its [`selector`](Self::selector) over the
/// [`assignment`](Self::assignment)'s values and restricts each selected
/// variable per [`continuous`](Self::continuous) (SM-06.4, SM-06.5). A lock is
/// distinct from a persistent [`VariableFixing`](crate::VariableFixing), a MIP
/// start, and a hint (D8) — ROML never silently converts among them.
#[derive(Clone, Debug, PartialEq)]
pub struct SolutionLock {
    /// The assignment whose selected values are locked.
    pub assignment: PrimalAssignment,
    /// Which of the assignment's variables the lock restricts.
    pub selector: LockSelector,
    /// How each selected variable is restricted (exact value or absolute band).
    pub continuous: ContinuousLock,
}

/// Which variables a [`SolutionLock`] restricts (SM-06.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LockSelector {
    /// All variables present in the assignment.
    AllAssigned,
    /// Only integer-typed variables present in the assignment.
    IntegerAssigned,
    /// Only binary-typed variables present in the assignment.
    BinaryAssigned,
    /// Exactly the named set (intersected with the assignment's values).
    Variables(BTreeSet<crate::Variable>),
    /// All assigned variables except the named set.
    Except(BTreeSet<crate::Variable>),
}

/// How a [`SolutionLock`] restricts each selected variable (SM-06.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContinuousLock {
    /// Fix each selected variable to its assigned value.
    Exact,
    /// Restrict each selected continuous variable to `[v - absolute, v + absolute]`.
    /// Valid for continuous variables only — an integer band cannot round-trip
    /// exactly (typed error).
    Within {
        /// The absolute band half-width.
        absolute: f64,
    },
}

impl SolutionLock {
    /// Resolve the `(variable, value)` pairs this lock selects over the
    /// assignment's values (SM-06.4).
    ///
    /// The assignment is validated against `model` first (lineage + live
    /// generation + value/domain, SM-06.6) so a stale or unrelated assignment
    /// fails **before** any lock op can be produced. Selector resolution is
    /// deterministic: the result is sorted by variable (dense compiled order).
    pub(crate) fn resolve(
        &self,
        model: &Model,
    ) -> Result<Vec<(crate::Variable, f64)>, AssignmentError> {
        self.assignment.validate_for(model)?;
        let mut selected: Vec<crate::Variable> = Vec::new();
        match &self.selector {
            LockSelector::AllAssigned => selected.extend(self.assignment.values.keys().copied()),
            LockSelector::IntegerAssigned => {
                for var in self.assignment.values.keys() {
                    if matches!(
                        model.variable_domain(*var).map(|d| d.var_type),
                        Some(VarType::Integer)
                    ) {
                        selected.push(*var);
                    }
                }
            }
            LockSelector::BinaryAssigned => {
                for var in self.assignment.values.keys() {
                    if matches!(
                        model.variable_domain(*var).map(|d| d.var_type),
                        Some(VarType::Binary)
                    ) {
                        selected.push(*var);
                    }
                }
            }
            LockSelector::Variables(set) => {
                for var in set {
                    if self.assignment.values.contains_key(var) {
                        selected.push(*var);
                    }
                }
            }
            LockSelector::Except(set) => {
                for var in self.assignment.values.keys() {
                    if !set.contains(var) {
                        selected.push(*var);
                    }
                }
            }
        }
        selected.sort_unstable();
        selected.dedup();
        Ok(selected
            .into_iter()
            .map(|var| (var, self.assignment.values[&var]))
            .collect())
    }
}
