//! Curtain walls: a wall made of a grid of mullions holding glass or solid
//! panels, as Revit's curtain walls or ArchiCAD's curtain wall tool.
//!
//! A curtain wall is an ordinary [`Wall`] with a [`CurtainGrid`], so it is
//! drawn with the Wall tool, joins, moves, has a level and a height like
//! any wall. Its grid lines run up the wall every so far along it (or
//! split it into so many bays) and across it every so far up. Mullions sit
//! on every grid line and round the edge; panels fill the cells between.
//! A door or bare hole hosted in a curtain wall takes the place of the
//! panels it covers, and the mullions through it are left out.
//!
//! This module lays the grid out in the wall's own coordinates (`u` along
//! the wall from its start, `v` up from its base); `creabim-geom` turns the
//! layout into meshes and plans, `creabim-ifc` into IfcMember and IfcPlate.
//!
//! A curtain wall drawn in, or moved into, another wall cuts its own
//! opening there, as a door or window does ([`voids_in`]): the host wall
//! loses the curtain wall's length and height, wherever the curtain wall
//! goes. Nothing is stored for it, so the void always follows.

use crate::{ElementId, ElementKind, Model, Opening, OpeningKind, Wall};
use serde::{Deserialize, Serialize};

/// How a curtain grid divides one direction.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum GridRule {
    /// A grid line every so many metres, from the start (or the base); the
    /// last bay takes what is left.
    Spacing(f64),
    /// So many equal bays.
    Count(u32),
}

impl GridRule {
    /// The grid lines strictly inside `0..len`.
    pub fn lines(&self, len: f64) -> Vec<f64> {
        match *self {
            GridRule::Spacing(s) if s > 1e-3 => {
                let mut out = vec![];
                let mut at = s;
                // a bay thinner than 5 cm at the end joins the one before
                while at < len - 0.05 && out.len() < 500 {
                    out.push(at);
                    at += s;
                }
                out
            }
            GridRule::Spacing(_) => vec![],
            GridRule::Count(n) => {
                let n = n.clamp(1, 200);
                (1..n).map(|i| len * i as f64 / n as f64).collect()
            }
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            GridRule::Spacing(_) => "Spacing",
            GridRule::Count(_) => "Count",
        }
    }
}

/// What fills a curtain wall's cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelKind {
    #[default]
    Glass,
    /// An opaque panel, in the wall's material.
    Solid,
}

impl PanelKind {
    pub const ALL: [PanelKind; 2] = [PanelKind::Glass, PanelKind::Solid];

    pub fn label(self) -> &'static str {
        match self {
            PanelKind::Glass => "Glass",
            PanelKind::Solid => "Solid",
        }
    }
}

/// The grid, mullions and panels of a curtain wall.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurtainGrid {
    /// Grid lines running up the wall, placed along it.
    pub vertical: GridRule,
    /// Grid lines running along the wall, placed up it.
    pub horizontal: GridRule,
    /// Face width of the mullions, seen from the front.
    pub mullion_width: f64,
    /// Depth of the mullions through the wall: the wall's thickness.
    pub mullion_depth: f64,
    #[serde(default)]
    pub panel: PanelKind,
    #[serde(default = "default_panel_thickness")]
    pub panel_thickness: f64,
    /// Cells that take the other kind of panel, as (bay along, row up).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub swapped: Vec<(u32, u32)>,
}

fn default_panel_thickness() -> f64 {
    0.024
}

impl Default for CurtainGrid {
    fn default() -> Self {
        Self {
            vertical: GridRule::Spacing(1.5),
            horizontal: GridRule::Spacing(1.5),
            mullion_width: 0.06,
            mullion_depth: 0.15,
            panel: PanelKind::Glass,
            panel_thickness: default_panel_thickness(),
            swapped: vec![],
        }
    }
}

/// Which way a mullion runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MullionRun {
    Vertical,
    Horizontal,
}

/// A rectangle on a curtain wall's face, in its own coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceRect {
    pub u0: f64,
    pub u1: f64,
    pub v0: f64,
    pub v1: f64,
}

impl FaceRect {
    fn overlaps(&self, o: &FaceRect) -> bool {
        self.u0 < o.u1 - 1e-6
            && o.u0 < self.u1 - 1e-6
            && self.v0 < o.v1 - 1e-6
            && o.v0 < self.v1 - 1e-6
    }
}

