use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_BUNDLED");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_SYSTEM");

    if env::var_os("CARGO_FEATURE_BUNDLED").is_some()
        && env::var_os("CARGO_FEATURE_SYSTEM").is_some()
    {
        panic!("features `bundled` and `system` are mutually exclusive; activate at most one");
    }
}
