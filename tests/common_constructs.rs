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
    AbsoluteValueVariant, BooleanKind, CardinalityKind, ConstructKind, FormulationPreference,
    IndicatorDirection, MinMaxRelation, MinMaxSense,
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

/// Full + the four logical-construct features AND the three algebraic-construct
/// features declared as exact ROML bridges (P32's bridge declarations —
/// SM-04.2).
fn bridge_caps() -> BackendCapabilitySet {
    let mut set = full_caps();
    for f in [
        BackendFeature::Indicator,
        BackendFeature::Reification,
        BackendFeature::Boolean,
        BackendFeature::Cardinality,
        BackendFeature::MinMax,
        BackendFeature::AbsoluteValue,
        BackendFeature::BinaryProduct,
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
// Algebraic-construct helpers (P32 Task 17a/17b/17c)
// ===========================================================================

/// The compiled id of a user variable (exactly one compiled projection).
fn compiled_user_var(compiled: &BackendSnapshot, var: VarId) -> CompiledVariableId {
    let ids = compiled
        .origin_map
        .variables_for_origin(&EntityOrigin::UserVariable(var));
    assert_eq!(ids.len(), 1, "each user variable has one compiled id");
    ids[0]
}

/// All compiled linear rows hold for the given fixed compiled-variable values.
fn rows_hold(compiled: &BackendSnapshot, values: &HashMap<CompiledVariableId, f64>) -> bool {
    compiled.linear_rows.iter().all(|row| {
        let sum: f64 = row
            .coefficients
            .iter()
            .map(|(vid, c)| values.get(vid).copied().unwrap_or(0.0) * c)
            .sum();
        row.bounds.lower - 1e-9 <= sum && sum <= row.bounds.upper + 1e-9
    })
}

/// Whether the fixed assignment is feasible in the compiled snapshot: every
/// fixed value respects its compiled variable bounds, and SOME assignment of
/// the generated binaries makes every row hold. This is the compiled feasible
/// set projected onto the fixed user/generated variables (existential over the
/// generated binaries).
fn assignment_feasible(
    compiled: &BackendSnapshot,
    fixed: &HashMap<CompiledVariableId, f64>,
    generated_binaries: &[CompiledVariableId],
) -> bool {
    for (&vid, &val) in fixed {
        if let Some(cv) = compiled.variables.iter().find(|v| v.id == vid) {
            if val < cv.bounds.lower - 1e-9 || val > cv.bounds.upper + 1e-9 {
                return false;
            }
        }
    }
    if generated_binaries.is_empty() {
        return rows_hold(compiled, fixed);
    }
    for mask in 0..(1usize << generated_binaries.len()) {
        let mut values = fixed.clone();
        for (i, &bid) in generated_binaries.iter().enumerate() {
            values.insert(bid, ((mask >> i) & 1) as f64);
        }
        if rows_hold(compiled, &values) {
            return true;
        }
    }
    false
}

/// Generated binary variable ids for a construct role.
fn generated_binaries(
    compiled: &BackendSnapshot,
    construct: roml::Construct,
    role: GeneratedRole,
) -> Vec<CompiledVariableId> {
    compiled
        .origin_map
        .variables_for_origin(&EntityOrigin::Construct { construct, role })
}

/// The single generated variable id for a construct role.
fn generated_var_for(
    compiled: &BackendSnapshot,
    construct: roml::Construct,
    role: GeneratedRole,
) -> CompiledVariableId {
    let ids = compiled
        .origin_map
        .variables_for_origin(&EntityOrigin::Construct { construct, role });
    assert_eq!(
        ids.len(),
        1,
        "expected exactly one generated variable for role {role:?}, got {ids:?}"
    );
    ids[0]
}

/// A tiny deterministic LCG for fixed-seed "random" tests (no external dep).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    /// Uniform integer in `[lo, hi]` (inclusive).
    fn range(&mut self, lo: i64, hi: i64) -> f64 {
        let span = (hi - lo + 1) as u64;
        lo as f64 + (self.next() % span) as f64
    }
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

// ===========================================================================
// Task 17a — min/max (exact vs one-sided, selector, bounds, direct eval)
// ===========================================================================

#[test]
fn minmax_rejects_fewer_than_two_operands() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let err = model
        .add_minmax(
            vec![x.into()],
            MinMaxSense::Max,
            MinMaxRelation::Exact,
            None,
        )
        .unwrap_err();
    assert_eq!(err, ModelError::MinMaxTooFewOperands);
}

#[test]
fn minmax_rejects_trivially_satisfiable_relations() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    // A min epigraph (output >= min) is trivially satisfiable — reject.
    let err = model
        .add_minmax(
            vec![x.into(), y.into()],
            MinMaxSense::Min,
            MinMaxRelation::Epigraph,
            None,
        )
        .unwrap_err();
    assert_eq!(err, ModelError::TriviallySatisfiableMinMax);
    // A max hypograph (output <= max) is trivially satisfiable — reject.
    let err = model
        .add_minmax(
            vec![x.into(), y.into()],
            MinMaxSense::Max,
            MinMaxRelation::Hypograph,
            None,
        )
        .unwrap_err();
    assert_eq!(err, ModelError::TriviallySatisfiableMinMax);
}

