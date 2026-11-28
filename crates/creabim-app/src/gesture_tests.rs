//! Gesture tests (regression plan, G0 and G1): real mouse and keyboard
//! events sent to a view's pane, without a window, on a copy of the
//! starter house kept in memory.
//!
//! [`Pane`] is the bench: it opens a view of the house in a pane and sends
//! it frames of input (the pointer moving, the wheel, buttons, keys), as
//! the app gets them from the system. The tests read what changed: the
//! pane's pan and zoom, the selection, the model.
//!
//! What each test holds is a behaviour agreed as right, not merely the
//! one the code has today: see `docs/regressioni.md`.

use crate::planview::{self, PaneKind, PaneStyle, PlanState, Xf};
use creabim_core::{Document, ElementId, ElementKind};
use eframe::egui::{self, Event, Modifiers, MouseWheelUnit, Pos2, Rect, TouchPhase, Vec2};
use glam::DVec2;

/// One view of the starter house in a pane, and the input sent to it.
pub(crate) struct Pane {
    pub doc: Document,
    pub view: ElementId,
    pub kind: PaneKind,
    pub state: PlanState,
    pub sel: crate::selection::Selection,
    pub rect: Rect,
    ctx: egui::Context,
    time: f64,
}

impl Pane {
    /// View `name` of a fresh copy of the starter house, in a pane
    /// 1200 × 800 points, centred on `centre` at `scale` pixels a metre.
    pub fn open(name: &str, centre: DVec2, scale: f32) -> Self {
        crate::units::set(creabim_core::Units::metric());
        let doc = Document::from_model(creabim_core::starter::starter_house());
        let (view, kind) = doc
            .model()
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::View(v) if e.name == name => Some((e.id, planview::pane_kind(v))),
                _ => None,
            })
            .unwrap_or_else(|| panic!("the starter house has no view called {name:?}"));
        let mut state = PlanState::default();
        state.cameras.insert(view, (centre, scale));
        let mut pane = Self {
            doc,
            view,
            kind,
            state,
            sel: Default::default(),
            rect: Rect::from_min_size(Pos2::ZERO, egui::vec2(1200.0, 800.0)),
            ctx: egui::Context::default(),
            time: 0.0,
        };
        // a first frame, so the pane is laid out before any input
        pane.frame(vec![]);
        pane
    }

    /// One frame of input, the pane drawn and its input handled.
    pub fn frame(&mut self, events: Vec<Event>) {
        self.time += 1.0 / 60.0;
        let raw = egui::RawInput {
            screen_rect: Some(self.rect),
            events,
            time: Some(self.time),
            // a common GPU's largest texture (egui's default is 2048)
            max_texture_side: Some(8192),
            ..Default::default()
        };
        let drawing = creabim_geom::section::view_drawing_with(self.doc.model(), self.view, false)
            .expect("the view draws");
        let (rect, view, kind) = (self.rect, self.view, self.kind);
        let (doc, state, sel) = (&mut self.doc, &mut self.state, &mut self.sel);
        let mut out = self.ctx.run_ui(raw, |ui| {
            let style = PaneStyle {
                background: None,
                decorations: false,
                camera: None,
                scale: 50.0,
            };
            planview::show(ui, rect, doc, view, kind, &drawing, state, sel, style);
        });
        out.textures_delta.clear();
    }

    /// The pointer rests at `p` (two frames, as a real move settles).
    pub fn hover(&mut self, p: Pos2) {
        self.frame(vec![Event::PointerMoved(p)]);
        self.frame(vec![Event::PointerMoved(p)]);
    }

    /// One wheel event of `delta` in `unit` with the pointer at `p`.
    pub fn wheel(&mut self, p: Pos2, unit: MouseWheelUnit, delta: Vec2) {
        self.frame(vec![
            Event::PointerMoved(p),
            wheel_event(unit, delta, Modifiers::NONE),
        ]);
    }

    /// The pane's pan and zoom: centre in drawing units, pixels a metre.
    pub fn camera(&self) -> (DVec2, f32) {
        self.state.cameras[&self.view]
    }

    /// The drawing point under screen point `p`.
    pub fn world_at(&self, p: Pos2) -> DVec2 {
        let (center, scale) = self.camera();
        Xf {
            rect: self.rect,
            center,
            scale,
            rot: 0.0,
        }
        .to_world(p)
    }
}

pub(crate) fn wheel_event(unit: MouseWheelUnit, delta: Vec2, modifiers: Modifiers) -> Event {
    Event::MouseWheel {
        unit,
        delta,
        phase: TouchPhase::Move,
        modifiers,
    }
}

