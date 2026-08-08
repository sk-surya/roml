#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use crate::io::mps::{MpsErrorKind, MpsFormat, MpsResourceLimits, MpsSection};

    use super::lex_records;
    use crate::io::mps::record::{BoundKind, IntegerMarker, MpsRecord, RowKind};

    fn lex(input: &str, format: MpsFormat) -> Result<Vec<MpsRecord>, crate::io::mps::MpsError> {
        lex_records(
            Cursor::new(input.as_bytes()),
            format,
            &MpsResourceLimits::default(),
        )
        .map(|document| document.records)
    }

    #[test]
    fn recognizes_fixed_records_and_their_optional_second_pairs() {
        let column = format!(
            "    {:<8}  {:<8}  {:<12}   {:<8}  {:<12}\n",
            "ITEM", "COST", "1.5", "LIMIT", "-2"
        );
        let input = [
            "NAME          FIXED\n",
            "ROWS\n",
            " N  COST\n",
            " L  LIMIT\n",
            "COLUMNS\n",
            &column,
            "RHS\n",
            "    RHS1      LIMIT     10\n",
            "RANGES\n",
            "    RNG1      LIMIT     4\n",
            "BOUNDS\n",
            " UP BND1      ITEM      7\n",
            "ENDATA\n",
        ]
        .concat();

        let records = lex(&input, MpsFormat::Fixed).expect("fixed input must lex");
        assert!(records.iter().any(|record| matches!(
            record,
            MpsRecord::Row { kind: RowKind::N, name, .. } if name == "COST"
        )));
        assert!(records.iter().any(|record| matches!(
            record,
            MpsRecord::Column {
                variable,
                entries,
                integer: false,
                ..
            } if variable == "ITEM"
                && entries.len() == 2
                && entries[0].row == "COST"
                && entries[0].value == 1.5
                && entries[1].row == "LIMIT"
                && entries[1].value == -2.0
        )));
        assert!(records.iter().any(|record| matches!(
            record,
            MpsRecord::Rhs { vector, entries, .. }
                if vector == "RHS1" && entries[0].row == "LIMIT" && entries[0].value == 10.0
        )));
        assert!(records.iter().any(|record| matches!(
            record,
            MpsRecord::Ranges { vector, entries, .. }
                if vector == "RNG1" && entries[0].row == "LIMIT" && entries[0].value == 4.0
        )));
        assert!(records.iter().any(|record| matches!(
            record,
            MpsRecord::Bound { kind: BoundKind::Up, vector, variable, value: Some(7.0), .. }
                if vector == "BND1" && variable == "ITEM"
        )));
    }

    #[test]
    fn recognizes_free_long_names_scientific_numbers_comments_and_crlf() {
        let input = concat!(
            "* a comment\r\n",
            "\r\n",
            "NAME free_problem\r\n",
            "ROWS\r\n",
            " N objective_with_a_long_name\r\n",
            " L constraint_with_a_long_name\r\n",
            "COLUMNS\r\n",
            " variable_with_a_long_name constraint_with_a_long_name -2.3e+08\r\n",
            "ENDATA\r\n",
        );

        let records = lex(input, MpsFormat::Free).expect("free input must lex");
        let column = records
            .iter()
            .find(|record| matches!(record, MpsRecord::Column { .. }))
            .expect("free input must contain one column record");
        assert!(matches!(
            column,
            MpsRecord::Column { variable, entries, .. }
                if variable == "variable_with_a_long_name"
                    && entries[0].row == "constraint_with_a_long_name"
                    && entries[0].value == -2.3e8
        ));
        assert_eq!(column.span().line(), 8);
        assert_eq!(column.span().start(), 1);
        assert_eq!(column.span().end(), 64);
    }

    #[test]
    fn recognizes_balanced_integer_markers_without_creating_columns() {
        let input = concat!(
            "ROWS\n",
            " N COST\n",
            "COLUMNS\n",
            " MARK0000  'MARKER'  'INTORG'\n",
            " X1 COST 1\n",
            " MARK0001  'MARKER'  'INTEND'\n",
            "ENDATA\n",
        );

        let records = lex(input, MpsFormat::Free).expect("marker block must lex");
        assert_eq!(
            records
                .iter()
                .filter(|record| matches!(record, MpsRecord::Column { .. }))
                .count(),
            1
        );
        assert!(records.iter().any(|record| matches!(
            record,
            MpsRecord::Marker {
                marker: IntegerMarker::Start,
                ..
            }
        )));
        assert!(records.iter().any(|record| matches!(
            record,
            MpsRecord::Column { variable, integer: true, .. } if variable == "X1"
        )));
        assert!(records.iter().any(|record| matches!(
            record,
            MpsRecord::Marker {
                marker: IntegerMarker::End,
                ..
            }
        )));
    }

    #[test]
    fn recognizes_conventional_fixed_integer_marker_fields() {
        let intorg = format!(
            "    {:<8}  {:<8}  {:<12}   {:<8}  {:<12}\n",
            "MARK0000", "'MARKER'", "'INTORG'", "", ""
        );
        let column = format!("    {:<8}  {:<8}  {:<12}\n", "X1", "COST", "1");
        let intend = format!(
            "    {:<8}  {:<8}  {:<12}   {:<8}  {:<12}\n",
            "MARK0001", "'MARKER'", "'INTEND'", "", ""
        );
        let input = [
            "ROWS\n",
            " N  COST\n",
            "COLUMNS\n",
            &intorg,
            &column,
            &intend,
            "ENDATA\n",
        ]
        .concat();

        let records = lex(&input, MpsFormat::Fixed)
            .expect("conventional fixed INTORG/INTEND marker fields must lex");
        assert!(matches!(
            records.get(1),
            Some(MpsRecord::Marker {
                marker: IntegerMarker::Start,
                ..
            })
        ));
        assert!(matches!(
            records.get(2),
            Some(MpsRecord::Column { integer: true, .. })
        ));
        assert!(matches!(
            records.get(3),
            Some(MpsRecord::Marker {
                marker: IntegerMarker::End,
                ..
            })
        ));
    }

    #[test]
    fn reports_deterministic_errors_for_malformed_structure_and_limits() {
        let invalid_number = lex(
            "ROWS\n N OBJ\nCOLUMNS\n X OBJ NaN\nENDATA\n",
            MpsFormat::Free,
        )
        .expect_err("NaN is not a finite MPS number");
        assert_eq!(invalid_number.kind(), &MpsErrorKind::InvalidNumber);
        assert_eq!(invalid_number.diagnostic().span().unwrap().line(), 4);
        assert_eq!(
            invalid_number.diagnostic().section(),
            Some(&MpsSection::Columns)
        );

        let missing_endata = lex("ROWS\n N OBJ\nCOLUMNS\n X OBJ 1\n", MpsFormat::Free)
            .expect_err("ENDATA is required");
        assert_eq!(missing_endata.kind(), &MpsErrorKind::MissingEndata);

        let invalid_order =
            lex("COLUMNS\nENDATA\n", MpsFormat::Free).expect_err("ROWS must precede COLUMNS");
        assert_eq!(invalid_order.kind(), &MpsErrorKind::InvalidSectionOrder);

        let nested_marker = lex(
            "ROWS\n N OBJ\nCOLUMNS\n M1 'MARKER' 'INTORG'\n M2 'MARKER' 'INTORG'\nENDATA\n",
            MpsFormat::Free,
        )
        .expect_err("nested INTORG is invalid");
        assert_eq!(nested_marker.kind(), &MpsErrorKind::InvalidMarkerNesting);

        let unsupported = lex("ROWS\n N OBJ\nQMATRIX\n", MpsFormat::Free)
            .expect_err("quadratic sections are unsupported");
        assert!(matches!(
            unsupported.kind(),
            MpsErrorKind::UnsupportedSection {
                section: MpsSection::QMatrix
            }
        ));

        let line_limits = MpsResourceLimits {
            max_line_bytes: 3,
            ..MpsResourceLimits::default()
        };
        let line_limit = lex_records(Cursor::new(b"ROWS\n"), MpsFormat::Free, &line_limits)
            .expect_err("line limit must be checked before parsing");
        assert_eq!(line_limit.kind(), &MpsErrorKind::InvalidRecord);

        let record_limits = MpsResourceLimits {
            max_records: 1,
            ..MpsResourceLimits::default()
        };
        let record_limit = lex_records(
            Cursor::new(b"ROWS\nCOLUMNS\nENDATA\n"),
            MpsFormat::Free,
            &record_limits,
        )
        .expect_err("record limit must be checked");
        assert_eq!(record_limit.kind(), &MpsErrorKind::InvalidRecord);
    }

    #[test]
    fn auto_locks_unique_free_layout_and_counts_crlf_without_its_terminator() {
        let input = concat!(
            "ROWS\r\n",
            " N objective_name_that_is_long\r\n",
            "COLUMNS\r\n",
            " variable_name_that_is_long objective_name_that_is_long 1\r\n",
            "ENDATA\r\n",
        );
        let document = lex_records(
            Cursor::new(input.as_bytes()),
            MpsFormat::Auto,
            &MpsResourceLimits::default(),
        )
        .expect("long free records uniquely determine free format");
        assert_eq!(document.format, MpsFormat::Free);

        let limits = MpsResourceLimits {
            max_line_bytes: 4,
            ..MpsResourceLimits::default()
        };
        let error = lex_records(
            BufReader::with_capacity(1, Cursor::new(b"ROWS\r\n")),
            MpsFormat::Free,
            &limits,
        )
        .expect_err("the required ENDATA is the only error after accepting a four-byte CRLF line");
        assert_eq!(error.kind(), &MpsErrorKind::MissingEndata);
    }

    #[test]
    fn reports_source_aware_errors_for_trailing_data_and_every_recognized_unsupported_section() {
        let trailing = lex("ROWS\nCOLUMNS\nENDATA\n X OBJ 1\n", MpsFormat::Free)
            .expect_err("non-comment data after ENDATA is invalid");
        assert_eq!(trailing.kind(), &MpsErrorKind::InvalidRecord);
        assert_eq!(trailing.diagnostic().span().unwrap().line(), 4);
        assert_eq!(trailing.diagnostic().section(), Some(&MpsSection::Endata));

        for (header, section) in [
            ("QMATRIX", MpsSection::QMatrix),
            ("QSECTION", MpsSection::QSection),
            ("QUADOBJ", MpsSection::QuadObj),
            ("QCMATRIX", MpsSection::QCMatrix),
            ("CSECTION", MpsSection::CSection),
            ("SOS", MpsSection::Sos),
            ("INDICATORS", MpsSection::Indicators),
            ("PWLOBJ", MpsSection::PwlObj),
            ("LAZYCONS", MpsSection::LazyCons),
            ("USERCUTS", MpsSection::UserCuts),
        ] {
            let error = lex(&format!("ROWS\nCOLUMNS\n{header}\n"), MpsFormat::Free)
                .expect_err("recognized unsupported sections must never be ignored");
            assert_eq!(
                error.kind(),
                &MpsErrorKind::UnsupportedSection {
                    section: section.clone()
                }
            );
            assert_eq!(error.diagnostic().span().unwrap().line(), 3);
        }
    }

    #[test]
    fn rejects_unsupported_payload_sections_with_their_typed_category() {
        let error = lex("ROWS\n N OBJ\nQSECTION payload\n", MpsFormat::Free)
            .expect_err("quadratic payload sections are outside P35");
        assert!(matches!(
            error.kind(),
            MpsErrorKind::UnsupportedSection {
                section: MpsSection::QSection
            }
        ));
    }

    #[test]
    fn accepts_omitted_fixed_rim_vector_names() {
        let rhs = format!("    {:<8}  {:<8}  {:<12}\n", "", "OBJ", "1");
        let column = format!("    {:<8}  {:<8}  {:<12}\n", "X", "OBJ", "1");
        let input = [
            "ROWS\n",
            " N  OBJ\n",
            "COLUMNS\n",
            &column,
            "RHS\n",
            &rhs,
            "ENDATA\n",
        ]
        .concat();
        lex(&input, MpsFormat::Fixed).expect("historical fixed RHS may omit its vector name");
    }

    #[test]
    fn ignores_conventional_fixed_marker_fields_five_and_six() {
        let marker = format!(
            "    {:<8}  {:<8}  {:<12}   {:<8}  {:<12}\n",
            "MARK0000", "'MARKER'", "'INTORG'", "IGNORED", "123"
        );
        let column = format!("    {:<8}  {:<8}  {:<12}\n", "X", "OBJ", "1");
        let intend = format!("    {:<8}  {:<8}  {:<12}\n", "M2", "'MARKER'", "'INTEND'");
        let input = [
            "ROWS\n",
            " N  OBJ\n",
            "COLUMNS\n",
            &marker,
            &column,
            &intend,
            "ENDATA\n",
        ]
        .concat();
        let records = lex(&input, MpsFormat::Fixed).expect("ignored marker fields are permitted");
        assert!(records.iter().any(|record| matches!(
            record,
            MpsRecord::Marker {
                marker: IntegerMarker::Start,
                ..
            }
        )));
    }

    #[test]
    fn marker_tokens_must_be_quoted() {
        let error = lex(
            "ROWS\n N OBJ\nCOLUMNS\n M MARKER INTORG\nENDATA\n",
            MpsFormat::Free,
        )
        .expect_err("unquoted marker words are not the frozen marker syntax");
        assert_eq!(error.kind(), &MpsErrorKind::InvalidNumber);
    }

    #[test]
    fn accepts_optional_pre_rows_sections_only_in_their_frozen_order() {
        let records = lex(
            concat!(
                "NAME model\n",
                "OBJSENSE MAX\n",
                "OBJNAME objective\n",
                "ROWS\n",
                " N objective\n",
                "COLUMNS\n",
                " x objective 1\n",
                "ENDATA\n",
            ),
            MpsFormat::Free,
        )
        .expect("NAME, OBJSENSE, and OBJNAME are legal before ROWS");
        assert!(matches!(
            &records[0],
            MpsRecord::Name { name, span } if name == "model" && span.line() == 1
        ));
        assert!(matches!(
            &records[1],
            MpsRecord::ObjSense {
                sense: super::ObjectiveSense::Maximize,
                ..
            }
        ));
        assert!(matches!(
            &records[2],
            MpsRecord::ObjName { name, .. } if name == "objective"
        ));

        let error = lex(
            "ROWS\n N objective\nCOLUMNS\nOBJNAME objective\nENDATA\n",
            MpsFormat::Free,
        )
        .expect_err("OBJNAME after the matrix section is invalid");
        assert_eq!(error.kind(), &MpsErrorKind::InvalidSectionOrder);
        assert_eq!(error.diagnostic().span().unwrap().line(), 4);
    }

    #[test]
    fn reports_non_ascii_input_at_one_based_display_columns() {
        let error = lex("\u{00ff}", MpsFormat::Free).expect_err("non-ASCII input must be rejected");
        let span = error
            .diagnostic()
            .span()
            .expect("encoding errors retain their source location");
        assert_eq!(span.line(), 1);
        assert_eq!((span.start(), span.end()), (1, 2));
    }

    #[test]
    fn rejects_source_columns_that_cannot_be_converted_to_display_coordinates() {
        assert!(super::source_span(1, usize::MAX, usize::MAX).is_err());
    }

    #[test]
    fn auto_locks_a_uniquely_fixed_record_layout() {
        let fixed_column = format!("    {:<8}  {:<8}  {:<12}\n", "X Y", "OBJ", "1");
        let input = [
            "ROWS\n",
            " N  OBJ\n",
            "COLUMNS\n",
            &fixed_column,
            "ENDATA\n",
        ]
        .concat();

        let document = lex_records(
            Cursor::new(input.as_bytes()),
            MpsFormat::Auto,
            &MpsResourceLimits::default(),
        )
        .expect("the fixed-only record determines the layout");
        assert_eq!(document.format, MpsFormat::Fixed);
    }

    #[test]
    fn auto_keeps_dual_identical_records_undecided() {
        let column = format!("    {:<8}  {:<8}  {:<12}\n", "X", "OBJ", "1");
        let input = ["ROWS\n", " N  OBJ\n", "COLUMNS\n", &column, "ENDATA\n"].concat();

        let document = lex_records(
            Cursor::new(input.as_bytes()),
            MpsFormat::Auto,
            &MpsResourceLimits::default(),
        )
        .expect("identical fixed/free interpretations are valid");
        assert_eq!(document.format, MpsFormat::Auto);
    }

    #[test]
    fn auto_rejects_ambiguous_and_mixed_record_layouts() {
        let ambiguous = format!("    {:<8}  {:<8}  {:<12}\n", "X R 2", "OBJ", "1");
        let ambiguity = lex(
            &["ROWS\n", " N  OBJ\n", "COLUMNS\n", &ambiguous, "ENDATA\n"].concat(),
            MpsFormat::Auto,
        )
        .expect_err("distinct fixed and free records are ambiguous");
        assert_eq!(ambiguity.kind(), &MpsErrorKind::AmbiguousFormat);
        let rendered = ambiguity.to_string();
        for expected in [
            "fixed interpretation:",
            "free interpretation:",
            "variable: \"X R 2\"",
            "variable: \"X\"",
        ] {
            assert!(
                rendered.contains(expected),
                "missing {expected:?} from {rendered:?}"
            );
        }

        let fixed_column = format!("    {:<8}  {:<8}  {:<12}\n", "X Y", "OBJ", "1");
        let mixed = [
            "ROWS\n",
            " N  OBJ\n",
            "COLUMNS\n",
            &fixed_column,
            " long_free_variable_name OBJ 1\n",
            "ENDATA\n",
        ]
        .concat();
        let mixed_error = lex(&mixed, MpsFormat::Auto)
            .expect_err("a fixed document cannot switch to free layout mid-stream");
        assert_eq!(mixed_error.kind(), &MpsErrorKind::InvalidRecord);
    }

    #[test]
    fn rejects_duplicate_and_out_of_order_sections() {
        let duplicate =
            lex("ROWS\nROWS\n", MpsFormat::Free).expect_err("a section may occur at most once");
        assert_eq!(duplicate.kind(), &MpsErrorKind::InvalidSectionOrder);

        let out_of_order = lex("ROWS\nCOLUMNS\nRANGES\nRHS\n", MpsFormat::Free)
            .expect_err("sections must follow the frozen order");
        assert_eq!(out_of_order.kind(), &MpsErrorKind::InvalidSectionOrder);
    }

    #[test]
    fn handles_unmatched_and_multiple_integer_marker_blocks() {
        let unmatched = lex(
            "ROWS\n N OBJ\nCOLUMNS\n M 'MARKER' 'INTEND'\nENDATA\n",
            MpsFormat::Free,
        )
        .expect_err("INTEND requires a preceding INTORG");
        assert_eq!(unmatched.kind(), &MpsErrorKind::InvalidMarkerNesting);

        let records = lex(
            concat!(
                "ROWS\n",
                " N OBJ\n",
                "COLUMNS\n",
                " M1 'MARKER' 'INTORG'\n",
                " X1 OBJ 1\n",
                " M2 'MARKER' 'INTEND'\n",
                " M3 'MARKER' 'INTORG'\n",
                " X2 OBJ 2\n",
                " M4 'MARKER' 'INTEND'\n",
                "ENDATA\n",
            ),
            MpsFormat::Free,
        )
        .expect("separate marker blocks are valid");
        assert_eq!(
            records
                .iter()
                .filter(|record| matches!(record, MpsRecord::Column { integer: true, .. }))
                .count(),
            2
        );
    }

    #[test]
    fn rejects_leaving_columns_with_an_active_integer_marker() {
        let error = lex(
            "ROWS\n N OBJ\nCOLUMNS\n M 'MARKER' 'INTORG'\nRHS\n",
            MpsFormat::Free,
        )
        .expect_err("INTORG must end before a new section begins");
        assert_eq!(error.kind(), &MpsErrorKind::InvalidMarkerNesting);
    }

    #[test]
    fn rejects_a_final_bare_carriage_return() {
        let error = lex("ROWS\n N OBJ\nCOLUMNS\n X OBJ 1\nENDATA\r", MpsFormat::Free)
            .expect_err("a bare final carriage return is not a line terminator");
        assert_eq!(error.kind(), &MpsErrorKind::InvalidEncoding);
    }
}
// Handwritten streaming lexer for the fixed/free linear MPS dialect.

