# MIR Design — Normative

Terminology: *core* = canonical `roml` model/coefficient/journal/delta machinery; *modeling* = new `roml::modeling` Level-1 array IR; *bulk* = Level-2 block/CSR descriptors and commit primitives; *advanced* = existing raw/advanced surface.

## 1. Layering

```text
                        LABEL / COMPONENT BOUNDARY
               Rust Labeled<_,_> / Python Set, RangeSet, pd.Index
                         names + labels ↔ ordinals
                                  │
                                  ▼
                   MODEL-OWNED ORDINAL ARRAY IR
                         roml::modeling (L1)
                                  │
          VarView / ParamView = owner + strided View<Span>
                                  │
                               LinArray
              Σ Term{VarView, CoeffView} + ConstantView
                                  │
                    RowBlockSpec / ObjectiveSpec
                                  │
                                  ▼
                     CSR / BLOCK BUILDER (L2)
             constant stream + parametric stream + sink map
                                  │
                                  ▼
                       CANONICAL CORE COMMIT
           packed base / packed p-base / sparse general fallback
                                  │
                                  ▼
                 revisioned self-contained ModelOps
                                  │
                                  ▼
                         backend application
```

The general `ValueExpr` / overlay / sparse dependency path remains a correct sibling path. MIR does not remove it or force exotic expressions into the fast IR.

## 2. Trusted spans and L2 strided layouts

Fresh block allocation returns opaque spans:

```rust
pub struct VarSpan   { /* crate-private start,len */ }
pub struct ParamSpan { /* crate-private start,len */ }
```

Only core block allocation can construct them. A user cannot manufacture a span from `(start,len)`.

All members of a fresh block use the arena's fresh generation. IDs are never reused, so a block view reconstructs candidate IDs from the span and fresh generation; normal liveness validation catches a deleted member without invalidating its siblings. Do not hard-code a public numeric generation value and do not add a span epoch.

The persisted dependency representation must not make core depend on L1 modeling types. L2 owns language-independent strided ordinal descriptors conceptually equivalent to:

```rust
pub struct StridedMap {
    shape: Arc<[usize]>,
    strides: Arc<[isize]>,
    offset: isize,
}

pub struct ParamSlice {
    span: ParamSpan,
    map: StridedMap,
}

pub struct PackedCoeffSlice {
    /* packed p-base span/identity + StridedMap; core-created */
}

pub struct ParamDepBlock {
    params: ParamSlice,
    cells: PackedCoeffSlice,
    scale: f64,
    topology: Arc<CanonicalCellTopology>, // self-contained target/var mapping
}
```

Exact storage structs may differ, but these properties are required:
- no dependency on Python or `roml::modeling`;
- no per-cell reverse-index list on the eligible path;
- the stored descriptor identifies parameter ordinals, packed coefficient positions, and self-contained solver-facing canonical cells;
- the descriptor is immutable/shareable across retained deltas.

## 3. Core block primitives (MIR-01)

Target Level-2 primitives:

```rust
pub enum BlockBounds<'a> {
    Uniform(Bounds),
    PerElement(&'a [Bounds]),
}

impl Model {
    pub fn add_variable_block(
        &mut self,
        n: usize,
        ty: VarType,
        bounds: BlockBounds<'_>,
    ) -> Result<VarSpan, ModelError>;

    pub fn add_parameter_block(
        &mut self,
        values: &[f64],
    ) -> Result<ParamSpan, ModelError>;
}
```

Rules:

- Validate the whole block before mutation.
- Reserve once, then allocate sequentially.
- `add_variable_block` emits one packed `Change::VariableBlockAdded` and one self-contained `ModelOp::AddVariableBlock`; the op may contain uniform bounds or an owned/shared dense bounds buffer.
- `add_parameter_block` preserves current scalar semantics: parameter creation itself is not a solver-facing mutation and does not introduce `ParameterBlockAdded`/`AddParameterBlock` merely for existence. It performs one bulk store mutation with zero per-element journal records.
- Existing `variable_name()/parameter_name() -> Option<&str>` semantics remain intact for scalar stored names. Array/component base names are compact L1/frontend metadata. MIR does not allocate `name[i]` strings in core.
- Existing scalar add APIs remain available.

## 4. Parametric packed construction and update (MIR-02)

Existing constant bulk rows remain supported unchanged. MIR adds a parametric p-base path and the internal mixed-row commit seam needed by the future CSR builder.

Conceptually:

