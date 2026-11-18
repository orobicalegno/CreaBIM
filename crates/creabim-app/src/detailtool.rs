//! Detailing in the 2D panes: the Callout tool, which draws a box on a
//! plan or section and makes a larger-scale detail view of it, and the
//! Detail tool, which drafts detail components (flashings, membranes,
//! insulation, blocking, brick courses, hatched regions, break lines) in
//! the view. Also the Properties fields of a detail component and of a
//! junction rule.

use crate::planview::{PlanState, Xf};
use crate::selection::Selection;
use creabim_core::{
    DetailItem, DetailKind, Document, Element, ElementId, ElementKind, Hatch, JoinRule, JoinWinner,
    Model, ModelError, Transaction, View,
};
use eframe::egui::{self, Stroke};
use glam::DVec2;

/// What the Detail and Callout tools draw next, and the Detail tool's
/// points so far.
#[derive(Clone, Debug)]
pub struct DetailSettings {
    pub kind: DetailKind,
    /// Width of the next component; set from the kind when it is picked.
    pub width: f64,
    /// Points clicked so far, and the view they are in.
    pub path: Vec<DVec2>,
    /// Where each point was clicked from its snapped spot: which side of
    /// the line the pointer was on, for a membrane's side.
    pub off: Vec<DVec2>,
    /// Space while drawing a membrane puts it on the other side.
    pub swap: bool,
    pub view: Option<ElementId>,
    /// Scale of the next detail view, 1:`callout_scale`.
    pub callout_scale: f64,
    /// The drafting layer and line type new linework is drawn with; none
    /// is no layer, and the layer's (or the pen's) line type.
    pub layer: Option<String>,
    pub linetype: Option<creabim_core::LineType>,
    /// Editing linework instead of drawing: what the next clicks do.
    pub edit: Option<DraftEdit>,
    /// The piece picked first, and where, by an edit that takes two clicks.
    pub picked: Option<(ElementId, DVec2)>,
    /// A fillet's radius, a chamfer's distance back from the corner and
    /// an offset's distance.
    pub radius: f64,
    pub chamfer: f64,
    pub offset: f64,
    /// An array's columns and rows, and the step between them.
    pub across: u32,
    pub up: u32,
    pub step: DVec2,
    /// A polar array's count and the angle it is spread over, degrees.
    pub count: u32,
    pub angle: f64,
}

/// A change to the project's drafting layers made from Properties.
#[derive(Clone, Debug, PartialEq)]
pub enum LayerEdit {
    /// The table as it is to be: a layer added, removed, recoloured...
    Set(creabim_core::DraftLayers),
    /// A layer renamed, on its linework too.
    Rename { old: String, new: String },
    /// Make a symbol of the linework selected.
    MakeSymbol,
    /// Turn this placed symbol back into linework.
    Explode,
}

/// An edit of plain linework against other linework, CAD-style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftEdit {
    Trim,
    Extend,
    Fillet,
    Chamfer,
    Offset,
    Array,
    Polar,
}

impl DraftEdit {
    pub const ALL: [DraftEdit; 7] = [
        DraftEdit::Trim,
        DraftEdit::Extend,
        DraftEdit::Fillet,
        DraftEdit::Chamfer,
        DraftEdit::Offset,
        DraftEdit::Array,
        DraftEdit::Polar,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DraftEdit::Trim => "Trim",
            DraftEdit::Extend => "Extend",
            DraftEdit::Fillet => "Fillet",
            DraftEdit::Chamfer => "Chamfer",
            DraftEdit::Offset => "Offset",
            DraftEdit::Array => "Array",
            DraftEdit::Polar => "Polar array",
        }
    }

    /// What to click, for the status bar.
    pub fn hint(self) -> &'static str {
        match self {
            DraftEdit::Trim => "Trim: click the stretch of a line to take away, back to the lines that cross it.",
            DraftEdit::Extend => "Extend: click a line near the end to run on to the next line it meets.",
            DraftEdit::Fillet => "Fillet: click the two lines of a corner; it is rounded to the radius in the top bar (0 makes it sharp).",
            DraftEdit::Chamfer => "Chamfer: click the two lines of a corner; it is cut off the distance in the top bar back along each.",
            DraftEdit::Offset => "Offset: click a line, then the side to copy it to, at the distance in the top bar.",
            DraftEdit::Array => "Array: click a line to repeat it in the columns and rows in the top bar.",
            DraftEdit::Polar => "Polar array: click a line, then the centre to repeat it round.",
        }
    }
}

impl Default for DetailSettings {
    fn default() -> Self {
        Self {
            kind: DetailKind::Batt,
            width: DetailKind::Batt.default_width(),
            path: vec![],
            off: vec![],
            swap: false,
            view: None,
            callout_scale: 10.0,
            layer: None,
            linetype: None,
            edit: None,
            picked: None,
            radius: 0.05,
            chamfer: 0.05,
            offset: 0.1,
            across: 3,
            up: 1,
            step: DVec2::new(1.0, 1.0),
            count: 6,
            angle: 360.0,
        }
    }
}

/// The detail scales the Callout tool offers.
pub const CALLOUT_SCALES: [f64; 4] = [2.0, 5.0, 10.0, 20.0];

impl DetailSettings {
    /// Was the path clicked from the right of the way it runs? A membrane
    /// then lies on that side.
    pub fn clicked_right(&self) -> bool {
        self.swap != self.voted_right()
    }

    fn voted_right(&self) -> bool {
        let n = self.path.len();
        if n < 2 {
            return false;
        }
        let vote: f64 = (0..n.min(self.off.len()))
            .map(|i| {
                let (a, b) = if i + 1 < n {
                    (self.path[i], self.path[i + 1])
                } else {
                    (self.path[i - 1], self.path[i])
                };
                self.off[i].dot((b - a).normalize_or_zero().perp())
            })
            .sum();
        vote < -1e-9
    }

    /// Pick what the Detail tool draws, at its usual width.
    pub fn pick(&mut self, kind: DetailKind) {
        self.kind = kind;
        self.width = kind.default_width();
        self.edit = None;
        self.picked = None;
    }

    /// Pick an edit of linework: the next clicks edit instead of drawing.
    pub fn pick_edit(&mut self, edit: DraftEdit) {
        self.edit = Some(edit);
        self.picked = None;
        self.path.clear();
        self.off.clear();
    }
}

fn view_of(model: &Model, id: ElementId) -> Option<&View> {
    match &model.get(id)?.kind {
        ElementKind::View(v) => Some(v),
        _ => None,
    }
}

