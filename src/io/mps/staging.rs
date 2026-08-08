//! Private MPS staging representation shared by the lexer and resolver.

use std::collections::HashSet;

use super::{
    vectors::MpsNamedVectors, MpsDiagnostic, MpsError, MpsErrorKind, MpsResourceLimits, MpsSection,
    MpsSourceSpan,
};

/// The finite set of supported MPS row kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MpsRowKind {
    /// An equality row.
    Equal,
    /// A greater-than row.
    GreaterThan,
    /// A less-than row.
    LessThan,
    /// A free row, potentially selected as the objective.
    Free,
}

/// The objective sense declared by `OBJSENSE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MpsObjectiveSense {
    /// Minimize the selected objective.
    Minimize,
    /// Maximize the selected objective.
    Maximize,
}

/// A supported MPS BOUNDS record kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MpsBoundKind {
    /// `FR`.
    Free,
    /// `FX`.
    Fixed,
    /// `LO`.
    Lower,
    /// `MI`.
    MinusInfinity,
    /// `PL`.
    PlusInfinity,
    /// `UP`.
    Upper,
    /// `BV`.
    Binary,
    /// `LI`.
    IntegerLower,
    /// `UI`.
    IntegerUpper,
}

impl MpsBoundKind {
    fn requires_value(self) -> bool {
        matches!(
            self,
            Self::Fixed | Self::Lower | Self::Upper | Self::IntegerLower | Self::IntegerUpper
        )
    }
}

/// One declared MPS row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MpsStagedRow {
    name: String,
    kind: MpsRowKind,
    span: MpsSourceSpan,
}

impl MpsStagedRow {
    /// Returns the exact row name.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Returns the declared row kind.
    pub(crate) fn kind(&self) -> MpsRowKind {
        self.kind
    }

    /// Returns the row declaration span.
    pub(crate) fn span(&self) -> &MpsSourceSpan {
        &self.span
    }
}

/// A single sparse matrix record retained in file order.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsColumnEntry {
    row_name: String,
    value: f64,
    span: MpsSourceSpan,
}

impl MpsColumnEntry {
    /// Returns the referenced row name.
    pub(crate) fn row_name(&self) -> &str {
        &self.row_name
    }

    /// Returns the uncombined MPS coefficient.
    pub(crate) fn value(&self) -> f64 {
        self.value
    }

    /// Returns the source span of this record.
    pub(crate) fn span(&self) -> &MpsSourceSpan {
        &self.span
    }
}

/// One MPS column with all of its sparse records.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsStagedColumn {
    name: String,
    entries: Vec<MpsColumnEntry>,
}

/// One staged RHS vector record.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsRhsEntry {
    row_name: String,
    value: f64,
    span: MpsSourceSpan,
}

impl MpsRhsEntry {
    /// Returns the referenced row name.
    pub(crate) fn row_name(&self) -> &str {
        &self.row_name
    }

    /// Returns the staged RHS value.
    pub(crate) fn value(&self) -> f64 {
        self.value
    }

    /// Returns the source span of this record.
    pub(crate) fn span(&self) -> &MpsSourceSpan {
        &self.span
    }
}

/// One staged RANGES vector record.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsRangeEntry {
    row_name: String,
    value: f64,
    span: MpsSourceSpan,
}

impl MpsRangeEntry {
    /// Returns the referenced row name.
    pub(crate) fn row_name(&self) -> &str {
        &self.row_name
    }

    /// Returns the staged range value.
    pub(crate) fn value(&self) -> f64 {
        self.value
    }

    /// Returns the source span of this record.
    pub(crate) fn span(&self) -> &MpsSourceSpan {
        &self.span
    }
}

/// One staged BOUNDS vector record.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsBoundEntry {
    kind: MpsBoundKind,
    variable_name: String,
    value: Option<f64>,
    span: MpsSourceSpan,
}

impl MpsBoundEntry {
    /// Returns the supported bound transition kind.
    pub(crate) fn kind(&self) -> MpsBoundKind {
        self.kind
    }

    /// Returns the targeted variable name.
    pub(crate) fn variable_name(&self) -> &str {
        &self.variable_name
    }

    /// Returns the optional numeric operand.
    pub(crate) fn value(&self) -> Option<f64> {
        self.value
    }

    /// Returns the source span of this record.
    pub(crate) fn span(&self) -> &MpsSourceSpan {
        &self.span
    }
}

