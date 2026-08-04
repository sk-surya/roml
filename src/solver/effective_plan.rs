//! Effective solve plan and feature recording (SM-04.5, SM-07.7).
//!
//! [`EffectiveSolvePlan`] records what actually happened during a solve
//! attempt: the native features/bridges selected and applied, every
//! conversion/adjustment applied to the requested plan, every rejected
//! request, and (from P31) the per-stage objective results. It is carried by
//! [`SolveMetadata`](crate::solution::metadata::SolveMetadata) so every real
//! solve is self-describing (SM-04.5): applications, conversions, and
//! rejections are never silent.

use crate::id::ObjId;
use crate::solver::SolveStatus;

/// What actually happened during one solve attempt (packet "Solve plan and
/// exact result identity"; SM-04.5, SM-07.7).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectiveSolvePlan {
    /// Native features/bridges selected and applied this solve (SM-04.5).
    pub applied_features: Vec<AppliedFeature>,
    /// Conversions and adjustments applied to the requested plan (SM-08.5).
    pub adjustments: Vec<PlanAdjustment>,
    /// Features requested but rejected — recorded, never silently dropped
    /// (SM-08.4).
    pub rejections: Vec<PlanRejection>,
    /// Per-stage objective results. P28 declares the field empty; P31
    /// populates it when executing lexicographic stages (SM-07.7).
    pub objective_stages: Vec<ObjectiveStageResult>,
}

/// One feature selected and applied during the solve (SM-04.5).
#[derive(Clone, Debug, PartialEq)]
pub struct AppliedFeature {
    /// Feature key (e.g. `"mip_start"`, `"variable_hint"`).
    pub feature: String,
    /// Human-readable detail of what was applied.
    pub detail: String,
}

/// One conversion or adjustment applied to the requested plan (SM-08.5).
#[derive(Clone, Debug, PartialEq)]
pub struct PlanAdjustment {
    /// The plan element key (e.g. `"mip_start[0]"`, `"hints"`).
    pub key: String,
    /// The requested form.
    pub requested: String,
    /// The applied form.
    pub applied: String,
    /// Why it was adjusted (the policy/feature reason).
    pub reason: String,
}

/// One requested feature rejected (recorded, never silent).
#[derive(Clone, Debug, PartialEq)]
pub struct PlanRejection {
    /// The plan element key.
    pub key: String,
    /// Why it was rejected.
    pub reason: String,
}

/// One objective-stage result (design §15.2; SM-07.7).
///
/// P28 declares the type; P31 populates it when executing lexicographic
/// stages. Each stage reports the objective solved, the stage optimum, and the
/// stage's termination status.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectiveStageResult {
    /// Stage index (0-based).
    pub stage: usize,
    /// The objective solved in this stage, if any.
    pub objective: Option<ObjId>,
    /// The optimal value of the stage objective, if known.
    pub value: Option<f64>,
    /// The stage's termination status.
    pub status: SolveStatus,
}
