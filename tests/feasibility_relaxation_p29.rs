//! P29 conflict-origin composition tests for the P30 relaxation mapper.

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
use roml::solver::{map_p29_members, FeasibilityRelaxationError, RelaxationRestriction};
use roml::{continuous, ConstraintExprExt, Model};

fn report(model: &Model, member: ConflictMember) -> InfeasibilityReport {
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
            atom_count: 1,
            grouping: ConflictGrouping::Individual,
        },
        outcome: InfeasibilityOutcome::Conflict,
        completion: AnalysisCompletion::Complete,
        guarantee: ConflictGuarantee::InfeasibleSubsystem,
        oracle_strength: FeasibilityProofStrength::Proven,
        numerical_policy: AnalysisNumericalPolicy::default(),
        members: vec![member],
        native_evidence: None,
        statistics: InfeasibilityStatistics::default(),
        warnings: Vec::new(),
    }
}

fn member(origin: ConflictOrigin, name: Option<&str>) -> ConflictMember {
    ConflictMember {
        atom_id: ConflictAtomId(1),
        declaration: ConflictMemberSnapshot {
            origin,
            name: name.map(str::to_owned),
            value: None,
        },
        compiled_evidence: Vec::new(),
    }
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