/// One mullion: a box `rect` on the face, `depth` through the wall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mullion {
    pub run: MullionRun,
    pub rect: FaceRect,
}

/// One panel filling a cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    pub bay: u32,
    pub row: u32,
    pub kind: PanelKind,
    pub rect: FaceRect,
}

/// A curtain wall laid out: its grid lines, mullions and panels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    /// Grid lines along the wall, ends included.
    pub us: Vec<f64>,
    /// Grid lines up the wall, base and top included.
    pub vs: Vec<f64>,
    pub mullions: Vec<Mullion>,
    pub panels: Vec<Panel>,
}

impl CurtainGrid {
    pub fn validate(&self) -> Result<(), String> {
        let ok = |v: f64| v.is_finite() && v > 0.0;
        if !(ok(self.mullion_width) && ok(self.mullion_depth) && ok(self.panel_thickness)) {
            return Err("curtain wall mullions and panels need a positive size".into());
        }
        for r in [self.vertical, self.horizontal] {
            if let GridRule::Spacing(s) = r {
                if !(s.is_finite() && s >= 0.05) {
                    return Err("curtain grid spacing must be at least 50 mm".into());
                }
            }
        }
        Ok(())
    }

    /// The panel kind of a cell, its swap applied.
    pub fn panel_of(&self, bay: u32, row: u32) -> PanelKind {
        let swapped = self.swapped.contains(&(bay, row));
        match (self.panel, swapped) {
            (PanelKind::Glass, true) => PanelKind::Solid,
            (PanelKind::Solid, true) => PanelKind::Glass,
            (k, false) => k,
        }
    }

    /// The grid laid out on a wall `len` long and `height` high, leaving
    /// room for `holes` (doors and bare openings, in the same coordinates):
    /// no panel where one covers a cell, no mullion through one.
    pub fn layout(&self, len: f64, height: f64, holes: &[FaceRect]) -> Layout {
        let mut us = vec![0.0];
        us.extend(self.vertical.lines(len));
        us.push(len);
        let mut vs = vec![0.0];
        vs.extend(self.horizontal.lines(height));
        vs.push(height);
        let half = self.mullion_width / 2.0;
        // edge mullions sit inside the wall's outline
        let span = |xs: &[f64], i: usize, end: f64| -> (f64, f64) {
            let x = xs[i];
            if i == 0 {
                (0.0, self.mullion_width.min(end))
            } else if i == xs.len() - 1 {
                ((end - self.mullion_width).max(0.0), end)
            } else {
                (x - half, x + half)
            }
        };
        let mut mullions = vec![];
        // a mullion piece, less what holes take out of it
        let mut push = |run: MullionRun, r: FaceRect| {
            let mut pieces = vec![r];
            for h in holes {
                let mut next = vec![];
                for p in pieces {
                    if !p.overlaps(h) {
                        next.push(p);
                        continue;
                    }
                    match run {
                        MullionRun::Vertical => {
                            if h.v0 > p.v0 + 1e-6 {
                                next.push(FaceRect { v1: h.v0, ..p });
                            }
                            if h.v1 < p.v1 - 1e-6 {
                                next.push(FaceRect { v0: h.v1, ..p });
                            }
                        }
                        MullionRun::Horizontal => {
                            if h.u0 > p.u0 + 1e-6 {
                                next.push(FaceRect { u1: h.u0, ..p });
                            }
                            if h.u1 < p.u1 - 1e-6 {
                                next.push(FaceRect { u0: h.u1, ..p });
                            }
                        }
                    }
                }
                pieces = next;
            }
            for p in pieces {
                mullions.push(Mullion { run, rect: p });
            }
        };
        for i in 0..us.len() {
            let (u0, u1) = span(&us, i, len);
            push(
                MullionRun::Vertical,
                FaceRect {
                    u0,
                    u1,
                    v0: 0.0,
                    v1: height,
                },
            );
        }
        // horizontal mullions run between the vertical ones
        let mut panels = vec![];
        for bay in 0..us.len() - 1 {
            let u0 = span(&us, bay, len).1;
            let u1 = span(&us, bay + 1, len).0;
            if u1 <= u0 + 1e-6 {
                continue;
            }
            for j in 0..vs.len() {
                let (v0, v1) = span(&vs, j, height);
                push(MullionRun::Horizontal, FaceRect { u0, u1, v0, v1 });
            }
            for row in 0..vs.len() - 1 {
                let v0 = span(&vs, row, height).1;
                let v1 = span(&vs, row + 1, height).0;
                if v1 <= v0 + 1e-6 {
                    continue;
                }
                let rect = FaceRect { u0, u1, v0, v1 };
                if holes.iter().any(|h| h.overlaps(&rect)) {
                    continue;
                }
                panels.push(Panel {
                    bay: bay as u32,
                    row: row as u32,
                    kind: self.panel_of(bay as u32, row as u32),
                    rect,
                });
            }
        }
        Layout {
            us,
            vs,
            mullions,
            panels,
        }
    }

