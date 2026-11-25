//! Gesture tests (regression plan, G0): real mouse and keyboard
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
use eframe::egui::{self, Event, Modifiers, Pos2, Rect};
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

    /// The pane's pan and zoom: centre in drawing units, pixels a metre.
    pub fn camera(&self) -> (DVec2, f32) {
        self.state.cameras[&self.view]
    }
}

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
