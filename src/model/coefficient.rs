//! Coefficient storage with multi-indexing.
//!
//! Coefficients are first-class objects linking variables to targets (constraints or objectives).
//! They support efficient lookup by:
//! - Variable (for deletion, solver projection)
//! - Constraint (for deletion, iteration)
//! - Objective (for deletion, iteration)
//! - Parameter (for value propagation)
//!
//! Key idea is the use of expr from which value can be evaluated.

use std::collections::{HashMap, HashSet};

use crate::id::{CoeffId, ConId, IdArena, ObjId, ParamId, VarId};
use crate::value_expr::ValueExpr;

/// Target of a coefficient (constraint or objective).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

/// Multi-indexed coefficient storage.
///
/// Provides O(1) lookup in multiple dimensions:
/// - By coefficient ID (primary)
/// - By variable (for deletion cascades)
/// - By constraint (for constraint operations)
/// - By objective (for objective operations)
/// - By parameter (for value propagation)
#[derive(Clone, Debug, Default)]
pub struct CoefficientIndex {
    /// Primary Storage
    arena: IdArena<CoefficientData>,

    /// Coefficients by variable: VarId -> Set of CoeffIds.
    by_var: HashMap<VarId, HashSet<CoeffId>>,

    /// Coefficients by constraint: ConId -> Set of CoeffIds.
    by_constraint: HashMap<ConId, HashSet<CoeffId>>,

    /// Coefficients by objective: ObjId -> Set of CoeffIds.
    by_objective: HashMap<ObjId, HashSet<CoeffId>>,

    /// Coefficients by parameter (dependency graph): ParamId -> Set of CoeffIds.
    by_param: HashMap<ParamId, HashSet<CoeffId>>,
}

impl CoefficientIndex {
    /// Create an empty coefficient index.
    pub fn new() -> Self {
        Self {
            arena: IdArena::new(),
            by_var: HashMap::new(),
            by_constraint: HashMap::new(),
            by_objective: HashMap::new(),
            by_param: HashMap::new(),
        }
    }

    /// Add a new coefficient.
    ///
    /// Automatically updates all secondary indexes based on the value expression's dependencies.
    pub fn add(
        &mut self,
        var: VarId,
        target: CoefficientTarget,
        value_expr: ValueExpr,
        initial_value: f64,
    ) -> CoeffId {
        let data = CoefficientData::new(var, target, value_expr.clone(), initial_value);
        let (index, generation) = self.arena.allocate(data);
        let id = CoeffId::new(index, generation);

        // Update by_var index
        self.by_var.entry(var).or_default().insert(id);

        // Update by_constraint or by_objective index
        match target {
            CoefficientTarget::Constraint(con) => {
                self.by_constraint.entry(con).or_default().insert(id);
            }
            CoefficientTarget::Objective(obj) => {
                self.by_objective.entry(obj).or_default().insert(id);
            }
        }

        // Update by_param based on dependencies
        for param in value_expr.dependencies() {
            self.by_param.entry(param).or_default().insert(id);
        }

        id
    }

    /// Remove a coefficient by ID.
    ///
    /// Returns the data if it existed. Automatically cleans up all secondary indices.
    pub fn remove(&mut self, id: CoeffId) -> Option<CoefficientData> {
        let data = self.arena.remove(id.index(), id.generation())?;

        // Clean up by_var index
        if let Some(set) = self.by_var.get_mut(&data.var) {
            set.remove(&id);
            if set.is_empty() {
                self.by_var.remove(&data.var);
            }
        }

        // Clean up by_constraint or by_objective index
        match data.target {
            CoefficientTarget::Constraint(con) => {
                if let Some(set) = self.by_constraint.get_mut(&con) {
                    set.remove(&id);
                    if set.is_empty() {
                        self.by_constraint.remove(&con);
                    }
                }
            }
            CoefficientTarget::Objective(obj) => {
                if let Some(set) = self.by_objective.get_mut(&obj) {
                    set.remove(&id);
                    if set.is_empty() {
                        self.by_objective.remove(&obj);
                    }
                }
            }
        }

        // Clean up by_param index
        for param in data.value_expr.dependencies() {
            if let Some(set) = self.by_param.get_mut(&param) {
                set.remove(&id);
                if set.is_empty() {
                    self.by_param.remove(&param);
                }
            }
        }

        Some(data)
    }

    /// Get coefficient data by ID.
    pub fn get(&self, id: CoeffId) -> Option<&CoefficientData> {
        self.arena.get(id.index(), id.generation())
    }

    /// Get mutable coefficient data by ID.
    pub fn get_mut(&mut self, id: CoeffId) -> Option<&mut CoefficientData> {
        self.arena.get_mut(id.index(), id.generation())
    }

    /// Check if a coefficient ID is valid.
    pub fn contains(&self, id: CoeffId) -> bool {
        self.arena.contains(id.index(), id.generation())
    }