impl MpsStagedColumn {
    /// Returns the exact variable name.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Returns all uncombined matrix records in encounter order.
    pub(crate) fn entries(&self) -> &[MpsColumnEntry] {
        &self.entries
    }

    /// Returns the first ordinary COLUMNS-record span for this variable.
    pub(crate) fn first_entry_span(&self) -> Option<&MpsSourceSpan> {
        self.entries.first().map(MpsColumnEntry::span)
    }
}

/// MPS-specific input retained until selected-vector semantic resolution.
#[derive(Clone, Debug)]
pub(crate) struct MpsStaging {
    limits: MpsResourceLimits,
    staged_records: usize,
    staged_nonzeros: usize,
    problem_name: Option<(String, MpsSourceSpan)>,
    objective_sense: Option<(MpsObjectiveSense, MpsSourceSpan)>,
    objective_name: Option<(String, MpsSourceSpan)>,
    rows: Vec<MpsStagedRow>,
    columns: Vec<MpsStagedColumn>,
    rhs_vectors: MpsNamedVectors<MpsRhsEntry>,
    range_vectors: MpsNamedVectors<MpsRangeEntry>,
    bound_vectors: MpsNamedVectors<MpsBoundEntry>,
}

impl MpsStaging {
    /// Creates empty private MPS staging storage.
    pub(crate) fn new(limits: MpsResourceLimits) -> Self {
        Self {
            limits,
            staged_records: 0,
            staged_nonzeros: 0,
            problem_name: None,
            objective_sense: None,
            objective_name: None,
            rows: Vec::new(),
            columns: Vec::new(),
            rhs_vectors: MpsNamedVectors::new(),
            range_vectors: MpsNamedVectors::new(),
            bound_vectors: MpsNamedVectors::new(),
        }
    }

    /// Records the optional `NAME` record.
    pub(crate) fn set_problem_name(&mut self, name: impl Into<String>, span: MpsSourceSpan) {
        self.problem_name = Some((name.into(), span));
    }

    /// Records an optional `OBJSENSE` declaration.
    pub(crate) fn set_objective_sense(&mut self, sense: MpsObjectiveSense, span: MpsSourceSpan) {
        self.objective_sense = Some((sense, span));
    }

    /// Records an optional `OBJNAME` declaration.
    pub(crate) fn set_objective_name(&mut self, name: impl Into<String>, span: MpsSourceSpan) {
        self.objective_name = Some((name.into(), span));
    }

    /// Adds a declared row in input order.
    pub(crate) fn add_row(
        &mut self,
        kind: MpsRowKind,
        name: impl Into<String>,
        span: MpsSourceSpan,
    ) -> Result<(), MpsError> {
        self.check_next_count(
            self.rows.len(),
            self.limits.max_rows,
            "max_rows",
            MpsSection::Rows,
            &span,
        )?;
        self.reserve_record(MpsSection::Rows, &span)?;
        self.rows.push(MpsStagedRow {
            name: name.into(),
            kind,
            span,
        });
        Ok(())
    }

    /// Adds one matrix record, merging repeated column blocks but never cells.
    pub(crate) fn add_column_entry(
        &mut self,
        column_name: impl Into<String>,
        row_name: impl Into<String>,
        value: f64,
        span: MpsSourceSpan,
    ) -> Result<(), MpsError> {
        let column_name = column_name.into();
        let column_is_new = !self.columns.iter().any(|column| column.name == column_name);
        if column_is_new {
            self.check_next_count(
                self.columns.len(),
                self.limits.max_columns,
                "max_columns",
                MpsSection::Columns,
                &span,
            )?;
        }
        let next_nonzeros = self.checked_next_count(
            self.staged_nonzeros,
            self.limits.max_nonzeros,
            "max_nonzeros",
            MpsSection::Columns,
            &span,
        )?;
        self.reserve_record(MpsSection::Columns, &span)?;
        let entry = MpsColumnEntry {
            row_name: row_name.into(),
            value,
            span,
        };
        if let Some(column) = self
            .columns
            .iter_mut()
            .find(|column| column.name == column_name)
        {
            column.entries.push(entry);
        } else {
            self.columns.push(MpsStagedColumn {
                name: column_name,
                entries: vec![entry],
            });
        }
        self.staged_nonzeros = next_nonzeros;
        Ok(())
    }

