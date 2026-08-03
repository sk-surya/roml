//! P32 Task 16 — logical semantic constructs (design §7, §16; packet Task 16).
//!
//! Covers validation rejections (non-binary activators, duplicate cardinality
//! inputs, invalid `k`, continuous exact reification without separation),
//! exact payload + per-construct formulation-preference storage (A29),
//! compilation through the Task 15 bridge framework (native/bridge selection,
//! origin completeness, `UnboundedBigM` / `UnsupportedFeature`), reification as
//! two implications, exact Boolean/cardinality rows, and small-binary-domain
//! feasible-set enumeration comparing semantic/reference/native/portable
//! feasible sets.

use std::collections::HashMap;

use roml::compiler::backend_ir::{BackendSnapshot, CompiledVariableId};
use roml::compiler::capability::{
    BackendCapabilitySet, BackendFeature, CompilationPolicy, FeatureSupport,
};
use roml::compiler::origin::{EntityOrigin, GeneratedRole};
use roml::compiler::session::CompilationSession;
use roml::compiler::CompileError;
use roml::construct::{
    BooleanKind, CardinalityKind, ConstructKind, FormulationPreference, IndicatorDirection,
};
use roml::id::VarId;
use roml::model::ModelError;
use roml::prelude::*;
use roml::ConstraintBounds;
use roml::ModelRevision;

// ===========================================================================
// Capability helpers
// ===========================================================================

fn full_caps() -> BackendCapabilitySet {
    let mut set = BackendCapabilitySet::new();
    for f in [
        BackendFeature::Lp,
        BackendFeature::Mip,
        BackendFeature::IncrementalBounds,
        BackendFeature::IncrementalRows,
        BackendFeature::IncrementalCoefficients,
    ] {
        set.set(f, FeatureSupport::native(Default::default()));
    }
    set
}

/// Full + the four logical-construct features declared as exact ROML bridges
/// (P32's first bridge declarations — SM-04.2).
fn bridge_caps() -> BackendCapabilitySet {
    let mut set = full_caps();
    for f in [
        BackendFeature::Indicator,
        BackendFeature::Reification,
        BackendFeature::Boolean,
        BackendFeature::Cardinality,
    ] {
        set.set(f, FeatureSupport::bridge(Default::default()));
    }
    set
}

/// Full + `BackendFeature::Indicator` declared native (qualified native).
fn native_indicator_caps() -> BackendCapabilitySet {
    let mut set = full_caps();
    set.set(
        BackendFeature::Indicator,
        FeatureSupport::native(Default::default()),
    );
    set
}

fn compile(
    model: &Model,
    policy: CompilationPolicy,
    caps: &BackendCapabilitySet,
) -> BackendSnapshot {
    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    session
        .compile_snapshot(model.instance(), &snapshot, &policy, caps)
        .expect("snapshot must compile")
}

fn compile_err(
    model: &Model,
    policy: CompilationPolicy,
    caps: &BackendCapabilitySet,
) -> CompileError {
    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    session
        .compile_snapshot(model.instance(), &snapshot, &policy, caps)
        .expect_err("snapshot compilation must fail")
}

// ===========================================================================
// Feasible-set enumeration helpers (small binary domains only)
// ===========================================================================

fn all_assignments(n: usize) -> Vec<Vec<u8>> {
    (0..(1 << n))
        .map(|mask| (0..n).map(|i| ((mask >> i) & 1) as u8).collect())
        .collect()
}

/// Semantic feasible set: assignments satisfying `predicate`.
fn semantic_feasible(n: usize, predicate: impl Fn(&[u8]) -> bool) -> Vec<Vec<u8>> {
    let mut out = all_assignments(n)
        .into_iter()
        .filter(|a| predicate(a))
        .collect::<Vec<_>>();
    out.sort();
    out
}

