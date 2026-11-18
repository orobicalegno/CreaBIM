//! The sheet pane: a sheet drawn exactly as it prints, with live views.
//! Drag a view to move it on the sheet; double-click it to work on the model
//! through it, with the same tools as the view itself.

use crate::hoverpick::{self, Pane};
use crate::planview::{self, PaneStyle, PlanState};
use crate::selection::Selection;
use creabim_core::{Document, ElementId, ElementKind};
use creabim_geom::paper::{self, Anchor, Canvas, Stroke};
use creabim_geom::sheet::{self, PlacedView, SheetInfo};
use creabim_geom::triangulate::triangulate;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};
use glam::DVec2;
use std::collections::HashMap;

#[derive(Default)]
pub struct SheetState {
    /// Pan and zoom per sheet: centre in mm, pixels per mm.
    cameras: HashMap<ElementId, (DVec2, f32)>,
    /// The viewport being worked through, if any.
    pub active: Option<ElementId>,
    /// Viewport being moved: its centre and the grab point when the drag began.
    drag: Option<(ElementId, DVec2, DVec2)>,
    /// Crop edge being dragged: viewport, edge (left, right, bottom, top)
    /// and the crop when the drag began.
    crop_drag: Option<(ElementId, usize, [DVec2; 2])>,
    /// A paper mark being moved: it, where it was, and the grab point.
    mark_drag: Option<(ElementId, DVec2, DVec2)>,
    /// A revision cloud's grip being dragged (MU25): the cloud as it was
    /// and which grip.
    cloud_grip: Option<(creabim_core::Element, CloudGrip)>,
    /// A selection box being dragged on empty paper: where it began.
    band: Option<Pos2>,
    /// A view's title being dragged (MU27): the view, its offset then,
    /// and the grab point.
    title_drag: Option<(ElementId, DVec2, DVec2)>,
    /// Several things on the sheet dragged together (MU27): them as they
    /// were, and the grab point.
    group: Option<(Vec<creabim_core::Element>, DVec2)>,
    /// A guide being dragged (MU26): its index (None: a new one off a
    /// ruler) and whether it runs up and down.
    guide_drag: Option<(Option<usize>, bool)>,
    /// Move or G on the sheet's own notes and marks (MU23): them as they
    /// were, and the base point once picked.
    moving: Option<(Vec<creabim_core::Element>, Option<DVec2>)>,
}

/// The ruler strips a guide is dragged out of, along the sheet pane's top
/// and left edges.
const RULER: f32 = 12.0;

/// `p` pulled onto the sheet's guides within `tol` mm, testing it at each
/// of `offsets` (a box's edges round it, say): up-and-down guides take x,
/// across guides y.
fn snap_guides(p: DVec2, offsets: &[DVec2], guides: &[(bool, f64)], tol: f64) -> DVec2 {
    let mut out = p;
    let mut best = [tol, tol];
    for (vertical, at) in guides {
        let a = usize::from(!*vertical);
        for o in offsets {
            let v = if *vertical { p.x + o.x } else { p.y + o.y };
            let d = (v - at).abs();
            if d < best[a] {
                best[a] = d;
                if *vertical {
                    out.x = at - o.x;
                } else {
                    out.y = at - o.y;
                }
            }
        }
    }
    out
}

/// A revision cloud's grips (MU25).
#[derive(Clone, Copy, Debug, PartialEq)]
enum CloudGrip {
    /// A box's corner or edge: which sides it moves (-1, 0 or 1 in x, y).
    Side(i8, i8),
    /// A freeform outline's point.
    Node(usize),
    /// The middle of a freeform edge: dragged, a new point there.
    Insert(usize),
}

/// The grips of a selected cloud, on paper.
fn cloud_grips(m: &creabim_core::PaperMark) -> Vec<(CloudGrip, DVec2)> {
    use creabim_core::PaperMarkKind as K;
    // a picture sizes by the same box grips (MU40)
    let none: Vec<DVec2> = vec![];
    let (w, h, points) = match &m.kind {
        K::Cloud { w, h, points, .. } => (w, h, points),
        K::Picture { w, h, .. } | K::Vector { w, h, .. } => (w, h, &none),
        _ => return vec![],
    };
    if points.len() >= 3 {
        let pts: Vec<DVec2> = points.iter().map(|p| m.at + *p).collect();
        let n = pts.len();
        let mut out: Vec<(CloudGrip, DVec2)> = pts
            .iter()
            .enumerate()
            .map(|(i, p)| (CloudGrip::Node(i), *p))
            .collect();
        out.extend((0..n).map(|i| (CloudGrip::Insert(i), (pts[i] + pts[(i + 1) % n]) / 2.0)));
        out
    } else {
        let mut out = vec![];
        for sx in [-1i8, 0, 1] {
            for sy in [-1i8, 0, 1] {
                if sx != 0 || sy != 0 {
                    let p = m.at + DVec2::new(*w / 2.0 * sx as f64, *h / 2.0 * sy as f64);
                    out.push((CloudGrip::Side(sx, sy), p));
                }
            }
        }
        out
    }
}

/// `was` with grip `g` dragged to `to` (paper mm).
fn drag_cloud(was: &creabim_core::PaperMark, g: CloudGrip, to: DVec2) -> creabim_core::PaperMark {
    let mut m = was.clone();
    let at = m.at;
    // a picture: its corners keep its proportions, its edges stretch it
    let picture = match &mut m.kind {
        creabim_core::PaperMarkKind::Picture { w, h, .. }
        | creabim_core::PaperMarkKind::Vector { w, h, .. } => Some((w, h)),
        _ => None,
    };
    if let (Some((w, h)), CloudGrip::Side(sx, sy)) = (picture, g) {
        let (lo, hi) = (at - DVec2::new(*w, *h) / 2.0, at + DVec2::new(*w, *h) / 2.0);
        let fixed = DVec2::new(
            if sx < 0 { hi.x } else { lo.x },
            if sy < 0 { hi.y } else { lo.y },
        );
        let (mut nw, mut nh) = (*w, *h);
        if sx != 0 {
            nw = ((to.x - fixed.x) * sx as f64).max(2.0);
        }
        if sy != 0 {
            nh = ((to.y - fixed.y) * sy as f64).max(2.0);
        }
        if sx != 0 && sy != 0 {
            let k = (nw / *w).max(nh / *h);
            nw = *w * k;
            nh = *h * k;
        }
        // the far side stays put; an axis not dragged stays centred
        let lo2 = DVec2::new(
            match sx {
                0 => at.x - nw / 2.0,
                s if s < 0 => hi.x - nw,
                _ => lo.x,
            },
            match sy {
                0 => at.y - nh / 2.0,
                s if s < 0 => hi.y - nh,
                _ => lo.y,
            },
        );
        *w = nw;
        *h = nh;
        m.at = lo2 + DVec2::new(nw, nh) / 2.0;
        return m;
    }
    if let creabim_core::PaperMarkKind::Cloud { w, h, points, .. } = &mut m.kind {
        match g {
            CloudGrip::Side(sx, sy) => {
                let (mut lo, mut hi) =
                    (at - DVec2::new(*w, *h) / 2.0, at + DVec2::new(*w, *h) / 2.0);
                if sx < 0 {
                    lo.x = to.x.min(hi.x - 5.0);
                }
                if sx > 0 {
                    hi.x = to.x.max(lo.x + 5.0);
                }
                if sy < 0 {
                    lo.y = to.y.min(hi.y - 5.0);
                }
                if sy > 0 {
                    hi.y = to.y.max(lo.y + 5.0);
                }
                *w = hi.x - lo.x;
                *h = hi.y - lo.y;
                m.at = (lo + hi) / 2.0;
            }
            CloudGrip::Node(i) => {
                if let Some(p) = points.get_mut(i) {
                    *p = to - at;
                }
            }
            CloudGrip::Insert(i) => points.insert(i + 1, to - at),
        }
    }
    m
}

/// What `sheet` itself holds of the selection: its notes and paper marks.
fn on_sheet(doc: &Document, sheet: ElementId, selection: &Selection) -> Vec<creabim_core::Element> {
    selection
        .all()
        .iter()
        .filter_map(|id| doc.model().get(*id))
        .filter(|e| match &e.kind {
            ElementKind::Note(n) => n.view == sheet,
            ElementKind::PaperMark(m) => m.sheet == sheet,
            ElementKind::Detail(d) => d.view == sheet,
            _ => false,
        })
        .cloned()
        .collect()
}

