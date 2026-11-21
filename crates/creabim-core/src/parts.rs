//! Building parts modelled from a profile: extrusions, sweeps and voids.
//!
//! An extrusion is a plan outline (with holes) pulled straight up between
//! two heights above its level: a chimney breast, a hearth, a plinth
//! block, a pier. A sweep carries a section profile along a path: a
//! coping along a wall's top, a plinth along its base, a kerb, or a flight
//! of steps (a stepped profile along a straight line). A sweep can follow
//! a wall, so it stays on the wall when the wall moves or changes height.
//!
//! Either can be a void instead of a solid: then it cuts every wall, slab,
//! roof, site surface and solid part it meets (a firebox in a chimney, a
//! hole through a slab), and is itself only shown as a see-through
//! outline in 3D. Which elements a void cuts is worked out from where
//! things are, so a void moved away stops cutting.
//!
//! CreaBIM gives these the geometry and nothing more: what a part *is* (a
//! chimney, a coping, a step) is its name, its material and the IFC class
//! the user picks for it.

use crate::{transform::Xform, Element, ElementId, ElementKind, Model};
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

/// The IFC class a part exports as unless the user picks another.
pub const PROXY: &str = "IfcBuildingElementProxy";

/// An extrusion or a sweep, solid or void.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub level: ElementId,
    pub form: Form,
    /// Cuts what it meets instead of being solid.
    #[serde(default, skip_serializing_if = "is_false")]
    pub void: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<ElementId>,
    /// The IFC class it exports as (a void is always an IfcOpeningElement).
    #[serde(default = "proxy", skip_serializing_if = "is_proxy")]
    pub ifc_class: String,
    /// The library profile it was made with (MU36): editing that profile
    /// updates it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

fn is_false(v: &bool) -> bool {
    !*v
}
fn proxy() -> String {
    PROXY.into()
}
fn is_proxy(s: &String) -> bool {
    s == PROXY
}

/// How a part's shape is made.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Form {
    /// A plan outline, counter-clockwise, pulled up from `base` above the
    /// level by `height`, less its holes.
    Extrusion {
        outline: Vec<DVec2>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        holes: Vec<Vec<DVec2>>,
        base: f64,
        height: f64,
        /// Drawn on a face: the outline lies on this plane (its origin's
        /// height above the level) and `base` and `height` run along its
        /// normal. None for a plan outline pulled up.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        plane: Option<crate::Workplane>,
    },
    /// A section profile carried along a path. The profile is in the
    /// path's own axes: `x` across it (positive to the left, looking
    /// along the path), `y` up from it.
    Sweep { profile: Vec<DVec2>, path: Path },
}

/// What a sweep runs along.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Path {
    /// Through points in plan, `z` above the level. A closed path runs on
    /// from the last point back to the first.
    Points {
        points: Vec<DVec3>,
        #[serde(default, skip_serializing_if = "is_false")]
        closed: bool,
        /// Drawn on a face: the face's outward normal, which the profile's
        /// `y` runs along (its `x` across the path, on the face). None for
        /// a path whose profile stands upright.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        normal: Option<DVec3>,
    },
    /// Along a wall's centreline, from its start to its end, at its top or
    /// its base: it follows the wall when the wall moves or grows.
    /// `offset` lifts the path off that edge; `extend` runs it on past the
    /// wall's start and end (negative stops it short).
    Wall {
        wall: ElementId,
        edge: WallEdge,
        #[serde(default)]
        offset: f64,
        #[serde(default)]
        extend: [f64; 2],
    },
    /// Along eaves of a roof, just outside them: a gutter. It follows the
    /// roof when its outline, overhang, pitch or height change.
    Roof(RoofEdges),
}

/// A gutter's place on a roof (see [`Path::Roof`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoofEdges {
    pub roof: ElementId,
    /// The roof's edges (of its wall-plate outline, as [`crate::Roof`]
    /// numbers them) the gutter runs along, at the eaves.
    pub edges: Vec<usize>,
    /// How far out from the eaves the path runs (the gutter's middle).
    #[serde(default)]
    pub out: f64,
    /// How far below the underside's edge the gutter's bottom hangs.
    #[serde(default)]
    pub down: f64,
}

