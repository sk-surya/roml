//! Rust expression wrappers with core-backed affine semantics (DESIGN §3).
//!
//! `Expr` is an affine combination of decision variables with
//! parameter-dependent coefficients (`ValueExpr`) plus a parameter-dependent
//! constant. All algebra executes in Rust with validation at construction:
//! variable-times-variable rejects as nonlinear, division admits only
//! nonzero numeric constants, and symbolic truthiness raises `TypeError`.

use pyo3::prelude::*;
use roml::{ParamId, ValueExpr, VarId};
use std::sync::Arc;

use super::errors::{InvalidModelError, UnsupportedExpressionError};
use super::handles::{Param, Var};
use super::model::{py_numeric, Model};

/// One affine term: decision variable times a parameter-dependent coefficient.
#[derive(Clone, Debug)]
pub(crate) struct ExprTerm {
    pub var: VarId,
    pub coeff: ValueExpr,
}

/// Owned affine expression with its model's identity for checks.
#[derive(Debug)]
pub(crate) struct Affine {
    pub owner: Py<Model>,
    pub terms: Vec<ExprTerm>,
    pub constant: ValueExpr,
}

impl Clone for Affine {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            owner: self.owner.clone_ref(py),
            terms: self.terms.clone(),
            constant: self.constant.clone(),
        })
    }
}

/// Canonical flat affine form: the single lowering target.
///
/// Construction NEVER builds this incrementally (that was the O(n²)
/// pathology: clone-per-`+` plus a linear duplicate scan per term).
/// All syntax operations build [`Lazy`] trees in O(1); exactly one
/// [`Scalar::materialize`] at a model sink flattens and canonicalizes.
fn is_finite_expr(expr: &ValueExpr) -> bool {
    match expr.as_constant() {
        Some(v) => v.is_finite(),
        // Parameter-dependent coefficients validate against live values at
        // model insertion; construction only rejects constant non-finite.
        None => true,
    }
}

/// Extract a numeric operand; bools reject, non-numerics reject.
fn operand_numeric(value: &Bound<'_, PyAny>, op: &str) -> PyResult<f64> {
    py_numeric(value, &format!("{op} operand"))
}

/// Fold one emitted leaf through the enclosing scale-factor path.
///
/// `factors` accumulate outermost-first during the top-down walk; the old
/// eager code applied the innermost operation first, so emission folds
/// `Mul` left over `(leaf, reversed factors)`. A single factor reproduces
/// the old `coeff * factor` shape exactly; the empty path is a no-op.
fn apply_factors(leaf: ValueExpr, factors: &[ValueExpr]) -> ValueExpr {
    if factors.is_empty() {
        return leaf;
    }
    let mut acc = leaf;
    for f in factors.iter().rev() {
        acc = ValueExpr::mul(acc, f.clone());
    }
    acc
}

/// Flatten a tree into encounter-ordered raw emissions plus raw constant
/// parts. Iterative over an explicit stack (a million-deep left spine
/// must not recurse); left children pop before right ones so emission
/// order matches the old incremental push order.
fn flatten(root: &ExprNode) -> (Vec<(VarId, ValueExpr)>, Vec<ValueExpr>) {
    let mut terms: Vec<(VarId, ValueExpr)> = Vec::new();
    let mut consts: Vec<ValueExpr> = Vec::new();
    // (node, enclosing factors outermost-first). Cloning the factor vec
    // per Add is allocation-free while the path carries no scales (the
    // common chain shape); scale-nested paths pay per-node vec growth.
    let mut stack: Vec<(&ExprNode, Vec<ValueExpr>)> = vec![(root, Vec::new())];
    while let Some((node, factors)) = stack.pop() {
        match node {
            ExprNode::Term { var, coeff } => {
                terms.push((*var, apply_factors(coeff.clone(), &factors)));
            }
            ExprNode::Const(c) => {
                consts.push(apply_factors(c.clone(), &factors));
            }
            ExprNode::Add(l, r) => {
                stack.push((r, factors.clone()));
                stack.push((l, factors));
            }
            ExprNode::Scale(f, c) => {
                let mut inner = factors;
                inner.push(f.clone());
                stack.push((c, inner));
            }
            ExprNode::Flat(flat) => {
                for t in &flat.terms {
                    terms.push((t.var, apply_factors(t.coeff.clone(), &factors)));
                }
                consts.push(apply_factors(flat.constant.clone(), &factors));
            }
            ExprNode::PackedArray(array) => {
                for t in &array.terms {
                    match &t.coeffs {
                        PackedCoeffs::One => {
                            for v in &t.vars {
                                terms.push((*v, apply_factors(ValueExpr::constant(1.0), &factors)));
                            }
                        }
                        PackedCoeffs::Scalar(c) => {
                            for v in &t.vars {
                                terms.push((*v, apply_factors(ValueExpr::constant(*c), &factors)));
                            }
                        }
                        PackedCoeffs::Dense(values) => {
                            for (v, c) in t.vars.iter().zip(values.iter()) {
                                terms.push((*v, apply_factors(ValueExpr::constant(*c), &factors)));
                            }
                        }
                    }
                }
                if array.constant != 0.0 {
                    consts.push(apply_factors(ValueExpr::constant(array.constant), &factors));
                }
            }
            ExprNode::PackedSym(sym) => {
                for ((var, param), scale) in sym
                    .vars
                    .iter()
                    .zip(sym.params.iter())
                    .zip(sym.scales.iter())
                {
                    terms.push((
                        *var,
                        apply_factors(ValueExpr::scaled_param(*scale, *param), &factors),
                    ));
                }
                consts.push(apply_factors(sym.constant.clone(), &factors));
            }
        }
    }
    (terms, consts)
}

