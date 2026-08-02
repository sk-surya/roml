---
phase: 24-consumer-qualification
verified: 2026-08-02T17:20:00Z
status: passed
score: 16/17 must-haves verified
behavior_unverified: 0
human_verification:
  - test: "Merge PR for the P24 branch and complete the independent API/protocol/documentation review of the P24 artifacts (evidence doc, README/guide rewrite, 5 examples, packaging feature fix)."
    expected: "Reviewer confirms API coherence, protocol preservation, error semantics, and documentation accuracy with no unresolved blocker; the P24 PR review disposition recorded in M2_PUBLIC_API.md Section 9."
    why_human: "Independent review (API-10.6) is an explicit gate element that the executing agent cannot perform on itself. The P23 surface review (PR #23) completed, and P24 changes no public item (0-line public-api diff), so the P23 disposition stands, but the P24 branch review is the pending human checkpoint."
  - test: "Confirm the positive system-HiGHS discovery path on a host that has pkg-config and a system HiGHS (or via the ci-highs.yml system job)."
    expected: "roml-highs --no-default-features --features system builds and solves against the installed HiGHS via discovery."
    why_human: "This host has no pkg-config binary, so only the negative path is locally verifiable; the positive path is CI-covered and needs an environment that has the toolchain."
---

# Phase 24: Consumer Qualification Verification Report

