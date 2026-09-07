#!/usr/bin/env bash
# P34 packed-consumer/package protocol (contract §5). Run from a clean
# exact-head worktree. Records its own output; exits nonzero on any failure.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
HEAD_SHA="$(git rev-parse HEAD)"
# Fixed /tmp paths: concurrent runs of this qualification script collide by
# design; run one at a time and keep the log.
PACKED=/tmp/p34-packed
OUT=/tmp/p34-packed-consumers.log
exec > >(tee "$OUT") 2>&1

echo "=== p34-packed-consumers ==="
echo "head: $HEAD_SHA"
echo "rustc: $(rustc --version)"
echo "cargo: $(cargo --version)"
cd "$ROOT"

echo "--- package lists ---"
cargo package --list -p roml > /tmp/p34-roml-package-list.txt
cargo package --list -p roml-highs > /tmp/p34-roml-highs-package-list.txt
wc -l /tmp/p34-roml-package-list.txt /tmp/p34-roml-highs-package-list.txt

echo "--- pack roml ---"
cargo package -p roml --locked
ROML_CRATE="$(ls -t target/package/roml-*.crate | head -1)"
echo "roml crate: $ROML_CRATE"

echo "--- attempt pack roml-highs ---"
if cargo package -p roml-highs --locked 2>/tmp/p34-highs-pack-err.txt; then
    HIGHS_CRATE="$(ls -t target/package/roml-highs-*.crate | head -1)"
    echo "roml-highs crate: $HIGHS_CRATE"
    HIGHS_PACK_MODE="crate"
else
    echo "roml-highs pack failed; checking for the documented unpublished-roml limitation only:"
    cat /tmp/p34-highs-pack-err.txt
    if grep -qE "no matching package named|not published|failed to resolve|location searched: crates.io" /tmp/p34-highs-pack-err.txt; then
        echo "accepted: unpublished-roml resolution limitation"
        HIGHS_PACK_MODE="packed-tree"
    else
        echo "FATAL: roml-highs packaging failed for another reason"
        exit 1
    fi
fi

echo "--- extract ---"
rm -rf "$PACKED"
mkdir -p "$PACKED"
tar -xzf "$ROML_CRATE" -C "$PACKED"
ROML_DIR="$PACKED/$(basename "$ROML_CRATE" .crate)"
if [ "${HIGHS_PACK_MODE:-}" = "crate" ]; then
    tar -xzf "$HIGHS_CRATE" -C "$PACKED"
    HIGHS_DIR="$PACKED/$(basename "$HIGHS_CRATE" .crate)"
else
    # Workspace-independent packed-source tree from the package-list manifest
    # plus the extracted roml .crate: no live-workspace paths.
    HIGHS_VER="$(cargo metadata --no-deps --format-version 1 2>/dev/null | python3 -c 'import json,sys; print([p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="roml-highs"][0])')"
    HIGHS_DIR="$PACKED/roml-highs-$HIGHS_VER"
    mkdir -p "$HIGHS_DIR"
    while IFS= read -r f; do
        # Generated pack metadata is not present in the working tree.
        case "$f" in
            .cargo_vcs_info.json | .cargo-checksum.json | Cargo.toml.orig) continue ;;
        esac
        # The highs package list is relative to roml-highs/, not the root.
        # Cargo.lock is generated at consumer build time, not shipped from
        # the workspace (root lock only); record the skip explicitly.
        case "$f" in
            Cargo.lock)
                echo "note: skipping Cargo.lock (generated per consumer build)"
                continue
                ;;
        esac
        if [ ! -e "$ROOT/roml-highs/$f" ]; then
            echo "FATAL: packed file missing from working tree: roml-highs/$f"
            exit 1
        fi
        mkdir -p "$HIGHS_DIR/$(dirname "$f")"
        cp "$ROOT/roml-highs/$f" "$HIGHS_DIR/$f"
    done < /tmp/p34-roml-highs-package-list.txt
    # Point the packed tree at the extracted roml crate sources and
    # materialize workspace-inherited fields (cargo normalizes these only
    # inside .crate archives, which roml-highs cannot produce while roml
    # is unpublished).
    python3 - "$HIGHS_DIR/Cargo.toml" "$ROML_DIR" <<'EOF'
import sys
path, roml_dir = sys.argv[1], sys.argv[2]
s = open(path).read()
s = s.replace('roml = { version = "0.1.0", path = ".." }',
              'roml = { version = "0.1.0", path = "%s" }' % roml_dir)
