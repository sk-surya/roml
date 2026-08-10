//! Canonical free-MPS byte formatting for a normalized write document.
//!
//! This module deliberately starts after semantic projection.  It does not
//! inspect model entities, infer domains, combine expressions, or choose an
//! objective.  The serial Wave 1 integration task can feed it the normalized
//! records produced by projection.

use std::{collections::HashSet, fmt, fmt::Write as _};

/// A normalized, already-representable free-MPS document.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsWriteDocument {
    /// The normalized problem name.
    pub(crate) name: String,
    /// The normalized objective sense.
    pub(crate) objective_sense: MpsObjectiveSense,
    /// The normalized objective row name, when the projection has one.
    pub(crate) objective_name: Option<String>,
    /// Rows in normalized export order, including any objective row.
    pub(crate) rows: Vec<MpsRowRecord>,
    /// Matrix records in normalized export order.
    pub(crate) columns: Vec<MpsColumnRecord>,
    /// The normalized RHS entries, if the projection emits an RHS vector.
    pub(crate) rhs: Option<Vec<MpsEntry>>,
    /// The normalized RANGES entries, if the projection emits a range vector.
    pub(crate) ranges: Option<Vec<MpsEntry>>,
    /// The normalized BOUNDS entries, if the projection emits a bound vector.
    pub(crate) bounds: Option<Vec<MpsBoundRecord>>,
}

impl MpsWriteDocument {
    /// Constructs the smallest normalized document with a zero objective row.
    pub(crate) fn minimal(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            objective_sense: MpsObjectiveSense::Minimize,
            objective_name: Some("OBJ".to_owned()),
            rows: vec![MpsRowRecord {
                kind: MpsRowKind::Free,
                name: "OBJ".to_owned(),
            }],
            columns: Vec::new(),
            rhs: None,
            ranges: None,
            bounds: None,
        }
    }
}

/// A normalized objective sense.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MpsObjectiveSense {
    /// Minimize the objective.
    Minimize,
    /// Maximize the objective.
    Maximize,
}

/// A normalized row classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MpsRowKind {
    /// A free row, conventionally used for an objective row.
    Free,
    /// An equality row.
    Equal,
    /// A greater-than row.
    GreaterThan,
    /// A less-than row.
    LessThan,
}

impl MpsRowKind {
    fn token(self) -> &'static str {
        match self {
            Self::Free => "N",
            Self::Equal => "E",
            Self::GreaterThan => "G",
            Self::LessThan => "L",
        }
    }
}

/// One normalized row record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MpsRowRecord {
    /// The row classification.
    pub(crate) kind: MpsRowKind,
    /// The already-allocated MPS row name.
    pub(crate) name: String,
}

/// One normalized sparse matrix or rim-vector entry.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsEntry {
    /// The row targeted by the entry.
    pub(crate) row: String,
    /// The finite evaluated value.
    pub(crate) value: f64,
}

/// Integer marker controls supplied by normalized projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MpsMarkerKind {
    /// Begin an integer marker region.
    Start,
    /// End an integer marker region.
    End,
}

/// A normalized COLUMNS record.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MpsColumnRecord {
    /// A marker record whose placement was selected by projection.
    Marker {
        /// The normalized marker name.
        name: String,
        /// The marker control.
        kind: MpsMarkerKind,
    },
    /// One normalized column and its sparse cells.
    Entries {
        /// The already-allocated MPS column name.
        name: String,
        /// Sparse cells for the column.
        entries: Vec<MpsEntry>,
    },
}

/// A normalized BOUNDS record.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsBoundRecord {
    /// The normalized bound kind.
    pub(crate) kind: MpsBoundKind,
    /// The already-allocated MPS column name.
    pub(crate) variable: String,
    /// The finite operand, when required by the bound kind.
    pub(crate) value: Option<f64>,
}

/// Supported normalized BOUNDS record kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MpsBoundKind {
    /// Free variable.
    Free,
    /// Fixed variable.
    Fixed,
    /// Finite lower bound.
    Lower,
    /// Negative infinity lower bound.
    MinusInfinity,
    /// Positive infinity upper bound.
    PlusInfinity,
    /// Finite upper bound.
    Upper,
    /// Binary variable.
    Binary,
    /// Integer lower bound.
    IntegerLower,
    /// Integer upper bound.
    IntegerUpper,
}

impl MpsBoundKind {
    fn token(self) -> &'static str {
        match self {
            Self::Free => "FR",
            Self::Fixed => "FX",
            Self::Lower => "LO",
            Self::MinusInfinity => "MI",
            Self::PlusInfinity => "PL",
            Self::Upper => "UP",
            Self::Binary => "BV",
            Self::IntegerLower => "LI",
            Self::IntegerUpper => "UI",
        }
    }

    fn requires_value(self) -> bool {
        matches!(
            self,
            Self::Fixed | Self::Lower | Self::Upper | Self::IntegerLower | Self::IntegerUpper
        )
    }
}

/// Formatting failures caused by an invalid normalized document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MpsFormatError {
    /// A numeric field is NaN or infinite.
    NonFiniteValue,
    /// The normalized projection supplied the same matrix cell more than once.
    DuplicateMatrixCell,
    /// A bound's value presence does not match its bound kind.
    InvalidBoundValue,
}

impl fmt::Display for MpsFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteValue => formatter.write_str("non-finite MPS numeric value"),
            Self::DuplicateMatrixCell => {
                formatter.write_str("duplicate normalized MPS matrix cell")
            }
            Self::InvalidBoundValue => formatter.write_str("invalid normalized MPS bound value"),
        }
    }
}

