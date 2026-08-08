# Phase 35 Written-Spec Review — Round 1 Disposition

**Reviewed head:** `87956652a9cfca359b848351cb773260d428d9f8`  
**Reviewer:** independent Codex review relayed by owner  
**Disposition before fixes:** not approved; three written-spec blockers + one P2  
**Fix scope:** design/spec only; no production implementation

## Finding 1 — RANGE on `N` contradicted selected-vector behavior

### Review finding

`35-MPS-SEMANTICS.md` rejected RANGE-on-`N` unconditionally, while test R09 allowed an unselected vector to contain a semantically malformed combination. The packet did not define whether semantic validation applies to all staged vectors or only the selected one.

### Resolution

**Closed by D35-34 and D35-35.**

P35 now has two validation layers:

1. **structural validation for all staged vectors** — syntax, finite numeric values, supported record kinds, and referenced row/variable existence;
2. **model-semantic validation only for the selected RHS/RANGES/BOUNDS vector**.

Therefore:

```text
selected RANGES contains RANGE on N
    -> typed semantic error

unselected RANGES contains RANGE on N
    -> stage successfully if syntax/references are valid; no model effect

later select that vector
    -> same typed semantic error
```

Tests R07/R09/R10 and V09-V11 freeze this distinction.

## Finding 2 — differential authority was unresolved

### Review finding

The packet aimed for `semantics(ROML) == semantics(HiGHS)` but did not specify what to do when HiGHS differs, particularly for repeated RHS records.

### Resolution

**Closed by D35-36 and D35-37.**

The frozen ROML semantics reference is normative. HiGHS is an independent differential oracle, not a semantic authority.

Duplicate selected RHS same-row entries are now a typed ambiguity error and are **not** summed. Duplicate selected RANGE same-row entries are also rejected. The additive duplicate rule is explicitly COLUMNS-only. BOUNDS remains an ordered transition stream.

For an input P35 accepts, a HiGHS semantic mismatch blocks production qualification until one disposition is recorded:

- `roml_bug_fixed`;
- `dialect_narrowed`;
- `compatibility_exception` backed by authoritative format evidence and owner approval.

For a deliberately strict ROML rejection that HiGHS accepts, the harness records `intentional_roml_rejection`; it does not change ROML automatically.

The test matrix includes accepted-input and strict-policy differential probes and forbids a `follow_highs_automatically` disposition.

## Finding 3 — implicit bound provenance was incomplete

### Review finding

Continuous default lower bounds and INTORG `[0,1]` defaults have no BOUNDS record, while IIS acceptance requires every bound member to resolve to MPS provenance.

### Resolution

**Closed by D35-38.**

`MpsSourceMap` must represent synthetic format-derived origins:

```text
ImplicitContinuousDefault {
    side: Lower,
    value: 0,
    variable_first_columns_span,
}

ImplicitIntegerMarkerDefault {
    side: Lower | Upper,
    value: 0 | 1,
    intorg_marker_span,
    variable_first_marked_columns_span,
}
```

Explicit selected BOUNDS records retain exact spans. If an explicit record overrides only one INTORG-default side, the retained side keeps synthetic marker provenance.

Invariant:

> Every finite imported variable-bound restriction that Phase 29 can report resolves to exactly one explicit-or-synthetic MPS origin.

Synthetic origins render as format-derived defaults and never fabricate BOUNDS source lines. Tests P01-P08 cover provenance completeness and IIS lookup.

## Finding 4 — archive extraction path safety

### Review finding

The Chinneck archive materializer did not specify protection from absolute paths, `..` traversal, or symlinks.

### Resolution

**Closed by D35-39 and MPS-S05.**

Before writing any archive entry, the materializer rejects:

- POSIX absolute paths;
- Windows drive-qualified and UNC paths;
- normalized `..` traversal;
- symlink and hardlink entries;
- device/FIFO/socket/special files;
- any destination escaping the fresh extraction root.

Extraction uses a new temporary root with no-follow filesystem behavior. Partial output is never promoted/reused. Cache/completion state is atomically promoted only after full validation and success.

A blind extractor invocation followed only by post-hoc validation is explicitly prohibited. Test cases A01-A11 freeze the security behavior.

## Files amended

- `docs/superpowers/specs/2026-08-07-mps-io-design.md`
- `35-DESIGN-PACKET.md`
- `35-DECISIONS.md`
- `35-REQUIREMENTS.md`
- `35-MPS-SEMANTICS.md`
- `35-TEST-MATRIX.md`
- `35-CORPUS-QUALIFICATION.md`
- `35-RISKS.md`

## Re-review gate

No executable `35-PLAN.md` should be generated until independent re-review confirms these written-spec blockers are closed. Production code, submodule gitlinks, workflow changes, and active roadmap routing remain out of scope on this design branch.