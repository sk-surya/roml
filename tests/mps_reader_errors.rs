//! Public MPS reader error-path coverage for the section/marker state machine.

use std::io::Cursor;

use roml::io::mps::{MpsErrorKind, MpsReader};

fn read(text: &str) -> Result<roml::io::mps::MpsImport, roml::io::mps::MpsError> {
    MpsReader::new().read(Cursor::new(text.as_bytes()))
}

const INTORG: &str = " MARK0000 'MARKER' 'INTORG'\n";
const INTEND: &str = " MARK0001 'MARKER' 'INTEND'\n";

#[test]
fn missing_endata_is_reported() {
    let error = read("NAME X\nROWS\n N OBJ\nCOLUMNS\n X OBJ 1\n").expect_err("no ENDATA");
    assert_eq!(error.kind(), &MpsErrorKind::MissingEndata);
}

#[test]
fn columns_before_rows_is_rejected() {
    let error = read("COLUMNS\n X OBJ 1\nENDATA\n").expect_err("no ROWS");
    assert_eq!(error.kind(), &MpsErrorKind::InvalidSectionOrder);
}

#[test]
fn rhs_before_columns_is_rejected() {
    let error = read("ROWS\n N OBJ\nRHS\n RHS1 OBJ 1\nENDATA\n").expect_err("no COLUMNS");
    assert_eq!(error.kind(), &MpsErrorKind::InvalidSectionOrder);
}

#[test]
fn duplicated_section_is_rejected() {
    let error = read("ROWS\n N OBJ\nROWS\n N OBJ2\nCOLUMNS\n X OBJ 1\nENDATA\n")
        .expect_err("duplicate ROWS");
    assert_eq!(error.kind(), &MpsErrorKind::InvalidSectionOrder);
}

#[test]
fn data_after_endata_is_rejected() {
    let error =
        read("ROWS\n N OBJ\nCOLUMNS\n X OBJ 1\nENDATA\nROWS\n").expect_err("data after ENDATA");
    assert_eq!(error.kind(), &MpsErrorKind::InvalidRecord);
}

#[test]
fn unterminated_intorg_is_rejected() {
    let text = format!("ROWS\n N OBJ\nCOLUMNS\n{INTORG} X OBJ 1\nENDATA\n");
    let error = read(&text).expect_err("INTORG without INTEND");
    assert_eq!(error.kind(), &MpsErrorKind::InvalidMarkerNesting);
}

#[test]
fn int_end_without_intorg_is_rejected() {
    let text = format!("ROWS\n N OBJ\nCOLUMNS\n{INTEND} X OBJ 1\nENDATA\n");
    let error = read(&text).expect_err("INTEND without INTORG");
    assert_eq!(error.kind(), &MpsErrorKind::InvalidMarkerNesting);
}

#[test]
fn nested_intorg_is_rejected() {
    let text = format!("ROWS\n N OBJ\nCOLUMNS\n{INTORG}{INTORG} X OBJ 1\n{INTEND}ENDATA\n");
    let error = read(&text).expect_err("nested INTORG");
    assert_eq!(error.kind(), &MpsErrorKind::InvalidMarkerNesting);
}

#[test]
fn obj_sense_without_payload_is_rejected() {
    let error = read("NAME X\nOBJSENSE\nROWS\n N OBJ\nCOLUMNS\n X OBJ 1\nENDATA\n")
        .expect_err("OBJSENSE requires a record");
    assert_eq!(error.kind(), &MpsErrorKind::InvalidRecord);
}
