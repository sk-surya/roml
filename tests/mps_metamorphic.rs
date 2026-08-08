//! Tier-0 synthetic, metamorphic, and fuzz-shaped MPS qualification tests.

use std::{io::Cursor, panic::AssertUnwindSafe};

use roml::io::mps::{MpsFormat, MpsReadOptions, MpsReader};

const BASE: &str = "NAME META\nROWS\n N OBJ\n L LIM\n E BAL\nCOLUMNS\n X OBJ 2 LIM 1\n X BAL 3\nRHS\n RHS1 LIM 4 BAL 5\nENDATA\n";

fn read(input: &str) -> String {
    read_format(input, MpsFormat::Free)
}

fn read_format(input: &str, format: MpsFormat) -> String {
    MpsReader::with_options(MpsReadOptions {
        format,
        ..MpsReadOptions::default()
    })
    .read(Cursor::new(input.as_bytes()))
    .expect("synthetic MPS must import")
    .model
    .pprint()
}

#[test]
fn fixed_and_free_forms_have_the_same_canonical_summary() {
    let fixed_column = format!(
        "    {:<8}  {:<8}  {:<12}   {:<8}  {:<12}\n",
        "X", "OBJ", "2", "LIM", "1"
    );
    let fixed_rhs = format!("    {:<8}  {:<8}  {:<12}\n", "RHS1", "LIM", "4");
    let fixed = [
        "NAME FIXED\n",
        "ROWS\n",
        " N  OBJ\n",
        " L  LIM\n",
        "COLUMNS\n",
        &fixed_column,
        "RHS\n",
        &fixed_rhs,
        "ENDATA\n",
    ]
    .concat();
    let free =
        "NAME FIXED\nROWS\n N OBJ\n L LIM\nCOLUMNS\n X OBJ 2 LIM 1\nRHS\n RHS1 LIM 4\nENDATA\n";
    assert_eq!(read_format(&fixed, MpsFormat::Fixed), read(free));
}

#[test]
fn synthetic_fixture_covers_marker_ranges_bounds_and_objective_metadata() {
    let imported = MpsReader::new()
        .read(Cursor::new(
            include_str!("fixtures/mps/synthetic-edge.mps").as_bytes(),
        ))
        .expect("checked-in synthetic fixture must import");
    assert_eq!(imported.model.num_variables(), 2);
    assert_eq!(imported.model.num_constraints(), 3);
    assert_eq!(imported.metadata.objective_row.as_deref(), Some("PROFIT"));
    assert_eq!(imported.metadata.rhs_vector.as_deref(), Some("RHS1"));
    assert_eq!(imported.metadata.ranges_vector.as_deref(), Some("RNG1"));
    assert_eq!(imported.metadata.bounds_vector.as_deref(), Some("BND1"));
    assert_eq!(imported.source_map.variable_bound_origins().len(), 4);
}

#[test]
fn legal_duplicate_cell_and_column_block_forms_are_equivalent() {
    let grouped = BASE.to_owned();
    let repeated = BASE.replace(" X BAL 3\n", " X BAL 1\n X BAL 2\n");
    assert_eq!(read(&grouped), read(&repeated));
}

#[test]
fn omitted_zero_rhs_and_explicit_zero_rhs_are_equivalent() {
    let omitted = "NAME META\nROWS\n N OBJ\n L LIM\nCOLUMNS\n X OBJ 2 LIM 1\nENDATA\n";
    let explicit =
        "NAME META\nROWS\n N OBJ\n L LIM\nCOLUMNS\n X OBJ 2 LIM 1\nRHS\n RHS1 LIM 0\nENDATA\n";
    assert_eq!(read(omitted), read(explicit));
}

#[test]
fn arbitrary_bytes_have_only_success_or_typed_error_and_never_panic() {
    let reader = MpsReader::new();
    for length in 0..=128_usize {
        let bytes: Vec<u8> = (0..length)
            .map(|index| (index as u8).wrapping_mul(37).wrapping_add(11))
            .collect();
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| reader.read(Cursor::new(bytes))));
        assert!(
            result.is_ok(),
            "reader panicked for generated input length {length}"
        );
    }
}
