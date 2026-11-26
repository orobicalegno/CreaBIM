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

// ---------------------------------------------------------------------
// KNOWN BUGS, found by G4. Each test holds the RIGHT behaviour, so today
// it fails: it is ignored (`#[ignore]`) so the suite stays green, and is
// listed in docs/regressioni.md. When its bug is fixed, remove the
// `#[ignore]` line in the same commit as the fix. To see them fail now:
//
//     cargo test -p creabim-app --bin creabim-app known_bug -- --ignored
// ---------------------------------------------------------------------

/// A frame of the plan as the app draws it, with `events`.
fn plan_frame(app: &mut CreaBimApp, ctx: &egui::Context, t: &mut f64, events: Vec<egui::Event>) {
    *t += 1.0 / 60.0;
    let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0));
    let raw = egui::RawInput {
        screen_rect: Some(rect),
        events,
        time: Some(*t),
        max_texture_side: Some(8192),
        ..Default::default()
    };
    let mut out = ctx.run_ui(raw, |ui| app.show_plan(ui, rect, Color32::WHITE));
    out.textures_delta.clear();
}

/// A click with the left button at `at`, as the pointer comes and goes.
fn click_plan(app: &mut CreaBimApp, ctx: &egui::Context, t: &mut f64, at: egui::Pos2) {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    plan_frame(app, ctx, t, vec![egui::Event::PointerMoved(at)]);
    plan_frame(app, ctx, t, vec![egui::Event::PointerMoved(at)]);
    plan_frame(app, ctx, t, vec![button(true)]);
    plan_frame(app, ctx, t, vec![button(false)]);
    plan_frame(app, ctx, t, vec![]);
}

/// The starter house with its ground floor plan shown, tool `tool` picked,
/// after one earlier change (a layer thickened): the app, a context, the
/// clock and the project as it was before the tool.
fn drawing_with(tool: crate::planview::Tool) -> (CreaBimApp, egui::Context, f64, Model) {
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
    let mut t = 0.0;
    plan_frame(&mut app, &ctx, &mut t, vec![]);
    thicken_a_layer(&mut app);
    let before = snap(&app);
    let i = crate::planview::Tool::ALL
        .iter()
        .position(|x| *x == tool)
        .unwrap();
    app.run_action(&ctx, &format!("tool:{i}"));
    (app, ctx, t, before)
}

#[test]
fn fixed_c1_undo_while_drawing_a_slab_outline_keeps_earlier_work() {
    let (mut app, ctx, mut t, before) = drawing_with(crate::planview::Tool::Slab);
    for x in [500.0, 700.0, 700.0] {
        let y = if x == 700.0
            && app
                .plan
                .outline
                .as_ref()
                .is_some_and(|o| o.points.len() == 2)
        {
            600.0
        } else {
            400.0
        };
        click_plan(&mut app, &ctx, &mut t, egui::Pos2::new(x, y));
    }
    let corners = |app: &CreaBimApp| app.plan.outline.as_ref().map_or(0, |o| o.points.len());
    assert_eq!(corners(&app), 3, "three corners clicked");
    same(
        app.doc.model(),
        &before,
        "while drawing, nothing is confirmed yet",
    );
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    // the right behaviour: the last corner goes, the earlier work stays
    same(app.doc.model(), &before, "Ctrl+Z while drawing the outline");
    assert_eq!(corners(&app), 2, "Ctrl+Z takes back the last corner");
}

#[test]
fn fixed_c1_undo_after_a_walls_first_click_keeps_earlier_work() {
    let (mut app, ctx, mut t, before) = drawing_with(crate::planview::Tool::Wall);
    click_plan(&mut app, &ctx, &mut t, egui::Pos2::new(500.0, 400.0));
    assert!(app.plan.pending.is_some(), "the wall's start is clicked");
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    // the right behaviour: the start point goes, the earlier work stays
    same(app.doc.model(), &before, "Ctrl+Z after the first click");
    assert!(
        app.plan.pending.is_none(),
        "Ctrl+Z takes back the start point"
    );
}

/// Was a known bug (G4), fixed by C2.
#[test]
fn fixed_c2_undo_in_a_wall_chain_resumes_from_the_previous_point() {
    let (mut app, ctx, mut t, before) = drawing_with(crate::planview::Tool::Wall);
    let pts = [
        egui::Pos2::new(400.0, 400.0),
        egui::Pos2::new(600.0, 400.0),
        egui::Pos2::new(600.0, 600.0),
    ];
    for p in pts {
        click_plan(&mut app, &ctx, &mut t, p);
    }
    let walls = |app: &CreaBimApp| {
        app.doc
            .model()
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
            .count()
    };
    let n_before = before
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
        .count();
    assert_eq!(walls(&app), n_before + 2, "two walls drawn in the chain");
    let second_start = app.plan.pending.map(|_| ()).and_then(|_| {
        app.doc
            .model()
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Wall(w) => Some(w.start),
                _ => None,
            })
            .last()
    });
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    assert_eq!(walls(&app), n_before + 1, "Ctrl+Z takes back the last wall");
    // the right behaviour: the chain goes on from where the remaining wall
    // ends (the removed wall's start), not from a point with no wall
    assert_eq!(
        app.plan.pending, second_start,
        "the chain resumes from the previous point"
    );
}

/// Was a known bug (G4), fixed by C3: and the window keeps running a few
/// frames after the change elsewhere, as it does in the app.
#[test]
fn fixed_c3_cancel_in_a_settings_window_keeps_settings_changed_elsewhere() {
    for w in &WINDOWS {
        let mut r = Run::new();
        r.open(w);
        // while it is open: the 3D style from the Render menu, the snaps
        // switched off (F3), the polar angle and the UCS from the command
        // line
        let want = elsewhere(&mut r.app);
        for _ in 0..3 {
            r.frame(w);
        }
        r.close(w, false);
        assert_eq!(what_settings(&r.app), want, "{}: Cancel keeps them", w.key);
    }
}

// ---------------------------------------------------------------------
// C3: Cancel in a window puts back the settings that window changed, and
// only those.
// ---------------------------------------------------------------------

/// The settings Cancel may put back, as one value to compare.
type Settled = (
    crate::files::Settings,
    crate::snaps::SnapSettings,
    f64,
    crate::planview::Ucs,
);

