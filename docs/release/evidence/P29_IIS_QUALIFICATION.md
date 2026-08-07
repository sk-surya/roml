# Phase 29 LP IIS qualification evidence

## Correctness and safety matrix

| Evidence | Result |
| --- | --- |
| contract and tri-state classification | PASS |
| exact CompilationId and stale-map rejection | PASS |
| semantic row-side and variable-bound atoms | PASS |
| persistent-fixing layer restoration | PASS |
| solve-lock and temporary-fixing atom execution | DEFERRED: overlay input is not yet part of the public analysis plan |
| grouped multi-restriction construct execution | PASS: `ByConstruct` aggregates generated row sides and variable bounds into one toggle |
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
cargo test -p roml-highs --test iis -- --nocapture             PASS (7 tests)
cargo bench --bench iis                                       PASS (harness builds)
```

The planted corpus is deterministic and records oracle calls and final member
count. Full performance qualification still requires machine metadata and
comparison against a naive one-at-a-time baseline before release claims are
made.

The solve-lock and temporary-fixing row remains a deliberate scope gate: the
public `analyze_infeasibility` plan currently analyzes the canonical model and
does not accept a `SolveOverlay`. Their atom/origin types and bound-stack
support remain contract groundwork for the overlay-aware follow-up.

## Guarantee discipline

The only portable irreducibility claim is semantic and conditional on the
recorded oracle and numerical policy. A native backend claim is retained as
evidence and never promoted to minimum cardinality or native semantic
irreducibility. Unknown checks downgrade the result.

## Stop conditions and non-goals

Phase 29 stops at qualified LP and explicitly requested LP-relaxation
analysis. It does not diagnose integrality-only MIP infeasibility, add
feasibility relaxation, enumerate all IISs, optimize cardinality, or label
local nonlinear restoration failure as an IIS. System HiGHS portable analysis
is supported by the system feature; system native IIS remains unsupported
until its exact header/library version matrix is qualified.
