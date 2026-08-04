//! Structured compilation reports and backend identity (design §13; P26 Task 5).
//!
//! A [`CompilationReport`] records the deterministic recipe fingerprint, the
//! generated-entity inventory, and the formulation decisions made during
//! compilation — the evidence surface for tests, cache use, and later
//! diagnostics. [`BackendIdentity`] is a backend name/version pair used for
//! report provenance.

use super::backend_ir::{
    CompiledEntityRef, CompiledLinearRow, CompiledObjective, CompiledObjectivePolicy,
    CompiledVariable, RecipeFingerprint,
};

/// A backend name/version pair used for report provenance (design §13 gloss;
/// consumed by P29 IIS/conflict reports).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendIdentity {
    /// Backend name, e.g. `"highs"`.
    pub name: String,
    /// Backend version string, e.g. `"1.15.0"`.
    pub version: String,
}

/// One formulation decision recorded during compilation.
///
/// A stable decision key (e.g. `"objective_policy"`), the selected option, and
/// the reason for the selection. The decision set grows as the P32/P33 bridge
/// tasks land exact construct formulations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormulationDecision {
    /// Stable decision key.
    pub decision: String,
    /// What was selected.
    pub selection: String,
    /// Why it was selected.
    pub reason: String,
}

/// Structured compilation report (design §3.2, §8.5; SM-03.5, SM-03.6).
///
/// - `recipe_fingerprint`: deterministic evidence/cache digest of the compiled
///   recipe (never stale-state authority — D28).
/// - `generated_entities`: inventory of every compiled entity produced.
/// - `formulation_decisions`: the choices made (e.g. the compiled objective
///   policy).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompilationReport {
    /// Deterministic recipe fingerprint (evidence/cache only).
    pub recipe_fingerprint: RecipeFingerprint,
    /// Inventory of generated compiled entities.
    pub generated_entities: Vec<CompiledEntityRef>,
    /// Formulation decisions made during compilation.
    pub formulation_decisions: Vec<FormulationDecision>,
}

impl CompilationReport {
    /// Build the report for one finalized compiled state.
    ///
    /// The generated-entity inventory lists every compiled variable, row, and
    /// objective (deterministic declaration order). The formulation-decision
    /// list records the objective-policy selection; P32/P33 bridges extend it
    /// with per-construct decisions.
    pub(crate) fn new(
        recipe_fingerprint: RecipeFingerprint,
        variables: &[CompiledVariable],
        linear_rows: &[CompiledLinearRow],
        objectives: &[CompiledObjective],
        objective_policy: &CompiledObjectivePolicy,
    ) -> Self {
        let mut generated_entities = Vec::new();
        generated_entities.extend(variables.iter().map(|v| CompiledEntityRef::Variable(v.id)));
        generated_entities.extend(
            linear_rows
                .iter()
                .map(|r| CompiledEntityRef::Constraint(r.id)),
        );
        generated_entities.extend(
            objectives
                .iter()
                .map(|o| CompiledEntityRef::Objective(o.id)),
        );

        let formulation_decisions = vec![FormulationDecision {
            decision: "objective_policy".to_string(),
            selection: format!("{objective_policy:?}"),
            reason: "the compiled objective policy defines which optimization problem is active"
                .to_string(),
        }];

        Self {
            recipe_fingerprint,
            generated_entities,
            formulation_decisions,
        }
    }
}