for key, value in [
    # rust-version first: it contains 'version.workspace' as a substring.
    ('rust-version.workspace = true', 'rust-version = "1.85"'),
    ('version.workspace = true', 'version = "0.1.0"'),
    ('edition.workspace = true', 'edition = "2021"'),
    ('authors.workspace = true', 'authors = ["Surya Krishnan"]'),
    ('repository.workspace = true',
     'repository = "https://github.com/sk-surya/roml"'),
    ('license.workspace = true', 'license = "MIT OR Apache-2.0"'),
]:
    assert key in s, "expected workspace inheritance for " + key
    s = s.replace(key, value)
open(path, 'w').write(s)
print("packed roml-highs manifest materialized")
EOF
fi
echo "roml dir: $ROML_DIR"
echo "highs dir: $HIGHS_DIR"

echo "--- packed-tree assertions ---"
for d in "$ROML_DIR" "$HIGHS_DIR"; do
    for banned in ".planning" ".worktrees" "testdata/corpora" ".git" "target" ".crate"; do
        if [ -e "$d/$banned" ]; then echo "FATAL: banned path $banned in $d"; exit 1; fi
    done
    for need in "Cargo.toml" "src"; do
        if [ ! -e "$d/$need" ]; then echo "FATAL: missing $need in $d"; exit 1; fi
    done
    if [ ! -e "$d/README.md" ] && [ ! -e "$d/LICENSE-MIT" ] && [ ! -e "$d/LICENSE-APACHE" ]; then
        echo "WARN: no readme/license file inside $d (checked at closure)"
    fi
done
if grep -rn "path *= *\"\.\./" "$HIGHS_DIR/Cargo.toml" | grep -v "$ROML_DIR" ; then
    echo "FATAL: packed roml-highs Cargo.toml points at the live workspace"
    exit 1
fi
if grep -rIl "/home/\|/srv/repos\|/tmp/p34\|machine-local" "$ROML_DIR/src" "$HIGHS_DIR/src" 2>/dev/null | head -3; then
    echo "FATAL: machine-local absolute paths inside packed sources"
    exit 1
fi
echo "packed-tree assertions PASS"

consumer() {
    local name="$1" dep="$2" src="$3" expect="$4"
    local dir="/tmp/p34-consumer-$name"
    rm -rf "$dir"
    cargo new --quiet --bin "$dir"
    cat > "$dir/Cargo.toml" <<EOF
[package]
name = "p34-consumer-$name"
version = "0.1.0"
edition = "2021"

[dependencies]
$dep
EOF
    mkdir -p "$dir/src"
    printf '%s' "$src" > "$dir/src/main.rs"
    local got errfile="/tmp/p34-consumer-$name-err.txt"
    if ! got="$(cargo run --quiet --manifest-path "$dir/Cargo.toml" --offline 2>"$errfile" | tail -1)"; then
        echo "FATAL: consumer $name failed to build/run:"
        cat "$errfile"
        exit 1
    fi
    if [ "$got" != "$expect" ]; then
        echo "FATAL: consumer $name: expected [$expect], got [$got]"
        exit 1
    fi
    echo "consumer $name PASS: $got"
}

echo "--- consumers ---"
consumer core 'roml = { path = "'"$ROML_DIR"'" }' '
use roml::{continuous, ConstraintExprExt, Model};
fn main() {
    let mut model = Model::with_name("p34-core");
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x).ge(2.0)).unwrap();
    let obj = model.minimize(x).unwrap();
    model.set_active_objective(obj).unwrap();
    model.commit().unwrap();
    let snapshot = model.take_snapshot().unwrap();
    assert_eq!(snapshot.revision, model.current_revision());
    println!("core-ok rev={} vars=1", snapshot.revision);
}' "core-ok rev=r1 vars=1"

consumer highs 'roml = { path = "'"$ROML_DIR"'" }
roml-highs = { path = "'"$HIGHS_DIR"'" }' '
use roml::{continuous, ConstraintExprExt, Model, SolveStatus, SolverSession, ValueExpr};
use roml_highs::HighsSession;
fn main() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x).ge(2.0)).unwrap();
    let obj = model.minimize(x).unwrap();
    let p = model.add_parameter(3.0).unwrap();
    model.add_objective_coefficient(obj, x, ValueExpr::param(p)).unwrap();
    let mut session = SolverSession::new(HighsSession::try_new().unwrap());
    let first = session.solve(&mut model).unwrap();
    assert_eq!(first.status(), SolveStatus::Optimal);
    assert!((first.objective_value().unwrap() - 8.0).abs() < 1e-9);
    model.set_parameter(p, 5.0).unwrap();
    let second = session.solve(&mut model).unwrap();
    assert!((second.objective_value().unwrap() - 12.0).abs() < 1e-9);
    println!("highs-ok 8.0 12.0");
}' "highs-ok 8.0 12.0"

