//! Canonical variable-domain and integer-marker projection for free MPS.
//!
//! This module owns only the BOUNDS records and the placement of contiguous
//! integer-marker regions.  It consumes the evaluated variables produced by
//! [`super::projection`] and emits normalized records for
//! [`super::format`].

use super::{format, projection};
use crate::{
    io::mps::write::{MpsWriteContext, MpsWriteError, MpsWriteErrorKind, MpsWriteReport},
    model::{Bounds, VarType},
};

/// Encode the exact effective domain of every projected variable.
pub(crate) fn encode_bounds(
    variables: &[projection::MpsWriteVariable],
    report: &MpsWriteReport,
) -> Result<Vec<format::MpsBoundRecord>, MpsWriteError> {
    let mut records = Vec::new();
    for variable in variables {
        validate_bounds(variable, report)?;
        match variable.var_type {
            VarType::Continuous => encode_continuous(variable, report, &mut records)?,
            VarType::Integer => encode_integer(variable, report, &mut records)?,
            VarType::Binary => encode_binary(variable, report, &mut records)?,
        }
    }
    Ok(records)
}

/// Encode columns and deterministic contiguous integer-marker regions.
pub(crate) fn encode_columns(
    variables: &[projection::MpsWriteVariable],
    entries_by_variable: Vec<Vec<format::MpsEntry>>,
    report: &MpsWriteReport,
) -> Result<Vec<format::MpsColumnRecord>, MpsWriteError> {
    if variables.len() != entries_by_variable.len() {
        return Err(internal_invariant(
            report,
            "column entries do not match projected variables",
        ));
    }

    let used_names = variables
        .iter()
        .map(|variable| variable.name.as_str())
        .chain(
            report
                .name_map
                .rows
                .iter()
                .map(|row| row.emitted_name.as_str()),
        )
        .chain(
            report
                .name_map
                .objective
                .iter()
                .map(|objective| objective.emitted_name.as_str()),
        )
        .collect::<std::collections::BTreeSet<_>>();
    let mut marker_ordinal = 1usize;
    let mut marker_name = || loop {
        let candidate = format!("MARK{marker_ordinal:06}");
        marker_ordinal += 1;
        if !used_names.contains(candidate.as_str()) {
            break candidate;
        }
    };

    let mut columns = Vec::new();
    let mut integer_region = None;
    for (variable, entries) in variables.iter().zip(entries_by_variable) {
        let is_integer = matches!(variable.var_type, VarType::Integer | VarType::Binary);
        match (is_integer, integer_region.take()) {
            (true, None) => {
                let marker = marker_name();
                columns.push(format::MpsColumnRecord::Marker {
                    name: marker.clone(),
                    kind: format::MpsMarkerKind::Start,
                });
                integer_region = Some(marker);
            }
            (false, Some(marker)) => columns.push(format::MpsColumnRecord::Marker {
                name: marker,
                kind: format::MpsMarkerKind::End,
            }),
            (_, marker) => integer_region = marker,
        }
        columns.push(format::MpsColumnRecord::Entries {
            name: variable.name.clone(),
            entries,
        });
    }
    if let Some(marker) = integer_region {
        columns.push(format::MpsColumnRecord::Marker {
            name: marker,
            kind: format::MpsMarkerKind::End,
        });
    }
    Ok(columns)
}

fn validate_bounds(
    variable: &projection::MpsWriteVariable,
    report: &MpsWriteReport,
) -> Result<(), MpsWriteError> {
    let bounds = variable.effective_bounds;
    if !bounds.is_valid()
        || bounds.lower.is_nan()
        || bounds.upper.is_nan()
        || bounds.lower == f64::INFINITY
        || bounds.upper == f64::NEG_INFINITY
    {
        return Err(internal_invariant(
            report,
            "projection emitted invalid variable bounds",
        ));
    }
    Ok(())
}

fn encode_continuous(
    variable: &projection::MpsWriteVariable,
    report: &MpsWriteReport,
    records: &mut Vec<format::MpsBoundRecord>,
) -> Result<(), MpsWriteError> {
    let bounds = variable.effective_bounds;
    if bounds == Bounds::NON_NEGATIVE {
        return Ok(());
    }
    if bounds.lower == f64::NEG_INFINITY && bounds.upper == f64::INFINITY {
        records.push(bound(format::MpsBoundKind::Free, variable, None));
    } else if bounds.lower == f64::NEG_INFINITY {
        records.push(bound(format::MpsBoundKind::MinusInfinity, variable, None));
        records.push(bound(
            format::MpsBoundKind::Upper,
            variable,
            Some(finite(bounds.upper, report, "variable upper bound")?),
        ));
    } else if bounds.upper == f64::INFINITY {
        records.push(bound(
            format::MpsBoundKind::Lower,
            variable,
            Some(finite(bounds.lower, report, "variable lower bound")?),
        ));
    } else if bounds.lower == bounds.upper {
        records.push(bound(
            format::MpsBoundKind::Fixed,
            variable,
            Some(finite(bounds.lower, report, "fixed variable bound")?),
        ));
    } else {
        records.push(bound(
            format::MpsBoundKind::Lower,
            variable,
            Some(finite(bounds.lower, report, "variable lower bound")?),
        ));
        records.push(bound(
            format::MpsBoundKind::Upper,
            variable,
            Some(finite(bounds.upper, report, "variable upper bound")?),
        ));
    }
    Ok(())
}

