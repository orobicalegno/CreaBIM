//! Undo, Redo and the Save/Cancel of windows (regression plan, G4): work
//! must never be lost by surprise.
//!
//! Every check compares the **whole project**: each element with all it
//! holds (geometry, materials, layers, properties, links to other
//! elements) and their order, not a count. [`same`] says which element
//! differs and how.
//!
//! The edits are real ones on a copy of the starter house in memory: a
//! wall moved (geometry), a layer of an assembly thickened (layers), a
//! material recoloured (materials), a wall's height and name changed
//! (properties). What a window would change is made inside
//! [`CreaBimApp::in_dialog`], as the window's own code makes it.

use super::*;
use creabim_core::Model;

/// The two models are the same in every element and its order; else a
/// panic naming what differs.
#[track_caller]
fn same(now: &Model, want: &Model, when: &str) {
    if now == want {
        return;
    }
    let mut out = Vec::new();
    for e in want.iter() {
        match now.get(e.id) {
            None => out.push(format!("missing: {} ({:?})", e.name, e.id)),
            Some(n) if n != e => {
                let a = format!("{:?}", e.kind);
                let b = format!("{:?}", n.kind);
                let at = a
                    .chars()
                    .zip(b.chars())
                    .position(|(x, y)| x != y)
                    .unwrap_or(a.len().min(b.len()));
                let from = at.saturating_sub(60);
                out.push(format!(
                    "changed: {} — expected …{}…, found …{}…",
                    e.name,
                    a.get(from..(at + 60).min(a.len())).unwrap_or(""),
                    b.get(from..(at + 60).min(b.len())).unwrap_or("")
                ));
            }
            _ => {}
        }
    }
    for e in now.iter() {
        if want.get(e.id).is_none() {
            out.push(format!("added: {} ({:?})", e.name, e.id));
        }
    }
    if out.is_empty() {
        out.push("the same elements, in another order".into());
    }
    panic!("{when}: the project differs:\n  {}", out.join("\n  "));
}

fn first(app: &CreaBimApp, pick: impl Fn(&ElementKind) -> bool) -> ElementId {
    app.doc
        .model()
        .iter()
        .find(|e| pick(&e.kind))
        .map(|e| e.id)
        .expect("the sample house has one")
}

/// Geometry: the first wall's start moves half a metre.
fn move_a_wall(app: &mut CreaBimApp) {
    let id = first(app, |k| matches!(k, ElementKind::Wall(_)));
    app.doc
        .edit(id, "Move wall end", |e| {
            if let ElementKind::Wall(w) = &mut e.kind {
                w.start.x += 0.5;
            }
        })
        .unwrap();
}

/// Layers: the first assembly's first layer one centimetre thicker.
fn thicken_a_layer(app: &mut CreaBimApp) {
    let id = first(
        app,
        |k| matches!(k, ElementKind::Assembly(a) if !a.layers.is_empty()),
    );
    app.doc
        .edit(id, "Layer thickness", |e| {
            if let ElementKind::Assembly(a) = &mut e.kind {
                a.layers[0].thickness += 0.01;
            }
        })
        .unwrap();
}

/// Materials: the first material another colour.
fn recolour_a_material(app: &mut CreaBimApp) {
    let id = first(app, |k| matches!(k, ElementKind::Material(_)));
    app.doc
        .edit(id, "Material colour", |e| {
            if let ElementKind::Material(m) = &mut e.kind {
                m.color = [0.11, 0.22, 0.33];
            }
        })
        .unwrap();
}

/// Properties: the last wall's height and name.
fn change_a_wall(app: &mut CreaBimApp) {
    let id = app
        .doc
        .model()
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
        .last()
        .map(|e| e.id)
        .unwrap();
    app.doc
        .edit(id, "Wall height", |e| {
            e.name = "Changed wall".into();
            if let ElementKind::Wall(w) = &mut e.kind {
                w.height += 0.25;
            }
        })
        .unwrap();
}

/// The starter house the app opens with: walls built of layered
/// assemblies, their materials, doors and windows hosted in the walls.
fn house() -> CreaBimApp {
    let mut app = CreaBimApp::blank(None);
    app.load_model(creabim_core::starter::starter_house());
    app
}

