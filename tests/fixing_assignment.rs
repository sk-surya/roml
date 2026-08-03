//! P27 Task 8 — declared domains and first-class persistent fixing
//! (SM-05.1..SM-05.7).
//!
//! Reference state-machine test over continuous/integer/binary/semi domains,
//! bound changes, fixing, and unfix. Asserts:
//! - fix tightens effective bounds to equal lower/upper (SM-05.3);
//! - unfix restores the CURRENT declared bounds, not the fix-time bounds
//!   (SM-05.4);
//! - non-finite / out-of-domain / non-integral (beyond the named tolerance)
//!   fix values are typed errors (SM-05.5);
//! - `set_variable_bounds` excluding an active fixing fails atomically
//!   (SM-05.6);
//! - fixing survives `commit` → snapshot → rebuild (phase gate);
//! - the compiled delta emits `BackendOp::SetVariableBounds` under
//!   `IncrementalBounds` and a typed `UnsupportedFeature` capability gate
//!   otherwise (SM-05.7, SM-04.4; the facade recovers with a rebuild).

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, BackendOp, CompilationSession, CompileError,
    CompiledVariableId, FeatureSupport, SupportLevel,
};
use roml::compiler::capability::CompilationPolicy;
use roml::model::{binary, continuous, integer, Bounds, VarType};
use roml::{
    ConstraintExprExt, FixingProvenance, Model, ModelError, SemiDomain, VariableDomain,
    VariableFixing,
};

/// A typed capability set declaring the full M2 primitive surface native.
fn full_capabilities() -> BackendCapabilitySet {
    let mut set = BackendCapabilitySet::new();
    for feature in [
        BackendFeature::Lp,
        BackendFeature::Mip,
        BackendFeature::IncrementalBounds,
        BackendFeature::IncrementalRows,
        BackendFeature::IncrementalCoefficients,
    ] {
        set.set(
            feature,
            FeatureSupport {
                level: SupportLevel::Native,
                limitations: Default::default(),
            },
        );
    }
    set
}

/// The full incremental capability set minus one feature.
fn caps_without(feature: BackendFeature) -> BackendCapabilitySet {
    let mut set = full_capabilities();
    set.set(
        feature,
        FeatureSupport {
            level: SupportLevel::Unsupported,
            limitations: Default::default(),
        },
    );
    set
}

// ---------------------------------------------------------------------------
// 1. Declared vs effective bounds (SM-05.1)
// ---------------------------------------------------------------------------

#[test]
fn declared_and_effective_bounds_are_distinct() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();

    // Before fixing, declared == effective.
    assert_eq!(model.declared_bounds(x), Some(Bounds::new(0.0, 10.0)));
    assert_eq!(model.effective_bounds(x), Some(Bounds::new(0.0, 10.0)));
    assert_eq!(model.variable_bounds(x), Some(Bounds::new(0.0, 10.0)));
    // `variable_bounds` is the declared bound view (backward compatible).
    assert_eq!(model.declared_bounds(x), model.variable_bounds(x));

    model.fix(x, 4.0).unwrap();
    model.commit().unwrap();

    // Declared unchanged; effective is the equal-bound fixing (SM-05.3).
    assert_eq!(model.declared_bounds(x), Some(Bounds::new(0.0, 10.0)));
    assert_eq!(model.effective_bounds(x), Some(Bounds::new(4.0, 4.0)));
    // The declared bound view still reports the declared bounds.
    assert_eq!(model.variable_bounds(x), Some(Bounds::new(0.0, 10.0)));
}

// ---------------------------------------------------------------------------
// 2. Fix / unfix state machine (SM-05.2, SM-05.4)
// ---------------------------------------------------------------------------

#[test]
fn fix_tightens_effective_bounds_and_advances_revision_once() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();
    let rev_before = model.current_revision();

    model.fix(x, 4.0).unwrap();
    // A pending fix does not yet advance the revision.
    assert_eq!(model.current_revision(), rev_before);
    assert!(model.has_pending_changes());

    let rev_after = model.commit().unwrap();
    // Fixing advances the canonical revision exactly once (SM-05.2).
    assert_eq!(rev_after, rev_before.next().unwrap());
    assert_eq!(model.current_revision(), rev_after);
    assert!(!model.has_pending_changes());
}

