//! Solve overlay contract and compiler (issue #26 item 1; design §12).
//!
//! [`SolveOverlay`] carries solve-scoped restrictions — temporary fixings,
//! solution locks, objective-lock rows, and cutoffs — that never mutate the
//! canonical model (SM-07.3). Task 9 defines the packet shapes and the
//! [`compile_overlay`] compiler against the exact base [`CompilationId`];
//! Task 10 implements the transactional apply/rollback execution.
//!
//! The overlay is compiled against `C_base = compiler.current_compilation()`
//! and receives a FRESH `CompilationId` `C_overlay` for the
//! overlay-compounded state (D28). The overlay is **never** compiled into the
//! [`CompilationSession`] — `compiler.current_compilation()` stays `C_base`
//! throughout (design §12).

use std::collections::{BTreeMap, BTreeSet};

use crate::assignment::{AssignmentError, ContinuousLock, SolutionLock};
use crate::compiler::backend_ir::{
    CompilationId, CompiledConstraintId, CompiledEntityRef, CompiledEntityRegistry,
    CompiledLinearRow, CompiledObjectiveId, CompiledObjectivePolicy, CompiledVariableId,
};
use crate::compiler::origin::{EntityOrigin, GeneratedRole, OriginMap, OverlayId};
use crate::compiler::session::CompilationSession;
use crate::compiler::CompileError;
use crate::identity::IdentityOverflow;
use crate::model::{Bounds, ConstraintBounds, Model, Objective, Sense, VarType};
use crate::Variable;

/// A solve-scoped overlay (issue #26 item 1; design §12).
///
/// Contents are exactly the pinned shape: temporary fixings, solution locks,
/// objective-lock rows, and cutoffs, plus an opaque [`OverlayId`] allocated at
/// construction. Overlay application and rollback are transactional from the
/// caller's perspective (SM-07.4) and never advance the canonical model
/// revision (SM-07.3).
#[derive(Clone, Debug, PartialEq)]
pub struct SolveOverlay {
    /// Opaque overlay identity, allocated at construction through the checked
    /// atomic counter (zero reserved, typed [`IdentityOverflow`]).
    pub id: OverlayId,
    /// Solve-scoped variable fixings (SM-07.3) — distinct from persistent
    /// [`VariableFixing`](crate::VariableFixing). Apply as equal lower/upper
    /// compiled bounds.
    pub temporary_fixings: BTreeMap<Variable, f64>,
    /// Solution locks: each applies its selector over its assignment's values
    /// (SM-06.4) and restricts the selected variables per `ContinuousLock`
    /// (SM-06.5).
    pub locks: Vec<SolutionLock>,
    /// Temporary degradation rows for lexicographic stages (design §15.2).
    pub objective_locks: Vec<ObjectiveLock>,
    /// Temporary objective cutoff rows.
    pub cutoffs: Vec<ObjectiveCutoff>,
}

impl SolveOverlay {
    /// Construct a new overlay, allocating a fresh opaque [`OverlayId`].
    pub fn new(
        temporary_fixings: BTreeMap<Variable, f64>,
        locks: Vec<SolutionLock>,
        objective_locks: Vec<ObjectiveLock>,
        cutoffs: Vec<ObjectiveCutoff>,
    ) -> Result<Self, IdentityOverflow> {
        Ok(Self {
            id: OverlayId::allocate()?,
            temporary_fixings,
            locks,
            objective_locks,
            cutoffs,
        })
    }
}

/// One temporary objective-lock row (design §15.2).
///
/// The degradation row is `f(x) <= z + abs + rel*|z|` for a minimization stage
/// optimum `z`, and `f(x) >= z - abs - rel*|z|` for maximization. P27 declares
/// the type and compiles the row through [`compile_overlay`]; P31 supplies the
/// stage optimum `z` when executing lexicographic stages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectiveLock {
    /// The objective whose value is degraded.
    pub objective: Objective,
    /// Absolute degradation tolerance.
    pub absolute_tolerance: f64,
    /// Relative degradation tolerance (scales with `|z|`).
    pub relative_tolerance: f64,
}

/// A temporary objective cutoff row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectiveCutoff {
    /// The objective whose value is cut.
    pub objective: Objective,
    /// The cutoff limit.
    pub limit: f64,
    /// Whether the cutoff bounds the objective above or below.
    pub direction: CutoffDirection,
}