/// Combine raw emissions into canonical terms.
///
/// Every emission is simplified first (mirroring the old per-push
/// simplification, so fresh-variable chains come out structurally
/// identical). An already sorted-and-unique run — the common chain and
/// bulk shapes — skips hashing entirely; otherwise a stable sort plus
/// encounter-ordered run folding reproduces the old incremental combine
/// order. Algebraic zeros are NOT dropped here (the old code kept them;
/// the core canonicalizes downstream).
fn combine_terms(mut terms: Vec<(VarId, ValueExpr)>) -> Vec<ExprTerm> {
    for (_, coeff) in terms.iter_mut() {
        let simp = simplify_value(std::mem::replace(coeff, ValueExpr::constant(0.0)));
        *coeff = simp;
    }
    let sorted_unique = terms.windows(2).all(|w| w[0].0 < w[1].0);
    if sorted_unique {
        return terms
            .into_iter()
            .map(|(var, coeff)| ExprTerm { var, coeff })
            .collect();
    }
    terms.sort_by_key(|(var, _)| *var);
    let mut out: Vec<ExprTerm> = Vec::new();
    for (var, coeff) in terms {
        match out.last_mut() {
            Some(tail) if tail.var == var => {
                let acc = std::mem::replace(&mut tail.coeff, ValueExpr::constant(0.0));
                tail.coeff = simplify_value(ValueExpr::add(acc, coeff));
            }
            _ => out.push(ExprTerm { var, coeff }),
        }
    }
    out
}

/// Canonicalize raw constant parts: left fold with simplification,
/// mirroring the old incremental constant handling.
fn combine_consts(parts: Vec<ValueExpr>) -> ValueExpr {
    let mut acc = ValueExpr::constant(0.0);
    for part in parts {
        acc = simplify_value(ValueExpr::add(acc, part));
    }
    acc
}

impl Lazy {
    /// Lower once: flatten, combine, and return the canonical [`Affine`].
    /// This is the ONLY path from trees to flat terms; every model sink
    /// goes through [`Scalar::materialize`].
    pub(crate) fn flatten_lower(&self, py: Python<'_>) -> Affine {
        let (terms, consts) = flatten(self.root.get());
        Affine {
            owner: self.owner.clone_ref(py),
            terms: combine_terms(terms),
            constant: combine_consts(consts),
        }
    }

    /// Flatten just the constant of a decision-free tree (rare
    /// param-times-variable shapes). Debug-asserts the absence of terms:
    /// callers must have checked `has_vars` first.
    fn const_only(&self) -> ValueExpr {
        let (terms, consts) = flatten(self.root.get());
        debug_assert!(terms.is_empty(), "const_only on decision-bearing tree");
        combine_consts(consts)
    }
}

/// Canonical flat terms shared by reference: the leaf form for already-
/// combined term vectors (dot fallbacks, array folding). Never mutated in
/// place; flattening expands it into the sink's emission buffer.
#[derive(Clone, Debug)]
pub(crate) struct FlatTerms {
    pub terms: Vec<ExprTerm>,
    pub constant: ValueExpr,
}

impl From<Affine> for FlatTerms {
    fn from(a: Affine) -> Self {
        Self {
            terms: a.terms,
            constant: a.constant,
        }
    }
}

/// One node of a persistent scalar expression tree (P1D).
///
/// Trees are immutable and structurally shared (`Arc`): `b = a + z` bumps
/// two refcounts and allocates one node — it never copies or
/// canonicalizes `a`'s terms, and `a` itself is unmodified. Sinks flatten
/// once (see [`flatten`]).
#[derive(Debug)]
pub(crate) enum ExprNode {
    /// One decision-variable term. The coefficient is checked finite (when
    /// constant) at construction, exactly like the old `push_term`.
    Term { var: VarId, coeff: ValueExpr },
    /// Parameter-only or numeric constant part.
    Const(ValueExpr),
    /// Unordered combination; flattening emits left before right.
    Add(Arc<ExprNode>, Arc<ExprNode>),
    /// Parameter-only or numeric factor over a child. The factor can never
    /// carry decision variables (`ValueExpr` cannot name a `VarId`).
    Scale(ValueExpr, Arc<ExprNode>),
    /// Already-combined flat terms (dot/array fallbacks).
    Flat(Arc<FlatTerms>),
    /// Numeric packed array leaf: expands per element at flattening, never
    /// into per-term objects during construction.
    PackedArray(PackedLinearArray),
    /// Scaled-parameter packed leaf: parallel `(variable, parameter,
    /// scale)` vectors plus the (usually zero) constant.
    PackedSym(PackedSymTerms),
}

/// Payload of an [`ExprNode::PackedSym`] leaf (owner lives on [`Lazy`]).
#[derive(Clone, Debug)]
pub(crate) struct PackedSymTerms {
    pub vars: Vec<VarId>,
    pub params: Vec<ParamId>,
    pub scales: Vec<f64>,
    pub constant: ValueExpr,
}

/// Owning handle for iterative teardown (see [`drop_tree`]).
/// Crate-visible because [`Lazy`] exposes it; only [`Lazy`] constructs it.
///
/// A million-deep left-associated tree would overflow the stack if its
/// nested `Arc`s dropped recursively. Only this wrapper ever owns a
/// tree root on the Python side, so exactly one iterative dismantling
/// per tree teardown keeps deallocation O(depth) time and O(1) stack.
#[derive(Debug)]
pub(crate) struct Root(Option<Arc<ExprNode>>);

impl Root {
    fn new(node: Arc<ExprNode>) -> Self {
        Self(Some(node))
    }

    fn get(&self) -> &Arc<ExprNode> {
        self.0.as_ref().expect("live expression root")
    }
}

impl Clone for Root {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        if let Some(root) = self.0.take() {
            drop_tree(root);
        }
    }
}

/// Iteratively dismantle a tree, freeing every uniquely-owned node.
///
/// Shared subtrees (strong count above one) are left alone: the last
/// owner's teardown frees them. Expression trees are acyclic by
/// construction, so every node is visited at most once per teardown.
fn drop_tree(root: Arc<ExprNode>) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        match Arc::try_unwrap(node) {
            Ok(ExprNode::Add(l, r)) => {
                stack.push(l);
                stack.push(r);
            }
            Ok(ExprNode::Scale(_, c)) => stack.push(c),
            // Leaves (Term/Const/Flat/PackedArray/PackedSym) and shared
            // subtrees drop here: leaf payloads are flat allocations, and
            // a shared node is still owned elsewhere.
            Ok(_) | Err(_) => {}
        }
    }
}