impl std::error::Error for MpsFormatError {}

/// Formats one normalized document as deterministic free-MPS bytes.
pub(crate) fn format_document(document: &MpsWriteDocument) -> Result<Vec<u8>, MpsFormatError> {
    validate_document(document)?;

    let mut output = String::new();
    line(&mut output, &format_args!("NAME {}", document.name));
    line(
        &mut output,
        &format_args!(
            "OBJSENSE {}",
            match document.objective_sense {
                MpsObjectiveSense::Minimize => "MIN",
                MpsObjectiveSense::Maximize => "MAX",
            }
        ),
    );
    if let Some(name) = &document.objective_name {
        line(&mut output, &format_args!("OBJNAME {name}"));
    }

    line(&mut output, &format_args!("ROWS"));
    for row in &document.rows {
        line(
            &mut output,
            &format_args!("{} {}", row.kind.token(), row.name),
        );
    }

    line(&mut output, &format_args!("COLUMNS"));
    for column in &document.columns {
        match column {
            MpsColumnRecord::Marker { name, kind } => line(
                &mut output,
                &format_args!(
                    "{name} 'MARKER' '{}'",
                    match kind {
                        MpsMarkerKind::Start => "INTORG",
                        MpsMarkerKind::End => "INTEND",
                    }
                ),
            ),
            MpsColumnRecord::Entries { name, entries } => entry_lines(&mut output, name, entries),
        }
    }

    if let Some(entries) = &document.rhs {
        line(&mut output, &format_args!("RHS"));
        entry_lines(&mut output, "RHS1", entries);
    }
    if let Some(entries) = &document.ranges {
        line(&mut output, &format_args!("RANGES"));
        entry_lines(&mut output, "RNG1", entries);
    }
    if let Some(bounds) = &document.bounds {
        line(&mut output, &format_args!("BOUNDS"));
        for bound in bounds {
            let mut rendered = format!("{} BND1 {}", bound.kind.token(), bound.variable);
            if let Some(value) = bound.value {
                let _ = write!(rendered, " {}", format_finite(value)?);
            }
            line(&mut output, &format_args!("{rendered}"));
        }
    }

    line(&mut output, &format_args!("ENDATA"));
    Ok(output.into_bytes())
}

fn validate_document(document: &MpsWriteDocument) -> Result<(), MpsFormatError> {
    let mut matrix_cells = HashSet::new();
    for column in &document.columns {
        if let MpsColumnRecord::Entries { name, entries } = column {
            for entry in entries {
                format_finite(entry.value)?;
                if !matrix_cells.insert((name.as_str(), entry.row.as_str())) {
                    return Err(MpsFormatError::DuplicateMatrixCell);
                }
            }
        }
    }
    for entries in [&document.rhs, &document.ranges].into_iter().flatten() {
        for entry in entries {
            format_finite(entry.value)?;
        }
    }
    if let Some(bounds) = &document.bounds {
        for bound in bounds {
            if bound.kind.requires_value() != bound.value.is_some() {
                return Err(MpsFormatError::InvalidBoundValue);
            }
            if let Some(value) = bound.value {
                format_finite(value)?;
            }
        }
    }
    Ok(())
}

fn entry_lines(output: &mut String, name: &str, entries: &[MpsEntry]) {
    for chunk in entries.chunks(2) {
        let mut rendered = name.to_owned();
        for entry in chunk {
            let _ = write!(
                rendered,
                " {} {}",
                entry.row,
                format_finite_unchecked(entry.value)
            );
        }
        line(output, &format_args!("{rendered}"));
    }
}

fn line(output: &mut String, content: &fmt::Arguments<'_>) {
    output
        .write_fmt(*content)
        .expect("writing to an in-memory String cannot fail");
    output.push('\n');
}

fn format_finite(value: f64) -> Result<String, MpsFormatError> {
    if !value.is_finite() {
        return Err(MpsFormatError::NonFiniteValue);
    }
    Ok(format_finite_unchecked(value))
}

fn format_finite_unchecked(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }

    let magnitude = value.abs();
    let rendered = value.to_string();
    if let Some((mantissa, exponent)) = rendered.split_once('e') {
        return normalized_exponent(mantissa, exponent);
    }

    if !(1.0e-6..1.0e20).contains(&magnitude) {
        return fixed_to_scientific(&rendered);
    }

    rendered
}

fn normalized_exponent(mantissa: &str, exponent: &str) -> String {
    let exponent = exponent
        .parse::<i32>()
        .expect("Rust f64 formatting always produces an integer exponent");
    format!("{mantissa}e{exponent:+}")
}

fn fixed_to_scientific(rendered: &str) -> String {
    let (sign, unsigned) = match rendered.strip_prefix('-') {
        Some(unsigned) => ("-", unsigned),
        None => ("", rendered),
    };
    let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let digits = format!("{integer}{fraction}");
    let first = digits
        .bytes()
        .position(|digit| digit != b'0')
        .expect("zero is normalized before fixed-to-scientific formatting");
    let significant = digits[first..].trim_end_matches('0');
    let exponent = integer.len() as i32 - first as i32 - 1;
    let mantissa = if significant.len() == 1 {
        significant.to_owned()
    } else {
        format!("{}.{}", &significant[..1], &significant[1..])
    };
    format!("{sign}{mantissa}e{exponent:+}")
}
