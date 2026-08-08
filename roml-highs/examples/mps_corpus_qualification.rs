//! Optional deterministic Netlib/Chinneck MPS qualification runner.
//!
//! The runner is intentionally outside the library API. It validates the
//! reviewed corpus pins, discovers already-materialized MPS files, and emits
//! one JSON object per file. Chinneck archives are never extracted by this
//! command: callers must use the separately reviewed safe materializer and
//! place its atomically completed output below `target/roml-corpora`.

use std::{
    env,
    error::Error,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use roml::io::mps::MpsReader;
use roml_highs::{observe_mps_differential, Highs};

const CHINNECK_COMMIT: &str = "97a936498e5240d44adaf7dcfe84877fa34ce301";
const CHINNECK_REPOSITORY: &str = "https://github.com/sk-surya/infeasiblelps";
const NETLIB_COMMIT: &str = "56257eea85b433ce6aa67d26156b36385318fd6f";
const NETLIB_REPOSITORY: &str = "https://github.com/sk-surya/lp-data-netlib";

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
        if !checkout.is_dir() {
            emit_skip(pin, &checkout, "optional corpus checkout is absent")?;
            continue;
        }
        validate_pin(pin, &checkout)?;

        let source_root = if pin.id == "netlib-lp-data" {
            checkout.join("mps_files")
        } else {
            repository_root.join("target/roml-corpora/chinneck")
        };
        let files = discover_mps_files(&source_root)?;
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
    let structural = match (&observation.roml, &observation.highs) {
        (Ok(roml), Ok(highs))
            if roml.columns == highs.columns
                && roml.rows == highs.rows
                && roml.nonzeros == highs.nonzeros
                && roml.objective_offset == highs.objective_offset =>
        {
            "equivalent"
        }
        (Ok(_), Ok(_)) => "unresolved_discrepancy",
        _ => "not_comparable",
    };
    let disposition = match (&observation.roml, &observation.highs, structural) {
        (Ok(_), Ok(_), "equivalent") => "equivalent",
        (Err(_), Ok(_), _) => "intentional_roml_rejection",
        (Ok(_), Ok(_), "unresolved_discrepancy") => "unresolved_discrepancy",
        _ => "both_or_native_rejected",
    };
    let solve_status = if !solve {
        "not_requested".to_owned()
    } else if let Ok(mut import) = MpsReader::new().read_path(path) {
        match Highs::new()?.solve(&mut import.model) {
            Ok(solution) => format!("{:?}", solution.status()),
            Err(error) => format!("error:{error}"),
        }
    } else {
        "not_attempted".to_owned()
    };
    let absolute = path.strip_prefix(repository_root).unwrap_or(path);
    let mut line = String::new();
    write!(
        line,
        "{{\"corpus\":{},\"commit\":{},\"path\":{},\"file_bytes\":{},\"roml_parse_status\":{},\"native_highs_read_status\":{},\"structural_comparison_status\":{},\"differential_disposition\":{},\"solve_status\":{}}}",
        json(pin.id),
        json(pin.commit),
        json(&absolute.display().to_string()),
        bytes,
        json(&roml_status),
        json(&highs_status),
        json(structural),
        json(disposition),
        json(&solve_status)
    )?;
    println!("{line}");
    if structural == "unresolved_discrepancy" {
        return Err(format!(
            "accepted MPS input {} has an unresolved ROML/HiGHS discrepancy",
            path.display()
        )
        .into());
    }
    Ok(())
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
