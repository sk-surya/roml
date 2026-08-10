# P36 MPS Write-Back Frozen Contract

**Authority:** P36 implementation must satisfy this file plus the milestone `SHARED-CONTRACTS.md` and MPS-W01–W14. Any mismatch is a written-spec blocker.

## 1. Default public API

P36 has one semantic meaning: write the current canonical/evaluated mathematical model to deterministic free MPS.

Target API shape:

```rust
pub struct MpsWriter {
    options: MpsWriteOptions,
}

pub struct MpsWriteOptions {
    pub name_policy: MpsNamePolicy,
    pub destination_policy: MpsDestinationPolicy,
}

pub enum MpsNamePolicy {
    PreserveOrGenerate,
    StrictPreserve,
}

pub enum MpsDestinationPolicy {
    AtomicReplace,
    CreateNew,
}
```

The callable writer API is frozen as follows:

```rust
impl MpsWriter {
    pub fn new() -> Self;
    pub fn with_options(options: MpsWriteOptions) -> Self;

    /// Serialize to the caller-provided stream. This method may leave partial
    /// bytes on failure and never performs destination replacement.
    pub fn write<W: std::io::Write>(
        &self,
        model: &Model,
        output: W,
    ) -> Result<MpsWriteReport, MpsWriteError>;

    /// Serialize and commit according to `options.destination_policy`.
    pub fn write_path<P: AsRef<std::path::Path>>(
        &self,
        model: &Model,
        path: P,
    ) -> Result<MpsWriteReport, MpsWriteError>;
}
```

`destination_policy` is consulted only by `write_path`; `write` is a stream operation and has no atomic-commit guarantee. `new()` is equivalent to `with_options(MpsWriteOptions::default())`. Both methods capture one evaluated canonical snapshot before emitting any output. A failed operation returns `MpsWriteError` and no success report.

`MpsWriteOptions::default()` is frozen as:

```text
name_policy        = PreserveOrGenerate
destination_policy = AtomicReplace
output dialect     = free MPS
line endings       = LF
numeric formatting = ROML canonical finite f64 formatting
```

Output dialect, LF line endings, vector names, and numeric formatting are **not** user-tunable in P36. This keeps one canonical byte representation.

A compiled-formulation export is **not** a P36 public option. If later required, it receives a separate design/API because it has different identity, provenance, and backend-IR semantics. P36 never ships an option that can only return `Unsupported`.

## 2. Successful report

A successful writer returns `MpsWriteReport` with at least:

```rust
pub struct MpsWriteReport {
    pub model_lineage: ModelLineageId,
    pub model_instance: ModelInstanceId,
    pub model_revision: ModelRevision,
    pub evaluated_parameters: Vec<MpsEvaluatedParameter>,
    pub columns: usize,
    pub rows: usize,
    pub nonzeros: usize,
    pub integer_columns: usize,
    pub objective_present: bool,
    pub rhs_vector: Option<String>,
    pub ranges_vector: Option<String>,
    pub bounds_vector: Option<String>,
    pub name_map: MpsWriteNameMap,
    pub lowerings: Vec<MpsWriteLowering>,
    pub omitted_inactive_entities: usize,
}
```

`MpsEvaluatedParameter` identifies the parameter through the model's public semantic handle/name context and records the finite value consumed by export. Ordering is deterministic and never based on debug-formatted arena IDs.

`MpsWriteLowering` records mathematically exact but non-source-preserving semantic lowerings such as a persistent fixing emitted as an `FX`/equal-bound domain. It must never describe an approximation.

A failed write returns no success report.

## 3. Error taxonomy

The public error must preserve a stable top-level kind plus structured context. Exact Rust variant names may be adjusted during API review, but the following distinctions are mandatory:

```text
Io
DestinationExists
AtomicReplaceUnavailable
PathTransaction
ModelValidation
Unrepresentable
ParameterEvaluation
NonFiniteValue
NameAllocation
Serialization
StaleEntity
InternalInvariant
```

