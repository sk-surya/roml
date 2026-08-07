//! Semantic restriction atoms and exact compiled-state conflict universes.

use crate::compiler::backend_ir::BackendSnapshot;
use crate::compiler::origin::{EntityOrigin, GeneratedRole};
use crate::construct::Construct;
use crate::snapshot::ModelSnapshot;
use crate::solver::infeasibility::{
    BoundSide, CompiledRestrictionRef, ConflictAtomId, ConflictAtomKind, ConflictGrouping,
    ConflictMemberSnapshot, ConflictOrigin, InfeasibilityError, InfeasibilityScope,
    RestrictionToggleAction, RestrictionTogglePlan, SemanticConflictUniverse,
    SemanticRestrictionAtom,
};

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
    if grouping == ConflictGrouping::ByConstruct {
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
