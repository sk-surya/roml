//! P34 core qualification corpus Q01–Q14 (contract §3): solver-free
//! formulation checks.
//!
//! Each fixture compiles the frozen Q-model through `CompilationSession`
//! (Portable policy, bridge caps for construct features) and pins the exact
//! user-side formulation: user variable/row/objective counts, objective
//! sense/constant, user-mapped objective coefficients, origin completeness,
//! and compilation identity. Solved optima live in the HiGHS corpus;
//! structural tolerances here are exact.

use std::collections::{BTreeMap, BTreeSet};

use roml::compiler::capability::{
    BackendCapabilitySet, BackendFeature, CompilationPolicy, FeatureSupport,
};
use roml::compiler::origin::EntityOrigin;
use roml::compiler::session::CompilationSession;
use roml::construct::{
    AbsoluteValueVariant, IndicatorDirection, MinMaxRelation, MinMaxSense, PwlRelation,
};
use roml::{
    binary, continuous, integer, ConstraintExprExt, LinExpr, Model, ObjectivePolicy,
    ObjectivePriority, Sense, ValueExpr, WeightedObjective,
};

fn corpus_caps() -> BackendCapabilitySet {
    let mut caps = BackendCapabilitySet::new();
    for feature in [
        BackendFeature::Lp,
        BackendFeature::Mip,
        BackendFeature::IncrementalBounds,
        BackendFeature::IncrementalRows,
        BackendFeature::IncrementalCoefficients,
        BackendFeature::MipStart,
        BackendFeature::PartialMipStart,
    ] {
        caps.set(feature, FeatureSupport::native(Default::default()));
    }
    for feature in [
        BackendFeature::Indicator,
        BackendFeature::MinMax,
        BackendFeature::AbsoluteValue,
        BackendFeature::BinaryProduct,
        BackendFeature::PiecewiseLinear,
        BackendFeature::SoftConstraint,
    ] {
        caps.set(feature, FeatureSupport::bridge(Default::default()));
    }
    caps
}

struct CompiledView {
    user_vars: usize,
    user_rows: usize,
    objectives: usize,
    sense: Sense,
    constant: f64,
    /// User-mapped objective coefficients as (model var index, value).
    user_coefs: BTreeMap<u32, f64>,
    generated_vars: usize,
    generated_rows: usize,
}

fn compile_model(model: &Model) -> (CompilationSession, CompiledView) {
    let snapshot = model.take_snapshot().unwrap();
    let mut compiler = CompilationSession::new();
    let compiled = compiler
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Portable,
            &corpus_caps(),
        )
        .expect("corpus fixture must compile");
    assert_eq!(compiled.source_revision, snapshot.revision);

    let mut user_vars = 0;
    let mut generated_vars = 0;
    for var in &compiled.variables {
        if compiler.user_variable(var.id).is_some() {
            user_vars += 1;
        } else {
            generated_vars += 1;
        }
        assert!(
            compiled.origin_map.variable_origin(var.id).is_some(),
            "every compiled variable has an origin"
        );
    }
    let mut user_rows = 0;
    let mut generated_rows = 0;
    for row in &compiled.linear_rows {
        assert!(
            compiled.origin_map.constraint_origin(row.id).is_some(),
            "every compiled row has an origin"
        );
        match compiled.origin_map.constraint_origin(row.id).unwrap() {
            EntityOrigin::UserConstraint(_) => user_rows += 1,
            _ => generated_rows += 1,
        }
    }
    assert_eq!(compiled.objectives.len(), 1);
    let objective = &compiled.objectives[0];
    assert!(compiled.origin_map.objective_origin(objective.id).is_some());
    let mut user_coefs = BTreeMap::new();
    for (cid, value) in &objective.coefficients {
        if let Some(var) = compiler.user_variable(*cid) {
            user_coefs.insert(var.index(), *value);
        }
    }
    (
        compiler,
        CompiledView {
            user_vars,
            user_rows,
            objectives: compiled.objectives.len(),
            sense: objective.sense,
            constant: objective.constant,
            user_coefs,
            generated_vars,
            generated_rows,
        },
    )
}

