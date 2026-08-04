//! P28 Task 3 — HiGHS start/hint audit-derived behavior (SM-08.2, SM-08.4,
//! SM-08.5, SM-08.6, SM-08.7, SM-04.5).
//!
//! Every capability declaration in these tests traces to the pinned official
//! header audit (`docs/knowledge/highs_mip_start_api.md`): `MipStart` and
//! `PartialMipStart` are qualified via `Highs_setSparseSolution`; `VariableHints`
//! and `MultipleMipStarts` have NO API in the bundled version and reject by
//! default; `InitialBasis` has an API but is a separate P28-scope-out artifact
//! (SM-08.6). Nothing is simulated.

use std::collections::BTreeMap;

use roml::advanced::{BackendFeature, SupportLevel};
use roml::model::integer;
use roml::solver::backend::BackendError;
use roml::{
    ConstraintExprExt, HintPriority, MipStart, Model, PlanError, PrimalAssignment, RepairPolicy,
    SolveError, SolveOptions, SolvePlan, SolverSession, UnsupportedFeaturePolicy, VariableHint,
    VariableHints,
};
use roml_highs::{highs_capability_set, HighsSession};

/// A MIP fixture: maximize `3x + y` over two integer variables with
/// `x + y <= 10`. Returns `(model, x, y, obj)`.
fn mip_model() -> (Model, roml::VarId, roml::VarId, roml::ObjId) {
    let mut model = Model::new();
    let x = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    let y = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x + y).le(10.0)).unwrap();
    let obj = model.maximize(3.0 * x + y).unwrap();
    (model, x, y, obj)
}

fn assignment_for(model: &Model, values: BTreeMap<roml::VarId, f64>) -> PrimalAssignment {
    PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values,
    }
}

/// A fresh session solving a MIP end-to-end via `SolvePlan`.
fn plan_session() -> SolverSession<HighsSession> {
    SolverSession::new(HighsSession::try_new().expect("HiGHS should be available"))
}

// ── Capability matrix (SM-08.7, SM-08.2, SM-08.6) ────────────────────────────

/// `highs_capability_set` declares the start/hint features exactly as the
/// pinned header audit qualifies them: `MipStart`/`PartialMipStart` Native
/// via `Highs_setSparseSolution`; `MultipleMipStarts`/`VariableHints`/
/// `InitialBasis` Unsupported with a note citing the audit record.
#[test]
fn highs_capability_set_declares_start_hint_features_per_audit() {
    for (major, minor, patch) in [(1, 15, 0), (1, 9, 0)] {
        let set = highs_capability_set(major, minor, patch);

        // Qualified native start primitives.
        assert!(
            set.supports(BackendFeature::MipStart),
            "MipStart must be Native (Highs_setSparseSolution, audit)"
        );
        assert!(
            set.supports(BackendFeature::PartialMipStart),
            "PartialMipStart must be Native (Highs_setSparseSolution, audit)"
        );

        // Absent / out-of-scope features are typed Unsupported with the audit
        // record cited in the notes (SM-08.7, D19).
        for feature in [
            BackendFeature::MultipleMipStarts,
            BackendFeature::VariableHints,
            BackendFeature::InitialBasis,
        ] {
            assert!(
                !set.supports(feature),
                "{feature:?} must NOT be declared Native"
            );
            let support = set
                .support(feature)
                .unwrap_or_else(|| panic!("{feature:?} must be declared"));
            assert_eq!(
                support.level,
                SupportLevel::Unsupported,
                "{feature:?} must be Unsupported"
            );
            assert!(
                support
                    .limitations
                    .notes
                    .iter()
                    .any(|n| n.contains("highs_mip_start_api")),
                "{feature:?} notes must cite the audit record, got {:?}",
                support.limitations.notes
            );
        }
    }
}

// ── Equivalence at the backend (SM-07.2) ─────────────────────────────────────

