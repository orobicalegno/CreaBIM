//! Curtain walls built from their grid (see [`creabim_core::curtain`]):
//! every mullion and panel a box in the wall's own frame, so the 3D view,
//! plans, sections and the IFC export share one description.

use crate::plan::{Pen, PlanItem};
use crate::TriMesh;
use creabim_core::curtain::{CurtainGrid, PanelKind};
use creabim_core::{ElementId, Opening, Wall};
use glam::{DVec2, DVec3};

/// What a box of a curtain wall is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CurtainPart {
    Mullion,
    Glass,
    /// An opaque panel.
    Panel,
}

/// One part as a box in wall coordinates: `x` along the wall from its
/// start, `y` across it from its centreline (left positive), `z` up from
/// its base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurtainBox {
    pub part: CurtainPart,
    pub lo: DVec3,
    pub hi: DVec3,
}

/// Every mullion and panel of a curtain wall, leaving room for its doors
/// and holes.
pub fn curtain_boxes(wall: &Wall, grid: &CurtainGrid, openings: &[&Opening]) -> Vec<CurtainBox> {
    let layout = grid.wall_layout(wall, openings);
    let md = grid.mullion_depth.min(wall.thickness.max(0.01)) / 2.0;
    let pt = grid.panel_thickness.min(md * 2.0) / 2.0;
    let mut out = Vec::new();
    for m in &layout.mullions {
        let r = m.rect;
        out.push(CurtainBox {
            part: CurtainPart::Mullion,
            lo: DVec3::new(r.u0, -md, r.v0),
            hi: DVec3::new(r.u1, md, r.v1),
        });
    }
    for p in &layout.panels {
        let r = p.rect;
        out.push(CurtainBox {
            part: match p.kind {
                PanelKind::Glass => CurtainPart::Glass,
                PanelKind::Solid => CurtainPart::Panel,
            },
            lo: DVec3::new(r.u0, -pt, r.v0),
            hi: DVec3::new(r.u1, pt, r.v1),
        });
    }
    out.retain(|b| (b.hi - b.lo).min_element() > 1e-6);
    out
}

/// Wall coordinates to model coordinates for a wall on a level at `level_z`.
pub fn to_model(wall: &Wall, level_z: f64) -> impl Fn(DVec3) -> DVec3 {
    let d = wall.direction();
    let n = DVec2::new(-d.y, d.x);
    let start = wall.start;
    let z0 = level_z + wall.base_offset;
    move |p: DVec3| (start + d * p.x + n * p.y).extend(z0 + p.z)
}

/// Wall coordinates to model coordinates for one box of the wall, from
/// `u0` to `u1` along it: on a curved wall the box runs straight along the
/// chord of its stretch of arc, as curved curtain walls are built (flat
/// glass between mullions set round the curve). Its length along the wall
/// maps onto the chord, so neighbours meet at the mullions.
pub fn box_to_model(wall: &Wall, u0: f64, u1: f64, level_z: f64) -> impl Fn(DVec3) -> DVec3 {
    let (a, c) = (wall.place(u0, 0.0), wall.place(u1, 0.0));
    let d = (c - a)
        .try_normalize()
        .unwrap_or_else(|| wall.tangent_at(u0));
    let n = DVec2::new(-d.y, d.x);
    let k = if u1 - u0 > 1e-9 {
        a.distance(c) / (u1 - u0)
    } else {
        1.0
    };
    let z0 = level_z + wall.base_offset;
    move |p: DVec3| (a + d * ((p.x - u0) * k) + n * p.y).extend(z0 + p.z)
}

/// The curtain wall's parts as meshes in model coordinates, one per kind.
pub fn curtain_meshes(
    wall: &Wall,
    grid: &CurtainGrid,
    openings: &[&Opening],
    level_z: f64,
) -> Vec<(CurtainPart, TriMesh)> {
    let mut out: Vec<(CurtainPart, TriMesh)> = Vec::new();
    for b in curtain_boxes(wall, grid, openings) {
        let at = box_to_model(wall, b.lo.x, b.hi.x, level_z);
        let i = match out.iter().position(|(p, _)| *p == b.part) {
            Some(i) => i,
            None => {
                out.push((b.part, TriMesh::default()));
                out.len() - 1
            }
        };
        crate::opening::add_box(&mut out[i].1, b.lo, b.hi, &at);
    }
    out
}