/// Which edge of a wall a sweep follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallEdge {
    #[default]
    Top,
    Base,
}

impl WallEdge {
    pub fn label(self) -> &'static str {
        match self {
            WallEdge::Top => "Top",
            WallEdge::Base => "Base",
        }
    }
}

/// Point `p` of a face-drawn outline, `h` off its plane along the normal.
pub fn on_plane(pl: &crate::Workplane, p: DVec2, h: f64) -> DVec3 {
    pl.to_model(p) + pl.normal() * h
}

impl Part {
    /// A solid extrusion of `outline` between `base` and `base + height`
    /// above `level`.
    pub fn extrusion(level: ElementId, mut outline: Vec<DVec2>, base: f64, height: f64) -> Self {
        if crate::polygon::signed_area2(&outline) < 0.0 {
            outline.reverse();
        }
        Self {
            level,
            form: Form::Extrusion {
                outline,
                holes: vec![],
                base,
                height,
                plane: None,
            },
            void: false,
            material: None,
            ifc_class: proxy(),
            profile: None,
        }
    }

    /// A solid sweep of `profile` along `path`.
    pub fn sweep(level: ElementId, profile: Vec<DVec2>, path: Path) -> Self {
        Self {
            level,
            form: Form::Sweep { profile, path },
            void: false,
            material: None,
            ifc_class: proxy(),
            profile: None,
        }
    }

    /// The same part as a void.
    pub fn as_void(mut self) -> Self {
        self.void = true;
        self
    }

