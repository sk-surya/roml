# Phase 35 Test Matrix — MPS Import and Qualification

This matrix is the minimum verification contract for P35. Implementation planning must map each row to concrete tests and evidence.

## A. Lexical and source-location tests

| ID | Case | Expected evidence |
|---|---|---|
| L01 | fixed `ROWS` field boundaries | exact row kind/name extraction |
| L02 | fixed COLUMNS first pair | exact variable/row/value extraction |
| L03 | fixed COLUMNS second pair | exact optional second pair extraction |
| L04 | fixed RHS/RANGES | vector/row/value extraction |
| L05 | fixed BOUNDS | bound kind/vector/variable/value extraction |
| L06 | free long names | names preserved without truncation |
| L07 | free scientific notation | exact `f64` parse |
| L08 | `LF` input | same semantics as CRLF fixture |
| L09 | `CRLF` input | same semantics/source line numbers as LF |
| L10 | comment lines | ignored, next source line preserved |
| L11 | blank lines | accepted where legal |
| L12 | non-ASCII byte | typed encoding error |
| L13 | malformed number | typed invalid-number with span |
| L14 | overly long line under configured limit | typed resource/line-length error |
| L15 | fixed/free auto unique-fixed | locks Fixed |
| L16 | fixed/free auto unique-free | locks Free |
| L17 | fixed/free dual-identical | accepted without premature lock |
| L18 | fixed/free dual-different | `AmbiguousFormat` includes interpretations |
| L19 | mixed format after lock | deterministic parse error |

## B. Section-state tests

| ID | Case | Expected |
|---|---|---|
| S01 | minimal ROWS/COLUMNS/ENDATA | success |
| S02 | NAME omitted | success |
| S03 | RHS omitted | zero RHS defaults |
| S04 | RANGES omitted | no ranges |
| S05 | BOUNDS omitted | default domains |
| S06 | ENDATA missing | typed error |
| S07 | duplicate ROWS section | typed duplicate/order error |
| S08 | BOUNDS before COLUMNS | typed order error |
| S09 | data after ENDATA | typed trailing-data error |
| S10 | recognized quadratic section | `UnsupportedSection` |
| S11 | recognized conic section | `UnsupportedSection` |
| S12 | unknown section header | typed invalid/unknown section error |
| S13 | OBJSENSE legal position | success |
| S14 | OBJNAME legal position | success |
| S15 | duplicate OBJSENSE | error |
| S16 | OBJNAME after matrix section | order error |

## C. Objective tests

| ID | Case | Expected |
|---|---|---|
| O01 | first `N` objective | first `N` selected |
| O02 | `OBJNAME` selects later `N` | named row selected |
| O03 | invalid `OBJNAME` | typed error |
| O04 | OBJNAME references E/L/G | typed error |
| O05 | no N row | zero objective |
| O06 | empty objective row | zero coefficient vector |
| O07 | absent OBJSENSE | minimize |
| O08 | MIN/MINIMIZE | minimize |
| O09 | MAX/MAXIMIZE | maximize |
| O10 | RHS objective row = +25 | objective constant = -25 |
| O11 | RHS objective row = -3.1415 | objective constant = +3.1415 |
| O12 | duplicate objective coefficients | algebraic sum |
| O13 | nonselected N coefficients | no constraint/objective effect |

## D. Matrix and row tests

| ID | Case | Expected |
|---|---|---|
| M01 | E row, RHS b | `[b,b]` |
| M02 | G row, RHS b | `[b,+inf]` |
| M03 | L row, RHS b | `[-inf,b]` |
| M04 | omitted RHS | b = 0 |
| M05 | duplicate matrix entry | coefficient sum |
| M06 | duplicates sum to zero | canonical zero omitted |
| M07 | repeated column blocks | merged correctly |
| M08 | unknown row reference | typed error |
| M09 | duplicate row declaration | typed error |
| M10 | free row not objective | ignored as constraint, preserved metadata |

## E. RANGES tests

| ID | Base row | Range | Expected bounds |
|---|---|---:|---|
| R01 | E, b | `+r` | `[b,b+r]` |
| R02 | E, b | `-r` | `[b-r,b]` |
| R03 | G, b | `+r` | `[b,b+r]` |
| R04 | G, b | `-r` | `[b,b+r]` |
| R05 | L, b | `+r` | `[b-r,b]` |
| R06 | L, b | `-r` | `[b-r,b]` |
| R07 | RANGE on N row | typed error |
| R08 | duplicate selected range for row | typed ambiguity error |
| R09 | unselected range vector contains malformed semantic combination | parse/stage succeeds if syntactically valid; selected vector alone affects model |

Here `r > 0` in the expected column.

## F. Bound/domain tests

| ID | Records/context | Expected final domain |
|---|---|---|
| B01 | no bounds, continuous | `[0,+inf]` continuous |
| B02 | FR | `[-inf,+inf]` continuous |
| B03 | FX 3 | `[3,3]` continuous |
| B04 | LO -2 | `[-2,+inf]` continuous |
| B05 | UP 7 | `[0,7]` continuous |
| B06 | MI | `[-inf,+inf]` from default upper |
| B07 | UP 7 then MI | `[-inf,7]` |
| B08 | FR then LO 2 | `[2,+inf]` |
| B09 | PL after finite UP | upper resets `+inf` |
| B10 | BV | `[0,1]` binary |
| B11 | LI 2.2 | integer lower 3 |
| B12 | UI 8.8 | integer upper 8 |
| B13 | LI + UI | general integer interval |
| B14 | lower > upper | typed domain error |
| B15 | marker variable no BOUNDS | integer `[0,1]` |
| B16 | marker + UP 10 | integer `[0,10]` |
| B17 | marker + LO -5 | integer `[-5,1]` |
| B18 | marker + FR | free integer `[-inf,+inf]` |
| B19 | BV then broadening record | typed conflicting-binary error under P35 policy |
| B20 | continuous LO/UP after integer marker | remains integer |

