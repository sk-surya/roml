//! Semantic restriction atoms and exact compiled-state conflict universes.

use crate::compiler::backend_ir::BackendSnapshot;
use crate::compiler::origin::{EntityOrigin, GeneratedRole};
use crate::construct::Construct;
use crate::model::Model;
use crate::snapshot::ModelSnapshot;
use crate::solver::infeasibility::{
    BoundSide, CompiledRestrictionRef, ConflictAtomId, ConflictAtomKind, ConflictGrouping,
    ConflictMemberSnapshot, ConflictOrigin, InfeasibilityError, InfeasibilityScope,
    RestrictionToggleAction, RestrictionTogglePlan, SemanticConflictUniverse,
    SemanticRestrictionAtom,
};
use crate::solver::overlay::SolveOverlay;

type GroupedRestrictions = Vec<(
    Construct,
    GeneratedRole,
    Vec<(CompiledRestrictionRef, Option<f64>)>,
)>;

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
    pub fn from_model_snapshot(
        snapshot: &BackendSnapshot,
        canonical: &ModelSnapshot,
        scope: InfeasibilityScope,
        grouping: ConflictGrouping,
    ) -> Result<Self, InfeasibilityError> {
        Self::from_snapshot_with_fixings(snapshot, Some(canonical), scope, grouping)
    }

    /// Build a universe over a canonical compiled state plus one exact
    /// solve-scoped overlay. Base restrictions remain separate lower layers;
    /// overlay bounds and rows are appended as semantic contributions.
    pub fn from_model_snapshot_with_overlay(
        base: &BackendSnapshot,
        overlay_snapshot: &BackendSnapshot,
        model: &Model,
        canonical: &ModelSnapshot,
        overlay: &SolveOverlay,
        scope: InfeasibilityScope,
        grouping: ConflictGrouping,
    ) -> Result<Self, InfeasibilityError> {
        let mut universe = Self::from_model_snapshot(base, canonical, scope, grouping)?;
        universe.compilation_id = overlay_snapshot.compilation_id;

        for row in overlay_snapshot.linear_rows.iter().filter(|row| {
            !base
                .linear_rows
                .iter()
                .any(|base_row| base_row.id == row.id)
        }) {
            let origin = overlay_snapshot
                .origin_map
                .constraint_origin(row.id)
                .ok_or_else(|| InfeasibilityError::InvalidUniverse {
                    reason: format!("overlay row {:?} has no semantic origin", row.id),
                })?;
            if row.bounds.lower.is_finite() {
                push_restriction(
                    &mut universe.atoms,
                    &mut universe.compiled_restrictions,
                    &mut Vec::new(),
                    grouping,
                    ConflictAtomKind::SolveOverlay,
                    origin,
                    CompiledRestrictionRef::ConstraintLower(row.id),
                    row.name.clone(),
                    Some(row.bounds.lower),
                )?;
            }
            if row.bounds.upper.is_finite() {
                push_restriction(
                    &mut universe.atoms,
                    &mut universe.compiled_restrictions,
                    &mut Vec::new(),
                    grouping,
                    ConflictAtomKind::SolveOverlay,
                    origin,
                    CompiledRestrictionRef::ConstraintUpper(row.id),
                    row.name.clone(),
                    Some(row.bounds.upper),
                )?;
            }
        }

        for (variable, value) in &overlay.temporary_fixings {
            append_overlay_variable_atom(
                &mut universe,
                overlay_snapshot,
                *variable,
                *value,
                ConflictAtomKind::TemporaryFixing,
                ConflictOrigin::TemporaryFixing {
                    variable: *variable,
                },
            )?;
        }
        for lock in &overlay.locks {
            for (variable, value) in
                lock.resolve(model)
                    .map_err(|error| InfeasibilityError::InvalidUniverse {
                        reason: format!("overlay lock resolution failed: {error:?}"),
                    })?
            {
                append_overlay_variable_atom(
                    &mut universe,
                    overlay_snapshot,
                    variable,
                    value,
                    ConflictAtomKind::SolveLock,
                    ConflictOrigin::SolveLock { variable },
                )?;
            }
        }
        Ok(universe)
    }

    fn from_snapshot_with_fixings(
        snapshot: &BackendSnapshot,
        canonical: Option<&ModelSnapshot>,
        _scope: InfeasibilityScope,
        grouping: ConflictGrouping,
    ) -> Result<Self, InfeasibilityError> {
        let mut atoms = Vec::new();
        let mut compiled_restrictions = Vec::new();
        let mut grouped: GroupedRestrictions = Vec::new();

        for row in &snapshot.linear_rows {
            let origin = snapshot
                .origin_map
                .constraint_origin(row.id)
                .ok_or_else(|| InfeasibilityError::InvalidUniverse {
                    reason: format!("compiled row {:?} has no semantic origin", row.id),
                })?;
            if grouping == ConflictGrouping::Semantic
                && row.bounds.lower.is_finite()
                && row.bounds.upper.is_finite()
                && row.bounds.lower == row.bounds.upper
                && matches!(origin, EntityOrigin::UserConstraint(_))
            {
                push_constraint_equality(
                    &mut atoms,
                    &mut compiled_restrictions,
                    origin,
                    CompiledRestrictionRef::ConstraintLower(row.id),
                    CompiledRestrictionRef::ConstraintUpper(row.id),
                    row.name.clone(),
                    row.bounds.lower,
                )?;
                continue;
            }
            if row.bounds.lower.is_finite() {
                push_restriction(
                    &mut atoms,
                    &mut compiled_restrictions,
                    &mut grouped,
                    grouping,
                    ConflictAtomKind::ConstraintSide(BoundSide::Lower),
                    origin,
                    CompiledRestrictionRef::ConstraintLower(row.id),
                    row.name.clone(),
                    Some(row.bounds.lower),
                )?;
            }
            if row.bounds.upper.is_finite() {
                push_restriction(
                    &mut atoms,
                    &mut compiled_restrictions,
                    &mut grouped,
                    grouping,
                    ConflictAtomKind::ConstraintSide(BoundSide::Upper),
                    origin,
                    CompiledRestrictionRef::ConstraintUpper(row.id),
                    row.name.clone(),
                    Some(row.bounds.upper),
                )?;
            }
        }

        for variable in &snapshot.variables {
            let origin = snapshot
                .origin_map
                .variable_origin(variable.id)
                .ok_or_else(|| InfeasibilityError::InvalidUniverse {
                    reason: format!("compiled variable {:?} has no semantic origin", variable.id),
                })?;
            let user_variable = match &origin {
                EntityOrigin::UserVariable(variable) => Some(*variable),
                _ => None,
            };
            let canonical_fixing = user_variable.and_then(|user_variable| {
                canonical.and_then(|snapshot| {
                    snapshot.variables.iter().find(|candidate| {
                        candidate.id == user_variable && candidate.fixing.is_some()
                    })
                })
            });
            if let Some(declared) = canonical_fixing {
                if grouping == ConflictGrouping::Semantic
                    && declared.bounds.lower.is_finite()
                    && declared.bounds.upper.is_finite()
                    && declared.bounds.lower == declared.bounds.upper
                {
                    push_variable_equality(
                        &mut atoms,
                        &mut compiled_restrictions,
                        origin,
                        CompiledRestrictionRef::VariableLower(variable.id),
                        CompiledRestrictionRef::VariableUpper(variable.id),
                        variable.name.clone(),
                        declared.bounds.lower,
                    )?;
                } else {
                    if declared.bounds.lower.is_finite() {
                        push_restriction(
                            &mut atoms,
                            &mut compiled_restrictions,
                            &mut grouped,
                            grouping,
                            ConflictAtomKind::VariableBound(BoundSide::Lower),
                            origin,
                            CompiledRestrictionRef::VariableLower(variable.id),
                            variable.name.clone(),
                            Some(declared.bounds.lower),
                        )?;
                    }
                    if declared.bounds.upper.is_finite() {
                        push_restriction(
                            &mut atoms,
                            &mut compiled_restrictions,
                            &mut grouped,
                            grouping,
                            ConflictAtomKind::VariableBound(BoundSide::Upper),
                            origin,
                            CompiledRestrictionRef::VariableUpper(variable.id),
                            variable.name.clone(),
                            Some(declared.bounds.upper),
                        )?;
                    }
                }
                continue;
            }
            if grouping == ConflictGrouping::Semantic
                && variable.bounds.lower.is_finite()
                && variable.bounds.upper.is_finite()
                && variable.bounds.lower == variable.bounds.upper
                && matches!(origin, EntityOrigin::UserVariable(_))
            {
                push_variable_equality(
                    &mut atoms,
                    &mut compiled_restrictions,
                    origin,
                    CompiledRestrictionRef::VariableLower(variable.id),
                    CompiledRestrictionRef::VariableUpper(variable.id),
                    variable.name.clone(),
                    variable.bounds.lower,
                )?;
                continue;
            }
            if variable.bounds.lower.is_finite() {
                push_restriction(
                    &mut atoms,
                    &mut compiled_restrictions,
                    &mut grouped,
                    grouping,
                    ConflictAtomKind::VariableBound(BoundSide::Lower),
                    origin,
                    CompiledRestrictionRef::VariableLower(variable.id),
                    variable.name.clone(),
                    Some(variable.bounds.lower),
                )?;
            }
            if variable.bounds.upper.is_finite() {
                push_restriction(
                    &mut atoms,
                    &mut compiled_restrictions,
                    &mut grouped,
                    grouping,
                    ConflictAtomKind::VariableBound(BoundSide::Upper),
                    origin,
                    CompiledRestrictionRef::VariableUpper(variable.id),
                    variable.name.clone(),
                    Some(variable.bounds.upper),
                )?;
            }
        }

        // A construct atom owns every finite row side and generated-variable
        // bound belonging to that construct. Disabling it therefore removes
        // the complete formulation artifact, not one arbitrary bridge row.
        for (construct, role, restrictions) in grouped {
            let id = ConflictAtomId(atoms.len() as u64);
            let refs: Vec<_> = restrictions
                .iter()
                .map(|(reference, _)| *reference)
                .collect();
            compiled_restrictions.extend(refs.iter().copied());
            let origin = ConflictOrigin::GroupedConstruct { construct, role };
            atoms.push(SemanticRestrictionAtom {
                id,
                kind: ConflictAtomKind::GroupedConstruct,
                origin: origin.clone(),
                compiled_restrictions: refs,
                restriction_values: restrictions,
                disable: RestrictionTogglePlan {
                    atom_id: id,
                    action: RestrictionToggleAction::Disable,
                },
                restore: RestrictionTogglePlan {
                    atom_id: id,
                    action: RestrictionToggleAction::Restore,
                },
                snapshot: ConflictMemberSnapshot {
                    origin,
                    name: None,
                    value: None,
                },
            });
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

                // The ordinary bound atoms represent the declared lower layer
                // when a fixing exists. Their values are changed per side,
                // while the fixing atom contributes the later equal bounds.
                for atom in &mut universe.atoms {
                    if matches!(atom.kind, ConflictAtomKind::VariableBound(_))
                        && matches!(atom.origin, ConflictOrigin::VariableBound { variable: v, .. } if v == variable.id)
                    {
                        let value = match atom.kind {
                            ConflictAtomKind::VariableBound(BoundSide::Lower) => {
                                Some(variable.bounds.lower)
                            }
                            ConflictAtomKind::VariableBound(BoundSide::Upper) => {
                                Some(variable.bounds.upper)
                            }
                            _ => atom.snapshot.value,
                        };
                        atom.snapshot.value = value;
                        for (_, restriction_value) in &mut atom.restriction_values {
                            *restriction_value = value;
                        }
                    }
                }

                let id = ConflictAtomId(universe.atoms.len() as u64);
                let lower = CompiledRestrictionRef::VariableLower(compiled);
                let upper = CompiledRestrictionRef::VariableUpper(compiled);
                universe.compiled_restrictions.extend([lower, upper]);
                let origin = ConflictOrigin::PersistentFixing {
                    variable: variable.id,
                };
                universe.atoms.push(SemanticRestrictionAtom {
                    id,
                    kind: ConflictAtomKind::PersistentFixing,
                    origin: origin.clone(),
                    compiled_restrictions: vec![lower, upper],
                    restriction_values: vec![
                        (lower, Some(fixing.value)),
                        (upper, Some(fixing.value)),
                    ],
                    disable: RestrictionTogglePlan {
                        atom_id: id,
                        action: RestrictionToggleAction::Disable,
                    },
                    restore: RestrictionTogglePlan {
                        atom_id: id,
                        action: RestrictionToggleAction::Restore,
                    },
                    snapshot: ConflictMemberSnapshot {
                        origin,
                        name: None,
                        value: Some(fixing.value),
                    },
                });
            }
        }

        Ok(universe)
    }
}

