//! The command line's editing commands of its own: BREAK, JOIN,
//! LENGTHEN, STRETCH, MATCHPROP, DONUT, POINT, DIVIDE, MEASURE, PLINE
//! with arcs, HATCH by a point inside, CIRCLE TTR, FROM and M2P, ZOOM.

use super::cmdline::{Finish, NumberFor, Running, Wants};
use super::*;
use creabim_core::draftedit;
use creabim_core::{DetailItem, DetailKind, DraftShape, Hatch, Transaction};
use glam::DVec2;

impl CreaBimApp {
    /// How near, in drawing units, a pick must be to a line: 3 mm on paper.
    fn pick_tol(&self, view: ElementId) -> f64 {
        self.view(view).map_or(100.0, |v| v.scale) * 0.003
    }

    /// The plain linework of `view` nearest `p`, within the pick distance.
    fn linework_at(&self, view: ElementId, p: DVec2) -> Option<(Element, DetailItem)> {
        let tol = self.pick_tol(view);
        self.doc
            .model()
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Detail(d) if d.view == view && d.kind.as_draft().is_some() => {
                    let h = draftedit::hit(d, p)?;
                    (h.distance <= tol).then(|| (h.distance, e.clone(), d.clone()))
                }
                _ => None,
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, e, d)| (e, d))
    }

    /// Put `new` in place of `old` (none: remove it), as one undo step.
    fn replace_linework(&mut self, label: &str, old: &Element, new: Vec<DetailItem>) -> bool {
        let mut tx = Transaction::new(label);
        let mut first = true;
        for d in new {
            if first {
                let mut e = old.clone();
                e.kind = ElementKind::Detail(d);
                tx = tx.replace(old.clone(), e);
                first = false;
            } else {
                tx = tx.insert(Element::new(old.name.clone(), ElementKind::Detail(d)));
            }
        }
        if first {
            tx = tx.remove_cascade(self.doc.model(), old.id);
        }
        match self.doc.commit(tx) {
            Ok(_) => true,
            Err(e) => {
                self.say(format!("Couldn't do that: {e}"));
                false
            }
        }
    }

    /// New pieces of linework, as one undo step.
    fn add_linework(&mut self, label: &str, items: Vec<DetailItem>) {
        if items.is_empty() {
            return;
        }
        let mut tx = Transaction::new(label);
        for d in items {
            let name = d.kind.label().to_string();
            tx = tx.insert(Element::new(name, ElementKind::Detail(d)));
        }
        if let Err(e) = self.doc.commit(tx) {
            self.say(format!("Couldn't do that: {e}"));
        }
    }

    /// A new piece in the command line's view, drawn as the Detail tool
    /// would draw it (its pen, layer and line type).
    fn new_piece(&self, view: ElementId, shape: DraftShape, points: Vec<DVec2>) -> DetailItem {
        let pen = self
            .plan
            .details
            .kind
            .as_draft()
            .map_or(creabim_core::LinePen::Medium, |d| d.1);
        let mut d = DetailItem::new(view, DetailKind::Draft { shape, pen }, points);
        d.layer = self.plan.details.layer.clone();
        d.linetype = self.plan.details.linetype;
        d
    }

    /// A point, as AutoCAD's (PDMODE 34): a small ring, its centre snaps.
    fn point_piece(&self, view: ElementId, p: DVec2) -> DetailItem {
        let r = self.view(view).map_or(100.0, |v| v.scale) * 0.0006;
        self.new_piece(view, DraftShape::Circle, vec![p, p + DVec2::X * r])
    }

    /// Start one of the commands here; false when it is not one of them.
    pub(super) fn cmd_more_start(&mut self, ctx: &egui::Context, name: &'static str) -> bool {
        self.cmd.values.clear();
        self.cmd.picks.clear();
        self.cmd.source = None;
        match name {
            "BREAK" => self.cmd_pick_start(
                name,
                "Select object at the first break point, then second break point:",
                2,
                Finish::Break,
            ),
            "BREAKATPOINT" => self.cmd_pick_start(
                name,
                "Select object at the break point:",
                1,
                Finish::BreakAt,
            ),
            "JOIN" => {
                if self.selection.len() >= 2 {
                    self.cmd_selected("JOIN");
                } else {
                    self.pick_tool(ctx, crate::planview::Tool::Select);
                    self.ask(
                        name,
                        "Select lines to join, then Enter:",
                        Wants::SelectThen("JOIN"),
                    );
                }
            }
            "LENGTHEN" => self.ask(
                name,
                "Enter delta length (negative shortens):",
                Wants::Number(NumberFor::LengthenDelta),
            ),
            "STRETCH" => self.cmd_pick_start(
                name,
                "Specify crossing window corners, then base point and second point:",
                4,
                Finish::Stretch,
            ),
            "MATCHPROP" => self.cmd_pick_start(
                name,
                "Select source object, then destination objects; Enter ends:",
                0,
                Finish::MatchProp,
            ),
            "DONUT" => self.ask(
                name,
                "Specify inside diameter of donut:",
                Wants::Number(NumberFor::DonutInner),
            ),
            "POINT" => self.cmd_pick_start(name, "Specify points; Enter ends:", 0, Finish::Point),
            "DIVIDE" => {
                self.cmd_pick_start(name, "Select object to divide:", 1, Finish::DividePick)
            }
            "MEASURE" => {
                self.cmd_pick_start(name, "Select object to measure:", 1, Finish::MeasurePick)
            }
            "PLINE" => {
                self.cmd.pline.clear();
                self.cmd.arc_mode = false;
                self.cmd_pick_start(
                    name,
                    "Specify start point, then next points [Arc/Line/Close/Undo]; Enter ends:",
                    0,
                    Finish::Pline,
                );
            }
            "HATCH" => self.cmd_pick_start(
                name,
                "Pick internal point [Select corners]; Enter ends:",
                0,
                Finish::HatchPick,
            ),
            "UCS" => self.cmd_pick_start(
                name,
                "Specify origin of UCS or [World/Previous/OBject/Z/View/NAmed/Save/Restore/Delete/?] <World>:",
                0,
                Finish::Ucs,
            ),
            "UCSMAN" => self.shell.ucs_manager = true,
            "PLAN" => self.ask(name, "Enter an option [Current ucs/World/name of a saved UCS] <Current>:", Wants::Word(super::cmdline::WordFor::Plan)),
            "UCSICON" => self.ask(name, "Enter an option [ON/OFF/ORigin/Noorigin] <ON>:", Wants::Word(super::cmdline::WordFor::UcsIcon)),
            "ZOOM" => self.ask(
                name,
                "[All/Extents/Window/Previous] <Extents>:",
                Wants::Points {
                    n: 0,
                    then: Finish::ZoomWindow,
                },
            ),
            _ => return false,
        }
        true
    }

    /// Typed words for these commands: PLINE's options, HATCH's corners,
    /// ZOOM's; true when one was taken.
    pub(super) fn cmd_more_option(&mut self, ctx: &egui::Context, r: &Running, word: &str) -> bool {
        let w = word.to_uppercase();
        match (r.name, w.as_str()) {
            ("PLINE", "A" | "ARC" | "ARCO") => {
                self.cmd.arc_mode = true;
                self.say("PLINE: arc segments, each tangent to the last [Line].");
            }
            ("PLINE", "L" | "LINE" | "LINEA") => {
                self.cmd.arc_mode = false;
                self.say("PLINE: straight segments [Arc].");
            }
            ("PLINE", "U") => {
                // back to the last point clicked: an arc's chords go with it
                let clicked = self
                    .plan
                    .cmd_pick
                    .as_ref()
                    .map(|p| p.1.clone())
                    .unwrap_or_default();
                if clicked.len() > 1 {
                    let keep = clicked[..clicked.len() - 1].to_vec();
                    self.rebuild_pline(&keep);
                    if let Some(p) = self.plan.cmd_pick.as_mut() {
                        p.1 = keep;
                    }
                    self.cmd.seen = self.plan.cmd_pick.as_ref().map_or(0, |p| p.1.len());
                }
            }
            ("PLINE", "C" | "CL" | "CLOSE") => {
                if let Some(first) = self.cmd.pline.first().copied() {
                    self.cmd.pline.push(first);
                }
                self.cmd.running = None;
                self.plan.cmd_pick = None;
                self.finish_pline(true);
            }
            ("HATCH", "S" | "C" | "SELECT" | "CORNERS") => {
                self.plan.cmd_pick = None;
                self.cmd.running = None;
                self.pick_tool(ctx, crate::planview::Tool::Detail);
                if let Some(i) = DetailKind::ALL
                    .iter()
                    .position(|k| matches!(k, DetailKind::Region { .. }))
                {
                    self.run_action(ctx, &format!("detail.kind:{i}"));
                }
                self.ask(
                    "HATCH",
                    "Specify the corners of the area to hatch; Enter ends it:",
                    Wants::ToolPoints,
                );
            }
            ("UCS", "W" | "WORLD" | "GLOBALE" | "G") => {
                self.end_pick();
                self.set_ucs(crate::planview::Ucs::default(), "UCS: World.");
            }
            ("UCS", "P" | "PREVIOUS" | "PRECEDENTE") => {
                self.end_pick();
                match self.plan.ucs_prev.pop() {
                    Some(u) => {
                        self.plan.ucs = u;
                        self.say("UCS: the previous one.");
                    }
                    None => self.say("UCS: no previous UCS."),
                }
            }
            ("UCS", "OB" | "OBJECT" | "E" | "ENTITY" | "OGGETTO") => {
                self.cmd_pick_start("UCS", "Select object to align UCS:", 1, Finish::UcsObject);
            }
            ("UCS", "Z") => {
                self.end_pick();
                self.ask(
                    "UCS",
                    "Specify rotation angle about Z axis <90>:",
                    Wants::Number(NumberFor::UcsZ),
                );
            }
            ("UCS", "V" | "VIEW" | "VISTA") => {
                self.end_pick();
                let rot = self
                    .cmd_view()
                    .and_then(|v| self.plan.view_rot.get(&v).copied())
                    .unwrap_or(0.0);
                let mut u = self.plan.ucs;
                u.angle = rot;
                self.set_ucs(u, "UCS: square to the view.");
            }
            ("UCS", "S" | "SAVE" | "SALVA") => {
                self.end_pick();
                self.ask(
                    "UCS",
                    "Enter name to save current UCS:",
                    Wants::Word(super::cmdline::WordFor::UcsSave),
                );
            }
            ("UCS", "R" | "RESTORE" | "RIPRISTINA" | "NA" | "NAMED") => {
                self.end_pick();
                let names = self.ucs_names();
                self.ask(
                    "UCS",
                    format!(
                        "Enter name of UCS to restore ({}):",
                        if names.is_empty() {
                            "none saved".into()
                        } else {
                            names.join(", ")
                        }
                    ),
                    Wants::Word(super::cmdline::WordFor::UcsRestore),
                );
            }
            ("UCS", "D" | "DELETE" | "ELIMINA") => {
                self.end_pick();
                self.ask(
                    "UCS",
                    "Enter name of UCS to delete:",
                    Wants::Word(super::cmdline::WordFor::UcsDelete),
                );
            }
            ("UCS", "?") => {
                let names = self.ucs_names();
                self.say(if names.is_empty() {
                    "No UCS saved. UCS S saves the current one.".to_string()
                } else {
                    format!("Saved UCS: {}", names.join(", "))
                });
            }
            ("ZOOM", "E" | "EXTENTS" | "ESTENSIONI") => {
                self.cmd.running = None;
                self.plan.cmd_pick = None;
                self.zoom_extents();
            }
            ("ZOOM", "A" | "ALL" | "TUTTO") => {
                self.end_pick();
                self.zoom_all();
            }
            ("ZOOM", "W" | "WINDOW" | "FINESTRA") => {
                self.cmd_pick_start(
                    "ZOOM",
                    "Specify first corner, then opposite corner:",
                    2,
                    Finish::ZoomWindow,
                );
            }
            ("ZOOM", "P" | "PREVIOUS" | "PRECEDENTE") => {
                self.cmd.running = None;
                self.plan.cmd_pick = None;
                match self.cmd.zoomed.pop() {
                    Some((v, cam)) => {
                        self.set_camera(v, cam);
                        self.say("ZOOM Previous.");
                    }
                    None => self.say("No previous view."),
                }
            }
            _ => return false,
        }
        true
    }

    /// Show `cam` in every area that shows `v`.
    fn set_camera(&mut self, v: ElementId, cam: (DVec2, f32)) {
        self.plan.cameras.insert(v, cam);
        for (k, c) in self.plan.area_cameras.iter_mut() {
            if k.1 == v {
                *c = cam;
            }
        }
    }

    /// ZOOM All: everything in the view and the origin, with room round it.
    fn zoom_all(&mut self) {
        let Some(v) = self.cmd_view() else { return };
        let Some(d) = self.drawing_live(v) else {
            return;
        };
        let mut lo = DVec2::ZERO;
        let mut hi = DVec2::ZERO;
        for it in &d.items {
            if let Some((a, b)) = it.bounds() {
                lo = lo.min(a);
                hi = hi.max(b);
            }
        }
        self.remember_zoom(v);
        let size = ((hi - lo) * 1.2).max(DVec2::splat(1.0));
        let pane = self.rect_2d.map_or(egui::vec2(800.0, 600.0), |r| r.size());
        let scale = (pane.x as f64 / size.x).min(pane.y as f64 / size.y);
        self.set_camera(v, ((lo + hi) / 2.0, scale as f32));
        self.say("ZOOM All.");
    }

    fn remember_zoom(&mut self, v: ElementId) {
        if let Some(c) = self.plan.cameras.get(&v) {
            self.cmd.zoomed.push((v, *c));
            if self.cmd.zoomed.len() > 20 {
                self.cmd.zoomed.remove(0);
            }
        }
    }

    fn zoom_extents(&mut self) {
        let Some(v) = self.cmd_view() else { return };
        self.remember_zoom(v);
        // the view fits its drawing again when its camera is gone: in
        // every area that shows it (a docked area keeps its own camera)
        self.plan.cameras.remove(&v);
        self.plan.area_cameras.retain(|(_, view), _| *view != v);
        self.say("ZOOM Extents.");
    }

    /// A number asked for by these commands; true when it was one.
    pub(super) fn cmd_more_number(
        &mut self,
        name: &'static str,
        what: NumberFor,
        v: Option<f64>,
    ) -> bool {
        match what {
            NumberFor::LengthenDelta => {
                let d = v.unwrap_or(self.cmd.values.first().copied().unwrap_or(0.1));
                self.cmd.values = vec![d];
                self.cmd_pick_start(
                    name,
                    "Select an object near the end to change; Enter ends:",
                    0,
                    Finish::Lengthen,
                );
            }
            NumberFor::DonutInner => {
                self.cmd.values = vec![v.unwrap_or(0.5).max(0.0)];
                self.ask(
                    name,
                    "Specify outside diameter of donut:",
                    Wants::Number(NumberFor::DonutOuter),
                );
            }
            NumberFor::DonutOuter => {
                let inner = self.cmd.values.first().copied().unwrap_or(0.5);
                self.cmd.values = vec![inner, v.unwrap_or(1.0).max(inner + 1e-6)];
                self.cmd_pick_start(
                    name,
                    "Specify center of donut; Enter ends:",
                    0,
                    Finish::Donut,
                );
            }
            NumberFor::DivideCount | NumberFor::MeasureLength => {
                self.cmd.running = None;
                let (Some(src), Some(v)) = (self.cmd.source, v) else {
                    self.say("Nothing to divide.");
                    return true;
                };
                let Some(ElementKind::Detail(d)) =
                    self.doc.model().get(src).map(|e| e.kind.clone())
                else {
                    return true;
                };
                let pts = if what == NumberFor::DivideCount {
                    draftedit::divide(&d, v.round().max(2.0) as usize)
                } else {
                    draftedit::measure(&d, v)
                };
                let n = pts.len();
                let items: Vec<DetailItem> = pts
                    .into_iter()
                    .map(|p| self.point_piece(d.view, p))
                    .collect();
                self.add_linework(
                    if what == NumberFor::DivideCount {
                        "Divide"
                    } else {
                        "Measure"
                    },
                    items,
                );
                self.say(format!("{name}: {n} points placed (Center snaps to them)."));
            }
            NumberFor::UcsZ => {
                self.cmd.running = None;
                let a = v.unwrap_or(90.0);
                let mut u = self.plan.ucs;
                u.angle += a.to_radians();
                self.set_ucs(u, &format!("UCS turned {a}° about Z."));
            }
            NumberFor::TtrRadius => {
                self.cmd.running = None;
                let r = v.unwrap_or(0.5);
                self.circle_ttr(r);
            }
            _ => return false,
        }
        true
    }

    fn circle_ttr(&mut self, r: f64) {
        let Some(view) = self.cmd_view() else { return };
        let [pa, pb] = [
            self.cmd.picks.first().copied(),
            self.cmd.picks.get(1).copied(),
        ];
        let (Some(pa), Some(pb)) = (pa, pb) else {
            return;
        };
        // the stretch of linework under each pick
        let seg = |app: &Self, p: DVec2| -> Option<(DVec2, DVec2)> {
            let (_, d) = app.linework_at(view, p)?;
            let h = draftedit::hit(&d, p)?;
            let (path, closed) = d.draft_path()?;
            let mut path = path;
            if closed {
                path.push(path[0]);
            }
            Some((path[h.segment], path[h.segment + 1]))
        };
        match (seg(self, pa), seg(self, pb)) {
            (Some((a1, a2)), Some((b1, b2))) => {
                match draftedit::circle_ttr(a1, a2, pa, b1, b2, pb, r) {
                    Some(c) => self.cmd_add_circle(c, r),
                    None => self.say("The two lines are parallel: no circle touches both there."),
                }
            }
            _ => self.say("Pick two straight lines of linework."),
        }
    }

    /// EXPLODE on the selection: a placed symbol back into its linework; a
    /// polyline, rectangle or polygon into its lines; a DXF reference into
    /// linework, its layers becoming drafting layers.
    pub(super) fn explode_selection(&mut self) {
        let model = self.doc.model();
        let mut tx = Transaction::new("Explode");
        let (mut parts, mut layers_new) = (0usize, Vec::<String>::new());
        let mut table = creabim_core::detail::layers(model);
        let mut touched = false;
        for id in self.selection.all() {
            let Some(e) = model.get(id) else { continue };
            match &e.kind {
                ElementKind::Detail(d) if matches!(d.kind, DetailKind::Symbol { .. }) => {
                    if let Some(t) = creabim_core::detail::explode(model, id) {
                        parts += t.ops.len().saturating_sub(1);
                        tx.ops.extend(t.ops);
                        touched = true;
                    }
                }
                ElementKind::Detail(d) => {
                    let Some((mut path, closed)) = d.draft_path() else {
                        continue;
                    };
                    let shape = d.kind.as_draft().map(|k| k.0);
                    // circles, ellipses, arcs and splines stay whole
                    if !matches!(
                        shape,
                        Some(
                            DraftShape::Line
                                | DraftShape::Closed
                                | DraftShape::Rectangle
                                | DraftShape::Polygon { .. }
                        )
                    ) {
                        continue;
                    }
                    if closed && !path.is_empty() {
                        path.push(path[0]);
                    }
                    if path.len() < 3 {
                        continue;
                    }
                    for w in path.windows(2) {
                        let mut piece = d.clone();
                        piece.kind = DetailKind::Draft {
                            shape: DraftShape::Line,
                            pen: d
                                .kind
                                .as_draft()
                                .map_or(creabim_core::LinePen::Medium, |k| k.1),
                        };
                        piece.points = vec![w[0], w[1]];
                        piece.fill = None;
                        tx = tx.insert(Element::new("Line", ElementKind::Detail(piece)));
                        parts += 1;
                    }
                    tx = tx.remove_cascade(model, id);
                    touched = true;
                }
                ElementKind::Reference(r) => {
                    let creabim_core::RefSource::Dxf { path } = &r.source else {
                        continue;
                    };
                    let Some(view) = self
                        .cmd_view()
                        .filter(|v| r.shown_in.contains(v))
                        .or_else(|| r.shown_in.first().copied())
                    else {
                        continue;
                    };
                    let items = match std::fs::read(path)
                        .map_err(|e| e.to_string())
                        .and_then(|b| {
                            let text = creabim_exchange::titleblock::dxf_text(&b);
                            creabim_exchange::titleblock::dxf_items_layered(&text)
                        }) {
                        Ok(items) => items,
                        Err(err) => {
                            self.say(format!("EXPLODE: couldn't read {path}: {err}"));
                            return;
                        }
                    };
                    for (it, layer) in items {
                        let creabim_core::titleblock::TitleItem::Line { points, .. } = it else {
                            continue;
                        };
                        if points.len() < 2 {
                            continue;
                        }
                        // the reference's own place, turn and size; mm to m
                        let pts: Vec<DVec2> = points.iter().map(|p| r.place(*p / 1000.0)).collect();
                        let closed = pts.len() > 3 && pts[0].distance(*pts.last().unwrap()) < 1e-9;
                        let (shape, pts) = if closed {
                            (DraftShape::Closed, pts[..pts.len() - 1].to_vec())
                        } else {
                            (DraftShape::Line, pts)
                        };
                        let mut d = self.new_piece(view, shape, pts);
                        let name = if layer.trim().is_empty() {
                            "0".to_string()
                        } else {
                            layer.trim().to_string()
                        };
                        if table.get(&name).is_none() && !layers_new.contains(&name) {
                            layers_new.push(name.clone());
                        }
                        d.layer = Some(name);
                        d.linetype = None;
                        tx = tx.insert(Element::new("Line", ElementKind::Detail(d)));
                        parts += 1;
                    }
                    tx = tx.remove_cascade(model, id);
                    touched = true;
                }
                _ => {}
            }
        }
        if !touched {
            self.say("EXPLODE: nothing that explodes is selected (symbols, polylines, rectangles, polygons, DXF references).");
            return;
        }
        if !layers_new.is_empty() {
            for n in &layers_new {
                table.layers.push(creabim_core::DraftLayer::named(n));
            }
            tx.ops
                .extend(creabim_core::detail::set_layers(self.doc.model(), table).ops);
        }
        match self.doc.commit(tx) {
            Ok(_) => {
                self.selection.clear();
                self.say(format!(
                    "EXPLODE: {parts} pieces{}. Ctrl+Z puts it back.",
                    if layers_new.is_empty() {
                        String::new()
                    } else {
                        format!(", {} drafting layers from the DXF", layers_new.len())
                    }
                ));
            }
            Err(e) => self.say(format!("Couldn't do that: {e}")),
        }
    }

    /// JOIN and the like, on the selection.
    pub(super) fn cmd_selected(&mut self, what: &'static str) {
        if what == "EXPLODE" {
            self.explode_selection();
            return;
        }
        if what != "JOIN" {
            return;
        }
        let ids = self.selection.all();
        let items: Vec<(Element, DetailItem)> = ids
            .iter()
            .filter_map(|id| {
                let e = self.doc.model().get(*id)?;
                match &e.kind {
                    ElementKind::Detail(d) if d.kind.as_draft().is_some() => {
                        Some((e.clone(), d.clone()))
                    }
                    _ => None,
                }
            })
            .collect();
        if items.len() < 2 {
            self.say("JOIN: select two or more lines of linework.");
            return;
        }
        let tol = self.pick_tol(items[0].1.view) * 0.2;
        let refs: Vec<&DetailItem> = items.iter().map(|i| &i.1).collect();
        let Some(joined) = draftedit::join_all(&refs, tol) else {
            self.say("JOIN: the lines don't meet end to end.");
            return;
        };
        let mut tx = Transaction::new("Join");
        let mut first = items[0].0.clone();
        first.kind = ElementKind::Detail(joined);
        tx = tx.replace(items[0].0.clone(), first);
        for (e, _) in &items[1..] {
            tx = tx.remove_cascade(self.doc.model(), e.id);
        }
        match self.doc.commit(tx) {
            Ok(_) => {
                let id = items[0].0.id;
                self.selection.only(id);
                self.say(format!("JOIN: {} lines joined into one.", items.len()));
            }
            Err(e) => self.say(format!("Couldn't do that: {e}")),
        }
    }

    /// Points that act as they come: from `from` on.
    pub(super) fn cmd_stream(&mut self, then: Finish, pts: &[DVec2], from: usize) {
        let Some(view) = self.cmd_view() else { return };
        for i in from..pts.len() {
            let p = pts[i];
            match then {
                Finish::Point => {
                    let d = self.point_piece(view, p);
                    self.add_linework("Point", vec![d]);
                }
                Finish::Donut => {
                    let (inner, outer) = (self.cmd.values[0] / 2.0, self.cmd.values[1] / 2.0);
                    let n = 48;
                    let ring = |r: f64| -> Vec<DVec2> {
                        (0..=n)
                            .map(|k| {
                                p + DVec2::from_angle(std::f64::consts::TAU * k as f64 / n as f64)
                                    * r
                            })
                            .collect()
                    };
                    // the outer circle out, the inner one back: one filled ring
                    let mut pts = ring(outer);
                    if inner > 1e-9 {
                        let mut back = ring(inner);
                        back.reverse();
                        pts.extend(back);
                    }
                    let mut d = self.new_piece(view, DraftShape::Closed, pts);
                    d.fill = Some(Hatch::Solid);
                    self.add_linework("Donut", vec![d]);
                }
                Finish::MatchProp => {
                    let Some((e, d)) = self.linework_at(view, p) else {
                        self.say("MATCHPROP: no linework there.");
                        continue;
                    };
                    match self.cmd.source {
                        None => {
                            self.cmd.source = Some(e.id);
                            self.say("MATCHPROP: now the objects to give its properties.");
                        }
                        Some(src) => {
                            let Some(ElementKind::Detail(s)) =
                                self.doc.model().get(src).map(|e| e.kind.clone())
                            else {
                                continue;
                            };
                            let mut to = d.clone();
                            if let (DetailKind::Draft { pen, .. }, Some((_, sp))) =
                                (&mut to.kind, s.kind.as_draft())
                            {
                                *pen = sp;
                            }
                            to.layer = s.layer.clone();
                            to.linetype = s.linetype;
                            to.color = s.color;
                            if to.fillable() {
                                to.fill = s.fill;
                            }
                            self.replace_linework("Match properties", &e, vec![to]);
                        }
                    }
                }
                Finish::Lengthen => {
                    let Some((e, d)) = self.linework_at(view, p) else {
                        self.say("LENGTHEN: no linework there.");
                        continue;
                    };
                    let delta = self.cmd.values.first().copied().unwrap_or(0.0);
                    match draftedit::lengthen(&d, p, delta) {
                        Some(n) => {
                            self.replace_linework("Lengthen", &e, vec![n]);
                        }
                        None => {
                            self.say("LENGTHEN: only open lines, and not shorter than nothing.")
                        }
                    }
                }
                Finish::HatchPick => self.hatch_at(view, p),
                Finish::Ucs => self.ucs_point(i, p),
                Finish::Pline => {
                    let clicked = pts[..=i].to_vec();
                    self.rebuild_pline(&clicked);
                }
                _ => {}
            }
        }
        // a polyline shows its arcs as it goes
        if then == Finish::Pline {
            if let Some((v, _)) = self.plan.cmd_pick.clone() {
                self.plan.cmd_preview = Some((v.unwrap_or(view), self.cmd.pline.clone()));
            }
        }
    }

    /// The polyline through the points clicked, arcs where the arc mode
    /// was on (kept per point in `pline_arcs`).
    fn rebuild_pline(&mut self, clicked: &[DVec2]) {
        // arcs are kept as which clicked points were reached by an arc
        while self.plan.pline_arcs.len() < clicked.len() {
            self.plan.pline_arcs.push(self.cmd.arc_mode);
        }
        self.plan.pline_arcs.truncate(clicked.len());
        let mut out: Vec<DVec2> = vec![];
        for (k, p) in clicked.iter().enumerate() {
            if k == 0 {
                out.push(*p);
                continue;
            }
            let last = *out.last().expect("a start");
            if self.plan.pline_arcs[k] {
                let dir = if out.len() >= 2 {
                    last - out[out.len() - 2]
                } else {
                    DVec2::X
                };
                let arc = draftedit::tangent_arc(last, dir, *p);
                out.extend(arc.into_iter().skip(1));
            } else {
                out.push(*p);
            }
        }
        self.cmd.pline = out;
    }

    fn finish_pline(&mut self, closed: bool) {
        self.plan.cmd_preview = None;
        self.plan.pline_arcs.clear();
        let mut pts = std::mem::take(&mut self.cmd.pline);
        let Some(view) = self.cmd_view() else { return };
        if pts.len() < 2 {
            self.say("PLINE: it needs two points.");
            return;
        }
        let shape = if closed && pts.len() > 3 {
            pts.pop();
            DraftShape::Closed
        } else {
            DraftShape::Line
        };
        let d = self.new_piece(view, shape, pts);
        self.add_linework("Polyline", vec![d]);
        self.say("PLINE done. Ctrl+Z to undo.");
    }

    /// HATCH by a point inside: the smallest closed linework, or room,
    /// around `p`, filled with a region of the Detail tool's hatch.
    fn hatch_at(&mut self, view: ElementId, p: DVec2) {
        let inside = |poly: &[DVec2]| {
            let mut c = false;
            let n = poly.len();
            for i in 0..n {
                let (a, b) = (poly[i], poly[(i + 1) % n]);
                if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y)
                {
                    c = !c;
                }
            }
            c
        };
        let area = |poly: &[DVec2]| super::cmdline::area_of(poly).0;
        let mut best: Option<(f64, Vec<DVec2>)> = None;
        let mut consider = |poly: Vec<DVec2>| {
            if poly.len() >= 3 && inside(&poly) {
                let a = area(&poly);
                if best.as_ref().is_none_or(|b| a < b.0) {
                    best = Some((a, poly));
                }
            }
        };
        for e in self.doc.model().iter() {
            if let ElementKind::Detail(d) = &e.kind {
                if d.view == view {
                    if let Some((path, true)) = d.draft_path() {
                        consider(path);
                    }
                }
            }
        }
        // rooms in a plan: their outline as the boundary
        if let Some(drawing) = self.drawing_live(view) {
            for it in &drawing.items {
                if let creabim_geom::PlanItem::Room { points, .. } = it {
                    consider(points.clone());
                }
            }
        }
        let Some((_, outline)) = best else {
            self.say("HATCH: no closed boundary round that point (closed linework or a room). Type S to draw its corners.");
            return;
        };
        let hatch = match self.plan.details.kind {
            DetailKind::Region { hatch } => hatch,
            _ => Hatch::Diagonal,
        };
        let mut d = DetailItem::new(view, DetailKind::Region { hatch }, outline);
        d.layer = self.plan.details.layer.clone();
        self.add_linework("Hatch", vec![d]);
        self.say("HATCH: filled. Pick another point, or Enter.");
    }

    /// The points of a command here are in.
    pub(super) fn cmd_more_finish(&mut self, then: Finish, pts: &[DVec2]) -> bool {
        let Some(view) = self.cmd_view() else {
            return false;
        };
        match (then, pts) {
            (Finish::Break, [a, b, ..]) => match self.linework_at(view, *a) {
                Some((e, d)) => match draftedit::break_between(&d, *a, *b) {
                    Some(parts) => {
                        if self.replace_linework("Break", &e, parts) {
                            self.say("BREAK done.");
                        }
                    }
                    None => self.say("BREAK: couldn't break it there."),
                },
                None => self.say("BREAK: pick linework at the first break point."),
            },
            (Finish::BreakAt, [a, ..]) => match self.linework_at(view, *a) {
                Some((e, d)) => match draftedit::break_at(&d, *a) {
                    Some(parts) => {
                        if self.replace_linework("Break at point", &e, parts) {
                            self.say("BREAKATPOINT done: two pieces.");
                        }
                    }
                    None => self.say("BREAKATPOINT: not at an end."),
                },
                None => self.say("BREAKATPOINT: pick linework."),
            },
            (Finish::Stretch, [c1, c2, base, to, ..]) => {
                let (lo, hi) = (c1.min(*c2), c1.max(*c2));
                let by = *to - *base;
                let moved: Vec<(Element, DetailItem)> = self
                    .doc
                    .model()
                    .iter()
                    .filter_map(|e| match &e.kind {
                        ElementKind::Detail(d) if d.view == view && d.kind.as_draft().is_some() => {
                            draftedit::stretch_in(d, lo, hi, by).map(|n| (e.clone(), n))
                        }
                        _ => None,
                    })
                    .collect();
                if moved.is_empty() {
                    self.say("STRETCH: no linework in the window.");
                } else {
                    let mut tx = Transaction::new("Stretch");
                    for (e, d) in &moved {
                        let mut after = e.clone();
                        after.kind = ElementKind::Detail(d.clone());
                        tx = tx.replace(e.clone(), after);
                    }
                    match self.doc.commit(tx) {
                        Ok(_) => self.say(format!("STRETCH: {} pieces stretched.", moved.len())),
                        Err(e) => self.say(format!("Couldn't do that: {e}")),
                    }
                }
            }
            (Finish::DividePick | Finish::MeasurePick, [a, ..]) => match self.linework_at(view, *a)
            {
                Some((e, _)) => {
                    self.cmd.source = Some(e.id);
                    if then == Finish::DividePick {
                        self.ask(
                            "DIVIDE",
                            "Enter the number of segments:",
                            Wants::Number(NumberFor::DivideCount),
                        );
                    } else {
                        self.ask(
                            "MEASURE",
                            "Specify length of segment:",
                            Wants::Number(NumberFor::MeasureLength),
                        );
                    }
                }
                None => self.say("Pick linework."),
            },
            (Finish::Ttr, [a, b, ..]) => {
                self.cmd.picks = vec![*a, *b];
                self.ask(
                    "CIRCLE",
                    "Specify radius of circle:",
                    Wants::Number(NumberFor::TtrRadius),
                );
            }
            (Finish::FromBase, [base, ..]) => {
                self.plan.last_point = Some((view, *base));
                if let Some(r) = self.cmd.resume.take() {
                    self.ask(
                        r.name,
                        "<Offset>: (@dx,dy or @dist<angle from the base point)",
                        r.wants,
                    );
                }
            }
            (Finish::Mid2, [a, b, ..]) => {
                let m = (*a + *b) / 2.0;
                if let Some(r) = self.cmd.resume.take() {
                    self.cmd.running = Some(r);
                }
                self.plan.forced_point = Some((view, m));
            }
            (Finish::ZoomWindow, [a, b, ..]) => {
                self.remember_zoom(view);
                let (lo, hi) = (a.min(*b), a.max(*b));
                let size = (hi - lo).max(DVec2::splat(1e-3));
                let pane = self.rect_2d.map_or(egui::vec2(800.0, 600.0), |r| r.size());
                let scale = (pane.x as f64 / size.x).min(pane.y as f64 / size.y) * 0.95;
                self.set_camera(view, ((lo + hi) / 2.0, scale as f32));
                self.say("ZOOM Window.");
            }
            (Finish::ZoomWindow, []) => self.zoom_extents(),
            (Finish::Pline, _) => self.finish_pline(false),
            // UCS with no point: the world's (Enter); one point: its origin
            (Finish::Ucs, []) => self.set_ucs(crate::planview::Ucs::default(), "UCS: World."),
            (Finish::Ucs, _) => self.say("UCS set."),
            (Finish::UcsObject, [p, ..]) => self.ucs_object(view, *p),
            (f, _) if f.streams() => self.say("Done."),
            _ => return false,
        }
        true
    }

    fn end_pick(&mut self) {
        self.plan.cmd_pick = None;
        self.cmd.running = None;
    }

    /// Make `u` the UCS, the one before kept for UCS Previous.
    pub(super) fn set_ucs(&mut self, u: crate::planview::Ucs, msg: &str) {
        if u != self.plan.ucs {
            self.plan.ucs_prev.push(self.plan.ucs);
            if self.plan.ucs_prev.len() > 10 {
                self.plan.ucs_prev.remove(0);
            }
            self.plan.ucs = u;
        }
        let deg = u.angle.to_degrees().rem_euclid(360.0);
        self.say(format!(
            "{msg} Origin {}, {}; X axis at {:.4}°. PLAN turns the plan to it.",
            crate::units::len(u.origin.x),
            crate::units::len(u.origin.y),
            deg
        ));
    }

    /// UCS by points: the first its origin, the second along its X axis.
    fn ucs_point(&mut self, i: usize, p: DVec2) {
        let mut u = self.plan.ucs;
        if i == 0 {
            u.origin = p;
            self.set_ucs(u, "UCS origin.");
            self.ask(
                "UCS",
                "Specify point on X-axis or <accept>:",
                Wants::Points {
                    n: 0,
                    then: Finish::Ucs,
                },
            );
        } else {
            let d = p - u.origin;
            if d.length() > 1e-9 {
                u.angle = d.y.atan2(d.x);
            }
            self.end_pick();
            self.set_ucs(u, "UCS by its origin and X axis.");
        }
    }

    /// UCS OBject: its origin at the nearer end of the line picked, its X
    /// axis along it.
    fn ucs_object(&mut self, view: ElementId, p: DVec2) {
        let tol = self.pick_tol(view);
        let mut best: Option<(f64, DVec2, DVec2)> = None;
        let mut consider = |a: DVec2, b: DVec2| {
            let ab = b - a;
            let l2 = ab.length_squared();
            if l2 < 1e-12 {
                return;
            }
            let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
            let d = (a + ab * t).distance(p);
            if d <= tol && best.is_none_or(|b| d < b.0) {
                best = Some((d, a, b));
            }
        };
        for e in self.doc.model().iter() {
            if let ElementKind::Detail(d) = &e.kind {
                if d.view == view {
                    if let Some((mut path, closed)) = d.draft_path() {
                        if closed && !path.is_empty() {
                            path.push(path[0]);
                        }
                        for w in path.windows(2) {
                            consider(w[0], w[1]);
                        }
                    }
                }
            }
        }
        if let Some(drawing) = self.drawing_live(view) {
            for it in &drawing.items {
                match it {
                    creabim_geom::PlanItem::Line { a, b, .. } => consider(*a, *b),
                    creabim_geom::PlanItem::Polygon { points, .. } => {
                        for k in 0..points.len() {
                            consider(points[k], points[(k + 1) % points.len()]);
                        }
                    }
                    _ => {}
                }
            }
        }
        let Some((_, a, b)) = best else {
            self.say("UCS: no line there.");
            return;
        };
        let (o, x) = if p.distance(a) <= p.distance(b) {
            (a, b)
        } else {
            (b, a)
        };
        let d = x - o;
        self.set_ucs(
            crate::planview::Ucs {
                origin: o,
                angle: d.y.atan2(d.x),
            },
            "UCS on the object.",
        );
    }

    fn ucs_names(&self) -> Vec<String> {
        self.doc
            .model()
            .project_info()
            .map(|(_, p)| p.ucs.iter().map(|u| u.name.clone()).collect())
            .unwrap_or_default()
    }

    fn saved_ucs(&self, name: &str) -> Option<crate::planview::Ucs> {
        let (_, p) = self.doc.model().project_info()?;
        let u = p.ucs.iter().find(|u| u.name.eq_ignore_ascii_case(name))?;
        Some(crate::planview::Ucs {
            origin: u.origin,
            angle: u.angle.to_radians(),
        })
    }

    /// Keep the current UCS in the project by `name` (replacing one of
    /// that name), or take one out.
    pub(super) fn store_ucs(&mut self, name: &str, keep: bool) {
        let mut info = self
            .doc
            .model()
            .project_info()
            .map(|(_, p)| p.clone())
            .unwrap_or_default();
        info.ucs.retain(|u| !u.name.eq_ignore_ascii_case(name));
        if keep {
            info.ucs.push(creabim_core::NamedUcs {
                name: name.to_string(),
                origin: self.plan.ucs.origin,
                angle: self.plan.ucs.angle.to_degrees(),
            });
        }
        let tx = creabim_core::set_project(self.doc.model(), info);
        if let Err(e) = self.doc.commit(tx) {
            self.say(format!("Couldn't do that: {e}"));
        }
    }

    /// The word typed for a command: a UCS's name, PLAN's or UCSICON's
    /// option.
    pub(super) fn cmd_word(&mut self, what: super::cmdline::WordFor, t: &str) {
        use super::cmdline::WordFor as W;
        let up = t.trim().to_uppercase();
        match what {
            W::UcsSave => {
                if up.is_empty() {
                    self.say("UCS: a name is needed.");
                } else {
                    self.store_ucs(t.trim(), true);
                    self.say(format!("UCS saved as \"{}\".", t.trim()));
                }
            }
            W::UcsRestore => match self.saved_ucs(t.trim()) {
                Some(u) => self.set_ucs(u, &format!("UCS \"{}\".", t.trim())),
                None => self.say(format!("No UCS named \"{}\".", t.trim())),
            },
            W::UcsDelete => {
                if self.saved_ucs(t.trim()).is_some() {
                    self.store_ucs(t.trim(), false);
                    self.say(format!("UCS \"{}\" deleted.", t.trim()));
                } else {
                    self.say(format!("No UCS named \"{}\".", t.trim()));
                }
            }
            W::Plan => {
                let Some(v) = self.cmd_view().filter(|v| {
                    matches!(self.view(*v).map(|x| &x.kind), Some(ViewKind::Plan { .. }))
                }) else {
                    self.say("PLAN works in a plan view.");
                    return;
                };
                let angle = match up.as_str() {
                    "" | "C" | "CURRENT" | "CORRENTE" => Some(self.plan.ucs.angle),
                    "W" | "WORLD" | "GLOBALE" => Some(0.0),
                    _ => self.saved_ucs(t.trim()).map(|u| u.angle),
                };
                match angle {
                    Some(a) => {
                        if a.abs() < 1e-12 {
                            self.plan.view_rot.remove(&v);
                        } else {
                            self.plan.view_rot.insert(v, a);
                        }
                        self.say(format!(
                            "PLAN: the plan turned so the X axis at {:.4}° reads level. PLAN W turns it back.",
                            a.to_degrees().rem_euclid(360.0)
                        ));
                    }
                    None => self.say(format!("No UCS named \"{}\".", t.trim())),
                }
            }
            W::UcsIcon => match up.as_str() {
                "" | "ON" => {
                    self.plan.ucs_icon = true;
                    self.say("UCS icon on.");
                }
                "OFF" => {
                    self.plan.ucs_icon = false;
                    self.say("UCS icon off.");
                }
                "OR" | "ORIGIN" | "ORIGINE" => {
                    self.plan.ucs_icon_origin = true;
                    self.say("UCS icon at the UCS's origin, when it is in view.");
                }
                "N" | "NOORIGIN" | "NOORIGINE" => {
                    self.plan.ucs_icon_origin = false;
                    self.say("UCS icon in the corner.");
                }
                _ => self.say("UCSICON: ON, OFF, ORigin or Noorigin."),
            },
        }
    }
}
