//! Bridge contract and finalizer (design §8.5; D11, D12; SM-13.5, SM-02.5).
//!
//! Every bridge produces compiled variables/rows, mandatory construct
//! origins, a representation kind, captured dependencies, bound/Big-M
//! evidence, and report entries (design §8.5). [`BridgeFinalizer`] is the
//! shared framework the P32 Task 16 bridge modules are written against: it
//! allocates dense compiled ids in the bridge's fixed per-role call sequence
//! (deterministic generated order), records `EntityOrigin::Construct {
//! construct, role }` for every generated entity (D5, SM-02.5), captures
//! dependencies, and appends bound-evidence [`FormulationDecision`] report
//! entries recording M values, derivations, and bound sources (SM-13.5).
//! Exact representations that cannot be produced surface as a typed
//! [`crate::compiler::CompileError`] (design §19) — a bridge never silently
//! relaxes.

pub(crate) mod absolute;
pub(crate) mod boolean;
pub(crate) mod cardinality;
pub(crate) mod indicator;
pub(crate) mod minmax;
pub(crate) mod piecewise_linear;
pub(crate) mod product;
pub(crate) mod reification;
pub(crate) mod soft_constraint;

use std::collections::HashMap;

use crate::compiler::backend_ir::{
    CompiledConstraintId, CompiledLinearRow, CompiledVariable, CompiledVariableId,
};
use crate::compiler::bounds::BoundSource;
use crate::compiler::capability::{BackendCapabilitySet, BackendFeature, CompilationPolicy};
use crate::compiler::origin::{EntityOrigin, GeneratedRole, OriginMap};
use crate::compiler::report::FormulationDecision;
use crate::compiler::CompileError;
use crate::construct::Construct;
use crate::expr::TermCoeff;
use crate::function::ScalarFunction;
use crate::id::{ParamId, VarId};
use crate::model::{Bounds, ConstraintBounds, VarType};
use crate::snapshot::ModelSnapshot;
use crate::value_expr::ValueExpr;

/// The representation kind a bridge produced (design §8.5).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BridgeRepresentation {
    /// Exact linear rows only (no generated auxiliary variables).
    LinearRows,
    /// Exact linear rows plus generated auxiliary variables.
    LinearRowsWithAuxiliaryVariables,
}

/// A dependency the generated bridge formulation has on canonical state
/// (design §8.5 "parameter/domain dependencies").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BridgeDependency {
    /// The generated entities originate from this construct.
    Construct(Construct),
    /// The generated entities reference a user variable's domain.
    Variable(VarId),
    /// The generated entities depend on a parameter value.
    Parameter(ParamId),
}

/// Accumulates one bridge's generated entities and finalizes them into a
/// [`BridgeOutput`] (design §8.5).
///
/// Dense compiled ids are allocated in the order entities are added — the
/// bridge's fixed per-role sequence — and every generated variable/row is
/// recorded with `EntityOrigin::Construct { construct, role }` (D5, SM-02.5).
/// Dependencies and bound-evidence report entries are captured as recorded.
/// The session merges the finished [`BridgeOutput`] into the compiled
/// snapshot; `BridgeFinalizer` itself never silently relaxes an exact
/// representation (a bridge that cannot produce one returns a typed
/// [`crate::compiler::CompileError`] before reaching the finalizer).
pub struct BridgeFinalizer {
    construct: Construct,
    origin_map: OriginMap,
    variables: Vec<CompiledVariable>,
    rows: Vec<CompiledLinearRow>,
    dependencies: Vec<BridgeDependency>,
    decisions: Vec<FormulationDecision>,
    next_variable_index: u32,
    next_row_index: u32,
}

impl BridgeFinalizer {
    /// Begin finalizing entities for `construct`, continuing the session's
    /// dense compiled-id allocation at `next_variable_index` /
    /// `next_row_index`.
    pub fn new(construct: Construct, next_variable_index: u32, next_row_index: u32) -> Self {
        Self {
            construct,
            origin_map: OriginMap::new(),
            variables: Vec::new(),
            rows: Vec::new(),
            dependencies: Vec::new(),
            decisions: Vec::new(),
            next_variable_index,
            next_row_index,
        }
    }

    /// Add a generated auxiliary variable with the given role, type, bounds,
    /// and optional name. Returns its dense compiled id.
    pub fn add_variable(
        &mut self,
        role: GeneratedRole,
        var_type: VarType,
        bounds: Bounds,
        name: Option<String>,
    ) -> CompiledVariableId {
        let id = CompiledVariableId(self.next_variable_index);
        self.next_variable_index += 1;
        self.origin_map.insert_variable(
            id,
            EntityOrigin::Construct {
                construct: self.construct,
                role,
            },
        );
        self.variables.push(CompiledVariable {
            id,
            bounds,
            var_type,
            name,
        });
        id
    }