/// Persistent general scalar expression: owner plus an immutable tree.
///
/// `has_vars` (any decision variable below) drives construction-time
/// nonlinear rejection exactly like the old `terms.is_empty()` checks;
/// `size` (leaf-term estimate) serves `repr` in O(1). Both are computed
/// from the immediate children — never by traversal.
#[derive(Debug)]
pub(crate) struct Lazy {
    pub owner: Py<Model>,
    pub root: Root,
    pub has_vars: bool,
    pub size: usize,
}

impl Clone for Lazy {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            owner: self.owner.clone_ref(py),
            root: self.root.clone(),
            has_vars: self.has_vars,
            size: self.size,
        })
    }
}

impl Lazy {
    fn wrap(owner: Py<Model>, root: Arc<ExprNode>, has_vars: bool, size: usize) -> Self {
        Self {
            owner,
            root: Root::new(root),
            has_vars,
            size,
        }
    }

    /// Single decision-variable term (mirrors the old `var_affine` leaf,
    /// including its finiteness discipline).
    pub(crate) fn term(owner: Py<Model>, var: VarId, coeff: ValueExpr) -> PyResult<Self> {
        if !is_finite_expr(&coeff) {
            return Err(InvalidModelError::new_err(
                "expression coefficient must be finite",
            ));
        }
        Ok(Self::wrap(
            owner,
            Arc::new(ExprNode::Term { var, coeff }),
            true,
            1,
        ))
    }

    /// Parameter-only or numeric constant leaf.
    pub(crate) fn constant(owner: Py<Model>, constant: ValueExpr) -> Self {
        Self::wrap(owner, Arc::new(ExprNode::Const(constant)), false, 0)
    }

    /// Already-combined flat terms (dot/array fallbacks).
    pub(crate) fn flat(owner: Py<Model>, flat: FlatTerms) -> Self {
        let has_vars = !flat.terms.is_empty();
        let size = flat.terms.len();
        Self::wrap(
            owner,
            Arc::new(ExprNode::Flat(Arc::new(flat))),
            has_vars,
            size,
        )
    }

    fn add_nodes(
        owner: Py<Model>,
        l: &Lazy,
        r: Arc<ExprNode>,
        r_vars: bool,
        r_size: usize,
    ) -> Self {
        let (has_vars, size) = (l.has_vars || r_vars, l.size.saturating_add(r_size));
        let root = Arc::new(ExprNode::Add(l.root.get().clone(), r));
        Self::wrap(owner, root, has_vars, size)
    }

    fn scale_node(owner: Py<Model>, factor: ValueExpr, child: &Lazy) -> Self {
        let root = Arc::new(ExprNode::Scale(factor, child.root.get().clone()));
        Self::wrap(owner, root, child.has_vars, child.size)
    }
}

/// Packed constant-coefficient vector form (P0 bulk path).
///
/// Produced by `rm.sum(VarArray)` and numeric `rm.dot` reductions: it retains
/// the owner plus a structural multi-term array WITHOUT expanding it into
/// per-term `Affine` structures, so a million-term objective crosses into
/// the core as flat buffers. Any arithmetic on a packed value materializes
/// it to a general `Affine` first; only `minimize`/`maximize` consume the
/// packed form directly.
#[derive(Debug)]
pub(crate) struct PackedVars {
    pub owner: Py<Model>,
    pub array: PackedLinearArray,
}

impl Clone for PackedVars {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            owner: self.owner.clone_ref(py),
            array: self.array.clone(),
        })
    }
}

/// Coefficient storage for packed forms; always finite numerics by
/// construction (parameterized coefficients stay on the general path...
/// and on [`Scalar::PackedSymbolic`] in P1C-2).
#[derive(Clone, Debug)]
pub(crate) enum PackedCoeffs {
    /// All-ones (from `rm.sum`).
    One,
    /// Uniform scalar (from `rm.dot(scalar, ...)`).
    Scalar(f64),
    /// Dense per-variable values, C order (from `rm.dot(array, ...)`).
    Dense(Vec<f64>),
}

/// One elementwise term of a packed array: `coeffs[i] * vars[i]` per
/// element, C-order aligned with the array shape.
#[derive(Clone, Debug)]
pub(crate) struct PackedArrayTerm {
    pub vars: Vec<VarId>,
    pub coeffs: PackedCoeffs,
}

/// Structural packed linear array (P1C-1): elementwise terms plus a numeric
/// scalar constant. Coefficients are numeric-only (`One` / `Scalar` /
/// `Dense`); anything parameterized stays on the general `Vec<Affine>`
/// path. Term vectors align elementwise; construction never allocates
/// per-element expression objects.
#[derive(Clone, Debug)]
pub(crate) struct PackedLinearArray {
    pub shape: Vec<usize>,
    pub terms: Vec<PackedArrayTerm>,
    pub constant: f64,
}

impl PackedLinearArray {
    pub(crate) fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Single unit-coefficient term over a variable vector.
    pub(crate) fn from_vars(vars: Vec<VarId>, shape: Vec<usize>) -> Self {
        debug_assert_eq!(vars.len(), shape.iter().product::<usize>());
        Self {
            shape,
            terms: vec![PackedArrayTerm {
                vars,
                coeffs: PackedCoeffs::One,
            }],
            constant: 0.0,
        }
    }

    /// Scale every coefficient and the constant by a finite factor.
    pub(crate) fn scale(&mut self, factor: f64) {
        for term in &mut self.terms {
            term.coeffs = match std::mem::replace(&mut term.coeffs, PackedCoeffs::One) {
                PackedCoeffs::One => PackedCoeffs::Scalar(factor),
                PackedCoeffs::Scalar(c) => PackedCoeffs::Scalar(c * factor),
                PackedCoeffs::Dense(v) => {
                    PackedCoeffs::Dense(v.into_iter().map(|x| x * factor).collect())
                }
            };
        }
        self.constant *= factor;
    }

