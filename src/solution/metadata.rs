//! Solve metadata attached to a [`Solution`](crate::Solution).
//!
//! Carries the provenance and negotiation context of one solve (API-03.4):
//! which backend produced it, at which model revision, with which effective
//! configuration, and how the model was synchronized into the backend.

use crate::identity::{ModelInstanceId, ModelLineageId};
use crate::revision::ModelRevision;
use crate::solver::request::EffectiveConfig;

/// How the model was synchronized into the backend for this solve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SynchronizationMode {
    /// One or more delta batches were applied incrementally.
    Delta,
    /// The backend was rebuilt from a full model snapshot.
    Rebuild,
    /// The backend was already current; no synchronization occurred.
    NoChange,
}

/// Metadata describing how a [`Solution`](crate::Solution) was produced.
///
/// P25 (SM-02.7, design §4): the metadata records every canonical state id
/// available at solve time — [`model_lineage`](Self::model_lineage),
/// [`model_instance`](Self::model_instance), and
/// [`model_revision`](Self::model_revision).
#[derive(Clone, Debug, PartialEq)]
pub struct SolveMetadata {
    /// Human-readable backend identity (e.g. "HiGHS 1.15.0").
    pub backend_name: String,
    /// The model revision this solution corresponds to.
    pub model_revision: ModelRevision,
    /// The effective configuration applied by the backend (including any
    /// adjustments and rejections from option negotiation).
    pub effective_configuration: EffectiveConfig,
    /// How the model was synchronized into the backend for this solve.
    pub synchronization: SynchronizationMode,
    /// The lineage of the model this solution was produced from.
    pub model_lineage: ModelLineageId,
    /// The instance of the model this solution was produced from.
    pub model_instance: ModelInstanceId,
}

impl Default for SolveMetadata {
    fn default() -> Self {
        Self {
            backend_name: String::new(),
            model_revision: ModelRevision::ZERO,
            effective_configuration: EffectiveConfig::default(),
            synchronization: SynchronizationMode::NoChange,
            // Each default allocates fresh opaque ids (no zero sentinel).
            model_lineage: ModelLineageId::allocate().expect("solve lineage counter exhausted"),
            model_instance: ModelInstanceId::allocate().expect("solve instance counter exhausted"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_metadata_is_nochange_at_zero_revision() {
        let m = SolveMetadata::default();
        assert_eq!(m.model_revision, ModelRevision::ZERO);
        assert_eq!(m.synchronization, SynchronizationMode::NoChange);
        assert!(m.backend_name.is_empty());
        assert!(m.effective_configuration.adjustments.is_empty());
        assert!(m.effective_configuration.rejections.is_empty());
    }

    #[test]
    fn metadata_round_trips_through_equality() {
        let m = SolveMetadata {
            backend_name: "ReferenceBackend".to_string(),
            model_revision: ModelRevision::from_u64(7),
            effective_configuration: EffectiveConfig {
                threads: Some(4),
                ..EffectiveConfig::default()
            },
            synchronization: SynchronizationMode::Rebuild,
            ..SolveMetadata::default()
        };
        assert_eq!(m.clone(), m);
        assert_ne!(m, SolveMetadata::default());
    }
}