    /// Adds a record to its named RHS vector without resolving its semantics.
    pub(crate) fn add_rhs_entry(
        &mut self,
        vector_name: impl Into<String>,
        row_name: impl Into<String>,
        value: f64,
        span: MpsSourceSpan,
    ) -> Result<(), MpsError> {
        self.reserve_record(MpsSection::Rhs, &span)?;
        self.rhs_vectors.push(
            vector_name,
            MpsRhsEntry {
                row_name: row_name.into(),
                value,
                span,
            },
        );
        Ok(())
    }

    /// Adds a record to its named RANGES vector without applying it to a row.
    pub(crate) fn add_range_entry(
        &mut self,
        vector_name: impl Into<String>,
        row_name: impl Into<String>,
        value: f64,
        span: MpsSourceSpan,
    ) -> Result<(), MpsError> {
        self.reserve_record(MpsSection::Ranges, &span)?;
        self.range_vectors.push(
            vector_name,
            MpsRangeEntry {
                row_name: row_name.into(),
                value,
                span,
            },
        );
        Ok(())
    }

    /// Adds an ordered record to its named BOUNDS vector without applying it.
    pub(crate) fn add_bound_entry(
        &mut self,
        vector_name: impl Into<String>,
        kind: MpsBoundKind,
        variable_name: impl Into<String>,
        value: Option<f64>,
        span: MpsSourceSpan,
    ) -> Result<(), MpsError> {
        self.reserve_record(MpsSection::Bounds, &span)?;
        self.bound_vectors.push(
            vector_name,
            MpsBoundEntry {
                kind,
                variable_name: variable_name.into(),
                value,
                span,
            },
        );
        Ok(())
    }

    /// Verifies structural rules that do not depend on rim-vector selection.
    pub(crate) fn validate(self) -> Result<Self, MpsError> {
        let mut row_names = HashSet::with_capacity(self.rows.len());
        for row in &self.rows {
            if !row_names.insert(row.name.as_str()) {
                return Err(structural_error(
                    MpsErrorKind::DuplicateRow,
                    MpsSection::Rows,
                    row.name(),
                    row.span(),
                ));
            }
        }
        if let Some((objective_name, span)) = &self.objective_name {
            if !row_names.contains(objective_name.as_str()) {
                return Err(structural_error(
                    MpsErrorKind::UnknownRow,
                    MpsSection::ObjName,
                    objective_name,
                    span,
                ));
            }
        }
        let column_names = self
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<HashSet<_>>();
        for column in &self.columns {
            for entry in &column.entries {
                if !entry.value.is_finite() {
                    return Err(structural_error(
                        MpsErrorKind::InvalidNumber,
                        MpsSection::Columns,
                        entry.row_name(),
                        entry.span(),
                    ));
                }
                if !row_names.contains(entry.row_name()) {
                    return Err(structural_error(
                        MpsErrorKind::UnknownRow,
                        MpsSection::Columns,
                        entry.row_name(),
                        entry.span(),
                    ));
                }
            }
        }
        for vector in self.rhs_vectors.vectors() {
            for entry in vector.entries() {
                if !entry.value.is_finite() {
                    return Err(structural_error(
                        MpsErrorKind::InvalidNumber,
                        MpsSection::Rhs,
                        entry.row_name(),
                        entry.span(),
                    ));
                }
                if !row_names.contains(entry.row_name()) {
                    return Err(structural_error(
                        MpsErrorKind::UnknownRow,
                        MpsSection::Rhs,
                        entry.row_name(),
                        entry.span(),
                    ));
                }
            }
        }
        for vector in self.range_vectors.vectors() {
            for entry in vector.entries() {
                if !entry.value.is_finite() {
                    return Err(structural_error(
                        MpsErrorKind::InvalidNumber,
                        MpsSection::Ranges,
                        entry.row_name(),
                        entry.span(),
                    ));
                }
                if !row_names.contains(entry.row_name()) {
                    return Err(structural_error(
                        MpsErrorKind::UnknownRow,
                        MpsSection::Ranges,
                        entry.row_name(),
                        entry.span(),
                    ));
                }
            }
        }
        for vector in self.bound_vectors.vectors() {
            for entry in vector.entries() {
                if entry.kind.requires_value() && entry.value.is_none() {
                    return Err(structural_error(
                        MpsErrorKind::InvalidRecord,
                        MpsSection::Bounds,
                        entry.variable_name(),
                        entry.span(),
                    ));
                }
                if entry.value.is_some_and(|value| !value.is_finite()) {
                    return Err(structural_error(
                        MpsErrorKind::InvalidNumber,
                        MpsSection::Bounds,
                        entry.variable_name(),
                        entry.span(),
                    ));
                }
                if !column_names.contains(entry.variable_name()) {
                    return Err(structural_error(
                        MpsErrorKind::UnknownVariable,
                        MpsSection::Bounds,
                        entry.variable_name(),
                        entry.span(),
                    ));
                }
            }
        }
        Ok(self)
    }

