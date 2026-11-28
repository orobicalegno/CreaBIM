//! Banco di prova delle prestazioni interattive (analisi; NON nel progetto).
use super::*;
use creabim_core::Model;
use std::time::Instant;

fn starter() -> CreaBimApp {
    let mut app = test_app();
    app.load_model(creabim_core::starter::starter_house());
    app
}

struct Bench {
    app: CreaBimApp,
    ctx: egui::Context,
    t: f64,
    rect: Rect,
    ppp: f32,
}

impl Bench {
    fn new(mut app: CreaBimApp) -> Self {
        let ctx = egui::Context::default();
        let plan = app.doc.model().iter().find(|e| e.name == "Ground Floor plan").map(|e| e.id).unwrap();
        app.active_view = Some(plan);
        crate::units::set(creabim_core::Units::metric());
        let (w, h, ppp) = std::env::var("BENCH_SCREEN").ok().and_then(|s| { let v: Vec<f32> = s.split('x').filter_map(|x| x.parse().ok()).collect(); (v.len() == 3).then(|| (v[0], v[1], v[2])) }).unwrap_or((1600.0, 1000.0, 1.0));
        ctx.set_pixels_per_point(ppp);
        let mut b = Bench { app, ctx, t: 0.0, rect: Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w / ppp, h / ppp)), ppp };
        for _ in 0..3 { b.frame(vec![]); }
        b
    }
    /// One frame: (ui ms, tessellation ms, vertices)
    fn frame(&mut self, events: Vec<egui::Event>) -> (f64, f64, usize) {
        self.t += 1.0 / 60.0;
        let mut raw = egui::RawInput { screen_rect: Some(self.rect), events, time: Some(self.t), max_texture_side: Some(8192), ..Default::default() };
        raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(self.ppp);
        let rect = self.rect;
        let app = &mut self.app;
        let a = Instant::now();
        let mut out = self.ctx.run_ui(raw, |ui| app.show_plan(ui, rect, Color32::WHITE));
        let ui_ms = a.elapsed().as_secs_f64() * 1000.0;
        let b = Instant::now();
        let prims = self.ctx.tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
        let tess_ms = b.elapsed().as_secs_f64() * 1000.0;
        let verts = prims.iter().map(|p| match &p.primitive { egui::epaint::Primitive::Mesh(m) => m.vertices.len(), _ => 0 }).sum();
        let px: usize = out.textures_delta.set.iter().flat_map(|(_, ds)| ds.iter()).map(|d| d.image.width() * d.image.height()).sum();
        if std::env::var_os("BENCH_TEX").is_some() && px > 0 { println!("TEX aggiornamento atlante/texture: {px} pixel, {} texture, ui {ui_ms:.1} ms", out.textures_delta.set.len()); }
        out.textures_delta.clear();
        (ui_ms, tess_ms, verts)
    }
    fn hover(&mut self, p: egui::Pos2) -> (f64, f64, usize) { self.frame(vec![egui::Event::PointerMoved(p)]) }
    fn click(&mut self, p: egui::Pos2) {
        let b = |pressed| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        self.hover(p); self.frame(vec![b(true)]); self.frame(vec![b(false)]); self.frame(vec![]);
    }
}

fn stats(name: &str, xs: &[(f64, f64, usize)]) {
    let mut ui: Vec<f64> = xs.iter().map(|x| x.0).collect();
    let mut tot: Vec<f64> = xs.iter().map(|x| x.0 + x.1).collect();
    ui.sort_by(|a, b| a.partial_cmp(b).unwrap());
    tot.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |v: &Vec<f64>, p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
    let tess: f64 = xs.iter().map(|x| x.1).sum::<f64>() / xs.len() as f64;
    let verts = xs.iter().map(|x| x.2).max().unwrap_or(0);
    println!("BENCH {name:<34} frames={:3} ui mediana={:7.2} p95={:7.2} max={:7.2} | tess media={:6.2} | totale mediana={:7.2} p95={:7.2} | vertici={verts}", xs.len(), q(&ui, 0.5), q(&ui, 0.95), q(&ui, 1.0), tess, q(&tot, 0.5), q(&tot, 0.95));
}