    /// Add a generated linear row with the given role, bounds, coefficients,
    /// and optional name. Returns its dense compiled id.
    ///
    /// Coefficients reference compiled variable ids already allocated by this
    /// finalizer (auxiliary variables) or by the session (user variables).
    pub fn add_row(
        &mut self,
        role: GeneratedRole,
        bounds: ConstraintBounds,
        coefficients: Vec<(CompiledVariableId, f64)>,
        name: Option<String>,
    ) -> CompiledConstraintId {
        let id = CompiledConstraintId(self.next_row_index);
        self.next_row_index += 1;
        self.origin_map.insert_constraint(
            id,
            EntityOrigin::Construct {
                construct: self.construct,
                role,
            },
        );
        self.rows.push(CompiledLinearRow {
            id,
            bounds,
            coefficients,
            name,
        });
        id
    }

    /// Capture a dependency of the generated formulation (design §8.5).
    pub fn add_dependency(&mut self, dependency: BridgeDependency) {
        self.dependencies.push(dependency);
    }

    /// Record a bound/Big-M evidence report entry (SM-13.5): the selected M
    /// value (or unboundedness), the derivation, and the bound sources.
    pub fn record_bound_evidence(
        &mut self,
        key: &str,
        m_value: Option<f64>,
        derivation: &str,
        bound_sources: &[BoundSource],
    ) {
        let sources: Vec<String> = bound_sources.iter().map(|s| format!("{s:?}")).collect();
        self.decisions.push(FormulationDecision::bound_evidence(
            key, m_value, derivation, &sources,
        ));
    }

    /// Append an arbitrary formulation decision (e.g. a representation
    /// selection).
    pub fn add_decision(&mut self, decision: FormulationDecision) {
        self.decisions.push(decision);
    }

    /// Finalize the accumulated entities into a [`BridgeOutput`].
    pub fn finish(self) -> BridgeOutput {
        let representation = if self.variables.is_empty() {
            BridgeRepresentation::LinearRows
        } else {
            BridgeRepresentation::LinearRowsWithAuxiliaryVariables
        };
        BridgeOutput {
            construct: self.construct,
            representation,
            variables: self.variables,
            rows: self.rows,
            origin_map: self.origin_map,
            dependencies: self.dependencies,
            decisions: self.decisions,
        }
    }
}

/// The complete output of one bridge finalization (design §8.5).
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeOutput {
    /// The originating construct.
    pub construct: Construct,
    /// What representation the bridge produced.
    pub representation: BridgeRepresentation,
    /// Generated compiled variables (dense ids in insertion order).
    pub variables: Vec<CompiledVariable>,
    /// Generated compiled rows (dense ids in insertion order).
    pub rows: Vec<CompiledLinearRow>,
    /// Complete origins for every generated entity (`EntityOrigin::Construct`,
    /// D5/SM-02.5).
    pub origin_map: OriginMap,
    /// Captured dependencies of the generated formulation.
    pub dependencies: Vec<BridgeDependency>,
    /// Bound/Big-M evidence and formulation decisions (SM-13.5).
    pub decisions: Vec<FormulationDecision>,
}

// ===========================================================================
// Shared bridge context and helpers (P32 Task 16)
// ===========================================================================

/// The shared context a P32 construct bridge needs to compile one construct.
///
/// Carries the originating construct, the canonical snapshot (for declared
/// bounds / parameter values via [`BoundAnalyzer`](crate::compiler::bounds::BoundAnalyzer)),
/// the user→compiled variable map, the evaluated parameter-value map, and the
/// effective compilation policy (global narrowed by the per-construct
/// preference) plus the backend capability set (for native/bridge selection).
pub(crate) struct BridgeContext<'a> {
    /// The originating construct.
    pub construct: Construct,
    /// The canonical snapshot being compiled.
    pub snapshot: &'a ModelSnapshot,
    /// User variable → compiled variable id.
    pub variable_ids: &'a HashMap<VarId, CompiledVariableId>,
    /// Evaluated parameter values.
    pub parameter_values: &'a HashMap<ParamId, f64>,
    /// The effective compilation policy (global narrowed by per-construct
    /// preference).
    pub policy: &'a CompilationPolicy,
    /// The backend's typed capability set.
    pub capabilities: &'a BackendCapabilitySet,
}