#[allow(clippy::too_many_arguments)]
fn push_restriction(
    atoms: &mut Vec<SemanticRestrictionAtom>,
    compiled_restrictions: &mut Vec<CompiledRestrictionRef>,
    grouped: &mut GroupedRestrictions,
    grouping: ConflictGrouping,
    kind: ConflictAtomKind,
    entity_origin: &EntityOrigin,
    compiled: CompiledRestrictionRef,
    name: Option<String>,
    value: Option<f64>,
) -> Result<(), InfeasibilityError> {
    if matches!(
        grouping,
        ConflictGrouping::Semantic | ConflictGrouping::ByConstruct
    ) {
        if let EntityOrigin::Construct { construct, role } = entity_origin {
            if let Some((_, _, restrictions)) = grouped
                .iter_mut()
                .find(|(existing, _, _)| existing == construct)
            {
                restrictions.push((compiled, value));
            } else {
                grouped.push((*construct, *role, vec![(compiled, value)]));
            }
            return Ok(());
        }
    }

    let origin = match kind {
        ConflictAtomKind::ConstraintSide(side) => constraint_origin(entity_origin, side)?,
        ConflictAtomKind::VariableBound(side) => variable_origin(entity_origin, side)?,
        ConflictAtomKind::SolveOverlay => match entity_origin {
            EntityOrigin::SolveOverlay { overlay, role } => ConflictOrigin::SolveOverlay {
                overlay: *overlay,
                role: *role,
            },
            _ => {
                return Err(InfeasibilityError::InvalidUniverse {
                    reason: "overlay restriction has a non-overlay origin".to_string(),
                })
            }
        },
        _ => {
            return Err(InfeasibilityError::InvalidUniverse {
                reason: "side atom has unsupported origin kind".to_string(),
            })
        }
    };
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
        restriction_values: vec![(compiled, value)],
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
    Ok(())
}

