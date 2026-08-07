//! Semantic restriction atoms and exact compiled-state conflict universes.

use crate::compiler::backend_ir::BackendSnapshot;
use crate::compiler::origin::EntityOrigin;
use crate::snapshot::ModelSnapshot;
use crate::solver::infeasibility::{
    BoundSide, CompiledRestrictionRef, ConflictAtomId, ConflictAtomKind, ConflictGrouping,
    ConflictMemberSnapshot, ConflictOrigin, InfeasibilityError, InfeasibilityScope,
    RestrictionToggleAction, RestrictionTogglePlan, SemanticConflictUniverse,
    SemanticRestrictionAtom,
};

impl SemanticConflictUniverse {
    /// Build a deterministic side-level universe from one exact snapshot.
    pub fn from_snapshot(
        snapshot: &BackendSnapshot,
        _scope: InfeasibilityScope,
        grouping: ConflictGrouping,
    ) -> Result<Self, InfeasibilityError> {
        Self::from_snapshot_with_fixings(snapshot, None, _scope, grouping)
    }

    /// Build a universe with canonical persistent-fixing layers retained.
    ///
    /// The compiled variable bounds contain the effective fixing, while the
    /// canonical snapshot retains the declared lower layer. Keeping both here
    /// lets an isolated oracle disable a fixing and restore the declared bound
    /// rather than relaxing the variable to infinity.
    pub fn from_model_snapshot(
        snapshot: &BackendSnapshot,
        canonical: &ModelSnapshot,
        scope: InfeasibilityScope,
        grouping: ConflictGrouping,
    ) -> Result<Self, InfeasibilityError> {
        Self::from_snapshot_with_fixings(snapshot, Some(canonical), scope, grouping)
    }

    fn from_snapshot_with_fixings(
        snapshot: &BackendSnapshot,
        canonical: Option<&ModelSnapshot>,
        _scope: InfeasibilityScope,
        grouping: ConflictGrouping,
    ) -> Result<Self, InfeasibilityError> {
        let mut atoms = Vec::new();
        let mut compiled_restrictions = Vec::new();

        for row in &snapshot.linear_rows {
            let origin = snapshot
                .origin_map
                .constraint_origin(row.id)
                .ok_or_else(|| InfeasibilityError::InvalidUniverse {
                    reason: format!("compiled row {:?} has no semantic origin", row.id),
                })?;
            if row.bounds.lower.is_finite() {
                push_atom(
                    &mut atoms,
                    &mut compiled_restrictions,
                    ConflictAtomKind::ConstraintSide(BoundSide::Lower),
                    constraint_origin(origin, BoundSide::Lower)?,
                    CompiledRestrictionRef::ConstraintLower(row.id),
                    row.name.clone(),
                    Some(row.bounds.lower),
                );
            }
            if row.bounds.upper.is_finite() {
                push_atom(
                    &mut atoms,
                    &mut compiled_restrictions,
                    ConflictAtomKind::ConstraintSide(BoundSide::Upper),
                    constraint_origin(origin, BoundSide::Upper)?,
                    CompiledRestrictionRef::ConstraintUpper(row.id),
                    row.name.clone(),
                    Some(row.bounds.upper),
                );
            }
        }

        for variable in &snapshot.variables {
            let origin = snapshot
                .origin_map
                .variable_origin(variable.id)
                .ok_or_else(|| InfeasibilityError::InvalidUniverse {
                    reason: format!("compiled variable {:?} has no semantic origin", variable.id),
                })?;
            if variable.bounds.lower.is_finite() {
                push_atom(
                    &mut atoms,
                    &mut compiled_restrictions,
                    ConflictAtomKind::VariableBound(BoundSide::Lower),
                    variable_origin(origin, BoundSide::Lower)?,
                    CompiledRestrictionRef::VariableLower(variable.id),
                    variable.name.clone(),
                    Some(variable.bounds.lower),
                );
            }
            if variable.bounds.upper.is_finite() {
                push_atom(
                    &mut atoms,
                    &mut compiled_restrictions,
                    ConflictAtomKind::VariableBound(BoundSide::Upper),
                    variable_origin(origin, BoundSide::Upper)?,
                    CompiledRestrictionRef::VariableUpper(variable.id),
                    variable.name.clone(),
                    Some(variable.bounds.upper),
                );
            }
        }

        if grouping == ConflictGrouping::ByConstruct {
            // P32/P33 generated rows are represented as grouped origins. The
            // current identity compiler emits no native construct atoms, so
            // preserving individual rows is lossless until generated roles
            // become a multi-row bridge input.
        }

        let mut universe = Self {
            compilation_id: snapshot.compilation_id,
            atoms,
            compiled_restrictions,
            grouping,
        };

        if let Some(canonical) = canonical {
            for variable in canonical.variables.iter().filter(|v| v.fixing.is_some()) {
                let compiled = snapshot
                    .origin_map
                    .variables_for_origin(&EntityOrigin::UserVariable(variable.id))
                    .into_iter()
                    .next()
                    .ok_or_else(|| InfeasibilityError::InvalidUniverse {
                        reason: format!(
                            "persistent fixing variable {:?} has no compiled id",
                            variable.id
                        ),
                    })?;
                let fixing = variable.fixing.as_ref().expect("filtered fixing");

                // The regular variable-bound atoms represent the declared
                // layer when a fixing exists; the fixing atom is the later
                // contribution that overrides both sides.
                for atom in &mut universe.atoms {
                    if matches!(atom.kind, ConflictAtomKind::VariableBound(_))
                        && matches!(atom.origin, ConflictOrigin::VariableBound { variable: v, .. } if v == variable.id)
                    {
                        atom.snapshot.value = match atom.kind {
                            ConflictAtomKind::VariableBound(BoundSide::Lower) => {
                                Some(variable.bounds.lower)
                            }
                            ConflictAtomKind::VariableBound(BoundSide::Upper) => {
                                Some(variable.bounds.upper)
                            }
                            _ => atom.snapshot.value,
                        };
                    }
                }

                let id = ConflictAtomId(universe.atoms.len() as u64);
                let lower = CompiledRestrictionRef::VariableLower(compiled);
                let upper = CompiledRestrictionRef::VariableUpper(compiled);
                universe.compiled_restrictions.extend([lower, upper]);
                universe.atoms.push(SemanticRestrictionAtom {
                    id,
                    kind: ConflictAtomKind::PersistentFixing,
                    origin: ConflictOrigin::PersistentFixing {
                        variable: variable.id,
                    },
                    compiled_restrictions: vec![lower, upper],
                    disable: RestrictionTogglePlan {
                        atom_id: id,
                        action: RestrictionToggleAction::Disable,
                    },
                    restore: RestrictionTogglePlan {
                        atom_id: id,
                        action: RestrictionToggleAction::Restore,
                    },
                    snapshot: ConflictMemberSnapshot {
                        origin: ConflictOrigin::PersistentFixing {
                            variable: variable.id,
                        },
                        name: None,
                        value: Some(fixing.value),
                    },
                });
            }
        }

        Ok(universe)
    }
}

