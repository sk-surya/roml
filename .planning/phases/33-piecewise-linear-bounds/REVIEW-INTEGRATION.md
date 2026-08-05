# P33 Review — Pass 2 (Integration and Operations)

**Review:** gsd-integration-checker
**Phase:** 33-piecewise-linear-bounds
**Branch:** `phase-roml-P33-piecewise-linear-bounds` (worktree `.git/p33-impl`)
**Base:** `main@40af9f4` (confirmed ancestor of HEAD; parent of plan commit `c7cacf5`)
**Scope:** commits `1804c90`, `ba88f54`, `4d01605`, `ad707a4` on `c7cacf5`
**Review date:** 2026-08-04

## Verdict

**PASS — no P0/P1/P2 findings.** All cross-phase connections resolve to WIRED.
The PWL vertical slice wires end-to-end: `Model::add_piecewise_linear` (model
builder) → `ConstructKind::PiecewiseLinear` payload → session `compile_snapshot`
dispatch → `bridge::piecewise_linear::compile` via `BridgeFinalizer` →
`BackendSnapshot` rows/entities → `CompilationReport.formulation_decisions` →
facade `BackendSnapshot.report`. No compiler-contract change was introduced;
the P32 bridge framework, `BackendSnapshot`/`BackendDeltaBatch` semantics, and
`CompiledEntityRegistry` behavior are unmodified. Every phase verification
command was re-run and exits 0.

## Findings

### P0 (blocking)

None.

### P1 (blocking)

None.

### P2 (accepted / advisory)

None classified as defects. The following are documented limitations carried as
residual risks in the evidence file (not integration breaks):

- `PiecewiseLinearConstraint::point_value` (`src/construct/piecewise_linear.rs:142`)
  panics on a parameter-dependent point value during direct
  `evaluate`/`classify_curvature`. The compiler bridge resolves parameter
  dependencies correctly against the snapshot. Documented in evidence
  "Residual risks". Accepted.
- `pwl.scaling` diagnostic is minimal (value span / breakpoint span) and not
  yet hardened; flagged for P34 numerical-quality audit. Accepted.

## Findings count

| Severity | Count |
|----------|-------|
| P0 | 0 |
| P1 | 0 |
| P2 | 0 |

## Dimension-by-dimension verification

### 1. Incremental / rebuild behavior — WIRED

- PWL compiles through the frozen P26/P32 session path: `preflight_constructs`
  (`src/compiler/session.rs:1412`) handles PWL via `derive_variable_dependencies`
  and `derive_parameter_dependencies` (both extended in `src/construct/mod.rs`)
  plus the new `validate_construct_finiteness` PWL arm
  (`src/compiler/session.rs:1376`).
- Dispatch arm added at `src/compiler/session.rs:541` in the same
  deterministic construct-id loop as the other P32 bridges.
- Incremental/rebuild correctness: any `AddConstruct`/`RemoveConstruct`/
  `SetConstructActive` forces `RebuildRequired` (`session.rs:1192-1199`); a
  variable change touching a PWL dependency forces a rebuild
  (`session.rs:745,768,823` via `construct_depends_on_variable`); a parameter
  change on a PWL dependency forces a rebuild (`session.rs:1179`). All route
  back through `compile_snapshot` → PWL bridge. No stale-artifact path found.
- `BackendSnapshot`/`BackendDeltaBatch`/`CompiledEntityRegistry` semantics are
  unchanged: `src/compiler/backend_ir.rs` is not in the diff. No new fields
  added to any compiled-state struct. Report reuses the existing
  `FormulationDecision`/`CompilationReport` machinery (`src/compiler/report.rs`
  is unchanged). No compiler-contract change. Good.
- Report surfacing verified end-to-end: `BridgeFinalizer::add_decision`
  (`bridge/mod.rs:183`) → `BridgeOutput.decisions` → `construct_decisions`
  (`session.rs:577`) → `add_formulation_decisions` (`session.rs:605`) →
  `BackendSnapshotBuilder.formulation_decisions` (`backend_ir.rs:873`) →
  `CompilationReport::new` (`backend_ir.rs:911`) → public
  `BackendSnapshot.report` (`backend_ir.rs:297`), read by tests
  (`tests/piecewise_linear.rs:796`).

