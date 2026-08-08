//! Section and integer-marker state for the streaming MPS lexer.

use super::{record::MpsRecord, MpsDiagnostic, MpsError, MpsErrorKind, MpsSection, MpsSourceSpan};

#[derive(Default)]
pub(crate) struct LexerState {
    current: Option<MpsSection>,
    previous_rank: Option<u8>,
    rows_seen: bool,
    columns_seen: bool,
    endata_seen: bool,
    marker_active: bool,
    pending_payload: Option<MpsSection>,
}

impl LexerState {
    pub(crate) fn current(&self) -> Option<&MpsSection> {
        self.current.as_ref()
    }

    pub(crate) fn marker_active(&self) -> bool {
        self.marker_active
    }

    pub(crate) fn begin_section(
        &mut self,
        section: MpsSection,
        span: &MpsSourceSpan,
    ) -> Result<(), MpsError> {
        if self.endata_seen {
            return Err(error(
                MpsErrorKind::InvalidRecord,
                span,
                self.current.clone(),
                "data appears after ENDATA",
            ));
        }
        if self.marker_active && !matches!(section, MpsSection::Columns) {
            return Err(error(
                MpsErrorKind::InvalidMarkerNesting,
                span,
                Some(MpsSection::Columns),
                "INTORG must be closed by INTEND before leaving COLUMNS",
            ));
        }
        if let Some(pending) = &self.pending_payload {
            return Err(error(
                MpsErrorKind::InvalidRecord,
                span,
                Some(pending.clone()),
                "section requires one supported data record",
            ));
        }

        let rank = section_rank(&section).ok_or_else(|| {
            error(
                MpsErrorKind::InvalidRecord,
                span,
                self.current.clone(),
                "unknown MPS section header",
            )
        })?;
        if self.previous_rank.is_some_and(|previous| rank <= previous) {
            return Err(error(
                MpsErrorKind::InvalidSectionOrder,
                span,
                Some(section.clone()),
                "section is duplicated or out of order",
            ));
        }
        if matches!(section, MpsSection::Columns) && !self.rows_seen {
            return Err(error(
                MpsErrorKind::InvalidSectionOrder,
                span,
                Some(section.clone()),
                "COLUMNS requires a preceding ROWS section",
            ));
        }
        if matches!(
            section,
            MpsSection::Rhs | MpsSection::Ranges | MpsSection::Bounds | MpsSection::Endata
        ) && !self.columns_seen
        {
            return Err(error(
                MpsErrorKind::InvalidSectionOrder,
                span,
                Some(section.clone()),
                "section requires a preceding COLUMNS section",
            ));
        }

        self.previous_rank = Some(rank);
        self.rows_seen |= matches!(section, MpsSection::Rows);
        self.columns_seen |= matches!(section, MpsSection::Columns);
        self.endata_seen |= matches!(section, MpsSection::Endata);
        self.pending_payload = matches!(section, MpsSection::ObjSense | MpsSection::ObjName)
            .then_some(section.clone());
        self.current = Some(section);
        Ok(())
    }

    pub(crate) fn accept_record(&mut self, record: &MpsRecord) -> Result<(), MpsError> {
        let section = self.current.clone().ok_or_else(|| {
            error(
                MpsErrorKind::InvalidSectionOrder,
                record.span(),
                None,
                "record appears before its section header",
            )
        })?;
        if !record_matches_section(record, &section) {
            return Err(error(
                MpsErrorKind::InvalidRecord,
                record.span(),
                Some(section),
                "record does not belong to the active section",
            ));
        }
        if matches!(
            record,
            MpsRecord::ObjSense { .. } | MpsRecord::ObjName { .. }
        ) {
            self.pending_payload = None;
        }
        match record {
            MpsRecord::Marker {
                marker: super::record::IntegerMarker::Start,
                ..
            } if self.marker_active => Err(error(
                MpsErrorKind::InvalidMarkerNesting,
                record.span(),
                Some(MpsSection::Columns),
                "INTORG cannot nest inside another integer-marker block",
            )),
            MpsRecord::Marker {
                marker: super::record::IntegerMarker::End,
                ..
            } if !self.marker_active => Err(error(
                MpsErrorKind::InvalidMarkerNesting,
                record.span(),
                Some(MpsSection::Columns),
                "INTEND requires an active INTORG block",
            )),
            MpsRecord::Marker {
                marker: super::record::IntegerMarker::Start,
                ..
            } => {
                self.marker_active = true;
                Ok(())
            }
            MpsRecord::Marker {
                marker: super::record::IntegerMarker::End,
                ..
            } => {
                self.marker_active = false;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn finish(&self) -> Result<(), MpsError> {
        let diagnostic = MpsDiagnostic::new().with_section(
            self.current
                .clone()
                .unwrap_or(MpsSection::Other("input".to_owned())),
        );
        if self.marker_active {
            return Err(MpsError::new(
                MpsErrorKind::InvalidMarkerNesting,
                diagnostic,
            ));
        }
        if !self.endata_seen {
            return Err(MpsError::new(MpsErrorKind::MissingEndata, diagnostic));
        }
        if !self.rows_seen || !self.columns_seen {
            return Err(MpsError::new(
                MpsErrorKind::MissingRequiredSection,
                diagnostic,
            ));
        }
        if let Some(section) = &self.pending_payload {
            return Err(MpsError::new(
                MpsErrorKind::InvalidRecord,
                MpsDiagnostic::new()
                    .with_section(section.clone())
                    .with_message("section requires one supported data record"),
            ));
        }
        Ok(())
    }
}

fn section_rank(section: &MpsSection) -> Option<u8> {
    match section {
        MpsSection::Name => Some(0),
        MpsSection::ObjSense => Some(1),
        MpsSection::ObjName => Some(2),
        MpsSection::Rows => Some(3),
        MpsSection::Columns => Some(4),
        MpsSection::Rhs => Some(5),
        MpsSection::Ranges => Some(6),
        MpsSection::Bounds => Some(7),
        MpsSection::Endata => Some(8),
        _ => None,
    }
}

fn record_matches_section(record: &MpsRecord, section: &MpsSection) -> bool {
    matches!(
        (record, section),
        (MpsRecord::Name { .. }, MpsSection::Name)
            | (MpsRecord::ObjSense { .. }, MpsSection::ObjSense)
            | (MpsRecord::ObjName { .. }, MpsSection::ObjName)
            | (MpsRecord::Row { .. }, MpsSection::Rows)
            | (
                MpsRecord::Column { .. } | MpsRecord::Marker { .. },
                MpsSection::Columns
            )
            | (MpsRecord::Rhs { .. }, MpsSection::Rhs)
            | (MpsRecord::Ranges { .. }, MpsSection::Ranges)
            | (MpsRecord::Bound { .. }, MpsSection::Bounds)
    )
}

fn error(
    kind: MpsErrorKind,
    span: &MpsSourceSpan,
    section: Option<MpsSection>,
    message: &'static str,
) -> MpsError {
    let mut diagnostic = MpsDiagnostic::new()
        .with_span(span.clone())
        .with_message(message);
    if let Some(section) = section {
        diagnostic = diagnostic.with_section(section);
    }
    MpsError::new(kind, diagnostic)
}
