# Phase 35 MPS Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` to execute the DAG task-by-task with review after each task.

**Goal:** Import fixed/free linear LP/MILP MPS into ROML transactionally and qualify semantics, provenance, HiGHS interoperability, Netlib, and Chinneck IIS workflows.

**Architecture:** Streaming records feed a private MPS staging document. All records receive structural validation; selected vectors receive model-semantic validation. Resolution creates a fresh ROML model plus source metadata. The core remains solver-free.

**Tech Stack:** Rust 1.85, standard-library streaming I/O, existing ROML model/compiler APIs, `roml-highs` for independent differential qualification, optional pinned external corpora.

## Global Constraints

- Use a handwritten parser; do not add LALRPOP or another parser generator.
- Support linear LP/MILP only; reject unsupported semantic sections explicitly.
- Keep parser code solver-free and non-panicking on malformed input.
- Preserve all named rim vectors in staging and apply semantic validation only after selection.
- Treat ROML’s frozen MPS semantics as normative; HiGHS is an independent oracle.
- Give implicit finite bounds synthetic provenance suitable for P29 IIS reporting.
- Extract archives with pre-write path/link/special-file rejection and atomic promotion.
- Keep P36 writer production code out of this phase.

## Execution

Use the DAG and task briefs in `.planning/phases/35-mps-import/35-PLAN.md`. Execute Wave 1 tasks 35-01, 35-02, and 35-03 concurrently only after 35-00. Execute later waves only after their dependency gates and task reviews pass. Do not assign overlapping file ownership to concurrent agents.

## Verification

Run focused tests after each task. At phase completion run the full applicable matrix from Task 35-09 and record exact outputs in `docs/release/evidence/P35_MPS_QUALIFICATION.md`.