#[test]
fn unfix_restores_current_declared_bounds_not_fix_time_bounds() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();

    // Fix at 3 (declared [0, 10] at fix time).
    model.fix(x, 3.0).unwrap();
    model.commit().unwrap();
    assert_eq!(model.effective_bounds(x), Some(Bounds::new(3.0, 3.0)));

    // Tighten the declared bounds to [0, 5] (still includes the fixing).
    model.set_variable_bounds(x, Bounds::new(0.0, 5.0)).unwrap();
    model.commit().unwrap();
    assert_eq!(model.declared_bounds(x), Some(Bounds::new(0.0, 5.0)));
    assert_eq!(model.effective_bounds(x), Some(Bounds::new(3.0, 3.0)));

    // Unfix: the effective bounds restore the CURRENT declared bounds
    // [0, 5], NOT the fix-time declared bounds [0, 10] (SM-05.4).
    model.unfix(x).unwrap();
    model.commit().unwrap();
    assert_eq!(model.declared_bounds(x), Some(Bounds::new(0.0, 5.0)));
    assert_eq!(model.effective_bounds(x), Some(Bounds::new(0.0, 5.0)));
}

#[test]
fn unfix_with_no_fixing_is_a_noop() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();
    let rev_before = model.current_revision();

    model.unfix(x).unwrap();
    model.commit().unwrap();
    // No change recorded, revision unchanged.
    assert_eq!(model.current_revision(), rev_before);
    assert_eq!(model.declared_bounds(x), Some(Bounds::new(0.0, 10.0)));
}

#[test]
fn fix_provenance_is_recorded() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();
    model.fix(x, 2.5).unwrap();
    model.commit().unwrap();

    let snapshot = model.take_snapshot().unwrap();
    let entry = snapshot
        .variables
        .iter()
        .find(|v| v.id == x)
        .expect("variable in snapshot");
    let fixing = entry.fixing.as_ref().expect("fixing recorded");
    assert_eq!(fixing.value, 2.5);
    assert_eq!(fixing.provenance, FixingProvenance::User);
}

// ---------------------------------------------------------------------------
// 3. Fix validation (SM-05.5)
// ---------------------------------------------------------------------------

#[test]
fn fix_value_outside_declared_bounds_is_rejected_atomically() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();
    let rev_before = model.current_revision();

    let err = model.fix(x, 12.0).unwrap_err();
    assert!(matches!(err, ModelError::ValueOutOfBounds { .. }));
    // No state change and no revision advance.
    assert_eq!(model.current_revision(), rev_before);
    assert!(!model.has_pending_changes());
    assert_eq!(model.effective_bounds(x), Some(Bounds::new(0.0, 10.0)));
}

#[test]
fn fix_non_finite_value_is_rejected() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();

    assert!(matches!(
        model.fix(x, f64::NAN).unwrap_err(),
        ModelError::NonFiniteValue(_)
    ));
    assert!(matches!(
        model.fix(x, f64::INFINITY).unwrap_err(),
        ModelError::NonFiniteValue(_)
    ));
    assert_eq!(model.effective_bounds(x), Some(Bounds::new(0.0, 10.0)));
}

#[test]
fn fix_non_integral_value_on_integer_rejected_beyond_tolerance() {
    let mut model = Model::new();
    let x = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();

    let err = model.fix(x, 2.5).unwrap_err();
    assert!(matches!(
        err,
        ModelError::NonIntegralValue { value, .. } if value == 2.5
    ));
    assert_eq!(model.effective_bounds(x), Some(Bounds::new(0.0, 10.0)));
}

#[test]
fn fix_non_integral_value_within_tolerance_is_accepted() {
    let mut model = Model::new();
    let x = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();

    // Within the default integrality tolerance (1e-9).
    let near_integral = 2.0 + 0.5 * model.integrality_tolerance();
    model.fix(x, near_integral).unwrap();
    model.commit().unwrap();
    assert_eq!(
        model.effective_bounds(x),
        Some(Bounds::new(near_integral, near_integral))
    );
}