use std::io::BufRead;

use super::{
    record::{
        BoundKind, IntegerMarker, LexedDocument, MpsRecord, ObjectiveSense, RowKind, RowValue,
    },
    state::LexerState,
    MpsDiagnostic, MpsError, MpsErrorKind, MpsFormat, MpsResourceLimits, MpsSection, MpsSourceSpan,
};

#[derive(Clone, Debug)]
struct Token {
    text: String,
    start: usize,
    end: usize,
}

impl Token {
    fn new(line: &str, start: usize, end: usize) -> Self {
        Self {
            text: line[start..end].to_owned(),
            start,
            end,
        }
    }
}

/// Lexes one MPS document without constructing a model or retaining input text.
pub(crate) fn lex_records<R: BufRead>(
    mut input: R,
    requested_format: MpsFormat,
    limits: &MpsResourceLimits,
) -> Result<LexedDocument, MpsError> {
    let mut state = LexerState::default();
    let mut records = Vec::new();
    let mut line_number = 0_usize;
    let mut record_count = 0_usize;
    let mut locked_format = match requested_format {
        MpsFormat::Auto => None,
        format => Some(format),
    };

    while let Some(mut bytes) =
        read_line(&mut input, limits, line_number.saturating_add(1), &state)?
    {
        line_number = line_number.checked_add(1).ok_or_else(|| {
            error(
                MpsErrorKind::InvalidRecord,
                None,
                state.current().cloned(),
                None,
                "line counter overflow",
            )
        })?;
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        if let Some((offset, _)) = bytes.iter().enumerate().find(|(_, byte)| !byte.is_ascii()) {
            return Err(error(
                MpsErrorKind::InvalidEncoding,
                Some((line_number, offset, offset + 1)),
                state.current().cloned(),
                None,
                "MPS input must be ASCII",
            ));
        }
        if let Some((offset, _)) = bytes
            .iter()
            .enumerate()
            .find(|(_, byte)| **byte == 0 || (**byte < b' ' && **byte != b'\t'))
        {
            return Err(error(
                MpsErrorKind::InvalidEncoding,
                Some((line_number, offset, offset + 1)),
                state.current().cloned(),
                None,
                "MPS input contains a control byte",
            ));
        }
        let line = std::str::from_utf8(&bytes).map_err(|_| {
            error(
                MpsErrorKind::InvalidEncoding,
                Some((line_number, 0, bytes.len())),
                state.current().cloned(),
                None,
                "MPS input is not valid ASCII",
            )
        })?;
        if is_ignored(line) {
            continue;
        }
        record_count = record_count.checked_add(1).ok_or_else(|| {
            error(
                MpsErrorKind::InvalidRecord,
                Some((line_number, 0, line.len())),
                state.current().cloned(),
                None,
                "record counter overflow",
            )
        })?;
        if record_count > limits.max_records {
            return Err(error(
                MpsErrorKind::InvalidRecord,
                Some((line_number, 0, line.len())),
                state.current().cloned(),
                None,
                "MPS record limit exceeded",
            ));
        }

        if let Some((section, fields)) = section_header(line) {
            let span = record_span(line_number, line.len())?;
            match section {
                MpsSection::QMatrix
                | MpsSection::QSection
                | MpsSection::QuadObj
                | MpsSection::QCMatrix
                | MpsSection::CSection
                | MpsSection::Sos
                | MpsSection::Indicators
                | MpsSection::PwlObj
                | MpsSection::LazyCons
                | MpsSection::UserCuts => {
                    return Err(error(
                        MpsErrorKind::UnsupportedSection {
                            section: section.clone(),
                        },
                        Some((line_number, 0, line.len())),
                        Some(section),
                        None,
                        "section is outside the supported linear LP/MILP dialect",
                    ));
                }
                MpsSection::Other(_) => {
                    return Err(error(
                        MpsErrorKind::InvalidRecord,
                        Some((line_number, 0, line.len())),
                        state.current().cloned(),
                        None,
                        "unknown MPS section header",
                    ));
                }
                _ => {}
            }
            state.begin_section(section.clone(), &span)?;
            let record = parse_header_payload(&section, &fields, line_number, line.len())?;
            if let Some(record) = record {
                state.accept_record(&record)?;
                records.push(record);
            }
            continue;
        }

        let section = state.current().cloned().ok_or_else(|| {
            error(
                MpsErrorKind::InvalidSectionOrder,
                Some((line_number, 0, line.len())),
                None,
                None,
                "record appears before an MPS section header",
            )
        })?;
        if matches!(section, MpsSection::Endata) {
            return Err(error(
                MpsErrorKind::InvalidRecord,
                Some((line_number, 0, line.len())),
                Some(section),
                None,
                "data appears after ENDATA",
            ));
        }

        let active_format = locked_format;
        let record = match active_format {
            Some(format) => {
                parse_record(line, line_number, &section, format, state.marker_active())?
            }
            None => {
                let fixed = parse_record(
                    line,
                    line_number,
                    &section,
                    MpsFormat::Fixed,
                    state.marker_active(),
                );
                let free = parse_record(
                    line,
                    line_number,
                    &section,
                    MpsFormat::Free,
                    state.marker_active(),
                );
                match (fixed, free) {
                    (Ok(fixed), Ok(free)) if fixed == free => fixed,
                    (Ok(fixed), Ok(free)) => {
                        return Err(MpsError::new(
                            MpsErrorKind::AmbiguousFormat,
                            diagnostic(
                                Some((line_number, 0, line.len())),
                                Some(section),
                                None,
                                "fixed and free interpretations produce different records",
                            )
                            .with_message(format!(
                                "fixed interpretation: {fixed:?}; free interpretation: {free:?}"
                            )),
                        ));
                    }
                    (Ok(record), Err(_)) => {
                        locked_format = Some(MpsFormat::Fixed);
                        record
                    }
                    (Err(_), Ok(record)) => {
                        locked_format = Some(MpsFormat::Free);
                        record
                    }
                    (Err(fixed), Err(_)) => return Err(fixed),
                }
            }
        };
        let mut record = record;
        if let MpsRecord::Column {
            integer_marker_span,
            ..
        } = &mut record
        {
            *integer_marker_span = state.marker_span().cloned();
        }
        state.accept_record(&record)?;
        records.push(record);
    }

    state.finish()?;
    Ok(LexedDocument {
        format: locked_format.unwrap_or(MpsFormat::Auto),
        records,
    })
}