fn path(n: usize, y: f32) -> Vec<egui::Pos2> { (0..n).map(|i| egui::Pos2::new(600.0 + i as f32 * 3.0, y + (i % 7) as f32)).collect() }

fn add_lines(app: &mut CreaBimApp, n: usize) -> Vec<ElementId> {
    let v = app.active_view.unwrap();
    let mut tx = creabim_core::Transaction::new("lines");
    let mut ids = vec![];
    for i in 0..n {
        let x = (i % 100) as f64 * 0.15; let y = (i / 100) as f64 * 0.15 - 3.0;
        let e = creabim_core::Element::new("Line", ElementKind::Detail(creabim_core::DetailItem::new(v, creabim_core::DetailKind::draft(creabim_core::DraftShape::Line), vec![DVec2::new(x, y), DVec2::new(x + 0.1, y + 0.05)])));
        ids.push(e.id); tx = tx.insert(e);
    }
    app.doc.commit(tx).unwrap();
    ids
}

fn run_move(name: &str, mut b: Bench, ids: Vec<ElementId>) {
    // baseline: pointer moving, nothing in progress
    let idle: Vec<_> = path(40, 470.0).into_iter().map(|p| b.hover(p)).collect();
    stats(&format!("{name}: solo puntatore"), &idle);
    b.app.selection.clear();
    for id in &ids { b.app.selection.extend(vec![*id], false); }
    b.app.run_command(&b.ctx.clone(), crate::commands::Command::Move, None);
    b.click(egui::Pos2::new(600.0, 500.0));
    let mv: Vec<_> = path(60, 500.0).into_iter().map(|p| b.hover(p)).collect();
    assert!(b.app.plan.modal.is_some(), "{name}: move in progress");
    stats(&format!("{name}: Sposta in corso"), &mv);
    let still: Vec<_> = (0..20).map(|_| b.frame(vec![])).collect();
    stats(&format!("{name}: Sposta, mouse fermo"), &still);
}

#[test]
fn bench_move_one_wall() {
    let mut b = Bench::new(starter());
    let w = b.app.doc.model().iter().find(|e| e.name == "GF South").map(|e| e.id).unwrap();
    let _ = &mut b;
    run_move("casa, 1 muro", b, vec![w]);
}

#[test]
fn bench_move_all_ground_floor_walls() {
    let b = Bench::new(starter());
    let ids: Vec<_> = b.app.doc.model().iter().filter(|e| e.name.starts_with("GF ") && matches!(e.kind, ElementKind::Wall(_))).map(|e| e.id).collect();
    let n = ids.len();
    run_move(&format!("casa, {n} muri PT"), b, ids);
}

#[test]
fn bench_move_lines() {
    for n in [10usize, 100, 1000, 10000] {
        let mut b = Bench::new(starter());
        let ids = add_lines(&mut b.app, n);
        run_move(&format!("casa + {n} linee"), b, ids);
    }
}

#[test]
fn bench_move_lines_empty_model() {
    for n in [10usize, 1000, 10000] {
        let mut app = test_app();
        app.load_model(creabim_core::sample::sample_house());
        let mut b = Bench::new(app);
        let ids = add_lines(&mut b.app, n);
        run_move(&format!("piccola + {n} linee"), b, ids);
    }
}

