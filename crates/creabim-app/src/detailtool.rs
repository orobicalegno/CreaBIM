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

/// What the status bar says while a path is drawn: how many points it has
/// and how to finish it.
pub fn path_hint(kind: DetailKind, n: usize) -> String {
    let points = if n == 1 {
        "1 point".to_string()
    } else {
        format!("{n} points")
    };
    let label = kind.label();
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
    view_of(model, view).map_or(100.0, |v| v.scale)
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
    let label = d.kind.label();
    // a rectangle or a polygon is kept as the closed outline it draws
    let e = Element::new(label, ElementKind::Detail(item.baked()));
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
            let look = creabim_core::styles::active_pack(model).membranes;
            for it in creabim_geom::detail::detail_items(ElementId::new(), &item, mm, look) {
                crate::planview::draw_item(painter, xf, &it, true, mm);
            }
        }
        _ => {}
    }
}

/// A component's settings in Properties; returns the edit's name when one
/// changed.
pub fn detail_fields(ui: &mut egui::Ui, d: &mut DetailItem) -> &'static str {
    let mut label = "";
    if let DetailKind::Draft { shape, pen } = &mut d.kind {
        // plain linework: its shape is what its points mean, its pen can change
        let name = shape.label();
        crate::props::Combo::new("Pen")
            .selected_text(pen.label())
            .show_ui(ui, |ui| {
                for p in creabim_core::LinePen::ALL {
                    if ui.selectable_value(pen, p, p.label()).changed() {
                        label = "Line pen";
                    }
                }
            });
        if let creabim_core::DraftShape::Line | creabim_core::DraftShape::Closed = shape {
            let mut closed = *shape == creabim_core::DraftShape::Closed;
            if d.points.len() >= 3
                && crate::props::switch(ui, &mut closed, "Closed")
                    .on_hover_text("Join its end back to its start.")
                    .changed()
            {
                *shape = if closed {
                    creabim_core::DraftShape::Closed
                } else {
                    creabim_core::DraftShape::Line
                };
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