    /// Elementwise dense scaling. Requires a zero scalar constant (a dense
    /// factor would otherwise densify it); callers fall back to the general
    /// path when the constant is nonzero.
    pub(crate) fn scale_dense(&mut self, s: &[f64]) {
        debug_assert_eq!(s.len(), self.numel());
        debug_assert_eq!(self.constant, 0.0);
        for term in &mut self.terms {
            term.coeffs = match std::mem::replace(&mut term.coeffs, PackedCoeffs::One) {
                PackedCoeffs::One => PackedCoeffs::Dense(s.to_vec()),
                PackedCoeffs::Scalar(c) => PackedCoeffs::Dense(s.iter().map(|x| x * c).collect()),
                PackedCoeffs::Dense(v) => {
                    PackedCoeffs::Dense(v.into_iter().zip(s.iter()).map(|(a, b)| a * b).collect())
                }
            };
        }
    }

    /// Add a scalar to the constant.
    pub(crate) fn add_scalar(&mut self, v: f64) {
        self.constant += v;
    }

    /// Add (`sign` +1) or subtract (−1) another same-shape packed array.
    pub(crate) fn combine(&mut self, other: &PackedLinearArray, sign: f64) {
        debug_assert_eq!(self.shape, other.shape);
        for term in &other.terms {
            let coeffs = match &term.coeffs {
                PackedCoeffs::One => PackedCoeffs::Scalar(sign),
                PackedCoeffs::Scalar(c) => PackedCoeffs::Scalar(c * sign),
                PackedCoeffs::Dense(v) => PackedCoeffs::Dense(v.iter().map(|x| x * sign).collect()),
            };
            self.terms.push(PackedArrayTerm {
                vars: term.vars.clone(),
                coeffs,
            });
        }
        self.constant += sign * other.constant;
    }

    /// Sub-select C-order flat positions into a new shape.
    pub(crate) fn select(&self, flat: &[usize], shape: Vec<usize>) -> Self {
        debug_assert_eq!(flat.len(), shape.iter().product::<usize>());
        let terms = self
            .terms
            .iter()
            .map(|t| {
                let vars = flat.iter().map(|&f| t.vars[f]).collect();
                let coeffs = match &t.coeffs {
                    PackedCoeffs::One => PackedCoeffs::One,
                    PackedCoeffs::Scalar(c) => PackedCoeffs::Scalar(*c),
                    PackedCoeffs::Dense(v) => {
                        PackedCoeffs::Dense(flat.iter().map(|&f| v[f]).collect())
                    }
                };
                PackedArrayTerm { vars, coeffs }
            })
            .collect();
        Self {
            shape,
            terms,
            constant: self.constant,
        }
    }

    /// Expand to per-element affines (fallback/materialization boundary).
    pub(crate) fn materialize(&self, py: Python<'_>, owner: &pyo3::Py<Model>) -> Vec<Affine> {
        let n = self.numel();
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let mut terms = Vec::with_capacity(self.terms.len());
            for t in &self.terms {
                let c = match &t.coeffs {
                    PackedCoeffs::One => 1.0,
                    PackedCoeffs::Scalar(c) => *c,
                    PackedCoeffs::Dense(v) => v[i],
                };
                terms.push(ExprTerm {
                    var: t.vars[i],
                    coeff: ValueExpr::constant(c),
                });
            }
            out.push(Affine {
                owner: owner.clone_ref(py),
                terms,
                constant: ValueExpr::constant(self.constant),
            });
        }
        out
    }
}

/// A scalar objective/expression form: a general lazy tree, a packed
/// constant-coefficient array, or a packed scaled-parameter vector.
///
/// `Lazy` is the only general form: syntax operations extend the tree in
/// O(1) without copying or canonicalizing terms (P1D). Bulk leaves stay
/// matched by `minimize`/`maximize`; every other sink flattens once via
/// [`Scalar::materialize`].
#[derive(Debug)]
pub(crate) enum Scalar {
    Lazy(Lazy),
    Packed(PackedVars),
    PackedSymbolic(PackedSymbolic),
}

/// Packed scaled-parameter vector form (P1C-2).
///
/// Produced by `rm.dot` with structurally cheap parameter-only
/// coefficients (`ParamArray`, broadcast scalar `Param`, trivially
/// representable parameter scalar expressions) over a packed numeric
/// decision array: it retains parallel `(variable, scale, parameter)`
/// vectors WITHOUT expanding them into per-term `Affine` structures, so a
/// 57.6k-term parameterized objective crosses into the core as three flat
/// buffers via `set_linear_objective_param_bulk`. Any arithmetic on a
/// packed-symbolic value materializes it to a general `Affine` first (with
/// coefficient forms bit-identical to the scalar fold); only
/// `minimize`/`maximize` consume the packed form directly.
#[derive(Debug)]
pub(crate) struct PackedSymbolic {
    pub owner: Py<Model>,
    pub vars: Vec<VarId>,
    pub params: Vec<ParamId>,
    pub scales: Vec<f64>,
    pub constant: ValueExpr,
}

impl Clone for PackedSymbolic {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            owner: self.owner.clone_ref(py),
            vars: self.vars.clone(),
            params: self.params.clone(),
            scales: self.scales.clone(),
            constant: self.constant.clone(),
        })
    }
}

impl Clone for Scalar {
    fn clone(&self) -> Self {
        match self {
            // Arc bump plus one owner ref: O(1), never a term copy.
            Self::Lazy(l) => Self::Lazy(l.clone()),
            Self::Packed(p) => Python::attach(|py| {
                Self::Packed(PackedVars {
                    owner: p.owner.clone_ref(py),
                    array: p.array.clone(),
                })
            }),
            Self::PackedSymbolic(s) => Self::PackedSymbolic(s.clone()),
        }
    }
}

