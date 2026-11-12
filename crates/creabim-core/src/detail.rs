//! Detail components: 2D drafting for what the model does not show, drawn
//! in one view (most often a detail view, see [`crate::element::Callout`])
//! over the drawing the model gives it. A flashing, a damp-proof membrane,
//! a quilt of insulation, blocking, brick courses, a hatched region and a
//! break line. They are annotation: they are never in 3D, in schedules or
//! exported as building elements.
//!
//! Each component is a path of points in its view's drawing coordinates
//! (metres along and up a section, or across a plan) with a width across
//! it; how each kind is drawn lives in `creabim-geom`'s `detail` module.

use crate::{Element, ElementId, ElementKind, Hatch, Model, Transaction};
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// What a detail component is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum DetailKind {
    /// Soft insulation (a batt or quilt): a looping line filling a band
    /// `width` wide along the path.
    #[default]
    Batt,
    /// A membrane, damp-proof course or vapour control layer: a heavy
    /// line along the path.
    Membrane,
    /// A flashing or sheet-metal trim: a solid band `width` thick along
    /// the path.
    Flashing,
    /// Blocking or a timber seen in section: each stretch of the path is a
    /// piece `width` deep, crossed corner to corner.
    Blocking,
    /// A repeating detail (brick or block courses): units `module` long
    /// and `width` deep laid along the path with `joint` between them.
    Repeat { module: f64, joint: f64 },
    /// An area filled with a hatch (concrete, hardcore, earth), the path
    /// its outline.
    Region { hatch: Hatch },
    /// A break line across the path: where a drawing stops short.
    BreakLine,
    /// Plain linework, as a CAD program draws it: a line, a circle, an
    /// arc... in a pen. The points mean what the shape says.
    Draft { shape: DraftShape, pen: LinePen },
}

/// What a piece of plain linework is, and so what its points are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DraftShape {
    /// A line through its points, in order: one stretch or many.
    #[default]
    Line,
    /// A closed outline through its points.
    Closed,
    /// Two opposite corners, square to the drawing. Once drawn it is kept
    /// as a closed outline of four points, so it can turn.
    Rectangle,
    /// A regular polygon: its centre, then a corner. Kept as a closed
    /// outline once drawn.
    Polygon { sides: u8 },
    /// Its centre, then a point on it.
    Circle,
    /// Its start, a point it passes through, its end.
    Arc,
    /// Its centre, the end of one axis, then a point that gives the
    /// other axis its length.
    Ellipse,
    /// A smooth curve through its points.
    Spline,
}

impl DraftShape {
    pub const ALL: [DraftShape; 8] = [
        DraftShape::Line,
        DraftShape::Closed,
        DraftShape::Rectangle,
        DraftShape::Polygon { sides: 6 },
        DraftShape::Circle,
        DraftShape::Arc,
        DraftShape::Ellipse,
        DraftShape::Spline,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DraftShape::Line => "Line",
            DraftShape::Closed => "Closed line",
            DraftShape::Rectangle => "Rectangle",
            DraftShape::Polygon { .. } => "Polygon",
            DraftShape::Circle => "Circle",
            DraftShape::Arc => "Arc",
            DraftShape::Ellipse => "Ellipse",
            DraftShape::Spline => "Spline",
        }
    }

    /// The shapes made of a set number of clicks, and that number.
    pub fn fixed_points(self) -> Option<usize> {
        match self {
            DraftShape::Rectangle | DraftShape::Polygon { .. } | DraftShape::Circle => Some(2),
            DraftShape::Arc | DraftShape::Ellipse => Some(3),
            DraftShape::Line | DraftShape::Closed | DraftShape::Spline => None,
        }
    }

    /// What the next click gives, with `n` points clicked so far.
    pub fn next_point(self, n: usize) -> &'static str {
        match (self, n) {
            (_, 0) => match self {
                DraftShape::Circle | DraftShape::Ellipse | DraftShape::Polygon { .. } => {
                    "its centre"
                }
                DraftShape::Rectangle => "its first corner",
                _ => "its start",
            },
            (DraftShape::Rectangle, _) => "the opposite corner",
            (DraftShape::Polygon { .. }, _) => "a corner",
            (DraftShape::Circle, _) => "a point on it",
            (DraftShape::Arc, 1) => "a point it passes through",
            (DraftShape::Arc, _) => "its end",
            (DraftShape::Ellipse, 1) => "the end of its first axis",
            (DraftShape::Ellipse, _) => "a point for its other axis",
            _ => "the next point",
        }
    }
}

