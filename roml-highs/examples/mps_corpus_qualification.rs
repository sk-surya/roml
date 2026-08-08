//! Optional deterministic Netlib/Chinneck MPS qualification runner.
//!
//! The runner is intentionally outside the library API. It validates the
//! reviewed corpus pins, discovers already-materialized MPS files, and emits
//! one JSON object per file. Chinneck archives are never extracted by this
//! command: callers must use the separately reviewed safe materializer and
//! place its atomically completed output below `target/roml-corpora`.

#[allow(dead_code)]
#[path = "../../tests/support/corpus.rs"]
mod corpus;

use std::{
    env,
    error::Error,
    fmt::Write as _,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    process::Command,
};

use corpus::{
    materialize_chinneck_archive_stream, ArchiveEntryKind, CorpusCacheKey,
    ExpectedArchiveInventory, MaterializationError,
};
use roml::{
    advanced::{BoundSide, ConflictOrigin, InfeasibilityPlan},
    io::mps::{MpsBoundSide, MpsReader},
    solver::facade::SolverSession,
};
use roml_highs::{
    compare_mps_solve, compare_mps_structure, observe_mps_differential,
    observe_mps_solve_differential, HighsSession, MPS_STRUCTURAL_ABS_TOLERANCE,
    MPS_STRUCTURAL_REL_TOLERANCE,
};
use sevenz_rust::{Password, SevenZReader};
use sha2::{Digest, Sha256};

const CHINNECK_COMMIT: &str = "97a936498e5240d44adaf7dcfe84877fa34ce301";
const CHINNECK_REPOSITORY: &str = "https://github.com/sk-surya/infeasiblelps";
const NETLIB_COMMIT: &str = "56257eea85b433ce6aa67d26156b36385318fd6f";
const NETLIB_REPOSITORY: &str = "https://github.com/sk-surya/lp-data-netlib";
const CHINNECK_ARCHIVES: [&str; 2] = ["INFfromNetlibLPs.7z", "INFfromClassificationData.7z"];
const CHINNECK_SELECTED_MODELS: [&str; 3] = [
    "IC-balancescale-LB.mps",
    "IC-balancescale.mps",
    "IC-breast1-LB.mps",
];

#[derive(Clone, Copy)]
struct CorpusPin {
    id: &'static str,
    repository: &'static str,
    commit: &'static str,
    checkout: &'static str,
}

const PINS: [CorpusPin; 2] = [
    CorpusPin {
        id: "chinneck-infeasible-lps",
        repository: CHINNECK_REPOSITORY,
        commit: CHINNECK_COMMIT,
        checkout: "testdata/corpora/infeasible-lps",
    },
    CorpusPin {
        id: "netlib-lp-data",
        repository: NETLIB_REPOSITORY,
        commit: NETLIB_COMMIT,
        checkout: "testdata/corpora/netlib",
    },
];

fn main() -> Result<(), Box<dyn Error>> {
    let mut repository_root = env::current_dir()?;
    let mut root_supplied = false;
    let mut max_files = None;
    let mut solve = true;
    let mut arguments = env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_string_lossy().as_ref() {
            "--max-files" => {
                let value = arguments
                    .next()
                    .ok_or("--max-files requires a positive integer")?;
                max_files = Some(value.to_string_lossy().parse::<usize>()?);
            }
            "--no-solve" => solve = false,
            value if value.starts_with('-') => return Err(format!("unknown option {value}").into()),
            value if !root_supplied => {
                repository_root = PathBuf::from(value);
                root_supplied = true;
            }
            _ => return Err("only one repository root may be supplied".into()),
        }
    }
    println!(
        "{{\"schema\":\"p35-mps-qualification-v1\",\"root\":{}}}",
        json(&repository_root.display().to_string())
    );

    for pin in PINS {
        let checkout = repository_root.join(pin.checkout);
        if !checkout.is_dir() || !is_initialized_submodule(&checkout) {
            emit_skip(
                pin,
                &checkout,
                "optional corpus checkout is absent or uninitialized",
            )?;
            continue;
        }
        validate_pin(pin, &checkout)?;

        let source_root = if pin.id == "netlib-lp-data" {
            checkout.join("mps_files")
        } else {
            materialize_selected_chinneck_archives(&checkout, &repository_root)?;
            repository_root.join("target/roml-corpora/chinneck")
        };
        let mut files = discover_mps_files(&source_root)?;
        if pin.id == "chinneck-infeasible-lps" {
            files.retain(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| CHINNECK_SELECTED_MODELS.contains(&name))
            });
        }
        if files.is_empty() {
            let reason = if pin.id == "netlib-lp-data" {
                "pinned checkout contains no mps_files/*.mps entries"
            } else {
                "no atomically materialized Chinneck MPS files; use the safe archive materializer"
            };
            emit_skip(pin, &source_root, reason)?;
            continue;
        }
        for (index, path) in files.into_iter().enumerate() {
            if max_files.is_some_and(|limit| index >= limit) {
                break;
            }
            emit_file(pin, &repository_root, &source_root, &path, solve)?;
        }
    }
    Ok(())
}