impl Scalar {
    /// Owner reference without materializing.
    pub(crate) fn owner_ref(&self, py: Python<'_>) -> Py<Model> {
        match self {
            Self::Lazy(l) => l.owner.clone_ref(py),
            Self::Packed(p) => p.owner.clone_ref(py),
            Self::PackedSymbolic(s) => s.owner.clone_ref(py),
        }
    }

    /// General `Affine` view: lazy trees flatten and combine once here;
    /// packed forms expand term-by-term as before. Only non-bulk consumers
    /// pay this; `minimize`/`maximize` match on [`Scalar::Packed`] and
    /// [`Scalar::PackedSymbolic`] directly.
    pub(crate) fn materialize(&self, py: Python<'_>) -> Affine {
        match self {
            Self::Lazy(l) => l.flatten_lower(py),
            Self::PackedSymbolic(s) => Affine {
                owner: s.owner.clone_ref(py),
                terms: s
                    .vars
                    .iter()
                    .zip(s.params.iter())
                    .zip(s.scales.iter())
                    .map(|((var, param), scale)| ExprTerm {
                        var: *var,
                        coeff: ValueExpr::scaled_param(*scale, *param),
                    })
                    .collect(),
                constant: s.constant.clone(),
            },
            Self::Packed(p) => {
                let array = &p.array;
                let mut terms = Vec::with_capacity(array.terms.iter().map(|t| t.vars.len()).sum());
                for t in &array.terms {
                    match &t.coeffs {
                        PackedCoeffs::One => {
                            for v in &t.vars {
                                terms.push(ExprTerm {
                                    var: *v,
                                    coeff: ValueExpr::constant(1.0),
                                });
                            }
                        }
                        PackedCoeffs::Scalar(c) => {
                            for v in &t.vars {
                                terms.push(ExprTerm {
                                    var: *v,
                                    coeff: ValueExpr::constant(*c),
                                });
                            }
                        }
                        PackedCoeffs::Dense(values) => {
                            for (v, c) in t.vars.iter().zip(values.iter()) {
                                terms.push(ExprTerm {
                                    var: *v,
                                    coeff: ValueExpr::constant(*c),
                                });
                            }
                        }
                    }
                }
                Affine {
                    owner: p.owner.clone_ref(py),
                    terms,
                    constant: ValueExpr::constant(array.constant),
                }
            }
        }
    }
}

/// Lift one operand of `+`/`-` to a lazy leaf, checking ownership.
/// Numerics become constants; anything else is the caller's error.
fn lift_add_operand(py: Python<'_>, owner: &Py<Model>, other: &Bound<'_, PyAny>) -> PyResult<Lazy> {
    if let Ok(var) = other.cast::<Var>() {
        let var = var.borrow();
        same_owner(&var.owner, owner)?;
        return Lazy::term(owner.clone_ref(py), var.id, ValueExpr::constant(1.0));
    }
    if let Ok(param) = other.cast::<Param>() {
        let param = param.borrow();
        same_owner(&param.owner, owner)?;
        return Ok(Lazy::constant(
            owner.clone_ref(py),
            ValueExpr::param(param.id),
        ));
    }
    if let Ok(expr) = other.cast::<Expr>() {
        let expr = expr.borrow();
        let inner = expr.inner.clone();
        same_owner(&inner.owner_ref(py), owner)?;
        return Ok(scalar_to_lazy(py, &inner));
    }
    let v = operand_numeric(other, "addition")?;
    Ok(Lazy::constant(owner.clone_ref(py), ValueExpr::constant(v)))
}

/// Clone any scalar into lazy-tree form (O(1) for trees: one `Arc` bump).
fn scalar_to_lazy(py: Python<'_>, base: &Scalar) -> Lazy {
    match base {
        Scalar::Lazy(l) => l.clone(),
        Scalar::Packed(p) => Lazy {
            owner: p.owner.clone_ref(py),
            root: Root::new(Arc::new(ExprNode::PackedArray(p.array.clone()))),
            has_vars: p.array.numel() > 0,
            size: p.array.numel(),
        },
        Scalar::PackedSymbolic(s) => {
            let terms = PackedSymTerms {
                vars: s.vars.clone(),
                params: s.params.clone(),
                scales: s.scales.clone(),
                constant: s.constant.clone(),
            };
            Lazy {
                owner: s.owner.clone_ref(py),
                root: Root::new(Arc::new(ExprNode::PackedSym(terms))),
                has_vars: !s.vars.is_empty(),
                size: s.vars.len(),
            }
        }
    }
}

/// Add (sign +1) or subtract (sign −1) an operand onto a scalar, returning
/// a new lazy tree. Array operands never reach here (callers return
/// `NotImplemented` first). Semantics mirror the old eager fold exactly;
/// combination is deferred to the sink.
fn scalar_add(
    py: Python<'_>,
    base: &Scalar,
    other: &Bound<'_, PyAny>,
    sign: f64,
) -> PyResult<Lazy> {
    let owner = base.owner_ref(py);
    let lhs = scalar_to_lazy(py, base);
    // Lift the operand, negating once for subtraction.
    let mut rhs = lift_add_operand(py, &owner, other)?;
    if sign == -1.0 {
        rhs = Lazy::scale_node(owner.clone_ref(py), ValueExpr::constant(-1.0), &rhs);
    } else {
        debug_assert_eq!(sign, 1.0);
    }
    Ok(Lazy::add_nodes(
        owner,
        &lhs,
        rhs.root.get().clone(),
        rhs.has_vars,
        rhs.size,
    ))
}

/// Negate: one scale node, no traversal.
fn scalar_neg(py: Python<'_>, base: &Scalar) -> Lazy {
    let owner = base.owner_ref(py);
    let inner = scalar_to_lazy(py, base);
    Lazy::scale_node(owner, ValueExpr::constant(-1.0), &inner)
}

/// Reverse subtraction (`other - base`): lift the operand, then subtract.
fn scalar_rsub(py: Python<'_>, base: &Scalar, other: &Bound<'_, PyAny>) -> PyResult<Lazy> {
    let owner = base.owner_ref(py);
    let lhs = lift_add_operand(py, &owner, other)?;
    let rhs = scalar_to_lazy(py, base);
    let neg = Lazy::scale_node(owner.clone_ref(py), ValueExpr::constant(-1.0), &rhs);
    Ok(Lazy::add_nodes(
        owner,
        &lhs,
        neg.root.get().clone(),
        neg.has_vars,
        neg.size,
    ))
}