/// Move what is written on the sheet: Move (toolbar, M) picks a base point
/// then where it goes; G follows the pointer at once. A click places it;
/// Esc or a right-click puts it back. `Some` when it took the frame (with
/// an error, if any).
#[allow(clippy::too_many_arguments)]
/// Points on a sheet the Move tool snaps to (MU29): the paper's corners,
/// marks' and notes' anchors and corners, views' corners and centres, but
/// none of what is being moved.
fn sheet_snaps(
    doc: &Document,
    sheet: ElementId,
    views: &[PlacedView],
    skip: &[ElementId],
) -> Vec<DVec2> {
    let model = doc.model();
    let mut out = vec![];
    let corners = |out: &mut Vec<DVec2>, lo: DVec2, hi: DVec2| {
        out.extend([
            lo,
            hi,
            DVec2::new(lo.x, hi.y),
            DVec2::new(hi.x, lo.y),
            (lo + hi) / 2.0,
        ]);
    };
    if let Some(s) = sheet::sheet_of(model, sheet) {
        corners(&mut out, DVec2::ZERO, DVec2::new(s.width, s.height));
    }
    for e in model.iter().filter(|e| !skip.contains(&e.id)) {
        match &e.kind {
            ElementKind::PaperMark(m) if m.sheet == sheet => {
                out.push(m.at);
                let (lo, hi) = m.bounds();
                corners(&mut out, lo, hi);
            }
            ElementKind::Note(n) if n.view == sheet => out.push(n.at),
            _ => {}
        }
    }
    for v in views.iter().filter(|v| !skip.contains(&v.viewport)) {
        corners(&mut out, v.bounds.0, v.bounds.1);
    }
    out
}

/// `p` pulled onto the nearest of `points` within `tol`, else onto the
/// guides; and the point it took, to mark.
fn snap_on_sheet(p: DVec2, points: &[DVec2], guides: &[(bool, f64)], tol: f64) -> (DVec2, bool) {
    if let Some(q) = points
        .iter()
        .filter(|q| q.distance(p) < tol)
        .min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)))
    {
        return (*q, true);
    }
    let g = snap_guides(p, &[DVec2::ZERO], guides, tol);
    (g, g != p)
}

#[allow(clippy::too_many_arguments)]
fn sheet_move(
    ui: &egui::Ui,
    doc: &mut Document,
    sheet: ElementId,
    response: &egui::Response,
    xf: &PaperXf,
    state: &mut SheetState,
    plan: &mut PlanState,
    selection: &Selection,
    painter: &egui::Painter,
    views: &[PlacedView],
    guides: &[(bool, f64)],
) -> Option<Option<String>> {
    use crate::modal::Mode;
    // armed, a right-click or Esc lets it go, as everywhere (MU30)
    if state.moving.is_none()
        && plan.armed.is_some()
        && response.hovered()
        && (response.secondary_clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)))
    {
        plan.armed = None;
        plan.status = Some("Move cancelled.".into());
        return Some(None);
    }
    // a move another view took while still waiting for its base point
    // (the pointer crossed it on the way here) comes back to the sheet
    let base_waiting = plan.modal.as_ref().is_some_and(|m| {
        m.flow == crate::modal::Flow::Tool
            && m.stage() == crate::modal::Stage::Base
            && m.mode == Mode::Move
    });
    if base_waiting && response.hovered() && !on_sheet(doc, sheet, selection).is_empty() {
        if let Some(mut old) = plan.modal.take() {
            old.discard(doc);
            plan.armed = Some((Mode::Move, false));
        }
    }
    // snapped to what is on the sheet and to its guides (MU29)
    let skip: Vec<ElementId> = state
        .moving
        .as_ref()
        .map(|(items, _)| items.iter().map(|e| e.id).collect())
        .unwrap_or_else(|| selection.all());
    let points = sheet_snaps(doc, sheet, views, &skip);
    let tol = 8.0 / xf.zoom as f64;
    let here = response.hover_pos().map(|p| {
        let (q, took) = snap_on_sheet(xf.to_paper(p), &points, guides, tol);
        if took {
            let at = xf.to_screen(q);
            painter.rect_stroke(
                Rect::from_center_size(at, Vec2::splat(9.0)),
                1.0,
                egui::Stroke::new(1.5, interaction_ink()),
                egui::StrokeKind::Middle,
            );
        }
        q
    });
    if state.moving.is_none() {
        let armed = matches!(plan.armed, Some((Mode::Move, false)));
        let g = response.hovered()
            && matches!(planview::start_keys(ui.ctx()), Some((Mode::Move, false)));
        // the toolbar's Move starts with its base click, in the area
        // clicked (MU29: crossing another sheet's area took it there)
        let base_click = armed && response.clicked();
        if armed && response.hovered() {
            if let Some(h) = here {
                painter.circle_stroke(
                    xf.to_screen(h),
                    5.0,
                    egui::Stroke::new(1.5, interaction_ink()),
                );
            }
        }
        if !(base_click || g) {
            return None;
        }
        let items = on_sheet(doc, sheet, selection);
        if items.is_empty() {
            return None;
        }
        plan.armed = None;
        let base = if g || base_click { here } else { None };
        plan.status = Some(if base.is_some() {
            "Move: move the pointer and click where it goes. Esc or right-click puts it back."
                .into()
        } else {
            "Move: click the base point, then where it goes.".into()
        });
        state.moving = Some((items, base));
        // the click that set the base point is not also the target (MU30:
        // it finished every move at once, 0 mm)
        if base_click {
            ui.ctx().request_repaint();
            return Some(None);
        }
    }
    let (items, base) = state.moving.clone()?;
    ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    let ids: Vec<ElementId> = items.iter().map(|e| e.id).collect();
    outline_items(painter, xf, doc.model(), sheet, &ids);
    let cancel = response.secondary_clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape));
    let mut error = None;
    let put = |doc: &mut Document, by: DVec2| -> Option<String> {
        for e in &items {
            let id = e.id;
            let orig = e.kind.clone();
            let r = doc.edit_merged(id, "Move on the sheet", |el| {
                el.kind = orig.clone();
                match &mut el.kind {
                    ElementKind::Note(n) => {
                        n.at += by;
                        if let Some(el) = &mut n.elbow {
                            *el += by;
                        }
                    }
                    ElementKind::PaperMark(m) => m.at += by,
                    ElementKind::Detail(d) => d.points.iter_mut().for_each(|p| *p += by),
                    _ => {}
                }
            });
            if let Err(e) = r {
                return Some(e.to_string());
            }
        }
        None
    };
    if cancel {
        error = put(doc, DVec2::ZERO);
        state.moving = None;
        plan.status = Some("Move cancelled.".into());
        return Some(error);
    }
    match (base, here) {
        (None, Some(p)) => {
            painter.circle_stroke(
                xf.to_screen(p),
                5.0,
                egui::Stroke::new(1.5, interaction_ink()),
            );
            if response.clicked() {
                state.moving = Some((items, Some(p)));
                plan.status =
                    Some("Move: click where it goes. Esc or right-click puts it back.".into());
            }
        }
        (Some(b), Some(p)) => {
            let by = p - b;
            painter.line_segment(
                [xf.to_screen(b), xf.to_screen(b + by)],
                egui::Stroke::new(1.0, interaction_ink()),
            );
            error = put(doc, by);
            if response.clicked() {
                state.moving = None;
                plan.status = Some(format!("Moved {:.0}, {:.0} mm on the sheet.", by.x, by.y));
            }
        }
        _ => {}
    }
    ui.ctx().request_repaint();
    Some(error)
}

/// Put `items` back as they were, moved by `by` paper mm: one undo step
/// while dragging.
fn move_items(
    doc: &mut Document,
    items: &[creabim_core::Element],
    by: DVec2,
    label: &str,
) -> Option<String> {
    for e in items {
        let orig = e.kind.clone();
        let r = doc.edit_merged(e.id, label, |el| {
            el.kind = orig.clone();
            match &mut el.kind {
                ElementKind::Note(n) => {
                    n.at += by;
                    if let Some(el) = &mut n.elbow {
                        *el += by;
                    }
                }
                ElementKind::PaperMark(m) => m.at += by,
                ElementKind::Detail(d) => d.points.iter_mut().for_each(|p| *p += by),
                _ => {}
            }
        });
        if let Err(e) = r {
            return Some(e.to_string());
        }
    }
    None
}

/// Outline what is being moved, so it reads as the selection (MU27).
fn outline_items(
    painter: &egui::Painter,
    xf: &PaperXf,
    model: &creabim_core::Model,
    sheet: ElementId,
    ids: &[ElementId],
) {
    let stroke = egui::Stroke::new(1.5, interaction_ink());
    for e in model.iter().filter(|e| ids.contains(&e.id)) {
        let b = match &e.kind {
            ElementKind::PaperMark(m) if m.sheet == sheet => Some(m.bounds()),
            ElementKind::Detail(d) if d.view == sheet => Some(crate::detailtool::bounds(model, d)),
            ElementKind::Note(n) if n.view == sheet => {
                let items = creabim_geom::annotate::sheet_notes(model, sheet);
                items.iter().find_map(|it| match it {
                    creabim_geom::PlanItem::Annotation { id, item } if *id == e.id => {
                        let pts = item.extent();
                        let lo = pts.iter().copied().reduce(DVec2::min)?;
                        let hi = pts.iter().copied().reduce(DVec2::max)?;
                        Some((lo, hi))
                    }
                    _ => None,
                })
            }
            _ => None,
        };
        if let Some((lo, hi)) = b {
            painter.rect_stroke(
                xf.rect_of(lo, hi).expand(3.0),
                2.0,
                stroke,
                egui::StrokeKind::Outside,
            );
        }
    }
}

