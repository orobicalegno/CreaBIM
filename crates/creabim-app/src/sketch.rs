//! Drawing sketches: lines, arcs and circles on a workplane, set on any face
//! in 3D, on a level in a plan, or on the section plane in a section. A
//! closed sketch extrudes into a mass you can keep editing. Constraints
//! (lengths, radii, horizontal, vertical, perpendicular, parallel, equal)
//! are added in the Properties panel and shown on the sketch.

use crate::planview::{PaneKind, Xf};
use creabim_core::{
    bulge_through, Constraint, Curve, Document, Element, ElementId, ElementKind, FreeMesh, Model,
    ModelError, SegmentKind, Sketch, Transaction, Workplane,
};
use eframe::egui::{self, Color32, Pos2, Stroke};
use glam::{DVec2, DVec3};

/// What the next click draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    Line,
    Arc,
    Circle,
    /// Two opposite corners, square to the plane (MU38).
    Rectangle,
}

impl Shape {
    pub const ALL: [Shape; 4] = [Shape::Line, Shape::Rectangle, Shape::Arc, Shape::Circle];

    /// Drawn whole from its first point: a sketch of its own.
    fn own_sketch(self) -> bool {
        matches!(self, Shape::Circle | Shape::Rectangle)
    }

    pub fn label(self) -> &'static str {
        match self {
            Shape::Line => "Line (L)",
            Shape::Arc => "Arc (A)",
            Shape::Circle => "Circle (C)",
            Shape::Rectangle => "Rectangle (R)",
        }
    }

    fn key(self) -> egui::Key {
        match self {
            Shape::Line => egui::Key::L,
            Shape::Arc => egui::Key::A,
            Shape::Circle => egui::Key::C,
            Shape::Rectangle => egui::Key::R,
        }
    }
}

/// Set each frame: whether the Sketch tool is the one in hand, so its
/// shape keys (R: Rectangle) don't also start a rotate.
pub fn set_sketching(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("sketching"), on));
}

pub fn sketching(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp(egui::Id::new("sketching")))
        .unwrap_or(false)
}

fn shape_id() -> egui::Id {
    egui::Id::new("sketch shape")
}

fn pick_id() -> egui::Id {
    egui::Id::new("sketch shape picked")
}

/// Pick the shape from the top bar's tool options, for the next frame.
pub fn pick_shape(ctx: &egui::Context, s: Shape) {
    ctx.data_mut(|d| d.insert_temp(pick_id(), s));
}

/// The shape the Sketch tool draws, shared by every pane.
pub fn shape(ctx: &egui::Context) -> Shape {
    ctx.data(|d| d.get_temp(shape_id())).unwrap_or_default()
}

/// The Line / Arc / Circle row in a pane's corner and the L, A and C keys.
/// Keeps `draft` drawing the chosen shape; returns a hint when it changes.
pub fn shape_controls(
    ui: &egui::Ui,
    rect: egui::Rect,
    pane: Option<ElementId>,
    draft: Option<&mut SketchDraft>,
) -> Option<String> {
    let ctx = ui.ctx().clone();
    let mut chosen = shape(&ctx);
    // picked in the top bar's tool options
    if let Some(s) = ctx.data_mut(|d| d.remove_temp::<Shape>(pick_id())) {
        chosen = s;
    }
    if !ctx.egui_wants_keyboard_input() && ui.rect_contains_pointer(rect) {
        ctx.input(|i| {
            for s in Shape::ALL {
                if !i.modifiers.command && i.key_pressed(s.key()) {
                    chosen = s;
                }
            }
        });
    }
    if !crate::slabtool::options_in_top_bar(&ctx) {
        egui::Area::new(egui::Id::new(("sketch shapes", pane)))
            .fixed_pos(rect.left_top() + egui::vec2(8.0, 8.0))
            .order(egui::Order::Foreground)
            .show(&ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for s in Shape::ALL {
                            if ui.selectable_label(chosen == s, s.label()).clicked() {
                                chosen = s;
                            }
                        }
                    });
                });
            });
    }
    let before = shape(&ctx);
    let mut msg = None;
    // a circle or a rectangle is a sketch of its own
    if chosen.own_sketch() && chosen != before && draft.as_ref().is_some_and(|d| d.points.len() > 1)
    {
        msg = Some(format!(
            "A {} is a sketch of its own: finish this one first (Enter).",
            if chosen == Shape::Circle {
                "circle"
            } else {
                "rectangle"
            }
        ));
        chosen = before;
    }
    if chosen != before {
        ctx.data_mut(|d| d.insert_temp(shape_id(), chosen));
        msg = Some(hint(chosen, draft.as_deref()));
    }
    if let Some(d) = draft {
        if d.shape != chosen {
            d.arc_end = None;
        }
        d.shape = chosen;
    }
    msg
}

/// What to click next, for the status bar.
pub fn hint(shape: Shape, draft: Option<&SketchDraft>) -> String {
    let started = draft.is_some();
    match (shape, draft.and_then(|d| d.arc_end)) {
        (Shape::Line, _) if started => {
            "Click the corners; click the first one to close the sketch, Enter to leave it open."
        }
        (Shape::Line, _) => {
            "Click where the sketch starts, then its corners. A and C draw arcs and circles."
        }
        (Shape::Arc, Some(_)) => "Click a point the arc passes through.",
        (Shape::Arc, None) if started => "Click the arc's end, then a point it passes through.",
        (Shape::Arc, None) => {
            "Click where the arc starts, then its end, then a point it passes through."
        }
        (Shape::Circle, _) if started => "Click a point on the circle.",
        (Shape::Circle, _) => "Click the circle's centre, then a point on it.",
        (Shape::Rectangle, _) if started => "Click its opposite corner.",
        (Shape::Rectangle, _) => "Click a corner, then the opposite corner.",
    }
    .into()
}