fn materialize_selected_chinneck_archives(
    checkout: &Path,
    repository_root: &Path,
) -> Result<(), Box<dyn Error>> {
    let cache_root = repository_root.join("target/roml-corpora/chinneck");
    for archive_name in CHINNECK_ARCHIVES {
        let archive_path = checkout.join(archive_name);
        if !archive_path.is_file() {
            return Err(format!(
                "selected Chinneck archive is absent: {}",
                archive_path.display()
            )
            .into());
        }
        let archive_hash = sha256_file(&archive_path)?;
        let cache_key = CorpusCacheKey::new(CHINNECK_COMMIT, archive_name, &archive_hash)?;
        let expected = read_7z_inventory(&archive_path)?;
        materialize_chinneck_archive_stream(&cache_root, &cache_key, &expected, |emit| {
            let mut archive = SevenZReader::open(&archive_path, Password::empty())
                .map_err(|error| materializer_error(&archive_path, error))?;
            archive
                .for_each_entries(|entry, reader| {
                    let kind = seven_z_entry_kind(entry)
                        .map_err(|error| seven_z_archive_error(&archive_path, error))?;
                    emit(entry.name(), kind, reader)
                        .map_err(|error| seven_z_archive_error(&archive_path, error))?;
                    Ok(true)
                })
                .map_err(|error| materializer_error(&archive_path, error))?;
            Ok(())
        })?;
        eprintln!(
            "materialized selected Chinneck archive {} into {}",
            archive_name,
            cache_key.directory_name()
        );
    }
    Ok(())
}

fn read_7z_inventory(path: &Path) -> Result<ExpectedArchiveInventory, Box<dyn Error>> {
    let mut archive = SevenZReader::open(path, Password::empty())?;
    let mut files = Vec::new();
    archive.for_each_entries(|entry, reader| {
        let kind = seven_z_entry_kind(entry).map_err(sevenz_rust::Error::other)?;
        if kind == ArchiveEntryKind::RegularFile {
            let mut limited = reader.take(entry.size());
            let mut digest = Sha256::new();
            let copied = io::copy(&mut limited, &mut digest).map_err(sevenz_rust::Error::io)?;
            if copied != entry.size() {
                return Err(sevenz_rust::Error::other(format!(
                    "archive entry {} ended at {} bytes, expected {}",
                    entry.name(),
                    copied,
                    entry.size()
                )));
            }
            files.push((entry.name().to_owned(), format!("{:x}", digest.finalize())));
        }
        Ok(true)
    })?;
    Ok(ExpectedArchiveInventory::new(files)?)
}

fn seven_z_entry_kind(entry: &sevenz_rust::SevenZArchiveEntry) -> Result<ArchiveEntryKind, String> {
    if entry.is_anti_item() {
        return Err(format!("anti-item entry {} is not accepted", entry.name()));
    }
    const FILE_ATTRIBUTE_DEVICE: u32 = 0x40;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    if entry.has_windows_attributes
        && (entry.windows_attributes() & (FILE_ATTRIBUTE_DEVICE | FILE_ATTRIBUTE_REPARSE_POINT))
            != 0
    {
        return Err(format!(
            "special or reparse entry {} has unsupported Windows attributes 0x{:x}",
            entry.name(),
            entry.windows_attributes()
        ));
    }
    if entry.is_directory() {
        if entry.has_stream() {
            return Err(format!("directory entry {} has a payload", entry.name()));
        }
        return Ok(ArchiveEntryKind::Directory);
    }
    if entry.has_stream() {
        return Ok(ArchiveEntryKind::RegularFile);
    }
    Err(format!(
        "non-directory archive entry {} has no regular payload",
        entry.name()
    ))
}

fn seven_z_archive_error(path: &Path, error: impl std::fmt::Display) -> sevenz_rust::Error {
    sevenz_rust::Error::other(format!("cannot materialize {}: {error}", path.display()))
}