/// Reference-formulation feasible set: hand-written exact rows evaluated
/// directly over the binary domain (EXECUTION.md: a mathematical reference
/// formulation independent of the bridge implementation).
fn reference_feasible(rows: &[(&[f64], ConstraintBounds)], n: usize) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for a in all_assignments(n) {
        let feasible = rows.iter().all(|(coeffs, bounds)| {
            let sum: f64 = coeffs.iter().zip(&a).map(|(c, v)| c * (*v as f64)).sum();
            bounds.lower - 1e-9 <= sum && sum <= bounds.upper + 1e-9
        });
        if feasible {
            out.push(a);
        }
    }
    out.sort();
    out
}

/// Compiled feasible set: assignments where every compiled row holds.
fn compiled_feasible_assignments(
    compiled: &BackendSnapshot,
    binary_vars: &[VarId],
) -> Vec<Vec<u8>> {
    let n = binary_vars.len();
    let mut out = Vec::new();
    for a in all_assignments(n) {
        let mut values: HashMap<CompiledVariableId, f64> = HashMap::new();
        for (i, &var) in binary_vars.iter().enumerate() {
            let ids = compiled
                .origin_map
                .variables_for_origin(&EntityOrigin::UserVariable(var));
            assert_eq!(ids.len(), 1, "each user variable has one compiled id");
            values.insert(ids[0], a[i] as f64);
        }
        let feasible = compiled.linear_rows.iter().all(|row| {
            let sum: f64 = row
                .coefficients
                .iter()
                .map(|(vid, c)| values.get(vid).copied().unwrap_or(0.0) * c)
                .sum();
            row.bounds.lower - 1e-9 <= sum && sum <= row.bounds.upper + 1e-9
        });
        if feasible {
            out.push(a);
        }
    }
    out.sort();
    out
}

// ===========================================================================
// Validation rejections
// ===========================================================================

#[test]
fn indicator_rejects_non_binary_activator() {
    let mut model = Model::new();
    let z = model.add_variable(continuous()).unwrap();
    let x = model.add_variable(binary()).unwrap();
    let err = model
        .add_indicator(z, IndicatorDirection::WhenOne, (x).le(1.0), None)
        .unwrap_err();
    assert_eq!(err, ModelError::NonBinaryVariable(z));

    // Integer activators are equally rejected.
    let mut model = Model::new();
    let zi = model.add_variable(integer()).unwrap();
    let x = model.add_variable(binary()).unwrap();
    let err = model
        .add_indicator(zi, IndicatorDirection::WhenOne, (x).le(1.0), None)
        .unwrap_err();
    assert_eq!(err, ModelError::NonBinaryVariable(zi));
}

#[test]
fn boolean_rejects_non_binary_variables() {
    let mut model = Model::new();
    let a = model.add_variable(continuous()).unwrap();
    let b = model.add_variable(binary()).unwrap();
    let err = model
        .add_boolean(
            BooleanKind::Implication {
                antecedent: a,
                consequent: b,
            },
            None,
        )
        .unwrap_err();
    assert_eq!(err, ModelError::NonBinaryVariable(a));

    let err = model
        .add_boolean(
            BooleanKind::Any {
                variables: vec![a, b],
            },
            None,
        )
        .unwrap_err();
    assert_eq!(err, ModelError::NonBinaryVariable(a));
}

#[test]
fn cardinality_rejects_duplicate_inputs() {
    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let b = model.add_variable(binary()).unwrap();
    let err = model
        .add_cardinality(vec![a, b, a], CardinalityKind::AtMost, 1.0, None)
        .unwrap_err();
    assert_eq!(err, ModelError::DuplicateCardinalityVariable(a));
}

#[test]
fn cardinality_rejects_negative_k() {
    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let err = model
        .add_cardinality(vec![a], CardinalityKind::Exactly, -1.0, None)
        .unwrap_err();
    assert!(
        matches!(err, ModelError::InvalidCardinalityK { k, .. } if k == -1.0),
        "negative k must be a typed InvalidCardinalityK, got {err:?}"
    );
}