/// Sheet millimetres (y up) to screen points.
#[derive(Clone, Copy)]
struct PaperXf {
    rect: Rect,
    center: DVec2,
    zoom: f32,
}

impl PaperXf {
    fn to_screen(self, p: DVec2) -> Pos2 {
        let d = (p - self.center).as_vec2() * self.zoom;
        self.rect.center() + Vec2::new(d.x, -d.y)
    }

    fn to_paper(self, p: Pos2) -> DVec2 {
        let d = (p - self.rect.center()) / self.zoom;
        self.center + DVec2::new(d.x as f64, -d.y as f64)
    }

    fn rect_of(self, lo: DVec2, hi: DVec2) -> Rect {
        Rect::from_two_pos(self.to_screen(lo), self.to_screen(hi))
    }
}

/// A [`Canvas`] that paints into egui, so the sheet looks as it prints.
struct EguiCanvas<'a> {
    painter: &'a egui::Painter,
    xf: PaperXf,
}

fn gray(g: f64) -> Color32 {
    Color32::from_gray((g.clamp(0.0, 1.0) * 255.0) as u8)
}

impl EguiCanvas<'_> {
    fn line_in(&mut self, pts: &[DVec2], s: Stroke, color: Color32) {
        let pts: Vec<Pos2> = pts.iter().map(|p| self.xf.to_screen(*p)).collect();
        let stroke = egui::Stroke::new((s.width as f32 * self.xf.zoom).max(0.5), color);
        match s.dash {
            Some([a, b]) => {
                let z = self.xf.zoom;
                self.painter.extend(egui::Shape::dashed_line(
                    &pts,
                    stroke,
                    (a as f32 * z).max(2.0),
                    (b as f32 * z).max(1.0),
                ));
            }
            None => {
                self.painter.add(egui::Shape::line(pts, stroke));
            }
        }
    }
}

impl Canvas for EguiCanvas<'_> {
    fn line(&mut self, pts: &[DVec2], s: Stroke) {
        self.line_in(pts, s, gray(s.gray));
    }

    fn line_rgb(&mut self, pts: &[DVec2], s: Stroke, rgb: [f32; 3]) {
        self.line_in(pts, s, crate::annotate::color32(rgb));
    }

    fn fill_rgb(&mut self, pts: &[DVec2], rgb: [f32; 3]) {
        let pts: Vec<Pos2> = pts.iter().map(|p| self.xf.to_screen(*p)).collect();
        let c = crate::annotate::color32(rgb);
        self.painter
            .add(egui::Shape::convex_polygon(pts, c, egui::Stroke::NONE));
    }

    fn styled_text(
        &mut self,
        at: DVec2,
        text: &str,
        anchor: Anchor,
        angle: f64,
        look: &creabim_geom::annotation::TextLook,
    ) {
        let px = look.size as f32 * self.xf.zoom;
        if px < 3.0 {
            return;
        }
        let color = look.rgb.map_or(Color32::BLACK, crate::annotate::color32);
        crate::annotate::text(
            self.painter,
            self.xf.to_screen(at),
            text,
            px,
            color,
            angle,
            anchor,
            look,
        );
    }

    fn fill(&mut self, pts: &[DVec2], g: f64) {
        let mut mesh = egui::Mesh::default();
        for p in pts {
            mesh.colored_vertex(self.xf.to_screen(*p), gray(g));
        }
        for [a, b, c] in triangulate(pts) {
            mesh.add_triangle(a as u32, b as u32, c as u32);
        }
        self.painter.add(egui::Shape::mesh(mesh));
    }

    fn text_angle(&mut self, at: DVec2, size: f64, text: &str, anchor: Anchor, angle: f64) {
        let px = size as f32 * self.xf.zoom;
        if px < 3.0 {
            return;
        }
        let galley = self.painter.layout_no_wrap(
            text.to_string(),
            egui::FontId::proportional(px),
            Color32::BLACK,
        );
        let w = galley.size().x;
        // centre of the text, along its baseline direction from the anchor
        let shift = match anchor {
            Anchor::Start => w / 2.0,
            Anchor::Middle => 0.0,
            Anchor::End => -w / 2.0,
        };
        let rot = egui::emath::Rot2::from_angle(-angle as f32);
        let centre = self.xf.to_screen(at) + rot * Vec2::new(shift, 0.0);
        let pos = centre - rot * (galley.size() / 2.0);
        self.painter.add(
            egui::epaint::TextShape::new(pos, galley, Color32::BLACK).with_angle(-angle as f32),
        );
    }

    fn image(
        &mut self,
        lo: DVec2,
        hi: DVec2,
        clip: Option<(DVec2, DVec2)>,
        raster: &std::sync::Arc<creabim_geom::raster::Raster>,
    ) {
        let rect = self.xf.rect_of(lo, hi);
        let clip = clip.map(|(a, b)| self.xf.rect_of(a, b));
        crate::textures::paint(self.painter, rect, clip, raster);
    }
}

fn interaction_ink() -> Color32 {
    crate::theme::interaction()
}

/// Draw `d` as it prints, as large as fits in `rect`: a type's preview.
/// Lines keep their printed weights at three pixels a millimetre.
pub fn draw_fit(painter: &egui::Painter, rect: Rect, d: &creabim_geom::PlanView) {
    let Some((lo, hi)) = d.bounds() else { return };
    let size = (hi - lo).max(DVec2::splat(0.1));
    let zoom = 3.0f32;
    // paper millimetres a model metre, for the drawing to fill the rect
    let k = (rect.width() as f64 / zoom as f64 / size.x)
        .min(rect.height() as f64 / zoom as f64 / size.y)
        * 0.86;
    let map = paper::PaperMap {
        model: (lo + hi) / 2.0,
        paper: DVec2::ZERO,
        scale: 1000.0 / k.max(1e-6),
    };
    let mut canvas = EguiCanvas {
        painter,
        xf: PaperXf {
            rect,
            center: DVec2::ZERO,
            zoom,
        },
    };
    paper::draw(&mut canvas, d, &map);
}

/// The scales a view is usually drawn at on a sheet, as denominators.
pub const SCALES: [f64; 10] = [1.0, 2.0, 5.0, 10.0, 20.0, 25.0, 50.0, 100.0, 200.0, 500.0];

/// Where a viewport's centre goes when its view's scale changes from
/// `old` to `new` (denominators), so that the drawing grows or shrinks
/// about `mid`, the middle of what it shows (drawing coordinates), and not
/// about the point it is pinned by.
pub fn rescaled_center(vp: &creabim_core::Viewport, mid: DVec2, old: f64, new: f64) -> DVec2 {
    let at = |scale: f64| (mid - vp.target) * (1000.0 / scale);
    vp.center + at(old) - at(new)
}

/// A dropdown of the usual scales and a box for any other: the new
/// denominator when one is picked or typed.
pub fn scale_picker(ui: &mut egui::Ui, salt: ElementId, scale: f64) -> Option<f64> {
    let mut out = None;
    // a Properties row like the rest, sized to the panel: a fixed-width
    // pair here stretched the whole column past a narrow panel's edge
    crate::props::row(ui, "Scale", |ui| {
        let standard = SCALES.iter().any(|s| (s - scale).abs() < 1e-6);
        let h = ui.spacing().interact_size.y;
        let typed = 56.0_f32.min(ui.available_width() * 0.4);
        let gap = ui.spacing().item_spacing.x;
        egui::ComboBox::from_id_salt(("scale", salt))
            .width((ui.available_width() - typed - gap).max(30.0))
            .icon(crate::props::combo_icon)
            .selected_text(if standard {
                format!("1:{scale:.0}")
            } else {
                format!("1:{scale:.0} (custom)")
            })
            .show_ui(ui, |ui| {
                for s in SCALES {
                    if ui
                        .selectable_label((s - scale).abs() < 1e-6, format!("1:{s:.0}"))
                        .clicked()
                    {
                        out = Some(s);
                    }
                }
            });
        let mut v = scale;
        if ui
            .add_sized(
                [typed, h],
                egui::DragValue::new(&mut v)
                    .speed(1.0)
                    .range(1.0..=5000.0)
                    .prefix("1:"),
            )
            .on_hover_text("Any other scale: type its denominator")
            .changed()
        {
            out = Some(v.round().max(1.0));
        }
    });
    out
}

