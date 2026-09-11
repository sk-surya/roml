# MIR Tranche 1 — Kickoff Prompt (MIR-00 / MIR-01 / MIR-02)

You are implementing tranche 1 of the MIR milestone in `sk-surya/roml`.

Start from the current `main`, not from the planning base blindly. Read in order:

1. `AGENTS.md`
2. `.planning/milestones/MIR-modeling-ir/README.md`
3. `DESIGN.md`
4. `REQUIREMENTS.md`
5. `ROADMAP.md`
6. `IMPLEMENTATION-PLAN.md`
7. `QUALIFICATION.md`
8. `PLANNING-REVIEW.md`
9. `skills/roml-ir-invariants/SKILL.md`
10. `docs/release/ARCHITECTURE_DECISIONS.md` D-019

Use an isolated branch/worktree. One phase at a time. Tests/characterization first. Do not ask the owner to reconfirm already-authorized MIR-00/01/02 work.

## MIR-00 — Baseline before redesign

- Record exact execution-base SHA in `STATE.md`.
- Measure the current LP-scale/BESS construction path and the current `rm.sum(price * (discharge - charge))` lowering route. Do not assume `PackedSymbolic`; prove it.
- Profile repricing **28,800 price parameters** affecting **57,600 objective cells** for 100 cycles.
- Add only the minimal diagnostic counters/probes necessary to distinguish numeric bulk, packed parametric, general affine, reverse-index lookups, overlay evaluations and journal/ModelOp counts.
- Write `evidence/BASELINE.md` with exact commands, raw numbers and source-inspection facts separated from measurements.
- Reconcile this packet against any `main` changes since planning base `c590692ace5446cc20c7eb91cb8fa0d594a054b0` before MIR-01.

## MIR-01 — Trusted block allocation

Implement DESIGN §2–3 / IR-02…IR-07.

Key constraints:
- opaque `VarSpan`/`ParamSpan`; no public `(start,len)` constructor;
- fresh-generation reconstruction only for trusted block spans; deleting one member invalidates only that member;
- variable block: validate once, reserve once, one packed variable-add Change/ModelOp;
- parameter block: validate/reserve/allocate in bulk **without inventing a solver-facing creation op**; preserve current scalar parameter-creation revision semantics;
- do not materialize `x[0]`, `x[1]`, ... strings in core; keep existing borrowed scalar name APIs compatible;
- existing `add_linear_rows_bulk` tests pass unmodified;
- snapshot/delta/backend behavior remains equivalent.

## MIR-02 — Parametric packed construction and block-native propagation

Implement DESIGN §4 and §7 / IR-08…IR-17.

Key constraints:
- packed p-base row path plus retrofit of the existing packed parametric objective path;
- one packed cell represents exactly one `scale × ParamId`;
- duplicate same-var/same-param scales may merge; distinct params into one canonical `(target,var)` are typed not-packable and must fall back without partial mutation;
- `ParamDepLayout` is an L2 witness; core revalidates it after canonicalization before storing a `ParamDepBlock`;
- eligible families do not populate per-cell `param_positions`, but dependency iteration/introspection and scalar parameter updates remain correct;
- `set_parameters_bulk` participates in the existing transaction/commit contract;
- one committed parameter block emits one packed parameter-value change plus one packed coefficient-patch batch containing all affected eligible dependency blocks;
- retained delta is self-contained; no `ModelOp` may dereference mutable live-model p-base positions;
- shadowed/dead p-base cells are skipped correctly;
- direct/core block-shaped fixtures may hand-construct verified layouts in MIR-02. Do **not** fake MIR-03's production eligibility proof merely to make the high-level BESS pass early.

## Rules of engagement

- Use TDD/characterization before refactoring current packed storage.
- Small commits; draft implementation PR; leave reviewable unless separately authorized.
- A fast/general semantic mismatch is P0: stop and report.
- Never weaken stale-ID, owner, transaction, snapshot or revision guarantees for performance.
- Update milestone STATE and requirement evidence after each phase.
- No Sets, `ConcreteModel`, pandas or macro DSL work in tranche 1.
