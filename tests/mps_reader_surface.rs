//! Public MPS reader surface: path errors, source spans, section/error-kind
//! display, and the source-map accessors.

use std::io::Cursor;

use roml::io::mps::{
    MpsErrorKind, MpsInputSource, MpsReader, MpsSection, MpsSourceSpan, MpsSourceSpanError,
};

const SIMPLE_MPS: &str =
    "NAME SIMPLE\nROWS\n N OBJ\n L LIMIT\nCOLUMNS\n X OBJ 2 LIMIT 1\nRHS\n RHS1 LIMIT 4\nENDATA\n";

#[test]
fn read_path_missing_file_reports_an_io_error_with_path_source() {
    let path = std::env::temp_dir().join(format!(
        "roml-mps-surface-missing-{}-{}.mps",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let _ = std::fs::remove_file(&path);
    let error = MpsReader::new()
        .read_path(&path)
        .expect_err("missing path is an I/O error");
    assert_eq!(error.kind(), &MpsErrorKind::Io);
    assert!(matches!(
        error.diagnostic().input_source(),
        Some(MpsInputSource::Path(actual)) if actual == &path
    ));
}

#[test]
fn source_span_validation_covers_all_error_kinds() {
    assert_eq!(
        MpsSourceSpan::try_new(0, 1, 2),
        Err(MpsSourceSpanError::ZeroLine)
    );
    assert_eq!(
        MpsSourceSpan::try_new(1, 0, 2),
        Err(MpsSourceSpanError::ZeroColumn)
    );
    assert_eq!(
        MpsSourceSpan::try_new(1, 5, 2),
        Err(MpsSourceSpanError::ReversedOffsets { start: 5, end: 2 })
    );
    let span = MpsSourceSpan::try_new(3, 1, 5).expect("valid span");
    assert_eq!((span.line(), span.start(), span.end()), (3, 1, 5));

    assert!(format!("{}", MpsSourceSpanError::ZeroLine).contains("one-based"));
    assert!(format!("{}", MpsSourceSpanError::ZeroColumn).contains("one-based"));
    assert!(format!(
        "{}",
        MpsSourceSpanError::ReversedOffsets { start: 5, end: 2 }
    )
    .contains("before"));
}

#[test]
fn section_display_covers_every_variant() {
    let cases = [
        (MpsSection::Name, "NAME"),
        (MpsSection::ObjSense, "OBJSENSE"),
        (MpsSection::ObjName, "OBJNAME"),
        (MpsSection::Rows, "ROWS"),
        (MpsSection::Columns, "COLUMNS"),
        (MpsSection::Rhs, "RHS"),
        (MpsSection::Ranges, "RANGES"),
        (MpsSection::Bounds, "BOUNDS"),
        (MpsSection::Endata, "ENDATA"),
        (MpsSection::QMatrix, "QMATRIX"),
        (MpsSection::QSection, "QSECTION"),
        (MpsSection::QuadObj, "QUADOBJ"),
        (MpsSection::QCMatrix, "QCMATRIX"),
        (MpsSection::CSection, "CSECTION"),
        (MpsSection::Sos, "SOS"),
        (MpsSection::Indicators, "INDICATORS"),
        (MpsSection::PwlObj, "PWLOBJ"),
        (MpsSection::LazyCons, "LAZYCONS"),
        (MpsSection::UserCuts, "USERCUTS"),
        (MpsSection::Other("CUSTOM".to_string()), "CUSTOM"),
    ];
    for (section, name) in cases {
        assert_eq!(format!("{section}"), name);
    }
}

#[test]
fn error_kind_display_covers_every_variant() {
    let cases: Vec<(MpsErrorKind, &str)> = vec![
        (MpsErrorKind::Io, "I/O failure"),
        (MpsErrorKind::InvalidEncoding, "invalid encoding"),
        (MpsErrorKind::InvalidSectionOrder, "invalid section order"),
        (
            MpsErrorKind::UnsupportedSection {
                section: MpsSection::Sos,
            },
            "unsupported MPS section SOS",
        ),
        (MpsErrorKind::InvalidRecord, "invalid record"),
        (MpsErrorKind::InvalidNumber, "invalid number"),
        (MpsErrorKind::DuplicateRow, "duplicate row"),
        (MpsErrorKind::UnknownRow, "unknown row"),
        (MpsErrorKind::UnknownVariable, "unknown variable"),
        (
            MpsErrorKind::InvalidMarkerNesting,
            "invalid integer-marker nesting",
        ),
        (
            MpsErrorKind::DuplicateRhsEntry,
            "duplicate selected RHS entry",
        ),
        (
            MpsErrorKind::DuplicateRangeEntry,
            "duplicate selected RANGES entry",
        ),
        (
            MpsErrorKind::InvalidRangeForNRow,
            "range entry for an N row",
        ),
        (MpsErrorKind::InvalidBound, "invalid bound"),
        (MpsErrorKind::InvalidRange, "invalid range"),
        (
            MpsErrorKind::MissingRequiredSection,
            "missing required section",
        ),
        (MpsErrorKind::MissingEndata, "missing ENDATA"),
        (MpsErrorKind::UnknownVector, "unknown vector"),
        (MpsErrorKind::AmbiguousFormat, "ambiguous MPS format"),
        (MpsErrorKind::RepresentationError, "representation error"),
        (
            MpsErrorKind::ModelConstruction,
            "model construction failure",
        ),
    ];
    for (kind, needle) in cases {
        let text = format!("{kind}");
        assert!(text.contains(needle), "{text:?} must contain {needle:?}");
    }
}

#[test]
fn source_map_spans_resolve_declared_entities() {
    let imported = MpsReader::new()
        .read(Cursor::new(SIMPLE_MPS.as_bytes()))
        .expect("valid MPS");
    assert!(imported.source_map.row_span("LIMIT").is_some());
    assert!(imported.source_map.row_span("MISSING").is_none());
    assert!(imported.source_map.column_span("X").is_some());
    assert!(imported.source_map.column_span("MISSING").is_none());
}
