//! Coefficient storage: packed constant base plus sparse mutation overlay.
//!
//! Coefficients are first-class objects linking variables to targets
//! (constraints or objectives). The store preserves the exact canonical
//! contract of the former multi-indexed implementation — one cell per
//! `(target, variable)` pair with algebraic combine, stable generational
//! `CoeffId` identities, reverse parameter dependencies, and complete
//! removal cleanup — with different physics:
//!
//! - Bulk constant construction appends to contiguous base arrays (no
//!   per-cell hashing at all).
//! - Post-build mutation lives in a sparse overlay under the same logical
//!   identities; the packed topology is never mutated after construction.
//! - The global variable index over the base is built lazily on first use.
//!
//! The packed base and the overlay TOGETHER are the canonical coefficient
//! authority. The overlay is not a cache.

use std::collections::HashMap;

use super::ModelError;
use crate::bulk::{ParamSpan, StridedMap};
use crate::id::{CoeffId, ConId, IdArena, ObjId, ParamId, VarId};
use crate::model::parameter::ParameterStore;
use crate::value_expr::ValueExpr;

/// Target of a coefficient (constraint or objective).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoefficientTarget {
    /// Coefficient belongs to a constraint.
    Constraint(ConId),
    /// Coefficient belongs to an objective.
    Objective(ObjId),
}

/// Internal data for a coefficient.
#[derive(Clone, Debug)]
pub struct CoefficientData {
    /// The variable this coefficient is multiplied with.
    pub var: VarId,
    /// The target (constraint or objective) this coefficient belongs to.
    pub target: CoefficientTarget,
    /// The value expression (constant or can depend on parameters).
    pub value_expr: ValueExpr,
    /// Cached evaluated value (updated on parameter changes)
    pub cached_value: f64,
}

impl CoefficientData {
    /// Create a new coefficient.
    pub fn new(
        var: VarId,
        target: CoefficientTarget,
        value_expr: ValueExpr,
        initial_value: f64,
    ) -> Self {
        Self {
            var,
            target,
            value_expr,
            cached_value: initial_value,
        }
    }
}

/// A unique cell key: one cell per (target, variable) pair.
pub type CellKey = (CoefficientTarget, VarId);

/// Where a live logical cell's data resides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CellLocation {
    /// Position in the packed constant base arrays.
    Packed(u32),
    /// Position in the packed parametric base arrays (P1C-2).
    ParamBase(u32),
    /// Slot in the sparse overlay.
    Overlay(u32),
}

/// One canonical packed parametric cell for bulk insertion: the
/// coefficient is `scale * parameter`, evaluated to `cached` at build.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ParamCell {
    /// The variable this coefficient multiplies.
    pub var: VarId,
    /// The parameter it scales.
    pub param: ParamId,
    /// Finite multiplier applied to the parameter.
    pub scale: f64,
    /// Evaluated value at insertion.
    pub cached: f64,
}

/// A parameter propagation update resolved inside the store (P1C-2).
/// The caller journals the matching `Change::CoefficientValueChanged`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ParamUpdate {
    /// The affected logical cell (identity preserved).
    pub id: CoeffId,
    /// The variable the coefficient multiplies.
    pub var: VarId,
    /// The target row the cell belongs to.
    pub target: CoefficientTarget,
    /// Finite multiplier applied to the parameter.
    pub scale: f64,
    /// Previous evaluated value.
    pub old: f64,
    /// New evaluated value.
    pub new: f64,
}

/// One contiguous run of packed cells for a single target.
#[derive(Clone, Copy, Debug)]
struct TargetSlice {
    target: CoefficientTarget,
    start: u32,
    len: u32,
}

/// A stored, validated L2 dependency block (MIR-02).
///
/// Maps a family ordinal to a parameter member and to a packed p-base cell.
/// The block carries its uniform target; the per-cell variable is read from
/// the packed base. No per-cell reverse-index (`param_positions`) entries are
/// created for a covered family, and no L1/Python view type appears here.
#[derive(Clone, Debug)]
pub(crate) struct StoredParamDepBlock {
    pub(crate) params: ParamSpan,
    pub(crate) param_map: StridedMap,
    pub(crate) cell_start: u32,
    pub(crate) cell_map: StridedMap,
    pub(crate) scale: f64,
    pub(crate) target: CoefficientTarget,
}

/// One resolved coefficient change from block propagation (MIR-02).
///
/// The model layer maps this to a per-cell changelog entry (scalar update) or
/// strips it to a self-contained [`crate::delta::CoefficientPatch`] (packed
/// block update).
#[derive(Clone, Copy, Debug)]
pub(crate) struct BlockPatch {
    pub coeff: CoeffId,
    pub target: CoefficientTarget,
    pub var: VarId,
    pub old: f64,
    pub new: f64,
    pub scale: f64,
    pub param: ParamId,
}

/// A sparse-overlay slot: full general representation plus tombstone state.
/// Slots are never compacted, so indices (and the `CoeffId`s derived from
/// the identity arena) stay stable; tombstoned slots keep their delta-list
/// references and are skipped by flag.
#[derive(Clone, Debug)]
struct OverlaySlot {
    id: CoeffId,
    data: CoefficientData,
    tombstone: bool,
}

/// Lazily built global variable index over the packed base.
#[derive(Clone, Debug, Default)]
struct PackedVarIndex {
    vars_sorted: Vec<VarId>,
    var_offsets: Vec<u32>,
    positions: Vec<u32>,
}

impl PackedVarIndex {
    fn build(base_vars: &[VarId]) -> Self {
        let mut order: Vec<u32> = (0..base_vars.len() as u32).collect();
        order.sort_by_key(|&i| base_vars[i as usize]);
        let mut vars_sorted = Vec::new();
        let mut var_offsets = Vec::with_capacity(order.len() + 1);
        let mut positions = Vec::with_capacity(order.len());
        let mut i = 0;
        while i < order.len() {
            let v = base_vars[order[i] as usize];
            vars_sorted.push(v);
            var_offsets.push(positions.len() as u32);
            while i < order.len() && base_vars[order[i] as usize] == v {
                positions.push(order[i]);
                i += 1;
            }
        }
        var_offsets.push(positions.len() as u32);
        Self {
            vars_sorted,
            var_offsets,
            positions,
        }
    }

    fn positions(&self, var: VarId) -> &[u32] {
        match self.vars_sorted.binary_search(&var) {
            Ok(k) => {
                let s = self.var_offsets[k] as usize;
                let e = self.var_offsets[k + 1] as usize;
                &self.positions[s..e]
            }
            Err(_) => &[],
        }
    }
}

fn bit_get(bits: &[u64], i: u32) -> bool {
    bits.get(i as usize / 64)
        .is_some_and(|w| w & (1u64 << (i % 64)) != 0)
}

fn bit_set(bits: &mut Vec<u64>, i: u32) {
    let need = i as usize / 64 + 1;
    if bits.len() < need {
        bits.resize(need, 0);
    }
    bits[i as usize / 64] |= 1u64 << (i % 64);
}

/// Slice list for one target in the packed-base directory.
///
/// A single slice is stored inline without allocation; the multi-slice
/// form exists for future multi-block targets (P1 rows). Slice starts are
/// strictly increasing (append-only base), which the position resolver
/// relies on for binary search.
#[derive(Clone, Debug)]
enum TargetSlices {
    One(usize),
    Many(Vec<usize>),
}

impl TargetSlices {
    fn push(&mut self, slice_idx: usize) {
        match self {
            TargetSlices::One(first) => *self = TargetSlices::Many(vec![*first, slice_idx]),
            TargetSlices::Many(list) => list.push(slice_idx),
        }
    }

    fn as_slice(&self) -> &[usize] {
        match self {
            TargetSlices::One(first) => std::slice::from_ref(first),
            TargetSlices::Many(list) => list,
        }
    }
}
/// Packed-base plus sparse-overlay coefficient storage.
///
/// Provides the same canonical contract as the former fully-indexed store:
/// - By coefficient ID (identity arena with generations)
/// - By variable (lazy packed index plus overlay delta)
/// - By constraint / objective (target slices plus overlay delta)
/// - By parameter (overlay only; packed cells are constant by construction)
/// - By cell key (overlay map, then per-target slice search)
///
/// When a coefficient is added for a cell that already exists, the
/// expressions are algebraically combined rather than creating a duplicate.
#[derive(Clone, Debug, Default)]
pub(crate) struct CoefficientIndex {
    /// Stable identities: arena slot per logical cell ever created.
    ids: IdArena<CellLocation>,
    /// Packed base: append-only parallel arrays.
    base_vars: Vec<VarId>,
    base_values: Vec<f64>,
    base_ids: Vec<CoeffId>,
    slices: Vec<TargetSlice>,
    directory: HashMap<CoefficientTarget, TargetSlices>,
    shadowed: Vec<u64>,
    dead: Vec<u64>,
    /// Packed parametric base (P1C-2): append-only parallel arrays holding
    /// `scale * parameter` cells with evaluated caches plus a compact
    /// reverse parameter index. No `ValueExpr` is stored per cell.
    p_vars: Vec<VarId>,
    p_params: Vec<ParamId>,
    p_scales: Vec<f64>,
    p_cached: Vec<f64>,
    p_ids: Vec<CoeffId>,
    p_slices: Vec<TargetSlice>,
    p_directory: HashMap<CoefficientTarget, TargetSlices>,
    p_shadowed: Vec<u64>,
    p_dead: Vec<u64>,
    param_positions: HashMap<ParamId, Vec<u32>>,
    /// Eligible dependency blocks (MIR-02): compact family descriptors that
    /// replace per-cell `param_positions` entries on the fast path.
    param_dep_blocks: Vec<StoredParamDepBlock>,
    /// Sparse overlay: every post-build mutation lives here.
    overlay: Vec<OverlaySlot>,
    overlay_by_cell: HashMap<CellKey, u32>,
    overlay_by_var: HashMap<VarId, Vec<u32>>,
    overlay_by_target: HashMap<CoefficientTarget, Vec<u32>>,
    overlay_by_param: HashMap<ParamId, Vec<u32>>,
    /// Lazy global variable index over the base (None until first use).
    var_index: Option<PackedVarIndex>,
    /// Live logical cell count.
    live: usize,
}