/// A sketch being drawn. The plane is in model coordinates (absolute
/// heights) until the sketch is finished and given a level.
#[derive(Clone, Debug)]
pub struct SketchDraft {
    pub plane: Workplane,
    pub points: Vec<DVec2>,
    /// How each segment drawn so far runs.
    pub kinds: Vec<SegmentKind>,
    /// The pane drawing it: a view, or `None` for the 3D view.
    pub pane: Option<ElementId>,
    /// What the next click draws.
    pub shape: Shape,
    /// An arc's end, clicked and waiting for a point it passes through.
    pub arc_end: Option<DVec2>,
    /// A circle's radius, once its second click is in.
    radius: Option<f64>,
}

pub enum Click {
    Added,
    Closed,
}

impl SketchDraft {
    /// Start on `plane` with its first point at the plane's origin.
    pub fn new(plane: Workplane, pane: Option<ElementId>) -> Self {
        Self {
            plane,
            points: vec![DVec2::ZERO],
            kinds: vec![],
            pane,
            shape: Shape::Line,
            arc_end: None,
            radius: None,
        }
    }

    /// Whether a click on the first point would close the outline.
    fn closes_at_first(&self) -> bool {
        match self.shape {
            Shape::Line => self.points.len() >= 3,
            Shape::Arc => self.points.len() >= 2 && self.arc_end.is_none(),
            Shape::Circle | Shape::Rectangle => false,
        }
    }

    /// Where a pointer on the plane lands: on the first point when it is
    /// within `tol`, else on a 5 cm grid when `round` (a point already
    /// snapped to something is kept); Shift keeps the new line square to the
    /// plane's axes.
    pub fn snap(&self, p: DVec2, tol: f64, ortho: bool, round: bool) -> DVec2 {
        if self.closes_at_first() && p.distance(self.points[0]) < tol {
            return self.points[0];
        }
        let mut p = if round { (p / 0.05).round() * 0.05 } else { p };
        if let (true, None, Some(a)) = (ortho, self.arc_end, self.points.last()) {
            let d = p - *a;
            p = if d.x.abs() >= d.y.abs() {
                DVec2::new(p.x, a.y)
            } else {
                DVec2::new(a.x, p.y)
            };
        }
        p
    }

    /// Add a snapped point; clicking the first point again closes the
    /// sketch, and a circle's second click finishes it.
    pub fn click(&mut self, p: DVec2) -> Option<Click> {
        let last = *self.points.last()?;
        let first = self.points[0];
        match self.shape {
            Shape::Rectangle => {
                let d = p - first;
                if self.points.len() != 1 || d.x.abs() < 1e-6 || d.y.abs() < 1e-6 {
                    return None;
                }
                self.points = rectangle(first, p).to_vec();
                self.kinds = vec![SegmentKind::Line; 4];
                Some(Click::Closed)
            }
            Shape::Circle => {
                let r = p.distance(first);
                if self.points.len() != 1 || r < 1e-6 {
                    return None;
                }
                self.radius = Some(r);
                Some(Click::Closed)
            }
            Shape::Arc => match self.arc_end {
                None => {
                    if last.distance(p) < 1e-6 {
                        return None;
                    }
                    self.arc_end = Some(p);
                    Some(Click::Added)
                }
                Some(end) => {
                    let bulge = bulge_through(last, end, p);
                    if bulge.abs() > 50.0 {
                        // too near an end to say which way the arc runs
                        return None;
                    }
                    self.arc_end = None;
                    self.kinds.push(if bulge.abs() < 1e-6 {
                        SegmentKind::Line
                    } else {
                        SegmentKind::Arc { bulge }
                    });
                    if self.points.len() >= 2 && end.distance(first) < 1e-9 {
                        return Some(Click::Closed);
                    }
                    self.points.push(end);
                    Some(Click::Added)
                }
            },
            Shape::Line => {
                if self.points.len() >= 3 && p.distance(first) < 1e-9 {
                    self.kinds.push(SegmentKind::Line);
                    return Some(Click::Closed);
                }
                if last.distance(p) < 1e-6 {
                    return None;
                }
                self.points.push(p);
                self.kinds.push(SegmentKind::Line);
                Some(Click::Added)
            }
        }
    }

    /// The sketch as drawn so far, on its model-coordinate plane.
    fn sketch(&self, closed: bool) -> Sketch {
        let mut s = match self.radius {
            Some(r) => Sketch::circle(None, self.plane, self.points[0], r),
            None => Sketch::polyline(None, self.plane, self.points.clone(), false),
        };
        if self.radius.is_none() {
            s.kinds = self.kinds.clone();
            s.set_closed(closed);
        }
        s
    }

    /// The sketch on the level below it, its plane's height above that
    /// level.
    fn placed(&self, doc: &Document, closed: bool) -> Sketch {
        let (level, elevation) =
            level_below(doc, self.plane.origin.z).map_or((None, 0.0), |(l, z)| (Some(l), z));
        let mut sketch = self.sketch(closed);
        sketch.level = level;
        sketch.plane.origin.z -= elevation;
        sketch
    }

