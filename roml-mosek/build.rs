use std::env;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=MOSEK_BINDIR");
    println!("cargo:rerun-if-env-changed=MOSEK_ROOT");

    let bindir = find_mosek_bindir()
        .map_err(|msg| format!("roml-mosek build configuration error: {msg}"))?;

    println!("cargo:rustc-link-search=native={}", bindir.display());
    println!("cargo:rustc-link-lib=dylib=mosek64");

    // Add rpath so the dylib is found at runtime without DYLD_LIBRARY_PATH.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", bindir.display());
    } else {
        println!("cargo:rustc-link-arg=-Wl,-rpath={}", bindir.display());
    }

    Ok(())
}

fn find_mosek_bindir() -> Result<PathBuf, String> {
    if let Some(dir) = non_empty_env("MOSEK_BINDIR") {
        let path = PathBuf::from(&dir);
        if contains_mosek_library(&path) {
            return Ok(path);
        }
        return Err(format!("MOSEK_BINDIR={dir} does not contain libmosek64"));
    }

    if let Some(root) = non_empty_env("MOSEK_ROOT") {
        let root = PathBuf::from(&root);
        for candidate in [
            root.join("tools/platform/osxaarch64/bin"),
            root.join("tools/platform/linux64x86/bin"),
            root.join("bin"),
            root.clone(),
        ] {
            if contains_mosek_library(&candidate) {
                return Ok(candidate);
            }
        }
        return Err(format!(
            "could not find libmosek64 under MOSEK_ROOT={}; \
             tried tools/platform/osxaarch64/bin, tools/platform/linux64x86/bin, bin/",
            root.display()
        ));
    }

    Err(
        "set MOSEK_BINDIR to the directory containing libmosek64.dylib/.so, \
         or MOSEK_ROOT to the MOSEK installation root"
            .to_string(),
    )
}

fn non_empty_env(key: &str) -> Option<String> {
    match env::var(key) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

fn contains_mosek_library(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .any(|name| name.starts_with("libmosek64"))
}
