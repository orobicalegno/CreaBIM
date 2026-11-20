//! A layered wall meeting another at a T: each layer runs on through the
//! other's weaker layers (insulation through plaster to insulation), and
//! the lines between joined walls' layers inside the mass are thin.

use creabim_core::assembly::{Assembly, AssemblyUse, Layer, LayerFunction as F};
use creabim_core::{Element, ElementKind, Hatch, Level, Material, Model, Transaction, Wall};
use creabim_geom::plan::{Pen, PlanItem};
use glam::DVec2;

fn model() -> (Model, creabim_core::ElementId) {
    let mut doc = creabim_core::Document::new();
    let level = Element::new(
        "L0",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let lid = level.id;
    let paint = Element::new(
        "Paint",
        ElementKind::Material(Material::new(Hatch::Empty, [0.97, 0.97, 0.95])),
    );
    let pir = Element::new(
        "PIR",
        ElementKind::Material(Material::new(Hatch::Cross, [0.95, 0.9, 0.8])),
    );
    let clt = Element::new(
        "CLT",
        ElementKind::Material(Material::new(Hatch::Diagonal, [0.93, 0.85, 0.75])),
    );
    let asm = Assembly::new(
        AssemblyUse::Wall,
        vec![
            Layer::new(F::Finish, Some(paint.id), 0.005),
            Layer::new(F::Insulation, Some(pir.id), 0.06),
            Layer::new(F::Structure, Some(clt.id), 0.1),
            Layer::new(F::Insulation, Some(pir.id), 0.08),
            Layer::new(F::Finish, Some(paint.id), 0.025),
        ],
    );
    let a = Element::new("Muro esterno", ElementKind::Assembly(asm));
    let aid = a.id;
    let mut main = Wall::new(lid, DVec2::new(-2.0, 0.0), DVec2::new(2.0, 0.0), 0.27, 3.0);
    main.assembly = Some(aid);
    let mut branch = Wall::new(lid, DVec2::new(0.0, 0.135), DVec2::new(0.0, 2.0), 0.27, 3.0);
    branch.assembly = Some(aid);
    let tx = Transaction::new("t")
        .insert(level)
        .insert(paint)
        .insert(pir)
        .insert(clt)
        .insert(a)
        .insert(Element::new("A", ElementKind::Wall(main)))
        .insert(Element::new("B", ElementKind::Wall(branch)));
    doc.commit(tx).unwrap();
    (doc.model().clone(), lid)
}

#[test]
fn a_layered_tee_joins_layer_by_layer() {
    let (m, lid) = model();
    let items = creabim_geom::plan::plan_view(&m, lid, 1.2).items;
    if let Ok(path) = std::env::var("TEE_DUMP") {
        let mut out = String::new();
        for it in &items {
            match it {
                PlanItem::Fill { points, rgb, .. } => {
                    let c = rgb.unwrap_or([0.8, 0.8, 0.8]);
                    out += &format!("F {} {} {}", c[0], c[1], c[2]);
                    for p in points {
                        out += &format!(" {} {}", p.x, p.y);
                    }
                    out += "\n";
                }
                PlanItem::Line { pen, a, b, .. } => {
                    out += &format!(
                        "L {} {} {} {} {}\n",
                        if *pen == Pen::Cut { 2 } else { 1 },
                        a.x,
                        a.y,
                        b.x,
                        b.y
                    );
                }
                _ => {}
            }
        }
        std::fs::write(path, out).unwrap();
    }
    // no thick line runs inside the mass: under the branch, between its
    // layers and the main wall's, the lines are thin or merged
    let thick_inside = items.iter().any(|it| match it {
        PlanItem::Line {
            pen: Pen::Cut,
            a,
            b,
            ..
        } => {
            let m = (*a + *b) / 2.0;
            m.x.abs() < 0.12 && m.y > -0.13 && m.y < 0.13
        }
        _ => false,
    });
    assert!(!thick_inside, "a thick line inside the joined walls");
}
