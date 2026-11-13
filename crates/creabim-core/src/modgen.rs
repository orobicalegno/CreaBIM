//! The modifiers made for architecture (MU38): repeating along a circle or
//! a path, bevelled corners, shells, floors sliced from a mass, facade
//! panels, louvres and fins, tapering and twisting towers. Each one takes
//! the mesh as the modifiers above left it and returns the new one, with
//! the modelled face each face came from (`None` for what it made up).

use crate::modifiers::{Corners, Evaluated, OpenFaces};
use crate::EditMesh;
use glam::{DVec2, DVec3};

/// The mesh's bounding box.
pub fn bounds(m: &EditMesh) -> (DVec3, DVec3) {
    m.vertices.iter().fold(
        (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)),
        |(lo, hi), v| (lo.min(*v), hi.max(*v)),
    )
}

fn rot_z(v: DVec3, deg: f64) -> DVec3 {
    let (s, c) = deg.to_radians().sin_cos();
    DVec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

/// Add a copy of `e`'s mesh with each vertex moved by `f`.
fn add_copy(out: &mut Evaluated, e: &Evaluated, f: impl Fn(DVec3) -> DVec3) {
    let base = out.mesh.vertices.len() as u32;
    out.mesh
        .vertices
        .extend(e.mesh.vertices.iter().map(|v| f(*v)));
    out.mesh.faces.extend(
        e.mesh
            .faces
            .iter()
            .map(|f| f.iter().map(|v| v + base).collect()),
    );
    out.source.extend_from_slice(&e.source);
}

/// A box about `centre`, `half` along each of the axes `a`, `b` and
/// `a × b`, faces outward.
pub fn push_box(out: &mut Evaluated, centre: DVec3, a: DVec3, b: DVec3, half: DVec3) {
    let c = a.cross(b);
    let base = out.mesh.vertices.len() as u32;
    for k in [-1.0, 1.0] {
        for j in [-1.0, 1.0] {
            for i in [-1.0, 1.0] {
                out.mesh
                    .vertices
                    .push(centre + a * (i * half.x) + b * (j * half.y) + c * (k * half.z));
            }
        }
    }
    for f in [
        [0, 4, 6, 2],
        [1, 3, 7, 5],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 2, 3, 1],
        [4, 5, 7, 6],
    ] {
        out.mesh.faces.push(f.iter().map(|v| v + base).collect());
        out.source.push(None);
    }
}

/// Array, the rich one: a step (fixed, or in multiples of the mesh's own
/// size), a count or as many as fit a length, and a turn and a growth for
/// each copy after the first.
pub fn array(
    e: &Evaluated,
    count: u32,
    offset: DVec3,
    relative: bool,
    fit_length: Option<f64>,
    turn: f64,
    grow: f64,
) -> Evaluated {
    let (lo, hi) = bounds(&e.mesh);
    let centre = (lo + hi) / 2.0;
    let step = if relative { offset * (hi - lo) } else { offset };
    let n = match fit_length {
        Some(len) if step.length() > 1e-9 => (len / step.length()).floor() as u32 + 1,
        _ => count,
    }
    .clamp(1, 500);
    let mut out = Evaluated::default();
    for i in 0..n {
        let k = i as f64;
        let g = grow.powf(k);
        add_copy(&mut out, e, |v| {
            centre + rot_z((v - centre) * g, turn * k) + step * k
        });
    }
    out
}

/// `count` copies about the upright axis through `center`, spread over
/// `angle` degrees: a whole turn shares it out, less puts one at each end.
pub fn radial(
    e: &Evaluated,
    count: u32,
    angle: f64,
    center: DVec2,
    turn_copies: bool,
) -> Evaluated {
    let full = (angle.abs() - 360.0).abs() < 1e-6;
    let n = count.max(1);
    let step = if full {
        angle / n as f64
    } else {
        angle / (n.max(2) - 1) as f64
    };
    let c = center.extend(0.0);
    let (lo, hi) = bounds(&e.mesh);
    let mid = (lo + hi) / 2.0;
    let mut out = Evaluated::default();
    for i in 0..n {
        let a = step * i as f64;
        if turn_copies {
            add_copy(&mut out, e, |v| c + rot_z(v - c, a));
        } else {
            let d = c + rot_z(mid - c, a) - mid;
            add_copy(&mut out, e, |v| v + d.truncate().extend(0.0));
        }
    }
    out
}