/// The direction of an [`ObjectiveCutoff`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CutoffDirection {
    /// `f(x) <= limit`.
    Upper,
    /// `f(x) >= limit`.
    Lower,
}

/// The compiled form of a [`SolveOverlay`] against one exact base
/// [`CompilationId`] (issue #26 item 1).
///
/// `base_compilation` is the exact state the overlay applies on top of;
/// `compilation_id` is a FRESH id for the overlay-compounded state (D28).
/// Every added temporary row carries a `SolveOverlay` origin (D5).
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledOverlay {
    /// The exact compiled state this overlay applies on top of.
    pub base_compilation: CompilationId,
    /// A fresh id for the overlay-compounded state (distinct from the base).
    pub compilation_id: CompilationId,
    /// The originating overlay.
    pub overlay_id: OverlayId,
    /// Ordered overlay operations.
    pub operations: Vec<OverlayOp>,
    /// Origins of the temporary rows added by this overlay (D5).
    pub origin_additions: OriginMap,
    /// The compiled objective-policy override, if any.
    pub objective_policy_override: Option<CompiledObjectivePolicy>,
}

/// One compiled overlay operation (design §12).
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum OverlayOp {
    /// Temporarily set a compiled variable's bounds.
    SetTemporaryVariableBounds {
        /// The compiled variable.
        variable: CompiledVariableId,
        /// The temporary bounds.
        bounds: Bounds,
    },
    /// Add a temporary compiled row (objective-lock or cutoff).
    AddTemporaryRow {
        /// The temporary row.
        row: CompiledLinearRow,
    },
    /// Remove a previously added temporary row (rollback).
    RemoveTemporaryRow {
        /// The temporary row to remove.
        row: CompiledConstraintId,
    },
    /// Override the active compiled objective policy.
    SetObjectivePolicy(CompiledObjectivePolicy),
}

/// The receipt returned by
/// [`OverlaySession::apply_overlay`](crate::solver::session::OverlaySession::apply_overlay)
/// (design §12; SM-07.4).
///
/// Explicit transactional apply/rollback receipts are the mechanism — a
/// fallible rollback is never delegated solely to `Drop`. The receipt records
/// the exact `(base, applied)` [`CompilationId`] pair so rollback can verify
/// the exact state it must restore (D28).
#[derive(Clone, Debug, PartialEq)]
pub struct OverlayApplyReceipt {
    /// The originating overlay.
    pub overlay_id: OverlayId,
    /// The exact compiled state the overlay was applied on top of (`C_base`).
    pub base_compilation: CompilationId,
    /// The overlay-compounded compiled state the backend now holds (`C_overlay`).
    pub applied_compilation: CompilationId,
}

/// The outcome of
/// [`OverlaySession::rollback_overlay`](crate::solver::session::OverlaySession::rollback_overlay)
/// (design §12, §19; SM-07.4, SM-07.5).
///
/// A [`Clean`](Self::Clean) outcome restores the exact base compiled state.
/// A [`RequiresRebuild`](Self::RequiresRebuild) outcome means the rollback
/// could not be proven clean — the session MUST be rebuilt before reuse
/// (D7 invariant: "rollback uncertainty forces backend rebuild"; D22).
#[derive(Clone, Debug, PartialEq)]
pub enum OverlayRollbackOutcome {
    /// The backend was restored to the exact base compiled state.
    Clean {
        /// The restored base `CompilationId`.
        restored_compilation: CompilationId,
    },
    /// The backend state is uncertain after the failed rollback; it must be
    /// rebuilt before the next solve.
    RequiresRebuild {
        /// Why the rollback could not be proven clean.
        reason: String,
    },
}

