//! User-facing HiGHS solver façade (D3, API-01).
//!
//! [`Highs`] is the golden-path solver type for M2: a thin wrapper around
//! [`SolverSession`] backed by a [`HighsSession`]. It exposes only
//! `new`/`solve`/`solve_with` — ordinary users never construct snapshots,
//! delta batches, cursors, or call `BackendSession::synchronize` (D2/D5).
//!
//! # Quick Start
//!
//! The canonical M2 quickstart (compiled and run by this crate's doctests):
//!
//! ```rust
//! use roml::prelude::*;
//! use roml_highs::Highs;
//!
//! # fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let mut model = Model::named("production");
//! let x = model.add_variable(continuous().named("x"))?;
//! let y = model.add_variable(integer().bounds(0.0, 10.0).named("y"))?;
//! model.add_constraint((x + y).le(4.0).named("capacity"))?;
//! model.maximize(3.0 * x + y)?;
//!
//! let mut highs = Highs::new()?;
//! let solution = highs.solve(&mut model)?;
//! assert!(solution.status().is_optimal());
//! # Ok(())
//! # }
//! # run().unwrap();
//! ```
//!
//! Re-solve incrementally on the same instance after mutating the model;
//! synchronization is automatic (parameter updates apply as deltas, and
//! unsupported changes recover via one snapshot rebuild).

use roml::id::ObjId;
use roml::model::Model;
use roml::solution::Solution;
use roml::solver::facade::SolverSession;
use roml::solver::options::SolveOptions;
use roml::solver::overlay::SolveOverlay;
use roml::solver::plan::SolvePlan;
use roml::solver::SolveError;

use crate::error::HighsError;
use crate::lifecycle::HighsSession;

/// The golden-path HiGHS solver façade.
///
/// Wraps [`SolverSession`]`<`[`HighsSession`]`>` and re-exposes the three
/// user-facing entry points. The wrapped [`HighsSession`] remains available
/// on the crate's public surface for framework authors and tests (D3).
pub struct Highs {
    inner: SolverSession<HighsSession>,
}

impl Highs {
    /// Create a new HiGHS-backed solve session.
    ///
    /// # Errors
    ///
    /// Returns [`HighsError`] when the native HiGHS session cannot be
    /// initialized (for example, the HiGHS library is not available).
    pub fn new() -> Result<Self, HighsError> {
        let session = HighsSession::try_new()?;
        Ok(Self {
            inner: SolverSession::new(session),
        })
    }

    /// Solve the current model with default options.
    ///
    /// Commits pending model mutations, synchronizes (delta or rebuild), and
    /// returns the normalized [`Solution`]. Mathematical terminations such
    /// as infeasible return `Ok(Solution)` with no primal values; operational
    /// failures return `Err` (API-03.3).
    ///
    /// # Errors
    ///
    /// Returns [`SolveError`] when the model cannot be committed, options are
    /// invalid, synchronization fails (after at most one rebuild retry), the
    /// backend solve fails, or the native termination is uninterpretable.
    /// No previously computed solution is ever reported as current after an
    /// error.
    pub fn solve(&mut self, model: &mut Model) -> Result<Solution, SolveError> {
        self.inner.solve(model)
    }

    /// Solve the current model with explicit [`SolveOptions`].
    ///
    /// Options are validated before any synchronization, so a failed
    /// validation leaves the model and backend state unchanged.
    ///
    /// # Errors
    ///
    /// Same failure classes as [`Highs::solve`]; additionally returns
    /// [`SolveError::InvalidOptions`] when `options` fails validation (for
    /// example a negative gap or non-positive thread count).
    pub fn solve_with(
        &mut self,
        model: &mut Model,
        options: SolveOptions,
    ) -> Result<Solution, SolveError> {
        self.inner.solve_with(model, options)
    }

    /// Execute a [`SolvePlan`] through the single plan executor (P28;
    /// design §12, SM-07.1).
    ///
    /// The full solve-attempt contract: plan validation, unsupported-feature
    /// policy, reversible overlays, warm starts/hints, and effective-plan
    /// recording — see [`SolverSession::solve_plan`]. An empty plan is
    /// exactly `solve`/`solve_with`.
    ///
    /// # Errors
    ///
    /// Returns [`SolveError::Plan`] when the plan fails validation or
    /// requests a feature the backend does not qualify under the plan's
    /// default-reject policy, plus the ordinary
    /// [`solve_with`](Highs::solve_with) failure classes.
    pub fn solve_plan(
        &mut self,
        model: &mut Model,
        plan: SolvePlan,
    ) -> Result<Solution, SolveError> {
        self.inner.solve_plan(model, plan)
    }

    /// Solve under a reversible [`SolveOverlay`] (P27; design §12, SM-07.3).
    ///
    /// Temporary fixings, solution locks, objective-lock rows, and cutoffs
    /// apply for this solve attempt only and are rolled back (and verified)
    /// afterward — the canonical model is never mutated by the overlay, and
    /// the returned metadata reports the overlay identity.
    ///
    /// # Errors
    ///
    /// Returns [`SolveError::Overlay`] when the overlay fails to compile
    /// (before any backend mutation), [`SolveError::Rollback`] on an
    /// uncertain rollback, plus the ordinary
    /// [`solve_with`](Highs::solve_with) failure classes.
    pub fn solve_with_overlay(
        &mut self,
        model: &mut Model,
        options: SolveOptions,
        overlay: &SolveOverlay,
        objective_override: Option<ObjId>,
    ) -> Result<Solution, SolveError> {
        self.inner
            .solve_with_overlay(model, options, overlay, objective_override)
    }
}