/// `count` copies evenly along `path` (in the mesh's own axes), each with
/// the mesh's origin on the path; `align` turns each to face along it.
pub fn along_path(e: &Evaluated, path: &[DVec3], count: u32, align: bool) -> Evaluated {
    if path.len() < 2 {
        return e.clone();
    }
    let lens: Vec<f64> = path.windows(2).map(|w| w[0].distance(w[1])).collect();
    let total: f64 = lens.iter().sum();
    if total < 1e-9 {
        return e.clone();
    }
    let n = count.clamp(1, 500);
    let at = |s: f64| -> (DVec3, DVec3) {
        let mut s = s.clamp(0.0, total);
        for (i, l) in lens.iter().enumerate() {
            if s <= *l || i + 1 == lens.len() {
                let t = if *l > 0.0 { (s / l).min(1.0) } else { 0.0 };
                let d = (path[i + 1] - path[i]).normalize_or_zero();
                return (path[i].lerp(path[i + 1], t), d);
            }
            s -= l;
        }
        (path[path.len() - 1], DVec3::X)
    };
    let mut out = Evaluated::default();
    for i in 0..n {
        let s = if n == 1 {
            0.0
        } else {
            total * i as f64 / (n - 1) as f64
        };
        let (p, d) = at(s);
        let heading = d.y.atan2(d.x).to_degrees();
        add_copy(&mut out, e, |v| {
            p + if align { rot_z(v, heading) } else { v }
        });
    }
    out
}

/// Narrower (or wider) going up: plan sizes scaled from 1 at the bottom to
/// `x`, `y` at the top, about the middle.
pub fn taper(e: &Evaluated, x: f64, y: f64) -> Evaluated {
    let (lo, hi) = bounds(&e.mesh);
    let h = (hi.z - lo.z).max(1e-9);
    let c = (lo + hi) / 2.0;
    let mut out = e.clone();
    for v in &mut out.mesh.vertices {
        let t = (v.z - lo.z) / h;
        v.x = c.x + (v.x - c.x) * (1.0 + (x - 1.0) * t);
        v.y = c.y + (v.y - c.y) * (1.0 + (y - 1.0) * t);
    }
    out
}

/// Turned `angle` degrees from bottom to top about its upright middle.
pub fn twist(e: &Evaluated, angle: f64) -> Evaluated {
    let (lo, hi) = bounds(&e.mesh);
    let h = (hi.z - lo.z).max(1e-9);
    let c = ((lo + hi) / 2.0).truncate().extend(0.0);
    let mut out = e.clone();
    for v in &mut out.mesh.vertices {
        let t = (v.z - lo.z) / h;
        *v = c + rot_z(*v - c, angle * t);
    }
    out
}

/// An upright extrusion's plan outline (anticlockwise) and its bottom and
/// top heights, with the faces it was read from; none for other shapes.
fn prism(m: &EditMesh) -> Option<(Vec<DVec2>, f64, f64, usize, usize)> {
    let (lo, hi) = bounds(m);
    if hi.z - lo.z < 1e-6 {
        return None;
    }
    let level = |z: f64| (z - lo.z).abs() < 1e-6 || (z - hi.z).abs() < 1e-6;
    if !m.vertices.iter().all(|v| level(v.z)) {
        return None;
    }
    let (mut top, mut bottom) = (vec![], vec![]);
    for f in 0..m.faces.len() {
        let n = m.face_normal(f);
        if n.z > 0.999 {
            top.push(f);
        } else if n.z < -0.999 {
            bottom.push(f);
        } else if n.z.abs() > 1e-6 {
            return None;
        }
    }
    if top.len() != 1 || bottom.len() != 1 {
        return None;
    }
    let outline = m.faces[top[0]]
        .iter()
        .map(|v| m.vertices[*v as usize].truncate())
        .collect();
    Some((outline, lo.z, hi.z, top[0], bottom[0]))
}

