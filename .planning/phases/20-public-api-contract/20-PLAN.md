# Phase 20 — Public API Contract Baseline

> **For agentic workers:** execute with test-first characterization and independent review. Do not modify production behavior until current and target contracts are captured.

**Goal:** freeze the exact M2 signatures, dispositions, and baseline evidence needed to implement without API drift.

**Requirements:** API-04, API-07, API-08, API-10.

## Files

Create:

- `tests/public_api_compile.rs`
- `tests/ui/target_quickstart.rs`
- `tests/ui/target_incremental.rs`
- `tests/ui/current_readme_drift.rs`
- `docs/release/evidence/M2_P20_BASELINE.md`
- `docs/release/PUBLIC_API_M2_DISPOSITION.md`

Modify only if required for test harness setup:

- `Cargo.toml`
- test support files

## Task 1 — Record exact baseline

- [ ] Record `git rev-parse HEAD`, Rust/Cargo versions, target, and OS.
- [ ] Run the core and HiGHS baseline matrices from `EXECUTION.md`.
- [ ] Run `cargo public-api -p roml` and `cargo public-api -p roml-highs`.
- [ ] Store complete commands and concise results in `M2_P20_BASELINE.md`.
- [ ] Commit as `docs: record M2 public API baseline`.

## Task 2 — Characterize documentation drift

- [ ] Create a compile fixture containing the README's current `HighsAdapter::solve_model` code.
- [ ] Run the fixture and record the expected unresolved import/method failure.
- [ ] Preserve this as characterization evidence, not a permanently failing default test; use a script or ignored UI fixture referenced by the baseline report.
- [ ] Verify repository search finds no production `HighsAdapter` or `solve_model`.
- [ ] Commit as `test: characterize documented solve API drift`.

## Task 3 — Add target compile contracts

Create compile-pass fixtures for these exact forms:

```rust
use roml::prelude::*;
use roml_highs::Highs;

fn quickstart() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("production");
    let x = model.add_variable(continuous().named("x"))?;
    let y = model.add_variable(integer().bounds(0.0, 10.0).named("y"))?;
    model.add_constraint((x + y).le(4.0).named("capacity"))?;
    model.maximize(3.0 * x + y)?;
    let mut highs = Highs::new()?;
    let solution = highs.solve(&mut model)?;
    assert!(solution.status().is_optimal());
    Ok(())
}
```

```rust
fn incremental() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();
    let x = model.add_variable(continuous())?;
    let price = model.add_parameter(parameter(1.0).named("price"))?;
    model.add_constraint(x.le(10.0))?;
    model.maximize(price * x)?;
    let mut highs = Highs::new()?;
    let _first = highs.solve(&mut model)?;
    model.set_parameter(price, 3.0)?;
    let _second = highs.solve(&mut model)?;
    Ok(())
}
```

- [ ] Add fixtures as initially non-compiling target contracts.
- [ ] Document that P21/P22 will turn them green; do not weaken signatures during implementation without updating `DECISIONS.md` and review.
- [ ] Commit as `test: define M2 target API contracts`.

## Task 4 — Inventory and disposition every public entry point

For each public item, assign exactly one disposition:

- golden path;
- optional syntax sugar;
- advanced backend extension;
- compatibility/deprecated;
- internal exposure to remove.

At minimum cover:

- root/prelude re-exports;
- model constructors and mutators;
- expression/operator traits;
- all four macros;
- IDs and coefficient APIs;
- solution/status/result types;
- backend session traits and sync types;
- callback and capability types.

- [ ] Write the table in `PUBLIC_API_M2_DISPOSITION.md`.
- [ ] Include replacement signatures and deprecation order.
- [ ] Commit as `docs: classify M2 public API surface`.

## Task 5 — Baseline repeated-solve protocol behavior

- [ ] Build a direct `HighsSession` test that rebuilds, solves, applies one parameter/bound delta, and solves again.
- [ ] Record revisions, health, status, objective, and solution availability.
- [ ] Add an unsupported/dirty path showing deterministic snapshot recovery.
- [ ] Use results as expected behavior for P21 façade tests.
- [ ] Commit as `test: baseline repeated backend session behavior`.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
```

## Gate

P20 passes when target signatures, public-item dispositions, current drift, protocol behavior, and baseline commands are reviewed. No production API implementation is required in this phase.