#[test]
fn set_integrality_tolerance_controls_fix_acceptance() {
    let mut model = Model::new();
    let x = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();

    // Loosen the tolerance to 0.1: 2.05 is now "integral enough" (distance
    // 0.05 to the nearest integer).
    model.set_integrality_tolerance(0.1).unwrap();
    assert_eq!(model.integrality_tolerance(), 0.1);
    model.fix(x, 2.05).unwrap();
    model.commit().unwrap();
    assert_eq!(model.effective_bounds(x), Some(Bounds::new(2.05, 2.05)));

    // A value still beyond the loosened tolerance is rejected (2.2 is 0.2
    // from the nearest integer).
    assert!(matches!(
        model.fix(x, 2.2).unwrap_err(),
        ModelError::NonIntegralValue { .. }
    ));
}

#[test]
fn set_integrality_tolerance_rejects_negative_and_non_finite() {
    let mut model = Model::new();
    assert!(model.set_integrality_tolerance(-0.1).is_err());
    assert!(model.set_integrality_tolerance(f64::NAN).is_err());
    assert!(model.set_integrality_tolerance(f64::INFINITY).is_err());
    // Default preserved.
    assert_eq!(model.integrality_tolerance(), 1e-9);
}

#[test]
fn fix_binary_variable_accepts_zero_and_one() {
    let mut model = Model::new();
    let x = model.add_variable(binary()).unwrap();
    model.commit().unwrap();

    model.fix(x, 1.0).unwrap();
    model.commit().unwrap();
    assert_eq!(model.effective_bounds(x), Some(Bounds::new(1.0, 1.0)));

    model.unfix(x).unwrap();
    model.commit().unwrap();
    model.fix(x, 0.0).unwrap();
    model.commit().unwrap();
    assert_eq!(model.effective_bounds(x), Some(Bounds::new(0.0, 0.0)));
}

#[test]
fn fix_on_unknown_variable_is_rejected() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();
    model.remove_variable(x).unwrap();
    model.commit().unwrap();

    assert!(matches!(
        model.fix(x, 1.0).unwrap_err(),
        ModelError::VariableNotFound(_)
    ));
    assert!(matches!(
        model.unfix(x).unwrap_err(),
        ModelError::VariableNotFound(_)
    ));
}

// ---------------------------------------------------------------------------
// 4. Atomicity of declared-bound changes (SM-05.6)
// ---------------------------------------------------------------------------

#[test]
fn set_variable_bounds_excluding_active_fixing_fails_atomically() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();
    model.fix(x, 3.0).unwrap();
    model.commit().unwrap();
    let rev_before = model.current_revision();

    let err = model
        .set_variable_bounds(x, Bounds::new(4.0, 10.0))
        .unwrap_err();
    assert!(matches!(err, ModelError::BoundsExcludeFixing { .. }));
    // No state change and no revision advance.
    assert_eq!(model.current_revision(), rev_before);
    assert!(!model.has_pending_changes());
    assert_eq!(model.declared_bounds(x), Some(Bounds::new(0.0, 10.0)));
    assert_eq!(model.effective_bounds(x), Some(Bounds::new(3.0, 3.0)));

    // A bound change that still includes the fixing value succeeds.
    model.set_variable_bounds(x, Bounds::new(0.0, 5.0)).unwrap();
    model.commit().unwrap();
    assert_eq!(model.declared_bounds(x), Some(Bounds::new(0.0, 5.0)));
    assert_eq!(model.effective_bounds(x), Some(Bounds::new(3.0, 3.0)));
}

// ---------------------------------------------------------------------------
// 5. remove_variable clears the fixing
// ---------------------------------------------------------------------------

#[test]
fn remove_variable_clears_fixing() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();
    model.fix(x, 3.0).unwrap();
    model.commit().unwrap();

    model.remove_variable(x).unwrap();
    model.commit().unwrap();
    assert_eq!(model.declared_bounds(x), None);
    assert_eq!(model.effective_bounds(x), None);
}