fn what_settings(app: &CreaBimApp) -> Settled {
    (
        app.shell.settings.clone(),
        app.plan.snap,
        app.plan.guide_step,
        app.plan.ucs,
    )
}

/// Settings changed somewhere else than the window; the settings then.
fn elsewhere(app: &mut CreaBimApp) -> Settled {
    app.shell.settings.render_3d =
        if app.shell.settings.render_3d == crate::render3d::RenderMode::Maquette {
            crate::render3d::RenderMode::Shaded
        } else {
            crate::render3d::RenderMode::Maquette
        };
    app.plan.snap.on = !app.plan.snap.on;
    app.plan.guide_step = 7.5;
    app.plan.ucs.angle = 0.3;
    what_settings(app)
}

/// Settings changed by a window itself.
fn in_the_window(app: &mut CreaBimApp) {
    app.shell.settings.crosshair = !app.shell.settings.crosshair;
    app.shell.settings.marker_size = app.shell.settings.marker_size.wrapping_add(3);
    app.plan.snap.midpoint = !app.plan.snap.midpoint;
    app.plan.guide_step = 22.5;
    app.plan.ucs.origin = DVec2::new(2.0, 3.0);
}

#[test]
fn c3_cancel_puts_back_what_the_window_changed_and_save_keeps_it() {
    for w in &WINDOWS {
        let mut r = Run::new();
        let at_open = what_settings(&r.app);
        r.open(w);
        r.in_window(w, in_the_window);
        r.frame(w);
        r.close(w, false);
        assert_eq!(
            what_settings(&r.app),
            at_open,
            "{}: Cancel puts them back",
            w.key
        );

        let mut r = Run::new();
        r.open(w);
        r.in_window(w, in_the_window);
        let changed = what_settings(&r.app);
        r.close(w, true);
        assert_eq!(what_settings(&r.app), changed, "{}: Save keeps them", w.key);
    }
}

#[test]
fn c3_cancel_puts_back_its_own_and_keeps_what_was_done_elsewhere_meanwhile() {
    for w in &WINDOWS {
        let mut r = Run::new();
        r.open(w);
        r.in_window(w, in_the_window);
        // after the window's change, others elsewhere
        let mut want = elsewhere(&mut r.app);
        r.frame(w);
        r.close(w, false);
        // what the window changed goes back; the guide step, changed by
        // both, keeps the later change (made elsewhere)
        let at_open = Run::new();
        want.0.crosshair = at_open.app.shell.settings.crosshair;
        want.0.marker_size = at_open.app.shell.settings.marker_size;
        want.1.midpoint = at_open.app.plan.snap.midpoint;
        want.3.origin = at_open.app.plan.ucs.origin;
        assert_eq!(what_settings(&r.app), want, "{}", w.key);
        assert!(
            r.app.status.contains("changed elsewhere"),
            "{}: the status says what stays: {}",
            w.key,
            r.app.status
        );
    }
}

#[test]
fn c3_a_setting_changed_elsewhere_then_by_the_window_goes_back_to_the_elsewhere_value() {
    for w in &WINDOWS {
        let mut r = Run::new();
        r.open(w);
        // first elsewhere, then the window changes the same setting
        r.app.plan.guide_step = 7.5;
        r.frame(w);
        r.in_window(w, |a| a.plan.guide_step = 22.5);
        r.close(w, false);
        assert_eq!(
            r.app.plan.guide_step, 7.5,
            "{}: back to before the window's change",
            w.key
        );
    }
}

#[test]
fn c3_two_windows_open_each_cancel_puts_back_only_its_own() {
    let (a, b) = (&WINDOWS[9], &WINDOWS[10]);
    assert_eq!((a.key, b.key), ("settings", "dsettings"));
    let mut r = Run::new();
    let at_open = what_settings(&r.app);
    r.open(a);
    r.open(b);
    r.in_window(a, |s| {
        s.shell.settings.crosshair = !s.shell.settings.crosshair
    });
    r.in_window(b, |s| s.plan.snap.midpoint = !s.plan.snap.midpoint);
    let both = what_settings(&r.app);
    r.close(a, false);
    assert_eq!(
        r.app.shell.settings.crosshair, at_open.0.crosshair,
        "a's change goes"
    );
    assert_eq!(r.app.plan.snap.midpoint, both.1.midpoint, "b's stays");
    r.frame(b);
    r.close(b, false);
    assert_eq!(what_settings(&r.app), at_open, "then b's goes too");
}

#[test]
fn c3_opening_again_and_again_with_save_and_cancel() {
    for w in &WINDOWS {
        let mut r = Run::new();
        let at_start = what_settings(&r.app);
        r.open(w);
        r.in_window(w, in_the_window);
        r.close(w, false);
        assert_eq!(what_settings(&r.app), at_start, "{}: first Cancel", w.key);
        r.open(w);
        r.in_window(w, |s| s.plan.guide_step = 30.0);
        r.close(w, true);
        r.open(w);
        r.in_window(w, |s| s.plan.guide_step = 45.0);
        r.close(w, false);
        assert_eq!(
            r.app.plan.guide_step, 30.0,
            "{}: back to what was saved",
            w.key
        );
        r.open(w);
        r.close(w, false);
        assert_eq!(
            r.app.plan.guide_step, 30.0,
            "{}: an empty Cancel changes nothing",
            w.key
        );
    }
}

#[test]
fn c3_after_cancel_the_settings_and_the_project_save_and_reopen_as_they_are() {
    let w = &WINDOWS[9];
    let mut r = Run::new();
    r.open(w);
    r.in_window(w, in_the_window);
    let want = elsewhere(&mut r.app);
    r.close(w, false);
    // the settings, as stored in settings.json and read back
    let text = serde_json::to_string_pretty(&r.app.shell.settings).unwrap();
    let back: crate::files::Settings = serde_json::from_str(&text).unwrap();
    assert_eq!(
        back, r.app.shell.settings,
        "settings.json reads back the same"
    );
    assert_eq!(
        back.render_3d, want.0.render_3d,
        "with the change made elsewhere"
    );
    // the project, saved and reopened
    let path = std::env::temp_dir().join(format!("c3-{}.creabim", std::process::id()));
    creabim_store::save(&path, &r.app.doc).expect("saves");
    let reopened = creabim_store::load(&path).expect("reopens");
    let _ = std::fs::remove_file(&path);
    same(reopened.model(), r.app.doc.model(), "the project");
}

