# P0 Baseline Evidence Report

**Date:** 2026-07-13
**Base SHA:** `82e2ed95545635b628187ba0081fe8c8b03eaafb`
**Host:** aarch64-apple-darwin
**Rust:** 1.95.0 (59807616e 2026-04-14)
**Cargo:** 1.95.0 (f2d3ce0bd 2026-03-21)

## Solver Environment (redacted to presence/absence)

| Variable | Present |
|----------|---------|
| HIGHS_LIB_DIR | Yes |
| MOSEK_BINDIR | Yes |
| MOSEK_LIB_DIR | Yes |
| XPRESS_DIR | No |

## Baseline Command Results

### 1. Formatting (`cargo fmt --all -- --check`)

**Result: FAIL**

Formatting violations across `roml-highs/build.rs`, `roml-highs/src/adapter.rs`, `roml-highs/src/ffi.rs`, `roml-highs/src/lib.rs`, `roml-mosek/build.rs`, `roml-mosek/src/adapter.rs`, `roml-mosek/src/ffi.rs`, `roml-xpress/build.rs`, `roml-xpress/src/adapter.rs`, `roml-xpress/src/ffi.rs`.

### 2. Check (`cargo check --workspace --all-targets`)

**Result: WARNINGS**

Warnings:
- `roml`: 1 warning (unconditional_recursion in ModelConstants::default())
- `roml-highs`: 17 warnings (unused imports, non_camel_case_types, non_snake_case, unused constants, etc.)
- `roml-xpress`: 1 warning
- `roml-mosek`: compiles

### 3. Clippy (`cargo clippy --workspace --all-targets --all-features -- -D warnings`)

**Result: FAIL (11 errors in roml)**

Core errors:
- `src/expr/linear.rs:727` — useless_conversion
- `src/expr/linear.rs:918` — needless_borrows_for_generic_args
- `src/logging.rs:205` — write_with_newline
- `src/logging.rs:228` — write_with_newline
- Plus 7 more

Backend crates: warnings only (17 in roml-highs, 1 each in roml-mosek/roml-xpress)

### 4. Tests (`cargo test -p roml --all-targets`)

**Result: FAIL (76 passed, 1 failed)**

Failing test: `logging::tests::workspace_root_sets_env` — assertion fails on path prefix

Previously failing test `logging::tests::init_with_explicit_path` passed on second run (logger initialization race)

### 5. Documentation (`RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps`)

**Result: FAIL**

- `src/model/mod.rs:253` — broken intra-doc link: `[0,1]` parsed as link
- `src/expr/linear.rs:128` — unclosed HTML tag: `Into<TermCoeff>` parsed as tag

Backend doc builds also fail (link resolution issues in backend crates).

### 6. Workspace Metadata

| Field | Value |
|-------|-------|
| roml version | 0.1.0 |
| License | null |
| License file | null |
| Description | "A production-grade, incremental MILP modeling library" |
| Repository | None |
| Documentation | None |
| Homepage | None |

### 7. Dependency Tree (core, non-dev)

| Dependency | Purpose | Removal Candidate |
|------------|---------|-------------------|
| log 0.4 | Logging facade | No (retain) |
| log4rs 1.4 | Logging implementation | Yes (Task 0.5) |
| rand 0.9 | Random number generation | Yes (Task 0.4) |
| serde_yaml 0.9 | YAML parsing | Yes (Task 0.5) |

### 8. Package Content (`cargo package --list`)

**roml (35 files):** Contamination detected — `.claude/settings.json`, `.gitignore`, `.python-version`, `.vscode/settings.json`, `AGENTS.md`, `Cargo.lock`, `config.yaml`, `log4rs.bak`, `main.py`, `pyproject.toml`, `roml.log`, `uv.lock`

**roml-highs (12 files):** `roml.log`

**roml-mosek (13 files):** `MOSEK_API_CHEATSHEET.md`, `roml.log`

**roml-xpress (11 files):** `roml.log`

All manifests lack license, documentation, homepage, and repository links.

### 9. Unsafe Inventory

Not yet enumerated systematically. Known:
- `roml-highs/src/ffi.rs` — handwritten HiGHS ABI declarations
- `roml-mosek/src/ffi.rs` — handwritten MOSEK ABI declarations
- `roml-xpress/src/ffi.rs` — handwritten Xpress ABI declarations

### 10. Tracked Contamination