    /// Commit the closed outline as a slab with its top on the draft's
    /// face, which must lie level (`state` gives its type, or a roof).
    pub fn finish_slab(
        self,
        doc: &mut Document,
        state: &crate::planview::PlanState,
    ) -> Result<Option<ElementId>, String> {
        if self.points.len() < 3 && self.radius.is_none() {
            return Ok(None);
        }
        let sk = self.placed(doc, true);
        let slab = creabim_core::slab_from_sketch(doc.model(), &sk, crate::outline::NEW_SLAB_THICKNESS)
            .map_err(|_| {
                "A slab lies level: draw it on a level face (a floor, a terrace, a flat roof), or draw an extrusion on a sloped one.".to_string()
            })?;
        let (e, label) = crate::memberstool::outline_element(doc.model(), state, slab);
        let id = e.id;
        doc.commit(Transaction::new(label).insert(e))
            .map_err(|e| e.to_string())?;
        Ok(Some(id))
    }

    /// Commit the outline as a bare opening in the wall whose face it was
    /// drawn on: the rectangle around it, along the wall and up it.
    pub fn finish_opening(self, doc: &mut Document) -> Result<Option<ElementId>, String> {
        if self.points.len() < 2 && self.radius.is_none() {
            return Ok(None);
        }
        let model = doc.model();
        let n = self.plane.normal();
        let pts: Vec<DVec3> = self
            .sketch(true)
            .outline()
            .iter()
            .map(|p| self.plane.to_model(*p))
            .collect();
        // the wall with a face on the plane, under the outline
        let wall = model
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Wall(w) if w.curtain.is_none() => Some((e.id, w)),
                _ => None,
            })
            .filter(|(_, w)| {
                let across = w.direction().perp().extend(0.0);
                if n.dot(across).abs() < 0.999 {
                    return false;
                }
                let s = (self.plane.origin.truncate() - w.start).dot(w.direction().perp());
                (s.abs() - w.thickness / 2.0).abs() < 0.02
            })
            .map(|(id, w)| {
                let d = w.direction();
                let base = creabim_core::walls::base_z(model, w);
                let us: Vec<f64> = pts
                    .iter()
                    .map(|p| (p.truncate() - w.start).dot(d))
                    .collect();
                let zs: Vec<f64> = pts.iter().map(|p| p.z - base).collect();
                let lo = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
                let hi = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let (u0, u1) = (lo(&us).max(0.0), hi(&us).min(w.length()));
                let (z0, z1) = (lo(&zs).max(0.0), hi(&zs).min(w.height));
                (id, u0, u1, z0, z1)
            })
            .filter(|(_, u0, u1, z0, z1)| u1 - u0 > 0.01 && z1 - z0 > 0.01)
            .max_by(|a, b| ((a.2 - a.1) * (a.4 - a.3)).total_cmp(&((b.2 - b.1) * (b.4 - b.3))));
        let Some((host, u0, u1, z0, z1)) = wall else {
            return Err("Draw a wall opening on a wall's face.".into());
        };
        let o = creabim_core::Opening {
            host,
            kind: creabim_core::OpeningKind::Hole,
            offset: u0,
            sill: z0,
            width: u1 - u0,
            height: z1 - z0,
            material: None,
            frame: Default::default(),
            flip_hand: false,
            flip_facing: false,
            wrap: None,
            frame_at: None,
        };
        let e = Element::new("Opening", ElementKind::Opening(o));
        let id = e.id;
        doc.commit(Transaction::new("Wall opening on a face").insert(e))
            .map_err(|e| e.to_string())?;
        Ok(Some(id))
    }

    /// Commit the sketch, if it has enough points. Returns the new element.
    pub fn finish(self, doc: &mut Document, closed: bool) -> Result<Option<ElementId>, ModelError> {
        if self.points.len() < 2 && self.radius.is_none() {
            return Ok(None);
        }
        let sketch = self.placed(doc, closed);
        let e = Element::new("Sketch", ElementKind::Sketch(sketch));
        let id = e.id;
        doc.commit(Transaction::new("Draw sketch").insert(e))?;
        Ok(Some(id))
    }

    /// Commit the closed outline as an extrusion or void lying on the
    /// draft's plane, `base` off it and `height` deep along its normal.
    /// Returns the new element, or None while it has too few corners.
    pub fn finish_part(
        self,
        doc: &mut Document,
        base: f64,
        height: f64,
        void: bool,
    ) -> Result<Option<ElementId>, ModelError> {
        if self.points.len() < 3 && self.radius.is_none() {
            return Ok(None);
        }
        let Some((level, elevation)) = level_below(doc, self.plane.origin.z) else {
            return Ok(None);
        };
        let outline = self.sketch(true).outline();
        let mut plane = self.plane;
        plane.origin.z -= elevation;
        let mut part = creabim_core::parts::Part::extrusion(level, outline, base, height);
        if let creabim_core::parts::Form::Extrusion { plane: p, .. } = &mut part.form {
            *p = Some(plane);
        }
        let (name, label) = if void {
            part = part.as_void();
            ("Void", "Draw void on a face")
        } else {
            ("Extrusion", "Draw extrusion on a face")
        };
        let e = Element::new(name, ElementKind::Part(part));
        let id = e.id;
        doc.commit(Transaction::new(label).insert(e))?;
        Ok(Some(id))
    }

    /// Commit the points drawn as the path of a sweep of `profile` lying on
    /// the draft's face: its profile stands out of the face, along its
    /// normal. Returns the new element, or None while it has one point.
    pub fn finish_sweep(
        self,
        doc: &mut Document,
        profile: Vec<DVec2>,
        name: &str,
        closed: bool,
    ) -> Result<Option<ElementId>, ModelError> {
        if self.points.len() < 2 + usize::from(closed) {
            return Ok(None);
        }
        let Some((level, elevation)) = level_below(doc, self.plane.origin.z) else {
            return Ok(None);
        };
        // arcs go in short straight pieces
        let sk = self.sketch(closed);
        let mut pts = sk.outline();
        if !closed {
            pts = (0..sk.segment_count())
                .flat_map(|i| sk.curve(i).points())
                .fold(Vec::<DVec2>::new(), |mut acc, p| {
                    if acc.last().is_none_or(|q| q.distance(p) > 1e-9) {
                        acc.push(p);
                    }
                    acc
                });
        }
        let points = pts
            .iter()
            .map(|p| self.plane.to_model(*p) - DVec3::Z * elevation)
            .collect();
        let part = creabim_core::parts::Part::sweep(
            level,
            profile,
            creabim_core::parts::Path::Points {
                points,
                closed,
                normal: Some(self.plane.normal()),
            },
        );
        let e = Element::new(name, ElementKind::Part(part));
        let id = e.id;
        doc.commit(Transaction::new("Draw sweep on a face").insert(e))?;
        Ok(Some(id))
    }

    /// The draft and the line, arc or circle to the pointer, drawn through
    /// `project`.
    pub fn draw(
        &self,
        painter: &egui::Painter,
        project: &dyn Fn(DVec3) -> Option<Pos2>,
        hover: Option<DVec2>,
    ) {
        let accent = crate::theme::interaction();
        let stroke = Stroke::new(2.0, accent);
        let at = |p: DVec2| project(self.plane.to_model(p));
        let line = |pts: &[DVec2]| {
            let scr: Option<Vec<Pos2>> = pts.iter().map(|p| at(*p)).collect();
            if let Some(scr) = scr {
                painter.add(egui::Shape::line(scr, stroke));
            }
        };
        let drawn = self.sketch(false);
        if drawn.points.len() >= 2 {
            for c in drawn.curves() {
                line(&c.points());
            }
        }
        let last = *self.points.last().unwrap_or(&DVec2::ZERO);
        // what the next click would add, and its size
        let ghost = hover.map(|h| match (self.shape, self.arc_end) {
            (Shape::Circle, _) if self.points.len() == 1 => {
                let r = h.distance(self.points[0]);
                let c = Curve::Arc {
                    center: self.points[0],
                    radius: r,
                    start: 0.0,
                    sweep: std::f64::consts::TAU,
                };
                (c, format!("R {}", crate::units::len(r)))
            }
            (Shape::Arc, Some(end)) => {
                let c = Curve::arc(last, end, bulge_through(last, end, h).clamp(-50.0, 50.0));
                let r = match c {
                    Curve::Arc { radius, .. } => format!("R {}", crate::units::len(radius)),
                    Curve::Line(..) => crate::units::len(last.distance(end)),
                };
                (c, r)
            }
            _ => (Curve::Line(last, h), crate::units::len(h.distance(last))),
        });
        // a rectangle shows whole, with its two sides
        let boxed = self.shape == Shape::Rectangle && self.points.len() == 1;
        let ghost = match (boxed, hover) {
            (true, Some(h)) => {
                let c = rectangle(self.points[0], h);
                line(&[c[0], c[1], c[2], c[3], c[0]]);
                let d = (h - self.points[0]).abs();
                Some((
                    Curve::Line(h, h),
                    format!("{} × {}", crate::units::len(d.x), crate::units::len(d.y)),
                ))
            }
            _ => ghost,
        };
        if let (false, Some((c, _))) = (boxed, &ghost) {
            line(&c.points());
        }
        if let Some(e) = self.arc_end.and_then(at) {
            painter.circle_stroke(e, 4.0, stroke);
        }
        if let Some(first) = at(self.points[0]) {
            let closing = self.closes_at_first() && hover == Some(self.points[0]);
            painter.circle_stroke(first, if closing { 8.0 } else { 4.0 }, stroke);
        }
        if let (Some((_, text)), Some(s)) = (ghost, hover.and_then(at)) {
            painter.text(
                s + egui::vec2(10.0, -10.0),
                egui::Align2::LEFT_BOTTOM,
                text,
                egui::FontId::proportional(13.0),
                accent,
            );
        }
        draw_plane(painter, project, &self.plane, accent);
    }
}