#[test]
fn cardinality_rejects_non_integral_k() {
    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let err = model
        .add_cardinality(vec![a], CardinalityKind::AtMost, 1.5, None)
        .unwrap_err();
    assert!(
        matches!(err, ModelError::InvalidCardinalityK { k, .. } if k == 1.5),
        "non-integral k must be a typed InvalidCardinalityK, got {err:?}"
    );
}

#[test]
fn cardinality_rejects_k_exceeding_input_length() {
    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let b = model.add_variable(binary()).unwrap();
    let err = model
        .add_cardinality(vec![a, b], CardinalityKind::AtLeast, 3.0, None)
        .unwrap_err();
    assert!(
        matches!(err, ModelError::InvalidCardinalityK { k, .. } if k == 3.0),
        "k above the input length must be a typed InvalidCardinalityK, got {err:?}"
    );
}

#[test]
fn cardinality_rejects_non_binary_inputs() {
    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let x = model.add_variable(continuous()).unwrap();
    let err = model
        .add_cardinality(vec![a, x], CardinalityKind::AtMost, 1.0, None)
        .unwrap_err();
    assert_eq!(err, ModelError::NonBinaryVariable(x));
}

#[test]
fn reification_rejects_continuous_expression_without_separation() {
    // Continuous exact reification without an explicit separation tolerance is
    // a typed error (SM-12.2, D14).
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let err = model.add_reify((2.0 * x).ge(5.0), None, None).unwrap_err();
    assert_eq!(err, ModelError::ContinuousReificationWithoutSeparation);
}

#[test]
fn reification_accepts_proven_integral_expression_without_separation() {
    // x binary ⇒ 2x + x is proven integer-valued ⇒ unit gap is inferred.
    let mut model = Model::new();
    let x = model.add_variable(binary()).unwrap();
    let k = model.add_reify((2.0 * x + x).ge(1.0), None, None).unwrap();
    let snap = model.take_snapshot().unwrap();
    let entry = snap.constructs.iter().find(|e| e.id == k).unwrap();
    match &entry.kind {
        ConstructKind::Reification(p) => {
            assert!(
                p.proven_integrality,
                "integer-valued expression is proven integral"
            );
            assert_eq!(p.separation_tolerance, None, "unit gap inferred");
        }
        other => panic!("expected Reification payload, got {other:?}"),
    }
}

#[test]
fn reification_rejects_invalid_separation_tolerance() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let err = model
        .add_reify((2.0 * x).ge(5.0), Some(0.0), None)
        .unwrap_err();
    assert!(
        matches!(err, ModelError::InvalidReificationSeparation(s) if s == 0.0),
        "non-positive separation must be a typed error, got {err:?}"
    );
}

// ===========================================================================
// Payload storage + per-construct preference (A29 single authority)
// ===========================================================================