/// The next free "Detail N" name.
fn next_detail_name(model: &Model) -> String {
    (1..)
        .map(|n| format!("Detail {n}"))
        .find(|n| !model.iter().any(|e| &e.name == n))
        .expect("unbounded")
}

/// A click with the Callout tool in `view`: the box's first corner, then
/// its second, which makes the detail view. Returns the new view's id.
pub fn callout_click(
    doc: &mut Document,
    view: ElementId,
    p: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<Option<ElementId>, ModelError> {
    let Some(parent) = view_of(doc.model(), view).cloned() else {
        return Ok(None);
    };
    if !matches!(
        parent.kind,
        creabim_core::ViewKind::Plan { .. } | creabim_core::ViewKind::Section { .. }
    ) {
        state.status = Some("Callouts are drawn on plans and sections.".into());
        return Ok(None);
    }
    let Some(a) = state.pending.take() else {
        state.pending = Some(p);
        state.status = Some("Click the opposite corner of the callout's box.".into());
        return Ok(None);
    };
    let size = (p - a).abs();
    if size.x < 0.05 || size.y < 0.05 {
        state.pending = Some(a);
        return Ok(None);
    }
    let name = next_detail_name(doc.model());
    let detail = View::callout(view, &parent, a, p, state.details.callout_scale);
    let e = Element::new(name.clone(), ElementKind::View(detail));
    let id = e.id;
    doc.commit(Transaction::new(format!("Call out {name}")).insert(e))?;
    selection.only(id);
    state.status = Some(format!(
        "{name} at 1:{:.0} is under Details in the project browser. Open this view (in Properties) or double-click the callout to draw in it; it follows the model.",
        state.details.callout_scale
    ));
    Ok(Some(id))
}

/// A click with the Detail tool in `view`: each click adds a point; a
/// click on the last point again (or Enter) finishes the path, and for a
/// filled region a click on the first point closes it. A break line ends
/// at its second point.
pub fn detail_click(
    doc: &mut Document,
    view: ElementId,
    p: DVec2,
    raw: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    if let Some(edit) = state.details.edit {
        return edit_click(doc, view, edit, p, raw, state, selection);
    }
    let d = &mut state.details;
    if d.view != Some(view) {
        d.path.clear();
        d.off.clear();
        d.view = Some(view);
    }
    let near = |a: DVec2| a.distance(p) < 0.004_f64.max(view_scale(doc.model(), view) * 0.0015);
    let fixed = d.kind.fixed_points();
    let done = match (d.path.first(), d.path.last()) {
        (Some(first), _) if d.kind.is_area() && d.path.len() >= 3 && near(*first) => true,
        // a symbol clicked twice on one spot is placed level
        (_, Some(last)) if near(*last) && matches!(d.kind, DetailKind::Symbol { .. }) => {
            let level = *last + DVec2::X;
            d.path.push(level);
            d.off.push(DVec2::ZERO);
            true
        }
        // a shape of a set number of clicks takes them all: a click on
        // the last point again is dropped, not the end of it
        (_, Some(last)) if near(*last) && fixed.is_some() => false,
        (_, Some(last)) if near(*last) => true,
        _ => {
            if d.path.is_empty() {
                d.swap = false;
            }
            d.path.push(p);
            d.off.truncate(d.path.len() - 1);
            d.off.resize(d.path.len() - 1, DVec2::ZERO);
            d.off.push(raw - p);
            fixed == Some(d.path.len())
        }
    };
    if done {
        return finish(doc, state, selection);
    }
    state.status = Some(path_hint(d.kind, d.path.len()));
    Ok(())
}

/// The plain linework drawn in `view`.
fn linework(model: &Model, view: ElementId) -> Vec<(Element, DetailItem)> {
    model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Detail(d) if d.view == view && d.kind.as_draft().is_some() => {
                Some((e.clone(), d.clone()))
            }
            _ => None,
        })
        .collect()
}

