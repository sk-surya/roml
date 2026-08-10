# P36 Exact Netlib Transcode Manifest

**Pinned corpus:** `sk-surya/lp-data-netlib@56257eea85b433ce6aa67d26156b36385318fd6f`

**Expected path:** `testdata/corpora/netlib/mps_files/`

**Expected inventory:** exactly 94 `.mps` files listed below. This is the P36 qualification set; runtime directory discovery is not an authority for deciding which files count.

The source repository documents QAP8, QAP12, QAP15, and STOCFOR3 as unconverted. The converted corpus used by P35 contains the standard Netlib converted set plus generated TRUSS. P36 freezes the exact filenames rather than relying on a count-only assertion.

```text
25fv47.mps
80bau3b.mps
adlittle.mps
afiro.mps
agg.mps
agg2.mps
agg3.mps
bandm.mps
beaconfd.mps
blend.mps
bnl1.mps
bnl2.mps
boeing1.mps
boeing2.mps
bore3d.mps
brandy.mps
capri.mps
cycle.mps
czprob.mps
d2q06c.mps
d6cube.mps
degen2.mps
degen3.mps
dfl001.mps
e226.mps
etamacro.mps
fffff800.mps
finnis.mps
fit1d.mps
fit1p.mps
fit2d.mps
fit2p.mps
forplan.mps
ganges.mps
gfrd-pnc.mps
greenbea.mps
greenbeb.mps
grow15.mps
grow22.mps
grow7.mps
israel.mps
kb2.mps
lotfi.mps
maros-r7.mps
maros.mps
modszk1.mps
nesm.mps
perold.mps
pilot.ja.mps
pilot.mps
pilot.we.mps
pilot4.mps
pilot87.mps
pilotnov.mps
recipe.mps
sc105.mps
sc205.mps
sc50a.mps
sc50b.mps
scagr25.mps
scagr7.mps
scfxm1.mps
scfxm2.mps
scfxm3.mps
scorpion.mps
scrs8.mps
scsd1.mps
scsd6.mps
scsd8.mps
sctap1.mps
sctap2.mps
sctap3.mps
seba.mps
share1b.mps
share2b.mps
shell.mps
ship04l.mps
ship04s.mps
ship08l.mps
ship08s.mps
ship12l.mps
ship12s.mps
sierra.mps
stair.mps
standata.mps
standgub.mps
standmps.mps
stocfor1.mps
stocfor2.mps
truss.mps
tuff.mps
vtp.base.mps
wood1p.mps
woodw.mps
```

## Qualification rules

1. The submodule must be initialized at the exact pinned SHA before the broad P36 corpus gate.
2. Every manifest file must exist as a regular file under the pinned `mps_files/` directory.
3. A missing manifest file is a **qualification failure**, not a skip.
4. An unexpected additional `.mps` file is reported as corpus drift and blocks qualification until the manifest/pin is reviewed; it is not silently added.
5. Every one of the 94 files must pass:
   - P35 ROML import;
   - P36 `SemanticModel` writer representability;
   - deterministic second write;
   - independent ROML mathematical round-trip oracle;
   - native HiGHS structural differential.
6. Writer `Unrepresentable`, missing corpus, parser rejection, unresolved structural mismatch, or unexpected panic is a **failure** for this frozen corpus.
7. Solve comparison runs for the plan's bounded deterministic solve subset and may be expanded; structural transcode remains mandatory for all 94.
8. Qualification output records all 94 paths explicitly with one final state each. The number of PASS rows must equal 94.

## Pre-implementation validation gate

Task 36-00 must verify this manifest against the pinned gitlink before any writer production code is committed. If the exact pinned checkout disagrees with this list, stop P36 and amend the written spec through review rather than editing the implementation to accommodate drift.
