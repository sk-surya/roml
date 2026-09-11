//! MIR-04 M4-5: the ordinary L1 example code stays free of raw ids and manual
//! linear-expression construction (IR-24).
//!
//! Scalar/raw APIs remain available as lower-level escape hatches, but the
//! vectorized examples must not reach for them.

const EXAMPLES: &[(&str, &str)] = &[
    ("l1_bess.rs", include_str!("../examples/l1_bess.rs")),
    (
        "l1_transportation.rs",
        include_str!("../examples/l1_transportation.rs"),
    ),
    (
        "l1_min_cost_flow.rs",
        include_str!("../examples/l1_min_cost_flow.rs"),
    ),
];

const FORBIDDEN: &[&str] = &[
    "VarId",
    "LinExpr",
    "ValueExpr",
    "add_var(",
    "add_constraint(",
    "add_constraint_coefficient(",
    "add_linear_rows_bulk(",
];

const REQUIRED: &[&str] = &[".var(", ".param(", "normalized_ordinal_fingerprint"];

#[test]
fn l1_examples_use_no_raw_ids_or_manual_linear_expressions() {
    for (name, source) in EXAMPLES {
        for token in FORBIDDEN {
            assert!(
                !source.contains(token),
                "{name} contains raw construction `{token}`"
            );
        }
    }
}

#[test]
fn l1_examples_exercise_the_array_surface() {
    for (name, source) in EXAMPLES {
        for token in REQUIRED {
            assert!(source.contains(token), "{name} does not use `{token}`");
        }
    }
}
