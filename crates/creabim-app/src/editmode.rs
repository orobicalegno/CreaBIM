//! Edit mode: an element opened for editing (Properties' Edit button, a
//! double-click on it, or Enter) shows every grip it has, in every view
//! it is in.
//!
//! Every editable outline follows one grammar: a slab's outline and its
//! holes, a roof's and its holes, a shaft's, a sketch's corners and a
//! free railing's path. Selected, an outline shows its corners (drag to
//! move) and, closed, a push grip on each edge. Opened in Edit mode it
//! also shows its holes and an add-corner ◇ in each edge's middle, and
//! its corners can be picked (click, Shift+click, or a box) and removed
//! with Delete, a double-click or the right-click menu, which also adds a
//! corner on an edge, bends a slab's edge into an arc and removes a hole.
//! A drag inside a hole moves it. Every reshape is checked as it happens:
//! an outline that would cross itself, a hole outside or across its
//! outline, holes overlapping, are shown red and never kept, and a corner
//! dropped on its neighbour merges with it. Esc cancels a drag, then drops
//! the picked corners, then leaves Edit mode.

use crate::handles::{Constraint, Handle, HandleKind, Ring};
use creabim_core::parts::{Form, Path};
use creabim_core::{Document, Element, ElementId, ElementKind, Model, SiteForm};
use glam::{DVec2, DVec3};

/// How far an edge's push grip stands off the add-corner grip in the
/// edge's middle, in screen pixels.
pub const EDGE_NUDGE: f32 = 16.0;

/// A corner dropped this close to its neighbour merges with it.
pub const MERGE: f64 = 0.02;

/// The editable outlines of `e`, each as its corners in the model, whether
/// it closes and the plane its corners move on (a point and a normal).
#[derive(Clone, Debug)]
pub struct RingInfo {
    pub ring: Ring,
    pub corners: Vec<DVec3>,
    pub closed: bool,
    pub normal: DVec3,
    /// Corners can be added and taken away (not on a sketch with arcs or
    /// constraints, whose corners those refer to).
    pub reshapes: bool,
}

impl RingInfo {
    /// The fewest corners the outline may have.
    pub fn fewest(&self) -> usize {
        if self.closed {
            3
        } else {
            2
        }
    }
}

fn elevation(model: &Model, level: Option<ElementId>) -> f64 {
    level
        .and_then(|l| model.level(l))
        .map_or(0.0, |l| l.elevation)
}

/// Every outline of `e` Edit mode works on.
pub fn rings(model: &Model, e: &Element) -> Vec<RingInfo> {
    let flat = |ring, pts: &[DVec2], z: f64| RingInfo {
        ring,
        corners: pts.iter().map(|p| p.extend(z)).collect(),
        closed: true,
        normal: DVec3::Z,
        reshapes: true,
    };
    let holes = |pts: &[Vec<DVec2>], z: f64| -> Vec<RingInfo> {
        pts.iter()
            .enumerate()
            .map(|(k, h)| flat(Ring::Hole(k), h, z))
            .collect()
    };
    match &e.kind {
        ElementKind::Slab(s) => {
            let z = s.top(elevation(model, Some(s.level)));
            let mut out = vec![flat(Ring::Outline, &s.boundary, z)];
            out.extend(holes(&s.holes, z));
            out
        }
        ElementKind::Shaft(s) => {
            let (z0, _) = s.span(model);
            vec![flat(Ring::Outline, &s.boundary, z0)]
        }
        ElementKind::Roof(r) => {
            let z = r.base(model);
            let mut out = vec![flat(Ring::Outline, &r.boundary, z)];
            out.extend(holes(&r.holes, z));
            out
        }
        ElementKind::Railing(r) if r.stair.is_none() => {
            let z = elevation(model, Some(r.level));
            vec![RingInfo {
                ring: Ring::Outline,
                corners: r.path.iter().map(|p| *p + DVec3::Z * z).collect(),
                closed: false,
                normal: DVec3::Z,
                reshapes: true,
            }]
        }
        // extrusions and voids, terrain and hard landscape: at their top
        ElementKind::Part(p) => match &p.form {
            Form::Extrusion {
                outline,
                holes: h,
                base,
                height,
                plane: Some(pl),
            } => {
                // on its face, at its far side from the plane
                let lift = DVec3::Z * elevation(model, Some(p.level));
                let on = |ring, pts: &[DVec2]| RingInfo {
                    ring,
                    corners: pts
                        .iter()
                        .map(|q| creabim_core::parts::on_plane(pl, *q, base + height) + lift)
                        .collect(),
                    closed: true,
                    normal: pl.normal(),
                    reshapes: true,
                };
                let mut out = vec![on(Ring::Outline, outline)];
                out.extend(h.iter().enumerate().map(|(k, q)| on(Ring::Hole(k), q)));
                out
            }
            Form::Extrusion {
                outline,
                holes: h,
                base,
                height,
                plane: None,
            } => {
                let z = elevation(model, Some(p.level)) + base + height;
                let mut out = vec![flat(Ring::Outline, outline, z)];
                out.extend(holes(h, z));
                out
            }
            Form::Sweep {
                path:
                    Path::Points {
                        points,
                        closed,
                        normal,
                    },
                ..
            } => {
                let z = elevation(model, Some(p.level));
                vec![RingInfo {
                    ring: Ring::Outline,
                    corners: points.iter().map(|q| *q + DVec3::Z * z).collect(),
                    closed: *closed,
                    normal: normal.unwrap_or(DVec3::Z),
                    reshapes: true,
                }]
            }
            Form::Sweep { .. } => vec![],
        },
        ElementKind::Site(s) => {
            let z = elevation(model, Some(s.level));
            match &s.form {
                SiteForm::Terrain { boundary, top, .. } => {
                    vec![flat(Ring::Outline, boundary, z + top)]
                }
                SiteForm::Area {
                    boundary,
                    holes: h,
                    top,
                    ..
                } => {
                    let mut out = vec![flat(Ring::Outline, boundary, z + top)];
                    out.extend(holes(h, z + top));
                    out
                }
                SiteForm::Plant { .. } => vec![],
            }
        }
        // drafting, on its view's paper: a region's outline, a path's points
        ElementKind::Detail(d) => {
            let Some(frame) = creabim_core::DrawingFrame::of(model, d.view) else {
                return vec![];
            };
            vec![RingInfo {
                ring: Ring::Outline,
                corners: d.model_points(frame),
                closed: d.kind.is_area(),
                normal: frame.normal(),
                // a circle, an arc or an ellipse is its set points
                reshapes: d.kind.fixed_points().is_none(),
            }]
        }
        ElementKind::Sketch(sk) if sk.circle.is_none() => {
            let lift = DVec3::Z * elevation(model, sk.level);
            let straight = (0..sk.segment_count())
                .all(|i| sk.kind(i) == creabim_core::sketch::SegmentKind::Line);
            vec![RingInfo {
                ring: Ring::Outline,
                corners: sk
                    .points
                    .iter()
                    .map(|p| sk.plane.to_model(*p) + lift)
                    .collect(),
                closed: sk.closed,
                normal: sk.plane.normal(),
                reshapes: straight && sk.constraints.is_empty(),
            }]
        }
        _ => vec![],
    }
}

