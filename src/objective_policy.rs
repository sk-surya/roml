//! Canonical objective-policy and priority model (P31, design §15).
//!
//! P31 is the sole owner of objective-policy and objective-priority
//! semantics. This module defines the frozen canonical data model plus its
//! atomic validation. The portable weighted/lexicographic executor, stage
//! locks, and P30 priority-penalty integration are built on these types by
//! later P31 tasks.
//!
//! # Semantics
//!
//! * `ObjectivePriority(0)` is the highest priority; levels execute in
//!   ascending numeric order.
//! * M3 objective weights are plain, finite, nonnegative `f64`.
//!   Parameterized objective weights are out of scope for M3 and are
//!   rejected, never represented by a second expression type.
//! * Weighted normalization: a `MIN` objective contributes `+w*f(x)` and a
//!   `MAX` objective contributes `-w*f(x)` to a scalar stage that is always
//!   minimized.
//! * Stage locks use the canonical `|z*|` scale: `delta = abs_tol + rel_tol *
//!   |z*|`.

use crate::compiler::backend_ir::CompilationId;
use crate::model::Objective;
use crate::solution::Solution;
use crate::solver::SolveStatus;

/// A lexicographic priority level. `ObjectivePriority(0)` is the highest
/// priority; levels execute in ascending numeric order (SM-11.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectivePriority(u32);

impl ObjectivePriority {
    /// Construct a priority level.
    pub const fn new(level: u32) -> Self {
        Self(level)
    }

    /// The numeric priority level (ascending order of execution).
    pub const fn level(self) -> u32 {
        self.0
    }
}

/// One weighted objective term in a weighted or lexicographic policy
/// (design §15.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedObjective {
    /// The referenced objective.
    pub objective: Objective,
    /// Finite nonnegative weight (`MIN` contributes `+w*f`, `MAX` `-w*f`).
    pub weight: f64,
}

/// One lexicographic priority level: one or more objectives whose weighted
/// combination is minimized, plus degradation tolerances (SM-11.2).
#[derive(Clone, Debug, PartialEq)]
pub struct WeightedObjectiveLevel {
    /// The priority of this level.
    pub priority: ObjectivePriority,
    /// Objectives combined at this level.
    pub objectives: Vec<WeightedObjective>,
    /// Absolute degradation tolerance for the stage lock.
    pub absolute_tolerance: f64,
    /// Relative degradation tolerance (multiplied by `|z*|`).
    pub relative_tolerance: f64,
}

/// A single-stage weighted objective policy (design §15.1).
#[derive(Clone, Debug, PartialEq)]
pub struct WeightedObjectives {
    /// Weighted objectives combined into one scalar stage.
    pub objectives: Vec<WeightedObjective>,
}

/// A lexicographic objective policy: an ordered list of weighted levels
/// (design §15.2).
#[derive(Clone, Debug, PartialEq)]
pub struct LexicographicObjectives {
    /// Levels ordered by ascending priority.
    pub levels: Vec<WeightedObjectiveLevel>,
}

/// Canonical objective policy for one model/solve (design §15; SM-11.1).
///
/// P28 forward-declared the [`Single`](Self::Single) variant; P31 adds
/// [`None`](Self::None), [`Weighted`](Self::Weighted), and
/// [`Lexicographic`](Self::Lexicographic). The `#[non_exhaustive]` boundary
/// keeps future extension non-breaking. `Objective` is the model objective
/// handle.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectivePolicy {
    /// No active objective.
    None,
    /// Solve for a single objective.
    Single(Objective),
    /// A single-stage weighted combination.
    Weighted(WeightedObjectives),
    /// A lexicographic priority list of weighted levels.
    Lexicographic(LexicographicObjectives),
}

impl ObjectivePolicy {
    /// Atomically validate the policy's structural and numeric contract.
    /// `objective_exists` may be `None` to defer stale-reference checking.
    pub fn validate(
        &self,
        objective_exists: Option<impl Fn(Objective) -> bool>,
    ) -> Result<(), ObjectivePolicyError> {
        let checker = objective_exists
            .as_ref()
            .map(|f| f as &dyn Fn(Objective) -> bool);
        match self {
            ObjectivePolicy::None => Ok(()),
            ObjectivePolicy::Single(_) => Ok(()),
            ObjectivePolicy::Weighted(w) => w.validate(checker),
            ObjectivePolicy::Lexicographic(l) => l.validate(checker),
        }
    }
}

