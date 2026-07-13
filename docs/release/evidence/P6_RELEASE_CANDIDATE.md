# P6 Release Candidate Evidence

**Date:** 2026-07-13  
**Candidate SHA:** `c1fe456` (worktree-phase-roml-P0-release-baseline)  
**Base:** `main@82e2ed95545635b628187ba0081fe8c8b03eaafb`

## Pre-release checks

| Check | Command | Result |
|-------|---------|--------|
| Formatting | `cargo fmt --all -- --check` | PASS |
| Clippy | `cargo clippy -p roml --all-targets -- -D warnings` | PASS |
| Tests (lib) | `cargo test -p roml` | 157 pass, 0 fail |
| Tests (all) | `cargo test -p roml --all-targets` | 217 pass, 0 fail, 11 ignored |
| Docs | `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | PASS |
| Package list | `cargo package --list -p roml` | 31 files, no contamination |
| Package build | `cargo package -p roml --no-verify` | 228KiB compressed |
| Build examples | `cargo build --examples` | PASS |
| Unsafe code | `#[deny(unsafe_code)]` in core | PASS (0 unsafe) |

## Package contents

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
src/ (23 .rs files)
tests/ (5 .rs files)
```

## Publication order

1. `roml` (core) — no solver dependencies, builds on stable Rust ≥ 1.85
2. `roml-highs` — depends on roml, requires HiGHS libraries
3. `roml-mosek` — `publish = false` (experimental, requires license)
4. `roml-xpress` — `publish = false` (experimental, requires license)

## Test matrix

| Layer | Linux | macOS | Windows | Verified |
|-------|-------|-------|---------|----------|
| Core build | — | ✅ aarch64 | — | CI pending |
| Core tests | — | ✅ 217 passed | — | CI pending |
| Core clippy | — | ✅ clean | — | CI pending |
| Core docs | — | ✅ clean | — | CI pending |
| HiGHS | — | ⬜ (needs native) | — | |
| MOSEK | — | ⬜ (needs native) | — | |
| Xpress | — | ⬜ (needs native) | — | |

## Known issues (deferred to future phases)

- License files (MIT, Apache-2.0) not yet committed — pending owner confirmation
- HiGHS/MOSEK/Xpress handwritten FFI not yet migrated to official bindings (P3 requires native installs)
- Cross-platform CI not yet triggered (needs push to GitHub)
- `cargo-semver-checks` not yet run (advisory until baseline exists)
- `cargo deny check` not yet run locally (tool available, config exists)
- `rand` 0.9.2 dev-dependency has RUSTSEC-2026-0097 advisory

## Gate verdict

**READY for publication** contingent on:
1. Owner confirms license (MIT OR Apache-2.0) → commit LICENSE files
2. Push worktree → create PR → CI passes on all 3 OS
3. `cargo deny check` passes
4. Owner explicitly authorizes publication of exact SHA and crate list

**Do not publish without explicit owner authorization.**
