# ROML Agent Instructions

## Mission

Harden ROML from a functional macOS-tested prototype into a production-grade, cross-platform Rust MILP modeling workspace suitable for crates.io publication and later language bindings.

The public-release program is governed by:

1. `.planning/PROJECT.md` — product intent and non-goals.
2. `.planning/REQUIREMENTS.md` — release requirements and traceability IDs.
3. `.planning/ROADMAP.md` — phase order, gates, and dependencies.
4. `docs/release/PRINCIPAL_ENGINEERING_AUDIT.md` — evidence-backed baseline audit.
5. `docs/release/ARCHITECTURE_DECISIONS.md` — binding, crate, portability, and API decisions.
6. `docs/superpowers/plans/` — executable phase plans and task-level acceptance criteria.

If these documents conflict, the architecture decisions and requirements take precedence over task prose. Do not silently reinterpret an accepted decision; propose an ADR amendment.

## Required workflow

Use GSD for milestone state and phase progression. Use Superpowers for design, TDD, systematic debugging, verification, and review discipline.

Before implementation:

- Read all governance files listed above.
- Record the exact base SHA.
- Run the baseline commands specified in Phase 00.
- Create an isolated worktree or branch.
- Select only one phase whose prerequisites are satisfied.
- Convert each task into a small red-green-refactor loop.

During implementation:

- Preserve a clean separation between the solver-independent core, safe backend adapters, and raw native bindings.
- Add a failing test before every behavior change or bug fix.
- Never infer an FFI signature, enum value, struct layout, callback rule, or native-library filename. Derive it from a pinned official header/API and verify it on supported targets.
- Never permit a Rust panic to cross an `extern "C"` boundary.
- Never swallow native return codes.
- Never mutate the model journal destructively until an adapter has acknowledged a revision.
- Never add a new public API without rustdoc, tests, and a semver rationale.
- Do not introduce language wrappers before the Rust core and C ABI boundary are release-qualified.

Before claiming completion:

- Run the full phase verification matrix.
- Run formatting, clippy with warnings denied, tests, docs, package checks, and relevant platform/backend jobs.
- Inspect `cargo package --list` for every publishable crate.
- Record commands and evidence in the phase completion note.
- Request an independent code review.
- Do not merge with unresolved P0/P1 findings or skipped mandatory gates.

## Release safety rules

- Do not publish any crate, create a Git tag, or push a release without explicit owner authorization.
- Do not commit proprietary solver binaries, headers, licenses, credentials, generated logs, or machine-specific paths.
- Keep MOSEK and Xpress tests license-aware and opt-in.
- Keep the core crate usable and testable without any native solver installed.
- Prefer official or established bindings over maintaining handwritten ABI declarations.
- Treat `unsafe impl Send/Sync`, callback trampolines, dynamic loading, and native library lifecycle as security-critical code requiring explicit invariants and dedicated tests.

## Completion definition

A phase is complete only when its acceptance criteria, automated checks, documentation updates, and evidence are all present. "Compiles on my Mac" is never sufficient evidence.