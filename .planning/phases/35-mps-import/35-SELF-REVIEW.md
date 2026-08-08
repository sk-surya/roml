# Phase 35 Written-Spec Self-Review

**Reviewed branch:** `docs/p35-mps-io-design`  
**Baseline:** `main@ff99389b9bf1318c555dc1f72dff6b5c7a4111c0`  
**Scope:** design/planning only; no production parser, writer, corpus gitlinks, or roadmap-routing mutation

## 1. Placeholder scan

No implementation-blocking `TBD`/`TODO` semantic decisions remain in the P35 packet.

Items intentionally deferred are explicitly assigned to implementation planning or P36 rather than left as ambiguous P35 behavior. Examples:

- exact Netlib Tier-1 allowlist is selected after corpus inventory, while the required coverage categories are already frozen;
- deterministic writer numeric formatting is P36 implementation design, while semantic round-trip and no-loss requirements are already frozen;
- performance thresholds are intentionally not correctness gates on unstable hosted hardware.

## 2. Internal consistency check

The packet consistently defines:

```text
P35 = reader + semantic resolution + HiGHS differential + external corpus/IIS qualification
P36 = writer + semantic round-trip + external write/read qualification
```

All documents agree on:

- handwritten parser; no LALRPOP;
- `roml::io::mps` ownership;
- fixed + free input;
- staging before Model construction;
- linear LP/MILP scope;
- hard failure on unsupported semantic extensions;
- objective selection/offset policy;
- one semantic ranged row;
- duplicate matrix accumulation;
- selected named rim-vector policy;
- source provenance outside canonical `Model`;
- HiGHS as independent oracle only;
- optional pinned corpus submodules;
- semantic rather than textual write-back round trip.

## 3. Repository-model compatibility review

One ambiguity in the initial test matrix was resolved against current ROML code.

Current ROML represents variable type separately from bounds (`VarType::Integer`) and accepts `-inf/+inf` variable bounds. Therefore:

```text
INTORG variable + FR
    -> VarType::Integer
    -> bounds [-inf,+inf]
```

is a required supported P35 behavior. It is no longer conditional on a future ROML domain capability.

## 4. External-format evidence review

The semantic contract is grounded in MOSEK's current MPS documentation for:

- fixed field positions;
- fixed/free dialects;
- `E/G/L/N` rows;
- objective selection from `N` rows;
- variable defaults;
- `FR/FX/LO/MI/PL/UP/BV/LI/UI` behavior;
- integer marker default `[0,1]`;
- RANGE transformations;
- additive duplicate COLUMNS matrix entries;
- ASCII expectation.

CPLEX-compatible objective-offset/first-vector conventions are frozen explicitly and scheduled for independent HiGHS differential probes where historical MPS behavior is not fully standardized.

The design intentionally accepts repeated COLUMNS blocks for one variable as a relaxed compatibility behavior even though strict MPS expects a variable's elements to be grouped. The semantic result is deterministic because records merge by column name and duplicate cells add. This is covered by synthetic and HiGHS differential tests rather than being mistaken for strict-format conformance.

## 5. Corpus-layout review

The original corpus design assumed external repositories could simply expose model paths. Inspection showed:

- Netlib exposes expanded `.mps` files under `mps_files/`;
- Chinneck stores collections in `.7z`/`.zip` archives.

The packet was corrected so Chinneck archives are materialized at test time into ignored `target/roml-corpora/...` output. No extracted model files are committed to ROML and no production/archive dependency is added to `roml`.

## 6. Licensing/provenance review

At design time neither external upstream repository exposes a root `LICENSE` file. The packet therefore:

- uses submodule gitlinks rather than copying dataset contents;
- pins exact commits on the owner's forks;
- records upstream URLs;
- keeps extracted Chinneck files generated/untracked;
- requires a separate review before any future vendoring.

This is an engineering provenance policy, not a legal conclusion.

## 7. Scope review

The work is appropriately decomposed into two production phases rather than one oversized implementation:

### P35

Reader semantics, diagnostics, transactionality, synthetic/metamorphic/fuzz tests, native HiGHS differential validation, corpus acquisition, Netlib qualification, and Chinneck IIS qualification.

### P36

Representability analysis, deterministic free-MPS writer, strict optional fixed writer, semantic round trips, and external native-HiGHS write/read qualification.

P35 does not expand into quadratic/conic MPS or other file formats. Those remain later independent additions to `roml::io`.

## 8. Architecture isolation review

Boundaries satisfy the isolation test:

- lexical layer can be fuzzed without ROML model construction;
- MPS staging can be tested without native solvers;
- semantic resolver can be tested with synthetic documents/models;
- core parser has no HiGHS dependency;
- native HiGHS reader is confined to qualification;
- source mapping is separate from canonical model/compiler state;
- external corpus setup is separate from ordinary unit/CI paths;
- P36 writer can consume ROML semantics without depending on original reader byte layout.

No unrelated refactor is required to begin P35.

## 9. Branch-scope verification

Comparison to the baseline shows only design/spec files under:

```text
docs/superpowers/specs/
.planning/phases/35-mps-import/
```

There are no changes to `src/`, `tests/`, manifests, workflows, `.gitmodules`, submodule gitlinks, or active roadmap/state routing in this design branch.

## 10. Written-spec disposition

**Disposition: READY FOR OWNER WRITTEN-SPEC REVIEW.**

The next action is owner review of the written spec/packet. After approval, invoke the repository's implementation-planning workflow and create the executable P35 task plan. Do not begin production implementation before that gate.