/// Stage continuation semantics (design §15.2; SM-11.5, SM-11.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageContinuation {
    /// A stage must prove optimality before descending.
    RequireOptimal,
    /// Descend from a valid best-feasible stage, recording the qualification.
    BestFeasible,
}

/// Provider-selection policy for objective execution (design §15.3; SM-11.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ObjectiveProviderPolicy {
    /// Force the portable sequential executor.
    #[default]
    PortableOnly,
    /// Prefer a qualified native provider, otherwise fall back portably.
    PreferNative,
    /// Reject before synchronization when qualified native support is absent.
    NativeRequired,
}

/// Which provider actually executed a policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectiveExecutionProvider {
    /// Solver-neutral portable sequential executor.
    PortableSequential,
    /// A qualified native backend (reserved for a later qualified backend).
    Native {
        /// Backend family.
        backend: String,
        /// Qualified backend version.
        version: String,
    },
}

/// Classification of whether execution may descend to the next lexicographic
/// stage (design §15.2; SM-11.5, SM-11.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageContinuationDecision {
    /// The stage proved optimality; descend to the next level.
    ContinueOptimal,
    /// The stage produced a valid best-feasible incumbent; descend recording
    /// the qualification.
    ContinueBestFeasible,
    /// The stage did not prove optimality and continuation requires it.
    StopNotOptimal,
    /// The stage has no feasible point.
    StopNoFeasiblePoint,
    /// The outcome is unknown; do not descend.
    StopUnknown,
}

/// Exact canonical degradation lock for a solved normalized stage (design
/// §15.2; SM-11.2).
///
/// Because the normalized stage `g` is always minimized, the lock is
/// canonically `g(x) <= normalized_upper_bound` with
/// `delta = absolute_tolerance + relative_tolerance * relative_scale` and
/// `relative_scale = |z*|`. At `z* = 0` the relative tolerance contributes
/// zero; negative values use positive magnitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectiveLockReport {
    /// The priority of the stage that produced this lock.
    pub priority: ObjectivePriority,
    /// The solved normalized scalar stage value (`z*`).
    pub reference_value: f64,
    /// Absolute degradation tolerance.
    pub absolute_tolerance: f64,
    /// Relative degradation tolerance (scales with `|z*|`).
    pub relative_tolerance: f64,
    /// `|z*|`; the relative-tolerance scale.
    pub relative_scale: f64,
    /// `absolute_tolerance + relative_tolerance * relative_scale`.
    pub allowed_degradation: f64,
    /// `z* + allowed_degradation`; the canonical `g(x) <=` lock bound.
    pub normalized_upper_bound: f64,
}

impl ObjectiveLockReport {
    /// Compute the exact canonical lock for a solved normalized stage value.
    ///
    /// # Panics
    ///
    /// Panics on a non-finite `z*` or non-finite/negative tolerances; these
    /// are rejected by policy validation before execution.
    pub fn from_stage(
        priority: ObjectivePriority,
        stage_value: f64,
        absolute_tolerance: f64,
        relative_tolerance: f64,
    ) -> Self {
        assert!(stage_value.is_finite(), "stage value must be finite");
        assert!(absolute_tolerance.is_finite() && absolute_tolerance >= 0.0);
        assert!(relative_tolerance.is_finite() && relative_tolerance >= 0.0);
        let relative_scale = stage_value.abs();
        let allowed_degradation = absolute_tolerance + relative_tolerance * relative_scale;
        let normalized_upper_bound = stage_value + allowed_degradation;
        Self {
            priority,
            reference_value: stage_value,
            absolute_tolerance,
            relative_tolerance,
            relative_scale,
            allowed_degradation,
            normalized_upper_bound,
        }
    }
}

/// Value of one objective at a stage's final point (design §15.4; SM-11.7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectiveValue {
    /// The objective whose value is reported.
    pub objective: Objective,
    /// The objective's evaluated value at the final point.
    pub value: f64,
}