fn push_atom(
    atoms: &mut Vec<SemanticRestrictionAtom>,
    compiled_restrictions: &mut Vec<CompiledRestrictionRef>,
    kind: ConflictAtomKind,
    origin: ConflictOrigin,
    compiled: CompiledRestrictionRef,
    name: Option<String>,
    value: Option<f64>,
) {
    let id = ConflictAtomId(atoms.len() as u64);
    compiled_restrictions.push(compiled);
    let snapshot = ConflictMemberSnapshot {
        origin: origin.clone(),
        name,
        value,
    };
    atoms.push(SemanticRestrictionAtom {
        id,
        kind,
        origin,
        compiled_restrictions: vec![compiled],
        disable: RestrictionTogglePlan {
            atom_id: id,
            action: RestrictionToggleAction::Disable,
        },
        restore: RestrictionTogglePlan {
            atom_id: id,
            action: RestrictionToggleAction::Restore,
        },
        snapshot,
    });
}

fn constraint_origin(
    origin: &EntityOrigin,
    side: BoundSide,
) -> Result<ConflictOrigin, InfeasibilityError> {
    match origin {
        EntityOrigin::UserConstraint(constraint) => Ok(ConflictOrigin::ConstraintSide {
            constraint: *constraint,
            side,
        }),
        EntityOrigin::Construct { construct, role } => Ok(ConflictOrigin::GroupedConstruct {
            construct: *construct,
            role: *role,
        }),
        EntityOrigin::SolveOverlay { overlay, role } => Err(InfeasibilityError::InvalidUniverse {
            reason: format!(
                "solve overlay origin {:?}/{:?} requires an overlay scope",
                overlay, role
            ),
        }),
        _ => Err(InfeasibilityError::InvalidUniverse {
            reason: "compiled row has a non-constraint origin".to_string(),
        }),
    }
}

fn variable_origin(
    origin: &EntityOrigin,
    side: BoundSide,
) -> Result<ConflictOrigin, InfeasibilityError> {
    match origin {
        EntityOrigin::UserVariable(variable) => Ok(ConflictOrigin::VariableBound {
            variable: *variable,
            side,
        }),
        EntityOrigin::Construct { construct, role } => Ok(ConflictOrigin::GroupedConstruct {
            construct: *construct,
            role: *role,
        }),
        EntityOrigin::SolveOverlay { overlay, role } => Err(InfeasibilityError::InvalidUniverse {
            reason: format!(
                "solve overlay origin {:?}/{:?} requires an overlay scope",
                overlay, role
            ),
        }),
        _ => Err(InfeasibilityError::InvalidUniverse {
            reason: "compiled variable has a non-variable origin".to_string(),
        }),
    }
}
