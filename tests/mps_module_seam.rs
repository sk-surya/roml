//! Public contract tests for the Phase 35 MPS module seam.
//!
//! These tests intentionally exercise only the typed contract shared by later
//! parser, staging, and reader tasks. They do not assert parsing semantics.

use std::{
    error::Error as _,
    io::{self, BufReader},
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
};

use roml::io::mps::{
    MpsDiagnostic, MpsError, MpsErrorKind, MpsFormat, MpsImport, MpsInputSource, MpsMetadata,
    MpsReadOptions, MpsReader, MpsResourceLimits, MpsSection, MpsSourceMap, MpsSourceSpan,
    MpsSourceSpanError, MpsVectorSelection,
};

#[test]
fn public_mps_contract_is_available_without_a_solver_dependency() {
    let default_reader = MpsReader::new();
    assert_eq!(default_reader.options(), &MpsReadOptions::default());

    let options = MpsReadOptions {
        format: MpsFormat::Free,
        rhs: MpsVectorSelection::Named("rhs-scenario".to_owned()),
        ranges: MpsVectorSelection::None,
        bounds: MpsVectorSelection::First,
        limits: MpsResourceLimits::default(),
    };
    let configured_reader = MpsReader::with_options(options.clone());
    assert_eq!(configured_reader.options(), &options);

    let _metadata = MpsMetadata::default();
    let _source_map = MpsSourceMap::default();

    fn consumes_import(import: MpsImport) {
        let MpsImport {
            model: _,
            metadata: _,
            source_map: _,
            diagnostics: _,
        } = import;
    }
    let _ = consumes_import as fn(MpsImport);
}

#[test]
fn default_options_are_deterministic_and_select_first_vectors() {
    let first = MpsReadOptions::default();
    let second = MpsReadOptions::default();

    assert_eq!(first, second);
    assert_eq!(first.format, MpsFormat::Auto);
    assert_eq!(first.rhs, MpsVectorSelection::First);
    assert_eq!(first.ranges, MpsVectorSelection::First);
    assert_eq!(first.bounds, MpsVectorSelection::First);
}

#[test]
fn unsupported_section_errors_preserve_the_section_and_source_context() {
    let diagnostic = MpsDiagnostic::new()
        .with_input_source(MpsInputSource::Path(PathBuf::from("fixtures/q.mps")))
        .with_span(MpsSourceSpan::try_new(7, 1, 8).unwrap())
        .with_section(MpsSection::QMatrix)
        .with_raw_field("x^2")
        .with_entity("x")
        .with_message("quadratic matrix records are outside the P35 dialect");
    let error = MpsError::new(
        MpsErrorKind::UnsupportedSection {
            section: MpsSection::QMatrix,
        },
        diagnostic.clone(),
    );

    assert!(matches!(
        error.kind(),
        MpsErrorKind::UnsupportedSection {
            section: MpsSection::QMatrix
        }
    ));
    assert_eq!(error.diagnostic(), &diagnostic);
    assert_eq!(
        error.diagnostic().input_source(),
        Some(&MpsInputSource::Path(PathBuf::from("fixtures/q.mps")))
    );
    assert_eq!(error.diagnostic().raw_field(), Some("x^2"));
    assert_eq!(error.diagnostic().entity(), Some("x"));

    let rendered = error.to_string();
    for expected in ["fixtures/q.mps", "line 7", "QMATRIX", "x^2", "entity x"] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?} from {rendered:?}"
        );
    }
}

#[test]
fn io_errors_retain_a_typed_cause_and_full_diagnostic_context() {
    let diagnostic = MpsDiagnostic::new()
        .with_input_source(MpsInputSource::Label("uploaded model".to_owned()))
        .with_span(MpsSourceSpan::try_new(3, 5, 10).unwrap())
        .with_section(MpsSection::Columns)
        .with_raw_field("not-a-number")
        .with_entity("shipment_1")
        .with_message("unable to continue reading the input");
    let error = MpsError::io(
        diagnostic,
        io::Error::new(io::ErrorKind::PermissionDenied, "fixture is unreadable"),
    );

    assert_eq!(error.kind(), &MpsErrorKind::Io);
    assert_eq!(error.io_kind(), Some(io::ErrorKind::PermissionDenied));
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("fixture is unreadable")
    );

    let rendered = error.to_string();
    for expected in [
        "uploaded model",
        "line 3",
        "COLUMNS",
        "not-a-number",
        "entity shipment_1",
        "fixture is unreadable",
    ] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?} from {rendered:?}"
        );
    }
}

#[test]
fn source_spans_use_one_based_display_coordinates() {
    let empty_first_column = MpsSourceSpan::try_new(1, 1, 1).unwrap();
    assert_eq!(empty_first_column.line(), 1);
    assert_eq!(empty_first_column.start(), 1);
    assert_eq!(empty_first_column.end(), 1);

    assert_eq!(
        MpsSourceSpan::try_new(0, 1, 1),
        Err(MpsSourceSpanError::ZeroLine)
    );
    assert_eq!(
        MpsSourceSpan::try_new(1, 0, 1),
        Err(MpsSourceSpanError::ZeroColumn)
    );
    assert_eq!(
        MpsSourceSpan::try_new(1, 8, 7),
        Err(MpsSourceSpanError::ReversedOffsets { start: 8, end: 7 })
    );
}

#[test]
fn reader_entry_point_is_non_panicking_for_malformed_input() {
    let result = catch_unwind(AssertUnwindSafe(|| {
        MpsReader::new().read(BufReader::new(&b"\0 malformed MPS"[..]))
    }));

    let error = match result.expect("the reader must not unwind") {
        Err(error) => error,
        Ok(_) => panic!("malformed input must reject"),
    };
    assert_eq!(error.kind(), &MpsErrorKind::InvalidEncoding);
    assert!(error.diagnostic().input_source().is_some());
}
