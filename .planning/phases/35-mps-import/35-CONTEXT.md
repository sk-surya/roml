---
phase: 35-mps-import
status: written-spec-round-1-addressed-re-review-pending
owner_priority: pulled-forward
baseline: ff99389b9bf1318c555dc1f72dff6b5c7a4111c0
follow_on: P36-mps-write-roundtrip
---

# Phase 35 Context — MPS Import and External Qualification

## Why this phase exists

Phase 29 delivered solver-agnostic LP infeasibility analysis. The next owner-directed objective is to exercise that functionality on real MPS models rather than only programmatically constructed fixtures.

Immediate external corpora:

- `sk-surya/infeasiblelps`, forked from John Chinneck's infeasible LP repository;
- `sk-surya/lp-data-netlib`, forked from the converted Netlib LP repository.

The feature is production ROML functionality, not a temporary test parser. MPS is the first file-format member of a future `roml::io` layer. Write-back is the immediate follow-on and is designed concurrently so the reader does not create a read-only dead end.

## Owner direction

Binding owner decisions:

1. Start with linear LP/MILP MPS only.
2. Other formats will be added later.
3. MPS write-back is the next step and must be accounted for now.
4. Validate ROML parsing against HiGHS's independent MPS reader.
5. Use the owner's forks of Chinneck and Netlib for reproducible corpus testing.
6. Corpus integration mechanism may be chosen architecturally.

## Phase numbering and routing

The existing M3 roadmap already allocates P30-P34. This work is P35 rather than renumbering those phases. Owner priority pulls P35 forward because it directly qualifies completed P29 functionality. P30-P34 retain their existing contracts.

Roadmap/current-phase routing is not modified on this design branch. Routing changes are implementation-start actions after written-spec approval.

## Repository constraints

- `roml` remains solver-free; core MPS parsing cannot depend on HiGHS.
- HiGHS differential tests live in `roml-highs` or qualification tooling that may depend on both crates.
- Rust MSRV remains 1.85.
- external corpus data remains outside published crate contents.
- production parser code may not rely on corpus availability.
- normal CI must work from a non-recursive clone.

## Corpus pins at design time

```text
sk-surya/infeasiblelps
  97a936498e5240d44adaf7dcfe84877fa34ce301

sk-surya/lp-data-netlib
  56257eea85b433ce6aa67d26156b36385318fd6f
```

Neither upstream repository exposed a root `LICENSE` file at design time. P35 therefore does not copy corpus files into ROML.

## Chosen corpus layout

Planned optional repository-only submodules:

```text
testdata/corpora/infeasible-lps
testdata/corpora/netlib
```

The submodules point to owner forks and exact reviewed SHAs. Ordinary development does not initialize them.

Chinneck collections are archived; materialization is generated test state and follows the mandatory safe-extraction contract in `35-CORPUS-QUALIFICATION.md` and MPS-S05.

## Primary success scenario

```text
external .mps
   |
   v
ROML MpsReader
   |
   v
ROML Model
   |
   +------------------------------+
   |                              |
   v                              v
ROML -> HiGHS solve/IIS      native HiGHS readModel
   |                              |
   +----------- compare ----------+
```

For **accepted P35 inputs**, normalized ROML and HiGHS interpretations are expected to agree. HiGHS is an independent oracle, not normative authority: a mismatch blocks qualification until the explicit bug-fix/dialect-narrowing/evidence-backed compatibility-exception policy is resolved.

For deliberate strict ROML rejections, native HiGHS behavior is recorded as compatibility telemetry rather than used to redefine P35.

For Chinneck models, the end-to-end path must preserve infeasibility and Phase 29 guarantees, and every reported finite variable bound must resolve to explicit or synthetic MPS provenance.

For Netlib, the primary goal is broad parser/solver interoperability on feasible LPs and diverse historical MPS encodings.

## Written-spec review round 1

Independent review of head `87956652` identified:

1. selected/unselected RANGE-on-`N` ambiguity;
2. missing policy for HiGHS semantic divergence/repeated RHS;
3. missing synthetic provenance for implicit bounds;
4. archive extraction path/link safety.

These findings are addressed in the binding packet and recorded in `35-REVIEW-ROUND-1.md`. The phase remains at the **independent re-review gate**; no executable implementation plan is generated yet.

## Write-back follow-on

P36 implements deterministic MPS serialization and round-trip qualification. P35 owns the shared semantic vocabulary required to avoid reverse-engineering importer details. P35 does not implement writer production code.

## Design authority

Canonical written design:

`docs/superpowers/specs/2026-08-07-mps-io-design.md`

Binding P35 semantic/requirements/test/corpus/risk documents live in this directory. No production implementation begins until independent written-spec re-review and owner acceptance.