#[test]
fn bench_live_edit_alone() {
    for n in [1usize, 10, 100, 1000, 10000] {
        let mut b = Bench::new(starter());
        let ids = add_lines(&mut b.app, n);
        let originals: Vec<_> = ids.iter().map(|id| b.app.doc.model().get(*id).unwrap().clone()).collect();
        let mut e = crate::modal::LiveEdit::new("Move", originals.clone());
        let mut times = vec![];
        for k in 0..20 {
            let moved: Vec<_> = originals.iter().map(|o| { let mut o = o.clone(); if let ElementKind::Detail(d) = &mut o.kind { for p in &mut d.points { p.x += 0.01 * k as f64 + 0.01; } } o }).collect();
            let a = Instant::now();
            e.update(&mut b.app.doc, moved).unwrap();
            times.push(a.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // the plan drawing rebuilt after one such change
        let v = b.app.active_view.unwrap();
        let a = Instant::now();
        let _ = creabim_geom::section::view_drawing_with(b.app.doc.model(), v, false);
        let draw = a.elapsed().as_secs_f64() * 1000.0;
        let a = Instant::now();
        let _ = creabim_geom::shared_scene(b.app.doc.model());
        let scene = a.elapsed().as_secs_f64() * 1000.0;
        println!("BENCH LiveEdit.update {n:>5} linee: mediana {:7.2} ms max {:7.2} | pianta rigenerata {draw:7.2} ms | scena 3D {scene:7.2} ms", times[times.len()/2], times[times.len()-1]);
    }
}

#[test]
fn bench_pan_and_wheel() {
    let mut b = Bench::new(starter());
    let mid = |pressed, pos| egui::Event::PointerButton { pos, button: egui::PointerButton::Middle, pressed, modifiers: Default::default() };
    let p0 = egui::Pos2::new(800.0, 500.0);
    b.hover(p0); b.frame(vec![mid(true, p0)]);
    let pan: Vec<_> = path(60, 500.0).into_iter().map(|p| b.hover(p)).collect();
    b.frame(vec![mid(false, egui::Pos2::new(780.0, 500.0))]);
    stats("casa: pan (tasto centrale)", &pan);
    let wheel: Vec<_> = (0..40).map(|i| b.frame(vec![egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: egui::vec2(0.0, if i < 20 { 1.0 } else { -1.0 }), modifiers: Default::default(), phase: egui::TouchPhase::Move }])).collect();
    stats("casa: rotellina", &wheel);
}

#[test]
fn bench_line_tool_rubber_band() {
    let mut b = Bench::new(starter());
    let i = crate::planview::Tool::ALL.iter().position(|x| *x == crate::planview::Tool::Wall).unwrap();
    b.app.run_action(&b.ctx.clone(), &format!("tool:{i}"));
    b.click(egui::Pos2::new(600.0, 450.0));
    let rb: Vec<_> = path(60, 480.0).into_iter().map(|p| b.hover(p)).collect();
    stats("casa: muro, elastico al puntatore", &rb);
}

#[test]
fn bench_wall_drag() {
    let mut b = Bench::new(starter());
    let w = b.app.doc.model().iter().find(|e| e.name == "GF South").map(|e| e.id).unwrap();
    b.app.selection.only(w);
    let mid = match &b.app.doc.model().get(w).unwrap().kind { ElementKind::Wall(x) => x.start + (x.end - x.start) * 0.3, _ => unreachable!() };
    let view = b.app.active_view.unwrap();
    let (center, scale) = b.app.plan.cameras[&view];
    let at = crate::planview::Xf { rect: b.rect, center, scale, rot: 0.0 }.to_screen(mid);
    b.hover(at);
    b.frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
    let dr: Vec<_> = (1..60).map(|i| b.hover(at + egui::vec2(i as f32 * 2.0, 0.0))).collect();
    assert!(b.app.plan.grip.is_some());
    stats("casa: trascina un muro", &dr);
}

#[test]
fn bench_box_select() {
    let mut b = Bench::new(starter());
    let p0 = egui::Pos2::new(300.0, 200.0);
    b.hover(p0);
    b.frame(vec![egui::Event::PointerButton { pos: p0, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
    let bx: Vec<_> = (1..60).map(|i| b.hover(p0 + egui::vec2(i as f32 * 12.0, i as f32 * 8.0))).collect();
    stats("casa: selezione a finestra", &bx);
}

/// Il fotogramma completo (pannelli, proprieta', elenco, area delle viste),
/// con il modello invariato e poi cambiato a ogni fotogramma (come Sposta).
#[test]
fn bench_full_frame() {
    let _ = gtk::init();
    for with_3d in [false, true] {
        let mut app = starter();
        let ctx = egui::Context::default();
        let mut frame = eframe::Frame::_new_kittest();
        let plan = app.doc.model().iter().find(|e| e.name == "Ground Floor plan").map(|e| e.id).unwrap();
        app.active_view = Some(plan);
        app.shell.settings.single_view = !with_3d;
        let w = app.doc.model().iter().find(|e| e.name == "GF South").map(|e| e.id).unwrap();
        app.selection.only(w);
        let mut t = 0.0;
        let mut run = |app: &mut CreaBimApp, ev: Vec<egui::Event>| -> (f64, f64, usize) {
            t += 1.0 / 60.0;
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0))), events: ev, time: Some(t), max_texture_side: Some(8192), ..Default::default() };
            let a = Instant::now();
            let mut out = ctx.run_ui(raw, |ui| app.frame_body(ui, &mut frame));
            let ui_ms = a.elapsed().as_secs_f64() * 1000.0;
            let b = Instant::now();
            let prims = ctx.tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
            let tess = b.elapsed().as_secs_f64() * 1000.0;
            out.textures_delta.clear();
            (ui_ms, tess, prims.len())
        };
        for _ in 0..5 { run(&mut app, vec![]); }
        let still: Vec<_> = (0..30).map(|i| run(&mut app, vec![egui::Event::PointerMoved(egui::Pos2::new(700.0 + i as f32, 500.0))])).collect();
        stats(&format!("finestra completa{}: modello fermo", if with_3d { " + 3D" } else { "" }), &still);
        let o = app.doc.model().get(w).unwrap().clone();
        let mut e = crate::modal::LiveEdit::new("Move GF South", vec![o.clone()]);
        let moving: Vec<_> = (0..30).map(|i| {
            let mut n = o.clone();
            if let ElementKind::Wall(x) = &mut n.kind { x.start.y += 0.01 * (i + 1) as f64; x.end.y += 0.01 * (i + 1) as f64; }
            e.update(&mut app.doc, vec![n]).unwrap();
            run(&mut app, vec![egui::Event::PointerMoved(egui::Pos2::new(700.0 + i as f32, 500.0))])
        }).collect();
        stats(&format!("finestra completa{}: modello cambia ogni fotogramma", if with_3d { " + 3D" } else { "" }), &moving);
    }
}