61 tracked files. Known contaminants:
- `main.py` — placeholder Python scaffold
- `pyproject.toml` — Python project config
- `uv.lock` — Python lock file
- `config.yaml` — solver configuration
- `log4rs.bak` — stale logging config backup
- `roml.log` in multiple crates — generated solver logs
- `.python-version` — Python tooling
- `.vscode/settings.json` — IDE config
- `.claude/settings.json` — AI tooling config

### 11. `.claude/settings.json` Review

Present in repo root and in `roml` package list. Contains project-specific AI tooling configuration. Should not be packaged for crates.io publication. Should remain for development but be excluded from published crates.

### 12. Known Defects (Not Yet Characterized)

- `ModelConstants::default()` recursive definition
- Duplicate parameterized terms → last-write-wins
- `sync_model` drains changes before backend acknowledgement
- Semi-continuous HiGHS partial-apply counterexample
- `Model` owns one-shot `SolveOptions`
- Unsupported solve options silently ignored
- Handwritten ABI declarations in all three backends
- MOSEK callback mutates task inside callback

---

## Post-Cleanup Results (same session)

**Commit:** `fa38eb6` (HEAD of worktree-phase-roml-P0-release-baseline)

### Cleaned Baseline

| Check | Result |
|-------|--------|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy -p roml --all-targets -- -D warnings` | PASS |
| `cargo test -p roml --all-targets` | PASS (80 tests, 0 failures) |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | PASS |
| `cargo package --list -p roml` | 31 files, clean |
| `cargo package -p roml --no-verify` | 228.0KiB (51.4KiB compressed) |

### Cleaned Package Content

```
CHANGELOG.md
CONTRIBUTING.md
Cargo.lock
Cargo.toml
MODELING_API.md
README.md
SECURITY.md
docs/release/RELEASE_CHECKLIST.md
docs/release/SUPPORT_MATRIX.md
src/ (19 .rs files)
tests/ (2 .rs files)
```

### Removed Contamination

- main.py, pyproject.toml, uv.lock (Python scaffold)
- config.yaml, log4rs.bak (solver configuration)
- .python-version, .vscode/settings.json (tooling)
- roml.log, roml-*/roml.log (generated solver logs)
- src/logging.rs (global logger, log4rs, serde_yaml)

### Dependency Cleanup

| Action | Details |
|--------|---------|
| Removed | log4rs, serde_yaml from core |
| Moved to dev | rand (used in tests) |
| Retained | log (facade only) |
| Unused deps | cargo-machete: none found |

### Tooling

| Tool | Result |
|------|--------|
| cargo-audit | 1 warning: rand 0.9.2 unsound advisory (dev-dep, RUSTSEC-2026-0097) |
| cargo-machete | No unused dependencies |
| cargo-deny | Not installed (needs deny.toml config) |

### Metadata

| Crate | Version | License | Publish |
|-------|---------|---------|---------|
| roml | 0.1.0 | MIT OR Apache-2.0 | true |
| roml-highs | 0.1.0 | MIT OR Apache-2.0 | true |
| roml-mosek | 0.1.0 | MIT OR Apache-2.0 | false |
| roml-xpress | 0.1.0 | MIT OR Apache-2.0 | false |

### New Files Created

- `.github/workflows/ci-core.yml` — core CI on ubuntu/macos/windows
- `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`
- `docs/release/RELEASE_CHECKLIST.md`
- `docs/release/SUPPORT_MATRIX.md`

### Defects Fixed

- ModelConstants::default() recursion (removed inherent method, trait impl remains)
- rustdoc broken intra-doc links and unclosed HTML tags
- All clippy errors in core (7 total across lib and test targets)

### Explicit Blockers

- License files (LICENSE-MIT, LICENSE-APACHE): pending owner confirmation
- cargo-deny: needs deny.toml configuration per workspace
- Cross-platform CI: not yet pushed/verified (local worktree)
- Unsafe inventory: deferred to P3 (solver boundary phase)
- MODELING_API.md review: not completed

## Residual Risks

- Backend crates not fully tested (require native libraries)
- CI workflow not yet triggered (local worktree, needs push + PR)
- Unsafe code not inventoried (P3 task)
- Windows/Linux not tested locally
- rand dev-dependency advisory (RUSTSEC-2026-0097, unsound with custom logger)
- License files absent pending owner confirmation
