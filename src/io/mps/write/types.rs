//! Public data types for the P36 MPS writer contract.

use crate::{
    id::{ParamId, VarId},
    identity::{ModelInstanceId, ModelLineageId},
    revision::ModelRevision,
};

use super::MpsEntityKind;

/// Controls how source names are handled during deterministic export.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MpsNamePolicy {
    /// Preserve valid unique names and deterministically generate replacements
    /// for missing, invalid, or colliding names.
    #[default]
    PreserveOrGenerate,
    /// Reject an entity when its source name cannot be preserved exactly.
    StrictPreserve,
}

/// Controls how a path destination is committed by [`super::MpsWriter`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MpsDestinationPolicy {
    /// Stage the bytes and atomically replace the destination.
    #[default]
    AtomicReplace,
    /// Stage the bytes and commit only when the destination does not exist.
    CreateNew,
}

/// Options that affect P36 writer behavior.
///
/// Free MPS, LF line endings, canonical finite `f64` formatting, and the
/// canonical vector names are fixed by the contract and are intentionally not
/// represented as options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MpsWriteOptions {
    /// Policy for preserving or generating entity names.
    pub name_policy: MpsNamePolicy,
    /// Policy used only by [`super::MpsWriter::write_path`].
    pub destination_policy: MpsDestinationPolicy,
}

impl Default for MpsWriteOptions {
    fn default() -> Self {
        Self {
            name_policy: MpsNamePolicy::PreserveOrGenerate,
            destination_policy: MpsDestinationPolicy::AtomicReplace,
        }
    }
}

/// One finite parameter value consumed while evaluating an export snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct MpsEvaluatedParameter {
    /// The public semantic parameter handle consumed by the export.
    pub id: ParamId,
    /// The source parameter name, when one was assigned.
    pub name: Option<String>,
    /// The finite value used in the emitted numeric formulation.
    pub value: f64,
}

/// One deterministic mapping from a source name to an emitted MPS name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MpsWriteName {
    /// The deterministic MPS entity namespace for this assignment.
    pub entity_kind: MpsEntityKind,
    /// One-based export-local ordinal within the entity namespace.
    pub ordinal: usize,
    /// The source semantic name, when one exists.
    pub source_name: Option<String>,
    /// The deterministic name emitted in the MPS document.
    pub emitted_name: String,
}

/// Deterministic name assignments recorded by a successful write.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MpsWriteNameMap {
    /// Name assignments for variable/column entities in export order.
    pub variables: Vec<MpsWriteName>,
    /// Name assignments for constraint/row entities in export order.
    pub rows: Vec<MpsWriteName>,
    /// The emitted objective row assignment, when an objective row exists.
    pub objective: Option<MpsWriteName>,
}

/// An exact semantic lowering recorded by a successful write.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum MpsWriteLowering {
    /// A persistent fixing was represented by effective equal bounds.
    PersistentFixingAsBound {
        /// The fixed variable.
        variable: VarId,
        /// The exact fixed value represented by the equal bounds.
        value: f64,
    },
}

/// Report returned after a successful MPS write.
#[derive(Clone, Debug, PartialEq)]
pub struct MpsWriteReport {
    /// Lineage identity of the canonical model snapshot that was exported.
    pub model_lineage: ModelLineageId,
    /// Instance identity of the canonical model snapshot that was exported.
    pub model_instance: ModelInstanceId,
    /// Revision of the canonical model snapshot that was exported.
    pub model_revision: ModelRevision,
    /// Name-preservation policy used for this export.
    pub name_policy: MpsNamePolicy,
    /// Parameter values consumed while evaluating the exported snapshot.
    pub evaluated_parameters: Vec<MpsEvaluatedParameter>,
    /// Number of emitted variable/column entities.
    pub columns: usize,
    /// Number of emitted constraint/row entities.
    pub rows: usize,
    /// Number of emitted mathematical matrix entries, including explicit
    /// zero-valued cells and synthetic zero entries used to declare empty
    /// columns.
    pub nonzeros: usize,
    /// Number of emitted integer variable/column entities.
    pub integer_columns: usize,
    /// Whether an active objective was emitted.
    pub objective_present: bool,
    /// Canonical RHS vector name, when one was emitted.
    pub rhs_vector: Option<String>,
    /// Canonical RANGES vector name, when one was emitted.
    pub ranges_vector: Option<String>,
    /// Canonical BOUNDS vector name, when one was emitted.
    pub bounds_vector: Option<String>,
    /// Deterministic source-to-output name assignments.
    pub name_map: MpsWriteNameMap,
    /// Exact semantic lowerings applied during export.
    pub lowerings: Vec<MpsWriteLowering>,
    /// Number of inactive canonical entities omitted from the output.
    pub omitted_inactive_entities: usize,
}
