# Phase 29 LP IIS qualification evidence

## Correctness and safety matrix

| Evidence | Result |
| --- | --- |
| contract and tri-state classification | PASS |
| exact CompilationId and stale-map rejection | PASS |
| semantic row-side and variable-bound atoms | PASS |
| persistent-fixing layer restoration | PASS |
| solve-lock and temporary-fixing atom execution | DEFERRED: overlay input is not yet part of the public analysis plan |
| grouped multi-restriction construct execution | DEFERRED: identity compiler currently exposes individual generated origins |
| isolated oracle cache/recovery semantics | PASS |
| adaptive reduction and mandatory fresh verifier | PASS |
| deterministic Text/Markdown renderers | PASS |
| Unknown/limit/numerical outcomes do not claim IIS | PASS |
| bundled HiGHS native seed extraction | PASS |
| native-seeded semantic reduction | PASS |
| mutation, differential, and planted-IIS tests | PASS |

Focused commands:

```text
cargo test -p roml --test iis_contract --test restriction_universe \
  --test infeasibility_oracle --test iis_reducer --test iis_report \
  --test iis_qualification --test iis_differential --test iis_mutation \
  --test iis_planted                                           PASS
cargo test -p roml-highs --test iis -- --nocapture             PASS (3 tests)
cargo bench --bench iis                                       PASS (harness builds)
```

The planted corpus is deterministic and records oracle calls and final member
count. Full performance qualification still requires machine metadata and
comparison against a naive one-at-a-time baseline before release claims are
made.

The two deferred rows are deliberate scope gates, not silent fallbacks. The
public `analyze_infeasibility` plan currently analyzes the canonical model and
does not accept a `SolveOverlay`; therefore solve locks and temporary fixings
have contract-level atom/origin types and bound-stack support, but are not
claimed as executable Phase 29 members. Likewise, `ByConstruct` preserves
individual identity-compiler origins until a multi-row construct bridge is
available. These gaps must close before the phase can be called fully
complete against the packet's entire atom-inventory requirement.

## Guarantee discipline

The only portable irreducibility claim is semantic and conditional on the
recorded oracle and numerical policy. A native backend claim is retained as
evidence and never promoted to minimum cardinality or native semantic
irreducibility. Unknown checks downgrade the result.

## Stop conditions and non-goals

Phase 29 stops at qualified LP and explicitly requested LP-relaxation
analysis. It does not diagnose integrality-only MIP infeasibility, add
feasibility relaxation, enumerate all IISs, optimize cardinality, or label
local nonlinear restoration failure as an IIS. System HiGHS native support
stays unsupported until its exact version matrix is qualified.