// ---------------------------------------------------------------------
// C1: Ctrl+Z while a tool is part way takes back what the tool has not
// confirmed yet, never earlier work.
// ---------------------------------------------------------------------

/// Does a tool have input not confirmed yet (points clicked, a pick made)?
fn part_way(app: &CreaBimApp) -> bool {
    app.plan.has_input()
        || !app.plan.parts.path.is_empty()
        || app.pending_3d.is_some()
        || app
            .plan
            .cmd_pick
            .as_ref()
            .is_some_and(|(_, p)| !p.is_empty())
}

/// Every plan tool, in groups run side by side (each group is a test).
const TOOL_GROUPS: [&[crate::planview::Tool]; 6] = {
    use crate::planview::Tool::*;
    [
        &[Wall, Slab, Shaft, Beam, Roof, Stair, Railing, Footing],
        &[
            Extrusion, Sweep, Void, Terrain, Paving, Planting, Object, Mass, Column,
        ],
        &[Room, RoomSeparator, Ceiling, Door, Window, Opening],
        &[Section, Elevation, Callout, Camera, Level],
        &[
            Dimension, Tag, Angle, Radius, Diameter, ArcLength, Note, SpotLevel,
        ],
        &[Sketch, Guide, Grid, Align, Detail],
    ]
};

