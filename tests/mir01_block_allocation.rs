//! MIR-01: trusted block allocation primitives.
//!
//! Covers the parameter-creation side (IR-04): bulk store mutation with zero
//! per-parameter journal records and scalar-equivalent revision semantics.
//! The packed variable-block Change/ModelOp (IR-03) and the Model-level
//! variable block API consume the same primitives and land in the following
//! MIR-01 task.
//!
//! Span opacity (IR-02) is enforced by private fields plus the absence of a
//! public constructor; `src/bulk.rs` carries the `compile_fail` doctest.

#![allow(deprecated)]

use roml::prelude::*;

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
