//! Canonical semantic constructs (design §7).
//!
//! High-level constructs remain canonical entities and are not eagerly erased
//! into backend rows. One generation-safe construct arena owns lifecycle —
//! ROML does not add a separate side map for every feature.
//!
//! Design §7 declares nine construct kinds (`Indicator`, `Reification`,
//! `MinMax`, `AbsoluteValue`, `Boolean`, `Cardinality`, `BinaryProduct`,
//! `PiecewiseLinear`, `SoftConstraint`) as the extension surface. P25 declared
//! [`ConstructKind`] and its `#[non_exhaustive]` extension boundary and stored
//! only the crate-private [`FixturePayload`]. P32 Task 16 activates the four
//! logical-construct variants (`Indicator`, `Reification`, `Boolean`,
//! `Cardinality`) with exact semantic payloads; the remaining variants land in
//! P30/P32/P33 follow-up plans.
//!
//! # Public exports (A30, P32)
//!
//! P32 Task 16 activates the real per-construct variants, so the module and
//! [`ConstructKind`]/[`ConstructEntry`] become **public** exports (A30). The
//! `Fixture` variant, [`FixturePayload`], and the fixture-only builders stay
//! crate-private: [`FixturePayload`]'s fields are private and it exposes a
//! `pub(crate)` constructor so the in-crate `#[cfg(test)]` fixture helper keeps
//! working. The `#[non_exhaustive]` extension boundary on [`ConstructKind`]
//! stays (A30).

pub mod boolean;
pub mod cardinality;
pub mod indicator;
pub mod reification;

use std::collections::HashMap;

use crate::id::ParamId;
use crate::identity::{ConstructId, IdentityOverflow};

pub use boolean::{BooleanConstraint, BooleanKind};
pub use cardinality::{CardinalityConstraint, CardinalityKind};
pub use indicator::{IndicatorConstraint, IndicatorDirection};
pub use reification::ReificationConstraint;

/// A canonical semantic construct handle (design §7).
pub type Construct = ConstructId;

/// The kind of a canonical semantic construct (design §7).
///
/// The enum is `#[non_exhaustive]`: the design §7 variants
/// (`Indicator`, `Reification`, `MinMax`, `AbsoluteValue`, `Boolean`,
/// `Cardinality`, `BinaryProduct`, `PiecewiseLinear`, `SoftConstraint`) are
/// the declared extension surface and land with the per-construct modules in
/// P30/P32/P33. P32 Task 16 activates the four logical-construct variants; the
/// `Fixture` variant remains crate-private (A30) and pre-implements no
/// formulation.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ConstructKind {
    /// Indicator: a one-way implication over a binary activator (design §16.1).
    Indicator(IndicatorConstraint),
    /// Reification: `b = 1 ⟺ function ∈ set` (design §16.2).
    Reification(ReificationConstraint),
    /// Boolean: implication/equivalence/any/all over binary variables (design §16.4).
    Boolean(BooleanConstraint),
    /// Cardinality: exactly/at-most/at-least-k over binary variables (design §16.4).
    Cardinality(CardinalityConstraint),
    /// P32-only crate-private fixture payload used by the in-crate construct
    /// lifecycle tests (A30 — never exported).
    #[doc(hidden)]
    Fixture(FixturePayload),
}

/// P32-only crate-private fixture payload for the in-crate construct lifecycle
/// tests (A30).
///
/// Minimal and intentionally solver-free; carries no formulation. The type is
/// `#[doc(hidden)]` and its fields are `pub(crate)` (not exported publicly —
/// external code cannot construct or read the fixture scaffolding); the
/// `pub(crate)` constructor keeps the in-crate `#[cfg(test)]` fixture helper
/// working.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct FixturePayload {
    /// A distinguishing key (crate-visible only — A30).
    pub(crate) key: String,
    /// A numeric value (crate-visible only — A30).
    pub(crate) value: f64,
}

impl FixturePayload {
    /// Build a fixture payload (crate-private — A30).
    #[allow(dead_code)]
    pub(crate) fn new(key: String, value: f64) -> Self {
        Self { key, value }
    }
}

