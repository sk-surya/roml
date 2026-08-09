//! Public MPS reader integration tests.

use std::io::Cursor;

use roml::io::mps::{MpsErrorKind, MpsFormat, MpsReadOptions, MpsReader};

const SIMPLE_MPS: &str =
    "NAME SIMPLE\nROWS\n N OBJ\n L LIMIT\nCOLUMNS\n X OBJ 2 LIMIT 1\nRHS\n RHS1 LIMIT 4\nENDATA\n";

#[test]
fn stream_reader_constructs_a_fresh_model_and_metadata() {
    let imported = MpsReader::with_options(MpsReadOptions {
        format: MpsFormat::Free,
        ..MpsReadOptions::default()
    })
    .read(Cursor::new(SIMPLE_MPS.as_bytes()))
    .expect("valid MPS must import");

    assert_eq!(imported.model.num_variables(), 1);
    assert_eq!(imported.model.num_constraints(), 1);
    assert_eq!(imported.metadata.problem_name.as_deref(), Some("SIMPLE"));
    assert_eq!(imported.metadata.objective_row.as_deref(), Some("OBJ"));
    assert_eq!(imported.metadata.rhs_vector.as_deref(), Some("RHS1"));
}

#[test]
fn stream_and_path_reads_have_the_same_semantic_summary() {
    let stream = MpsReader::new()
        .read(Cursor::new(SIMPLE_MPS.as_bytes()))
        .expect("stream read");
    let path = std::env::temp_dir().join(format!(
        "roml-mps-reader-{}-{}.mps",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    std::fs::write(&path, SIMPLE_MPS).expect("write temporary MPS");
    let from_path = MpsReader::new().read_path(&path).expect("path read");
    std::fs::remove_file(&path).expect("remove temporary MPS");

    assert_eq!(stream.metadata, from_path.metadata);
    assert_eq!(
        stream.model.num_variables(),
        from_path.model.num_variables()
    );
    assert_eq!(
        stream.model.num_constraints(),
        from_path.model.num_constraints()
    );
    assert_eq!(stream.source_map, from_path.source_map);
}

#[test]
fn malformed_input_is_typed_and_keeps_path_context() {
    let path = std::env::temp_dir().join(format!(
        "roml-mps-reader-missing-{}-{}.mps",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    std::fs::write(&path, "ROWS\n N OBJ\n").expect("write temporary MPS");
    let error = MpsReader::new()
        .read_path(&path)
        .expect_err("missing COLUMNS/ENDATA must reject");
    std::fs::remove_file(&path).expect("remove temporary MPS");

    assert_eq!(error.kind(), &MpsErrorKind::MissingEndata);
    assert!(matches!(
        error.diagnostic().input_source(),
        Some(roml::io::mps::MpsInputSource::Path(actual)) if actual == &path
    ));
}