fn read_line<R: BufRead>(
    input: &mut R,
    limits: &MpsResourceLimits,
    next_line: usize,
    state: &LexerState,
) -> Result<Option<Vec<u8>>, MpsError> {
    let mut line = Vec::new();
    loop {
        let buffer = input.fill_buf().map_err(|cause| {
            MpsError::io(
                diagnostic(
                    Some((next_line, 0, line.len())),
                    state.current().cloned(),
                    None,
                    "unable to read MPS input",
                ),
                cause,
            )
        })?;
        if buffer.is_empty() {
            if line.last() == Some(&b'\r') {
                let start = line.len().saturating_sub(1);
                return Err(error(
                    MpsErrorKind::InvalidEncoding,
                    Some((next_line, start, line.len())),
                    state.current().cloned(),
                    None,
                    "bare carriage return is not an MPS line terminator",
                ));
            }
            return if line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(line))
            };
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let content_len = newline.unwrap_or(buffer.len());
        let logical_content_len = if content_len > 0 && buffer[content_len - 1] == b'\r' {
            content_len - 1
        } else {
            content_len
        };
        // A one-byte `BufRead` buffer can split `\r\n`; retain the `\r` until
        // the caller removes the line terminator, but never charge it against
        // the logical MPS line length.
        let counted_line_len = line.len() - usize::from(line.last() == Some(&b'\r'));
        let next_len = counted_line_len
            .checked_add(logical_content_len)
            .ok_or_else(|| {
                error(
                    MpsErrorKind::InvalidRecord,
                    Some((next_line, 0, counted_line_len)),
                    state.current().cloned(),
                    None,
                    "line length overflow",
                )
            })?;
        if next_len > limits.max_line_bytes {
            return Err(error(
                MpsErrorKind::InvalidRecord,
                Some((next_line, 0, next_len)),
                state.current().cloned(),
                None,
                "MPS line length limit exceeded",
            ));
        }
        let append_len = if newline.is_some() {
            logical_content_len
        } else {
            content_len
        };
        line.extend_from_slice(&buffer[..append_len]);
        input.consume(if newline.is_some() {
            content_len + 1
        } else {
            content_len
        });
        if newline.is_some() {
            return Ok(Some(line));
        }
    }
}

