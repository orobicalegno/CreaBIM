use crate::TriMesh;
use creabim_core::walls::TopProfile;
use creabim_core::{Opening, OpeningWrap, Wall, WallEnds};
use glam::{DVec2, DVec3};

/// Rectangles in wall-local coordinates: `u` along the wall, `v` up from its base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cut {
    pub u0: f64,
    pub u1: f64,
    pub v0: f64,
    pub v1: f64,
}

impl Cut {
    pub fn of(o: &Opening, wall: &Wall) -> Option<Cut> {
        let c = Cut {
            u0: o.offset.max(0.0),
            u1: (o.offset + o.width).min(wall.length()),
            v0: o.sill.max(0.0),
            v1: (o.sill + o.height).min(wall.height),
        };
        (c.u1 > c.u0 + 1e-9 && c.v1 > c.v0 + 1e-9).then_some(c)
    }

    fn contains(&self, u: f64, v: f64) -> bool {
        u > self.u0 && u < self.u1 && v > self.v0 && v < self.v1
    }

    /// The cut grown by `by` at its jambs, its head, and its sill where
    /// it has one (not at the wall's base).
    fn grown(&self, by: f64) -> Cut {
        Cut {
            u0: self.u0 - by,
            u1: self.u1 + by,
            v0: if self.v0 > 1e-9 {
                self.v0 - by
            } else {
                self.v0
            },
            v1: self.v1 + by,
        }
    }
}

/// An opening's cut and how the wall's layers wrap into its reveals.
#[derive(Clone, Debug, PartialEq)]
pub struct WrappedCut {
    pub cut: Cut,
    pub wrap: OpeningWrap,
}

/// Build a closed solid for a straight wall with rectangular openings.
///
/// The wall face is split into a grid at every opening edge; each grid cell is
/// either solid or void. Solid cells get front and back faces, and every
/// boundary between a solid cell and void (or the wall's outer edge) gets a
/// side face, so ends, top, bottom and opening reveals all come out closed.
/// `ends` mitres or trims the two ends where the wall joins its neighbours
/// and leaves out the gaps where walls cross it; with `top`, the top
/// follows the undersides it is attached to instead of being level.
pub fn wall_mesh(
    wall: &Wall,
    base_z: f64,
    openings: &[&Opening],
    ends: &WallEnds,
    top: Option<&TopProfile>,
) -> TriMesh {
    let t = wall.thickness / 2.0;
    wall_layer_mesh(wall, base_z, openings, ends, top, (-t, t), false, [0.0; 2])
}

/// One layer of a layered wall as a closed solid: the part of the wall
/// between `band.0` and `band.1` across it (left positive), its ends on
/// the wall's end lines, or past them for a `core` layer that runs on,
/// and `ext` past them (start, end) where its ends were moved by hand.
#[allow(clippy::too_many_arguments)]
pub fn wall_layer_mesh(
    wall: &Wall,
    base_z: f64,
    openings: &[&Opening],
    ends: &WallEnds,
    top: Option<&TopProfile>,
    band: (f64, f64),
    core: bool,
    ext: [f64; 2],
) -> TriMesh {
    wall_layer_mesh_v(wall, base_z, openings, ends, top, band, core, ext, [0.0; 2])
}

/// [`wall_layer_mesh`], its bottom run down `vext[0]` past the wall's base
/// and its top up `vext[1]` past the wall's top (A7-10; negative stops
/// short).
#[allow(clippy::too_many_arguments)]
pub fn wall_layer_mesh_v(
    wall: &Wall,
    base_z: f64,
    openings: &[&Opening],
    ends: &WallEnds,
    top: Option<&TopProfile>,
    band: (f64, f64),
    core: bool,
    ext: [f64; 2],
    vext: [f64; 2],
) -> TriMesh {
    let cuts: Vec<WrappedCut> = openings
        .iter()
        .filter_map(|o| Cut::of(o, wall))
        .map(|cut| WrappedCut {
            cut,
            wrap: OpeningWrap::default(),
        })
        .collect();
    wall_layer_mesh_wrapped(wall, base_z, &cuts, ends, top, band, None, core, ext, vext)
}

