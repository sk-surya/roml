//! Public contract tests for the P36 MPS write-back seam.
//!
//! These tests freeze the solver-free writer surface before any projection,
//! formatting, or path-transaction implementation is added.

use std::{error::Error as _, io, path::PathBuf};

use roml::{
    io::mps::write::{
        MpsDestinationPolicy, MpsEvaluatedParameter, MpsNamePolicy, MpsPathStage, MpsWriteContext,
        MpsWriteError, MpsWriteErrorKind, MpsWriteLowering, MpsWriteNameMap, MpsWriteOptions,
        MpsWriteReport, MpsWriter,
    },
    Model,
};

#[test]
fn default_writer_options_are_exactly_the_frozen_contract() {
    let options = MpsWriteOptions::default();

    assert_eq!(options.name_policy, MpsNamePolicy::PreserveOrGenerate);
    assert_eq!(
        options.destination_policy,
        MpsDestinationPolicy::AtomicReplace
    );
    assert_eq!(MpsWriter::new().options(), &options);

    let configured = MpsWriteOptions {
        name_policy: MpsNamePolicy::StrictPreserve,
        destination_policy: MpsDestinationPolicy::CreateNew,
    };
    assert_eq!(
        MpsWriter::with_options(configured.clone()).options(),
        &configured
    );
}

#[test]
fn public_writer_api_and_report_fields_are_callable_without_a_solver() {
    fn accepts_report(report: MpsWriteReport) {
        let MpsWriteReport {
            model_lineage: _,
            model_instance: _,
            model_revision: _,
            evaluated_parameters: _,
            columns: _,
            rows: _,
            nonzeros: _,
            integer_columns: _,
            objective_present: _,
            rhs_vector: _,
            ranges_vector: _,
            bounds_vector: _,
            name_map: _,
            lowerings: _,
            omitted_inactive_entities: _,
        } = report;
    }

    fn accepts_parameter(entry: MpsEvaluatedParameter) {
        let MpsEvaluatedParameter {
            id: _,
            name: _,
            value: _,
        } = entry;
    }

    fn accepts_lowering(lowering: MpsWriteLowering) {
        let _ = matches!(
            lowering,
            MpsWriteLowering::PersistentFixingAsBound {
                variable: _,
                value: _,
            }
        );
    }

    let _ = accepts_report as fn(MpsWriteReport);
    let _ = accepts_parameter as fn(MpsEvaluatedParameter);
    let _ = accepts_lowering as fn(MpsWriteLowering);
    let _ = MpsWriteNameMap::default();

    let model = Model::new();
    let mut stream = Vec::new();
    let stream_error = MpsWriter::new()
        .write(&model, &mut stream)
        .expect_err("the Wave 0 writer is typed but not implemented");
    assert_eq!(stream_error.kind(), &MpsWriteErrorKind::NotYetImplemented);
    assert!(stream.is_empty(), "the stub must not emit output");

    let path = PathBuf::from("mps-write-public-contract-unused.mps");
    let path_error = MpsWriter::new()
        .write_path(&model, &path)
        .expect_err("the Wave 0 path writer is typed but not implemented");
    assert_eq!(path_error.kind(), &MpsWriteErrorKind::NotYetImplemented);
}

#[test]
fn mandatory_top_level_error_kinds_remain_distinct_and_contextual() {
    let required = [
        MpsWriteErrorKind::Io,
        MpsWriteErrorKind::DestinationExists,
        MpsWriteErrorKind::AtomicReplaceUnavailable,
        MpsWriteErrorKind::PathTransaction,
        MpsWriteErrorKind::ModelValidation,
        MpsWriteErrorKind::Unrepresentable,
        MpsWriteErrorKind::ParameterEvaluation,
        MpsWriteErrorKind::NonFiniteValue,
        MpsWriteErrorKind::NameAllocation,
        MpsWriteErrorKind::Serialization,
        MpsWriteErrorKind::StaleEntity,
        MpsWriteErrorKind::InternalInvariant,
    ];

    assert_eq!(required.len(), 12);
    for kind in required {
        let error = MpsWriteError::new(
            kind.clone(),
            MpsWriteContext::default()
                .with_path(PathBuf::from("/tmp/contract-output.mps"))
                .with_stage(MpsPathStage::Write),
        );
        assert_eq!(error.kind(), &kind);
        assert_eq!(
            error.context().path(),
            Some(std::path::Path::new("/tmp/contract-output.mps"))
        );
        assert_eq!(error.context().stage(), Some(MpsPathStage::Write));
    }
}

#[test]
fn error_sources_and_path_transaction_stage_are_preserved() {
    let error = MpsWriteError::with_source(
        MpsWriteErrorKind::Io,
        MpsWriteContext::default()
            .with_path(PathBuf::from("output.mps"))
            .with_stage(MpsPathStage::Sync),
        io::Error::new(io::ErrorKind::PermissionDenied, "sync denied"),
    );

    assert_eq!(error.kind(), &MpsWriteErrorKind::Io);
    assert_eq!(error.context().stage(), Some(MpsPathStage::Sync));
    assert_eq!(error.io_kind(), Some(io::ErrorKind::PermissionDenied));
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("sync denied")
    );
}