fn is_ignored(line: &str) -> bool {
    line.trim_matches([' ', '\t']).is_empty() || line.starts_with('*')
}

fn section_header(line: &str) -> Option<(MpsSection, Vec<Token>)> {
    if line.as_bytes().first().is_some_and(u8::is_ascii_whitespace) {
        return None;
    }
    let fields = tokens(line);
    let first = fields.first()?;
    let section = match first.text.as_str() {
        "NAME" => MpsSection::Name,
        "OBJSENSE" => MpsSection::ObjSense,
        "OBJNAME" => MpsSection::ObjName,
        "ROWS" => MpsSection::Rows,
        "COLUMNS" => MpsSection::Columns,
        "RHS" => MpsSection::Rhs,
        "RANGES" => MpsSection::Ranges,
        "BOUNDS" => MpsSection::Bounds,
        "ENDATA" => MpsSection::Endata,
        "QMATRIX" => MpsSection::QMatrix,
        "QSECTION" => MpsSection::QSection,
        "QUADOBJ" => MpsSection::QuadObj,
        "QCMATRIX" => MpsSection::QCMatrix,
        "CSECTION" => MpsSection::CSection,
        "SOS" => MpsSection::Sos,
        "INDICATORS" => MpsSection::Indicators,
        "PWLOBJ" => MpsSection::PwlObj,
        "LAZYCONS" => MpsSection::LazyCons,
        "USERCUTS" => MpsSection::UserCuts,
        _ => return None,
    };
    Some((section, fields))
}