/// Multiply by a numeric or parameter-only operand. Nonlinear rejection
/// uses the cached `has_vars` flags, so `x * y` and `(x+1) * (y+1)` still
/// fail here at construction — never at the sink.
fn scalar_mul(py: Python<'_>, base: &Scalar, other: &Bound<'_, PyAny>) -> PyResult<Lazy> {
    let owner = base.owner_ref(py);
    let inner = scalar_to_lazy(py, base);
    if let Ok(param) = other.cast::<Param>() {
        let param = param.borrow();
        same_owner(&param.owner, &owner)?;
        return Ok(Lazy::scale_node(owner, ValueExpr::param(param.id), &inner));
    }
    if let Ok(var) = other.cast::<Var>() {
        let var = var.borrow();
        same_owner(&var.owner, &owner)?;
        if inner.has_vars {
            return Err(nonlinear());
        }
        // Parameter-only self times a variable: the flattened constant
        // becomes the new term's coefficient (the tree is small here —
        // decision-free by the flag above).
        let coeff = inner.const_only();
        let term = Lazy::term(owner.clone_ref(py), var.id, coeff)?;
        return Ok(term);
    }
    if let Ok(expr) = other.cast::<Expr>() {
        let expr = expr.borrow();
        let other_inner = expr.inner.clone();
        drop(expr);
        same_owner(&other_inner.owner_ref(py), &owner)?;
        let other_lazy = scalar_to_lazy(py, &other_inner);
        if inner.has_vars && other_lazy.has_vars {
            return Err(nonlinear());
        }
        if !inner.has_vars {
            let factor = inner.const_only();
            return Ok(Lazy::scale_node(owner, factor, &other_lazy));
        }
        let factor = other_lazy.const_only();
        return Ok(Lazy::scale_node(owner, factor, &inner));
    }
    let v = operand_numeric(other, "multiplication")?;
    if !v.is_finite() {
        return Err(InvalidModelError::new_err(
            "expression scale factor must be finite",
        ));
    }
    Ok(Lazy::scale_node(owner, ValueExpr::constant(v), &inner))
}

/// Divide by a nonzero numeric constant (a parameter divisor rejects,
/// exactly as before).
fn scalar_div(py: Python<'_>, base: &Scalar, other: &Bound<'_, PyAny>) -> PyResult<Lazy> {
    let divisor = operand_numeric(other, "division")?;
    if divisor == 0.0 {
        return Err(InvalidModelError::new_err("division by zero"));
    }
    let scale = 1.0 / divisor;
    if !scale.is_finite() {
        return Err(InvalidModelError::new_err(
            "expression scale factor must be finite",
        ));
    }
    let owner = base.owner_ref(py);
    let inner = scalar_to_lazy(py, base);
    Ok(Lazy::scale_node(owner, ValueExpr::constant(scale), &inner))
}

#[pyclass(frozen, name = "Expr")]
pub struct Expr {
    pub(crate) inner: Scalar,
}

impl Expr {
    fn inner_term_count(&self) -> usize {
        match &self.inner {
            Scalar::Lazy(l) => l.size,
            Scalar::Packed(p) => p.array.numel(),
            Scalar::PackedSymbolic(s) => s.vars.len(),
        }
    }
}