#[test]
fn builders_store_exact_payloads_and_preference() {
    let mut model = Model::new();
    let z = model.add_variable(binary()).unwrap();
    let x = model.add_variable(binary()).unwrap();
    let y = model.add_variable(binary()).unwrap();

    let ind = model
        .add_indicator(
            z,
            IndicatorDirection::WhenOne,
            (x + y).le(1.0),
            Some(FormulationPreference::Portable),
        )
        .unwrap();
    let reify = model
        .add_reify(
            (x + y).le(1.0),
            Some(0.1),
            Some(FormulationPreference::NativeRequired),
        )
        .unwrap();
    let boo = model
        .add_boolean(
            BooleanKind::Equivalence { left: x, right: y },
            Some(FormulationPreference::Portable),
        )
        .unwrap();
    let card = model
        .add_cardinality(
            vec![x, y],
            CardinalityKind::AtMost,
            1.0,
            Some(FormulationPreference::Auto),
        )
        .unwrap();

    let snap = model.take_snapshot().unwrap();
    assert_eq!(snap.constructs.len(), 4);

    let ind_entry = snap.constructs.iter().find(|e| e.id == ind).unwrap();
    assert!(ind_entry.active);
    assert_eq!(ind_entry.preference, FormulationPreference::Portable);
    match &ind_entry.kind {
        ConstructKind::Indicator(p) => {
            assert_eq!(p.activator, z);
            assert_eq!(p.direction, IndicatorDirection::WhenOne);
        }
        other => panic!("expected Indicator payload, got {other:?}"),
    }

    let reify_entry = snap.constructs.iter().find(|e| e.id == reify).unwrap();
    assert_eq!(
        reify_entry.preference,
        FormulationPreference::NativeRequired
    );
    match &reify_entry.kind {
        ConstructKind::Reification(p) => {
            assert_eq!(p.separation_tolerance, Some(0.1));
        }
        other => panic!("expected Reification payload, got {other:?}"),
    }

    let boo_entry = snap.constructs.iter().find(|e| e.id == boo).unwrap();
    assert_eq!(boo_entry.preference, FormulationPreference::Portable);
    match &boo_entry.kind {
        ConstructKind::Boolean(p) => {
            assert_eq!(p.kind, BooleanKind::Equivalence { left: x, right: y })
        }
        other => panic!("expected Boolean payload, got {other:?}"),
    }

    let card_entry = snap.constructs.iter().find(|e| e.id == card).unwrap();
    assert_eq!(card_entry.preference, FormulationPreference::Auto);
    match &card_entry.kind {
        ConstructKind::Cardinality(p) => {
            assert_eq!(p.kind, CardinalityKind::AtMost);
            assert_eq!(p.k, 1);
            assert_eq!(p.variables, vec![x, y]);
        }
        other => panic!("expected Cardinality payload, got {other:?}"),
    }
}

#[test]
fn construct_snapshot_and_delta_round_trip_preserves_payload_and_preference() {
    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let b = model.add_variable(binary()).unwrap();
    let k = model
        .add_cardinality(
            vec![a, b],
            CardinalityKind::AtMost,
            1.0,
            Some(FormulationPreference::NativeRequired),
        )
        .unwrap();
    let r1 = model.commit().unwrap();

    // Snapshot carries the exact payload + preference.
    let snap = model.take_snapshot().unwrap();
    let snap_entry = snap.constructs.iter().find(|e| e.id == k).unwrap();
    match &snap_entry.kind {
        ConstructKind::Cardinality(p) => {
            assert_eq!(p.kind, CardinalityKind::AtMost);
            assert_eq!(p.k, 1);
            assert_eq!(p.variables, vec![a, b]);
        }
        other => panic!("expected Cardinality payload, got {other:?}"),
    }
    assert_eq!(snap_entry.preference, FormulationPreference::NativeRequired);

    // Delta carries the exact payload + preference (A29 single authority).
    // `DeltaBatch.constructs` is crate-private (P25 F3); the public view is the
    // ordered `AddConstruct` operation carrying the exact payload + preference.
    let batches = model.deltas_since(ModelRevision::ZERO).unwrap();
    let batch = batches.iter().find(|b| b.to == r1).unwrap();
    let add_op = batch
        .operations
        .iter()
        .find_map(|op| match op {
            roml::ModelOp::AddConstruct {
                construct,
                kind,
                preference,
                ..
            } if *construct == k => Some((kind, preference)),
            _ => None,
        })
        .expect("delta carries the AddConstruct operation");
    match &add_op.0 {
        ConstructKind::Cardinality(p) => {
            assert_eq!(p.kind, CardinalityKind::AtMost);
            assert_eq!(p.k, 1);
            assert_eq!(p.variables, vec![a, b]);
        }
        other => panic!("expected Cardinality payload in delta, got {other:?}"),
    }
    assert_eq!(*add_op.1, FormulationPreference::NativeRequired);

    // Deterministic re-snapshot equality (SM-01.4).
    assert_eq!(snap, model.take_snapshot().unwrap());
}

// ===========================================================================
// Compilation — native/bridge selection, origins, errors
// ===========================================================================