#[test]
fn minmax_payload_stores_operands_sense_relation_and_output() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let (k, output) = model
        .add_minmax(
            vec![x.into(), y.into()],
            MinMaxSense::Max,
            MinMaxRelation::Epigraph,
            Some(FormulationPreference::Portable),
        )
        .unwrap();
    let snap = model.take_snapshot().unwrap();
    let entry = snap.constructs.iter().find(|e| e.id == k).unwrap();
    assert_eq!(entry.preference, FormulationPreference::Portable);
    match &entry.kind {
        ConstructKind::MinMax(p) => {
            assert_eq!(p.sense, MinMaxSense::Max);
            assert_eq!(p.relation, MinMaxRelation::Epigraph);
            assert_eq!(p.output, output);
            assert_eq!(p.operands.len(), 2);
        }
        other => panic!("expected MinMax payload, got {other:?}"),
    }
}

#[test]
fn minmax_exact_vs_one_sided_feasible_sets_differ_with_no_objective() {
    // D13: exactness is never inferred from objective context. With x1 = 3,
    // x2 = 5 fixed and NO objective, the exact-min set is {y = 3} while the
    // hypograph-min set also admits y = 0.
    let mut model = Model::new();
    let x1 = model.add_variable(continuous().bounds(3.0, 3.0)).unwrap();
    let x2 = model.add_variable(continuous().bounds(5.0, 5.0)).unwrap();
    let (k_exact, y_exact) = model
        .add_minmax(
            vec![x1.into(), x2.into()],
            MinMaxSense::Min,
            MinMaxRelation::Exact,
            None,
        )
        .unwrap();
    let compiled_exact = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let sel = generated_binaries(
        &compiled_exact,
        k_exact,
        GeneratedRole::MinMaxSelectorBinary,
    );
    assert_eq!(
        sel.len(),
        2,
        "exact min has one selector binary per operand"
    );
    let x1id = compiled_user_var(&compiled_exact, x1);
    let x2id = compiled_user_var(&compiled_exact, x2);
    let yid = compiled_user_var(&compiled_exact, y_exact);
    let mut fixed = HashMap::new();
    fixed.insert(x1id, 3.0);
    fixed.insert(x2id, 5.0);
    fixed.insert(yid, 0.0);
    assert!(
        !assignment_feasible(&compiled_exact, &fixed, &sel),
        "exact-min must NOT admit y=0 with x1=3, x2=5 (D13 difference proof)"
    );
    fixed.insert(yid, 3.0);
    assert!(
        assignment_feasible(&compiled_exact, &fixed, &sel),
        "exact-min must admit y=3 = min(3,5)"
    );

    // Hypograph-min: rows y <= 3, y <= 5 — y=0 IS feasible.
    let mut model = Model::new();
    let x1 = model.add_variable(continuous().bounds(3.0, 3.0)).unwrap();
    let x2 = model.add_variable(continuous().bounds(5.0, 5.0)).unwrap();
    let (k_hypo, y_hypo) = model
        .add_minmax(
            vec![x1.into(), x2.into()],
            MinMaxSense::Min,
            MinMaxRelation::Hypograph,
            None,
        )
        .unwrap();
    let compiled_hypo = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let sel_hypo = generated_binaries(&compiled_hypo, k_hypo, GeneratedRole::MinMaxSelectorBinary);
    assert!(
        sel_hypo.is_empty(),
        "hypograph-min has zero generated binaries"
    );
    let mut fixed = HashMap::new();
    fixed.insert(compiled_user_var(&compiled_hypo, x1), 3.0);
    fixed.insert(compiled_user_var(&compiled_hypo, x2), 5.0);
    fixed.insert(compiled_user_var(&compiled_hypo, y_hypo), 0.0);
    assert!(
        assignment_feasible(&compiled_hypo, &fixed, &[]),
        "hypograph-min must admit y=0 — the one-sided feasible set differs from the exact set (D13)"
    );

    // Mirror for max: exact-max set is {y = 5}; max-epigraph also admits y = 7.
    let mut model = Model::new();
    let x1 = model.add_variable(continuous().bounds(3.0, 3.0)).unwrap();
    let x2 = model.add_variable(continuous().bounds(5.0, 5.0)).unwrap();
    let (k_exact, y_exact) = model
        .add_minmax(
            vec![x1.into(), x2.into()],
            MinMaxSense::Max,
            MinMaxRelation::Exact,
            None,
        )
        .unwrap();
    let compiled_exact = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let sel = generated_binaries(
        &compiled_exact,
        k_exact,
        GeneratedRole::MinMaxSelectorBinary,
    );
    let mut fixed = HashMap::new();
    fixed.insert(compiled_user_var(&compiled_exact, x1), 3.0);
    fixed.insert(compiled_user_var(&compiled_exact, x2), 5.0);
    fixed.insert(compiled_user_var(&compiled_exact, y_exact), 7.0);
    assert!(
        !assignment_feasible(&compiled_exact, &fixed, &sel),
        "exact-max must NOT admit y=7 with x1=3, x2=5 (D13)"
    );

    let mut model = Model::new();
    let x1 = model.add_variable(continuous().bounds(3.0, 3.0)).unwrap();
    let x2 = model.add_variable(continuous().bounds(5.0, 5.0)).unwrap();
    let (k_epi, y_epi) = model
        .add_minmax(
            vec![x1.into(), x2.into()],
            MinMaxSense::Max,
            MinMaxRelation::Epigraph,
            None,
        )
        .unwrap();
    let compiled_epi = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let sel_epi = generated_binaries(&compiled_epi, k_epi, GeneratedRole::MinMaxSelectorBinary);
    assert!(
        sel_epi.is_empty(),
        "max-epigraph has zero generated binaries"
    );
    let mut fixed = HashMap::new();
    fixed.insert(compiled_user_var(&compiled_epi, x1), 3.0);
    fixed.insert(compiled_user_var(&compiled_epi, x2), 5.0);
    fixed.insert(compiled_user_var(&compiled_epi, y_epi), 7.0);
    assert!(
        assignment_feasible(&compiled_epi, &fixed, &[]),
        "max-epigraph must admit y=7 — the one-sided feasible set differs from the exact set (D13)"
    );
}