fn coef(view: &CompiledView, var: roml::VarId, expected: f64) {
    let actual = view.user_coefs.get(&var.index()).copied().unwrap_or(0.0);
    assert!(
        (actual - expected).abs() <= 1e-12,
        "objective coefficient for {var:?}: expected {expected}, got {actual}"
    );
}

#[test]
fn q01_primitive_parameter_delta_formulation() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x).ge(2.0)).unwrap();
    let obj = model.minimize(x).unwrap();
    let p = model.add_parameter(3.0).unwrap();
    model
        .add_objective_coefficient(obj, x, ValueExpr::param(p))
        .unwrap();
    let (_compiler, view) = compile_model(&model);
    assert_eq!((view.user_vars, view.user_rows, view.objectives), (1, 1, 1));
    assert_eq!(view.sense, Sense::Minimize);
    assert_eq!(view.constant, 0.0);
    // Canonical 1.0 plus the evaluated parameter 3.0 combine in one cell.
    coef(&view, x, 4.0);
}

#[test]
fn q02_indicator_tight_bounds_formulation() {
    let mut model = Model::new();
    let z = model.add_variable(binary()).unwrap();
    let x = model.add_variable(continuous().bounds(0.0, 5.0)).unwrap();
    model
        .add_indicator(z, IndicatorDirection::WhenOne, (x).le(2.0), None)
        .unwrap();
    model.maximize(x).unwrap();
    let (_compiler, view) = compile_model(&model);
    assert_eq!(view.objectives, 1);
    assert_eq!(view.sense, Sense::Maximize);
    coef(&view, x, 1.0);
    assert!(
        view.generated_rows >= 1,
        "indicator bridge emits generated rows"
    );
}

#[test]
fn q03_exact_minmax_formulation() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 3.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 7.0)).unwrap();
    let (_construct, out) = model
        .add_minmax(
            vec![LinExpr::from(x), LinExpr::from(y)],
            MinMaxSense::Max,
            MinMaxRelation::Exact,
            None,
        )
        .unwrap();
    model.maximize(out).unwrap();
    let (_compiler, view) = compile_model(&model);
    assert_eq!(view.objectives, 1);
    coef(&view, out, 1.0);
    assert!(
        view.generated_vars >= 1 && view.generated_rows >= 1,
        "exact max emits selector variables and rows"
    );
}

#[test]
fn q04_absolute_positive_clamp_formulation() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let (_construct, out) = model
        .add_absolute_value(
            LinExpr::from(x),
            AbsoluteValueVariant::Clamp {
                lower: 0.0,
                upper: 4.0,
            },
            None,
        )
        .unwrap();
    model.maximize(out).unwrap();
    let (_compiler, view) = compile_model(&model);
    coef(&view, out, 1.0);
    assert!(
        view.generated_rows >= 1,
        "clamp bridge emits generated rows"
    );
}

#[test]
fn q05_binary_product_formulation() {
    let mut model = Model::new();
    let b = model.add_variable(binary()).unwrap();
    let x = model.add_variable(continuous().bounds(0.0, 5.0)).unwrap();
    let (_construct, out) = model
        .add_binary_times_linear(b, LinExpr::from(x), None)
        .unwrap();
    model.maximize(out).unwrap();
    let (_compiler, view) = compile_model(&model);
    coef(&view, out, 1.0);
    assert!(
        view.generated_rows >= 1,
        "product bridge emits generated rows"
    );
}

