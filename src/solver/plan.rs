//! Solve plan and warm-start types (design §11.4, §12; SM-07.1, SM-08).
//!
//! [`SolvePlan`] is the explicit solve-attempt contract: it combines solve
//! options, a reversible solve overlay, candidate MIP starts, variable hints,
//! an optional objective-policy override, a lexicographic-stage continuation
//! policy, and the unsupported-feature policy (SM-07.1). Starts
//! ([`MipStart`]) and hints ([`VariableHints`]) are distinct from persistent
//! fixings, solution locks, and the LP-basis artifact (D8, SM-08.6); ROML
//! never silently converts among them (design §11.2).
//!
//! Unsupported behavior rejects by default ([`UnsupportedFeaturePolicy::Reject`],
//! SM-08.4); any conversion is an explicit policy variant whose application is
//! recorded in the effective solve metadata (SM-08.5).
//!
//! [`SolvePlan::validate`] gates an invalid plan BEFORE any backend mutation:
//! assignment lineage/stale/value failures, duplicate start variables,
//! start/overlay conflicts, non-finite hint values, and incomplete
//! `RejectIncomplete` starts are all typed [`PlanError`]s (design §19).

use std::collections::BTreeMap;

use crate::assignment::{AssignmentError, PrimalAssignment};
use crate::identity::IdentityOverflow;
use crate::model::{Model, Objective, VarType};
use crate::solver::options::SolveOptions;
use crate::solver::overlay::SolveOverlay;
use crate::Variable;

/// The complete solve intent for one solve attempt (design §12, SM-07.1).
#[derive(Clone, Debug, PartialEq)]
pub struct SolvePlan {
    /// Solve options (solver policy) for this attempt.
    pub options: SolveOptions,
    /// Solve-scoped reversible restrictions (design §12; SM-07.3).
    pub overlay: SolveOverlay,
    /// Candidate MIP warm starts (design §11.4; SM-08.1).
    pub mip_starts: Vec<MipStart>,
    /// Independent variable hints (design §11.4; SM-08.3).
    pub hints: VariableHints,
    /// Objective-policy override, if any. P28 forward-declares `Single`;
    /// P31 owns the full `Weighted`/`Lexicographic` semantics.
    pub objective_override: Option<ObjectivePolicy>,
    /// Lexicographic-stage continuation policy (design §15.2). P31 executes
    /// stages; P28 declares the policy.
    pub lex_stage_policy: LexStagePolicy,
    /// What to do when a requested feature is not qualified by the backend
    /// (SM-08.4 default-reject; SM-08.5 explicit recorded conversions).
    pub unsupported: UnsupportedFeaturePolicy,
}

impl SolvePlan {
    /// Build an empty plan for `options`: an empty overlay, no starts or
    /// hints, no objective override, the default stage policy, and default
    /// rejection of unsupported features.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityOverflow`] when the empty overlay's identity cannot
    /// be allocated (counter exhausted).
    pub fn new(options: SolveOptions) -> Result<Self, IdentityOverflow> {
        Ok(Self {
            options,
            overlay: SolveOverlay::new(BTreeMap::new(), Vec::new(), Vec::new(), Vec::new())?,
            mip_starts: Vec::new(),
            hints: VariableHints::default(),
            objective_override: None,
            lex_stage_policy: LexStagePolicy::RequireOptimal,
            unsupported: UnsupportedFeaturePolicy::Reject,
        })
    }

