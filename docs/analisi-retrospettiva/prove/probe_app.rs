
// =====================================================================
// PROVE ESPLORATIVE (analisi retrospettiva). Non fanno parte del progetto.
// =====================================================================

fn wall_of(app: &CreaBimApp) -> creabim_core::Element {
    let id = first(app, |k| matches!(k, ElementKind::Wall(_)));
    app.doc.model().get(id).unwrap().clone()
}
fn shifted(e: &creabim_core::Element, dx: f64) -> creabim_core::Element {
    let mut e = e.clone();
    if let ElementKind::Wall(w) = &mut e.kind {
        w.start.x += dx;
        w.end.x += dx;
    }
    e
}

/// Q8: Ctrl+Z durante uno spostamento dal vivo, dopo uno spostamento
/// confermato con lo stesso nome: il primo resta?
#[test]
fn probe_q8_undo_during_a_live_move_after_a_confirmed_move() {
    let mut app = house();
    let ctx = egui::Context::default();
    let w0 = wall_of(&app);
    // spostamento A, confermato (LiveEdit aggiornato e poi lasciato)
    let mut a = crate::modal::LiveEdit::new("Move Wall", vec![w0.clone()]);
    a.update(&mut app.doc, vec![shifted(&w0, 1.0)]).unwrap();
    drop(a);
    let after_a = snap(&app);
    let steps_a = app.doc.undo_steps().len();
    // spostamento B, dal vivo
    let w1 = app.doc.model().get(w0.id).unwrap().clone();
    let mut b = crate::modal::LiveEdit::new("Move Wall", vec![w1.clone()]);
    b.update(&mut app.doc, vec![shifted(&w1, 0.5)]).unwrap();
    // Ctrl+Z nel mezzo
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    let after_undo = snap(&app);
    // il puntatore si muove ancora: il fotogramma successivo
    b.update(&mut app.doc, vec![shifted(&w1, 0.7)]).unwrap();
    // poi Esc: lo spostamento B viene annullato
    b.cancel(&mut app.doc);
    let now = snap(&app);
    println!(
        "Q8: passi dopo A={steps_a}, ora={}; redo={}; A dopo Ctrl+Z presente={}; ora uguale a dopo-A={}; ora uguale a dopo-Ctrl+Z={}",
        app.doc.undo_steps().len(),
        app.doc.redo_steps().len(),
        after_undo == after_a,
        now == after_a,
        now == after_undo
    );
    let x = |m: &Model| match &m.get(w0.id).unwrap().kind { ElementKind::Wall(w) => w.start.x - match &w0.kind { ElementKind::Wall(w) => w.start.x, _ => 0.0 }, _ => 0.0 };
    println!("Q8: spostamento del muro rispetto all'origine: dopo A={:.2}, dopo Ctrl+Z={:.2}, ora={:.2}", x(&after_a), x(&after_undo), x(&now));
    let labels: Vec<_> = app.doc.undo_steps().iter().rev().take(3).map(|s| s.tx.label.clone()).collect();
    println!("Q8: ultimi passi: {labels:?}");
}

/// Q1: Cancel nella finestra Impostazioni dopo che la finestra ha cambiato
/// tema e stile delle maniglie: valore memorizzato e stato applicato
/// coincidono?
#[test]
fn probe_q1_settings_cancel_and_applied_state() {
    let w = WINDOWS.iter().find(|w| w.key == "settings").unwrap();
    let mut r = Run::new();
    let theme0 = r.app.shell.theme;
    let grips0 = r.app.shell.settings.grips;
    r.open(w);
    let ctx = r.ctx.clone();
    let other_theme = theme0.next();
    r.app.in_dialog("settings", |a| {
        a.set_theme(&ctx, other_theme);
        let g = crate::handles::GripStyle::ALL.iter().copied().find(|g| *g != grips0).unwrap();
        a.shell.settings.grips = g;
        crate::handles::set_grip_style(g);
    });
    r.close(w, false);
    println!(
        "Q1: tema applicato={:?} memorizzato={:?} (all'apertura {:?}); maniglie applicate={:?} memorizzate={:?} (all'apertura {:?})",
        r.app.shell.theme, r.app.shell.settings.theme, theme0, crate::handles::grip_style(), r.app.shell.settings.grips, grips0
    );
}

/// Q2: Ripristina mentre il primo punto di un muro e' in attesa.
#[test]
fn probe_q2_redo_with_a_wall_part_way() {
    let (mut app, ctx, mut t, _before) = drawing_with(crate::planview::Tool::Wall);
    // annulla il lavoro precedente: c'e' qualcosa da ripristinare
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    let undone = snap(&app);
    click_plan(&mut app, &ctx, &mut t, egui::Pos2::new(400.0, 380.0));
    let pending = app.plan.pending;
    app.run_command(&ctx, crate::commands::Command::Redo, None);
    println!(
        "Q2: punto in attesa prima={:?} dopo Ctrl+Y={:?}; progetto cambiato da Ctrl+Y={}; stato={}",
        pending, app.plan.pending, snap(&app) != undone, app.status
    );
}

/// Q4: Canc, Annulla, Ripristina e la selezione.
#[test]
fn probe_q4_delete_undo_redo_selection() {
    let mut app = house();
    let ctx = egui::Context::default();
    let w = first(&app, |k| matches!(k, ElementKind::Wall(_)));
    let before = snap(&app);
    app.selection.only(w);
    app.run_command(&ctx, crate::commands::Command::Delete, None);
    let deleted = snap(&app);
    let sel_after_delete = app.selection.all();
    app.run_command(&ctx, crate::commands::Command::Undo, None);
    let back_equal = snap(&app) == before;
    let sel_after_undo = app.selection.all();
    app.run_command(&ctx, crate::commands::Command::Redo, None);
    let redo_equal = snap(&app) == deleted;
    let sel_after_redo = app.selection.all();
    let stale = sel_after_redo.iter().filter(|id| app.doc.model().get(**id).is_none()).count();
    println!(
        "Q4: removed={} sel dopo Canc={:?} undo identico={back_equal} sel dopo undo={:?} redo identico={redo_equal} sel dopo redo={:?} selezionati inesistenti={stale}",
        before.len() - deleted.len(), sel_after_delete, sel_after_undo, sel_after_redo
    );
}
