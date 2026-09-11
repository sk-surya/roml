//! MIR-04 M4-2: array expression composition reaches automatic eligibility.

use roml::Model;

#[test]
fn bess_objective_from_array_handles_is_automatically_eligible() {
    let (b, t) = (4usize, 3usize);
    let n = b * t;
    let mut model = Model::new();

    let charge = model
        .var("charge", [b, t])
        .bounds(0.0, 1.0)
        .build()
        .expect("charge");
    let discharge = model
        .var("discharge", [b, t])
        .bounds(0.0, 1.0)
        .build()
        .expect("discharge");
    let price = model.param("price", [b, t], &vec![50.0; n]).expect("price");

    // dt * price * (discharge - charge)
    let net = discharge
        .expr()
        .expect("discharge expr")
        .try_sub(charge.expr().expect("charge expr"))
        .expect("discharge - charge");
    let objective = price
        .try_mul(&net)
        .expect("fast composition")
        .expect("conservative IR covers price * (discharge - charge)")
        .scaled(0.25);
    model.maximize_array(&objective).expect("objective");

    let lowering = model.lowering_stats();
    assert!(
        lowering.param_dep_blocks >= 2,
        "automatic dependency blocks: {lowering:?}"
    );
    assert_eq!(lowering.param_positions_cells, 0);
    assert_eq!(lowering.general_affine, 0);
    assert_eq!(model.num_coefficients(), 2 * n);
}