    // ========== By-Variable Queries ==========

    /// Get all coefficients for a variable.
    pub fn for_var(&self, var: VarId) -> impl Iterator<Item = CoeffId> + '_ {
        self.by_var.get(&var).into_iter().flatten().copied()
    }

    /// Check if a variable has any coefficients.
    pub fn var_has_coefficients(&self, var: VarId) -> bool {
        self.by_var.get(&var).is_some_and(|s| !s.is_empty())
    }

    // ========== By-Constraint Queries ==========

    /// Get all coefficients for a constraint.
    pub fn for_constraint(&self, con: ConId) -> impl Iterator<Item = CoeffId> + '_ {
        self.by_constraint.get(&con).into_iter().flatten().copied()
    }

    /// Check if a constraint has any coefficients.
    pub fn constraint_has_coefficients(&self, con: ConId) -> bool {
        self.by_constraint.get(&con).is_some_and(|s| !s.is_empty())
    }

    // ========== By-Objective Queries ==========

    /// Get all coefficients for an objective.
    pub fn for_objective(&self, obj: ObjId) -> impl Iterator<Item = CoeffId> + '_ {
        self.by_objective.get(&obj).into_iter().flatten().copied()
    }

    /// Check if an objective has any coefficients.
    pub fn objective_has_coefficients(&self, obj: ObjId) -> bool {
        self.by_objective.get(&obj).is_some_and(|s| !s.is_empty())
    }

    // ========== By-Parameter Queries (Dependency Graph) ==========

    /// Get all coefficients that depend on a parameter.
    ///
    /// This is the dependency graph used for parameter propagation.
    pub fn for_param(&self, param: ParamId) -> impl Iterator<Item = CoeffId> + '_ {
        self.by_param.get(&param).into_iter().flatten().copied()
    }

    /// Check if a parameter has any dependent coefficients.
    pub fn param_has_dependents(&self, param: ParamId) -> bool {
        self.by_param.get(&param).is_some_and(|s| !s.is_empty())
    }

    /// Get the count of coefficients depending on a parameter.
    pub fn param_dependent_count(&self, param: ParamId) -> usize {
        self.by_param.get(&param).map_or(0, |s| s.len())
    }

    // ========== General Queries ==========

    /// Get the total number of coefficients.
    pub fn len(&self) -> usize {
        self.arena.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    /// Iterate over all coefficients.
    pub fn iter(&self) -> impl Iterator<Item = (CoeffId, &CoefficientData)> {
        self.arena
            .iter()
            .map(|(idx, gen, data)| (CoeffId::new(idx, gen), data))
    }

    /// Iterate mutably over all coefficients.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (CoeffId, &mut CoefficientData)> {
        self.arena
            .iter_mut()
            .map(|(idx, gen, data)| (CoeffId::new(idx, gen), data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Generation;

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
        let con = make_con(0);

        let id1 = index.add(
            var1,
            CoefficientTarget::Constraint(con),
            ValueExpr::constant(1.0),
            1.0,
        );
        let id2 = index.add(
            var1,
            CoefficientTarget::Constraint(con),
            ValueExpr::constant(2.0),
            2.0,
        );
        let _id3 = index.add(
            var2,
            CoefficientTarget::Constraint(con),
            ValueExpr::constant(3.0),
            3.0,
        );

        let var1_coeffs: HashSet<_> = index.for_var(var1).collect();
        assert_eq!(var1_coeffs.len(), 2);
        assert!(var1_coeffs.contains(&id1));
        assert!(var1_coeffs.contains(&id2));
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
    fn by_param_index() {
        let mut index = CoefficientIndex::new();
        let var = make_var(0);
        let con = make_con(0);
        let p1 = make_param(0);
        let p2 = make_param(1);

        // Coefficient depending on p1
        let id1 = index.add(
            var,
            CoefficientTarget::Constraint(con),
            ValueExpr::param(p1),
            1.0,
        );

        // Coefficient depending on both p1 and p2
        let id2 = index.add(
            var,
            CoefficientTarget::Constraint(con),
            ValueExpr::mul(ValueExpr::param(p1), ValueExpr::param(p2)),
            2.0,
        );

        // Constant coefficient (no dependencies)
        let _id3 = index.add(
            var,
            CoefficientTarget::Constraint(con),
            ValueExpr::constant(3.0),
            3.0,
        );

        // p1 should have both id1 and id2
        let p1_coeffs: HashSet<_> = index.for_param(p1).collect();
        assert_eq!(p1_coeffs.len(), 2);
        assert!(p1_coeffs.contains(&id1));
        assert!(p1_coeffs.contains(&id2));

        // p2 should only have id2
        let p2_coeffs: Vec<_> = index.for_param(p2).collect();
        assert_eq!(p2_coeffs, vec![id2]);
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
