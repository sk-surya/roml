//! Semantic restriction atoms and exact compiled-state conflict universes.

use crate::compiler::backend_ir::BackendSnapshot;
use crate::compiler::origin::EntityOrigin;
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

        Ok(Self {
            compilation_id: snapshot.compilation_id,
            atoms,
            compiled_restrictions,
            grouping,
        })
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
