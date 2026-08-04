---
phase: 32-common-constructs
verified: 2026-08-03T00:00:00Z
status: passed
score: 11/12 must-haves verified
behavior_unverified: 0
overrides_applied: 0
gaps:
  - truth: "Phase gate verification commands all exit 0 (including RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps)"
    status: failed
    reason: "The roml rustdoc build exits 101 under -D warnings: 3 unresolved intra-doc links to the #[cfg(test)]-gated FixturePayload type remain in src/construct/mod.rs (module-doc lines referencing `crate::construct::FixturePayload` / `FixturePayload`) and src/advanced.rs:130 (module-doc `crate::construct::FixturePayload`). The WR-05 fix commit 06b6465 cfg(test)-gated FixturePayload but left the doc links, so they are broken in non-test builds. The SUMMARY/evidence file claim the doc lane is clean, which is false for the current tree."
    artifacts:
      - path: "src/construct/mod.rs"
        issue: "Module doc references [`crate::construct::FixturePayload`] and [`FixturePayload`] (lines ~4, ~11, ~20); the type is #[cfg(test)]-gated so the links resolve in no non-test build."
      - path: "src/advanced.rs"
        issue: "Line ~130 module doc references [`crate::construct::FixturePayload`]; same cfg(test)-gated break."
    missing:
      - "Escape or reword the three doc links (or #[doc(cfg)] the references) so RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps exits 0, and re-run the full phase verification matrix."
  - truth: "Review findings and dispositions recorded in the evidence file before the gate is marked pass"
    status: failed
    reason: "The PLAN Gate requires reviewer findings and dispositions recorded in docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md. The evidence file was last written at the Task 17c commit (109e0d8) and does not record the 9 review findings (CR-01, WR-01..05, IN-01..03), their dispositions, the fix commits (f3002a9..0498794), the final gate matrix (roml 773 / highs 121), or the post-WR-05 public API diff. 32-REVIEW.md / 32-REVIEW-FIX.md document them, but the designated evidence sink does not."
    artifacts:
      - path: "docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md"
        issue: "Missing the review-fix dispositions, final gate matrix, and post-WR-05 public API diff; the recorded verification claims (doc clean, roml 759) are stale for the current tree."
    missing:
      - "Append a review-fix section to the evidence file: findings + dispositions, fix commits, final gate matrix, updated public API count (18819), and residual risks."
---

# Phase 32: Common Constructs Verification Report