consumer mps 'roml = { path = "'"$ROML_DIR"'" }
roml-highs = { path = "'"$HIGHS_DIR"'" }' '
use std::io::Cursor;
use roml::{continuous, ConstraintExprExt, Model, SolverSession};
use roml::io::mps::{MpsReader, MpsWriter};
use roml_highs::HighsSession;
fn main() {
    let mut model = Model::with_name("p34-mps");
    let x = model.add_variable(continuous().bounds(0.0, 10.0).named("x")).unwrap();
    model.add_constraint((x + 2.0 * x).le(8.0).named("cap")).unwrap();
    model.add_constraint((x).ge(1.0).named("floor")).unwrap();
    model.minimize(x).unwrap();
    let mut bytes = Vec::new();
    MpsWriter::new().write(&model, &mut bytes).unwrap();
    let imported = MpsReader::new().read(Cursor::new(bytes)).unwrap();
    let mut reread = imported.model;
    let mut session = SolverSession::new(HighsSession::try_new().unwrap());
    let solved = session.solve(&mut reread).unwrap();
    assert!((solved.objective_value().unwrap() - 1.0).abs() < 1e-7);
    println!("mps-ok 1.0");
}' "mps-ok 1.0"

consumer iis-relax 'roml = { path = "'"$ROML_DIR"'" }
roml-highs = { path = "'"$HIGHS_DIR"'" }' '
use roml::{continuous, ConstraintExprExt, InfeasibilityOutcome, Model, RelaxationOutcome, SolverSession};
use roml::solver::infeasibility::BoundSide;
use roml_highs::HighsSession;
fn main() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let con = model.add_constraint((x).ge(5.0)).unwrap();
    model.add_constraint((x).le(3.0)).unwrap();
    model.minimize(x).unwrap();
    let mut session = SolverSession::new(HighsSession::try_new().unwrap());
    let report = session.analyze_infeasibility(&model, &roml::InfeasibilityPlan::portable_lp()).unwrap();
    assert_eq!(report.outcome, InfeasibilityOutcome::Conflict);
    let repair = session.solve_feasibility_relaxation(&mut model, roml::FeasibilityRelaxationPlan {
        scope: roml::RelaxationScope::Explicit(vec![roml::RelaxationRestriction::ConstraintSide { constraint: con, side: BoundSide::Lower }]),
        ..Default::default()
    }).unwrap();
    assert_eq!(repair.outcome, RelaxationOutcome::OptimalRepair);
    println!("iis-relax-ok conflict repaired");
}' "iis-relax-ok conflict repaired"

consumer lexicographic 'roml = { path = "'"$ROML_DIR"'" }
roml-highs = { path = "'"$HIGHS_DIR"'" }' '
use roml::{continuous, ConstraintExprExt, Model, ObjectivePolicy, ObjectivePriority, SolverSession, StageContinuation, WeightedObjective};
use roml_highs::HighsSession;
fn main() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x + y).le(10.0)).unwrap();
    let obj0 = model.minimize(x).unwrap();
    let obj1 = model.maximize(y).unwrap();
    let mut session = SolverSession::new(HighsSession::try_new().unwrap());
    let result = session.solve_objective_policy(&mut model,
        ObjectivePolicy::Lexicographic(roml::LexicographicObjectives { levels: vec![
            roml::WeightedObjectiveLevel { priority: ObjectivePriority::new(0), objectives: vec![WeightedObjective { objective: obj0, weight: 1.0 }], absolute_tolerance: 1e-9, relative_tolerance: 0.0 },
            roml::WeightedObjectiveLevel { priority: ObjectivePriority::new(1), objectives: vec![WeightedObjective { objective: obj1, weight: 1.0 }], absolute_tolerance: 1e-9, relative_tolerance: 0.0 },
        ]}),
        roml::ObjectiveProviderPolicy::PortableOnly, StageContinuation::RequireOptimal).unwrap();
    assert_eq!(result.stages.len(), 2);
    let z0 = result.stages[0].scalar_stage_value.unwrap();
    assert!((z0 - 0.0).abs() < 1e-7);
    assert!(result.stages[0].lock.is_some());
    println!("lexicographic-ok 2-stages z0=0.0");
}' "lexicographic-ok 2-stages z0=0.0"

echo "=== ALL PACKED CONSUMERS PASS (head $HEAD_SHA) ==="
