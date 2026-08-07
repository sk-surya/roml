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
```

`OriginalLp` is the default scope. A caller must explicitly request
`LpRelaxation` for a MIP model; that result is not an original-MIP IIS. Native
HiGHS evidence is separately labeled and is semantically reduced before an
irreducibility guarantee is reported.

Unknown, numerical, interrupted, and limit oracle outcomes do not become
infeasible. They produce an incomplete report without an irreducibility claim.
Feasibility relaxation is a separate API, and minimum-cardinality IIS claims
are not part of Phase 29.