/// The pen plain linework is drawn in: the drawing's own line weights.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LinePen {
    /// As heavy as cut material.
    Heavy,
    /// As what is seen beyond the cut.
    #[default]
    Medium,
    /// As symbols: door swings, break lines.
    Light,
    /// The lightest line.
    Fine,
    /// Dashed, as what is hidden or overhead.
    Dashed,
}

impl LinePen {
    pub const ALL: [LinePen; 5] = [
        LinePen::Heavy,
        LinePen::Medium,
        LinePen::Light,
        LinePen::Fine,
        LinePen::Dashed,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LinePen::Heavy => "Heavy",
            LinePen::Medium => "Medium",
            LinePen::Light => "Light",
            LinePen::Fine => "Fine",
            LinePen::Dashed => "Dashed",
        }
    }
}

impl DetailKind {
    /// The kinds the detail tool offers, with a repeat laid as brick.
    /// The components: what the model doesn't hold, drawn as what it is.
    pub const COMPONENTS: usize = 7;

    /// A piece of plain linework in the usual pen.
    pub const fn draft(shape: DraftShape) -> DetailKind {
        DetailKind::Draft {
            shape,
            pen: LinePen::Medium,
        }
    }

    pub const ALL: [DetailKind; 15] = [
        DetailKind::Batt,
        DetailKind::Membrane,
        DetailKind::Flashing,
        DetailKind::Blocking,
        DetailKind::Repeat {
            module: 0.215,
            joint: 0.01,
        },
        DetailKind::Region {
            hatch: Hatch::Concrete,
        },
        DetailKind::BreakLine,
        DetailKind::draft(DraftShape::ALL[0]),
        DetailKind::draft(DraftShape::ALL[1]),
        DetailKind::draft(DraftShape::ALL[2]),
        DetailKind::draft(DraftShape::ALL[3]),
        DetailKind::draft(DraftShape::ALL[4]),
        DetailKind::draft(DraftShape::ALL[5]),
        DetailKind::draft(DraftShape::ALL[6]),
        DetailKind::draft(DraftShape::ALL[7]),
    ];

    pub fn label(self) -> &'static str {
        match self {
            DetailKind::Draft { shape, .. } => shape.label(),
            DetailKind::Batt => "Insulation batt",
            DetailKind::Membrane => "Membrane",
            DetailKind::Flashing => "Flashing",
            DetailKind::Blocking => "Blocking",
            DetailKind::Repeat { .. } => "Repeating detail",
            DetailKind::Region { .. } => "Filled region",
            DetailKind::BreakLine => "Break line",
        }
    }

    /// Is it the same kind, whatever its settings?
    pub fn same(self, other: DetailKind) -> bool {
        match (self, other) {
            (DetailKind::Draft { shape: a, .. }, DetailKind::Draft { shape: b, .. }) => {
                std::mem::discriminant(&a) == std::mem::discriminant(&b)
            }
            _ => std::mem::discriminant(&self) == std::mem::discriminant(&other),
        }
    }

    /// Plain linework's shape and pen.
    pub fn as_draft(self) -> Option<(DraftShape, LinePen)> {
        match self {
            DetailKind::Draft { shape, pen } => Some((shape, pen)),
            _ => None,
        }
    }

    /// Has it a width across its path (a batt's thickness, a course's
    /// depth)?
    pub fn has_width(self) -> bool {
        !matches!(
            self,
            DetailKind::Membrane
                | DetailKind::Region { .. }
                | DetailKind::BreakLine
                | DetailKind::Draft { .. }
        )
    }

    /// The kinds made of a set number of clicks, and that number.
    pub fn fixed_points(self) -> Option<usize> {
        match self {
            DetailKind::BreakLine => Some(2),
            DetailKind::Draft { shape, .. } => shape.fixed_points(),
            _ => None,
        }
    }

    /// The fewest points it can be made of.
    pub fn min_points(self) -> usize {
        match self.fixed_points() {
            Some(n) => n,
            None if self.is_area() => 3,
            None => 2,
        }
    }

    /// The width it is drawn when first placed, in metres.
    pub fn default_width(self) -> f64 {
        match self {
            DetailKind::Batt => 0.1,
            DetailKind::Membrane => 0.0,
            DetailKind::Flashing => 0.003,
            DetailKind::Blocking => 0.05,
            DetailKind::Repeat { .. } => 0.065,
            DetailKind::Region { .. } => 0.0,
            DetailKind::BreakLine => 0.0,
            DetailKind::Draft { .. } => 0.0,
        }
    }

    /// Is the path closed into an outline?
    pub fn is_area(self) -> bool {
        matches!(
            self,
            DetailKind::Region { .. }
                | DetailKind::Draft {
                    shape: DraftShape::Closed,
                    ..
                }
        )
    }
}