fn append_overlay_variable_atom(
    universe: &mut SemanticConflictUniverse,
    snapshot: &BackendSnapshot,
    variable: crate::Variable,
    value: f64,
    kind: ConflictAtomKind,
    origin: ConflictOrigin,
) -> Result<(), InfeasibilityError> {
    let compiled = snapshot
        .origin_map
        .variables_for_origin(&EntityOrigin::UserVariable(variable))
        .into_iter()
        .next()
        .ok_or_else(|| InfeasibilityError::InvalidUniverse {
            reason: format!("overlay variable {variable:?} has no compiled id"),
        })?;
    let id = ConflictAtomId(universe.atoms.len() as u64);
    let lower = CompiledRestrictionRef::VariableLower(compiled);
    let upper = CompiledRestrictionRef::VariableUpper(compiled);
    universe.compiled_restrictions.extend([lower, upper]);
    universe.atoms.push(SemanticRestrictionAtom {
        id,
        kind,
        origin: origin.clone(),
        compiled_restrictions: vec![lower, upper],
        restriction_values: vec![(lower, Some(value)), (upper, Some(value))],
        disable: RestrictionTogglePlan {
            atom_id: id,
            action: RestrictionToggleAction::Disable,
        },
        restore: RestrictionTogglePlan {
            atom_id: id,
            action: RestrictionToggleAction::Restore,
        },
        snapshot: ConflictMemberSnapshot {
            origin,
            name: None,
            value: Some(value),
        },
    });
    Ok(())
}