```rust
pub fn add_linear_rows_param_bulk(
    &mut self,
    row_ptr: &[u32],
    vars: &[VarId],
    params: &[ParamId],
    scales: &[f64],
    bounds: &[ConstraintBounds],
    dep: Option<&ParamDepLayout>,
) -> Result<Vec<ConId>, ModelError>;

pub fn set_parameters_bulk(
    &mut self,
    span: ParamSpan,
    values: &[f64],
) -> Result<(), ModelError>;
```

`ParamDepLayout` is an L2 metadata witness supplied by a trusted lowerer. MIR-02 tests may hand-construct it for known block-shaped fixtures; MIR-03's `try_param_block_layout` becomes the production proof function. Core never trusts it blindly: after canonicalization it validates that the witness matches the actual retained parameter/variable cells and resolves it into stored `ParamDepBlock`s. A wrong witness is a typed atomic rejection.

### Parametric canonicalization

The packed p-base stores one `scale × ParamId` per canonical `(target,var)` cell. Therefore duplicate variables in one row are handled as follows:

- same `VarId` + same `ParamId`: sum scales; drop near-zero result;
- same `VarId` + distinct `ParamId`s: not representable by one packed p-cell; return a typed *not-packable* result/error before mutation so the caller can use the general symbolic path;
- non-finite merged scales reject atomically.

Likewise, a constant and a parametric term targeting the same canonical cell is not represented as two physical cells. The CSR builder detects such collisions and routes that cell/spec to the general path. Rows containing disjoint constant and parametric cells may be committed together through an internal mixed-row batch seam so the row targets are allocated once.

The existing packed parametric objective path (`set_linear_objective_param_bulk`) is also retrofitted to accept/store eligible block-dependency layout; MIR's flagship repricing win is primarily here.

### Dependency authority

For eligible families:
- construction does not populate per-cell `param_positions`;
- an immutable `ParamDepBlock` is stored canonically and included in the packed construction payload;
- semantic dependency iteration/introspection remains complete by consulting both block dependencies and the sparse/overlay dependency representation;
- scalar updates to a block-created parameter remain correct (they may use one span/block lookup, never require a per-cell hash list).

For ineligible/general families, existing `param_positions`/overlay behavior remains unchanged.

### Transaction and revision semantics

`set_parameters_bulk` preserves the existing transaction model: it validates and queues a block update; `commit()` applies it atomically with other pending parameter updates.

A committed eligible parameter-block update produces:
1. one packed parameter-value change in the canonical revision log; and
2. one packed coefficient-patch batch containing all affected eligible `ParamDepBlock`s.

The coefficient patch delta is **self-contained**. It may share immutable topology created at construction, but a `ModelOp` must never require access to mutable p-base positions in the live `Model`. Backend adapters may expand the logical batch if a native API lacks a matching batch call.

Performance invariant for the eligible propagation path:
- zero `param_positions` lookups per cell;
- zero overlay lookups per cell;
- zero `ValueExpr` evaluations;
- zero per-cell changelog records;
- zero per-cell `ModelOp`s;
- O(n) strided arithmetic/value packing is expected.

## 5. Shared array IR (MIR-03)

Primitive view metadata remains:

```rust
pub struct View<S> {
    span: S,
    shape: Shape,
    strides: Strides, // signed
    offset: isize,
}
```

Model-owned symbolic views wrap it:

```rust
pub struct VarView   { owner: ModelInstanceId, view: View<VarSpan> }
pub struct ParamView { owner: ModelInstanceId, view: View<ParamSpan> }
pub struct NumView   { /* owned/shared numeric buffer + strided view */ }
```

Cross-model symbolic composition is a typed error before IDs are reconstructed.

Initial coefficient IR:

```rust
pub enum CoeffView {
    One,
    Scalar(f64),
    Dense { scale: f64, values: NumView },
    ScaledParam { scale: f64, params: ParamView },
}

pub enum ConstantView {
    Zero,
    Scalar(f64),
    Dense { scale: f64, values: NumView },
    ScaledParam { scale: f64, params: ParamView },
}

pub struct Term {
    vars: VarView,
    coeff: CoeffView,
}

pub struct LinArray {
    owner: ModelInstanceId,
    shape: Shape,
    terms: Vec<Term>,
    constant: ConstantView,
}
```

Properties:
- slice/transpose/negative-step operations edit metadata, not cells;
- `α * Dense` multiplies the stored scalar `scale`; it never copies the numeric buffer;
- `α * ScaledParam` folds `α` into the scalar scale;
- `LinArray ± LinArray` shape-checks, owner-checks and concatenates terms;
- reductions/dot/matmul may create additional compact topology metadata but do not gather `Vec<VarId>`/`Vec<ParamId>` merely to represent a view.

### Initial conservative parameter multiplication rule

