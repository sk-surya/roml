//! Public contract tests for the P36 MPS write-back seam.
//!
//! These tests freeze the solver-free writer surface before any projection,
//! formatting, or path-transaction implementation is added.

use std::{error::Error as _, io, path::PathBuf};

use roml::{
    io::mps::write::{
        MpsDestinationPolicy, MpsEntityKind, MpsEvaluatedParameter, MpsNamePolicy, MpsPathStage,
        MpsWriteContext, MpsWriteError, MpsWriteErrorKind, MpsWriteLowering, MpsWriteName,
        MpsWriteNameMap, MpsWriteOptions, MpsWriteReport, MpsWriter,
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
    assert_eq!(
        MpsWriter::new(),
        MpsWriter::with_options(MpsWriteOptions::default()),
        "new() retains the frozen default options"
    );

    let configured = MpsWriteOptions {
        name_policy: MpsNamePolicy::StrictPreserve,
        destination_policy: MpsDestinationPolicy::CreateNew,
    };
    assert_ne!(
        MpsWriter::new(),
        MpsWriter::with_options(configured),
        "with_options() retains configured options"
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
    let stream_report = MpsWriter::new()
        .write(&model, &mut stream)
        .expect("the public writer seam is wired to the qualified pipeline");
    assert_eq!(stream_report.columns, 0);
    assert!(
        !stream.is_empty(),
        "an empty model still has a valid MPS document"
    );
}

#[test]
fn mandatory_top_level_error_kinds_remain_distinct_and_contextual() {
    let required = [
        (MpsWriteErrorKind::Io, "I/O failure", MpsPathStage::Write),
        (
            MpsWriteErrorKind::DestinationExists,
            "destination exists",
            MpsPathStage::Replace,
        ),
        (
            MpsWriteErrorKind::AtomicReplaceUnavailable,
            "atomic replacement unavailable",
            MpsPathStage::Replace,
        ),
        (
            MpsWriteErrorKind::PathTransaction,
            "path transaction failure",
            MpsPathStage::Cleanup,
        ),
        (
            MpsWriteErrorKind::ModelValidation,
            "model validation failure",
            MpsPathStage::Write,
        ),
        (
            MpsWriteErrorKind::Unrepresentable,
            "unrepresentable model feature",
            MpsPathStage::Write,
        ),
        (
            MpsWriteErrorKind::ParameterEvaluation,
            "parameter evaluation failure",
            MpsPathStage::Write,
        ),
        (
            MpsWriteErrorKind::NonFiniteValue,
            "non-finite value",
            MpsPathStage::Write,
        ),
        (
            MpsWriteErrorKind::NameAllocation,
            "name allocation failure",
            MpsPathStage::Write,
        ),
        (
            MpsWriteErrorKind::Serialization,
            "serialization failure",
            MpsPathStage::Write,
        ),
        (
            MpsWriteErrorKind::StaleEntity,
            "stale entity",
            MpsPathStage::Write,
        ),
        (
            MpsWriteErrorKind::InternalInvariant,
            "internal invariant failure",
            MpsPathStage::Write,
        ),
    ];

    assert_eq!(required.len(), 12, "the frozen taxonomy has twelve kinds");
    for (kind, label, stage) in required {
        let error = MpsWriteError::new(
            kind.clone(),
            MpsWriteContext::default()
                .with_path(PathBuf::from("/tmp/contract-output.mps"))
                .with_stage(stage),
        );
        assert_eq!(error.kind(), &kind);
        assert_eq!(
            error.to_string(),
            format!("MPS write error: {label} at /tmp/contract-output.mps during {stage}")
        );
    }
}

#[test]
fn name_map_identity_disambiguates_missing_and_duplicate_source_names() {
    let name_map = MpsWriteNameMap {
        variables: vec![
            MpsWriteName {
                entity_kind: MpsEntityKind::Variable,
                ordinal: 1,
                source_name: None,
                emitted_name: "X000001".to_owned(),
            },
            MpsWriteName {
                entity_kind: MpsEntityKind::Variable,
                ordinal: 2,
                source_name: Some("duplicate".to_owned()),
                emitted_name: "X000002".to_owned(),
            },
            MpsWriteName {
                entity_kind: MpsEntityKind::Variable,
                ordinal: 3,
                source_name: Some("duplicate".to_owned()),
                emitted_name: "X000003".to_owned(),
            },
        ],
        rows: vec![MpsWriteName {
            entity_kind: MpsEntityKind::Constraint,
            ordinal: 1,
            source_name: Some("duplicate".to_owned()),
            emitted_name: "R000001".to_owned(),
        }],
        objective: Some(MpsWriteName {
            entity_kind: MpsEntityKind::Objective,
            ordinal: 1,
            source_name: None,
            emitted_name: "OBJ".to_owned(),
        }),
    };

    assert_eq!(name_map.variables[0].entity_kind, MpsEntityKind::Variable);
    assert_eq!(name_map.variables[0].ordinal, 1);
    assert_eq!(name_map.variables[0].source_name, None);
    assert_eq!(name_map.variables[0].emitted_name, "X000001");
    assert_eq!(
        name_map.variables[1].source_name.as_deref(),
        Some("duplicate")
    );
    assert_eq!(
        name_map.variables[2].source_name.as_deref(),
        Some("duplicate")
    );
    assert_ne!(name_map.variables[1].ordinal, name_map.variables[2].ordinal);
    assert_ne!(
        name_map.variables[1].emitted_name,
        name_map.variables[2].emitted_name
    );
    assert_eq!(name_map.rows[0].entity_kind, MpsEntityKind::Constraint);
    assert_eq!(name_map.rows[0].ordinal, 1);
    assert_eq!(
        name_map.objective.as_ref().unwrap().entity_kind,
        MpsEntityKind::Objective
    );
    assert_eq!(name_map.objective.as_ref().unwrap().emitted_name, "OBJ");
}

#[test]
fn structured_context_preserves_entity_feature_and_numeric_distinctions() {
    let error = MpsWriteError::new(
        MpsWriteErrorKind::Unrepresentable,
        MpsWriteContext::default()
            .with_entity(MpsEntityKind::Variable, "semi-continuous-x")
            .with_feature("semi-continuous domain")
            .with_numeric_field("lower bound")
            .with_message("standard MPS cannot preserve this active domain"),
    );

    assert_eq!(error.context().entity_kind, Some(MpsEntityKind::Variable));
    assert_eq!(
        error.context().entity_name.as_deref(),
        Some("semi-continuous-x")
    );
    assert_eq!(
        error.context().feature.as_deref(),
        Some("semi-continuous domain")
    );
    assert_eq!(
        error.context().numeric_field.as_deref(),
        Some("lower bound")
    );
    assert_eq!(
        error.context().message.as_deref(),
        Some("standard MPS cannot preserve this active domain")
    );
    assert_eq!(
        error.to_string(),
        "MPS write error: unrepresentable model feature for variable semi-continuous-x (semi-continuous domain) in numeric field lower bound: standard MPS cannot preserve this active domain"
    );
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

#[test]
fn primary_and_cleanup_failures_are_both_preserved_in_public_error_structure() {
    let primary = MpsWriteError::with_source(
        MpsWriteErrorKind::Io,
        MpsWriteContext::default()
            .with_path(PathBuf::from("output.mps"))
            .with_stage(MpsPathStage::Replace),
        io::Error::new(io::ErrorKind::PermissionDenied, "replace denied"),
    );
    let cleanup = MpsWriteError::with_source(
        MpsWriteErrorKind::Io,
        MpsWriteContext::default()
            .with_path(PathBuf::from("output.mps.tmp"))
            .with_stage(MpsPathStage::Cleanup),
        io::Error::other("cleanup denied"),
    );

    let composed = primary.with_cleanup(cleanup);
    assert_eq!(composed.kind(), &MpsWriteErrorKind::PathTransaction);
    let preserved_primary = composed.primary().expect("primary failure is preserved");
    let preserved_cleanup = composed.cleanup().expect("cleanup failure is preserved");
    assert_eq!(preserved_primary.kind(), &MpsWriteErrorKind::Io);
    assert_eq!(
        preserved_primary.context().stage(),
        Some(MpsPathStage::Replace)
    );
    assert_eq!(
        preserved_primary.io_kind(),
        Some(io::ErrorKind::PermissionDenied)
    );
    assert_eq!(preserved_cleanup.kind(), &MpsWriteErrorKind::Io);
    assert_eq!(
        preserved_cleanup.context().stage(),
        Some(MpsPathStage::Cleanup)
    );
    assert_eq!(preserved_cleanup.io_kind(), Some(io::ErrorKind::Other));
    assert!(
        composed
            .source()
            .and_then(|source| source.downcast_ref::<MpsWriteError>())
            .is_some(),
        "the primary typed error remains the source chain"
    );
    let rendered = composed.to_string();
    assert!(rendered.contains("replace denied"));
    assert!(rendered.contains("cleanup denied"));
}