/// Methods used by Model and backend adapters.
#[allow(dead_code)]
impl CoefficientIndex {
    /// Create an empty coefficient index.
    pub fn new() -> Self {
        Self {
            ids: IdArena::new(),
            base_vars: Vec::new(),
            base_values: Vec::new(),
            base_ids: Vec::new(),
            slices: Vec::new(),
            directory: HashMap::new(),
            shadowed: Vec::new(),
            dead: Vec::new(),
            p_vars: Vec::new(),
            p_params: Vec::new(),
            p_scales: Vec::new(),
            p_cached: Vec::new(),
            p_ids: Vec::new(),
            p_slices: Vec::new(),
            p_directory: HashMap::new(),
            p_shadowed: Vec::new(),
            p_dead: Vec::new(),
            param_positions: HashMap::new(),
            param_dep_blocks: Vec::new(),
            overlay: Vec::new(),
            overlay_by_cell: HashMap::new(),
            overlay_by_var: HashMap::new(),
            overlay_by_target: HashMap::new(),
            overlay_by_param: HashMap::new(),
            var_index: None,
            live: 0,
        }
    }

    // ========== Internal resolution ==========

    /// Overlay slot for a live overlay cell, if the key resolves there.
    /// A tombstoned slot resolves (callers distinguish by flag); use
    /// [`Self::for_cell`] for live-only resolution.
    fn overlay_slot(&self, key: &CellKey) -> Option<u32> {
        self.overlay_by_cell.get(key).copied()
    }

    /// Slice indices covering one target (empty when the target has no
    /// packed cells).
    fn slices_for(&self, target: CoefficientTarget) -> &[usize] {
        self.directory
            .get(&target)
            .map(TargetSlices::as_slice)
            .unwrap_or(&[])
    }

    /// Target owning a packed position, via binary search over slice starts
    /// (O(log slices); starts are strictly increasing on the append-only
    /// base). Returns `None` for out-of-range positions.
    fn target_at_position(&self, pos: u32) -> Option<CoefficientTarget> {
        let idx = self
            .slices
            .partition_point(|s| s.start <= pos)
            .checked_sub(1)?;
        let slice = &self.slices[idx];
        if pos < slice.start + slice.len {
            Some(slice.target)
        } else {
            None
        }
    }

    /// Base position for a live packed cell: present slice, sorted hit,
    /// neither dead nor shadowed.
    fn base_position(&self, target: CoefficientTarget, var: VarId) -> Option<u32> {
        let slices = self.slices_for(target);
        for &si in slices {
            let slice = &self.slices[si];
            let base = &self.base_vars[slice.start as usize..(slice.start + slice.len) as usize];
            if let Ok(pos) = base.binary_search(&var) {
                let idx = slice.start + pos as u32;
                if !bit_get(&self.dead, idx) && !bit_get(&self.shadowed, idx) {
                    return Some(idx);
                }
            }
        }
        None
    }

    /// Current logical identity for a live cell, if any.
    fn live_id(&self, target: CoefficientTarget, var: VarId) -> Option<CoeffId> {
        let key = (target, var);
        if let Some(oi) = self.overlay_slot(&key) {
            let slot = &self.overlay[oi as usize];
            if !slot.tombstone {
                return Some(slot.id);
            }
            return None;
        }
        if let Some(idx) = self.base_position(target, var) {
            return Some(self.base_ids[idx as usize]);
        }
        self.param_position(target, var)
            .map(|idx| self.p_ids[idx as usize])
    }

    /// Location a live `CoeffId` resolves to, checking generations.
    fn location_of(&self, id: CoeffId) -> Option<CellLocation> {
        self.ids.get(id.index(), id.generation()).copied()
    }

    /// Drop one overlay slot from the parameter delta lists (expression
    /// replacement or tombstoning). Other delta lists keep the slot and
    /// filter by flag.
    fn unlink_overlay_params(&mut self, oi: u32) {
        for list in self.overlay_by_param.values_mut() {
            if let Some(pos) = list.iter().position(|&x| x == oi) {
                list.swap_remove(pos);
            }
        }
    }

    /// Drop one parametric-base position from the reverse parameter index
    /// (shadowing or removal). The position stays allocated; liveness flows
    /// through the shadow/dead bits and the arena generation.
    fn unlink_param_position(&mut self, pos: u32) {
        let param = self.p_params[pos as usize];
        if let Some(list) = self.param_positions.get_mut(&param) {
            if let Some(at) = list.iter().position(|&x| x == pos) {
                list.swap_remove(at);
            }
            if list.is_empty() {
                self.param_positions.remove(&param);
            }
        }
    }

    // ========== Bulk construction ==========

    /// Append a block of constant coefficients.
    ///
    /// If `cells` are sorted strictly ascending by variable, they append
    /// directly with zero hashing. Otherwise they are canonicalized first
    /// (stable sort by variable, adjacent duplicates summed — R2.2 — with
    /// near-zero merged totals dropped exactly like `LinExpr::simplify`).
    /// Either way the stored run is canonical and one target slice covers
    /// exactly the appended range.
    pub fn append_constant_block(&mut self, target: CoefficientTarget, cells: &[(VarId, f64)]) {
        let sorted_unique = cells.windows(2).all(|w| w[0].0 < w[1].0);
        if sorted_unique {
            self.append_canonical_run(target, cells);
            return;
        }
        // Canonicalize: stable order by variable, then merge runs.
        let mut order: Vec<usize> = (0..cells.len()).collect();
        order.sort_by_key(|&i| cells[i].0);
        let mut canonical: Vec<(VarId, f64)> = Vec::with_capacity(cells.len());
        for i in order {
            let (var, value) = cells[i];
            if let Some(last) = canonical.last_mut() {
                if last.0 == var {
                    last.1 += value;
                    continue;
                }
            }
            canonical.push((var, value));
        }
        canonical.retain(|(_, v)| v.abs() >= f64::EPSILON);
        self.append_canonical_run(target, &canonical);
    }