/// On one session, `solve`, `solve_with`, and an empty `solve_plan` produce
/// identical status, objective, revision, and the exact `CompilationId`.
#[test]
fn highs_solve_solve_with_and_empty_solve_plan_are_equivalent() {
    let (mut model, _x, _y, _obj) = mip_model();
    let mut session = plan_session();

    let warm = session.solve(&mut model).unwrap();
    let c_base = warm.metadata().compilation_id.expect("base compilation");
    let rev = model.current_revision();

    let s1 = session.solve(&mut model).unwrap();
    let s2 = session
        .solve_with(&mut model, SolveOptions::default())
        .unwrap();
    let s3 = session
        .solve_plan(&mut model, SolvePlan::new(SolveOptions::default()).unwrap())
        .unwrap();

    for s in [&s1, &s2, &s3] {
        assert_eq!(s.status(), warm.status());
        assert_eq!(s.objective_value(), warm.objective_value());
        assert_eq!(s.metadata().model_revision, rev);
        assert_eq!(s.metadata().compilation_id, Some(c_base));
        assert!(s.metadata().effective_plan.objective_stages.is_empty());
    }
    for var in [_x, _y] {
        assert_eq!(s1.value(var), s2.value(var));
        assert_eq!(s1.value(var), s3.value(var));
    }
}

// ── Default rejection (SM-08.4) ──────────────────────────────────────────────

/// A plan requesting hints against the pinned backend rejects by default —
/// `VariableHints` has no API in the bundled version (the blocking decision).
#[test]
fn highs_variable_hints_reject_by_default() {
    let (mut model, x, _y, _obj) = mip_model();
    let mut session = plan_session();

    let mut hints = VariableHints::default();
    hints.insert(
        x,
        VariableHint {
            value: 1.0,
            priority: HintPriority(1),
        },
    );
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![],
        hints,
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::Reject,
    };
    let err = session
        .solve_plan(&mut model, plan)
        .expect_err("absent hints reject by default");
    assert!(
        matches!(
            err,
            SolveError::Plan(PlanError::UnsupportedFeature {
                feature: "VariableHints",
                policy: UnsupportedFeaturePolicy::Reject
            })
        ),
        "unexpected error: {err:?}"
    );
}

// ── Qualified native start path (SM-08.5) ────────────────────────────────────

/// A FULL start applies natively through `Highs_setSparseSolution` and is
/// recorded as an applied feature. Under an explicit conversion policy, the
/// QUALIFIED path wins (conversion is a fallback for unqualified features), so
/// no conversion adjustment is fabricated.
#[test]
fn highs_qualified_start_applies_natively_even_under_conversion_policy() {
    let (mut model, x, y, _obj) = mip_model();
    let mut session = plan_session();

    let start = MipStart::new(
        assignment_for(&model, BTreeMap::from([(x, 5.0), (y, 5.0)])),
        RepairPolicy::BackendDefault,
    );
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![start],
        hints: VariableHints::default(),
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::ConvertStartToTemporaryFixing,
    };
    let solution = session.solve_plan(&mut model, plan).unwrap();
    let ep = &solution.metadata().effective_plan;

    assert!(
        ep.applied_features.iter().any(|f| f.feature == "mip_start"),
        "the qualified start must be applied natively and recorded, got {:?}",
        ep.applied_features
    );
    assert!(
        ep.adjustments.is_empty(),
        "a qualified start must NOT be converted (conversion is a fallback for \
         unqualified features), got {:?}",
        ep.adjustments
    );
    // The start is a search hint; the proven optimum is unchanged.
    assert_eq!(solution.objective_value(), Some(30.0));
}

/// `ConvertHintToStart` converts hints into a `MipStart` when `MipStart` is
/// qualified and records the conversion (SM-08.5).
#[test]
fn highs_convert_hint_to_start_is_recorded() {
    let (mut model, x, _y, _obj) = mip_model();
    let mut session = plan_session();

    let mut hints = VariableHints::default();
    hints.insert(
        x,
        VariableHint {
            value: 5.0,
            priority: HintPriority(1),
        },
    );
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![],
        hints,
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::ConvertHintToStart,
    };
    let solution = session.solve_plan(&mut model, plan).unwrap();
    let ep = &solution.metadata().effective_plan;

    assert!(
        ep.adjustments.iter().any(|a| a.key == "hints"
            && a.requested == "variable_hints"
            && a.applied == "mip_start"),
        "the hint->start conversion must be recorded, got {:?}",
        ep.adjustments
    );
    assert!(
        ep.applied_features.iter().any(|f| f.feature == "mip_start"),
        "the converted start must be applied and recorded"
    );
}