Required context where applicable:
- source model instance/revision;
- entity kind + semantic handle/name;
- unsupported feature/domain/construct;
- numeric field and evaluated parameter dependencies;
- filesystem path;
- path-transaction stage (`CreateTemp`, `Write`, `Flush`, `Sync`, `Replace`, `Cleanup`);
- underlying I/O cause via `Error::source` or equivalent preserved cause.

`Unrepresentable` is never converted to a warning.

## 4. Representability matrix

| Canonical feature | P36 export | Export rule | Round-trip claim |
|---|---|---|---|
| continuous variable, default/nondefault bounds | supported | deterministic BOUNDS omission/records | same mathematical bounds |
| integer variable | supported | deterministic INTORG/INTEND + explicit bounds when needed | same integrality + bounds |
| binary variable | supported | deterministic BV or equivalent canonical form | same binary domain |
| free continuous/integer | supported | FR plus marker/type semantics as needed | same domain |
| fixed variable / equal declared bounds | supported | FX/equal bound | same domain |
| persistent `Model::fix` | supported as exact lowering | emit effective equal bounds; report `PersistentFixingAsBound` | same feasible set; fixing provenance is not reconstructed |
| semi-continuous / semi-integer | **reject** | `Unrepresentable::VariableDomain` | none |
| linear equality/lower/upper/ranged constraint | supported | E/G/L + RHS/RANGES | same normalized row |
| inactive primitive constraint | omitted | record inactive omission count | same active mathematical model |
| active semantic construct (indicator, Boolean, min/max, abs, product, PWL, etc.) | **reject** | typed construct error; do not silently compile | none |
| inactive semantic construct | omitted | report omission; no mathematical effect | same active mathematical model |
| active single objective | supported | ROWS/COLUMNS + OBJSENSE + offset convention | same scalar objective |
| no active objective | supported | deterministic zero-objective encoding | same feasibility problem |
| extra inactive objectives | omitted | report omission | same active mathematical model |
| weighted/lexicographic `ObjectivePolicy` after P31 | **reject** | standard linear MPS cannot preserve policy | none |
| parameterized coefficient/bound/objective/offset | supported by evaluation | evaluate against exact `(instance, revision)` environment | same evaluated mathematical snapshot; parameter graph not reconstructed |
| NaN/±inf coefficient or objective offset | **reject** | `NonFiniteValue` | none |
| ±inf variable/row bounds where standard MPS semantics can represent them | supported | omit/default or FR/MI/PL as canonical | same bounds |
| duplicate/missing/invalid user names | supported under default | `PreserveOrGenerate`; deterministic name map | mathematical equality; display names may be generated |
| invalid name under `StrictPreserve` | **reject** | `NameAllocation`/typed name error | none |
| source comments/layout/rim-vector spellings | not represented | explicit non-goal | no textual round-trip claim |
| solve overlay / temporary lock / cutoff | not part of canonical export | never observed by writer | base model only |
| compiler-generated bridge rows/variables | not part of P36 | typed `Unrepresentable` if an active semantic construct requires them | compiled-formulation export is deferred to a separate future design |

The P36 equality oracle is **normalized active mathematical equality**, not byte/source/provenance equality. Loss of a canonical distinction is allowed only when the emitted formulation is mathematically exact and the report explicitly records the lowering/omission rule above.

## 5. Parameterized export

Binding rules are in shared contracts §3. Additional P36 rules:
- take/validate one canonical snapshot before projection;
- parameter evaluation is completed before path commit;
- report the exact model instance/revision and every consumed parameter value;
- if the model mutates concurrently through an unsupported access path, ordinary Rust borrowing must prevent it; no writer-internal re-read of mutable state after projection is allowed;
- output contains numeric constants only.

## 6. Deterministic naming

Binding algorithm is shared contracts §4. The writer must not call `Debug`/`Display` on IDs to form bytes.

Name namespaces used by the P36 writer are frozen per MPS category. A collision in preserved names triggers deterministic generation for the colliding entities rather than iteration-order-dependent suffixing.