    /// "Extrusion", "Sweep", "Void"...: what it is called by default.
    pub fn label(&self) -> &'static str {
        match (&self.form, self.void) {
            (Form::Extrusion { .. }, false) => "Extrusion",
            (Form::Sweep { .. }, false) => "Sweep",
            (Form::Extrusion { .. }, true) => "Void",
            (Form::Sweep { .. }, true) => "Swept void",
        }
    }

    /// The outward normal of the face a sweep's path was drawn on.
    pub fn face_normal(&self) -> Option<DVec3> {
        match &self.form {
            Form::Sweep {
                path: Path::Points { normal, .. },
                ..
            } => *normal,
            _ => None,
        }
    }

    /// The wall a sweep follows.
    pub fn host(&self) -> Option<ElementId> {
        match &self.form {
            Form::Sweep {
                path: Path::Wall { wall, .. },
                ..
            } => Some(*wall),
            Form::Sweep {
                path: Path::Roof(g),
                ..
            } => Some(g.roof),
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let finite = |p: &[DVec2]| p.iter().all(|q| q.is_finite());
        match &self.form {
            Form::Extrusion {
                outline,
                holes,
                base,
                height,
                plane,
            } => {
                if let Some(pl) = plane {
                    let unit = |v: DVec3| v.is_finite() && (v.length() - 1.0).abs() < 1e-6;
                    if !(pl.origin.is_finite()
                        && unit(pl.x)
                        && unit(pl.y)
                        && pl.x.dot(pl.y).abs() < 1e-6)
                    {
                        return Err("an extrusion's plane needs two square unit axes".into());
                    }
                }
                if outline.len() < 3 || !finite(outline) {
                    return Err("an extrusion's outline needs three finite corners".into());
                }
                if !(height.is_finite() && *height > 0.0 && base.is_finite()) {
                    return Err(format!("extrusion height must be positive, got {height}"));
                }
                if holes.iter().any(|h| h.len() < 3 || !finite(h)) {
                    return Err("an extrusion's holes need three finite corners".into());
                }
            }
            Form::Sweep { profile, path } => {
                if profile.len() < 3 || !finite(profile) {
                    return Err("a sweep's profile needs three finite corners".into());
                }
                if crate::polygon::signed_area2(profile).abs() < 1e-12 {
                    return Err("a sweep's profile has no area".into());
                }
                match path {
                    Path::Points {
                        points,
                        closed,
                        normal,
                    } => {
                        if normal.is_some_and(|n| !n.is_finite() || n.length() < 1e-9) {
                            return Err("a sweep's face normal must be a direction".into());
                        }
                        if points.len() < 2 + usize::from(*closed)
                            || points.iter().any(|p| !p.is_finite())
                        {
                            return Err("a sweep's path needs two finite points".into());
                        }
                    }
                    Path::Wall { offset, extend, .. } => {
                        if !offset.is_finite() || !extend.iter().all(|e| e.is_finite()) {
                            return Err("a sweep's wall offsets must be finite".into());
                        }
                    }
                    Path::Roof(r) => {
                        if r.edges.is_empty() || !r.out.is_finite() || !r.down.is_finite() {
                            return Err("a gutter needs an edge of its roof".into());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// The path as built, in model coordinates, and whether it closes.
    /// None for an extrusion, or a sweep whose wall is gone or too short.
    pub fn path(&self, model: &Model) -> Option<(Vec<DVec3>, bool)> {
        let Form::Sweep { path, .. } = &self.form else {
            return None;
        };
        match path {
            Path::Points { points, closed, .. } => {
                let z = crate::members::elevation(model, self.level);
                Some((points.iter().map(|p| *p + DVec3::Z * z).collect(), *closed))
            }
            Path::Wall {
                wall,
                edge,
                offset,
                extend,
            } => {
                let ElementKind::Wall(w) = &model.get(*wall)?.kind else {
                    return None;
                };
                let d = w.direction();
                let (a, b) = (w.start - d * extend[0], w.end + d * extend[1]);
                if (b - a).dot(d) < 1e-3 {
                    return None;
                }
                let base = crate::members::elevation(model, w.level) + w.base_offset;
                let z = match edge {
                    WallEdge::Top => base + w.height,
                    WallEdge::Base => base,
                } + offset;
                Some((vec![a.extend(z), b.extend(z)], false))
            }
            Path::Roof(g) => {
                let ElementKind::Roof(r) = &model.get(g.roof)?.kind else {
                    return None;
                };
                roof_gutter_path(model, r, g)
            }
        }
    }

    /// The corners of its shape's bounding box, in model coordinates.
    pub fn bounds(&self, model: &Model) -> Option<(DVec3, DVec3)> {
        match &self.form {
            Form::Extrusion {
                outline,
                base,
                height,
                plane: Some(pl),
                ..
            } => {
                let z = crate::members::elevation(model, self.level);
                let mut lo = DVec3::splat(f64::INFINITY);
                let mut hi = DVec3::splat(f64::NEG_INFINITY);
                for p in outline {
                    for h in [*base, base + height] {
                        let q = on_plane(pl, *p, h) + DVec3::Z * z;
                        lo = lo.min(q);
                        hi = hi.max(q);
                    }
                }
                lo.is_finite().then_some((lo, hi))
            }
            Form::Extrusion {
                outline,
                base,
                height,
                ..
            } => {
                let z = crate::members::elevation(model, self.level) + base;
                let (lo, hi) = flat_bounds(outline)?;
                Some((lo.extend(z), hi.extend(z + height)))
            }
            Form::Sweep { profile, .. } => {
                let (pts, _) = self.path(model)?;
                let (plo, phi) = flat_bounds(profile)?;
                let mut reach = plo.x.abs().max(phi.x.abs());
                if self.face_normal().is_some() {
                    // on a face the profile stands out along its normal
                    reach = reach.max(plo.y.abs()).max(phi.y.abs());
                    let (mut lo, mut hi) =
                        (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
                    for p in pts {
                        lo = lo.min(p - DVec3::splat(reach * 2.0));
                        hi = hi.max(p + DVec3::splat(reach * 2.0));
                    }
                    return Some((lo, hi));
                }
                let mut lo = DVec3::splat(f64::INFINITY);
                let mut hi = DVec3::splat(f64::NEG_INFINITY);
                for p in pts {
                    lo = lo.min(p - DVec3::new(reach, reach, -plo.y));
                    hi = hi.max(p + DVec3::new(reach, reach, phi.y));
                }
                // a mitre at a sharp corner reaches further than the profile
                let pad = DVec3::new(reach, reach, 0.0);
                Some((lo - pad, hi + pad))
            }
        }
    }

    /// Where it is, for turning and scaling it about itself.
    pub fn pivot(&self, model: &Model) -> Option<DVec3> {
        let (lo, hi) = self.bounds(model)?;
        Some(((lo + hi) / 2.0).truncate().extend(lo.z))
    }
}

fn flat_bounds(pts: &[DVec2]) -> Option<(DVec2, DVec2)> {
    let first = *pts.first()?;
    Some(
        pts.iter()
            .fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p))),
    )
}

// ---- profiles ----

/// A rectangle `width` across and `height` up, centred across the path.
pub fn rectangle(width: f64, height: f64) -> Vec<DVec2> {
    let w = width / 2.0;
    vec![
        DVec2::new(-w, 0.0),
        DVec2::new(w, 0.0),
        DVec2::new(w, height),
        DVec2::new(-w, height),
    ]
}

/// A coping over a wall `wall` thick: `depth` deep, standing `overhang`
/// proud of both faces, with a drip groove's worth of step under each
/// overhang when there is room.
pub fn coping(wall: f64, overhang: f64, depth: f64) -> Vec<DVec2> {
    let (w, o) = (wall / 2.0, overhang.max(0.0));
    if o < 0.01 {
        return rectangle(wall, depth);
    }
    // the overhang's underside is a little up from the wall top
    let lip = (depth / 3.0).min(0.015);
    vec![
        DVec2::new(-w, 0.0),
        DVec2::new(w, 0.0),
        DVec2::new(w, lip),
        DVec2::new(w + o, lip),
        DVec2::new(w + o, depth),
        DVec2::new(-w - o, depth),
        DVec2::new(-w - o, lip),
        DVec2::new(-w, lip),
    ]
}

/// A plinth along a wall `wall` thick: `height` high, standing `proud` out
/// of the wall's right face (its exterior, unless the wall is flipped),
/// or out of both faces with `both`.
pub fn plinth(wall: f64, proud: f64, height: f64, both: bool) -> Vec<DVec2> {
    let w = wall / 2.0;
    let left = if both { -w - proud } else { -w };
    vec![
        DVec2::new(left, 0.0),
        DVec2::new(w + proud, 0.0),
        DVec2::new(w + proud, height),
        DVec2::new(left, height),
    ]
    .into_iter()
    .map(|p| DVec2::new(-p.x, p.y))
    .rev()
    .collect()
}

/// A flight of `count` steps, `rise` high and `going` deep each, `width`
/// wide: its profile across a path that runs along the nosings' line
/// (from side to side), so the steps climb to the path's left. Swept
/// along a straight path `width` long, it is a flight of steps as one
/// element.
pub fn steps(count: u32, rise: f64, going: f64) -> Vec<DVec2> {
    let n = count.max(1);
    // the profile climbs towards -x: the path's right side is the bottom
    let mut pts = vec![DVec2::new(0.0, 0.0)];
    for i in 0..n {
        let x = -(i as f64) * going;
        let z = (i + 1) as f64 * rise;
        pts.push(DVec2::new(x, z));
        pts.push(DVec2::new(x - going, z));
    }
    pts.push(DVec2::new(-(n as f64) * going, 0.0));
    // counter-clockwise, as profiles are
    if crate::polygon::signed_area2(&pts) < 0.0 {
        pts.reverse();
    }
    pts
}

// ---- voids ----

/// Can a void cut elements of this kind?
fn cuttable(kind: &ElementKind) -> bool {
    match kind {
        ElementKind::Wall(w) => w.curtain.is_none(),
        ElementKind::Slab(_) | ElementKind::Roof(_) => true,
        ElementKind::Part(p) => !p.void,
        ElementKind::Site(s) => s.cuttable(),
        _ => false,
    }
}

/// The corners of the box an element fills, roughly (a little generous),
/// for the elements voids can cut.
pub fn element_bounds(model: &Model, e: &Element) -> Option<(DVec3, DVec3)> {
    let z0 = |l: ElementId| crate::members::elevation(model, l);
    match &e.kind {
        ElementKind::Wall(w) => {
            let t = DVec2::splat(w.thickness);
            let base = z0(w.level) + w.base_offset;
            let top = if w.attach_top.is_empty() {
                w.height
            } else {
                // attached tops follow roofs over them
                w.height * 2.0 + 1.0
            };
            Some((
                (w.start.min(w.end) - t).extend(base),
                (w.start.max(w.end) + t).extend(base + top),
            ))
        }
        ElementKind::Slab(s) => {
            let (lo, hi) = flat_bounds(&s.outline())?;
            let top = s.top(z0(s.level));
            Some((lo.extend(top - s.thickness), hi.extend(top)))
        }
        ElementKind::Roof(r) => {
            let (lo, hi) = flat_bounds(&r.boundary)?;
            let o = DVec2::splat(r.overhang + 0.1);
            let base = r.base(model);
            Some((
                (lo - o).extend(base - r.vertical_thickness() - 0.1),
                (hi + o).extend(base + r.peak() + r.vertical_thickness() + 0.1),
            ))
        }
        ElementKind::Part(p) => p.bounds(model),
        ElementKind::Site(s) => s.bounds(model),
        _ => None,
    }
}

/// Every void and what it cuts: (target, void), in model order.
pub fn void_cuts(model: &Model) -> Vec<(ElementId, ElementId)> {
    let voids: Vec<(ElementId, (DVec3, DVec3))> = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Part(p) if p.void => Some((e.id, p.bounds(model)?)),
            _ => None,
        })
        .collect();
    if voids.is_empty() {
        return vec![];
    }
    let mut out = vec![];
    for e in model.iter().filter(|e| cuttable(&e.kind)) {
        let Some((lo, hi)) = element_bounds(model, e) else {
            continue;
        };
        for (v, (vlo, vhi)) in &voids {
            let eps = 1e-6;
            if vlo.cmplt(hi - eps).all() && lo.cmplt(*vhi - eps).all() {
                out.push((e.id, *v));
            }
        }
    }
    out
}

/// Is `id` a void?
pub fn is_void(model: &Model, id: ElementId) -> bool {
    matches!(model.get(id).map(|e| &e.kind), Some(ElementKind::Part(p)) if p.void)
}

// ---- editing ----

/// Move, turn, mirror or scale a part standing on a level at height
/// `lift`. `None` when it can't take `t` (a sweep along a wall moves only
/// with its wall). Turning one about a level axis (a tilt) or mirroring
/// it upside down stands it on a plane of its own, as if drawn on a face.
pub fn transform(part: &mut Part, t: &Xform, lift: f64) -> Option<()> {
    let up = DVec3::Z * lift;
    // points are relative to the level, the change is in the model
    let p3 = |p: DVec3| t.point3(p + up) - up;
    let dz = match t {
        Xform::Move(d) => d.z,
        _ => 0.0,
    };
    let factor = match t {
        Xform::Scale { factor, .. } => Some(*factor),
        _ => None,
    };
    // a mirror: a stretch that keeps sizes, turning one or three axes over
    let reflect = match t {
        Xform::Stretch { factor, .. } => {
            if (factor.abs() - DVec3::ONE).abs().max_element() > 1e-9 {
                return None;
            }
            Some(*factor)
        }
        _ => None,
    };
    let flips = reflect.is_some_and(|f| f.x * f.y * f.z < 0.0);
    let ccw = |pts: &mut Vec<DVec2>| {
        if crate::polygon::signed_area2(pts) < 0.0 {
            pts.reverse();
        }
    };
    // a tilt, or a mirror that turns it upside down, takes it off the level
    let leaves_level = !t.keeps_upright() || reflect.is_some_and(|f| f.z < 0.0);
    if leaves_level {
        match &mut part.form {
            Form::Extrusion { plane, .. } if plane.is_none() => {
                *plane = Some(crate::Workplane::ground(0.0));
            }
            Form::Sweep {
                path: Path::Points { normal, .. },
                ..
            } if normal.is_none() => *normal = Some(DVec3::Z),
            _ => {}
        }
    }
    match &mut part.form {
        Form::Extrusion {
            outline,
            holes,
            base,
            height,
            plane: Some(pl),
        } => {
            // drawn on a face: the plane goes where the change takes it,
            // the outline stays as drawn on it (a scale scales it too)
            pl.origin = p3(pl.origin);
            if t.is_rigid() {
                pl.x = t.direction(pl.x).normalize();
                pl.y = t.direction(pl.y).normalize();
            } else if let Some(f) = reflect {
                pl.x *= f;
                pl.y *= f;
                if flips {
                    // keep the plane right-handed, so the solid still
                    // stands out of the mirrored face: its y turns over,
                    // and the outline with it
                    pl.y = -pl.y;
                    for p in outline.iter_mut().chain(holes.iter_mut().flatten()) {
                        p.y = -p.y;
                    }
                    ccw(outline);
                }
            } else {
                let f = factor?;
                for p in outline.iter_mut().chain(holes.iter_mut().flatten()) {
                    *p *= f;
                }
                *height *= f;
                *base *= f;
            }
        }
        Form::Extrusion {
            outline,
            holes,
            base,
            height,
            plane: None,
        } => {
            for p in outline.iter_mut().chain(holes.iter_mut().flatten()) {
                *p = t.point2(*p);
            }
            ccw(outline);
            *base += dz;
            if let Some(f) = factor {
                *height *= f;
            }
        }
        Form::Sweep { profile, path } => {
            match path {
                Path::Points {
                    points,
                    normal: Some(n),
                    ..
                } => {
                    for p in points.iter_mut() {
                        *p = p3(*p);
                    }
                    *n = match reflect {
                        Some(f) => (*n * f).normalize_or(*n),
                        None => t.direction(*n).normalize_or(*n),
                    };
                }
                Path::Points { points, .. } => {
                    for p in points.iter_mut() {
                        *p = t.point2(p.truncate()).extend(p.z + dz);
                    }
                }
                // it goes where its wall (or roof) goes
                Path::Wall { .. } | Path::Roof(_) => return Some(()),
            }
            if flips {
                // the profile's across axis is the path's left, which a
                // mirror turns to its right
                for p in profile.iter_mut() {
                    p.x = -p.x;
                }
                ccw(profile);
            }
            if let Some(f) = factor {
                for p in profile.iter_mut() {
                    *p *= f;
                }
            }
        }
    }
    Some(())
}

/// A gutter's path along the chosen eaves of roof `r`: each edge of its
/// eave outline, `out` beyond it, at the height of the underside there
/// less `down`; edges next to each other join at their corners, into one
/// run (closed when it goes all round).
pub fn roof_gutter_path(
    model: &Model,
    r: &crate::Roof,
    g: &RoofEdges,
) -> Option<(Vec<DVec3>, bool)> {
    let eaves = r.eave_outline();
    let n = eaves.len();
    if n < 3 {
        return None;
    }
    let mut edges: Vec<usize> = g.edges.iter().copied().filter(|e| *e < n).collect();
    edges.sort_unstable();
    edges.dedup();
    if edges.is_empty() {
        return None;
    }
    let base = r.base(model);
    // the outline, as drawn anticlockwise: outward is to the right
    let ccw = crate::polygon::signed_area2(&eaves) > 0.0;
    let outward = |i: usize| {
        let d = (eaves[(i + 1) % n] - eaves[i]).normalize_or_zero();
        if ccw {
            -d.perp()
        } else {
            d.perp()
        }
    };
    // the underside's height at an eave point (just inside it)
    let z_at = |p: DVec2, i: usize| base + r.underside_at(p - outward(i) * 1e-3) - g.down;
    // a run starting where no chosen edge comes before it
    let start = (0..edges.len())
        .find(|k| {
            let e = edges[*k];
            !edges.contains(&((e + n - 1) % n))
        })
        .unwrap_or(0);
    let closed = edges.len() == n;
    let mut pts: Vec<DVec3> = vec![];
    let order: Vec<usize> = (0..edges.len())
        .map(|k| edges[(start + k) % edges.len()])
        .collect();
    for (k, &e) in order.iter().enumerate() {
        let a = eaves[e];
        let b = eaves[(e + 1) % n];
        let o = outward(e) * g.out;
        let prev_chosen = k > 0 && order[k - 1] == (e + n - 1) % n;
        if !prev_chosen || closed && k == 0 {
            // the corner: where this edge's offset line meets the one before
            let p = if closed {
                let pe = (e + n - 1) % n;
                corner(
                    eaves[pe] + outward(pe) * g.out,
                    eaves[e] + outward(pe) * g.out,
                    a + o,
                    b + o,
                )
            } else {
                a + o
            };
            pts.push(p.extend(z_at(a, e)));
        }
        let next = (e + 1) % n;
        let q = if order.get(k + 1) == Some(&next) || (closed && k + 1 == order.len()) {
            let o2 = outward(next) * g.out;
            corner(a + o, b + o, eaves[next] + o2, eaves[(next + 1) % n] + o2)
        } else {
            b + o
        };
        pts.push(q.extend(z_at(b, e)));
    }
    if closed {
        pts.pop();
    }
    (pts.len() >= 2).then_some((pts, closed))
}

/// Where line `a`-`b` meets line `c`-`d` (`b` when they are parallel).
fn corner(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> DVec2 {
    let (r, s) = (b - a, d - c);
    let den = r.perp_dot(s);
    if den.abs() < 1e-12 {
        return b;
    }
    a + r * ((c - a).perp_dot(s) / den)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Level, Slab, Wall};

    #[test]
    fn a_gutter_runs_along_the_eaves_just_below_and_outside_them() {
        let lv = level();
        let mut r = crate::Roof::new(
            lv.id,
            vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(10.0, 0.0),
                DVec2::new(10.0, 6.0),
                DVec2::new(0.0, 6.0),
            ],
            crate::RoofShape::Gable,
        );
        r.overhang = 0.5;
        let roof = Element::new("R", ElementKind::Roof(r.clone()));
        let m = Model::from_elements([lv.clone(), roof.clone()]).unwrap();
        let eaves = r.sloped_edges();
        let g = RoofEdges {
            roof: roof.id,
            edges: vec![eaves[0]],
            out: 0.1,
            down: 0.05,
        };
        let (pts, closed) = roof_gutter_path(&m, &r, &g).unwrap();
        assert!(!closed);
        assert_eq!(pts.len(), 2);
        // level along the eave, below the base (the eave hangs past it)
        assert!((pts[0].z - pts[1].z).abs() < 1e-6);
        assert!(pts[0].z < r.base(&m));
        // outside the eave outline, by `out`
        let e = r.eave_outline();
        let inside = |p: DVec2| crate::polygon::contains(&e, p);
        assert!(!inside(pts[0].truncate()) && !inside(pts[1].truncate()));
        // two eaves next to each other: one run, turning the corner
        let all: Vec<usize> = (0..4).collect();
        let g = RoofEdges {
            roof: roof.id,
            edges: all,
            out: 0.1,
            down: 0.05,
        };
        let (pts, closed) = roof_gutter_path(&m, &r, &g).unwrap();
        assert!(closed && pts.len() == 4, "{pts:?}");
    }

    fn level() -> Element {
        Element::new(
            "L0",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        )
    }

    #[test]
    fn sweep_follows_its_wall() {
        let lv = level();
        let l = lv.id;
        let mut w = Element::new(
            "W",
            ElementKind::Wall(Wall::new(l, DVec2::ZERO, DVec2::new(4.0, 0.0), 0.2, 2.0)),
        );
        let wid = w.id;
        let p = Part::sweep(
            l,
            coping(0.2, 0.03, 0.06),
            Path::Wall {
                wall: wid,
                edge: WallEdge::Top,
                offset: 0.0,
                extend: [0.0, 0.0],
            },
        );
        let m = Model::from_elements([lv.clone(), w.clone()]).unwrap();
        let (pts, closed) = p.path(&m).unwrap();
        assert!(!closed);
        assert_eq!(
            pts,
            vec![DVec3::new(0.0, 0.0, 2.0), DVec3::new(4.0, 0.0, 2.0)]
        );
        // the wall grows and moves; the coping goes with it
        if let ElementKind::Wall(w) = &mut w.kind {
            w.height = 2.5;
            w.end = DVec2::new(6.0, 0.0);
        }
        let m = Model::from_elements([lv, w]).unwrap();
        let (pts, _) = p.path(&m).unwrap();
        assert_eq!(pts[1], DVec3::new(6.0, 0.0, 2.5));
        assert_eq!(p.host(), Some(wid));
    }

    #[test]
    fn a_void_cuts_only_what_it_meets() {
        let lv = level();
        let l = lv.id;
        let near = Element::new(
            "Chimney",
            ElementKind::Part(Part::extrusion(
                l,
                crate::polygon::rectangle(DVec2::ZERO, DVec2::new(1.2, 0.6)),
                0.0,
                3.0,
            )),
        );
        let far = Element::new(
            "Slab",
            ElementKind::Slab(Slab::new(
                l,
                crate::polygon::rectangle(DVec2::new(10.0, 10.0), DVec2::new(12.0, 12.0)),
                0.2,
            )),
        );
        let firebox = Element::new(
            "Firebox",
            ElementKind::Part(
                Part::extrusion(
                    l,
                    crate::polygon::rectangle(DVec2::new(0.3, -0.1), DVec2::new(0.9, 0.4)),
                    0.3,
                    0.7,
                )
                .as_void(),
            ),
        );
        let (cid, vid) = (near.id, firebox.id);
        let m = Model::from_elements([lv, near, far, firebox]).unwrap();
        assert_eq!(void_cuts(&m), vec![(cid, vid)]);
        assert!(is_void(&m, vid) && !is_void(&m, cid));
    }

    #[test]
    fn steps_climb_in_equal_risers() {
        let p = steps(3, 0.15, 0.3);
        assert!(crate::polygon::signed_area2(&p) > 0.0);
        let top = p.iter().map(|q| q.y).fold(0.0, f64::max);
        assert!((top - 0.45).abs() < 1e-9);
        // the area is the sum of the stacked treads: 0.3 * 0.15 * (3 + 2 + 1)
        let area = crate::polygon::signed_area2(&p) / 2.0;
        assert!((area - 0.3 * 0.15 * 6.0).abs() < 1e-9);
    }

    #[test]
    fn a_part_on_a_face_moves_and_turns_with_its_plane() {
        let l = ElementId::new();
        let sq = crate::polygon::rectangle(DVec2::ZERO, DVec2::ONE);
        let mut p = Part::extrusion(l, sq.clone(), -0.2, 0.3);
        let pl = crate::Workplane::facing(DVec3::new(1.0, 0.0, 1.0), DVec3::X);
        if let Form::Extrusion { plane, .. } = &mut p.form {
            *plane = Some(pl);
        }
        transform(&mut p, &Xform::Move(DVec3::new(0.0, 2.0, 0.5)), 0.0).unwrap();
        transform(
            &mut p,
            &Xform::Rotate {
                center: DVec2::ZERO,
                degrees: 90.0,
            },
            0.0,
        )
        .unwrap();
        let Form::Extrusion {
            outline,
            plane: Some(q),
            base,
            ..
        } = &p.form
        else {
            unreachable!()
        };
        // drawn on the plane as before; the plane turned to face +y
        assert_eq!(outline, &sq);
        assert_eq!(*base, -0.2);
        assert!((q.origin - DVec3::new(-2.0, 1.0, 1.5)).length() < 1e-9);
        assert!((q.normal() - DVec3::Y).length() < 1e-9);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn parts_move_and_turn() {
        let l = ElementId::new();
        let mut p = Part::extrusion(
            l,
            crate::polygon::rectangle(DVec2::ZERO, DVec2::ONE),
            0.0,
            1.0,
        );
        transform(&mut p, &Xform::Move(DVec3::new(1.0, 0.0, 0.5)), 0.0).unwrap();
        let Form::Extrusion { outline, base, .. } = &p.form else {
            unreachable!()
        };
        assert_eq!(outline[0], DVec2::new(1.0, 0.0));
        assert_eq!(*base, 0.5);
        assert!(transform(
            &mut p,
            &Xform::Stretch {
                center: DVec3::ZERO,
                factor: DVec3::new(2.0, 1.0, 1.0)
            },
            0.0
        )
        .is_none());
    }
}
