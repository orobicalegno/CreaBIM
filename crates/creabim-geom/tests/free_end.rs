//! A layered wall's free end wraps its outer layers round it when its
//! assembly says so.

use creabim_core::assembly::{Assembly, AssemblyUse, Layer, LayerFunction as F, WrapSides};
use creabim_core::{Element, ElementKind, Hatch, Level, Material, Transaction, Wall};
use creabim_geom::plan::{Pen, PlanItem};
use glam::DVec2;

#[test]
fn a_free_end_is_closed_by_its_wrapping_layers() {
    let mut doc = creabim_core::Document::new();
    let level = Element::new(
        "L0",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let lid = level.id;
    let render = Element::new(
        "Render",
        ElementKind::Material(Material::new(Hatch::Empty, [0.95, 0.95, 0.95])),
    );
    let ins = Element::new(
        "Wool",
        ElementKind::Material(Material::new(Hatch::Cross, [0.95, 0.9, 0.6])),
    );
    let block = Element::new(
        "Block",
        ElementKind::Material(Material::new(Hatch::Diagonal, [0.8, 0.8, 0.8])),
    );
    let mut insl = Layer::new(F::Insulation, Some(ins.id), 0.15);
    insl.wraps = Some(true);
    let mut asm = Assembly::new(
        AssemblyUse::Wall,
        vec![
            Layer::new(F::Finish, Some(render.id), 0.02),
            insl,
            Layer::new(F::Structure, Some(block.id), 0.2),
            Layer::new(F::Finish, Some(render.id), 0.015),
        ],
    );
    asm.wrap_ends = WrapSides::Exterior;
    let a = Element::new("Ext", ElementKind::Assembly(asm));
    let aid = a.id;
    let mut w = Wall::new(lid, DVec2::new(0.0, 0.0), DVec2::new(0.0, 3.0), 0.385, 3.0);
    w.assembly = Some(aid);
    let tx = Transaction::new("t")
        .insert(level)
        .insert(render)
        .insert(ins)
        .insert(block)
        .insert(a)
        .insert(Element::new("W", ElementKind::Wall(w)));
    doc.commit(tx).unwrap();
    let items = creabim_geom::plan::plan_view(doc.model(), lid, 1.2).items;
    if let Ok(path) = std::env::var("END_DUMP") {
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
    // the core no longer reaches the end: the insulation's return covers it
    let block_fill_reaches_end = items.iter().any(|it| match it {
        PlanItem::Fill {
            points,
            rgb: Some(c),
            ..
        } if (c[0] - 0.8).abs() < 0.01 => points.iter().any(|p| p.y > 3.0 - 1e-6),
        _ => false,
    });
    assert!(!block_fill_reaches_end, "the block runs to the end, square");
    // in 3D the layers still fill the wall, closed at its end
    let scene = creabim_geom::scene::build_scene(doc.model());
    let vol: f64 = scene.iter().map(|i| i.mesh.volume()).sum();
    let full = 0.385 * 3.0 * 3.0;
    assert!((vol - full).abs() < full * 0.02, "layers {vol} of {full}");
    // the block stops short of the end by the returns over it
    let block_top = scene
        .iter()
        .filter(|i| i.layer.is_some_and(|l| l.index == 2))
        .map(|i| i.mesh.bounds().unwrap().1.y)
        .fold(f32::MIN, f32::max);
    assert!(block_top < 3.0 - 0.1, "block reaches {block_top}");
    for i in &scene {
        let (lo, hi) = i.mesh.bounds().unwrap();
        assert!(
            hi.y <= 3.0 + 1e-4 && lo.y >= -1e-4,
            "a layer past the wall: {lo} {hi}"
        );
    }
}
