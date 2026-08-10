# M3 Completion Requirements

This file adds only the requirements introduced by the post-P35 completion program. Original M3 requirement IDs in `REQUIREMENTS.md` remain unchanged and authoritative for P30/P31/P34.

## MPS-W — P36 deterministic write-back

- **MPS-W01 — Public write seam.** `roml::io::mps` exposes a solver-free writer separate from `Model`; successful semantic export consumes canonical ROML state and never requires a backend.
- **MPS-W02 — Free-MPS deterministic output.** For one canonical model state/options tuple, repeated P36 writes produce deterministic free-MPS bytes on the same ROML version. Entity/section order, vector names, marker placement, whitespace, and finite floating formatting are deterministic.
- **MPS-W03 — Representability.** `SemanticModel` export supports the P35 standard linear LP/MILP semantic surface and rejects every unsupported construct/domain/feature with a typed error identifying the blocking entity/feature. It never silently exports a weaker formulation.
- **MPS-W04 — Objective semantics.** Min/max sense, objective coefficients, and objective offset round-trip exactly according to P35's standard MPS offset convention.
- **MPS-W05 — Constraint semantics.** Equality/lower/upper/ranged rows serialize so `MpsReader` reconstructs the same normalized row bounds and coefficient cells. Ranged constraints remain one semantic constraint.
- **MPS-W06 — Variable-domain semantics.** Continuous/integer/binary bounds, free variables, fixed variables, integer marker defaults, and explicit BOUNDS records reconstruct the same normalized domains.
- **MPS-W07 — Numeric safety.** Non-finite coefficients/bounds/objective values reject before semantic path output is committed. Negative zero is normalized. Formatting is locale independent.
- **MPS-W08 — Path transaction.** `write_path` never replaces an existing destination until complete successful serialization. Errors preserve path/operation/cause. Stream writes document possible partial output.
- **MPS-W09 — Semantic round trip.** Deterministic fixtures and randomized legal primitive LP/MILP models satisfy normalized `Model == read(write(Model))` semantics.
- **MPS-W10 — Native structural differential.** For qualification fixtures, native HiGHS `readModel(write(Model))` agrees with direct ROML projection on dimensions, matrix cells, row/column bounds, integrality, objective sense/coefficients/offset under declared tolerances.
- **MPS-W11 — Solve differential.** Where termination/objective comparison is meaningful, direct ROML->HiGHS and MPS->native-HiGHS paths agree on termination class and objective under explicit tolerances.
- **MPS-W12 — Netlib transcode.** All P35-supported Netlib inputs are attempted through `external MPS -> ROML -> ROML MPS -> ROML/HiGHS`. Every non-pass receives a precise classification; zero unresolved semantic discrepancies are allowed among representable models.
- **MPS-W13 — Source-layout non-goal.** P36 never claims preservation of comments, original fixed/free layout, record order, duplicate spelling, original rim-vector names, or byte identity.
- **MPS-W14 — Package/CI.** Core remains solver-free; normal tests do not require initialized corpora; exact-head Core/MSRV, HiGHS, Coverage, Quality, and Policy gates pass before merge.

## Completion-program execution requirements

- **M3-C01 — Sequential WIP.** P36, P30, P31, and P34 production implementation are sequential. Only one is active at a time.
- **M3-C02 — Independent review.** Every phase requires an independent full review after task-level reviews and before owner merge.
- **M3-C03 — Evidence authority.** Completion claims cite exact SHA, commands/CI, differential results, capability/backend versions, and residuals.
- **M3-C04 — No guarantee inflation.** A native/heuristic/local result is never labeled with a stronger ROML semantic/global guarantee than its evidence supports.
- **M3-C05 — M4 gate.** Quadratic/nonlinear production implementation does not begin until P34 closes and NLP-readiness evidence explicitly approves the extension seams.