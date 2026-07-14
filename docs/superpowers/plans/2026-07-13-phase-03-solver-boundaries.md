# Phase 03 Plan — Solver Boundaries, Bindings, and Unsafe Safety

> Prerequisite: Phase 02 backend/session contract is frozen. Implement each backend in a separate worktree after writing backend-specific characterization tests. HiGHS is the reference and critical path; MOSEK/Xpress must not block it.

**Goal:** remove handwritten ABI risk where authoritative bindings exist, establish safe adapter sessions, and make callback/lifecycle behavior conform to official solver contracts.

**Requirements:** R4.*, R5.*, R6.*.

## Task 3.1 — Freeze backend contract and shared test suite

**Files:** `src/solver/` contract, create `tests/backend_contract/` or `roml-test-support` if needed.

Create backend-agnostic fixtures/tests for:

- snapshot load and revision identity;
- every `ModelOp`;
- add/remove/reindex sequences;
- activity toggles;
- objective switch/constant/sense;
- parameter-driven cell changes;
- unsupported operation -> rebuild;
- dirty apply -> rebuild;
- empty model;
- LP and small MIP solve/status;
- solution, dual, reduced-cost capability behavior;
- callback panic/error/cancellation contracts where supported.

The same suite must run against reference projection and each native backend with capability-based skips, never ad hoc backend-name skips.

## Task 3.2 — Design native error and metadata types

**Files:** solver error/status/metadata modules.

Add:

- backend name/version/build information;
- native operation and code/message;
- error category and adapter health effect;
- precise termination/solution status;
- capability descriptor;
- install/load diagnostics.

Replace constructor panics/asserts with `Result`. Remove false status collapses.

## Task 3.3 — Migrate HiGHS to maintained generated bindings

**Files:** `roml-highs/Cargo.toml`, delete/replace `roml-highs/src/ffi.rs`, `build.rs`, refactor adapter/native module and tests.

1. Pin a reviewed `highs-sys` release compatible with selected HiGHS.
2. Verify required C symbols, types, callback structs/constants, and version query are generated from official `highs_c_api.h`.
3. If callback APIs are absent:
   - reproduce against current `highs-sys`;
   - open/upstream a minimal change;
   - use a pinned fork only with commit/version documentation;
   - do not copy layouts into ROML.
4. Prefer bundled static HiGHS for default portability; keep a documented system-discovery feature if needed.
5. Delete ROML's duplicated C declarations and native build script when ownership moves to `highs-sys`.
6. Add adapter version/capability reporting.
7. Fix status mapping, empty objective switching, deterministic pair collection, and all ignored status codes.
8. Audit whether lazy-constraint behavior is officially supported for the pinned API; implement through official callback input/output semantics only.

**Safety tests:** callback null/length handling, handler panic captured, registration guard cleanup on every return, no use-after-free, repeated solve/reset/drop.

## Task 3.4 — Rewrite MOSEK over official Rust API

**Files:** `roml-mosek/Cargo.toml`, delete `src/ffi.rs` and obsolete `build.rs`, refactor adapter/tests/docs.

1. Add pinned official `mosek` dependency.
2. Reimplement environment/task creation, parameters, model operations, solve, statuses, solution extraction, and reset using safe official API where available.
3. Follow official installation environment (`MOSEK_INST_BASE`) and platform support rather than custom partial path guessing.
4. Share/no-explicit environment according to official API and concurrency/license design; document chosen ownership.
5. Remove task mutation from callbacks immediately.
6. Implement supported progress/cancellation/incumbent observation.
7. For lazy cuts, first add a test/design spike for collect -> terminate -> apply outside -> re-optimize. Proceed only if official return semantics and solution retrieval make it correct. Otherwise return `UnsupportedCapability`.
8. Test callback-enabled task thread restrictions; remove `unsafe impl Send` unless official API type guarantees and ROML invariants justify it.
9. Check runtime dependencies and license failures distinctly.

## Task 3.5 — Produce Xpress binding/legal spike before adapter rewrite

**Files:** `docs/release/XPRESS_BINDING_DECISION.md`; no publication code until accepted.

Investigate using installed official headers/docs:

- supported Xpress versions/platform triples;
- exact C signatures and constants;
- header/generated-binding redistribution terms;
- library/import-library/runtime filenames;
- process init/free rules;
- thread/callback mutation rules;
- license/runtime dependency discovery;
- docs.rs and clean-host compilation;
- CI runner feasibility.

Prototype both, where legally permissible:

A. `roml-xpress-sys` generated bindings with `links = "xprs"`; target-aware discovery.

B. runtime dynamic loading (`libloading` or generated loader) where crate compiles without Xpress and `XpressBackend::load(...)` returns diagnostics.

Decision criteria: safety, legal clarity, user install UX, CI, version pinning, callback support, and maintenance burden.

## Task 3.6 — Rewrite Xpress adapter after decision

1. Remove copied ABI/constants.
2. Make initialization return typed errors; own process-level init/free safely.
3. Replace direct stdout callback with logging/event delivery.
4. Check every return code and output pointer.
5. Implement only capabilities proven by official API; remove unsupported callback claims.
6. Test multiple sessions, drop order, reset, library missing, license missing, and version mismatch.
7. Remove `unsafe impl Send` unless formally justified.

## Task 3.7 — Deduplicate only stable shared mechanics

Compare the migrated adapters. If identical code remains for revision dispatch, index reindexing, or normalized operation traversal, create a private `roml-adapter-support` crate/module. Do not share:

- native lifecycle;
- statuses/errors beyond normalized interfaces;
- callback semantics;
- solver parameter policy;
- unsupported-operation decisions.

Add property tests for shared dense index maps under arbitrary deletions.

## Task 3.8 — Unsafe audit

Generate an inventory:

```bash
rg -n 'unsafe|extern "C"|\*mut|\*const|Send for|Sync for' . -g '*.rs'
```

For every block/impl:

- state safety invariants in comments;
- shrink scope;
- validate pointer/lifetime/thread assumptions;
- add regression test or executable assertion;
- verify no panic crosses FFI;
- verify RAII cleanup.

Run Miri on solver-neutral unsafe code and sanitizers/native smoke tests where feasible.

## Verification

Core/reference and HiGHS mandatory:

```bash
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
cargo clippy -p roml -p roml-highs --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml -p roml-highs --no-deps
```

MOSEK/Xpress use separate compile/load/license/solve commands documented by their support tier.

**Phase gate:** core contains no raw solver FFI; HiGHS uses maintained generated bindings; MOSEK uses official Rust API and no illegal callback mutation; Xpress has an accepted generated/runtime boundary; all unsafe/native lifecycle invariants are reviewed and tested.