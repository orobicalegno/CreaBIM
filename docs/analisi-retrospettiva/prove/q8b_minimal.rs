use super::*;
use creabim_core::Model;
fn first(app: &CreaBimApp, pick: impl Fn(&ElementKind) -> bool) -> ElementId {
    app.doc
        .model()
        .iter()
        .find(|e| pick(&e.kind))
        .map(|e| e.id)
        .expect("the sample house has one")
}
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
fn house() -> CreaBimApp {
    let mut app = CreaBimApp::blank(None);
    app.load_model(creabim_core::starter::starter_house());
    app
}
fn snap(app: &CreaBimApp) -> Model {
    app.doc.model().clone()
}
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
/// Q8b: lo stesso nel flusso reale: Sposta, punto base, punto finale;
/// di nuovo Sposta, punto base, puntatore che si muove, Ctrl+Z, puntatore,
/// Esc.
#[test]
fn probe_q8b_real_move_flow() {
    let (mut app, ctx, mut t, _) = drawing_with(crate::planview::Tool::Select);
    let w = first(&app, |k| matches!(k, ElementKind::Wall(_)));
    let mv = |app: &mut CreaBimApp, t: &mut f64, p: egui::Pos2| plan_frame(app, &ctx, t, vec![egui::Event::PointerMoved(p)]);
    let wx = |app: &CreaBimApp| match &app.doc.model().get(w).unwrap().kind { ElementKind::Wall(w) => w.start, _ => DVec2::ZERO };
    let x0 = wx(&app);
    let s0 = app.doc.undo_steps().len();
    app.selection.only(w);
    app.run_command(&ctx, crate::commands::Command::Move, None);
    click_plan(&mut app, &ctx, &mut t, egui::Pos2::new(500.0, 400.0));
    mv(&mut app, &mut t, egui::Pos2::new(540.0, 400.0));
    click_plan(&mut app, &ctx, &mut t, egui::Pos2::new(560.0, 400.0));
    let after_a = snap(&app);
    let xa = wx(&app);
    let labels = |app: &CreaBimApp| app.doc.undo_steps().iter().skip(s0).map(|s| s.tx.label.clone()).collect::<Vec<_>>();
    println!("Q8b: dopo A: muro {:?} -> {:?}; passi nuovi {:?}; modal={}; stato={}", x0, xa, labels(&app), app.plan.modal.is_some(), app.status);
    app.selection.only(w);
    app.run_command(&ctx, crate::commands::Command::Move, None);
    click_plan(&mut app, &ctx, &mut t, egui::Pos2::new(500.0, 450.0));
    mv(&mut app, &mut t, egui::Pos2::new(520.0, 450.0));
    mv(&mut app, &mut t, egui::Pos2::new(525.0, 450.0));
    println!("Q8b: B dal vivo: muro {:?}; passi {:?}; modal={}", wx(&app), labels(&app), app.plan.modal.is_some());
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    println!("Q8b: dopo Ctrl+Z: muro {:?}; passi {:?}; redo={}; stato={}", wx(&app), labels(&app), app.doc.redo_steps().len(), app.status);
    mv(&mut app, &mut t, egui::Pos2::new(530.0, 450.0));
    mv(&mut app, &mut t, egui::Pos2::new(535.0, 450.0));
    println!("Q8b: puntatore ancora: muro {:?}; passi {:?}; modal={}", wx(&app), labels(&app), app.plan.modal.is_some());
    let esc = egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
    plan_frame(&mut app, &ctx, &mut t, vec![esc]);
    plan_frame(&mut app, &ctx, &mut t, vec![]);
    println!("Q8b: dopo Esc: muro {:?}; passi {:?}; redo={}; modal={}; uguale a dopo-A={}", wx(&app), labels(&app), app.doc.redo_steps().len(), app.plan.modal.is_some(), snap(&app) == after_a);
    while app.doc.undo_steps().len() > s0 { app.doc.undo(); }
    println!("Q8b: annullando tutto fino a prima di A: muro {:?} (all'inizio {:?})", wx(&app), x0);
}