fn snap(app: &CreaBimApp) -> Model {
    app.doc.model().clone()
}

#[test]
fn the_sample_house_has_what_these_tests_change() {
    let app = house();
    let m = app.doc.model();
    assert!(m.iter().any(|e| matches!(e.kind, ElementKind::Wall(_))));
    assert!(m
        .iter()
        .any(|e| matches!(&e.kind, ElementKind::Assembly(a) if !a.layers.is_empty())));
    assert!(m.iter().any(|e| matches!(e.kind, ElementKind::Material(_))));
    // walls hosting openings: links that undo and redo must keep
    assert!(m.iter().any(|e| matches!(e.kind, ElementKind::Opening(_))));
}

#[test]
fn undo_takes_back_the_last_step_and_nothing_before_it() {
    let mut app = house();
    let s0 = snap(&app);
    move_a_wall(&mut app);
    let s1 = snap(&app);
    thicken_a_layer(&mut app);
    let s2 = snap(&app);
    recolour_a_material(&mut app);
    let s3 = snap(&app);
    change_a_wall(&mut app);
    let s4 = snap(&app);
    for (want, step) in [
        (&s3, "wall height"),
        (&s2, "colour"),
        (&s1, "layer"),
        (&s0, "wall move"),
    ] {
        assert!(app.doc.undo(), "undo {step}");
        same(app.doc.model(), want, &format!("after undoing the {step}"));
    }
    assert!(!app.doc.undo(), "nothing more to undo");
    same(app.doc.model(), &s0, "with nothing more to undo");
    let _ = s4;
}

#[test]
fn redo_rebuilds_each_state_exactly() {
    let mut app = house();
    let mut states = vec![snap(&app)];
    for edit in [
        move_a_wall,
        thicken_a_layer,
        recolour_a_material,
        change_a_wall,
    ] {
        edit(&mut app);
        states.push(snap(&app));
    }
    while app.doc.undo() {}
    same(app.doc.model(), &states[0], "all undone");
    for (k, want) in states.iter().enumerate().skip(1) {
        assert!(app.doc.redo(), "redo step {k}");
        same(app.doc.model(), want, &format!("after redoing step {k}"));
    }
    assert!(!app.doc.redo(), "nothing more to redo");
    // back and forth a few times: always the same states
    for round in 0..3 {
        while app.doc.undo() {}
        same(
            app.doc.model(),
            &states[0],
            &format!("round {round}, all undone"),
        );
        while app.doc.redo() {}
        same(
            app.doc.model(),
            &states[4],
            &format!("round {round}, all redone"),
        );
    }
}

#[test]
fn a_new_change_after_undo_replaces_what_could_be_redone() {
    let mut app = house();
    move_a_wall(&mut app);
    thicken_a_layer(&mut app);
    assert!(app.doc.undo());
    recolour_a_material(&mut app);
    let now = snap(&app);
    assert!(
        !app.doc.redo(),
        "the undone step is gone once a new one is made"
    );
    same(app.doc.model(), &now, "redo changes nothing");
}

#[test]
fn undo_in_a_polyline_takes_back_its_last_point_and_no_earlier_work() {
    let mut app = house();
    let ctx = egui::Context::default();
    let plan = app
        .doc
        .model()
        .iter()
        .find(
            |e| matches!(&e.kind, ElementKind::View(v) if matches!(v.kind, ViewKind::Plan { .. })),
        )
        .map(|e| e.id)
        .unwrap();
    app.active_view = Some(plan);
    // work done before the command
    thicken_a_layer(&mut app);
    let before = snap(&app);
    let steps = app.doc.undo_steps().len();
    // the plan drawn as the app draws it, frame by frame: a point typed
    // is taken there
    let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0));
    let mut t = 0.0;
    let mut frame = |app: &mut CreaBimApp| {
        t += 1.0 / 60.0;
        let raw = egui::RawInput {
            screen_rect: Some(rect),
            events: vec![egui::Event::PointerMoved(egui::Pos2::new(600.0, 450.0))],
            time: Some(t),
            max_texture_side: Some(8192),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| app.show_plan(ui, rect, Color32::WHITE));
        out.textures_delta.clear();
    };
    frame(&mut app);
    app.cmd_start(&ctx, "PLINE");
    frame(&mut app);
    // PLINE gathers its points as a command's picks
    let points = |app: &CreaBimApp| app.plan.cmd_pick.as_ref().map_or(0, |(_, p)| p.len());
    for p in ["0,0", "5,0", "5,4"] {
        app.cmd_enter(&ctx, p);
        frame(&mut app);
    }
    assert_eq!(points(&app), 3, "three points taken");
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    assert_eq!(points(&app), 2, "the last point goes");
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    assert_eq!(points(&app), 1, "and the one before");
    same(app.doc.model(), &before, "undo in the command");
    assert_eq!(app.doc.undo_steps().len(), steps, "no step taken back");
}

