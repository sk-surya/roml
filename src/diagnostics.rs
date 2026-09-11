//! Read-only diagnostics guarding MIR fast paths (D-019, DESIGN §11).
//!
//! These counters are a qualification/debug surface, not mathematical model
//! semantics. They never participate in canonical state, snapshots, deltas,
//! revisions, or solver projection, and incrementing one can never change a
//! result. They exist so a fast path cannot silently decay into a general
//! path between releases.
//!
//! Units are deliberately coarse:
//!
//! - lowering counters count **construction operations** that chose a path;
//! - `param_positions_cells` counts per-cell reverse-index entries populated;
//! - propagation counters count **work items** (per-cell lookups/evals).
//!
//! MIR-00 introduces the vocabulary and the baseline measurements. MIR-02
//! adds the packed `ParamDepBlock` counters whose fast path drives them to
//! zero.

/// Lowering-path counters (construction time).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoweringStats {
    /// Bulk constant insertions (constant objective/row blocks).
    pub numeric_bulk: u64,
    /// Bulk parametric insertions (`scale × ParamId` packed runs).
    pub parametric_bulk: u64,
    /// General symbolic cell insertions (the sparse/general fallback).
    pub general_affine: u64,
    /// Stored `ParamDepBlock` descriptors (MIR-02 fast path).
    pub param_dep_blocks: u64,
    /// Per-cell `param_positions` reverse-index entries populated.
    pub param_positions_cells: u64,
    /// Rule rows accumulated into a CSR builder (MIR-05).
    pub rule_rows_accumulated: u64,
    /// Rule bulk commits performed (MIR-05).
    pub rule_bulk_commits: u64,
}

/// Propagation counters (parameter-update time).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PropagationStats {
    /// Packed `param_positions` reverse-index positions examined.
    pub param_position_lookups: u64,
    /// Per-cell reverse-index entries examined during overlay propagation.
    pub overlay_lookups: u64,
    /// `ValueExpr` evaluations performed during propagation.
    pub value_expr_evals: u64,
    /// Packed coefficient-patch batches emitted on commit (MIR-02).
    pub coefficient_patch_batches: u64,
}

/// Mutable accumulator owned by one [`Model`](crate::model::Model).
///
/// The struct is `pub(crate)`; callers read the two public snapshots through
/// [`Model::lowering_stats`](crate::model::Model::lowering_stats) and
/// [`Model::propagation_stats`](crate::model::Model::propagation_stats).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Diagnostics {
    /// Construction-time counters.
    pub lowering: LoweringStats,
    /// Update-time counters.
    pub propagation: PropagationStats,
}

impl Diagnostics {
    /// Zero every counter. Used by microbenchmarks between measured runs.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}