/// The outline with its corners rounded (or cut) by `radius`.
pub fn bevel_outline(
    p: &[DVec2],
    radius: f64,
    segments: u32,
    chamfer: bool,
    corners: Corners,
) -> Vec<DVec2> {
    let n = p.len();
    let mut out = vec![];
    for i in 0..n {
        let (a, c, b) = (p[(i + n - 1) % n], p[i], p[(i + 1) % n]);
        let (u, w) = ((a - c).normalize_or_zero(), (b - c).normalize_or_zero());
        let convex = (c - a).perp_dot(b - c) > 0.0;
        let wanted = match corners {
            Corners::All => true,
            Corners::Convex => convex,
            Corners::Concave => !convex,
        };
        let theta = u.dot(w).clamp(-1.0, 1.0).acos();
        if !wanted || !(1e-3..=std::f64::consts::PI - 1e-3).contains(&theta) || radius <= 0.0 {
            out.push(c);
            continue;
        }
        let room = 0.5 * a.distance(c).min(b.distance(c));
        let d = (radius / (theta / 2.0).tan()).min(room);
        let (t1, t2) = (c + u * d, c + w * d);
        if chamfer {
            out.extend([t1, t2]);
            continue;
        }
        let r = d * (theta / 2.0).tan();
        let centre = c + (u + w).normalize_or_zero() * (r / (theta / 2.0).sin());
        let a1 = (t1 - centre).to_angle();
        let mut sweep = (t2 - centre).to_angle() - a1;
        while sweep > std::f64::consts::PI {
            sweep -= std::f64::consts::TAU;
        }
        while sweep < -std::f64::consts::PI {
            sweep += std::f64::consts::TAU;
        }
        let s = segments.max(1);
        for k in 0..=s {
            out.push(centre + DVec2::from_angle(a1 + sweep * k as f64 / s as f64) * r);
        }
    }
    out
}

/// Rounded or chamfered upright corners, for an upright extrusion (a
/// slab, a counter, a mass from a plan outline); others stay as they are.
pub fn bevel(
    e: &Evaluated,
    radius: f64,
    segments: u32,
    chamfer: bool,
    corners: Corners,
) -> Evaluated {
    let Some((outline, z0, z1, top, bottom)) = prism(&e.mesh) else {
        return e.clone();
    };
    let q = bevel_outline(&outline, radius, segments, chamfer, corners);
    let n = q.len() as u32;
    let mut out = Evaluated::default();
    out.mesh.vertices.extend(q.iter().map(|p| p.extend(z0)));
    out.mesh.vertices.extend(q.iter().map(|p| p.extend(z1)));
    out.mesh.faces.push((0..n).rev().collect());
    out.source.push(e.source[bottom]);
    out.mesh.faces.push((n..2 * n).collect());
    out.source.push(e.source[top]);
    for i in 0..n {
        let j = (i + 1) % n;
        out.mesh.faces.push(vec![i, j, n + j, n + i]);
        out.source.push(None);
    }
    out
}

/// Hollowed to `thickness`, with its top and/or bottom left open.
pub fn shell(e: &Evaluated, thickness: f64, open: OpenFaces) -> Evaluated {
    let drop = |n: DVec3| match open {
        OpenFaces::None => false,
        OpenFaces::Top => n.z > 0.999,
        OpenFaces::Bottom => n.z < -0.999,
        OpenFaces::Both => n.z.abs() > 0.999,
    };
    let mut kept = Evaluated {
        mesh: EditMesh {
            vertices: e.mesh.vertices.clone(),
            faces: vec![],
        },
        source: vec![],
    };
    for f in 0..e.mesh.faces.len() {
        if !drop(e.mesh.face_normal(f)) {
            kept.mesh.faces.push(e.mesh.faces[f].clone());
            kept.source.push(e.source[f]);
        }
    }
    crate::modifiers::solidify(&kept, thickness)
}