/// Whether a construct should use a qualified native primitive or the exact
/// portable bridge (design §8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConstructPath {
    /// A qualified exact native primitive.
    Native,
    /// The exact portable ROML bridge.
    Bridge,
}

/// Select the representation path for a construct feature under the effective
/// policy (design §8.1).
///
/// `Auto` prefers a qualified native primitive, then an exact bridge, then a
/// typed `UnsupportedFeature`. `Portable` forces the bridge. `NativeRequired`
/// rejects a non-native feature. An unqualified feature is never silently
/// ignored (SM-04.4).
///
/// # F4 — no native payload in P32
///
/// Until the backend IR carries a real native payload
/// ([`BackendConstraint`](crate::compiler::backend_ir::BackendConstraint) is
/// empty), these construct features are reported/selected ONLY as Bridge:
/// a backend's native declaration does NOT make the feature selectable as
/// `Native` (there is no native representation to emit), and `NativeRequired`
/// rejects the bridge-only path as a typed error. Under `Auto` a native
/// declaration falls back to the exact bridge (the bridge is the only
/// representable path), so bridge selection is available when either a bridge
/// or a (currently unrepresentable) native declaration exists.
pub(crate) fn select_path(
    capabilities: &BackendCapabilitySet,
    policy: &CompilationPolicy,
    feature: BackendFeature,
    context: &str,
) -> Result<ConstructPath, CompileError> {
    let native_available =
        crate::compiler::backend_ir::native_payloads_available() && capabilities.supports(feature);
    let bridge_available = capabilities.is_bridge(feature) || capabilities.supports(feature);
    match policy {
        CompilationPolicy::Auto => {
            if native_available {
                Ok(ConstructPath::Native)
            } else if bridge_available {
                Ok(ConstructPath::Bridge)
            } else {
                Err(CompileError::UnsupportedFeature(format!(
                    "{feature:?} has neither qualified native support nor an exact ROML bridge \
                     ({context})"
                )))
            }
        }
        CompilationPolicy::Portable => {
            if bridge_available {
                Ok(ConstructPath::Bridge)
            } else {
                Err(CompileError::UnsupportedFeature(format!(
                    "{feature:?} requires an exact ROML bridge which this backend does not \
                     declare ({context}; Portable policy)"
                )))
            }
        }
        CompilationPolicy::NativeRequired => {
            if native_available {
                Ok(ConstructPath::Native)
            } else {
                Err(CompileError::UnsupportedFeature(format!(
                    "{feature:?} requires exact native support which this backend lacks \
                     ({context}; NativeRequired policy)"
                )))
            }
        }
    }
}

/// Resolve a user variable to its compiled id, or a typed
/// `MissingConstructReference` when the construct references a variable absent
/// from the compiled snapshot (design §19) — never a silently dropped
/// coefficient.
pub(crate) fn resolve_variable(
    variable_ids: &HashMap<VarId, CompiledVariableId>,
    var: VarId,
    construct: Construct,
) -> Result<CompiledVariableId, CompileError> {
    variable_ids
        .get(&var)
        .copied()
        .ok_or(CompileError::MissingConstructReference {
            construct,
            variable: var,
        })
}

/// Convert a linear scalar function into compiled (id, coefficient) pairs plus
/// the constant term. Terms are processed in sorted variable order
/// (determinism, matching `BoundAnalyzer`).
pub(crate) fn function_coefficients(
    function: &ScalarFunction,
    construct: Construct,
    variable_ids: &HashMap<VarId, CompiledVariableId>,
    parameter_values: &HashMap<ParamId, f64>,
) -> Result<(Vec<(CompiledVariableId, f64)>, f64), CompileError> {
    match function {
        ScalarFunction::Linear(expr) => {
            let mut coefficients = Vec::new();
            let mut terms: Vec<_> = expr.terms.iter().collect();
            terms.sort_by_key(|t| t.var);
            for term in terms {
                let value = match &term.coeff {
                    TermCoeff::Constant(v) => *v,
                    // F5: a coefficient referencing a parameter absent from the
                    // evaluated map is a typed `MissingConstructParameter` —
                    // never a silent default of zero.
                    TermCoeff::Expr(e) => e
                        .eval_checked(|p| parameter_values.get(&p).copied().ok_or(p))
                        .map_err(|parameter| CompileError::MissingConstructParameter {
                            construct,
                            parameter,
                        })?,
                };
                let vid = resolve_variable(variable_ids, term.var, construct)?;
                coefficients.push((vid, value));
            }
            Ok((coefficients, expr.constant))
        }
    }
}