/// The windows with Save and Cancel, as the app runs them each frame.
struct Window {
    key: &'static str,
    open: fn(&mut CreaBimApp),
    draw: fn(&mut CreaBimApp, &egui::Context),
    /// A window whose footer sends its Save or Cancel through
    /// `dialog_close` (the project's windows); the settings windows apply
    /// theirs within their own frame.
    by_close: bool,
}

const WINDOWS: [Window; 13] = [
    Window {
        key: "materials",
        open: |a| a.mat.open = true,
        draw: |a, c| a.materials_window(c),
        by_close: true,
    },
    Window {
        key: "asm",
        open: |a| a.asm.open = true,
        draw: |a, c| a.assemblies_window(c),
        by_close: true,
    },
    Window {
        key: "types",
        open: |a| a.types.open = true,
        draw: |a, c| a.types_window(c),
        by_close: true,
    },
    Window {
        key: "levels",
        open: |a| a.levels.open = true,
        draw: |a, c| a.levels_window(c),
        by_close: true,
    },
    Window {
        key: "annstyles",
        open: |a| a.drafting.styles_open = true,
        draw: |a, c| a.styles_window(c),
        by_close: true,
    },
    Window {
        key: "templates",
        open: |a| a.drafting.templates_open = true,
        draw: |a, c| a.templates_window(c),
        by_close: true,
    },
    Window {
        key: "units",
        open: |a| a.levels.units_open = true,
        draw: |a, c| a.units_window(c),
        by_close: true,
    },
    Window {
        key: "marks",
        open: |a| a.marks.open = true,
        draw: |a, c| a.marks_window(c),
        by_close: true,
    },
    Window {
        key: "gstyles",
        open: |a| a.gstyles.open = true,
        draw: |a, c| a.graphic_styles_window(c),
        by_close: true,
    },
    Window {
        key: "settings",
        open: |a| a.settings_open = true,
        draw: |a, c| a.settings_window(c),
        by_close: false,
    },
    Window {
        key: "dsettings",
        open: |a| a.shell.drafting_settings = true,
        draw: |a, c| a.drafting_settings_window(c),
        by_close: false,
    },
    Window {
        key: "layers",
        open: |a| a.shell.layer_manager = true,
        draw: |a, c| a.layer_manager_window(c),
        by_close: false,
    },
    Window {
        key: "ucs",
        open: |a| a.shell.ucs_manager = true,
        draw: |a, c| a.ucs_manager_window(c),
        by_close: false,
    },
];

/// The app and a frame clock, to run windows as the app does.
struct Run {
    app: CreaBimApp,
    ctx: egui::Context,
    time: f64,
}

impl Run {
    fn new() -> Self {
        Self {
            app: house(),
            ctx: egui::Context::default(),
            time: 0.0,
        }
    }