fn push_constraint_equality(
    atoms: &mut Vec<SemanticRestrictionAtom>,
    compiled_restrictions: &mut Vec<CompiledRestrictionRef>,
    entity_origin: &EntityOrigin,
    lower: CompiledRestrictionRef,
    upper: CompiledRestrictionRef,
    name: Option<String>,
    value: f64,
) -> Result<(), InfeasibilityError> {
    let constraint = match entity_origin {
        EntityOrigin::UserConstraint(constraint) => *constraint,
        _ => {
            return Err(InfeasibilityError::InvalidUniverse {
                reason: "exact constraint equality has a non-user origin".to_string(),
            })
        }
    };
    let id = ConflictAtomId(atoms.len() as u64);
    let refs = vec![lower, upper];
    compiled_restrictions.extend(refs.iter().copied());
    let origin = ConflictOrigin::ConstraintEquality { constraint };
    atoms.push(SemanticRestrictionAtom {
        id,
        kind: ConflictAtomKind::ConstraintEquality,
        origin: origin.clone(),
        compiled_restrictions: refs.clone(),
        restriction_values: refs
            .iter()
            .map(|reference| (*reference, Some(value)))
            .collect(),
        disable: RestrictionTogglePlan {
            atom_id: id,
            action: RestrictionToggleAction::Disable,
        },
        restore: RestrictionTogglePlan {
            atom_id: id,
            action: RestrictionToggleAction::Restore,
        },
        snapshot: ConflictMemberSnapshot {
            origin,
            name,
            value: Some(value),
        },
    });
    Ok(())
}