// ---------------------------------------------------------------------------
// 6. Semi-continuous domain (declared canonical state only)
// ---------------------------------------------------------------------------

#[test]
fn semi_continuous_variable_has_declared_semi_domain_and_can_be_fixed() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.set_semicontinuous(x, 2.0).unwrap();
    model.commit().unwrap();

    let snapshot = model.take_snapshot().unwrap();
    let entry = snapshot
        .variables
        .iter()
        .find(|v| v.id == x)
        .expect("variable in snapshot");
    assert_eq!(entry.semicontinuous_lower, Some(2.0));

    // The declared domain of a semi-continuous variable is visible through
    // the public `VariableDomain` shape.
    let domain = model.variable_domain(x).expect("variable domain");
    assert_eq!(
        domain,
        VariableDomain {
            bounds: Bounds::new(2.0, 10.0),
            var_type: VarType::Continuous,
            semi: Some(SemiDomain::Continuous { nonzero_lower: 2.0 }),
        }
    );

    // Fixing still works on a semi-continuous variable.
    model.fix(x, 5.0).unwrap();
    model.commit().unwrap();
    assert_eq!(model.effective_bounds(x), Some(Bounds::new(5.0, 5.0)));
}

// ---------------------------------------------------------------------------
// 7. Rebuild survival (phase gate "fix/unfix survives rebuild")
// ---------------------------------------------------------------------------

#[test]
fn fixing_survives_snapshot_rebuild() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((1.0 * x).le(10.0)).unwrap();
    model.maximize(x).unwrap();
    model.commit().unwrap();

    model.fix(x, 4.0).unwrap();
    model.commit().unwrap();
    let rev = model.current_revision();

    // The snapshot at the fixed revision carries the fixing AND the declared
    // bounds (declared vs effective are both reconstructible).
    let snapshot = model.take_snapshot().unwrap();
    assert_eq!(snapshot.revision, rev);
    let entry = snapshot
        .variables
        .iter()
        .find(|v| v.id == x)
        .expect("variable in snapshot");
    assert_eq!(entry.bounds, Bounds::new(0.0, 10.0)); // declared
    assert_eq!(
        entry.fixing,
        Some(VariableFixing {
            value: 4.0,
            provenance: FixingProvenance::User,
        })
    );

    // Rebuilding from the snapshot reproduces the same declared domain and
    // fixing (the phase gate: a fixed model's compiled effective bounds after
    // snapshot rebuild equal the incremental path).
    let rebuilt = crate_rebuild_from_snapshot(&snapshot);
    assert_eq!(rebuilt.declared_bounds(x), Some(Bounds::new(0.0, 10.0)));
    assert_eq!(rebuilt.effective_bounds(x), Some(Bounds::new(4.0, 4.0)));
}

/// Rebuild a model from a snapshot via the canonical snapshot projection.
/// This mirrors the deterministic backend rebuild path: the snapshot is the
/// source of truth and the fixing must round-trip.
fn crate_rebuild_from_snapshot(snapshot: &roml::ModelSnapshot) -> Model {
    let mut rebuilt = Model::new();
    // The snapshot carries every entity; rebuild the variable layer from it.
    // (The model API is used here only to re-create the variable records; the
    // point is that the snapshot carries the fixing so a backend rebuild
    // preserves it.)
    for entry in &snapshot.variables {
        let def = match entry.var_type {
            VarType::Continuous => continuous().bounds(entry.bounds.lower, entry.bounds.upper),
            VarType::Integer => integer().bounds(entry.bounds.lower, entry.bounds.upper),
            VarType::Binary => binary(),
        };
        let var = rebuilt.add_variable(def).unwrap();
        if let Some(lower) = entry.semicontinuous_lower {
            rebuilt.set_semicontinuous(var, lower).unwrap();
        }
        if let Some(fixing) = &entry.fixing {
            rebuilt.fix(var, fixing.value).unwrap();
        }
    }
    rebuilt.commit().unwrap();
    rebuilt
}