/// A mouse wheel's notch, as every system sends it: one line up (forward)
/// or down (back).
const NOTCH_FORWARD: Vec2 = Vec2::new(0.0, 1.0);
const NOTCH_BACK: Vec2 = Vec2::new(0.0, -1.0);

#[cfg(test)]
mod g0_bench {
    use super::*;

    #[test]
    fn the_bench_opens_a_view_and_a_click_on_a_wall_selects_it() {
        // a wall of the house's ground floor, and a point inside it: a
        // third along, off its axis (a grid line runs on the axis)
        let probe = Pane::open("Ground Floor plan", DVec2::ZERO, 150.0);
        let PaneKind::Plan { level } = probe.kind else {
            unreachable!()
        };
        let (name, mid) = probe
            .doc
            .model()
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Wall(w) if w.level == level => {
                    let along = (w.end - w.start).normalize();
                    let off = along.perp() * w.thickness * 0.35;
                    Some((e.name.clone(), w.start + (w.end - w.start) * 0.3 + off))
                }
                _ => None,
            })
            .expect("a wall on the plan's level");
        // each copy of the house has ids of its own: the wall of this one
        let mut pane = Pane::open("Ground Floor plan", mid, 150.0);
        let wall = pane
            .doc
            .model()
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.id)
            .unwrap();
        let (center, scale) = pane.camera();
        let xf = Xf {
            rect: pane.rect,
            center,
            scale,
            rot: 0.0,
        };
        let at = xf.to_screen(mid);
        pane.hover(at);
        let press = |pressed| Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        pane.frame(vec![press(true)]);
        pane.frame(vec![press(false)]);
        pane.frame(vec![]);
        let picked: Vec<String> = pane
            .sel
            .all()
            .into_iter()
            .filter_map(|id| pane.doc.model().get(id).map(|e| e.name.clone()))
            .collect();
        assert!(
            pane.sel.contains(wall),
            "the clicked wall is selected: {picked:?}"
        );
        // and a click far from everything selects nothing
        let empty = Pos2::new(5.0, 5.0);
        pane.hover(empty);
        let press = |pressed| Event::PointerButton {
            pos: empty,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        pane.frame(vec![press(true)]);
        pane.frame(vec![press(false)]);
        pane.frame(vec![]);
        assert!(!pane.sel.contains(wall), "a click on nothing clears it");
    }
}

/// G1: the wheel. Held here: a wheel notch zooms, forward in and back out,
/// about the point under the pointer, in every kind of 2D view; the same
/// amount however the notch comes (one event, or the many small ones of
/// smooth scrolling); only the pane under the pointer; Ctrl + wheel counted
/// once. Not held yet (to be agreed, see docs/regressioni.md): what a
/// scroll in points, a sideways scroll and a touchpad should do.
#[cfg(test)]
mod g1_wheel {
    use super::*;

