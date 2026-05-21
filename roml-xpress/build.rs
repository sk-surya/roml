use std::env;
use std::path::PathBuf;

fn main() {
    // XPRESS_DIR should point to the xpressmp directory (contains lib/ and bin/).
    // Default: the path inside the macOS app bundle.
    let xpress_dir = env::var("XPRESS_DIR").unwrap_or_else(|_| {
        "/Applications/FICO Xpress/Xpress Workbench.app/Contents/Resources/xpressmp".to_string()
    });

    let lib_dir = PathBuf::from(&xpress_dir).join("lib");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=xprs");

    // Embed rpath so the binary finds libxprs.dylib and libxprl.dylib at run time
    // without requiring DYLD_LIBRARY_PATH.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        println!(
            "cargo:rustc-link-arg=-Wl,-rpath,{}",
            lib_dir.display()
        );
    } else if target_os == "linux" {
        println!(
            "cargo:rustc-link-arg=-Wl,-rpath,{}",
            lib_dir.display()
        );
    }

    println!("cargo:rerun-if-env-changed=XPRESS_DIR");
}
