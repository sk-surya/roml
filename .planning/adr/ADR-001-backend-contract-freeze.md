# ADR-001: Backend Contract Freeze

**Date:** 2026-07-18
**Phase:** M1R-01 (Phase 10)
**Freeze Commit SHA:** `bf3ba70a3490acc60fa6e3c32fe0d64d8c44656a`

## Frozen Traits

### BackendSession (required -- every backend implements)
- `synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError>`
- `solve(&mut self, request: &SolveRequest) -> Result<SolveResult, BackendError>`
- `close(self) -> Result<(), BackendError>`

### SessionHealth (optional)
- `health(&self) -> AdapterHealth`
- `revision(&self) -> ModelRevision`

### SolutionView (optional)
- `value(&self, var: VarId) -> Option<f64>`
- `dual(&self, con: ConId) -> Option<f64>`
- `reduced_cost(&self, var: VarId) -> Option<f64>`
- `objective_value(&self) -> Option<f64>`

### CallbackSession (optional -- MIP backends)
- `set_callback_handler(handler: Box<dyn CallbackHandler>) -> Result<(), BackendError>`
- `clear_callback_handler() -> Result<(), BackendError>`

### BackendMetadata (optional)
- `name(&self) -> &str`
- `capabilities(&self) -> BackendCapabilities`

## Frozen Types
- SolveRequest, SolveResult, TerminationStatus (12 variants with Unknown default), EffectiveConfig, SolveSolution
- BackendError, ErrorCategory, HealthEffect, BackendCapabilities
- Synchronization, SyncReceipt, DeltaBatch, ModelOp (16 variants)
- ModelRevision, AdapterCursor, AdapterHealth, ApplyOutcome, Journal, ModelSnapshot

## Change Process
Any post-freeze edit to a frozen trait or type signature requires:
1. A recorded decision (update to this ADR or a new ADR).
2. Notification to all downstream workers (HiGHS, MOSEK, Xpress).
3. Coordinated rebase of all downstream branches.
