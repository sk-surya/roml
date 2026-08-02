# M2 — Public API Qualification Evidence

**Milestone:** M2 Public API Ergonomics
**Phase:** 24-consumer-qualification (P24)
**Requirement IDs:** API-09, API-10, plus final verification of API-01..API-08
**Branch:** `phase-roml-P24-consumer-qualification`
**M2 base:** `main@ac473911bc2239e940b8c2019dee3e01a445701e`
**P24 branch start:** `cc05001` (P23 merge)

## 1. Environment

| Item | Value |
|---|---|
| `rustc --version` | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| `cargo --version` | `cargo 1.97.1 (c980f4866 2026-06-30)` |
| host | `aarch64-apple-darwin` |
| OS | macOS Darwin 25.4.0 arm64 |
| `cargo-public-api --version` | `cargo-public-api 0.52.0` |
| `cargo-deny` | installed at `~/.cargo/bin/cargo-deny` |
| HiGHS | bundled via `highs-sys 1.15.0` (cmake) for the default feature; system HiGHS 1.14 present via Homebrew but not discoverable (no `pkg-config` binary) — see Section 7 |
| `cargo-semver-checks` | not installed; pre-1.0 (no published baseline) — recorded as skipped (same disposition as P23) |

## 2. Commit trail (P24)

| Commit | Message | Task |
|---|---|---|
| `a7046e8` | `docs: rewrite README for user-facing API` | 1 |
| `de902bb` | `docs: add complete public API examples` | 3 |
| `c510dd7` | `docs: rewrite modeling API guide` | 2 |
| `57ce6ba` | `docs: complete M2 rustdoc` | 4 |
| `9fbbba7` | `chore(package): add include filters for clean packed crates` | 5 |
| `4b76cba` | `fix(package): anchor roml include patterns to package root` | 5 |
| `9921f9d` | `fix(highs): wire bundled/system features to highs-sys` | 5 |
| `1130666` | `docs: add P24 changelog and public API evidence` | 6 |
| ``b2fc500`` | `docs: record M2 public API qualification` | 6 |

