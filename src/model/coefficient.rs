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

use crate::id::{CoeffId, ConId, IdArena, ObjId, ParamId, VarId};
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
    /// Position in the packed base arrays.
    Packed(u32),
    /// Slot in the sparse overlay.
    Overlay(u32),
}

/// One contiguous run of packed cells for a single target.
#[derive(Clone, Copy, Debug)]
struct TargetSlice {
    target: CoefficientTarget,
    start: u32,
    len: u32,
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
    directory: HashMap<CoefficientTarget, Vec<usize>>,
    shadowed: Vec<u64>,
    dead: Vec<u64>,
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

    /// Base position for a live packed cell: present slice, sorted hit,
    /// neither dead nor shadowed.
    fn base_position(&self, target: CoefficientTarget, var: VarId) -> Option<u32> {
        let slices = self.directory.get(&target)?;
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
        self.base_position(target, var)
            .map(|idx| self.base_ids[idx as usize])
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
        self.directory.entry(target).or_default().push(slice_idx);
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
                let (var, target) = {
                    let slice = self
                        .slices
                        .iter()
                        .find(|s| pos >= s.start && pos < s.start + s.len)
                        .expect("packed position always in a slice");
                    (self.base_vars[pos as usize], slice.target)
                };
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
                    .slices
                    .iter()
                    .find(|s| pos >= s.start && pos < s.start + s.len)
                    .map(|s| s.target)
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
                    .slices
                    .iter()
                    .find(|s| pos >= s.start && pos < s.start + s.len)
                    .map(|s| s.target)
                    .expect("packed position always in a slice");
                Some(CoefficientData::new(
                    self.base_vars[pos as usize],
                    target,
                    ValueExpr::constant(value),
                    value,
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
        if let Some(slices) = self.directory.get(&target) {
            for &si in slices {
                let slice = &self.slices[si];
                for k in 0..slice.len {
                    let idx = slice.start + k;
                    if bit_get(&self.dead, idx) || bit_get(&self.shadowed, idx) {
                        continue;
                    }
                    out.push(self.base_ids[idx as usize]);
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
        if let Some(slices) = self.directory.get(&target) {
            for &si in slices {
                let slice = &self.slices[si];
                for k in 0..slice.len {
                    let idx = slice.start + k;
                    if bit_get(&self.dead, idx) || bit_get(&self.shadowed, idx) {
                        continue;
                    }
                    out.push((self.base_vars[idx as usize], self.base_values[idx as usize]));
                }
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
    /// Overlay-only: packed cells are constant by construction.
    pub fn for_param(&self, param: ParamId) -> impl Iterator<Item = CoeffId> + '_ {
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
    pub fn iter(&self) -> impl Iterator<Item = (CoeffId, CoefficientData)> + '_ {
        let base = (0..self.base_vars.len() as u32).filter_map(|idx| {
            if bit_get(&self.dead, idx) || bit_get(&self.shadowed, idx) {
                return None;
            }
            let target = self
                .slices
                .iter()
                .find(|s| idx >= s.start && idx < s.start + s.len)
                .map(|s| s.target)?;
            let value = self.base_values[idx as usize];
            Some((
                self.base_ids[idx as usize],
                CoefficientData::new(
                    self.base_vars[idx as usize],
                    target,
                    ValueExpr::constant(value),
                    value,
                ),
            ))
        });
        let over = self.overlay.iter().filter_map(|slot| {
            if slot.tombstone {
                None
            } else {
                Some((slot.id, slot.data.clone()))
            }
        });
        base.chain(over)
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
                Some(list) if list.contains(&si) => {}
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
        // Live counter exactness.
        let mut counted = 0usize;
        for idx in 0..self.base_vars.len() as u32 {
            if !bit_get(&self.dead, idx) && !bit_get(&self.shadowed, idx) {
                counted += 1;
            }
        }
        counted += self.overlay.iter().filter(|s| !s.tombstone).count();
        if counted != self.live {
            violations.push(format!("live counter {} != counted {counted}", self.live));
        }
        // Lazy index agreement when built.
        if let Some(index) = &self.var_index {
            for (k, var) in index.vars_sorted.iter().enumerate() {
                let s = index.var_offsets[k] as usize;
                let e = index.var_offsets[k + 1] as usize;
                for &pos in &index.positions[s..e] {
                    if self.base_vars.get(pos as usize) != Some(var) {
                        violations.push(format!("var index mismatch for {var:?}"));
                    }
                }
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
    #[ignore]
    fn perf_probe_append_1m() {
        use std::time::Instant;
        let n = 1_000_000usize;
        let obj = ObjId::new(0, Generation::new());
        let vars: Vec<VarId> = (0..n as u32).map(|i| VarId::new(i, Generation::new())).collect();
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