fn materializer_error(path: &Path, error: impl std::fmt::Display) -> MaterializationError {
    MaterializationError::Io {
        operation: "read Chinneck 7z archive",
        path: path.to_owned(),
        source: io::Error::other(error.to_string()),
    }
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    io::copy(&mut file, &mut digest)?;
    Ok(format!("{:x}", digest.finalize()))
}

fn validate_pin(pin: CorpusPin, checkout: &Path) -> Result<(), Box<dyn Error>> {
    let origin = git_value(checkout, &["remote", "get-url", "origin"])?;
    if origin.trim() != pin.repository {
        return Err(format!(
            "{} origin is {:?}, expected {:?}",
            pin.id,
            origin.trim(),
            pin.repository
        )
        .into());
    }
    let head = git_value(checkout, &["rev-parse", "HEAD"])?;
    if head.trim() != pin.commit {
        return Err(format!(
            "{} HEAD is {:?}, expected {:?}",
            pin.id,
            head.trim(),
            pin.commit
        )
        .into());
    }
    let status = git_value(
        checkout,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    if !status.trim().is_empty() {
        return Err(format!("{} checkout is dirty: {:?}", pin.id, status.trim()).into());
    }
    Ok(())
}

fn is_initialized_submodule(checkout: &Path) -> bool {
    let metadata = checkout.join(".git");
    if !metadata.is_file() && !metadata.is_dir() {
        return false;
    }
    let output = match Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(["rev-parse", "--show-toplevel"])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return false,
    };
    let reported = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    match (fs::canonicalize(checkout), fs::canonicalize(reported)) {
        (Ok(checkout), Ok(reported)) => checkout == reported,
        _ => false,
    }
}