impl CompiledOverlay {
    /// All-or-nothing preflight validation of this overlay against `existing`
    /// (the backend's exact live compiled entity registry) BEFORE any native
    /// mutation (F2).
    ///
    /// Checks, in order:
    /// - the D28 envelope: [`compilation_id`](Self::compilation_id) !=
    ///   [`base_compilation`](Self::base_compilation) — the overlay-compounded
    ///   state must receive a FRESH exact id;
    /// - every op SIMULATED in operation order against a working registry
    ///   clone:
    ///   - `SetTemporaryVariableBounds` references a compiled variable present
    ///     in the live state;
    ///   - `AddTemporaryRow` rejects a duplicate/live row id, every
    ///     coefficient variable must be present, and the row MUST carry a
    ///     matching [`EntityOrigin::SolveOverlay`] origin in
    ///     [`origin_additions`](Self::origin_additions) (D5, SM-02.5);
    ///   - `RemoveTemporaryRow` is a rollback-only op and is REJECTED in the
    ///     apply stream;
    ///   - `SetObjectivePolicy` references only compiled objectives present in
    ///     the live state (no dangling policy);
    /// - a FUTURE `OverlayOp` variant (`#[non_exhaustive]`) is rejected as
    ///   unsupported.
    ///
    /// Backends run this BEFORE any native mutation, so a malformed overlay —
    /// even one whose ops are self-consistent only in a different order — never
    /// partially mutates the backend.
    pub fn validate(&self, existing: &CompiledEntityRegistry) -> Result<(), CompileError> {
        if self.compilation_id == self.base_compilation {
            return Err(CompileError::InvalidDeltaEnvelope {
                reason: "overlay compilation id equals its base compilation id (D28)".to_string(),
            });
        }

        let mut present = existing.clone();
        for op in &self.operations {
            match op {
                OverlayOp::SetTemporaryVariableBounds { variable, .. } => {
                    if !present.variables.contains(variable) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Variable(*variable),
                        });
                    }
                }
                OverlayOp::AddTemporaryRow { row } => {
                    if !present.rows.insert(row.id) {
                        return Err(CompileError::DuplicateEntity {
                            entity: CompiledEntityRef::Constraint(row.id),
                        });
                    }
                    // D5/SM-02.5: every added temporary row must trace to a
                    // SolveOverlay origin, and the embedded overlay MUST be
                    // THIS overlay (provenance check — a row attributed to
                    // another overlay while being applied as this one would
                    // misattribute the restriction).
                    let origin_matches = matches!(
                        self.origin_additions.constraint_origin(row.id),
                        Some(EntityOrigin::SolveOverlay { overlay, .. })
                            if *overlay == self.overlay_id
                    );
                    if !origin_matches {
                        return Err(CompileError::MissingOrigin {
                            entity: CompiledEntityRef::Constraint(row.id),
                        });
                    }
                    for (cid, _) in &row.coefficients {
                        if !present.variables.contains(cid) {
                            return Err(CompileError::InvalidReference {
                                entity: CompiledEntityRef::Variable(*cid),
                            });
                        }
                    }
                }
                OverlayOp::RemoveTemporaryRow { row } => {
                    return Err(CompileError::InvalidDeltaEnvelope {
                        reason: format!(
                            "RemoveTemporaryRow ({row:?}) is a rollback-only op and is not \
                             accepted in the apply stream"
                        ),
                    });
                }
                OverlayOp::SetObjectivePolicy(policy) => {
                    if let Some(id) = dangling_policy_objective(policy, &present.objectives) {
                        return Err(CompileError::InvalidReference {
                            entity: CompiledEntityRef::Objective(id),
                        });
                    }
                } // `OverlayOp` is `#[non_exhaustive]`: this match is exhaustive
                  // within the defining crate, so a FUTURE variant fails to
                  // compile here (a compile-time rejection) AND is rejected at
                  // runtime by each backend's apply loop (which carries the
                  // required wildcard arm).
            }
        }
        Ok(())
    }
}

/// Return the first compiled objective id referenced by `policy` that is not
/// present in `objectives`, if any (F2 — the apply-stream analogue of the
/// snapshot builder's dangling-policy check).
fn dangling_policy_objective(
    policy: &CompiledObjectivePolicy,
    objectives: &BTreeSet<CompiledObjectiveId>,
) -> Option<CompiledObjectiveId> {
    match policy {
        CompiledObjectivePolicy::None => None,
        CompiledObjectivePolicy::Single(id) => {
            if objectives.contains(id) {
                None
            } else {
                Some(*id)
            }
        }
        CompiledObjectivePolicy::Weighted(items) => items.iter().find_map(|item| {
            if objectives.contains(&item.objective) {
                None
            } else {
                Some(item.objective)
            }
        }),
        CompiledObjectivePolicy::Lexicographic(items) => items.iter().find_map(|item| {
            if objectives.contains(&item.objective) {
                None
            } else {
                Some(item.objective)
            }
        }),
    }
}