    /// One frame of window `w`, as the app runs it.
    fn frame(&mut self, w: &Window) {
        self.time += 1.0 / 60.0;
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 1000.0),
            )),
            time: Some(self.time),
            max_texture_side: Some(8192),
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(raw, |ui| {
            let ctx = ui.ctx().clone();
            app.in_dialog(w.key, |s| (w.draw)(s, &ctx));
        });
        out.textures_delta.clear();
    }

    /// Open window `w` and let it settle.
    fn open(&mut self, w: &Window) {
        (w.open)(&mut self.app);
        for _ in 0..3 {
            self.frame(w);
        }
        assert!(
            self.app.dialogs.contains_key(w.key),
            "{}: open, it notes what Cancel puts back",
            w.key
        );
    }

    /// Save (`keep`) or Cancel window `w`, as its buttons do.
    fn close(&mut self, w: &Window, keep: bool) {
        if w.by_close {
            self.app.dialog_close.push((w.key, keep));
            self.frame(w);
        } else {
            self.app.dialog_end(w.key, keep);
        }
        assert!(!self.app.dialogs.contains_key(w.key), "{}: closed", w.key);
    }

    /// A change made by window `w` itself.
    fn in_window(&mut self, w: &Window, edit: fn(&mut CreaBimApp)) {
        self.app.in_dialog(w.key, edit);
    }
}

#[test]
fn cancel_takes_back_the_windows_own_change_and_keeps_what_came_before() {
    for w in WINDOWS.iter().filter(|w| w.by_close) {
        let mut r = Run::new();
        move_a_wall(&mut r.app);
        let before_open = snap(&r.app);
        r.open(w);
        r.in_window(w, thicken_a_layer);
        r.close(w, false);
        same(
            r.app.doc.model(),
            &before_open,
            &format!("{}: after Cancel", w.key),
        );
    }
}

#[test]
fn cancel_with_nothing_changed_in_the_window_changes_nothing() {
    for w in &WINDOWS {
        let mut r = Run::new();
        move_a_wall(&mut r.app);
        thicken_a_layer(&mut r.app);
        let before_open = snap(&r.app);
        let steps = r.app.doc.undo_steps().len();
        r.open(w);
        r.close(w, false);
        same(
            r.app.doc.model(),
            &before_open,
            &format!("{}: after Cancel", w.key),
        );
        assert_eq!(
            r.app.doc.undo_steps().len(),
            steps,
            "{}: no step taken back",
            w.key
        );
    }
}

#[test]
fn cancel_never_takes_back_work_done_elsewhere_while_the_window_is_open() {
    for w in WINDOWS.iter().filter(|w| w.by_close) {
        let mut r = Run::new();
        r.open(w);
        r.in_window(w, thicken_a_layer);
        // work in a view while the window stays open
        recolour_a_material(&mut r.app);
        let now = snap(&r.app);
        r.close(w, false);
        same(r.app.doc.model(), &now, &format!("{}: after Cancel", w.key));
    }
}

#[test]
fn save_keeps_the_windows_change_as_an_undo_step() {
    for w in WINDOWS.iter().filter(|w| w.by_close) {
        let mut r = Run::new();
        move_a_wall(&mut r.app);
        let before_open = snap(&r.app);
        r.open(w);
        r.in_window(w, thicken_a_layer);
        let changed = snap(&r.app);
        r.close(w, true);
        same(
            r.app.doc.model(),
            &changed,
            &format!("{}: after Save", w.key),
        );
        // saved, it is an ordinary step: Undo takes back just it
        assert!(r.app.doc.undo());
        same(
            r.app.doc.model(),
            &before_open,
            &format!("{}: Undo after Save", w.key),
        );
    }
}

#[test]
fn opening_and_closing_a_window_again_and_again_leaves_the_project_as_it_was() {
    for w in &WINDOWS {
        let mut r = Run::new();
        let initial = snap(&r.app);
        move_a_wall(&mut r.app);
        let before = snap(&r.app);
        let steps = r.app.doc.undo_steps().len();
        for keep in [false, true, false, true, false, true] {
            r.open(w);
            for _ in 0..3 {
                r.frame(w);
            }
            r.close(w, keep);
            same(
                r.app.doc.model(),
                &before,
                &format!("{}: opened and closed", w.key),
            );
        }
        assert_eq!(
            r.app.doc.undo_steps().len(),
            steps,
            "{}: no step added or lost",
            w.key
        );
        // and Undo still takes back the work done before
        assert!(r.app.doc.undo());
        same(
            r.app.doc.model(),
            &initial,
            &format!("{}: Undo after", w.key),
        );
    }
}