#[test]
fn minmax_one_sided_rows_have_zero_binaries_and_distinct_roles() {
    // Max epigraph: exactly the rows x1 <= y, x2 <= y with zero generated
    // binaries and MinMaxEpigraphRow roles.
    let mut model = Model::new();
    let x1 = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let x2 = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let (k, y) = model
        .add_minmax(
            vec![x1.into(), x2.into()],
            MinMaxSense::Max,
            MinMaxRelation::Epigraph,
            None,
        )
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let generated_vars = compiled
        .variables
        .iter()
        .filter(|v| {
            matches!(
                compiled.origin_map.variable_origin(v.id),
                Some(EntityOrigin::Construct { construct, .. }) if *construct == k
            )
        })
        .count();
    assert_eq!(generated_vars, 0, "max epigraph generates zero variables");
    let epi_rows: Vec<_> = compiled
        .linear_rows
        .iter()
        .filter(|r| {
            matches!(
                compiled.origin_map.constraint_origin(r.id),
                Some(EntityOrigin::Construct {
                    role: GeneratedRole::MinMaxEpigraphRow,
                    ..
                })
            )
        })
        .collect();
    assert_eq!(epi_rows.len(), 2, "max epigraph is exactly two rows");
    let yid = compiled_user_var(&compiled, y);
    for row in &epi_rows {
        // each row has the operand coefficient + (-1)·y, i.e. x_i - y <= 0.
        assert!(
            row.bounds.upper.abs() < 1e-9,
            "x_i - y <= 0, got {:?}",
            row.bounds
        );
        let y_coeff = row
            .coefficients
            .iter()
            .find(|(id, _)| *id == yid)
            .map(|(_, c)| *c);
        assert_eq!(y_coeff, Some(-1.0), "row must contain -y");
    }

    // Min hypograph: exactly the rows x1 >= y, x2 >= y with zero binaries and
    // MinMaxHypographRow roles.
    let mut model = Model::new();
    let x1 = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let x2 = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let (k, y) = model
        .add_minmax(
            vec![x1.into(), x2.into()],
            MinMaxSense::Min,
            MinMaxRelation::Hypograph,
            None,
        )
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let generated_vars = compiled
        .variables
        .iter()
        .filter(|v| {
            matches!(
                compiled.origin_map.variable_origin(v.id),
                Some(EntityOrigin::Construct { construct, .. }) if *construct == k
            )
        })
        .count();
    assert_eq!(generated_vars, 0, "min hypograph generates zero variables");
    let hypo_rows: Vec<_> = compiled
        .linear_rows
        .iter()
        .filter(|r| {
            matches!(
                compiled.origin_map.constraint_origin(r.id),
                Some(EntityOrigin::Construct {
                    role: GeneratedRole::MinMaxHypographRow,
                    ..
                })
            )
        })
        .collect();
    assert_eq!(hypo_rows.len(), 2, "min hypograph is exactly two rows");
    let yid = compiled_user_var(&compiled, y);
    for row in &hypo_rows {
        assert!(
            row.bounds.lower.abs() < 1e-9,
            "x_i - y >= 0, got {:?}",
            row.bounds
        );
        let y_coeff = row
            .coefficients
            .iter()
            .find(|(id, _)| *id == yid)
            .map(|(_, c)| *c);
        assert_eq!(y_coeff, Some(-1.0), "row must contain -y");
    }
}