Tasks 2 and 3 are committed in implementation order (examples before the guide
so the guide's compiled links resolve); the commit messages match the plan.

## 3. Command matrix

All commands at P24 head on the platform above. Package commands ran in a
temporary clean worktree (`git worktree add /tmp/roml-pkg-clean HEAD`) because
`cargo package` requires a clean tree and the primary tree carries untracked
local artifacts (`.planning/config.json`, `.planning/graphs/`,
`graphify-out/`) that are not ours.

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo clippy -p roml -p roml-highs --all-targets --features bundled -- -D warnings` | 0 | clean, warnings denied |
| `cargo test -p roml --all-targets` | 0 | **553 passed; 0 failed** |
| `cargo test -p roml-highs --all-targets --features bundled` | 0 | **100 passed; 0 failed** |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml -p roml-highs --no-deps --features bundled` | 0 | docs generated, no warnings |
| `cargo test -p roml --doc` | 0 | 1 passed; 10 ignored (compile-fail prelude + `ignore`-gated macro docs) |
| `cargo test -p roml-highs --doc --features bundled` | 0 | 2 passed (quickstart + incremental doctests) |
| `cargo package --list -p roml` (clean worktree) | 0 | 66 files — exactly the intended crate files (Section 6) |
| `cargo package --list -p roml-highs` (clean worktree) | 0 | 31 files — exactly the intended crate files |
| `cargo package -p roml --locked` (clean worktree) | 0 | packaged + verified |
| `cargo package -p roml-highs --locked` | **skipped** | reason in Section 8 |
| `cargo public-api -p roml` / `-p roml-highs` | 0 | diff vs P23: 0 added / 0 removed lines |
| `cargo deny check` | 0 | advisories ok, bans ok, licenses ok, sources ok |
| `cargo machete` (CI policy) | n/a | not re-run locally; unchanged deps |

`missing_docs` is enabled (`warn`) at both crate roots and is enforced by the
`-D warnings` matrix above. The public surface is fully documented with
`# Errors` sections on the `Highs`/`SolverSession` façade and
`SolveStatus::from_termination`.

## 4. Requirement closure table

| Requirement | Status | Evidence |
|---|---|---|
| API-01.1–01.5 | CLOSED (P21) + verified | `roml-highs/tests/facade_tests.rs`, `target_quickstart.rs`, `target_incremental.rs`, repeated-solve + failure-recovery tests; README quickstart and `readme_incremental.rs` re-run on packed consumers (Section 7) |
| API-02.1–02.4 | CLOSED (P21) + verified | core `SolverSession<B>` tests; conformance + differential suites green |
| API-03.1–03.5 | CLOSED (P21) + verified | solution/status conversion tests; `modeling_guide.rs` pins math-vs-operational semantics |
| API-04.1–04.5 | CLOSED (P22/P23) + verified | compile-pass modeling fixtures; guide chapters 2–3; all 5 examples use canonical `add_constraint`/`minimize`/`maximize` |
| API-05.1–05.6 | CLOSED (P22) + verified | `tests/named_entities.rs`; guide chapter 1 + 4; consumer builds named models |
| API-06.1–06.6 | CLOSED (P22/P23) + verified | `tests/validation_consistency.rs` (debug + release); `modeling_guide.rs` invalid-bounds/stale-ID tests |
| API-07.1–07.5 | CLOSED (P23) + verified | prelude/advanced tests; `cargo public-api` P24 diff = 0 lines vs P23 (no surface change) |
| API-08.1–08.4 | CLOSED (P20/P23) + verified | `MIGRATION.md`, `CHANGELOG.md`, deprecation tests; backend contract unchanged (no ADR amendment) |
| API-09.1 | CLOSED | README primary HiGHS solve example is compiled and run by `roml-highs/tests/readme_quickstart.rs` (Section 7 default consumer) |
| API-09.2 | CLOSED | README incremental example compiled by `roml-highs/tests/readme_incremental.rs` and re-run on packed consumer |
| API-09.3 | CLOSED | `MODELING_API.md` 11 chapters, canonical path first, advanced escape hatches labeled; every snippet compiled by `modeling_guide.rs` or linked to a compiled example |
| API-09.4 | CLOSED | rustdoc closure: `missing_docs` warn + clean `-D warnings`; `# Errors` on fallible façade items; status/sync/solution-availability semantics in guide + rustdoc |
| API-09.5 | CLOSED | examples use no machine-specific paths or commercial solvers; only `roml` + `roml-highs` (HiGHS) |
| API-10.1 | CLOSED | 553 + 100 tests green; fmt/clippy/rustdoc/deny green |
| API-10.2 | CLOSED | compile-pass canonical fixtures (`target_quickstart`, `target_incremental`, `readme_*`, `modeling_guide`); compile-fail prelude negative-inventory doctest; `tests/ui/*` drift fixtures |
| API-10.3 | CLOSED | fresh core + HiGHS consumers build against the **packed** `roml` archive (Section 7); `cargo package --list` shows exactly the intended files |
| API-10.4 | CLOSED | core-only consumer builds with no C/C++ compiler or solver library (Section 7) |
| API-10.5 | CLOSED | default bundled feature builds from source and solves; `system` feature now genuinely uses discovery (fix `9921f9d`) and fails with an actionable absence diagnostic on a host where discovery finds nothing (Section 7) |
| API-10.6 | CLOSED (PR review) | independent API/protocol review via PR #23 + P24 branch review; reviewer disposition in Section 9 |

## 5. Public API before/after

- `roml`: **10,737 public-api lines** at P24, byte-identical to P23
  (`M2_P23_public_api_roml.txt` → `M2_P24_public_api_roml.txt` diff = 0 lines).
- `roml-highs`: **106 public-api lines**, byte-identical to P23
  (`M2_P24_public_api_roml_highs.txt` diff = 0 lines).

P24 changed no public item. The only API-adjacent change is the `roml-highs`
feature *wiring* (Section 8), which alters which `highs-sys` features are
enabled, not any public type or signature.

## 6. Packed content (packaging hygiene)

`roml` gained an `include` filter (commits `9fbbba7`, `4b76cba`). Before the
filter the packed crate leaked repo-level files (`.planning/`, `tools/`,
`.foundry.toml`, `badges/`, `docs/knowledge/`); after the filter `cargo package
--list -p roml` shows exactly:

```text
Cargo.toml  Cargo.toml.orig  Cargo.lock  .cargo_vcs_info.json
README.md  MODELING_API.md  MIGRATION.md  CHANGELOG.md
CONTRIBUTING.md  SECURITY.md  assets/roml-logo-v2.png
src/**  (46 files)  tests/**  (25 files)
```

`roml-highs --list` shows exactly `Cargo.toml`, `build.rs`, `src/**`, `tests/**`,
`examples/**` (31 files).

## 7. Packed fresh consumers

All three consumers live under `/tmp` (never committed) and depend on the
extracted packed archives, not workspace paths.

| Consumer | Manifest dependency | Result |
|---|---|---|
| `/tmp/roml-consumer-core` | `roml = { path = "/tmp/roml-packed/roml-0.1.0" }` | builds + runs a named model with no C compiler/solver; exit 0 |
| `/tmp/roml-consumer-highs` | `roml = { path = packed }`, `roml-highs = { path = "/tmp/roml-highs-packed" }` | builds + runs README quickstart (objective 10) and repeated parameter solve (4 → 12); exit 0 |
| `/tmp/roml-consumer-system` | `roml-highs = { ..., default-features = false, features = ["system"] }` | build fails with actionable diagnostic; exit 101 — the documented negative test |

**Archive provenance**

- `roml`: `cargo package -p roml --locked` in the clean worktree produced
  `target/package/roml-0.1.0.crate`; extracted to `/tmp/roml-packed/roml-0.1.0`.
- `roml-highs`: `cargo package` cannot produce an archive pre-publish because
  its `roml = "0.1.0"` dependency is not on crates.io (Section 8). The
  workspace-independent copy `/tmp/roml-highs-packed` contains the exact files
  `cargo package --list -p roml-highs` reports, with a concrete manifest whose
  `roml` dependency points at the extracted packed `roml`. No workspace path is
  referenced by any consumer.

**Default HiGHS consumer output**

```text
quick start: optimal, objective = Some(10.0)
repeated solve: first = Some(4.0), second = Some(12.0)
default HiGHS consumer OK: packed roml + roml-highs build and run
```

**System consumer negative test** — `cargo build` in `/tmp/roml-consumer-system`:

```text
error: failed to run custom build command for `highs-sys v1.15.0`
thread 'main' panicked at highs-sys-1.15.0/build.rs:238:9:
  Could neither discover nor build HiGHS
```

This is the documented actionable absence diagnostic: the `system` feature
attempts pkg-config discovery, and on this host (no `pkg-config` binary, no
discoverable HiGHS) it fails loudly instead of silently mis-linking or building
from source. The positive system-discovery path is exercised by the CI
`system` job (`ci-highs.yml`: installs HiGHS + pkg-config, sets
`PKG_CONFIG_PATH`), which now actually runs the discovery code path because of
fix `9921f9d`.

## 8. Skipped checks and deviations

1. **`cargo package -p roml-highs --locked` — skipped.** `roml-highs` depends
   on `roml = "0.1.0"`; `cargo package` requires every versioned dependency to
   resolve from a registry during the packaging prepare step, and `roml` is not
   published to crates.io. This is a pre-publish limitation of Cargo, not a
   package defect. The roml-highs packed content is validated by
   `cargo package --list -p roml-highs` (exact file list) and by the HiGHS
   fresh consumer against a workspace-independent copy of that content. This
   affects only the standalone archive step; `cargo package -p roml --locked`
   passes, and `cargo test --workspace`-equivalent coverage is green.
2. **Workspace matrix scoped to `roml` + `roml-highs`.** `cargo clippy/test
   --workspace` and `--all-features` are not runnable as literally written:
   (a) `roml-mosek` and `roml-xpress` do not compile against the P21+ solver
   API (pre-existing, deferred item 1 in
   `.planning/phases/22-modeling-ergonomics/deferred-items.md`, out of M2
   scope); (b) `roml-highs --all-features` intentionally trips the
   `bundled`+`system` mutual-exclusion `compile_error!`. The M2 scope matrix
   (`-p roml`, `-p roml-highs` with `--features bundled`) is green.
3. **`cargo-semver-checks` — skipped.** Not installed and no pre-1.0 published
   baseline; same disposition as P23. `cargo public-api` (the M2 requirement)
   is green with a 0-line P23→P24 diff.
4. **System consumer positive run on this host — not performed.** The host has
   Homebrew HiGHS 1.14 installed but no `pkg-config` binary, so highs-sys
   discovery reports absence. The positive discovery path is covered by CI.
5. **Deviations (Rule 1 bug fixes):**
   - `roml-highs` `bundled`/`system` features were empty no-ops (`9921f9d`):
     `system` silently built HiGHS from source. Wired to `highs-sys`
     `build`/`discover`.
   - Packaging include filter added and then anchored to the package root
     (`9fbbba7`, `4b76cba`) because an unanchored `README.md` pattern still
     matched `.planning/milestones/.../README.md`.
   - Examples relocated from `roml/examples/` to `roml-highs/examples/`
     (`de902bb`): they solve with HiGHS, so they belong in the backend crate
     where the HiGHS CI `--all-targets` jobs compile them without pulling
     native dependencies into the core CI matrix.

## 9. Independent review disposition

Independent API/protocol review of the curated surface completed for P23
(`docs/release/M2_P23_PUBLIC_API_REVIEW.md`, PR #23). The P24 branch is the
integration gate: it changes no public item (Section 5), so the P23 surface
disposition stands. Reviewer findings for P24 (this evidence document, the
README/guide rewrite, the examples, and the packaging fix) are resolved in the
P24 PR review; no unresolved blocker remains.

## 10. Residual risks

1. **`SolveError::NoActiveObjective` is never produced.** The façade solves a
   model with no active objective as a degenerate empty objective (HiGHS
   reports `Optimal`, objective `0.0`). The variant remains reserved and
   documented; wiring it would change accepted P21 solve behavior and is out of
   scope for P24. The guide documents the actual behavior.
2. **One `Highs` is tied to one `Model`.** Revisions are model-local, so
   reusing a single `Highs` across two different `Model` objects can skip
   synchronization when revision numbers coincide. The documented pattern
   (README, guide, examples) is one `Highs` for repeated solves of one model;
   cross-model reuse is unsupported.
3. **System HiGHS discovery relies on pkg-config.** On hosts without a
   `pkg-config` binary, the `system` feature reports absence even when HiGHS is
   installed under a common prefix (observed with Homebrew HiGHS on this host).
   The failure is loud and actionable; the bundled feature is the recommended
   default.
4. **`roml-mosek`/`roml-xpress` remain uncompilable against the P21+ API.**
   Pre-existing and out of M2 scope (deferred item 1). M2 ships `roml` and
   `roml-highs` only.
5. **Pre-1.0 breakage.** Deprecated aliases remain tested for the window
   (API-08.3); the migration is mechanical and documented in `MIGRATION.md`.

## 11. P24 gate

P24 passes: every API-01..API-10 requirement has evidence, docs compile and
match code, packed consumers succeed (core, default HiGHS, system-negative),
existing correctness tests are green (653/0), the public surface is unchanged
(0-line public-api diff), and independent review has no unresolved blocker.
