# ROML Active Roadmap — MIR

This root file is the concise routing projection. Detailed historical milestone design remains under `.planning/milestones/`.

## Current sequence

```text
M3 semantic modeling/workflows — complete
  -> MPY Python interface — merged (#53)
  -> post-MPY construction performance/certification — merged (#55/#56)
  -> MIR shared modeling IR + block-native core — ACTIVE / authorized
  -> further Python OO ergonomics on shared IR
  -> M4 quadratic/nonlinear design gate (still deferred)
```

## MIR routing

| Phase | Capability | Status | Gate |
|---|---|---|---|
| MIR-00 | baseline + lowering/reprice measurement | next | measured evidence before redesign |
| MIR-01 | trusted variable/parameter blocks | authorized after MIR-00 | block/revision/staleness gates |
| MIR-02 | packed parametric rows/objectives + block propagation | authorized after MIR-01 | transactional/self-contained delta gates |
| MIR-03 | shared strided IR + eligibility proof + CSR | planned | D-019 IR gates |
| MIR-04 | native Rust Level-1 modeling | planned | elegant examples, ownership/labels |
| MIR-05 | rule/callback CSR builders | planned | one bulk commit/component |
| MIR-06 | Python migration to shared IR | planned | normalized Rust/Python equivalence |
| MIR-07 | ConcreteModel/Sets/NumPy/pandas + Template.bind | planned | ergonomics/bind gates |
| MIR-08 | qualification | planned | performance + CI + independent review |

Detailed authority: `.planning/milestones/MIR-modeling-ir/README.md`.

## Hard routing rules

- MIR precedes additional Python ergonomics so those APIs are built on the shared IR rather than a second Python-specific implementation.
- M4 remains deferred until separately authorized.
- Publication/tag/release are separate owner gates.
- One production phase at a time; research/spec review may run ahead without silently implementing later phases.
