//! Focused tests for the isolated P36 canonical free-MPS formatter.
//!
//! Wave 1 integration is intentionally serial.  Including the formatter here
//! keeps these tests local to this task without changing `write/mod.rs`.

#[path = "../src/io/mps/write/format.rs"]
#[allow(dead_code)]
mod format;

use format::{
    format_document, MpsBoundKind, MpsBoundRecord, MpsColumnRecord, MpsEntry, MpsFormatError,
    MpsMarkerKind, MpsObjectiveSense, MpsRowKind, MpsRowRecord, MpsWriteDocument,
};

fn golden_document() -> MpsWriteDocument {
    MpsWriteDocument {
        name: "DEMO".to_owned(),
        objective_sense: MpsObjectiveSense::Maximize,
        objective_name: Some("OBJ".to_owned()),
        rows: vec![
            MpsRowRecord {
                kind: MpsRowKind::Free,
                name: "OBJ".to_owned(),
            },
            MpsRowRecord {
                kind: MpsRowKind::Equal,
                name: "EQ".to_owned(),
            },
            MpsRowRecord {
                kind: MpsRowKind::LessThan,
                name: "CAP".to_owned(),
            },
        ],
        columns: vec![
            MpsColumnRecord::Marker {
                name: "MARK000001".to_owned(),
                kind: MpsMarkerKind::Start,
            },
            MpsColumnRecord::Entries {
                name: "X000001".to_owned(),
                entries: vec![
                    MpsEntry {
                        row: "OBJ".to_owned(),
                        value: 1.0,
                    },
                    MpsEntry {
                        row: "EQ".to_owned(),
                        value: 2.0,
                    },
                ],
            },
            MpsColumnRecord::Entries {
                name: "X000002".to_owned(),
                entries: vec![MpsEntry {
                    row: "OBJ".to_owned(),
                    value: 1.25,
                }],
            },
            MpsColumnRecord::Marker {
                name: "MARK000002".to_owned(),
                kind: MpsMarkerKind::End,
            },
        ],
        rhs: Some(vec![
            MpsEntry {
                row: "EQ".to_owned(),
                value: 10.0,
            },
            MpsEntry {
                row: "CAP".to_owned(),
                value: 12.0,
            },
        ]),
        ranges: Some(vec![MpsEntry {
            row: "CAP".to_owned(),
            value: 4.0,
        }]),
        bounds: Some(vec![
            MpsBoundRecord {
                kind: MpsBoundKind::Lower,
                variable: "X000001".to_owned(),
                value: Some(0.0),
            },
            MpsBoundRecord {
                kind: MpsBoundKind::Upper,
                variable: "X000001".to_owned(),
                value: Some(5.0),
            },
        ]),
    }
}

#[test]
fn emits_canonical_section_order_and_stable_free_field_whitespace() {
    let output = format_document(&golden_document()).expect("golden document formats");

    assert_eq!(
        output,
        concat!(
            "NAME DEMO\n",
            "OBJSENSE MAX\n",
            "OBJNAME OBJ\n",
            "ROWS\n",
            "N OBJ\n",
            "E EQ\n",
            "L CAP\n",
            "COLUMNS\n",
            "MARK000001 'MARKER' 'INTORG'\n",
            "X000001 OBJ 1 EQ 2\n",
            "X000002 OBJ 1.25\n",
            "MARK000002 'MARKER' 'INTEND'\n",
            "RHS\n",
            "RHS1 EQ 10 CAP 12\n",
            "RANGES\n",
            "RNG1 CAP 4\n",
            "BOUNDS\n",
            "LO BND1 X000001 0\n",
            "UP BND1 X000001 5\n",
            "ENDATA\n",
        )
        .as_bytes()
    );
    assert!(!output.windows(2).any(|window| window == b"\r\n"));
}

#[test]
fn uses_fixed_canonical_vector_names() {
    let document = MpsWriteDocument {
        rhs: Some(vec![MpsEntry {
            row: "R1".to_owned(),
            value: 1.0,
        }]),
        ranges: Some(vec![MpsEntry {
            row: "R1".to_owned(),
            value: 2.0,
        }]),
        bounds: Some(vec![MpsBoundRecord {
            kind: MpsBoundKind::Fixed,
            variable: "X1".to_owned(),
            value: Some(3.0),
        }]),
        ..MpsWriteDocument::minimal("VECTORS")
    };

    let output = String::from_utf8(format_document(&document).expect("formats"))
        .expect("formatter emits UTF-8");
    assert!(output.contains("RHS1 R1 1\n"));
    assert!(output.contains("RNG1 R1 2\n"));
    assert!(output.contains("FX BND1 X1 3\n"));
    assert!(!output.contains("CUSTOM"));
}

#[test]
fn canonical_finite_formatting_normalizes_negative_zero_and_exponent_magnitudes() {
    let mut document = MpsWriteDocument::minimal("NUMBERS");
    document.columns = vec![MpsColumnRecord::Entries {
        name: "X".to_owned(),
        entries: vec![
            MpsEntry {
                row: "OBJ".to_owned(),
                value: -0.0,
            },
            MpsEntry {
                row: "R1".to_owned(),
                value: 1.0e123,
            },
            MpsEntry {
                row: "R2".to_owned(),
                value: -1.0e-123,
            },
        ],
    }];
    document.rows.push(MpsRowRecord {
        kind: MpsRowKind::LessThan,
        name: "R1".to_owned(),
    });
    document.rows.push(MpsRowRecord {
        kind: MpsRowKind::LessThan,
        name: "R2".to_owned(),
    });

    let output = String::from_utf8(format_document(&document).expect("formats"))
        .expect("formatter emits UTF-8");
    assert!(output.contains("X OBJ 0 R1 1e+123\n"));
    assert!(output.contains("X R2 -1e-123\n"));
    assert!(!output.contains("-0"));
}

#[test]
fn repeated_writes_of_one_document_are_byte_identical() {
    let document = golden_document();
    let first = format_document(&document).expect("first write formats");
    for _ in 0..100 {
        assert_eq!(
            format_document(&document).expect("repeated write formats"),
            first
        );
    }
}

#[test]
fn rejects_non_finite_values_before_emitting_bytes() {
    let mut document = MpsWriteDocument::minimal("FINITE");
    document.columns = vec![MpsColumnRecord::Entries {
        name: "X".to_owned(),
        entries: vec![MpsEntry {
            row: "OBJ".to_owned(),
            value: f64::INFINITY,
        }],
    }];

    let error = format_document(&document).expect_err("non-finite values are rejected");
    assert_eq!(error.to_string(), "non-finite MPS numeric value");
}

#[test]
fn rejects_duplicate_matrix_cells_across_column_records() {
    let mut document = MpsWriteDocument::minimal("DUPLICATE");
    document.columns = vec![
        MpsColumnRecord::Entries {
            name: "X".to_owned(),
            entries: vec![MpsEntry {
                row: "OBJ".to_owned(),
                value: 1.0,
            }],
        },
        MpsColumnRecord::Entries {
            name: "X".to_owned(),
            entries: vec![MpsEntry {
                row: "OBJ".to_owned(),
                value: 2.0,
            }],
        },
    ];

    assert_eq!(
        format_document(&document).expect_err("duplicate matrix cells are rejected"),
        MpsFormatError::DuplicateMatrixCell
    );
}
