# M3 Completion Requirements

This file adds only requirements introduced by the post-P35 completion program. Original M3 leaf IDs in `REQUIREMENTS.md` remain authoritative for P30/P31/P34 semantics.

## MPS-W — P36 deterministic write-back

- **MPS-W01 — Public write seam.** `roml::io::mps` exposes one solver-free semantic writer separate from `Model`; successful export consumes canonical ROML state and never requires a backend. Compiled-formulation export is not a P36 public option.
- **MPS-W02 — Free-MPS deterministic output.** For one canonical evaluated model state/options tuple, repeated P36 writes produce deterministic free-MPS bytes on the same ROML version. Entity/section order, vector names, marker placement, whitespace, and finite floating formatting are deterministic and do not depend on raw arena/debug IDs.
- **MPS-W03 — Representability.** P36 supports the frozen primitive linear LP/MILP representability matrix in `36-CONTRACT.md` and rejects every unsupported active construct/domain/feature with a typed entity-specific error. It never silently exports a weaker formulation.
- **MPS-W04 — Objective semantics.** Min/max sense, objective coefficients, and objective offset round-trip according to P35's frozen standard MPS offset convention.
- **MPS-W05 — Constraint semantics.** Equality/lower/upper/ranged rows serialize so `MpsReader` reconstructs the same normalized active row bounds and coefficient cells. Ranged constraints remain one semantic constraint.
- **MPS-W06 — Variable-domain semantics.** Continuous/integer/binary bounds, free variables, fixed variables, persistent-fixing exact lowering, integer marker defaults, and explicit BOUNDS records reconstruct the same normalized feasible domains according to `36-CONTRACT.md`; unsupported semi domains reject.
- **MPS-W07 — Numeric and parameter safety.** Parameterized supported values are evaluated from one exact `(ModelInstanceId, ModelRevision)` snapshot and recorded in the report. Unbound/stale/non-finite evaluation and non-finite coefficients/objective values reject before path commit. Negative zero is normalized; formatting is locale independent.
- **MPS-W08 — Path transaction.** `write_path` obeys frozen `CreateNew`/`AtomicReplace` semantics on supported platforms, never uses remove-then-rename replacement, preserves path/stage/cause plus cleanup error, and is covered through an internal fault-injection seam. Stream writes document possible partial output.
- **MPS-W09 — Independent semantic round trip.** Deterministic fixtures and randomized legal primitive LP/MILP models satisfy independently normalized evaluated-mathematical `Model == read(write(Model))` semantics without reusing writer projection/normalization helpers.
- **MPS-W10 — Native structural differential.** Native HiGHS `readModel(write(Model))` agrees with direct ROML projection on dimensions, matrix cells, row/column bounds, integrality, objective sense/coefficients/offset under frozen tolerances.
- **MPS-W11 — Solve differential.** Direct ROML->HiGHS and MPS->native-HiGHS paths obey the frozen normalized termination-class and objective comparison rules. Ambiguous/unknown statuses are never strengthened.
- **MPS-W12 — Exact Netlib transcode.** The exact 94-file manifest pinned in `36-NETLIB-MANIFEST.md` must be present at the exact corpus SHA and produce 94/94 required deterministic-write, ROML-structure, and native-structure PASS results. Missing corpus/file, manifest drift, parser rejection, writer rejection, or unresolved mismatch is qualification failure.
- **MPS-W13 — Source-layout non-goal.** P36 never claims preservation of comments, original fixed/free layout, record order, duplicate spelling, original rim-vector names, parameter graph identity, fixing provenance, or byte identity to source input.
- **MPS-W14 — Package/CI.** Core remains solver-free; normal tests work without initialized corpora; qualification explicitly initializes the exact corpus; exact-head Core/MSRV, HiGHS, Coverage, Quality, Policy, docs/package/review gates pass before merge.

## Completion-program execution requirements

- **M3-C01 — Sequential authorization.** PR #45 must be accepted/merged before P36 production starts. Then P36, P30, P31, and P34 production implementation execute sequentially; each successor remains unauthorized until its predecessor is accepted/merged and state is updated.
- **M3-C02 — Independent review.** Every production phase requires task/wave review plus an independent full review before owner merge; zero unresolved P0/P1 is necessary but does not replace affirmative qualification gates.
- **M3-C03 — Evidence authority.** Completion claims cite exact SHA, leaf requirement IDs, commands/hosted CI, backend/version/OS scope, independent/reference/differential results, review disposition, and residual risk.
- **M3-C04 — No guarantee inflation.** Native/heuristic/local/limited/unknown results are never labeled with a stronger ROML mathematical/global guarantee than evidence supports; operational failures remain errors rather than mathematical outcomes.
- **M3-C05 — M4 gate.** Quadratic/nonlinear production implementation does not begin until P34's full positive closure predicate passes, P34 is owner-merged, and the NLP-readiness evidence explicitly approves the extension seams.
