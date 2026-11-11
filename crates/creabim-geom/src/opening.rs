//! Doors and windows built inside their wall openings: a frame round the
//! edge, glass set back in it (split into panes by glazing bars), and for a
//! door a leaf with a handle. Every part is a box in the opening's own
//! frame, so the 3D view, drawings and IFC export share one description.

use crate::wall::Cut;
use crate::TriMesh;
use creabim_core::{Opening, OpeningFrame, OpeningKind, Wall};
use glam::{DVec2, DVec3};

/// What a part of a door or window is, which decides how it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    /// Frame members and glazing bars, in the opening's material.
    Frame,
    /// A door's leaf, in the opening's material.
    Leaf,
    Glass,
    Handle,
}

/// One part as a box in opening coordinates: `x` along the wall from the
/// opening's left edge, `y` across the wall from its centre line (towards
/// the wall's left, where a door swings), `z` up from the opening's bottom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartBox {
    pub part: Part,
    pub lo: DVec3,
    pub hi: DVec3,
}

/// Glass thickness, kept thin but solid so drawings show it.
const GLASS: f64 = 0.02;

/// The opening as built: the rectangle it fills in the wall (clipped to
/// the wall) and its frame fitted to that size.
#[derive(Clone, Copy, Debug)]
pub struct Fitted {
    pub cut: Cut,
    pub frame: OpeningFrame,
}

impl Fitted {
    pub fn of(o: &Opening, wall: &Wall) -> Option<Fitted> {
        let cut = Cut::of(o, wall)?;
        let frame = o
            .frame
            .fitted(cut.u1 - cut.u0, cut.v1 - cut.v0, wall.thickness);
        Some(Fitted { cut, frame })
    }

    pub fn width(&self) -> f64 {
        self.cut.u1 - self.cut.u0
    }

    pub fn height(&self) -> f64 {
        self.cut.v1 - self.cut.v0
    }

    /// A door leaf's thickness.
    pub fn leaf(&self) -> f64 {
        (self.frame.depth * 0.6).min(0.045)
    }
}

/// Every part of a door or window, in opening coordinates.
pub fn opening_boxes(o: &Opening, wall: &Wall) -> Vec<PartBox> {
    let Some(f) = Fitted::of(o, wall) else {
        return vec![];
    };
    // a bare hole has nothing in it
    if o.kind == OpeningKind::Hole {
        return vec![];
    }
    let (w, h) = (f.width(), f.height());
    let fw = f.frame.width;
    let hd = f.frame.depth / 2.0;
    let face = wall.thickness / 2.0;
    let mut out = Vec::new();
    let mut add = |part, lo: DVec3, hi: DVec3| {
        if (hi - lo).min_element() > 1e-6 {
            out.push(PartBox { part, lo, hi });
        }
    };
    let b = DVec3::new;
    // jambs and head, round every opening
    add(Part::Frame, b(0.0, -hd, 0.0), b(fw, hd, h));
    add(Part::Frame, b(w - fw, -hd, 0.0), b(w, hd, h));
    add(Part::Frame, b(fw, -hd, h - fw), b(w - fw, hd, h));
    match o.kind {
        OpeningKind::Window => {
            // the sill member, then glazing bars splitting the glass
            add(Part::Frame, b(fw, -hd, 0.0), b(w - fw, hd, fw));
            // the outside sill, from the frame out past the wall's face,
            // on the side the window faces out of (built facing +y)
            if face > hd + 0.005 {
                add(Part::Frame, b(0.0, hd, 0.0), b(w, face + SILL_NOSE, 0.03));
            }
            let (x0, x1, z0, z1) = (fw, w - fw, fw, h - fw);
            let (na, nu) = (f.frame.panes_across, f.frame.panes_up);
            let bar = fw * 0.6;
            let bd = hd * 0.7;
            // pane edges along x and z: between bars, bars centred on the splits
            let edges = |a: f64, z: f64, n: u32| -> Vec<(f64, f64)> {
                let step = (z - a) / n as f64;
                (0..n)
                    .map(|i| {
                        let lo = a + step * i as f64 + if i > 0 { bar / 2.0 } else { 0.0 };
                        let hi =
                            a + step * (i + 1) as f64 - if i + 1 < n { bar / 2.0 } else { 0.0 };
                        (lo, hi)
                    })
                    .collect()
            };
            let xs = edges(x0, x1, na);
            let zs = edges(z0, z1, nu);
            for pair in xs.windows(2) {
                add(Part::Frame, b(pair[0].1, -bd, z0), b(pair[1].0, bd, z1));
            }
            for &(xa, xb) in &xs {
                for pair in zs.windows(2) {
                    add(Part::Frame, b(xa, -bd, pair[0].1), b(xb, bd, pair[1].0));
                }
                for &(za, zb) in &zs {
                    add(Part::Glass, b(xa, -GLASS / 2.0, za), b(xb, GLASS / 2.0, zb));
                }
            }
        }
        OpeningKind::Door => {
            // the leaf, flush with the frame's face on the side it opens to
            let gap = 0.004;
            let lt = f.leaf();
            let (l0, l1) = (fw + gap, w - fw - gap);
            add(Part::Leaf, b(l0, hd - lt, 0.01), b(l1, hd, h - fw - gap));
            // a lever handle on each face, on the side away from the hinge
            if l1 - l0 > 0.3 && h > 1.2 {
                let x = l1 - 0.07;
                let z = 1.0f64.min(h * 0.45);
                let (f0, f1) = (hd - lt, hd);
                for (r0, r1, a0, a1) in [
                    (f1, f1 + 0.012, f1 + 0.045, f1 + 0.062),
                    (f0 - 0.012, f0, f0 - 0.062, f0 - 0.045),
                ] {
                    add(
                        Part::Handle,
                        b(x - 0.025, r0, z - 0.04),
                        b(x + 0.025, r1, z + 0.04),
                    );
                    add(
                        Part::Handle,
                        b(x - 0.008, r0.min(a0), z - 0.008),
                        b(x + 0.008, r1.max(a1), z + 0.008),
                    );
                    add(
                        Part::Handle,
                        b(x - 0.13, a0, z - 0.01),
                        b(x + 0.01, a1, z + 0.01),
                    );
                }
            }
        }
        OpeningKind::Hole => {}
    }
    // built opening to (facing out of) +y, a door hinged at x = 0: turn it
    // round as the opening is set
    let across = !o.faces_left(wall);
    let along = o.kind == OpeningKind::Door && o.flip_hand;
    if across || along {
        for pb in &mut out {
            let (lo, hi) = (pb.lo, pb.hi);
            if along {
                (pb.lo.x, pb.hi.x) = (w - hi.x, w - lo.x);
            }
            if across {
                (pb.lo.y, pb.hi.y) = (-hi.y, -lo.y);
            }
        }
    }
    out
}

