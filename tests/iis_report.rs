//! RED characterization for canonical infeasibility reports and renderers.

use roml::advanced::{
    AnalysisCompletion, AnalysisNumericalPolicy, AnalysisProviderRecord, BackendIdentity,
    BoundSide, CandidateUniverseSummary, CompiledRestrictionEvidence, CompiledRestrictionRef,
    ConflictGuarantee, ConflictMember, ConflictMemberSnapshot, ConflictOrigin,
    FeasibilityProofStrength, InfeasibilityOutcome, InfeasibilityReport, InfeasibilityScope,
    InfeasibilityStatistics, MarkdownInfeasibilityReport, NativeConflictEvidence,
    NativeConflictMember, NativeMembership, TextInfeasibilityReport,
};
use roml::{ConflictAtomId, Model, ModelRevision};

fn report() -> InfeasibilityReport {
    let model = Model::new();
    let compilation =
        roml::advanced::BackendSnapshotBuilder::new(model.instance(), model.current_revision())
            .finalize()
            .unwrap()
            .compilation_id;
    InfeasibilityReport {
        model_lineage: model.lineage(),
        model_instance: model.instance(),
        model_revision: ModelRevision::ZERO,
        compilation_id: compilation,
        backend: BackendIdentity {
            name: "test-backend".to_string(),
            version: "1.2".to_string(),
        },
        provider_chain: vec![AnalysisProviderRecord {
            name: "roml-portable".to_string(),
        }],
        scope: InfeasibilityScope::OriginalLp,
        candidate_universe: CandidateUniverseSummary {
            atom_count: 1,
            grouping: roml::advanced::ConflictGrouping::Individual,
        },
        outcome: InfeasibilityOutcome::Conflict,
        completion: AnalysisCompletion::Complete,
        guarantee: ConflictGuarantee::Irreducible,
        oracle_strength: FeasibilityProofStrength::Proven,
        numerical_policy: AnalysisNumericalPolicy::default(),
        members: vec![ConflictMember {
            atom_id: ConflictAtomId(7),
            declaration: ConflictMemberSnapshot {
                origin: ConflictOrigin::VariableBound {
                    variable: roml::VarId::new(0, roml::id::Generation::new()),
                    side: BoundSide::Lower,
                },
                name: Some("x_[raw]".to_string()),
                value: Some(1.0),
            },
            compiled_evidence: vec![CompiledRestrictionEvidence {
                reference: CompiledRestrictionRef::VariableLower(
                    roml::advanced::CompiledVariableId(0),
                ),
                native_membership: None,
                native_bound: None,
            }],
        }],
        native_evidence: Some(NativeConflictEvidence {
            provider: "test-native".to_string(),
            evidence: vec![NativeConflictMember {
                restriction: CompiledRestrictionRef::VariableLower(
                    roml::advanced::CompiledVariableId(0),
                ),
                membership: NativeMembership::Possible,
                bound: Some(roml::advanced::NativeBoundStatus::Lower),
            }],
        }),
        statistics: InfeasibilityStatistics {
            oracle_calls: 4,
            iterations: 2,
            fresh_verification_checks: 2,
        },
        warnings: vec![],
    }
}

#[test]
fn text_and_markdown_renderers_are_deterministic_and_separate() {
    let report = report();
    let text = TextInfeasibilityReport(&report).to_string();
    let markdown = MarkdownInfeasibilityReport(&report).to_string();
    assert!(text.find("scope").unwrap() < text.find("members").unwrap());
    assert!(
        markdown.find("## Semantic members").unwrap()
            < markdown.find("## Technical evidence").unwrap()
    );
    assert_ne!(text, markdown);
    assert!(report.to_string().len() < text.len());
    assert!(text.contains("not minimum-cardinality"));
    assert!(markdown.contains("not minimum-cardinality"));
    assert!(text.contains("Possible"));
    assert!(markdown.contains("Possible"));
}

#[test]
fn report_keeps_exact_identity_and_explicit_lp_relaxation_scope() {
    let mut report = report();
    assert_eq!(report.model_revision, ModelRevision::ZERO);
    assert_ne!(report.model_instance, Model::new().instance());
    report.scope = InfeasibilityScope::LpRelaxation;
    assert_eq!(report.scope, InfeasibilityScope::LpRelaxation);
}