/// Evaluate a scalar-set bound (`ValueExpr`) against the evaluated parameter
/// values, surfacing a missing parameter as a typed error (F5) — never a
/// silent default of zero.
pub(crate) fn eval_bound(
    bound: &ValueExpr,
    construct: Construct,
    parameter_values: &HashMap<ParamId, f64>,
) -> Result<f64, CompileError> {
    bound
        .eval_checked(|p| parameter_values.get(&p).copied().ok_or(p))
        .map_err(|parameter| CompileError::MissingConstructParameter {
            construct,
            parameter,
        })
}

/// Combine the function's compiled coefficients with a single-var term
/// (e.g. `M·activator`), merging if the variable already appears.
pub(crate) fn combine_coefficients(
    coefficients: Vec<(CompiledVariableId, f64)>,
    extra: (CompiledVariableId, f64),
) -> Vec<(CompiledVariableId, f64)> {
    let mut out = coefficients;
    let (extra_id, extra_value) = extra;
    if let Some(slot) = out.iter_mut().find(|(id, _)| *id == extra_id) {
        slot.1 += extra_value;
    } else {
        out.push((extra_id, extra_value));
    }
    out.sort_by_key(|(id, _)| *id);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::bounds::BoundSource;
    use crate::compiler::origin::{EntityOrigin, GeneratedRole};
    use crate::id::Generation;
    use crate::identity::ConstructId;

    fn construct() -> Construct {
        ConstructId::allocate().expect("construct id allocation")
    }

    fn var(index: u32) -> VarId {
        VarId::new(index, Generation::new())
    }

    /// Generated variables/rows get dense compiled ids in call order, so a
    /// per-construct, per-role sequence is preserved.
    #[test]
    fn finalizer_allocates_dense_ids_in_call_order() {
        let c = construct();
        let mut finalizer = BridgeFinalizer::new(c, 0, 0);
        let v0 =
            finalizer.add_variable(GeneratedRole::Bridge, VarType::Binary, Bounds::BINARY, None);
        let v1 =
            finalizer.add_variable(GeneratedRole::Bridge, VarType::Binary, Bounds::BINARY, None);
        let r0 = finalizer.add_row(
            GeneratedRole::Bridge,
            ConstraintBounds::le(10.0),
            vec![(v0, 1.0), (v1, -1.0)],
            None,
        );
        let output = finalizer.finish();

        assert_eq!(v0, CompiledVariableId(0));
        assert_eq!(v1, CompiledVariableId(1));
        assert_eq!(r0, CompiledConstraintId(0));
        assert_eq!(output.variables.len(), 2);
        assert_eq!(output.rows.len(), 1);
        // Dense ids, distinct families.
        assert!(output
            .variables
            .iter()
            .all(|v| v.id == CompiledVariableId(0) || v.id == CompiledVariableId(1)));
        assert_eq!(output.rows[0].id, CompiledConstraintId(0));
    }

    /// Every generated entity carries `EntityOrigin::Construct { construct,
    /// role }` — no generated entity is finalized without an origin (D5,
    /// SM-02.5).
    #[test]
    fn finalizer_records_construct_origins_for_every_generated_entity() {
        let c = construct();
        let mut finalizer = BridgeFinalizer::new(c, 0, 0);
        let v0 =
            finalizer.add_variable(GeneratedRole::Bridge, VarType::Binary, Bounds::BINARY, None);
        let r0 = finalizer.add_row(
            GeneratedRole::Bridge,
            ConstraintBounds::le(10.0),
            vec![(v0, 1.0)],
            None,
        );
        let output = finalizer.finish();

        assert_eq!(
            output.origin_map.variable_origin(v0),
            Some(&EntityOrigin::Construct {
                construct: c,
                role: GeneratedRole::Bridge
            })
        );
        assert_eq!(
            output.origin_map.constraint_origin(r0),
            Some(&EntityOrigin::Construct {
                construct: c,
                role: GeneratedRole::Bridge
            })
        );
        // Completeness validator finds no missing origins.
        assert!(output
            .origin_map
            .missing_origins(&output.variables, &output.rows, &[])
            .is_empty());
    }

    /// The finalizer captures declared dependencies.
    #[test]
    fn finalizer_captures_dependencies() {
        let c = construct();
        let x = var(7);
        let mut finalizer = BridgeFinalizer::new(c, 0, 0);
        finalizer.add_dependency(BridgeDependency::Construct(c));
        finalizer.add_dependency(BridgeDependency::Variable(x));
        let output = finalizer.finish();

        assert!(output
            .dependencies
            .contains(&BridgeDependency::Construct(c)));
        assert!(output.dependencies.contains(&BridgeDependency::Variable(x)));
    }

    /// Bound-evidence report entries record M values, derivations, and bound
    /// sources (SM-13.5).
    #[test]
    fn finalizer_records_bound_evidence_report_entries() {
        let c = construct();
        let x = var(0);
        let mut finalizer = BridgeFinalizer::new(c, 0, 0);
        finalizer.record_bound_evidence(
            "indicator.big_m",
            Some(42.0),
            "max 2x over x in [0, 10] minus rhs 5",
            &[BoundSource::DeclaredVariableBounds(x)],
        );
        let output = finalizer.finish();

        let entry = output
            .decisions
            .iter()
            .find(|d| d.decision == "indicator.big_m")
            .expect("bound-evidence decision must be recorded");
        assert_eq!(entry.selection, "M = 42");
        assert!(entry
            .reason
            .contains("derivation: max 2x over x in [0, 10] minus rhs 5"));
        assert!(entry.reason.contains("DeclaredVariableBounds"));
    }

    /// An unbounded Big-M records the unbounded marker in the report entry —
    /// never a silent default constant (SM-13.5, D12).
    #[test]
    fn finalizer_records_unbounded_evidence_without_a_constant() {
        let c = construct();
        let mut finalizer = BridgeFinalizer::new(c, 0, 0);
        finalizer.record_bound_evidence("indicator.big_m", None, "free variable", &[]);
        let output = finalizer.finish();

        let entry = output
            .decisions
            .iter()
            .find(|d| d.decision == "indicator.big_m")
            .expect("bound-evidence decision must be recorded");
        assert_eq!(entry.selection, "unbounded (no finite Big-M)");
    }

    /// The representation kind reflects whether auxiliary variables were
    /// generated.
    #[test]
    fn finalizer_representation_reflects_generated_variables() {
        let c = construct();
        let mut rows_only = BridgeFinalizer::new(c, 0, 0);
        rows_only.add_row(
            GeneratedRole::Bridge,
            ConstraintBounds::le(10.0),
            vec![],
            None,
        );
        assert_eq!(
            rows_only.finish().representation,
            BridgeRepresentation::LinearRows
        );

        let mut with_aux = BridgeFinalizer::new(c, 0, 0);
        with_aux.add_variable(GeneratedRole::Bridge, VarType::Binary, Bounds::BINARY, None);
        with_aux.add_row(
            GeneratedRole::Bridge,
            ConstraintBounds::le(10.0),
            vec![],
            None,
        );
        assert_eq!(
            with_aux.finish().representation,
            BridgeRepresentation::LinearRowsWithAuxiliaryVariables
        );
    }

    /// Two constructs finalized in construct-id order produce non-overlapping
    /// dense id ranges ordered by construct id (deterministic generated
    /// order).
    #[test]
    fn finalizer_deterministic_order_across_constructs() {
        let c1 = construct();
        let c2 = construct();
        assert!(c1 < c2, "construct ids are issued in increasing order");

        // Finalize c1 first (the session's construct-id order).
        let mut f1 = BridgeFinalizer::new(c1, 0, 0);
        let c1_v = f1.add_variable(GeneratedRole::Bridge, VarType::Binary, Bounds::BINARY, None);
        let c1_r = f1.add_row(
            GeneratedRole::Bridge,
            ConstraintBounds::le(10.0),
            vec![(c1_v, 1.0)],
            None,
        );
        let out1 = f1.finish();

        // Continue dense allocation for c2 after c1's entities.
        let mut f2 = BridgeFinalizer::new(c2, out1.variables.len() as u32, out1.rows.len() as u32);
        let c2_v = f2.add_variable(GeneratedRole::Bridge, VarType::Binary, Bounds::BINARY, None);
        let c2_r = f2.add_row(
            GeneratedRole::Bridge,
            ConstraintBounds::le(10.0),
            vec![(c2_v, 1.0)],
            None,
        );
        let out2 = f2.finish();

        assert_eq!(c1_v, CompiledVariableId(0));
        assert_eq!(c1_r, CompiledConstraintId(0));
        assert_eq!(c2_v, CompiledVariableId(1));
        assert_eq!(c2_r, CompiledConstraintId(1));
        // Origins name the correct construct per entity.
        assert_eq!(
            out1.origin_map.variable_origin(c1_v),
            Some(&EntityOrigin::Construct {
                construct: c1,
                role: GeneratedRole::Bridge
            })
        );
        assert_eq!(
            out2.origin_map.variable_origin(c2_v),
            Some(&EntityOrigin::Construct {
                construct: c2,
                role: GeneratedRole::Bridge
            })
        );
    }
}
