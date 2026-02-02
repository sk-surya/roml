

#[derive(Clone, Debug)]
pub struct Variable {
    /// Unique identifier for the variable.
    pub id: VarId,
    /// Lower bound of the variable.
    pub lower_bound: f64,
    /// Upper bound of the variable.
    pub upper_bound: f64,
    /// Type of the variable (e.g., continuous, integer, binary).
    pub var_type: VarType,
}

#[derive(Clone, Debug)]
pub struct VariableStore {
    variables: HashMap<VarId, Variable>,
}