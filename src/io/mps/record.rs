//! Private lexical records passed from the streaming MPS lexer to staging.

use super::{MpsFormat, MpsSourceSpan};

/// The lexer output for one syntactically valid MPS document.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LexedDocument {
    pub(crate) format: MpsFormat,
    pub(crate) records: Vec<MpsRecord>,
}

/// One supported MPS lexical record with its complete source-line span.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MpsRecord {
    Name {
        name: String,
        span: MpsSourceSpan,
    },
    ObjSense {
        sense: ObjectiveSense,
        span: MpsSourceSpan,
    },
    ObjName {
        name: String,
        span: MpsSourceSpan,
    },
    Row {
        kind: RowKind,
        name: String,
        span: MpsSourceSpan,
    },
    Column {
        variable: String,
        entries: Vec<RowValue>,
        integer: bool,
        integer_marker_span: Option<MpsSourceSpan>,
        span: MpsSourceSpan,
    },
    Marker {
        marker: IntegerMarker,
        span: MpsSourceSpan,
    },
    Rhs {
        vector: String,
        entries: Vec<RowValue>,
        span: MpsSourceSpan,
    },
    Ranges {
        vector: String,
        entries: Vec<RowValue>,
        span: MpsSourceSpan,
    },
    Bound {
        kind: BoundKind,
        vector: String,
        variable: String,
        value: Option<f64>,
        span: MpsSourceSpan,
    },
}

impl MpsRecord {
    pub(crate) fn span(&self) -> &MpsSourceSpan {
        match self {
            Self::Name { span, .. }
            | Self::ObjSense { span, .. }
            | Self::ObjName { span, .. }
            | Self::Row { span, .. }
            | Self::Column { span, .. }
            | Self::Marker { span, .. }
            | Self::Rhs { span, .. }
            | Self::Ranges { span, .. }
            | Self::Bound { span, .. } => span,
        }
    }
}

/// A row/value pair on a matrix or rim-vector record.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RowValue {
    pub(crate) row: String,
    pub(crate) value: f64,
}

/// A supported MPS row classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowKind {
    E,
    G,
    L,
    N,
}

/// A supported objective-sense token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ObjectiveSense {
    Minimize,
    Maximize,
}

/// A balanced integer-marker control record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IntegerMarker {
    Start,
    End,
}

/// A supported MPS bound transition kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BoundKind {
    Fr,
    Fx,
    Lo,
    Mi,
    Pl,
    Up,
    Bv,
    Li,
    Ui,
}

impl BoundKind {
    pub(crate) fn requires_value(self) -> bool {
        matches!(self, Self::Fx | Self::Lo | Self::Up | Self::Li | Self::Ui)
    }
}