/// A construct entry in canonical state (design §7).
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructEntry {
    /// The stable generation-safe construct identity.
    pub id: Construct,
    /// The exact semantic construct type.
    pub kind: ConstructKind,
    /// Whether the construct is active in the model.
    pub active: bool,
    /// Per-construct formulation preference (F4).
    ///
    /// Threaded through `Change::ConstructAdded`/`ModelOp::AddConstruct` and
    /// the snapshot/delta reconstruction paths so P26 can honor
    /// Auto/Portable/NativeRequired from canonical snapshots/deltas. The
    /// construct arena reads preference exclusively from this entry
    /// (single authority).
    pub preference: FormulationPreference,
}

/// Per-construct formulation preference (design §7, §8.1).
///
/// Narrows the global compilation policy but can never weaken exactness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormulationPreference {
    /// Prefer a qualified exact native primitive, otherwise an exact portable
    /// bridge.
    Auto,
    /// Force deterministic ROML formulations.
    Portable,
    /// Reject when the backend lacks exact native support.
    NativeRequired,
}

/// Internal construct data held by the arena.
///
/// Crate-private (A30): the construct arena is internal scaffolding; external
/// consumers reach constructs through the public builders and the
/// [`ConstructEntry`] in canonical snapshots/deltas.
#[derive(Clone, Debug)]
pub(crate) struct ConstructData {
    /// The canonical construct entry (single authority for kind, activity, and
    /// formulation preference — F4).
    pub entry: ConstructEntry,
    /// Derived parameter dependencies of the payload.
    pub parameter_dependencies: Vec<ParamId>,
}

/// The generation-safe construct arena (design §7).
///
/// Ids are issued by the checked atomic counter — never reused, zero
/// reserved — and the store invalidates removed ids: any operation on a
/// removed id is rejected with a typed error.
#[derive(Clone, Debug, Default)]
pub(crate) struct ConstructStore {
    arena: HashMap<Construct, ConstructData>,
}

impl ConstructStore {
    /// Create an empty construct store.
    pub fn new() -> Self {
        Self {
            arena: HashMap::new(),
        }
    }

    /// Allocate a fresh construct id and insert the entry. Active by default.
    ///
    /// P25 (F3): crate-private scaffolding exercised by the in-crate construct
    /// lifecycle tests via `Model::add_construct_fixture`; the real per-kind
    /// builder APIs land in P32.
    #[allow(dead_code)]
    pub fn add(
        &mut self,
        kind: ConstructKind,
        preference: FormulationPreference,
    ) -> Result<Construct, IdentityOverflow> {
        let id = ConstructId::allocate()?;
        let parameter_dependencies = derive_parameter_dependencies(&kind);
        self.arena.insert(
            id,
            ConstructData {
                entry: ConstructEntry {
                    id,
                    kind,
                    active: true,
                    preference,
                },
                parameter_dependencies,
            },
        );
        Ok(id)
    }

    /// Read construct data by id (stale/removed ids return `None`).
    pub fn get(&self, id: Construct) -> Option<&ConstructData> {
        self.arena.get(&id)
    }

    /// Mutate construct data by id (stale/removed ids return `None`).
    pub fn get_mut(&mut self, id: Construct) -> Option<&mut ConstructData> {
        self.arena.get_mut(&id)
    }

    /// Whether a live construct with this id exists.
    pub fn contains(&self, id: Construct) -> bool {
        self.arena.contains_key(&id)
    }

    /// Remove a construct, invalidating its id. Returns the removed data.
    pub fn remove(&mut self, id: Construct) -> Option<ConstructData> {
        self.arena.remove(&id)
    }

    /// Number of live constructs.
    pub fn len(&self) -> usize {
        self.arena.len()
    }

    /// Iterate over live constructs in arbitrary (deterministic per-store)
    /// order; callers sort for deterministic output.
    pub fn iter(&self) -> impl Iterator<Item = (Construct, &ConstructData)> {
        self.arena.iter().map(|(id, data)| (*id, data))
    }
}

/// Derive the parameter dependencies of a construct payload (design §7).
///
/// The P25 fixture payload carries no parameters. Later per-construct modules
/// derive their dependencies from their payloads; any cached dependency list
/// is invariant-checked against this derivation (P25 Task 4).
pub(crate) fn derive_parameter_dependencies(kind: &ConstructKind) -> Vec<ParamId> {
    match kind {
        ConstructKind::Indicator(payload) => payload.parameter_dependencies(),
        ConstructKind::Reification(payload) => payload.parameter_dependencies(),
        ConstructKind::Boolean(_) => Vec::new(),
        ConstructKind::Cardinality(_) => Vec::new(),
        ConstructKind::Fixture(_) => Vec::new(),
    }
}
