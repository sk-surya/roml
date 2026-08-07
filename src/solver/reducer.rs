//! Deterministic portable semantic conflict reduction.

use crate::solver::infeasibility::{
    AnalysisSession, ConflictAtomId, ConflictGuarantee, FeasibilityOutcome, InfeasibilityError,
    OracleBudget, RestrictionSelection, SemanticConflictUniverse,
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
    let budget = OracleBudget::default();
    let initial = session.check(&seed, &budget)?;
    if matches!(initial, FeasibilityOutcome::ProvenFeasible(_)) {
        return Ok(ReductionOutcome::NoConflict);
    }
    if !matches!(initial, FeasibilityOutcome::ProvenInfeasible(_)) {
        return Ok(ReductionOutcome::NoConflictProof);
    }

    let mut selected = seed.atom_ids;
    let mut stats = ReductionStatistics::default();
    let mut chunk_size = (selected.len() / 2).max(1);

    while chunk_size > 0 && selected.len() > 1 {
        stats.iterations += 1;
        let mut index = 0;
        while index < selected.len() && selected.len() > 1 {
            let end = (index + chunk_size).min(selected.len());
            let mut candidate = selected.clone();
            candidate.drain(index..end);
            stats.chunk_deletions += 1;
            let outcome = session.check(
                &RestrictionSelection {
                    compilation_id: seed.compilation_id,
                    atom_ids: candidate.clone(),
                },
                &budget,
            )?;
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
        stats.iterations += 1;
        let mut candidate = selected.clone();
        candidate.remove(position);
        let outcome = session.check_fresh(
            &RestrictionSelection {
                compilation_id: seed.compilation_id,
                atom_ids: candidate.clone(),
            },
            &budget,
        )?;
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

    let final_outcome = session.check_fresh(
        &RestrictionSelection {
            compilation_id: seed.compilation_id,
            atom_ids: selected.clone(),
        },
        &budget,
    )?;
    stats.fresh_verification_checks += 1;
    if !matches!(final_outcome, FeasibilityOutcome::ProvenInfeasible(_)) {
        guarantee = ConflictGuarantee::InfeasibleSubsystem;
    }

    for atom in selected.clone() {
        let mut candidate = selected.clone();
        candidate.retain(|member| *member != atom);
        let outcome = session.check_fresh(
            &RestrictionSelection {
                compilation_id: seed.compilation_id,
                atom_ids: candidate,
            },
            &budget,
        )?;
        stats.fresh_verification_checks += 1;
        if !matches!(outcome, FeasibilityOutcome::ProvenFeasible(_)) {
            guarantee = ConflictGuarantee::InfeasibleSubsystem;
        }
    }

    let _ = universe;
    stats.oracle_calls = session.oracle_calls();
    Ok(ReductionOutcome::Conflict(ReducedConflict {
        members: selected,
        guarantee,
        statistics: stats,
    }))
}