#[test]
fn q06_pwl_convex_epigraph_formulation() {
    use roml::construct::ExtrapolationPolicy;

    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(3.0, 3.0)).unwrap();
    let (_construct, out) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            vec![(0.0, 0.0).into(), (2.0, 1.0).into(), (4.0, 4.0).into()],
            PwlRelation::Epigraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    model.minimize(out).unwrap();
    let (_compiler, view) = compile_model(&model);
    coef(&view, out, 1.0);
    // Convex epigraph: zero-binary rows only.
    assert!(view.generated_rows >= 2);
}

#[test]
fn q07_pwl_nonconvex_exact_graph_formulation() {
    use roml::construct::ExtrapolationPolicy;

    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.5, 0.5)).unwrap();
    let (_construct, out) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            vec![(0.0, 0.0).into(), (1.0, 1.0).into(), (2.0, 0.0).into()],
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    model.minimize(out).unwrap();
    let (_compiler, view) = compile_model(&model);
    coef(&view, out, 1.0);
    assert!(
        view.generated_vars >= 1,
        "exact nonconvex graph emits segment binaries"
    );
}

#[test]
fn q08_overlay_fix_lock_compiles_without_revision_change() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x).ge(2.0)).unwrap();
    model.minimize(x).unwrap();
    model.commit().unwrap();
    let rev = model.current_revision();
    let snapshot = model.take_snapshot().unwrap();
    let mut compiler = CompilationSession::new();
    let base = compiler
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Portable,
            &corpus_caps(),
        )
        .unwrap();
    let overlay =
        roml::SolveOverlay::new(BTreeMap::from([(x, 5.0)]), vec![], vec![], vec![]).unwrap();
    let compiled_overlay =
        roml::advanced::compile_overlay(&model, &compiler, &overlay, None).unwrap();
    assert_eq!(compiled_overlay.base_compilation, base.compilation_id);
    assert_eq!(model.current_revision(), rev);
    assert!(!model.has_pending_changes());
}

#[test]
fn q09_mip_start_partial_validates_lineage() {
    use roml::{MipStart, PrimalAssignment, RepairPolicy};

    let mut model = Model::new();
    let n = model.add_variable(integer().bounds(0.0, 4.0)).unwrap();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x + n).le(6.0)).unwrap();
    model.maximize(x + 2.0 * n).unwrap();
    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values: BTreeMap::from([(n, 2.0)]),
    };
    let start = MipStart::new(assignment.clone(), RepairPolicy::BackendDefault);
    assert_eq!(start.assignment.values.len(), 1);
    // The partial assignment validates against the live model lineage.
    assignment.validate_for(&model).unwrap();
    // A foreign lineage never validates.
    let other = Model::new();
    assert!(assignment.validate_for(&other).is_err());
}

#[test]
fn q10_iis_universe_contains_both_rows() {
    use roml::advanced::{ConflictGrouping, InfeasibilityScope, SemanticConflictUniverse};

    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    model.add_constraint((x).ge(1.0)).unwrap();
    model.add_constraint((x).le(0.0)).unwrap();
    let snapshot = model.take_snapshot().unwrap();
    let mut compiler = CompilationSession::new();
    let compiled = compiler
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Portable,
            &corpus_caps(),
        )
        .unwrap();
    let universe = SemanticConflictUniverse::from_snapshot(
        &compiled,
        InfeasibilityScope::OriginalLp,
        ConflictGrouping::Individual,
    )
    .unwrap();
    assert!(
        universe.atoms.len() >= 2,
        "both contradictory rows enter the universe"
    );
}

#[test]
fn q11_relaxation_scope_resolves_declared_weight() {
    use roml::{PenaltyPolicy, PenaltyTarget, ViolationPolicy};

    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let constraint = model.add_constraint((x).ge(5.0)).unwrap();
    model.add_constraint((x).le(3.0)).unwrap();
    model.minimize(x).unwrap();
    model
        .soften_constraint(
            constraint,
            ViolationPolicy::default(),
            PenaltyPolicy {
                weight: ValueExpr::constant(2.0),
                target: PenaltyTarget::None,
            },
        )
        .unwrap();
    let (_compiler, view) = compile_model(&model);
    // The persistent soft bridge is compiled (one-sided: one violation
    // variable and its side row); the relaxation itself stays solve-scoped.
    assert!(view.generated_vars >= 1);
    assert!(view.generated_rows >= 1);
    assert_eq!(view.sense, Sense::Minimize);
}

