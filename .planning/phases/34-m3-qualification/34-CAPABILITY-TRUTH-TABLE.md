# P34 Capability Truth Table (contract §3.5, generated from declarations)

Source of truth: `highs_capability_set(1, 15, 0)` in
`roml-highs/src/session.rs:617` with feature lists at `:545-606`.
Executable form: `roml-highs` in-crate tests
`highs_capability_set_declares_m2_native_and_m3_unsupported` and
`highs_capability_set_declares_p32_bridge_support_without_native_claims`
(`roml-highs/src/session.rs:1747+`), run in hosted CI on
ubuntu/macos/windows. Docs consume this table; no doc may claim more.

Bundled backend: HiGHS 1.15.0 via pinned `highs-sys` (feature `bundled`).
System floor: HiGHS 1.9.0 on Linux (`Test (system, ubuntu)` CI lane);
other-OS system lanes are explicit non-blocking per contract §3.2.

| Feature | Bundled 1.15.0 | System 1.9.0 floor | Basis |
|---|---|---|---|
| Lp | native | native | M2 audit |
| Mip | native | native | M2 audit |
| IncrementalBounds | native | native | M2 audit |
| IncrementalRows | native | native | M2 audit |
| IncrementalCoefficients | native | native | M2 audit |
| MipStart | native | native | pinned-header audit (`Highs_setSparseSolution`) |
| PartialMipStart | native | native | pinned-header audit |
| FeasibilityRelaxation | bridge (portable weighted-L1; no native claim) | bridge | P30 qualification |
| SoftConstraint | bridge (exact violation bridge; no native claim) | bridge | P30 qualification |
| Indicator | bridge | bridge | P32 |
| Reification | bridge | bridge | P32 |
| Boolean | bridge | bridge | P32 |
| Cardinality | bridge | bridge | P32 |
| MinMax | bridge | bridge | P32 |
| AbsoluteValue | bridge | bridge | P32 |
| BinaryProduct | bridge | bridge | P32 |
| PiecewiseLinear | bridge | bridge | P33 |
| Iis | native (LP only, bundled 1.15.0) | unsupported (typed `Unsupported`) | header-version qualification |
| MultipleMipStarts | unsupported | unsupported | unqualified |
| VariableHints | unsupported | unsupported | unqualified |
| InitialBasis | unsupported | unsupported | unqualified |
| Sos1 | unsupported | unsupported | unqualified |
| Sos2 | unsupported | unsupported | unqualified |
| NativePiecewiseLinear | unsupported | unsupported | unqualified |
| NativeMultiObjective | unsupported | unsupported | portable P31 path normative |

Unlisted `BackendFeature` variants default to `Unsupported`. Native IIS
beyond LP, native relaxation, native multiobjective, and MOSEK/Xpress
providers are explicitly unqualified (documented residuals, not passes).