/// Outline `ring` of `e`.
pub fn ring_of(model: &Model, e: &Element, ring: Ring) -> Option<RingInfo> {
    rings(model, e).into_iter().find(|r| r.ring == ring)
}

/// The number of edges of an outline of `n` corners.
fn edge_count(n: usize, closed: bool) -> usize {
    if closed {
        n
    } else {
        n.saturating_sub(1)
    }
}

/// The grips of `e`'s outlines, the same for every element that has one:
/// selected, the corners of its own outline and a push grip on each edge
/// of a closed one; in Edit mode (`edit`) its holes' too, and an
/// add-corner grip in each edge's middle.
pub fn outline_handles(model: &Model, id: ElementId, edit: bool) -> Vec<Handle> {
    let Some(e) = model.get(id) else {
        return vec![];
    };
    let mut out = Vec::new();
    // from the outline's height to the middle of the thickness: a slab's
    // outline is its top, a roof's its base
    let half = match &e.kind {
        ElementKind::Slab(s) => -s.thickness / 2.0,
        ElementKind::Roof(r) => r.thickness / 2.0,
        _ => 0.0,
    };
    for r in rings(model, e) {
        if !edit && r.ring != Ring::Outline {
            continue;
        }
        let n = r.corners.len();
        for (i, c) in r.corners.iter().enumerate() {
            out.push(Handle {
                kind: HandleKind::Vertex { ring: r.ring, i },
                at: *c,
                constraint: Constraint::Plane {
                    point: *c,
                    normal: r.normal,
                },
                nudge: 0.0,
            });
        }
        let ccw = signed_area(&r.corners, r.normal) > 0.0;
        for i in 0..edge_count(n, r.closed) {
            let (a, b) = (r.corners[i], r.corners[(i + 1) % n]);
            let mid = edge_mid(e, r.ring, i, a.z).unwrap_or((a + b) / 2.0);
            if edit && r.reshapes {
                out.push(Handle {
                    kind: HandleKind::AddVertex { ring: r.ring, i },
                    at: mid,
                    constraint: Constraint::Plane {
                        point: mid,
                        normal: r.normal,
                    },
                    nudge: 0.0,
                });
            }
            // sketches keep their edges where their corners put them
            if r.closed && !matches!(e.kind, ElementKind::Sketch(_)) {
                let along = (b - a).normalize_or_zero();
                let left = r.normal.cross(along);
                let out_dir = if ccw { -left } else { left };
                if out_dir != DVec3::ZERO {
                    // a push bar sits half-way through the element's
                    // thickness, not on its top or underside, so in a
                    // section it is on the edge it pushes (MU25)
                    let mid = mid + DVec3::Z * half;
                    out.push(Handle {
                        kind: HandleKind::Edge { ring: r.ring, i },
                        at: mid,
                        constraint: Constraint::Line {
                            point: mid,
                            dir: out_dir,
                        },
                        // standing inside, off the add-corner grip there
                        nudge: if edit { -EDGE_NUDGE } else { 0.0 },
                    });
                }
            }
        }
    }
    out
}

/// The middle of edge `i` of outline `ring`, at height `z`, when it is
/// a slab's bent edge: on the arc. None for a straight edge.
fn edge_mid(e: &Element, ring: Ring, i: usize, z: f64) -> Option<DVec3> {
    match (&e.kind, ring) {
        (ElementKind::Slab(s), Ring::Outline) if i < s.boundary.len() && s.bulge(i) != 0.0 => {
            Some(s.edge(i).at(0.5).extend(z))
        }
        _ => None,
    }
}

/// Twice the area of `pts` seen along `normal`, positive counter-clockwise.
fn signed_area(pts: &[DVec3], normal: DVec3) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| pts[i].cross(pts[(i + 1) % n]).dot(normal))
        .sum()
}

/// The corners of outline `ring` of an element, in its own coordinates
/// (plan points, or a sketch's plane). A railing's path keeps heights,
/// so it isn't one of these: see [`set_corner`].
pub fn ring_mut(kind: &mut ElementKind, ring: Ring) -> Option<&mut Vec<DVec2>> {
    match (kind, ring) {
        (ElementKind::Slab(s), Ring::Outline) => Some(&mut s.boundary),
        (ElementKind::Slab(s), Ring::Hole(k)) => s.holes.get_mut(k),
        (ElementKind::Shaft(s), Ring::Outline) => Some(&mut s.boundary),
        (ElementKind::Roof(r), Ring::Outline) => Some(&mut r.boundary),
        (ElementKind::Roof(r), Ring::Hole(k)) => r.holes.get_mut(k),
        (ElementKind::Sketch(sk), Ring::Outline) => Some(&mut sk.points),
        (ElementKind::Detail(d), Ring::Outline) => Some(&mut d.points),
        (ElementKind::Part(p), ring) => match (&mut p.form, ring) {
            (Form::Extrusion { outline, .. }, Ring::Outline) => Some(outline),
            (Form::Extrusion { holes, .. }, Ring::Hole(k)) => holes.get_mut(k),
            _ => None,
        },
        (ElementKind::Site(s), ring) => match (&mut s.form, ring) {
            (SiteForm::Terrain { boundary, .. }, Ring::Outline)
            | (SiteForm::Area { boundary, .. }, Ring::Outline) => Some(boundary),
            (SiteForm::Area { holes, .. }, Ring::Hole(k)) => holes.get_mut(k),
            _ => None,
        },
        _ => None,
    }
}

/// A sweep's path through points, which keep their heights.
fn sweep_points(kind: &mut ElementKind) -> Option<&mut Vec<DVec3>> {
    match kind {
        ElementKind::Part(p) => match &mut p.form {
            Form::Sweep {
                path: Path::Points { points, .. },
                ..
            } => Some(points),
            _ => None,
        },
        _ => None,
    }
}

/// Put corner `i` of `ring` at `p` (a railing's keeps its height).
fn set_corner(e: &mut Element, ring: Ring, i: usize, p: DVec2) -> Result<(), String> {
    if let (ElementKind::Railing(r), Ring::Outline) = (&mut e.kind, ring) {
        let c = r.path.get_mut(i).ok_or("that corner is gone")?;
        *c = p.extend(c.z);
        return Ok(());
    }
    if let ElementKind::Part(part) = &e.kind {
        if let Some(pl) = sweep_plane(part) {
            // a path on a face stays on that face
            let path = sweep_points(&mut e.kind).ok_or("that path is gone")?;
            *path.get_mut(i).ok_or("that corner is gone")? = pl.to_model(p);
            return Ok(());
        }
    }
    if let Some(path) = sweep_points(&mut e.kind) {
        let c = path.get_mut(i).ok_or("that corner is gone")?;
        *c = p.extend(c.z);
        return Ok(());
    }
    let pts = ring_mut(&mut e.kind, ring).ok_or("that outline is gone")?;
    *pts.get_mut(i).ok_or("that corner is gone")? = p;
    Ok(())
}

/// The face a sweep's path was drawn on, through its first point (above
/// its level).
fn sweep_plane(part: &creabim_core::parts::Part) -> Option<creabim_core::Workplane> {
    match &part.form {
        Form::Sweep {
            path:
                Path::Points {
                    points,
                    normal: Some(n),
                    ..
                },
            ..
        } => Some(creabim_core::Workplane::facing(*points.first()?, *n)),
        _ => None,
    }
}