/// With each tool of `tools`, one, two and three clicks, then Ctrl+Z:
/// while the tool is part way, nothing confirmed may change. The number of
/// cases where a tool was part way.
///
/// One exception, on purpose since C2: in a chain of walls or beams the
/// pieces drawn are confirmed, and Ctrl+Z takes back the last of them (and
/// only that: one step, the project exactly as before the last click).
fn undo_part_way_keeps_confirmed_work(tools: &[crate::planview::Tool]) -> usize {
    let pts = [
        egui::Pos2::new(450.0, 380.0),
        egui::Pos2::new(650.0, 380.0),
        egui::Pos2::new(650.0, 560.0),
    ];
    let mut checked = 0;
    let mut wrong = Vec::new();
    for &tool in tools {
        for clicks in 1..=pts.len() {
            let (mut app, ctx, mut t, _) = drawing_with(tool);
            let mut before_last = snap(&app);
            for p in &pts[..clicks] {
                before_last = snap(&app);
                click_plan(&mut app, &ctx, &mut t, *p);
            }
            if !part_way(&app) {
                continue;
            }
            checked += 1;
            let now = snap(&app);
            let steps = app.doc.undo_steps().len();
            let in_chain = !app.plan.chain_steps.is_empty();
            app.run_command(&ctx, crate::commands::Command::Undo, None);
            let kept = app.doc.undo_steps().len() == steps && app.doc.model() == &now;
            let chain_piece_back = in_chain
                && app.doc.undo_steps().len() + 1 == steps
                && app.doc.model() == &before_last;
            if !kept && !chain_piece_back {
                wrong.push(format!("{tool:?} after {clicks} click(s)"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "Ctrl+Z took back confirmed work while a tool was part way:\n  {}",
        wrong.join("\n  ")
    );
    checked
}

#[test]
fn c1_every_plan_tool_is_in_a_group() {
    for tool in crate::planview::Tool::ALL {
        if tool != crate::planview::Tool::Select {
            assert!(
                TOOL_GROUPS.iter().any(|g| g.contains(&tool)),
                "{tool:?} is checked by no c1_undo_part_way test"
            );
        }
    }
}

#[test]
fn c1_undo_part_way_walls_slabs_roofs_stairs() {
    assert!(undo_part_way_keeps_confirmed_work(TOOL_GROUPS[0]) >= 15);
}

#[test]
fn c1_undo_part_way_parts_and_site() {
    assert!(undo_part_way_keeps_confirmed_work(TOOL_GROUPS[1]) >= 9);
}

#[test]
fn c1_undo_part_way_rooms_and_openings() {
    assert!(undo_part_way_keeps_confirmed_work(TOOL_GROUPS[2]) >= 6);
}

#[test]
fn c1_undo_part_way_views() {
    assert!(undo_part_way_keeps_confirmed_work(TOOL_GROUPS[3]) >= 6);
}

#[test]
fn c1_undo_part_way_annotation() {
    assert!(undo_part_way_keeps_confirmed_work(TOOL_GROUPS[4]) >= 10);
}

#[test]
fn c1_undo_part_way_drafting() {
    assert!(undo_part_way_keeps_confirmed_work(TOOL_GROUPS[5]) >= 8);
}

#[test]
fn c1_with_nothing_part_way_undo_takes_back_the_last_step_as_ever() {
    use crate::planview::Tool;
    for tool in [Tool::Select, Tool::Wall, Tool::Slab, Tool::Dimension] {
        let (mut app, ctx, _t, before) = drawing_with(tool);
        assert!(!part_way(&app), "{tool:?}: picked, nothing clicked yet");
        let steps = app.doc.undo_steps().len();
        app.run_command(&ctx, crate::commands::Command::Undo, None);
        assert_eq!(
            app.doc.undo_steps().len(),
            steps - 1,
            "{tool:?}: one step undone"
        );
        assert_ne!(
            app.doc.model(),
            &before,
            "{tool:?}: the earlier change undone"
        );
    }
}

#[test]
fn c1_esc_drops_the_whole_outline_and_ctrl_z_only_its_last_corner() {
    let corners = |app: &CreaBimApp| app.plan.outline.as_ref().map_or(0, |o| o.points.len());
    let draw = || {
        let (mut app, ctx, mut t, before) = drawing_with(crate::planview::Tool::Slab);
        for p in [
            egui::Pos2::new(450.0, 380.0),
            egui::Pos2::new(650.0, 380.0),
            egui::Pos2::new(650.0, 560.0),
        ] {
            click_plan(&mut app, &ctx, &mut t, p);
        }
        (app, ctx, before)
    };
    // Ctrl+Z: one corner at a time, the tool and the earlier work kept
    let (mut app, ctx, before) = draw();
    assert_eq!(corners(&app), 3);
    for left in [2, 1, 0] {
        app.run_command(&ctx, crate::commands::Command::Undo, None);
        assert_eq!(corners(&app), left, "Ctrl+Z takes back one corner");
        same(app.doc.model(), &before, "Ctrl+Z while drawing");
    }
    assert_eq!(app.plan.tool, crate::planview::Tool::Slab, "the tool stays");
    // Esc (and a right-click, through the same step back): the whole
    // outline, then the tool
    let (mut app, _ctx, before) = draw();
    assert!(app.plan.step_back());
    assert_eq!(corners(&app), 0, "Esc drops the outline");
    assert_eq!(
        app.plan.tool,
        crate::planview::Tool::Slab,
        "first Esc keeps the tool"
    );
    same(app.doc.model(), &before, "Esc");
    assert!(app.plan.step_back());
    assert_eq!(
        app.plan.tool,
        crate::planview::Tool::Select,
        "second Esc leaves it"
    );
}

// ---------------------------------------------------------------------
// C2: Ctrl+Z in a chain of walls or beams takes back the chain's last
// piece and goes on from where it started; Redo puts it back.
// ---------------------------------------------------------------------

/// The plan points a chain is clicked through.
const CHAIN: [egui::Pos2; 4] = [
    egui::Pos2::new(400.0, 380.0),
    egui::Pos2::new(600.0, 380.0),
    egui::Pos2::new(600.0, 560.0),
    egui::Pos2::new(420.0, 560.0),
];

/// After each piece of a chain: the project, and where the chain was then
/// to go on from.
type Pieces = Vec<(Model, Option<DVec2>)>;

/// Draw a chain with `tool` through every point of `CHAIN`: the project
/// before the tool, and after each piece.
fn draw_chain(
    tool: crate::planview::Tool,
    arc: bool,
) -> (CreaBimApp, egui::Context, f64, Model, Pieces) {
    let (mut app, ctx, mut t, before) = drawing_with(tool);
    if arc {
        app.plan.wall_shape = 1;
    }
    let mut after = Vec::new();
    let mut steps = app.doc.undo_steps().len();
    let points: Vec<egui::Pos2> = if arc {
        // an arc wall: its start, its end, a point it passes through; the
        // next one starts where it ended: its end, a point it passes
        vec![
            CHAIN[0],
            CHAIN[1],
            egui::Pos2::new(500.0, 330.0),
            CHAIN[2],
            egui::Pos2::new(650.0, 470.0),
        ]
    } else {
        CHAIN.to_vec()
    };
    for p in points {
        click_plan(&mut app, &ctx, &mut t, p);
        if app.doc.undo_steps().len() > steps {
            steps = app.doc.undo_steps().len();
            after.push((snap(&app), app.plan.pending));
        }
    }
    (app, ctx, t, before, after)
}

fn count(m: &Model, pick: fn(&ElementKind) -> bool) -> usize {
    m.iter().filter(|e| pick(&e.kind)).count()
}

/// The chain checks, for walls straight or arced and for beams: Ctrl+Z
/// steps back a piece at a time to the first click, then takes that back,
/// then (nothing part way) undoes the earlier work as ever; the project is
/// each time exactly as it was, joins and layers included.
fn chain_steps_back(tool: crate::planview::Tool, arc: bool, what: &str) {
    let (mut app, ctx, _t, before, after) = draw_chain(tool, arc);
    let pieces = after.len();
    assert!(pieces >= 2, "{what}: at least two pieces drawn ({pieces})");
    let steps_before = app.doc.undo_steps().len() - pieces;
    for k in (0..pieces).rev() {
        app.run_command(&ctx, crate::commands::Command::Undo, None);
        let (want, from) = if k == 0 {
            (&before, None)
        } else {
            (&after[k - 1].0, after[k - 1].1)
        };
        same(
            app.doc.model(),
            want,
            &format!("{what}: after taking back piece {}", k + 1),
        );
        assert_eq!(app.plan.tool, tool, "{what}: the tool stays");
        if k > 0 {
            assert_eq!(
                app.plan.pending, from,
                "{what}: the chain goes on from where the remaining piece ends"
            );
        }
    }
    // back to the first click: still part way, then nothing
    assert!(
        app.plan.pending.is_some(),
        "{what}: the first click is left"
    );
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    assert!(app.plan.pending.is_none(), "{what}: then it goes");
    same(
        app.doc.model(),
        &before,
        &format!("{what}: the earlier work kept"),
    );
    assert_eq!(
        app.doc.undo_steps().len(),
        steps_before,
        "{what}: no earlier step undone"
    );
}

#[test]
fn c2_undo_in_a_chain_of_straight_walls_steps_back_a_wall_at_a_time() {
    chain_steps_back(crate::planview::Tool::Wall, false, "straight walls");
}

#[test]
fn c2_undo_in_a_chain_of_arc_walls_steps_back_an_arc_at_a_time() {
    chain_steps_back(crate::planview::Tool::Wall, true, "arc walls");
}

#[test]
fn c2_undo_in_a_chain_of_beams_steps_back_a_beam_at_a_time() {
    chain_steps_back(crate::planview::Tool::Beam, false, "beams");
}

#[test]
fn c2_redo_in_a_chain_puts_the_piece_back_and_the_chain_goes_on_from_its_end() {
    for (tool, what) in [
        (crate::planview::Tool::Wall, "walls"),
        (crate::planview::Tool::Beam, "beams"),
    ] {
        let (mut app, ctx, mut t, _before, after) = draw_chain(tool, false);
        let last = after.len() - 1;
        app.run_command(&ctx, crate::commands::Command::Undo, None);
        app.run_command(&ctx, crate::commands::Command::Undo, None);
        app.run_command(&ctx, crate::commands::Command::Redo, None);
        same(
            app.doc.model(),
            &after[last - 1].0,
            &format!("{what}: Redo puts a piece back"),
        );
        assert_eq!(
            app.plan.pending,
            after[last - 1].1,
            "{what}: and the chain is at its end"
        );
        app.run_command(&ctx, crate::commands::Command::Redo, None);
        same(
            app.doc.model(),
            &after[last].0,
            &format!("{what}: Redo again"),
        );
        assert_eq!(
            app.plan.pending, after[last].1,
            "{what}: at the last piece's end"
        );
        // and on: a new piece starts where the chain stands
        let pieces = |m: &Model| {
            count(m, |k| {
                matches!(k, ElementKind::Wall(_) | ElementKind::Beam(_))
            })
        };
        let n = pieces(app.doc.model());
        click_plan(&mut app, &ctx, &mut t, egui::Pos2::new(380.0, 470.0));
        assert_eq!(pieces(app.doc.model()), n + 1, "{what}: the chain goes on");
    }
}

#[test]
fn c2_a_beam_drawn_after_undo_joins_the_beam_the_chain_stands_on() {
    let (mut app, ctx, mut t, _before, after) = draw_chain(crate::planview::Tool::Beam, false);
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    // the beam the chain now ends on
    let last_beam = after[after.len() - 2]
        .0
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Beam(_)))
        .last()
        .map(|e| e.id);
    assert_eq!(
        app.plan.members.last_beam, last_beam,
        "the chain stands on the beam before"
    );
    click_plan(&mut app, &ctx, &mut t, egui::Pos2::new(380.0, 470.0));
    // the new beam is drawn, as one step with its join to that beam
    let tx = &app.doc.undo_steps().last().unwrap().tx;
    assert_eq!(
        tx.ops.len(),
        2,
        "the new beam and its join to the one before"
    );
}

#[test]
fn c2_a_chain_interrupted_by_other_work_never_undoes_that_work_by_surprise() {
    let (mut app, ctx, _t, _before, after) = draw_chain(crate::planview::Tool::Wall, false);
    // a change elsewhere while the chain is on (a material recoloured)
    recolour_a_material(&mut app);
    let recoloured = snap(&app);
    // Ctrl+Z: the chain's point goes (C1), nothing in the project
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    same(
        app.doc.model(),
        &recoloured,
        "Ctrl+Z with the chain interrupted",
    );
    assert!(app.plan.pending.is_none(), "the chain ends");
    // then Ctrl+Z is the history's: the colour, then the last wall
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    same(
        app.doc.model(),
        &after[after.len() - 1].0,
        "the colour undone",
    );
}

#[test]
fn c2_an_arcs_end_clicked_goes_first_and_the_chain_stays() {
    let (mut app, ctx, mut t, _before, after) = draw_chain(crate::planview::Tool::Wall, true);
    // the next arc's end clicked, not bent yet
    click_plan(&mut app, &ctx, &mut t, egui::Pos2::new(420.0, 560.0));
    assert!(app.plan.wall_arc_end.is_some());
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    assert!(app.plan.wall_arc_end.is_none(), "the arc's end goes first");
    same(
        app.doc.model(),
        &after[after.len() - 1].0,
        "nothing in the project",
    );
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    same(
        app.doc.model(),
        &after[after.len() - 2].0,
        "then the last arc",
    );
    assert_eq!(
        app.plan.pending,
        after[after.len() - 2].1,
        "and on from its start"
    );
}

#[test]
fn c2_after_esc_or_another_tool_ctrl_z_is_the_historys_again() {
    for end in ["esc", "tool"] {
        let (mut app, ctx, _t, _before, after) = draw_chain(crate::planview::Tool::Wall, false);
        if end == "esc" {
            app.plan.cancel();
        } else {
            app.pick_tool(&ctx, crate::planview::Tool::Slab);
            assert_eq!(app.plan.tool, crate::planview::Tool::Slab);
        }
        app.run_command(&ctx, crate::commands::Command::Undo, None);
        same(
            app.doc.model(),
            &after[after.len() - 2].0,
            &format!("{end}: the last wall undone"),
        );
        assert!(app.plan.pending.is_none(), "{end}: no chain brought back");
        app.run_command(&ctx, crate::commands::Command::Redo, None);
        same(
            app.doc.model(),
            &after[after.len() - 1].0,
            &format!("{end}: Redo as ever"),
        );
        assert!(app.plan.pending.is_none(), "{end}: still no chain");
    }
}

#[test]
fn c2_a_chain_stepped_back_saves_and_reopens_as_it_is() {
    let (mut app, ctx, _t, _before, after) = draw_chain(crate::planview::Tool::Wall, false);
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    let path = std::env::temp_dir().join(format!("c2-chain-{}.creabim", std::process::id()));
    creabim_store::save(&path, &app.doc).expect("saves");
    let back = creabim_store::load(&path).expect("reopens");
    let _ = std::fs::remove_file(&path);
    same(
        back.model(),
        &after[after.len() - 2].0,
        "saved and reopened",
    );
}

#[test]
fn c2_the_walls_left_meet_as_they_did_before_the_last_one() {
    // layers and corner joins are worked out from the walls themselves:
    // the plan drawn after Ctrl+Z is the plan drawn before the last wall
    let (mut app, ctx, _t, _before, after) = draw_chain(crate::planview::Tool::Wall, false);
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    let plan = |m: &Model| {
        let level = m
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Level(_)))
            .unwrap()
            .id;
        format!("{:?}", creabim_geom::plan::plan_view(m, level, 1.2).items)
    };
    assert_eq!(
        plan(app.doc.model()),
        plan(&after[after.len() - 2].0),
        "the plan, layers and joins"
    );
}

