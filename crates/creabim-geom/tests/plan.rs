use creabim_core::*;
use creabim_geom::{plan_view, PlanItem};
use glam::DVec2;

#[test]
fn door_splits_wall_into_two_cut_pieces_with_a_swing() {
    let mut doc = Document::new();
    let l = Element::new(
        "L0",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let w = Element::new(
        "W",
        ElementKind::Wall(Wall {
            level: l.id,
            start: DVec2::ZERO,
            end: DVec2::new(5.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            base_offset: 0.0,
            material: None,
            assembly: None,
            flip: false,
            location: Default::default(),
            top: Default::default(),
            disallow_join: [false; 2],
            attach_top: vec![],
            curtain: None,
            layer_ends: vec![],
            bulge: 0.0,
        }),
    );
    let d = Element::new(
        "D",
        ElementKind::Opening(Opening {
            host: w.id,
            kind: OpeningKind::Door,
            offset: 2.0,
            sill: 0.0,
            width: 1.0,
            height: 2.1,
            material: None,
            frame: Default::default(),
            flip_hand: false,
            flip_facing: false,
            wrap: None,
            frame_at: None,
        }),
    );
    let lid = l.id;
    doc.commit(Transaction::new("t").insert(l).insert(w).insert(d))
        .unwrap();

    let plan = plan_view(doc.model(), lid, 1.2);
    let cuts: Vec<_> = plan
        .items
        .iter()
        .filter(|i| matches!(i, PlanItem::Fill { .. }))
        .collect();
    assert_eq!(cuts.len(), 2);
    assert!(plan.items.iter().any(|i| matches!(i, PlanItem::Arc { .. })));

    let svg = plan.to_svg(100.0);
    assert!(svg.starts_with("<svg"));
    // two filled pieces; their outlines and the swing are polylines
    assert_eq!(svg.matches("<polygon").count(), 2);
    assert!(svg.matches("<polyline").count() >= 8);
}

#[test]
fn a_mass_pushed_out_of_a_wall_face_shows_in_the_plan() {
    let mut m = sample::sample_house();
    let level = m.levels().next().map(|(e, _)| e.id).unwrap();
    let sketch = Sketch::polyline(
        Some(level),
        Workplane::facing(glam::DVec3::new(-0.15, 1.35, 2.5), -glam::DVec3::X),
        vec![
            DVec2::ZERO,
            DVec2::new(0.0, -2.0),
            DVec2::new(1.5, -2.0),
            DVec2::new(1.5, 0.0),
        ],
        true,
    );
    let mass = Element::new(
        "Mass",
        ElementKind::FreeMesh(FreeMesh::new(
            Some(level),
            sketch.plane.origin,
            sketch.extrude(1.0),
        )),
    );
    let id = mass.id;
    let mut doc = Document::from_model(m);
    doc.commit(Transaction::new("t").insert(mass)).unwrap();
    m = doc.model().clone();
    let plan = plan_view(&m, level, 1.2);
    let lines = plan.items.iter().filter(|i| i.id() == id).count();
    assert!(lines >= 4, "{lines} lines");
}

#[test]
fn a_box_subtracted_from_a_wall_leaves_a_gap_in_the_plan() {
    let mut doc = Document::new();
    let l = Element::new(
        "L0",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let w = Element::new(
        "W",
        ElementKind::Wall(Wall {
            level: l.id,
            start: DVec2::ZERO,
            end: DVec2::new(5.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            base_offset: 0.0,
            material: None,
            assembly: None,
            flip: false,
            location: Default::default(),
            top: Default::default(),
            disallow_join: [false; 2],
            attach_top: vec![],
            curtain: None,
            layer_ends: vec![],
            bulge: 0.0,
        }),
    );
    // a 1 m box through the wall, 0.5 to 2.5 m up
    let b = Element::new(
        "Box",
        ElementKind::FreeMesh(FreeMesh::new(
            Some(l.id),
            glam::DVec3::new(2.5, 0.0, 0.5),
            EditMesh::cuboid(glam::DVec3::new(1.0, 1.0, 2.0)),
        )),
    );
    let c = Element::new(
        "Cut",
        ElementKind::Cut(Cut {
            target: w.id,
            cutter: b.id,
            op: BoolOp::Difference,
        }),
    );
    let (lid, wid, bid) = (l.id, w.id, b.id);
    doc.commit(
        Transaction::new("t")
            .insert(l)
            .insert(w)
            .insert(b)
            .insert(c),
    )
    .unwrap();

    let plan = plan_view(doc.model(), lid, 1.2);
    let fills: Vec<_> = plan
        .items
        .iter()
        .filter(|i| matches!(i, PlanItem::Fill { id, .. } if *id == wid))
        .collect();
    assert_eq!(fills.len(), 2, "the wall is cut in two");
    assert!(
        plan.items.iter().all(|i| i.id() != bid),
        "cutters are not drawn"
    );

    // above the box the wall is whole again
    let plan = plan_view(doc.model(), lid, 2.8);
    let fills = plan
        .items
        .iter()
        .filter(|i| matches!(i, PlanItem::Fill { id, .. } if *id == wid))
        .count();
    assert_eq!(fills, 1);
}

#[test]
fn sketch_arcs_draw_as_arcs_in_plans_and_sections() {
    let mut doc = Document::new();
    let l = Element::new(
        "L0",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let level = l.id;
    // a circle on the floor, and a D standing on the section line y = 0
    let circle = Element::new(
        "Circle",
        ElementKind::Sketch(Sketch::circle(
            Some(level),
            Workplane::ground(0.0),
            DVec2::new(2.0, 1.0),
            0.5,
        )),
    );
    let mut d = Sketch::polyline(
        Some(level),
        Workplane::facing(glam::DVec3::new(0.0, 0.0, 0.0), -glam::DVec3::Y),
        vec![DVec2::new(1.0, 0.0), DVec2::new(3.0, 0.0)],
        true,
    );
    d.kinds = vec![SegmentKind::Line, SegmentKind::Arc { bulge: 1.0 }];
    let d = Element::new("D", ElementKind::Sketch(d));
    let (cid, did) = (circle.id, d.id);
    doc.commit(Transaction::new("t").insert(l).insert(circle).insert(d))
        .unwrap();

    let plan = plan_view(doc.model(), level, 1.2);
    let arc = plan.items.iter().find_map(|it| match it {
        PlanItem::Arc {
            id,
            center,
            radius,
            start,
            end,
            ..
        } if *id == cid => Some((*center, *radius, end - start)),
        _ => None,
    });
    let (center, radius, sweep) = arc.expect("the circle is drawn as an arc");
    assert!(center.distance(DVec2::new(2.0, 1.0)) < 1e-9 && (radius - 0.5).abs() < 1e-12);
    assert!((sweep - std::f64::consts::TAU).abs() < 1e-9);
    // the standing D is seen edge on: straight pieces along y = 0
    assert!(plan
        .items
        .iter()
        .any(|it| matches!(it, PlanItem::Line { id, a, b, .. }
        if *id == did && a.y.abs() < 1e-9 && b.y.abs() < 1e-9)));

    // a section along y = -1 looking north sees the D face on, its arc above
    let section = creabim_geom::section::section_view(
        doc.model(),
        DVec2::new(0.0, -1.0),
        DVec2::new(5.0, -1.0),
        5.0,
        true,
        false,
    );
    let half = section.items.iter().find_map(|it| match it {
        PlanItem::Arc {
            id,
            center,
            radius,
            start,
            end,
            ..
        } if *id == did => Some((*center, *radius, *start, *end)),
        _ => None,
    });
    let (center, radius, start, end) = half.expect("the D's arc is drawn in the section");
    assert!(center.distance(DVec2::new(2.0, 0.0)) < 1e-9 && (radius - 1.0).abs() < 1e-9);
    let top = center + DVec2::from_angle((start + end) / 2.0) * radius;
    assert!(top.distance(DVec2::new(2.0, 1.0)) < 1e-9, "{top:?}");
}

/// The area of a polygon.
fn area(points: &[DVec2]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| points[i].perp_dot(points[(i + 1) % n]))
        .sum::<f64>()
        .abs()
        / 2.0
}

#[test]
fn crossing_walls_fill_the_cross_once() {
    let mut doc = Document::new();
    let l = Element::new(
        "L0",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let lid = l.id;
    let a = Element::new(
        "A",
        ElementKind::Wall(Wall::new(lid, DVec2::ZERO, DVec2::new(6.0, 0.0), 0.2, 3.0)),
    );
    let b = Element::new(
        "B",
        ElementKind::Wall(Wall::new(
            lid,
            DVec2::new(3.0, -2.0),
            DVec2::new(3.0, 2.0),
            0.2,
            3.0,
        )),
    );
    doc.commit(Transaction::new("t").insert(l).insert(a).insert(b))
        .unwrap();
    let filled = |m: &Model| -> f64 {
        plan_view(m, lid, 1.2)
            .items
            .iter()
            .filter_map(|i| match i {
                PlanItem::Fill { points, .. } => Some(area(points)),
                _ => None,
            })
            .sum()
    };
    // the crossing square is filled once, not twice
    let expected = 6.0 * 0.2 + 4.0 * 0.2 - 0.2 * 0.2;
    assert!((filled(doc.model()) - expected).abs() < 1e-6);

    // built of layers, the cores still cross and nothing overlaps
    let (tx, ids) = assembly::add_starters(doc.model());
    doc.commit(tx).unwrap();
    let walls: Vec<ElementId> = doc
        .model()
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
        .map(|e| e.id)
        .collect();
    let (tx, _) = assembly::assign(doc.model(), &walls, Some(ids[0]));
    doc.commit(tx).unwrap();
    let t = match &doc.model().get(walls[0]).unwrap().kind {
        ElementKind::Wall(w) => w.thickness,
        _ => unreachable!(),
    };
    let expected = 6.0 * t + 4.0 * t - t * t;
    assert!(
        (filled(doc.model()) - expected).abs() < 1e-6,
        "{} vs {expected}",
        filled(doc.model())
    );
}