/// A model point as a corner of `e`'s outlines: in plan, or in a sketch's plane.
fn local(model: &Model, e: &Element, p: DVec3) -> DVec2 {
    if let ElementKind::Part(part) = &e.kind {
        if let Some(pl) = sweep_plane(part) {
            return pl.to_plane(p - DVec3::Z * elevation(model, Some(part.level)));
        }
    }
    match &e.kind {
        ElementKind::Sketch(sk) => sk.plane.to_plane(p - DVec3::Z * elevation(model, sk.level)),
        ElementKind::Part(creabim_core::parts::Part {
            form: Form::Extrusion {
                plane: Some(pl), ..
            },
            level,
            ..
        }) => pl.to_plane(p - DVec3::Z * elevation(model, Some(*level))),
        ElementKind::Detail(d) => {
            creabim_core::DrawingFrame::of(model, d.view).map_or(p.truncate(), |f| f.to_drawing(p))
        }
        _ => p.truncate(),
    }
}

fn round(v: f64, step: f64) -> f64 {
    if step > 0.0 {
        (v / step).round() * step
    } else {
        v
    }
}

/// Insert a corner after corner `i` of `ring`, at `p`: on a slab's
/// outline its arcs split, a roof keeps its sloping edge, a sketch its
/// segment kinds, a railing the heights either side.
fn insert(e: &mut Element, ring: Ring, i: usize, p: DVec2) {
    let face = match &e.kind {
        ElementKind::Part(part) => sweep_plane(part),
        _ => None,
    };
    match (&mut e.kind, ring) {
        (ElementKind::Slab(s), Ring::Outline) => {
            s.split_edge(i);
            if let Some(c) = s.boundary.get_mut(i + 1) {
                *c = p;
            }
        }
        (ElementKind::Roof(r), Ring::Outline) => {
            r.boundary.insert(i + 1, p);
            if r.edge > i {
                r.edge += 1;
            }
        }
        (ElementKind::Sketch(sk), Ring::Outline) => {
            sk.points.insert(i + 1, p);
            if sk.kinds.len() > i {
                sk.kinds.insert(i + 1, Default::default());
            }
        }
        (ElementKind::Railing(r), Ring::Outline) => {
            let n = r.path.len();
            let z = match (r.path.get(i), r.path.get((i + 1).min(n.saturating_sub(1)))) {
                (Some(a), Some(b)) => (a.z + b.z) / 2.0,
                _ => 0.0,
            };
            r.path.insert((i + 1).min(n), p.extend(z));
        }
        (
            ElementKind::Part(creabim_core::Part {
                form:
                    Form::Sweep {
                        path: Path::Points { points: path, .. },
                        ..
                    },
                ..
            }),
            _,
        ) => {
            let n = path.len();
            let z = match (path.get(i), path.get((i + 1) % n.max(1))) {
                (Some(a), Some(b)) => (a.z + b.z) / 2.0,
                _ => 0.0,
            };
            let at = face.map_or(p.extend(z), |pl| pl.to_model(p));
            path.insert((i + 1).min(n), at);
        }
        (kind, ring) => {
            if let Some(pts) = ring_mut(kind, ring) {
                pts.insert(i + 1, p);
            }
        }
    }
}

/// Take corner `i` of `ring` away.
fn remove(e: &mut Element, ring: Ring, i: usize) {
    match (&mut e.kind, ring) {
        (ElementKind::Slab(s), Ring::Outline) => s.remove_corner(i),
        (ElementKind::Roof(r), Ring::Outline) => {
            r.boundary.remove(i);
            if r.edge >= i && r.edge > 0 {
                r.edge -= 1;
            }
            r.edge = r.edge.min(r.boundary.len().saturating_sub(1));
        }
        (ElementKind::Sketch(sk), Ring::Outline) => {
            sk.points.remove(i);
            if i < sk.kinds.len() {
                sk.kinds.remove(i);
            }
            let count = sk.segment_count();
            sk.kinds.truncate(count);
        }
        (ElementKind::Railing(r), Ring::Outline) => {
            if i < r.path.len() {
                r.path.remove(i);
            }
        }
        (
            ElementKind::Part(creabim_core::Part {
                form:
                    Form::Sweep {
                        path: Path::Points { points: path, .. },
                        ..
                    },
                ..
            }),
            _,
        ) => {
            if i < path.len() {
                path.remove(i);
            }
        }
        (kind, ring) => {
            if let Some(pts) = ring_mut(kind, ring) {
                if i < pts.len() {
                    pts.remove(i);
                }
            }
        }
    }
}

/// Take hole `k` away from a slab or a roof.
fn remove_hole(e: &mut Element, k: usize) -> bool {
    let holes = match &mut e.kind {
        ElementKind::Slab(s) => &mut s.holes,
        ElementKind::Roof(r) => &mut r.holes,
        ElementKind::Part(p) => match &mut p.form {
            Form::Extrusion { holes, .. } => holes,
            _ => return false,
        },
        ElementKind::Site(s) => match &mut s.form {
            SiteForm::Area { holes, .. } => holes,
            _ => return false,
        },
        _ => return false,
    };
    if k < holes.len() {
        holes.remove(k);
        true
    } else {
        false
    }
}

/// A corner moved onto (or very near) a neighbour merges with it, if the
/// outline keeps enough corners. True when it did.
fn merge_if_on_neighbour(model: &Model, e: &mut Element, ring: Ring, i: usize) -> bool {
    let Some(info) = ring_of(model, e, ring) else {
        return false;
    };
    let n = info.corners.len();
    if n <= info.fewest() || i >= n {
        return false;
    }
    let c = info.corners[i];
    let prev = (i > 0 || info.closed).then(|| info.corners[(i + n - 1) % n]);
    let next = (i + 1 < n || info.closed).then(|| info.corners[(i + 1) % n]);
    if [prev, next]
        .into_iter()
        .flatten()
        .any(|q| q.distance(c) < MERGE)
    {
        remove(e, ring, i);
        true
    } else {
        false
    }
}

/// Grip `kind` of `e` dragged from `grab` to `now`: a corner moved, an
/// edge pushed square to itself (rounded to `step`), a corner pulled out
/// of an edge, a hole moved, a shaft's base or top raised or lowered.
/// The shape is checked (see [`problem`]): one that is broken is refused
/// with the reason, and left in `e` for showing.
pub fn reshape(
    model: &Model,
    e: &mut Element,
    kind: HandleKind,
    grab: DVec3,
    now: DVec3,
    step: f64,
) -> Result<(), String> {
    reshape_unchecked(model, e, kind, grab, now, step)?;
    match problem(model, e) {
        Some(p) => Err(p),
        None => Ok(()),
    }
}

