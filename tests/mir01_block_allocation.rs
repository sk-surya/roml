//! MIR-01: trusted block allocation primitives.
//!
//! Covers the parameter-creation side (IR-04) and the packed variable-block
//! side (IR-02/IR-03/IR-05/IR-06/IR-07): bulk store mutation with one packed
//! `Change`/`ModelOp`, no per-element names, per-member staleness, and
//! canonical equivalence with the scalar path.
//!
//! Span opacity (IR-02) is enforced by private fields plus the absence of a
//! public constructor; `src/bulk.rs` carries the `compile_fail` doctest.

#![allow(deprecated)]

use roml::bulk::BlockBounds;
use roml::delta::ModelOp;
use roml::prelude::*;
use roml::solver::reference::ReferenceBackend;
use roml::sync::{AdapterCursor, ApplyOutcome};
use roml::{ConstraintBounds, ModelRevision, VarId};

// ── Parameter block (IR-04) ───────────────────────────────────────────────

#[test]
fn parameter_block_matches_scalar_creation_revision_semantics() {
    let mut block = Model::new();
    let r0 = block.current_revision();
    let span = block
        .add_parameter_block(&[1.0, 2.0, 3.0])
        .expect("finite block");
    assert_eq!(span.len(), 3);
    assert_eq!(block.num_parameters(), 3);
    assert_eq!(
        block.current_revision(),
        r0,
        "parameter existence alone does not advance the revision"
    );
    assert_eq!(
        block.journal_len(),
        0,
        "parameter block creation records no changelog/delta"
    );

    let mut scalar = Model::new();
    let sr0 = scalar.current_revision();
    for value in [1.0, 2.0, 3.0] {
        scalar.add_parameter(value).expect("scalar parameter");
    }
    assert_eq!(scalar.num_parameters(), 3);
    assert_eq!(scalar.current_revision(), sr0);
    assert_eq!(scalar.journal_len(), 0);
}

#[test]
fn parameter_block_rejects_non_finite_atomically() {
    let mut model = Model::new();
    model
        .add_parameter_block(&[1.0, 2.0])
        .expect("finite block");
    let before = model.num_parameters();
    let revision = model.current_revision();

    assert!(
        model.add_parameter_block(&[3.0, f64::NAN]).is_err(),
        "non-finite value rejects the block"
    );
    assert_eq!(model.num_parameters(), before, "no partial mutation");
    assert_eq!(model.current_revision(), revision);
}

// ── Variable block packed journal/delta (IR-03) ───────────────────────────

#[test]
fn variable_block_emits_one_packed_change_and_op() {
    for n in [1usize, 100_000] {
        let mut model = Model::new();
        let span = model
            .add_variable_block(
                n,
                VarType::Continuous,
                BlockBounds::Uniform(Bounds::new(0.0, 2.5)),
            )
            .expect("valid uniform block");
        assert_eq!(span.len(), n);
        assert_eq!(model.num_variables(), n);

        let to = model.commit().expect("commit variable block");
        let batches = model
            .deltas_since(ModelRevision::ZERO)
            .expect("retained block delta");
        assert_eq!(batches.len(), 1, "one delta batch for n={n}");
        assert_eq!(
            batches[0].operations.len(),
            1,
            "one packed op, not n VariableAdded events (n={n})"
        );
        assert!(matches!(
            batches[0].operations[0],
            ModelOp::AddVariableBlock { .. }
        ));
        assert_eq!(batches[0].to, to);
    }
}

// ── Canonical equivalence and mixed bounds (IR-03) ────────────────────────

#[test]
fn variable_block_matches_scalar_canonical_state() {
    let per = [
        Bounds::new(0.0, 1.0),
        Bounds::new(2.0, 2.0),
        Bounds::new(-3.0, 7.5),
    ];

    let mut block = Model::new();
    let span = block
        .add_variable_block(3, VarType::Continuous, BlockBounds::PerElement(&per))
        .expect("per-element block");
    let block_ids: Vec<VarId> = span.ids().collect();

    let mut scalar = Model::new();
    let mut scalar_ids = Vec::new();
    for bounds in per {
        scalar_ids.push(
            scalar
                .add_variable(continuous().bounds(bounds.lower, bounds.upper))
                .expect("scalar variable"),
        );
    }

    assert_eq!(block.num_variables(), scalar.num_variables());
    for (b, s) in block_ids.iter().zip(scalar_ids.iter()) {
        assert_eq!(block.variable_bounds(*b), scalar.variable_bounds(*s));
        assert_eq!(block.variable_name(*b), scalar.variable_name(*s));
    }
    assert_eq!(
        block.take_snapshot().expect("block snapshot"),
        scalar.take_snapshot().expect("scalar snapshot"),
        "block and scalar construction reach the same canonical snapshot"
    );
}

