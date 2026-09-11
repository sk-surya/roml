//! MIR-04 M4-6: solution results are readable through array handles, so
//! examples need no raw variable ids for results.

use std::collections::HashMap;

use roml::solution::SolutionReadError;
use roml::{Model, Solution, SolveStatus};

#[test]
fn solution_reads_arrays_without_raw_ids() {
    let (b, t) = (2usize, 3usize);
    let mut model = Model::new();
    let x = model.var("x", [b, t]).bounds(0.0, 1.0).build().unwrap();

    let mut values = HashMap::new();
    for ordinal in 0..x.len() {
        values.insert(x.get(ordinal).unwrap(), ordinal as f64 + 0.5);
    }
    let solution = Solution::from_values(values, SolveStatus::Optimal);
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
fn missing_array_values_are_typed_errors_not_silent_zeros() {
    let mut model = Model::new();
    let x = model.var("x", 4).bounds(0.0, 1.0).build().unwrap();

    let mut partial = HashMap::new();
    partial.insert(x.get(0).unwrap(), 1.0);
    let partial = Solution::from_values(partial, SolveStatus::Optimal);

    assert_eq!(partial.array_values(&x), vec![Some(1.0), None, None, None]);
    assert!(matches!(
        partial.try_array_values(&x),
        Err(SolutionReadError::MissingValue { ordinal: 1 })
    ));
}
