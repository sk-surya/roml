//! MIR-04 M4-3: labels are a checked boundary and never enter the ordinal IR.

use roml::modeling::{Axis, LabelError, Labeled};
use roml::Model;

fn axis(name: &str, n: usize) -> Axis {
    Axis::new(
        Some(name.to_string()),
        (0..n).map(|i| format!("{name}_{i}")).collect(),
    )
}

#[test]
fn label_boundary_validates_rank_width_and_alignment() {
    let (b, t) = (2usize, 3usize);
    let mut model = Model::new();
    let a = model.var("a", [b, t]).bounds(0.0, 1.0).build().unwrap();
    let c = model.var("c", [b, t]).bounds(0.0, 1.0).build().unwrap();
    let la = Labeled::new(a, vec![axis("bus", b), axis("time", t)]).unwrap();
    let lc = Labeled::new(c, vec![axis("bus", b), axis("time", t)]).unwrap();
    la.align(&lc).expect("aligned");
    la.align_axis(&lc, 1).expect("aligned axis");
    assert_eq!(la.rank(), 2);
    assert_eq!(
        la.axis(1).expect("axis").labels(),
        &["time_0", "time_1", "time_2"]
    );

    // Rank/width rejections at construction.
    let d = model.var("d", [b, t]).bounds(0.0, 1.0).build().unwrap();
    assert!(matches!(
        Labeled::new(d, vec![axis("bus", b)]),
        Err(LabelError::RankMismatch { .. })
    ));
    let e = model.var("e", [b, t]).bounds(0.0, 1.0).build().unwrap();
    assert!(matches!(
        Labeled::new(e, vec![axis("bus", b), axis("time", t + 1)]),
        Err(LabelError::WidthMismatch { .. })
    ));

    // Axis name / label mismatch is a typed error carrying the axis index.
    let f = model.var("f", [b, t]).bounds(0.0, 1.0).build().unwrap();
    let other = Labeled::new(f, vec![axis("bus", b), axis("period", t)]).unwrap();
    match la.align(&other) {
        Err(LabelError::AxisMismatch { axis, .. }) => assert_eq!(axis, 1),
        other => panic!("expected axis mismatch, got {other:?}"),
    }

    let g = model.var("g", [b, t]).bounds(0.0, 1.0).build().unwrap();
    let reversed = Axis::new(
        Some("time".into()),
        (0..t).rev().map(|i| format!("time_{i}")).collect(),
    );
    let reversed = Labeled::new(g, vec![axis("bus", b), reversed]).unwrap();
    assert!(matches!(
        la.align_axis(&reversed, 1),
        Err(LabelError::AxisMismatch { axis: 1, .. })
    ));

    assert_eq!(model.num_constraints(), 0, "no mutation on rejection");
}

#[test]
fn relabeling_does_not_change_the_ordinal_ir() {
    let (b, t) = (2usize, 3usize);

    let build = |axis_name: &str, label_prefix: &str| {
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
        let labels: Vec<String> = (0..t).map(|i| format!("{label_prefix}{i}")).collect();
        let charge = Labeled::new(
            charge,
            vec![
                axis("bus", b),
                Axis::new(Some(axis_name.to_string()), labels.clone()),
            ],
        )
        .unwrap();
        let discharge = Labeled::new(
            discharge,
            vec![
                axis("bus", b),
                Axis::new(Some(axis_name.to_string()), labels),
            ],
        )
        .unwrap();
        charge.align(&discharge).expect("aligned");

        // The formulation is built from the ordinal arrays only.
        let row = discharge.inner().expr().expect("discharge expr");
        model.add_row(row.eq(0.0)).expect("rows");
        model.take_snapshot().expect("snapshot")
    };

    assert_eq!(build("time", "t"), build("period", "p"));
}