**Phase Goal:** deliver a bounded high-value exact MILP construct library over the frozen compiler contract.
**Verified:** 2026-08-03
**Status:** gaps_found
**Re-verification:** No — initial verification.

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Every construct has one semantic definition | ✓ VERIFIED | `ConstructKind` carries exactly one exact payload per construct: `IndicatorConstraint`, `ReificationConstraint`, `BooleanConstraint`, `CardinalityConstraint`, `MinMaxConstraint`, `AbsoluteValueConstraint`, `BinaryProductConstraint` (`src/construct/*.rs`). No second representation in canonical state; per-construct preference lives only in `ConstructEntry.preference` (A29). |
| 2 | Complete origins: every generated bridge entity carries `EntityOrigin::Construct { construct, role }` | ✓ VERIFIED | `BridgeFinalizer::add_variable`/`add_row` (`src/compiler/bridge/mod.rs`) record `EntityOrigin::Construct` for every generated entity; `BackendSnapshotBuilder::finalize` rejects any unoriginated entity with `CompileError::MissingOrigin` (D5, `src/compiler/backend_ir.rs:870-881`). Per-construct `GeneratedRole` variants present in `src/compiler/origin.rs`. Tests: `every_generated_entity_carries_construct_origin`, `absolute_value_every_generated_entity_carries_construct_origin`, `binary_product_every_generated_entity_carries_construct_origin`. |
| 3 | Exact portable formulation: exact, never a silent relaxation; zero-binary epigraph/hypograph rows distinct from exact | ✓ VERIFIED | Min/max: exact selectors (`y ≥ x_i`, `Σz_i=1`, `y ≤ x_i + M_i(1-z_i)`) vs zero-binary `MinMaxEpigraphRow`/`MinMaxHypographRow` (D13); abs/positive-part/clamp exact decompositions with `M_p = max(U,0)`, `M_n = max(-L,0)`, composed clamp selectors (no one-sided `z ≥ x, z ≥ -x`); products exact rows (binary-binary `w≤a,w≤b,w≥a+b−1`; binary×linear 4 rows). D13 difference proof test `minmax_exact_vs_one_sided_feasible_sets_differ_with_no_objective` passes with no objective. |
| 4 | Explicit failure when bounds/support are insufficient | ✓ VERIFIED | `CompileError::UnboundedBigM { construct, expression }` (SM-13.4, D12) for no finite Big-M — verified in indicator (`indicator_insufficient_bounds_returns_unbounded_big_m`), minmax (`minmax_exact_rejects_unbounded_operand_with_construct_aware_error`), abs (`absolute_value_unbounded_at_compile_returns_construct_aware_error`), product (`binary_product_unbounded_linear_operand_returns_construct_aware_error`). `UnsupportedFeature` under unqualified feature / NativeRequired; continuous×continuous product → typed `ModelError::ContinuousTimesContinuousProduct` with no compiled entities (SM-12.7, D23). No silent default constant exists (`record_bound_evidence` records "unbounded (no finite Big-M)"). |
| 5 | Deterministic bound analysis (SM-13.1/13.6) | ✓ VERIFIED | `BoundAnalyzer` (`src/compiler/bounds.rs`) deterministic linear interval propagation over coefficient signs/constants/fixed/infinite/parameter classes, terms sorted by var; NaN/non-finite → typed `BoundError`; no auxiliary LP ever run. Tests in `tests/compiler_bridges.rs` (15) + in-crate suites. |
| 6 | Logical constructs reject invalid inputs | ✓ VERIFIED | Non-binary activators → `NonBinaryVariable`; duplicate cardinality → `DuplicateCardinalityVariable`; invalid k → `InvalidCardinalityK`; continuous exact reification without separation → `ContinuousReificationWithoutSeparation`. All tested in `tests/common_constructs.rs`. |
| 7 | Algebraic constructs reject invalid inputs | ✓ VERIFIED | Continuous×continuous → `ContinuousTimesContinuousProduct` (no construct/rows added); non-binary product operands → `NonBinaryVariable`; unbounded abs/minmax operands → `UnboundedConstructExpression`/`UnboundedBigM`; invalid clamp bounds (`lower>upper`/non-finite) → `InvalidClampBounds`; trivially-satisfiable min+Epigraph/max+Hypograph → `TriviallySatisfiableMinMax`. Tested. |
| 8 | Exactness is never inferred from objective context (D13) | ✓ VERIFIED | `minmax_exact_vs_one_sided_feasible_sets_differ_with_no_objective` proves exact-min set {y=3} vs hypograph-min {y=0 admitted} differ with x1=3,x2=5 fixed and no objective; exact-max vs max-epigraph mirror. |
| 9 | A30 public surface: ConstructKind/ConstructEntry public; fixture scaffolding absent from cargo public-api | ✓ VERIFIED | `pub mod construct` + crate-root re-exports in `src/lib.rs`. `cargo public-api -p roml` (18819 items) — grep counts: `ConstructKind::Fixture` 0, `FixturePayload` 0, `add_construct_fixture` 0, `ConstructData` 0. `Fixture`/`FixturePayload` are `#[cfg(test)]`-gated. `#[non_exhaustive]` boundary on `ConstructKind` stays. |
| 10 | Capability declarations honest (SupportLevel::Bridge, no false native claims) | ✓ VERIFIED | `roml-highs/src/session.rs`: 7 P32 construct features declared `SupportLevel::Bridge` (`BRIDGE_SUPPORTED_M3_FEATURES`); no unqualified native claims (SM-04.3); unqualified M3 features declared `Unsupported`. WR-01 gate fix: `boolean::compile`/`cardinality::compile` call `select_path` → `boolean_unqualified_feature_is_unsupported`, `cardinality_unqualified_feature_is_unsupported`, `boolean_and_cardinality_reject_under_native_required` all pass. WR-02 Mip gate: `construct_generated_binaries_require_mip_gate` + `zero_binary_construct_compiles_without_mip` pass. |
| 11 | Small-binary-domain feasible-set equivalence (semantic/reference/native/portable) | ✓ VERIFIED | `tests/common_constructs.rs` enumerates semantic, hand-written reference, native, and portable feasible sets for indicator/reification/boolean/cardinality/minmax/abs/product and asserts equality (e.g. `indicator_feasible_sets_semantic_reference_native_portable_equal`, `reification_feasible_sets_semantic_reference_portable_equal`, exact-selector, abs/positive-part/clamp, binary-binary/binary-times-linear enumeration). `roml-highs/tests/formulation_equivalence.rs` (6 tests) solves fixed-probe models on bundled HiGHS and asserts Auto (bridge) and Portable feasible sets equal semantic sets. |
| 12 | Phase gate verification commands all exit 0 (incl. rustdoc with warnings denied) | ✗ FAILED | `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` exits **101** — 3 unresolved intra-doc links to the `#[cfg(test)]`-gated `FixturePayload` in `src/construct/mod.rs` (module doc) and `src/advanced.rs:130`. See Gaps. |

