//! MIR-04 M4-4: normalized ordinal-IR fingerprint (IR-25).

use roml::Model;

fn build(shift_ids: bool, rename: bool, extra: bool) -> u64 {
    let mut model = Model::new();
    if shift_ids {
        // Allocate then remove: live variables keep the same ordinal structure
        // but shifted absolute ids and a fresh generation.
        let dead = model.var("dead", 1).bounds(0.0, 1.0).build().unwrap();
        model.remove_variable(dead.get(0).unwrap()).unwrap();
    }
    let (nx, ny) = if rename {
        ("cost", "revenue")
    } else {
        ("x", "y")
    };
    let x = model.var(nx, [2, 3]).bounds(0.0, 1.0).build().unwrap();
    let y = model.var(ny, [2, 3]).bounds(0.0, 1.0).build().unwrap();
    let price = model.param("price", [2, 3], &[5.0; 6]).unwrap();
    let net = y.expr().unwrap().try_sub(x.expr().unwrap()).unwrap();
    let objective = price.try_mul(&net).unwrap().unwrap();
    model.maximize_array(&objective).unwrap();
    if extra {
        let z = model.var("z", 1).bounds(0.0, 2.0).build().unwrap();
        model.add_row(z.expr().unwrap().le(1.0)).unwrap();
    }
    model.normalized_ordinal_fingerprint().unwrap()
}

#[test]
fn identical_ordinal_structure_ignores_names_owners_and_generations() {
    assert_eq!(build(false, false, false), build(true, true, false));
}

#[test]
fn structural_difference_changes_the_fingerprint() {
    assert_ne!(build(false, false, false), build(false, false, true));
}

#[test]
fn parameter_values_do_not_change_the_fingerprint() {
    let mut model = Model::new();
    let x = model.var("x", [2, 3]).bounds(0.0, 1.0).build().unwrap();
    let price = model.param("price", [2, 3], &[1.0; 6]).unwrap();
    let objective = price.try_mul(&x.expr().unwrap()).unwrap().unwrap();
    model.maximize_array(&objective).unwrap();

    let before = model.normalized_ordinal_fingerprint().unwrap();
    let span = *price.view().view().span();
    model.set_parameters_bulk(span, &[9.0; 6]).unwrap();
    model.commit().unwrap();
    assert_eq!(before, model.normalized_ordinal_fingerprint().unwrap());
}