#[test]
fn indicator_auto_selects_native_when_declared() {
    let mut model = Model::new();
    let z = model.add_variable(binary()).unwrap();
    let x = model.add_variable(binary()).unwrap();
    model
        .add_indicator(z, IndicatorDirection::WhenOne, (x).le(1.0), None)
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Auto, &native_indicator_caps());

    let dec = compiled
        .report
        .formulation_decisions
        .iter()
        .find(|d| d.decision == "indicator.representation")
        .expect("indicator representation decision recorded");
    assert_eq!(dec.selection, "native indicator");

    // The generated row carries the native indicator role.
    let row = compiled.linear_rows.iter().find(|r| {
        matches!(
            compiled.origin_map.constraint_origin(r.id),
            Some(EntityOrigin::Construct {
                role: GeneratedRole::IndicatorNative,
                ..
            })
        )
    });
    assert!(
        row.is_some(),
        "native indicator row present with IndicatorNative role"
    );
}

#[test]
fn indicator_auto_selects_bridge_when_only_bridge_declared() {
    let mut model = Model::new();
    let z = model.add_variable(binary()).unwrap();
    let x = model.add_variable(binary()).unwrap();
    model
        .add_indicator(z, IndicatorDirection::WhenOne, (x).le(1.0), None)
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Auto, &bridge_caps());

    let dec = compiled
        .report
        .formulation_decisions
        .iter()
        .find(|d| d.decision == "indicator.representation")
        .expect("indicator representation decision recorded");
    assert_eq!(dec.selection, "exact bridge (finite bound)");

    let row = compiled.linear_rows.iter().find(|r| {
        matches!(
            compiled.origin_map.constraint_origin(r.id),
            Some(EntityOrigin::Construct {
                role: GeneratedRole::IndicatorImplicationRow,
                ..
            })
        )
    });
    assert!(
        row.is_some(),
        "bridge indicator row present with IndicatorImplicationRow role"
    );
}

#[test]
fn indicator_portable_forces_bridge_even_with_native_declared() {
    let mut model = Model::new();
    let z = model.add_variable(binary()).unwrap();
    let x = model.add_variable(binary()).unwrap();
    model
        .add_indicator(z, IndicatorDirection::WhenOne, (x).le(1.0), None)
        .unwrap();
    let compiled = compile(
        &model,
        CompilationPolicy::Portable,
        &native_indicator_caps(),
    );

    let dec = compiled
        .report
        .formulation_decisions
        .iter()
        .find(|d| d.decision == "indicator.representation")
        .expect("indicator representation decision recorded");
    assert_eq!(dec.selection, "exact bridge (portable forced)");
}

#[test]
fn indicator_native_required_rejects_without_native() {
    let mut model = Model::new();
    let z = model.add_variable(binary()).unwrap();
    let x = model.add_variable(binary()).unwrap();
    model
        .add_indicator(z, IndicatorDirection::WhenOne, (x).le(1.0), None)
        .unwrap();
    let err = compile_err(&model, CompilationPolicy::NativeRequired, &bridge_caps());
    assert!(
        matches!(err, CompileError::UnsupportedFeature(_)),
        "NativeRequired without native indicator must reject, got {err:?}"
    );
}

#[test]
fn indicator_unqualified_feature_is_unsupported() {
    // No native and no bridge declaration → typed UnsupportedFeature under Auto.
    let mut model = Model::new();
    let z = model.add_variable(binary()).unwrap();
    let x = model.add_variable(binary()).unwrap();
    model
        .add_indicator(z, IndicatorDirection::WhenOne, (x).le(1.0), None)
        .unwrap();
    let err = compile_err(&model, CompilationPolicy::Auto, &full_caps());
    assert!(matches!(err, CompileError::UnsupportedFeature(_)));
}

#[test]
fn indicator_insufficient_bounds_returns_unbounded_big_m() {
    let mut model = Model::new();
    let z = model.add_variable(binary()).unwrap();
    let x = model.add_variable(continuous()).unwrap(); // unbounded above
    let ind = model
        .add_indicator(z, IndicatorDirection::WhenOne, (2.0 * x).le(5.0), None)
        .unwrap();
    let err = compile_err(&model, CompilationPolicy::Auto, &bridge_caps());
    assert!(
        matches!(&err, CompileError::UnboundedBigM { construct, expression }
            if *construct == ind && !expression.is_empty()),
        "insufficient bounds must surface the construct-aware UnboundedBigM naming the construct, got {err:?}"
    );
}

