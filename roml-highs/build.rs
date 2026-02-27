fn main() {
    let lib_dir = std::env::var("HIGHS_LIB_DIR")
        .unwrap_or_else(|_| "/home/ilan/repos/highs/lib".to_string());
    println!("cargo:rustc-link-search=native={lib_dir}");
    println!("cargo:rustc-link-lib=static=highs");
    println!("cargo:rustc-link-lib=static=openblas");
    println!("cargo:rustc-link-lib=stdc++");
    // zlib: link by versioned filename since libz.so symlink may be absent
    // (zlib1g-dev not installed). libz.so.1 is always present on Ubuntu.
    println!("cargo:rustc-link-search=native=/usr/lib/x86_64-linux-gnu");
    println!("cargo:rustc-link-arg=-l:libz.so.1");
    println!("cargo:rustc-link-lib=pthread");
    println!("cargo:rustc-link-lib=dl");
    println!("cargo:rustc-link-lib=m");
    println!("cargo:rerun-if-env-changed=HIGHS_LIB_DIR");
}
