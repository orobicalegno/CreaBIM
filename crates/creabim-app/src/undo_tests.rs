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

#[test]
#[ignore = "known bug (G4): Cancel in a settings window puts back settings changed elsewhere, see docs/regressioni.md"]
fn known_bug_cancel_in_a_settings_window_keeps_settings_changed_elsewhere() {
    for w in WINDOWS.iter().filter(|w| !w.by_close) {
        let mut r = Run::new();
        let before = r.app.shell.settings.render_3d;
        let other = if before == crate::render3d::RenderMode::Maquette {
            crate::render3d::RenderMode::Shaded
        } else {
            crate::render3d::RenderMode::Maquette
        };
        r.open(w);
        // while it is open, the 3D style is changed from the Render menu
        r.app.shell.settings.render_3d = other;
        r.close(w, false);
        assert_eq!(
            r.app.shell.settings.render_3d, other,
            "{}: Cancel keeps a setting the window never touched",
            w.key
        );
    }
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