#[test]
fn every_generated_entity_carries_construct_origin() {
    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let b = model.add_variable(binary()).unwrap();
    let c = model.add_variable(binary()).unwrap();
    model
        .add_cardinality(vec![a, b, c], CardinalityKind::AtMost, 1.0, None)
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Auto, &bridge_caps());

    for row in &compiled.linear_rows {
        assert!(
            matches!(
                compiled.origin_map.constraint_origin(row.id),
                Some(EntityOrigin::Construct { .. })
            ),
            "generated row {row:?} must carry a construct origin (SM-02.5)"
        );
    }
    // Completeness validator finds no missing origins (D5).
    assert!(compiled
        .origin_map
        .missing_origins(
            &compiled.variables,
            &compiled.linear_rows,
            &compiled.objectives
        )
        .is_empty());
}

// ===========================================================================
// Reification semantics — two implications, unit gap only from integrality
// ===========================================================================

#[test]
fn reification_compiles_to_two_implications() {
    let mut model = Model::new();
    let x = model.add_variable(binary()).unwrap();
    let y = model.add_variable(binary()).unwrap();
    let k = model.add_reify((x + y).le(1.0), None, None).unwrap(); // proven integral → unit gap
                                                                   // The reification result binary variable is created by the builder and
                                                                   // stored in the payload (design §16.2).
    let snap = model.take_snapshot().unwrap();
    let entry = snap.constructs.iter().find(|e| e.id == k).unwrap();
    match &entry.kind {
        ConstructKind::Reification(p) => {
            assert!(matches!(
                model.variable_bounds(p.activator),
                Some(Bounds::BINARY)
            ));
        }
        other => panic!("expected Reification payload, got {other:?}"),
    }
    let compiled = compile(&model, CompilationPolicy::Auto, &bridge_caps());

    let roles: Vec<GeneratedRole> = compiled
        .linear_rows
        .iter()
        .filter_map(|r| match compiled.origin_map.constraint_origin(r.id) {
            Some(EntityOrigin::Construct { role, .. }) => Some(*role),
            _ => None,
        })
        .collect();
    assert!(
        roles.contains(&GeneratedRole::ReificationImplicationRow),
        "reification emits an implication row, got {roles:?}"
    );
    assert!(
        roles.contains(&GeneratedRole::ReificationComplement),
        "reification emits a complement row, got {roles:?}"
    );
    assert_eq!(
        compiled.linear_rows.len(),
        2,
        "reification is exactly two implications"
    );
}

#[test]
fn reification_honors_explicit_separation_tolerance() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model
        .add_reify((2.0 * x).ge(5.0), Some(1e-6), None)
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Auto, &bridge_caps());

    // Complement row enforces `b=0 ⇒ f(x) <= rhs - sep`, i.e. `2x <= 5 - 1e-6`.
    let complement = compiled
        .linear_rows
        .iter()
        .find(|r| {
            matches!(
                compiled.origin_map.constraint_origin(r.id),
                Some(EntityOrigin::Construct {
                    role: GeneratedRole::ReificationComplement,
                    ..
                })
            )
        })
        .expect("complement row present");
    assert!(
        (complement.bounds.upper - (5.0 - 1e-6)).abs() < 1e-6,
        "complement upper bound must reflect rhs - separation, got {:?}",
        complement.bounds
    );
}

// ===========================================================================
// Feasible-set enumeration — semantic/reference/native/portable equality
// ===========================================================================