/// One detail component in a view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DetailItem {
    /// The view it is drawn in.
    pub view: ElementId,
    pub kind: DetailKind,
    /// Its path, in the view's drawing coordinates.
    pub points: Vec<DVec2>,
    /// Across the path, metres: a batt's thickness, a course's depth. The
    /// path runs along the middle of it.
    #[serde(default)]
    pub width: f64,
    /// A membrane lies to the right of the way it was drawn, not the left:
    /// the side of the line it was clicked on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flip: bool,
    /// Plain linework's own colour; none takes its layer's, or black.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    /// Plain linework's own line type; none takes its layer's, or its pen's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linetype: Option<LineType>,
    /// The drafting layer plain linework is on, by name ([`DraftLayers`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    /// A closed piece of linework filled with a hatch, over what is under
    /// it (unlike a filled region, it hides nothing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Hatch>,
}

/// How a line is broken along its length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineType {
    #[default]
    Continuous,
    Dashed,
    Dotted,
    /// A long dash and a dot: centre lines and axes.
    DashDot,
}

impl LineType {
    pub const ALL: [LineType; 4] = [
        LineType::Continuous,
        LineType::Dashed,
        LineType::Dotted,
        LineType::DashDot,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LineType::Continuous => "Continuous",
            LineType::Dashed => "Dashed",
            LineType::Dotted => "Dotted",
            LineType::DashDot => "Dash-dot",
        }
    }

    /// Its dashes and gaps in turn, in paper millimetres; none when it
    /// isn't broken.
    pub fn pattern(self) -> Option<&'static [f64]> {
        match self {
            LineType::Continuous => None,
            LineType::Dashed => Some(&[3.0, 1.5]),
            LineType::Dotted => Some(&[0.3, 1.2]),
            LineType::DashDot => Some(&[6.0, 1.5, 0.3, 1.5]),
        }
    }
}

/// How a piece of plain linework is drawn, all told: its weight, its
/// colour and how it is broken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LineLook {
    /// One of the solid pens: heavy, medium, light or fine.
    pub weight: LinePen,
    /// None is the drawing's own ink.
    pub rgb: Option<[u8; 3]>,
    pub linetype: LineType,
}

/// A drafting layer: a name linework is put on, to colour, break and
/// hide it together.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DraftLayer {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    #[serde(default)]
    pub linetype: LineType,
    /// Its linework is not drawn, in any view or on any sheet.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

/// The project's drafting layers: at most one such element per project,
/// read through [`layers`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DraftLayers {
    #[serde(default)]
    pub layers: Vec<DraftLayer>,
}

impl DraftLayers {
    pub fn get(&self, name: &str) -> Option<&DraftLayer> {
        self.layers.iter().find(|l| l.name == name)
    }

    pub fn validate(&self) -> Result<(), String> {
        for (i, l) in self.layers.iter().enumerate() {
            if l.name.trim().is_empty() {
                return Err("a drafting layer needs a name".into());
            }
            if self.layers[..i].iter().any(|o| o.name == l.name) {
                return Err(format!("two drafting layers are named {:?}", l.name));
            }
        }
        Ok(())
    }
}