    /// The layout of a curtain wall, with its doors and holes.
    pub fn wall_layout(&self, wall: &Wall, openings: &[&Opening]) -> Layout {
        let holes: Vec<FaceRect> = openings
            .iter()
            .map(|o| FaceRect {
                u0: o.offset,
                u1: o.offset + o.width,
                v0: o.sill,
                v1: o.sill + o.height,
            })
            .collect();
        self.layout(wall.length(), wall.height, &holes)
    }
}

/// How far off a host wall's direction a curtain wall may run and still
/// sit in it (the sine of about one degree).
const PARALLEL: f64 = 0.0175;

/// The opening curtain wall `cw` cuts in wall `host`, if it sits in it:
/// running along it, its centreline within the host's thickness, and
/// overlapping it along its length and up its height. The opening is
/// where they overlap, in the host's own coordinates.
/// [`void_of`] for curved walls: only a curtain wall on the same arc as
/// its host sits in it (the other half of a circle shares the host's chord
/// line, and cut it through); the hole runs round the arc.
fn arc_void(model: &Model, host_id: ElementId, host: &Wall, cw: &Wall) -> Option<Opening> {
    use crate::sketch::Curve;
    let (
        Curve::Arc {
            center: c1,
            radius: r1,
            ..
        },
        Curve::Arc {
            center: c2,
            radius: r2,
            ..
        },
    ) = (host.curve(), cw.curve())
    else {
        return None;
    };
    if c1.distance(c2) > 1e-3 || (r1 - r2).abs() > host.thickness / 2.0 + 1e-3 {
        return None;
    }
    // its middle on the host's stretch of the circle, not past its ends
    let mid = cw.place(cw.length() / 2.0, 0.0);
    if host.along(mid).1 > host.thickness / 2.0 + 1e-3 {
        return None;
    }
    let (a, b) = (host.along(cw.start).0, host.along(cw.end).0);
    let (u0, u1) = (a.min(b), a.max(b));
    let (h0, c0) = (
        crate::walls::base_z(model, host),
        crate::walls::base_z(model, cw),
    );
    let v0 = c0.max(h0) - h0;
    let v1 = (c0 + cw.height).min(h0 + host.height) - h0;
    if u1 - u0 < 0.05 || v1 - v0 < 0.05 {
        return None;
    }
    Some(Opening {
        host: host_id,
        kind: OpeningKind::Hole,
        offset: u0,
        sill: v0,
        width: u1 - u0,
        height: v1 - v0,
        material: None,
        frame: Default::default(),
        flip_hand: false,
        flip_facing: false,
        wrap: None,
        frame_at: None,
    })
}

pub fn void_of(model: &Model, host_id: ElementId, host: &Wall, cw: &Wall) -> Option<Opening> {
    if cw.curtain.is_none() || host.curtain.is_some() || host.length() < 1e-6 {
        return None;
    }
    if host.is_arc() || cw.is_arc() {
        return arc_void(model, host_id, host, cw);
    }
    let d = host.direction();
    let e = cw.direction();
    if d.perp_dot(e).abs() > PARALLEL {
        return None;
    }
    let mid = (cw.start + cw.end) / 2.0;
    let off = (mid - host.start).perp_dot(d).abs();
    if off > host.thickness / 2.0 + 1e-3 {
        return None;
    }
    let along = |p: glam::DVec2| (p - host.start).dot(d);
    let (a, b) = (along(cw.start), along(cw.end));
    let u0 = a.min(b).max(0.0);
    let u1 = a.max(b).min(host.length());
    let (h0, c0) = (
        crate::walls::base_z(model, host),
        crate::walls::base_z(model, cw),
    );
    let v0 = c0.max(h0) - h0;
    let v1 = (c0 + cw.height).min(h0 + host.height) - h0;
    if u1 - u0 < 0.05 || v1 - v0 < 0.05 {
        return None;
    }
    Some(Opening {
        host: host_id,
        kind: OpeningKind::Hole,
        offset: u0,
        sill: v0,
        width: u1 - u0,
        height: v1 - v0,
        material: None,
        frame: Default::default(),
        flip_hand: false,
        flip_facing: false,
        wrap: None,
        frame_at: None,
    })
}