    /// What `planview::wheel` makes of one frame's events: (pan, zoom).
    fn read(events: Vec<Event>) -> (Vec2, f32) {
        let ctx = egui::Context::default();
        let mut got = (Vec2::ZERO, 0.0);
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))),
            events,
            time: Some(1.0),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| got = planview::wheel(ui));
        out.textures_delta.clear();
        got
    }

    #[test]
    fn a_wheel_notch_zooms_forward_in_and_back_out_and_never_pans() {
        let (pan, zoom) = read(vec![wheel_event(
            MouseWheelUnit::Line,
            NOTCH_FORWARD,
            Modifiers::NONE,
        )]);
        assert_eq!(pan, Vec2::ZERO);
        assert!(zoom > 0.0, "forward zooms in: {zoom}");
        let (pan, back) = read(vec![wheel_event(
            MouseWheelUnit::Line,
            NOTCH_BACK,
            Modifiers::NONE,
        )]);
        assert_eq!(pan, Vec2::ZERO);
        assert!(
            (back + zoom).abs() < 1e-6,
            "back zooms out by as much: {back}"
        );
    }

    #[test]
    fn smooth_scrolling_zooms_as_much_as_a_whole_notch() {
        // Windows' smooth scrolling (and fine wheels) send a notch as many
        // fractions of a line
        let (_, whole) = read(vec![wheel_event(
            MouseWheelUnit::Line,
            NOTCH_FORWARD,
            Modifiers::NONE,
        )]);
        let tenths = (0..10)
            .map(|_| wheel_event(MouseWheelUnit::Line, NOTCH_FORWARD * 0.1, Modifiers::NONE))
            .collect();
        let (pan, split) = read(tenths);
        assert_eq!(pan, Vec2::ZERO, "a fine wheel never pans");
        assert!((split - whole).abs() < 1e-4, "{split} for {whole}");
    }

    #[test]
    fn ctrl_and_the_wheel_zoom_once_not_twice() {
        let ctx = egui::Context::default();
        let mut got = (0.0, 0.0);
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))),
            events: vec![
                Event::ModifiersChanged(Modifiers::CTRL),
                wheel_event(MouseWheelUnit::Line, NOTCH_FORWARD, Modifiers::CTRL),
            ],
            time: Some(1.0),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let pinch = ui.input(|i| i.zoom_delta());
            got = (planview::wheel(ui).1, (pinch - 1.0) * 500.0);
        });
        out.textures_delta.clear();
        let (zoom, from_egui) = got;
        assert!(zoom > 0.0, "Ctrl + wheel forward zooms in");
        assert!(
            (zoom - from_egui).abs() < 1e-4,
            "only egui's own zoom counts: {zoom} for {from_egui}"
        );
    }

    /// In view `name`, a notch forward then back over a point: zoomed in,
    /// then out to where it started, the point under the pointer still.
    fn zooms_about_the_pointer(name: &str, centre: DVec2) {
        let mut pane = Pane::open(name, centre, 40.0);
        let at = Pos2::new(800.0, 300.0);
        pane.hover(at);
        let (_, s0) = pane.camera();
        let under = pane.world_at(at);
        pane.wheel(at, MouseWheelUnit::Line, NOTCH_FORWARD);
        let (_, s1) = pane.camera();
        assert!(
            s1 > s0 * 1.05,
            "{name}: a notch forward zooms in ({s0} → {s1})"
        );
        assert!(
            pane.world_at(at).distance(under) < 1e-3,
            "{name}: the point under the pointer stays"
        );
        pane.wheel(at, MouseWheelUnit::Line, NOTCH_BACK);
        let (_, s2) = pane.camera();
        assert!(
            (s2 - s0).abs() < 1e-3 * s0,
            "{name}: back out to where it was ({s0} → {s2})"
        );
        assert!(
            pane.world_at(at).distance(under) < 1e-3,
            "{name}: and still under it"
        );
    }

    #[test]
    fn the_wheel_zooms_about_the_pointer_in_a_plan() {
        zooms_about_the_pointer("Ground Floor plan", DVec2::new(5.0, 5.0));
    }

    #[test]
    fn the_wheel_zooms_about_the_pointer_in_a_section() {
        zooms_about_the_pointer("Section A-A", DVec2::new(5.0, 3.0));
    }

    #[test]
    fn the_wheel_zooms_about_the_pointer_in_an_elevation() {
        zooms_about_the_pointer("South Elevation", DVec2::new(5.0, 3.0));
    }

    #[test]
    fn the_wheel_zooms_about_the_pointer_in_a_saved_3d_view() {
        // a 3D view in a 2D pane: the case once reported broken
        zooms_about_the_pointer("3D Street view", DVec2::ZERO);
    }

    #[test]
    fn a_notch_split_by_smooth_scrolling_zooms_a_pane_as_much_as_a_whole_one() {
        let at = Pos2::new(700.0, 350.0);
        let mut whole = Pane::open("Ground Floor plan", DVec2::new(5.0, 5.0), 40.0);
        whole.hover(at);
        whole.wheel(at, MouseWheelUnit::Line, NOTCH_FORWARD);
        let mut split = Pane::open("Ground Floor plan", DVec2::new(5.0, 5.0), 40.0);
        split.hover(at);
        for _ in 0..12 {
            split.wheel(at, MouseWheelUnit::Line, NOTCH_FORWARD / 12.0);
        }
        let (c1, s1) = whole.camera();
        let (c2, s2) = split.camera();
        assert!((s1 - s2).abs() < 1e-3 * s1, "{s1} and {s2}");
        assert!(c1.distance(c2) < 1e-4, "and the same place");
    }

    #[test]
    fn the_wheel_away_from_a_pane_leaves_it_as_it_is() {
        let mut pane = Pane::open("Ground Floor plan", DVec2::new(5.0, 5.0), 40.0);
        let before = pane.camera();
        // the pointer outside the pane
        let outside = Pos2::new(1500.0, 300.0);
        pane.frame(vec![
            Event::PointerMoved(outside),
            wheel_event(MouseWheelUnit::Line, NOTCH_FORWARD, Modifiers::NONE),
        ]);
        pane.frame(vec![]);
        assert_eq!(pane.camera(), before);
    }
}
