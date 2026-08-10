//! P36 deterministic 94-model MPS write-back qualification runner.
//!
//! The runner is intentionally an example rather than a library API. It
//! validates the reviewed corpus pin and exact manifest, writes every model to
//! a private qualification directory, and emits one machine-readable result
//! row per manifest entry.

#[allow(dead_code)]
#[path = "../../tests/support/corpus.rs"]
mod corpus;

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

use roml::{
    io::mps::{MpsReader, MpsWriter},
    model::{CoefficientTarget, Sense, VarType},
    snapshot::ModelSnapshot,
    ConstraintBounds, Model,
};
use roml_highs::{
    compare_mps_solve, compare_mps_structure, observe_mps_differential,
    observe_mps_solve_differential, MpsSolveComparison,
};

const EXPECTED_NETLIB_COMMIT: &str = "56257eea85b433ce6aa67d26156b36385318fd6f";
const SOLVE_SUBSET: [&str; 3] = ["blend.mps", "fit2d.mps", "gfrd-pnc.mps"];
const P36_STRUCTURAL_ABS_TOLERANCE: f64 = 1e-10;
const P36_STRUCTURAL_REL_TOLERANCE: f64 = 1e-10;

#[derive(Clone, Debug, PartialEq)]
struct SemanticModel {
    variables: Vec<VariableSemantics>,
    rows: Vec<ConstraintBounds>,
    matrix: Vec<(usize, usize, f64)>,
    objective: ObjectiveSemantics,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct VariableSemantics {
    lower: f64,
    upper: f64,
    var_type: VarType,
}

#[derive(Clone, Debug, PartialEq)]
struct ObjectiveSemantics {
    sense: Sense,
    offset: f64,
    coefficients: Vec<(usize, f64)>,
}

#[derive(Debug)]
struct FileResult {
    name: String,
    status: &'static str,
    detail: String,
    output_bytes: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os().skip(1);
    let mut repository_root = env::current_dir()?;
    let mut root_supplied = false;
    let mut only = None;
    while let Some(argument) = arguments.next() {
        if argument == "--only" {
            only = Some(
                arguments
                    .next()
                    .ok_or("--only requires a manifest filename")?
                    .to_string_lossy()
                    .into_owned(),
            );
        } else if !root_supplied {
            repository_root = PathBuf::from(argument);
            root_supplied = true;
        } else {
            return Err(
                "usage: mps_write_corpus_qualification [repository-root] [--only filename]".into(),
            );
        }
    }

    let expected = expected_manifest();
    if expected.len() != 94 {
        return Err(format!(
            "P36 manifest contains {} entries, expected 94",
            expected.len()
        )
        .into());
    }
    let Some([_, netlib]) = corpus::validate_optional_corpora(&repository_root)? else {
        return Err("P36 requires initialized pinned corpus submodules".into());
    };
    let head = git_value(&netlib, ["rev-parse", "HEAD"])?;
    if head.trim() != EXPECTED_NETLIB_COMMIT {
        return Err(format!(
            "Netlib checkout is at {}, expected {EXPECTED_NETLIB_COMMIT}",
            head.trim()
        )
        .into());
    }

    let source = netlib.join("mps_files");
    let actual = regular_mps_names(&source)?;
    if actual != expected {
        return Err(format!(
            "Netlib inventory drift: expected {}, found {}",
            expected.len(),
            actual.len()
        )
        .into());
    }

    let output_root = repository_root.join("target/roml-corpora/p36-mps-writeback");
    fs::create_dir_all(&output_root)?;
    println!(
        "{{\"schema\":\"p36-mps-writeback-qualification-v1\",\"manifest_count\":{},\"commit\":{}}}",
        expected.len(),
        json(EXPECTED_NETLIB_COMMIT)
    );