#[pymethods]
impl Expr {
    fn __repr__(&self) -> String {
        format!(
            "Expr({} terms{})",
            self.inner_term_count(),
            if self.inner_term_count() == 0 {
                String::new()
            } else {
                ", affine".to_string()
            }
        )
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "expressions are unhashable",
        ))
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "symbolic expressions have no truth value; use m.add(...) to constrain them",
        ))
    }

    fn __neg__(slf: Py<Self>) -> PyResult<Py<Self>> {
        Python::attach(|py| {
            let base = slf.bind(py).borrow();
            let inner = scalar_neg(py, &base.inner);
            drop(base);
            Ok(Bound::new(
                py,
                Self {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .unbind())
        })
    }

    fn __add__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let base = slf.bind(py).borrow();
            let inner = scalar_add(py, &base.inner, &other, 1.0)?;
            drop(base);
            Ok(Bound::new(
                py,
                Self {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __radd__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::__add__(slf, other)
    }

    fn __sub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let base = slf.bind(py).borrow();
            let inner = scalar_add(py, &base.inner, &other, -1.0)?;
            drop(base);
            Ok(Bound::new(
                py,
                Self {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __rsub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let base = slf.bind(py).borrow();
            let inner = scalar_rsub(py, &base.inner, &other)?;
            drop(base);
            Ok(Bound::new(
                py,
                Self {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __mul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let base = slf.bind(py).borrow();
            let inner = scalar_mul(py, &base.inner, &other)?;
            drop(base);
            Ok(Bound::new(
                py,
                Self {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __rmul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::__mul__(slf, other)
    }

    fn __truediv__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let base = slf.bind(py).borrow();
            let inner = scalar_div(py, &base.inner, &other)?;
            drop(base);
            Ok(Bound::new(
                py,
                Self {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __le__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let c = compare_operand(&slf, &other, Sense::Le)?;
            Ok(Bound::new(py, c)?.into_any().unbind())
        })
    }

    fn __ge__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let c = compare_operand(&slf, &other, Sense::Ge)?;
            Ok(Bound::new(py, c)?.into_any().unbind())
        })
    }

    fn __eq__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let c = compare_operand(&slf, &other, Sense::Eq)?;
            Ok(Bound::new(py, c)?.into_any().unbind())
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sense {
    Le,
    Ge,
    Eq,
}

fn chained() -> PyErr {
    pyo3::exceptions::PyTypeError::new_err(
        "chained comparisons such as 0 <= x <= 1 are not supported; write m.add(...) for each bound",
    )
}

fn nonlinear() -> PyErr {
    UnsupportedExpressionError::new_err(
        "variable-times-variable products are not supported: this interface is LP/MILP only",
    )
}

fn compare_operand(slf: &Py<Expr>, other: &Bound<'_, PyAny>, sense: Sense) -> PyResult<Comparison> {
    Python::attach(|py| {
        // Move the operand to the left: lhs = self - other, bound 0.
        // Array operands never reach here: every comparison dunder
        // returns NotImplemented for them first. The tree flattens once
        // here; comparisons stay out of the hot construction path.
        let base = slf.bind(py).borrow();
        let lhs = scalar_add(py, &base.inner, other, -1.0)?;
        let owner = base.inner.owner_ref(py);
        drop(base);
        let flat = lhs.flatten_lower(py);
        let rhs = match sense {
            Sense::Le => BoundSide::Upper(ValueExpr::constant(0.0)),
            Sense::Ge => BoundSide::Lower(ValueExpr::constant(0.0)),
            Sense::Eq => BoundSide::Eq(ValueExpr::constant(0.0)),
        };
        Ok(Comparison {
            owner,
            expr: flat,
            rhs,
        })
    })
}

/// Identity check between two owner references.
fn same_owner(a: &Py<Model>, b: &Py<Model>) -> PyResult<()> {
    if a.as_ptr() == b.as_ptr() {
        Ok(())
    } else {
        Err(super::errors::ModelMismatchError::new_err(
            "variable, parameter, or expression from a different model",
        ))
    }
}

#[derive(Clone, Debug)]
pub(crate) enum BoundSide {
    Upper(ValueExpr),
    Lower(ValueExpr),
    Eq(ValueExpr),
}

/// A symbolic comparison (`<=`, `>=`, `==`) awaiting `Model.add`.
/// Truthiness raises `TypeError`; chained comparisons fail loudly because
/// each comparison step returns this descriptor rather than a bool.
#[pyclass(frozen)]
pub struct Comparison {
    pub(crate) owner: Py<Model>,
    pub(crate) expr: Affine,
    pub(crate) rhs: BoundSide,
}

impl Comparison {
    pub(crate) fn owner_check(&self, model: &Bound<'_, Model>) -> PyResult<()> {
        super::handles::check_owner(&self.owner, model)
    }
}

#[pymethods]
impl Comparison {
    fn __repr__(&self) -> &'static str {
        "Comparison(<symbolic>)"
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "comparisons are unhashable",
        ))
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "symbolic comparisons have no truth value; pass them to m.add(...). Chained comparisons such as 0 <= x <= 1 are not supported.",
        ))
    }

    fn __le__(&self, _other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Err(chained())
    }

    fn __ge__(&self, _other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Err(chained())
    }

    fn __eq__(&self, _other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Err(chained())
    }
}

/// Convert a scalar operand into an owned scalar form homed to `model`:
/// `Var` lifts to a unit term, `Param` to a parameter-only expression,
/// numerics to a constant, and `Expr` clones through (preserving the packed
/// form so `minimize`/`maximize` can take the bulk path).
/// Foreign-model handles fail here, so `Model.minimize` never sees them.
pub(crate) fn to_scalar(model: &Bound<'_, Model>, other: &Bound<'_, PyAny>) -> PyResult<Scalar> {
    Python::attach(|py| {
        if let Ok(var) = other.cast::<Var>() {
            let var = var.borrow();
            same_owner(&var.owner, &model.clone().unbind())?;
            return Ok(Scalar::Lazy(var_lazy(py, &var)?));
        }
        if let Ok(param) = other.cast::<Param>() {
            let param = param.borrow();
            same_owner(&param.owner, &model.clone().unbind())?;
            return Ok(Scalar::Lazy(param_lazy(py, &param)));
        }
        if let Ok(expr) = other.cast::<Expr>() {
            let expr = expr.borrow();
            same_owner(&expr.inner.owner_ref(py), &model.clone().unbind())?;
            return Ok(expr.inner.clone());
        }
        let v = operand_numeric(other, "objective")?;
        Ok(Scalar::Lazy(Lazy::constant(
            model.clone().unbind(),
            ValueExpr::constant(v),
        )))
    })
}

/// Operator lifts: a variable becomes one lazy term, a parameter one lazy
/// constant. Both are O(1); combination happens at the sink.
fn var_lazy(py: Python<'_>, var: &Var) -> PyResult<Lazy> {
    Lazy::term(var.owner.clone_ref(py), var.id, ValueExpr::constant(1.0))
}

fn param_lazy(py: Python<'_>, param: &Param) -> Lazy {
    Lazy::constant(param.owner.clone_ref(py), ValueExpr::param(param.id))
}

/// Wrap a lazy tree as an expression value.
fn wrap_expr(py: Python<'_>, inner: Lazy) -> PyResult<Py<Expr>> {
    Ok(Bound::new(
        py,
        Expr {
            inner: Scalar::Lazy(inner),
        },
    )?
    .unbind())
}

#[pymethods]
impl Var {
    fn __repr__(&self) -> String {
        format!("Var({:?})", self.name)
    }

    fn __str__(&self) -> String {
        format!("Var({:?})", self.name)
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "variables are unhashable: symbolic == does not define key equality; use solution.value(x)",
        ))
    }

    fn __neg__(slf: Py<Self>) -> PyResult<Py<Expr>> {
        Python::attach(|py| {
            let var = slf.bind(py).borrow();
            let base = var_lazy(py, &var)?;
            drop(var);
            let inner = scalar_neg(py, &Scalar::Lazy(base));
            wrap_expr(py, inner)
        })
    }

    fn __add__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let var = slf.bind(py).borrow();
            let base = Scalar::Lazy(var_lazy(py, &var)?);
            drop(var);
            let inner = scalar_add(py, &base, &other, 1.0)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __radd__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::__add__(slf, other)
    }

    fn __sub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let var = slf.bind(py).borrow();
            let base = Scalar::Lazy(var_lazy(py, &var)?);
            drop(var);
            let inner = scalar_add(py, &base, &other, -1.0)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __rsub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let var = slf.bind(py).borrow();
            let base = Scalar::Lazy(var_lazy(py, &var)?);
            drop(var);
            let inner = scalar_rsub(py, &base, &other)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __mul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let var = slf.bind(py).borrow();
            let base = Scalar::Lazy(var_lazy(py, &var)?);
            drop(var);
            let inner = scalar_mul(py, &base, &other)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __rmul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::__mul__(slf, other)
    }

    fn __truediv__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let var = slf.bind(py).borrow();
            let base = Scalar::Lazy(var_lazy(py, &var)?);
            drop(var);
            let inner = scalar_div(py, &base, &other)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __le__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let var = slf.bind(py).borrow();
            let inner = var_lazy(py, &var)?;
            drop(var);
            let expr = Bound::new(
                py,
                Expr {
                    inner: Scalar::Lazy(inner),
                },
            )?;
            let c = compare_operand(&expr.unbind(), &other, Sense::Le)?;
            Ok(Bound::new(py, c)?.into_any().unbind())
        })
    }

    fn __ge__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let var = slf.bind(py).borrow();
            let inner = var_lazy(py, &var)?;
            drop(var);
            let expr = Bound::new(
                py,
                Expr {
                    inner: Scalar::Lazy(inner),
                },
            )?;
            let c = compare_operand(&expr.unbind(), &other, Sense::Ge)?;
            Ok(Bound::new(py, c)?.into_any().unbind())
        })
    }

    fn __eq__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let var = slf.bind(py).borrow();
            let inner = var_lazy(py, &var)?;
            drop(var);
            let expr = Bound::new(
                py,
                Expr {
                    inner: Scalar::Lazy(inner),
                },
            )?;
            let c = compare_operand(&expr.unbind(), &other, Sense::Eq)?;
            Ok(Bound::new(py, c)?.into_any().unbind())
        })
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "variables have no truth value; use m.add(...) to constrain them",
        ))
    }
}