/// Error compiling (or, in Task 10, applying) a [`SolveOverlay`]
/// (design §19; SM-06.6; issue #26 item 1).
#[derive(Clone, Debug, PartialEq)]
pub enum OverlayError {
    /// The overlay's exact base compiled state is stale or absent.
    ///
    /// `expected` is the base the overlay requires; `actual` is the state the
    /// compiler/backend actually holds. `None` means "no compiled state".
    StaleCompilation {
        /// The base the overlay was compiled against (or `None` if none).
        expected: Option<CompilationId>,
        /// The actual current compiled state (or `None` if none).
        actual: Option<CompilationId>,
    },
    /// An assignment or lock failed validation (SM-06.6).
    Assignment(AssignmentError),
    /// An objective referenced by an objective lock, cutoff, or override is
    /// absent from the model or the compiled base.
    ObjectiveNotFound(Objective),
    /// Overlay identity allocation failed (counter exhausted).
    IdentityOverflow,
    /// A `Within` band was applied to an integer/binary variable — an integer
    /// band cannot round-trip exactly.
    WithinBandOnNonContinuous {
        /// The affected variable.
        variable: Variable,
    },
    /// A `Within` band has an invalid absolute half-width (non-finite or
    /// negative).
    InvalidLockBand {
        /// The affected variable.
        variable: Variable,
        /// The invalid absolute half-width.
        absolute: f64,
    },
    /// An [`ObjectiveLock`] has an invalid degradation tolerance (non-finite or
    /// negative). IN-02: validated at compile time — a negative
    /// `absolute_tolerance` would silently produce a loosened/absurd
    /// degradation row, and a non-finite tolerance corrupts the row RHS.
    InvalidObjectiveLock {
        /// The objective whose degradation row is invalid.
        objective: Objective,
        /// The invalid absolute degradation tolerance.
        absolute_tolerance: f64,
        /// The invalid relative degradation tolerance.
        relative_tolerance: f64,
    },
    /// The overlay restrictions on a variable, intersected against its
    /// canonical EFFECTIVE base bounds, produce an EMPTY feasible region (F1).
    ///
    /// A temporary fixing / lock is a feasible-region RESTRICTION
    /// (SM-06.3/06.5) and can never widen or contradict the effective base
    /// (declared ∩ persistent fixing ∩ activity). The overlay is rejected
    /// during compilation, before any backend mutation.
    InfeasibleOverlay {
        /// The affected variable.
        variable: Variable,
        /// The accumulated intersection at the point it became empty.
        bounds: Bounds,
    },
    /// An [`ObjectiveCutoff`] has a non-finite limit (NaN or ±inf) — it would
    /// corrupt the temporary row RHS (`rhs = limit - constant`) and reach a
    /// backend constraint row. F4: rejected at compile time before any op is
    /// produced.
    InvalidCutoff {
        /// The objective whose cutoff row is invalid.
        objective: Objective,
        /// The invalid cutoff limit.
        limit: f64,
    },
}