#[test]
fn indicator_feasible_sets_semantic_reference_native_portable_equal() {
    let mut model = Model::new();
    let z = model.add_variable(binary()).unwrap();
    let x = model.add_variable(binary()).unwrap();
    let y = model.add_variable(binary()).unwrap();
    model
        .add_indicator(z, IndicatorDirection::WhenOne, (x + y).le(1.0), None)
        .unwrap();
    let vars = [z, x, y];

    // semantic: z=1 ⇒ x+y <= 1
    let semantic = semantic_feasible(3, |a| !(a[0] == 1) || (a[1] + a[2] <= 1));

    // reference: exact indicator row `x + y + M z <= 1 + M` with M = max(0, max(x+y) - 1) = 1
    let reference = reference_feasible(&[(&[1.0, 1.0, 1.0][..], ConstraintBounds::le(2.0))], 3);

    // native (Auto + native Indicator)
    let compiled_native = compile(&model, CompilationPolicy::Auto, &native_indicator_caps());
    let native = compiled_feasible_assignments(&compiled_native, &vars);

    // portable bridge
    let compiled_portable = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let portable = compiled_feasible_assignments(&compiled_portable, &vars);

    assert_eq!(semantic, reference);
    assert_eq!(
        semantic, native,
        "native feasible set must equal the semantic set"
    );
    assert_eq!(
        semantic, portable,
        "portable feasible set must equal the semantic set"
    );
}

#[test]
fn indicator_when_zero_feasible_sets_match_semantic() {
    let mut model = Model::new();
    let z = model.add_variable(binary()).unwrap();
    let x = model.add_variable(binary()).unwrap();
    let y = model.add_variable(binary()).unwrap();
    model
        .add_indicator(z, IndicatorDirection::WhenZero, (x + y).le(1.0), None)
        .unwrap();
    let vars = [z, x, y];

    // semantic: z=0 ⇒ x+y <= 1
    let semantic = semantic_feasible(3, |a| !(a[0] == 0) || (a[1] + a[2] <= 1));

    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let portable = compiled_feasible_assignments(&compiled, &vars);

    assert_eq!(semantic, portable);
}

#[test]
fn reification_feasible_sets_semantic_reference_portable_equal() {
    let mut model = Model::new();
    let x = model.add_variable(binary()).unwrap();
    let y = model.add_variable(binary()).unwrap();
    let k = model.add_reify((x + y).le(1.0), None, None).unwrap(); // proven integral → unit gap
                                                                   // The reification result binary variable is created by the builder.
    let snap = model.take_snapshot().unwrap();
    let b = match &snap.constructs.iter().find(|e| e.id == k).unwrap().kind {
        ConstructKind::Reification(p) => p.activator,
        other => panic!("expected Reification payload, got {other:?}"),
    };
    let vars = [b, x, y];

    // semantic: b ⟺ (x+y <= 1)
    let semantic = semantic_feasible(3, |a| {
        let lhs = a[1] + a[2];
        (a[0] == 1) == (lhs <= 1)
    });

    // reference (assignment order [b, x, y]): `x + y + b <= 2` and
    // `x + y + 2b >= 2` (unit gap).
    let reference = reference_feasible(
        &[
            (&[1.0, 1.0, 1.0][..], ConstraintBounds::le(2.0)),
            (&[2.0, 1.0, 1.0][..], ConstraintBounds::ge(2.0)),
        ],
        3,
    );

    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let portable = compiled_feasible_assignments(&compiled, &vars);

    assert_eq!(semantic, reference);
    assert_eq!(
        semantic, portable,
        "portable reification feasible set must equal semantic"
    );
}

#[test]
fn boolean_implication_feasible_sets_match_semantic() {
    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let b = model.add_variable(binary()).unwrap();
    model
        .add_boolean(
            BooleanKind::Implication {
                antecedent: a,
                consequent: b,
            },
            None,
        )
        .unwrap();
    let vars = [a, b];

    // semantic: a ⇒ b, i.e. a <= b
    let semantic = semantic_feasible(2, |a| a[0] <= a[1]);

    // reference: `a - b <= 0`
    let reference = reference_feasible(&[(&[1.0, -1.0][..], ConstraintBounds::le(0.0))], 2);

    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let portable = compiled_feasible_assignments(&compiled, &vars);

    assert_eq!(semantic, reference);
    assert_eq!(semantic, portable);
}