fn contains(v: &PlacedView, p: DVec2) -> bool {
    let (lo, hi) = v.bounds;
    let pad = DVec2::splat(2.0);
    let title = v.title_at() - DVec2::new(0.0, 5.0);
    p.cmpge(lo.min(title) - pad).all() && p.cmple(hi + pad).all()
}

/// Show sheet `sheet` with its `views` in `rect`. Returns a status message when an edit fails.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    rect: Rect,
    doc: &mut Document,
    sheet_id: ElementId,
    views: &[PlacedView],
    info: &SheetInfo,
    state: &mut SheetState,
    plan: &mut PlanState,
    selection: &mut Selection,
) -> Option<String> {
    let s = sheet::sheet_of(doc.model(), sheet_id)?.clone();
    let title = doc
        .model()
        .get(sheet_id)
        .map(|e| e.name.clone())
        .unwrap_or_default();
    let size = DVec2::new(s.width, s.height);
    if state
        .active
        .is_some_and(|a| !views.iter().any(|v| v.viewport == a))
    {
        state.active = None;
    }

    let response = ui.interact(
        rect,
        ui.id().with(("sheet", sheet_id)),
        Sense::click_and_drag(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_gray(205));

    let (mut center, mut zoom) = *state.cameras.entry(sheet_id).or_insert_with(|| {
        let z = (rect.width() / s.width as f32).min(rect.height() / s.height as f32) * 0.92;
        (size / 2.0, z)
    });
    if response.dragged_by(egui::PointerButton::Secondary)
        || response.dragged_by(egui::PointerButton::Middle)
    {
        let d = response.drag_delta() / zoom;
        center -= DVec2::new(d.x as f64, -d.y as f64);
    }
    if response.hovered() || state.active.is_some() && ui.rect_contains_pointer(rect) {
        let (pan, scroll) = crate::planview::wheel(ui);
        center -= DVec2::new(pan.x as f64, -pan.y as f64) / zoom as f64;
        if let (true, Some(hover)) = (scroll != 0.0, ui.input(|i| i.pointer.hover_pos())) {
            let before = PaperXf { rect, center, zoom }.to_paper(hover);
            zoom = (zoom * (1.0 + scroll * 0.002)).clamp(0.2, 60.0);
            center += before - PaperXf { rect, center, zoom }.to_paper(hover);
        }
    }
    // the drawing areas' zoom and pan column (MU21)
    let fit = || {
        let z = (rect.width() / s.width as f32).min(rect.height() / s.height as f32) * 0.92;
        (size / 2.0, z)
    };
    // the selection on the sheet: views, paper marks, notes (MU25)
    let picked = {
        let model = doc.model();
        let mut lo = DVec2::splat(f64::MAX);
        let mut hi = DVec2::splat(f64::MIN);
        for id in selection.all() {
            let b = if let Some(v) = views.iter().find(|v| v.viewport == id || v.view == id) {
                Some(v.bounds)
            } else {
                match model.get(id).map(|e| &e.kind) {
                    Some(ElementKind::PaperMark(m)) if m.sheet == sheet_id => Some(m.bounds()),
                    Some(ElementKind::Detail(d)) if d.view == sheet_id => {
                        Some(crate::detailtool::bounds(model, d))
                    }
                    Some(ElementKind::Note(n)) if n.view == sheet_id => {
                        Some((n.at - DVec2::new(5.0, 5.0), n.at + DVec2::new(60.0, 10.0)))
                    }
                    _ => None,
                }
            };
            if let Some((a, b)) = b {
                lo = lo.min(a);
                hi = hi.max(b);
            }
        }
        (lo.x <= hi.x).then(|| {
            let span = (hi - lo).max(DVec2::splat(10.0));
            let z = (rect.width() as f64 / span.x).min(rect.height() as f64 / span.y) * 0.7;
            ((lo + hi) / 2.0, (z as f32).clamp(0.2, 60.0))
        })
    };
    // read now, drawn once the paper is down so it never hides under it
    // (MU25)
    let (asked, nav_look) =
        crate::planview::nav_input(ui, rect, sheet_id, fit, picked, center, zoom);
    if let Some((c, z)) = asked {
        center = c;
        zoom = z;
    }
    state.cameras.insert(sheet_id, (center, zoom));
    let xf = PaperXf { rect, center, zoom };

    // the paper, its frame and every view not being worked on
    let paper_rect = xf.rect_of(DVec2::ZERO, size);
    painter.rect_filled(
        paper_rect.translate(Vec2::splat(4.0)),
        0.0,
        Color32::from_gray(150),
    );
    painter.rect_filled(paper_rect, 0.0, Color32::WHITE);
    let mut canvas = EguiCanvas {
        painter: &painter,
        xf,
    };
    sheet::draw_frame(
        &mut canvas,
        doc.model(),
        &s,
        &title,
        &sheet::scale_text(doc.model(), views),
        info,
    );
    for v in views {
        if state.active != Some(v.viewport) {
            paper::draw_looks(&mut canvas, &v.drawing, &v.map, &v.looks);
        }
        sheet::draw_view_title(&mut canvas, v);
        if selection.contains(v.viewport) && state.active.is_none() {
            let (lo, hi) = v.bounds;
            painter.rect_stroke(
                xf.rect_of(lo, hi).expand(4.0),
                2.0,
                egui::Stroke::new(1.5, interaction_ink()),
                egui::StrokeKind::Outside,
            );
        }
    }
    sheet::draw_sheet_notes(&mut canvas, doc.model(), sheet_id);
    // working inside a view (double-click): the rest of the sheet greys
    // out, so it is clear where clicks go (MU24, Annotate board step 2)
    if let Some(v) = state
        .active
        .and_then(|a| views.iter().find(|v| v.viewport == a))
    {
        let (lo, hi) = v.bounds;
        let inner = xf.rect_of(lo, hi).expand(6.0);
        let veil = Color32::from_rgba_unmultiplied(120, 124, 128, 90);
        let outer = paper_rect.intersect(rect);
        for r in [
            Rect::from_min_max(outer.min, egui::pos2(outer.max.x, inner.min.y)),
            Rect::from_min_max(egui::pos2(outer.min.x, inner.max.y), outer.max),
            Rect::from_min_max(
                egui::pos2(outer.min.x, inner.min.y),
                egui::pos2(inner.min.x, inner.max.y),
            ),
            Rect::from_min_max(
                egui::pos2(inner.max.x, inner.min.y),
                egui::pos2(outer.max.x, inner.max.y),
            ),
        ] {
            if r.is_positive() {
                painter.rect_filled(r, 0.0, veil);
            }
        }
        painter.rect_stroke(
            inner,
            3.0,
            egui::Stroke::new(1.5, interaction_ink()),
            egui::StrokeKind::Outside,
        );
    }

    // guides (MU26): ruler strips along the top and left edges to drag
    // them out of; dragged back onto a ruler, a guide goes
    // only while the header's Guides is on (MU27): the sheet stays clean
    let on = plan.sheet_guides;
    let guides = if on { s.guides.clone() } else { vec![] };
    let top = Rect::from_min_max(
        rect.left_top() + Vec2::new(RULER, 0.0),
        egui::pos2(rect.right(), rect.top() + RULER),
    );
    let left = Rect::from_min_max(
        rect.left_top() + Vec2::new(0.0, RULER),
        egui::pos2(rect.left() + RULER, rect.bottom()),
    );
    let (top, left) = if on {
        (top, left)
    } else {
        (Rect::NOTHING, Rect::NOTHING)
    };
    let guide_ink = Color32::from_rgb(40, 170, 220);
    if on {
        let p = crate::theme::Palette::of(ui.ctx());
        // the theme's own colours: dark paper leaves the rulers alone (MU30)
        let first = painter.add(egui::Shape::Noop);
        for r in [top, left] {
            painter.rect_filled(r, 0.0, p.panel.gamma_multiply(0.85));
        }
        // a tick every 10 mm of paper, longer each 50
        let step = 10.0;
        let (lo, hi) = (
            xf.to_paper(rect.left_top()),
            xf.to_paper(rect.right_bottom()),
        );
        if zoom * step as f32 > 4.0 {
            let mut x = (lo.x / step).floor() * step;
            while x < hi.x {
                let sx = xf.to_screen(DVec2::new(x, 0.0)).x;
                let len = if (x / 50.0).round() * 50.0 == x {
                    RULER * 0.7
                } else {
                    RULER * 0.35
                };
                painter.line_segment(
                    [
                        egui::pos2(sx, top.bottom()),
                        egui::pos2(sx, top.bottom() - len),
                    ],
                    egui::Stroke::new(1.0, p.muted),
                );
                x += step;
            }
            let mut y = (hi.y / step).floor() * step;
            while y < lo.y {
                let sy = xf.to_screen(DVec2::new(0.0, y)).y;
                let len = if (y / 50.0).round() * 50.0 == y {
                    RULER * 0.7
                } else {
                    RULER * 0.35
                };
                painter.line_segment(
                    [
                        egui::pos2(left.right(), sy),
                        egui::pos2(left.right() - len, sy),
                    ],
                    egui::Stroke::new(1.0, p.muted),
                );
                y += step;
            }
        }
        let last = painter.add(egui::Shape::Noop);
        planview::keep_colours(ui.ctx(), painter.layer_id(), first, last);
    }
    let line_of = |vertical: bool, at: f64| -> [Pos2; 2] {
        if vertical {
            let x = xf.to_screen(DVec2::new(at, 0.0)).x;
            [
                egui::pos2(x, rect.top() + RULER),
                egui::pos2(x, rect.bottom()),
            ]
        } else {
            let y = xf.to_screen(DVec2::new(0.0, at)).y;
            [
                egui::pos2(rect.left() + RULER, y),
                egui::pos2(rect.right(), y),
            ]
        }
    };
    let dragging_guide = state.guide_drag.map(|g| g.0);
    for (i, (vertical, at)) in guides.iter().enumerate() {
        if dragging_guide == Some(Some(i)) {
            continue;
        }
        // dashed, so they read as guides, never as drawing (MU29)
        painter.extend(egui::Shape::dashed_line(
            &line_of(*vertical, *at),
            egui::Stroke::new(1.0, guide_ink.gamma_multiply(0.85)),
            6.0,
            4.0,
        ));
    }
    if on && state.active.is_none() && plan.tool == planview::Tool::Select {
        let near_guide = |q: Pos2| {
            guides.iter().position(|(vertical, at)| {
                let [a, _] = line_of(*vertical, *at);
                if *vertical {
                    (q.x - a.x).abs() < 4.0
                } else {
                    (q.y - a.y).abs() < 4.0
                }
            })
        };
        let hover = response.hover_pos();
        if let Some(h) = hover {
            if top.contains(h) || left.contains(h) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            } else if let Some(i) = near_guide(h) {
                ui.ctx().set_cursor_icon(if guides[i].0 {
                    egui::CursorIcon::ResizeHorizontal
                } else {
                    egui::CursorIcon::ResizeVertical
                });
            }
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(q) = ui.input(|i| i.pointer.press_origin()) {
                if top.contains(q) {
                    state.guide_drag = Some((None, false));
                } else if left.contains(q) {
                    state.guide_drag = Some((None, true));
                } else if let Some(i) = near_guide(q) {
                    state.guide_drag = Some((Some(i), guides[i].0));
                }
            }
        }
        if let Some((which, vertical)) = state.guide_drag {
            if let Some(q) = response.interact_pointer_pos().or(hover) {
                let at = xf.to_paper(q);
                let at = if vertical { at.x } else { at.y };
                let at = (at * 2.0).round() / 2.0;
                painter.line_segment(line_of(vertical, at), egui::Stroke::new(1.5, guide_ink));
                painter.text(
                    q + Vec2::new(10.0, -10.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{at:.1} mm"),
                    egui::FontId::proportional(11.0),
                    guide_ink,
                );
                if response.drag_stopped() {
                    let gone = top.contains(q) || left.contains(q) || !rect.contains(q);
                    let _ = doc.edit(
                        sheet_id,
                        if gone { "Remove guide" } else { "Place guide" },
                        |e| {
                            if let ElementKind::Sheet(sh) = &mut e.kind {
                                match (which, gone) {
                                    (Some(i), true) if i < sh.guides.len() => {
                                        sh.guides.remove(i);
                                    }
                                    (Some(i), false) if i < sh.guides.len() => {
                                        sh.guides[i] = (vertical, at)
                                    }
                                    (None, false) => sh.guides.push((vertical, at)),
                                    _ => {}
                                }
                            }
                        },
                    );
                    plan.status = Some(if gone {
                        "Guide removed.".into()
                    } else {
                        "Guide placed: marks, notes and views snap to it. Drag it back onto a ruler to remove it.".into()
                    });
                    state.guide_drag = None;
                }
            }
            if !response.dragged() && !response.drag_stopped() {
                state.guide_drag = None;
            }
            if let Some(look) = &nav_look {
                crate::planview::nav_draw(ui, look);
            }
            return None;
        }
    }

    if let Some(look) = &nav_look {
        crate::planview::nav_draw(ui, look);
    }

    // pre-selection: what is under the pointer, topmost first, and the
    // others under it where things overlap (Tab cycles them): paper
    // marks, notes, then views (MU25)
    let pane = Pane::Sheet(sheet_id);
    let note_items = creabim_geom::annotate::sheet_notes(doc.model(), sheet_id);
    let mark_boxes: Vec<(ElementId, (DVec2, DVec2))> = doc
        .model()
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::PaperMark(m) if m.sheet == sheet_id => Some((e.id, m.bounds())),
            // linework drawn on the paper is picked by its box, with a
            // little room round a thin line
            ElementKind::Detail(d) if d.view == sheet_id => {
                let (lo, hi) = crate::detailtool::bounds(doc.model(), d);
                Some((e.id, (lo - DVec2::splat(1.0), hi + DVec2::splat(1.0))))
            }
            _ => None,
        })
        .collect();
    if state.active.is_none() && !response.dragged() {
        let pointer = response
            .hover_pos()
            .filter(|_| plan.tool == planview::Tool::Select && plan.modal.is_none());
        let said = hoverpick::track(
            ui.ctx(),
            &mut plan.hover,
            pane,
            pointer,
            doc.model().revision(),
            doc.model(),
            |p| {
                let p = xf.to_paper(p);
                let mut out: Vec<ElementId> = mark_boxes
                    .iter()
                    .rev()
                    .filter(|(_, (lo, hi))| {
                        p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y
                    })
                    .map(|(id, _)| *id)
                    .collect();
                out.extend(note_items.iter().rev().filter_map(|it| match it {
                    creabim_geom::PlanItem::Annotation { id, item }
                        if item.hit(p, 6.0 / zoom as f64, 1.0) =>
                    {
                        Some(*id)
                    }
                    _ => None,
                }));
                out.extend(
                    views
                        .iter()
                        .rev()
                        .filter(|v| contains(v, p))
                        .map(|v| v.viewport),
                );
                out
            },
        );
        if said.is_some() {
            plan.status = said;
        }
    }
    let lit = hoverpick::highlighted(&plan.hover).filter(|id| !selection.contains(*id));
    let hover_stroke = egui::Stroke::new(1.5, hoverpick::HOVER.gamma_multiply(0.7));
    for it in &note_items {
        if let creabim_geom::PlanItem::Annotation { id, item } = it {
            if Some(*id) == lit {
                let pxf = planview::Xf {
                    rect,
                    center,
                    scale: zoom,
                };
                crate::annotate::paint_weighted(
                    &painter,
                    &pxf,
                    item,
                    hover_stroke.color,
                    1.0,
                    planview::TRACE,
                );
            }
        }
    }
    if let Some((_, (lo, hi))) = mark_boxes.iter().find(|(id, _)| Some(*id) == lit) {
        painter.rect_stroke(
            xf.rect_of(*lo, *hi).expand(3.0),
            2.0,
            hover_stroke,
            egui::StrokeKind::Outside,
        );
    }
    if let Some(v) = views.iter().find(|v| Some(v.viewport) == lit) {
        let (lo, hi) = v.bounds;
        painter.rect_stroke(
            xf.rect_of(lo, hi).expand(4.0),
            2.0,
            egui::Stroke::new(1.5, hoverpick::HOVER.gamma_multiply(0.7)),
            egui::StrokeKind::Outside,
        );
    }
    if state.active.is_none() {
        if let Some(id) = hoverpick::badge(ui.ctx(), &mut plan.hover, pane, doc.model()) {
            selection.set(Some(id));
        }
    }

    // crop handles on the selected viewport: drag an edge to crop the view
    let mut error = None;
    let pointer = response.interact_pointer_pos().or(response.hover_pos());
    let selected = views
        .iter()
        .find(|v| selection.contains(v.viewport) && state.active.is_none());
    if let Some(v) = selected {
        let crop = match doc.model().get(v.viewport).map(|e| &e.kind) {
            Some(ElementKind::Viewport(vp)) => vp.crop,
            _ => None,
        };
        let full = crop.or_else(|| v.drawing.bounds().map(|(a, b)| [a, b]));
        if let Some([lo, hi]) = full {
            let (plo, phi) = (v.map.to_paper(lo), v.map.to_paper(hi));
            let r = xf.rect_of(plo, phi);
            painter.extend(egui::Shape::dashed_line(
                &[
                    r.left_top(),
                    r.right_top(),
                    r.right_bottom(),
                    r.left_bottom(),
                    r.left_top(),
                ],
                egui::Stroke::new(1.0, interaction_ink()),
                6.0,
                4.0,
            ));
            let handles = [
                r.left_center(),
                r.right_center(),
                r.center_bottom(),
                r.center_top(),
            ];
            for h in handles {
                painter.rect(
                    Rect::from_center_size(h, Vec2::splat(8.0)),
                    0.0,
                    Color32::WHITE,
                    egui::Stroke::new(1.5, interaction_ink()),
                    egui::StrokeKind::Middle,
                );
            }
            if response.drag_started_by(egui::PointerButton::Primary) {
                let press = ui.input(|i| i.pointer.press_origin()).or(pointer);
                if let Some(p) = press {
                    if let Some(edge) = handles.iter().position(|h| h.distance(p) < 9.0) {
                        state.crop_drag = Some((v.viewport, edge, [lo, hi]));
                    }
                }
            }
            if let (Some((id, edge, [lo0, hi0])), Some(p)) = (state.crop_drag, pointer) {
                if id == v.viewport && response.dragged_by(egui::PointerButton::Primary) {
                    let m = v.map.to_model(xf.to_paper(p));
                    let snap = |x: f64| (x / 0.05).round() * 0.05;
                    let (mut lo, mut hi) = (lo0, hi0);
                    match edge {
                        0 => lo.x = snap(m.x).min(hi.x - 0.2),
                        1 => hi.x = snap(m.x).max(lo.x + 0.2),
                        2 => lo.y = snap(m.y).min(hi.y - 0.2),
                        _ => hi.y = snap(m.y).max(lo.y + 0.2),
                    }
                    let r = doc.edit_merged(id, "Crop view", |e| {
                        if let ElementKind::Viewport(vp) = &mut e.kind {
                            vp.crop = Some([lo, hi]);
                        }
                    });
                    error = r.err().map(|e| e.to_string());
                }
            }
        }
    }
    if response.drag_stopped() {
        state.crop_drag = None;
    }

    // linework drawn on the paper itself with the Detail tool
    if state.active.is_none() && state.moving.is_none() && plan.tool == planview::Tool::Detail {
        let pxf = planview::Xf {
            rect,
            center,
            scale: zoom,
        };
        let mut failed = None;
        if crate::detailtool::sheet_input(
            ui,
            doc,
            sheet_id,
            &painter,
            &response,
            plan,
            selection,
            &pxf,
            &mut failed,
        ) {
            return failed.or(error);
        }
    }

    // as everywhere (MU23): a right-click or Esc cancels what a tool has
    // started, and again leaves the tool for Select
    if state.active.is_none()
        && state.moving.is_none()
        && plan.tool != planview::Tool::Select
        && response.hovered()
        && (response.secondary_clicked()
            || ui.input(|i| i.key_pressed(egui::Key::Escape))
                && !ui.ctx().egui_wants_keyboard_input())
    {
        plan.step_back();
        return error;
    }

    // Move (the toolbar's or M) and G on what is written on the sheet
    // itself: notes and paper marks (MU23)
    if state.active.is_none() {
        if let Some(r) = sheet_move(
            ui, doc, sheet_id, &response, &xf, state, plan, selection, &painter, views, &guides,
        ) {
            return r.or(error);
        }
    }

    // several selected, dragged by one of them: all of them move (MU27)
    if state.active.is_none() && plan.tool == planview::Tool::Select {
        if response.drag_started_by(egui::PointerButton::Primary) && state.group.is_none() {
            let items = on_sheet(doc, sheet_id, selection);
            let press = ui
                .input(|i| i.pointer.press_origin())
                .or(pointer)
                .map(|q| xf.to_paper(q));
            if let (true, Some(p)) = (items.len() > 1, press) {
                let on_one = mark_boxes.iter().any(|(id, (lo, hi))| {
                    selection.contains(*id)
                        && p.x >= lo.x
                        && p.x <= hi.x
                        && p.y >= lo.y
                        && p.y <= hi.y
                }) || note_items.iter().any(|it| {
                    matches!(it, creabim_geom::PlanItem::Annotation { id, item }
                        if selection.contains(*id) && item.hit(p, 6.0 / zoom as f64, 1.0))
                });
                if on_one {
                    state.group = Some((items, p));
                }
            }
        }
        if let Some((items, grab)) = state.group.clone() {
            if let (true, Some(q)) = (response.dragged_by(egui::PointerButton::Primary), pointer) {
                let by = (xf.to_paper(q) - grab).round();
                error = move_items(doc, &items, by, "Move on the sheet");
            }
            let ids: Vec<ElementId> = items.iter().map(|e| e.id).collect();
            outline_items(&painter, &xf, doc.model(), sheet_id, &ids);
            if response.drag_stopped() || !response.dragged() {
                state.group = None;
                plan.status = Some(format!(
                    "Moved {} together. Ctrl+Z puts them back.",
                    ids.len()
                ));
            }
            return error;
        }
    }

    // north arrows and scale bars on the sheet (MU22): a click picks one,
    // a drag moves it, millimetre by millimetre
    if state.active.is_none() && plan.tool == planview::Tool::Select {
        let marks: Vec<(ElementId, DVec2, (DVec2, DVec2))> = doc
            .model()
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::PaperMark(m) if m.sheet == sheet_id => Some((e.id, m.at, m.bounds())),
                _ => None,
            })
            .collect();
        let at_point = |p: DVec2| {
            marks
                .iter()
                .rev()
                .find(|(_, _, (lo, hi))| p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y)
        };
        // a selected revision cloud's grips: size its box, or move, add
        // and (right-click) remove its points (MU25)
        let cloud = selection.active.filter(|_| selection.len() == 1).and_then(|id| {
            doc.model().get(id).filter(|e| {
                matches!(&e.kind, ElementKind::PaperMark(m)
                    if m.sheet == sheet_id && matches!(m.kind, creabim_core::PaperMarkKind::Cloud { .. }
                        | creabim_core::PaperMarkKind::Picture { .. }
                        | creabim_core::PaperMarkKind::Vector { .. }))
            })
        });
        if let Some(e) = cloud.cloned() {
            let ElementKind::PaperMark(m) = &e.kind else {
                unreachable!()
            };
            let grips = cloud_grips(m);
            let near = |p: Pos2| {
                grips
                    .iter()
                    .map(|(g, q)| (*g, xf.to_screen(*q)))
                    .filter(|(_, q)| q.distance(p) < 8.0)
                    .min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p)))
                    .map(|(g, _)| g)
            };
            let over = response.hover_pos().and_then(near);
            for (g, q) in &grips {
                let at = xf.to_screen(*q);
                let lit = over == Some(*g) || state.cloud_grip.as_ref().is_some_and(|c| c.1 == *g);
                match g {
                    CloudGrip::Insert(_) => {
                        painter.circle(
                            at,
                            if lit { 5.0 } else { 3.5 },
                            Color32::WHITE,
                            egui::Stroke::new(1.2, interaction_ink()),
                        );
                        painter.line_segment(
                            [at - Vec2::new(2.0, 0.0), at + Vec2::new(2.0, 0.0)],
                            egui::Stroke::new(1.0, interaction_ink()),
                        );
                        painter.line_segment(
                            [at - Vec2::new(0.0, 2.0), at + Vec2::new(0.0, 2.0)],
                            egui::Stroke::new(1.0, interaction_ink()),
                        );
                    }
                    _ => {
                        let r =
                            Rect::from_center_size(at, Vec2::splat(if lit { 10.0 } else { 8.0 }));
                        painter.rect(
                            r,
                            1.0,
                            if lit {
                                interaction_ink()
                            } else {
                                Color32::WHITE
                            },
                            egui::Stroke::new(1.3, interaction_ink()),
                            egui::StrokeKind::Middle,
                        );
                    }
                }
            }
            if over.is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
            if response.drag_started_by(egui::PointerButton::Primary) {
                let press = ui.input(|i| i.pointer.press_origin()).or(pointer);
                if let Some(g) = press.and_then(near) {
                    state.cloud_grip = Some((e.clone(), g));
                }
            }
            if let Some((was, g)) = state.cloud_grip.clone() {
                if let (true, Some(p)) =
                    (response.dragged_by(egui::PointerButton::Primary), pointer)
                {
                    if let ElementKind::PaperMark(m0) = &was.kind {
                        let to = xf.to_paper(p);
                        let to = DVec2::new((to.x * 2.0).round() / 2.0, (to.y * 2.0).round() / 2.0);
                        let to = snap_guides(to, &[DVec2::ZERO], &guides, 6.0 / zoom as f64);
                        let m1 = drag_cloud(m0, g, to);
                        let r = doc.edit_merged(was.id, "Shape revision cloud", |e| {
                            e.kind = ElementKind::PaperMark(m1.clone())
                        });
                        error = r.err().map(|e| e.to_string());
                    }
                }
                if response.drag_stopped() {
                    // an inserted point keeps being dragged as that point
                    state.cloud_grip = None;
                }
                return error;
            }
            if response.secondary_clicked() {
                if let Some(CloudGrip::Node(i)) = response.hover_pos().and_then(near) {
                    let r = doc.edit(e.id, "Remove a cloud point", |e| {
                        if let ElementKind::PaperMark(m) = &mut e.kind {
                            if let creabim_core::PaperMarkKind::Cloud { points, .. } = &mut m.kind {
                                if points.len() > 3 {
                                    points.remove(i);
                                }
                            }
                        }
                    });
                    error = r.err().map(|e| e.to_string());
                    return error;
                }
            }
        }
        let hovered = response
            .hover_pos()
            .map(|p| xf.to_paper(p))
            .and_then(at_point);
        for (id, _, (lo, hi)) in &marks {
            let lit = selection.contains(*id);
            if lit || hovered.is_some_and(|h| h.0 == *id) {
                painter.rect_stroke(
                    xf.rect_of(*lo, *hi).expand(3.0),
                    2.0,
                    egui::Stroke::new(
                        1.5,
                        if lit {
                            interaction_ink()
                        } else {
                            hoverpick::HOVER.gamma_multiply(0.7)
                        },
                    ),
                    egui::StrokeKind::Outside,
                );
            }
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            let press = ui.input(|i| i.pointer.press_origin()).or(pointer);
            if let Some(p) = press.map(|p| xf.to_paper(p)) {
                if let Some((id, at, _)) = at_point(p) {
                    state.mark_drag = Some((*id, *at, p));
                    selection.only(*id);
                }
            }
        }
        if let Some((id, at0, grab)) = state.mark_drag {
            if let (true, Some(p)) = (response.dragged_by(egui::PointerButton::Primary), pointer) {
                let to = (at0 + xf.to_paper(p) - grab).round();
                let bx = mark_boxes.iter().find(|(m, _)| *m == id).map(|(_, b)| *b);
                let offs: Vec<DVec2> = match bx {
                    Some((lo, hi)) => {
                        let now = doc.model().get(id).and_then(|e| match &e.kind {
                            ElementKind::PaperMark(m) => Some(m.at),
                            _ => None,
                        });
                        let c = now.unwrap_or(at0);
                        vec![DVec2::ZERO, lo - c, hi - c]
                    }
                    None => vec![DVec2::ZERO],
                };
                let to = snap_guides(to, &offs, &guides, 6.0 / zoom as f64);
                let r = doc.edit_merged(id, "Move paper mark", |e| {
                    if let ElementKind::PaperMark(m) = &mut e.kind {
                        m.at = to;
                    }
                });
                error = r.err().map(|e| e.to_string());
            }
            if response.drag_stopped() {
                state.mark_drag = None;
            }
            return error;
        }
        if response.clicked() {
            if let Some((id, _, _)) = pointer.map(|p| xf.to_paper(p)).and_then(at_point) {
                selection.only(*id);
                return error;
            }
        }
    }

    // a note double-clicked: its editor, not the view under it (MU27); checked before the notes take the click (MU29)
    if response.double_clicked() && state.active.is_none() {
        if let Some(p) = pointer.map(|q| xf.to_paper(q)) {
            let note = note_items.iter().rev().find_map(|it| match it {
                creabim_geom::PlanItem::Annotation { id, item }
                    if item.hit(p, 6.0 / zoom as f64, 1.0) =>
                {
                    Some(*id)
                }
                _ => None,
            });
            if let Some(id) = note {
                selection.only(id);
                plan.edit_note = Some(id);
                return error;
            }
        }
    }
    // a view's title dragged on its own (MU27); checked before the notes take the click (MU29)
    if state.active.is_none() && plan.tool == planview::Tool::Select {
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(p) = ui
                .input(|i| i.pointer.press_origin())
                .or(pointer)
                .map(|q| xf.to_paper(q))
            {
                let on = views.iter().rev().find(|v| {
                    v.title_box().is_some_and(|(lo, hi)| {
                        p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y
                    })
                });
                if let Some(v) = on {
                    state.title_drag = Some((v.view, v.title_look.offset, p));
                    selection.only(v.viewport);
                }
            }
        }
        if let Some((view, off0, grab)) = state.title_drag {
            if let (true, Some(q)) = (response.dragged_by(egui::PointerButton::Primary), pointer) {
                let to = (off0 + xf.to_paper(q) - grab).round();
                let r = doc.edit_merged(view, "Move view title", |e| {
                    if let ElementKind::View(v) = &mut e.kind {
                        v.sheet_title.offset = to;
                    }
                });
                error = r.err().map(|e| e.to_string());
            }
            if response.drag_stopped() || !response.dragged() {
                state.title_drag = None;
            }
            return error;
        }
        // the title under the pointer: outlined, so it reads as its own thing
        if let Some(p) = response.hover_pos().map(|q| xf.to_paper(q)) {
            if let Some((lo, hi)) = views
                .iter()
                .filter_map(|v| v.title_box())
                .find(|(lo, hi)| p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y)
            {
                painter.rect_stroke(
                    xf.rect_of(lo, hi),
                    2.0,
                    egui::Stroke::new(1.0, hoverpick::HOVER.gamma_multiply(0.7)),
                    egui::StrokeKind::Outside,
                );
            }
        }
    }
    // notes written on the sheet itself
    if state.active.is_none() {
        let pxf = planview::Xf {
            rect,
            center,
            scale: zoom,
        };
        let took = crate::annotate::sheet_input(
            doc,
            sheet_id,
            &painter,
            &response,
            plan.tool,
            &mut plan.anno,
            selection,
            &pxf,
            &mut plan.status,
            &mut error,
        );
        if took {
            return error;
        }
    }

    // picking, moving and activating viewports
    let hit = pointer.and_then(|p| {
        let p = xf.to_paper(p);
        views.iter().rev().find(|v| contains(v, p))
    });

    // a box dragged on empty paper picks several at once: left to right
    // what is wholly inside, right to left what it touches; Shift adds
    // (MU25)
    if state.active.is_none() && plan.tool == planview::Tool::Select && plan.modal.is_none() {
        if response.drag_started_by(egui::PointerButton::Primary) {
            let press = ui.input(|i| i.pointer.press_origin()).or(pointer);
            if let Some(q) = press {
                let p = xf.to_paper(q);
                let on_mark = mark_boxes
                    .iter()
                    .any(|(_, (lo, hi))| p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y);
                let on_note = note_items.iter().any(|it| {
                    matches!(it, creabim_geom::PlanItem::Annotation { item, .. } if item.hit(p, 6.0 / zoom as f64, 1.0))
                });
                if !on_mark && !on_note && !views.iter().any(|v| contains(v, p)) {
                    state.band = Some(q);
                }
            }
        }
        if let Some(a) = state.band {
            if let Some(b) = pointer {
                let r = Rect::from_two_pos(a, b);
                let touching = b.x < a.x;
                let stroke = egui::Stroke::new(1.0, interaction_ink());
                painter.rect_filled(r, 0.0, interaction_ink().gamma_multiply(0.08));
                if touching {
                    let c = [
                        r.left_top(),
                        r.right_top(),
                        r.right_bottom(),
                        r.left_bottom(),
                        r.left_top(),
                    ];
                    painter.add(egui::Shape::dashed_line(&c, stroke, 5.0, 3.0));
                } else {
                    painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Middle);
                }
                // what the box catches, lit as it is dragged (MU26)
                let (pa, pb) = (xf.to_paper(a), xf.to_paper(b));
                let (lo, hi) = (pa.min(pb), pa.max(pb));
                let take = |(l, h): (DVec2, DVec2)| {
                    if touching {
                        l.x <= hi.x && h.x >= lo.x && l.y <= hi.y && h.y >= lo.y
                    } else {
                        l.x >= lo.x && h.x <= hi.x && l.y >= lo.y && h.y <= hi.y
                    }
                };
                let lit = egui::Stroke::new(2.0, interaction_ink());
                let mut ids: Vec<ElementId> = vec![];
                for (id, bx) in &mark_boxes {
                    if take(*bx) {
                        ids.push(*id);
                        painter.rect_stroke(
                            xf.rect_of(bx.0, bx.1).expand(3.0),
                            2.0,
                            lit,
                            egui::StrokeKind::Outside,
                        );
                    }
                }
                let pxf = planview::Xf {
                    rect,
                    center,
                    scale: zoom,
                };
                let model = doc.model();
                for it in &note_items {
                    if let creabim_geom::PlanItem::Annotation { id, item } = it {
                        if let Some(ElementKind::Note(n)) = model.get(*id).map(|e| &e.kind) {
                            if take((n.at - DVec2::splat(2.0), n.at + DVec2::splat(2.0))) {
                                ids.push(*id);
                                crate::annotate::paint_weighted(
                                    &painter,
                                    &pxf,
                                    item,
                                    interaction_ink(),
                                    1.0,
                                    planview::TRACE,
                                );
                            }
                        }
                    }
                }
                for v in views {
                    if take(v.bounds) {
                        ids.push(v.viewport);
                        painter.rect_stroke(
                            xf.rect_of(v.bounds.0, v.bounds.1).expand(4.0),
                            2.0,
                            lit,
                            egui::StrokeKind::Outside,
                        );
                    }
                }
                let n = ids.len();
                painter.text(
                    b + Vec2::new(12.0, 12.0),
                    egui::Align2::LEFT_TOP,
                    format!("{n}"),
                    egui::FontId::proportional(12.0),
                    interaction_ink(),
                );
                if response.drag_stopped() {
                    let add = ui.input(|i| i.modifiers.shift);
                    selection.extend(ids.iter().copied(), add);
                    plan.status = Some(match ids.len() {
                        0 => "Nothing in the box.".into(),
                        1 => "1 selected.".into(),
                        n => format!("{n} selected. M moves them together; Delete removes them."),
                    });
                    state.band = None;
                }
            }
            if !response.dragged() && !response.drag_stopped() {
                state.band = None;
            }
            return error;
        }
    }
    if response.double_clicked() {
        state.active = hit.map(|v| v.viewport);
        if state.active.is_some() {
            selection.clear();
        }
    } else if response.clicked() {
        let chosen = pointer.and_then(|p| plan.hover.as_ref()?.at(pane, p));
        selection.set(chosen.or(hit.map(|v| v.viewport)));
        state.active = None;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !plan.busy() {
        state.active = None;
    }
    // right-click a view for its menu: its scale, and opening it
    if response.secondary_clicked()
        && state.active.is_none()
        && plan.tool == planview::Tool::Select
        && plan.web_menus
    {
        if let Some(v) = hit {
            selection.only(v.viewport);
        }
        plan.menu_at = None;
        plan.menu_request = pointer;
    }
    if response.drag_started_by(egui::PointerButton::Primary) && state.crop_drag.is_none() {
        let press = ui.input(|i| i.pointer.press_origin()).or(pointer);
        let hit = press.and_then(|p| {
            let p = xf.to_paper(p);
            views.iter().rev().find(|v| contains(v, p))
        });
        if let (Some(v), Some(p)) = (hit, press) {
            let center = match doc.model().get(v.viewport).map(|e| &e.kind) {
                Some(ElementKind::Viewport(vp)) => vp.center,
                _ => v.map.paper,
            };
            state.drag = Some((v.viewport, center, xf.to_paper(p)));
            selection.only(v.viewport);
        }
    }
    if let (Some((id, c0, grab)), Some(p)) = (state.drag, pointer) {
        if response.dragged_by(egui::PointerButton::Primary) {
            let mut moved = (c0 + xf.to_paper(p) - grab).round();
            // Align views (Annotate board): in line with another view's
            // centre across or up the sheet, within 6 px, a guide shown
            let tol = 6.0 / zoom as f64;
            let others: Vec<DVec2> = views
                .iter()
                .filter(|v| v.viewport != id)
                .filter_map(|v| match doc.model().get(v.viewport).map(|e| &e.kind) {
                    Some(ElementKind::Viewport(vp)) => Some(vp.center),
                    _ => None,
                })
                .collect();
            let guide = egui::Stroke::new(1.0, interaction_ink());
            if let Some(o) = others.iter().find(|o| (o.x - moved.x).abs() < tol) {
                moved.x = o.x;
                painter.extend(egui::Shape::dashed_line(
                    &[
                        xf.to_screen(DVec2::new(o.x, o.y)),
                        xf.to_screen(DVec2::new(o.x, moved.y)),
                    ],
                    guide,
                    5.0,
                    4.0,
                ));
            }
            if let Some(o) = others.iter().find(|o| (o.y - moved.y).abs() < tol) {
                moved.y = o.y;
                painter.extend(egui::Shape::dashed_line(
                    &[
                        xf.to_screen(DVec2::new(o.x, o.y)),
                        xf.to_screen(DVec2::new(moved.x, o.y)),
                    ],
                    guide,
                    5.0,
                    4.0,
                ));
            }
            // and onto the sheet's guides, by its centre or its edges
            if let Some(v) = views.iter().find(|v| v.viewport == id) {
                let now = match doc.model().get(id).map(|e| &e.kind) {
                    Some(ElementKind::Viewport(vp)) => vp.center,
                    _ => moved,
                };
                let offs = [DVec2::ZERO, v.bounds.0 - now, v.bounds.1 - now];
                moved = snap_guides(moved, &offs, &guides, tol);
            }
            let r = doc.edit_merged(id, "Move view on sheet", |e| {
                if let ElementKind::Viewport(vp) = &mut e.kind {
                    vp.center = moved;
                }
            });
            error = r.err().map(|e| e.to_string());
        }
    }
    if response.drag_stopped() {
        state.drag = None;
    }

    // the active viewport is a live editor of the model
    if let Some(v) = state
        .active
        .and_then(|a| views.iter().find(|v| v.viewport == a))
    {
        let (lo, hi) = v.bounds;
        let area = xf.rect_of(lo, hi).expand(6.0).intersect(rect);
        let paper_mid = xf.to_paper(area.center());
        let camera = (v.map.to_model(paper_mid), zoom * v.map.k() as f32);
        let kind = doc.model().get(v.view).and_then(|e| match &e.kind {
            ElementKind::View(view) => Some(planview::pane_kind(view)),
            _ => None,
        });
        if let Some(kind) = kind {
            let style = PaneStyle {
                background: Some(Color32::from_rgb(255, 253, 248)),
                decorations: false,
                camera: Some(camera),
                scale: v.map.scale,
            };
            // work on the whole view, not just its cropped part
            let full;
            let drawing = if matches!(doc.model().get(v.viewport).map(|e| &e.kind), Some(ElementKind::Viewport(vp)) if vp.crop.is_some())
            {
                full = creabim_geom::section::view_drawing(doc.model(), v.view).unwrap_or_default();
                &full
            } else {
                &v.drawing
            };
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
            if let Some(e) = planview::show(
                &mut child, area, doc, v.view, kind, drawing, plan, selection, style,
            ) {
                error = Some(e);
            }
            painter.rect_stroke(
                area,
                2.0,
                egui::Stroke::new(2.0, interaction_ink()),
                egui::StrokeKind::Outside,
            );
            painter.text(
                area.left_top() + Vec2::new(0.0, -6.0),
                egui::Align2::LEFT_BOTTOM,
                format!("Working in {}. Esc or click outside to finish.", v.title),
                egui::FontId::proportional(12.0),
                interaction_ink(),
            );
        }
    }

    // said in the status bar, never over the sheet
    if ui.rect_contains_pointer(rect) {
        plan.hint = Some(if state.active.is_some() {
            "Edits here change the model: every view, sheet and the 3D update at once."
        } else {
            "Drag a view to move it, or its orange handles to crop it. Double-click a view to work on the model through it. Right-drag pans, the wheel zooms."
        });
    }
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_scale_grows_the_view_about_its_middle() {
        // pinned by the drawing's corner (0, 0) at 100, 100 on the sheet;
        // its middle (10, 5) is then at 200, 150 at 1:100
        let vp = creabim_core::Viewport {
            sheet: ElementId::NIL,
            view: ElementId::NIL,
            center: DVec2::new(100.0, 100.0),
            target: DVec2::ZERO,
            crop: None,
        };
        let mid = DVec2::new(10.0, 5.0);
        let map = |c: DVec2, s: f64| c + (mid - vp.target) * (1000.0 / s);
        assert_eq!(map(vp.center, 100.0), DVec2::new(200.0, 150.0));
        for new in [50.0, 200.0, 20.0] {
            let c = rescaled_center(&vp, mid, 100.0, new);
            // the middle stays where it was on the sheet
            assert!(
                (map(c, new) - DVec2::new(200.0, 150.0)).length() < 1e-9,
                "1:{new}"
            );
        }
        // pinned by its middle: the pin doesn't move
        let pinned = creabim_core::Viewport { target: mid, ..vp };
        assert_eq!(rescaled_center(&pinned, mid, 100.0, 50.0), vp.center);
        assert!(SCALES.contains(&50.0) && SCALES.contains(&1.0) && SCALES.contains(&500.0));
    }
}
