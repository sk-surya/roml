# M3 Final Qualification (P34 Task 34-11)

**P34 branch:** `phase-roml-P34-m3-qualification` (head recorded at PR).
**Baseline:** post-P31 `main@4cbe13c`.
**Contract:** `34-QUALIFICATION-CONTRACT.md` §7 — every conjunction below
carries affirmative evidence. No publication, tag, or release is implied.

## Predicate evaluation

| Gate | State | Evidence |
|---|---|---|
| P25–P29 accepted | PASS | Merged; leaf ledger rows with phase evidence |
| P30 accepted | PASS | PR #47 `28a019e` |
| P31 accepted | PASS | PR #49 `4cbe13c`; 3 owner review rounds + independent CLEAR re-review (0 P0/P1) |
| P32, P33 accepted | PASS | Merged; evidence `M3_P32_COMMON_CONSTRUCTS.md`, `M3_P33_PIECEWISE_LINEAR_BOUNDS.md` |
| P35, P36 accepted | PASS | PR #44 `7159fad`, PR #46 `8838eff` |
| Every leaf SM/MPS-W/M3-C PASS | PASS (126/127; SM-15.3 closed by this document) | `34-REQUIREMENT-LEDGER.md` |
| Executable fault matrix 20/20 | PASS | `34-FAULT-MATRIX.md` + `.csv`; suites green |
| Native/portable/backend-version matrix | PASS | Q-corpus native 14 + core 14 green; bundled HiGHS 1.15.0; system 1.9.0 floor via `Test (system, ubuntu)` CI lane; ReferenceBackend formulation checks |
| Performance gate | PASS | `M3_PERFORMANCE.md`: +0.187 ms vs 0.470 ms allowance |
| Packed consumers/packages | PASS | `scripts/p34-packed-consumers.sh` ALL PASS (log retained); `cargo package -p roml` verified; roml-highs limited only by unpublished-roml resolution (documented) |
| Public examples/docs/rustdoc/API/package | PASS | rustdoc `-D warnings` clean; `M3_P34_public_api_*.txt`; CHANGELOG; examples compiled in CI lanes |
| NLP readiness, no BLOCKED | PASS | `M3_NLP_READINESS.md`: 0 `BLOCKED_REPLACEMENT_REQUIRED` |
| Independent reviews, zero P0/P1 | PASS | P34 gauntlet: formulation (0 P0/P1, 4 P2 fixed), native boundary (0/0, 3 nits fixed), evidence integrity (0 P0, 2 P1 staleness fixed), NLP trace (independent), API coherence (executor) |
| Exact-head hosted mandatory CI | PENDING (runs on the P34 PR) | Core/MSRV/HiGHS/Coverage/Quality/Policy lanes |
| Owner-authorized P34 merge | PENDING | separate authorization; no bypass |

## Residuals (bounded, approved by contract terms)

- System-HiGHS lanes beyond Linux are explicit non-blocking (§3.2).
- No qualified native relaxation/multiobjective providers; portable paths normative.
- MOSEK/Xpress remain unpublished/experimental until independently qualified.
- Packed crates ship SPDX license fields; no LICENSE text files (owner decision).
- `roml` examples excluded from the packed crate per include policy
  (`cargo package` notes `examples/mps_write.rs`).

## SM-15.3 closure

This document is the P34 check-matrix record: focused/full/cross-platform
intent is executed via the exact-head hosted CI on the P34 PR;
rustdoc/public-API/package/fresh-consumer checks are evidenced above.
**SM-15.3: PASS** conditional on the pending CI row going green.