/// How far a window's outside sill runs past the wall's face.
pub const SILL_NOSE: f64 = 0.04;

/// Which way a door is hung, in words: its hinge side seen from the
/// side it opens to, and which side that is.
pub fn door_hand(o: &Opening, wall: &Wall) -> &'static str {
    // looking at the side it opens to, the wall's start is on the right
    // when that is the wall's left
    let hinge_at_start = !o.flip_hand;
    match (hinge_at_start == o.faces_left(wall), o.flip_facing) {
        (true, false) => "Right, in",
        (false, false) => "Left, in",
        (true, true) => "Right, out",
        (false, true) => "Left, out",
    }
}

/// Opening coordinates → model coordinates, for an opening in `wall`
/// standing on a level at `level_z`.
pub fn to_model(o: &Opening, wall: &Wall, level_z: f64) -> Option<impl Fn(DVec3) -> DVec3> {
    let c = Cut::of(o, wall)?;
    // square to the wall at the opening's middle: on a curved wall the
    // door or window stands on the arc, along its tangent there
    let um = (c.u0 + c.u1) / 2.0;
    let d = wall.tangent_at(um);
    let n = DVec2::new(-d.y, d.x);
    let start = wall.place(um, 0.0) - d * um;
    let z0 = level_z + wall.base_offset + c.v0;
    Some(move |p: DVec3| (start + d * (c.u0 + p.x) + n * p.y).extend(z0 + p.z))
}

/// The opening's parts as meshes in model coordinates, one per kind of part.
pub fn opening_meshes(o: &Opening, wall: &Wall, level_z: f64) -> Vec<(Part, TriMesh)> {
    let Some(at) = to_model(o, wall, level_z) else {
        return vec![];
    };
    let mut out: Vec<(Part, TriMesh)> = Vec::new();
    for pb in opening_boxes(o, wall) {
        let i = match out.iter().position(|(p, _)| *p == pb.part) {
            Some(i) => i,
            None => {
                out.push((pb.part, TriMesh::default()));
                out.len() - 1
            }
        };
        add_box(&mut out[i].1, pb.lo, pb.hi, &at);
    }
    out
}

