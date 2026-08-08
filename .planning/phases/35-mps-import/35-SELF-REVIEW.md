# Phase 35 Written-Spec Self-Review

**Reviewed branch:** `docs/p35-mps-io-design`  
**Baseline:** `main@ff99389b9bf1318c555dc1f72dff6b5c7a4111c0`  
**Scope:** design/planning only; no production parser, writer, corpus gitlinks, workflow changes, or roadmap-routing mutation  
**Current gate:** independent re-review after round-1 blocker resolution

## 1. Review history

The first written-spec review of head `87956652` found three blockers and one P2:

1. contradiction over RANGE-on-`N` in unselected vectors;
2. unresolved authority/disposition when HiGHS differs, especially repeated RHS;
3. missing provenance for implicit MPS variable bounds;
4. archive extraction lacked path/link safety requirements.

The exact dispositions are recorded in `35-REVIEW-ROUND-1.md`.

## 2. Internal consistency after round 1

The binding documents now agree on:

```text
P35 = reader + semantic resolution + HiGHS differential + external corpus/IIS qualification
P36 = writer + semantic round-trip + external write/read qualification
```

and:

- handwritten parser; no LALRPOP;
- `roml::io::mps` ownership;
- fixed + free input;
- staging before `Model` construction;
- linear LP/MILP scope;
- hard failure on unsupported semantic extensions;
- objective selection/negative-offset policy;
- one semantic ranged row;
- duplicate COLUMNS accumulation only;
- duplicate selected RHS/RANGE rejection;
- structural validation for every staged vector;
- model-semantic validation only for selected rim vectors;
- selected RANGE-on-`N` rejection and unselected inert staging;
- explicit named rim-vector policy;
- synthetic provenance for implicit continuous and INTORG bounds;
- HiGHS as independent oracle, not semantic authority;
- explicit divergence disposition policy;
- optional pinned corpus submodules;
- pre-write-safe Chinneck archive extraction;
- semantic rather than textual P36 round trip.

## 3. Repository-model compatibility review

Current ROML represents variable type independently from bounds (`VarType::Integer`) and accepts `-inf/+inf` variable bounds. Therefore:

```text
INTORG variable + FR
    -> Integer [-inf,+inf]
```

is a required P35 behavior, not a deferred capability probe.

## 4. Vector-validation consistency review

There is now one explicit two-layer rule:

### Always validate

- lexical syntax;
- finite numeric fields;
- supported record kinds;
- row/variable references;
- section/order/marker structure.

### Validate only if selected

- duplicate same-row RHS assignment;
- duplicate same-row RANGE transformation;
- RANGE-on-`N` semantic applicability;
- final BOUNDS-domain consistency.

This closes the prior R07/R09 contradiction. Tests R07-R12, M11-M13, B21-B22, and V09-V11 make the distinction executable.

## 5. Differential policy review

The semantics reference, requirements, test matrix, design packet, and corpus document now use the same authority policy:

- ROML's frozen semantics are normative;
- HiGHS is independent compatibility evidence;
- an accepted-input mismatch blocks merge until `roml_bug_fixed`, `dialect_narrowed`, or owner-approved evidence-backed `compatibility_exception`;
- strict ROML rejection + HiGHS acceptance is recorded as `intentional_roml_rejection`;
- there is no automatic `follow_highs` disposition.

Repeated selected RHS/RANGE records are strict typed errors. Additive duplicate semantics remain specific to COLUMNS cells.

## 6. Provenance completeness review

The source-map contract now distinguishes explicit records from implicit format defaults.

Finite default restrictions have synthetic origins:

```text
ImplicitContinuousDefault
ImplicitIntegerMarkerDefault
```

with source anchors to variable-introduction/INTORG records. Explicit BOUNDS overrides replace provenance only on the sides they actually determine.

The invariant is explicit: every finite imported variable-bound restriction reportable by P29 must resolve to exactly one explicit-or-synthetic MPS origin. Tests P01-P08 cover this.

## 7. Archive security review

Chinneck materialization no longer permits blind extraction plus post-checking. Before any write the helper must reject:

- POSIX absolute paths;
- drive/UNC paths;
- normalized traversal;
- symlink/hardlink entries;
- special files;
- destinations outside the fresh root.

Extraction is temporary, no-follow, and atomically promoted only after full success. Security tests A01-A11 are binding.

## 8. External-format evidence review

The semantic contract remains grounded in MOSEK's current MPS documentation for fixed/free layouts, E/G/L/N rows, objective selection, variable bounds, integer markers, RANGE transforms, and additive duplicate COLUMNS elements.

The design deliberately accepts repeated COLUMNS blocks as a compatibility relaxation while still applying additive duplicate-cell semantics. This relaxation gets synthetic and HiGHS differential tests rather than being mislabeled strict MPS conformance.

## 9. Corpus/layout and provenance review

- Netlib exposes expanded `.mps` files.
- Chinneck stores collections in `.7z`/`.zip` archives.
- Corpus repositories are pinned submodules to owner forks.
- Extracted Chinneck data remains generated/untracked.
- Neither upstream exposed a root LICENSE at design time, so no external dataset contents are copied into ROML.

This is an engineering provenance policy, not a legal conclusion.

## 10. Architecture isolation review

- lexical layer is fuzzable without a solver;
- staging is solver-independent;
- selected semantic resolution is independently testable;
- core parser has no HiGHS dependency;
- HiGHS reader is confined to qualification;
- source mapping remains outside canonical model/compiler state;
- archive setup remains outside ordinary unit/package paths;
- P36 can consume ROML semantics without original byte layout.

No unrelated production refactor is required to begin P35 after approval.

## 11. Branch-scope expectation

This design branch must continue to contain only planning/spec files under:

```text
docs/superpowers/specs/
.planning/phases/35-mps-import/
```

It must not add `src/`, parser tests, manifests, workflows, `.gitmodules`, corpus gitlinks, or active roadmap/state routing before written-spec approval.

## 12. Current disposition

**Disposition: ROUND-1 FINDINGS ADDRESSED; INDEPENDENT RE-REVIEW REQUIRED.**

Do not generate executable `35-PLAN.md` or begin production implementation until re-review clears the written spec.