/// One executed objective stage (design §15.2, §15.4; SM-11.7).
///
/// P28 declared the placeholder stage record; P31 supersedes it with this
/// richer, mandatory stage result. Every field is populated honestly by the
/// portable executor; determinants never overstate a `BestFeasible` outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectiveStageResult {
    /// The priority of this stage.
    pub priority: ObjectivePriority,
    /// The stage termination status.
    pub status: SolveStatus,
    /// Whether execution may descend to the next level.
    pub continuation: StageContinuationDecision,
    /// Every referenced objective's value at the stage's final point.
    pub objective_values: Vec<ObjectiveValue>,
    /// The normalized scalar stage value (`z*`), if the solve produced one.
    pub scalar_stage_value: Option<f64>,
    /// The exact degradation lock emitted for this stage, if a valid feasible
    /// outcome allowed descent.
    pub lock: Option<ObjectiveLockReport>,
    /// Which provider executed this stage.
    pub provider: ObjectiveExecutionProvider,
    /// The exact overlay-compounded compilation id the stage solved against.
    pub compilation_id: CompilationId,
}

/// Result of executing a weighted or lexicographic objective policy.
#[derive(Clone, Debug, PartialEq)]
pub struct MultiObjectiveResult {
    /// The final solution after all executed stages.
    pub final_solution: Solution,
    /// Per-stage results in ascending priority order.
    pub stages: Vec<ObjectiveStageResult>,
    /// Which provider executed the policy.
    pub provider: ObjectiveExecutionProvider,
}

/// Result of validating an objective policy against a model or the frozen
/// numeric contract (design §19, SM-08.4).
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectivePolicyError {
    /// A referenced objective is stale or does not exist.
    ObjectiveNotFound {
        /// The offending objective id.
        objective: Objective,
    },
    /// A weight is not finite or is negative.
    InvalidWeight {
        /// The offending objective id.
        objective: Objective,
        /// The invalid weight.
        weight: f64,
    },
    /// A tolerance is not finite or is negative.
    InvalidTolerance {
        /// The priority level.
        priority: ObjectivePriority,
        /// Field that failed validation.
        field: &'static str,
        /// The invalid value.
        value: f64,
    },
    /// The same objective is repeated within one weighted level.
    DuplicateObjectiveInLevel {
        /// The duplicated objective id.
        objective: Objective,
        /// The priority level.
        priority: ObjectivePriority,
    },
    /// The same priority is declared on two different levels.
    DuplicatePriority {
        /// The duplicated priority.
        priority: ObjectivePriority,
    },
    /// A weighted level or weighted policy has no objectives.
    EmptyLevel,
    /// A lexicographic policy declares no levels.
    NoLevels,
}

impl std::fmt::Display for ObjectivePolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ObjectiveNotFound { objective } => {
                write!(
                    f,
                    "objective policy references stale objective {objective:?}"
                )
            }
            Self::InvalidWeight { objective, weight } => {
                write!(f, "objective {objective:?} has invalid weight {weight}")
            }
            Self::InvalidTolerance {
                priority,
                field,
                value,
            } => write!(
                f,
                "priority {priority:?} has invalid {field} tolerance {value}"
            ),
            Self::DuplicateObjectiveInLevel {
                objective,
                priority,
            } => write!(
                f,
                "objective {objective:?} duplicated in priority {priority:?}"
            ),
            Self::DuplicatePriority { priority } => {
                write!(f, "priority {priority:?} declared on multiple levels")
            }
            Self::EmptyLevel => write!(f, "a weighted level contains no objectives"),
            Self::NoLevels => write!(f, "a lexicographic policy contains no levels"),
        }
    }
}

impl std::error::Error for ObjectivePolicyError {}

impl WeightedObjective {
    /// Validate the frozen numeric contract: finite, nonnegative weight.
    pub fn validate(&self) -> Result<(), ObjectivePolicyError> {
        validate_weight(self.objective, self.weight)
    }
}