fn git_value(checkout: &Path, args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git {:?} failed in {}: {}",
            args,
            checkout.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn discover_mps_files(root: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    discover_mps_files_inner(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn discover_mps_files_inner(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            discover_mps_files_inner(&path, files)?;
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mps"))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn emit_file(
    pin: CorpusPin,
    repository_root: &Path,
    source_root: &Path,
    path: &Path,
    solve: bool,
) -> Result<(), Box<dyn Error>> {
    let _relative = path.strip_prefix(source_root)?.display().to_string();
    let bytes = fs::metadata(path)?.len();
    let observation = observe_mps_differential(path);
    let roml_status = match &observation.roml {
        Ok(summary) => format!(
            "ok:{}:{}:{}:{}",
            summary.rows, summary.columns, summary.nonzeros, summary.objective_offset
        ),
        Err(error) => format!("error:{}", error),
    };
    let highs_status = match &observation.highs {
        Ok(summary) => format!(
            "ok:{}:{}:{}:{}",
            summary.rows, summary.columns, summary.nonzeros, summary.objective_offset
        ),
        Err(error) => format!("error:{}", error),
    };
    let structural_comparison = match (&observation.roml, &observation.highs) {
        (Ok(roml), Ok(highs)) => Some(compare_mps_structure(
            highs,
            roml,
            MPS_STRUCTURAL_ABS_TOLERANCE,
            MPS_STRUCTURAL_REL_TOLERANCE,
        )),
        _ => None,
    };
    let structural = match &structural_comparison {
        Some(comparison) if comparison.equivalent => "equivalent",
        Some(_) => "unresolved_discrepancy",
        None => "not_comparable",
    };
    let disposition = match (&observation.roml, &observation.highs, structural) {
        (Ok(_), Ok(_), "equivalent") => "equivalent",
        (Err(_), Ok(_), _) => "intentional_roml_rejection",
        (Ok(_), Ok(_), "unresolved_discrepancy") => "unresolved_discrepancy",
        _ => "both_or_native_rejected",
    };
    let (native_solve_status, roml_solve_status, solve_comparison) = if !solve {
        (
            "not_requested".to_owned(),
            "not_requested".to_owned(),
            "not_requested".to_owned(),
        )
    } else {
        let solve_observation = observe_mps_solve_differential(path);
        let native_status = match &solve_observation.highs {
            Ok(observation) => format!("{:?}", observation.status),
            Err(error) => format!("error:{error}"),
        };
        let roml_status = match &solve_observation.roml {
            Ok(observation) => format!("{:?}", observation.status),
            Err(error) => format!("error:{error}"),
        };
        let comparison = match (&solve_observation.highs, &solve_observation.roml) {
            (Ok(highs), Ok(roml)) => {
                let comparison = compare_mps_solve(
                    highs,
                    roml,
                    MPS_STRUCTURAL_ABS_TOLERANCE,
                    MPS_STRUCTURAL_REL_TOLERANCE,
                );
                if comparison.equivalent {
                    "equivalent".to_owned()
                } else {
                    "unresolved_discrepancy".to_owned()
                }
            }
            _ => "not_comparable".to_owned(),
        };
        (native_status, roml_status, comparison)
    };
    let (iis_status, iis_error) = if pin.id == "chinneck-infeasible-lps" {
        match qualify_chinneck_iis(path) {
            Ok(member_count) => (format!("irreducible_exact_source:{member_count}"), None),
            Err(error) => ("failed".to_owned(), Some(error.to_string())),
        }
    } else {
        ("not_applicable".to_owned(), None)
    };
    let absolute = path.strip_prefix(repository_root).unwrap_or(path);
    let mut line = String::new();
    write!(
        line,
        "{{\"corpus\":{},\"commit\":{},\"path\":{},\"file_bytes\":{},\"roml_parse_status\":{},\"native_highs_read_status\":{},\"structural_comparison_status\":{},\"structural_difference_count\":{},\"differential_disposition\":{},\"native_solve_status\":{},\"roml_solve_status\":{},\"solve_comparison_status\":{},\"iis_qualification_status\":{},\"iis_error\":{}}}",
        json(pin.id),
        json(pin.commit),
        json(&absolute.display().to_string()),
        bytes,
        json(&roml_status),
        json(&highs_status),
        json(structural),
        structural_comparison
            .as_ref()
            .map_or(0, |comparison| comparison.differences.len()),
        json(disposition),
        json(&native_solve_status),
        json(&roml_solve_status),
        json(&solve_comparison),
        json(&iis_status),
        iis_error.as_deref().map_or_else(|| "null".to_owned(), json)
    )?;
    println!("{line}");
    if structural == "unresolved_discrepancy" {
        return Err(format!(
            "accepted MPS input {} has an unresolved ROML/HiGHS discrepancy",
            path.display()
        )
        .into());
    }
    if solve_comparison == "unresolved_discrepancy" {
        return Err(format!(
            "accepted MPS input {} has an unresolved native/ROML solve discrepancy",
            path.display()
        )
        .into());
    }
    if iis_status == "failed" {
        return Err(format!(
            "Chinneck IIS qualification failed for {}: {}",
            path.display(),
            iis_error.unwrap_or_else(|| "unknown error".to_owned())
        )
        .into());
    }
    Ok(())
}

fn qualify_chinneck_iis(path: &Path) -> Result<usize, Box<dyn Error>> {
    let import = MpsReader::new().read_path(path)?;
    let mut session = SolverSession::new(HighsSession::try_new()?);
    let report = session.analyze_infeasibility(&import.model, &InfeasibilityPlan::portable_lp())?;
    if report.outcome != roml::InfeasibilityOutcome::Conflict
        || report.guarantee != roml::ConflictGuarantee::Irreducible
    {
        return Err(format!(
            "expected an irreducible infeasible conflict, got {:?}/{:?}",
            report.outcome, report.guarantee
        )
        .into());
    }
    for member in &report.members {
        match &member.declaration.origin {
            ConflictOrigin::ConstraintSide { constraint, .. } => {
                let name = import
                    .model
                    .constraint_name(*constraint)?
                    .ok_or("reported Chinneck constraint has no name")?;
                if import.source_map.row_span(name).is_none() {
                    return Err(format!("IIS constraint {name:?} has no MPS row origin").into());
                }
            }
            ConflictOrigin::VariableBound { variable, side } => {
                let name = import
                    .model
                    .variable_name(*variable)?
                    .ok_or("reported Chinneck variable has no name")?;
                let mps_side = match side {
                    BoundSide::Lower => MpsBoundSide::Lower,
                    BoundSide::Upper => MpsBoundSide::Upper,
                };
                let matches = import
                    .source_map
                    .variable_bound_origins()
                    .iter()
                    .filter(|origin| origin.variable == name && origin.side == mps_side)
                    .count();
                if matches != 1 {
                    return Err(format!(
                        "IIS bound ({name}, {mps_side:?}) resolved to {matches} MPS origins"
                    )
                    .into());
                }
            }
            origin => {
                return Err(
                    format!("unexpected non-original origin in Chinneck IIS: {origin:?}").into(),
                )
            }
        }
    }
    Ok(report.members.len())
}

fn emit_skip(pin: CorpusPin, path: &Path, reason: &str) -> Result<(), Box<dyn Error>> {
    println!(
        "{{\"corpus\":{},\"commit\":{},\"path\":{},\"differential_disposition\":\"skipped\",\"skip_or_failure_reason\":{}}}",
        json(pin.id),
        json(pin.commit),
        json(&path.display().to_string()),
        json(reason)
    );
    Ok(())
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