    /// Returns the optional problem name.
    pub(crate) fn problem_name(&self) -> Option<&str> {
        self.problem_name.as_ref().map(|(name, _)| name.as_str())
    }

    /// Returns the `NAME` record span when a problem name was declared.
    pub(crate) fn problem_name_span(&self) -> Option<&MpsSourceSpan> {
        self.problem_name.as_ref().map(|(_, span)| span)
    }

    /// Returns the optional declared objective sense.
    pub(crate) fn objective_sense(&self) -> Option<MpsObjectiveSense> {
        self.objective_sense.as_ref().map(|(sense, _)| *sense)
    }

    /// Returns the `OBJSENSE` record span when a sense was declared.
    pub(crate) fn objective_sense_span(&self) -> Option<&MpsSourceSpan> {
        self.objective_sense.as_ref().map(|(_, span)| span)
    }

    /// Returns the optional selected-objective row name.
    pub(crate) fn objective_name(&self) -> Option<&str> {
        self.objective_name.as_ref().map(|(name, _)| name.as_str())
    }

    /// Returns the `OBJNAME` record span when an objective row was declared.
    pub(crate) fn objective_name_span(&self) -> Option<&MpsSourceSpan> {
        self.objective_name.as_ref().map(|(_, span)| span)
    }

    /// Returns declared rows in source order.
    pub(crate) fn rows(&self) -> &[MpsStagedRow] {
        &self.rows
    }

    /// Returns staged columns in first-seen order.
    pub(crate) fn columns(&self) -> &[MpsStagedColumn] {
        &self.columns
    }

    /// Returns named RHS vectors in first-seen order.
    pub(crate) fn rhs_vectors(&self) -> &MpsNamedVectors<MpsRhsEntry> {
        &self.rhs_vectors
    }

    /// Returns named RANGES vectors in first-seen order.
    pub(crate) fn range_vectors(&self) -> &MpsNamedVectors<MpsRangeEntry> {
        &self.range_vectors
    }

    /// Returns named BOUNDS vectors in first-seen order.
    pub(crate) fn bound_vectors(&self) -> &MpsNamedVectors<MpsBoundEntry> {
        &self.bound_vectors
    }

    fn reserve_record(
        &mut self,
        section: MpsSection,
        span: &MpsSourceSpan,
    ) -> Result<(), MpsError> {
        self.staged_records = self.checked_next_count(
            self.staged_records,
            self.limits.max_records,
            "max_records",
            section,
            span,
        )?;
        Ok(())
    }

    fn check_next_count(
        &self,
        current: usize,
        limit: usize,
        limit_name: &str,
        section: MpsSection,
        span: &MpsSourceSpan,
    ) -> Result<(), MpsError> {
        self.checked_next_count(current, limit, limit_name, section, span)
            .map(|_| ())
    }

    fn checked_next_count(
        &self,
        current: usize,
        limit: usize,
        limit_name: &str,
        section: MpsSection,
        span: &MpsSourceSpan,
    ) -> Result<usize, MpsError> {
        let next = current
            .checked_add(1)
            .ok_or_else(|| limit_error(limit_name, section.clone(), span, "counter overflow"))?;
        if next > limit {
            return Err(limit_error(
                limit_name,
                section,
                span,
                "configured limit exceeded",
            ));
        }
        Ok(next)
    }
}

fn structural_error(
    kind: MpsErrorKind,
    section: MpsSection,
    entity: &str,
    span: &MpsSourceSpan,
) -> MpsError {
    MpsError::new(
        kind,
        MpsDiagnostic::new()
            .with_section(section)
            .with_entity(entity)
            .with_span(span.clone()),
    )
}

fn limit_error(
    limit_name: &str,
    section: MpsSection,
    span: &MpsSourceSpan,
    reason: &str,
) -> MpsError {
    MpsError::new(
        MpsErrorKind::InvalidRecord,
        MpsDiagnostic::new()
            .with_section(section)
            .with_span(span.clone())
            .with_message(format!("{limit_name}: {reason}")),
    )
}