#[test]
fn boolean_equivalence_feasible_sets_match_semantic() {
    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let b = model.add_variable(binary()).unwrap();
    model
        .add_boolean(BooleanKind::Equivalence { left: a, right: b }, None)
        .unwrap();
    let vars = [a, b];

    let semantic = semantic_feasible(2, |a| a[0] == a[1]);
    let reference = reference_feasible(
        &[
            (&[1.0, -1.0][..], ConstraintBounds::le(0.0)),
            (&[-1.0, 1.0][..], ConstraintBounds::le(0.0)),
        ],
        2,
    );

    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let portable = compiled_feasible_assignments(&compiled, &vars);

    assert_eq!(semantic, reference);
    assert_eq!(semantic, portable);
}

#[test]
fn boolean_any_all_feasible_sets_match_semantic() {
    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let b = model.add_variable(binary()).unwrap();
    let c = model.add_variable(binary()).unwrap();
    model
        .add_boolean(
            BooleanKind::Any {
                variables: vec![a, b, c],
            },
            None,
        )
        .unwrap();
    let vars = [a, b, c];
    let semantic = semantic_feasible(3, |a| a[0] + a[1] + a[2] >= 1);
    let reference = reference_feasible(&[(&[1.0, 1.0, 1.0][..], ConstraintBounds::ge(1.0))], 3);
    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    assert_eq!(semantic, reference);
    assert_eq!(semantic, compiled_feasible_assignments(&compiled, &vars));

    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let b = model.add_variable(binary()).unwrap();
    let c = model.add_variable(binary()).unwrap();
    model
        .add_boolean(
            BooleanKind::All {
                variables: vec![a, b, c],
            },
            None,
        )
        .unwrap();
    let vars = [a, b, c];
    let semantic = semantic_feasible(3, |a| a[0] + a[1] + a[2] >= 3);
    let reference = reference_feasible(&[(&[1.0, 1.0, 1.0][..], ConstraintBounds::ge(3.0))], 3);
    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    assert_eq!(semantic, reference);
    assert_eq!(semantic, compiled_feasible_assignments(&compiled, &vars));
}

#[test]
fn cardinality_feasible_sets_match_semantic() {
    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let b = model.add_variable(binary()).unwrap();
    let c = model.add_variable(binary()).unwrap();
    model
        .add_cardinality(vec![a, b, c], CardinalityKind::Exactly, 1.0, None)
        .unwrap();
    let vars = [a, b, c];
    let semantic = semantic_feasible(3, |a| a[0] + a[1] + a[2] == 1);
    let reference = reference_feasible(&[(&[1.0, 1.0, 1.0][..], ConstraintBounds::eq(1.0))], 3);
    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    assert_eq!(semantic, reference);
    assert_eq!(semantic, compiled_feasible_assignments(&compiled, &vars));

    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let b = model.add_variable(binary()).unwrap();
    let c = model.add_variable(binary()).unwrap();
    model
        .add_cardinality(vec![a, b, c], CardinalityKind::AtMost, 2.0, None)
        .unwrap();
    let vars = [a, b, c];
    let semantic = semantic_feasible(3, |a| a[0] + a[1] + a[2] <= 2);
    let reference = reference_feasible(&[(&[1.0, 1.0, 1.0][..], ConstraintBounds::le(2.0))], 3);
    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    assert_eq!(semantic, reference);
    assert_eq!(semantic, compiled_feasible_assignments(&compiled, &vars));

    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let b = model.add_variable(binary()).unwrap();
    let c = model.add_variable(binary()).unwrap();
    model
        .add_cardinality(vec![a, b, c], CardinalityKind::AtLeast, 2.0, None)
        .unwrap();
    let vars = [a, b, c];
    let semantic = semantic_feasible(3, |a| a[0] + a[1] + a[2] >= 2);
    let reference = reference_feasible(&[(&[1.0, 1.0, 1.0][..], ConstraintBounds::ge(2.0))], 3);
    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    assert_eq!(semantic, reference);
    assert_eq!(semantic, compiled_feasible_assignments(&compiled, &vars));
}
