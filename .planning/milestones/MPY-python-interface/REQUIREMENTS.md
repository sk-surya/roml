# MPY Requirements and Acceptance Ledger

Every row is required. The executor expands the Evidence column to exact commands, test node IDs, head SHAs and artifact paths. A planned test is not evidence. Source inspection findings and measured behavior remain separately labeled.

| ID | Requirement | Owner | Acceptance evidence target |
|---|---|---|---|
| PY-01 | Fresh inventory and dependency-aware normal merging of pending prerequisite PRs | MPY-00 | `evidence/PR-INVENTORY.md`; merge SHAs and current checks |
| PY-02 | P31 complete objective accounting, no-candidate outcomes and cleanup | MPY-00 | real HiGHS + fault regressions; independent review |
| PY-03 | P34 integrated qualification and truthful routing | MPY-00 | existing P34 closure predicate; merged closure |
| PY-04 | Explicit P31 options and total staged solve budget | MPY-00 | controlled-clock + native limited-solve tests |
| PY-05 | Python-free, solver-free core and preserved core MSRV | MPY-01 | existing core CI; dependency tree and package audit |
| PY-06 | Pinned compatible PyO3/maturin/NumPy/native dependency set | MPY-01 | `evidence/DEPENDENCIES.md`; lockfiles and build logs |
| PY-07 | One public vocabulary, documented signatures, complete typing | MPY-02 | public stub checks, example execution, typing fixture |
| PY-08 | Owner-safe scalar handles and stale/foreign handle rejection | MPY-02 | cross-model and lifetime tests |
| PY-09 | Scalar affine/parameter math, duplicate algebra and numeric validation | MPY-02 | reference LPs; invalid coefficient and domain tests |
| PY-10 | Symbolic truthiness guard and explicit nonlinear rejection | MPY-02 | chained comparison, bool, multiplication/division tests |
| PY-11 | Named namespace uniqueness and useful bounded repr | MPY-02 | name collision and no-implicit-solve tests |
| PY-12 | Shaped arrays, C order, slices and scalar-only broadcast | MPY-03 | 0-D scalar distinction; 1-D/2-D/empty/mismatch tests |
| PY-13 | Rust bulk array math and dot/sum; no object-array hot path | MPY-03 | scale benchmarks and binding-call inspection |
| PY-14 | Atomic named batch updates preserve earlier pending state | MPY-03 | mixed valid/invalid batch and derived-overflow rollback tests |
| PY-15 | Atomic CSR and array constraint insertion with duplicate accumulation | MPY-03 | malformed CSR, duplicate coefficients, all-or-none insertion |
| PY-16 | Deliberate numeric input ownership, dtype and stride handling | MPY-03 | dtype, read-only, noncontiguous and concurrent-input tests |
| PY-17 | Persistent model-bound HiGHS sessions and per-call options | MPY-04 | multiple revisions; foreign-model rejection; option nonleak |
| PY-18 | Detached native solve and deterministic busy errors | MPY-04 | heartbeat, overlap, independent-session and worker-drop tests |
| PY-19 | No new unjustified unsafe Send/Sync; exact-once native destruction | MPY-04 | lifecycle review, close/error/drop stress tests |
| PY-20 | Immutable results with provenance and honest currentness | MPY-04 | old result survives update; `is_current`; foreign-handle rejection |
| PY-21 | Mathematical outcomes separated from operational errors/incumbents | MPY-04 | infeasible/unbounded/unknown/limits/partial-primal fixtures |
| PY-22 | No missing-as-zero values; valid LP-only diagnostics | MPY-04 | missing result/dual handling and LP sensitivity fixture |
| PY-23 | Native primary/cleanup failures preserved as typed Python errors | MPY-04 | failure injection and error attribute assertions |
| PY-24 | Explicit warm-start request and measured/applied disposition | MPY-04 | same-model valid/rejected start tests; no fabricated reuse claim |
| PY-25 | Honest timing and cancellation limits | MPY-04 | timing metadata; deadline overrun and interrupt documentation |
| PY-26 | Causal synthetic rolling BESS MILP with physical accounting | MPY-05 | 1,000-step replay; native oracle equivalence |
| PY-27 | Cold/warm/bulk/native/rebuild comparison with pinned identical models | MPY-05 | raw benchmark data and reproducible commands |
| PY-28 | Performance and memory thresholds met without weakened semantics | MPY-05 | qualification thresholds; any miss blocks completion |
| PY-29 | Actual synchronization/rebuild and warm-start reporting | MPY-05 | primitive + semantic-dependency fixtures |
| PY-30 | Clean installed wheels on three OS targets and two Python versions | MPY-06 | six installed-wheel cells; native LP/MIP and update smokes |
| PY-31 | Source distribution can build outside repository with path dependencies | MPY-06 | clean extracted-sdist build; wheel content audit |
| PY-32 | Correct license/native packaging and no customer/commercial content | MPY-06 | wheel/sdist manifests; dependency license inventory |
| PY-33 | Public examples, documentation, typing and actionable install errors | MPY-06 | examples run from installed wheel; stub/runtime comparison |
| PY-34 | Final independent review, no open P0/P1, complete traceability | MPY-06 | `evidence/FINAL-REPORT.md`; required CI at final head |
| PY-35 | No publication, tags, live actions or unsolicited breadth | All | diff and action audit |

## Acceptance priority

Correctness and lifecycle safety are release blockers even when performance passes. Slow correct implementation is an incomplete performance milestone, not permission to weaken validation. An unavailable platform is not a passed platform. Core tests alone do not establish Python usability or binary portability.
