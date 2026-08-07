# LP infeasibility analysis

Phase 29 analyzes only LP infeasibility. The primary operation is planned on a
`SolverSession`, so the persistent solve session remains separate from the
isolated candidate-check session:

```rust,ignore
use roml::{InfeasibilityPlan, SolverSession};

let plan = InfeasibilityPlan::portable_lp();
let report = session.analyze_infeasibility(&model, &plan)?;
println!("{}", report);
println!("{}", roml::TextInfeasibilityReport(&report));
println!("{}", roml::MarkdownInfeasibilityReport(&report));
```

`OriginalLp` is the default scope. A caller must explicitly request
`LpRelaxation` for a MIP model; that result is not an original-MIP IIS. Native
HiGHS evidence is separately labeled and is semantically reduced before an
irreducibility guarantee is reported.

The explicit provider choices are `RomlPortable`, `Auto`, `NativeThenRoml`,
and `NativeOnly`. The bundled HiGHS 1.15.0 provider is qualified for native
seeding; system-discovered HiGHS remains typed `Unsupported` until its exact
header and library version are independently qualified.

The report's `Irreducible` guarantee means semantic irreducibility under the
recorded candidate universe, oracle, and numerical policy. It never means
minimum cardinality, and native output alone never supplies that guarantee.

Unknown, numerical, interrupted, and limit oracle outcomes do not become
infeasible. They produce an incomplete report without an irreducibility claim.
Feasibility relaxation is a separate API, and minimum-cardinality IIS claims
are not part of Phase 29.
