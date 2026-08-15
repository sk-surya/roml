//! P29 conflict-origin composition tests for the P30 relaxation mapper.

#[path = "support/p30_reference_session.rs"]
mod p30_reference_session;

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, BackendIdentity, CompilationPolicy, CompilationSession,
    FeatureSupport,
};
use roml::solver::infeasibility::{
    AnalysisCompletion, AnalysisNumericalPolicy, CandidateUniverseSummary, ConflictAtomId,
    ConflictGrouping, ConflictGuarantee, ConflictMember, ConflictMemberSnapshot, ConflictOrigin,
    FeasibilityProofStrength, InfeasibilityOutcome, InfeasibilityReport, InfeasibilityScope,
    InfeasibilityStatistics,
};
use roml::solver::relaxation::{FeasibilityRelaxationPlan, RelaxationOutcome};
use roml::solver::{map_p29_members, FeasibilityRelaxationError, RelaxationRestriction};
use roml::{continuous, ConstraintExprExt, Model, SolverSession};

use p30_reference_session::ReferenceSolveSession;

fn report(model: &Model, member: ConflictMember) -> InfeasibilityReport {
    report_with_members(model, vec![member])
}

fn report_with_members(model: &Model, members: Vec<ConflictMember>) -> InfeasibilityReport {
    let mut compiler = CompilationSession::new();
    let snapshot = model.take_snapshot().unwrap();
    let mut caps = BackendCapabilitySet::new();
    caps.set(
        BackendFeature::Lp,
        FeatureSupport::native(Default::default()),
    );
    let compiled = compiler
        .compile_snapshot(model.instance(), &snapshot, &CompilationPolicy::Auto, &caps)
        .unwrap();
    InfeasibilityReport {
        model_lineage: model.lineage(),
        model_instance: model.instance(),
        model_revision: model.current_revision(),
        compilation_id: compiled.compilation_id,
        backend: BackendIdentity {
            name: "test".into(),
            version: "0".into(),
        },
        provider_chain: Vec::new(),
        scope: InfeasibilityScope::OriginalLp,
        candidate_universe: CandidateUniverseSummary {
            atom_count: members.len(),
            grouping: ConflictGrouping::Individual,
        },
        outcome: InfeasibilityOutcome::Conflict,
        completion: AnalysisCompletion::Complete,
        guarantee: ConflictGuarantee::InfeasibleSubsystem,
        oracle_strength: FeasibilityProofStrength::Proven,
        numerical_policy: AnalysisNumericalPolicy::default(),
        members,
        native_evidence: None,
        statistics: InfeasibilityStatistics::default(),
        warnings: Vec::new(),
    }
}

fn member(origin: ConflictOrigin, name: Option<&str>) -> ConflictMember {
    member_with_id(1, origin, name)
}

fn member_with_id(atom_id: u64, origin: ConflictOrigin, name: Option<&str>) -> ConflictMember {
    ConflictMember {
        atom_id: ConflictAtomId(atom_id),
        declaration: ConflictMemberSnapshot {
            origin,
            name: name.map(str::to_owned),
            value: None,
        },
        compiled_evidence: Vec::new(),
    }
}