/// The corners of the rectangle from `a` to its opposite corner `b`,
/// square to the plane, anticlockwise.
fn rectangle(a: DVec2, b: DVec2) -> [DVec2; 4] {
    let (lo, hi) = (a.min(b), a.max(b));
    [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)]
}

/// A small square showing where a workplane lies.
pub fn draw_plane(
    painter: &egui::Painter,
    project: &dyn Fn(DVec3) -> Option<Pos2>,
    plane: &Workplane,
    color: Color32,
) {
    let r = 0.6;
    let corners = [(-r, -r), (r, -r), (r, r), (-r, r)];
    let pts: Option<Vec<Pos2>> = corners
        .iter()
        .map(|(x, y)| project(plane.to_model(DVec2::new(*x, *y))))
        .collect();
    if let Some(mut pts) = pts {
        pts.push(pts[0]);
        painter.add(egui::Shape::dashed_line(
            &pts,
            Stroke::new(1.0, color.gamma_multiply(0.7)),
            4.0,
            3.0,
        ));
    }
}

/// The level a height belongs to: the highest one at or below it.
pub fn level_below(doc: &Document, z: f64) -> Option<(ElementId, f64)> {
    let levels: Vec<(ElementId, f64)> = doc
        .model()
        .levels()
        .map(|(e, l)| (e.id, l.elevation))
        .collect();
    levels
        .iter()
        .filter(|(_, el)| *el <= z + 1e-6)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .or_else(|| levels.iter().min_by(|a, b| a.1.total_cmp(&b.1)))
        .copied()
}