    let mut failures = Vec::new();
    for name in expected {
        if only.as_ref().is_some_and(|selected| selected != &name) {
            continue;
        }
        let result = qualify_one(&source.join(&name), &output_root.join(&name), &name);
        if result.status != "PASS" {
            failures.push(result.name.clone());
        }
        println!(
            "{{\"path\":{},\"status\":{},\"detail\":{},\"output_bytes\":{}}}",
            json(&result.name),
            json(result.status),
            json(&result.detail),
            result.output_bytes
        );
    }
    if !failures.is_empty() {
        return Err(format!("P36 qualification failed for {} models", failures.len()).into());
    }
    Ok(())
}

fn qualify_one(source: &Path, destination: &Path, name: &str) -> FileResult {
    match qualify_one_inner(source, destination, name) {
        Ok(output_bytes) => FileResult {
            name: name.to_owned(),
            status: "PASS",
            detail: "writer+deterministic+roml+highs-structure".to_owned(),
            output_bytes,
        },
        Err(error) => FileResult {
            name: name.to_owned(),
            status: "FAIL",
            detail: error,
            output_bytes: 0,
        },
    }
}

fn qualify_one_inner(source: &Path, destination: &Path, name: &str) -> Result<usize, String> {
    let original = MpsReader::new()
        .read_path(source)
        .map_err(|error| format!("P35 import: {error}"))?;
    let expected =
        normalize(&original.model, None).map_err(|error| format!("normalize source: {error}"))?;

    let writer = MpsWriter::new();
    let mut first = Vec::new();
    let report = writer
        .write(&original.model, &mut first)
        .map_err(|error| format!("P36 write: {error}"))?;
    let mut second = Vec::new();
    writer
        .write(&original.model, &mut second)
        .map_err(|error| format!("P36 deterministic write: {error}"))?;
    if first != second {
        return Err("repeated writes differ byte-for-byte".to_owned());
    }
    fs::write(destination, &first).map_err(|error| format!("qualification artifact: {error}"))?;

    let round_trip = MpsReader::new()
        .read_path(destination)
        .map_err(|error| format!("P35 re-import: {error}"))?;
    let actual = normalize(&round_trip.model, Some(&report.name_map))
        .map_err(|error| format!("normalize output: {error}"))?;
    assert_semantic_equivalent(&expected, &actual)?;

    let observation = observe_mps_differential(destination);
    let (native, imported) = match (observation.highs, observation.roml) {
        (Ok(native), Ok(imported)) => (native, imported),
        (Err(error), _) => return Err(format!("native HiGHS read: {error}")),
        (_, Err(error)) => return Err(format!("ROML differential import: {error}")),
    };
    let structure = compare_mps_structure(
        &native,
        &imported,
        P36_STRUCTURAL_ABS_TOLERANCE,
        P36_STRUCTURAL_REL_TOLERANCE,
    );
    if !structure.equivalent {
        return Err(format!(
            "native structure mismatch: {:?}",
            structure.differences
        ));
    }

    if SOLVE_SUBSET.contains(&name) {
        let solve = observe_mps_solve_differential(destination);
        let (highs, roml) = match (solve.highs, solve.roml) {
            (Ok(highs), Ok(roml)) => (highs, roml),
            (Err(error), _) => return Err(format!("native solve: {error}")),
            (_, Err(error)) => return Err(format!("ROML solve: {error}")),
        };
        let comparison = compare_mps_solve(&highs, &roml, 1e-7, 1e-8);
        require_solve_equivalent(comparison)?;
    }
    Ok(first.len())
}

fn normalize(
    model: &Model,
    emitted_names: Option<&roml::io::mps::MpsWriteNameMap>,
) -> Result<SemanticModel, String> {
    let snapshot = model.take_snapshot().map_err(|error| error.to_string())?;
    normalize_snapshot(model, &snapshot, emitted_names)
}

fn normalize_snapshot(
    model: &Model,
    snapshot: &ModelSnapshot,
    emitted_names: Option<&roml::io::mps::MpsWriteNameMap>,
) -> Result<SemanticModel, String> {
    let mut active_variables = snapshot
        .variables
        .iter()
        .filter(|variable| variable.active)
        .collect::<Vec<_>>();
    let mut active_rows = snapshot
        .constraints
        .iter()
        .filter(|row| row.active)
        .collect::<Vec<_>>();
    if let Some(names) = emitted_names {
        active_variables.sort_by_key(|entry| {
            model
                .variable_name(entry.id)
                .ok()
                .flatten()
                .and_then(|name| {
                    names
                        .variables
                        .iter()
                        .find(|assignment| assignment.emitted_name == name)
                })
                .map_or(usize::MAX, |assignment| assignment.ordinal)
        });
        active_rows.sort_by_key(|entry| {
            model
                .constraint_name(entry.id)
                .ok()
                .flatten()
                .and_then(|name| {
                    names
                        .rows
                        .iter()
                        .find(|assignment| assignment.emitted_name == name)
                })
                .map_or(usize::MAX, |assignment| assignment.ordinal)
        });
    }
    let active_variable_indexes = active_variables
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.id, index))
        .collect::<BTreeMap<_, _>>();
    let active_row_indexes = active_rows
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.id, index))
        .collect::<BTreeMap<_, _>>();

    let variables = active_variables
        .iter()
        .map(|entry| {
            let bounds = entry.fixing.as_ref().map_or(entry.bounds, |fixing| {
                roml::Bounds::new(fixing.value, fixing.value)
            });
            VariableSemantics {
                lower: bounds.lower,
                upper: bounds.upper,
                var_type: entry.var_type,
            }
        })
        .collect::<Vec<_>>();
    let rows = active_rows
        .iter()
        .map(|entry| entry.bounds)
        .collect::<Vec<_>>();

    let active_objective = snapshot
        .objectives
        .iter()
        .find(|objective| objective.active);
    let mut matrix = Vec::new();
    let mut objective_coefficients = Vec::new();
    for cell in &snapshot.cells {
        if cell.evaluated_value == 0.0 {
            continue;
        }
        let Some(&variable) = active_variable_indexes.get(&cell.cell_key.1) else {
            continue;
        };
        match cell.cell_key.0 {
            CoefficientTarget::Constraint(row) => {
                if let Some(&row) = active_row_indexes.get(&row) {
                    matrix.push((row, variable, cell.evaluated_value));
                }
            }
            CoefficientTarget::Objective(objective)
                if active_objective.is_some_and(|entry| entry.id == objective) =>
            {
                objective_coefficients.push((variable, cell.evaluated_value));
            }
            _ => {}
        }
    }
    matrix.sort_by_key(|(row, variable, _)| (*row, *variable));
    objective_coefficients.sort_by_key(|(variable, _)| *variable);
    let objective = active_objective.map_or(
        ObjectiveSemantics {
            sense: Sense::Minimize,
            offset: 0.0,
            coefficients: Vec::new(),
        },
        |objective| ObjectiveSemantics {
            sense: objective.sense,
            offset: objective.constant,
            coefficients: objective_coefficients,
        },
    );
    Ok(SemanticModel {
        variables,
        rows,
        matrix,
        objective,
    })
}