/// Costo della selezione contro costo del resto: 10.000 linee nella pianta,
/// se ne spostano 1, 10, 100, 1.000, 10.000; snap attivi e disattivati.
#[test]
fn bench_selected_vs_rest() {
    for snaps in [false, true] {
        let ks: Vec<usize> = std::env::var("BENCH_K").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or(vec![1, 10, 100, 1000, 10000]);
        for k in ks {
            let mut b = Bench::new(starter());
            let ids = add_lines(&mut b.app, 10000);
            b.app.plan.snap.on = snaps;
            b.app.selection.clear();
            b.app.selection.extend(ids[..k].to_vec(), false);
            b.app.run_command(&b.ctx.clone(), crate::commands::Command::Move, None);
            b.click(egui::Pos2::new(600.0, 500.0));
            let mv: Vec<_> = path(40, 500.0).into_iter().map(|p| b.hover(p)).collect();
            stats(&format!("10.000 linee, sposta {k:>5}, snap {}", if snaps { "si" } else { "no" }), &mv);
        }
    }
}

/// Copia completa del modello (fatta per i lavori in sottofondo) e impronta
/// del disegno (calcolata a ogni fotogramma per la cache degli snap).
#[test]
fn bench_model_clone_and_print() {
    for n in [0usize, 10000] {
        let mut b = Bench::new(starter());
        add_lines(&mut b.app, n);
        let m = b.app.doc.model();
        let a = Instant::now();
        for _ in 0..10 { std::hint::black_box(m.clone()); }
        let clone = a.elapsed().as_secs_f64() * 100.0;
        let v = b.app.active_view.unwrap();
        let d = creabim_geom::section::view_drawing_with(m, v, false).unwrap();
        let items = d.items.len();
        let a = Instant::now();
        let mut h = 0u64;
        for _ in 0..10 { h ^= crate::planview::drawing_print_for_bench(&d.items); }
        std::hint::black_box(h);
        let print = a.elapsed().as_secs_f64() * 100.0;
        println!("BENCH impronta del disegno ({items} elementi grafici): {print:.3} ms");
        println!("BENCH casa + {n} linee ({} elementi): copia del modello {clone:.2} ms; elementi disegnati {items}", m.iter().count());
    }
}