// ── Feasibility signature invariance (SM-08.3) ───────────────────────────────

/// A HiGHS MIP solved with and without a qualified start yields the same
/// optimal objective, the same canonical revision, and the same compiled base
/// id — starts are search hints, never feasible-region changes.
#[test]
fn highs_qualified_start_leaves_feasible_region_signature_unchanged() {
    let (mut model, x, y, _obj) = mip_model();
    let mut session = plan_session();

    let base = session.solve(&mut model).unwrap();
    let c_base = base.metadata().compilation_id.expect("base compilation");
    let rev = model.current_revision();
    let base_obj = base.objective_value();

    // A feasible-but-suboptimal incumbent (obj 20 vs the optimum 30).
    let start = MipStart::new(
        assignment_for(&model, BTreeMap::from([(x, 5.0), (y, 5.0)])),
        RepairPolicy::BackendDefault,
    );
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![start],
        hints: VariableHints::default(),
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::Reject,
    };
    let with_start = session.solve_plan(&mut model, plan).unwrap();

    assert_eq!(with_start.objective_value(), base_obj);
    assert_eq!(with_start.status(), base.status());
    assert_eq!(model.current_revision(), rev);
    assert_eq!(with_start.metadata().compilation_id, Some(c_base));
}

// ── Checked return codes (T-28-01, D19) ──────────────────────────────────────

/// A failed `Highs_setSparseSolution` (a start value inside the DECLARED
/// bounds but outside the EFFECTIVE native column bounds — here narrowed by a
/// persistent fixing) maps to a typed `BackendError`, never a panic.
#[test]
fn highs_failed_sparse_solution_maps_to_typed_backend_error() {
    let mut model = Model::new();
    let x = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    let y = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x + y).le(10.0)).unwrap();
    model.maximize(3.0 * x + y).unwrap();
    // Persistent fixing narrows x's effective/native bound to [4,4].
    model.fix(x, 4.0).unwrap();
    model.commit().unwrap();

    let mut session = plan_session();
    let _ = session.solve(&mut model).unwrap();

    // x=5 is inside the DECLARED [0,10] (plan.validate passes) but outside the
    // effective native [4,4], so Highs_setSparseSolution returns kError.
    let start = MipStart::new(
        assignment_for(&model, BTreeMap::from([(x, 5.0), (y, 1.0)])),
        RepairPolicy::BackendDefault,
    );
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![start],
        hints: VariableHints::default(),
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::Reject,
    };
    let err = session
        .solve_plan(&mut model, plan)
        .expect_err("native rejection");
    match err {
        SolveError::Solve(BackendError { message, .. }) => {
            assert!(
                message.contains("Highs_setSparseSolution"),
                "the typed error must name the native call, got: {message}"
            );
        }
        other => panic!("expected SolveError::Solve with a BackendError, got {other:?}"),
    }
}

// ── No stale start (the lifecycle question) ──────────────────────────────────

/// After a solve with a start, a subsequent solve of a CHANGED model without a
/// start is deterministic and equal to a fresh no-start solve — a stale
/// incumbent never seeds an unrelated solve.
#[test]
fn highs_no_stale_start_leakage_into_subsequent_solve() {
    let (mut model, x, _y, _obj) = mip_model();
    let mut session = plan_session();

    let _ = session.solve(&mut model).unwrap();

    let start = MipStart::new(
        assignment_for(&model, BTreeMap::from([(x, 5.0)])),
        RepairPolicy::BackendDefault,
    );
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![start],
        hints: VariableHints::default(),
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::Reject,
    };
    let _with_start = session.solve_plan(&mut model, plan).unwrap();

    // Modify the model (tighten the capacity row) and solve WITHOUT a start.
    model.add_constraint((x).le(3.0)).unwrap();
    let clean = session.solve(&mut model).unwrap();

    let mut fresh = plan_session();
    let fresh_sol = fresh.solve(&mut model).unwrap();

    assert_eq!(clean.objective_value(), fresh_sol.objective_value());
    assert_eq!(clean.status(), fresh_sol.status());
    assert_eq!(clean.value(x), fresh_sol.value(x));
}