/// The openings the curtain walls sitting in wall `host` cut in it, each
/// with the curtain wall that fills it.
pub fn voids_in(model: &Model, host: ElementId) -> Vec<(ElementId, Opening)> {
    let Some(ElementKind::Wall(h)) = model.get(host).map(|e| &e.kind) else {
        return vec![];
    };
    if h.curtain.is_some() {
        return vec![];
    }
    model
        .iter()
        .filter(|e| e.id != host)
        .filter_map(|e| match &e.kind {
            ElementKind::Wall(cw) => Some((e.id, void_of(model, host, h, cw)?)),
            _ => None,
        })
        .collect()
}

/// The walls curtain wall `cw` sits in, each with the opening it cuts.
pub fn hosts_of(model: &Model, cw: ElementId) -> Vec<(ElementId, Opening)> {
    let Some(ElementKind::Wall(c)) = model.get(cw).map(|e| &e.kind) else {
        return vec![];
    };
    if c.curtain.is_none() {
        return vec![];
    }
    model
        .iter()
        .filter(|e| e.id != cw)
        .filter_map(|e| match &e.kind {
            ElementKind::Wall(h) => Some((e.id, void_of(model, e.id, h, c)?)),
            _ => None,
        })
        .collect()
}

/// Every opening through wall `id` as built: its doors, windows and holes,
/// then the voids the curtain walls in it cut.
pub fn wall_openings(model: &Model, id: ElementId) -> Vec<Opening> {
    let mut out: Vec<Opening> = model
        .dependents(id)
        .into_iter()
        .filter_map(|d| match &model.get(d)?.kind {
            ElementKind::Opening(o) => Some(o.clone()),
            _ => None,
        })
        .collect();
    out.extend(voids_in(model, id).into_iter().map(|(_, o)| o));
    // a screen wall's holes
    out.extend(crate::screen::screen_openings(model, id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curtain_half_of_a_circle_cuts_nothing_from_the_solid_half() {
        use crate::{Element, Level};
        use glam::DVec2;
        let lv = Element::new(
            "L0",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let (a, b) = (DVec2::new(-3.0, 0.0), DVec2::new(3.0, 0.0));
        let mut solid = Wall::new(lv.id, a, b, 0.3, 3.0);
        solid.bulge = 1.0;
        let mut glass = Wall::new(lv.id, b, a, 0.15, 3.0);
        glass.bulge = 1.0;
        glass.curtain = Some(CurtainGrid::default());
        let (s, g) = (
            Element::new("Wall", ElementKind::Wall(solid)),
            Element::new("Glass", ElementKind::Wall(glass)),
        );
        let sid = s.id;
        let m = Model::from_elements([lv, s, g]).unwrap();
        assert!(voids_in(&m, sid).is_empty());
    }

    /// A level with a 10 m wall along x and a 3 m curtain wall at `at`
    /// along it, `off` to its side.
    fn wall_and_curtain(at: f64, off: f64) -> (Model, ElementId, ElementId) {
        use crate::{Element, Level};
        use glam::DVec2;
        let lv = Element::new(
            "L0",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let host = Element::new(
            "Wall",
            ElementKind::Wall(Wall::new(
                lv.id,
                DVec2::ZERO,
                DVec2::new(10.0, 0.0),
                0.3,
                3.0,
            )),
        );
        let mut w = Wall::new(
            lv.id,
            DVec2::new(at, off),
            DVec2::new(at + 3.0, off),
            0.15,
            2.5,
        );
        w.curtain = Some(CurtainGrid::default());
        let cw = Element::new("Storefront", ElementKind::Wall(w));
        let (h, c) = (host.id, cw.id);
        (Model::from_elements([lv, host, cw]).unwrap(), h, c)
    }

    #[test]
    fn a_curtain_wall_in_a_wall_cuts_its_own_opening_which_follows_it() {
        let (m, host, cw) = wall_and_curtain(2.0, 0.05);
        let v = voids_in(&m, host);
        assert_eq!(v.len(), 1);
        let (filler, o) = &v[0];
        assert_eq!(*filler, cw);
        assert!((o.offset - 2.0).abs() < 1e-9 && (o.width - 3.0).abs() < 1e-9);
        assert!(o.sill.abs() < 1e-9 && (o.height - 2.5).abs() < 1e-9);
        assert_eq!(hosts_of(&m, cw).len(), 1);
        assert_eq!(wall_openings(&m, host).len(), 1);
        // moved along, the opening goes with it; past the end it is cut short
        let (m, host, _) = wall_and_curtain(8.5, 0.0);
        let o = &voids_in(&m, host)[0].1;
        assert!((o.offset - 8.5).abs() < 1e-9 && (o.width - 1.5).abs() < 1e-9);
        // out of the wall, or beside it, it stands on its own
        let (m, host, cw) = wall_and_curtain(2.0, 0.6);
        assert!(voids_in(&m, host).is_empty() && hosts_of(&m, cw).is_empty());
        let (m, host, _) = wall_and_curtain(12.0, 0.0);
        assert!(voids_in(&m, host).is_empty());
        // a curtain wall hosts no curtain-wall voids itself
        let (m, _, cw) = wall_and_curtain(2.0, 0.0);
        assert!(voids_in(&m, cw).is_empty());
    }

    #[test]
    fn spacing_and_count_place_grid_lines() {
        assert_eq!(GridRule::Spacing(1.5).lines(4.0), vec![1.5, 3.0]);
        // a last bay of 3 cm joins the one before
        assert_eq!(GridRule::Spacing(1.0).lines(3.03), vec![1.0, 2.0]);
        assert_eq!(GridRule::Count(4).lines(4.0), vec![1.0, 2.0, 3.0]);
        assert!(GridRule::Count(1).lines(4.0).is_empty());
    }

    #[test]
    fn a_grid_has_mullions_round_and_through_and_a_panel_per_cell() {
        let g = CurtainGrid {
            vertical: GridRule::Count(3),
            horizontal: GridRule::Count(2),
            ..Default::default()
        };
        let l = g.layout(3.0, 3.0, &[]);
        assert_eq!(l.panels.len(), 6);
        // 4 uprights, and 3 rows of transoms in each of 3 bays
        assert_eq!(l.mullions.len(), 4 + 9);
        let p = l.panels[0].rect;
        assert!((p.u0 - 0.06).abs() < 1e-9 && (p.u1 - 0.97).abs() < 1e-9);
        assert!(l.panels.iter().all(|p| p.kind == PanelKind::Glass));
    }

    #[test]
    fn a_door_takes_the_panels_and_mullions_it_covers() {
        let g = CurtainGrid {
            vertical: GridRule::Count(3),
            horizontal: GridRule::Count(2),
            swapped: vec![(2, 1)],
            ..Default::default()
        };
        // a door in the middle bay, through the transom at 1.5 m
        let door = FaceRect {
            u0: 1.03,
            u1: 1.97,
            v0: 0.0,
            v1: 2.1,
        };
        let l = g.layout(3.0, 3.0, &[door]);
        assert_eq!(l.panels.len(), 4);
        assert!(!l.mullions.iter().any(|m| m.rect.overlaps(&door)));
        assert_eq!(
            l.panels
                .iter()
                .find(|p| (p.bay, p.row) == (2, 1))
                .unwrap()
                .kind,
            PanelKind::Solid
        );
    }

    #[test]
    fn walls_from_older_files_are_not_curtain_walls() {
        let level = "00000000-0000-0000-0000-000000000001";
        let wall = format!(
            r#"{{"level":"{level}","start":[0.0,0.0],"end":[4.0,0.0],"thickness":0.2,"height":3.0,"base_offset":0.0}}"#
        );
        let w: Wall = serde_json::from_str(&wall).unwrap();
        assert!(w.curtain.is_none());
        assert!(!serde_json::to_string(&w).unwrap().contains("curtain"));
        let mut w = w;
        w.curtain = Some(CurtainGrid::default());
        let back: Wall = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        assert_eq!(back, w);
    }
}