#[test]
fn p29_declared_bound_on_fixed_variable_maps_and_composes_with_fixing() {
    let mut model = Model::new();
    let variable = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.fix(variable, 5.0).unwrap();
    model.commit().unwrap();

    assert_eq!(
        model.variable_bounds(variable),
        Some(roml::Bounds::new(0.0, 10.0))
    );
    assert_eq!(
        model.effective_bounds(variable),
        Some(roml::Bounds::new(5.0, 5.0))
    );

    let mut report = report_with_members(
        &model,
        vec![
            member_with_id(
                1,
                ConflictOrigin::VariableBound {
                    variable,
                    side: roml::solver::infeasibility::BoundSide::Lower,
                },
                Some("mps:BOUND_LO"),
            ),
            member_with_id(
                2,
                ConflictOrigin::PersistentFixing { variable },
                Some("model:FIX_X"),
            ),
        ],
    );
    let mut session = SolverSession::new(ReferenceSolveSession::new());
    let base_solution = session.solve(&mut model).unwrap();
    report.compilation_id = base_solution.metadata().compilation_id.unwrap();

    let mapped = map_p29_members(
        &report,
        model.instance(),
        model.current_revision(),
        report.compilation_id,
    )
    .unwrap();

    assert_eq!(mapped.len(), 2);
    assert_eq!(mapped[0].source_provenance.as_deref(), Some("mps:BOUND_LO"));
    assert_eq!(mapped[1].source_provenance.as_deref(), Some("model:FIX_X"));
    assert_eq!(
        mapped[0].restriction,
        RelaxationRestriction::VariableBound {
            variable,
            side: roml::solver::infeasibility::BoundSide::Lower,
        }
    );
    assert_eq!(
        mapped[1].restriction,
        RelaxationRestriction::PersistentFixing { variable }
    );

    let p30_scope = roml::solver::RelaxationScope::Explicit(
        mapped
            .into_iter()
            .map(|mapped| mapped.restriction)
            .collect(),
    );
    assert_eq!(
        p30_scope,
        roml::solver::RelaxationScope::Explicit(vec![
            RelaxationRestriction::VariableBound {
                variable,
                side: roml::solver::infeasibility::BoundSide::Lower,
            },
            RelaxationRestriction::PersistentFixing { variable },
        ])
    );

    let repair = session
        .solve_feasibility_relaxation_from_p29(
            &mut model,
            FeasibilityRelaxationPlan {
                scope: p30_scope,
                ..Default::default()
            },
            &report,
        )
        .unwrap();
    assert_eq!(repair.outcome, RelaxationOutcome::OptimalRepair);
    assert_eq!(repair.members.len(), 2);
    assert_eq!(
        repair
            .members
            .iter()
            .find(|member| member.restriction
                == RelaxationRestriction::VariableBound {
                    variable,
                    side: roml::solver::infeasibility::BoundSide::Lower,
                })
            .unwrap()
            .source_provenance
            .as_deref(),
        Some("mps:BOUND_LO")
    );
    assert_eq!(
        repair
            .members
            .iter()
            .find(|member| member.restriction
                == RelaxationRestriction::PersistentFixing { variable })
            .unwrap()
            .source_provenance
            .as_deref(),
        Some("model:FIX_X")
    );
    assert!((repair.total_weighted_violation - 5.0).abs() < 1e-9);
}

#[test]
fn primitive_and_imported_constraint_side_map_with_source_name() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let constraint = model.add_constraint((x).le(3.0)).unwrap();
    model.commit().unwrap();
    let report = report(
        &model,
        member(
            ConflictOrigin::ConstraintSide {
                constraint,
                side: roml::solver::infeasibility::BoundSide::Upper,
            },
            Some("mps:ROW_A"),
        ),
    );
    let mapped = map_p29_members(
        &report,
        model.instance(),
        model.current_revision(),
        report.compilation_id,
    )
    .unwrap();
    assert_eq!(mapped.len(), 1);
    assert_eq!(mapped[0].source_provenance.as_deref(), Some("mps:ROW_A"));
    assert_eq!(
        mapped[0].restriction,
        RelaxationRestriction::ConstraintSide {
            constraint,
            side: roml::solver::infeasibility::BoundSide::Upper
        }
    );
}

#[test]
fn unsupported_member_rejects_the_complete_mapping() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let _constraint = model.add_constraint((x).le(3.0)).unwrap();
    model.commit().unwrap();
    let report = report(
        &model,
        member(ConflictOrigin::SolveLock { variable: x }, None),
    );
    let result = map_p29_members(
        &report,
        model.instance(),
        model.current_revision(),
        report.compilation_id,
    );
    assert!(matches!(
        result,
        Err(FeasibilityRelaxationError::UnsupportedOrigin(_))
    ));
}

#[test]
fn stale_report_identity_rejects_before_conversion() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let constraint = model.add_constraint((x).le(3.0)).unwrap();
    model.commit().unwrap();
    let report = report(
        &model,
        member(
            ConflictOrigin::ConstraintSide {
                constraint,
                side: roml::solver::infeasibility::BoundSide::Upper,
            },
            None,
        ),
    );
    let other = Model::new();
    let result = map_p29_members(
        &report,
        other.instance(),
        model.current_revision(),
        report.compilation_id,
    );
    assert!(matches!(
        result,
        Err(FeasibilityRelaxationError::StaleIdentity(_))
    ));
}