#[test]
fn minmax_exact_selector_feasible_sets_match_semantic() {
    // Exact max over 3 binary operands: the compiled feasible set over
    // (x1,x2,x3,y) equals { y = max(x1,x2,x3) }.
    let mut model = Model::new();
    let x1 = model.add_variable(binary()).unwrap();
    let x2 = model.add_variable(binary()).unwrap();
    let x3 = model.add_variable(binary()).unwrap();
    let (k, y) = model
        .add_minmax(
            vec![x1.into(), x2.into(), x3.into()],
            MinMaxSense::Max,
            MinMaxRelation::Exact,
            None,
        )
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let sel = generated_binaries(&compiled, k, GeneratedRole::MinMaxSelectorBinary);
    assert_eq!(sel.len(), 3, "one selector binary per exact-max operand");
    let ids = [
        compiled_user_var(&compiled, x1),
        compiled_user_var(&compiled, x2),
        compiled_user_var(&compiled, x3),
    ];
    let yid = compiled_user_var(&compiled, y);
    for a in 0..2 {
        for b in 0..2 {
            for c in 0..2 {
                for yv in 0..2 {
                    let semantic = (yv as f64) == (a.max(b).max(c) as f64);
                    let mut fixed = HashMap::new();
                    fixed.insert(ids[0], a as f64);
                    fixed.insert(ids[1], b as f64);
                    fixed.insert(ids[2], c as f64);
                    fixed.insert(yid, yv as f64);
                    let feasible = assignment_feasible(&compiled, &fixed, &sel);
                    assert_eq!(
                        feasible, semantic,
                        "exact-max feasible set mismatch at (x1,x2,x3,y)=({a},{b},{c},{yv})"
                    );
                }
            }
        }
    }

    // The report records the finite derived M values with bound sources
    // (SM-13.5).
    let m_entries: Vec<_> = compiled
        .report
        .formulation_decisions
        .iter()
        .filter(|d| d.decision.starts_with("minmax.selector_m"))
        .collect();
    assert_eq!(m_entries.len(), 3, "one M record per exact-max operand");
    for e in &m_entries {
        assert!(
            e.selection.starts_with("M = "),
            "finite M recorded, got {}",
            e.selection
        );
        assert!(
            e.reason.contains("u_max"),
            "M derivation names u_max, got {}",
            e.reason
        );
    }

    // Exact min mirror.
    let mut model = Model::new();
    let x1 = model.add_variable(binary()).unwrap();
    let x2 = model.add_variable(binary()).unwrap();
    let (k, y) = model
        .add_minmax(
            vec![x1.into(), x2.into()],
            MinMaxSense::Min,
            MinMaxRelation::Exact,
            None,
        )
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let sel = generated_binaries(&compiled, k, GeneratedRole::MinMaxSelectorBinary);
    assert_eq!(sel.len(), 2);
    let ids = [
        compiled_user_var(&compiled, x1),
        compiled_user_var(&compiled, x2),
    ];
    let yid = compiled_user_var(&compiled, y);
    for a in 0..2 {
        for b in 0..2 {
            for yv in 0..2 {
                let semantic = (yv as f64) == (a.min(b) as f64);
                let mut fixed = HashMap::new();
                fixed.insert(ids[0], a as f64);
                fixed.insert(ids[1], b as f64);
                fixed.insert(yid, yv as f64);
                let feasible = assignment_feasible(&compiled, &fixed, &sel);
                assert_eq!(
                    feasible, semantic,
                    "exact-min feasible set mismatch at (x1,x2,y)=({a},{b},{yv})"
                );
            }
        }
    }
}

