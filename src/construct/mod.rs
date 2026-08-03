//! Canonical semantic constructs (design §7).
//!
//! High-level constructs remain canonical entities and are not eagerly erased
//! into backend rows. One generation-safe construct arena owns lifecycle —
//! ROML does not add a separate side map for every feature.
//!
//! Design §7 declares nine construct kinds (`Indicator`, `Reification`,
//! `MinMax`, `AbsoluteValue`, `Boolean`, `Cardinality`, `BinaryProduct`,
//! `PiecewiseLinear`, `SoftConstraint`) as the extension surface. P25 declares
//! [`ConstructKind`] and its `#[non_exhaustive]` extension boundary but stores
//! only the crate-private [`FixturePayload`] until the per-construct modules
//! land in P30/P32/P33 (P25 scope note: P25 must not pre-implement their
//! formulations).
//!
//! # Crate-private in P25 (F3)
//!
//! The whole module is `pub(crate)` in P25: [`ConstructKind`],
//! [`ConstructEntry`], [`FixturePayload`], and [`ConstructData`] are the
//! internal construct scaffolding and are NOT exported publicly. The public
//! exports are [`Construct`]/[`ConstructId`] and [`FormulationPreference`]
//! (re-exported from the crate root). The per-construct variants and
//! `ConstructKind`/`ConstructEntry` become public exports in P32 when the real
//! per-construct payloads exist.

use std::collections::HashMap;

use crate::id::ParamId;
use crate::identity::{ConstructId, IdentityOverflow};
use crate::metadata::EntityMetadata;

/// A canonical semantic construct handle (design §7).
pub type Construct = ConstructId;

/// The kind of a canonical semantic construct (design §7).
///
/// The enum is `#[non_exhaustive]`: the design §7 variants
/// (`Indicator`, `Reification`, `MinMax`, `AbsoluteValue`, `Boolean`,
/// `Cardinality`, `BinaryProduct`, `PiecewiseLinear`, `SoftConstraint`) are
/// the declared extension surface and land with the per-construct modules in
/// P30/P32/P33. P25 stores only the fixture payload and pre-implements no
/// formulation.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ConstructKind {
    /// P25-only fixture payload used by the construct lifecycle tests.
    ///
    /// Replaced by the design §7 payload variants as per-construct modules land.
    #[doc(hidden)]
    Fixture(FixturePayload),
}

/// P25 fixture payload for construct lifecycle testing.
///
/// Minimal and intentionally solver-free; carries no formulation.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct FixturePayload {
    /// A distinguishing key.
    pub key: String,
    /// A numeric value.
    pub value: f64,
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
    /// [`ConstructData`] arena reads preference exclusively from this entry
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
#[derive(Clone, Debug)]
pub struct ConstructData {
    /// The canonical construct entry (single authority for kind, activity, and
    /// formulation preference — F4).
    pub entry: ConstructEntry,
    /// Derived parameter dependencies of the payload.
    pub parameter_dependencies: Vec<ParamId>,
    /// Entity metadata (also reachable through
    /// [`EntityRef::Construct`](crate::metadata::EntityRef::Construct)).
    ///
    /// P25 (F5): crate-private scaffolding with no readers — the model-level
    /// metadata map is the single authority, so F5 removes this field.
    #[allow(dead_code)]
    pub metadata: EntityMetadata,
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
                metadata: EntityMetadata::default(),
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
        ConstructKind::Fixture(_) => Vec::new(),
    }
}
