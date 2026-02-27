



#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SolverStatus {
    #[default]
    NotSolved,
    Optimal,
    Infeasible,
    Unbounded,
    TimeLimit,
    IterationLimit,
    MemoryLimit,
    Error,
}