/// A click while editing linework in `view`: `raw` is where the pointer
/// is, to pick a line by, and `p` that point snapped, for a side or a
/// centre.
fn edit_click(
    doc: &mut Document,
    view: ElementId,
    edit: DraftEdit,
    p: DVec2,
    raw: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    use creabim_core::draftedit as de;
    let lines = linework(doc.model(), view);
    // within 3 mm on paper of a line picks it
    let reach = view_scale(doc.model(), view) * 0.003;
    let under = |at: DVec2| -> Option<usize> {
        lines
            .iter()
            .enumerate()
            .filter_map(|(i, (_, d))| Some((i, de::hit(d, at)?.distance)))
            .filter(|(_, dist)| *dist <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };
    let others = |i: usize| -> Vec<&DetailItem> {
        lines
            .iter()
            .enumerate()
            .filter(|(k, _)| *k != i)
            .map(|(_, l)| &l.1)
            .collect()
    };
    let changed = |e: &Element, d: DetailItem| {
        let mut after = e.clone();
        after.kind = ElementKind::Detail(d);
        after
    };
    let fresh = |d: DetailItem| Element::new(d.kind.label(), ElementKind::Detail(d));
    let d = &mut state.details;
    d.view = Some(view);
    // an edit of two clicks: the piece picked first, if it is still there
    let first = d
        .picked
        .take()
        .and_then(|(id, at)| Some((lines.iter().position(|l| l.0.id == id)?, at)));
    let nothing = |state: &mut PlanState| {
        state.status = Some(format!(
            "{} Nothing drawn is under the pointer.",
            edit.hint()
        ));
        Ok(())
    };
    match (edit, first) {
        (DraftEdit::Trim, _) => {
            let Some(i) = under(raw) else {
                return nothing(state);
            };
            let Some(mut left) = de::trim(&lines[i].1, &others(i), raw) else {
                state.status = Some(
                    "Trim: nothing crosses that line to trim it back to. Delete removes it whole."
                        .into(),
                );
                return Ok(());
            };
            let mut tx = Transaction::new("Trim line");
            let kept = left.remove(0);
            tx = tx.replace(lines[i].0.clone(), changed(&lines[i].0, kept));
            for d in left {
                tx = tx.insert(fresh(d));
            }
            state.status = Some(
                "Trimmed. Click another stretch, or pick something else in the top bar.".into(),
            );
            doc.commit(tx)
        }
        (DraftEdit::Extend, _) => {
            let Some(i) = under(raw) else {
                return nothing(state);
            };
            let Some(longer) = de::extend(&lines[i].1, &others(i), raw) else {
                state.status = Some(
                    "Extend: that end meets no line if it runs straight on (only open lines extend).".into(),
                );
                return Ok(());
            };
            state.status = Some("Extended. Click another end.".into());
            doc.commit(
                Transaction::new("Extend line")
                    .replace(lines[i].0.clone(), changed(&lines[i].0, longer)),
            )
        }
        (DraftEdit::Fillet | DraftEdit::Chamfer, None) => {
            let Some(i) = under(raw) else {
                return nothing(state);
            };
            state.details.picked = Some((lines[i].0.id, raw));
            selection.only(lines[i].0.id);
            state.status = Some(format!(
                "{}: click the other line of the corner.",
                edit.label()
            ));
            Ok(())
        }
        (DraftEdit::Fillet | DraftEdit::Chamfer, Some((a, at))) => {
            let Some(b) = under(raw) else {
                return nothing(state);
            };
            let how = if edit == DraftEdit::Fillet {
                de::Join::Fillet(state.details.radius)
            } else {
                de::Join::Chamfer(state.details.chamfer)
            };
            match de::join(&lines[a].1, at, &lines[b].1, raw, a == b, how) {
                Err(why) => {
                    state.status = Some(format!("{}: {why}", edit.label()));
                    Ok(())
                }
                Ok(j) => {
                    let mut tx = Transaction::new(format!("{} corner", edit.label()))
                        .replace(lines[a].0.clone(), changed(&lines[a].0, j.a));
                    if let Some(nb) = j.b {
                        tx = tx.replace(lines[b].0.clone(), changed(&lines[b].0, nb));
                    }
                    if let Some(link) = j.between {
                        tx = tx.insert(fresh(link));
                    }
                    state.status = Some(format!(
                        "{} done. Click the next corner's first line.",
                        edit.label()
                    ));
                    doc.commit(tx)
                }
            }
        }
        (DraftEdit::Offset | DraftEdit::Polar, None) => {
            let Some(i) = under(raw) else {
                return nothing(state);
            };
            state.details.picked = Some((lines[i].0.id, raw));
            selection.only(lines[i].0.id);
            state.status = Some(if edit == DraftEdit::Offset {
                "Offset: click the side to copy it to.".into()
            } else {
                "Polar array: click the centre to repeat it round.".into()
            });
            Ok(())
        }
        (DraftEdit::Offset, Some((i, _))) => {
            let Some(copy) = de::offset(&lines[i].1, state.details.offset, p) else {
                state.status = Some("Offset: there is no room for a copy that far inside.".into());
                return Ok(());
            };
            let e = fresh(copy);
            // the copy is the next to offset, as CAD programs do
            state.details.picked = None;
            selection.only(e.id);
            state.status = Some("Offset. Click another line to offset.".into());
            doc.commit(Transaction::new("Offset line").insert(e))
        }
        (DraftEdit::Polar, Some((i, _))) => {
            let copies = de::array_polar(
                &lines[i].1,
                p,
                state.details.count,
                state.details.angle.to_radians(),
            );
            let n = copies.len();
            let mut tx = Transaction::new("Polar array");
            for d in copies.into_iter().skip(1) {
                tx = tx.insert(fresh(d));
            }
            state.status = Some(format!("{n} round the centre, the first where it was."));
            doc.commit(tx)
        }
        (DraftEdit::Array, _) => {
            let Some(i) = under(raw) else {
                return nothing(state);
            };
            let d = &state.details;
            let copies = de::array_grid(&lines[i].1, d.across, d.up, d.step);
            let n = copies.len();
            let mut tx = Transaction::new("Array");
            for d in copies.into_iter().skip(1) {
                tx = tx.insert(fresh(d));
            }
            state.status = Some(format!("{n} in rows and columns, the first where it was."));
            doc.commit(tx)
        }
    }
}

/// What the status bar says while a path is drawn: how many points it has
/// and how to finish it.
pub fn path_hint(kind: DetailKind, n: usize) -> String {
    let points = if n == 1 {
        "1 point".to_string()
    } else {
        format!("{n} points")
    };
    let label = kind.label();
    if let DetailKind::Symbol { .. } = kind {
        return if n == 0 {
            "Symbol: click where it goes.".into()
        } else {
            "Symbol: click a point to turn it towards, or the same spot again to place it level."
                .into()
        };
    }
    if let (Some((shape, _)), Some(_)) = (kind.as_draft(), kind.fixed_points()) {
        return format!("{label}: click {}. Esc cancels.", shape.next_point(n));
    }
    match (kind.is_area(), n) {
        (true, n) if n >= 3 => format!(
            "{label}: {points}. Enter to finish (or click the first corner); double-click or right-click also finish."
        ),
        (true, _) => format!("{label}: {points}. Click the next corner."),
        (false, 1) => format!("{label}: {points}. Click the next point."),
        _ if matches!(kind, DetailKind::Membrane) => format!(
            "{label}: {points}. Enter to finish; double-click or right-click also finish. Esc keeps it. Space puts it on the other side."
        ),
        _ => format!(
            "{label}: {points}. Enter to finish; double-click or right-click also finish. Esc keeps it."
        ),
    }
}

/// Has the path enough points to make its component?
pub fn finishable(state: &PlanState) -> bool {
    let d = &state.details;
    d.view.is_some() && d.path.len() >= d.kind.min_points()
}

/// Esc, a right-click or another tool while a path is drawn: one that can
/// be made is kept, never dropped without a word. True when it was.
pub fn keep(doc: &mut Document, state: &mut PlanState, selection: &mut Selection) -> bool {
    if !finishable(state) {
        return false;
    }
    let n = state.details.path.len();
    let label = state.details.kind.label();
    match finish(doc, state, selection) {
        Ok(()) => {
            state.status = Some(format!(
                "{label} kept with its {n} points. Ctrl+Z takes it away."
            ));
            true
        }
        Err(e) => {
            state.status = Some(e.to_string());
            false
        }
    }
}

fn view_scale(model: &Model, view: ElementId) -> f64 {
    match model.get(view).map(|e| &e.kind) {
        Some(ElementKind::View(v)) => v.scale,
        // on a sheet a drawing unit is a paper millimetre
        Some(ElementKind::Sheet(_)) => 1000.0,
        _ => 100.0,
    }
}

/// The box round a detail component or a piece of linework, in its
/// view's drawing units (on a sheet, paper millimetres).
pub fn bounds(model: &Model, d: &DetailItem) -> (DVec2, DVec2) {
    let mut pts = match (d.kind, d.draft_path()) {
        (_, Some((path, _))) => path,
        (DetailKind::Symbol { def }, _) => match model.get(def).map(|e| &e.kind) {
            Some(ElementKind::Symbol(s)) => d
                .symbol_pieces(s)
                .iter()
                .flat_map(|p| p.draft_path().map(|q| q.0).unwrap_or_default())
                .collect(),
            _ => d.points.clone(),
        },
        _ => d.points.clone(),
    };
    if pts.is_empty() {
        pts.push(DVec2::ZERO);
    }
    let lo = pts.iter().copied().fold(DVec2::splat(f64::MAX), DVec2::min);
    let hi = pts.iter().copied().fold(DVec2::splat(f64::MIN), DVec2::max);
    let pad = DVec2::splat(d.width / 2.0);
    (lo - pad, hi + pad)
}

/// The Detail tool on a sheet, outside any view: linework and symbols
/// drawn on the paper itself, in millimetres. `xf` maps paper to the
/// screen. True when it took this frame's input.
#[allow(clippy::too_many_arguments)]
pub fn sheet_input(
    ui: &egui::Ui,
    doc: &mut Document,
    sheet: ElementId,
    painter: &egui::Painter,
    response: &egui::Response,
    state: &mut PlanState,
    selection: &mut Selection,
    xf: &Xf,
    error: &mut Option<String>,
) -> bool {
    let hover = response.hover_pos().map(|p| xf.to_world(p));
    preview(
        doc.model(),
        painter,
        xf,
        crate::planview::Tool::Detail,
        state,
        sheet,
        hover,
        1.0,
    );
    let keys = !ui.ctx().egui_wants_keyboard_input();
    let enter = keys && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let esc = keys && ui.input(|i| i.key_pressed(egui::Key::Escape));
    let here = state.details.view == Some(sheet);
    // Esc or a right-click keeps a path that can be made
    if response.hovered() && (esc || response.secondary_clicked()) && here {
        if keep(doc, state, selection) {
            return true;
        }
        if !state.details.path.is_empty() || state.details.picked.is_some() {
            state.details.path.clear();
            state.details.off.clear();
            state.details.picked = None;
            return true;
        }
        return false;
    }
    let double = response.double_clicked() && finishable(state);
    if (enter || double) && here && finishable(state) {
        if let Err(e) = finish(doc, state, selection) {
            *error = Some(e.to_string());
        }
        return true;
    }
    if response.clicked() {
        if let Some(p) = response.interact_pointer_pos().map(|p| xf.to_world(p)) {
            if let Err(e) = detail_click(doc, sheet, p, p, state, selection) {
                *error = Some(e.to_string());
            }
            return true;
        }
    }
    false
}

/// Make the component from the points clicked so far.
pub fn finish(
    doc: &mut Document,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let d = &mut state.details;
    let (Some(view), path) = (d.view, std::mem::take(&mut d.path)) else {
        return Ok(());
    };
    let need = d.kind.min_points();
    if path.len() < need {
        state.status = Some(format!(
            "A {} needs {need} points or more.",
            d.kind.label().to_lowercase()
        ));
        return Ok(());
    }
    d.path = path;
    let flip = d.clicked_right();
    let path = std::mem::take(&mut d.path);
    d.off.clear();
    let mut item = DetailItem::new(view, d.kind, path);
    item.width = d.width;
    item.flip = flip && matches!(d.kind, DetailKind::Membrane);
    if d.kind.as_draft().is_some() {
        item.layer = d.layer.clone();
        item.linetype = d.linetype;
    }
    if let DetailKind::Symbol { .. } = d.kind {
        // the second click only turns it: it is placed at its own size
        let (o, x) = (item.points[0], item.points[1]);
        item.points[1] = o + (x - o).try_normalize().unwrap_or(DVec2::X);
        item.layer = d.layer.clone();
    }
    let label = d.kind.label();
    // a placed symbol is named after its symbol
    let name = match d.kind {
        DetailKind::Symbol { def } => doc
            .model()
            .get(def)
            .map_or(label.to_string(), |e| e.name.clone()),
        _ => label.to_string(),
    };
    // a rectangle or a polygon is kept as the closed outline it draws
    let e = Element::new(name, ElementKind::Detail(item.baked()));
    selection.only(e.id);
    state.status = Some(if d.kind.as_draft().is_some() {
        format!("{label} drawn. Its pen is in Properties; it shows in this view only.")
    } else {
        format!("{label} drawn. Its width and kind are in Properties; it shows in this view only.")
    });
    doc.commit(Transaction::new(format!("Draw {}", label.to_lowercase())).insert(e))
}

/// The Callout box or the Detail path being drawn, under the pointer.
#[allow(clippy::too_many_arguments)]
pub fn preview(
    model: &Model,
    painter: &egui::Painter,
    xf: &Xf,
    tool: crate::planview::Tool,
    state: &PlanState,
    view: ElementId,
    hover: Option<DVec2>,
    mm: f64,
) {
    let accent = Stroke::new(1.5, crate::theme::interaction());
    match tool {
        crate::planview::Tool::Callout => {
            if let (Some(a), Some(b)) = (state.pending, hover) {
                let r = egui::Rect::from_two_pos(xf.to_screen(a), xf.to_screen(b));
                painter.rect_stroke(r, 0.0, accent, egui::StrokeKind::Middle);
            }
        }
        crate::planview::Tool::Detail => {
            let d = &state.details;
            if d.view != Some(view) || d.path.is_empty() {
                return;
            }
            let mut pts = d.path.clone();
            pts.extend(hover);
            // an arc or an ellipse with a point still to come: the line so far
            let short = d.kind.fixed_points().is_some_and(|n| pts.len() < n);
            let kind = match d.kind.as_draft() {
                Some((_, pen)) if short => DetailKind::Draft {
                    shape: creabim_core::DraftShape::Line,
                    pen,
                },
                _ => d.kind,
            };
            // the side as it will be with the pointer's point added
            let mut next = d.clone();
            next.path = pts.clone();
            next.off.resize(pts.len(), DVec2::ZERO);
            let mut item = DetailItem::new(view, kind, pts);
            item.width = d.width;
            item.flip = next.clicked_right();
            item.linetype = d.linetype;
            let look = creabim_core::styles::active_pack(model).membranes;
            // drawn even while its layer is hidden, so it can be seen going down
            let mut drafting = creabim_core::Drafting::of(model);
            drafting.layers = Default::default();
            if let DetailKind::Symbol { .. } = d.kind {
                // one point down: level under the pointer; then turned to it
                let o = item.points[0];
                let x = item.points.get(1).copied().unwrap_or(o + DVec2::X);
                item.points = vec![o, o + (x - o).try_normalize().unwrap_or(DVec2::X)];
            }
            for it in
                creabim_geom::detail::detail_items(ElementId::new(), &item, mm, look, &drafting)
            {
                crate::planview::draw_item(painter, xf, &it, true, mm);
            }
        }
        _ => {}
    }
}

/// A component's settings in Properties; returns the edit's name when one
/// changed. Plain linework also shows the project's drafting `layers`,
/// and puts a change to them in `layer_edit`.
pub fn detail_fields(
    ui: &mut egui::Ui,
    d: &mut DetailItem,
    layers: &creabim_core::DraftLayers,
    layer_edit: &mut Option<LayerEdit>,
) -> &'static str {
    use creabim_core::{DraftLayer, DraftShape, LinePen, LineType};
    let mut label = "";
    let fillable = d.fillable();
    if let DetailKind::Symbol { .. } = d.kind {
        let (o, x) = (d.points[0], d.points[1]);
        let mut scale = o.distance(x);
        crate::props::row(ui, "Scale", |ui| {
            if ui
                .add(
                    egui::DragValue::new(&mut scale)
                        .speed(0.01)
                        .range(0.001..=1000.0),
                )
                .on_hover_text("1 is the size the symbol was drawn at.")
                .changed()
            {
                d.points[1] = o + (x - o).normalize_or(DVec2::X) * scale.max(0.001);
                label = "Symbol scale";
            }
        });
        let mut angle = (x - o).to_angle().to_degrees();
        crate::props::row(ui, "Angle", |ui| {
            if ui
                .add(egui::DragValue::new(&mut angle).speed(1.0).suffix("°"))
                .changed()
            {
                d.points[1] = o + DVec2::from_angle(angle.to_radians()) * scale;
                label = "Symbol angle";
            }
        });
        if ui
            .button("Explode")
            .on_hover_text("Turn this one back into linework of its own, where it is. The symbol and its other placings stay.")
            .clicked()
        {
            *layer_edit = Some(LayerEdit::Explode);
        }
        ui.label(
            egui::RichText::new(
                "A placed symbol: change the symbol's linework by exploding one, editing it and making a symbol of it again.",
            )
            .weak(),
        );
        return label;
    }
    if let DetailKind::Draft { shape, pen } = &mut d.kind {
        // plain linework: its shape is what its points mean
        let name = shape.label();
        crate::props::Combo::new("Pen")
            .selected_text(pen.label())
            .show_ui(ui, |ui| {
                for p in LinePen::ALL {
                    if ui.selectable_value(pen, p, p.label()).changed() {
                        label = "Line pen";
                    }
                }
            });
        let by = |what: &str| format!("By {what}");
        let on_layer = d.layer.as_deref().and_then(|n| layers.get(n));
        crate::props::Combo::new("Line type")
            .selected_text(
                d.linetype
                    .map_or(by(if on_layer.is_some() { "layer" } else { "pen" }), |t| {
                        t.label().into()
                    }),
            )
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(&mut d.linetype, None, by("layer or pen"))
                    .changed()
                {
                    label = "Line type";
                }
                for t in LineType::ALL {
                    if ui
                        .selectable_value(&mut d.linetype, Some(t), t.label())
                        .changed()
                    {
                        label = "Line type";
                    }
                }
            });
        crate::props::row(ui, "Colour", |ui| {
            crate::props::flow(ui, |ui| {
                let mut own = d.color.is_some();
                if crate::props::switch(ui, &mut own, "Own")
                    .on_hover_text("Off, it takes its layer's colour, or black.")
                    .changed()
                {
                    d.color = own.then(|| on_layer.and_then(|l| l.color).unwrap_or([0, 0, 0]));
                    label = "Line colour";
                }
                if let Some(c) = &mut d.color {
                    if ui.color_edit_button_srgb(c).changed() {
                        label = "Line colour";
                    }
                }
            })
        });
        crate::props::Combo::new("Layer")
            .selected_text(d.layer.clone().unwrap_or_else(|| "None".into()))
            .show_ui(ui, |ui| {
                if ui.selectable_value(&mut d.layer, None, "None").changed() {
                    label = "Line layer";
                }
                for l in &layers.layers {
                    if ui
                        .selectable_value(&mut d.layer, Some(l.name.clone()), &l.name)
                        .changed()
                    {
                        label = "Line layer";
                    }
                }
            });
        if fillable {
            crate::props::Combo::new("Fill")
                .selected_text(d.fill.map_or("None", |h| h.label()))
                .show_ui(ui, |ui| {
                    if ui.selectable_value(&mut d.fill, None, "None").changed() {
                        label = "Line fill";
                    }
                    for h in Hatch::ALL {
                        if h != Hatch::Empty
                            && ui
                                .selectable_value(&mut d.fill, Some(h), h.label())
                                .changed()
                        {
                            label = "Line fill";
                        }
                    }
                });
        }
        if let DraftShape::Line | DraftShape::Closed = shape {
            let mut closed = *shape == DraftShape::Closed;
            if d.points.len() >= 3
                && crate::props::switch(ui, &mut closed, "Closed")
                    .on_hover_text("Join its end back to its start.")
                    .changed()
            {
                *shape = if closed {
                    DraftShape::Closed
                } else {
                    DraftShape::Line
                };
                if !closed {
                    d.fill = None;
                }
                label = "Close line";
            }
        }
        ui.label(
            egui::RichText::new(format!(
                "{name}, {} long. Linework is drafting: it shows in this view only, never in 3D, schedules or IFC.",
                crate::units::len(d.length())
            ))
            .weak(),
        );
        if ui
            .button("Make symbol")
            .on_hover_text("Make a symbol of the linework selected, to place again and again with the Detail tool. One of it takes their place.")
            .clicked()
        {
            *layer_edit = Some(LayerEdit::MakeSymbol);
        }
        // the project's layers, edited here: they are every view's
        if crate::props::subsection(ui, "draft.layers", "Drafting layers", false) {
            let mut table = layers.clone();
            let mut changed = false;
            let mut remove = None;
            for (i, l) in table.layers.iter_mut().enumerate() {
                crate::props::flow(ui, |ui| {
                    let mut shown = !l.hidden;
                    if ui
                        .checkbox(&mut shown, "")
                        .on_hover_text(
                            "Show or hide this layer's linework, in every view and on sheets.",
                        )
                        .changed()
                    {
                        l.hidden = !shown;
                        changed = true;
                    }
                    // a name is typed whole, then kept: every line on the
                    // layer is renamed with it
                    let id = ui.id().with(("draft.layer.name", i));
                    let mut text = ui
                        .data_mut(|m| m.get_temp::<String>(id))
                        .unwrap_or_else(|| l.name.clone());
                    let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(90.0));
                    if r.lost_focus() {
                        ui.data_mut(|m| m.remove::<String>(id));
                        if text.trim() != l.name && !text.trim().is_empty() {
                            *layer_edit = Some(LayerEdit::Rename {
                                old: l.name.clone(),
                                new: text.trim().to_string(),
                            });
                        }
                    } else if r.changed() || r.has_focus() {
                        ui.data_mut(|m| m.insert_temp(id, text));
                    }
                    let mut own = l.color.is_some();
                    if ui
                        .checkbox(&mut own, "")
                        .on_hover_text("Give the layer a colour; off, its linework is black.")
                        .changed()
                    {
                        l.color = own.then_some([200, 0, 0]);
                        changed = true;
                    }
                    if let Some(c) = &mut l.color {
                        changed |= ui.color_edit_button_srgb(c).changed();
                    }
                    egui::ComboBox::from_id_salt(("draft.layer.type", i))
                        .icon(crate::props::combo_icon)
                        .width(84.0)
                        .selected_text(l.linetype.label())
                        .show_ui(ui, |ui| {
                            for t in LineType::ALL {
                                changed |=
                                    ui.selectable_value(&mut l.linetype, t, t.label()).changed();
                            }
                        });
                    if ui
                        .small_button("✕")
                        .on_hover_text("Remove the layer. Its linework stays, on no layer.")
                        .clicked()
                    {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                table.layers.remove(i);
                changed = true;
            }
            if ui
                .button("Add layer")
                .on_hover_text(
                    "A new drafting layer; rename it, then put linework on it with Layer above.",
                )
                .clicked()
            {
                let name = (1..)
                    .map(|n| format!("Layer {n}"))
                    .find(|n| table.get(n).is_none())
                    .expect("unbounded");
                table.layers.push(DraftLayer {
                    name,
                    color: None,
                    linetype: LineType::Continuous,
                    hidden: false,
                });
                changed = true;
            }
            if changed && layer_edit.is_none() {
                *layer_edit = Some(LayerEdit::Set(table));
            }
        }
        return label;
    }
    egui::ComboBox::from_id_salt("detail.kind")
        .icon(crate::props::combo_icon)
        .selected_text(d.kind.label())
        .show_ui(ui, |ui| {
            for k in &DetailKind::ALL[..DetailKind::COMPONENTS] {
                let k = *k;
                if ui.selectable_label(d.kind.same(k), k.label()).clicked() && !d.kind.same(k) {
                    d.kind = k;
                    if d.width <= 0.0 {
                        d.width = k.default_width();
                    }
                    label = "Detail kind";
                }
            }
        });
    if d.kind.has_width() {
        crate::props::flow(ui, |ui| {
            ui.label("Width");
            if ui
                .add(crate::units::drag(&mut d.width).range(0.0..=5.0))
                .changed()
            {
                label = "Detail width";
            }
        });
    }
    if matches!(d.kind, DetailKind::Membrane)
        && crate::props::switch(ui, &mut d.flip, "On the other side")
            .on_hover_text("A membrane lies just off the line it follows, on the side it was clicked. This puts it on the other side.")
            .changed()
    {
        label = "Membrane side";
    }
    match &mut d.kind {
        DetailKind::Repeat { module, joint } => {
            crate::props::flow(ui, |ui| {
                ui.label("Unit");
                if ui
                    .add(crate::units::drag(module).range(0.005..=5.0))
                    .changed()
                {
                    label = "Detail unit";
                }
                ui.label("Joint");
                if ui.add(crate::units::drag(joint).range(0.0..=0.5)).changed() {
                    label = "Detail joint";
                }
            });
        }
        DetailKind::Region { hatch } => {
            egui::ComboBox::from_id_salt("detail.hatch")
                .icon(crate::props::combo_icon)
                .selected_text(hatch.label())
                .show_ui(ui, |ui| {
                    for h in Hatch::ALL {
                        if ui.selectable_value(hatch, h, h.label()).changed() {
                            label = "Detail hatch";
                        }
                    }
                });
        }
        _ => {}
    }
    ui.label(
        egui::RichText::new(format!(
            "{} points, {} long. Detail components are drafting: they show in this view only, never in 3D, schedules or IFC.",
            d.points.len(),
            crate::units::len(d.length())
        ))
        .weak(),
    );
    label
}

