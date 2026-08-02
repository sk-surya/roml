# M2 Requirement Traceability

| Requirement | Phase | Primary implementation evidence | Qualification evidence |
|---|---|---|---|
| API-01.1–01.5 | P21 | `roml-highs` façade tests | repeated solve, failure recovery, stale solution tests |
| API-02.1–02.4 | P21 | core `SolverSession<B>` tests | backend conformance and differential suites |
| API-03.1–03.5 | P21 | solution/status conversion tests | objective offset, dual, reduced-cost, status matrix tests |
| API-04.1–04.5 | P22/P23 | compile-pass modeling fixtures | migration and public API review |
| API-05.1–05.6 | P22 | named entity lifecycle tests | model formatting and diagnostics examples |
| API-06.1–06.6 | P22/P23 | invalid-input and atomicity tests | release-profile tests and compile-fail fixtures |
| API-07.1–07.5 | P23 | prelude/root module tests | `cargo public-api` diff and reviewer signoff |
| API-08.1–08.4 | P20/P23 | replacement-first sequence | `MIGRATION.md`, deprecation tests, changelog |
| API-09.1–09.5 | P24 | README/guide/examples | doctests and example CI |
| API-10.1–10.6 | P20/P24 | baseline and final matrices | packed consumers and independent review |

## Phase-to-requirement closure

### P20

Closes planning/characterization portions of API-04, API-07, API-08, and API-10. It cannot mark behavioral requirements complete.

### P21

Must close all API-01, API-02, and API-03 requirements before P22 starts.

### P22

Must close API-04 canonical-path behavior, API-05, and creation/mutation portions of API-06.

### P23

Must close API-06 consistency, all API-07, and all API-08.

### P24

Must close API-09, API-10, and verify every earlier requirement against packaged consumers.

## Required evidence bundle

Create `docs/release/evidence/M2_PUBLIC_API.md` during P24 containing:

- base and final SHAs;
- requirement closure table;
- command matrix and outputs;
- public API before/after summary;
- packaged consumer results;
- platform/feature coverage;
- skipped checks with reasons;
- residual risks;
- independent review disposition.