/// Cut into floors at `heights` (above the mesh's own zero), with `gap`
/// left between them (a slab's thickness, a shadow joint).
pub fn slice(e: &Evaluated, heights: &[f64], gap: f64) -> Evaluated {
    use crate::csg;
    let (lo, hi) = bounds(&e.mesh);
    let mut cuts: Vec<f64> = heights
        .iter()
        .copied()
        .filter(|h| *h > lo.z + 1e-4 && *h < hi.z - 1e-4)
        .collect();
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    if cuts.is_empty() {
        return e.clone();
    }
    let polys = csg::from_mesh(&e.mesh, DVec3::ZERO);
    let mut edges = vec![lo.z];
    edges.extend(&cuts);
    edges.push(hi.z);
    let mut pieces = vec![];
    for w in edges.windows(2) {
        // the end boxes reach past the mesh, so no face lies on theirs
        let b0 = if w[0] > lo.z {
            w[0] + gap / 2.0
        } else {
            w[0] - 1.0
        };
        let b1 = if w[1] < hi.z {
            w[1] - gap / 2.0
        } else {
            w[1] + 1.0
        };
        if b1 - b0 < 1e-4 {
            continue;
        }
        let mut bx = Evaluated::default();
        let mid = DVec3::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0, (b0 + b1) / 2.0);
        let half = DVec3::new(
            (hi.x - lo.x) / 2.0 + 1.0,
            (hi.y - lo.y) / 2.0 + 1.0,
            (b1 - b0) / 2.0,
        );
        push_box(&mut bx, mid, DVec3::X, DVec3::Y, half);
        let cut = csg::from_mesh(&bx.mesh, DVec3::ZERO);
        pieces.extend(csg::boolean(polys.clone(), cut, csg::BoolOp::Intersection));
    }
    let mesh = csg::to_mesh(&pieces, DVec3::ZERO);
    Evaluated {
        source: vec![None; mesh.faces.len()],
        mesh,
    }
}

/// A face's corners when it is an upright rectangle: a corner and its two
/// sides.
fn upright_rect(m: &EditMesh, f: usize) -> Option<(DVec3, DVec3, DVec3)> {
    let face = &m.faces[f];
    if face.len() != 4 || m.face_normal(f).z.abs() > 0.3 {
        return None;
    }
    let p: Vec<DVec3> = face.iter().map(|v| m.vertices[*v as usize]).collect();
    let (u, v) = (p[1] - p[0], p[3] - p[0]);
    let square = u.normalize_or_zero().dot(v.normalize_or_zero()).abs() < 1e-3;
    let closes = (p[0] + u + v).distance(p[2]) < 1e-6;
    (square && closes && u.length() > 1e-6 && v.length() > 1e-6).then_some((p[0], u, v))
}

/// Facade panels on each upright rectangular face: as near `width` by
/// `height` as fits evenly, `joint` apart, standing `depth` proud.
pub fn panelize(e: &Evaluated, width: f64, height: f64, joint: f64, depth: f64) -> Evaluated {
    let mut out = e.clone();
    let mut made = 0usize;
    for f in 0..e.mesh.faces.len() {
        let Some((o, u, v)) = upright_rect(&e.mesh, f) else {
            continue;
        };
        let n = e.mesh.face_normal(f);
        // the side nearer horizontal takes the width
        let (u, v) = if u.z.abs() <= v.z.abs() {
            (u, v)
        } else {
            (v, u)
        };
        let (lu, lv) = (u.length(), v.length());
        let nu = ((lu / width.max(0.01)).round() as usize).max(1);
        let nv = ((lv / height.max(0.01)).round() as usize).max(1);
        let (a, b) = (u / lu, n.cross(u / lu));
        let (hu, hv) = (
            (lu / nu as f64 - joint) / 2.0,
            (lv / nv as f64 - joint) / 2.0,
        );
        if hu <= 0.0 || hv <= 0.0 {
            continue;
        }
        for i in 0..nu {
            for j in 0..nv {
                if made >= 20_000 {
                    return out;
                }
                let c = o
                    + u * ((i as f64 + 0.5) / nu as f64)
                    + v * ((j as f64 + 0.5) / nv as f64)
                    + n * (depth / 2.0);
                // b may run along v or against it: the box is the same
                push_box(&mut out, c, a, b, DVec3::new(hu, hv, depth.max(1e-4) / 2.0));
                made += 1;
            }
        }
    }
    out
}

/// Which compass side an outward normal faces: 0 north (+y), 1 east (+x),
/// 2 south, 3 west.
fn side(n: DVec3) -> usize {
    let s = [n.y, n.x, -n.y, -n.x];
    (0..4).max_by(|a, b| s[*a].total_cmp(&s[*b])).unwrap_or(0)
}

