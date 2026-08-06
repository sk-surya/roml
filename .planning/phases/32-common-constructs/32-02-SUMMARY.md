---
phase: 32-common-constructs
plan: 02
subsystem: compiler
tags: [constructs, minmax, absolute, product, indicator, reification, bridge, big-m]
dependency_graph:
  requires: [P32 Plan 01 (BoundAnalyzer, BridgeFinalizer, tests/compiler_bridges.rs), P26 compiler backend IR, P25 construct arena]
  provides: [min/max/absolute/clamp/product construct bridges, logical constructs (indicator/reification/boolean/cardinality), Fixture/FixturePayload test gating, exactness fixes F1-F6]
  affects: [P33 (PWL bridges over the same BridgeFinalizer), P30 (soft-constraint slacks), P31 (lexicographic), P34 (qualification)]
tech-stack:
  added: []
  patterns:
    - exact segment-binary formulations (Rule 1): generated rows enforce the construct relationship from current intervals; exact selectors emit the complete selector rows
    - one-sided Big-M rows only where the relation is one-sided; finite validated M or construct-aware UnboundedBigM (never a default constant, D12)
    - bridge dependency graph persisted and enforced: SetParameter/SetVariableBounds/RemoveVariable on a construct dependency returns RebuildRequired before any compiled delta
    - construct selection gated through select_path on native_payloads_available() (F4) — Bridge until native payloads exist, NativeRequired rejects
    - malformed snapshots never zero-substitute missing parameters (F5) — typed MissingConstructParameter/MissingParameter errors
key-files:
  created:
    - src/construct/minmax.rs
    - src/construct/absolute.rs
    - src/construct/product.rs
    - src/compiler/bridge/minmax.rs
    - src/compiler/bridge/absolute.rs
    - src/compiler/bridge/product.rs
    - tests/common_constructs.rs
    - docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md
  modified:
    - src/construct/mod.rs
    - src/model/mod.rs
    - src/lib.rs
    - src/advanced.rs
    - src/compiler/origin.rs
    - src/compiler/session.rs
    - src/compiler/bridge/mod.rs
    - tests/compiler_bridges.rs
    - roml-highs/tests/formulation_equivalence.rs
decisions:
  - Exact formulations always: y = max/min/|x|/clamp/b·f generated with exact segment binaries, never a convex relaxation (SM-12.3/12.4/12.6/12.7)
  - Construct output bounds decoupled from build-time intervals (F2): static safe domains at add time; exact bridge rows enforce the relationship from current intervals at compile time
  - Reification thresholds revalidated at every compilation (F3): fractional threshold → typed NonIntegralReificationThreshold before backend mutation
  - Fixture/FixturePayload/add_construct_fixture #[cfg(test)]-gated (A30, WR-05): absent from cargo public-api (grep 0)
  - Reference backend does not solve MILPs: core randomized direct-evaluation tests verify exact formulations algebraically (existential check over generated binaries); solve-based equivalence runs on HiGHS (formulation_equivalence.rs)
metrics:
  duration: ~2-3 days (multi-task phase; Task 17 follow-up plan)
  completed: 2026-08-03
  tasks: 3 (logical constructs Task 16; min/max Task 17a; absolute/clamp Task 17b; algebraic constructs Task 17c)
  commits: cd204be, 3be5093, 109e0d8 + review-fix commits f3002a9..0498794, 67447aa, 4d46263, 8ac092a, 1b2fef2, d61ccf3, 87cdf13, 2520c0b
status: complete
actuals:
  tokens: ~40000   # evidence doc ~35KB; verify against diff if precision needed
  tasks: 4
  commits: 12
---

# Phase [32] Task [16-17]: Logical and algebraic semantic constructs

Logical constructs (indicator, reification, boolean, cardinality) and algebraic constructs (min/max, absolute value, positive part/clamp, binary product) as exact bridge formulations over the Plan 01 bridge framework. Requirements closed: SM-12.3, SM-12.4, SM-12.6, SM-12.7, SM-12.8 (stable-handle surface), SM-13.2–13.5. This is Plan 02 of P32 (the Task 17 follow-up plan recorded in Plan 01).

## What was built

