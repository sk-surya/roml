# M3 NLP Readiness (P34 Task 34-09)

**Head traced:** see researcher record below (P34 branch, 2026-09-07).
**Method:** independent read-only trace of N1–N4 through all 16 contract
components with grep-verified file:line seams. No implementation changed.
**Verdict: zero `BLOCKED_REPLACEMENT_REQUIRED`.** M3 closure is not blocked
by NLP readiness; all 7 bounded amendments per shape name an exact owner
and blast radius.

```text
N1 convex QP objective:      min 0.5 x'Qx + c'x + k s.t. Ax <= b, Q PSD
N2 convex quadratic constraint: min c'x s.t. x'Qx + a'x <= b, Q PSD
N3 nonconvex bilinear:        min x*y s.t. linear bounds
N4 smooth nonlinear parameterized: min exp(x) + (y-p)^2 s.t. sin(x)+y <= 1
```

| Component | N1 | N2 | N3 | N4 |
|---|---|---|---|---|
| ScalarFunction | A | A | A | A |
| ScalarSet | A | A | A | A |
| function-in-set constraint | A | A | A | A |
| parameter dependency graph | A | A | A | A |
| canonical snapshots | B | B | B | B |
| deltas | B | B | B | B |
| backend IR | B | B | B | B |
| compiler recipes/report | B | B | B | B |
| capability registry | A | A | A | A |
| origin map | A | A | A | A |
| lineage/instance/revision | A | A | A | A |
| CompilationId | A | A | A | A |
| SolvePlan / overlays | B | B | B | B |
| assignments/starts/hints | A | A | A | A |
| ObjectivePolicy | A | A | A | A |
| IIS/relaxation reporting | B | B | B | B |
| file-I/O boundary | B | B | B | B |

`A` = READY_ADDITIVE, `B` = READY_WITH_BOUNDED_M4_AMENDMENT (9xA / 7xB per
shape). Bounded amendments with owners and blast radii:

- **Snapshots:** owner `src/snapshot.rs` (`ModelSnapshot`/`CellEntry`) +
  `src/model/coefficient.rs` (`CellKey`); add quad-term keys/storage and
  nonlinear reconstruction arms; blast radius confined to snapshot struct,
  ordering, and lowering (N4 also needs evaluated-value/derivative caches).
- **Deltas:** owner `src/delta.rs` (`ModelOp`, reconstruction); new quad /
  nonlinear ops; unknown ops already fail safe to `RebuildRequired`.
- **Backend IR:** owner `src/compiler/backend_ir.rs`; new payload variants
  + op arms + validation/registry; N3 must never route through the
  `BinaryProduct` MILP bridge (already blocked by typed rejection);
  N4 needs an evaluation/derivative payload interface.
- **Recipes/report:** owner `RecipeFingerprint` + `CompilationReport`;
  version bump + new payload families; evidence-only blast radius
  (fingerprints are never stale-state authority).
- **SolvePlan/overlays:** owner `src/solver/overlay.rs` (`OverlayOp`) +
  session lowering; new quadratic temporary variants; `SolvePlan` itself
  unchanged; overlays never advance canonical revision.
- **IIS/relaxation:** owner `src/solver/infeasibility.rs`
  (`ConflictAtomKind::FunctionInSet` already reserved) +
  `src/compiler/restriction.rs` + `src/solver/relaxation.rs`; N3/N4 must
  label local-vs-global scope.
- **File I/O:** owner `src/io/mps/*`; N1/N2 `QMATRIX`/`QUADOBJ` currently
  honest typed rejections (parse + map later); N3/N4 need a format decision.

Safety facts established: no quadratic/nonlinear expression type exists
(nothing falsely labeled convex or MILP-bridged); continuous-times-
continuous is a typed build error; MPS quadratic sections are typed
rejections; unknown delta ops fail safe. Full researcher record with
file:line citations is retained out-of-tree with the P34 evidence set.

**SM-15.7: PASS.**
