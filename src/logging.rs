//! Simple logging initialization helpers.
//!
//! The library uses the `log` crate for logging; `log4rs` is offered as an
//! optional backend that can be configured at runtime via a YAML file.  Call
//! `init_logging()` as early as possible in your application to configure the
//! logger.  If the configuration file cannot be found or parsed the error is
//! returned to the caller so it can decide how to proceed (often tests will
//! simply ignore the result).

use std::path::PathBuf;

/// Initialise the global logger using `log4rs`.
///
/// The configuration file path is taken from the `LOG4RS_CONFIG` environment
/// variable; if that variable is not set we fall back to `log4rs.yaml` in the
/// current working directory.  The returned `log4rs::Handle` can be stored if
/// the caller intends to modify the configuration later (for example to reload
/// at runtime), but it is perfectly valid to ignore it.
///
/// # Errors
///
/// Returns any error produced by `log4rs::init_file`.
// locate the nearest `log4rs.yaml` by walking parent directories.
// this helper is reused by `init_logging` and some unit tests.
fn search_upwards(start: &std::path::Path) -> Option<PathBuf> {
    let mut cur = start.to_path_buf();
    loop {
        let candidate = cur.join("log4rs.yaml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

/// Locate the Cargo workspace root by walking parent directories looking for
/// a `Cargo.toml` that contains a `[workspace]` section.  This is a cheap
/// heuristic that matches the behaviour of `cargo` itself.
fn find_workspace_root(start: &std::path::Path) -> Option<PathBuf> {
    let mut cur = start.to_path_buf();
    loop {
        let candidate = cur.join("Cargo.toml");
        if candidate.exists() {
            if let Ok(text) = std::fs::read_to_string(&candidate) {
                if text.contains("[workspace]") {
                    return Some(cur.clone());
                }
            }
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

/// Try to read `ROML_LOG_FILE` from an optional `config.yaml` located at the
/// workspace root.  The file is assumed to contain a top‑level `env` mapping
/// like:
///
/// ```yaml
/// env:
///   ROML_LOG_FILE: "/some/path"
/// ```
///
/// Returns the value if present and valid UTF‑8.
fn read_logfile_from_config() -> Option<String> {
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(root) = find_workspace_root(&cwd) {
            let cfg = root.join("config.yaml");
            if !cfg.exists() {
                // one level above
                let parent = root.parent()?;
                let cfg = parent.join("config.yaml");
            }
            if cfg.exists() {
                if let Ok(text) = std::fs::read_to_string(&cfg) {
                    if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&text) {
                        if let Some(env) = val.get("env") {
                            if let Some(lf) = env.get("ROML_LOG_FILE") {
                                if let Some(s) = lf.as_str() {
                                    // if the string is just a filename, resolve it relative to the config file directory
                                    let path = if s.contains(std::path::MAIN_SEPARATOR) {
                                        std::path::PathBuf::from(s)
                                    } else {
                                        cfg.parent().unwrap_or(&root).join(s)
                                    };
                                    return Some(s.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Ensure the `ROML_LOG_FILE` environment variable is set to a sensible default
/// before `log4rs` reads the configuration.  Priority is
/// 1. existing environment variable
/// 2. value from `config.yaml` (workspace root)
/// 3. `<workspace>/roml.log` if we can discover a workspace
/// 4. leave it unset
fn ensure_logfile_env() {
    if std::env::var_os("ROML_LOG_FILE").is_none() {
        if let Some(from_cfg) = read_logfile_from_config() {
            std::env::set_var("ROML_LOG_FILE", from_cfg);
            // return;
        }
    }
    if std::env::var_os("ROML_LOG_FILE").is_none() {
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(root) = find_workspace_root(&cwd) {
                let path = root.join("roml.log");
                std::env::set_var("ROML_LOG_FILE", path);
            }
        }
    }
    println!("ROML_LOG_FILE set to {:?}", std::env::var("ROML_LOG_FILE").unwrap());
}

pub fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    // set up environment variables that the configuration may depend on
    ensure_logfile_env();

    // Determine configuration path.  If the caller has set `LOG4RS_CONFIG`,
    // use that verbatim.  Otherwise perform a search upward from the current
    // working directory for a file named `log4rs.yaml`.  The workspace root is
    // the most common location for the sample config, so this allows tests run
    // from sub-crates to still find it.
    let config_path: PathBuf = if let Ok(env) = std::env::var("LOG4RS_CONFIG") {
        PathBuf::from(env)
    } else {
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(found) = search_upwards(&cwd) {
                found
            } else {
                PathBuf::from("log4rs.yaml")
            }
        } else {
            PathBuf::from("log4rs.yaml")
        }
    };

    // `init_file` returns an `anyhow::Result<()>`; convert it to a boxed error
    log4rs::init_file(config_path, Default::default())?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn init_with_explicit_path() {
        // ensure config search does not interfere with this case
        std::env::remove_var("LOG4RS_CONFIG");
        let dir = tempdir().unwrap();
        let path = dir.path().join("cfg.yaml");
        let mut f = File::create(&path).unwrap();
        // minimal configuration: root logger at error level to a console appender
        writeln!(
            f,
            "appenders:\n  stdout:\n    kind: console\nroot:\n  level: error\n  appenders:\n    - stdout\n"
        )
        .unwrap();

        std::env::set_var("LOG4RS_CONFIG", &path);
        init_logging().expect("should init with valid file");
    }

    #[test]
    fn missing_config_returns_error() {
        std::env::remove_var("LOG4RS_CONFIG");
        // remove any found config by searching up and temporarily renaming
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(found) = search_upwards(&cwd) {
                let backup = found.with_extension("bak");
                std::fs::rename(&found, &backup).unwrap();
                let res = init_logging();
                assert!(res.is_err());
                std::fs::rename(&backup, &found).unwrap();
                return;
            }
        }
        // fallback if no file existed anywhere
        let res = init_logging();
        assert!(res.is_err());
    }

    #[test]
    fn workspace_root_sets_env() {
        // create a temporary directory hierarchy mimicking a workspace
        let root = tempdir().unwrap();
        let mut workspace_toml = File::create(root.path().join("Cargo.toml")).unwrap();
        write!(workspace_toml, "[workspace]
").unwrap();
        let child = root.path().join("child");
        std::fs::create_dir(&child).unwrap();

        // start the search from within the child directory
        let found = find_workspace_root(&child).expect("should find workspace root");
        assert_eq!(found, root.path());

        // ensure env var not set yet
        std::env::remove_var("ROML_LOG_FILE");
        std::env::set_current_dir(&child).unwrap();
        ensure_logfile_env();
        let val = std::env::var("ROML_LOG_FILE").unwrap();
        assert!(val.ends_with("roml.log"));
        assert!(val.starts_with(root.path().to_str().unwrap()));
    }

    #[test]
    fn config_file_precedence() {
        // workspace -> config.yaml containing env key
        let root = tempdir().unwrap();
        let mut workspace_toml = File::create(root.path().join("Cargo.toml")).unwrap();
        write!(workspace_toml, "[workspace]
").unwrap();
        let cfg_path = root.path().join("config.yaml");
        let mut cfg = File::create(&cfg_path).unwrap();
        writeln!(
            cfg,
            "env:\n  ROML_LOG_FILE: \"/tmp/from_config.log\"\n"
        )
        .unwrap();

        let child = root.path().join("child");
        std::fs::create_dir(&child).unwrap();
        std::env::remove_var("ROML_LOG_FILE");
        std::env::set_current_dir(&child).unwrap();

        ensure_logfile_env();
        assert_eq!(std::env::var("ROML_LOG_FILE").unwrap(), "/tmp/from_config.log");
    }
}