#[test]
fn minmax_exact_rejects_unbounded_operand_with_construct_aware_error() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap(); // free
    let y = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let (k, _) = model
        .add_minmax(
            vec![x.into(), y.into()],
            MinMaxSense::Max,
            MinMaxRelation::Exact,
            None,
        )
        .unwrap();
    let err = compile_err(&model, CompilationPolicy::Portable, &bridge_caps());
    assert!(
        matches!(&err, CompileError::UnboundedBigM { construct, expression }
            if *construct == k && !expression.is_empty()),
        "unbounded exact-max operand must surface the construct-aware UnboundedBigM naming the \
         construct, got {err:?}"
    );
}

#[test]
fn minmax_randomized_direct_evaluation_matches_max_and_min() {
    // Fixed-seed random bounded operands: the exact compiled rows force
    // y = max/min at every sampled operand value (direct evaluation, no
    // solver). Sample operands strictly inside their bounds so the selector
    // rows — not the output bounds — are what reject y = max ± 0.5.
    let mut rng = Lcg(0x5eed_1234);
    for _ in 0..5 {
        // Max.
        let (l1, u1) = (-5.0, 5.0);
        let (l2, u2) = (-5.0, 5.0);
        let mut model = Model::new();
        let x1 = model.add_variable(continuous().bounds(l1, u1)).unwrap();
        let x2 = model.add_variable(continuous().bounds(l2, u2)).unwrap();
        let (k, y) = model
            .add_minmax(
                vec![x1.into(), x2.into()],
                MinMaxSense::Max,
                MinMaxRelation::Exact,
                None,
            )
            .unwrap();
        let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
        let sel = generated_binaries(&compiled, k, GeneratedRole::MinMaxSelectorBinary);
        let x1id = compiled_user_var(&compiled, x1);
        let x2id = compiled_user_var(&compiled, x2);
        let yid = compiled_user_var(&compiled, y);
        for _ in 0..5 {
            let v1 = rng.range(-4, 4);
            let v2 = rng.range(-4, 4);
            let m = v1.max(v2);
            let mut fixed = HashMap::new();
            fixed.insert(x1id, v1);
            fixed.insert(x2id, v2);
            fixed.insert(yid, m);
            assert!(
                assignment_feasible(&compiled, &fixed, &sel),
                "y = max({v1},{v2}) = {m} must be feasible"
            );
            fixed.insert(yid, m + 0.5);
            assert!(
                !assignment_feasible(&compiled, &fixed, &sel),
                "y = max + 0.5 must be infeasible (exact selector)"
            );
        }

        // Min.
        let mut model = Model::new();
        let x1 = model.add_variable(continuous().bounds(-5.0, 5.0)).unwrap();
        let x2 = model.add_variable(continuous().bounds(-5.0, 5.0)).unwrap();
        let (k, y) = model
            .add_minmax(
                vec![x1.into(), x2.into()],
                MinMaxSense::Min,
                MinMaxRelation::Exact,
                None,
            )
            .unwrap();
        let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
        let sel = generated_binaries(&compiled, k, GeneratedRole::MinMaxSelectorBinary);
        let x1id = compiled_user_var(&compiled, x1);
        let x2id = compiled_user_var(&compiled, x2);
        let yid = compiled_user_var(&compiled, y);
        for _ in 0..5 {
            let v1 = rng.range(-4, 4);
            let v2 = rng.range(-4, 4);
            let m = v1.min(v2);
            let mut fixed = HashMap::new();
            fixed.insert(x1id, v1);
            fixed.insert(x2id, v2);
            fixed.insert(yid, m);
            assert!(
                assignment_feasible(&compiled, &fixed, &sel),
                "y = min({v1},{v2}) = {m} must be feasible"
            );
            fixed.insert(yid, m - 0.5);
            assert!(
                !assignment_feasible(&compiled, &fixed, &sel),
                "y = min - 0.5 must be infeasible (exact selector)"
            );
        }
    }
}