/// The project's drafting layers; none when it has no such element.
pub fn layers(model: &Model) -> DraftLayers {
    model
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::DraftLayers(l) => Some(l.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// The edit that renames drafting layer `old` to `new`, in the table and
/// on every piece of linework on it; nothing when `new` is blank or taken.
pub fn rename_layer(model: &Model, old: &str, new: &str) -> Transaction {
    let mut table = layers(model);
    let new = new.trim();
    if new.is_empty() || new == old || table.get(new).is_some() {
        return Transaction::new("Rename drafting layer");
    }
    for l in &mut table.layers {
        if l.name == old {
            l.name = new.to_string();
        }
    }
    let mut tx = set_layers(model, table);
    tx.label = "Rename drafting layer".into();
    for e in model.iter() {
        if let ElementKind::Detail(d) = &e.kind {
            if d.layer.as_deref() == Some(old) {
                let mut after = e.clone();
                if let ElementKind::Detail(d) = &mut after.kind {
                    d.layer = Some(new.to_string());
                }
                tx = tx.replace(e.clone(), after);
            }
        }
    }
    tx
}

/// The edit that gives `model` these drafting layers, as one undo step.
pub fn set_layers(model: &Model, layers: DraftLayers) -> Transaction {
    let tx = Transaction::new("Drafting layers");
    let current = model
        .iter()
        .find(|e| matches!(e.kind, ElementKind::DraftLayers(_)));
    match current {
        Some(e) if e.kind == ElementKind::DraftLayers(layers.clone()) => tx,
        Some(e) => {
            let mut after = e.clone();
            after.kind = ElementKind::DraftLayers(layers);
            tx.replace(e.clone(), after)
        }
        None => tx.insert(Element::new(
            "Drafting layers",
            ElementKind::DraftLayers(layers),
        )),
    }
}

/// How a membrane is drawn beside the line it follows (style pack,
/// Annotation): always a heavy line 2 mm off it on paper, so it never lies
/// on that line, with a partner further out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MembraneLook {
    /// A thin dashed partner.
    #[default]
    Dashed,
    /// A second, lighter solid line.
    Double,
    /// A zig-zag between the two.
    ZigZag,
}

impl MembraneLook {
    pub const ALL: [MembraneLook; 3] = [
        MembraneLook::Dashed,
        MembraneLook::Double,
        MembraneLook::ZigZag,
    ];

    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn label(self) -> &'static str {
        match self {
            MembraneLook::Dashed => "Heavy line, dashed partner",
            MembraneLook::Double => "Double line",
            MembraneLook::ZigZag => "Zig-zag",
        }
    }
}

impl DetailItem {
    pub fn new(view: ElementId, kind: DetailKind, points: Vec<DVec2>) -> Self {
        Self {
            view,
            kind,
            points,
            width: kind.default_width(),
            flip: false,
            color: None,
            linetype: None,
            layer: None,
            fill: None,
        }
    }

    /// How plain linework is drawn with the project's `layers`: its own
    /// colour and line type, else its layer's, else its pen's. `None` for
    /// a component, and for linework on a hidden layer, which isn't drawn.
    pub fn look(&self, layers: &DraftLayers) -> Option<LineLook> {
        let (_, pen) = self.kind.as_draft()?;
        let layer = self.layer.as_deref().and_then(|n| layers.get(n));
        if layer.is_some_and(|l| l.hidden) {
            return None;
        }
        // the dashed pen is a light line, dashed
        let (weight, by_pen) = match pen {
            LinePen::Dashed => (LinePen::Light, LineType::Dashed),
            p => (p, LineType::Continuous),
        };
        Some(LineLook {
            weight,
            rgb: self.color.or(layer.and_then(|l| l.color)),
            linetype: self
                .linetype
                .or(layer
                    .map(|l| l.linetype)
                    .filter(|t| *t != LineType::Continuous))
                .unwrap_or(by_pen),
        })
    }

    /// Can it be filled: plain linework that closes on itself?
    pub fn fillable(&self) -> bool {
        self.draft_path().is_some_and(|(_, closed)| closed)
    }

