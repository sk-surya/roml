//! MIR-04 M4-2/M4-5: natural operator algebra and contiguous reshape.

use roml::{Model, ModelSnapshot};

fn bess(explicit: bool) -> ModelSnapshot {
    let (b, t) = (2usize, 4usize);
    let dt = 0.25;
    let eta = 0.9;
    let mut model = Model::new();
    let charge = model
        .var("charge", [b, t])
        .bounds(0.0, 1.0)
        .build()
        .unwrap();
    let discharge = model
        .var("discharge", [b, t])
        .bounds(0.0, 1.0)
        .build()
        .unwrap();
    let energy = model
        .var("energy", [b, t])
        .bounds(0.0, 1.0)
        .build()
        .unwrap();

    let next = energy.slice(1, 1, t - 1).unwrap();
    let prev = energy.slice(1, 0, t - 1).unwrap();
    let c = charge.slice(1, 1, t - 1).unwrap();
    let d = discharge.slice(1, 1, t - 1).unwrap();

    let residual = if explicit {
        let rhs = prev
            .expr()
            .unwrap()
            .try_add(c.expr().unwrap().scaled(dt * eta))
            .unwrap()
            .try_add(d.expr().unwrap().scaled(-dt / eta))
            .unwrap();
        next.expr().unwrap().try_sub(rhs).unwrap()
    } else {
        // energy[:, 1:] - (energy[:, :-1] + dt * (eta * charge[:, 1:] - discharge[:, 1:] / eta))
        next.clone() - (prev.clone() + dt * (eta * c.clone() - d.clone() / eta))
    };
    model.add_row(residual.eq(0.0)).unwrap();
    model.take_snapshot().unwrap()
}

#[test]
fn operator_algebra_matches_explicit_composition() {
    assert_eq!(bess(true), bess(false));
}

#[test]
fn negation_and_scalar_mul_match_explicit_scaling() {
    let mut a = Model::new();
    let x = a.var("x", 3).bounds(0.0, 1.0).build().unwrap();
    a.minimize_array(&(-x.clone() * 2.0)).unwrap();

    let mut b = Model::new();
    let x = b.var("x", 3).bounds(0.0, 1.0).build().unwrap();
    b.minimize_array(&x.expr().unwrap().scaled(-2.0)).unwrap();

    assert_eq!(a.take_snapshot().unwrap(), b.take_snapshot().unwrap());
}

#[test]
fn operator_scalar_constant_shifts_a_reduction_per_cell() {
    let (m, n) = (2usize, 3usize);
    let values = vec![10.0; m];

    let mut a = Model::new();
    let x = a.var("x", [m, n]).bounds(0.0, 1.0).build().unwrap();
    let cons = a
        .add_rows((x.clone() + 2.0).rows_eq(&values).unwrap())
        .unwrap();
    for con in cons {
        let b = a.constraint_bounds(con).unwrap();
        assert_eq!(b.upper, 10.0 - 2.0 * n as f64);
    }

    let mut b_model = Model::new();
    let x = b_model.var("x", [m, n]).bounds(0.0, 1.0).build().unwrap();
    b_model
        .add_rows(
            x.expr()
                .unwrap()
                .try_shift(2.0)
                .unwrap()
                .rows_eq(&values)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(a.take_snapshot().unwrap(), b_model.take_snapshot().unwrap());
}

#[test]
fn contiguous_reshape_is_metadata_only_and_rejects_strided_views() {
    let mut model = Model::new();
    let x = model.var("x", [2, 3]).bounds(0.0, 1.0).build().unwrap();
    let flat = x.reshape([6]).unwrap();
    assert_eq!(flat.shape(), &[6]);
    assert_eq!(flat.get(4).unwrap(), x.get(4).unwrap());
    assert_eq!(model.num_variables(), 6, "reshape allocates nothing");

    let back = flat.reshape([3, 2]).unwrap();
    assert_eq!(back.shape(), &[3, 2]);
    assert_eq!(back.get(5).unwrap(), x.get(5).unwrap());

    // A strided (non-contiguous) view cannot be flattened.
    let strided = x.slice(1, 1, 2).unwrap();
    assert!(strided.reshape([4]).is_err());
    // A size-changing reshape is rejected.
    assert!(x.reshape([5]).is_err());
}