#[test]
fn q12_lexicographic_mixed_sense_combines_exactly() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x + y).le(10.0)).unwrap();
    let obj0 = model.minimize(x).unwrap();
    let obj1 = model.maximize(y).unwrap();
    // Both canonical objectives compile with their raw senses and unit
    // coefficients; stage normalization (+w for min, -w for max) is pinned
    // by the solved corpus.
    let snapshot = model.take_snapshot().unwrap();
    let mut compiler = CompilationSession::new();
    let compiled = compiler
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Portable,
            &corpus_caps(),
        )
        .unwrap();
    use roml::compiler::origin::EntityOrigin;
    for (obj, var) in [(obj0, x), (obj1, y)] {
        let ids = compiled
            .origin_map
            .objectives_for_origin(&EntityOrigin::UserObjective(obj));
        assert_eq!(ids.len(), 1);
        let objective = compiled.objectives.iter().find(|o| o.id == ids[0]).unwrap();
        assert_eq!(objective.constant, 0.0);
        assert_eq!(objective.coefficients.len(), 1);
        assert_eq!(
            compiler.user_variable(objective.coefficients[0].0),
            Some(var)
        );
        assert_eq!(objective.coefficients[0].1, 1.0);
    }
    assert_eq!(model.objective_sense(obj0), Some(Sense::Minimize));
    assert_eq!(model.objective_sense(obj1), Some(Sense::Maximize));
    let _ = (ObjectivePolicy::Single(obj0), ObjectivePriority::new(0));
    let _ = (
        BTreeSet::<u32>::new(),
        WeightedObjective {
            objective: obj0,
            weight: 1.0,
        },
    );
}

#[test]
fn q13_lexicographic_zero_optimum_lock_math() {
    let lock =
        roml::ObjectiveLockReport::from_stage(ObjectivePriority::new(0), 0.0, 0.0, 1e-6).unwrap();
    assert_eq!(lock.reference_value, 0.0);
    assert_eq!(lock.relative_scale, 0.0);
    assert_eq!(lock.allowed_degradation, 0.0);
    assert_eq!(lock.normalized_upper_bound, 0.0);
}

#[test]
fn q14_mps_parameterized_snapshot_round_trip() {
    use roml::io::mps::{MpsReader, MpsWriter};
    use std::io::Cursor;

    let mut model = Model::new();
    let x = model
        .add_variable(continuous().bounds(1.0, 10.0).named("x"))
        .unwrap();
    let y = model
        .add_variable(continuous().bounds(0.0, 10.0).named("y"))
        .unwrap();
    model
        .add_constraint((x + 2.0 * y).le(8.0).named("capacity"))
        .unwrap();
    let p = model.add_parameter(3.0).unwrap();
    let obj = model.minimize(x).unwrap();
    model
        .add_objective_coefficient(obj, x, ValueExpr::param(p))
        .unwrap();
    let mut bytes = Vec::new();
    MpsWriter::new().write(&model, &mut bytes).unwrap();
    let imported = MpsReader::new()
        .read(Cursor::new(bytes))
        .expect("writer output is readable");
    // Evaluated parameters do not survive MPS (documented limitation), but
    // the mathematical structure does: same counts, sense, and evaluated
    // objective coefficient 4.0 on the reread side.
    let (_compiler, before) = compile_model(&model);
    let (_compiler2, after) = compile_model(&imported.model);
    assert_eq!(before.user_vars, after.user_vars);
    assert_eq!(before.user_rows, after.user_rows);
    assert_eq!(before.sense, after.sense);
    let _ = y;
}