    /// The side a membrane is drawn on: +1 left of the way it was drawn,
    /// -1 right.
    pub fn side(&self) -> f64 {
        if self.flip {
            -1.0
        } else {
            1.0
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let need = self.kind.min_points();
        if self.points.len() < need {
            return Err(format!(
                "a {} needs at least {need} points",
                self.kind.label().to_lowercase()
            ));
        }
        if !self.points.iter().all(|p| p.is_finite()) || !self.width.is_finite() {
            return Err("detail points must be finite".into());
        }
        if self.width < 0.0 {
            return Err("a detail's width can't be negative".into());
        }
        if let DetailKind::Repeat { module, joint } = self.kind {
            if !(module > 1e-3 && joint >= 0.0) {
                return Err("a repeating detail needs a unit length".into());
            }
        }
        Ok(())
    }

    /// The path's length; plain linework's, along the line as drawn.
    pub fn length(&self) -> f64 {
        match self.draft_path() {
            Some((path, closed)) => {
                let open: f64 = path.windows(2).map(|w| w[0].distance(w[1])).sum();
                match (closed, path.first(), path.last()) {
                    (true, Some(a), Some(b)) => open + a.distance(*b),
                    _ => open,
                }
            }
            None => self.points.windows(2).map(|w| w[0].distance(w[1])).sum(),
        }
    }

    /// Plain linework as the line it draws, curves in short chords, and
    /// whether it closes back on its start. `None` for a component.
    pub fn draft_path(&self) -> Option<(Vec<DVec2>, bool)> {
        let (shape, _) = self.kind.as_draft()?;
        let p = &self.points;
        let tau = std::f64::consts::TAU;
        Some(match shape {
            DraftShape::Line => (p.clone(), false),
            DraftShape::Closed => (p.clone(), true),
            DraftShape::Rectangle => match p[..] {
                [a, b, ..] => (vec![a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y)], true),
                _ => (p.clone(), false),
            },
            DraftShape::Polygon { sides } => match p[..] {
                [c, v, ..] => {
                    let n = sides.clamp(3, 64) as usize;
                    let r = v - c;
                    (
                        (0..n)
                            .map(|i| c + DVec2::from_angle(tau * i as f64 / n as f64).rotate(r))
                            .collect(),
                        true,
                    )
                }
                _ => (p.clone(), false),
            },
            DraftShape::Circle => match p[..] {
                [c, q, ..] => {
                    let r = q - c;
                    (
                        (0..CHORDS)
                            .map(|i| {
                                c + DVec2::from_angle(tau * i as f64 / CHORDS as f64).rotate(r)
                            })
                            .collect(),
                        true,
                    )
                }
                _ => (p.clone(), false),
            },
            DraftShape::Arc => match p[..] {
                [a, m, b, ..] => (arc_through(a, m, b), false),
                _ => (p.clone(), false),
            },
            DraftShape::Ellipse => match p[..] {
                [c, a, q, ..] => {
                    let u = a - c;
                    let along = u.normalize_or_zero();
                    let across = along.perp();
                    let (ra, rb) = (u.length(), (q - c).dot(across).abs());
                    (
                        (0..CHORDS)
                            .map(|i| {
                                let t = tau * i as f64 / CHORDS as f64;
                                c + along * ra * t.cos() + across * rb * t.sin()
                            })
                            .collect(),
                        true,
                    )
                }
                [c, a] => (vec![c, a], false),
                _ => (p.clone(), false),
            },
            DraftShape::Spline => (spline_through(p), false),
        })
    }

    /// A rectangle or a regular polygon as the closed outline it is kept
    /// as, so it turns and reshapes like any other; anything else as it is.
    pub fn baked(mut self) -> Self {
        if let DetailKind::Draft {
            shape: DraftShape::Rectangle | DraftShape::Polygon { .. },
            pen,
        } = self.kind
        {
            if let Some((path, true)) = self.draft_path() {
                self.points = path;
                self.kind = DetailKind::Draft {
                    shape: DraftShape::Closed,
                    pen,
                };
            }
        }
        self
    }
}

/// How many chords a full circle is drawn in.
const CHORDS: usize = 96;

/// The arc from `a` to `b` that passes through `m`, in chords; the
/// straight line where the three lie on one.
pub fn arc_through(a: DVec2, m: DVec2, b: DVec2) -> Vec<DVec2> {
    let (ab, am) = (b - a, m - a);
    let cross = am.perp_dot(ab);
    if cross.abs() < 1e-12 * (1.0 + ab.length_squared() + am.length_squared()) {
        return vec![a, m, b];
    }
    // the centre is as far from each of the three
    let (d1, d2) = (am.length_squared() / 2.0, ab.length_squared() / 2.0);
    let c = a + DVec2::new(d1 * ab.y - d2 * am.y, d2 * am.x - d1 * ab.x) / cross;
    let r = a - c;
    // round the way that passes m before b
    let turn = |to: DVec2| {
        let t = r.angle_to(to - c);
        if cross > 0.0 {
            t.rem_euclid(std::f64::consts::TAU)
        } else {
            -(-t).rem_euclid(std::f64::consts::TAU)
        }
    };
    let sweep = turn(b);
    let n = ((sweep.abs() / std::f64::consts::TAU * CHORDS as f64).ceil() as usize).max(2);
    (0..=n)
        .map(|i| c + DVec2::from_angle(sweep * i as f64 / n as f64).rotate(r))
        .collect()
}

/// A smooth curve through `pts` (Catmull-Rom), in chords.
pub fn spline_through(pts: &[DVec2]) -> Vec<DVec2> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    const STEPS: usize = 16;
    let at = |i: isize| pts[i.clamp(0, pts.len() as isize - 1) as usize];
    let mut out = vec![pts[0]];
    for i in 0..pts.len() as isize - 1 {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        for k in 1..=STEPS {
            let t = k as f64 / STEPS as f64;
            let (t2, t3) = (t * t, t * t * t);
            out.push(
                0.5 * (2.0 * p1
                    + (p2 - p0) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3),
            );
        }
    }
    out
}