## 7. Cross-platform `write_path` transaction

### 7.1 `CreateNew`

- serialize/validate first;
- create a same-directory temporary file with collision-safe `create_new` semantics;
- write, flush, and sync the temporary file;
- commit only if destination still does not exist;
- destination-exists race returns `DestinationExists` and removes the temp;
- no existing destination is modified.

### 7.2 `AtomicReplace`

Required behavior on supported P36 platforms (Linux/macOS/Windows):
- serialize/validate before commit;
- stage in a unique file in the destination directory;
- write + flush + sync staged bytes;
- replace the destination with one platform-appropriate atomic same-volume replace operation;
- never implement replace as `remove(destination)` followed by `rename(temp)`;
- if an atomic replacement primitive cannot be provided on a target, return `AtomicReplaceUnavailable` **before modifying the destination**;
- best-effort cleanup failure is preserved alongside the primary path error.

The implementation may use a narrowly scoped platform support dependency/API if required; the dependency and unsafe boundary receive independent review.

### 7.3 Injection seam

Implement path commit behind an internal testable seam conceptually equivalent to:

```rust
trait MpsPathOps {
    fn create_temp(&self, destination: &Path) -> io::Result<TempHandle>;
    fn write_all(&self, temp: &mut TempHandle, bytes: &[u8]) -> io::Result<()>;
    fn flush_and_sync(&self, temp: &mut TempHandle) -> io::Result<()>;
    fn atomic_commit(&self, temp: TempHandle, destination: &Path, policy: MpsDestinationPolicy) -> io::Result<()>;
    fn cleanup(&self, temp: TempHandle) -> io::Result<()>;
}
```

Fault injection must cover every stage and verify destination bytes + temp cleanup + error composition.

## 8. Independent semantic oracle

The P36 ROML round-trip test oracle must be test-local and independent of writer internals.

It may use public/model snapshot APIs to extract a normalized mathematical tuple:

```text
objective sense/coefficients/offset
sorted variable domains and integrality
sorted row lower/upper bounds
sorted matrix coordinates and coefficients
```

It must **not** call:
- writer projection/normalization helpers;
- writer naming helpers;
- the HiGHS oracle implementation;
- writer report generation as the source of expected values.

The before and after models are independently normalized by this test-only extractor and compared under frozen numeric rules.

## 9. Native structural and solve mismatch policy

### Structural comparison

For every qualified model compare:
- row/column counts;
- named/normalized matrix cells;
- row bounds;
- column bounds;
- integrality;
- objective sense;
- objective coefficients;
- objective offset.

Tolerance: `abs_diff <= 1e-10 + 1e-10 * max(|a|, |b|)` for finite scalar structural values unless P36 implementation evidence proves a stricter exact comparison is safe. Any changed tolerance requires review before rerun.

### Solve comparison

Normalize termination into ROML classes. Required equality:
- `Optimal` ↔ `Optimal`;
- `Infeasible` ↔ `Infeasible`;
- `Unbounded` ↔ `Unbounded` when both APIs make that distinction;
- ambiguous `InfeasibleOrUnbounded`, limits, numerical/unknown outcomes are not silently coerced into stronger classes.

For paired `Optimal` results compare objective with:

```text
abs_diff <= 1e-7 + 1e-8 * max(|z_direct|, |z_mps|)
```

No primal-vector equality is required when multiple optima exist.

### Frozen dispositions

A mismatch is one of:

```text
roml_writer_bug
roml_reader_bug
roml_projection_bug
backend_oracle_limitation
intentional_roml_rejection
corpus_out_of_contract
```

`backend_oracle_limitation` and `corpus_out_of_contract` require written evidence and owner/reviewer approval. No unresolved mismatch may count as a pass.

## 10. P36 closure predicate

P36 is complete iff all MPS-W01–W14 are evidenced, the exact 94-model manifest passes with zero missing files and zero writer rejections, all mandatory CI/review gates pass, and no unresolved P0/P1 finding or semantic mismatch remains.
