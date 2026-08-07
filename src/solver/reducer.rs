//! Deterministic portable semantic conflict reduction.

use crate::solver::infeasibility::{
    AnalysisSession, ConflictAtomId, ConflictGuarantee, FeasibilityOutcome, InfeasibilityError,
    OracleBudget, ReductionPolicy, RestrictionSelection, SemanticConflictUniverse,
};

/// Reduction counters retained for evidence and performance qualification.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReductionStatistics {
    /// Number of oracle calls observed by the session.
    pub oracle_calls: u64,
    /// Number of reducer iterations.
    pub iterations: u64,
    /// Number of fresh final/member-verification checks.
    pub fresh_verification_checks: u64,
    /// Number of chunk-deletion attempts.
    pub chunk_deletions: u64,
    /// Whether a configured call or iteration budget stopped verification.
    pub budget_exhausted: bool,
}

/// A reduced semantic conflict and its verified guarantee.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReducedConflict {
    /// Stable semantic members in deterministic order.
    pub members: Vec<ConflictAtomId>,
    /// Guarantee established by the mandatory verifier.
    pub guarantee: ConflictGuarantee,
    /// Reduction counters.
    pub statistics: ReductionStatistics,
}

/// Entry-state result of one portable reduction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReductionOutcome {
    /// The initial universe was proven feasible.
    NoConflict,
    /// The initial universe was not proven infeasible.
    NoConflictProof,
    /// A proven infeasible subsystem was reduced and verified.
    Conflict(ReducedConflict),
}

/// Reduce one semantic selection without optimizing cardinality.
pub fn reduce<O: crate::solver::infeasibility::FeasibilityOracle>(
    session: &mut AnalysisSession<O>,
    universe: &SemanticConflictUniverse,
    seed: RestrictionSelection,
) -> Result<ReductionOutcome, InfeasibilityError> {
    reduce_with_limits(session, universe, seed, OracleBudget::default(), None, None)
}

/// Reduce one selection with explicit oracle-call and iteration limits.
pub fn reduce_with_limits<O: crate::solver::infeasibility::FeasibilityOracle>(
    session: &mut AnalysisSession<O>,
    universe: &SemanticConflictUniverse,
    seed: RestrictionSelection,
    budget: OracleBudget,
    max_oracle_calls: Option<u64>,
    max_iterations: Option<u64>,
) -> Result<ReductionOutcome, InfeasibilityError> {
    reduce_with_limits_and_policy(
        session,
        universe,
        seed,
        budget,
        max_oracle_calls,
        max_iterations,
        ReductionPolicy::Adaptive,
    )
}