**Score:** 11/12 truths verified (1 present-behavior-unverified: 0).

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | --------- | ------ | ------- |
| `src/compiler/bounds.rs` | Interval/BoundTrace/BoundAnalyzer/one-sided Big-M/UnboundedBigM | ✓ VERIFIED | 1014 lines; deterministic propagation, NaN rejection, WR-04 fail-closed overflow fix, in-crate tests. |
| `src/compiler/bridge/{mod,indicator,reification,boolean,cardinality}.rs` | Bridge finalizer + 4 logical bridges | ✓ VERIFIED | Exact rows; origins; bound evidence; WR-01 gates; reification two implications with unit gap only from proven integrality. |
| `src/construct/{mod,indicator,reification,boolean,cardinality}.rs` | Exact payloads + ConstructKind | ✓ VERIFIED | One semantic definition each; `#[non_exhaustive]`; Fixture cfg(test)-gated (A30). |
| `src/compiler/bridge/{minmax,absolute,product}.rs` | Algebraic exact bridges | ✓ VERIFIED | Exact selectors, abs/positive-part/clamp decompositions, product rows; no relaxation. |
| `src/construct/{minmax,absolute,product}.rs` | MinMax/AbsoluteValue/BinaryProduct payloads | ✓ VERIFIED | Explicit `MinMaxRelation`, `AbsoluteValueVariant`, `ProductOperand`. |
| `src/lib.rs` | A30 public construct exports | ✓ VERIFIED | `pub mod construct`; crate-root re-exports; fixture scaffolding absent from public-api. |
| `tests/compiler_bridges.rs` | Interval/Big-M/bridge-finalization tests | ✓ VERIFIED | 15 integration tests passing. |
| `tests/common_constructs.rs` | Validation/payload/compilation/enumeration tests | ✓ VERIFIED | 64 tests passing incl. D13 proof, CR-01, WR-01/02/03, IN-01/02/03 coverage. |
| `roml-highs/tests/formulation_equivalence.rs` | HiGHS reference-vs-portable equivalence | ✓ VERIFIED | 6 tests passing (solver-based). |
| `docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md` | Baseline + per-task evidence + review dispositions | ⚠️ WARNING | Missing review-fix dispositions / final gate / post-WR-05 public API diff (see Gaps). |

### Key Link Verification