/// [`wall_layer_mesh_v`] for layer `layer` of the wall's assembly, with
/// the layers wrapping into the reveals of its openings as each cut's
/// [`OpeningWrap`] says: where returns pass, the layer stops short of the
/// opening by their thickness, and where it wraps itself it turns in and
/// lines the jambs, head and sill as far as the frame. One closed solid.
#[allow(clippy::too_many_arguments)]
pub fn wall_layer_mesh_wrapped(
    wall: &Wall,
    base_z: f64,
    cuts: &[WrappedCut],
    ends: &WallEnds,
    top: Option<&TopProfile>,
    band: (f64, f64),
    layer: Option<usize>,
    core: bool,
    ext: [f64; 2],
    vext: [f64; 2],
) -> TriMesh {
    let len = wall.length();
    let h = wall.height;
    let (lo, hi) = band;
    let returns = |c: &WrappedCut| layer.and_then(|l| c.wrap.return_of(l));

    // across the wall: the layer itself, split where returns stop, and
    // on to where its own returns run
    let mut ss = vec![lo, hi];
    for c in cuts {
        ss.extend(c.wrap.stops().filter(|s| *s > lo && *s < hi));
        if let Some(r) = returns(c) {
            ss.extend([r.from, r.to]);
        }
    }

    let mut us = vec![0.0, len];
    // a curved wall bends at even steps round its arc (MU40)
    us.extend(wall.bends());
    let mut vs = vec![0.0, h];
    for c in cuts {
        // the opening's edges, and each step back where layers wrap
        let mut steps = vec![0.0];
        if !c.wrap.is_empty() {
            let mut at = ss.clone();
            at.sort_by(|a, b| a.partial_cmp(b).unwrap());
            steps.extend(at.windows(2).map(|w| c.wrap.inset_at((w[0] + w[1]) / 2.0)));
            if let Some(r) = returns(c) {
                steps.extend([r.inset, r.inset + r.thickness]);
            }
        }
        for by in steps {
            let g = c.cut.grown(by);
            us.extend([g.u0, g.u1].into_iter().filter(|u| *u > 0.0 && *u < len));
            vs.extend([g.v0, g.v1].into_iter().filter(|v| *v > 0.0 && *v < h));
        }
    }
    for g in &ends.gaps {
        us.extend([g.lo_mid(), g.hi_mid()]);
    }
    if let Some(top) = top {
        // where the top bends over a ridge or steps at an eave
        // each break twice, a millimetre apart: where the top changes
        // height it steps, not slopes (MU20)
        us.extend(
            top.breaks(wall.start, wall.end)
                .into_iter()
                .flat_map(|k| [k * len - 0.001, k * len + 0.001])
                .filter(|u| *u > 0.0 && *u < len),
        );
    }
    let sort = |xs: &mut Vec<f64>| {
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    };
    sort(&mut us);
    sort(&mut vs);
    sort(&mut ss);

    let (ni, nj, nk) = (us.len() - 1, vs.len() - 1, ss.len() - 1);
    let solid = |i: isize, j: isize, k: isize| -> bool {
        if i < 0 || j < 0 || k < 0 || i as usize >= ni || j as usize >= nj || k as usize >= nk {
            return false;
        }
        let (i, j, k) = (i as usize, j as usize, k as usize);
        let (u, v) = ((us[i] + us[i + 1]) / 2.0, (vs[j] + vs[j + 1]) / 2.0);
        let s = (ss[k] + ss[k + 1]) / 2.0;
        if ends.in_gap(u) {
            return false;
        }
        if s > lo && s < hi {
            // the layer itself, short of each opening by the returns
            // that pass between them
            !cuts
                .iter()
                .any(|c| c.cut.grown(c.wrap.inset_at(s)).contains(u, v))
        } else {
            // its own return: a lining round the opening
            cuts.iter().any(|c| {
                returns(c).is_some_and(|r| {
                    s > r.from
                        && s < r.to
                        && c.cut.grown(r.inset + r.thickness).contains(u, v)
                        && !c.cut.grown(r.inset).contains(u, v)
                })
            })
        }
    };

    let d = wall.direction();
    let n2 = DVec2::new(-d.y, d.x);
    let z0 = base_z + wall.base_offset;
    // an attached top comes down no lower than the heads of openings
    let floor = vs[vs.len().saturating_sub(2)].max(0.0) + 0.01;
    let p = |u: f64, v: f64, w: f64| -> DVec3 {
        // the end grid lines and gap sides follow the joins; a curved
        // wall is laid round its arc, its ends square
        let xy = if wall.is_arc() {
            // its ends as its joins put them, along its tangent there
            if u <= 1e-9 || u >= len - 1e-9 {
                wall.place_end(u > 1e-9, ends.place_layer(wall, u, w, core, ext), w)
            } else {
                wall.place(u, w)
            }
        } else {
            let u = ends.place_layer(wall, u, w, core, ext);
            wall.start + d * u + n2 * w
        };
        let z = match top {
            Some(t) if v >= h - 1e-9 => {
                // an end running into a joined wall, just past the ceiling
                // or roof it is held to, keeps that height rather than
                // rising for a few centimetres (MU20)
                let near = [0.0, 0.05, -0.05, 0.1, -0.1, 0.2, -0.2, 0.3, -0.3]
                    .iter()
                    .filter_map(|o| t.z_at(xy + d * *o))
                    .next();
                near.map_or(z0 + h, |z| z.max(z0 + floor.min(h)))
            }
            _ => z0 + v,
        };
        // a layer run on past the base or the top, or stopped short
        let z = if v <= 1e-9 {
            z - vext[0]
        } else if v >= h - 1e-9 {
            z + vext[1]
        } else {
            z
        };
        DVec3::new(xy.x, xy.y, z)
    };
    let along = DVec3::new(d.x, d.y, 0.0);
    let across = DVec3::new(n2.x, n2.y, 0.0);

    let mut m = TriMesh::default();
    for i in 0..ni as isize {
        for j in 0..nj as isize {
            for k in 0..nk as isize {
                if !solid(i, j, k) {
                    continue;
                }
                let (ua, ub) = (us[i as usize], us[i as usize + 1]);
                let (va, vb) = (vs[j as usize], vs[j as usize + 1]);
                let (lo, hi) = (ss[k as usize], ss[k as usize + 1]);
                // on a curve, the faces turn with it
                let (along, across) = if wall.is_arc() {
                    let t = wall.tangent_at((ua + ub) / 2.0);
                    (DVec3::new(t.x, t.y, 0.0), DVec3::new(-t.y, t.x, 0.0))
                } else {
                    (along, across)
                };
                if !solid(i, j, k + 1) {
                    m.add_quad(
                        [p(ua, va, hi), p(ub, va, hi), p(ub, vb, hi), p(ua, vb, hi)],
                        across,
                    );
                }
                if !solid(i, j, k - 1) {
                    m.add_quad(
                        [p(ua, va, lo), p(ub, va, lo), p(ub, vb, lo), p(ua, vb, lo)],
                        -across,
                    );
                }
                if !solid(i - 1, j, k) {
                    m.add_quad(
                        [p(ua, va, lo), p(ua, va, hi), p(ua, vb, hi), p(ua, vb, lo)],
                        end_normal(p(ua, va, lo), p(ua, va, hi), -along),
                    );
                }
                if !solid(i + 1, j, k) {
                    m.add_quad(
                        [p(ub, va, lo), p(ub, va, hi), p(ub, vb, hi), p(ub, vb, lo)],
                        end_normal(p(ub, va, lo), p(ub, va, hi), along),
                    );
                }
                if !solid(i, j - 1, k) {
                    m.add_quad(
                        [p(ua, va, lo), p(ub, va, lo), p(ub, va, hi), p(ua, va, hi)],
                        -DVec3::Z,
                    );
                }
                if !solid(i, j + 1, k) {
                    let q = [p(ua, vb, lo), p(ub, vb, lo), p(ub, vb, hi), p(ua, vb, hi)];
                    // an attached top slopes: shade it as it lies
                    let n = (q[2] - q[0]).cross(q[3] - q[1]).normalize_or(DVec3::Z);
                    m.add_quad(q, if n.z < 0.0 { -n } else { n });
                }
            }
        }
    }
    m
}

