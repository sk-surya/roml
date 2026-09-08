# MPY Final Report — ROML Python Interface (DRAFT)

Branch: `mpy-python-interface` (implementation PR pending; DO NOT MERGE
without separate authorization).
Base: P34 closure + prerequisite merges (see packet).

## Head under review

branch `mpy-python-interface`, PR #53 (draft, reviewable, DO NOT MERGE
without separate authorization). The exact head under review is recorded
in the PR body (editing the body does not move the head). Base: `main`
post-P34 with planning PR #50 merged.

## What was built

PyO3 + maturin interface (`roml-python` 0.1.0) per DESIGN/PLANS:

- MPY-01: distribution skeleton, import from built wheel.
- MPY-02: scalar API, owner-bearing handles, error hierarchy,
  preliminary solve, golden LP, ergonomics review (P1s fixed).
- MPY-03: shaped arrays/CSR, atomic updates, overflow pre-validation,
  39 tests, interface review (P0 + P1s fixed, incl. a reviewer-caught
  `add_array` template gap with red/green proof).
- MPY-04: detached solves, outcomes, warm starts, LP-only duals,
  metadata, lifecycle review (P1s + P2s fixed).
- MPY-05: BESS MPC, highspy oracle, benchmark matrix.
- MPY-06: typing, wheels, sdist, docs (this report).

## Verification (all green, commands in packet)

- `cargo fmt/clippy(-D)/test/doc` for `roml-python`: clean.
- `pytest python/tests` (65+ tests incl. typing-adjacent): green from
  installed release wheel outside the tree.
- `mypy --strict` on examples + typing fixture: clean.
- sdist rebuild → wheel → full suite green in a clean venv.
- Wheel contents: only `roml/` package + dist-info (no leakage).

## Performance evidence

Canonical release-wheel numbers, quiet host (see
`evidence/PERFORMANCE_NOTES.md` and benchmark JSONs):

| Workload | Arm | p50 |
|---|---|---|
| MPC matched MILP | Python persistent | 4.45 ms/gate |
| MPC matched MILP | Python fresh | 5.99 ms/gate (+33%) |
| MPC matched MILP | highspy persistent | 3.97 ms/gate |
| MPC matched MILP | Rust persistent | 3.98 ms/gate |
| LP-small | Python persistent | 0.64 ms/gate |
| LP-scale non-solve | Python persistent | ~2.95 ms/gate |
| LP-scale non-solve | Rust persistent | ~0.35 ms/gate |

Wrapper overhead on matched MILP: ~0.55 ms/gate (~14% over direct
highspy); gate-by-gate objectives agree exactly. PY-27 gate passes
(2.95 <= 5.53 ms). Bulk: amended contract in `BULK-CONTRACT.md`
(release-measured; end-to-end 1.4x, eliminable-work benefit 2.9x).
Memory soak: PASSES (0.0 MiB/10k after the #54 journal fix).

## Gate dispositions (owner; closed)

1. **Bulk 3x end-to-end — AMENDED AND PASSING** under the restated
   gate (no interpreter loops; vars/rows/eliminable-work rates with
   corrected Amdahl reconciliation in `BULK-CONTRACT.md`).
2. **Memory soak — FIXED AND MERGED (#54).** Count-bounded replay
   journal with snapshot recovery; soak PASSES (0.0 MiB/10k).

## Remediation round 2 (owner independent review, 2026-09-08)

Consecutive updates validated against stale committed values (P1):
the proposed environment now layers accepted-pending updates over
committed values, and expression lowering reads the same effective
view. Backend error categories preserved explicitly (follow-up).
Regressions: consecutive updates, sequential-vs-combined equivalence,
second-update overflow rejection with successful solve after,
lowering-after-pending (scalar and array paths).

## Remediation round 1 (owner independent review, 2026-09-08)

The owner's independent review of #53 found three P1 correctness
blockers and four P2s on the previously-reported head; MPY completion
was reopened. All seven are remediated here with regression tests:

- P1-1 (invalid incumbent after timeout): HiGHS extraction now
  requires native `primal_solution_status == feasible`; buffer
  defaults no longer fabricate primals. Native + binding contract
  tests (either clean absence or genuine feasible incumbent).
- P1-2 (failed bulk insertion mutated the model): constraint and
  objective lowering preflights evaluate coefficient finiteness, so
  overflow rejects before any row installs (lower-then-commit now
  holds). Failed-batch preservation tests.
- P1-3 (valid scalar updates rejected): coefficient-template
  recording centralized (`record_templates` + `record_con_coeffs` /
  `record_obj_coeffs`); every lowering site sets `has_complex_deps`
  consistently. Scalar repricing + overflow tests.
- P2-4 (oversized time limits panicked): checked duration conversion
  at construction and per-call; `1e300` rejects as input error.
- P2-5 (close retained native resources): `close` takes and drops
  the session immediately; idempotent; busy preserved; Rust unit
  test proves destruction with the wrapper referenced.
- P2-6 (incomplete error/metadata contracts): solver failures carry
  structured `category` / `health_effect` / `requires_rebuild` /
  `native_code` attributes; `best_bound` / `relative_gap` exposed as
  honest `None`; compilation identity and effective options in
  metadata.
- P2-7 (0-d arrays unupdatable): shape-preserving 0-d construction
  and update (scalar or 0-d input); `(1,)` stays rejected.

## Known limitations (documented, not hidden)

- Parameter-dependent objective constants rejected explicitly.
- Timing metadata records binding-measured total + sync mode; native
  segments are not separately observable without core hooks.
- Candidate rule is native incumbent evidence: extraction requires
  HiGHS `primal_solution_status == feasible`; buffer contents alone
  never establish feasibility (remediation round 1 corrected the
  earlier values-presence proxy).
- The Python surface exposes primitive-only updates, so every solve
  reports honest `Delta`/`NoChange` synchronization after warmup
  (asserted in `test_mpc.py`); construct-dependent rebuilds are
  unreachable from Python (no P30/PWL surface) and covered by core
  tests, not re-proven here.
- Forecast stream equals the realized stream windowed (perfect
  foresight within horizon); closed-loop economics are smoke signals.

## Install instructions (no publication)

```bash
python -m maturin build --release --locked --manifest-path roml-python/Cargo.toml --out dist
python -m pip install dist/roml_python-*.whl "numpy>=2.0"
python -m pytest python/tests -q            # from outside the tree
python python/examples/bess_mpc.py
```

No packages published, no releases created, no merges performed
without authorization. Implementation PR stays reviewable.

## Final qualifications (owner, 2026-09-07; re-verified 2026-09-08)

- **Oracle tests:** `importorskip` is appropriate where highspy is
  optional, but mandatory qualification CI now fails if `test_mpc`
  skips (`Assert oracle tests ran` step in `ci-python.yml`).
- **Tested commit:** six-cell matrix green on `6d1df58`; the delta to
  the branch head is planning-docs only (verified:
  `6d1df58..b6b0e16` touches `STATE.md` alone, 2 lines).
- **Cleanup:** delete only exact task-created directories (allocate
  with `mktemp -d`); `/tmp/*` is not an ownership boundary.
