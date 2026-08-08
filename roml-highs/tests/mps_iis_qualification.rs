//! P29 IIS qualification over an imported MPS model.

use std::path::PathBuf;

use roml::{
    advanced::{InfeasibilityPlan, InfeasibilityScope},
    io::mps::MpsReader,
    solver::facade::SolverSession,
};
use roml_highs::HighsSession;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/mps/infeasible-bound.mps")
}

fn implicit_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/mps/infeasible-implicit-bound.mps")
}

#[test]
fn imported_infeasible_lp_produces_irreducible_named_conflict() {
    let import = MpsReader::new()
        .read_path(fixture())
        .expect("infeasible fixture must import");
    assert_eq!(import.source_map.row_span("LOW").unwrap().line(), 4);
    assert_eq!(import.source_map.row_span("HIGH").unwrap().line(), 5);

    let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
    let plan = InfeasibilityPlan::portable_lp();
    assert_eq!(plan.scope, InfeasibilityScope::OriginalLp);
    let report = session
        .analyze_infeasibility(&import.model, &plan)
        .expect("P29 must analyze the imported infeasible LP");

    assert_eq!(report.outcome, roml::InfeasibilityOutcome::Conflict);
    assert_eq!(report.guarantee, roml::ConflictGuarantee::Irreducible);
    for member in report.members {
        if let roml::advanced::ConflictOrigin::ConstraintSide { constraint, .. } =
            member.declaration.origin
        {
            let name = import
                .model
                .constraint_name(constraint)
                .expect("reported constraint must remain live")
                .expect("imported constraints are named");
            assert!(import.source_map.row_span(name).is_some(), "{name}");
        }
    }
}

#[test]
fn imported_implicit_bound_conflict_has_exact_synthetic_provenance() {
    let import = MpsReader::new()
        .read_path(implicit_fixture())
        .expect("implicit-bound fixture must import");
    let origins = import.source_map.variable_bound_origins();
    assert_eq!(origins.len(), 1);
    assert_eq!(origins[0].variable, "X");
    assert!(matches!(
        origins[0].origin,
        roml::io::mps::MpsBoundOrigin::ImplicitContinuousDefault { .. }
    ));

    let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
    let report = session
        .analyze_infeasibility(&import.model, &InfeasibilityPlan::portable_lp())
        .expect("P29 must analyze the implicit-bound conflict");
    assert_eq!(report.guarantee, roml::ConflictGuarantee::Irreducible);
    let (variable, side) = report
        .members
        .iter()
        .find_map(|member| match member.declaration.origin {
            roml::advanced::ConflictOrigin::VariableBound { variable, side } => {
                Some((variable, side))
            }
            _ => None,
        })
        .expect("the exact reported IIS must contain a variable-bound member");
    let variable_name = import
        .model
        .variable_name(variable)
        .expect("reported variable must remain live")
        .expect("imported variable must be named");
    let mps_side = match side {
        roml::advanced::BoundSide::Lower => roml::io::mps::MpsBoundSide::Lower,
        roml::advanced::BoundSide::Upper => roml::io::mps::MpsBoundSide::Upper,
    };
    let matching_origins: Vec<_> = import
        .source_map
        .variable_bound_origins()
        .iter()
        .filter(|origin| origin.variable == variable_name && origin.side == mps_side)
        .collect();
    assert_eq!(
        matching_origins.len(),
        1,
        "the exact reported variable and side must resolve to one MPS origin"
    );
    assert!(matches!(
        matching_origins[0].origin,
        roml::io::mps::MpsBoundOrigin::ImplicitContinuousDefault { .. }
    ));
}