// ===========================================================================
// Task 17b — absolute value / positive part / clamp
// ===========================================================================

#[test]
fn absolute_value_rejects_unbounded_expression() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap(); // free
    let err = model
        .add_absolute_value(x.into(), AbsoluteValueVariant::Absolute, None)
        .unwrap_err();
    assert_eq!(err, ModelError::UnboundedConstructExpression);

    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(-5.0, 5.0)).unwrap();
    let p = model.add_parameter(2.0).unwrap();
    // A bare-parameter expression whose value is finite is fine; an expression
    // with a free variable is not.
    let err = model
        .add_absolute_value(p * x, AbsoluteValueVariant::PositivePart, None)
        .is_ok();
    assert!(err, "bounded parameterized expression must be accepted");
}

#[test]
fn absolute_value_rejects_invalid_clamp_bounds() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let err = model
        .add_absolute_value(
            x.into(),
            AbsoluteValueVariant::Clamp {
                lower: 5.0,
                upper: 1.0,
            },
            None,
        )
        .unwrap_err();
    assert!(
        matches!(err, ModelError::InvalidClampBounds { lower, upper } if lower == 5.0 && upper == 1.0),
        "lower > upper must be a typed InvalidClampBounds, got {err:?}"
    );
    let err = model
        .add_absolute_value(
            x.into(),
            AbsoluteValueVariant::Clamp {
                lower: f64::NAN,
                upper: 1.0,
            },
            None,
        )
        .unwrap_err();
    assert!(matches!(err, ModelError::InvalidClampBounds { .. }));
}

#[test]
fn absolute_value_payload_stores_expression_variant_and_output() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(-5.0, 5.0)).unwrap();
    let (k, output) = model
        .add_absolute_value(
            x.into(),
            AbsoluteValueVariant::Clamp {
                lower: -1.0,
                upper: 2.0,
            },
            Some(FormulationPreference::Portable),
        )
        .unwrap();
    let snap = model.take_snapshot().unwrap();
    let entry = snap.constructs.iter().find(|e| e.id == k).unwrap();
    assert_eq!(entry.preference, FormulationPreference::Portable);
    match &entry.kind {
        ConstructKind::AbsoluteValue(p) => {
            assert_eq!(p.output, output);
            assert_eq!(
                p.variant,
                AbsoluteValueVariant::Clamp {
                    lower: -1.0,
                    upper: 2.0
                }
            );
        }
        other => panic!("expected AbsoluteValue payload, got {other:?}"),
    }
}

