//! MIR-04 M4-1: structured array handles allocate once and slice as metadata.

use roml::Model;

#[test]
fn var_and_param_handles_allocate_once_and_slice_metadata_only() {
    let mut model = Model::new();
    let (b, t) = (2usize, 3usize);

    let charge = model
        .var("charge", [b, t])
        .bounds(0.0, 10.0)
        .build()
        .expect("charge array");
    assert_eq!(charge.name(), "charge");
    assert_eq!(charge.shape(), &[b, t]);
    assert_eq!(charge.len(), b * t);
    assert!(charge.owner() == model.instance());
    assert_eq!(charge.get(0).expect("cell").index(), 0);
    assert_eq!(charge.get(5).expect("cell").index(), 5);

    // Slicing is metadata-only: no new variables are allocated.
    let sliced = charge.slice(1, 1, 2).expect("slice");
    assert_eq!(sliced.shape(), &[b, 2]);
    assert_eq!(sliced.len(), 4);
    assert_eq!(sliced.get(0).expect("cell").index(), 1); // (0, 1)
    assert_eq!(sliced.get(1).expect("cell").index(), 2); // (0, 2)
    assert_eq!(sliced.get(2).expect("cell").index(), 4); // (1, 1)
    assert_eq!(model.num_variables(), b * t, "one packed block");

    // Reversal and transpose stay metadata-only.
    let reversed = charge.reverse(0).expect("reverse");
    assert_eq!(reversed.get(0).expect("cell").index(), 3); // (1, 0)
    let transposed = charge.transpose(0, 1).expect("transpose");
    assert_eq!(transposed.shape(), &[t, b]);
    assert_eq!(transposed.get(0).expect("cell").index(), 0);
    assert_eq!(transposed.get(1).expect("cell").index(), 3); // (0,1) -> (1,0)

    // Out-of-range axis is a typed error.
    assert!(charge.slice(2, 0, 1).is_err());

    // Parameter arrays.
    let values: Vec<f64> = (0..b * t).map(|i| i as f64).collect();
    let price = model.param("price", [b, t], &values).expect("price array");
    assert_eq!(price.shape(), &[b, t]);
    assert_eq!(price.len(), b * t);
    assert_eq!(price.get(4).expect("param").index(), 4);
    let price_slice = price.slice(0, 1, 1).expect("slice");
    assert_eq!(price_slice.shape(), &[1, t]);
    assert_eq!(price_slice.get(0).expect("param").index(), 3);

    // Shape/values mismatch is a typed rejection with no residue.
    let before = model.num_variables();
    assert!(model.param("bad", [b, t], &[1.0, 2.0]).is_err());
    assert_eq!(model.num_variables(), before);
}
