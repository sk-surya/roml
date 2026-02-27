



#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverStatus {
    Optimal,
    Infeasible,
    Unbounded,
    Unknown,
}