fn parse_header_payload(
    section: &MpsSection,
    fields: &[Token],
    line: usize,
    line_len: usize,
) -> Result<Option<MpsRecord>, MpsError> {
    let span = record_span(line, line_len)?;
    let payload = fields.get(1);
    let valid_header_shape = fields.len() == 1
        || (matches!(section, MpsSection::Name) && fields.len() >= 2)
        || (fields.len() == 2 && matches!(section, MpsSection::ObjSense | MpsSection::ObjName));
    if !valid_header_shape {
        return Err(error(
            MpsErrorKind::InvalidRecord,
            Some((line, 0, line_len)),
            Some(section.clone()),
            None,
            "MPS section header has an unexpected payload",
        ));
    }
    match section {
        MpsSection::Name => Ok(payload.map(|name| MpsRecord::Name {
            name: name.text.clone(),
            span,
        })),
        MpsSection::ObjSense => payload
            .map(|value| {
                parse_objective_sense(value, line, Some(section.clone()))
                    .map(|sense| MpsRecord::ObjSense { sense, span })
            })
            .transpose(),
        MpsSection::ObjName => Ok(payload.map(|name| MpsRecord::ObjName {
            name: name.text.clone(),
            span,
        })),
        MpsSection::Endata if payload.is_some() => Err(error(
            MpsErrorKind::InvalidRecord,
            Some((line, 0, line_len)),
            Some(section.clone()),
            None,
            "ENDATA does not accept a payload",
        )),
        _ => Ok(None),
    }
}