impl WeightedObjectives {
    /// Atomically validate a weighted policy: non-empty, finite nonnegative
    /// weights, no duplicated objective within the level. `objective_exists`
    /// may be `None` to defer stale-reference checking to model binding.
    pub fn validate(
        &self,
        objective_exists: Option<impl Fn(Objective) -> bool>,
    ) -> Result<(), ObjectivePolicyError> {
        validate_level(
            &self.objectives,
            ObjectivePriority::new(0),
            objective_exists
                .as_ref()
                .map(|f| f as &dyn Fn(Objective) -> bool),
        )
    }
}

impl WeightedObjectiveLevel {
    /// Atomically validate a level: non-empty objectives, finite nonnegative
    /// weights, no duplicated objective, finite nonnegative tolerances.
    pub fn validate(
        &self,
        objective_exists: Option<impl Fn(Objective) -> bool>,
    ) -> Result<(), ObjectivePolicyError> {
        validate_level(
            &self.objectives,
            self.priority,
            objective_exists
                .as_ref()
                .map(|f| f as &dyn Fn(Objective) -> bool),
        )?;
        for (field, value) in [
            ("absolute", self.absolute_tolerance),
            ("relative", self.relative_tolerance),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(ObjectivePolicyError::InvalidTolerance {
                    priority: self.priority,
                    field,
                    value,
                });
            }
        }
        Ok(())
    }
}

impl LexicographicObjectives {
    /// Atomically validate a lexicographic policy: non-empty levels, each
    /// level valid, and no duplicated priority across levels.
    pub fn validate(
        &self,
        objective_exists: Option<impl Fn(Objective) -> bool>,
    ) -> Result<(), ObjectivePolicyError> {
        if self.levels.is_empty() {
            return Err(ObjectivePolicyError::NoLevels);
        }
        let mut seen = Vec::new();
        for level in &self.levels {
            level.validate(
                objective_exists
                    .as_ref()
                    .map(|f| f as &dyn Fn(Objective) -> bool),
            )?;
            let priority = level.priority;
            if seen.contains(&priority) {
                return Err(ObjectivePolicyError::DuplicatePriority { priority });
            }
            seen.push(priority);
        }
        Ok(())
    }
}

fn validate_weight(objective: Objective, weight: f64) -> Result<(), ObjectivePolicyError> {
    if !weight.is_finite() || weight < 0.0 {
        return Err(ObjectivePolicyError::InvalidWeight { objective, weight });
    }
    Ok(())
}

