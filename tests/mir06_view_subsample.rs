//! MIR-06 M6-1B: metadata-only subsample/squeeze primitives for Python indexing.

use roml::Model;

#[test]
fn subsample_and_squeeze_are_metadata_only() {
    let mut model = Model::new();
    let x = model.var("x", [3, 4]).bounds(0.0, 1.0).build().unwrap();

    // Rows 0 and 2 (step 2), no cells gathered.
    let picked = x.subsample(0, 0, 2, 2).unwrap();
    assert_eq!(picked.shape(), &[2, 4]);
    assert_eq!(picked.get(0).unwrap(), x.get(0).unwrap());
    assert_eq!(picked.get(4).unwrap(), x.get(8).unwrap());
    assert_eq!(model.num_variables(), 12, "subsample allocates nothing");

    // Squeeze a length-1 axis.
    let one = x.slice(0, 1, 1).unwrap();
    assert_eq!(one.shape(), &[1, 4]);
    let squeezed = one.squeeze(0).unwrap();
    assert_eq!(squeezed.shape(), &[4]);
    assert_eq!(squeezed.get(0).unwrap(), x.get(4).unwrap());

    // Out-of-range subsample and non-unit squeeze reject.
    assert!(x.subsample(0, 0, 1, 4).is_err());
    assert!(x.squeeze(0).is_err());
}
