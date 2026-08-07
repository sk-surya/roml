# Phase 29 Report Contract Evidence

## Scope

- Slice: canonical historical report and deterministic renderers
- Base: `18a6b3a`
- Renderers: dedicated Text and Markdown views; concise `Display`

## Contract

The report keeps exact model lineage, instance, revision, mandatory
`CompilationId`, backend identity/version, provider chain, explicit LP scope,
candidate count/grouping, numerical policy, completion, proof strength,
guarantee, semantic declaration snapshots, compiled restriction evidence,
native evidence, statistics, and warnings.

Completion and guarantee remain independent. Native-reported membership is
not promoted to semantic irreducibility. No minimum-cardinality field or
claim is representable.

## Renderer invariants

- Text output uses stable field order and presents semantic members before
  technical compiled/native evidence.
- Markdown output escapes backend/member/warning strings and uses fixed section
  headings.
- `Display` is a concise one-line summary and is intentionally not the complete
  rendering contract.
- Historical declaration snapshots remain readable after source model mutation.

## Verification

```text
cargo fmt --all -- --check                 PASS
cargo clippy -p roml --test iis_report -- -D warnings PASS
cargo test -p roml --test iis_report      PASS (2 tests)
```