fn parse_record(
    line: &str,
    line_number: usize,
    section: &MpsSection,
    format: MpsFormat,
    marker_active: bool,
) -> Result<MpsRecord, MpsError> {
    match format {
        MpsFormat::Fixed => parse_fixed_record(line, line_number, section, marker_active),
        MpsFormat::Free => parse_free_record(line, line_number, section, marker_active),
        MpsFormat::Auto => unreachable!("Auto is resolved before record parsing"),
    }
}

fn parse_free_record(
    line: &str,
    line_number: usize,
    section: &MpsSection,
    marker_active: bool,
) -> Result<MpsRecord, MpsError> {
    let fields = tokens(line);
    let span = record_span(line_number, line.len())?;
    match section {
        MpsSection::ObjSense => one(&fields, line_number, section).and_then(|value| {
            parse_objective_sense(value, line_number, Some(section.clone()))
                .map(|sense| MpsRecord::ObjSense { sense, span })
        }),
        MpsSection::ObjName => one(&fields, line_number, section).map(|name| MpsRecord::ObjName {
            name: name.text.clone(),
            span,
        }),
        MpsSection::Rows => {
            exact_fields(&fields, 2, line_number, section)?;
            Ok(MpsRecord::Row {
                kind: parse_row_kind(&fields[0], line_number, section)?,
                name: fields[1].text.clone(),
                span,
            })
        }
        MpsSection::Columns => {
            parse_free_column(&fields, line_number, section, marker_active, span)
        }
        MpsSection::Rhs => {
            parse_free_pairs(&fields, line_number, section, true).map(|(vector, entries)| {
                MpsRecord::Rhs {
                    vector,
                    entries,
                    span,
                }
            })
        }
        MpsSection::Ranges => {
            parse_free_pairs(&fields, line_number, section, true).map(|(vector, entries)| {
                MpsRecord::Ranges {
                    vector,
                    entries,
                    span,
                }
            })
        }
        MpsSection::Bounds => parse_free_bound(&fields, line_number, section, span),
        _ => Err(error(
            MpsErrorKind::InvalidRecord,
            Some((line_number, 0, line.len())),
            Some(section.clone()),
            None,
            "section does not accept free-format data records",
        )),
    }
}

