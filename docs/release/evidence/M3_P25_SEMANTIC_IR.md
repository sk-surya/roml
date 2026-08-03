# P25 Evidence — Canonical Semantic IR, Identities, and Metadata

**Phase:** 25-semantic-ir-foundation
**Plan:** `25-PLAN.md`
**Requirements:** SM-01.1, SM-01.2, SM-01.3, SM-01.4, SM-01.5, SM-01.6, SM-02.1, SM-02.2, SM-02.3, SM-02.5 (foundations), SM-02.7, SM-15.1 (foundations)
**Branch:** `phase-roml-P25-semantic-ir-foundation`

This document records the P25 baseline, implementation evidence, public API diff, and reviewer dispositions per `EXECUTION.md` § "Evidence file structure".

## Scope and requirements

P25 establishes canonical semantic state before adding workflows: opaque identity (lineage/instance/construct), entity metadata, linear function-in-set constraints, and the generation-safe construct arena. It is a serial chain (Task 1 → Task 2 → Task 3 → Task 4) with no intra-phase parallelism.

## Baseline and environment

| Item | Value |
|---|---|
| Base commit (`git rev-parse HEAD`) | `7b124ad164ebd42259c2541cf8484826d6eecfba` |
| Branch | `phase-roml-P25-semantic-ir-foundation` |
| `rustc --version` | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| `cargo --version` | `cargo 1.97.1 (c980f4866 2026-06-30)` |
| `rustc -vV` host | `aarch64-apple-darwin` (LLVM 22.1.6) |
| OS | macOS `Darwin forge 25.4.0` Darwin Kernel 25.4.0 arm64 |
| `cargo public-api --version` | `cargo-public-api 0.52.0` |
| HiGHS build | bundled via `highs-sys 1.15.0` (cmake); no system HiGHS |

All commands below ran on the platform above with the toolchain above at the base commit, on the untouched tree (before any P25 source modification).

### Untouched baseline matrix — `roml` (Task 1)

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo check -p roml --all-targets` | 0 | clean |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo test -p roml --all-targets` | 0 | **553 passed; 0 failed; 2 ignored** |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 | docs generated, no warnings |
| `cargo package --list -p roml` | 0 | 70 files (include-filtered; see `M3_P25_package_roml.txt`) |

### Untouched baseline matrix — `roml-highs` (Task 1)

| Command | Exit | Result |
|---|---|---|
| `cargo check -p roml-highs --all-targets` | 0 | clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo test -p roml-highs --all-targets` | 0 | **100 passed; 0 failed; 0 ignored** |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 | docs generated, no warnings |
| `cargo package --list -p roml-highs` | 0 | 32 files (see `M3_P25_package_roml_highs.txt`) |

### Public API / package capture (Task 1)

- `docs/release/evidence/M3_P25_public_api_roml.txt` — `cargo public-api -p roml` output, normalized per M2 convention (repository absolute paths replaced with the `$REPO` token). `10737` lines.
- `docs/release/evidence/M3_P25_public_api_roml_highs.txt` — `cargo public-api -p roml-highs` output, normalized. `106` lines.
- `docs/release/evidence/M3_P25_package_roml.txt` — `cargo package --list -p roml` (70 files).
- `docs/release/evidence/M3_P25_package_roml_highs.txt` — `cargo package --list -p roml-highs` (32 files).

## Commit trail

| Task | Commit | Message |
|---|---|---|
| 1 | `(pending — filled at commit)` | `test(m3): capture semantic modeling baseline` |

## Public interfaces

<!-- Filled per task. -->

## Focused verification

### Task 1 — characterization on the untouched tree

`tests/m3_baseline_characterization.rs` (6 tests) covers: fluent linear modeling (`Model::new` + `add_variable` + `add_constraint` + `maximize`), deterministic snapshot round-trip, parameter update (`set_parameter`), objective constant propagation, solution metadata, and one-rebuild-retry behavior.

```text
running 6 tests
test solution_metadata_round_trip ... ok
test objective_constant_propagation ... ok
test parameter_update_propagates_to_coefficients ... ok
test deterministic_snapshot_round_trip ... ok
test fluent_linear_modeling ... ok
test one_rebuild_retry_recovers_post_update_state ... ok

test result: ok. 6 passed; 0 failed; 0 ignored
```

Command: `cargo test -p roml --test m3_baseline_characterization -- --nocapture` (exit 0).

This is characterization, not a red/green feature test — it passes on the untouched tree and must keep passing as P25 extends canonical state (SM-01.5 / SM-15.1).

## Full verification

<!-- Filled per task. -->

## Native/backend evidence

<!-- P25 touches no native surface; recorded here when applicable. -->

## Failure/recovery evidence

<!-- Filled per task. -->

## Public API and packaging

<!-- Filled at the phase boundary after Task 4. -->

## Deviations and decisions

<!-- Filled per task. -->

## Reviewer findings

<!-- Filled at the P25 phase boundary after Task 4 (independent review). -->

## Residual risks

<!-- Filled per task. -->

## Gate result

<!-- Filled at the P25 phase boundary. -->