For MIR's first IR, `ParamView * LinArray` stays on the fast representation only when:
- each variable term coefficient is `One` or `Scalar`; and
- the array constant is `Zero` or `Scalar`.

That yields `ScaledParam` terms/constants without introducing parameter×parameter or dense-numeric×parameter coefficient products. `Dense × ParamView`, `ScaledParam × ParamView`, and other uncovered forms fall to the general symbolic path. This is an initial optimization boundary, not a statement that such linear expressions are mathematically unsupported; counters decide whether a fifth coefficient kind is later justified.

Python's current packed expression representations are migration seeds, not a separate permanent IR.

## 6. ParamDepBlock eligibility (normative, D-019)

For each candidate parametric family, the lowering layer must prove from metadata only that:

1. `r -> (target(r), var(r))` is injective into canonical coefficient cells;
2. interactions with every other term in the same sink spec do not create a second contribution to the same canonical cell; and
3. after canonical ordering, the retained coefficient positions admit the L2 strided storage witness required by `ParamDepLayout`.

Eligibility is sink-aware. Reusing one `VarId` across distinct row targets may be eligible; reusing it repeatedly inside one objective target is not. Stride sign, bounding-range overlap and "positive stride" are not sufficient predicates.

The proof is conservative. Uncertain means fallback. The proof function:

```rust
fn try_param_block_layout(
    sink: &SinkMap,
    terms: &[Term],
) -> Option<ParamDepLayout>;
```

produces only an L2 witness; core revalidates it against the post-canonical block before storing it.

## 7. Storage rules

- Constant packed base and parametric p-base are append-only.
- Fresh canonical cells append to the appropriate base even after prior solves.
- Mutation/replacement of an existing logical cell moves/shadows that cell into the sparse overlay.
- Block propagation skips dead/shadowed packed positions without turning the whole dependency family into per-cell reverse-index entries.
- The canonical coefficient authority is the union of packed constant cells, packed parametric cells, eligible block dependencies and sparse overlay/general dependencies. Queries/removals must remain semantically complete.

## 8. Rule/callback path (MIR-05)

Rust closures and Python rule/decorator callbacks may execute once per index to *construct* row expressions, but they push into a CSR builder and commit the component in bulk. Per-index core insertion is a defect.

Native Rust closure rules should remain near the bulk path except for expression-construction cost. Python rule callbacks are Python-bound but must not compound that with per-row core hash/journal work.

## 9. Rust Level-1 surface (MIR-04)

Target style:

```rust
use roml::prelude::*;

let mut m = Model::named("bess");
let charge    = m.var("charge",    (b, t)).bounds(0.0, p_max).build()?;
let discharge = m.var("discharge", (b, t)).bounds(0.0, p_max).build()?;
let energy    = m.var("energy",    (b, t_plus_1)).bounds(0.0, cap).build()?;
let price     = m.param("price",    (b, t), &prices)?;

m.add(
    energy.slice((.., 1..)).eq(
        energy.slice((.., ..t_len))
        + dt * (eta * &charge - &discharge / eta)
    )
)?;

m.maximize(dt * sum(&price * (&discharge - &charge)))?;
```

No raw `VarId` or manual `LinExpr` in ordinary vectorized user code. Scalar/raw APIs remain available as lower-level escape hatches.

Labels and component names live at the boundary:
- Rust uses `Labeled<A, Axes>` or equivalent metadata around ordinal arrays;
- Python uses `Set`/`RangeSet`/`pd.Index`/`MultiIndex`;
- alignment is checked once at the labeled boundary and mismatches are errors.

## 10. Template / bind

Do not build Pyomo `AbstractModel`.

- fixed structure + changing data: parameter blocks + persistent session; `Template::bind` performs bulk parameter updates and incremental synchronization;
- changing structure/cardinality: `Template::build(shapes) -> Model`.

## 11. Diagnostics

Use read-only diagnostics to prevent silent fast-path decay:

```rust
pub struct LoweringStats {
    pub numeric_bulk: u64,
    pub parametric_bulk: u64,
    pub general_affine: u64,
    pub param_dep_blocks: u64,
    pub param_positions_cells: u64,
    pub rule_rows_accumulated: u64,
    pub rule_bulk_commits: u64,
}

pub struct PropagationStats {
    pub param_position_lookups: u64,
    pub overlay_lookups: u64,
    pub value_expr_evals: u64,
    pub coefficient_patch_batches: u64,
}
```

Exact exposure may be `roml::diagnostics` / a read-only model accessor and a private Python testing hook. These counters are qualification/debug surfaces, not part of the mathematical model semantics.