/// Compile a [`SolveOverlay`] against the compiler's exact current compiled
/// state into a [`CompiledOverlay`] (issue #26 item 1).
///
/// Read-only on both `model` and `compiler`. The overlay is compiled against
/// `compiler.current_compilation()` (the exact `C_base`); a stale base — no
/// compiled state, or a compiler bound to a different model instance — is a
/// typed [`OverlayError::StaleCompilation`] rejected before any op is
/// produced. Assignment/band/value validation also happens before any op
/// (SM-06.6), and every added temporary row receives a `SolveOverlay` origin
/// (D5).
pub fn compile_overlay(
    model: &Model,
    compiler: &CompilationSession,
    overlay: &SolveOverlay,
    objective_override: Option<Objective>,
) -> Result<CompiledOverlay, OverlayError> {
    // The exact base: the compiler's current compiled state (D28).
    let base_compilation =
        compiler
            .current_compilation()
            .ok_or(OverlayError::StaleCompilation {
                expected: None,
                actual: None,
            })?;

    // The compiled base belongs to ONE model instance. An overlay compiled
    // against another model's base would silently map wrong compiled ids —
    // reject as stale before any op is produced.
    if let Some(recorded) = compiler.source_instance() {
        if recorded != model.instance() {
            return Err(OverlayError::StaleCompilation {
                expected: Some(base_compilation),
                actual: None,
            });
        }
    }

    let mut operations: Vec<OverlayOp> = Vec::new();
    let mut origin_additions = OriginMap::new();
    let mut next_row = compiler.next_row_index().unwrap_or(0);

    // ── 1+2. temporary fixings + locks → at most ONE SetTemporaryVariableBounds
    // op per variable, carrying the INTERSECTION of every restriction with the
    // canonical EFFECTIVE base bounds (F1) ──────────────────────────────────
    //
    // Previously each temporary fixing and each lock emitted its own
    // `SetTemporaryVariableBounds` op, applied by REPLACEMENT in overlay
    // order — order-dependent, with a later restriction OVERWRITING an earlier
    // one and even LOOSENING the base (a persistently fixed or inactive
    // variable's effective base could be widened). Now all restrictions on a
    // variable are AGGREGATED first and intersected deterministically against
    // the effective base (`effective_bounds()`: declared ∩ persistent fixing ∩
    // activity); at most ONE op is emitted per variable, and an EMPTY
    // intersection is rejected as [`OverlayError::InfeasibleOverlay`] before
    // any op is produced.
    let mut variable_restrictions: BTreeMap<Variable, Bounds> = BTreeMap::new();

    for (variable, value) in &overlay.temporary_fixings {
        validate_value_in_domain(model, *variable, *value)?;
        intersect_overlay_restriction(
            model,
            *variable,
            Bounds::new(*value, *value),
            &mut variable_restrictions,
        )?;
    }

    for lock in &overlay.locks {
        for (variable, value) in lock.resolve(model).map_err(OverlayError::Assignment)? {
            let bounds = match lock.continuous {
                ContinuousLock::Exact => Bounds::new(value, value),
                ContinuousLock::Within { absolute } => {
                    continuous_band_bounds(model, variable, value, absolute)?
                }
            };
            intersect_overlay_restriction(model, variable, bounds, &mut variable_restrictions)?;
        }
    }

    // Emit one temporary-bound op per restricted variable, deterministically
    // ordered by variable (BTreeMap iteration is sorted by `Variable`).
    for (variable, bounds) in variable_restrictions {
        let compiled =
            compiler
                .compiled_variable_id(variable)
                .ok_or(OverlayError::StaleCompilation {
                    expected: Some(base_compilation),
                    actual: None,
                })?;
        operations.push(OverlayOp::SetTemporaryVariableBounds {
            variable: compiled,
            bounds,
        });
    }

    // ── 3. objective_locks → AddTemporaryRow (degradation row) ───────────
    for lock in &overlay.objective_locks {
        // IN-02: validate both tolerances (finite, non-negative) at compile
        // time — a negative absolute tolerance would silently produce a
        // loosened/absurd degradation row, and a non-finite one corrupts the
        // row RHS.
        if !lock.absolute_tolerance.is_finite()
            || lock.absolute_tolerance < 0.0
            || !lock.relative_tolerance.is_finite()
            || lock.relative_tolerance < 0.0
        {
            return Err(OverlayError::InvalidObjectiveLock {
                objective: lock.objective,
                absolute_tolerance: lock.absolute_tolerance,
                relative_tolerance: lock.relative_tolerance,
            });
        }
        let (coefficients, constant) = objective_compiled_terms(compiler, lock.objective)?;
        // P27 compiles the degradation row with a zero reference optimum `z`
        // (the row RHS is the absolute tolerance; P31 supplies the real stage
        // optimum and the relative term). Direction follows the objective's
        // sense (design §15.2).
        let bounds = match model
            .objective_sense(lock.objective)
            .unwrap_or(Sense::Minimize)
        {
            Sense::Minimize => ConstraintBounds::le(lock.absolute_tolerance - constant),
            Sense::Maximize => ConstraintBounds::ge(-lock.absolute_tolerance - constant),
        };
        let row_id = CompiledConstraintId(next_row);
        next_row += 1;
        origin_additions.insert_constraint(
            row_id,
            EntityOrigin::SolveOverlay {
                overlay: overlay.id,
                role: GeneratedRole::ObjectiveLockRow,
            },
        );
        operations.push(OverlayOp::AddTemporaryRow {
            row: CompiledLinearRow {
                id: row_id,
                bounds,
                coefficients,
                name: None,
            },
        });
    }

    // ── 4. cutoffs → AddTemporaryRow ─────────────────────────────────────
    for cutoff in &overlay.cutoffs {
        // F4: a non-finite cutoff limit must never reach a backend row RHS —
        // `rhs = limit - constant` with NaN/±inf would corrupt the native
        // constraint. Rejected at compile time, before any op is produced.
        if !cutoff.limit.is_finite() {
            return Err(OverlayError::InvalidCutoff {
                objective: cutoff.objective,
                limit: cutoff.limit,
            });
        }
        let (coefficients, constant) = objective_compiled_terms(compiler, cutoff.objective)?;
        let rhs = cutoff.limit - constant;
        let bounds = match cutoff.direction {
            CutoffDirection::Upper => ConstraintBounds::le(rhs),
            CutoffDirection::Lower => ConstraintBounds::ge(rhs),
        };
        let row_id = CompiledConstraintId(next_row);
        next_row += 1;
        origin_additions.insert_constraint(
            row_id,
            EntityOrigin::SolveOverlay {
                overlay: overlay.id,
                role: GeneratedRole::CutoffRow,
            },
        );
        operations.push(OverlayOp::AddTemporaryRow {
            row: CompiledLinearRow {
                id: row_id,
                bounds,
                coefficients,
                name: None,
            },
        });
    }

    // ── 5. objective override → SetObjectivePolicy(Single) ───────────────
    let objective_policy_override = match objective_override {
        Some(obj) => {
            let compiled = compiler
                .compiled_objective_id(obj)
                .ok_or(OverlayError::ObjectiveNotFound(obj))?;
            let policy = CompiledObjectivePolicy::Single(compiled);
            operations.push(OverlayOp::SetObjectivePolicy(policy.clone()));
            Some(policy)
        }
        None => None,
    };

    // The overlay-compounded state receives a FRESH exact id (D28), distinct
    // from the base — an override solve is recorded by `C_overlay`, never
    // `C_base`.
    let compilation_id = CompilationId::allocate().map_err(|_| OverlayError::IdentityOverflow)?;

    Ok(CompiledOverlay {
        base_compilation,
        compilation_id,
        overlay_id: overlay.id,
        operations,
        origin_additions,
        objective_policy_override,
    })
}