**Phase Goal:** prove the public API is understandable and usable from packed crates on clean consumers.
**Verified:** 2026-08-02T17:20:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | README rewritten around the golden path (8-section order); primary HiGHS solve example is compiled and RUNS (API-09.1) | ✓ VERIFIED | README §"Quick start" block matches `roml-highs/tests/readme_quickstart.rs` verbatim; test `readme_quickstart_compiles_and_runs` passes; equivalent code ran in the packed consumer |
| 2 | README incremental parameter example is compiled and RUNS — two solves, one `Highs` (API-09.2) | ✓ VERIFIED | README §"Incremental parameter updates" block matches `roml-highs/tests/readme_incremental.rs`; test passes; packed consumer printed `4.0 → 12.0` |
| 3 | MODELING_API.md covers the 11 plan chapters; every snippet compiled/linked; states implicit commit/rebuild + one-retry + math-vs-operational semantics (API-09.3) | ✓ VERIFIED | 11 `##` chapters present (MODELING_API.md:35–279); header states all snippets compiled by `modeling_guide.rs`; `cargo test -p roml-highs --test modeling_guide` 9/9 passed; one-retry at guide:235, math-vs-operational at guide:176–195 |
| 4 | Five HiGHS examples compile and run; no machine-specific paths or commercial solver deps (API-09.5) | ✓ VERIFIED | simple_lp (consumer ran), simple_mip (x=1,y=4,obj=21), parameter_update (4→12), solve_options (eff. gap 0.05, sync Delta), sparse_build (x=8/3,z=5,obj=62/3) all ran exit 0; only `roml`+`roml-highs` deps |
| 5 | Rustdoc closure: `missing_docs` (warn) + `# Errors` on fallible façade items + doctests pass + `-D warnings` clean (API-09.4) | ✓ VERIFIED | `#![warn(missing_docs)]` at src/lib.rs:14 and roml-highs/src/lib.rs:28; `# Errors` on `Highs::{new,solve,solve_with}` (roml-highs/src/facade.rs:56,74,90), `SolverSession` (src/solver/facade.rs:119,133), `SolveStatus::from_termination` (src/solver/mod.rs:90); orchestrator independently confirmed rustdoc `-D warnings` clean and doctests 1+2 passed |
| 6 | Fresh consumers build against PACKED archives with no workspace path leakage (API-10.3) | ✓ VERIFIED | manifests at /tmp/roml-consumer-{core,highs,system}/Cargo.toml point only at `/tmp/roml-packed/roml-0.1.0` and `/tmp/roml-highs-packed`; lockfiles show no repo paths; all three built (core/highs ran, system failed as designed) |
| 7 | Core-only consumer requires no C/C++ compiler or solver library (API-10.4) | ✓ VERIFIED | `/tmp/roml-consumer-core` ran exit 0 building a named 2-var/1-constraint/1-parameter model from packed roml |
| 8 | Default bundled HiGHS consumer works: README quickstart + repeated parameter solve (API-10.5 positive) | ✓ VERIFIED | `/tmp/roml-consumer-highs` ran exit 0: quickstart objective 10.0, repeated solve 4.0 → 12.0, built HiGHS 1.15.0 from source |
| 9 | System HiGHS mode produces an actionable absence diagnostic when discovery finds nothing (API-10.5 negative) | ✓ VERIFIED | `/tmp/roml-consumer-system` build failed exit 101 with `Could neither discover nor build HiGHS` (highs-sys build.rs:238) — exactly the documented diagnostic; host confirmed to have no `pkg-config` |
| 10 | Feature wiring functionally correct: bundled builds from source; system genuinely discovers/fails loudly | ✓ VERIFIED | roml-highs/Cargo.toml:23–26 — `bundled=["highs-sys/build","highs-sys/highs_release"]`, `system=["highs-sys/discover"]`, `default-features=false`; both paths behaviorally exercised |
| 11 | Existing correctness tests remain green (API-10.1) | ✓ VERIFIED | orchestrator independently re-ran 553/0 (roml) and 100/0 (roml-highs); I re-ran the P24-introduced suites: modeling_guide 9/9, readme 2/2, 5 examples, 3 consumers |
| 12 | Compile-pass canonical + compile-fail coverage (API-10.2) | ✓ VERIFIED | compile-pass fixtures target_quickstart/target_incremental/readme_*/modeling_guide execute; compile-fail prelude negative-inventory doctest at src/lib.rs:223–243; tests/ui/{current_readme_drift,current_solve_model_method}.rs drift fixtures |
| 13 | Public surface unchanged: 0-line public-api diff P23 → P24 (API-07.5) | ✓ VERIFIED | `diff M2_P23_public_api_roml.txt M2_P24_public_api_roml.txt` = 0 lines; roml_highs identical (10737 + 106 lines) |
| 14 | Every API-01..API-08 requirement has evidence in M2_PUBLIC_API.md | ✓ VERIFIED | Section 4 traceability table maps API-01.1–08.4 to tests/fixtures; all prior-phase suites green (orchestrator) |
| 15 | M2_PUBLIC_API.md written with traceability, command matrix, consumers, skipped checks, and residual risks — residual risks honestly recorded | ✓ VERIFIED | file at docs/release/evidence/M2_PUBLIC_API.md; §10 records NoActiveObjective-never-produced, one-Highs-one-Model, pkg-config system discovery, mosek/xpress uncompilable, pre-1.0 breakage — all confirmed accurate by code inspection |
| 16 | Docs compile: rustdoc `-D warnings` clean, doctests pass | ✓ VERIFIED | orchestrator independently verified `RUSTDOCFLAGS='-D warnings' cargo doc` clean and doctests 1 (roml) + 2 (roml-highs) passed; facade quickstart doctest present at roml-highs/src/facade.rs:14–35 |
| 17 | Independent review has no unresolved blocker (API-10.6) | ⚠️ HUMAN | P23 surface review (PR #23) completed; P24 branch PR review is pending (see Human Verification Required) |

**Score:** 16/17 truths verified (1 human verification item)

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `README.md` | 8-section golden-path order | ✓ VERIFIED | differentiator → install → quickstart → incremental → sync → topology → advanced link → status (README.md:19–159) |
| `MODELING_API.md` | 11 chapters, canonical-first, compiled snippets | ✓ VERIFIED | chapters at lines 35–279; compiled by modeling_guide.rs (9 tests pass) |
| `roml-highs/tests/readme_quickstart.rs` | compiled README solve fixture | ✓ VERIFIED | 33 lines; runs against real HiGHS; passes |
| `roml-highs/tests/readme_incremental.rs` | compiled README incremental fixture | ✓ VERIFIED | 37 lines; passes |
| `roml-highs/tests/modeling_guide.rs` | compiled guide-snippet fixture | ✓ VERIFIED | 226 lines; 9 tests covering ch.1–9 + 11; passes |
| `roml-highs/examples/{simple_lp,simple_mip,parameter_update,solve_options,sparse_build}.rs` | 5 working examples | ✓ VERIFIED | all run exit 0; no deprecated/machine-specific paths |
| `Cargo.toml` (roml) | root-anchored include filter | ✓ VERIFIED | lines 31–43; extracted archive = 70 files, zero repo-level leakage |
| `roml-highs/Cargo.toml` | include filter + bundled/system → highs-sys | ✓ VERIFIED | lines 10–16, 23–26; wiring behaviorally proven |
| `roml-highs/build.rs` | build support | ✓ VERIFIED | trivial (delegates to highs-sys build/discover) |
| `src/lib.rs`, `roml-highs/src/lib.rs` | `#![warn(missing_docs)]` + expanded docs | ✓ VERIFIED | both present; compile-fail prelude doctest at src/lib.rs:223 |
| `roml-highs/src/facade.rs` | `# Errors` on Highs | ✓ VERIFIED | lines 56, 74, 90 |
| `docs/release/evidence/M2_PUBLIC_API.md` | traceability + residual risks | ✓ VERIFIED | present; §4 closure table, §10 residual risks |
| `docs/release/evidence/M2_P24_public_api_{roml,roml_highs}.txt` | public-api dumps | ✓ VERIFIED | 10737 + 106 lines; 0-line diff vs P23 |
| `.planning/.../M2-public-api-ergonomics/STATE.md` | P24 record + closed IDs | ✓ VERIFIED | P24 record present, ledger updated |

### Key Link Verification

| From | To | Via | Status |
| ---- | --- | --- | ------ |
| README quickstart block | `roml-highs/tests/readme_quickstart.rs` | extracted fixture (same model/asserts) | ✓ WIRED |
| README incremental block | `roml-highs/tests/readme_incremental.rs` | extracted fixture | ✓ WIRED |
| MODELING_API.md snippets | `roml-highs/tests/modeling_guide.rs` | header states compile-link; 9 tests pass | ✓ WIRED |
| roml-highs `bundled` feature | highs-sys `build`/`highs_release` | Cargo.toml feature → built HiGHS 1.15.0 from source | ✓ WIRED |
| roml-highs `system` feature | highs-sys `discover` | Cargo.toml feature → pkg-config discovery; fails loudly when absent | ✓ WIRED |
| fresh consumers | extracted packed archives | /tmp manifests + lockfiles, no workspace paths | ✓ WIRED |
| `highs-sys` packaged content | roml-highs packed copy | `/tmp/roml-highs-packed` = Cargo.toml + build.rs + 5 examples + 11 src + 11 tests (matches `--list` minus auto-gen metadata) | ✓ WIRED |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| -------- | ------------- | ------ | ------------------ | ------ |
| HiGHS consumer quickstart | objective_value | real HiGHS solve against packed archives | objective 10.0 printed | ✓ FLOWING |
| HiGHS consumer repeated solve | objective_value ×2 | real parameter-delta re-solve | 4.0 → 12.0 printed | ✓ FLOWING |
| core consumer | model.pprint() | packed roml model store | named entities/constraints printed | ✓ FLOWING |
| simple_mip / solve_options / sparse_build | solution values | real HiGHS solves | 21 / gap 0.05 / 62÷3 | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Core-only consumer builds+runs packed roml, no solver | `cd /tmp/roml-consumer-core && cargo run` | exit 0; named model printed | ✓ PASS |
| Default HiGHS consumer: quickstart + repeated solve | `cd /tmp/roml-consumer-highs && cargo run` | exit 0; 10.0 / 4.0 → 12.0 | ✓ PASS |
| System consumer: actionable absence diagnostic | `cd /tmp/roml-consumer-system && cargo build` | exit 101; `Could neither discover nor build HiGHS` | ✓ PASS (negative, as documented) |
| README fixtures compile and run | `cargo test -p roml-highs --test readme_quickstart --test readme_incremental` | 2/2 ok | ✓ PASS |
| Modeling-guide snippets compile and run | `cargo test -p roml-highs --test modeling_guide --features bundled` | 9/9 ok | ✓ PASS |
| MILP example solves | `cargo run -p roml-highs --example simple_mip` | x=1, y=4, obj=21; exit 0 | ✓ PASS |
| Solve-options example | `cargo run -p roml-highs --example solve_options` | eff. gap 0.05, sync Delta; exit 0 | ✓ PASS |
| Sparse-build example | `cargo run -p roml-highs --example sparse_build` | x=8/3, z=5, obj=62/3; exit 0 | ✓ PASS |
| Public API unchanged | `diff M2_P23_* M2_P24_*` | 0 lines both crates | ✓ PASS |
| Packaging no leakage | `find /tmp/roml-packed/roml-0.1.0` | 70 files; no `.planning`/tools/foundry/badges | ✓ PASS |

### Probe Execution

Not applicable — this phase uses the workspace matrix commands (fmt/clippy/test/doc/package/deny) rather than `scripts/*/tests/probe-*.sh`; no probes declared in the PLAN. The matrix commands were independently re-run by the orchestrator and spot-re-run here.

### Requirements Coverage

| Requirement | Description | Status | Evidence |
| ----------- | ----------- | ------ | -------- |
| API-09.1 | compiled README HiGHS solve example | ✓ SATISFIED | readme_quickstart.rs runs |
| API-09.2 | compiled incremental two-solve example | ✓ SATISFIED | readme_incremental.rs runs |
| API-09.3 | canonical-first guide with labeled escape hatches | ✓ SATISFIED | 11 chapters; modeling_guide.rs 9/9 |
| API-09.4 | rustdoc errors/status/sync/solution coverage | ✓ SATISFIED | missing_docs + # Errors + doctests |
| API-09.5 | examples machine-independent, no commercial solver | ✓ SATISFIED | all 5 examples roml-only |
| API-10.1 | core + HiGHS suites green | ✓ SATISFIED | 553/0 + 100/0 (orchestrator); spot re-runs green |
| API-10.2 | compile-pass canonical + compile-fail | ✓ SATISFIED | fixtures + prelude negative doctest + tests/ui drift |
| API-10.3 | fresh consumers vs packaged archives | ✓ SATISFIED | 3 consumers built vs /tmp packed |
| API-10.4 | core-only no C/C++ compiler | ✓ SATISFIED | core consumer ran exit 0 |
| API-10.5 | default bundled + system-discovery modes | ✓ SATISFIED | bundled solved; system failed loudly (documented) |
| API-10.6 | independent review no unresolved blocker | ? NEEDS HUMAN | P24 PR review pending |
| API-01..API-08 | every earlier requirement evidence in M2 doc | ✓ SATISFIED | M2_PUBLIC_API.md §4 table + prior-phase suites |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| — | — | none (no TBD/FIXME/XXX/TODO/HACK/placeholder/empty-impl) | — | — |

### Human Verification Required

1. **Independent P24 review (API-10.6).**
   **Test:** Merge PR for the P24 branch and complete the independent API/protocol/documentation review of the P24 artifacts (evidence doc, README/guide rewrite, 5 examples, packaging feature fix).
   **Expected:** Reviewer confirms API coherence, protocol preservation, error semantics, and documentation accuracy with no unresolved blocker; the P24 PR review disposition recorded in M2_PUBLIC_API.md Section 9.
   **Why human:** Independent review is an explicit gate element the executing agent cannot self-perform. The P23 surface review (PR #23) completed; P24 changes no public item (0-line public-api diff), so the P23 disposition stands, but the P24 branch review is the pending checkpoint.
2. **Positive system-HiGHS discovery path.**
   **Test:** Run the `system` feature on a host that has pkg-config + an installed HiGHS (or confirm the ci-highs.yml system job passes).
   **Expected:** `roml-highs --no-default-features --features system` builds and solves via discovery.
   **Why human:** This host lacks a `pkg-config` binary, so only the negative path is locally verifiable; the positive path is CI-covered.

## Gaps Summary

No blocker-level gaps. All 17 must-haves are either verified or routed to human verification; the P24 gate is met on the automated dimension and awaits the independent P24 PR review.

Two documentation-accuracy warnings (non-blocking, recommended before the final M2 archive):

1. **Evidence file-count inaccuracy (WARNING).** M2_PUBLIC_API.md §3 records `cargo package --list -p roml` = "66 files" and roml-highs = "31 files", and §6 records "src/** (46 files), tests/** (25 files)". The authoritative extracted archive at `/tmp/roml-packed/roml-0.1.0` (produced by `cargo package -p roml --locked`) contains **70 files** (35 src, 24 tests, plus manifests, .cargo_vcs_info.json, Cargo.lock, 7 docs/assets), and `cargo package --list -p roml-highs` at HEAD reports **32 files**. The substantive claim — the include filter ships exactly the crate's intended files with zero repo-level leakage — is verified true; only the recorded counts are off. Correct the counts in M2_PUBLIC_API.md §3/§6 (and the matching SUMMARY D5 note).
2. **One-Highs-one-Model caveat placement (INFO).** The residual risk in M2_PUBLIC_API.md §10.2 is honestly recorded, and the positive pattern (repeated solves of one model) is well documented (README §Incremental, guide ch.8, facade docs), but the explicit "cross-model reuse is unsupported" warning is not stated in the user-facing docs — only in the evidence doc. Recommend a one-line note in MODELING_API.md ch.8/facade docs. Not a blocker; the risk is disclosed.

Verification note on the packaging check environment: `cargo package --list -p roml` fails from the primary tree because of an untracked `src/.DS_Store` (exit 101). This is why package commands run in a clean worktree (the documented P20 protocol); the extracted archive is the authoritative evidence and contains no `.DS_Store`.

**Deferred items:** none — everything not yet met (API-10.6) is a human checkpoint within this phase, not a later-phase item.

---

_Verified: 2026-08-02T17:20:00Z_
_Verifier: Claude (gsd-verifier)_