fn encode_integer(
    variable: &projection::MpsWriteVariable,
    report: &MpsWriteReport,
    records: &mut Vec<format::MpsBoundRecord>,
) -> Result<(), MpsWriteError> {
    let bounds = variable.effective_bounds;
    let lower = bounds.lower.is_finite().then(|| bounds.lower.ceil());
    let upper = bounds.upper.is_finite().then(|| bounds.upper.floor());
    let lower = lower.unwrap_or(f64::NEG_INFINITY);
    let upper = upper.unwrap_or(f64::INFINITY);
    if lower.is_nan() || upper.is_nan() || lower > upper {
        return Err(MpsWriteError::new(
            MpsWriteErrorKind::Unrepresentable,
            context(report)
                .with_entity(
                    crate::io::mps::write::MpsEntityKind::Variable,
                    &variable.name,
                )
                .with_feature("empty integer domain"),
        ));
    }
    if lower == upper {
        records.push(bound(
            format::MpsBoundKind::Fixed,
            variable,
            Some(finite(lower, report, "fixed integer bound")?),
        ));
        return Ok(());
    }

    // An INTORG region supplies the P35 default [0, 1].  Every evaluated
    // integer domain that differs from either side of that interval gets an
    // explicit transition, including the infinity transitions.
    if lower == f64::NEG_INFINITY {
        records.push(bound(format::MpsBoundKind::MinusInfinity, variable, None));
    } else if lower != 0.0 {
        records.push(bound(
            format::MpsBoundKind::IntegerLower,
            variable,
            Some(finite(lower, report, "integer lower bound")?),
        ));
    }
    if upper == f64::INFINITY {
        records.push(bound(format::MpsBoundKind::PlusInfinity, variable, None));
    } else if upper != 1.0 {
        records.push(bound(
            format::MpsBoundKind::IntegerUpper,
            variable,
            Some(finite(upper, report, "integer upper bound")?),
        ));
    }
    Ok(())
}

fn encode_binary(
    variable: &projection::MpsWriteVariable,
    report: &MpsWriteReport,
    records: &mut Vec<format::MpsBoundRecord>,
) -> Result<(), MpsWriteError> {
    let bounds = variable.effective_bounds;
    if bounds.lower < 0.0 || bounds.upper > 1.0 {
        return Err(internal_invariant(
            report,
            "projection emitted binary bounds outside [0, 1]",
        ));
    }
    records.push(bound(format::MpsBoundKind::Binary, variable, None));
    if bounds.lower == bounds.upper {
        records.push(bound(
            format::MpsBoundKind::Fixed,
            variable,
            Some(finite(bounds.lower, report, "fixed binary bound")?),
        ));
    } else {
        if bounds.lower != 0.0 {
            records.push(bound(
                format::MpsBoundKind::Lower,
                variable,
                Some(finite(bounds.lower, report, "binary lower bound")?),
            ));
        }
        if bounds.upper != 1.0 {
            records.push(bound(
                format::MpsBoundKind::Upper,
                variable,
                Some(finite(bounds.upper, report, "binary upper bound")?),
            ));
        }
    }
    Ok(())
}

fn bound(
    kind: format::MpsBoundKind,
    variable: &projection::MpsWriteVariable,
    value: Option<f64>,
) -> format::MpsBoundRecord {
    format::MpsBoundRecord {
        kind,
        variable: variable.name.clone(),
        value,
    }
}

fn finite(value: f64, report: &MpsWriteReport, field: &str) -> Result<f64, MpsWriteError> {
    if value.is_finite() {
        Ok(if value == 0.0 { 0.0 } else { value })
    } else {
        Err(MpsWriteError::new(
            MpsWriteErrorKind::NonFiniteValue,
            context(report).with_numeric_field(field),
        ))
    }
}

fn context(report: &MpsWriteReport) -> MpsWriteContext {
    MpsWriteContext::default().with_model_state(
        report.model_lineage,
        report.model_instance,
        report.model_revision,
    )
}

fn internal_invariant(report: &MpsWriteReport, message: &str) -> MpsWriteError {
    MpsWriteError::new(
        MpsWriteErrorKind::InternalInvariant,
        context(report).with_message(message),
    )
}
