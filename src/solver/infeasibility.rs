#![allow(missing_docs)]

//! LP infeasibility analysis contracts and portable reduction primitives.
//!
//! This module deliberately contains solver-neutral vocabulary. Backend-native
//! providers are optional and must add evidence without changing the semantic
//! report contract.

use std::fmt;

use crate::compiler::backend_ir::CompilationId;
use crate::identity::{ModelInstanceId, ModelLineageId};
use crate::revision::ModelRevision;
use crate::solver::backend::{BackendError, TerminationStatus};

/// Provider selection for infeasibility analysis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum InfeasibilityMode {
    #[default]
    Auto,
    RomlPortable,
    NativeOnly,
    NativeThenRoml,
}

/// Model scope being analyzed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum InfeasibilityScope {
    #[default]
    OriginalLp,
    LpRelaxation,
}

/// Feasibility proof strength returned by an oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FeasibilityProofStrength {
    SolverCertified,
    NativeReported,
    None,
}

/// The only outcomes an IIS feasibility oracle may return.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum FeasibilityOutcome {
    /// The selected restrictions admit a feasible LP solution.
    ProvenFeasible { proof: FeasibilityProofStrength },
    /// The selected restrictions are proven infeasible under the policy.
    ProvenInfeasible { proof: FeasibilityProofStrength },
    /// The backend did not establish either mathematical outcome.
    Unknown { reason: UnknownReason },
}

/// Why an oracle could not establish a proof.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnknownReason {
    Limit,
    Interrupted,
    Numerical,
    Ambiguous,
    Backend(String),
    RecoveryRequired,
}

impl From<&TerminationStatus> for FeasibilityOutcome {
    fn from(status: &TerminationStatus) -> Self {
        use TerminationStatus::*;
        match status {
            Optimal | Feasible => Self::ProvenFeasible {
                proof: FeasibilityProofStrength::SolverCertified,
            },
            Infeasible => Self::ProvenInfeasible {
                proof: FeasibilityProofStrength::SolverCertified,
            },
            InfeasibleOrUnbounded => Self::Unknown {
                reason: UnknownReason::Ambiguous,
            },
            TimeLimit | IterationLimit | NodeLimit => Self::Unknown {
                reason: UnknownReason::Limit,
            },
            Interrupted => Self::Unknown {
                reason: UnknownReason::Interrupted,
            },
            NumericalIssue => Self::Unknown {
                reason: UnknownReason::Numerical,
            },
            Error | Unknown => Self::Unknown {
                reason: UnknownReason::Backend(format!("status: {status:?}")),
            },
            Unbounded => Self::Unknown {
                reason: UnknownReason::Ambiguous,
            },
        }
    }
}

/// Analysis budget. `None` means unlimited for that dimension.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnalysisBudget {
    pub max_oracle_calls: Option<u64>,
    pub max_iterations: Option<u64>,
}

/// Numerical policy recorded in a report.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisNumericalPolicy {
    pub primal_feasibility_tolerance: f64,
    pub dual_feasibility_tolerance: f64,
}
impl Default for AnalysisNumericalPolicy {
    fn default() -> Self {
        Self {
            primal_feasibility_tolerance: 1e-7,
            dual_feasibility_tolerance: 1e-7,
        }
    }
}

/// User-selected analysis plan.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InfeasibilityPlan {
    pub mode: InfeasibilityMode,
    pub scope: InfeasibilityScope,
    pub budget: AnalysisBudget,
    pub numerical_policy: AnalysisNumericalPolicy,
}

/// Stable semantic member identifier. It is not a backend row number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConflictAtomId(pub u64);

/// Semantic kind of a conflict member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConflictAtomKind {
    ConstraintSide,
    VariableBound,
    PersistentFixing,
    SolveLock,
    TemporaryFixing,
    Construct,
}

/// A report member snapshot. Backend evidence is deliberately optional.
#[derive(Clone, Debug, PartialEq)]
pub struct ConflictMember {
    pub atom_id: ConflictAtomId,
    pub kind: ConflictAtomKind,
    pub display_name: String,
    pub native_membership: Option<String>,
}

/// Completion is independent from the mathematical guarantee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnalysisCompletion {
    Complete,
    TimeLimit,
    OracleCallLimit,
    IterationLimit,
    Interrupted,
    NumericalFailure,
    BackendFailure,
}

/// Guarantee attached to returned members.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConflictGuarantee {
    InfeasibleSubsystem,
    Irreducible,
    NativeReported,
    None,
}

/// Provider evidence retained in the historical report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisProviderRecord {
    pub provider: String,
    pub native: bool,
    pub detail: String,
}

/// Canonical structured result for LP infeasibility analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct InfeasibilityReport {
    pub model_lineage: ModelLineageId,
    pub model_instance: ModelInstanceId,
    pub model_revision: ModelRevision,
    pub compilation_id: Option<CompilationId>,
    pub scope: InfeasibilityScope,
    pub provider_chain: Vec<AnalysisProviderRecord>,
    pub completion: AnalysisCompletion,
    pub guarantee: ConflictGuarantee,
    pub oracle_strength: FeasibilityProofStrength,
    pub numerical_policy: AnalysisNumericalPolicy,
    pub members: Vec<ConflictMember>,
    pub oracle_calls: u64,
    pub warnings: Vec<String>,
}