fn parse_fixed_record(
    line: &str,
    line_number: usize,
    section: &MpsSection,
    marker_active: bool,
) -> Result<MpsRecord, MpsError> {
    let span = record_span(line_number, line.len())?;
    match section {
        MpsSection::ObjSense | MpsSection::ObjName => {
            parse_free_record(line, line_number, section, marker_active)
        }
        MpsSection::Rows => {
            fixed_layout(line, &[(1, 2), (4, 12)], line_number, section)?;
            let kind = required(fixed_field(line, 1, 2), line_number, section, "row kind")?;
            let name = required(fixed_field(line, 4, 12), line_number, section, "row name")?;
            Ok(MpsRecord::Row {
                kind: parse_row_kind(&kind, line_number, section)?,
                name: name.text,
                span,
            })
        }
        MpsSection::Columns => {
            fixed_layout(
                line,
                &[(4, 12), (14, 22), (24, 36), (39, 47), (49, 61)],
                line_number,
                section,
            )?;
            let variable = required(
                fixed_field(line, 4, 12),
                line_number,
                section,
                "column name",
            )?;
            let first = fixed_field(line, 14, 22);
            let value = fixed_field(line, 24, 36);
            let second = fixed_field(line, 39, 47);
            let second_value = fixed_field(line, 49, 61);
            if first
                .as_ref()
                .is_some_and(|field| marker_token(&field.text))
            {
                let control = required(value, line_number, section, "integer marker control")?;
                return marker(&control, line_number, section)
                    .map(|marker| MpsRecord::Marker { marker, span });
            }
            let row = required(first, line_number, section, "first row")?;
            let value = required(value, line_number, section, "first value")?;
            let mut entries = vec![RowValue {
                row: row.text,
                value: parse_number(&value, line_number, section)?,
            }];
            match (second, second_value) {
                (None, None) => {}
                (Some(row), Some(value)) => entries.push(RowValue {
                    row: row.text,
                    value: parse_number(&value, line_number, section)?,
                }),
                _ => {
                    return Err(invalid_record(
                        line_number,
                        section,
                        "second COLUMNS pair is incomplete",
                    ))
                }
            }
            Ok(MpsRecord::Column {
                variable: variable.text,
                entries,
                integer: marker_active,
                integer_marker_span: None,
                span,
            })
        }
        MpsSection::Rhs | MpsSection::Ranges => {
            fixed_layout(
                line,
                &[(4, 12), (14, 22), (24, 36), (39, 47), (49, 61)],
                line_number,
                section,
            )?;
            // Historical fixed-format corpora commonly omit the optional
            // RHS/RANGES vector name. Preserve that first-seen vector under
            // an empty synthetic name; free-format records still require an
            // explicit vector field.
            let vector = fixed_field(line, 4, 12).map_or_else(String::new, |field| field.text);
            let row = required(fixed_field(line, 14, 22), line_number, section, "first row")?;
            let value = required(
                fixed_field(line, 24, 36),
                line_number,
                section,
                "first value",
            )?;
            let mut entries = vec![RowValue {
                row: row.text,
                value: parse_number(&value, line_number, section)?,
            }];
            match (fixed_field(line, 39, 47), fixed_field(line, 49, 61)) {
                (None, None) => {}
                (Some(row), Some(value)) => entries.push(RowValue {
                    row: row.text,
                    value: parse_number(&value, line_number, section)?,
                }),
                _ => {
                    return Err(invalid_record(
                        line_number,
                        section,
                        "second rim-vector pair is incomplete",
                    ))
                }
            }
            Ok(if matches!(section, MpsSection::Rhs) {
                MpsRecord::Rhs {
                    vector,
                    entries,
                    span,
                }
            } else {
                MpsRecord::Ranges {
                    vector,
                    entries,
                    span,
                }
            })
        }
        MpsSection::Bounds => {
            fixed_layout(
                line,
                &[(1, 3), (4, 12), (14, 22), (24, 36)],
                line_number,
                section,
            )?;
            let kind = required(fixed_field(line, 1, 3), line_number, section, "bound kind")?;
            // A substantial historical fixed-format corpus omits the
            // optional bounds-vector name. Treat it like the omitted
            // RHS/RANGES vector name and retain an empty first-seen vector.
            let vector = fixed_field(line, 4, 12).map_or_else(String::new, |field| field.text);
            let variable = required(
                fixed_field(line, 14, 22),
                line_number,
                section,
                "bound variable",
            )?;
            parse_bound(
                kind,
                vector,
                variable,
                fixed_field(line, 24, 36),
                line_number,
                section,
                span,
            )
        }
        _ => Err(invalid_record(
            line_number,
            section,
            "section does not accept fixed-format data records",
        )),
    }
}

fn parse_free_column(
    fields: &[Token],
    line: usize,
    section: &MpsSection,
    marker_active: bool,
    span: MpsSourceSpan,
) -> Result<MpsRecord, MpsError> {
    if fields.len() >= 3 && marker_token(&fields[1].text) {
        return marker(&fields[2], line, section).map(|marker| MpsRecord::Marker { marker, span });
    }
    let (variable, entries) = parse_free_pairs(fields, line, section, false)?;
    Ok(MpsRecord::Column {
        variable,
        entries,
        integer: marker_active,
        integer_marker_span: None,
        span,
    })
}

fn parse_free_pairs(
    fields: &[Token],
    line: usize,
    section: &MpsSection,
    _require_vector: bool,
) -> Result<(String, Vec<RowValue>), MpsError> {
    let offset = 1;
    if fields.len() != 3 && fields.len() != 5 {
        return Err(invalid_record(
            line,
            section,
            "record requires one or two row/value pairs",
        ));
    }
    let vector = fields[0].text.clone();
    let mut entries = vec![RowValue {
        row: fields[offset].text.clone(),
        value: parse_number(&fields[offset + 1], line, section)?,
    }];
    if fields.len() == 5 {
        entries.push(RowValue {
            row: fields[offset + 2].text.clone(),
            value: parse_number(&fields[offset + 3], line, section)?,
        });
    }
    Ok((vector, entries))
}

fn parse_free_bound(
    fields: &[Token],
    line: usize,
    section: &MpsSection,
    span: MpsSourceSpan,
) -> Result<MpsRecord, MpsError> {
    if fields.len() != 3 && fields.len() != 4 {
        return Err(invalid_record(
            line,
            section,
            "BOUNDS record requires three or four fields",
        ));
    }
    parse_bound(
        fields[0].clone(),
        fields[1].text.clone(),
        fields[2].clone(),
        fields.get(3).cloned(),
        line,
        section,
        span,
    )
}

fn parse_bound(
    kind: Token,
    vector: String,
    variable: Token,
    value: Option<Token>,
    line: usize,
    section: &MpsSection,
    span: MpsSourceSpan,
) -> Result<MpsRecord, MpsError> {
    let kind = match kind.text.as_str() {
        "FR" => BoundKind::Fr,
        "FX" => BoundKind::Fx,
        "LO" => BoundKind::Lo,
        "MI" => BoundKind::Mi,
        "PL" => BoundKind::Pl,
        "UP" => BoundKind::Up,
        "BV" => BoundKind::Bv,
        "LI" => BoundKind::Li,
        "UI" => BoundKind::Ui,
        _ => {
            return Err(error(
                MpsErrorKind::InvalidRecord,
                Some((line, kind.start, kind.end)),
                Some(section.clone()),
                Some(kind.text),
                "unsupported BOUNDS record kind",
            ));
        }
    };
    if kind.requires_value() != value.is_some() {
        return Err(invalid_record(
            line,
            section,
            if kind.requires_value() {
                "bound kind requires a finite numeric value"
            } else {
                "bound kind does not accept a numeric value"
            },
        ));
    }
    let value = value
        .as_ref()
        .map(|value| parse_number(value, line, section))
        .transpose()?;
    Ok(MpsRecord::Bound {
        kind,
        vector,
        variable: variable.text,
        value,
        span,
    })
}

fn parse_objective_sense(
    value: &Token,
    line: usize,
    section: Option<MpsSection>,
) -> Result<ObjectiveSense, MpsError> {
    match value.text.as_str() {
        "MIN" | "MINIMIZE" => Ok(ObjectiveSense::Minimize),
        "MAX" | "MAXIMIZE" => Ok(ObjectiveSense::Maximize),
        _ => Err(error(
            MpsErrorKind::InvalidRecord,
            Some((line, value.start, value.end)),
            section,
            Some(value.text.clone()),
            "OBJSENSE must be MIN, MINIMIZE, MAX, or MAXIMIZE",
        )),
    }
}

