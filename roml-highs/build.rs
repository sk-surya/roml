use std::env;
use std::path::{Path, PathBuf};

struct LinkConfig {
    lib_dirs: Vec<PathBuf>,
    libs: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for key in [
        "HIGHS_ROOT",
        "HIGHS_LIB_DIR",
        "HIGHS_SOURCE_DIR",
        "HIGHS_EXTRA_LIB_DIRS",
        "HIGHS_EXTRA_LIBS",
        "HIGHS_BUILD_SHARED",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }

    let config = resolve_link_config()
        .map_err(|msg| format!("roml-highs build configuration error: {msg}"))?;

    for dir in config.lib_dirs {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }

    for lib in config.libs {
        println!("cargo:rustc-link-lib={lib}");
    }

    Ok(())
}

fn resolve_link_config() -> Result<LinkConfig, String> {
    if let Some(source_dir) = env_path("HIGHS_SOURCE_DIR")? {
        return build_from_source(&source_dir);
    }

    if let Some(lib_dir) = env_path("HIGHS_LIB_DIR")? {
        return link_from_lib_dir(&lib_dir);
    }

    if let Some(root) = env_path("HIGHS_ROOT")? {
        return link_from_root(&root);
    }

    Err(
        "set HIGHS_ROOT or HIGHS_LIB_DIR to link an existing HiGHS install, or set HIGHS_SOURCE_DIR to build HiGHS from source".to_string(),
    )
}

fn env_path(key: &str) -> Result<Option<PathBuf>, String> {
    match env::var_os(key) {
        Some(value) if !value.is_empty() => Ok(Some(PathBuf::from(value))),
        _ => Ok(None),
    }
}

fn link_from_lib_dir(lib_dir: &Path) -> Result<LinkConfig, String> {
    if !lib_dir.is_dir() {
        return Err(format!(
            "{lib_dir} is not a directory",
            lib_dir = lib_dir.display()
        ));
    }

    if !contains_highs_library(lib_dir) {
        return Err(format!(
            "no libhighs library found in {lib_dir}; set HIGHS_LIB_DIR to the directory containing libhighs.*",
            lib_dir = lib_dir.display()
        ));
    }

    let mut lib_dirs = vec![lib_dir.to_path_buf()];
    lib_dirs.extend(extra_lib_dirs()?);

    Ok(LinkConfig {
        lib_dirs,
        libs: default_link_libs(),
    })
}

fn link_from_root(root: &Path) -> Result<LinkConfig, String> {
    if !root.exists() {
        return Err(format!(
            "HIGHS_ROOT path does not exist: {}",
            root.display()
        ));
    }

    let candidates = [root.to_path_buf(), root.join("lib"), root.join("lib64")];
    let lib_dir = candidates
        .into_iter()
        .find(|candidate| candidate.is_dir() && contains_highs_library(candidate))
        .ok_or_else(|| {
            format!(
                "could not find libhighs.* under {}; expected it in the provided path, lib/, or lib64/",
                root.display()
            )
        })?;

    link_from_lib_dir(&lib_dir)
}

fn build_from_source(source_dir: &Path) -> Result<LinkConfig, String> {
    if !source_dir.exists() {
        return Err(format!(
            "HIGHS_SOURCE_DIR path does not exist: {}",
            source_dir.display()
        ));
    }

    println!(
        "cargo:warning=building HiGHS from source at {}",
        source_dir.display()
    );

    let build_shared = env::var("HIGHS_BUILD_SHARED").unwrap_or_else(|_| "ON".to_string());

    let install_dir = cmake::Config::new(source_dir)
        .profile("Release")
        .define("BUILD_SHARED_LIBS", build_shared)
        .build();

    link_from_root(&install_dir)
}

fn contains_highs_library(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .any(|name| {
            name == "highs.lib"
                || name == "highs.dll.lib"
                || name.starts_with("libhighs.")
                || name.starts_with("libhighs_")
        })
}

fn extra_lib_dirs() -> Result<Vec<PathBuf>, String> {
    let Some(value) = env::var_os("HIGHS_EXTRA_LIB_DIRS") else {
        return Ok(Vec::new());
    };

    Ok(env::split_paths(&value).collect())
}

fn default_link_libs() -> Vec<String> {
    let mut libs = vec!["highs".to_string()];

    match env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => libs.push("c++".to_string()),
        Ok("linux") => libs.push("stdc++".to_string()),
        _ => {}
    }

    libs.extend(extra_libs());
    libs
}

fn extra_libs() -> Vec<String> {
    env::var("HIGHS_EXTRA_LIBS")
        .ok()
        .into_iter()
        .flat_map(|value| {
            value
                .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
                .filter(|part| !part.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}