/// A closed box from `lo` to `hi`, its corners mapped through `at` (a
/// rigid map, so the faces stay square).
pub(crate) fn add_box(m: &mut TriMesh, lo: DVec3, hi: DVec3, at: &dyn Fn(DVec3) -> DVec3) {
    let c = |x: f64, y: f64, z: f64| at(DVec3::new(x, y, z));
    let o = at(DVec3::ZERO);
    let axis = |v: DVec3| at(v) - o;
    let (ex, ey, ez) = (axis(DVec3::X), axis(DVec3::Y), axis(DVec3::Z));
    let (a, b) = (lo, hi);
    m.add_quad(
        [
            c(a.x, a.y, a.z),
            c(a.x, b.y, a.z),
            c(a.x, b.y, b.z),
            c(a.x, a.y, b.z),
        ],
        -ex,
    );
    m.add_quad(
        [
            c(b.x, a.y, a.z),
            c(b.x, b.y, a.z),
            c(b.x, b.y, b.z),
            c(b.x, a.y, b.z),
        ],
        ex,
    );
    m.add_quad(
        [
            c(a.x, a.y, a.z),
            c(b.x, a.y, a.z),
            c(b.x, a.y, b.z),
            c(a.x, a.y, b.z),
        ],
        -ey,
    );
    m.add_quad(
        [
            c(a.x, b.y, a.z),
            c(b.x, b.y, a.z),
            c(b.x, b.y, b.z),
            c(a.x, b.y, b.z),
        ],
        ey,
    );
    m.add_quad(
        [
            c(a.x, a.y, a.z),
            c(b.x, a.y, a.z),
            c(b.x, b.y, a.z),
            c(a.x, b.y, a.z),
        ],
        -ez,
    );
    m.add_quad(
        [
            c(a.x, a.y, b.z),
            c(b.x, a.y, b.z),
            c(b.x, b.y, b.z),
            c(a.x, b.y, b.z),
        ],
        ez,
    );
}

