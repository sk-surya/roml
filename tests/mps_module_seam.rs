//! Public contract tests for the Phase 35 MPS module seam.
//!
//! These tests intentionally exercise only the typed contract shared by later
//! parser, staging, and reader tasks. They do not assert parsing semantics.

use std::panic::{catch_unwind, AssertUnwindSafe};

use roml::io::mps::{
    MpsDiagnostic, MpsError, MpsErrorKind, MpsFormat, MpsImport, MpsMetadata, MpsReadOptions,
    MpsReader, MpsResourceLimits, MpsSourceMap, MpsSourceSpan, MpsVectorSelection,
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
    let diagnostic = MpsDiagnostic::new(
        Some(MpsSourceSpan::new(7, 1, 8)),
        Some("QMATRIX".to_owned()),
    );
    let error = MpsError::new(
        MpsErrorKind::UnsupportedSection {
            section: "QMATRIX".to_owned(),
        },
        diagnostic.clone(),
    );

    assert!(matches!(
        error.kind(),
        MpsErrorKind::UnsupportedSection { section } if section == "QMATRIX"
    ));
    assert_eq!(error.diagnostic(), &diagnostic);
}

#[test]
fn malformed_input_harness_treats_typed_errors_as_non_panics() {
    assert_malformed_input_is_non_panicking(b"\0 malformed MPS", |input| {
        assert!(!input.is_empty());
        Err(MpsError::new(
            MpsErrorKind::InvalidRecord,
            MpsDiagnostic::new(Some(MpsSourceSpan::new(1, 1, 1)), None),
        ))
    });
}

fn assert_malformed_input_is_non_panicking<F>(input: &[u8], read: F)
where
    F: FnOnce(&[u8]) -> Result<(), MpsError>,
{
    let result = catch_unwind(AssertUnwindSafe(|| read(input)));
    assert!(result.is_ok(), "malformed input must not unwind");
    assert!(result.expect("unwind checked above").is_err());
}