### 2. Failure recovery — WIRED

- Four validation rejections are typed `ModelError` (SM-14.1):
  `PwlTooFewPoints`, `PwlNonFiniteBreakpoint`, `PwlNonFinitePointValue`,
  `PwlDuplicateBreakpoint`, `PwlOutOfOrderBreakpoint`
  (`src/model/mod.rs:179-203`). Tested
  (`pwl_rejects_*`).
- Relation/curvature mismatch is `CompileError::UnsupportedFeature`
  (`bridge/piecewise_linear.rs:115,139`) — never a silent relaxation (D13).
  Tested (`pwl_epigraph_on_nonconvex_pwl_is_typed_error`).
- `NativeRequired` on PWL → `CompileError::UnsupportedFeature` via `select_path`
  (`bridge/piecewise_linear.rs:51`; `bridge/mod.rs:314-322` with
  `native_payloads_available()` false). Tested
  (`pwl_native_required_rejects_exact_graph`).
- Argument-interval/unbounded-expression errors name the construct and
  expression: the `BoundAnalyzer` error is wrapped in
  `CompileError::InvalidBigM { construct, expression }`
  (`bridge/piecewise_linear.rs:86-90`), consistent with the P32 `InvalidBigM`
  precedent (`bounds.rs:510`). Satisfies SM-13.4.
- Atomic compile: the bridge builds a private `BridgeFinalizer` and only
  commits via `finish()` on the success path; any early `?` return drops the
  finalizer. The session extends `construct_variables`/`rows`/`decisions` only
  after the bridge returns `Ok` (`session.rs:555-577`), so a failing construct
  yields `CompileError` and no partial `BackendSnapshot`. Good.

### 3. Cross-platform / version behavior — WIRED

- `roml-highs/tests/formulation_equivalence.rs` PWL reference-vs-portable
  equivalence section: `pwl_highs_exact_graph_matches_reference_for_all_curvatures`
  covers convex/concave/nonconvex under both `Auto` and `Portable`. Passes.
- `highs_capability_set` (`roml-highs/src/session.rs:579`) declares
  `BackendFeature::PiecewiseLinear` in `BRIDGE_SUPPORTED_M3_FEATURES`
  (`session.rs:548`) = `SupportLevel::Bridge`; `Sos2`/`NativePiecewiseLinear`
  remain in the `UNSUPPORTED` list (`session.rs:567-568`). No false native
  claim (P32 F4). Asserted by
  `highs_capability_set_declares_p32_bridge_support_without_native_claims`
  (iterates the bridge array, now including PWL, checking Bridge + no native).
- `BackendConstraint` stays empty (`backend_ir.rs:254`) and
  `native_payloads_available()` stays false (`backend_ir.rs:265`). Both files
  unchanged.

### 4. Public API diff — WIRED

- `cargo public-api -p roml` re-run: 22180 raw items (evidence records 22178 —
  trivial output-version drift, not a defect). 736 PWL-related items.
- Intended additions present: `PiecewiseLinearConstraint`, `PwlRelation`,
  `ExtrapolationPolicy`, `PwlCurvature`, `PwlPoint`, `add_piecewise_linear`,
  `BackendFeature::PiecewiseLinear`, `GeneratedRole` Pwl variants,
  `ModelError` Pwl variants.
- No unintended surface leaked: the bridge module is `pub(crate) mod
  piecewise_linear;` (`bridge/mod.rs:21`) and `compile` is `pub(crate)`. The
  bridge internals (`emit_supporting_rows`, `emit_exact_graph`,
  `classify_evaluated_curvature`, `eval_point_value`, `segment_slope_at`,
  `record_scaling`, `classify_curvature_from_slopes`) have **0** occurrences in
  the public-api capture. The `BridgeDependency`/`BridgeFinalizer` public items
  pre-existed in P32 (`advanced.rs` re-export at base `c7cacf5`), not added by
  P33.