/// The hole an opening cuts, through the whole wall, as a box: what a
/// bare hole shows (see-through) and is picked by, and what the placement
/// ghost shows around a door or window (A7-4, MU18).
pub fn cut_box_mesh(o: &Opening, wall: &Wall, level_z: f64) -> Option<crate::TriMesh> {
    let c = crate::wall::Cut::of(o, wall)?;
    let t = wall.thickness / 2.0;
    // round the arc on a curved wall
    let n = if wall.is_arc() { 8 } else { 1 };
    let us: Vec<f64> = (0..=n)
        .map(|i| c.u0 + (c.u1 - c.u0) * i as f64 / n as f64)
        .collect();
    let mut outline: Vec<glam::DVec2> = us.iter().map(|u| wall.place(*u, -t)).collect();
    outline.extend(us.iter().rev().map(|u| wall.place(*u, t)));
    let z0 = level_z + wall.base_offset;
    Some(crate::scene::slab_mesh(&outline, z0 + c.v1, c.v1 - c.v0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::ElementId;

    fn wall() -> Wall {
        Wall {
            level: ElementId::new(),
            start: DVec2::new(0.0, 0.0),
            end: DVec2::new(5.0, 0.0),
            thickness: 0.3,
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
        }
    }

    fn opening(kind: OpeningKind, frame: OpeningFrame) -> Opening {
        Opening {
            host: ElementId::new(),
            kind,
            offset: 1.0,
            sill: if kind == OpeningKind::Door { 0.0 } else { 0.9 },
            width: 1.2,
            height: if kind == OpeningKind::Door { 2.1 } else { 1.3 },
            material: None,
            frame,
            flip_hand: false,
            flip_facing: false,
            wrap: None,
        }
    }

    fn count(boxes: &[PartBox], part: Part) -> usize {
        boxes.iter().filter(|b| b.part == part).count()
    }

    #[test]
    fn a_window_in_a_curved_wall_stands_on_its_arc() {
        // a half circle of radius 2.5 from (0,0) to (5,0)
        let w = Wall {
            bulge: 1.0,
            ..wall()
        };
        let o = opening(OpeningKind::Window, OpeningFrame::default());
        let at = to_model(&o, &w, 0.0).unwrap();
        // the window's middle, on the wall's centreline
        let mid = at(DVec3::new(o.width / 2.0, 0.0, 0.0)).truncate();
        let c = DVec2::new(2.5, 0.0);
        assert!((mid.distance(c) - 2.5).abs() < 0.01, "{mid}");
        assert!(w.along(mid).1 < 0.01);
        // square to the wall there: across it points at the centre
        let across = at(DVec3::new(o.width / 2.0, 1.0, 0.0)).truncate() - mid;
        assert!(across.normalize().perp_dot((c - mid).normalize()).abs() < 1e-6);
    }

    #[test]
    fn a_window_has_a_frame_round_set_back_glass() {
        let o = opening(OpeningKind::Window, OpeningFrame::default());
        let boxes = opening_boxes(&o, &wall());
        // four frame members and the outside sill
        assert_eq!(count(&boxes, Part::Frame), 5);
        assert_eq!(count(&boxes, Part::Glass), 1);
        let glass = boxes.iter().find(|b| b.part == Part::Glass).unwrap();
        // the glass fills the frame's inside, behind its faces
        assert!((glass.lo.x - 0.06).abs() < 1e-9 && (glass.hi.x - 1.14).abs() < 1e-9);
        assert!(glass.hi.y < 0.05 && glass.lo.y > -0.05);
        // everything stays inside the opening
        for b in &boxes {
            assert!(b.lo.x >= -1e-9 && b.hi.x <= 1.2 + 1e-9);
            assert!(b.lo.z >= -1e-9 && b.hi.z <= 1.3 + 1e-9);
        }
    }

    #[test]
    fn panes_split_the_glass_with_bars() {
        let frame = OpeningFrame {
            panes_across: 3,
            panes_up: 2,
            ..OpeningFrame::default()
        };
        let boxes = opening_boxes(&opening(OpeningKind::Window, frame), &wall());
        assert_eq!(count(&boxes, Part::Glass), 6);
        // the frame and sill, two upright bars, a cross bar in each column
        assert_eq!(count(&boxes, Part::Frame), 5 + 2 + 3);
        let glass: f64 = boxes
            .iter()
            .filter(|b| b.part == Part::Glass)
            .map(|b| (b.hi.x - b.lo.x) * (b.hi.z - b.lo.z))
            .sum();
        let bar = 0.06 * 0.6;
        let inside = (1.2 - 0.12 - 2.0 * bar) * (1.3 - 0.12 - bar);
        assert!((glass - inside).abs() < 1e-9);
    }

    #[test]
    fn a_door_has_a_leaf_and_handles_but_no_sill() {
        let o = opening(OpeningKind::Door, OpeningFrame::default());
        let boxes = opening_boxes(&o, &wall());
        assert_eq!(count(&boxes, Part::Frame), 3);
        assert_eq!(count(&boxes, Part::Leaf), 1);
        assert_eq!(count(&boxes, Part::Glass), 0);
        assert!(count(&boxes, Part::Handle) >= 2);
        let meshes = opening_meshes(&o, &wall(), 0.0);
        // every part is a closed box
        for (_, m) in &meshes {
            assert!(m.volume() > 0.0);
        }
    }

    #[test]
    fn the_frame_fits_a_thin_wall() {
        let mut w = wall();
        w.thickness = 0.05;
        let o = opening(OpeningKind::Window, OpeningFrame::default());
        for b in opening_boxes(&o, &w) {
            assert!(b.lo.y >= -0.025 - 1e-9 && b.hi.y <= 0.025 + 1e-9);
        }
    }

    fn leaf(o: &Opening) -> PartBox {
        *opening_boxes(o, &wall())
            .iter()
            .find(|b| b.part == Part::Leaf)
            .unwrap()
    }

    #[test]
    fn doors_flip_hand_and_facing() {
        let mut o = opening(OpeningKind::Door, OpeningFrame::default());
        // into the interior (the left of an unflipped wall), hinged at its start
        let l = leaf(&o);
        assert!(l.hi.y > 0.0 && l.lo.y > -0.01);
        assert_eq!(door_hand(&o, &wall()), "Right, in");
        let handle = |o: &Opening| {
            opening_boxes(o, &wall())
                .iter()
                .find(|b| b.part == Part::Handle)
                .unwrap()
                .lo
                .x
        };
        assert!(handle(&o) > 0.6, "the handle is away from the hinge");
        o.flip_hand = true;
        assert!(handle(&o) < 0.6);
        assert!((leaf(&o).lo.x - (1.2 - l.hi.x)).abs() < 1e-9);
        assert_eq!(door_hand(&o, &wall()), "Left, in");
        o.flip_facing = true;
        let l = leaf(&o);
        assert!(l.lo.y < 0.0 && l.hi.y < 0.01, "now to the wall's right");
        assert_eq!(door_hand(&o, &wall()), "Right, out");
    }

    #[test]
    fn a_window_faces_out_of_the_exterior_unless_flipped() {
        let mut o = opening(OpeningKind::Window, OpeningFrame::default());
        let sill = |o: &Opening| {
            opening_boxes(o, &wall())
                .into_iter()
                .filter(|b| b.part == Part::Frame)
                .min_by(|a, b| a.lo.y.min(-a.hi.y).total_cmp(&b.lo.y.min(-b.hi.y)))
                .unwrap()
        };
        // the exterior is the wall's right: the sill runs out past it
        assert!((sill(&o).lo.y + 0.15 + SILL_NOSE).abs() < 1e-9);
        o.flip_facing = true;
        assert!((sill(&o).hi.y - 0.15 - SILL_NOSE).abs() < 1e-9);
    }
}