/// Validate an assigned value against the model's canonical EFFECTIVE bounds
/// (tolerance-aware for integrality) — SM-06.6's "fail before any op".
///
/// F1(e): the gate is the effective bounds (declared ∩ persistent fixing ∩
/// activity), NOT the declared domain — a temporary fixing that attempts to
/// move a persistently fixed variable (effective `[fix, fix]`) or an inactive
/// variable (effective `[0, 0]`) must be rejected, never silently compiled
/// into a bounds op that LOOSENS the effective base.
fn validate_value_in_domain(
    model: &Model,
    variable: Variable,
    value: f64,
) -> Result<(), OverlayError> {
    let domain = model
        .variable_domain(variable)
        .ok_or(OverlayError::Assignment(AssignmentError::StaleVariable {
            variable,
        }))?;
    let bounds = model
        .effective_bounds(variable)
        .ok_or(OverlayError::Assignment(AssignmentError::StaleVariable {
            variable,
        }))?;
    // CR-01: a non-finite value is rejected FIRST — NaN passes both range
    // comparisons (both false) and +inf passes when the upper bound is itself
    // infinite. The overlay compiles to `Bounds::new(value, value)` pushed into
    // `Highs_changeColBounds`, so a NaN/±inf value must never survive compile
    // time.
    if !value.is_finite() {
        return Err(OverlayError::Assignment(AssignmentError::NonFiniteValue {
            variable,
            value,
        }));
    }
    if value < bounds.lower || value > bounds.upper {
        return Err(OverlayError::Assignment(
            AssignmentError::ValueOutOfBounds {
                variable,
                value,
                bounds,
            },
        ));
    }
    if matches!(domain.var_type, VarType::Integer | VarType::Binary) {
        let nearest = value.round();
        if (value - nearest).abs() > model.integrality_tolerance() {
            return Err(OverlayError::Assignment(
                AssignmentError::ValueOutOfBounds {
                    variable,
                    value,
                    bounds,
                },
            ));
        }
    }
    Ok(())
}