#[test]
fn variable_block_rejects_invalid_element_atomically() {
    let mut model = Model::new();
    let before = model.num_variables();
    let revision = model.current_revision();

    let inverted = [Bounds::new(0.0, 1.0), Bounds::new(2.0, 1.0)];
    assert!(
        model
            .add_variable_block(2, VarType::Continuous, BlockBounds::PerElement(&inverted))
            .is_err(),
        "inverted bounds reject the block"
    );
    assert_eq!(model.num_variables(), before);
    assert_eq!(model.current_revision(), revision);

    assert!(
        model
            .add_variable_block(3, VarType::Continuous, BlockBounds::PerElement(&inverted))
            .is_err(),
        "per-element length mismatch rejects the block"
    );
    assert_eq!(model.num_variables(), before);
    assert_eq!(model.current_revision(), revision);

    assert!(
        model
            .add_variable_block(
                1,
                VarType::Binary,
                BlockBounds::Uniform(Bounds::new(-1.0, 1.0)),
            )
            .is_err(),
        "binary bounds outside [0,1] reject the block"
    );
    assert_eq!(model.num_variables(), before);
    assert_eq!(model.current_revision(), revision);
}

// ── Names and per-member staleness (IR-05, IR-06) ─────────────────────────

#[test]
fn variable_block_materializes_no_names_and_stales_per_member() {
    let mut model = Model::new();
    let bounds = [
        Bounds::new(0.0, 1.0),
        Bounds::new(-1.0, 2.0),
        Bounds::new(0.0, 3.0),
    ];
    let span = model
        .add_variable_block(3, VarType::Continuous, BlockBounds::PerElement(&bounds))
        .expect("block");
    let ids: Vec<VarId> = span.ids().collect();
    assert_eq!(ids.len(), 3);

    for (id, expected) in ids.iter().zip(bounds.iter()) {
        assert_eq!(model.variable_bounds(*id), Some(*expected));
        assert_eq!(
            model.variable_name(*id).expect("live variable"),
            None,
            "block allocation must not materialize per-element names"
        );
    }

    // Borrowed scalar name APIs remain compatible.
    let named = model
        .add_variable(continuous().bounds(0.0, 1.0).named("x"))
        .expect("named scalar");
    assert_eq!(model.variable_name(named).expect("named"), Some("x"));

    // Deleting one member invalidates only that member (D-019 invariant 2).
    model.remove_variable(ids[1]).expect("remove member");
    assert!(model.variable_bounds(ids[0]).is_some());
    assert!(model.variable_bounds(ids[1]).is_none());
    assert!(model.variable_bounds(ids[2]).is_some());
}

// ── Interop with existing constant bulk rows (IR-07) ──────────────────────

#[test]
fn variable_block_participates_in_constant_row_bulk() {
    let mut model = Model::new();
    let span = model
        .add_variable_block(
            2,
            VarType::Continuous,
            BlockBounds::Uniform(Bounds::new(0.0, 10.0)),
        )
        .expect("block");
    let ids: Vec<VarId> = span.ids().collect();

    let cons = model
        .add_linear_rows_bulk(&[0, 2], &ids, &[1.0, -1.0], &[ConstraintBounds::le(5.0)])
        .expect("constant row over block variables");
    assert_eq!(cons.len(), 1);
    assert_eq!(model.num_constraints(), 1);
}

// ── Self-contained retained-delta replay (IR-03) ──────────────────────────

#[test]
fn variable_block_delta_replays_without_the_live_model() {
    let mut model = Model::new();
    model
        .add_variable_block(
            1_000,
            VarType::Continuous,
            BlockBounds::Uniform(Bounds::new(0.0, 4.0)),
        )
        .expect("block");
    model.commit().expect("commit block");

    // Apply the retained delta to a fresh backend, then compare against a
    // backend rebuilt from the model snapshot. `AddVariableBlock` must be
    // interpretable from its own payload, with no live-model access.
    let mut incremental = ReferenceBackend::new();
    let mut cursor = AdapterCursor::new();
    for batch in model
        .deltas_since(ModelRevision::ZERO)
        .expect("retained block delta")
    {
        let outcome = incremental
            .apply_batch(batch, &mut cursor)
            .expect("apply block delta");
        assert_eq!(
            outcome,
            ApplyOutcome::Applied {
                new_revision: batch.to
            }
        );
    }

    let mut rebuilt = ReferenceBackend::new();
    let mut rebuild_cursor = AdapterCursor::new();
    rebuilt.rebuild(
        &model.take_snapshot().expect("snapshot"),
        &mut rebuild_cursor,
    );
    assert_eq!(incremental.normalized_view(), rebuilt.normalized_view());
}