/// [`reshape`] without the check.
pub fn reshape_unchecked(
    model: &Model,
    e: &mut Element,
    kind: HandleKind,
    grab: DVec3,
    now: DVec3,
    step: f64,
) -> Result<(), String> {
    let d = now - grab;
    match kind {
        HandleKind::Vertex { ring, i } => {
            let p = local(model, e, now);
            set_corner(e, ring, i, p)?;
            merge_if_on_neighbour(model, e, ring, i);
        }
        HandleKind::AddVertex { ring, i } => {
            let p = local(model, e, now);
            insert(e, ring, i, p);
            merge_if_on_neighbour(model, e, ring, i + 1);
        }
        HandleKind::Edge { ring, i } => {
            let info = ring_of(model, e, ring).ok_or("that outline is gone")?;
            let n = info.corners.len();
            let ccw = signed_area(&info.corners, info.normal) > 0.0;
            let (a, b) = (info.corners[i % n], info.corners[(i + 1) % n]);
            let left = info.normal.cross((b - a).normalize_or_zero());
            let out_dir = if ccw { -left } else { left };
            let by = out_dir * round(d.dot(out_dir), step);
            let (pa, pb) = (local(model, e, a + by), local(model, e, b + by));
            set_corner(e, ring, i % n, pa)?;
            set_corner(e, ring, (i + 1) % n, pb)?;
        }
        HandleKind::Ring { ring } => {
            let info = ring_of(model, e, ring).ok_or("that outline is gone")?;
            let by = DVec3::new(round(d.x, step), round(d.y, step), d.z);
            for (i, c) in info.corners.iter().enumerate() {
                let p = local(model, e, *c + by);
                set_corner(e, ring, i, p)?;
            }
        }
        HandleKind::ShaftBase | HandleKind::ShaftTop => {
            let ElementKind::Shaft(s) = &mut e.kind else {
                return Err("only shafts have that grip".into());
            };
            let (z0, z1) = s.span(model);
            let dz = round(d.z, step);
            if kind == HandleKind::ShaftBase {
                if z0 + dz > z1 - 0.1 {
                    return Err("its base would be above its top".into());
                }
                s.base_offset += dz;
            } else {
                if z1 + dz < z0 + 0.1 {
                    return Err("its top would be below its base".into());
                }
                s.top_offset += dz;
            }
        }
        _ => return Err("this handle doesn't fit this element".into()),
    }
    Ok(())
}

/// What is wrong with `e`'s outlines, if anything: one with no area or
/// too few corners, one crossing itself, a hole outside or across the
/// outline, two holes overlapping.
pub fn problem(model: &Model, e: &Element) -> Option<String> {
    // a slab of two corners and arcs is a disc: its outline is checked
    // with its arcs below
    let disc = matches!(&e.kind, ElementKind::Slab(s) if s.has_arcs());
    let rs = rings(model, e);
    let mut flat = None;
    for r in rs.iter().filter(|r| !(disc && r.ring == Ring::Outline)) {
        let what = match r.ring {
            Ring::Outline => "the outline",
            Ring::Hole(_) => "a hole",
        };
        if r.corners.len() < r.fewest() {
            return Some(format!("{what} needs at least {} corners", r.fewest()));
        }
        if r.closed && signed_area(&r.corners, r.normal).abs() < 1e-8 {
            flat = flat.or(Some(format!("{what} would have no area")));
        }
    }
    // a crossing says more than the area it nets to
    let shape = shape_problem(e, disc);
    shape.or(flat)
}

/// An outline of `e` crossing itself, a hole outside or across it, holes
/// overlapping.
fn shape_problem(e: &Element, disc: bool) -> Option<String> {
    let (outline, closed, holes) = match &e.kind {
        ElementKind::Slab(s) => {
            let o = s.outline();
            if disc && creabim_core::polygon::signed_area2(&o).abs() < 1e-8 {
                return Some("the outline would have no area".into());
            }
            (o, true, s.holes.clone())
        }
        ElementKind::Roof(r) => (r.boundary.clone(), true, r.holes.clone()),
        ElementKind::Shaft(s) => (s.boundary.clone(), true, vec![]),
        ElementKind::Railing(r) if r.stair.is_none() => {
            (r.path.iter().map(|p| p.truncate()).collect(), false, vec![])
        }
        ElementKind::Sketch(sk) if sk.circle.is_none() => (sk.points.clone(), sk.closed, vec![]),
        ElementKind::Detail(d) if d.kind.is_area() => (d.points.clone(), true, vec![]),
        ElementKind::Part(p) => match &p.form {
            Form::Extrusion { outline, holes, .. } => (outline.clone(), true, holes.clone()),
            Form::Sweep {
                path: Path::Points { points, closed, .. },
                ..
            } => (
                points.iter().map(|q| q.truncate()).collect(),
                *closed,
                vec![],
            ),
            Form::Sweep { .. } => return None,
        },
        ElementKind::Site(s) => match &s.form {
            SiteForm::Terrain { boundary, .. } => (boundary.clone(), true, vec![]),
            SiteForm::Area {
                boundary, holes, ..
            } => (boundary.clone(), true, holes.clone()),
            SiteForm::Plant { .. } => return None,
        },
        _ => return None,
    };
    creabim_core::slab::outline_problem(&outline, closed, &holes)
}

/// Remove `corners` (outline and corner index) of `id` in one undo step,
/// keeping at least 3 on a closed outline and 2 on an open one. What the
/// status bar says, or why not.
pub fn remove_corners(
    doc: &mut Document,
    id: ElementId,
    corners: &[(Ring, usize)],
) -> Result<String, String> {
    let e = doc.model().get(id).ok_or("It is gone.")?.clone();
    if corners.is_empty() {
        return Err("Select a corner to remove it: click it, Shift+click for more.".into());
    }
    let model = doc.model();
    let mut edited = e.clone();
    let mut by_ring: Vec<(Ring, Vec<usize>)> = vec![];
    for &(ring, i) in corners {
        match by_ring.iter_mut().find(|(r, _)| *r == ring) {
            Some((_, v)) => v.push(i),
            None => by_ring.push((ring, vec![i])),
        }
    }
    for (ring, mut idx) in by_ring {
        let info = ring_of(model, &e, ring).ok_or("That outline is gone.")?;
        if !info.reshapes {
            return Err(
                "This sketch has arcs or constraints, which keep its corners: drag them instead."
                    .into(),
            );
        }
        idx.sort_unstable();
        idx.dedup();
        idx.retain(|i| *i < info.corners.len());
        let what = match ring {
            Ring::Outline => "outline",
            Ring::Hole(_) => "hole",
        };
        if info.corners.len() - idx.len() < info.fewest() {
            return Err(format!(
                "The {what} needs at least {} corners: {} would be left.",
                info.fewest(),
                info.corners.len() - idx.len()
            ));
        }
        // from the last, so the earlier indices stay put
        for i in idx.into_iter().rev() {
            remove(&mut edited, ring, i);
        }
    }
    if let Some(p) = problem(model, &edited) {
        return Err(format!("Can't remove that: {p}."));
    }
    edited.validate()?;
    let n = corners.len();
    let label = match n {
        1 => format!("Remove a corner of {}", e.name),
        n => format!("Remove {n} corners of {}", e.name),
    };
    doc.edit(id, &label, |el| *el = edited.clone())
        .map_err(|e| e.to_string())?;
    Ok(format!("{label}. Ctrl+Z to undo."))
}