    /// Validate this plan against `model` BEFORE any backend mutation
    /// (SM-08.4, design §19 "solve-plan conflict" family).
    ///
    /// Rejects, with a typed [`PlanError`]:
    /// - every start's assignment failing [`PrimalAssignment::validate_for`]
    ///   (lineage mismatch, stale variable, non-finite or out-of-bounds
    ///   value);
    /// - a variable assigned by two different starts
    ///   ([`DuplicateStartVariable`](PlanError::DuplicateStartVariable));
    /// - a variable assigned by a start AND present in the overlay's
    ///   temporary fixings ([`OverlayConflict`](PlanError::OverlayConflict));
    /// - a non-finite hint value
    ///   ([`NonFiniteHintValue`](PlanError::NonFiniteHintValue));
    /// - a [`RepairPolicy::RejectIncomplete`] start that omits an active
    ///   integer/binary variable
    ///   ([`IncompleteStart`](PlanError::IncompleteStart)).
    ///
    /// Validation is read-only on `model` and `self`: no backend call is
    /// reachable with an invalid plan.
    pub fn validate(&self, model: &Model) -> Result<(), PlanError> {
        // All active integer/binary variables, for the RejectIncomplete
        // completeness check. `Model::take_snapshot` is read-only and
        // infallible in practice (its `Result` is reserved for future
        // identity overflow).
        let snapshot = model
            .take_snapshot()
            .map_err(|_| PlanError::UnsupportedFeature {
                feature: "model snapshot",
                policy: self.unsupported,
            })?;
        let integer_binary: Vec<Variable> = snapshot
            .variables
            .iter()
            .filter(|v| v.active && matches!(v.var_type, VarType::Integer | VarType::Binary))
            .map(|v| v.id)
            .collect();

        let mut seen: BTreeMap<Variable, usize> = BTreeMap::new();
        for (index, start) in self.mip_starts.iter().enumerate() {
            start
                .assignment
                .validate_for(model)
                .map_err(PlanError::Assignment)?;
            for variable in start.assignment.values.keys() {
                if let Some(_prior) = seen.insert(*variable, index) {
                    return Err(PlanError::DuplicateStartVariable {
                        variable: *variable,
                    });
                }
                if self.overlay.temporary_fixings.contains_key(variable) {
                    return Err(PlanError::OverlayConflict {
                        variable: *variable,
                    });
                }
            }
            if start.repair == RepairPolicy::RejectIncomplete {
                let missing: Vec<Variable> = integer_binary
                    .iter()
                    .copied()
                    .filter(|v| !start.assignment.values.contains_key(v))
                    .collect();
                if !missing.is_empty() {
                    return Err(PlanError::IncompleteStart { missing });
                }
            }
        }

        for (variable, hint) in self.hints.iter() {
            if !hint.value.is_finite() {
                return Err(PlanError::NonFiniteHintValue {
                    variable: *variable,
                    value: hint.value,
                });
            }
            // Entity check (review P2-02): a hint keyed by a stale or foreign
            // variable would fail only LATER inside a conversion or the
            // backend application, after the model was committed — the packet
            // requires entity validation before any backend mutation.
            if model.variable_domain(*variable).is_none() {
                return Err(PlanError::Assignment(AssignmentError::StaleVariable {
                    variable: *variable,
                }));
            }
        }

        Ok(())
    }
}

/// A candidate MIP warm start (design §11.4, SM-08.1).
///
/// A coherent candidate incumbent — possibly partial and repairable — carried
/// by a [`PrimalAssignment`]. Distinct from a persistent
/// [`VariableFixing`](crate::VariableFixing), a solve-scoped solution lock /
/// temporary fixing, a variable hint, and the LP-basis artifact (D8,
/// SM-08.6).
#[derive(Clone, Debug, PartialEq)]
pub struct MipStart {
    /// The assignment of variable values to seed the solve.
    pub assignment: PrimalAssignment,
    /// How the backend may repair an incomplete/conflicting start.
    pub repair: RepairPolicy,
    /// Optional human-readable start name.
    pub name: Option<String>,
}

impl MipStart {
    /// Build an unnamed start for `assignment` with the given `repair` policy.
    pub fn new(assignment: PrimalAssignment, repair: RepairPolicy) -> Self {
        Self {
            assignment,
            repair,
            name: None,
        }
    }
}

/// How a [`MipStart`]'s incompleteness or conflict is handled (SM-08.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepairPolicy {
    /// Let the backend apply its default repair behavior.
    BackendDefault,
    /// Reject the start (typed error) unless every active integer/binary
    /// variable is assigned.
    RejectIncomplete,
    /// Allow the backend to repair the incomplete start.
    AllowRepair,
}

/// Independent variable hints that guide the MIP search (design §11.4,
/// SM-08.3).
///
/// A pure data record of independent value/priority entries. Hints NEVER claim
/// to change the feasible region — they are search guidance only. Each hint is
/// keyed by [`Variable`]; `VariableHint` carries the target value and a
/// [`HintPriority`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VariableHints {
    entries: BTreeMap<Variable, VariableHint>,
}

impl VariableHints {
    /// The hint for `variable`, if any.
    pub fn get(&self, variable: Variable) -> Option<&VariableHint> {
        self.entries.get(&variable)
    }

    /// Insert or replace the hint for `variable`, returning the previous
    /// hint, if any.
    pub fn insert(&mut self, variable: Variable, hint: VariableHint) -> Option<VariableHint> {
        self.entries.insert(variable, hint)
    }

    /// Iterate over `(variable, hint)` entries in ascending variable order.
    pub fn iter(&self) -> impl Iterator<Item = (&Variable, &VariableHint)> {
        self.entries.iter()
    }