// ---------------------------------------------------------------------
// V-01, V-10, V-11: an edit not confirmed yet (a move following the
// pointer, a grip dragged, a duplicate being placed) is told apart by its
// own step, never by its name; Ctrl+Z takes back only that edit; another
// project never takes what belongs to this one.
// ---------------------------------------------------------------------

/// Where model point `p` is on the screen of the plan the tests draw.
fn on_screen(app: &CreaBimApp, p: DVec2) -> egui::Pos2 {
    let view = app.active_view.unwrap();
    let (center, scale) = app.plan.cameras[&view];
    crate::planview::Xf {
        rect: Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0)),
        center,
        scale,
        rot: 0.0,
    }
    .to_screen(p)
}

/// The pointer to `at`, with nothing pressed.
fn hover_plan(app: &mut CreaBimApp, ctx: &egui::Context, t: &mut f64, at: egui::Pos2) {
    plan_frame(app, ctx, t, vec![egui::Event::PointerMoved(at)]);
}

fn escape_in_plan(app: &mut CreaBimApp, ctx: &egui::Context, t: &mut f64) {
    let esc = egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    };
    plan_frame(app, ctx, t, vec![esc]);
    plan_frame(app, ctx, t, vec![]);
}

/// The ids of the undo steps, oldest first.
fn step_ids(app: &CreaBimApp) -> Vec<creabim_core::StepId> {
    app.doc.undo_steps().iter().map(|s| s.id).collect()
}