### 5. Package / docs impact — WIRED

- Evidence file `docs/release/evidence/M3_P33_PIECEWISE_LINEAR_BOUNDS.md`
  complete: baseline, RED failures per task, verification matrix, deviations
  (none), residual risks. `docs/release/evidence/M3_P33_public_api_roml.txt`
  committed.
- `TRACEABILITY.md` updated: P33 evidence path corrected to
  `M3_P33_PIECEWISE_LINEAR_BOUNDS.md`; SM-12/SM-13/SM-14 closure records added
  (implementation; independent review pending per gate). Correct.
- `STATE.md`/`ROADMAP.md` unchanged (not in diff). Good.

### 6. Migration accuracy — WIRED

- `docs/migration/M3_BACKEND_IR.md` and other migration docs are not in the
  diff — unaffected. No migration doc was touched.

### 7. E2E sanity — WIRED

Commands re-run in the implementation worktree; all exit 0:

- `cargo test -p roml --test piecewise_linear` — 27 passed; 0 failed.
- `cargo test -p roml-highs --test formulation_equivalence pwl` — 1 passed; 0 failed.
- `cargo test -p roml --all-targets` — 35 test binaries, no failures.
- `cargo test -p roml-highs --all-targets` — 18 test binaries, no failures.
- `cargo clippy -p roml --all-targets -- -D warnings` — clean.
- `cargo clippy -p roml-highs --all-targets -- -D warnings` — clean.
- `cargo fmt --all -- --check` — exit 0.
- `cargo public-api -p roml` — 22180 items.

(No `roml-mosek`/`roml-xpress` exercised — known-broken and out of scope per
M2/M3 convention. No workspace-wide commands run.)

`git -C . status` — clean tree. `git log --oneline main..HEAD` — exactly
`ad707a4`/`4d01605`/`ba88f54`/`1804c90`/`c7cacf5`; base `40af9f4` is the parent
of the plan commit and an ancestor of HEAD.

## Requirements Integration Map

| Requirement | Integration Path | Status | Issue |
|-------------|-----------------|--------|-------|
| SM-12.8 | builder `add_piecewise_linear` → `(Construct, VarId)` + `BackendSnapshot.report` decisions | WIRED | — |
| SM-13.1/13.5 | `BoundAnalyzer::interval_of_snapshot` → `pwl.argument_interval` report decision (`bridge/piecewise_linear.rs:81-103`) | WIRED | — |
| SM-13.2 | no Big-M in `emit_exact_graph`/`emit_supporting_rows` | WIRED | — |
| SM-13.4 | `InvalidBigM {construct, expression}` / `UnsupportedFeature` naming PWL | WIRED | — |
| SM-14.1 | `add_piecewise_linear` validation → typed `ModelError` | WIRED | — |
| SM-14.2 | `classify_curvature_from_slopes` shared payload+bridge | WIRED | — |
| SM-14.3 | `emit_supporting_rows` zero-binary + report `generated_binaries=0` | WIRED | — |
| SM-14.4 | `emit_exact_graph` exact segment binaries + report | WIRED | — |
| SM-14.5 | `pwl_nonconvex_exact_graph_excludes_convex_relaxation` | WIRED | — |
| SM-14.6 | `pwl.*` report decisions via `add_decision` | WIRED | — |
| SM-14.7 | randomized fixed-input + HiGHS reference-vs-portable equivalence | WIRED | — |
| SM-02.5 | `EntityOrigin::Construct { role }` on every generated entity (`origin.rs` Pwl roles) | WIRED | — |
| P32 F4 | `native_payloads_available()` false; `NativeRequired`→`UnsupportedFeature`; Sos2/NativePWl Unsupported | WIRED | — |

**Requirements with no cross-phase wiring:** none. Every requirement in scope
has at least one cross-phase touchpoint verified end-to-end.