    /// Whether there are no hints.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The number of hint entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// One independent hint value/priority pair (SM-08.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VariableHint {
    /// The target value for the hinted variable.
    pub value: f64,
    /// The hint priority (higher = stronger guidance).
    pub priority: HintPriority,
}

/// A hint priority (search guidance strength).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HintPriority(pub i32);

/// Policy for a requested feature the backend does not qualify (SM-08.4,
/// SM-08.5).
///
/// The default is rejection: an unqualified start/hint request returns a typed
/// error rather than being silently ignored or simulated. The conversion
/// variants are explicit and, when applied, recorded in the effective solve
/// metadata — never silent (SM-08.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UnsupportedFeaturePolicy {
    /// Reject the request with a typed error.
    #[default]
    Reject,
    /// Convert a variable-hints request into a [`MipStart`] when the backend
    /// qualifies starts but not hints.
    ConvertHintToStart,
    /// Convert a [`MipStart`] into overlay temporary fixings when the backend
    /// qualifies neither starts nor hints.
    ConvertStartToTemporaryFixing,
}

/// Objective policy for one solve (design §15; SM-07.1).
///
/// P28 forward-declares the `Single` variant only; P31 owns the full
/// `Weighted`/`Lexicographic` semantics in `src/objective_policy.rs`. The
/// `#[non_exhaustive]` boundary keeps the P31 extension a non-breaking change
/// (A30/A32 extension-surface precedent).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ObjectivePolicy {
    /// Solve for a single objective.
    Single(Objective),
}

/// Lexicographic-stage continuation policy (design §15.2; SM-07.7).
///
/// P28 declares the policy; P31 executes the sequential stages and records the
/// per-stage results in `EffectiveSolvePlan`'s `objective_stages`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LexStagePolicy {
    /// A stage must prove optimality before descending.
    RequireOptimal,
    /// Continue with the best feasible stage result, recording the
    /// qualification.
    UseBestFeasible,
}

/// Error validating a [`SolvePlan`] against a model (design §19, SM-08.4).
#[derive(Clone, Debug, PartialEq)]
pub enum PlanError {
    /// A start's assignment failed model validation (lineage mismatch, stale
    /// variable, non-finite or out-of-bounds value).
    Assignment(AssignmentError),
    /// The same variable is assigned by two different starts.
    DuplicateStartVariable {
        /// The duplicated variable.
        variable: Variable,
    },
    /// A variable is assigned by a start AND present in the overlay's
    /// temporary fixings.
    OverlayConflict {
        /// The conflicting variable.
        variable: Variable,
    },
    /// A hint value is not finite (NaN or ±inf) — it must never reach a
    /// native solver (CR-01).
    NonFiniteHintValue {
        /// The affected variable.
        variable: Variable,
        /// The non-finite hint value.
        value: f64,
    },
    /// A [`RepairPolicy::RejectIncomplete`] start omits an active
    /// integer/binary variable.
    IncompleteStart {
        /// The active integer/binary variables the start omits.
        missing: Vec<Variable>,
    },
    /// A requested feature is not qualified by the backend and the plan's
    /// policy is default rejection (or the request cannot be satisfied by the
    /// declared policy).
    UnsupportedFeature {
        /// The unqualified feature name.
        feature: &'static str,
        /// The plan's unsupported-feature policy at the time of rejection.
        policy: UnsupportedFeaturePolicy,
    },
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::Assignment(e) => write!(f, "solve-plan assignment error: {e:?}"),
            PlanError::DuplicateStartVariable { variable } => {
                write!(f, "solve-plan duplicate start variable {variable:?}")
            }
            PlanError::OverlayConflict { variable } => write!(
                f,
                "solve-plan conflict: variable {variable:?} is both a start value and an \
                 overlay temporary fixing"
            ),
            PlanError::NonFiniteHintValue { variable, value } => {
                write!(
                    f,
                    "solve-plan non-finite hint value {value} for {variable:?}"
                )
            }
            PlanError::IncompleteStart { missing } => write!(
                f,
                "solve-plan incomplete start: missing {} variable(s)",
                missing.len()
            ),
            PlanError::UnsupportedFeature { feature, policy } => write!(
                f,
                "solve-plan unsupported feature '{feature}' under policy {policy:?}"
            ),
        }
    }
}

impl std::error::Error for PlanError {}

impl From<AssignmentError> for PlanError {
    fn from(e: AssignmentError) -> Self {
        PlanError::Assignment(e)
    }
}