/// The south wall of the ground floor, picked: the wall the moves below
/// move (its doors and its joins to the walls at its ends go with it).
fn pick_south_wall(app: &mut CreaBimApp) -> ElementId {
    let w = app
        .doc
        .model()
        .iter()
        .find(|e| e.name == "GF South")
        .map(|e| e.id)
        .unwrap();
    app.selection.only(w);
    w
}

/// A move with the Move tool: base point, then where it goes.
fn move_with_the_tool(
    app: &mut CreaBimApp,
    ctx: &egui::Context,
    t: &mut f64,
    from: egui::Pos2,
    to: egui::Pos2,
) {
    app.run_command(ctx, crate::commands::Command::Move, None);
    click_plan(app, ctx, t, from);
    hover_plan(app, ctx, t, to);
    click_plan(app, ctx, t, to);
}

#[test]
fn v01_ctrl_z_during_a_move_takes_back_only_the_move_in_progress() {
    let (mut app, ctx, mut t, before) = drawing_with(crate::planview::Tool::Select);
    pick_south_wall(&mut app);
    // move A, confirmed
    move_with_the_tool(
        &mut app,
        &ctx,
        &mut t,
        egui::Pos2::new(500.0, 400.0),
        egui::Pos2::new(560.0, 400.0),
    );
    assert!(app.plan.modal.is_none(), "move A is finished");
    let after_a = snap(&app);
    let steps_a = step_ids(&app);
    assert_ne!(after_a, before, "move A moved the wall");
    // move B, the same wall, the same name, following the pointer
    app.run_command(&ctx, crate::commands::Command::Move, None);
    click_plan(&mut app, &ctx, &mut t, egui::Pos2::new(500.0, 450.0));
    hover_plan(&mut app, &ctx, &mut t, egui::Pos2::new(520.0, 450.0));
    hover_plan(&mut app, &ctx, &mut t, egui::Pos2::new(525.0, 450.0));
    assert_ne!(snap(&app), after_a, "move B follows the pointer");
    // Ctrl+Z: move B goes, move A stays, nothing goes to Redo
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    same(app.doc.model(), &after_a, "Ctrl+Z during move B");
    assert_eq!(step_ids(&app), steps_a, "move A's step is still the last");
    assert!(
        app.doc.redo_steps().is_empty(),
        "the move in progress is not offered to Redo"
    );
    // the pointer goes on moving: nothing moves until a new base point
    hover_plan(&mut app, &ctx, &mut t, egui::Pos2::new(540.0, 450.0));
    hover_plan(&mut app, &ctx, &mut t, egui::Pos2::new(545.0, 450.0));
    same(app.doc.model(), &after_a, "the pointer after Ctrl+Z");
    // Ctrl+Z again: the Move tool ends, the project as it was
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    assert!(app.plan.modal.is_none(), "a second Ctrl+Z ends the move");
    same(app.doc.model(), &after_a, "the second Ctrl+Z");
    assert_eq!(step_ids(&app), steps_a, "and history untouched");
    // then Ctrl+Z is the history's: move A, back to the start, walls joined
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    same(app.doc.model(), &before, "move A undone");
    app.run_command(&ctx, crate::commands::Command::Redo, None);
    same(app.doc.model(), &after_a, "and redone");
}

#[test]
fn v01_esc_after_ctrl_z_during_a_move_keeps_the_confirmed_move() {
    // the sequence found in the analysis: Ctrl+Z, the pointer, Esc
    let (mut app, ctx, mut t, before) = drawing_with(crate::planview::Tool::Select);
    pick_south_wall(&mut app);
    move_with_the_tool(
        &mut app,
        &ctx,
        &mut t,
        egui::Pos2::new(500.0, 400.0),
        egui::Pos2::new(560.0, 400.0),
    );
    let after_a = snap(&app);
    let steps_a = step_ids(&app);
    app.run_command(&ctx, crate::commands::Command::Move, None);
    click_plan(&mut app, &ctx, &mut t, egui::Pos2::new(500.0, 450.0));
    hover_plan(&mut app, &ctx, &mut t, egui::Pos2::new(525.0, 450.0));
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    hover_plan(&mut app, &ctx, &mut t, egui::Pos2::new(535.0, 450.0));
    escape_in_plan(&mut app, &ctx, &mut t);
    if let Some(mut m) = app.plan.modal.take() {
        m.discard(&mut app.doc);
    }
    same(
        app.doc.model(),
        &after_a,
        "after Ctrl+Z, the pointer and Esc",
    );
    assert_eq!(step_ids(&app), steps_a, "move A's step kept");
    while app.doc.undo_steps().len() > steps_a.len() - 1 {
        app.doc.undo();
    }
    same(
        app.doc.model(),
        &before,
        "undoing move A puts the wall back",
    );
}

#[test]
fn v01_ctrl_z_while_dragging_a_wall_takes_back_only_the_drag() {
    let (mut app, ctx, mut t, before) = drawing_with(crate::planview::Tool::Select);
    let w = pick_south_wall(&mut app);
    move_with_the_tool(
        &mut app,
        &ctx,
        &mut t,
        egui::Pos2::new(500.0, 400.0),
        egui::Pos2::new(560.0, 400.0),
    );
    let after_a = snap(&app);
    let steps_a = step_ids(&app);
    // the wall's middle, now; a drag of the whole wall is a "Move" too
    let mid = match &app.doc.model().get(w).unwrap().kind {
        ElementKind::Wall(wl) => wl.start + (wl.end - wl.start) * 0.3,
        _ => unreachable!(),
    };
    let at = on_screen(&app, mid);
    app.selection.only(w);
    let press = |pressed, pos| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    hover_plan(&mut app, &ctx, &mut t, at);
    plan_frame(&mut app, &ctx, &mut t, vec![press(true, at)]);
    for dx in [10.0, 20.0, 30.0, 40.0] {
        hover_plan(&mut app, &ctx, &mut t, at + egui::vec2(dx, 0.0));
    }
    assert!(app.plan.grip.is_some(), "the wall is being dragged");
    assert_ne!(snap(&app), after_a, "the drag shows the wall moved");
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    same(app.doc.model(), &after_a, "Ctrl+Z during the drag");
    assert_eq!(step_ids(&app), steps_a, "move A's step is still the last");
    assert!(app.plan.grip.is_none(), "the drag ended");
    // the button comes up: nothing more happens
    hover_plan(&mut app, &ctx, &mut t, at + egui::vec2(50.0, 0.0));
    plan_frame(
        &mut app,
        &ctx,
        &mut t,
        vec![press(false, at + egui::vec2(50.0, 0.0))],
    );
    plan_frame(&mut app, &ctx, &mut t, vec![]);
    same(app.doc.model(), &after_a, "after the button comes up");
    assert_eq!(step_ids(&app), steps_a, "history untouched");
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    same(app.doc.model(), &before, "then Ctrl+Z undoes move A");
}

