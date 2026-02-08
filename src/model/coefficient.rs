//! Coefficient storage with multi-indexing.
//! 
//! Coefficients are first-class objects linking variables to targets (constraints or objectives).
//! They support efficient lookup by:
//! - Variable (for deletion, solver projection)
//! - Constraint (for deletion, iteration)
//! - Objective (for deletion, iteration)
//! - Parameter (for value propagation)
//! Key idea is the use of expr from which value can be evaluated.

use std::collections::{HashMap, HashSet};

use crate::id::{CoeffId, ConId, IdArena, ObjId, ParamId, VarId};
use crate::{value_expr::ValueExpr};

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
    pub fn new(var: VarId, target: CoefficientTarget, value_expr: ValueExpr, initial_value: f64) -> Self {
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