/// Produce the `[v - absolute, v + absolute]` band for a continuous variable,
/// rejecting a `Within` band on an integer/binary variable (an integer band
/// cannot round-trip exactly) and an invalid half-width.
fn continuous_band_bounds(
    model: &Model,
    variable: Variable,
    value: f64,
    absolute: f64,
) -> Result<Bounds, OverlayError> {
    if !absolute.is_finite() || absolute < 0.0 {
        return Err(OverlayError::InvalidLockBand { variable, absolute });
    }
    let domain = model
        .variable_domain(variable)
        .ok_or(OverlayError::Assignment(AssignmentError::StaleVariable {
            variable,
        }))?;
    if !matches!(domain.var_type, VarType::Continuous) {
        return Err(OverlayError::WithinBandOnNonContinuous { variable });
    }
    // WR-01 + F1: a lock is a feasible-region RESTRICTION (SM-06.3/06.5) and
    // must never LOOSEN a bound — INTERSECT the band with the canonical
    // EFFECTIVE bounds (declared ∩ persistent fixing ∩ activity), not merely
    // the declared domain. Without the clip, a band extending past a bound
    // (e.g. value 1.0, absolute 2.0 on `[0,10]` -> raw `[-1,3]`) lets the
    // overlay solve return a solution violating the effective bounds; and an
    // inactive variable's effective `[0, 0]` base can never be widened.
    let effective = model
        .effective_bounds(variable)
        .ok_or(OverlayError::Assignment(AssignmentError::StaleVariable {
            variable,
        }))?;
    let lower = (value - absolute).max(effective.lower);
    let upper = (value + absolute).min(effective.upper);
    Ok(Bounds::new(lower, upper))
}

/// Accumulate one overlay restriction on `variable` into `acc`, intersecting
/// it (and any prior restrictions) against the canonical EFFECTIVE base
/// bounds (F1).
///
/// The base is `model.effective_bounds(variable)` — declared ∩ persistent
/// fixing ∩ activity — never the declared bounds alone, so a persistently
/// fixed or inactive variable's effective base can never be loosened. An
/// EMPTY intersection (a restriction that contradicts the base or another
/// restriction) is a typed [`OverlayError::InfeasibleOverlay`], rejected
/// before any op is produced.
fn intersect_overlay_restriction(
    model: &Model,
    variable: Variable,
    restriction: Bounds,
    acc: &mut BTreeMap<Variable, Bounds>,
) -> Result<(), OverlayError> {
    let base = model
        .effective_bounds(variable)
        .ok_or(OverlayError::Assignment(AssignmentError::StaleVariable {
            variable,
        }))?;
    let current = acc.get(&variable).copied().unwrap_or(base);
    let lower = current.lower.max(restriction.lower);
    let upper = current.upper.min(restriction.upper);
    let intersected = Bounds::new(lower, upper);
    if lower > upper {
        return Err(OverlayError::InfeasibleOverlay {
            variable,
            bounds: intersected,
        });
    }
    acc.insert(variable, intersected);
    Ok(())
}

/// Resolve the compiled coefficients and constant of `objective` for a
/// temporary row (the compiled row for `f(x)`).
///
/// WR-03: the coefficients come from the COMPILED BASE (`compiler`'s evaluated
/// objective coefficient vector) — the exact values the backend holds — never
/// by re-deriving from the canonical symbolic expression. `as_constant()` on a
/// parameterized/composite coefficient returns `None` and previously produced a
/// misleading `ObjectiveNotFound` for a legitimate compiled objective.
/// `ObjectiveNotFound` is now reported ONLY when the objective is genuinely
/// absent from the compiled base.
fn objective_compiled_terms(
    compiler: &CompilationSession,
    objective: Objective,
) -> Result<(Vec<(CompiledVariableId, f64)>, f64), OverlayError> {
    let (mut coefficients, constant) = compiler
        .compiled_objective_terms(objective)
        .ok_or(OverlayError::ObjectiveNotFound(objective))?;
    coefficients.sort_by_key(|(id, _)| *id);
    Ok((coefficients, constant))
}
