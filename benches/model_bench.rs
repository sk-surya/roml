//! ROML core benchmarks — solver-free model operations.
//!
//! Run with: cargo bench -p roml

use criterion::{criterion_group, criterion_main, Criterion};
use roml::model::{ConstraintBounds, Model};
use roml::value_expr::ValueExpr;
use std::hint::black_box;

fn bench_model_construction(c: &mut Criterion) {
    c.bench_function("model/build_1000_vars", |b| {
        b.iter(|| {
            let mut model = Model::new();
            for _ in 0..1000 {
                black_box(model.add_var());
            }
            black_box(model)
        });
    });

    c.bench_function("model/build_100_vars_100_cons_1000_cells", |b| {
        b.iter(|| {
            let mut model = Model::new();
            let vars: Vec<_> = (0..100).map(|_| model.add_var()).collect();
            let cons: Vec<_> = (0..100)
                .map(|_| model.add_constraint(ConstraintBounds::le(10.0)))
                .collect();
            for i in 0..100 {
                for j in 0..10 {
                    let _ = model.add_coeff(cons[i], vars[(i + j) % 100], 1.0);
                }
            }
            black_box(model)
        });
    });
}

fn bench_parameter_propagation(c: &mut Criterion) {
    c.bench_function("params/propagate_50_params_200_cells_each", |b| {
        b.iter(|| {
            let mut model = Model::new();
            let vars: Vec<_> = (0..200).map(|_| model.add_var()).collect();
            let params: Vec<_> = (0..50).map(|i| model.add_parameter(i as f64)).collect();
            let con = model.add_constraint(ConstraintBounds::le(100.0));

            // Each parameter drives 200 cells
            for (j, param) in params.iter().enumerate() {
                for v in &vars {
                    let _ = model.add_constraint_coefficient(
                        con,
                        *v,
                        ValueExpr::param(*param),
                    );
                }
                // Only do first param to keep benchmark size reasonable
                if j == 0 {
                    break;
                }
            }

            // Change all 50 parameters and commit
            for param in &params {
                model.set_parameter(*param, 100.0);
            }
            model.commit();
            black_box(model)
        });
    });
}

fn bench_invariant_checking(c: &mut Criterion) {
    c.bench_function("invariants/check_1000_var_model", |b| {
        let mut model = Model::new();
        let vars: Vec<_> = (0..1000).map(|_| model.add_var()).collect();
        let cons: Vec<_> = (0..100)
            .map(|_| model.add_constraint(ConstraintBounds::le(10.0)))
            .collect();
        for i in 0..100 {
            for j in 0..10 {
                let _ = model.add_coeff(cons[i], vars[(i + j) % 1000], 1.0);
            }
        }
        model.commit();

        b.iter(|| {
            black_box(model.validate_invariants())
        });
    });
}

fn bench_canonical_cell_combining(c: &mut Criterion) {
    c.bench_function("cells/combine_100_terms_same_cell", |b| {
        b.iter(|| {
            let mut model = Model::new();
            let x = model.add_var();
            let con = model.add_constraint(ConstraintBounds::le(10.0));

            for i in 0..100 {
                let _ = model.add_constraint_coefficient(
                    con,
                    x,
                    ValueExpr::constant(i as f64),
                );
            }
            black_box(model)
        });
    });
}

criterion_group!(
    benches,
    bench_model_construction,
    bench_parameter_propagation,
    bench_invariant_checking,
    bench_canonical_cell_combining,
);
criterion_main!(benches);
