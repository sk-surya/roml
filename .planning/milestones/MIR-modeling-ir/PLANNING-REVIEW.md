# MIR Planning Review — 2026-09-10

This review qualifies the planning/governance packet only. No MIR runtime implementation or benchmark claim is made here.

## Repository facts checked

- Planning base is `main@c590692ace5446cc20c7eb91cb8fa0d594a054b0`.
- Current `IdArena` is monotonic, never reuses indices, and tracks staleness per slot.
- Current variable creation journals `VariableAdded`; current parameter creation does **not** journal parameter existence.
- Current `variable_name` / `parameter_name` return borrowed `Option<&str>`, so synthesizing block element strings "on demand" would require an API/storage change.
- Current constant bulk rows canonicalize to one `(target,var)` cell and journal one packed row block.
- Current packed parametric objective storage is SoA and still uses per-parameter reverse-position lists for propagation.
- Current Python packed symbolic objective route exists; MIR-00 must measure whether the flagship expression actually stays on it end-to-end.

## Review findings corrected before bootstrap

1. **Layering:** the draft stored `ParamDepBlock` in core using MIR-03 `ParamView/CoeffView`. Corrected: persisted dependency descriptors are L2/core strided ordinal layouts; L1 only produces witnesses.
2. **Parametric duplicate semantics:** "combine duplicate variables" was unsafe when duplicates use different parameters. Corrected: same-param scales may merge; distinct params in one canonical cell are typed not-packable/general fallback.
3. **Parameter creation semantics:** the draft added `ParameterBlockAdded`/solver `ModelOp`, unlike current scalar parameter creation. Corrected to preserve existing revision semantics.
4. **Block names:** the draft promised synthesized names through current borrowed name APIs. Corrected: block/component names remain compact L1/frontend metadata; no forced core API break.
5. **Dense scaling:** `CoeffView::Dense(NumView)` could not actually defer `α * Dense` without hidden state/copying. Corrected to `Dense{scale, values}`.
6. **Model ownership:** current Python arrays carry an owner and reject cross-model composition. Added the same requirement to shared/native IR.
7. **Self-contained deltas:** p-base storage positions alone cannot satisfy the revision protocol's self-contained `ModelOp` contract. Added immutable canonical-cell topology + self-contained packed patch requirement.
8. **Transaction semantics:** added explicit requirement that `set_parameters_bulk` queues/commits/rolls back consistently with scalar parameter updates.
9. **Dependency authority:** skipping `param_positions` on eligible blocks must not break dependency queries or scalar parameter updates. Added block+sparse union semantics.
10. **Flagship counting:** corrected 300×96 from "57,600 params" to 28,800 price params driving 57,600 objective coefficient cells.
11. **Patch count:** one reprice may affect multiple dependency blocks (e.g. charge/discharge). Replaced "one SetCoefficientBlock per dependency" with one packed coefficient-patch batch per committed parameter block.
12. **MIR-02/high-level dependency:** the draft required automatic BESS eligibility before MIR-03's proof function existed. MIR-02 now qualifies the core path with hand-verified direct fixtures; automatic BESS eligibility becomes an MIR-03 gate.
13. **Cross-frontend equality:** raw byte equality is invalid once model ownership/absolute IDs differ. Replaced with normalized ordinal-IR and semantic-journal fingerprints.
14. **Mixed rows:** added an internal mixed constant+parametric row commit seam so the four-kind IR can lower disjoint mixed cells without allocating the same row twice.

## Three Fable choices reviewed

- **Caller-supplied `ParamDepLayout` in MIR-02:** accepted with a correction: it is an L2 witness, and core must revalidate/resolve it after canonicalization.
- **Deferred dense scaling:** accepted, but encoded explicitly as `Dense{scale, values}`.
- **`ParamView * LinArray` conservative fast subset:** accepted as an initial optimization boundary, extended to state the required constant subset; unsupported forms use the general path and are measured rather than silently broadened.

## Planning verification obligations

Before opening the planning PR, verify:
- packet Markdown fences balance;
- requirement IDs IR-01…IR-32 are unique/contiguous;
- relative links resolve for all repository-known targets;
- D-019 is appended to the single governing `docs/release/ARCHITECTURE_DECISIONS.md`, not added as a competing standalone ADR file;
- root AGENTS/STATE/ROADMAP route MIR ahead of further ergonomics/M4 without claiming MIR runtime work is complete.
