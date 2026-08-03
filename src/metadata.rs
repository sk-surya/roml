//! Entity metadata and provenance (design §5).
//!
//! Names remain the existing first-class entity name fields. `EntityMetadata`
//! adds description, group, tags, and optional declaration provenance. The
//! model stores metadata keyed by [`EntityRef`]; metadata changes are
//! canonical but non-solver-affecting — they never advance the model revision
//! or emit a solver-facing change (EXECUTION.md "Incremental semantics").

use crate::identity::ConstructId;
use crate::model::{Constraint, Objective, Parameter, Variable};

/// Provenance of an entity's declaration site.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelSource {
    /// Source module name.
    pub module: Option<String>,
    /// Source file path.
    pub file: Option<String>,
    /// Source line number.
    pub line: Option<u32>,
    /// External key (e.g. a row in an external registry).
    pub external_key: Option<String>,
}

/// User metadata attached to a model entity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EntityMetadata {
    /// Optional human-readable description.
    pub description: Option<String>,
    /// Optional grouping label.
    pub group: Option<String>,
    /// Free-form tags.
    pub tags: Vec<String>,
    /// Optional declaration provenance.
    pub source: Option<ModelSource>,
}

/// A reference to a model entity, usable as a metadata key.
///
/// The `Construct` variant is declared now so the metadata key space is
/// stable; it becomes fully usable once the construct arena lands in P25
/// Task 4 (design §4.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntityRef {
    /// A variable.
    Variable(Variable),
    /// A constraint.
    Constraint(Constraint),
    /// An objective.
    Objective(Objective),
    /// A parameter.
    Parameter(Parameter),
    /// A canonical semantic construct.
    Construct(ConstructId),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_defaults() {
        let m = EntityMetadata::default();
        assert!(m.description.is_none());
        assert!(m.group.is_none());
        assert!(m.tags.is_empty());
        assert!(m.source.is_none());
    }

    #[test]
    fn source_and_metadata_round_trip() {
        let src = ModelSource {
            module: Some("m".to_string()),
            file: Some("f.rs".to_string()),
            line: Some(7),
            external_key: Some("k".to_string()),
        };
        let meta = EntityMetadata {
            description: Some("d".to_string()),
            group: Some("g".to_string()),
            tags: vec!["t".to_string()],
            source: Some(src.clone()),
        };
        assert_eq!(meta.clone(), meta);
        assert_eq!(meta.source, Some(src));
        assert_ne!(meta, EntityMetadata::default());
    }

    #[test]
    fn entity_ref_is_hashable_and_copy() {
        let v = crate::id::VarId::new(1, crate::id::Generation::new());
        let r = EntityRef::Variable(v);
        let r2 = r; // Copy
        assert_eq!(r, r2);
        let mut set = std::collections::HashSet::new();
        set.insert(r);
        assert!(set.contains(&r2));
    }
}
