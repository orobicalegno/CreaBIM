//! Layered walls, floors and roofs in drawings and 3D: every layer is cut
//! with its own material's hatch, with a thin line between layers, layers
//! of the same material merge across a corner, and 3D has a solid per
//! layer (air gaps left empty).

use creabim_core::{
    assembly_of, sample::layered_house, ElementId, ElementKind, Hatch, LayerFunction, Model,
};
use creabim_geom::plan::{plan_view, Pen, PlanItem};
use creabim_geom::scene::{build_scene, element_scene};
use creabim_geom::section::section_view;
use glam::DVec2;

fn id_of(m: &Model, name: &str) -> ElementId {
    m.iter().find(|e| e.name == name).unwrap().id
}

fn ground(m: &Model) -> ElementId {
    m.levels()
        .find(|(e, _)| e.name == "Ground Floor")
        .unwrap()
        .0
        .id
}

#[test]
fn a_layered_wall_has_a_solid_per_layer_but_its_air_gap() {
    let m = layered_house();
    let wall = id_of(&m, "GF Wall 1");
    let scene = build_scene(&m);
    let parts: Vec<_> = scene
        .iter()
        .filter(|i| i.id == wall && !i.is_lining())
        .collect();
    assert_eq!(parts.len(), 4, "five layers, one an air gap");
    assert!(parts.iter().all(|p| p.layer.is_some()));
    assert!(parts
        .iter()
        .all(|p| p.layer.unwrap().function != LayerFunction::AirGap));
    // each layer in its material's colour
    let brick = parts.iter().find(|p| p.layer.unwrap().index == 0).unwrap();
    let block = parts.iter().find(|p| p.layer.unwrap().index == 3).unwrap();
    assert_ne!(brick.color, block.color);
    // exports take one shape per element
    let merged = element_scene(&m);
    assert_eq!(merged.iter().filter(|i| i.id == wall).count(), 1);
    // the layers fill the wall but for its cavity
    let e = m.get(wall).unwrap();
    let (ElementKind::Wall(w), Some((_, a))) = (&e.kind, assembly_of(&m, e)) else {
        unreachable!()
    };
    let solid: f64 = parts.iter().map(|p| p.mesh.volume()).sum();
    let cavity = a.layers[1].thickness / a.total();
    let gross = merged.iter().find(|i| i.id == wall).unwrap().mesh.volume();
    assert!((solid - gross).abs() < 1e-3);
    assert!(solid < w.length() * w.height * w.thickness * (1.0 - cavity) * 1.1);
}

#[test]
fn plans_cut_each_layer_with_its_hatch() {
    let m = layered_house();
    let plan = plan_view(&m, ground(&m), 1.2);
    let wall = id_of(&m, "GF Wall 1");
    let fills: Vec<Hatch> = plan
        .items
        .iter()
        .filter_map(|i| match i {
            PlanItem::Fill { id, hatch, .. } if *id == wall => Some(*hatch),
            _ => None,
        })
        .collect();
    // brick, the empty cavity, insulation, block and plaster each have a fill
    assert!(fills.contains(&Hatch::Brick), "{fills:?}");
    assert!(fills.contains(&Hatch::Empty), "{fills:?}");
    assert!(fills.len() >= 5, "{fills:?}");
    // thin lines between layers of the one wall, heavy ones round it
    let pens = |pen: Pen| {
        plan.items
            .iter()
            .filter(|i| matches!(i, PlanItem::Line { id, pen: p, .. } if *id == wall && *p == pen))
            .count()
    };
    assert!(pens(Pen::Projection) >= 4, "{}", pens(Pen::Projection));
    assert!(pens(Pen::Cut) >= 2);
}

#[test]
fn a_corner_joins_layer_by_layer() {
    let m = layered_house();
    let plan = plan_view(&m, ground(&m), 1.2);
    let (a, b) = (id_of(&m, "GF Wall 1"), id_of(&m, "GF Wall 2"));
    // at the corner (10, 0) the brick faces of both walls meet on the mitre:
    // no heavy line runs across the brick there
    let corner = DVec2::new(10.0, 0.0);
    let across_brick = plan.items.iter().any(|i| match i {
        PlanItem::Line {
            id,
            pen: Pen::Cut,
            a: p,
            b: q,
        } if *id == a || *id == b => {
            // a short line near the outer corner, along the mitre
            let mid = (*p + *q) / 2.0;
            mid.distance(corner + DVec2::new(0.14, -0.14)) < 0.03
        }
        _ => false,
    });
    assert!(!across_brick, "the brick leaves merge round the corner");
}

#[test]
fn sections_cut_floors_walls_and_roofs_layer_by_layer() {
    let m = layered_house();
    // across the house, through both floors, the walls and the roof
    let d = section_view(
        &m,
        DVec2::new(3.0, -2.0),
        DVec2::new(3.0, 8.0),
        20.0,
        true,
        false,
    );
    let roof = m
        .iter()
        .find(|e| matches!(e.kind, ElementKind::Roof(_)))
        .unwrap()
        .id;
    let slab = id_of(&m, "GF Slab");
    for (id, at_least) in [(roof, 4), (slab, 4), (id_of(&m, "GF Wall 1"), 4)] {
        let n = d
            .items
            .iter()
            .filter(|i| matches!(i, PlanItem::Fill { id: f, .. } if *f == id))
            .count();
        assert!(n >= at_least, "{n} fills");
    }
}

#[test]
fn layers_wrap_into_the_jambs_of_openings_in_plan() {
    use creabim_core::{Document, Transaction, WrapSides};
    let mut doc = Document::from_model(layered_house());
    let wall = id_of(doc.model(), "GF Wall 1");
    let fills = |m: &Model| -> Vec<Vec<DVec2>> {
        plan_view(m, ground(m), 1.2)
            .items
            .iter()
            .filter_map(|i| match i {
                PlanItem::Fill { id, points, .. } if *id == wall => Some(points.clone()),
                _ => None,
            })
            .collect()
    };
    let square = fills(doc.model());
    // the wall's assembly wraps on both faces
    let e = doc.model().get(wall).unwrap();
    let (asm, _) = assembly_of(doc.model(), e).unwrap();
    let mut wrapped = asm.clone();
    if let ElementKind::Assembly(a) = &mut wrapped.kind {
        a.wrap_openings = WrapSides::Both;
    }
    let tx = Transaction::new("Wrap").replace(asm.clone(), wrapped);
    doc.commit(tx).unwrap();
    let wraps = fills(doc.model());
    // each jamb gains the returns of the finishes on both faces
    assert!(
        wraps.len() > square.len(),
        "{} {}",
        wraps.len(),
        square.len()
    );
    // and nothing is lost or doubled: the wall's cut area is the same
    let area = |ps: &Vec<Vec<DVec2>>| -> f64 {
        ps.iter()
            .map(|p| {
                (0..p.len())
                    .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
                    .sum::<f64>()
                    .abs()
                    / 2.0
            })
            .sum()
    };
    assert!((area(&wraps) - area(&square)).abs() < 1e-6);
    // the 3D view lines those reveals with the layers themselves
    let scene = build_scene(doc.model());
    assert!(scene.iter().any(|i| i.id == wall && i.layer.is_some()));
}
