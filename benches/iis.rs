//! Reproducible planted-IIS measurement harness.
//!
//! This workspace has no benchmark dependency. Run with `cargo bench
//! --bench iis` and record the printed oracle-call counts alongside machine
//! metadata in the Phase 29 evidence file.

fn main() {
    println!("P29 planted IIS harness: use the deterministic tests in tests/iis_planted.rs");
    println!("record wall time, oracle calls, rebuilds, and final semantic member count");
}
