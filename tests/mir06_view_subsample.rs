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

/// Integer indexing all the way to rank 0 preserves scalar member identity.
#[test]
fn integer_index_path_reaches_rank_zero() {
    let mut model = Model::new();
    let x = model.var("x", [3, 4]).bounds(0.0, 1.0).build().unwrap();

    let row = x.slice(0, 1, 1).unwrap().squeeze(0).unwrap(); // [4]
    assert_eq!(row.shape(), &[4]);
    let scalar = row.slice(0, 2, 1).unwrap().squeeze(0).unwrap(); // []
    assert_eq!(scalar.shape(), &[]);
    assert_eq!(scalar.len(), 1);
    assert_eq!(scalar.get(0).unwrap(), x.get(6).unwrap());
}

/// Chained multidimensional subsampling maps each final ordinal to the correct
/// root ordinal.
#[test]
fn chained_subsampling_maps_to_root_ordinals() {
    let mut model = Model::new();
    let x = model.var("x", [3, 4, 2]).bounds(0.0, 1.0).build().unwrap();

    // Rows {0, 2} and columns {1, 3}.
    let picked = x
        .subsample(0, 0, 2, 2)
        .unwrap()
        .subsample(1, 1, 2, 2)
        .unwrap();
    assert_eq!(picked.shape(), &[2, 2, 2]);
    for i in 0..2 {
        for j in 0..2 {
            for k in 0..2 {
                let ordinal = i * 4 + j * 2 + k;
                let root = (2 * i) * 8 + (1 + 2 * j) * 2 + k;
                assert_eq!(
                    picked.get(ordinal).unwrap(),
                    x.get(root).unwrap(),
                    "ordinal {ordinal} -> root {root}"
                );
            }
        }
    }
}

/// An empty positive-step selection is well-formed and allocates nothing.
#[test]
fn empty_subsample_is_well_formed() {
    let mut model = Model::new();
    let x = model.var("x", [3, 4]).bounds(0.0, 1.0).build().unwrap();
    let empty = x.subsample(0, 0, 2, 0).unwrap();
    assert_eq!(empty.shape(), &[0, 4]);
    assert_eq!(empty.len(), 0);
    assert!(empty.get(0).is_none());
    assert_eq!(
        model.num_variables(),
        12,
        "empty subsample allocates nothing"
    );
}