// ---------------------------------------------------------------------------
// 8. Compiler lowering (SM-05.3, SM-05.7, D22)
// ---------------------------------------------------------------------------

/// A model with one continuous variable `x ∈ [0,10]`, one constraint, one
/// active objective, committed at r1, then `fix(x, 4.0)` committed at r2.
struct FixedModel {
    model: Model,
    rev_r1: roml::ModelRevision,
    rev_r2: roml::ModelRevision,
    snapshot_r1: roml::ModelSnapshot,
}

fn fixed_model() -> FixedModel {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((1.0 * x).le(10.0)).unwrap();
    model.maximize(x).unwrap();
    model.commit().unwrap();
    let rev_r1 = model.current_revision();
    let snapshot_r1 = model.take_snapshot().unwrap();

    model.fix(x, 4.0).unwrap();
    model.commit().unwrap();
    let rev_r2 = model.current_revision();

    let _x = x;
    FixedModel {
        model,
        rev_r1,
        rev_r2,
        snapshot_r1,
    }
}

#[test]
fn compile_snapshot_folds_fixing_into_effective_compiled_bounds() {
    let f = fixed_model();
    let snapshot_r2 = f.model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    let compiled = session
        .compile_snapshot(
            f.model.instance(),
            &snapshot_r2,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .expect("fixed snapshot must compile");

    let entry = compiled
        .variables
        .iter()
        .find(|v| v.id == CompiledVariableId(0))
        .expect("compiled variable");
    // SM-05.3: the compiled representation of the fixing is equal lower/upper
    // bounds.
    assert_eq!(entry.bounds, Bounds::new(4.0, 4.0));
}

#[test]
fn compile_fix_delta_emits_set_variable_bounds_under_incremental_bounds() {
    let f = fixed_model();
    let mut session = CompilationSession::new();
    let base = session
        .compile_snapshot(
            f.model.instance(),
            &f.snapshot_r1,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .unwrap();

    let batches = f.model.deltas_since(f.rev_r1).unwrap();
    let fix_batch = batches
        .iter()
        .find(|b| b.from == f.rev_r1 && b.to == f.rev_r2)
        .expect("fix delta batch");

    let compiled = session
        .compile_delta(
            fix_batch,
            base.compilation_id,
            f.model.instance(),
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .expect("fix delta must compile under IncrementalBounds");

    // SM-05.7: the fix/unfix delta applies incrementally as
    // `SetVariableBounds` with the effective (equal) bounds.
    assert!(compiled.operations.iter().any(|op| matches!(
        op,
        BackendOp::SetVariableBounds {
            variable: CompiledVariableId(0),
            bounds
        } if *bounds == Bounds::new(4.0, 4.0)
    )));
}

#[test]
fn compile_fix_delta_gates_on_incremental_bounds_without_it() {
    let f = fixed_model();
    let mut session = CompilationSession::new();
    let base = session
        .compile_snapshot(
            f.model.instance(),
            &f.snapshot_r1,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .unwrap();

    let batches = f.model.deltas_since(f.rev_r1).unwrap();
    let fix_batch = batches
        .iter()
        .find(|b| b.from == f.rev_r1 && b.to == f.rev_r2)
        .expect("fix delta batch");

    // SM-04.4 (WR-3): the effective-bound delta gates on
    // `BackendFeature::IncrementalBounds` exactly like the sibling
    // `SetVariableBounds` op — an unqualified backend gets a typed
    // `UnsupportedFeature`, never a silent compile (the facade recovers with a
    // deterministic rebuild).
    let err = session
        .compile_delta(
            fix_batch,
            base.compilation_id,
            f.model.instance(),
            &CompilationPolicy::Auto,
            &caps_without(BackendFeature::IncrementalBounds),
        )
        .unwrap_err();
    assert!(
        matches!(err, CompileError::UnsupportedFeature(ref f) if f == "IncrementalBounds"),
        "expected UnsupportedFeature(IncrementalBounds), got {err:?}"
    );
    // The session must not advance on a rejected delta.
    assert_eq!(
        session.current_compilation(),
        Some(base.compilation_id),
        "a rejected fix delta must not advance the compiler"
    );
}