fn parse_row_kind(value: &Token, line: usize, section: &MpsSection) -> Result<RowKind, MpsError> {
    match value.text.as_str() {
        "E" => Ok(RowKind::E),
        "G" => Ok(RowKind::G),
        "L" => Ok(RowKind::L),
        "N" => Ok(RowKind::N),
        _ => Err(error(
            MpsErrorKind::InvalidRecord,
            Some((line, value.start, value.end)),
            Some(section.clone()),
            Some(value.text.clone()),
            "ROWS kind must be E, G, L, or N",
        )),
    }
}

fn marker(value: &Token, line: usize, section: &MpsSection) -> Result<IntegerMarker, MpsError> {
    match value.text.as_str() {
        "'INTORG'" => Ok(IntegerMarker::Start),
        "'INTEND'" => Ok(IntegerMarker::End),
        _ => Err(error(
            MpsErrorKind::InvalidRecord,
            Some((line, value.start, value.end)),
            Some(section.clone()),
            Some(value.text.clone()),
            "integer marker control must be 'INTORG' or 'INTEND'",
        )),
    }
}

fn marker_token(value: &str) -> bool {
    value == "'MARKER'"
}

fn parse_number(value: &Token, line: usize, section: &MpsSection) -> Result<f64, MpsError> {
    let parsed = value.text.parse::<f64>().ok();
    match parsed.filter(|value| value.is_finite()) {
        Some(value) => Ok(value),
        None => Err(error(
            MpsErrorKind::InvalidNumber,
            Some((line, value.start, value.end)),
            Some(section.clone()),
            Some(value.text.clone()),
            "numeric field must be a finite decimal or scientific value",
        )),
    }
}

fn tokens(line: &str) -> Vec<Token> {
    let bytes = line.as_bytes();
    let mut result = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        while start < bytes.len() && bytes[start].is_ascii_whitespace() {
            start += 1;
        }
        if start == bytes.len() {
            break;
        }
        let end = bytes[start..]
            .iter()
            .position(|byte| byte.is_ascii_whitespace())
            .map_or(bytes.len(), |length| start + length);
        result.push(Token::new(line, start, end));
        start = end;
    }
    result
}

fn fixed_field(line: &str, start: usize, end: usize) -> Option<Token> {
    let end = end.min(line.len());
    if start >= end {
        return None;
    }
    let bytes = line.as_bytes();
    let mut field_start = start;
    let mut field_end = end;
    while field_start < field_end && bytes[field_start].is_ascii_whitespace() {
        field_start += 1;
    }
    while field_end > field_start && bytes[field_end - 1].is_ascii_whitespace() {
        field_end -= 1;
    }
    (field_start < field_end).then(|| Token::new(line, field_start, field_end))
}

fn fixed_layout(
    line: &str,
    fields: &[(usize, usize)],
    line_number: usize,
    section: &MpsSection,
) -> Result<(), MpsError> {
    for (index, byte) in line.as_bytes().iter().enumerate() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if !fields
            .iter()
            .any(|(start, end)| *start <= index && index < *end)
        {
            return Err(error(
                MpsErrorKind::InvalidRecord,
                Some((line_number, index, index + 1)),
                Some(section.clone()),
                None,
                "nonblank character occurs outside a fixed MPS field",
            ));
        }
    }
    Ok(())
}

fn required(
    field: Option<Token>,
    line: usize,
    section: &MpsSection,
    name: &'static str,
) -> Result<Token, MpsError> {
    field.ok_or_else(|| invalid_record(line, section, name))
}

fn exact_fields(
    fields: &[Token],
    expected: usize,
    line: usize,
    section: &MpsSection,
) -> Result<(), MpsError> {
    (fields.len() == expected)
        .then_some(())
        .ok_or_else(|| invalid_record(line, section, "record has an invalid number of fields"))
}

fn one<'a>(fields: &'a [Token], line: usize, section: &MpsSection) -> Result<&'a Token, MpsError> {
    exact_fields(fields, 1, line, section)?;
    Ok(&fields[0])
}

fn invalid_record(line: usize, section: &MpsSection, message: &'static str) -> MpsError {
    error(
        MpsErrorKind::InvalidRecord,
        Some((line, 0, 0)),
        Some(section.clone()),
        None,
        message,
    )
}

fn record_span(line: usize, end: usize) -> Result<MpsSourceSpan, MpsError> {
    source_span(line, 0, end)
}

fn diagnostic(
    location: Option<(usize, usize, usize)>,
    section: Option<MpsSection>,
    raw_field: Option<String>,
    message: &'static str,
) -> MpsDiagnostic {
    let mut diagnostic = MpsDiagnostic::new().with_message(message);
    if let Some((line, start, end)) = location {
        if let Ok(span) = source_span(line, start, end) {
            diagnostic = diagnostic.with_span(span);
        }
    }
    if let Some(section) = section {
        diagnostic = diagnostic.with_section(section);
    }
    if let Some(raw_field) = raw_field {
        diagnostic = diagnostic.with_raw_field(raw_field);
    }
    diagnostic
}

fn error(
    kind: MpsErrorKind,
    location: Option<(usize, usize, usize)>,
    section: Option<MpsSection>,
    raw_field: Option<String>,
    message: &'static str,
) -> MpsError {
    MpsError::new(kind, diagnostic(location, section, raw_field, message))
}

fn source_span(line: usize, start: usize, end: usize) -> Result<MpsSourceSpan, MpsError> {
    let start = start.checked_add(1).ok_or_else(source_column_overflow)?;
    let end = end.checked_add(1).ok_or_else(source_column_overflow)?;
    MpsSourceSpan::try_new(line, start, end).map_err(|_| {
        MpsError::new(
            MpsErrorKind::InvalidRecord,
            MpsDiagnostic::new().with_message("unable to represent source span"),
        )
    })
}

fn source_column_overflow() -> MpsError {
    MpsError::new(
        MpsErrorKind::InvalidRecord,
        MpsDiagnostic::new().with_message("source column overflow"),
    )
}