#[test]
fn v01_a_duplicate_cancelled_never_takes_an_earlier_duplicate_with_it() {
    let mut app = house();
    let w = pick_south_wall(&mut app);
    let start = |doc: &mut creabim_core::Document| {
        crate::planview::start_modal(
            doc,
            (crate::modal::Mode::Move, true),
            &[w],
            None,
            crate::modal::Space::Model,
            None,
            egui::Pos2::new(500.0, 400.0),
            crate::modal::Pivot::Median,
            None,
            crate::modal::Flow::Keys,
            crate::modal::Target::Object,
            None,
        )
        .map(|(m, _)| m)
        .unwrap()
    };
    // duplicate A: its copy made, then the duplicate kept where it is
    let a = start(&mut app.doc);
    drop(a);
    let after_a = snap(&app);
    let steps_a = step_ids(&app);
    assert!(
        app.doc
            .undo_label()
            .is_some_and(|l| l.starts_with("Duplicate")),
        "duplicate A is a step: {:?}",
        app.doc.undo_label()
    );
    // duplicate B, the same wall, the same name; its step taken back
    // meanwhile (by anything), then B cancelled
    let mut b = start(&mut app.doc);
    assert!(app.doc.undo());
    b.cancel(&mut app.doc);
    same(app.doc.model(), &after_a, "duplicate B cancelled");
    assert_eq!(step_ids(&app), steps_a, "duplicate A's step kept");
}

/// Two projects open: this one, and a copy of it saved under another name
/// (same element ids, same names) where the south wall was moved.
fn two_projects() -> (
    CreaBimApp,
    egui::Context,
    f64,
    Model,
    Vec<creabim_core::StepId>,
) {
    let (mut app, ctx, mut t, _) = drawing_with(crate::planview::Tool::Select);
    app.multi = true;
    // the copy: the same model, opened as a second project
    let copy = app.doc.model().clone();
    app.load_model(copy);
    let plan = app
        .doc
        .model()
        .iter()
        .find(|e| e.name == "Ground Floor plan")
        .map(|e| e.id)
        .unwrap();
    app.active_view = Some(plan);
    plan_frame(&mut app, &ctx, &mut t, vec![]);
    pick_south_wall(&mut app);
    move_with_the_tool(
        &mut app,
        &ctx,
        &mut t,
        egui::Pos2::new(500.0, 400.0),
        egui::Pos2::new(560.0, 400.0),
    );
    assert!(
        app.doc.undo_label().is_some_and(|l| l.starts_with("Move")),
        "the copy has its move: {:?}",
        app.doc.undo_label()
    );
    let copy_model = snap(&app);
    let copy_steps = step_ids(&app);
    // back to the first project
    app.switch_project(1);
    let plan = app
        .doc
        .model()
        .iter()
        .find(|e| e.name == "Ground Floor plan")
        .map(|e| e.id)
        .unwrap();
    app.active_view = Some(plan);
    plan_frame(&mut app, &ctx, &mut t, vec![]);
    (app, ctx, t, copy_model, copy_steps)
}

/// A move started in the first project, following the pointer.
fn move_in_progress(app: &mut CreaBimApp, ctx: &egui::Context, t: &mut f64) -> Model {
    pick_south_wall(app);
    let before = snap(app);
    app.run_command(ctx, crate::commands::Command::Move, None);
    click_plan(app, ctx, t, egui::Pos2::new(500.0, 450.0));
    hover_plan(app, ctx, t, egui::Pos2::new(530.0, 450.0));
    assert!(app.plan.modal.is_some(), "a move is in progress");
    assert_ne!(snap(app), before, "and shows the wall moved");
    before
}

/// After a switch to the copy: the copy is exactly as left, with its
/// history, and nothing of the first project's move goes on in it.
fn the_copy_is_untouched(
    app: &mut CreaBimApp,
    ctx: &egui::Context,
    t: &mut f64,
    copy_model: &Model,
    copy_steps: &[creabim_core::StepId],
    how: &str,
) {
    assert!(
        app.plan.modal.is_none(),
        "{how}: no move goes on in the copy"
    );
    for dx in [0.0, 15.0, 30.0] {
        hover_plan(app, ctx, t, egui::Pos2::new(540.0 + dx, 450.0));
    }
    escape_in_plan(app, ctx, t);
    same(app.doc.model(), copy_model, &format!("{how}: the copy"));
    assert_eq!(step_ids(app), copy_steps, "{how}: the copy's history");
}

#[test]
fn v10_switching_project_during_a_move_leaves_the_other_project_untouched() {
    for how in ["Ctrl+Tab", "Window menu"] {
        let (mut app, ctx, mut t, copy_model, copy_steps) = two_projects();
        let first_before = move_in_progress(&mut app, &ctx, &mut t);
        let first_steps = step_ids(&app);
        if how == "Ctrl+Tab" {
            app.next_project();
        } else {
            app.switch_project(1);
        }
        // the 3D is built again from the project now shown, not kept from
        // the one before (data only: the drawing on screen is not checked)
        assert!(
            app.geometry_key.is_none(),
            "{how}: no 3D kept from the other project"
        );
        assert!(app.scene.is_empty(), "{how}: its scene is built afresh");
        the_copy_is_untouched(&mut app, &ctx, &mut t, &copy_model, &copy_steps, how);
        // and back: the first project is as it was before the move began
        app.switch_project(1);
        assert!(app.plan.modal.is_none(), "{how}: the move did not survive");
        same(
            app.doc.model(),
            &first_before,
            &format!("{how}: the first project"),
        );
        assert!(
            step_ids(&app).len() <= first_steps.len() && first_steps.starts_with(&step_ids(&app)),
            "{how}: none of the first project's confirmed steps lost"
        );
    }
}