#[cfg(test)]
mod tests {
    use super::{MpsBoundKind, MpsObjectiveSense, MpsRowKind, MpsStaging};
    use crate::io::mps::{MpsErrorKind, MpsResourceLimits, MpsSourceSpan};

    fn span(line: usize) -> MpsSourceSpan {
        MpsSourceSpan::try_new(line, 0, 1).unwrap()
    }

    fn staging_with_variable() -> MpsStaging {
        let mut staging = MpsStaging::new(MpsResourceLimits::default());
        staging
            .add_row(MpsRowKind::Free, "objective", span(1))
            .unwrap();
        staging
            .add_row(MpsRowKind::LessThan, "capacity", span(2))
            .unwrap();
        staging
            .add_column_entry("shipment", "capacity", 1.0, span(3))
            .unwrap();
        staging
    }

    #[test]
    fn preserves_duplicate_columns_and_objective_metadata() {
        let mut staging = MpsStaging::new(MpsResourceLimits::default());
        staging.set_problem_name("transport", span(1));
        staging.set_objective_sense(MpsObjectiveSense::Maximize, span(2));
        staging.set_objective_name("profit", span(3));
        staging
            .add_row(MpsRowKind::Free, "profit", span(4))
            .unwrap();
        staging
            .add_row(MpsRowKind::LessThan, "capacity", span(5))
            .unwrap();

        staging
            .add_column_entry("shipment", "profit", 4.0, span(6))
            .unwrap();
        staging
            .add_column_entry("shipment", "capacity", 2.5, span(7))
            .unwrap();
        staging
            .add_column_entry("shipment", "capacity", -0.5, span(8))
            .unwrap();

        let staging = staging.validate().unwrap();
        assert_eq!(staging.problem_name(), Some("transport"));
        assert_eq!(staging.objective_sense(), Some(MpsObjectiveSense::Maximize));
        assert_eq!(staging.objective_name(), Some("profit"));
        assert_eq!(staging.rows().len(), 2);
        assert_eq!(staging.columns().len(), 1);

        let column = &staging.columns()[0];
        assert_eq!(column.name(), "shipment");
        assert_eq!(column.entries().len(), 3);
        assert_eq!(column.entries()[1].row_name(), "capacity");
        assert_eq!(column.entries()[1].value(), 2.5);
        assert_eq!(column.entries()[2].row_name(), "capacity");
        assert_eq!(column.entries()[2].value(), -0.5);
    }

    #[test]
    fn rejects_unknown_row_in_unselected_rhs_vector() {
        let mut staging = staging_with_variable();
        staging
            .add_rhs_entry("alternative", "missing_row", 10.0, span(4))
            .unwrap();

        let error = staging.validate().unwrap_err();
        assert_eq!(error.kind(), &MpsErrorKind::UnknownRow);
        assert_eq!(error.diagnostic().entity(), Some("missing_row"));
        assert_eq!(error.diagnostic().span(), Some(&span(4)));
    }

    #[test]
    fn rejects_unknown_row_in_columns_before_vector_selection() {
        let mut staging = MpsStaging::new(MpsResourceLimits::default());
        staging
            .add_row(MpsRowKind::Free, "objective", span(1))
            .unwrap();
        staging
            .add_column_entry("shipment", "undeclared", 1.0, span(2))
            .unwrap();

        let error = staging.validate().unwrap_err();
        assert_eq!(error.kind(), &MpsErrorKind::UnknownRow);
        assert_eq!(error.diagnostic().entity(), Some("undeclared"));
        assert_eq!(error.diagnostic().span(), Some(&span(2)));
    }

    #[test]
    fn rejects_non_finite_value_in_unselected_ranges_vector() {
        let mut staging = staging_with_variable();
        staging
            .add_range_entry("alternative", "capacity", f64::INFINITY, span(4))
            .unwrap();

        let error = staging.validate().unwrap_err();
        assert_eq!(error.kind(), &MpsErrorKind::InvalidNumber);
        assert_eq!(error.diagnostic().entity(), Some("capacity"));
        assert_eq!(error.diagnostic().span(), Some(&span(4)));
    }

    #[test]
    fn rejects_unknown_variable_in_unselected_bounds_vector() {
        let mut staging = staging_with_variable();
        staging
            .add_bound_entry(
                "alternative",
                MpsBoundKind::Lower,
                "undeclared",
                Some(0.0),
                span(4),
            )
            .unwrap();

        let error = staging.validate().unwrap_err();
        assert_eq!(error.kind(), &MpsErrorKind::UnknownVariable);
        assert_eq!(error.diagnostic().entity(), Some("undeclared"));
        assert_eq!(error.diagnostic().span(), Some(&span(4)));
    }