/// How a view's drawing coordinates stand in the model: a plan lies level
/// at its level's height, a section (or a detail called out of one)
/// stands on its line, x along it and y up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrawingFrame {
    Plan { z: f64 },
    Section { start: DVec2, end: DVec2 },
}

impl DrawingFrame {
    /// The frame of view `view`, if it is a plan or a section.
    pub fn of(model: &crate::Model, view: ElementId) -> Option<DrawingFrame> {
        let crate::ElementKind::View(v) = &model.get(view)?.kind else {
            return None;
        };
        match v.kind {
            crate::ViewKind::Plan { level, .. } => Some(DrawingFrame::Plan {
                z: model.level(level).map_or(0.0, |l| l.elevation),
            }),
            crate::ViewKind::Section { start, end, .. } => {
                Some(DrawingFrame::Section { start, end })
            }
            _ => None,
        }
    }

    /// A drawing point in the model.
    pub fn to_model(self, p: DVec2) -> glam::DVec3 {
        match self {
            DrawingFrame::Plan { z } => p.extend(z),
            DrawingFrame::Section { start, end } => {
                (start + (end - start).normalize_or_zero() * p.x).extend(p.y)
            }
        }
    }

    /// A model point on the drawing (dropped square onto its plane).
    pub fn to_drawing(self, p: glam::DVec3) -> DVec2 {
        match self {
            DrawingFrame::Plan { .. } => p.truncate(),
            DrawingFrame::Section { start, end } => DVec2::new(
                (p.truncate() - start).dot((end - start).normalize_or_zero()),
                p.z,
            ),
        }
    }

    /// Square to the drawing: up out of a plan, the way a section looks.
    pub fn normal(self) -> glam::DVec3 {
        match self {
            DrawingFrame::Plan { .. } => glam::DVec3::Z,
            DrawingFrame::Section { start, end } => {
                (end - start).normalize_or_zero().perp().extend(0.0)
            }
        }
    }
}

impl DetailItem {
    /// Its points in the model, in its view's frame.
    pub fn model_points(&self, frame: DrawingFrame) -> Vec<glam::DVec3> {
        self.points.iter().map(|p| frame.to_model(*p)).collect()
    }

    /// The direction across its first stretch (left of the way it was
    /// drawn), on the drawing; the side its width grip stands.
    pub fn across(&self) -> DVec2 {
        match self.points.as_slice() {
            [a, b, ..] => (*b - *a).normalize_or_zero().perp(),
            _ => DVec2::Y,
        }
    }

    /// A repeating detail's unit and joint, from one unit's start to the
    /// next: where its spacing grip sits along the path.
    pub fn spacing(&self) -> Option<f64> {
        match self.kind {
            DetailKind::Repeat { module, joint } => Some(module + joint),
            _ => None,
        }
    }

    /// The point `d` along the path.
    pub fn at_length(&self, d: f64) -> DVec2 {
        let mut left = d;
        for w in self.points.windows(2) {
            let l = w[0].distance(w[1]);
            if left <= l || l <= 0.0 {
                return w[0] + (w[1] - w[0]).normalize_or_zero() * left.max(0.0);
            }
            left -= l;
        }
        self.points.last().copied().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn details_check_their_paths() {
        let v = ElementId::new();
        let p = |x, y| DVec2::new(x, y);
        assert!(DetailItem::new(v, DetailKind::Batt, vec![p(0.0, 0.0)])
            .validate()
            .is_err());
        assert!(
            DetailItem::new(v, DetailKind::Batt, vec![p(0.0, 0.0), p(1.0, 0.0)])
                .validate()
                .is_ok()
        );
        let region = DetailKind::Region {
            hatch: Hatch::Concrete,
        };
        assert!(DetailItem::new(v, region, vec![p(0.0, 0.0), p(1.0, 0.0)])
            .validate()
            .is_err());
        // a saved detail without a width still reads
        let json = format!(
            r#"{{"view":{},"kind":"Membrane","points":[[0.0,0.0],[1.0,0.0]]}}"#,
            serde_json::to_string(&v).unwrap()
        );
        let d: DetailItem = serde_json::from_str(&json).unwrap();
        assert_eq!(d.width, 0.0);
    }
}
