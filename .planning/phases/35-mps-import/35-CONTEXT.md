---
phase: 35-mps-import
status: design-approved-written-spec-review-pending
owner_priority: pulled-forward
baseline: ff99389b9bf1318c555dc1f72dff6b5c7a4111c0
follow_on: P36-mps-write-roundtrip
---

# Phase 35 Context — MPS Import and External Qualification

## Why this phase exists

Phase 29 delivered solver-agnostic LP infeasibility analysis. The next owner-directed objective is to exercise that functionality on real MPS models rather than only programmatically constructed fixtures.

The immediate external corpora are:

- `sk-surya/infeasiblelps`, forked from John Chinneck's infeasible LP repository;
- `sk-surya/lp-data-netlib`, forked from the converted Netlib LP repository.

The feature is intentionally designed as production ROML functionality, not a temporary test parser. MPS is the first file-format member of a future `roml::io` layer. Write-back is the immediate follow-on phase and is designed concurrently so the reader does not create a read-only dead end.

## Owner direction

Binding owner decisions from the design discussion:

1. Start with linear LP/MILP MPS only.
2. Other formats will be added later.
3. MPS write-back is the next step and must be accounted for in the reader architecture now.
4. Validate ROML parsing against HiGHS's independent MPS reader.
5. Use the owner's forks of the Chinneck and Netlib repositories for reproducible corpus testing.
6. Corpus integration mechanism may be chosen architecturally rather than prescribed by the owner.

## Phase numbering and routing

The existing M3 roadmap already allocates P30 through P34. This work is numbered P35 rather than renumbering existing phases. Owner priority pulls P35 forward for planning/execution because it directly qualifies completed P29 functionality. P30-P34 retain their existing contracts and numbering.

Roadmap/current-phase routing is not modified in this design-only branch. That routing change is an implementation-start action after written-spec approval.

## Repository constraints

- `roml` remains solver-free; MPS parsing belongs in core and cannot depend on HiGHS.
- HiGHS differential tests live in `roml-highs` or a qualification harness that may depend on both crates.
- Core Rust MSRV remains 1.85.
- The existing package allowlist does not include `testdata/`, so external corpus checkouts must remain outside published crate contents.
- Production parser code may not rely on external corpus availability.
- Normal CI must pass from an ordinary non-recursive clone.

## Corpus pins at design time

```text
sk-surya/infeasiblelps
  97a936498e5240d44adaf7dcfe84877fa34ce301

sk-surya/lp-data-netlib
  56257eea85b433ce6aa67d26156b36385318fd6f
```

The Chinneck repository is roughly 34 MB and the converted Netlib repository roughly 5 MB by GitHub repository metadata. Neither upstream repository exposes a root `LICENSE` file at design time. P35 therefore does not copy corpus files into ROML.

## Chosen corpus layout

Planned repository-only submodules:

```text
testdata/corpora/infeasible-lps
testdata/corpora/netlib
```

The submodules point to the owner's forks and exact reviewed SHAs. They are optional for ordinary development and initialized only for corpus qualification.

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

For Chinneck models, the end-to-end path must establish that ROML can parse the model, HiGHS can independently parse the same file, both interpretations are mathematically equivalent within the declared comparison contract, infeasibility is preserved, and ROML IIS reports remain valid in original MPS row/bound terms.

For Netlib models, the primary purpose is broad parser/solver equivalence on real feasible LPs and diverse historical MPS encodings.

## Write-back follow-on

P36 will implement deterministic MPS serialization and round-trip qualification. P35 owns the shared semantic vocabulary and APIs required to avoid re-parsing or reverse engineering importer details later. P35 does not implement writer production code.

## Design authority

The written design is:

`docs/superpowers/specs/2026-08-07-mps-io-design.md`

The binding P35 design packet and decision/requirements/test documents live in this directory. No production implementation begins until the owner reviews and accepts the written spec.