/// Normal of a vertical end face running from `a` to `b` in plan, facing `towards`.
fn end_normal(a: DVec3, b: DVec3, towards: DVec3) -> DVec3 {
    let e = b - a;
    let n = DVec3::new(-e.y, e.x, 0.0).normalize_or_zero();
    if n == DVec3::ZERO {
        towards
    } else if n.dot(towards) < 0.0 {
        -n
    } else {
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::walls::TopFace;
    use creabim_core::{ElementId, Gap, OpeningKind};

    fn wall() -> Wall {
        Wall::new(
            ElementId::new(),
            DVec2::new(1.0, 1.0),
            DVec2::new(1.0, 6.0),
            0.2,
            3.0,
        )
    }

    fn opening(offset: f64, sill: f64, w: f64, h: f64) -> Opening {
        Opening {
            host: ElementId::new(),
            kind: OpeningKind::Window,
            offset,
            sill,
            width: w,
            height: h,
            material: None,
            frame: Default::default(),
            flip_hand: false,
            flip_facing: false,
            wrap: None,
            frame_at: None,
        }
    }

    #[test]
    fn layers_that_wrap_line_the_reveals_and_the_rest_stop_short() {
        use creabim_core::{opening_wrap, Assembly, AssemblyUse, Layer, LayerFunction, WrapSides};
        // 20 render, 160 block, 20 plaster; a window with a 60 mm frame
        let mut a = Assembly::new(
            AssemblyUse::Wall,
            vec![
                Layer::new(LayerFunction::Finish, None, 0.02),
                Layer::new(LayerFunction::Structure, None, 0.16),
                Layer::new(LayerFunction::Finish, None, 0.02),
            ],
        );
        let w = wall();
        let o = opening(1.0, 0.9, 1.2, 1.3);
        let layers = |a: &Assembly, o: &Opening| -> Vec<f64> {
            let cuts = [WrappedCut {
                cut: Cut::of(o, &w).unwrap(),
                wrap: opening_wrap(a, &w, o, 0.06, creabim_core::frame_shift_in(&w, Some(a), o)),
            }];
            creabim_core::assembly::wall_bands(a, &w)
                .iter()
                .map(|b| {
                    wall_layer_mesh_wrapped(
                        &w,
                        0.0,
                        &cuts,
                        &WallEnds::default(),
                        None,
                        (b.s0, b.s1),
                        Some(b.index),
                        b.core,
                        [0.0; 2],
                        [0.0; 2],
                    )
                    .volume()
                })
                .collect()
        };
        let face = 5.0 * 3.0 - 1.2 * 1.3;
        let square = layers(&a, &o);
        for (v, t) in square.iter().zip([0.02, 0.16, 0.02]) {
            assert!((v - face * t).abs() < 1e-6, "{v}");
        }
        // a 20 mm lining round the opening, 50 mm from the face to the frame
        let ring = (1.24 * 1.34 - 1.2 * 1.3) * 0.05;
        a.wrap_openings = WrapSides::Exterior;
        let ext = layers(&a, &o);
        assert!((ext[0] - (face * 0.02 + ring)).abs() < 1e-6, "{}", ext[0]);
        assert!((ext[1] - (face * 0.16 - ring)).abs() < 1e-6, "{}", ext[1]);
        assert!((ext[2] - square[2]).abs() < 1e-9);
        a.wrap_openings = WrapSides::Both;
        let both = layers(&a, &o);
        assert!((both[2] - (face * 0.02 + ring)).abs() < 1e-6);
        assert!((both[1] - (face * 0.16 - 2.0 * ring)).abs() < 1e-6);
        // the wall as a whole is what it was
        let (sq, bo): (f64, f64) = (square.iter().sum(), both.iter().sum());
        assert!((sq - bo).abs() < 1e-6);
        // a layer told not to wrap stops square, and an opening can go
        // its own way
        a.layers[2].wraps = Some(false);
        assert!((layers(&a, &o)[2] - square[2]).abs() < 1e-9);
        let own = Opening {
            wrap: Some(WrapSides::None),
            ..o.clone()
        };
        assert_eq!(layers(&a, &own), square);
        // a door has no sill to line
        let door = opening(1.0, 0.0, 1.0, 2.1);
        let ring = (1.04 * 2.12 - 1.0 * 2.1) * 0.05;
        let d = layers(&a, &door);
        assert!(
            (d[0] - ((15.0 - 2.1) * 0.02 + ring)).abs() < 1e-6,
            "{}",
            d[0]
        );
    }

    #[test]
    fn a_crossing_wall_leaves_a_gap() {
        let ends = WallEnds {
            gaps: vec![Gap {
                lo: [2.0, 2.0],
                hi: [2.3, 2.3],
                core: [0.0; 2],
            }],
            ..Default::default()
        };
        let m = wall_mesh(&wall(), 0.0, &[], &ends, None);
        assert!(
            (m.volume() - 4.7 * 3.0 * 0.2).abs() < 1e-5,
            "{}",
            m.volume()
        );
    }

    #[test]
    fn an_attached_top_follows_the_slope() {
        // an underside over the whole wall, rising along it from 2.5 m at
        // its start to 3.5 m at its end
        let top = TopProfile {
            faces: vec![TopFace {
                outline: vec![
                    DVec2::new(0.0, 0.0),
                    DVec2::new(2.0, 0.0),
                    DVec2::new(2.0, 7.0),
                    DVec2::new(0.0, 7.0),
                ],
                g: DVec2::new(0.0, 0.2),
                c: 2.3,
            }],
        };
        let m = wall_mesh(&wall(), 0.0, &[], &WallEnds::default(), Some(&top));
        assert!(
            (m.volume() - 5.0 * 3.0 * 0.2).abs() < 1e-5,
            "{}",
            m.volume()
        );
        let (_, hi) = m.bounds().unwrap();
        assert!((hi.z - 3.5).abs() < 1e-5);
        // where nothing is overhead it keeps its own height
        let mut away = top.clone();
        away.faces[0].outline = vec![
            DVec2::new(10.0, 0.0),
            DVec2::new(12.0, 0.0),
            DVec2::new(12.0, 2.0),
        ];
        let m = wall_mesh(&wall(), 0.0, &[], &WallEnds::default(), Some(&away));
        assert!((m.volume() - 5.0 * 3.0 * 0.2).abs() < 1e-5);
    }

    #[test]
    fn a_layer_run_on_by_hand_grows_by_its_run() {
        // the middle 100 mm of the wall, 20 mm past its end
        let band = (-0.05, 0.05);
        let m = wall_layer_mesh(
            &wall(),
            0.0,
            &[],
            &WallEnds::default(),
            None,
            band,
            true,
            [0.0, 0.02],
        );
        assert!(
            (m.volume() - 5.02 * 3.0 * 0.1).abs() < 1e-6,
            "{}",
            m.volume()
        );
        let (_, hi) = m.bounds().unwrap();
        assert!((hi.y - 6.02).abs() < 1e-9);
        // held back 50 mm at its start
        let m = wall_layer_mesh(
            &wall(),
            0.0,
            &[],
            &WallEnds::default(),
            None,
            band,
            false,
            [-0.05, 0.0],
        );
        assert!((m.volume() - 4.95 * 3.0 * 0.1).abs() < 1e-6);
    }

    #[test]
    fn plain_wall_is_a_closed_box() {
        let m = wall_mesh(&wall(), 0.0, &[], &WallEnds::default(), None);
        assert_eq!(m.triangle_count(), 12);
        assert!((m.volume() - 5.0 * 3.0 * 0.2).abs() < 1e-5);
    }

    #[test]
    fn openings_remove_their_volume() {
        let win = opening(1.0, 0.9, 1.2, 1.2);
        let door = opening(3.0, 0.0, 0.9, 2.1);
        let m = wall_mesh(&wall(), 3.0, &[&win, &door], &WallEnds::default(), None);
        let expected = (15.0 - 1.44 - 0.9 * 2.1) * 0.2;
        assert!((m.volume() - expected).abs() < 1e-4, "{}", m.volume());
        let (lo, hi) = m.bounds().unwrap();
        assert!((lo.z - 3.0).abs() < 1e-6 && (hi.z - 6.0).abs() < 1e-6);
    }

    #[test]
    fn opening_past_the_wall_end_is_clipped() {
        let o = opening(4.5, 1.0, 2.0, 1.0);
        let m = wall_mesh(&wall(), 0.0, &[&o], &WallEnds::default(), None);
        assert!((m.volume() - (15.0 - 0.5) * 0.2).abs() < 1e-4);
    }

    #[test]
    fn a_curved_wall_follows_its_arc_with_its_thickness_across_it() {
        // a half circle of radius 3 about the origin, 0.2 thick
        let mut w = Wall::new(
            ElementId::new(),
            DVec2::new(3.0, 0.0),
            DVec2::new(-3.0, 0.0),
            0.2,
            3.0,
        );
        w.bulge = -1.0;
        assert!((w.length() - std::f64::consts::PI * 3.0).abs() < 1e-9);
        let m = wall_mesh(&w, 0.0, &[], &WallEnds::default(), None);
        // every corner lies 2.9 or 3.1 from the centre: on its faces
        for p in &m.positions {
            let r = (p[0] as f64).hypot(p[1] as f64);
            assert!((r - 2.9).abs() < 1e-4 || (r - 3.1).abs() < 1e-4, "{r}");
        }
        // and it goes round through the far side
        assert!(m.positions.iter().any(|p| (p[1] as f64).abs() > 2.8));
        // a window in it sits on the arc too
        let win = opening(4.0, 0.9, 1.0, 1.2);
        let m = wall_mesh(&w, 0.0, &[&win], &WallEnds::default(), None);
        assert!(m.positions.iter().all(|p| {
            let r = (p[0] as f64).hypot(p[1] as f64);
            (r - 2.9).abs() < 1e-4 || (r - 3.1).abs() < 1e-4
        }));
    }
}