#[test]
fn v10_closing_or_opening_a_project_during_a_move_ends_the_move() {
    // closing: the next project comes in untouched
    let (mut app, ctx, mut t, copy_model, copy_steps) = two_projects();
    move_in_progress(&mut app, &ctx, &mut t);
    assert!(app.close_project(), "the first project closes");
    the_copy_is_untouched(&mut app, &ctx, &mut t, &copy_model, &copy_steps, "Close");
    // opening another project (one project at a time): nothing of the move
    let (mut app, ctx, mut t, _) = drawing_with(crate::planview::Tool::Select);
    move_in_progress(&mut app, &ctx, &mut t);
    app.load_model(creabim_core::starter::starter_house());
    assert!(app.plan.modal.is_none(), "Open: the move ended");
    let opened = snap(&app);
    for dx in [0.0, 15.0] {
        hover_plan(&mut app, &ctx, &mut t, egui::Pos2::new(540.0 + dx, 450.0));
    }
    same(app.doc.model(), &opened, "Open: the project opened");
    assert!(app.doc.undo_steps().is_empty(), "Open: a fresh history");
}

#[test]
fn v10_a_project_switch_during_a_drag_ends_the_drag() {
    let (mut app, ctx, mut t, copy_model, copy_steps) = two_projects();
    let w = pick_south_wall(&mut app);
    let mid = match &app.doc.model().get(w).unwrap().kind {
        ElementKind::Wall(wl) => wl.start + (wl.end - wl.start) * 0.3,
        _ => unreachable!(),
    };
    let at = on_screen(&app, mid);
    let before = snap(&app);
    hover_plan(&mut app, &ctx, &mut t, at);
    plan_frame(
        &mut app,
        &ctx,
        &mut t,
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    for dx in [10.0, 20.0, 30.0] {
        hover_plan(&mut app, &ctx, &mut t, at + egui::vec2(dx, 0.0));
    }
    assert!(app.plan.grip.is_some(), "the wall is being dragged");
    app.next_project();
    assert!(app.plan.grip.is_none(), "the drag ended with the switch");
    the_copy_is_untouched(&mut app, &ctx, &mut t, &copy_model, &copy_steps, "drag");
    app.switch_project(1);
    same(
        app.doc.model(),
        &before,
        "the first project, as before the drag",
    );
}

#[test]
fn v11_no_point_or_pick_goes_from_one_project_to_another() {
    let (mut app, ctx, mut t, copy_model, copy_steps) = two_projects();
    // a wall chain under way in the first project, a point waiting
    let i = crate::planview::Tool::ALL
        .iter()
        .position(|x| *x == crate::planview::Tool::Wall)
        .unwrap();
    app.run_action(&ctx, &format!("tool:{i}"));
    click_plan(&mut app, &ctx, &mut t, egui::Pos2::new(400.0, 380.0));
    click_plan(&mut app, &ctx, &mut t, egui::Pos2::new(600.0, 380.0));
    assert!(app.plan.pending.is_some() && !app.plan.chain_steps.is_empty());
    app.next_project();
    assert!(app.plan.pending.is_none(), "no point waiting in the copy");
    assert!(app.plan.chain_steps.is_empty(), "no chain in the copy");
    assert!(!app.plan.has_input(), "nothing part way in the copy");
    assert!(
        app.plan.cmd_pick.is_none(),
        "no command's picks in the copy"
    );
    assert!(app.pending_3d.is_none(), "no 3D point in the copy");
    // a click with the Wall tool only starts a wall: none is drawn
    click_plan(&mut app, &ctx, &mut t, egui::Pos2::new(600.0, 560.0));
    same(app.doc.model(), &copy_model, "the copy after one click");
    assert_eq!(step_ids(&app), copy_steps, "the copy's history");
    // a move armed in one project does not start in the other
    app.switch_project(1);
    pick_south_wall(&mut app);
    app.run_command(&ctx, crate::commands::Command::Move, None);
    app.next_project();
    assert!(app.plan.armed.is_none(), "no armed move in the copy");
}

/// V-01 with Rotate: centre, reference, new direction. Ctrl+Z steps back a
/// click at a time and never takes the rotate confirmed before.
#[test]
fn v01_ctrl_z_during_a_rotate_steps_back_its_clicks_and_keeps_the_rotate_before() {
    let (mut app, ctx, mut t, before) = drawing_with(crate::planview::Tool::Select);
    pick_south_wall(&mut app);
    let rotate = |app: &mut CreaBimApp, t: &mut f64, finish: bool| {
        app.run_command(&ctx, crate::commands::Command::Rotate, None);
        click_plan(app, &ctx, t, egui::Pos2::new(500.0, 400.0));
        click_plan(app, &ctx, t, egui::Pos2::new(600.0, 400.0));
        hover_plan(app, &ctx, t, egui::Pos2::new(600.0, 430.0));
        hover_plan(app, &ctx, t, egui::Pos2::new(600.0, 440.0));
        if finish {
            click_plan(app, &ctx, t, egui::Pos2::new(600.0, 440.0));
        }
    };
    rotate(&mut app, &mut t, true);
    assert!(app.plan.modal.is_none(), "rotate A is finished");
    let after_a = snap(&app);
    let steps_a = step_ids(&app);
    assert_ne!(after_a, before, "rotate A turned the wall");
    rotate(&mut app, &mut t, false);
    assert!(app.plan.modal.is_some(), "rotate B follows the pointer");
    assert_ne!(snap(&app), after_a, "and shows the wall turned");
    // three clicks of rotate B to take back, one Ctrl+Z each
    for k in 0..3 {
        app.run_command(&ctx, crate::commands::Command::Undo, None);
        same(
            app.doc.model(),
            &after_a,
            &format!("Ctrl+Z {} in rotate B", k + 1),
        );
        assert_eq!(step_ids(&app), steps_a, "rotate A's step kept ({})", k + 1);
    }
    assert!(app.plan.modal.is_none(), "rotate B over");
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    same(app.doc.model(), &before, "then Ctrl+Z undoes rotate A");
}