/// Fins (upright) or louvres (level) on the upright faces facing the sides
/// picked, `spacing` apart, `depth` out from the face and `thickness`
/// thick, turned `angle` degrees about their roots.
#[allow(clippy::too_many_arguments)]
pub fn louvres(
    e: &Evaluated,
    sides: [bool; 4],
    upright: bool,
    spacing: f64,
    depth: f64,
    thickness: f64,
    angle: f64,
) -> Evaluated {
    let mut out = e.clone();
    let mut made = 0usize;
    let spacing = spacing.max(0.02);
    for f in 0..e.mesh.faces.len() {
        let n = e.mesh.face_normal(f);
        if n.z.abs() > 0.2 || !sides[side(n)] {
            continue;
        }
        let h = DVec3::Z.cross(n).normalize_or_zero();
        let o = e.mesh.vertices[e.mesh.faces[f][0] as usize];
        let pts: Vec<DVec2> = e.mesh.faces[f]
            .iter()
            .map(|v| {
                let p = e.mesh.vertices[*v as usize] - o;
                DVec2::new(p.dot(h), p.z)
            })
            .collect();
        let (s0, s1) = pts
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
        let (z0, z1) = pts
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        let (sa, ca) = angle.to_radians().sin_cos();
        if upright {
            let (a, d) = (h * ca + n * sa, n * ca - h * sa);
            let mut s = s0 + spacing / 2.0;
            while s < s1 && made < 5_000 {
                let c = o + h * s + DVec3::Z * ((z0 + z1) / 2.0) + d * (depth / 2.0);
                push_box(
                    &mut out,
                    c,
                    a,
                    d,
                    DVec3::new(thickness / 2.0, depth / 2.0, (z1 - z0) / 2.0),
                );
                s += spacing;
                made += 1;
            }
        } else {
            let d = n * ca - DVec3::Z * sa;
            let mut z = z0 + spacing / 2.0;
            while z < z1 && made < 5_000 {
                let c = o + h * ((s0 + s1) / 2.0) + DVec3::Z * z + d * (depth / 2.0);
                push_box(
                    &mut out,
                    c,
                    h,
                    d,
                    DVec3::new((s1 - s0) / 2.0, depth / 2.0, thickness / 2.0),
                );
                z += spacing;
                made += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modifiers::{evaluate, Modifier, ModifierKind};

    fn block(x: f64, y: f64, z: f64) -> EditMesh {
        let mut m = EditMesh::cuboid(DVec3::new(x, y, z));
        let (lo, _) = bounds(&m);
        for v in &mut m.vertices {
            *v -= DVec3::new(0.0, 0.0, lo.z);
        }
        m
    }

    fn run(m: &EditMesh, k: ModifierKind) -> EditMesh {
        let e = evaluate(m, &[Modifier::new(k)]);
        e.mesh.validate().unwrap();
        assert_eq!(e.mesh.faces.len(), e.source.len());
        e.mesh
    }

    #[test]
    fn radial_array_goes_round() {
        let m = run(
            &block(1.0, 1.0, 1.0),
            ModifierKind::RadialArray {
                count: 6,
                angle: 360.0,
                center: DVec2::new(5.0, 0.0),
                turn_copies: true,
            },
        );
        assert_eq!(m.faces.len(), 36);
        assert!((m.volume() - 6.0).abs() < 1e-9);
        let (lo, hi) = bounds(&m);
        assert!(hi.x > 9.0 && lo.x < 0.0);
    }

    #[test]
    fn path_array_spreads_copies_end_to_end() {
        let path = vec![
            DVec3::ZERO,
            DVec3::new(10.0, 0.0, 0.0),
            DVec3::new(10.0, 10.0, 0.0),
        ];
        let m = run(
            &block(0.2, 0.2, 1.0),
            ModifierKind::PathArray {
                path,
                count: 5,
                align: true,
            },
        );
        assert_eq!(m.faces.len(), 30);
        let (_, hi) = bounds(&m);
        assert!((hi.y - 10.1).abs() < 1e-6);
    }

    #[test]
    fn a_fillet_rounds_a_slab_and_keeps_it_closed() {
        let m = run(
            &block(4.0, 2.0, 0.2),
            ModifierKind::Bevel {
                radius: 0.5,
                segments: 4,
                chamfer: false,
                corners: Corners::All,
            },
        );
        // 4 corners of 5 points: 20 sides, a top and a bottom
        assert_eq!(m.faces.len(), 22);
        let rounded = (8.0 - (4.0 - std::f64::consts::PI) * 0.25) * 0.2;
        assert!((m.volume() - rounded).abs() < 0.01, "{}", m.volume());
    }

    #[test]
    fn a_chamfer_cuts_only_the_corners_asked_for() {
        let l = [
            DVec2::new(0.0, 0.0),
            DVec2::new(4.0, 0.0),
            DVec2::new(4.0, 2.0),
            DVec2::new(2.0, 2.0),
            DVec2::new(2.0, 4.0),
            DVec2::new(0.0, 4.0),
        ];
        assert_eq!(bevel_outline(&l, 0.3, 1, true, Corners::Concave).len(), 7);
        assert_eq!(bevel_outline(&l, 0.3, 1, true, Corners::Convex).len(), 11);
    }

    #[test]
    fn a_shell_open_at_the_top_is_a_tub() {
        let m = run(
            &block(4.0, 4.0, 2.0),
            ModifierKind::Shell {
                thickness: 0.25,
                open: OpenFaces::Top,
            },
        );
        let expected = 32.0 - 3.5 * 3.5 * 1.75;
        assert!((m.volume() - expected).abs() < 1e-6, "{}", m.volume());
    }

    #[test]
    fn slicing_by_levels_leaves_floors_with_gaps() {
        let m = run(
            &block(4.0, 4.0, 9.0),
            ModifierKind::Slice {
                heights: vec![3.0, 6.0, 20.0],
                gap: 0.3,
            },
        );
        assert!(
            (m.volume() - 16.0 * (9.0 - 0.6)).abs() < 1e-6,
            "{}",
            m.volume()
        );
        let (lo, hi) = bounds(&m);
        assert!((hi.z - 9.0).abs() < 1e-9 && lo.z.abs() < 1e-9);
    }

    #[test]
    fn panels_cover_the_side_faces_evenly() {
        let m = run(
            &block(6.0, 3.0, 3.0),
            ModifierKind::Panelize {
                width: 1.5,
                height: 1.5,
                joint: 0.02,
                depth: 0.05,
            },
        );
        // long sides 4x2, short sides 2x2: 24 panels of 6 faces on 6
        assert_eq!(m.faces.len(), 6 + 24 * 6);
    }

    #[test]
    fn fins_go_only_on_the_sides_picked() {
        let k = |sides| ModifierKind::Louvres {
            sides,
            upright: true,
            spacing: 0.5,
            depth: 0.3,
            thickness: 0.05,
            angle: 0.0,
        };
        let south = run(&block(4.0, 2.0, 3.0), k([false, false, true, false]));
        assert_eq!(south.faces.len(), 6 + 8 * 6);
        let level = run(
            &block(4.0, 2.0, 3.0),
            ModifierKind::Louvres {
                sides: [false, true, false, false],
                upright: false,
                spacing: 1.0,
                depth: 0.3,
                thickness: 0.05,
                angle: 30.0,
            },
        );
        assert_eq!(level.faces.len(), 6 + 3 * 6);
    }

    #[test]
    fn taper_and_twist_keep_the_base() {
        let b = block(2.0, 2.0, 10.0);
        let t = run(&b, ModifierKind::Taper { x: 0.5, y: 0.5 });
        let (lo, hi) = bounds(&t);
        assert!((hi.x - lo.x - 2.0).abs() < 1e-9);
        let top: Vec<_> = t.vertices.iter().filter(|v| v.z > 9.9).collect();
        assert!(top.iter().all(|v| v.x.abs() <= 0.5 + 1e-9));
        let w = run(&b, ModifierKind::Twist { angle: 90.0 });
        // the top turned a quarter, the bottom where it was
        for (v, w) in b.vertices.iter().zip(&w.vertices) {
            let want = if v.z > 9.9 {
                DVec3::new(-v.y, v.x, v.z)
            } else {
                *v
            };
            assert!(w.distance(want) < 1e-9);
        }
    }
}