/// Turn a sketch into a mass: its profile pushed `distance` off its plane.
/// The sketch is used up, as with SketchUp's push/pull. Returns the mass and,
/// for a closed sketch, its far cap (so E keeps pushing it).
pub fn extrude(
    doc: &mut Document,
    id: ElementId,
    distance: f64,
) -> Result<(ElementId, Option<usize>), String> {
    let Some(ElementKind::Sketch(s)) = doc.model().get(id).map(|e| e.kind.clone()) else {
        return Err("Select a sketch to extrude.".into());
    };
    if distance.abs() < 1e-6 {
        return Err("Set a distance to extrude by.".into());
    }
    let name = if s.closed { "Mass" } else { "Surface" };
    let mass = Element::new(
        name,
        ElementKind::FreeMesh(FreeMesh::new(s.level, s.plane.origin, s.extrude(distance))),
    );
    let mid = mass.id;
    let tx = Transaction::new("Extrude sketch")
        .remove_cascade(doc.model(), id)
        .insert(mass);
    doc.commit(tx).map_err(|e| e.to_string())?;
    Ok((mid, s.closed.then_some(1)))
}

// ---- constraints in the Properties panel ---------------------------------

/// What the Properties panel did to a sketch.
pub enum Edit {
    None,
    /// Changed it; commit under this label.
    Done(&'static str),
    /// Refused a change, and why. The sketch is as it was.
    Refused(String),
}

/// Kinds of constraint the Add row offers.
const KINDS: [&str; 8] = [
    "Length",
    "Horizontal",
    "Vertical",
    "Perpendicular",
    "Parallel",
    "Equal length",
    "Radius",
    "Corners together",
];

/// The Add row's choices, kept between frames.
#[derive(Clone, Copy, Default)]
struct AddForm {
    kind: usize,
    a: usize,
    b: usize,
}

/// A segment's name in pickers: its number, shape and size.
fn segment_label(sk: &Sketch, i: usize) -> String {
    match (sk.circle, sk.radius(i)) {
        (Some(c), _) => format!("Circle, R {}", crate::units::len(c.radius)),
        (None, Some(r)) => format!("{} · arc, R {}", i + 1, crate::units::len(r)),
        (None, None) => format!("{} · {}", i + 1, crate::units::len(sk.chord(i))),
    }
}

/// The sketch's shape, its constraints and the row to add one.
pub fn properties(ui: &mut egui::Ui, sk: &mut Sketch) -> Edit {
    let mut edit = Edit::None;
    let arcs = (0..sk.segment_count())
        .filter(|i| matches!(sk.kind(*i), SegmentKind::Arc { .. }))
        .count();
    match sk.circle {
        Some(_) => {
            ui.label("A circle");
        }
        None => {
            let n = sk.segment_count();
            crate::props::note(
                ui,
                format!(
                    "{n} segment{}{}, {}",
                    if n == 1 { "" } else { "s" },
                    match arcs {
                        0 => String::new(),
                        1 => " (1 arc)".into(),
                        k => format!(" ({k} arcs)"),
                    },
                    if sk.closed { "closed" } else { "open" }
                ),
            );
            let mut closed = sk.closed;
            if sk.can_close() && crate::props::switch(ui, &mut closed, "Closed").changed() {
                sk.set_closed(closed);
                edit = Edit::Done("Close sketch");
            }
        }
    }
    if let Some(c) = sk.circle {
        let mut r = c.radius;
        let changed = crate::props::row(ui, "Radius", |ui| {
            crate::props::fill(ui, crate::units::drag(&mut r).range(0.001..=1e4)).changed()
        });
        if changed {
            let fixed = sk
                .constraints
                .iter()
                .position(|c| matches!(c, Constraint::Radius { .. }));
            match fixed {
                Some(i) => match sk.set_dimension(i, r) {
                    Ok(()) => edit = Edit::Done("Dimension"),
                    Err(e) => edit = Edit::Refused(format!("Couldn't change that: {e}")),
                },
                None => {
                    if let Some(c) = &mut sk.circle {
                        c.radius = r;
                    }
                    edit = Edit::Done("Circle radius");
                }
            }
        }
    }

    // each constraint a label/field row, its value filling the field
    // column, a remove icon at its end (MU35)
    ui.add_space(4.0);
    crate::props::row(ui, "Constraints", |ui| {
        ui.label(format!("{}", sk.constraints.len()))
    });
    if sk.constraints.is_empty() {
        crate::props::note(ui, "None yet. Add one below; the sketch moves to meet it.");
    }
    let mut remove = None;
    for i in 0..sk.constraints.len() {
        let c = sk.constraints[i];
        let what = match (c, c.value()) {
            (Constraint::Radius { .. }, Some(_)) if sk.circle.is_some() => "Radius".to_string(),
            (Constraint::Radius { segment, .. }, Some(_)) => format!("Radius {}", segment + 1),
            (_, Some(_)) => format!("Length {}", c.segments()[0] + 1),
            (c, None) => c.glyph().to_string(),
        };
        crate::props::row(ui, &what, |ui| {
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 24.0).max(40.0);
                let h = ui.spacing().interact_size.y;
                match c.value() {
                    Some(v) => {
                        let mut value = v;
                        let r =
                            ui.add_sized([w, h], crate::units::drag(&mut value).range(0.001..=1e4));
                        if r.changed() {
                            match sk.set_dimension(i, value) {
                                Ok(()) => edit = Edit::Done("Dimension"),
                                Err(e) => {
                                    edit = Edit::Refused(format!("Couldn't change that: {e}"))
                                }
                            }
                        }
                    }
                    None => {
                        ui.add_sized([w, h], egui::Label::new(c.describe()).truncate());
                    }
                }
                if crate::props::icon_button(ui, "x", "Remove this constraint").clicked() {
                    remove = Some(i);
                }
            })
        });
    }
    if let Some(i) = remove {
        sk.unconstrain(i);
        edit = Edit::Done("Remove constraint");
    }

    // the Add row
    let id = ui.id().with("sketch constraint form");
    let mut form: AddForm = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let count = sk.segment_count();
    let corners = sk.points.len();
    if sk.circle.is_some() {
        form.kind = 6;
    }
    let two = matches!(form.kind, 3..=5);
    // Blender's rows (MU36): each choice a label/field row filling the
    // field column, Add a button there; nothing runs past the panel
    let picker = |ui: &mut egui::Ui, label: &str, salt: &str, v: &mut usize, corners_only: bool| {
        let n = if corners_only { corners } else { count };
        *v = (*v).min(n.saturating_sub(1));
        let name = |i: usize| {
            if corners_only {
                format!("Corner {}", i + 1)
            } else {
                segment_label(sk, i)
            }
        };
        crate::props::row(ui, label, |ui| {
            let w = ui.available_width();
            egui::ComboBox::from_id_salt(salt)
                .icon(crate::props::combo_icon)
                .width(w)
                .truncate()
                .selected_text(name(*v))
                .show_ui(ui, |ui| {
                    for i in 0..n {
                        ui.selectable_value(v, i, name(i));
                    }
                });
        });
    };
    if sk.circle.is_none() {
        crate::props::row(ui, "Add", |ui| {
            let w = ui.available_width();
            egui::ComboBox::from_id_salt("constraint_kind")
                .icon(crate::props::combo_icon)
                .width(w)
                .truncate()
                .selected_text(KINDS[form.kind])
                .show_ui(ui, |ui| {
                    for (k, name) in KINDS.iter().enumerate() {
                        ui.selectable_value(&mut form.kind, k, *name);
                    }
                });
        });
        let corner = form.kind == 7;
        picker(
            ui,
            if two || corner { "First" } else { "Of" },
            "constraint_a",
            &mut form.a,
            corner,
        );
        if two || corner {
            picker(ui, "Second", "constraint_b", &mut form.b, corner);
        }
    }
    if crate::props::action(
        ui,
        "",
        "Add constraint",
        "The sketch moves to meet it",
        true,
    )
    .clicked()
    {
        let (a, b) = (form.a, form.b);
        let mut c = match form.kind {
            0 => Constraint::Length {
                segment: a,
                value: 0.0,
            },
            1 => Constraint::Horizontal { segment: a },
            2 => Constraint::Vertical { segment: a },
            3 => Constraint::Perpendicular { a, b },
            4 => Constraint::Parallel { a, b },
            5 => Constraint::Equal { a, b },
            6 => Constraint::Radius {
                segment: if sk.circle.is_some() { 0 } else { a },
                value: 0.0,
            },
            _ => Constraint::Coincident { a, b },
        };
        // a new dimension starts at the size the sketch has now
        if let (Some(m), Some(v)) = (c.measure(sk), c.value_mut()) {
            *v = m;
        }
        match sk.constrain(c) {
            Ok(()) => edit = Edit::Done("Add constraint"),
            Err(e) => edit = Edit::Refused(format!("Couldn't add that: {e}")),
        }
    }
    ui.data_mut(|d| d.insert_temp(id, form));
    if !sk.constraints.is_empty() {
        crate::props::note(
            ui,
            "Drag a value or type it; the sketch follows. Its first corner stays put.",
        );
    }
    edit
}

