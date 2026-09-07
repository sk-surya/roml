"""ROML Python interface (MPY): typed MILP modeling over a persistent session."""

from . import _native

sum = _native.sum
dot = _native.dot

Model = _native.Model
Var = _native.Var
Param = _native.Param
Constraint = _native.Constraint
Objective = _native.Objective
Expr = _native.Expr
Comparison = _native.Comparison
ComparisonArray = _native.ComparisonArray
VarArray = _native.VarArray
ParamArray = _native.ParamArray
ExprArray = _native.ExprArray
ConstraintArray = _native.ConstraintArray
Highs = _native.Highs
Solution = _native.Solution
SolveStatus = _native.SolveStatus

RomlError = _native.RomlError
InvalidModelError = _native.InvalidModelError
InvalidHandleError = _native.InvalidHandleError
ModelMismatchError = _native.ModelMismatchError
ShapeError = _native.ShapeError
UnsupportedExpressionError = _native.UnsupportedExpressionError
UnsupportedFeatureError = _native.UnsupportedFeatureError
ModelBusyError = _native.ModelBusyError
SessionBusyError = _native.SessionBusyError
ClosedSessionError = _native.ClosedSessionError
SolverError = _native.SolverError
NoSolutionError = _native.NoSolutionError
MissingValueError = _native.MissingValueError
UnavailableDiagnosticError = _native.UnavailableDiagnosticError

__version__ = _native.version()

__all__ = [
    "__version__",
    "Model",
    "Var",
    "Param",
    "Constraint",
    "Objective",
    "Expr",
    "Comparison",
    "ComparisonArray",
    "VarArray",
    "ParamArray",
    "ExprArray",
    "ConstraintArray",
    "Highs",
    "Solution",
    "SolveStatus",
    "sum",
    "dot",
    "RomlError",
    "InvalidModelError",
    "InvalidHandleError",
    "ModelMismatchError",
    "ShapeError",
    "UnsupportedExpressionError",
    "UnsupportedFeatureError",
    "ModelBusyError",
    "SessionBusyError",
    "ClosedSessionError",
    "SolverError",
    "NoSolutionError",
    "MissingValueError",
    "UnavailableDiagnosticError",
]
