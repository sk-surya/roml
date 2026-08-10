//! Focused tests for P36 Task 36-02A: variable bounds and integer markers.
//!
//! The production writer integration is intentionally serial after Wave 2.
//! This test-local module wiring exercises the production projection and
//! formatter seams while keeping the bounds/marker implementation isolated.

use std::io::Cursor;

use roml::{
    binary, continuous, integer,
    io::mps::{MpsReader, MpsWriteError, MpsWriteErrorKind, MpsWriter},
    model::{Bounds, VarType, VariableDef},
    Model,
};

mod id {
    pub use roml::id::*;
}
mod snapshot {
    pub use roml::snapshot::*;
}
mod construct {
    pub use roml::construct::*;
}
mod model {
    pub use roml::model::*;
}
mod io {
    pub mod mps {
        pub mod write {
            pub use roml::io::mps::write::*;
        }
    }
}
pub use roml::{expr, function, value_expr};

#[path = "../src/io/mps/write/bounds.rs"]
#[allow(dead_code)]
mod bounds;
#[path = "../src/io/mps/write/format.rs"]
#[allow(dead_code)]
mod format;
#[path = "../src/io/mps/write/projection.rs"]
#[allow(dead_code)]
mod projection;

#[derive(Clone, Copy, Debug, PartialEq)]
struct ExpectedBound {
    kind: format::MpsBoundKind,
    value: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
struct DomainCase {
    label: &'static str,
    definition: fn() -> VariableDef,
    declared: Bounds,
    fixing: Option<f64>,
    expected: &'static [ExpectedBound],
}

const CONTINUOUS_DEFAULT: &[ExpectedBound] = &[];
const CONTINUOUS_FREE: &[ExpectedBound] = &[ExpectedBound {
    kind: format::MpsBoundKind::Free,
    value: None,
}];
const CONTINUOUS_LOWER: &[ExpectedBound] = &[ExpectedBound {
    kind: format::MpsBoundKind::Lower,
    value: Some(2.0),
}];
const CONTINUOUS_UPPER: &[ExpectedBound] = &[
    ExpectedBound {
        kind: format::MpsBoundKind::MinusInfinity,
        value: None,
    },
    ExpectedBound {
        kind: format::MpsBoundKind::Upper,
        value: Some(7.0),
    },
];
const CONTINUOUS_FIXED: &[ExpectedBound] = &[ExpectedBound {
    kind: format::MpsBoundKind::Fixed,
    value: Some(3.0),
}];
const CONTINUOUS_INTERVAL: &[ExpectedBound] = &[
    ExpectedBound {
        kind: format::MpsBoundKind::Lower,
        value: Some(-2.0),
    },
    ExpectedBound {
        kind: format::MpsBoundKind::Upper,
        value: Some(4.0),
    },
];
const BINARY_DEFAULT: &[ExpectedBound] = &[ExpectedBound {
    kind: format::MpsBoundKind::Binary,
    value: None,
}];
const BINARY_CUSTOM: &[ExpectedBound] = &[
    ExpectedBound {
        kind: format::MpsBoundKind::Binary,
        value: None,
    },
    ExpectedBound {
        kind: format::MpsBoundKind::Lower,
        value: Some(0.25),
    },
    ExpectedBound {
        kind: format::MpsBoundKind::Upper,
        value: Some(0.75),
    },
];
const INTEGER_DEFAULT: &[ExpectedBound] = &[ExpectedBound {
    kind: format::MpsBoundKind::PlusInfinity,
    value: None,
}];
const INTEGER_CUSTOM: &[ExpectedBound] = &[
    ExpectedBound {
        kind: format::MpsBoundKind::IntegerLower,
        value: Some(-2.0),
    },
    ExpectedBound {
        kind: format::MpsBoundKind::IntegerUpper,
        value: Some(5.0),
    },
];
const INTEGER_FREE: &[ExpectedBound] = &[
    ExpectedBound {
        kind: format::MpsBoundKind::MinusInfinity,
        value: None,
    },
    ExpectedBound {
        kind: format::MpsBoundKind::PlusInfinity,
        value: None,
    },
];
const PERSISTENT_FIXING: &[ExpectedBound] = &[ExpectedBound {
    kind: format::MpsBoundKind::Fixed,
    value: Some(6.0),
}];

const DOMAIN_CASES: &[DomainCase] = &[
    DomainCase {
        label: "continuous default",
        definition: || continuous(),
        declared: Bounds::NON_NEGATIVE,
        fixing: None,
        expected: CONTINUOUS_DEFAULT,
    },
    DomainCase {
        label: "continuous free",
        definition: || continuous().bounds(f64::NEG_INFINITY, f64::INFINITY),
        declared: Bounds::UNBOUNDED,
        fixing: None,
        expected: CONTINUOUS_FREE,
    },
    DomainCase {
        label: "continuous lower",
        definition: || continuous().bounds(2.0, f64::INFINITY),
        declared: Bounds::new(2.0, f64::INFINITY),
        fixing: None,
        expected: CONTINUOUS_LOWER,
    },
    DomainCase {
        label: "continuous upper",
        definition: || continuous().bounds(f64::NEG_INFINITY, 7.0),
        declared: Bounds::new(f64::NEG_INFINITY, 7.0),
        fixing: None,
        expected: CONTINUOUS_UPPER,
    },
    DomainCase {
        label: "continuous fixed",
        definition: || continuous().bounds(3.0, 3.0),
        declared: Bounds::new(3.0, 3.0),
        fixing: None,
        expected: CONTINUOUS_FIXED,
    },
    DomainCase {
        label: "continuous finite interval",
        definition: || continuous().bounds(-2.0, 4.0),
        declared: Bounds::new(-2.0, 4.0),
        fixing: None,
        expected: CONTINUOUS_INTERVAL,
    },
    DomainCase {
        label: "binary default",
        definition: || binary(),
        declared: Bounds::BINARY,
        fixing: None,
        expected: BINARY_DEFAULT,
    },
    DomainCase {
        label: "binary custom",
        definition: || binary().bounds(0.25, 0.75),
        declared: Bounds::new(0.25, 0.75),
        fixing: None,
        expected: BINARY_CUSTOM,
    },
    DomainCase {
        label: "integer default",
        definition: || integer(),
        declared: Bounds::NON_NEGATIVE,
        fixing: None,
        expected: INTEGER_DEFAULT,
    },
    DomainCase {
        label: "integer custom",
        definition: || integer().bounds(-2.0, 5.0),
        declared: Bounds::new(-2.0, 5.0),
        fixing: None,
        expected: INTEGER_CUSTOM,
    },
    DomainCase {
        label: "integer free",
        definition: || integer().bounds(f64::NEG_INFINITY, f64::INFINITY),
        declared: Bounds::UNBOUNDED,
        fixing: None,
        expected: INTEGER_FREE,
    },
    DomainCase {
        label: "persistent fixing",
        definition: || continuous().bounds(-2.0, 8.0),
        declared: Bounds::new(-2.0, 8.0),
        fixing: Some(6.0),
        expected: PERSISTENT_FIXING,
    },
];

fn project_case(case: &DomainCase) -> (Model, projection::MpsWriteDocument) {
    let mut model = Model::with_name(case.label);
    let variable = model
        .add_variable(((case.definition)()).named("v"))
        .expect("table case has a valid domain");
    if let Some(value) = case.fixing {
        model.fix(variable, value).expect("table fixing is valid");
    }
    let document = projection::project(
        &model,
        roml::io::mps::write::MpsNamePolicy::PreserveOrGenerate,
    )
    .expect("primitive table case projects");
    (model, document)
}

fn emit_bounds_document(semantic: &projection::MpsWriteDocument) -> Result<Vec<u8>, MpsWriteError> {
    let report = &semantic.report;
    let entries = vec![
        vec![format::MpsEntry {
            row: "OBJ".to_owned(),
            value: 0.0,
        }];
        semantic.variables.len()
    ];
    let columns = bounds::encode_columns(&semantic.variables, entries, report)?;
    let records = bounds::encode_bounds(&semantic.variables, report)?;
    let mut document = format::MpsWriteDocument::minimal("BOUNDS");
    document.columns = columns;
    document.bounds = (!records.is_empty()).then_some(records);
    format::format_document(&document).map_err(|error| {
        MpsWriteError::new(
            MpsWriteErrorKind::Serialization,
            roml::io::mps::write::MpsWriteContext::default().with_message(error.to_string()),
        )
    })
}

#[derive(Debug, PartialEq)]
struct DomainOracle {
    name: String,
    var_type: VarType,
    bounds: Bounds,
}

fn model_oracle(model: &Model) -> DomainOracle {
    let snapshot = model.take_snapshot().expect("model snapshot");
    let entry = snapshot
        .variables
        .iter()
        .find(|entry| entry.active)
        .expect("one active variable");
    DomainOracle {
        name: model
            .variable_name(entry.id)
            .expect("variable is live")
            .expect("table variable is named")
            .to_owned(),
        var_type: entry.var_type,
        bounds: model.effective_bounds(entry.id).expect("effective bounds"),
    }
}

fn imported_oracle(imported: &roml::io::mps::MpsImport) -> DomainOracle {
    let snapshot = imported.model.take_snapshot().expect("imported snapshot");
    let entry = snapshot
        .variables
        .iter()
        .find(|entry| entry.active)
        .expect("one imported active variable");
    DomainOracle {
        name: imported
            .model
            .variable_name(entry.id)
            .expect("imported variable is live")
            .expect("imported variable is named")
            .to_owned(),
        var_type: entry.var_type,
        bounds: entry.bounds,
    }
}

#[test]
fn table_drives_domains_defaults_overrides_and_persistent_fixing() {
    for case in DOMAIN_CASES {
        let (model, semantic) = project_case(case);
        assert_eq!(
            semantic.variables[0].declared_bounds, case.declared,
            "{}",
            case.label
        );
        let records = bounds::encode_bounds(&semantic.variables, &semantic.report)
            .unwrap_or_else(|error| panic!("{}: {error}", case.label));
        let actual = records
            .iter()
            .map(|record| ExpectedBound {
                kind: record.kind,
                value: record.value,
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, case.expected, "{}", case.label);

        let bytes = emit_bounds_document(&semantic)
            .unwrap_or_else(|error| panic!("{}: {error}", case.label));
        let imported = MpsReader::new()
            .read(Cursor::new(bytes))
            .unwrap_or_else(|error| panic!("{}: {error}", case.label));
        assert_eq!(
            imported_oracle(&imported),
            model_oracle(&model),
            "{}",
            case.label
        );
    }
}

#[test]
fn integer_marker_regions_are_deterministic_and_contiguous() {
    let mut model = Model::new();
    model
        .add_variable(continuous().named("c1"))
        .expect("continuous variable");
    model
        .add_variable(integer().named("i1"))
        .expect("integer variable");
    model
        .add_variable(binary().named("b1"))
        .expect("binary variable");
    model
        .add_variable(continuous().named("c2"))
        .expect("continuous variable");
    model
        .add_variable(integer().named("i2"))
        .expect("integer variable");
    let semantic = projection::project(
        &model,
        roml::io::mps::write::MpsNamePolicy::PreserveOrGenerate,
    )
    .expect("mixed domain projection");
    let entries = vec![
        vec![format::MpsEntry {
            row: "OBJ".to_owned(),
            value: 0.0,
        }];
        semantic.variables.len()
    ];
    let columns = bounds::encode_columns(&semantic.variables, entries, &semantic.report)
        .expect("marker encoding");

    let labels = columns
        .iter()
        .map(|column| match column {
            format::MpsColumnRecord::Marker { name, kind } => {
                format!("marker:{name}:{kind:?}")
            }
            format::MpsColumnRecord::Entries { name, .. } => format!("column:{name}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        [
            "column:c1",
            "marker:MARK000001:Start",
            "column:i1",
            "column:b1",
            "marker:MARK000001:End",
            "column:c2",
            "marker:MARK000002:Start",
            "column:i2",
            "marker:MARK000002:End",
        ]
    );

    let first = emit_bounds_document(&semantic).expect("first mixed encoding");
    let second = emit_bounds_document(&semantic).expect("second mixed encoding");
    assert_eq!(first, second);
    let text = String::from_utf8(first).expect("formatter emits UTF-8");
    assert_eq!(text.matches("'INTORG'").count(), 2);
    assert_eq!(text.matches("'INTEND'").count(), 2);
}

#[test]
fn production_writer_round_trip_keeps_persistent_fixing_as_effective_domain() {
    let mut model = Model::new();
    let variable = model
        .add_variable(continuous().named("fixed").bounds(-4.0, 9.0))
        .unwrap();
    model.fix(variable, 2.5).unwrap();

    let mut bytes = Vec::new();
    let report = MpsWriter::new().write(&model, &mut bytes).unwrap();
    let imported = MpsReader::new()
        .read(Cursor::new(bytes))
        .expect("integrated writer output is readable");
    assert_eq!(imported_oracle(&imported), model_oracle(&model));
    assert_eq!(report.lowerings.len(), 1);
}