/// Remove holes `holes` of slab or roof `id` in one undo step.
pub fn remove_holes(doc: &mut Document, id: ElementId, holes: &[usize]) -> Result<String, String> {
    let e = doc.model().get(id).ok_or("It is gone.")?.clone();
    let mut edited = e.clone();
    let mut ks = holes.to_vec();
    ks.sort_unstable();
    ks.dedup();
    for k in ks.iter().rev() {
        if !remove_hole(&mut edited, *k) {
            return Err("That hole is gone.".into());
        }
    }
    let label = match ks.as_slice() {
        [k] => format!("Remove hole {} of {}", k + 1, e.name),
        ks => format!("Remove {} holes of {}", ks.len(), e.name),
    };
    doc.edit(id, &label, |el| *el = edited.clone())
        .map_err(|e| e.to_string())?;
    Ok(format!("{label}. Ctrl+Z to undo."))
}

/// Add a corner on edge `i` of `ring`, at `at` (projected onto the edge)
/// or in its middle.
pub fn add_corner(
    doc: &mut Document,
    id: ElementId,
    ring: Ring,
    i: usize,
    at: Option<DVec3>,
) -> Result<String, String> {
    let e = doc.model().get(id).ok_or("It is gone.")?.clone();
    let model = doc.model();
    let info = ring_of(model, &e, ring).ok_or("That outline is gone.")?;
    if !info.reshapes {
        return Err(
            "This sketch has arcs or constraints, which keep its corners: drag them instead."
                .into(),
        );
    }
    let n = info.corners.len();
    if i >= edge_count(n, info.closed) {
        return Err("That edge is gone.".into());
    }
    let (a, b) = (info.corners[i], info.corners[(i + 1) % n]);
    let p = match at {
        // onto the straight edge, kept clear of its ends
        Some(p) if edge_mid(&e, ring, i, a.z).is_none() => {
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.05, 0.95);
            a + ab * t
        }
        _ => edge_mid(&e, ring, i, a.z).unwrap_or((a + b) / 2.0),
    };
    let mut edited = e.clone();
    let q = local(model, &e, p);
    insert(&mut edited, ring, i, q);
    edited.validate()?;
    let label = format!("Add a corner to {}", e.name);
    doc.edit(id, &label, |el| *el = edited.clone())
        .map_err(|e| e.to_string())?;
    Ok(format!("{label}: drag it into place. Ctrl+Z to undo."))
}

/// Bend edge `i` of a slab's outline into an arc, or straighten it.
pub fn set_arc(doc: &mut Document, id: ElementId, i: usize, arc: bool) -> Result<String, String> {
    let e = doc.model().get(id).ok_or("It is gone.")?.clone();
    let mut edited = e.clone();
    let ElementKind::Slab(s) = &mut edited.kind else {
        return Err("Only a slab's outline has arcs.".into());
    };
    let n = s.boundary.len();
    if i >= n {
        return Err("That edge is gone.".into());
    }
    s.bulges.resize(n, 0.0);
    // a quarter of a circle's worth of bend
    s.bulges[i] = if arc {
        (std::f64::consts::FRAC_PI_2 / 4.0).tan()
    } else {
        0.0
    };
    if s.bulges.iter().all(|b| *b == 0.0) {
        s.bulges.clear();
    }
    if let Some(p) = problem(doc.model(), &edited) {
        return Err(format!("Can't do that: {p}."));
    }
    let label = if arc {
        format!("Make an arc on {}", e.name)
    } else {
        format!("Straighten an edge of {}", e.name)
    };
    doc.edit(id, &label, |el| *el = edited.clone())
        .map_err(|e| e.to_string())?;
    Ok(format!("{label}. Ctrl+Z to undo."))
}

/// Add a corner in an edge's middle, or take a corner away.
#[cfg(test)]
pub fn add_or_remove(
    doc: &mut Document,
    id: ElementId,
    kind: HandleKind,
) -> Result<String, String> {
    match kind {
        HandleKind::AddVertex { ring, i } => add_corner(doc, id, ring, i, None),
        HandleKind::Vertex { ring, i } => remove_corners(doc, id, &[(ring, i)]),
        _ => Err("That isn't a corner or an edge.".into()),
    }
}

/// What the status bar says while `e` is in Edit mode.
pub fn hint(model: &Model, e: &Element) -> String {
    let outlines = !rings(model, e).is_empty();
    let name = &e.name;
    if outlines {
        let layers = if crate::handles::has_layers(model, e.id) {
            " Tab: its layers' edges."
        } else {
            ""
        };
        format!(
            "Editing {name}: select a corner and press Delete (Shift+click or drag a box for more); drag a corner to move it, an edge's grip to push it, a ◇ to pull out a corner, inside a hole to move it. Right-click for more.{layers} Esc finishes."
        )
    } else if crate::handles::has_layers(model, e.id) {
        format!("Editing {name}: drag its grips in any view. Its layers' ends in the Grips row. Tab finishes.")
    } else {
        format!("Editing {name}: drag its grips in any view. Tab finishes.")
    }
}

/// The short note Properties shows under the Edit button while editing.
pub fn panel_hint(kind: &ElementKind) -> &'static str {
    match kind {
        ElementKind::Slab(_) | ElementKind::Roof(_) => {
            "Click a corner (Shift+click or a box for more) and press Delete to remove it. Drag corners and edges of the outline and of each hole, a ◇ to add a corner, inside a hole to move it. Right-click a corner, an edge or a hole for more. Layered: Tab shows a grip on each edge of each layer, to push that layer out past the edge or pull it in."
        }
        ElementKind::Shaft(_) => {
            "Click a corner and press Delete to remove it. Drag its corners and edges in any plan, section or 3D view, a ◇ to add a corner. The grips in its middle move its base and top."
        }
        ElementKind::Part(_) | ElementKind::Site(_) => {
            "Click a corner (Shift+click or a box for more) and press Delete to remove it. Drag corners and edges of the outline and of each hole, a ◇ to add a corner. Right-click a corner, an edge or a hole for more."
        }
        ElementKind::Detail(_) => {
            "Click a point and press Delete to remove it; drag points to move them, a ◇ to add one. In this view only."
        }
        ElementKind::Sketch(_) | ElementKind::Railing(_) => {
            "Click a corner and press Delete to remove it; drag corners to move them, a ◇ to add one. Right-click for more."
        }
        ElementKind::Wall(w) if w.assembly.is_some() => {
            "Drag its grips in any view. Layers (the button below, or Tab): a grip at each end of each layer, and at its bottom and top; drag one to run that layer on or hold it back (Tab while dragging types how far). A filled grip was moved; right-click it to reset it or every layer."
        }
        _ => "Drag its grips in any view.",
    }
}

/// Open `id` in Edit mode, if it has grips; what the status bar says.
pub fn enter(model: &Model, id: ElementId, edit: &mut Option<ElementId>) -> String {
    match model.get(id) {
        Some(e) if editable(model, id) => {
            *edit = Some(id);
            hint(model, e)
        }
        Some(e) => format!("{} has no grips to edit: change it in Properties.", e.name),
        None => String::new(),
    }
}

