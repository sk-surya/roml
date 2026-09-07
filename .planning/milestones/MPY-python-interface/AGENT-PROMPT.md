# Coding Agent Kickoff

You are the implementation agent for `sk-surya/roml`. Execute the approved ultra-planning packet on branch `docs/python-interface-ultraplan`, rooted at `.planning/milestones/MPY-python-interface/README.md`.

The goal is a neat, intuitive, typed and performant Python API over ROML using PyO3 + maturin, for repeated optimization and Marginal's MPC models. This is an execution request: continue through review, corrective work, implementation, tests, packaging and final review. Do not stop after producing another plan or a binding skeleton.

First preserve local work, fetch current refs and read `AGENTS.md`, root planning state, this packet in its read order, and the governing P31/P34 documents. Use the packet branch as the planning source; implementation branches start from current qualified main after prerequisite merges. Do not assume the observed `main@659c30c` or PR #49 head remains current.

FIRST: inventory all open ROML PRs, review their code/discussions/checks, and resolve relevant prerequisites. The owner authorizes you to remediate and normally merge pending prerequisite PRs after independent review and required checks, including the planning PR containing this packet. No repeated permission request is needed for these routine gated merges. Do not blindly merge unrelated PRs, bypass protections, use admin merge, fabricate approvals or force-push someone else's work. Compare expected heads before merging and refresh evidence if they change. If a required human approval or access credential is genuinely missing, report exactly that blocker.

Pay particular attention to #49/P31: complete generated-term objective accounting in stage/final reports and the no-reported-scalar fallback; verify no-incumbent outcomes and rollback; qualify explicit solve options/shared staged deadlines. Reproduce findings on the current head rather than blindly trusting historical review text. Then finish and normally merge the existing P34 qualification closure. Update root state truthfully. Only then start Python runtime implementation.

SECOND: execute MPY-01 through MPY-06 from IMPLEMENTATION-PLAN.md using the exact API and contracts in DESIGN.md. Build `roml._native` in `roml-python`, packaged with maturin and a small typed `python/roml` surface. Keep Rust core Python-free and solver-free. No public ROML C ABI, universal nonlinear DSL or commercial-solver expansion.

Treat ergonomics as an acceptance requirement: clear scalar LP syntax, shaped variables/parameters, Rust bulk dot/sum and CSR operations, one-call atomic named parameter updates, persistent `Highs`, immutable `Solution`, useful exceptions, complete stubs and runnable examples. Invalid input cannot partially mutate a batch. Missing solution values cannot become zero. Old results retain their provenance. Release the GIL safely during native work without making native state concurrently mutable. Never add unjustified unsafe Send/Sync implementations.

Use focused TDD and small coherent commits. Keep one implementation phase active and require independent review at the packet's gates; reviewer agents are permitted when available. Fix P0/P1 findings before proceeding. Update milestone STATE and requirement evidence at every completed gate. Routine reviews are not owner comprehension gates; proceed autonomously unless there is an actual unresolved blocker or a material scope change.

Validate with the synthetic causal rolling BESS MILP, matching direct Rust ROML, fresh-rebuild ROML and highspy references. Prove objective/feasibility equivalence, atomic update behavior, busy/lifecycle errors, bounded wrapper overhead and memory stability. Measure performance; do not assert it from language choice. Keep all fixtures synthetic and all market/domain logic outside the library.

Finish by producing tested standard CPython 3.13/3.14 wheels for Linux x86_64, macOS arm64 and Windows x86_64, plus a clean sdist rebuild, installed-package examples and typing checks. Preserve core MSRV 1.85. Missing platforms or skipped mandatory tests are not passes. Leave the Python implementation in a reviewable PR with exact head, commands, artifact links, benchmark results, supported matrix and remaining limits. Do not publish to PyPI/crates.io, create tags/releases, merge MPY implementation without separate authorization, or interact with live markets.

Begin with the fresh PR inventory and close prerequisites, then carry the implementation to the completion predicate in ROADMAP.md.