- **`src/construct/{minmax,absolute,product}.rs`** — `add_minmax`, `add_absolute_value`, `add_binary_product` builders with static safe output domains (UNBOUNDED for exact/epigraph/hypograph minmax, [0,1] binary-binary, UNBOUNDED binary×linear, clamp constants); stable handles for downstream P33.
- **`src/compiler/bridge/{minmax,absolute,product}.rs`** — exact formulations: `y >= x_i` base rows + exact selectors for max (exact/epigraph), min (exact/hypograph), absolute value (two-arm zero-binary), positive part/clamp (one-sided Big-M rows), binary×linear/binary×binary products (linearized selector rows). Selector helpers emit the complete selector (sum-binary + Big-M rows), not just the Big-M part (Rule 1 fix).
- **Logical constructs (Task 16):** indicator, reification, boolean, cardinality bridges with capability gates (WR-01/WR-02), set-threshold parameter deps (WR-03), `validated_explicit_big_m` failing closed (WR-04).
- **Exactness across model evolution (second review round, F1–F6):** bridge dependency graph persisted on `CurrentCompilation.construct_dependencies` and completed centrally; `SetParameter`/`SetVariableBounds`/`RemoveVariable` on a construct dependency → `RebuildRequired` before any compiled delta; rejected deltas never advance `CompilationId`; reification thresholds revalidated at every compilation; `select_path` gates native on `native_payloads_available()`; malformed snapshots never zero-substitute missing parameters (typed `MissingConstructParameter`/`MissingParameter`, `eval_checked`).
- **`src/model/mod.rs` / `src/lib.rs` / `src/advanced.rs`** — public surface wiring; `CompileError::NonIntegralReificationThreshold` (CR-01 fix, build-time typed error).
- **Fixture gating (A30):** `Fixture`/`FixturePayload`/`add_construct_fixture` `#[cfg(test)]`-gated (WR-05), absent from `cargo public-api`; post-review `2520c0b` reworded the three intra-doc `FixturePayload` links left broken in non-test builds (phase verifier caught it as a blocker; resolved and re-verified).
- **`tests/common_constructs.rs`** (28 tests) + **`roml-highs/tests/formulation_equivalence.rs`** — feasible-set enumeration, randomized direct evaluation (algebraic existential check on core), HiGHS solve-based reference-vs-portable equivalence, full-rebuild regressions for bound widening and parameter changes, fixed-seed incremental-after-mutation == fresh-rebuild differential tests.

## Verification

| Command | Result |
|---|---|
| `cargo test -p roml --test common_constructs` | 0 — 28 passed |
| `cargo test -p roml --all-targets` | 0 — 798 passed; 0 failed (re-verification after F1–F6) |
| `cargo test -p roml-highs --all-targets` | 0 — 121 passed |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc` (both crates) | 0 — clean (after `2520c0b`) |
| `cargo fmt --all -- --check` | 0 — clean |
| `cargo public-api -p roml` | 0 — 18,819 items, fixture-free |

## Deviations from plan

1. **Selector helpers emit the complete selector** (Rule 1 bug fix): the clamp bridge's direct helper calls originally omitted the base rows, leaving `z = min(w, hi)` unbounded above. Moved base-row emission into the helpers; min/max behavior unchanged (re-verified).
2. **Exact-selector rows need negated operand coefficients** (`y - x_i` requires `-x_i`); first implementation added them positively. Caught by feasible-set enumeration and randomized direct-evaluation tests.
3. **One-sided minmax output bounds are relation-specific:** exact → `[l_min, u_max]`, max-epigraph → `[l_min, +inf)`, min-hypograph → `(-inf, u_max]`; initial `[l_min, u_max]` for every relation broke the D13 proof for one-sided models.
4. **Dispatch arms for not-yet-wired constructs returned `UnsupportedFeature`** at intermediate commits (exhaustive `ConstructKind` match); each replaced by the real bridge as its task landed.
5. **Test-surface fixes (Rule 1):** `compiled.variables.is_empty()` corrected to count only `Construct`-origin generated variables; integer literals fixed to `f64`.
6. **Public API frozen by Task 17a:** payload types, builders, `GeneratedRole`s, and `BackendFeature`s landed with 17a (needed to keep the crate compiling); 17b/17c wired bridge bodies behind them — public API count (18,783) unchanged 17a→17b→17c.
7. **Reference-backend solve adaptation:** `ReferenceBackend` is a projection/state-tracking backend and does not solve MILPs; core randomized direct-evaluation tests verify exact formulations algebraically, solve-based equivalence runs on HiGHS. Intent preserved, contract respected.

## Known Stubs

- **No native payloads in `BackendConstraint`** (F4): `native_payloads_available()` stays false; constructs are selected/reported only as `SupportLevel::Bridge`; `NativeRequired` on a construct rejects with `CompileError::UnsupportedFeature`. No false native claims (P32 F4 rule).
- **Semi-continuous / PWL / SOS2 remain unsupported** on the bridge surface — P33 scope.

## Self-Check: PASSED

Merged via PR #30 (`538336d`) onto main. Created files verified: `src/construct/{minmax,absolute,product}.rs`, `src/compiler/bridge/{minmax,absolute,product}.rs`, `tests/common_constructs.rs`, evidence appends. Commits verified: `cd204be`, `3be5093`, `109e0d8`. Re-verification matrix all exit 0 (roml 798 / highs 121 / both clippy lanes / both doc lanes / fmt / `cargo public-api` 18,819). Full review dispositions (CR-01, WR-01..05, IN-01..03, F1–F6) in `32-REVIEW.md` / `32-REVIEW-FIX.md`.
