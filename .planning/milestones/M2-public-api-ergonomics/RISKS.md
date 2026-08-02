# M2 Risks and Controls

## Risk 1 — Façade hides synchronization bugs

**Failure mode:** implicit commit/sync makes incorrect revision handling less visible.

**Controls:** assert model revision, backend revision, health, and stale-solution invalidation in every solve-path test; preserve differential and fault suites; allow only one rebuild retry.

**Stop trigger:** any solve succeeds while backend revision differs from the model revision used for that solve.

## Risk 2 — Objective constant double counting

**Failure mode:** backend result includes an offset and normalization adds it again.

**Controls:** status/solution conversion tests for positive, negative, and zero offsets on rebuild and incremental paths.

**Stop trigger:** façade objective differs from direct backend observable or reconstructed expression evaluation.

## Risk 3 — Unified status loses information

**Failure mode:** mapping collapses feasible-limit, unbounded-or-infeasible, license, numerical, or interrupted states.

**Controls:** exhaustive mapping test over every `TerminationStatus`; no wildcard arm; metadata retains native/backend context.

**Stop trigger:** two backend states with different user actions map to an indistinguishable public state.

## Risk 4 — Rust trait/coherence complexity expands scope

**Failure mode:** new handle wrappers or generic add traits require broad operator-overload rewrites.

**Controls:** use semantic type aliases in M2; use explicit `add_variable`/`add_parameter`; defer generic `model.add(...)` and indexed containers.

**Reversal trigger:** aliases cause irreparable rustdoc or inference ambiguity.

## Risk 5 — Naming metadata creates false identity guarantees

**Failure mode:** users assume names are unique or serialized identities.

**Controls:** names are optional diagnostics, not keys; duplicate names are allowed unless a later explicit uniqueness policy is added; docs state this clearly.

## Risk 6 — Breaking changes strand current code

**Failure mode:** signatures change before replacements or migration guidance exist.

**Controls:** replacement-first commits, compile-pass compatibility fixtures, deprecation notes, `MIGRATION.md`, and pre-1.0 changelog.

**Stop trigger:** a removed API has no mechanical replacement for supported current behavior.

## Risk 7 — Prelude reduction breaks backend authors

**Failure mode:** moving exports makes extension code unexpectedly difficult.

**Controls:** stable `advanced`/`backend` re-exports; backend contract examples; semver/public API diff review.

## Risk 8 — Definition builders duplicate domain semantics

**Failure mode:** builder validation and model validation diverge.

**Controls:** builders produce one internal validated domain representation; model remains the final authority; property tests compare convenience and low-level construction.

## Risk 9 — Automatic fallback masks unsupported incremental operations

**Failure mode:** every solve rebuilds and performance silently regresses.

**Controls:** metadata records synchronization mode; tests assert delta path after ordinary updates; optional tracing exposes rebuild reason; benchmark repeated parameter updates.

**Stop trigger:** supported parameter/bound updates rebuild under healthy HiGHS without an explicit reason.

## Risk 10 — Docs stabilize the wrong API too early

**Failure mode:** README is rewritten before signatures survive implementation tests.

**Controls:** P24 docs follow accepted P21–P23 interfaces; documentation inventory may begin earlier but final prose is gated.