#[test]
fn absolute_value_unbounded_at_compile_returns_construct_aware_error() {
    // The builder accepts the bounded expression; removing the bound afterwards
    // makes the bridge see an unbounded interval at compile time — a typed
    // `UnboundedBigM` naming the construct and expression (SM-13.4, D12).
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let (k, _) = model
        .add_absolute_value(x.into(), AbsoluteValueVariant::Absolute, None)
        .unwrap();
    model.set_variable_bounds(x, Bounds::UNBOUNDED).unwrap();
    let err = compile_err(&model, CompilationPolicy::Portable, &bridge_caps());
    assert!(
        matches!(&err, CompileError::UnboundedBigM { construct, expression }
            if *construct == k && !expression.is_empty()),
        "unbounded abs expression must surface the construct-aware UnboundedBigM, got {err:?}"
    );
}

#[test]
fn absolute_value_exact_feasible_set_matches_semantic() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(-2.0, 2.0)).unwrap();
    let (k, z) = model
        .add_absolute_value(x.into(), AbsoluteValueVariant::Absolute, None)
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let b = generated_var_for(&compiled, k, GeneratedRole::AbsoluteValueSelectorBinary);
    let p = generated_var_for(&compiled, k, GeneratedRole::AbsoluteValuePositivePartRow);
    let n = generated_var_for(&compiled, k, GeneratedRole::AbsoluteValueNegativePartRow);
    let xid = compiled_user_var(&compiled, x);
    let zid = compiled_user_var(&compiled, z);
    for xv in -2..=2 {
        for zv in -2..=2 {
            let semantic = (zv as f64) == (xv as f64).abs();
            let mut fixed = HashMap::new();
            fixed.insert(xid, xv as f64);
            fixed.insert(zid, zv as f64);
            fixed.insert(p, (xv as f64).max(0.0));
            fixed.insert(n, (-(xv as f64)).max(0.0));
            let feasible = assignment_feasible(&compiled, &fixed, &[b]);
            assert_eq!(
                feasible, semantic,
                "z = |x| feasible set mismatch at x={xv}, z={zv}"
            );
        }
    }
}

#[test]
fn positive_part_exact_feasible_set_matches_semantic() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(-2.0, 2.0)).unwrap();
    let (k, z) = model
        .add_absolute_value(x.into(), AbsoluteValueVariant::PositivePart, None)
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let b = generated_var_for(&compiled, k, GeneratedRole::AbsoluteValueSelectorBinary);
    let n = generated_var_for(&compiled, k, GeneratedRole::AbsoluteValueNegativePartRow);
    let xid = compiled_user_var(&compiled, x);
    let zid = compiled_user_var(&compiled, z);
    for xv in -2..=2 {
        for zv in -2..=2 {
            let semantic = (zv as f64) == (xv as f64).max(0.0);
            let mut fixed = HashMap::new();
            fixed.insert(xid, xv as f64);
            fixed.insert(zid, zv as f64);
            fixed.insert(n, (-(xv as f64)).max(0.0));
            let feasible = assignment_feasible(&compiled, &fixed, &[b]);
            assert_eq!(
                feasible, semantic,
                "z = max(x,0) feasible set mismatch at x={xv}, z={zv}"
            );
        }
    }
}

#[test]
fn clamp_exact_feasible_set_matches_semantic() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(-3.0, 5.0)).unwrap();
    let (k, z) = model
        .add_absolute_value(
            x.into(),
            AbsoluteValueVariant::Clamp {
                lower: 1.0,
                upper: 3.0,
            },
            None,
        )
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    let mut all_binaries =
        generated_binaries(&compiled, k, GeneratedRole::ClampInnerSelectorBinary);
    all_binaries.extend(generated_binaries(
        &compiled,
        k,
        GeneratedRole::ClampOuterSelectorBinary,
    ));
    assert_eq!(
        all_binaries.len(),
        4,
        "clamp has 2 inner + 2 outer selector binaries"
    );
    let w = generated_var_for(&compiled, k, GeneratedRole::ClampInnerSelectorRow);
    let xid = compiled_user_var(&compiled, x);
    let zid = compiled_user_var(&compiled, z);
    for xv in -3..=5 {
        for zv in 0..=4 {
            let semantic = (zv as f64) == (xv as f64).clamp(1.0, 3.0);
            let w0 = (xv as f64).max(1.0);
            let mut fixed = HashMap::new();
            fixed.insert(xid, xv as f64);
            fixed.insert(zid, zv as f64);
            fixed.insert(w, w0);
            let feasible = assignment_feasible(&compiled, &fixed, &all_binaries);
            assert_eq!(
                feasible, semantic,
                "z = clamp(x,1,3) feasible set mismatch at x={xv}, z={zv}"
            );
        }
    }
}