// ---- constraint marks on the sketch ----------------------------------------

const MARK_COLOR: Color32 = Color32::from_rgb(40, 70, 140);
const SELECTED_COLOR: Color32 = Color32::from_rgb(242, 140, 51);

/// A small label with a pale backing, centred on `at`.
fn tag(painter: &egui::Painter, at: Pos2, text: &str, color: Color32) {
    let galley = painter.layout_no_wrap(text.to_owned(), egui::FontId::proportional(11.0), color);
    let r = egui::Rect::from_center_size(at, galley.size()).expand2(egui::vec2(3.0, 1.0));
    painter.rect_filled(r, 3.0, Color32::from_white_alpha(210));
    painter.rect_stroke(
        r,
        3.0,
        Stroke::new(1.0, color.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    painter.galley(r.min + egui::vec2(3.0, 1.0), galley, color);
}

/// Constraint marks (H, V, ⟂, ∥, =) and dimension values beside the
/// segments they hold, for every sketch `show` accepts, drawn through
/// `project` (model coordinates).
pub fn draw_marks(
    painter: &egui::Painter,
    model: &Model,
    selection: Option<ElementId>,
    project: &dyn Fn(DVec3) -> Option<Pos2>,
    show: &dyn Fn(&Sketch) -> bool,
) {
    for e in model.iter() {
        let ElementKind::Sketch(sk) = &e.kind else {
            continue;
        };
        if sk.constraints.is_empty() || !show(sk) {
            continue;
        }
        let z = sk
            .level
            .and_then(|l| model.level(l))
            .map_or(0.0, |l| l.elevation);
        let at = |p: DVec2| project(sk.plane.to_model(p) + DVec3::Z * z);
        let color = if selection == Some(e.id) {
            SELECTED_COLOR
        } else {
            MARK_COLOR
        };
        // marks stack outwards from each segment
        let mut stacked = vec![0usize; sk.segment_count()];
        for c in &sk.constraints {
            let text = match *c {
                Constraint::Length { value, .. } => format!("{value:.2}"),
                Constraint::Radius { value, .. } => format!("R {value:.2}"),
                c => c.glyph().to_owned(),
            };
            for s in c.segments() {
                let curve = sk.curve(s);
                let (Some(m), Some(p), Some(q)) =
                    (at(curve.at(0.5)), at(curve.at(0.45)), at(curve.at(0.55)))
                else {
                    continue;
                };
                let along = (q - p).normalized();
                let mut out = egui::vec2(along.y, -along.x);
                if out == egui::Vec2::ZERO {
                    out = egui::vec2(0.0, -1.0);
                }
                // a circle's mark sits outside it, above
                if sk.circle.is_some() {
                    out = egui::vec2(0.0, -1.0);
                }
                let k = stacked.get(s).copied().unwrap_or(0);
                if let Some(n) = stacked.get_mut(s) {
                    *n += 1;
                }
                tag(painter, m + out * (12.0 + 17.0 * k as f32), &text, color);
            }
            for corner in c.corners() {
                if let Some(p) = sk.points.get(corner).and_then(|p| at(*p)) {
                    painter.circle_filled(p, 3.5, color);
                }
            }
        }
    }
}

/// The marks in a plan or section pane, and the selected sketch's lines
/// picked out (plans draw sketch lines plainly).
pub fn draw_marks_in_pane(
    painter: &egui::Painter,
    model: &Model,
    selection: Option<ElementId>,
    kind: PaneKind,
    xf: &Xf,
) {
    let (to_pane, facing): (Box<dyn Fn(DVec3) -> DVec2>, DVec3) = match kind {
        PaneKind::Plan { .. } => (Box::new(|p: DVec3| p.truncate()), DVec3::Z),
        PaneKind::Section { start, end } => {
            let d = (end - start).normalize_or_zero();
            (
                Box::new(move |p: DVec3| DVec2::new((p.truncate() - start).dot(d), p.z)),
                d.perp().extend(0.0),
            )
        }
        PaneKind::Axonometric | PaneKind::Schedule => return,
    };
    let project = |p: DVec3| Some(xf.to_screen(to_pane(p)));
    let show = |sk: &Sketch| match kind {
        PaneKind::Plan { level } => sk.level == Some(level),
        _ => sk.plane.normal().dot(facing).abs() > 0.999,
    };
    if let Some(ElementKind::Sketch(sk)) = selection.and_then(|id| model.get(id)).map(|e| &e.kind) {
        if show(sk) {
            let z = sk
                .level
                .and_then(|l| model.level(l))
                .map_or(0.0, |l| l.elevation);
            for (a, b) in sk.segments() {
                let (a, b) = (project(a + DVec3::Z * z), project(b + DVec3::Z * z));
                if let (Some(a), Some(b)) = (a, b) {
                    painter.line_segment([a, b], Stroke::new(2.0, SELECTED_COLOR));
                }
            }
        }
    }
    draw_marks(painter, model, selection, &project, &show);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> SketchDraft {
        SketchDraft::new(Workplane::ground(0.0), None)
    }

    /// A wall of the sample house and the plane of its face on the right
    /// (looking from its start), 1 m along and 0.5 m up.
    fn wall_face(doc: &Document) -> (ElementId, Workplane) {
        let (id, w) = doc
            .model()
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Wall(w) if w.curtain.is_none() && w.length() > 3.0 => {
                    Some((e.id, w.clone()))
                }
                _ => None,
            })
            .unwrap();
        let d = w.direction();
        let right = -d.perp();
        let base = creabim_core::walls::base_z(doc.model(), &w);
        let at = (w.start + d * 1.0 + right * (w.thickness / 2.0)).extend(base + 0.5);
        (id, Workplane::facing(at, right.extend(0.0)))
    }

    /// Draw a 1.2 x 1.0 rectangle on `plane` from its origin.
    fn rectangle(plane: Workplane) -> SketchDraft {
        let mut d = SketchDraft::new(plane, None);
        for p in [
            DVec2::new(1.2, 0.0),
            DVec2::new(1.2, 1.0),
            DVec2::new(0.0, 1.0),
        ] {
            d.click(p);
        }
        d
    }

    #[test]
    fn a_void_drawn_on_a_wall_face_lies_on_it_and_cuts_into_it() {
        let mut doc = Document::from_model(creabim_core::sample::sample_house());
        let (_, plane) = wall_face(&doc);
        let id = rectangle(plane)
            .finish_part(&mut doc, -0.5, 0.51, true)
            .unwrap()
            .unwrap();
        let Some(ElementKind::Part(p)) = doc.model().get(id).map(|e| &e.kind) else {
            panic!("a part");
        };
        assert!(p.void);
        let creabim_core::parts::Form::Extrusion {
            plane: Some(q),
            outline,
            ..
        } = &p.form
        else {
            panic!("on a plane");
        };
        assert!((q.normal() - plane.normal()).length() < 1e-9);
        assert_eq!(outline.len(), 4);
        // and it cuts the wall it was drawn on
        let cuts = creabim_core::parts::void_cuts(doc.model());
        assert!(cuts.iter().any(|(_, v)| *v == id), "{cuts:?}");
    }

    #[test]
    fn an_opening_drawn_on_a_wall_face_is_the_rectangle_around_it() {
        let mut doc = Document::from_model(creabim_core::sample::sample_house());
        let (wall, plane) = wall_face(&doc);
        let id = rectangle(plane).finish_opening(&mut doc).unwrap().unwrap();
        let Some(ElementKind::Opening(o)) = doc.model().get(id).map(|e| &e.kind) else {
            panic!("an opening");
        };
        assert_eq!(o.host, wall);
        assert_eq!(o.kind, creabim_core::OpeningKind::Hole);
        // 1 m along, 0.5 m up, 1.2 x 1.0, whichever way the face's axes run
        assert!(
            (o.width - 1.2).abs() < 1e-6 && (o.height - 1.0).abs() < 1e-6,
            "{o:?}"
        );
        assert!((o.sill - 0.5).abs() < 1e-6, "{o:?}");
        assert!(
            (o.offset - 1.0).abs() < 1e-6 || (o.offset - (1.0 - 1.2)).abs() < 1e-6,
            "{o:?}"
        );
        // off any wall it says so
        let air = Workplane::facing(DVec3::new(100.0, 100.0, 1.0), DVec3::X);
        assert!(rectangle(air).finish_opening(&mut doc).is_err());
    }

    #[test]
    fn a_slab_drawn_on_a_level_face_tops_out_on_it_and_not_on_a_slope() {
        let mut doc = Document::from_model(creabim_core::sample::sample_house());
        let state = crate::planview::PlanState::default();
        let level = Workplane::ground(1.2);
        let id = rectangle(level)
            .finish_slab(&mut doc, &state)
            .unwrap()
            .unwrap();
        let Some(ElementKind::Slab(s)) = doc.model().get(id).map(|e| &e.kind) else {
            panic!("a slab");
        };
        let z = doc.model().level(s.level).unwrap().elevation + s.top_offset;
        assert!((z - 1.2).abs() < 1e-9, "{z}");
        let sloped = Workplane::facing(DVec3::new(0.0, 0.0, 3.0), DVec3::new(0.0, -0.5, 1.0));
        assert!(rectangle(sloped).finish_slab(&mut doc, &state).is_err());
    }

    #[test]
    fn an_arc_takes_its_end_then_a_point_on_it() {
        let mut d = draft();
        d.click(DVec2::new(2.0, 0.0));
        d.shape = Shape::Arc;
        assert!(matches!(d.click(DVec2::new(2.0, 2.0)), Some(Click::Added)));
        assert_eq!(d.points.len(), 2);
        d.click(DVec2::new(3.0, 1.0));
        assert_eq!(d.points.len(), 3);
        let s = d.sketch(false);
        let SegmentKind::Arc { bulge } = s.kind(1) else {
            panic!("expected an arc");
        };
        assert!((bulge - 1.0).abs() < 1e-9);
        // an arc back to the start closes it
        d.click(DVec2::ZERO);
        assert!(matches!(d.click(DVec2::new(1.0, 1.5)), Some(Click::Closed)));
        let s = d.sketch(true);
        assert!(s.closed && s.check().is_ok());
    }

    #[test]
    fn a_rectangle_is_two_opposite_corners() {
        let mut d = draft();
        d.shape = Shape::Rectangle;
        assert!(
            d.click(DVec2::new(3.0, 0.0)).is_none(),
            "no width or no height"
        );
        assert!(matches!(
            d.click(DVec2::new(-2.0, 1.5)),
            Some(Click::Closed)
        ));
        let s = d.sketch(true);
        assert!(s.closed && s.check().is_ok());
        assert_eq!(s.points.len(), 4);
        assert!(s.points.contains(&DVec2::new(-2.0, 0.0)));
        assert!(s.points.contains(&DVec2::new(0.0, 1.5)));
    }

    #[test]
    fn a_circle_is_its_centre_and_a_point_on_it() {
        let mut d = draft();
        d.shape = Shape::Circle;
        assert!(matches!(d.click(DVec2::new(0.0, 1.5)), Some(Click::Closed)));
        let s = d.sketch(true);
        assert_eq!(s.circle.map(|c| c.radius), Some(1.5));
        s.check().unwrap();
    }
}