Current ROML already represents `VarType::Integer` independently from `Bounds` and accepts `-inf/+inf` as valid integer bounds, so B18 is a required supported case rather than a deferred capability probe.

## G. Marker tests

| ID | Case | Expected |
|---|---|---|
| I01 | one INTORG/INTEND block | variables inside integer |
| I02 | multiple disjoint blocks | all enclosed vars integer |
| I03 | nested INTORG | marker nesting error |
| I04 | INTEND without INTORG | marker nesting error |
| I05 | EOF/section transition before INTEND | marker nesting error |
| I06 | marker record creates no variable | structural count unchanged |
| I07 | same variable inside/outside marker blocks | integer dominates; coefficients merge |

## H. Rim-vector selection tests

| ID | Case | Expected |
|---|---|---|
| V01 | two RHS vectors default | first selected |
| V02 | named second RHS | second selected |
| V03 | `None` RHS | zero RHS |
| V04 | missing named RHS | typed error |
| V05 | two RANGES default/named | exact selected semantics |
| V06 | two BOUNDS default/named | exact selected domains |
| V07 | interleaved records for vectors | first-seen vector order preserved; records grouped correctly |
| V08 | import metadata | selected vector names recorded |

## I. Transaction/error invariants

| ID | Case | Expected |
|---|---|---|
| E01 | syntax failure after many valid rows | no `Model` result |
| E02 | semantic domain failure in BOUNDS | no partial model result |
| E03 | unknown row in RHS | source-aware error |
| E04 | unknown variable in BOUNDS | source-aware error |
| E05 | unsupported section after valid linear data | hard failure; no partial model |
| E06 | fuzz arbitrary bytes | no panic/UB; success or typed error only |

## J. Metamorphic equivalence

Each pair/group below must compile to equivalent canonical mathematical snapshots:

1. fixed and free forms of the same model;
2. one coefficient pair per line versus two pairs per line;
3. a coefficient `3` versus duplicate records `1 + 2`;
4. explicit RHS zero versus omitted RHS;
5. explicit default `LO 0` versus omitted bound;
6. grouped columns versus repeated column blocks;
7. alternate legal row/column declaration order with names preserved;
8. equivalent range-sign encodings where row sense makes sign irrelevant.

## K. HiGHS synthetic differential probes

For each probe, run:

```text
A: native HiGHS readModel(file)
B: ROML MpsReader(file) -> ROML compile -> HiGHS
```

Required probes:

- objective offset sign;
- objective selection;
- duplicate matrix/objective entries;
- each RANGE rule;
- default bounds;
- FR/MI/PL/FX sequences;
- INTORG default bounds;
- marker + explicit bounds;
- BV/LI/UI;
- multiple rim vectors;
- long-name free MPS;
- empty objective.

Compare model structure before solve where the HiGHS API exposes it, then solve status/objective where meaningful.

## L. Netlib corpus tiers

### PR smoke allowlist

Select a small set only after inspecting actual corpus features. The allowlist must include:

- a tiny sparse LP;
- an equality-heavy LP;
- a model with nontrivial bounds;
- a model with ranges if present;
- a numerically varied model.

Selection is an implementation-planning task based on corpus inventory, not a guessed filename list in this design packet.

### Broad scheduled set

Run every file classified as within P35 dialect. Record unsupported/skipped files deterministically.

Per file compare:

- ROML parse result;
- native HiGHS parse result;
- dimensions/nonzeros;
- domains/row bounds/objective when extractable;
- solve classification;
- objective value tolerance for feasible solved LPs.

## M. Chinneck corpus tiers

### PR IIS smoke

Use small/medium models chosen from Set 1 after inventory. Include at least:

- one all-free model;
- its lower-bounded counterpart where available;
- `IC-wine-LB.mps` or another small bound-participating model;
- one empty-objective case (the Set 1 models already provide this property).

### Scheduled IIS qualification

Run broader supported collections with explicit budgets. Very large Netlib-derived infeasible cases may be parser/solve-only under ordinary scheduled CI and full IIS only in manual/performance qualification.

Per complete IIS result record:

- model name;
- selected vector names;
- rows/cols/nonzeros;
- native HiGHS and ROML infeasibility status;
- analysis mode/provider chain;
- guarantee/completion;
- member row/bound counts;
- source mapping success;
- oracle-call count;
- elapsed analysis time;
- final verifier outcome.

Do not compare exact member identities to Gurobi as a pass/fail condition.

## N. P36 forward round-trip matrix

The following tests are not P35 exit gates but are frozen now:

1. synthetic ROML LP -> free MPS -> P35 read -> semantic equality;
2. synthetic MILP -> free MPS -> read -> domain equality;
3. ranged model -> write/read -> one ranged semantic constraint preserved;
4. objective constant -> write/read sign-preserved;
5. external Netlib -> read -> canonical write -> native HiGHS read -> solve equivalence;
6. external Chinneck -> read -> canonical write -> native HiGHS read -> infeasibility preserved;
7. deterministic byte-for-byte writer output for identical model/options;
8. unrepresentable ROML construct -> typed representation error.

## O. Verification evidence

P35 release evidence must include:

- exact ROML commit;
- Rust version/MSRV;
- HiGHS bundled version and system-version lanes used;
- corpus repository URLs and exact SHAs;
- synthetic test totals;
- differential corpus pass/fail/skip counts;
- unsupported-section classifications;
- fuzz/mutation evidence applicable to parser code;
- performance characterization without unsupported marketing claims;
- residual risks and intentionally deferred P36 work.
