//! Objective, RHS, and RANGES lowering for the P36 writer.
//!
//! This unit is deliberately not wired into `write/mod.rs` by the Wave 2
//! worker.  The serial integrator consumes these small encodings after the
//! independent objective and row semantics have been qualified here.

use super::{MpsEntityKind, MpsWriteContext, MpsWriteError, MpsWriteErrorKind, MpsWriteReport};
use crate::model::{ConstraintBounds, Sense};

use super::format::{MpsEntry, MpsObjectiveSense, MpsRowKind};

/// The normalized objective metadata and its optional objective-row RHS.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsObjectiveEncoding {
    /// The objective sense emitted in `OBJSENSE`.
    pub(crate) sense: MpsObjectiveSense,
    /// The objective-row RHS, when an active objective exists.
    pub(crate) rhs: Option<MpsEntry>,
}

/// The normalized row kind and rim-vector entries for one semantic row.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsRowEncoding {
    /// The one emitted `ROWS` record kind.
    pub(crate) kind: MpsRowKind,
    /// The row's RHS value.
    pub(crate) rhs: MpsEntry,
    /// The row's one optional RANGES value.
    pub(crate) range: Option<MpsEntry>,
}

/// Encodes one active objective without changing the P35 offset convention.
///
/// P35 resolves the selected objective-row RHS as `constant = -rhs`.  The
/// exact inverse therefore emits `rhs = -constant`, including for positive,
/// zero, and negative offsets.  A missing objective gets the deterministic
/// minimize/zero-objective metadata but no objective-row RHS entry.
pub(crate) fn encode_objective(
    sense: Option<Sense>,
    constant: Option<f64>,
    objective_name: &str,
    report: &MpsWriteReport,
) -> Result<MpsObjectiveEncoding, MpsWriteError> {
    let (Some(sense), Some(constant)) = (sense, constant) else {
        if sense.is_none() && constant.is_none() {
            return Ok(MpsObjectiveEncoding {
                sense: MpsObjectiveSense::Minimize,
                rhs: None,
            });
        }
        return Err(internal_invariant(
            report,
            "objective sense and constant must be present together",
        ));
    };

    let rhs = finite_value(-constant, report, "objective offset")?;
    Ok(MpsObjectiveEncoding {
        sense: match sense {
            Sense::Minimize => MpsObjectiveSense::Minimize,
            Sense::Maximize => MpsObjectiveSense::Maximize,
        },
        rhs: Some(MpsEntry {
            row: objective_name.to_owned(),
            value: rhs,
        }),
    })
}

/// Encodes one semantic row as one MPS row plus RHS/RANGES entries.
///
/// A finite interval is represented canonically as a `G` row with the lower
/// bound in RHS and the positive interval width in RANGES.  P35 reconstructs
/// exactly the same interval for a `G` row with a positive or negative range,
/// so this preserves one semantic ranged row without splitting it.
pub(crate) fn encode_row_bounds(
    bounds: ConstraintBounds,
    row_name: &str,
    report: &MpsWriteReport,
) -> Result<MpsRowEncoding, MpsWriteError> {
    if bounds.lower.is_nan() {
        return Err(nonfinite(report, "row lower bound", row_name));
    }
    if bounds.upper.is_nan() {
        return Err(nonfinite(report, "row upper bound", row_name));
    }
    if !bounds.is_valid() || bounds.lower == f64::INFINITY || bounds.upper == f64::NEG_INFINITY {
        return Err(internal_invariant(
            report,
            "row bounds are not representable as standard MPS",
        ));
    }

    let rhs = match (bounds.lower.is_finite(), bounds.upper.is_finite()) {
        (true, true) if bounds.is_equality() => MpsRowEncoding {
            kind: MpsRowKind::Equal,
            rhs: MpsEntry {
                row: row_name.to_owned(),
                value: finite_value(bounds.lower, report, "row equality RHS")?,
            },
            range: None,
        },
        (true, true) => {
            let range = bounds.upper - bounds.lower;
            if range < 0.0 {
                return Err(internal_invariant(
                    report,
                    "row interval width is not representable as finite RANGES",
                ));
            }
            MpsRowEncoding {
                kind: MpsRowKind::GreaterThan,
                rhs: MpsEntry {
                    row: row_name.to_owned(),
                    value: finite_value(bounds.lower, report, "row lower RHS")?,
                },
                range: Some(MpsEntry {
                    row: row_name.to_owned(),
                    value: finite_value(range, report, "row range")?,
                }),
            }
        }
        (true, false) => MpsRowEncoding {
            kind: MpsRowKind::GreaterThan,
            rhs: MpsEntry {
                row: row_name.to_owned(),
                value: finite_value(bounds.lower, report, "row lower RHS")?,
            },
            range: None,
        },
        (false, true) => MpsRowEncoding {
            kind: MpsRowKind::LessThan,
            rhs: MpsEntry {
                row: row_name.to_owned(),
                value: finite_value(bounds.upper, report, "row upper RHS")?,
            },
            range: None,
        },
        (false, false) => {
            return Err(MpsWriteError::new(
                MpsWriteErrorKind::Unrepresentable,
                report_context(report)
                    .with_message("free constraint row")
                    .with_entity(MpsEntityKind::Constraint, row_name),
            ));
        }
    };

    Ok(rhs)
}

fn finite_value(value: f64, report: &MpsWriteReport, field: &str) -> Result<f64, MpsWriteError> {
    if value.is_finite() {
        Ok(if value == 0.0 { 0.0 } else { value })
    } else {
        Err(nonfinite(report, field, ""))
    }
}

fn nonfinite(report: &MpsWriteReport, field: &str, entity_name: &str) -> MpsWriteError {
    let mut context = report_context(report).with_numeric_field(field);
    if !entity_name.is_empty() {
        context = context.with_entity(MpsEntityKind::Constraint, entity_name);
    }
    MpsWriteError::new(MpsWriteErrorKind::NonFiniteValue, context)
}

fn report_context(report: &MpsWriteReport) -> MpsWriteContext {
    MpsWriteContext::default().with_model_state(
        report.model_lineage,
        report.model_instance,
        report.model_revision,
    )
}

fn internal_invariant(report: &MpsWriteReport, message: impl Into<String>) -> MpsWriteError {
    MpsWriteError::new(
        MpsWriteErrorKind::InternalInvariant,
        report_context(report).with_message(message),
    )
}
