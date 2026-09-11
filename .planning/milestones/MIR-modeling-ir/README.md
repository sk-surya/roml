# MIR — Shared Modeling IR and Block-Native Core Packet

**Owner instruction, 2026-09-10:** before extending Python ergonomics (Sets, `ConcreteModel`, pandas), fix the shared native modeling IR and the block-native construction/update path so native Rust and Python elegant formulations lower through the same ordinal array IR into bulk core operations. MIR precedes further modeling ergonomics and the deferred M4 preview.

**Planning base:** `main@c590692ace5446cc20c7eb91cb8fa0d594a054b0`. MIR-00 must refresh `main` and record its exact execution base before touching runtime code.

**Deliverable:** a shared `roml::modeling` array IR (model-owned strided views over trusted block-allocated ID spans, four initial coefficient kinds), block allocation for variables/parameters, packed parametric rows and objectives, block-native parameter propagation, a Rust Level-1 API with no raw IDs in ordinary user code, Python arrays rewired onto the same IR, and a flagship BESS300×96 repricing qualification.

**Flagship cardinality:** 28,800 mutable price parameters drive 57,600 parameterized objective coefficient cells (charge + discharge); keep those counts distinct in every benchmark and counter report.

## Read order

1. [STATE.md](STATE.md) — actual progress and next gate.
2. [DESIGN.md](DESIGN.md) — normative layering, IR, block/update contracts and fallback rules.
3. [REQUIREMENTS.md](REQUIREMENTS.md) — acceptance ledger.
4. [ROADMAP.md](ROADMAP.md) — phases, dependencies and exit gates.
5. [IMPLEMENTATION-PLAN.md](IMPLEMENTATION-PLAN.md) — tranche-1 MIR-00/01/02 TDD execution plan.
6. [QUALIFICATION.md](QUALIFICATION.md) — flagship benchmark, counters and thresholds.
7. [skills/roml-ir-invariants/SKILL.md](skills/roml-ir-invariants/SKILL.md) — load before touching coefficient storage, lowering or array code.
8. [AGENT-PROMPT.md](AGENT-PROMPT.md) — kickoff instructions for MIR-00/01/02.
9. [PLANNING-REVIEW.md](PLANNING-REVIEW.md) — review findings corrected before bootstrap.
10. [`docs/release/ARCHITECTURE_DECISIONS.md` D-019](../../../docs/release/ARCHITECTURE_DECISIONS.md) — governing architecture decision.

## Authority and scope

MIR is the owner-requested successor to the merged MPY Python interface and the certified post-MPY construction-performance stack. Existing canonical-model, revision, snapshot, stale-ID, solver-independence and native-safety contracts remain binding.

Not authorized: Pyomo source/API compatibility; a Pyomo-style `AbstractModel` subsystem; macros as the Rust modeling foundation; changes to per-entity stale-ID semantics; registry publication; commercial-backend expansion.

No runtime implementation is delivered by this planning PR. API examples are acceptance targets, not claims of existing functions. Evidence is added under `evidence/` as execution produces it.

## Decisions fixed by this packet

- The modeling IR is ordinal. Labels are frontend metadata/boundary translation and never enter expression nodes.
- Array handles retain model ownership; cross-model composition is rejected before ID reconstruction.
- `VarSpan`/`ParamSpan` originate only from trusted block allocation. Views are span + shape + signed strides + offset; slicing is O(1) in cells; no span epoch.
- Core block allocation does **not** materialize per-element names. Component/base names stay compact in the modeling/frontend layer; the existing borrowed scalar name APIs are not broken merely to synthesize array-element strings.
- Variable block creation journals one packed variable-add change/op. Parameter block creation preserves current scalar parameter-creation semantics: bulk arena/storage mutation, but no solver-facing add change/op solely because the parameters exist.
- `LinArray` initially supports coefficient families `{1, α, scaled-dense numeric, α×ParamView}`. Unsupported compositions use the correct general symbolic path.
- The persisted block-dependency descriptor is an L2/core strided-ordinal layout, not an L1 `ParamView`/`CoeffView`; `roml::modeling` produces the witness and core validates it after canonicalization.
- Eligibility is a sink-aware metadata proof of injective canonical `(target, variable)` cells plus a strided post-canonical storage witness. It is conservative, decided once, stored canonically, and never rediscovered during bind/update.
- Block-created parameter dependencies are represented by dependency blocks rather than per-cell `param_positions`; semantic dependency queries remain complete by consulting both block and sparse dependency representations.
- Bulk parameter updates preserve transaction/commit semantics and emit packed, self-contained revision deltas. A parameter-block commit produces one packed parameter-value change plus one packed coefficient-patch batch containing all affected eligible dependency blocks.
- Packed base/p-base remain append-only; fresh canonical blocks append to base even after solves, while mutation of an existing logical cell uses the overlay.
- Rule/callback APIs accumulate into a CSR builder and mutate the model once.
- Data variation is persistent block parameter update (`Template::bind`); only structural variation rebuilds. No `AbstractModel`.
- MIR diagnostics distinguish lowering from propagation and are qualification/debug surfaces, not mathematical modeling semantics.
- One implementation phase active at a time. Tranche 1 is core-first but necessarily touches delta compilation, HiGHS batch application, diagnostics and existing Python benchmark hooks; it adds no Python ergonomics.