| From | To | Via | Status |
| ---- | --- | --- | ------ |
| BridgeFinalizer | EntityOrigin::Construct | `add_variable`/`add_row` + `finalize` MissingOrigin | WIRED |
| Bridge | CompileError::UnboundedBigM | `bound_big_m_implied`/`derived_big_m`/`validated_explicit_big_m` | WIRED |
| construct dispatch | bridges | `compile_snapshot` match arms (7 kinds, deterministic construct-id order) | WIRED |
| boolean/cardinality bridges | capability gate | `select_path(BackendFeature::Boolean/Cardinality)` | WIRED (WR-01 fixed) |
| session | Mip gate | `generated_has_integer` → `require_feature(Mip)` | WIRED (WR-02 fixed) |
| Model builders | payload storage | `add_indicator/add_reify/add_boolean/add_cardinality/add_minmax/add_absolute_value/add_binary_product/add_binary_times_linear` | WIRED |
| HiGHS caps | SupportLevel::Bridge | `BRIDGE_SUPPORTED_M3_FEATURES` → `highs_capability_set` | WIRED |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| roml full suite | `cargo test -p roml --all-targets` | 773 passed; 0 failed | ✓ PASS |
| roml-highs full suite | `cargo test -p roml-highs --all-targets` | 121 passed; 0 failed | ✓ PASS |
| roml clippy -D warnings | `cargo clippy -p roml --all-targets -- -D warnings` | exit 0 | ✓ PASS |
| roml-highs clippy -D warnings | `cargo clippy -p roml-highs --all-targets -- -D warnings` | exit 0 | ✓ PASS |
| fmt | `cargo fmt --all -- --check` | exit 0 | ✓ PASS |
| focused bridge tests | `cargo test -p roml --test compiler_bridges` | 15 passed | ✓ PASS |
| focused construct tests | `cargo test -p roml --test common_constructs` | 64 passed | ✓ PASS |
| HiGHS equivalence | `cargo test -p roml-highs --test formulation_equivalence` | 6 passed | ✓ PASS |
| roml docs -D warnings | `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | exit **101** — 3 broken intra-doc links | ✗ FAIL |
| roml-highs docs -D warnings | `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | exit 0 | ✓ PASS |
| public API (A30) | `cargo public-api -p roml` | exit 0; 18819 items; `ConstructKind::Fixture`/`FixturePayload`/`add_construct_fixture`/`ConstructData` absent | ✓ PASS |

### Requirements Coverage

| Requirement | Description | Status | Evidence |
| ----------- | ----------- | ------ | -------- |
| SM-12.1 | indicators one-way, native or exact bridge | SATISFIED | `indicator.rs` bridge; Auto native/bridge selection tests |
| SM-12.2 | reification separation unless integrality proves complement | SATISFIED | two-implication bridge; unit gap only from proven integrality; CR-01 integral-threshold validation |
| SM-12.3 | exact min/max distinct from epigraph/hypograph | SATISFIED | `MinMaxRelation`; D13 difference proof; zero-binary one-sided rows |
| SM-12.4 | abs/positive-part/clamp explicit exact semantics | SATISFIED | `AbsoluteValueVariant`; exact decompositions; `InvalidClampBounds`/`UnboundedConstructExpression` |
| SM-12.5 | Boolean/cardinality exact linear formulations | SATISFIED | exact rows; feasible-set equality tests |
| SM-12.6 | exact products limited to binary-binary / binary×bounded-linear | SATISFIED | `ProductOperand`; builder validation; exact rows |
| SM-12.7 | continuous×continuous not mislabeled exact | SATISFIED | typed `ContinuousTimesContinuousProduct`; no compiled entities; no relaxation |
| SM-12.8 | stable handles/results + formulation diagnostics | SATISFIED | builders return `(Construct, VarId)`; report `*.representation`/`*.path`/bound-evidence decisions |
| SM-13.1 | deterministic interval analysis | SATISFIED (P32 clause-level; full closure P33) | `BoundAnalyzer` + tests |
| SM-13.2 | Big-M requires finite derived/validated value | SATISFIED | `bound_big_m_implied`/`validated_explicit_big_m`; `UnboundedBigM` |
| SM-13.3 | explicit M validated against bounds | SATISFIED | `validated_explicit_big_m` rejects below-derived-min |
| SM-13.4 | compile errors identify construct + expression | SATISFIED | `CompileError::UnboundedBigM { construct, expression }` |
| SM-13.5 | reports record M values/derivations/sources | SATISFIED | `FormulationDecision::bound_evidence`; `minmax.selector_m.*`, `product.m_*`, `absolute.m_p/m_n` |
| SM-13.6 | no silent auxiliary LP for tightening | SATISFIED | bounds derived from declared bounds/coefficients only |
| SM-01.3 | stable handle, metadata, activity, parameter-dependency | SATISFIED | `ConstructEntry`; `Model::construct_parameter_dependencies` (incl. set-threshold deps, WR-03) |
| SM-02.3 | names/descriptions/groups/tags/source metadata | SATISFIED | `set_metadata(EntityRef::Construct(..))`; metadata store keyed by `EntityRef::Construct` |
| SM-02.5 | every generated entity maps to construct/overlay role | SATISFIED | `EntityOrigin::Construct`; `finalize` MissingOrigin; tests |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| `src/construct/mod.rs` | module doc (~4, 11, 20) | Broken intra-doc link to `#[cfg(test)]`-gated `FixturePayload` | 🛑 BLOCKER | `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` exits 101; phase gate mandates exit 0; SUMMARY/evidence claim clean docs (false) |
| `src/advanced.rs` | ~130 | Broken intra-doc link to `crate::construct::FixturePayload` | 🛑 BLOCKER | same |
| `docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md` | end | Evidence not updated with review dispositions / final gate / post-WR-05 public API diff; recorded verification claims stale | ⚠️ WARNING | Plan requires reviewer findings/dispositions recorded before gate marked pass |

