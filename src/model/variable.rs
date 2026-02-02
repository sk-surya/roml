

/// Internal data for a variable.
#[derive(Clone, Debug)]
pub struct VariableData {
    /// Variable bounds.
    pub bounds: Bounds,
    /// Variable type.
    pub var_type: VarType,
    /// Whether this variable is active in the model.
    pub active: bool,
    /// Optional name for debugging/printing.
    pub name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct VariableStore {
    variables: HashMap<VarId, Variable>,
}