    #[test]
    fn rejects_a_second_column_when_the_column_limit_is_reached() {
        let limits = MpsResourceLimits {
            max_columns: 1,
            ..MpsResourceLimits::default()
        };
        let mut staging = MpsStaging::new(limits);
        staging
            .add_row(MpsRowKind::Free, "objective", span(1))
            .unwrap();
        staging
            .add_column_entry("shipment", "objective", 1.0, span(2))
            .unwrap();

        let error = staging
            .add_column_entry("overflow", "objective", 1.0, span(3))
            .unwrap_err();
        assert_eq!(error.kind(), &MpsErrorKind::InvalidRecord);
        assert_eq!(
            error.diagnostic().section(),
            Some(&crate::io::mps::MpsSection::Columns)
        );
        assert!(error
            .diagnostic()
            .message()
            .unwrap_or_default()
            .contains("max_columns"),);
    }

    #[test]
    fn rejects_duplicate_row_declarations_before_vector_selection() {
        let mut staging = MpsStaging::new(MpsResourceLimits::default());
        staging
            .add_row(MpsRowKind::LessThan, "capacity", span(1))
            .unwrap();
        staging
            .add_row(MpsRowKind::GreaterThan, "capacity", span(2))
            .unwrap();

        let error = staging.validate().unwrap_err();
        assert_eq!(error.kind(), &MpsErrorKind::DuplicateRow);
        assert_eq!(error.diagnostic().entity(), Some("capacity"));
        assert_eq!(error.diagnostic().span(), Some(&span(2)));
    }

    #[test]
    fn rejects_missing_numeric_operand_in_unselected_bounds_vector() {
        let mut staging = staging_with_variable();
        staging
            .add_bound_entry(
                "alternative",
                MpsBoundKind::Lower,
                "shipment",
                None,
                span(4),
            )
            .unwrap();

        let error = staging.validate().unwrap_err();
        assert_eq!(error.kind(), &MpsErrorKind::InvalidRecord);
        assert_eq!(
            error.diagnostic().section(),
            Some(&crate::io::mps::MpsSection::Bounds)
        );
        assert_eq!(error.diagnostic().entity(), Some("shipment"));
    }

    #[test]
    fn preserves_named_rim_vectors_and_defers_selected_vector_semantics() {
        let mut staging = staging_with_variable();
        staging
            .add_rhs_entry("baseline", "capacity", 10.0, span(4))
            .unwrap();
        staging
            .add_rhs_entry("alternate", "capacity", 20.0, span(5))
            .unwrap();
        staging
            .add_rhs_entry("alternate", "capacity", 30.0, span(6))
            .unwrap();
        staging
            .add_range_entry("baseline", "objective", 3.0, span(7))
            .unwrap();
        staging
            .add_bound_entry(
                "baseline",
                MpsBoundKind::Lower,
                "shipment",
                Some(2.0),
                span(8),
            )
            .unwrap();

        let staging = staging.validate().unwrap();

        let rhs = staging
            .rhs_vectors()
            .select(&crate::io::mps::MpsVectorSelection::First)
            .unwrap()
            .unwrap();
        assert_eq!(rhs.name(), "baseline");
        assert_eq!(rhs.entries()[0].value(), 10.0);

        let alternate_rhs = staging
            .rhs_vectors()
            .select(&crate::io::mps::MpsVectorSelection::Named(
                "alternate".to_owned(),
            ))
            .unwrap()
            .unwrap();
        assert_eq!(alternate_rhs.entries().len(), 2);

        assert_eq!(
            staging
                .range_vectors()
                .select(&crate::io::mps::MpsVectorSelection::First)
                .unwrap()
                .unwrap()
                .entries()[0]
                .row_name(),
            "objective"
        );
        assert_eq!(
            staging
                .bound_vectors()
                .select(&crate::io::mps::MpsVectorSelection::First)
                .unwrap()
                .unwrap()
                .entries()[0]
                .kind(),
            MpsBoundKind::Lower
        );
        assert_eq!(
            staging
                .bound_vectors()
                .select(&crate::io::mps::MpsVectorSelection::None)
                .unwrap(),
            None
        );
    }
}
