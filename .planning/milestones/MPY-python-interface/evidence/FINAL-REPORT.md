# MPY Final Report — ROML Python Interface (DRAFT)

Branch: `mpy-python-interface` (implementation PR pending; DO NOT MERGE
without separate authorization).
Base: P34 closure + prerequisite merges (see packet).

## Head under review

`22e838d186e34033817152d7cb02102db011caec` on branch
`mpy-python-interface`, PR #53 (draft, reviewable, DO NOT MERGE
without separate authorization). Base: `main` post-P34 with planning
PR #50 merged.

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
(2.95 <= 5.53 ms). Bulk: vars 3.1x, per-coefficient ~2.3x,
end-to-end fixture ~1.3x (core entity costs dominate — fixture
property, see open gates). Memory soak: FAIL (see below).

## Open gates requiring owner disposition

1. **Bulk 3x end-to-end (QUALIFICATION):** unachievable as specified;
   ~85% of wall time is identical core insertion work in both arms.
   Mechanism + numbers recorded. Options: amend threshold to a
   per-coefficient or no-fixed-cost formulation, or authorize core
   batch-insert work.
2. **Memory soak (QUALIFICATION):** FAILS — unbounded core journal
   retention (~20 KB/committed-change cycle; 203 MiB/10k). Not a
   binding leak (Rust arms identical; highspy flat). Deliberately not
   worked around. Options: amend threshold, authorize a
   journal-bounding (cursor-acknowledged pruning) core design, or
   accept periodic model recycling with explicit semantics.

Neither gate was silently waived; both block MPY completion pending
disposition.

## Known limitations (documented, not hidden)

- Parameter-dependent objective constants rejected explicitly.
- Timing metadata records binding-measured total + sync mode; native
  segments are not separately observable without core hooks.
- Candidate rule is values-presence (HiGHS-verified); exact
  termination-based evidence is a labeled follow-up.
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