#[test]
fn absolute_value_every_generated_entity_carries_construct_origin() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(-2.0, 2.0)).unwrap();
    let (k, _) = model
        .add_absolute_value(x.into(), AbsoluteValueVariant::Absolute, None)
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
    for v in &compiled.variables {
        if let Some(EntityOrigin::Construct { construct, .. }) =
            compiled.origin_map.variable_origin(v.id)
        {
            assert_eq!(*construct, k, "generated var must trace to this construct");
        }
    }
    // Completeness validator finds no missing origins (D5, SM-02.5).
    assert!(compiled
        .origin_map
        .missing_origins(
            &compiled.variables,
            &compiled.linear_rows,
            &compiled.objectives
        )
        .is_empty());
}

#[test]
fn absolute_value_randomized_direct_evaluation_matches_functions() {
    // Fixed-seed random bounded x: the exact compiled rows force z = |x|,
    // z = max(x,0), and z = clamp(x,lo,hi) at every sampled x (direct
    // evaluation, no solver).
    let mut rng = Lcg(0xabc_def0);
    for _ in 0..5 {
        // Absolute.
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(-4.0, 4.0)).unwrap();
        let (k, z) = model
            .add_absolute_value(x.into(), AbsoluteValueVariant::Absolute, None)
            .unwrap();
        let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
        let b = generated_var_for(&compiled, k, GeneratedRole::AbsoluteValueSelectorBinary);
        let p = generated_var_for(&compiled, k, GeneratedRole::AbsoluteValuePositivePartRow);
        let n = generated_var_for(&compiled, k, GeneratedRole::AbsoluteValueNegativePartRow);
        let xid = compiled_user_var(&compiled, x);
        let zid = compiled_user_var(&compiled, z);
        for _ in 0..5 {
            let xv = rng.range(-3, 3);
            let z_ref = xv.abs();
            let mut fixed = HashMap::new();
            fixed.insert(xid, xv);
            fixed.insert(zid, z_ref);
            fixed.insert(p, xv.max(0.0));
            fixed.insert(n, (-xv).max(0.0));
            assert!(
                assignment_feasible(&compiled, &fixed, &[b]),
                "z=|x| must be feasible"
            );
            fixed.insert(zid, z_ref + 1.0);
            assert!(
                !assignment_feasible(&compiled, &fixed, &[b]),
                "z=|x|+1 must be infeasible"
            );
        }

        // Clamp.
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(-6.0, 6.0)).unwrap();
        let (k, z) = model
            .add_absolute_value(
                x.into(),
                AbsoluteValueVariant::Clamp {
                    lower: -2.0,
                    upper: 3.0,
                },
                None,
            )
            .unwrap();
        let compiled = compile(&model, CompilationPolicy::Portable, &bridge_caps());
        let mut all_binaries =
            generated_binaries(&compiled, k, GeneratedRole::ClampInnerSelectorBinary);
        all_binaries.extend(generated_binaries(
            &compiled,
            k,
            GeneratedRole::ClampOuterSelectorBinary,
        ));
        let w = generated_var_for(&compiled, k, GeneratedRole::ClampInnerSelectorRow);
        let xid = compiled_user_var(&compiled, x);
        let zid = compiled_user_var(&compiled, z);
        for _ in 0..5 {
            let xv = rng.range(-5, 5);
            let z_ref = xv.clamp(-2.0, 3.0);
            let w0 = xv.max(-2.0);
            let mut fixed = HashMap::new();
            fixed.insert(xid, xv);
            fixed.insert(zid, z_ref);
            fixed.insert(w, w0);
            assert!(
                assignment_feasible(&compiled, &fixed, &all_binaries),
                "z=clamp(x,-2,3) must be feasible"
            );
            fixed.insert(zid, z_ref + 0.5);
            assert!(
                !assignment_feasible(&compiled, &fixed, &all_binaries),
                "z=clamp(x,-2,3)+0.5 must be infeasible"
            );
        }
    }
}