    /// Append one canonical (sorted, unique) run for a fresh target range.
    fn append_canonical_run(&mut self, target: CoefficientTarget, cells: &[(VarId, f64)]) {
        if cells.is_empty() {
            return;
        }
        // The lazy packed variable index (if built) covers only the base
        // prefix at build time; appending extends the base, so the index
        // must be rebuilt on next use (certification review: stale index
        // hid post-build cells from `for_var` removal cascades).
        self.var_index = None;
        let n = cells.len();
        self.ids.reserve(n);
        self.base_vars.reserve(n);
        self.base_values.reserve(n);
        self.base_ids.reserve(n);
        let start = self.base_vars.len() as u32;
        for (var, value) in cells.iter() {
            let pos = self.base_vars.len() as u32;
            let (index, generation) = self.ids.allocate(CellLocation::Packed(pos));
            let id = CoeffId::new(index, generation);
            self.base_vars.push(*var);
            self.base_values.push(*value);
            self.base_ids.push(id);
            self.live += 1;
        }
        let slice_idx = self.slices.len();
        self.slices.push(TargetSlice {
            target,
            start,
            len: n as u32,
        });
        match self.directory.entry(target) {
            std::collections::hash_map::Entry::Occupied(mut e) => e.get_mut().push(slice_idx),
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(TargetSlices::One(slice_idx));
            }
        }
    }

    /// Canonicalize one constant row: sorted order, duplicates summed
    /// (R2.2), near-zero totals dropped exactly like `LinExpr::simplify`.
    /// Merged totals are finiteness-checked. Pure function (no mutation),
    /// shared by validation and insertion so both agree bit-for-bit.
    pub(crate) fn canonicalize_constant_row(
        vars: &[VarId],
        values: &[f64],
    ) -> Result<Vec<(VarId, f64)>, ModelError> {
        debug_assert_eq!(vars.len(), values.len());
        if vars.windows(2).all(|w| w[0] < w[1]) {
            return Ok(vars
                .iter()
                .zip(values.iter())
                .filter(|(_, v)| v.abs() >= f64::EPSILON)
                .map(|(var, value)| (*var, *value))
                .collect());
        }
        let mut order: Vec<usize> = (0..vars.len()).collect();
        order.sort_by_key(|&i| vars[i]);
        let mut merged: Vec<(VarId, f64)> = Vec::with_capacity(vars.len());
        for i in order {
            let (var, value) = (vars[i], values[i]);
            if let Some(last) = merged.last_mut() {
                if last.0 == var {
                    last.1 += value;
                    continue;
                }
            }
            merged.push((var, value));
        }
        for (_, v) in &merged {
            if !v.is_finite() {
                return Err(ModelError::NonFiniteValue("merged row coefficient"));
            }
        }
        merged.retain(|(_, v)| v.abs() >= f64::EPSILON);
        Ok(merged)
    }

    /// Append a multi-row block of constant coefficients (P1A).
    ///
    /// `targets[r]` owns `vars[row_ptr[r]..row_ptr[r+1]]` with the parallel
    /// `values` slice. The whole block is canonicalized first via
    /// [`Self::canonicalize_constant_row`] (sorted order, duplicate merge,
    /// zero drop, merged-finite check), so rejection is atomic — no partial
    /// rows, slices, or identities are installed on error. Each kept row
    /// appends one target slice; empty rows append an empty slice.
    pub fn append_row_block(
        &mut self,
        targets: &[CoefficientTarget],
        row_ptr: &[u32],
        vars: &[VarId],
        values: &[f64],
    ) -> Result<(), ModelError> {
        if targets.len() + 1 != row_ptr.len()
            || row_ptr.last().copied().unwrap_or(0) as usize != vars.len()
            || vars.len() != values.len()
        {
            return Err(ModelError::MismatchedBulkLengths {
                vars: vars.len(),
                coeffs: values.len(),
            });
        }
        // Phase 1 (pure): canonicalize every row, checking merged finiteness.
        let mut canon_ptr: Vec<u32> = Vec::with_capacity(row_ptr.len());
        let mut canon_vars: Vec<VarId> = Vec::new();
        let mut canon_values: Vec<f64> = Vec::new();
        canon_ptr.push(0);
        for r in 0..targets.len() {
            let (s, e) = (row_ptr[r] as usize, row_ptr[r + 1] as usize);
            for (var, value) in Self::canonicalize_constant_row(&vars[s..e], &values[s..e])? {
                canon_vars.push(var);
                canon_values.push(value);
            }
            canon_ptr.push(canon_vars.len() as u32);
        }
        // Phase 2: reserve once, append sequentially.
        self.append_canonical_block(targets, &canon_ptr, &canon_vars, &canon_values);
        Ok(())
    }

    /// Append pre-canonicalized rows: `targets[r]` owns
    /// `vars[row_ptr[r]..row_ptr[r+1]]`, each row sorted, unique,
    /// zero-dropped, and finite (debug-checked). Used by the model layer,
    /// which canonicalizes once during validation and journals the same
    /// buffers — no double canonicalization on the production path.
    pub(crate) fn append_canonical_block(
        &mut self,
        targets: &[CoefficientTarget],
        row_ptr: &[u32],
        vars: &[VarId],
        values: &[f64],
    ) {
        debug_assert_eq!(targets.len() + 1, row_ptr.len());
        debug_assert_eq!(row_ptr.last().copied().unwrap_or(0) as usize, vars.len());
        debug_assert_eq!(vars.len(), values.len());
        #[cfg(debug_assertions)]
        for r in 0..targets.len() {
            let (s, e) = (row_ptr[r] as usize, row_ptr[r + 1] as usize);
            debug_assert!(vars[s..e].windows(2).all(|w| w[0] < w[1]));
            debug_assert!(values[s..e].iter().all(|v| v.is_finite()));
        }
        // Same lazy-index invalidation as `append_canonical_run`: the base
        // grows, so a previously built index no longer covers it.
        self.var_index = None;
        let total: usize = row_ptr.windows(2).map(|w| (w[1] - w[0]) as usize).sum();
        self.ids.reserve(total);
        self.base_vars.reserve(total);
        self.base_values.reserve(total);
        self.base_ids.reserve(total);
        self.slices.reserve(targets.len());
        for r in 0..targets.len() {
            let (s, e) = (row_ptr[r] as usize, row_ptr[r + 1] as usize);
            let start = self.base_vars.len() as u32;
            for (&var, &value) in vars[s..e].iter().zip(&values[s..e]) {
                let pos = self.base_vars.len() as u32;
                let (index, generation) = self.ids.allocate(CellLocation::Packed(pos));
                let id = CoeffId::new(index, generation);
                self.base_vars.push(var);
                self.base_values.push(value);
                self.base_ids.push(id);
                self.live += 1;
            }
            let slice_idx = self.slices.len();
            self.slices.push(TargetSlice {
                target: targets[r],
                start,
                len: (e - s) as u32,
            });
            match self.directory.entry(targets[r]) {
                std::collections::hash_map::Entry::Occupied(mut e2) => e2.get_mut().push(slice_idx),
                std::collections::hash_map::Entry::Vacant(e2) => {
                    e2.insert(TargetSlices::One(slice_idx));
                }
            }
        }
    }

    /// Append one canonical packed parametric run for a fresh target
    /// range (P1C-2): `cells` sorted strictly ascending by variable, at
    /// most one entry per variable, all scales finite (debug-checked; the
    /// model layer canonicalizes). One target slice covers the range, and
    /// the compact reverse parameter index extends by the new positions.
    pub(crate) fn append_param_run(&mut self, target: CoefficientTarget, cells: &[ParamCell]) {
        self.append_param_run_inner(target, cells, true);
    }

    /// Append one canonical packed parametric run, optionally tracking
    /// per-cell reverse positions.
    fn append_param_run_inner(
        &mut self,
        target: CoefficientTarget,
        cells: &[ParamCell],
        track_positions: bool,
    ) {
        #[cfg(debug_assertions)]
        {
            debug_assert!(cells.windows(2).all(|w| w[0].var < w[1].var));
            debug_assert!(cells
                .iter()
                .all(|c| c.scale.is_finite() && c.cached.is_finite()));
        }
        let n = cells.len();
        if n == 0 {
            return;
        }
        self.ids.reserve(n);
        self.p_vars.reserve(n);
        self.p_params.reserve(n);
        self.p_scales.reserve(n);
        self.p_cached.reserve(n);
        self.p_ids.reserve(n);
        let start = self.p_vars.len() as u32;
        for cell in cells.iter() {
            let pos = self.p_vars.len() as u32;
            let (index, generation) = self.ids.allocate(CellLocation::ParamBase(pos));
            let id = CoeffId::new(index, generation);
            self.p_vars.push(cell.var);
            self.p_params.push(cell.param);
            self.p_scales.push(cell.scale);
            self.p_cached.push(cell.cached);
            self.p_ids.push(id);
            if track_positions {
                self.param_positions
                    .entry(cell.param)
                    .or_default()
                    .push(pos);
            }
            self.live += 1;
        }
        let slice_idx = self.p_slices.len();
        self.p_slices.push(TargetSlice {
            target,
            start,
            len: n as u32,
        });
        match self.p_directory.entry(target) {
            std::collections::hash_map::Entry::Occupied(mut e) => e.get_mut().push(slice_idx),
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(TargetSlices::One(slice_idx));
            }
        }
    }

    /// Append one canonical packed parametric run covered by validated L2
    /// dependency blocks (MIR-02).
    ///
    /// The witness is validated against the candidate canonical run *before*
    /// any mutation, so an invalid layout is a typed atomic rejection. On
    /// success the run is appended **without** per-cell `param_positions`
    /// entries and the resolved blocks are stored; their `cell_start` becomes
    /// the absolute packed ordinal.
    pub(crate) fn append_param_run_with_deps(
        &mut self,
        target: CoefficientTarget,
        cells: &[ParamCell],
        mut blocks: Vec<StoredParamDepBlock>,
    ) -> Result<(), ModelError> {
        let run_start = self.p_vars.len() as u32;
        for block in &blocks {
            if !block.scale.is_finite() {
                return Err(ModelError::NonFiniteValue("dependency scale"));
            }
            if block.param_map.len() != block.cell_map.len() {
                return Err(ModelError::InvalidParamDepLayout(
                    "parameter and cell maps have different arity",
                ));
            }
            for k in 0..block.param_map.len() {
                let param_offset = block
                    .param_map
                    .get(k)
                    .ok_or(ModelError::InvalidParamDepLayout("parameter map overflow"))?;
                if param_offset < 0 || param_offset as usize >= block.params.len() {
                    return Err(ModelError::InvalidParamDepLayout(
                        "parameter offset out of range",
                    ));
                }
                let cell_offset = block
                    .cell_map
                    .get(k)
                    .ok_or(ModelError::InvalidParamDepLayout("cell map overflow"))?;
                if cell_offset < 0 {
                    return Err(ModelError::InvalidParamDepLayout("negative cell offset"));
                }
                let idx = block.cell_start as usize + cell_offset as usize;
                let cell = cells.get(idx).ok_or(ModelError::InvalidParamDepLayout(
                    "cell offset outside the packed run",
                ))?;
                let expected = block.params.id_at(param_offset as usize).ok_or(
                    ModelError::InvalidParamDepLayout("parameter id out of range"),
                )?;
                if cell.param != expected {
                    return Err(ModelError::InvalidParamDepLayout(
                        "witness parameter does not match the canonical cell",
                    ));
                }
                if cell.scale != block.scale {
                    return Err(ModelError::InvalidParamDepLayout(
                        "witness scale does not match the canonical cell",
                    ));
                }
            }
        }
        for block in &mut blocks {
            block.cell_start += run_start;
        }
        self.append_param_run_inner(target, cells, false);
        self.param_dep_blocks.extend(blocks);
        Ok(())
    }

    /// Number of stored dependency blocks (MIR diagnostics / tests).
    pub(crate) fn dep_block_count(&self) -> usize {
        self.param_dep_blocks.len()
    }

    /// Target owning a parametric-base position, via binary search over
    /// parametric slice starts (O(log slices)).
    fn param_target_at(&self, pos: u32) -> Option<CoefficientTarget> {
        let idx = self
            .p_slices
            .partition_point(|s| s.start <= pos)
            .checked_sub(1)?;
        let slice = &self.p_slices[idx];
        if pos < slice.start + slice.len {
            Some(slice.target)
        } else {
            None
        }
    }

    /// Parametric-base position for a live cell: present slice, sorted hit,
    /// neither dead nor shadowed.
    fn param_position(&self, target: CoefficientTarget, var: VarId) -> Option<u32> {
        let slices = self
            .p_directory
            .get(&target)
            .map(TargetSlices::as_slice)
            .unwrap_or(&[]);
        for &si in slices {
            let slice = &self.p_slices[si];
            let base = &self.p_vars[slice.start as usize..(slice.start + slice.len) as usize];
            if let Ok(at) = base.binary_search(&var) {
                let idx = slice.start + at as u32;
                if !bit_get(&self.p_dead, idx) && !bit_get(&self.p_shadowed, idx) {
                    return Some(idx);
                }
            }
        }
        None
    }

    /// Propagate a parameter value through the packed parametric base,
    /// updating caches in place and reporting every changed cell. The
    /// caller journals the matching `Change::CoefficientValueChanged`
    /// entries (same shape as scalar propagation).
    ///
    /// `stats` receives one `param_position_lookups` tick per reverse-index
    /// position examined (including dead/shadowed positions that are
    /// skipped), giving a direct baseline for the MIR-02 packed-block path.
    pub(crate) fn propagate_packed_param(
        &mut self,
        param: ParamId,
        value: f64,
        stats: &mut crate::diagnostics::PropagationStats,
    ) -> Vec<ParamUpdate> {
        let mut out = Vec::new();
        let positions: &[u32] = self
            .param_positions
            .get(&param)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        stats.param_position_lookups += positions.len() as u64;
        // Borrow split: positions first (immutable copy of indices).
        let positions: Vec<u32> = positions.to_vec();
        for pos in positions {
            if bit_get(&self.p_dead, pos) || bit_get(&self.p_shadowed, pos) {
                continue;
            }
            let new = self.p_scales[pos as usize] * value;
            let old = self.p_cached[pos as usize];
            if (old - new).abs() >= f64::EPSILON {
                self.p_cached[pos as usize] = new;
                // Target resolution is O(log slices); updates are rare.
                if let Some(target) = self.param_target_at(pos) {
                    out.push(ParamUpdate {
                        id: self.p_ids[pos as usize],
                        var: self.p_vars[pos as usize],
                        target,
                        scale: self.p_scales[pos as usize],
                        old,
                        new,
                    });
                }
            }
        }
        out
    }

    /// Propagate a parameter span through stored dependency blocks (MIR-02).
    ///
    /// This is the eligible-family fast path: it reads the parameter store
    /// directly, walks only the compact dependency blocks, and performs **no**
    /// `param_positions` lookups, overlay lookups, or `ValueExpr`
    /// evaluations. Dead/shadowed packed cells are skipped (their correctness
    /// is handled by overlay semantics). The caller journals the result.
    pub(crate) fn propagate_packed_span(
        &mut self,
        span: ParamSpan,
        params: &ParameterStore,
        out: &mut Vec<BlockPatch>,
    ) {
        if self.param_dep_blocks.is_empty() {
            return;
        }
        let q0 = span.start() as usize;
        let q1 = q0 + span.len();
        for bi in 0..self.param_dep_blocks.len() {
            // Copy the compact descriptor out so the packed cache can be
            // mutated without holding a borrow of `param_dep_blocks`.
            let (params_span, param_map, cell_start, cell_map, scale, target) = {
                let b = &self.param_dep_blocks[bi];
                (
                    b.params,
                    b.param_map.clone(),
                    b.cell_start as usize,
                    b.cell_map.clone(),
                    b.scale,
                    b.target,
                )
            };
            // Fast rejection: no ordinal overlap with the update span.
            let p0 = params_span.start() as usize;
            let p1 = p0 + params_span.len();
            if p1 <= q0 || q1 <= p0 {
                continue;
            }
            for k in 0..param_map.len() {
                let param_offset = match param_map.get(k) {
                    Some(v) if v >= 0 => v as usize,
                    _ => continue,
                };
                if param_offset >= params_span.len() {
                    continue;
                }
                let param = match params_span.id_at(param_offset) {
                    Some(p) => p,
                    None => continue,
                };
                let pi = param.index() as usize;
                if pi < q0 || pi >= q1 {
                    continue;
                }
                let cell_offset = match cell_map.get(k) {
                    Some(v) if v >= 0 => v as usize,
                    _ => continue,
                };
                let pos = cell_start + cell_offset;
                if pos >= self.p_vars.len() {
                    continue;
                }
                let pos_u = pos as u32;
                if bit_get(&self.p_dead, pos_u) || bit_get(&self.p_shadowed, pos_u) {
                    continue;
                }
                let value = params.get_value(param).unwrap_or(0.0);
                let new = scale * value;
                let old = self.p_cached[pos];
                if (old - new).abs() < f64::EPSILON {
                    continue;
                }
                self.p_cached[pos] = new;
                out.push(BlockPatch {
                    coeff: self.p_ids[pos],
                    target,
                    var: self.p_vars[pos],
                    old,
                    new,
                    scale,
                    param,
                });
            }
        }
    }

    // ========== Scalar general path (overlay) ==========

    /// Add a new coefficient or combine with an existing cell.
    ///
    /// If a coefficient already exists for this (target, variable) pair,
    /// the expressions are algebraically added and the existing coefficient
    /// is updated in place. Returns the (possibly existing) CoeffId.
    ///
    /// Fresh cells land in the sparse overlay with full expressions;
    /// combining into a packed base cell shadows it into the overlay under
    /// the same logical identity.
    pub fn add(
        &mut self,
        var: VarId,
        target: CoefficientTarget,
        value_expr: ValueExpr,
        initial_value: f64,
    ) -> CoeffId {
        let key = (target, var);
        if let Some(id) = self.live_id(target, var) {
            // Combine into the existing logical cell.
            match self.location_of(id).expect("live id always has a location") {
                CellLocation::Overlay(oi) => {
                    let old_deps: Vec<ParamId> = self.overlay[oi as usize]
                        .data
                        .value_expr
                        .dependencies()
                        .into_iter()
                        .collect();
                    for param in &old_deps {
                        if let Some(set) = self.overlay_by_param.get_mut(param) {
                            set.retain(|&x| x != oi);
                            if set.is_empty() {
                                self.overlay_by_param.remove(param);
                            }
                        }
                    }
                    let slot = &mut self.overlay[oi as usize];
                    let combined = slot.data.value_expr.clone() + value_expr;
                    slot.data.value_expr = combined.clone();
                    slot.data.cached_value += initial_value;
                    for param in combined.dependencies() {
                        self.overlay_by_param.entry(param).or_default().push(oi);
                    }
                    id
                }
                CellLocation::Packed(pos) => {
                    // Shadow the packed cell: same identity, overlay data.
                    bit_set(&mut self.shadowed, pos);
                    let base_value = self.base_values[pos as usize];
                    let combined = ValueExpr::constant(base_value) + value_expr;
                    let new_cached = base_value + initial_value;
                    // Reuse the logical id: point the arena slot at overlay.
                    let oi = self.overlay.len() as u32;
                    if let Some(slot) = self.ids.get_mut(id.index(), id.generation()) {
                        *slot = CellLocation::Overlay(oi);
                    }
                    let data = CoefficientData::new(var, target, combined.clone(), new_cached);
                    self.overlay.push(OverlaySlot {
                        id,
                        data,
                        tombstone: false,
                    });
                    self.overlay_by_cell.insert(key, oi);
                    self.overlay_by_var.entry(var).or_default().push(oi);
                    self.overlay_by_target.entry(target).or_default().push(oi);
                    for param in combined.dependencies() {
                        self.overlay_by_param.entry(param).or_default().push(oi);
                    }
                    id
                }
                CellLocation::ParamBase(pos) => {
                    // Shadow the packed parametric cell: same identity,
                    // overlay data combining the stored scaled parameter.
                    bit_set(&mut self.p_shadowed, pos);
                    self.unlink_param_position(pos);
                    let scale = self.p_scales[pos as usize];
                    let param = self.p_params[pos as usize];
                    let base_value = self.p_cached[pos as usize];
                    let combined = ValueExpr::scaled_param(scale, param) + value_expr;
                    let new_cached = base_value + initial_value;
                    let oi = self.overlay.len() as u32;
                    if let Some(slot) = self.ids.get_mut(id.index(), id.generation()) {
                        *slot = CellLocation::Overlay(oi);
                    }
                    let data = CoefficientData::new(var, target, combined.clone(), new_cached);
                    self.overlay.push(OverlaySlot {
                        id,
                        data,
                        tombstone: false,
                    });
                    self.overlay_by_cell.insert(key, oi);
                    self.overlay_by_var.entry(var).or_default().push(oi);
                    self.overlay_by_target.entry(target).or_default().push(oi);
                    for param in combined.dependencies() {
                        self.overlay_by_param.entry(param).or_default().push(oi);
                    }
                    id
                }
            }
        } else {
            // Fresh cell: full general representation in the overlay.
            let oi = self.overlay.len() as u32;
            let (index, generation) = self.ids.allocate(CellLocation::Overlay(oi));
            // Fix the arena location at the final slot index (== oi here).
            let id = CoeffId::new(index, generation);
            let data = CoefficientData::new(var, target, value_expr.clone(), initial_value);
            self.overlay.push(OverlaySlot {
                id,
                data,
                tombstone: false,
            });
            self.overlay_by_cell.insert(key, oi);
            self.overlay_by_var.entry(var).or_default().push(oi);
            self.overlay_by_target.entry(target).or_default().push(oi);
            for param in value_expr.dependencies() {
                self.overlay_by_param.entry(param).or_default().push(oi);
            }
            self.live += 1;
            id
        }
    }

    /// Replace an existing coefficient's value expression, maintaining the
    /// parameter dependency index.
    ///
    /// Overlay cells are replaced in place; packed cells are shadowed into
    /// the overlay under the same identity. The coefficient's identity and
    /// cell key are unchanged (D11 replace-by-cell semantics).
    pub fn set_expr(&mut self, id: CoeffId, value_expr: ValueExpr, evaluated: f64) {
        let location = match self.location_of(id) {
            Some(loc) => loc,
            None => return,
        };
        match location {
            CellLocation::Overlay(oi) => {
                self.unlink_overlay_params(oi);
                let slot = &mut self.overlay[oi as usize];
                slot.data.value_expr = value_expr.clone();
                slot.data.cached_value = evaluated;
                for param in value_expr.dependencies() {
                    self.overlay_by_param.entry(param).or_default().push(oi);
                }
            }
            CellLocation::Packed(pos) => {
                bit_set(&mut self.shadowed, pos);
                let target = self
                    .target_at_position(pos)
                    .expect("packed position always in a slice");
                let var = self.base_vars[pos as usize];
                let oi = self.overlay.len() as u32;
                if let Some(slot) = self.ids.get_mut(id.index(), id.generation()) {
                    *slot = CellLocation::Overlay(oi);
                }
                let data = CoefficientData::new(var, target, value_expr.clone(), evaluated);
                self.overlay.push(OverlaySlot {
                    id,
                    data,
                    tombstone: false,
                });
                self.overlay_by_cell.insert((target, var), oi);
                self.overlay_by_var.entry(var).or_default().push(oi);
                self.overlay_by_target.entry(target).or_default().push(oi);
                for param in value_expr.dependencies() {
                    self.overlay_by_param.entry(param).or_default().push(oi);
                }
            }
            CellLocation::ParamBase(pos) => {
                bit_set(&mut self.p_shadowed, pos);
                self.unlink_param_position(pos);
                let target = self
                    .param_target_at(pos)
                    .expect("packed position always in a slice");
                let var = self.p_vars[pos as usize];
                let oi = self.overlay.len() as u32;
                if let Some(slot) = self.ids.get_mut(id.index(), id.generation()) {
                    *slot = CellLocation::Overlay(oi);
                }
                let data = CoefficientData::new(var, target, value_expr.clone(), evaluated);
                self.overlay.push(OverlaySlot {
                    id,
                    data,
                    tombstone: false,
                });
                self.overlay_by_cell.insert((target, var), oi);
                self.overlay_by_var.entry(var).or_default().push(oi);
                self.overlay_by_target.entry(target).or_default().push(oi);
                for param in value_expr.dependencies() {
                    self.overlay_by_param.entry(param).or_default().push(oi);
                }
            }
        }
    }

    /// Remove a coefficient by ID.
    ///
    /// Returns the data if it existed. Overlay slots tombstone in place;
    /// packed positions set the dead bit and record a tombstone so key
    /// resolution keeps stopping at the overlay. The arena slot is removed
    /// (generation bump), so the `CoeffId` goes stale exactly as before.
    pub fn remove(&mut self, id: CoeffId) -> Option<CoefficientData> {
        let location = self.location_of(id)?;
        let data = self.ids.remove(id.index(), id.generation())?;
        debug_assert_eq!(data, location);
        match location {
            CellLocation::Overlay(oi) => {
                self.unlink_overlay_params(oi);
                let slot = &mut self.overlay[oi as usize];
                slot.tombstone = true;
                self.live -= 1;
                Some(slot.data.clone())
            }
            CellLocation::Packed(pos) => {
                bit_set(&mut self.dead, pos);
                let var = self.base_vars[pos as usize];
                let value = self.base_values[pos as usize];
                let target = self
                    .target_at_position(pos)
                    .expect("packed position always in a slice");
                let oi = self.overlay.len() as u32;
                self.overlay.push(OverlaySlot {
                    id,
                    data: CoefficientData::new(var, target, ValueExpr::constant(value), value),
                    tombstone: true,
                });
                self.overlay_by_cell.insert((target, var), oi);
                self.overlay_by_var.entry(var).or_default().push(oi);
                self.overlay_by_target.entry(target).or_default().push(oi);
                self.live -= 1;
                Some(CoefficientData::new(
                    var,
                    target,
                    ValueExpr::constant(value),
                    value,
                ))
            }
            CellLocation::ParamBase(pos) => {
                bit_set(&mut self.p_dead, pos);
                self.unlink_param_position(pos);
                let var = self.p_vars[pos as usize];
                let target = self
                    .param_target_at(pos)
                    .expect("packed position always in a slice");
                let expr = ValueExpr::scaled_param(
                    self.p_scales[pos as usize],
                    self.p_params[pos as usize],
                );
                let value = self.p_cached[pos as usize];
                let oi = self.overlay.len() as u32;
                self.overlay.push(OverlaySlot {
                    id,
                    data: CoefficientData::new(var, target, expr.clone(), value),
                    tombstone: true,
                });
                self.overlay_by_cell.insert((target, var), oi);
                self.overlay_by_var.entry(var).or_default().push(oi);
                self.overlay_by_target.entry(target).or_default().push(oi);
                self.live -= 1;
                Some(CoefficientData::new(var, target, expr, value))
            }
        }
    }

    /// Get coefficient data by ID (owned snapshot of the logical cell).
    ///
    /// Packed constant cells materialize `ValueExpr::Constant` inline, so
    /// reads never allocate beyond the returned struct itself.
    pub fn get(&self, id: CoeffId) -> Option<CoefficientData> {
        match self.location_of(id)? {
            CellLocation::Overlay(oi) => {
                let slot = &self.overlay[oi as usize];
                if slot.tombstone {
                    return None;
                }
                Some(slot.data.clone())
            }
            CellLocation::Packed(pos) => {
                if bit_get(&self.dead, pos) || bit_get(&self.shadowed, pos) {
                    return None;
                }
                let value = self.base_values[pos as usize];
                let target = self
                    .target_at_position(pos)
                    .expect("packed position always in a slice");
                Some(CoefficientData::new(
                    self.base_vars[pos as usize],
                    target,
                    ValueExpr::constant(value),
                    value,
                ))
            }
            CellLocation::ParamBase(pos) => {
                if bit_get(&self.p_dead, pos) || bit_get(&self.p_shadowed, pos) {
                    return None;
                }
                let target = self
                    .param_target_at(pos)
                    .expect("packed position always in a slice");
                Some(CoefficientData::new(
                    self.p_vars[pos as usize],
                    target,
                    ValueExpr::scaled_param(
                        self.p_scales[pos as usize],
                        self.p_params[pos as usize],
                    ),
                    self.p_cached[pos as usize],
                ))
            }
        }
    }

    /// Cached value only, without materializing the expression.
    pub fn cached_value(&self, id: CoeffId) -> Option<f64> {
        match self.location_of(id)? {
            CellLocation::Overlay(oi) => {
                let slot = &self.overlay[oi as usize];
                if slot.tombstone {
                    return None;
                }
                Some(slot.data.cached_value)
            }
            CellLocation::Packed(pos) => {
                if bit_get(&self.dead, pos) || bit_get(&self.shadowed, pos) {
                    return None;
                }
                Some(self.base_values[pos as usize])
            }
            CellLocation::ParamBase(pos) => {
                if bit_get(&self.p_dead, pos) || bit_get(&self.p_shadowed, pos) {
                    return None;
                }
                Some(self.p_cached[pos as usize])
            }
        }
    }

    /// Update the cached value of an overlay cell in place (parameter
    /// propagation path; packed constants never depend on parameters, so a
    /// packed location here is a logic error). Returns the previous data.
    pub fn set_cached_value(&mut self, id: CoeffId, value: f64) -> Option<CoefficientData> {
        match self.location_of(id)? {
            CellLocation::Overlay(oi) => {
                let slot = &mut self.overlay[oi as usize];
                if slot.tombstone {
                    return None;
                }
                let old = slot.data.clone();
                slot.data.cached_value = value;
                Some(old)
            }
            CellLocation::Packed(_) => {
                debug_assert!(false, "packed constant cells admit no parameter updates");
                None
            }
            CellLocation::ParamBase(_) => {
                debug_assert!(
                    false,
                    "packed parametric cells update through propagate_packed_param"
                );
                None
            }
        }
    }

    /// Whether an id resolves to a live overlay cell (as opposed to a
    /// packed-base cell, which has its own propagation path).
    pub(crate) fn is_overlay_cell(&self, id: CoeffId) -> bool {
        match self.location_of(id) {
            Some(CellLocation::Overlay(oi)) => !self.overlay[oi as usize].tombstone,
            _ => false,
        }
    }

    /// Check if a coefficient ID is valid.
    pub fn contains(&self, id: CoeffId) -> bool {
        self.ids.contains(id.index(), id.generation())
    }

    // ========== By-Variable Queries ==========

    /// Ensure the lazy packed-base variable index exists.
    fn ensure_var_index(&mut self) {
        if self.var_index.is_none() {
            self.var_index = Some(PackedVarIndex::build(&self.base_vars));
        }
    }

    /// Get all coefficients for a variable.
    ///
    /// Builds the packed-base index lazily on first use (hence `&mut`;
    /// variable deletion pays this once — an intentional trade so ordinary
    /// construction never builds an index it may not need); overlay
    /// mutations maintain only their delta lists. Base order follows the
    /// index, overlay order follows mutation order (callers needing
    /// determinism sort, as before — the old order was hash-random).
    pub fn for_var(&mut self, var: VarId) -> Vec<CoeffId> {
        self.ensure_var_index();
        let mut out = Vec::new();
        if let Some(index) = &self.var_index {
            for &pos in index.positions(var) {
                if bit_get(&self.dead, pos) || bit_get(&self.shadowed, pos) {
                    continue;
                }
                out.push(self.base_ids[pos as usize]);
            }
        }
        // Parametric base has no global index by design; a linear scan
        // over it is rare-path only (variable deletion cascades).
        for (pos, v) in self.p_vars.iter().enumerate() {
            let pos = pos as u32;
            if *v == var && !bit_get(&self.p_dead, pos) && !bit_get(&self.p_shadowed, pos) {
                out.push(self.p_ids[pos as usize]);
            }
        }
        if let Some(list) = self.overlay_by_var.get(&var) {
            for &oi in list {
                let slot = &self.overlay[oi as usize];
                if !slot.tombstone {
                    out.push(slot.id);
                }
            }
        }
        out
    }

    /// Check if a variable has any coefficients.
    pub fn var_has_coefficients(&mut self, var: VarId) -> bool {
        if self.p_vars.iter().enumerate().any(|(i, v)| {
            *v == var && !bit_get(&self.p_dead, i as u32) && !bit_get(&self.p_shadowed, i as u32)
        }) {
            return true;
        }
        if let Some(list) = self.overlay_by_var.get(&var) {
            if list.iter().any(|&oi| !self.overlay[oi as usize].tombstone) {
                return true;
            }
        }
        self.ensure_var_index();
        if let Some(index) = &self.var_index {
            return index
                .positions(var)
                .iter()
                .any(|&pos| !bit_get(&self.dead, pos) && !bit_get(&self.shadowed, pos));
        }
        false
    }

    // ========== By-Constraint Queries ==========

    /// Collect live cell ids for one target: slice order, then overlay
    /// mutation order. Deterministic (the old order was hash-random).
    fn live_ids_for_target(&self, target: CoefficientTarget) -> Vec<CoeffId> {
        let mut out = Vec::new();
        for &si in self.slices_for(target) {
            let slice = &self.slices[si];
            for k in 0..slice.len {
                let idx = slice.start + k;
                if bit_get(&self.dead, idx) || bit_get(&self.shadowed, idx) {
                    continue;
                }
                out.push(self.base_ids[idx as usize]);
            }
        }
        if let Some(slices) = self.p_directory.get(&target) {
            for &si in slices.as_slice() {
                let slice = &self.p_slices[si];
                for k in 0..slice.len {
                    let idx = slice.start + k;
                    if bit_get(&self.p_dead, idx) || bit_get(&self.p_shadowed, idx) {
                        continue;
                    }
                    out.push(self.p_ids[idx as usize]);
                }
            }
        }
        if let Some(list) = self.overlay_by_target.get(&target) {
            for &oi in list {
                let slot = &self.overlay[oi as usize];
                if !slot.tombstone {
                    out.push(slot.id);
                }
            }
        }
        out
    }

    /// Get all coefficients for a constraint.
    pub fn for_constraint(&self, con: ConId) -> impl Iterator<Item = CoeffId> + '_ {
        self.live_ids_for_target(CoefficientTarget::Constraint(con))
            .into_iter()
    }

    /// Check if a constraint has any coefficients.
    pub fn constraint_has_coefficients(&self, con: ConId) -> bool {
        !self
            .live_ids_for_target(CoefficientTarget::Constraint(con))
            .is_empty()
    }

    // ========== By-Objective Queries ==========

    /// Get all coefficients for an objective.
    pub fn for_objective(&self, obj: ObjId) -> impl Iterator<Item = CoeffId> + '_ {
        self.live_ids_for_target(CoefficientTarget::Objective(obj))
            .into_iter()
    }

    /// Check if an objective has any coefficients.
    pub fn objective_has_coefficients(&self, obj: ObjId) -> bool {
        !self
            .live_ids_for_target(CoefficientTarget::Objective(obj))
            .is_empty()
    }

    /// All live `(variable, cached value)` pairs for an objective, in
    /// deterministic slice-then-overlay order.
    pub fn objective_cells(&self, obj: ObjId) -> Vec<(VarId, f64)> {
        let target = CoefficientTarget::Objective(obj);
        let mut out = Vec::new();
        if let Some(slices) = self.p_directory.get(&target) {
            for &si in slices.as_slice() {
                let slice = &self.p_slices[si];
                for k in 0..slice.len {
                    let idx = slice.start + k;
                    if bit_get(&self.p_dead, idx) || bit_get(&self.p_shadowed, idx) {
                        continue;
                    }
                    out.push((self.p_vars[idx as usize], self.p_cached[idx as usize]));
                }
            }
        }
        for &si in self.slices_for(target) {
            let slice = &self.slices[si];
            for k in 0..slice.len {
                let idx = slice.start + k;
                if bit_get(&self.dead, idx) || bit_get(&self.shadowed, idx) {
                    continue;
                }
                out.push((self.base_vars[idx as usize], self.base_values[idx as usize]));
            }
        }
        if let Some(list) = self.overlay_by_target.get(&target) {
            for &oi in list {
                let slot = &self.overlay[oi as usize];
                if !slot.tombstone {
                    out.push((slot.data.var, slot.data.cached_value));
                }
            }
        }
        out
    }

    // ========== By-Parameter Queries (Dependency Graph) ==========

    /// Get all coefficients that depend on a parameter.
    ///
    /// Complete across all three dependency representations: overlay cells,
    /// non-eligible per-cell packed positions, and eligible dependency blocks
    /// (MIR-02). A parameter covered by a block reports its block cells even
    /// though no `param_positions` entry exists.
    pub fn for_param(&self, param: ParamId) -> impl Iterator<Item = CoeffId> + '_ {
        let over = self
            .overlay_by_param
            .get(&param)
            .into_iter()
            .flat_map(|list| list.iter())
            .filter_map(|&oi| {
                let slot = &self.overlay[oi as usize];
                if slot.tombstone {
                    None
                } else {
                    Some(slot.id)
                }
            });
        let packed = self
            .param_positions
            .get(&param)
            .into_iter()
            .flat_map(|list| list.iter())
            .filter_map(|&pos| {
                if bit_get(&self.p_dead, pos) || bit_get(&self.p_shadowed, pos) {
                    None
                } else {
                    Some(self.p_ids[pos as usize])
                }
            });
        let block_ids = self.block_dep_ids_for_param(param);
        over.chain(packed).chain(block_ids)
    }

    /// Collect the live cell ids of dependency blocks covering `param`.
    fn block_dep_ids_for_param(&self, param: ParamId) -> Vec<CoeffId> {
        let mut out = Vec::new();
        let pi = param.index() as usize;
        for block in &self.param_dep_blocks {
            let p0 = block.params.start() as usize;
            let p1 = p0 + block.params.len();
            if pi < p0 || pi >= p1 {
                continue;
            }
            for k in 0..block.param_map.len() {
                let param_offset = match block.param_map.get(k) {
                    Some(v) if v >= 0 => v as usize,
                    _ => continue,
                };
                if param_offset >= block.params.len() {
                    continue;
                }
                if block.params.id_at(param_offset) != Some(param) {
                    continue;
                }
                let cell_offset = match block.cell_map.get(k) {
                    Some(v) if v >= 0 => v as usize,
                    _ => continue,
                };
                let pos = block.cell_start as usize + cell_offset;
                if pos >= self.p_vars.len() {
                    continue;
                }
                if bit_get(&self.p_dead, pos as u32) || bit_get(&self.p_shadowed, pos as u32) {
                    continue;
                }
                out.push(self.p_ids[pos]);
            }
        }
        out
    }

    /// Collect live overlay cell ids depending on `param` (overlay map only).
    ///
    /// Unlike [`Self::for_param`] this never consults the packed reverse index
    /// or dependency blocks, so it is zero-work on a fully eligible family.
    pub(crate) fn overlay_ids_for_param(&self, param: ParamId) -> Vec<CoeffId> {
        self.overlay_by_param
            .get(&param)
            .into_iter()
            .flat_map(|list| list.iter())
            .filter_map(|&oi| {
                let slot = &self.overlay[oi as usize];
                if slot.tombstone {
                    None
                } else {
                    Some(slot.id)
                }
            })
            .collect()
    }

    /// Check if a parameter has any dependent coefficients.
    pub fn param_has_dependents(&self, param: ParamId) -> bool {
        self.for_param(param).next().is_some()
    }

    /// Get the count of coefficients depending on a parameter.
    pub fn param_dependent_count(&self, param: ParamId) -> usize {
        self.for_param(param).count()
    }

    // ========== General Queries ==========

    /// Get the total number of live coefficients.
    pub fn len(&self) -> usize {
        self.live
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Iterate over all live coefficients with owned data.
    /// Iterate over all live coefficients with owned data, slice by slice
    /// (slice outer loop, cells inner loop) so iteration itself is linear
    /// in the live cells — never a scan per coefficient.
    pub fn iter(&self) -> impl Iterator<Item = (CoeffId, CoefficientData)> + '_ {
        let base = self.slices.iter().flat_map(|slice| {
            (slice.start..slice.start + slice.len).filter_map(|idx| {
                if bit_get(&self.dead, idx) || bit_get(&self.shadowed, idx) {
                    return None;
                }
                let value = self.base_values[idx as usize];
                Some((
                    self.base_ids[idx as usize],
                    CoefficientData::new(
                        self.base_vars[idx as usize],
                        slice.target,
                        ValueExpr::constant(value),
                        value,
                    ),
                ))
            })
        });
        let param = self.p_slices.iter().flat_map(|slice| {
            (slice.start..slice.start + slice.len).filter_map(|idx| {
                if bit_get(&self.p_dead, idx) || bit_get(&self.p_shadowed, idx) {
                    return None;
                }
                Some((
                    self.p_ids[idx as usize],
                    CoefficientData::new(
                        self.p_vars[idx as usize],
                        slice.target,
                        ValueExpr::scaled_param(
                            self.p_scales[idx as usize],
                            self.p_params[idx as usize],
                        ),
                        self.p_cached[idx as usize],
                    ),
                ))
            })
        });
        let over = self.overlay.iter().filter_map(|slot| {
            if slot.tombstone {
                None
            } else {
                Some((slot.id, slot.data.clone()))
            }
        });
        base.chain(param).chain(over)
    }

    // ========== Cell Queries ==========

    /// Get the coefficient ID for a specific (target, variable) cell.
    /// Returns `None` if no live coefficient exists for that cell.
    pub fn for_cell(&self, target: CoefficientTarget, var: VarId) -> Option<CoeffId> {
        self.live_id(target, var)
    }

    /// Check if a live cell exists for (target, variable).
    pub fn cell_exists(&self, target: CoefficientTarget, var: VarId) -> bool {
        self.live_id(target, var).is_some()
    }

    // ========== Consistency (invariant checking) ==========

    /// Structural audit of base/overlay agreement. Returns violation
    /// descriptions (empty when consistent). Used by model invariant
    /// checking in place of the former per-index walks.
    pub(crate) fn check_consistency(&self) -> Vec<String> {
        let mut violations = Vec::new();
        // Slices cover disjoint packed ranges within the base arrays.
        let mut covered = vec![false; self.base_vars.len()];
        for (si, slice) in self.slices.iter().enumerate() {
            let end = slice.start as usize + slice.len as usize;
            if end > self.base_vars.len()
                || end > self.base_values.len()
                || end > self.base_ids.len()
            {
                violations.push(format!("slice {si} out of range"));
                continue;
            }
            for (idx, slot_covered) in covered
                .iter_mut()
                .enumerate()
                .take(end)
                .skip(slice.start as usize)
            {
                if *slot_covered {
                    violations.push(format!("packed position {idx} covered twice"));
                }
                *slot_covered = true;
            }
            match self.directory.get(&slice.target) {
                Some(list) if list.as_slice().contains(&si) => {}
                _ => violations.push(format!("slice {si} missing from directory")),
            }
        }
        // Every live arena identity resolves to a live cell and back.
        for (idx, gen, loc) in self.ids.iter() {
            let id = CoeffId::new(idx, gen);
            match loc {
                CellLocation::Packed(pos) => {
                    if *pos as usize >= self.base_vars.len() {
                        violations.push(format!("packed location {pos} out of range"));
                        continue;
                    }
                    if bit_get(&self.dead, *pos) || bit_get(&self.shadowed, *pos) {
                        violations.push(format!("live id {id:?} on dead/shadowed base"));
                    }
                    if self.base_ids[*pos as usize] != id {
                        violations.push(format!("base id mismatch at {pos}"));
                    }
                }
                CellLocation::Overlay(oi) => match self.overlay.get(*oi as usize) {
                    Some(slot) if !slot.tombstone && slot.id == id => {}
                    _ => violations.push(format!("live id {id:?} missing overlay slot")),
                },
                CellLocation::ParamBase(pos) => {
                    if *pos as usize >= self.p_vars.len() {
                        violations.push(format!("param location {pos} out of range"));
                        continue;
                    }
                    if bit_get(&self.p_dead, *pos) || bit_get(&self.p_shadowed, *pos) {
                        violations.push(format!("live id {id:?} on dead/shadowed parambase"));
                    }
                    if self.p_ids[*pos as usize] != id {
                        violations.push(format!("parambase id mismatch at {pos}"));
                    }
                }
            }
        }
        // Overlay slots agree with their delta lists.
        for (oi, slot) in self.overlay.iter().enumerate() {
            let oi = oi as u32;
            let key = (slot.data.target, slot.data.var);
            match self.overlay_by_cell.get(&key) {
                Some(&found) if found == oi => {}
                _ => violations.push(format!("overlay slot {oi} missing cell link")),
            }
            if !slot.tombstone {
                // Live slots must be reachable through the arena identity.
                match self.ids.get(slot.id.index(), slot.id.generation()) {
                    Some(CellLocation::Overlay(found)) if *found == oi => {}
                    other => {
                        violations.push(format!("overlay slot {oi} identity mismatch: {other:?}"))
                    }
                }
            }
        }
        // Live counter exactness (constant base, parametric base, overlay).
        let mut counted = 0usize;
        for idx in 0..self.base_vars.len() as u32 {
            if !bit_get(&self.dead, idx) && !bit_get(&self.shadowed, idx) {
                counted += 1;
            }
        }
        for idx in 0..self.p_vars.len() as u32 {
            if !bit_get(&self.p_dead, idx) && !bit_get(&self.p_shadowed, idx) {
                counted += 1;
            }
        }
        counted += self.overlay.iter().filter(|s| !s.tombstone).count();
        if counted != self.live {
            violations.push(format!("live counter {} != counted {counted}", self.live));
        }
        // Parametric slices cover disjoint in-range runs with directory
        // membership, mirroring the constant-base audit above.
        {
            let mut covered = vec![false; self.p_vars.len()];
            for (si, slice) in self.p_slices.iter().enumerate() {
                let end = slice.start as usize + slice.len as usize;
                if end > self.p_vars.len()
                    || end > self.p_params.len()
                    || end > self.p_scales.len()
                    || end > self.p_cached.len()
                    || end > self.p_ids.len()
                {
                    violations.push(format!("param slice {si} out of range"));
                    continue;
                }
                for (idx, slot_covered) in covered
                    .iter_mut()
                    .enumerate()
                    .take(end)
                    .skip(slice.start as usize)
                {
                    if *slot_covered {
                        violations.push(format!("param position {idx} covered twice"));
                    }
                    *slot_covered = true;
                }
                match self.p_directory.get(&slice.target) {
                    Some(list) if list.as_slice().contains(&si) => {}
                    _ => violations.push(format!("param slice {si} missing from directory")),
                }
            }
        }
        // Reverse parameter index agreement: every live parametric position
        // is listed exactly under its own parameter **unless** it is covered
        // by an eligible dependency block (MIR-02, which deliberately keeps no
        // per-cell index); every listed position is live.
        {
            use std::collections::HashSet;
            // Positions covered by eligible dependency blocks.
            let mut block_covered: HashSet<u32> = HashSet::new();
            for (bi, block) in self.param_dep_blocks.iter().enumerate() {
                if block.cell_start as usize > self.p_vars.len() {
                    violations.push(format!("dependency block {bi} cell_start out of range"));
                }
                for k in 0..block.param_map.len() {
                    let (param_offset, cell_offset) =
                        (block.param_map.get(k), block.cell_map.get(k));
                    let (Some(param_offset), Some(cell_offset)) = (param_offset, cell_offset)
                    else {
                        violations.push(format!("dependency block {bi} map overflow at {k}"));
                        continue;
                    };
                    if param_offset < 0 || param_offset as usize >= block.params.len() {
                        violations.push(format!(
                            "dependency block {bi} parameter offset {param_offset}"
                        ));
                        continue;
                    }
                    if cell_offset < 0 {
                        violations.push(format!("dependency block {bi} negative cell offset"));
                        continue;
                    }
                    let pos = block.cell_start as usize + cell_offset as usize;
                    if pos >= self.p_vars.len() {
                        violations.push(format!("dependency block {bi} cell {pos} out of range"));
                        continue;
                    }
                    block_covered.insert(pos as u32);
                    if block.params.id_at(param_offset as usize) != Some(self.p_params[pos]) {
                        violations
                            .push(format!("dependency block {bi} parameter mismatch at {pos}"));
                    }
                    if self.p_scales[pos] != block.scale {
                        violations.push(format!("dependency block {bi} scale mismatch at {pos}"));
                    }
                }
            }
            let mut indexed: HashSet<u32> = HashSet::new();
            for (param, list) in self.param_positions.iter() {
                for &pos in list {
                    if !indexed.insert(pos) {
                        violations.push(format!("param position {pos} indexed twice"));
                    }
                    if pos as usize >= self.p_vars.len() {
                        violations.push(format!("param index out of range: {pos}"));
                        continue;
                    }
                    if self.p_params[pos as usize] != *param {
                        violations.push(format!("param index mismatch at {pos}"));
                    }
                    if bit_get(&self.p_dead, pos) || bit_get(&self.p_shadowed, pos) {
                        violations.push(format!("dead param position {pos} still indexed"));
                    }
                    if block_covered.contains(&pos) {
                        violations.push(format!(
                            "eligible block position {pos} also has a per-cell index entry"
                        ));
                    }
                }
            }
            for pos in 0..self.p_vars.len() as u32 {
                let live = !bit_get(&self.p_dead, pos) && !bit_get(&self.p_shadowed, pos);
                if live && !indexed.contains(&pos) && !block_covered.contains(&pos) {
                    violations.push(format!("live param position {pos} missing from index"));
                }
            }
        }
        // Live-key uniqueness across bases: one live cell per key.
        {
            use std::collections::HashSet;
            let mut seen: HashSet<CellKey> = HashSet::new();
            for (_, data) in self.iter() {
                if !seen.insert((data.target, data.var)) {
                    violations.push(format!("duplicate live cell {:?}", (data.target, data.var)));
                }
            }
        }
        // Lazy index agreement when built: every entry must point at its
        // variable, and together the entries must cover the whole packed
        // base (the index is append-invalidated, so a built index that
        // misses a suffix indicates a real staleness bug).
        if let Some(index) = &self.var_index {
            let mut covered = vec![false; self.base_vars.len()];
            for (k, var) in index.vars_sorted.iter().enumerate() {
                let s = index.var_offsets[k] as usize;
                let e = index.var_offsets[k + 1] as usize;
                for &pos in &index.positions[s..e] {
                    if self.base_vars.get(pos as usize) != Some(var) {
                        violations.push(format!("var index mismatch for {var:?}"));
                    } else {
                        covered[pos as usize] = true;
                    }
                }
            }
            if covered.iter().any(|c| !c) {
                violations.push("var index misses packed base positions".to_string());
            }
        }
        violations
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Generation;
    use std::collections::HashSet;

    fn make_var(index: u32) -> VarId {
        VarId::new(index, Generation::new())
    }

    fn make_con(index: u32) -> ConId {
        ConId::new(index, Generation::new())
    }

    fn make_obj(index: u32) -> ObjId {
        ObjId::new(index, Generation::new())
    }

    fn make_param(index: u32) -> ParamId {
        ParamId::new(index, Generation::new())
    }

    #[test]
    fn add_and_lookup() {
        let mut index = CoefficientIndex::new();
        let var = make_var(0);
        let con = make_con(0);

        let id = index.add(
            var,
            CoefficientTarget::Constraint(con),
            ValueExpr::constant(2.0),
            2.0,
        );

        assert!(index.contains(id));
        let data = index.get(id).unwrap();
        assert_eq!(data.var, var);
        assert_eq!(data.cached_value, 2.0);
    }

    #[test]
    fn by_var_index() {
        let mut index = CoefficientIndex::new();
        let var1 = make_var(0);
        let var2 = make_var(1);
        let con1 = make_con(0);
        let con2 = make_con(1);

        // Two coefficients for var1 on different constraints
        let id1 = index.add(
            var1,
            CoefficientTarget::Constraint(con1),
            ValueExpr::constant(1.0),
            1.0,
        );
        let id2 = index.add(
            var1,
            CoefficientTarget::Constraint(con2),
            ValueExpr::constant(2.0),
            2.0,
        );

        // P1.5B mechanical adaptation (documented): `for_var` now returns
        // an owned Vec (lazy base index requires `&mut`); same assertions.
        let var1_coeffs: HashSet<_> = index.for_var(var1).into_iter().collect();
        assert_eq!(var1_coeffs.len(), 2);
        assert!(var1_coeffs.contains(&id1));
        assert!(var1_coeffs.contains(&id2));

        // var2 has no coefficients
        assert!(index.for_var(var2).is_empty());
    }

    #[test]
    fn by_constraint_index() {
        let mut index = CoefficientIndex::new();
        let var = make_var(0);
        let con1 = make_con(0);
        let con2 = make_con(1);

        let id1 = index.add(
            var,
            CoefficientTarget::Constraint(con1),
            ValueExpr::constant(1.0),
            1.0,
        );
        let _id2 = index.add(
            var,
            CoefficientTarget::Constraint(con2),
            ValueExpr::constant(2.0),
            2.0,
        );

        let con1_coeffs: Vec<_> = index.for_constraint(con1).collect();
        assert_eq!(con1_coeffs, vec![id1]);
    }

    #[test]
    fn canonical_cell_combines_expressions() {
        let mut index = CoefficientIndex::new();
        let var = make_var(0);
        let con = make_con(0);
        let p1 = make_param(0);

        // Add first term: p1 * var
        let id1 = index.add(
            var,
            CoefficientTarget::Constraint(con),
            ValueExpr::param(p1),
            1.0,
        );

        // Add second term: 2.0 * var (constant) for the SAME cell
        // Should combine with existing: p1*var + 2.0*var = (p1+2.0)*var
        let id2 = index.add(
            var,
            CoefficientTarget::Constraint(con),
            ValueExpr::constant(2.0),
            2.0,
        );

        // Should return the same coefficient ID (combined in place)
        assert_eq!(id1, id2, "canonical cell should return same ID on combine");

        let data = index.get(id1).unwrap();
        // Cached value combines: 1.0 + 2.0 = 3.0
        assert_eq!(data.cached_value, 3.0);

        // p1 still depends on this coefficient
        assert!(index.param_has_dependents(p1));

        // Only one coefficient exists for this cell
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn separate_cells_do_not_combine() {
        let mut index = CoefficientIndex::new();
        let var = make_var(0);
        let con1 = make_con(0);
        let con2 = make_con(1);
        let p1 = make_param(0);

        // Different constraints → different cells
        let id1 = index.add(
            var,
            CoefficientTarget::Constraint(con1),
            ValueExpr::param(p1),
            1.0,
        );
        let id2 = index.add(
            var,
            CoefficientTarget::Constraint(con2),
            ValueExpr::constant(3.0),
            3.0,
        );

        assert_ne!(id1, id2);
        assert_eq!(index.len(), 2);

        // p1 depends only on id1
        let p1_coeffs: Vec<_> = index.for_param(p1).collect();
        assert_eq!(p1_coeffs, vec![id1]);
    }

    #[test]
    fn remove_cleans_indexes() {
        let mut index = CoefficientIndex::new();
        let var = make_var(0);
        let con = make_con(0);
        let param = make_param(0);

        let id = index.add(
            var,
            CoefficientTarget::Constraint(con),
            ValueExpr::param(param),
            1.0,
        );

        assert!(index.var_has_coefficients(var));
        assert!(index.constraint_has_coefficients(con));
        assert!(index.param_has_dependents(param));

        index.remove(id);

        assert!(!index.var_has_coefficients(var));
        assert!(!index.constraint_has_coefficients(con));
        assert!(!index.param_has_dependents(param));
    }

    #[test]
    fn objective_coefficients() {
        let mut index = CoefficientIndex::new();
        let var = make_var(0);
        let obj = make_obj(0);

        let id = index.add(
            var,
            CoefficientTarget::Objective(obj),
            ValueExpr::constant(5.0),
            5.0,
        );

        assert!(index.objective_has_coefficients(obj));
        let obj_coeffs: Vec<_> = index.for_objective(obj).collect();
        assert_eq!(obj_coeffs, vec![id]);
    }
}

#[cfg(test)]
mod stale_identity_tests {
    //! P1.5B: stale-`CoeffId` behavior must match the old store exactly —
    //! removal bumps the arena generation; every accessor then reports
    //! absence and repeat removal is `None`.
    use super::*;
    use crate::id::Generation;

    fn make_var(index: u32) -> VarId {
        VarId::new(index, Generation::new())
    }

    fn make_con(index: u32) -> ConId {
        ConId::new(index, Generation::new())
    }

    #[test]
    fn removed_overlay_id_goes_stale() {
        let mut index = CoefficientIndex::new();
        let var = make_var(0);
        let con = make_con(0);
        let id = index.add(
            var,
            CoefficientTarget::Constraint(con),
            ValueExpr::constant(1.0),
            1.0,
        );
        assert!(index.contains(id));
        assert!(index.remove(id).is_some());
        assert!(!index.contains(id));
        assert!(index.get(id).is_none());
        assert!(index.remove(id).is_none());
        assert!(index
            .for_cell(CoefficientTarget::Constraint(con), var)
            .is_none());
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn removed_packed_id_goes_stale() {
        let mut index = CoefficientIndex::new();
        let obj = ObjId::new(0, Generation::new());
        let vars = [make_var(0), make_var(1)];
        index.append_constant_block(
            CoefficientTarget::Objective(obj),
            &[(vars[0], 1.0), (vars[1], 2.0)],
        );
        let id = index
            .for_cell(CoefficientTarget::Objective(obj), vars[0])
            .unwrap();
        assert!(index.remove(id).is_some());
        assert!(!index.contains(id));
        assert!(index.get(id).is_none());
        assert!(index
            .for_cell(CoefficientTarget::Objective(obj), vars[0])
            .is_none());
        // Sibling cell unaffected.
        assert!(index
            .for_cell(CoefficientTarget::Objective(obj), vars[1])
            .is_some());
        assert_eq!(index.len(), 1);
    }
}

#[cfg(test)]
mod perf_probe_tests {
    //! Manual release-only measurement harness (P1.5B gate evidence).
    //!
    //! `#[ignore]`d so CI never depends on wall time. Run explicitly:
    //! `cargo test -p roml --release --lib perf_probe_append_1m -- --ignored --nocapture`
    //! Prints store-construction time for 1M canonical constant cells.
    use super::*;
    use crate::id::Generation;

    #[test]
    // quality-exception: manual release-only performance probe; CI must not depend on wall time.
    #[ignore]
    fn perf_probe_append_1m() {
        use std::time::Instant;
        let n = 1_000_000usize;
        let obj = ObjId::new(0, Generation::new());
        let vars: Vec<VarId> = (0..n as u32)
            .map(|i| VarId::new(i, Generation::new()))
            .collect();
        let vals: Vec<f64> = (0..n).map(|i| (i % 97) as f64 + 0.5).collect();
        let cells: Vec<(VarId, f64)> = vars.into_iter().zip(vals).collect();
        let mut index = CoefficientIndex::new();
        let t0 = Instant::now();
        index.append_constant_block(CoefficientTarget::Objective(obj), &cells);
        let dt = t0.elapsed();
        assert_eq!(index.len(), n);
        println!("store append 1M: {:.1} ms", dt.as_secs_f64() * 1e3);
    }
}

#[cfg(test)]
mod row_block_tests {
    //! P1A locks: bulk row-block append semantics.
    use super::*;
    use crate::id::Generation;

    fn var(i: u32) -> VarId {
        VarId::new(i, Generation::new())
    }
    fn con(i: u32) -> ConId {
        ConId::new(i, Generation::new())
    }
    fn tgt(i: u32) -> CoefficientTarget {
        CoefficientTarget::Constraint(con(i))
    }

    #[test]
    fn canonical_rows_append_directly() {
        let mut index = CoefficientIndex::new();
        let targets = [tgt(0), tgt(1), tgt(2)];
        // Row 1 empty; rows canonical and sorted.
        let row_ptr = [0, 2, 2, 5];
        let vars = [var(0), var(1), var(4), var(2), var(3)];
        let vals = [1.0, 2.0, 4.0, 2.0, 3.0];
        index
            .append_row_block(&targets, &row_ptr, &vars, &vals)
            .unwrap();
        assert_eq!(index.len(), 5);
        assert_eq!(
            index
                .for_cell(tgt(0), var(1))
                .map(|id| index.cached_value(id).unwrap()),
            Some(2.0)
        );
        // Empty row resolves nothing but exists structurally.
        assert!(index.for_cell(tgt(1), var(0)).is_none());
        // Row 2 input is unsorted ([4,2,3]): exercises the fallback
        // canonicalization, still 3 cells.
        let row2: Vec<(VarId, f64)> = index
            .for_constraint(con(2))
            .map(|id| {
                let d = index.get(id).unwrap();
                (d.var, d.cached_value)
            })
            .collect();
        assert_eq!(row2.len(), 3);
    }

    #[test]
    fn shuffled_and_duplicated_rows_canonicalize() {
        let mut index = CoefficientIndex::new();
        let targets = [tgt(0)];
        // Shuffled with a duplicate pair summing to 3.0.
        let row_ptr = [0, 4];
        let vars = [var(5), var(1), var(5), var(3)];
        let vals = [1.0, 2.0, 2.0, 3.0];
        index
            .append_row_block(&targets, &row_ptr, &vars, &vals)
            .unwrap();
        assert_eq!(index.len(), 3);
        // Merged duplicate reads back combined.
        let id = index.for_cell(tgt(0), var(5)).unwrap();
        assert_eq!(index.cached_value(id).unwrap(), 3.0);
        // Canonical order observable through iteration.
        let seen: Vec<VarId> = index
            .for_constraint(con(0))
            .map(|id| index.get(id).unwrap().var)
            .collect();
        let mut sorted = seen.clone();
        sorted.sort();
        assert_eq!(seen, sorted);
    }

    #[test]
    fn cancellation_and_merged_overflow() {
        let mut index = CoefficientIndex::new();
        let targets = [tgt(0), tgt(1)];
        let row_ptr = [0, 2, 4];
        // Row 0 cancels to zero (dropped); row 1 overflows on merge.
        let vars = [var(0), var(0), var(1), var(1)];
        let vals = [1.0, -1.0, f64::MAX, f64::MAX];
        let before = index.len();
        let err = index
            .append_row_block(&targets, &row_ptr, &vars, &vals)
            .unwrap_err();
        assert!(matches!(err, crate::model::ModelError::NonFiniteValue(_)));
        // Atomic: nothing appended.
        assert_eq!(index.len(), before);
        assert!(index.for_cell(tgt(0), var(0)).is_none());
        assert!(index.for_cell(tgt(1), var(1)).is_none());
    }
}

#[cfg(test)]
mod perf_probe_rows_tests {
    //! Manual release-only measurement harness (P1A gate evidence).
    //! `#[ignore]`d; run explicitly with `-- --ignored --nocapture`.
    use super::*;
    use crate::id::Generation;

    #[test]
    // quality-exception: manual release-only performance probe; CI must not depend on wall time.
    #[ignore]
    fn perf_probe_append_rows_100k() {
        use std::time::Instant;
        let nrows = 10_000usize;
        let targets: Vec<CoefficientTarget> = (0..nrows as u32)
            .map(|i| CoefficientTarget::Constraint(ConId::new(i, Generation::new())))
            .collect();
        let mut row_ptr = Vec::with_capacity(nrows + 1);
        let mut vars = Vec::with_capacity(nrows * 10);
        let mut vals = Vec::with_capacity(nrows * 10);
        row_ptr.push(0);
        for r in 0..nrows {
            for k in 0..10 {
                vars.push(VarId::new((10 * r + k) as u32, Generation::new()));
                vals.push(1.0);
            }
            row_ptr.push(vars.len() as u32);
        }
        let mut index = CoefficientIndex::new();
        let t0 = Instant::now();
        index
            .append_row_block(&targets, &row_ptr, &vars, &vals)
            .unwrap();
        let dt = t0.elapsed();
        assert_eq!(index.len(), nrows * 10);
        println!(
            "store append 100kx10 rows: {:.1} ms",
            dt.as_secs_f64() * 1e3
        );
    }
}