fn validate_level(
    objectives: &[WeightedObjective],
    priority: ObjectivePriority,
    objective_exists: Option<&dyn Fn(Objective) -> bool>,
) -> Result<(), ObjectivePolicyError> {
    if objectives.is_empty() {
        return Err(ObjectivePolicyError::EmptyLevel);
    }
    let mut seen = Vec::new();
    for wo in objectives {
        validate_weight(wo.objective, wo.weight)?;
        if seen.contains(&wo.objective) {
            return Err(ObjectivePolicyError::DuplicateObjectiveInLevel {
                objective: wo.objective,
                priority,
            });
        }
        if let Some(exists) = objective_exists {
            if !exists(wo.objective) {
                return Err(ObjectivePolicyError::ObjectiveNotFound {
                    objective: wo.objective,
                });
            }
        }
        seen.push(wo.objective);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Generation;

    fn obj(index: u32) -> Objective {
        Objective::new(index, Generation::new())
    }

    #[test]
    fn priority_orders_ascending() {
        let zero = ObjectivePriority::new(0);
        let one = ObjectivePriority::new(1);
        assert!(zero < one);
        assert_eq!(zero.level(), 0);
        assert_eq!(one.level(), 1);
    }

    #[test]
    fn weighted_objective_holds_finite_contract_fields() {
        let obj = obj(1);
        let wo = WeightedObjective {
            objective: obj,
            weight: 2.5,
        };
        assert_eq!(wo.objective, obj);
        assert_eq!(wo.weight, 2.5);
    }

    #[test]
    fn weighted_level_and_policy_shape() {
        let level = WeightedObjectiveLevel {
            priority: ObjectivePriority::new(0),
            objectives: vec![WeightedObjective {
                objective: obj(1),
                weight: 1.0,
            }],
            absolute_tolerance: 1e-6,
            relative_tolerance: 1e-9,
        };
        let policy = LexicographicObjectives {
            levels: vec![level],
        };
        assert_eq!(policy.levels.len(), 1);
        assert_eq!(policy.levels[0].priority, ObjectivePriority::new(0));
    }

    #[test]
    fn weighted_level_rejects_duplicate_objective() {
        let level = WeightedObjectiveLevel {
            priority: ObjectivePriority::new(0),
            objectives: vec![
                WeightedObjective {
                    objective: obj(1),
                    weight: 1.0,
                },
                WeightedObjective {
                    objective: obj(1),
                    weight: 2.0,
                },
            ],
            absolute_tolerance: 1e-6,
            relative_tolerance: 1e-9,
        };
        assert!(matches!(
            level.validate(None::<fn(Objective) -> bool>),
            Err(ObjectivePolicyError::DuplicateObjectiveInLevel { .. })
        ));
    }

    #[test]
    fn lexicographic_rejects_duplicate_priority() {
        let level = |priority: u32| WeightedObjectiveLevel {
            priority: ObjectivePriority::new(priority),
            objectives: vec![WeightedObjective {
                objective: obj(priority + 1),
                weight: 1.0,
            }],
            absolute_tolerance: 1e-6,
            relative_tolerance: 1e-9,
        };
        let policy = LexicographicObjectives {
            levels: vec![level(0), level(0)],
        };
        assert!(matches!(
            policy.validate(None::<fn(Objective) -> bool>),
            Err(ObjectivePolicyError::DuplicatePriority { .. })
        ));
    }

    #[test]
    fn empty_level_and_policy_rejected() {
        assert!(matches!(
            WeightedObjectives {
                objectives: Vec::new(),
            }
            .validate(None::<fn(Objective) -> bool>),
            Err(ObjectivePolicyError::EmptyLevel)
        ));
        assert!(matches!(
            LexicographicObjectives { levels: Vec::new() }.validate(None::<fn(Objective) -> bool>),
            Err(ObjectivePolicyError::NoLevels)
        ));
    }

    #[test]
    fn invalid_weight_and_tolerance_rejected() {
        let bad_weight = WeightedObjectives {
            objectives: vec![WeightedObjective {
                objective: obj(1),
                weight: -1.0,
            }],
        };
        assert!(matches!(
            bad_weight.validate(None::<fn(Objective) -> bool>),
            Err(ObjectivePolicyError::InvalidWeight { .. })
        ));
        let bad_tol = WeightedObjectiveLevel {
            priority: ObjectivePriority::new(0),
            objectives: vec![WeightedObjective {
                objective: obj(1),
                weight: 1.0,
            }],
            absolute_tolerance: -0.5,
            relative_tolerance: 1e-9,
        };
        assert!(matches!(
            bad_tol.validate(None::<fn(Objective) -> bool>),
            Err(ObjectivePolicyError::InvalidTolerance { .. })
        ));
    }

    #[test]
    fn stale_objective_rejected_when_checked() {
        let policy = WeightedObjectives {
            objectives: vec![WeightedObjective {
                objective: obj(7),
                weight: 1.0,
            }],
        };
        assert!(matches!(
            policy.validate(Some(|o: Objective| o.index() != 7)),
            Err(ObjectivePolicyError::ObjectiveNotFound { .. })
        ));
        // Without a checker the numeric/shape contract still passes.
        assert!(policy.validate(None::<fn(Objective) -> bool>).is_ok());
    }

    #[test]
    fn valid_two_level_policy_accepts() {
        let policy = LexicographicObjectives {
            levels: vec![
                WeightedObjectiveLevel {
                    priority: ObjectivePriority::new(0),
                    objectives: vec![WeightedObjective {
                        objective: obj(1),
                        weight: 1.0,
                    }],
                    absolute_tolerance: 1e-6,
                    relative_tolerance: 1e-9,
                },
                WeightedObjectiveLevel {
                    priority: ObjectivePriority::new(1),
                    objectives: vec![WeightedObjective {
                        objective: obj(2),
                        weight: 2.0,
                    }],
                    absolute_tolerance: 0.0,
                    relative_tolerance: 1e-9,
                },
            ],
        };
        assert!(policy.validate(None::<fn(Objective) -> bool>).is_ok());
    }

    #[test]
    fn objective_policy_variants_validate() {
        let none = ObjectivePolicy::None;
        assert!(none.validate(None::<fn(Objective) -> bool>).is_ok());

        let single = ObjectivePolicy::Single(obj(1));
        assert!(single.validate(None::<fn(Objective) -> bool>).is_ok());

        let weighted = ObjectivePolicy::Weighted(WeightedObjectives {
            objectives: vec![WeightedObjective {
                objective: obj(1),
                weight: 1.0,
            }],
        });
        assert!(weighted.validate(None::<fn(Objective) -> bool>).is_ok());

        let lex = ObjectivePolicy::Lexicographic(LexicographicObjectives {
            levels: vec![WeightedObjectiveLevel {
                priority: ObjectivePriority::new(0),
                objectives: vec![WeightedObjective {
                    objective: obj(1),
                    weight: 1.0,
                }],
                absolute_tolerance: 1e-6,
                relative_tolerance: 1e-9,
            }],
        });
        assert!(lex.validate(None::<fn(Objective) -> bool>).is_ok());
    }

    #[test]
    fn exact_lock_math_across_signs_and_tolerances() {
        let priority = ObjectivePriority::new(0);

        // z* > 0 with abs+rel tolerance.
        let pos = ObjectiveLockReport::from_stage(priority, 100.0, 1.0, 0.01);
        assert_eq!(pos.relative_scale, 100.0);
        assert_eq!(pos.allowed_degradation, 1.0 + 0.01 * 100.0);
        assert_eq!(pos.normalized_upper_bound, 100.0 + 1.0 + 1.0);

        // z* = 0: relative tolerance contributes zero; only abs applies.
        let zero = ObjectiveLockReport::from_stage(priority, 0.0, 0.5, 1e9);
        assert_eq!(zero.relative_scale, 0.0);
        assert_eq!(zero.allowed_degradation, 0.5);
        assert_eq!(zero.normalized_upper_bound, 0.5);

        // z* < 0 uses positive magnitude for the relative scale.
        let neg = ObjectiveLockReport::from_stage(priority, -50.0, 0.0, 0.02);
        assert_eq!(neg.relative_scale, 50.0);
        assert_eq!(neg.allowed_degradation, 1.0);
        assert_eq!(neg.normalized_upper_bound, -50.0 + 1.0);

        // Zero tolerances yield no degradation.
        let tight = ObjectiveLockReport::from_stage(priority, 10.0, 0.0, 0.0);
        assert_eq!(tight.allowed_degradation, 0.0);
        assert_eq!(tight.normalized_upper_bound, 10.0);
    }

    #[test]
    fn stage_and_multi_objective_result_shape() {
        use crate::solution::SolutionBuilder;
        use std::collections::HashMap;

        let priority = ObjectivePriority::new(0);
        let lock = ObjectiveLockReport::from_stage(priority, 2.0, 1e-6, 1e-9);
        let stage = ObjectiveStageResult {
            priority,
            status: crate::solver::SolveStatus::Optimal,
            continuation: StageContinuationDecision::ContinueOptimal,
            objective_values: vec![ObjectiveValue {
                objective: obj(1),
                value: 2.0,
            }],
            scalar_stage_value: Some(2.0),
            lock: Some(lock),
            provider: ObjectiveExecutionProvider::PortableSequential,
            compilation_id: crate::compiler::backend_ir::CompilationId::allocate().unwrap(),
        };
        let solution = SolutionBuilder::new()
            .status(crate::solver::SolveStatus::Optimal)
            .values(HashMap::new())
            .objective_value(2.0)
            .objective_id(obj(1))
            .build();
        let multi = MultiObjectiveResult {
            final_solution: solution,
            stages: vec![stage],
            provider: ObjectiveExecutionProvider::PortableSequential,
        };
        assert_eq!(multi.stages.len(), 1);
        assert_eq!(
            multi.stages[0].continuation,
            StageContinuationDecision::ContinueOptimal
        );
    }
}