/// Does `e` have grips to edit?
pub fn editable(model: &Model, id: ElementId) -> bool {
    !crate::handles::handles_in(model, id, None, true).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handles::{apply, handles_in, originals};
    use creabim_core::sample::sample_house;

    fn first(m: &Model, f: impl Fn(&ElementKind) -> bool) -> ElementId {
        m.iter().find(|e| f(&e.kind)).unwrap().id
    }

    fn drag(doc: &mut Document, id: ElementId, kind: HandleKind, to: DVec3) {
        let h = handles_in(doc.model(), id, None, true)
            .into_iter()
            .find(|h| h.kind == kind)
            .unwrap_or_else(|| panic!("no {kind:?} grip"));
        let (orig, joined) = originals(doc.model(), id, kind);
        let model = doc.model().clone();
        let out = apply(&model, &orig, &joined, kind, h.at, to, 0.0).unwrap();
        let new = out[0].clone();
        doc.edit(id, "drag", |e| *e = new.clone()).unwrap();
    }

    fn shaft(m: &Model, id: ElementId) -> creabim_core::Shaft {
        match &m.get(id).unwrap().kind {
            ElementKind::Shaft(s) => s.clone(),
            _ => panic!(),
        }
    }

    #[test]
    fn a_shaft_is_reshaped_and_resized_by_its_grips_in_edit_mode() {
        let m = creabim_core::sample::typed_house();
        let id = first(&m, |k| matches!(k, ElementKind::Shaft(_)));
        let mut doc = Document::from_model(m);
        let before = shaft(doc.model(), id);
        let n = before.boundary.len();
        // selected, a shaft has its corners and its base and top
        let plain = handles_in(doc.model(), id, None, false);
        assert!(plain.iter().any(|h| h.kind == HandleKind::ShaftTop));
        assert!(plain.iter().any(|h| matches!(
            h.kind,
            HandleKind::Vertex {
                ring: Ring::Outline,
                ..
            }
        )));
        assert!(!plain
            .iter()
            .any(|h| matches!(h.kind, HandleKind::AddVertex { .. })));
        // in Edit mode its edges and add-corner grips too
        let edit = handles_in(doc.model(), id, None, true);
        assert_eq!(
            edit.iter()
                .filter(|h| matches!(h.kind, HandleKind::AddVertex { .. }))
                .count(),
            n
        );
        // a corner moves
        let c = before.boundary[0];
        drag(
            &mut doc,
            id,
            HandleKind::Vertex {
                ring: Ring::Outline,
                i: 0,
            },
            (c + DVec2::new(-0.5, -0.25)).extend(0.0),
        );
        assert!(shaft(doc.model(), id).boundary[0].distance(c + DVec2::new(-0.5, -0.25)) < 1e-9);
        // an edge pushes out square to itself: the shaft gets bigger
        let area = |s: &creabim_core::Shaft| creabim_core::polygon::signed_area2(&s.boundary).abs();
        let a0 = area(&shaft(doc.model(), id));
        let h = handles_in(doc.model(), id, None, true)
            .into_iter()
            .find(|h| {
                h.kind
                    == HandleKind::Edge {
                        ring: Ring::Outline,
                        i: 1,
                    }
            })
            .unwrap();
        let Constraint::Line { dir, .. } = h.constraint else {
            panic!()
        };
        drag(
            &mut doc,
            id,
            HandleKind::Edge {
                ring: Ring::Outline,
                i: 1,
            },
            h.at + dir * 0.4,
        );
        assert!(area(&shaft(doc.model(), id)) > a0 + 0.1);
        // a corner pulled out of an edge, and taken away again
        let mid = handles_in(doc.model(), id, None, true)
            .into_iter()
            .find(|h| {
                h.kind
                    == HandleKind::AddVertex {
                        ring: Ring::Outline,
                        i: 2,
                    }
            })
            .unwrap()
            .at;
        drag(
            &mut doc,
            id,
            HandleKind::AddVertex {
                ring: Ring::Outline,
                i: 2,
            },
            mid + DVec3::new(0.3, 0.3, 0.0),
        );
        assert_eq!(shaft(doc.model(), id).boundary.len(), n + 1);
        add_or_remove(
            &mut doc,
            id,
            HandleKind::Vertex {
                ring: Ring::Outline,
                i: 3,
            },
        )
        .unwrap();
        assert_eq!(shaft(doc.model(), id).boundary.len(), n);
        // its top rises
        let (_, z1) = shaft(doc.model(), id).span(doc.model());
        let top = handles_in(doc.model(), id, None, true)
            .into_iter()
            .find(|h| h.kind == HandleKind::ShaftTop)
            .unwrap()
            .at;
        drag(&mut doc, id, HandleKind::ShaftTop, top + DVec3::Z * 0.5);
        let (_, z1b) = shaft(doc.model(), id).span(doc.model());
        assert!((z1b - z1 - 0.5).abs() < 1e-9);
        // the slabs it cuts follow
        assert!(!creabim_core::slab::cut_by_shaft(doc.model(), &shaft(doc.model(), id)).is_empty());
    }

    #[test]
    fn a_slab_hole_is_reshaped_in_edit_mode_and_keeps_three_corners() {
        let m = sample_house();
        let id = first(&m, |k| matches!(k, ElementKind::Slab(_)));
        let mut doc = Document::from_model(m);
        doc.edit(id, "hole", |e| {
            if let ElementKind::Slab(s) = &mut e.kind {
                let c = s.boundary.iter().copied().sum::<DVec2>() / s.boundary.len() as f64;
                s.holes = vec![vec![
                    c + DVec2::new(-0.5, -0.5),
                    c + DVec2::new(0.5, -0.5),
                    c + DVec2::new(0.5, 0.5),
                    c + DVec2::new(-0.5, 0.5),
                ]];
            }
        })
        .unwrap();
        let hole = |doc: &Document| match &doc.model().get(id).unwrap().kind {
            ElementKind::Slab(s) => s.holes[0].clone(),
            _ => panic!(),
        };
        // only in Edit mode
        assert!(!handles_in(doc.model(), id, None, false)
            .iter()
            .any(|h| matches!(
                h.kind,
                HandleKind::Vertex {
                    ring: Ring::Hole(0),
                    ..
                }
            )));
        let c0 = hole(&doc)[0];
        drag(
            &mut doc,
            id,
            HandleKind::Vertex {
                ring: Ring::Hole(0),
                i: 0,
            },
            (c0 - DVec2::splat(0.2)).extend(3.0),
        );
        assert!(hole(&doc)[0].distance(c0 - DVec2::splat(0.2)) < 1e-9);
        add_or_remove(
            &mut doc,
            id,
            HandleKind::AddVertex {
                ring: Ring::Hole(0),
                i: 0,
            },
        )
        .unwrap();
        assert_eq!(hole(&doc).len(), 5);
        for _ in 0..2 {
            add_or_remove(
                &mut doc,
                id,
                HandleKind::Vertex {
                    ring: Ring::Hole(0),
                    i: 0,
                },
            )
            .unwrap();
        }
        assert_eq!(hole(&doc).len(), 3);
        assert!(add_or_remove(
            &mut doc,
            id,
            HandleKind::Vertex {
                ring: Ring::Hole(0),
                i: 0,
            },
        )
        .is_err());
        // the slab's own outline gets add-corner grips, its edge grips
        // standing off them
        let edit = handles_in(doc.model(), id, None, true);
        assert!(edit.iter().any(|h| h.kind
            == HandleKind::AddVertex {
                ring: Ring::Outline,
                i: 0
            }));
        assert!(edit
            .iter()
            .filter(|h| matches!(
                h.kind,
                HandleKind::Edge {
                    ring: Ring::Outline,
                    ..
                }
            ))
            .all(|h| h.nudge < 0.0));
    }

    #[test]
    fn a_sketch_and_a_roof_take_corners_in_edit_mode() {
        let m = creabim_core::sample::layered_house();
        let level = m
            .levels()
            .min_by(|a, b| a.1.elevation.total_cmp(&b.1.elevation))
            .unwrap()
            .0
            .id;
        let roof = first(&m, |k| matches!(k, ElementKind::Roof(_)));
        let mut doc = Document::from_model(m);
        let sk = creabim_core::Sketch::polyline(
            Some(level),
            creabim_core::Workplane::ground(0.0),
            vec![DVec2::ZERO, DVec2::new(2.0, 0.0), DVec2::new(2.0, 1.0)],
            false,
        );
        let e = Element::new("Sketch", ElementKind::Sketch(sk));
        let sid = e.id;
        doc.commit(creabim_core::Transaction::new("Sketch").insert(e))
            .unwrap();
        drag(
            &mut doc,
            sid,
            HandleKind::Vertex {
                ring: Ring::Outline,
                i: 2,
            },
            DVec3::new(3.0, 1.5, 0.0),
        );
        let pts = |doc: &Document| match &doc.model().get(sid).unwrap().kind {
            ElementKind::Sketch(s) => s.points.clone(),
            _ => panic!(),
        };
        assert!(pts(&doc)[2].distance(DVec2::new(3.0, 1.5)) < 1e-9);
        // an open sketch has an add-corner grip per segment, no edge grips
        let edit = handles_in(doc.model(), sid, None, true);
        assert_eq!(
            edit.iter()
                .filter(|h| matches!(h.kind, HandleKind::AddVertex { .. }))
                .count(),
            2
        );
        assert!(!edit
            .iter()
            .any(|h| matches!(h.kind, HandleKind::Edge { .. })));
        add_or_remove(
            &mut doc,
            sid,
            HandleKind::AddVertex {
                ring: Ring::Outline,
                i: 0,
            },
        )
        .unwrap();
        assert!(pts(&doc)[1].distance(DVec2::new(1.0, 0.0)) < 1e-9);
        // a roof's corner grips become outline corners in Edit mode
        let n = match &doc.model().get(roof).unwrap().kind {
            ElementKind::Roof(r) => r.boundary.len(),
            _ => panic!(),
        };
        let edit = handles_in(doc.model(), roof, None, true);
        assert_eq!(
            edit.iter()
                .filter(|h| matches!(
                    h.kind,
                    HandleKind::Vertex {
                        ring: Ring::Outline,
                        ..
                    }
                ))
                .count(),
            n
        );
        add_or_remove(
            &mut doc,
            roof,
            HandleKind::AddVertex {
                ring: Ring::Outline,
                i: 0,
            },
        )
        .unwrap();
        match &doc.model().get(roof).unwrap().kind {
            ElementKind::Roof(r) => assert_eq!(r.boundary.len(), n + 1),
            _ => panic!(),
        }
    }

    /// Outline `ring` of `id` follows the grammar: a corner pulled out of
    /// an edge, a corner moved, a corner removed, each one undo step.
    fn add_move_remove(doc: &mut Document, id: ElementId, ring: Ring) {
        let count = |doc: &Document| {
            ring_of(doc.model(), doc.model().get(id).unwrap(), ring)
                .unwrap()
                .corners
        };
        let before = count(doc);
        let n = before.len();
        // add: pull a corner a little out of the first edge's middle
        let mid = handles_in(doc.model(), id, None, true)
            .into_iter()
            .find(|h| h.kind == HandleKind::AddVertex { ring, i: 0 })
            .unwrap_or_else(|| panic!("{ring:?} has no add-corner grip"))
            .at;
        let along = (before[1] - before[0]).normalize();
        let normal = ring_of(doc.model(), doc.model().get(id).unwrap(), ring)
            .unwrap()
            .normal;
        drag(
            doc,
            id,
            HandleKind::AddVertex { ring, i: 0 },
            mid + normal.cross(along) * 0.05,
        );
        assert_eq!(count(doc).len(), n + 1, "{ring:?}: added");
        // move: the new corner a little along its edge
        let c = count(doc)[1];
        drag(doc, id, HandleKind::Vertex { ring, i: 1 }, c + along * 0.02);
        assert!(
            count(doc)[1].distance(c + along * 0.02) < 1e-6,
            "{ring:?}: moved"
        );
        // remove: picked and deleted, one step with a clear label
        let label = remove_corners(doc, id, &[(ring, 1)]).unwrap();
        assert!(label.contains("Remove a corner of"), "{label}");
        assert_eq!(count(doc).len(), n, "{ring:?}: removed");
        for (a, b) in count(doc).iter().zip(&before) {
            assert!(a.distance(*b) < 1e-6);
        }
    }

    fn with_hole(doc: &mut Document, id: ElementId) {
        doc.edit(id, "hole", |e| {
            let (outline, holes) = match &mut e.kind {
                ElementKind::Slab(s) => (s.boundary.clone(), &mut s.holes),
                ElementKind::Roof(r) => (r.boundary.clone(), &mut r.holes),
                _ => panic!(),
            };
            let c = outline.iter().copied().sum::<DVec2>() / outline.len() as f64;
            *holes = vec![creabim_core::polygon::rectangle(
                c - DVec2::splat(0.4),
                c + DVec2::splat(0.4),
            )];
        })
        .unwrap();
    }

    fn level_of(m: &Model) -> ElementId {
        m.levels()
            .min_by(|a, b| a.1.elevation.total_cmp(&b.1.elevation))
            .unwrap()
            .0
            .id
    }

    #[test]
    fn every_outline_adds_moves_and_removes_corners_the_same_way() {
        let m = creabim_core::sample::typed_house();
        let level = level_of(&m);
        let slab = first(&m, |k| matches!(k, ElementKind::Slab(_)));
        let shaft = first(&m, |k| matches!(k, ElementKind::Shaft(_)));
        let mut doc = Document::from_model(m);
        // slab outline and hole
        with_hole(&mut doc, slab);
        add_move_remove(&mut doc, slab, Ring::Outline);
        add_move_remove(&mut doc, slab, Ring::Hole(0));
        // shaft
        add_move_remove(&mut doc, shaft, Ring::Outline);
        // roof outline and hole
        let m = creabim_core::sample::layered_house();
        let roof = first(&m, |k| matches!(k, ElementKind::Roof(_)));
        let mut rdoc = Document::from_model(m);
        with_hole(&mut rdoc, roof);
        add_move_remove(&mut rdoc, roof, Ring::Outline);
        add_move_remove(&mut rdoc, roof, Ring::Hole(0));
        // a closed sketch, an open one, a railing's path
        for closed in [true, false] {
            let sk = creabim_core::Sketch::polyline(
                Some(level),
                creabim_core::Workplane::ground(0.0),
                vec![DVec2::ZERO, DVec2::new(2.0, 0.0), DVec2::new(2.0, 1.5)],
                closed,
            );
            let e = Element::new("Sketch", ElementKind::Sketch(sk));
            let sid = e.id;
            doc.commit(creabim_core::Transaction::new("Sketch").insert(e))
                .unwrap();
            add_move_remove(&mut doc, sid, Ring::Outline);
        }
        let r = creabim_core::Railing::new(
            level,
            vec![
                DVec3::ZERO,
                DVec3::new(3.0, 0.0, 0.0),
                DVec3::new(3.0, 2.0, 0.0),
            ],
        );
        let e = Element::new("Railing", ElementKind::Railing(r));
        let rid = e.id;
        doc.commit(creabim_core::Transaction::new("Railing").insert(e))
            .unwrap();
        add_move_remove(&mut doc, rid, Ring::Outline);
        // an open path keeps two points, a closed one three
        let ElementKind::Railing(r) = &doc.model().get(rid).unwrap().kind else {
            panic!()
        };
        assert_eq!(r.path.len(), 3);
        remove_corners(&mut doc, rid, &[(Ring::Outline, 0)]).unwrap();
        assert!(remove_corners(&mut doc, rid, &[(Ring::Outline, 0)]).is_err());
    }

    #[test]
    fn outlines_use_one_kind_of_grip_selected_or_in_edit_mode() {
        let m = creabim_core::sample::layered_house();
        let roof = first(&m, |k| matches!(k, ElementKind::Roof(_)));
        let slab = first(&m, |k| matches!(k, ElementKind::Slab(_)));
        for id in [roof, slab] {
            let n = rings(&m, m.get(id).unwrap())[0].corners.len();
            for edit in [false, true] {
                let hs = handles_in(&m, id, None, edit);
                let corners = hs
                    .iter()
                    .filter(|h| {
                        matches!(
                            h.kind,
                            HandleKind::Vertex {
                                ring: Ring::Outline,
                                ..
                            }
                        )
                    })
                    .count();
                assert_eq!(corners, n, "edit {edit}");
                // corners come and go only in Edit mode
                let adds = hs
                    .iter()
                    .any(|h| matches!(h.kind, HandleKind::AddVertex { .. }));
                assert_eq!(adds, edit);
                // no other grip sits on a corner
                assert!(!hs
                    .iter()
                    .any(|h| matches!(h.kind, HandleKind::Member { i, .. } if i < n)));
            }
        }
    }

    #[test]
    fn a_crossing_outline_a_stray_hole_and_a_dropped_corner_are_handled() {
        let m = sample_house();
        let id = first(&m, |k| matches!(k, ElementKind::Slab(_)));
        let mut doc = Document::from_model(m);
        with_hole(&mut doc, id);
        let slab = |doc: &Document| match &doc.model().get(id).unwrap().kind {
            ElementKind::Slab(s) => s.clone(),
            _ => panic!(),
        };
        let s = slab(&doc);
        let model = doc.model().clone();
        let try_drag = |kind: HandleKind, from: DVec3, to: DVec3| {
            let (orig, joined) = originals(&model, id, kind);
            apply(&model, &orig, &joined, kind, from, to, 0.0)
        };
        // corner 1 pulled past corner 2: the outline would cross itself
        let (a, c) = (s.boundary[0], s.boundary[2]);
        let r = try_drag(
            HandleKind::Vertex {
                ring: Ring::Outline,
                i: 1,
            },
            s.boundary[1].extend(0.0),
            (c + (c - s.boundary[1])).extend(0.0),
        );
        let err = r.unwrap_err();
        assert!(err.contains("cross"), "self-crossing refused: {err}");
        // the hole dragged out of the slab
        let far = (s.boundary[0] - (c - a)).extend(0.0);
        let r = try_drag(
            HandleKind::Ring {
                ring: Ring::Hole(0),
            },
            s.holes[0][0].extend(0.0),
            far,
        );
        assert!(r.unwrap_err().contains("outside"), "a hole outside refused");
        // a second hole over the first
        let mut two = model.get(id).unwrap().clone();
        if let ElementKind::Slab(s2) = &mut two.kind {
            let h = s2.holes[0].clone();
            s2.holes
                .push(h.iter().map(|p| *p + DVec2::splat(0.2)).collect());
        }
        assert!(problem(&model, &two).unwrap().contains("overlap"));
        // a hole corner dropped on its neighbour merges with it
        let r = try_drag(
            HandleKind::Vertex {
                ring: Ring::Hole(0),
                i: 0,
            },
            s.holes[0][0].extend(0.0),
            (s.holes[0][1] + DVec2::splat(0.005)).extend(0.0),
        )
        .unwrap();
        let ElementKind::Slab(merged) = &r[0].kind else {
            panic!()
        };
        assert_eq!(merged.holes[0].len(), 3);
    }

    #[test]
    fn holes_are_removed_and_slab_edges_bend_and_straighten() {
        let m = sample_house();
        let id = first(&m, |k| matches!(k, ElementKind::Slab(_)));
        let mut doc = Document::from_model(m);
        with_hole(&mut doc, id);
        let said = remove_holes(&mut doc, id, &[0]).unwrap();
        assert!(said.contains("Remove hole 1 of"), "{said}");
        let ElementKind::Slab(s) = &doc.model().get(id).unwrap().kind else {
            panic!()
        };
        assert!(s.holes.is_empty());
        set_arc(&mut doc, id, 0, true).unwrap();
        let ElementKind::Slab(s) = &doc.model().get(id).unwrap().kind else {
            panic!()
        };
        assert!(s.bulge(0) > 0.1);
        set_arc(&mut doc, id, 0, false).unwrap();
        let ElementKind::Slab(s) = &doc.model().get(id).unwrap().kind else {
            panic!()
        };
        assert!(!s.has_arcs());
        // a corner added on a straight edge lands where it was asked for
        let (a, b) = (s.boundary[0], s.boundary[1]);
        let at = a.lerp(b, 0.25).extend(0.0);
        add_corner(&mut doc, id, Ring::Outline, 0, Some(at)).unwrap();
        let ElementKind::Slab(s) = &doc.model().get(id).unwrap().kind else {
            panic!()
        };
        assert!(s.boundary[1].distance(at.truncate()) < 1e-9);
    }

    #[test]
    fn parts_and_site_outlines_take_the_same_grips() {
        let m = creabim_core::garden::garden_house();
        let by = |m: &Model, n: &str| m.iter().find(|e| e.name == n).unwrap().id;
        let mut doc = Document::from_model(m);
        let outline = |m: &Model, id| rings(m, m.get(id).unwrap())[0].corners.clone();
        for name in ["Patio", "Ground", "Chimney stack", "Firebox"] {
            let id = by(doc.model(), name);
            let before = outline(doc.model(), id);
            let to = before[0] + DVec3::new(-0.2, -0.1, 0.0);
            drag(
                &mut doc,
                id,
                HandleKind::Vertex {
                    ring: Ring::Outline,
                    i: 0,
                },
                to,
            );
            let after = outline(doc.model(), id);
            assert!(after[0].truncate().distance(to.truncate()) < 1e-9, "{name}");
            add_corner(&mut doc, id, Ring::Outline, 1, None).unwrap();
            assert_eq!(outline(doc.model(), id).len(), before.len() + 1, "{name}");
        }
        // plants and a sweep along a wall have no outline to edit
        for name in ["Oak", "Coping"] {
            let id = by(doc.model(), name);
            assert!(rings(doc.model(), doc.model().get(id).unwrap()).is_empty());
        }
    }
}