/// Reduce one selection with explicit policy and work limits.
pub fn reduce_with_limits_and_policy<O: crate::solver::infeasibility::FeasibilityOracle>(
    session: &mut AnalysisSession<O>,
    universe: &SemanticConflictUniverse,
    seed: RestrictionSelection,
    budget: OracleBudget,
    max_oracle_calls: Option<u64>,
    max_iterations: Option<u64>,
    reduction: ReductionPolicy,
) -> Result<ReductionOutcome, InfeasibilityError> {
    if max_oracle_calls == Some(0) {
        return Ok(ReductionOutcome::NoConflictProof);
    }
    let mut stats = ReductionStatistics::default();
    let mut exhausted = false;
    let Some(initial) = limited_check(
        session,
        &seed,
        &budget,
        false,
        max_oracle_calls,
        &mut exhausted,
    )?
    else {
        return Ok(ReductionOutcome::NoConflictProof);
    };
    if matches!(initial, FeasibilityOutcome::ProvenFeasible(_)) {
        return Ok(ReductionOutcome::NoConflict);
    }
    if !matches!(initial, FeasibilityOutcome::ProvenInfeasible(_)) {
        return Ok(ReductionOutcome::NoConflictProof);
    }

    let mut selected = seed.atom_ids;
    let mut chunk_size = (selected.len() / 2).max(1);

    while reduction == ReductionPolicy::Adaptive && chunk_size > 0 && selected.len() > 1 {
        if max_iterations.is_some_and(|limit| stats.iterations >= limit) {
            exhausted = true;
            break;
        }
        stats.iterations += 1;
        let mut index = 0;
        while index < selected.len() && selected.len() > 1 {
            let end = (index + chunk_size).min(selected.len());
            let mut candidate = selected.clone();
            candidate.drain(index..end);
            stats.chunk_deletions += 1;
            let Some(outcome) = limited_check(
                session,
                &RestrictionSelection {
                    compilation_id: seed.compilation_id,
                    atom_ids: candidate.clone(),
                },
                &budget,
                false,
                max_oracle_calls,
                &mut exhausted,
            )?
            else {
                break;
            };
            if matches!(outcome, FeasibilityOutcome::ProvenInfeasible(_)) {
                selected = candidate;
            } else {
                index = end;
            }
        }
        if chunk_size == 1 {
            break;
        }
        chunk_size = (chunk_size / 2).max(1);
    }

    let mut guarantee = ConflictGuarantee::Irreducible;
    let mut position = 0;
    while position < selected.len() {
        if max_iterations.is_some_and(|limit| stats.iterations >= limit) {
            exhausted = true;
            break;
        }
        stats.iterations += 1;
        let mut candidate = selected.clone();
        candidate.remove(position);
        let Some(outcome) = limited_check(
            session,
            &RestrictionSelection {
                compilation_id: seed.compilation_id,
                atom_ids: candidate.clone(),
            },
            &budget,
            true,
            max_oracle_calls,
            &mut exhausted,
        )?
        else {
            break;
        };
        stats.fresh_verification_checks += 1;
        match outcome {
            FeasibilityOutcome::ProvenInfeasible(_) => selected = candidate,
            FeasibilityOutcome::ProvenFeasible(_) => position += 1,
            FeasibilityOutcome::Unknown(_) => {
                guarantee = ConflictGuarantee::InfeasibleSubsystem;
                position += 1;
            }
        }
    }

    let final_outcome = limited_check(
        session,
        &RestrictionSelection {
            compilation_id: seed.compilation_id,
            atom_ids: selected.clone(),
        },
        &budget,
        true,
        max_oracle_calls,
        &mut exhausted,
    )?;
    if let Some(final_outcome) = final_outcome {
        stats.fresh_verification_checks += 1;
        if !matches!(final_outcome, FeasibilityOutcome::ProvenInfeasible(_)) {
            return Err(InfeasibilityError::VerificationFailure {
                reason: format!(
                    "fresh final verification returned {final_outcome:?} for a candidate previously proven infeasible"
                ),
            });
        }
    }

    for atom in selected.clone() {
        let mut candidate = selected.clone();
        candidate.retain(|member| *member != atom);
        let Some(outcome) = limited_check(
            session,
            &RestrictionSelection {
                compilation_id: seed.compilation_id,
                atom_ids: candidate,
            },
            &budget,
            true,
            max_oracle_calls,
            &mut exhausted,
        )?
        else {
            break;
        };
        stats.fresh_verification_checks += 1;
        if !matches!(outcome, FeasibilityOutcome::ProvenFeasible(_)) {
            guarantee = ConflictGuarantee::InfeasibleSubsystem;
        }
    }

    let _ = universe;
    stats.oracle_calls = session.oracle_calls();
    if exhausted {
        guarantee = ConflictGuarantee::InfeasibleSubsystem;
    }
    stats.budget_exhausted = exhausted;
    Ok(ReductionOutcome::Conflict(ReducedConflict {
        members: selected,
        guarantee,
        statistics: stats,
    }))
}

fn limited_check<O: crate::solver::infeasibility::FeasibilityOracle>(
    session: &mut AnalysisSession<O>,
    selection: &RestrictionSelection,
    budget: &OracleBudget,
    fresh: bool,
    max_oracle_calls: Option<u64>,
    exhausted: &mut bool,
) -> Result<Option<FeasibilityOutcome>, InfeasibilityError> {
    if !fresh {
        if let Some(cached) = session.cached_check(selection)? {
            return Ok(Some(cached));
        }
    }
    if !max_oracle_calls
        .map(|limit| session.oracle_calls() < limit)
        .unwrap_or(true)
    {
        *exhausted = true;
        return Ok(None);
    }
    let outcome = if fresh {
        session.check_fresh(selection, budget)
    } else {
        session.check(selection, budget)
    }?;
    Ok(Some(outcome))
}
