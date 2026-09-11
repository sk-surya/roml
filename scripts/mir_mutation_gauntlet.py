#!/usr/bin/env python3
"""MIR invariant mutation gauntlet (MIR-02 architecture defense).

For each named MIR invariant, apply source mutation(s) that break it, run the
focused test(s) that are supposed to defend it, and require the run to FAIL
(mutation killed). Every edit is reverted immediately, and the script refuses
to run if the working tree is not clean.

This is a hand-crafted semantic gauntlet (not `cargo-mutants` operator soup) so
each mutation corresponds to a specific architectural claim from the MIR-02
remediation review:

  1. overlapping ParamDepBlocks        -> overlap/double-ownership rejection
  2. dropped uncovered param_positions -> non-eligible packed fallback
  3. wrong StridedMap traversal        -> frozen row-major convention
  4. SetObjectiveCosts expanded        -> packed objective batching
  5. shadowed packed cell not skipped  -> shadowing correctness
  6. ValueExpr lost during replay      -> symbolic reference state

Usage:
    CARGO_TARGET_DIR=... python3 scripts/mir_mutation_gauntlet.py \
        [--out evidence/mir-mutation-report.json]

Exit status is non-zero if any mutation survives or the tree is dirty.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Each mutation is one or more exact source edits (file, old, new). Every `old`
# must occur exactly once in its file. `tests` are nextest invocations that must
# FAIL while the edits are applied. The overlap invariant is enforced by two
# layered checks, so its mutant disables both.
MUTATIONS = [
    {
        "id": "overlapping_param_dep_blocks",
        "invariant": "A canonical packed cell is owned by at most one dependency block",
        "edits": [
            ("src/model/coefficient.rs",
             "                owner[idx] += 1;\n                if owner[idx] > 1 {\n",
             "                owner[idx] += 1;\n                if false {\n"),
            ("src/model/mod.rs",
             "                owner[idx] += 1;\n                if owner[idx] > 1 {\n",
             "                owner[idx] += 1;\n                if false {\n"),
        ],
        "tests": [
            ("roml lib: duplicate cell in one witness", ["-p", "roml", "--lib", "-E",
                "test(duplicate_cell_positions_in_one_witness_are_rejected)"]),
            ("mir02_remediation: overlapping witnesses", ["-p", "roml", "--test",
                "mir02_remediation", "-E", "test(overlapping_witnesses_are_rejected_atomically)"]),
        ],
    },
    {
        "id": "dropped_uncovered_param_positions",
        "invariant": "A bulk parameter update also drives non-eligible packed fallback cells",
        "edits": [
            ("src/model/mod.rs",
             "        self.coefficients.propagate_packed_positions_span(\n"
             "            span,\n"
             "            &self.parameters,\n"
             "            &mut self.diagnostics.propagation,\n"
             "            &mut block_patches,\n"
             "        );\n",
             "        // MUTATION: non-eligible packed fallback dropped\n"),
        ],
        "tests": [
            ("mir02_remediation: non-eligible fallback", ["-p", "roml", "--test",
                "mir02_remediation", "-E", "test(bulk_update_propagates_non_eligible_packed_positions)"]),
        ],
    },
    {
        "id": "wrong_strided_map_traversal",
        "invariant": "StridedMap ordinals are row-major (last dimension fastest)",
        "edits": [
            ("src/bulk.rs",
             "        for dim in (0..self.shape.len()).rev() {\n",
             "        for dim in 0..self.shape.len() {\n"),
        ],
        "tests": [
            ("roml lib: row-major ordinals", ["-p", "roml", "--lib", "-E",
                "test(strided_map_row_major_ordinals_and_zero_stride)"]),
        ],
    },
    {
        "id": "expanded_set_objective_costs",
        "invariant": "An eligible objective reprice stays one packed SetObjectiveCosts op",
        "edits": [
            ("src/compiler/session.rs",
             "                        operations.push(BackendOp::SetObjectiveCosts { objective, costs });\n",
             "                        for (variable, value) in costs {\n"
             "                            operations.push(BackendOp::SetObjectiveCoefficient {\n"
             "                                objective,\n"
             "                                variable,\n"
             "                                value,\n"
             "                            });\n"
             "                        }\n"),
        ],
        "tests": [
            ("mir02_backend_batching: one packed cost op", ["-p", "roml", "--test",
                "mir02_backend_batching", "-E", "test(eligible_reprice_compiles_to_one_packed_cost_op)"]),
        ],
    },
    {
        "id": "shadowed_packed_cell_not_skipped",
        "invariant": "Shadowed packed cells are skipped during span propagation",
        "edits": [
            ("src/model/coefficient.rs",
             "                if bit_get(&self.p_dead, pos_u) || bit_get(&self.p_shadowed, pos_u) {\n",
             "                if bit_get(&self.p_dead, pos_u) {\n"),
        ],
        "tests": [
            ("mir02_edge_cases: fully shadowed family", ["-p", "roml", "--test",
                "mir02_edge_cases", "-E",
                "test(fully_shadowed_family_emits_value_change_without_patch_batch)"]),
        ],
    },
    {
        "id": "value_expr_lost_during_replay",
        "invariant": "ReferenceBackend patch replay preserves the symbolic ValueExpr",
        "edits": [
            ("src/solver/reference.rs",
             "                        CoefficientTarget::Objective(_) => {\n"
             "                            let entry = self.objective_cells.get_mut(&key).ok_or_else(|| {\n"
             "                                format!(\n"
             "                                    \"coefficient patch targets missing objective cell {:?}\",\n"
             "                                    key\n"
             "                                )\n"
             "                            })?;\n"
             "                            entry.1 = patch.new;\n"
             "                        }\n",
             "                        CoefficientTarget::Objective(_) => {\n"
             "                            let entry = self.objective_cells.get_mut(&key).ok_or_else(|| {\n"
             "                                format!(\n"
             "                                    \"coefficient patch targets missing objective cell {:?}\",\n"
             "                                    key\n"
             "                                )\n"
             "                            })?;\n"
             "                            entry.0 = ValueExpr::constant(patch.new);\n"
             "                            entry.1 = patch.new;\n"
             "                        }\n"),
        ],
        "tests": [
            ("mir02_remediation: symbolic replay", ["-p", "roml", "--test",
                "mir02_remediation", "-E", "test(reference_replay_preserves_symbolic_patch_cells)"]),
        ],
    },
]


def run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    env = dict(os.environ)
    env.setdefault("CARGO_TERM_COLOR", "never")
    return subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)


def git_dirty() -> bool:
    out = subprocess.run(["git", "status", "--porcelain", "--untracked-files=no"],
                         cwd=ROOT, capture_output=True, text=True).stdout
    return bool(out.strip())


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default=None)
    args = parser.parse_args()

    if git_dirty():
        print("error: working tree is dirty; refusing to mutate sources", file=sys.stderr)
        return 2

    results = []
    for mut in MUTATIONS:
        # Baseline: the defending tests must pass unmutated, so a later non-zero
        # exit really means the mutation was killed.
        for label, selector in mut["tests"]:
            proc = run(["cargo", "nextest", "run", *selector])
            if proc.returncode != 0:
                print(f"error: {mut['id']}: baseline '{label}' did not pass unmutated "
                      f"(exit {proc.returncode})", file=sys.stderr)
                print(proc.stderr[-2000:], file=sys.stderr)
                return 2

        touched: list[str] = []
        try:
            for file, old, new in mut["edits"]:
                path = ROOT / file
                source = path.read_text()
                occurrences = source.count(old)
                if occurrences != 1:
                    print(f"error: {mut['id']}: expected 1 occurrence in {file}, "
                          f"found {occurrences}", file=sys.stderr)
                    return 2
                path.write_text(source.replace(old, new))
                touched.append(file)

            for label, selector in mut["tests"]:
                proc = run(["cargo", "nextest", "run", *selector])
                killed = proc.returncode != 0
                detail = ""
                if killed:
                    lines = [ln for ln in proc.stderr.splitlines() if "FAIL" in ln]
                    detail = lines[0].strip() if lines else "did not compile / non-zero exit"
                results.append({
                    "id": mut["id"],
                    "invariant": mut["invariant"],
                    "test": label,
                    "killed": killed,
                    "exit_code": proc.returncode,
                    "detail": detail,
                })
                print(f"[{'KILLED' if killed else 'SURVIVED':8s}] {mut['id']:34s} <- {label}")
        finally:
            subprocess.run(["git", "checkout", "--", *touched], cwd=ROOT, check=True)

    if git_dirty():
        print("error: working tree is dirty after the gauntlet", file=sys.stderr)
        return 2

    survived = [r for r in results if not r["killed"]]
    report = {
        "mutations": results,
        "killed": sum(1 for r in results if r["killed"]),
        "survived": len(survived),
        "verdict": "all-killed" if not survived else "survivors-present",
    }
    if args.out:
        out = ROOT / args.out
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(report, indent=1) + "\n")
        print(f"\nwrote {out}")

    print(f"\n{report['killed']}/{len(results)} defended tests killed their mutation")
    return 0 if not survived else 1


if __name__ == "__main__":
    raise SystemExit(main())