### Gaps Summary

The substantive P32 construct library is verified: all 7 constructs have one exact semantic definition each, complete `EntityOrigin::Construct` origins enforced at finalization, exact portable formulations (exact selectors, zero-binary one-sided rows, abs/positive-part/clamp decompositions, exact product rows) with the D13 difference proof, explicit `UnboundedBigM`/`UnsupportedFeature`/typed-validation failures, honest HiGHS `SupportLevel::Bridge` capability declarations, A30 public surface (fixture scaffolding absent from public API), and small-binary-domain feasible-set equivalence on the core enumeration tests and on HiGHS solves. All test/clippy/fmt/public-api lanes pass (roml 773, roml-highs 121, clippy clean both, public-api 18819 with fixture scaffolding absent).

**Two gaps block the phase gate:**

1. **BLOCKER — roml rustdoc build fails under `-D warnings`.** `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` exits 101. The WR-05 fix (06b6465) cfg(test)-gated `FixturePayload`/`ConstructKind::Fixture` but left three intra-doc links to `FixturePayload` in `src/construct/mod.rs` (module doc) and `src/advanced.rs:130`. In non-test builds the target does not exist, so rustdoc fails. The phase's Gate section explicitly requires "rustdoc with warnings denied" to exit 0, and the SUMMARY/evidence file claims it is clean — both false for the current tree. The REVIEW-FIX.md gate list omitted the doc build, so the regression slipped through. Fix: reword/escape the three doc links (or `#[doc(cfg(...))]` the references) so the doc build exits 0, then re-run the full phase matrix.

2. **WARNING — evidence file not updated with the review-fix dispositions.** The PLAN Gate requires reviewer findings and dispositions recorded in `docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md`; the file's last write is the Task 17c commit. The 9 review findings, their dispositions, the fix commits (f3002a9..0498794), the final gate matrix (roml 773 / highs 121), and the post-WR-05 public API diff (18819) are documented only in `32-REVIEW.md`/`32-REVIEW-FIX.md`, not the evidence sink. The evidence file's recorded verification claims (doc clean, roml 759) are stale.

**Residual risks / out-of-scope disclosures (recorded correctly):** full SM-13 closure (PWL intervals, curvature, bound-source traces on P33 fixtures) is explicitly deferred to P33; `examples/m3_constructs.rs` and full public reporting are deferred to P34 Task 19; native-constraint IR emission for a qualified native indicator is unreachable in practice (P26 IR has no native-constraint representation; no P32 backend declares native). `roml-mosek`/`roml-xpress` remain out of scope (M2 convention).

---

_Verified: 2026-08-03_
_Verifier: Claude (gsd-verifier)_


## Post-verification resolution

The single blocker (rustdoc -D warnings exit 101 from cfg(test)-gated FixturePayload intra-doc links) was fixed in `2520c0b` (links reworded to code spans); `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` and `-p roml-highs --no-deps` both exit 0 (re-verified). Evidence dispositions recorded; final matrix: roml 773 / roml-highs 121, clippy clean.