fn name_of(model: &Model, id: ElementId) -> String {
    model
        .get(id)
        .map_or("(missing)".to_string(), |e| e.name.clone())
}

/// What a junction rule's winner is called, for `a` and `b`.
pub fn winner_label(model: &Model, rule: &JoinRule, w: JoinWinner) -> String {
    match w {
        JoinWinner::Auto => "Automatic (by layer priority)".into(),
        JoinWinner::First => format!("{} cuts through", name_of(model, rule.a)),
        JoinWinner::Second => format!("{} cuts through", name_of(model, rule.b)),
    }
}

/// A junction rule in Properties.
pub fn join_fields(ui: &mut egui::Ui, model: &Model, j: &mut JoinRule) -> &'static str {
    let mut label = "";
    ui.label(format!(
        "How {} and {} join where they meet in plans and sections.",
        name_of(model, j.a),
        name_of(model, j.b)
    ));
    if crate::props::switch(ui, &mut j.cleanup, "Clean up the junction")
        .on_hover_text("Off: both are drawn whole where they overlap, with their own outlines.")
        .changed()
    {
        label = "Junction cleanup";
    }
    ui.add_enabled_ui(j.cleanup, |ui| {
        let rule = *j;
        for w in JoinWinner::ALL {
            if ui
                .radio_value(&mut j.winner, w, winner_label(model, &rule, w))
                .changed()
            {
                label = "Junction winner";
            }
        }
    });
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planview::Tool;

    fn doc_with_section() -> (Document, ElementId) {
        let mut doc = Document::new();
        let v = Element::new(
            "Section A",
            ElementKind::View(View::section(DVec2::ZERO, DVec2::new(5.0, 0.0), 3.0, true)),
        );
        let id = v.id;
        doc.commit(Transaction::new("setup").insert(v)).unwrap();
        (doc, id)
    }

    #[test]
    fn a_callout_makes_a_detail_view_of_its_box() {
        let (mut doc, section) = doc_with_section();
        let mut state = PlanState::default();
        state.tool = Tool::Callout;
        let mut sel = Selection::default();
        let p = DVec2::new;
        assert_eq!(
            callout_click(&mut doc, section, p(2.0, 1.0), &mut state, &mut sel).unwrap(),
            None
        );
        let id = callout_click(&mut doc, section, p(1.0, 0.0), &mut state, &mut sel)
            .unwrap()
            .unwrap();
        let e = doc.model().get(id).unwrap();
        assert_eq!(e.name, "Detail 1");
        let ElementKind::View(v) = &e.kind else {
            panic!()
        };
        let c = v.callout.unwrap();
        assert_eq!((c.parent, c.lo, c.hi), (section, p(1.0, 0.0), p(2.0, 1.0)));
        assert_eq!(v.scale, 10.0);
        // undo takes it away again
        assert!(doc.undo());
        assert!(doc.model().get(id).is_none());
    }

    #[test]
    fn the_detail_tool_draws_a_path_and_finishes_on_the_last_point() {
        let (mut doc, section) = doc_with_section();
        let mut state = PlanState::default();
        state.details.pick(DetailKind::Membrane);
        let mut sel = Selection::default();
        let p = DVec2::new;
        for q in [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 0.5), p(1.0, 0.5)] {
            detail_click(&mut doc, section, q, q, &mut state, &mut sel).unwrap();
        }
        let items: Vec<&DetailItem> = doc
            .model()
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Detail(d) => Some(d),
                _ => None,
            })
            .collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].points.len(), 3);
        assert_eq!(items[0].view, section);
        assert!(state.details.path.is_empty());
    }

    #[test]
    fn linework_takes_the_clicks_its_shape_needs() {
        use creabim_core::{DraftShape as S, LinePen};
        let (mut doc, section) = doc_with_section();
        let mut state = PlanState::default();
        state.tool = Tool::Detail;
        let mut sel = Selection::default();
        let p = DVec2::new;
        let mut draw = |shape: S, clicks: &[DVec2]| -> DetailItem {
            state.details.pick(DetailKind::Draft {
                shape,
                pen: LinePen::Heavy,
            });
            for q in clicks {
                detail_click(&mut doc, section, *q, *q, &mut state, &mut sel).unwrap();
            }
            assert!(state.details.path.is_empty(), "{shape:?} is finished");
            let id = sel.all()[0];
            match &doc.model().get(id).unwrap().kind {
                ElementKind::Detail(d) => d.clone(),
                _ => panic!(),
            }
        };
        // a circle is done at its second click, an arc at its third
        let c = draw(S::Circle, &[p(1.0, 1.0), p(2.0, 1.0)]);
        assert_eq!(
            (c.points.len(), c.kind.as_draft().unwrap().0),
            (2, S::Circle)
        );
        let a = draw(S::Arc, &[p(0.0, 0.0), p(1.0, 1.0), p(2.0, 0.0)]);
        assert_eq!(a.points.len(), 3);
        // a second click on the same spot is not the end of a circle
        let c = draw(S::Circle, &[p(1.0, 1.0), p(1.0, 1.0), p(1.0, 3.0)]);
        assert!((c.points[1].y - 3.0).abs() < 1e-9);
        // a rectangle and a polygon are kept as closed outlines, in their pen
        let r = draw(S::Rectangle, &[p(0.0, 0.0), p(2.0, 1.0)]);
        assert_eq!(
            r.kind,
            DetailKind::Draft {
                shape: S::Closed,
                pen: LinePen::Heavy
            }
        );
        assert_eq!(r.points.len(), 4);
        let h = draw(S::Polygon { sides: 5 }, &[p(0.0, 0.0), p(1.0, 0.0)]);
        assert_eq!(h.points.len(), 5);
        // a line goes on until its last point is clicked again; a closed
        // line closes on its first
        let l = draw(
            S::Line,
            &[p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(1.0, 1.0)],
        );
        assert_eq!(l.points.len(), 3);
        let k = draw(
            S::Closed,
            &[p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 0.0)],
        );
        assert_eq!(k.points.len(), 3);
        assert!((k.length() - (2.0 + 2f64.sqrt())).abs() < 1e-9);
    }

    #[test]
    fn linework_is_trimmed_filleted_offset_and_arrayed_by_clicks() {
        use creabim_core::DraftShape as S;
        let (mut doc, section) = doc_with_section();
        let mut state = PlanState::default();
        state.tool = Tool::Detail;
        let mut sel = Selection::default();
        let p = DVec2::new;
        let lines = |doc: &Document| -> Vec<DetailItem> {
            linework(doc.model(), section)
                .into_iter()
                .map(|l| l.1)
                .collect()
        };
        // a line along the floor, and two uprights crossing it
        for pts in [
            [p(0.0, 0.0), p(4.0, 0.0)],
            [p(1.0, -0.5), p(1.0, 2.0)],
            [p(3.0, -0.5), p(3.0, 2.0)],
        ] {
            state.details.pick(DetailKind::draft(S::Line));
            for q in [pts[0], pts[1], pts[1]] {
                detail_click(&mut doc, section, q, q, &mut state, &mut sel).unwrap();
            }
        }
        assert_eq!(lines(&doc).len(), 3);
        // trim the floor line between the uprights: two pieces of it are left
        state.details.pick_edit(DraftEdit::Trim);
        let at = p(2.0, 0.0);
        detail_click(&mut doc, section, at, at, &mut state, &mut sel).unwrap();
        assert_eq!(lines(&doc).len(), 4);
        // a click on nothing changes nothing and says so
        let off = p(2.0, 1.0);
        detail_click(&mut doc, section, off, off, &mut state, &mut sel).unwrap();
        assert_eq!(lines(&doc).len(), 4);
        assert!(state.status.as_deref().unwrap().contains("Nothing drawn"));
        // undo puts the line back whole
        assert!(doc.undo());
        assert_eq!(lines(&doc).len(), 3);
        // fillet the floor line's start with the first upright's foot:
        // two clicks, and an arc joins them
        state.details.pick_edit(DraftEdit::Fillet);
        state.details.radius = 0.2;
        for at in [p(0.5, 0.0), p(1.0, 1.0)] {
            detail_click(&mut doc, section, at, at, &mut state, &mut sel).unwrap();
        }
        let now = lines(&doc);
        assert_eq!(now.len(), 4, "{:?}", state.status);
        assert!(now.iter().any(|d| d.kind.as_draft().unwrap().0 == S::Arc));
        // offset the second upright 0.5 to its right
        state.details.pick_edit(DraftEdit::Offset);
        state.details.offset = 0.5;
        for at in [p(3.0, 1.0), p(3.8, 1.0)] {
            detail_click(&mut doc, section, at, at, &mut state, &mut sel).unwrap();
        }
        assert!(lines(&doc)
            .iter()
            .any(|d| d.points.iter().all(|q| (q.x - 3.5).abs() < 1e-9)));
        // an array of it: 3 across by 2 up adds five
        let before = lines(&doc).len();
        state.details.pick_edit(DraftEdit::Array);
        (state.details.across, state.details.up) = (3, 2);
        let at = p(3.5, 1.0);
        detail_click(&mut doc, section, at, at, &mut state, &mut sel).unwrap();
        assert_eq!(lines(&doc).len(), before + 5);
    }

    #[test]
    fn linework_is_drawn_on_a_sheet_in_paper_millimetres() {
        use creabim_core::DraftShape as S;
        let mut doc = Document::from_model(creabim_core::starter::starter_house());
        let (sheet, _) = creabim_geom::sheet::sheets(doc.model())[0];
        let before = creabim_geom::annotate::sheet_linework(doc.model(), sheet).len();
        let mut state = PlanState::default();
        state.tool = Tool::Detail;
        let mut sel = Selection::default();
        let p = DVec2::new;
        // a rectangle 100 by 50 mm on the paper
        state.details.pick(DetailKind::draft(S::Rectangle));
        for q in [p(40.0, 40.0), p(140.0, 90.0)] {
            detail_click(&mut doc, sheet, q, q, &mut state, &mut sel).unwrap();
        }
        let drawn = creabim_geom::annotate::sheet_linework(doc.model(), sheet);
        assert_eq!(drawn.len(), before + 4);
        let ElementKind::Detail(d) = &doc.model().get(sel.all()[0]).unwrap().kind else {
            panic!()
        };
        assert_eq!(d.view, sheet);
        assert_eq!(bounds(doc.model(), d), (p(40.0, 40.0), p(140.0, 90.0)));
        // clicks 5 mm apart are points of their own, and a click on the
        // last one again ends the line: on paper the reach is millimetres
        state.details.pick(DetailKind::draft(S::Line));
        for q in [p(10.0, 10.0), p(15.0, 10.0), p(30.0, 10.0), p(30.5, 10.0)] {
            detail_click(&mut doc, sheet, q, q, &mut state, &mut sel).unwrap();
        }
        let ElementKind::Detail(l) = &doc.model().get(sel.all()[0]).unwrap().kind else {
            panic!()
        };
        assert_eq!(l.points.len(), 3);
        // and it is on the sheet's SVG and DXF
        let info = creabim_geom::sheet::SheetInfo::default();
        let dxf = creabim_geom::sheet::sheet_dxf(doc.model(), sheet, &info).unwrap();
        assert!(dxf.contains("140.0000"));
    }

    #[test]
    fn a_symbol_is_placed_by_two_clicks_at_its_own_size() {
        use creabim_core::DraftShape as S;
        let (mut doc, section) = doc_with_section();
        let mut state = PlanState::default();
        state.tool = Tool::Detail;
        let mut sel = Selection::default();
        let p = DVec2::new;
        state.details.pick(DetailKind::draft(S::Line));
        for q in [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 0.0)] {
            detail_click(&mut doc, section, q, q, &mut state, &mut sel).unwrap();
        }
        let (tx, _) = creabim_core::detail::make_symbol(doc.model(), &sel.all(), "Mark").unwrap();
        doc.commit(tx).unwrap();
        let def = creabim_core::detail::symbols(doc.model())[0].0;
        let placed = |doc: &Document, id: ElementId| match &doc.model().get(id).unwrap().kind {
            ElementKind::Detail(d) => d.clone(),
            _ => panic!(),
        };
        // a click, then a point far away to turn it towards: still one unit
        state.details.pick(DetailKind::Symbol { def });
        for q in [p(3.0, 1.0), p(3.0, 9.0)] {
            detail_click(&mut doc, section, q, q, &mut state, &mut sel).unwrap();
        }
        let d = placed(&doc, sel.all()[0]);
        assert_eq!(d.points, vec![p(3.0, 1.0), p(3.0, 2.0)]);
        assert_eq!(doc.model().get(sel.all()[0]).unwrap().name, "Mark");
        // the same spot twice places it level
        for q in [p(5.0, 1.0), p(5.0, 1.0)] {
            detail_click(&mut doc, section, q, q, &mut state, &mut sel).unwrap();
        }
        assert_eq!(
            placed(&doc, sel.all()[0]).points,
            vec![p(5.0, 1.0), p(6.0, 1.0)]
        );
    }

    #[test]
    fn a_membrane_lies_on_the_side_it_was_clicked_and_esc_keeps_it() {
        let (mut doc, section) = doc_with_section();
        let mut state = PlanState::default();
        state.tool = Tool::Detail;
        state.details.pick(DetailKind::Membrane);
        let mut sel = Selection::default();
        let p = DVec2::new;
        // clicked just below a level line drawn left to right: its right
        for q in [p(0.0, 0.0), p(2.0, 0.0)] {
            detail_click(
                &mut doc,
                section,
                q,
                q - p(0.0, 0.003),
                &mut state,
                &mut sel,
            )
            .unwrap();
        }
        assert!(state
            .status
            .as_deref()
            .unwrap()
            .contains("2 points. Enter to finish"));
        // Esc (or another tool) keeps it
        assert!(keep(&mut doc, &mut state, &mut sel));
        let d = doc
            .model()
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Detail(d) => Some(d.clone()),
                _ => None,
            })
            .unwrap();
        assert!(d.flip);
        assert_eq!(d.points.len(), 2);
        // one point is not enough to keep
        detail_click(
            &mut doc,
            section,
            p(0.0, 1.0),
            p(0.0, 1.0),
            &mut state,
            &mut sel,
        )
        .unwrap();
        assert!(!keep(&mut doc, &mut state, &mut sel));
    }
}