/// A curtain wall in a plan cut `cut_v` above its base: the mullions cut
/// (filled), glass as a thin pair of lines, solid panels cut.
pub fn curtain_plan(
    id: ElementId,
    wall: &Wall,
    grid: &CurtainGrid,
    openings: &[&Opening],
    cut_v: f64,
) -> Vec<PlanItem> {
    let mut out = Vec::new();
    for b in curtain_boxes(wall, grid, openings) {
        if !(b.lo.z < cut_v && cut_v < b.hi.z) {
            continue;
        }
        let m = box_to_model(wall, b.lo.x, b.hi.x, 0.0);
        let at = |u: f64, s: f64| m(DVec3::new(u, s, 0.0)).truncate();
        let points = vec![
            at(b.lo.x, b.lo.y),
            at(b.hi.x, b.lo.y),
            at(b.hi.x, b.hi.y),
            at(b.lo.x, b.hi.y),
        ];
        let pen = match b.part {
            CurtainPart::Glass => Pen::Symbol,
            CurtainPart::Mullion | CurtainPart::Panel => Pen::Cut,
        };
        out.push(PlanItem::Polygon { id, pen, points });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{GridRule, OpeningKind};

    fn wall() -> Wall {
        let mut w = Wall::new(
            ElementId::new(),
            DVec2::ZERO,
            DVec2::new(3.0, 0.0),
            0.15,
            3.0,
        );
        w.curtain = Some(CurtainGrid {
            vertical: GridRule::Count(3),
            horizontal: GridRule::Count(2),
            ..Default::default()
        });
        w
    }

    #[test]
    fn mullions_and_glass_fill_the_wall() {
        let w = wall();
        let g = w.curtain.clone().unwrap();
        let parts = curtain_meshes(&w, &g, &[], 0.0);
        let glass = parts.iter().find(|p| p.0 == CurtainPart::Glass).unwrap();
        // six panes of glass between 60 mm mullions
        // (edge mullions are a whole width inside the wall, inner ones centred)
        let widths = 0.91 + 0.94 + 0.91;
        let area = widths * 2.0 * 1.41;
        assert!((glass.1.volume() - area * 0.024).abs() < 1e-4);
        let (lo, hi) = parts[0].1.bounds().unwrap();
        assert!(lo.x.abs() < 1e-6 && (hi.x - 3.0).abs() < 1e-6 && (hi.z - 3.0).abs() < 1e-6);
        // the plan at 1.2 m cuts four uprights and three panes
        let plan = curtain_plan(ElementId::new(), &w, &g, &[], 1.2);
        let cut = plan
            .iter()
            .filter(|i| matches!(i, PlanItem::Polygon { pen: Pen::Cut, .. }))
            .count();
        assert_eq!((cut, plan.len()), (4, 7));
    }

    #[test]
    fn a_curved_curtain_wall_sets_its_mullions_round_the_arc() {
        // a half circle of radius 1.5 round (1.5, 0)
        let mut w = wall();
        w.bulge = 1.0;
        let g = w.curtain.clone().unwrap();
        let c = DVec2::new(1.5, 0.0);
        for b in curtain_boxes(&w, &g, &[]) {
            let at = box_to_model(&w, b.lo.x, b.hi.x, 0.0);
            // each box's two ends, on its centreline, sit on the arc
            for x in [b.lo.x, b.hi.x] {
                let p = at(DVec3::new(x, 0.0, 0.0)).truncate();
                assert!((p.distance(c) - 1.5).abs() < 1e-9, "{p}");
            }
        }
    }

    #[test]
    fn a_door_replaces_its_panel() {
        let w = wall();
        let g = w.curtain.clone().unwrap();
        let door = Opening {
            host: ElementId::new(),
            kind: OpeningKind::Door,
            offset: 1.03,
            sill: 0.0,
            width: 0.94,
            height: 2.1,
            material: None,
            frame: Default::default(),
            flip_hand: false,
            flip_facing: false,
            wrap: None,
        };
        let boxes = curtain_boxes(&w, &g, &[&door]);
        let glass = boxes
            .iter()
            .filter(|b| b.part == CurtainPart::Glass)
            .count();
        assert_eq!(glass, 4);
    }
}