impl fmt::Display for InfeasibilityReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LP infeasibility analysis: {:?}, {:?}, {} member(s)",
            self.scope,
            self.guarantee,
            self.members.len()
        )
    }
}

/// Deterministic compact text rendering.
pub struct TextInfeasibilityReport<'a>(pub &'a InfeasibilityReport);
impl fmt::Display for TextInfeasibilityReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.0)?;
        for m in &self.0.members {
            writeln!(f, "- {} ({:?})", m.display_name, m.kind)?;
        }
        Ok(())
    }
}
/// Deterministic Markdown rendering.
pub struct MarkdownInfeasibilityReport<'a>(pub &'a InfeasibilityReport);
impl fmt::Display for MarkdownInfeasibilityReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "## Infeasibility analysis\n\n- Scope: `{:?}`\n- Completion: `{:?}`\n- Guarantee: `{:?}`\n", self.0.scope, self.0.completion, self.0.guarantee)?;
        if self.0.members.is_empty() {
            writeln!(f, "No conflict members.")
        } else {
            for m in &self.0.members {
                writeln!(f, "- `{}` — {:?}", m.display_name, m.kind)?;
            }
            Ok(())
        }
    }
}

/// Errors raised before an infeasibility report can be produced.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum InfeasibilityError {
    /// The requested scope or provider is not qualified by the backend.
    Unsupported { reason: String },
    /// A backend returned an operational failure.
    Backend(BackendError),
}
impl fmt::Display for InfeasibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { reason } => {
                write!(f, "infeasibility analysis unsupported: {reason}")
            }
            Self::Backend(e) => write!(f, "infeasibility backend error: {}", e.message),
        }
    }
}
impl std::error::Error for InfeasibilityError {}

/// Oracle abstraction used by the portable reducer.
pub trait FeasibilityOracle {
    /// Check the currently selected semantic atoms.
    fn check(&mut self, selected: &[ConflictAtomId]) -> Result<FeasibilityOutcome, BackendError>;
}

/// Reduce a proven-infeasible candidate using deterministic single-atom deletion.
/// Unknown outcomes retain the atom and prevent an irreducibility claim.
pub fn reduce_single_deletion<O: FeasibilityOracle>(
    oracle: &mut O,
    mut candidate: Vec<ConflictAtomId>,
) -> Result<(Vec<ConflictAtomId>, bool), BackendError> {
    let initial = oracle.check(&candidate)?;
    if !matches!(initial, FeasibilityOutcome::ProvenInfeasible { .. }) {
        return Ok((candidate, false));
    }
    let mut complete = true;
    let mut i = 0;
    while i < candidate.len() {
        let removed = candidate.remove(i);
        match oracle.check(&candidate)? {
            FeasibilityOutcome::ProvenInfeasible { .. } => {}
            FeasibilityOutcome::ProvenFeasible { .. } => {
                candidate.insert(i, removed);
                i += 1;
            }
            FeasibilityOutcome::Unknown { .. } => {
                candidate.insert(i, removed);
                complete = false;
                i += 1;
            }
        }
    }
    Ok((candidate, complete))
}

/// Translate a backend termination without ever treating ambiguity as infeasible.
pub fn tri_state(status: TerminationStatus) -> FeasibilityOutcome {
    FeasibilityOutcome::from(&status)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TableOracle;
    impl FeasibilityOracle for TableOracle {
        fn check(
            &mut self,
            selected: &[ConflictAtomId],
        ) -> Result<FeasibilityOutcome, BackendError> {
            if selected.iter().any(|id| id.0 == 1) && selected.iter().any(|id| id.0 == 2) {
                Ok(FeasibilityOutcome::ProvenInfeasible {
                    proof: FeasibilityProofStrength::SolverCertified,
                })
            } else {
                Ok(FeasibilityOutcome::ProvenFeasible {
                    proof: FeasibilityProofStrength::SolverCertified,
                })
            }
        }
    }

    #[test]
    fn ambiguous_and_limited_statuses_are_unknown() {
        assert!(matches!(
            tri_state(TerminationStatus::InfeasibleOrUnbounded),
            FeasibilityOutcome::Unknown {
                reason: UnknownReason::Ambiguous
            }
        ));
        assert!(matches!(
            tri_state(TerminationStatus::TimeLimit),
            FeasibilityOutcome::Unknown {
                reason: UnknownReason::Limit
            }
        ));
        assert!(matches!(
            tri_state(TerminationStatus::NumericalIssue),
            FeasibilityOutcome::Unknown {
                reason: UnknownReason::Numerical
            }
        ));
    }

    #[test]
    fn single_deletion_preserves_an_irreducible_pair() {
        let mut oracle = TableOracle;
        let (members, complete) = reduce_single_deletion(
            &mut oracle,
            vec![ConflictAtomId(1), ConflictAtomId(2), ConflictAtomId(3)],
        )
        .unwrap();
        assert!(complete);
        assert_eq!(members, vec![ConflictAtomId(1), ConflictAtomId(2)]);
    }
}
