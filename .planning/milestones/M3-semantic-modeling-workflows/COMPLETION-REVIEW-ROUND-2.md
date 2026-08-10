# M3 Completion Packet — Independent Review Round 2 Disposition

**Reviewed prior head:** `b34213e691e3fc43f24dc78ae5142b169c4ed830`  
**Review method:** five parallel read-only agents, owner-reported synthesis.  
**Verdict:** request changes; no P0, multiple P1 written-spec blockers.  
**Production effect:** none. No P36 production branch/code may start before corrected packet acceptance + merge.

## 1. Governance reconciliation — RESOLVED

- Root `.planning/ROADMAP.md`, root `.planning/STATE.md`, and milestone `STATE.md` now agree.
- While PR #45 is open: planned routing target = P36; active production implementation = none; `implementation_authorized=false`.
- P35 is complete; PR #45 merge -> P36 merge -> P30 -> P31 -> P34 is binding.
- P36 is an explicit program dependency for P30.
- One P36 requirement namespace exists: MPS-W01–W14.

## 2. Shared contract freeze — RESOLVED

`SHARED-CONTRACTS.md` freezes:
- lineage/model-instance/revision/CompilationId lifecycle and exact equality authority;
- fingerprints as non-authoritative evidence/cache aids;
- solve-scoped apply/rollback, no canonical overlay commit, dirty state/rebuild, multi-error preservation;
- parameterized MPS export as one evaluated exact snapshot plus environment metadata;
- deterministic export-local naming independent of raw slot/debug IDs;
- objective degradation `scale=abs(z*)`, including zero/negative optima;
- P31 sole canonical `ObjectivePolicy` and `ObjectivePriority(u32)` ownership.

## 3. P36 hardening — RESOLVED

`36-CONTRACT.md` freezes default options, report schema, error taxonomy/context, complete representability matrix, cross-platform path transaction, injection seam, independent oracle, native structural/solve rules, tolerances, and mismatch dispositions.

Additional YAGNI correction: the decorative `CompiledLinearFormulation` option was removed completely; P36 has one semantic writer meaning.

`36-NETLIB-MANIFEST.md` freezes exactly 94 filenames at `sk-surya/lp-data-netlib@56257eea85b433ce6aa67d26156b36385318fd6f`. Missing corpus/file, drift, parser/writer failure, unresolved mismatch, or panic is failure; 94 explicit PASS rows are required.

P36 execution is frozen to:

```text
Wave 0 serial: contract/manifest/public seam
Wave 1 parallel: projection | formatter | path
Wave 2 parallel: bounds | objective/RHS/RANGES | ROML oracle | HiGHS oracle
Wave 3 serial: exact 94-model qualification
Wave 4 serial: docs/package/review/exact-head closure
```

Wave 1/2 production ownership is disjoint in `36-PLAN.md`.

## 4. P30/P31 interface hardening — RESOLVED

P30:
- dedicated `RelaxationProviderPolicy` and `RelaxationAcceptance::{RequireOptimal,AcceptFeasible}`; defaults PortableOnly + RequireOptimal;
- `FeasibleRepair` only under `AcceptFeasible` with a valid feasible incumbent;
- `NoRepairFound` requires proof under scope/caps; limits/numerical/interruption remain `Unknown` absent accepted repair/proof; operational failures remain `Err`;
- numerical/provider/identity report schema fixed;
- P29 row/bound/fixing map is all-or-error; unsupported/stale origins reject;
- parameterized penalty weights resolve finite/nonnegative before mutation;
- primary + cleanup/rebuild failures are preserved.

P31:
- one canonical `ObjectivePolicy` and one `ObjectivePriority`;
- P31 objective weights are finite nonnegative plain `f64` in M3; symbolic objective weights are deferred;
- weighted-level/provider/stage/lock/aggregate-result schemas fixed;
- all weighted stages normalize to minimization and use `g(x) <= z* + abs_tol + rel_tol*abs(z*)`;
- P30 priority variant is added only in P31 and P30 parameterized penalty weights are resolved before stage construction/mutation;
- portable/native provider and continuation/cleanup behavior are explicit.

## 5. P34 qualification hardening — RESOLVED

`34-QUALIFICATION-CONTRACT.md` requires:
1. one row for every leaf `SM-xx.y`, MPS-Wxx, M3-Cxx with evidence/command/backend-OS/review/residual/PASS-BLOCKED;
2. executable fault matrix across canonical/compiler/delta/rebuild/overlay/P29/P30/P31/P36 boundaries;
3. Q01–Q14 fixture corpus; ReferenceBackend; bundled HiGHS 1.15.0 Linux/macOS/Windows; system HiGHS 1.9.0 Linux floor; frozen observables/tolerances/discrepancy rules;
4. deterministic `P34_PRIMITIVE_PARAMETER_UPDATE_V1` (512 vars/rows, 4096 nnz, fixed seed, parameter in 64 cells, 20 warmups/200 measurements, release/one-thread bundled HiGHS), historical baseline `main@4d111cceafce17aea44a6e396a838d1cc9ef255d`, original max(5%,50us) gate;
5. exact package/list assertions and fresh `/tmp/p34-consumer-{core,highs,mps,iis-relax,lexicographic}` packed consumers;
6. N1 convex QP, N2 convex QCQP, N3 nonconvex bilinear, N4 smooth nonlinear parameterized shapes with per-component additive/bounded-amendment/replacement-required verdicts;
7. a positive closure conjunction requiring every mandatory gate to affirmatively pass.

## Final contradiction scan

Four residual ambiguities found after the main pass were also closed:
1. stale P36 writer-target report field removed;
2. stale `SemanticModel` manifest label removed;
3. P31 objective-weight representation frozen to `f64`;
4. P30 feasible-repair discretion replaced by explicit acceptance policy/default.

The top-level design spec was updated to match these final contracts rather than relying on phase-plan precedence.

## Post-round-2 callable API closure

The final P36 contract refinement freezes the previously implicit public methods:

```rust
impl MpsWriter {
    pub fn new() -> Self;
    pub fn with_options(options: MpsWriteOptions) -> Self;
    pub fn write<W: std::io::Write>(&self, model: &Model, output: W) -> Result<MpsWriteReport, MpsWriteError>;
    pub fn write_path<P: AsRef<std::path::Path>>(&self, model: &Model, path: P) -> Result<MpsWriteReport, MpsWriteError>;
}
```

`write` is stream-only and may leave partial bytes; `write_path` alone applies the destination policy. The signatures and semantics are mirrored in `36-CONTRACT.md`, `36-PLAN.md`, and the top-level design. This is a planning correction, not implementation authorization.

## Re-review gate

This disposition does not self-approve PR #45. The corrected exact head must receive independent written-spec re-review. P36 production remains forbidden until that review clears and PR #45 is owner-merged.
