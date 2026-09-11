//! MIR-04 M4-6: solution results are readable through array handles, with
//! model-instance ownership enforced so examples need no raw variable ids.

use std::collections::HashMap;

use roml::solution::SolutionReadError;
use roml::{Model, Solution, SolveStatus};

#[test]
fn solution_reads_arrays_without_raw_ids() {
    let (b, t) = (2usize, 3usize);
    let mut model = Model::new();
    let x = model.var("x", [b, t]).bounds(0.0, 1.0).build().unwrap();
    let owner = model.instance();

    let mut values = HashMap::new();
    for ordinal in 0..x.len() {
        values.insert(x.get(ordinal).unwrap(), ordinal as f64 + 0.5);
    }
    let solution = Solution::from_values(values, SolveStatus::Optimal).with_source_instance(owner);
    assert!(solution.is_optimal());

    let expected: Vec<f64> = (0..b * t).map(|k| k as f64 + 0.5).collect();
    assert_eq!(solution.try_array_values(&x).unwrap(), expected);
    assert_eq!(solution.array_value(&x, 4), Some(4.5));
    assert_eq!(
        solution.array_values(&x),
        expected
            .iter()
            .map(|value| Some(*value))
            .collect::<Vec<_>>()
    );
}

#[test]
fn cross_model_arrays_are_rejected() {
    let mut model_a = Model::new();
    let ax = model_a.var("x", 4).bounds(0.0, 1.0).build().unwrap();
    let mut model_b = Model::new();
    let bx = model_b.var("x", 4).bounds(0.0, 1.0).build().unwrap();

    let mut values = HashMap::new();
    for ordinal in 0..ax.len() {
        values.insert(ax.get(ordinal).unwrap(), 1.0);
    }
    let solution_a = Solution::from_values(values, SolveStatus::Optimal)
        .with_source_instance(model_a.instance());

    // Model B's array must never resolve against model A's solution.
    assert!(matches!(
        solution_a.try_array_values(&bx),
        Err(SolutionReadError::CrossModel { .. })
    ));
    assert_eq!(solution_a.array_values(&bx), vec![None; 4]);
    assert_eq!(solution_a.array_value(&bx, 0), None);
    // The solution's own array still reads.
    assert_eq!(solution_a.try_array_values(&ax).unwrap(), vec![1.0; 4]);
}

#[test]
fn unbound_synthetic_solution_rejects_strict_array_reads() {
    let mut model = Model::new();
    let x = model.var("x", 4).bounds(0.0, 1.0).build().unwrap();

    let mut values = HashMap::new();
    for ordinal in 0..x.len() {
        values.insert(x.get(ordinal).unwrap(), 2.0);
    }
    // No `with_source_instance`: the default metadata carries no real
    // provenance, so strict reads must reject rather than trust it.
    let solution = Solution::from_values(values, SolveStatus::Optimal);
    assert!(matches!(
        solution.try_array_values(&x),
        Err(SolutionReadError::CrossModel { .. })
    ));
    assert_eq!(solution.array_values(&x), vec![None; 4]);
}

#[test]
fn missing_array_values_are_typed_errors_not_silent_zeros() {
    let mut model = Model::new();
    let x = model.var("x", 4).bounds(0.0, 1.0).build().unwrap();
    let owner = model.instance();

    let mut partial = HashMap::new();
    partial.insert(x.get(0).unwrap(), 1.0);
    let partial = Solution::from_values(partial, SolveStatus::Optimal).with_source_instance(owner);

    assert_eq!(partial.array_values(&x), vec![Some(1.0), None, None, None]);
    assert!(matches!(
        partial.try_array_values(&x),
        Err(SolutionReadError::MissingValue { ordinal: 1 })
    ));
}