fn assert_semantic_equivalent(
    expected: &SemanticModel,
    actual: &SemanticModel,
) -> Result<(), String> {
    if expected.variables.len() != actual.variables.len() {
        return Err("variable counts differ".to_owned());
    }
    for (index, (expected, actual)) in expected.variables.iter().zip(&actual.variables).enumerate()
    {
        compare_float(
            &format!("variable {index} lower"),
            expected.lower,
            actual.lower,
        )?;
        compare_float(
            &format!("variable {index} upper"),
            expected.upper,
            actual.upper,
        )?;
        if expected.var_type != actual.var_type {
            return Err(format!("variable {index} type differs"));
        }
    }
    if expected.rows.len() != actual.rows.len() {
        return Err("row counts differ".to_owned());
    }
    for (index, (expected, actual)) in expected.rows.iter().zip(&actual.rows).enumerate() {
        compare_float(&format!("row {index} lower"), expected.lower, actual.lower)?;
        compare_float(&format!("row {index} upper"), expected.upper, actual.upper)?;
    }
    if expected.matrix.len() != actual.matrix.len() {
        return Err("matrix nonzero counts differ".to_owned());
    }
    for (index, (expected, actual)) in expected.matrix.iter().zip(&actual.matrix).enumerate() {
        if expected.0 != actual.0 || expected.1 != actual.1 {
            return Err(format!("matrix coordinate differs at {index}"));
        }
        compare_float(&format!("matrix {index}"), expected.2, actual.2)?;
    }
    if expected.objective.sense != actual.objective.sense
        || expected.objective.coefficients.len() != actual.objective.coefficients.len()
    {
        return Err("objective structure differs".to_owned());
    }
    compare_float(
        "objective offset",
        expected.objective.offset,
        actual.objective.offset,
    )?;
    for (index, (variable, expected)) in expected.objective.coefficients.iter().enumerate() {
        let Some((actual_variable, actual)) = actual.objective.coefficients.get(index) else {
            return Err(format!("missing objective coefficient {variable}"));
        };
        if variable != actual_variable {
            return Err(format!("objective variable differs at {index}"));
        }
        compare_float(&format!("objective {variable}"), *expected, *actual)?;
    }
    Ok(())
}

fn compare_float(field: &str, expected: f64, actual: f64) -> Result<(), String> {
    if expected.is_infinite() || actual.is_infinite() {
        return (expected == actual)
            .then_some(())
            .ok_or_else(|| format!("{field}: {expected:?} != {actual:?}"));
    }
    let tolerance = 1e-10 + 1e-10 * expected.abs().max(actual.abs());
    ((expected - actual).abs() <= tolerance)
        .then_some(())
        .ok_or_else(|| format!("{field}: {expected:?} != {actual:?} (tol {tolerance:?})"))
}

fn require_solve_equivalent(comparison: MpsSolveComparison) -> Result<(), String> {
    if comparison.equivalent {
        Ok(())
    } else {
        Err(format!("solve mismatch: {:?}", comparison.differences))
    }
}

fn expected_manifest() -> BTreeSet<String> {
    let manifest = include_str!("../../.planning/phases/36-mps-writeback/36-NETLIB-MANIFEST.md");
    let mut in_names = false;
    let mut names = BTreeSet::new();
    for line in manifest.lines().map(str::trim) {
        if line == "```text" {
            in_names = true;
        } else if in_names && line == "```" {
            break;
        } else if in_names && !line.is_empty() {
            names.insert(line.to_owned());
        }
    }
    names
}

fn regular_mps_names(directory: &Path) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.metadata()?.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mps"))
        {
            names.insert(entry.file_name().to_string_lossy().into_owned());
        }
    }
    Ok(names)
}

fn git_value<const N: usize>(
    checkout: &Path,
    arguments: [&str; N],
) -> Result<String, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(arguments)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git {:?} failed in {}: {}",
            arguments,
            checkout.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                let _ = write!(escaped, "\\u{:04x}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}