fn push_variable_equality(
    atoms: &mut Vec<SemanticRestrictionAtom>,
    compiled_restrictions: &mut Vec<CompiledRestrictionRef>,
    entity_origin: &EntityOrigin,
    lower: CompiledRestrictionRef,
    upper: CompiledRestrictionRef,
    name: Option<String>,
    value: f64,
) -> Result<(), InfeasibilityError> {
    let variable = match entity_origin {
        EntityOrigin::UserVariable(variable) => *variable,
        _ => {
            return Err(InfeasibilityError::InvalidUniverse {
                reason: "exact variable equality has a non-user origin".to_string(),
            })
        }
    };
    let id = ConflictAtomId(atoms.len() as u64);
    let refs = vec![lower, upper];
    compiled_restrictions.extend(refs.iter().copied());
    let origin = ConflictOrigin::VariableEquality { variable };
    atoms.push(SemanticRestrictionAtom {
        id,
        kind: ConflictAtomKind::VariableEquality,
        origin: origin.clone(),
        compiled_restrictions: refs.clone(),
        restriction_values: refs
            .iter()
            .map(|reference| (*reference, Some(value)))
            .collect(),
        disable: RestrictionTogglePlan {
            atom_id: id,
            action: RestrictionToggleAction::Disable,
        },
        restore: RestrictionTogglePlan {
            atom_id: id,
            action: RestrictionToggleAction::Restore,
        },
        snapshot: ConflictMemberSnapshot {
            origin,
            name,
            value: Some(value),
        },
    });
    Ok(())
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
        EntityOrigin::SolveOverlay { overlay, role } => Ok(ConflictOrigin::SolveOverlay {
            overlay: *overlay,
            role: *role,
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
        EntityOrigin::SolveOverlay { overlay, role } => Ok(ConflictOrigin::SolveOverlay {
            overlay: *overlay,
            role: *role,
        }),
        _ => Err(InfeasibilityError::InvalidUniverse {
            reason: "compiled variable has a non-variable origin".to_string(),
        }),
    }
}
