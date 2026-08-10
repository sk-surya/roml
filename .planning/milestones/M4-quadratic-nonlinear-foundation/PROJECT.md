# M4 Preview — Quadratic and Nonlinear Semantic Foundation

**Status:** deferred design target; implementation forbidden until P34 closes M3 and its NLP-readiness review passes.

## Mission

Extend ROML from a trustworthy LP/MILP semantic modeling/compiler substrate into quadratic and nonlinear optimization without discarding the identity, provenance, capability, solve-plan, reporting, and backend boundaries proven in M3.

## Why this is a new milestone

Quadratic/nonlinear work changes the mathematical function space, solver termination meaning, derivative/evaluation requirements, convexity/globality claims, and diagnostic semantics. It should not be smuggled into late M3 as “one more construct.”

## Architectural hypothesis to validate in P34

```text
Canonical semantic state
  ScalarFunction
    Linear
    Quadratic
    later: nonlinear expression/function
        |
        v
Capability-aware compiler
        |
        +--> linear backend IR
        +--> quadratic backend IR
        +--> later nonlinear evaluation/derivative interface
        |
        v
Backend sessions / SolvePlan / reports
```

The following M3 concepts are presumed reusable and must be explicitly certified by P34:

- ModelLineageId / ModelInstanceId / revision;
- exact CompilationId;
- generation-safe entity IDs;
- metadata/provenance;
- construct arena;
- snapshots/deltas;
- capability registry;
- compilation recipes/reports;
- origin maps;
- SolvePlan and overlays;
- assignments/starts/hints;
- objective policies;
- analysis report guarantee language.

## Proposed M4 workstreams

### M4-P0 — Quadratic semantic IR and evaluation

Goal: canonical representation for quadratic scalar functions without backend coupling.

Core questions:

- unique authority for linear and quadratic coefficient cells;
- symmetric term normalization (`x_i x_j == x_j x_i`);
- duplicate algebra and parameter dependencies;
- finite-value validation;
- evaluation and gradient for quadratic functions;
- objective and function-in-set use;
- clone/snapshot/delta semantics;
- public ergonomic expression construction without combinatorial operator overload mistakes.

### M4-P1 — QP objective compilation

Goal: solve convex/nonconvex quadratic objectives only when backend capability/guarantee is explicit.

- backend IR quadratic objective primitive;
- HiGHS or another backend qualified from official APIs;
- convexity metadata must not be mistaken for proof unless ROML proves it;
- local/global/nonconvex solver claims recorded honestly;
- linear models remain byte/behavior compatible through identity compiler path.

### M4-P2 — Quadratic constraints / QCQP contract

Goal: add quadratic function-in-set constraints with precise capability/globality semantics.

- convex quadratic inequality versus equality/nonconvex cases;
- no automatic linearization mislabeled exact;
- backend-native/unsupported distinction;
- provenance and compilation reports;
- feasibility/IIS language revised for global versus local evidence.

### M4-P3 — General nonlinear function/evaluation interface design

Goal: design, not immediately implement, a smooth nonlinear extension seam.

Candidate concerns:

- expression graph versus user callback/function object;
- value/gradient/Jacobian/Hessian interfaces;
- automatic differentiation strategy and dependency policy;
- sparsity pattern authority;
- parameter mutation;
- thread safety/lifetime/FFI callback behavior;
- deterministic hashing only as cache aid;
- serialization limitations.

No AD library is selected before this design is reviewed.

### M4-P4 — First smooth NLP backend qualification

Goal: prove one constrained smooth NLP path end-to-end.

Backend selection criteria:

- official maintained API/bindings;
- explicit derivative contracts;
- usable status/termination model;
- open or practical licensing for CI;
- warm-start support as a bonus, not a gate.

### M4-P5 — Nonlinear workflow integration

Potential scope after the core is proven:

- nonlinear starts/warm starts;
- objective policies where semantics are meaningful;
- local feasibility restoration reports;
- source/origin-aware nonlinear diagnostics;
- constrained/unconstrained examples;
- MINLP design research only after continuous NLP semantics stabilize.

## Critical semantic rule: diagnostics

M3's exact LP IIS guarantee must not be generalized carelessly.

```text
LP/global convex certificate -> may support global infeasibility claim
local NLP restoration failure -> local diagnostic / Unknown, never IIS
nonconvex NLP -> global claim only with globally conclusive provider/evidence
MINLP -> separate later contract
```

## Explicit non-goals for initial M4

- general MINLP;
- global nonconvex solver implementation;
- automatic differentiation framework before function-interface design;
- neural-network embedding;
- arbitrary black-box simulation optimization;
- replacing Prism or becoming a general metaheuristic framework;
- format breadth unrelated to quadratic/NLP qualification.

## Entry gate

M4 planning may be activated only when P34's `M3_NLP_READINESS.md` says the current seams can support the proposed quadratic shapes with bounded additive changes, or clearly identifies and resolves a required M4 migration before feature implementation.