#[pymethods]
impl Param {
    fn __repr__(&self) -> String {
        format!("Param({:?})", self.name)
    }

    fn __str__(&self) -> String {
        format!("Param({:?})", self.name)
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "parameters are unhashable; use model.update(name=value)",
        ))
    }

    fn __neg__(slf: Py<Self>) -> PyResult<Py<Expr>> {
        Python::attach(|py| {
            let param = slf.bind(py).borrow();
            let base = param_lazy(py, &param);
            drop(param);
            let inner = scalar_neg(py, &Scalar::Lazy(base));
            wrap_expr(py, inner)
        })
    }

    fn __add__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let param = slf.bind(py).borrow();
            let base = Scalar::Lazy(param_lazy(py, &param));
            drop(param);
            let inner = scalar_add(py, &base, &other, 1.0)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __radd__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::__add__(slf, other)
    }

    fn __sub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let param = slf.bind(py).borrow();
            let base = Scalar::Lazy(param_lazy(py, &param));
            drop(param);
            let inner = scalar_add(py, &base, &other, -1.0)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __rsub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let param = slf.bind(py).borrow();
            let base = Scalar::Lazy(param_lazy(py, &param));
            drop(param);
            let inner = scalar_rsub(py, &base, &other)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __mul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let param = slf.bind(py).borrow();
            let base = Scalar::Lazy(param_lazy(py, &param));
            drop(param);
            let inner = scalar_mul(py, &base, &other)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __rmul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::__mul__(slf, other)
    }

    fn __truediv__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let param = slf.bind(py).borrow();
            let base = Scalar::Lazy(param_lazy(py, &param));
            drop(param);
            let inner = scalar_div(py, &base, &other)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Lazy(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "parameters have no truth value",
        ))
    }
}

/// Canonicalize a coefficient/constant expression: constant folding plus
/// zero/one identities. `ValueExpr` trees never simplify themselves, so
/// binding-built arithmetic (`0 * p`, accumulated folds) would otherwise
/// carry phantom parameter dependencies. Only exact identities apply;
/// nothing is reordered or approximated.
pub(crate) fn simplify_value(expr: ValueExpr) -> ValueExpr {
    use roml::ValueExpr as V;
    match expr {
        V::Constant(_) | V::Param(_) => expr,
        V::Neg(inner) => match simplify_value(*inner) {
            V::Constant(c) => V::constant(-c),
            s => V::neg(s),
        },
        V::Add(l, r) => {
            let (l, r) = (simplify_value(*l), simplify_value(*r));
            match (&l, &r) {
                (V::Constant(a), V::Constant(b)) => V::constant(a + b),
                (V::Constant(a), _) if *a == 0.0 => r,
                (_, V::Constant(b)) if *b == 0.0 => l,
                _ => V::add(l, r),
            }
        }
        V::Sub(l, r) => {
            let (l, r) = (simplify_value(*l), simplify_value(*r));
            match (&l, &r) {
                (V::Constant(a), V::Constant(b)) => V::constant(a - b),
                (_, V::Constant(b)) if *b == 0.0 => l,
                _ => V::sub(l, r),
            }
        }
        V::Mul(l, r) => {
            let (l, r) = (simplify_value(*l), simplify_value(*r));
            match (&l, &r) {
                (V::Constant(a), V::Constant(b)) => V::constant(a * b),
                (V::Constant(a), _) if *a == 0.0 => V::constant(0.0),
                (_, V::Constant(b)) if *b == 0.0 => V::constant(0.0),
                (V::Constant(a), _) if *a == 1.0 => r,
                (_, V::Constant(b)) if *b == 1.0 => l,
                _ => V::mul(l, r),
            }
        }
        V::Div(l, r) => {
            let (l, r) = (simplify_value(*l), simplify_value(*r));
            match (&l, &r) {
                (V::Constant(a), V::Constant(b)) => V::constant(a / b),
                (V::Constant(a), _) if *a == 0.0 => V::constant(0.0),
                (_, V::Constant(b)) if *b == 1.0 => l,
                _ => V::div(l, r),
            }
        }
    }
}
