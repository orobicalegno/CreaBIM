//! Benchmarks of interactive work (docs/prestazioni): the real plan pane
//! (`show_plan`) driven by real pointer events, frame by frame, on the
//! starter house and on drafting lines added to it.
//!
//! Ignored in ordinary test runs. Run them in an optimised build, one at a
//! time, on a machine left alone:
//!
//! ```text
//! cargo test --profile fast -p creabim-app --bin creabim-app bench_ -- --ignored --nocapture --test-threads=1
//! ```
//!
//! `BENCH_OUT=<file.csv>` appends one row per case; `BENCH_SCREEN=3840x2160x1.5`
//! sets the window (physical pixels, scale); `BENCH_QUICK=1` runs fewer
//! frames. Every case first checks that it does what it says (the command
//! running, its stage, how many elements are selected, that they moved),
//! then warms up, then measures three repetitions.
//!
//! The times are CPU only: the GPU, the presentation and the screen are
//! not part of them (see the diagnostics in the app for those).

use super::*;
use creabim_core::{DetailItem, DetailKind, DraftShape, Element, Transaction};
use std::time::Instant;

/// One frame's cost: the UI (layout, the edit, drawings) and the
/// tessellation of what it painted, in ms, and the vertices painted.
#[derive(Clone, Copy)]
struct Cost {
    ui: f64,
    tess: f64,
    verts: usize,
}

struct Bench {
    app: CreaBimApp,
    ctx: egui::Context,
    t: f64,
    rect: Rect,
    ppp: f32,
}

/// The window the benches draw: from `BENCH_SCREEN` ("3840x2160x1.5":
/// physical pixels and scale), else 1600 × 1000 at 100%.
fn screen() -> (f32, f32, f32) {
    std::env::var("BENCH_SCREEN")
        .ok()
        .and_then(|s| {
            let v: Vec<f32> = s.split('x').filter_map(|x| x.parse().ok()).collect();
            (v.len() == 3).then(|| (v[0], v[1], v[2]))
        })
        .unwrap_or((1600.0, 1000.0, 1.0))
}

thread_local! {
    /// A short run, to check a scenario rather than measure it.
    static CHECK_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn quick() -> bool {
    CHECK_ONLY.with(|c| c.get()) || std::env::var_os("BENCH_QUICK").is_some_and(|v| !v.is_empty())
}

impl Bench {
    /// `model` open, its ground floor plan in the pane.
    fn new(model: creabim_core::Model) -> Self {
        let mut app = test_app();
        app.load_model(model);
        let plan = app
            .doc
            .model()
            .iter()
            .find(|e| e.name == "Ground Floor plan")
            .map(|e| e.id)
            .expect("a ground floor plan");
        app.active_view = Some(plan);
        crate::units::set(creabim_core::Units::metric());
        let (w, h, ppp) = screen();
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(ppp);
        let mut b = Bench {
            app,
            ctx,
            t: 0.0,
            rect: Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w / ppp, h / ppp)),
            ppp,
        };
        for _ in 0..3 {
            b.frame(vec![]);
        }
        b
    }

    fn frame(&mut self, events: Vec<egui::Event>) -> Cost {
        self.t += 1.0 / 60.0;
        let mut raw = egui::RawInput {
            screen_rect: Some(self.rect),
            events,
            time: Some(self.t),
            max_texture_side: Some(8192),
            ..Default::default()
        };
        raw.viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(self.ppp);
        let rect = self.rect;
        let app = &mut self.app;
        let a = Instant::now();
        let mut out = self
            .ctx
            .run_ui(raw, |ui| app.show_plan(ui, rect, Color32::WHITE));
        let ui = a.elapsed().as_secs_f64() * 1000.0;
        let b = Instant::now();
        let prims = self
            .ctx
            .tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
        let tess = b.elapsed().as_secs_f64() * 1000.0;
        let verts = prims
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(m) => m.vertices.len(),
                _ => 0,
            })
            .sum();
        out.textures_delta.clear();
        Cost { ui, tess, verts }
    }

    fn hover(&mut self, p: egui::Pos2) -> Cost {
        self.frame(vec![egui::Event::PointerMoved(p)])
    }

    fn button(&mut self, p: egui::Pos2, b: egui::PointerButton, pressed: bool) -> Cost {
        self.frame(vec![egui::Event::PointerButton {
            pos: p,
            button: b,
            pressed,
            modifiers: Default::default(),
        }])
    }

    fn click(&mut self, p: egui::Pos2) {
        self.hover(p);
        self.button(p, egui::PointerButton::Primary, true);
        self.button(p, egui::PointerButton::Primary, false);
        self.frame(vec![]);
    }

    fn select(&mut self, ids: &[ElementId]) {
        self.app.selection.clear();
        self.app.selection.extend(ids.iter().copied(), false);
        assert_eq!(
            self.app.selection.len(),
            ids.len(),
            "the selection has the size asked for"
        );
    }

    fn on_screen(&self, p: DVec2) -> egui::Pos2 {
        let view = self.app.active_view.unwrap();
        let (center, scale) = self.app.plan.cameras[&view];
        crate::planview::Xf {
            rect: self.rect,
            center,
            scale,
            rot: 0.0,
        }
        .to_screen(p)
    }

    fn elements(&self) -> usize {
        self.app.doc.model().iter().count()
    }
}

/// Points along a short wiggly path to the right of `from`.
fn path(from: egui::Pos2, n: usize) -> Vec<egui::Pos2> {
    (1..=n)
        .map(|i| from + egui::vec2(i as f32 * 3.0, (i % 7) as f32))
        .collect()
}

/// `n` short drafting lines in `app`'s ground floor plan, in a block; their
/// ids, oldest first.
fn add_lines(app: &mut CreaBimApp, n: usize) -> Vec<ElementId> {
    let v = app.active_view.unwrap();
    let mut tx = Transaction::new("bench lines");
    let mut ids = vec![];
    for i in 0..n {
        let x = (i % 100) as f64 * 0.15;
        let y = (i / 100) as f64 * 0.15 - 3.0;
        let e = Element::new(
            "Bench line",
            ElementKind::Detail(DetailItem::new(
                v,
                DetailKind::draft(DraftShape::Line),
                vec![DVec2::new(x, y), DVec2::new(x + 0.1, y + 0.05)],
            )),
        );
        ids.push(e.id);
        tx = tx.insert(e);
    }
    app.doc.commit(tx).unwrap();
    ids
}

fn first_point(app: &CreaBimApp, id: ElementId) -> DVec2 {
    match &app.doc.model().get(id).unwrap().kind {
        ElementKind::Detail(d) => d.points[0],
        ElementKind::Wall(w) => w.start,
        _ => DVec2::ZERO,
    }
}

/// What a case measured, printed and (with `BENCH_OUT`) kept as a row.
struct Report<'a> {
    scenario: &'a str,
    elements: usize,
    selected: usize,
    snaps: bool,
}

fn quantile(v: &[f64], q: f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s[((s.len() - 1) as f64 * q).round() as usize]
}

fn report(r: Report, reps: &[Vec<Cost>]) {
    let all: Vec<Cost> = reps.iter().flatten().copied().collect();
    let total: Vec<f64> = all.iter().map(|c| c.ui + c.tess).collect();
    let rep_medians: Vec<String> = reps
        .iter()
        .map(|r| {
            format!(
                "{:.2}",
                quantile(&r.iter().map(|c| c.ui + c.tess).collect::<Vec<_>>(), 0.5)
            )
        })
        .collect();
    let tess = all.iter().map(|c| c.tess).sum::<f64>() / all.len() as f64;
    let verts = all.iter().map(|c| c.verts).max().unwrap_or(0);
    let (w, h, ppp) = screen();
    println!(
        "BENCH {:<44} elementi={:6} selezionati={:5} snap={:2} | ms per fotogramma: mediana {:8.2} p95 {:8.2} p99 {:8.2} max {:8.2} | ripetizioni {} | tessellazione {:5.2} | vertici {}",
        r.scenario,
        r.elements,
        r.selected,
        if r.snaps { "si" } else { "no" },
        quantile(&total, 0.5),
        quantile(&total, 0.95),
        quantile(&total, 0.99),
        quantile(&total, 1.0),
        rep_medians.join("/"),
        tess,
        verts
    );
    if let Ok(path) = std::env::var("BENCH_OUT") {
        use std::io::Write;
        let new = !std::path::Path::new(&path).exists();
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();
        if new {
            writeln!(f, "commit,scenario,elements,selected,snaps,screen,frames,p50_ms,p95_ms,p99_ms,max_ms,tess_ms,vertices").unwrap();
        }
        writeln!(
            f,
            "{},{},{},{},{},{}x{}@{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{}",
            env!("CREABIM_COMMIT"),
            r.scenario,
            r.elements,
            r.selected,
            r.snaps,
            w,
            h,
            ppp,
            all.len(),
            quantile(&total, 0.5),
            quantile(&total, 0.95),
            quantile(&total, 0.99),
            quantile(&total, 1.0),
            tess,
            verts
        )
        .unwrap();
    }
}

/// Warm up, then three repetitions of `frames` frames of `step` (given the
/// frame's index); fewer frames for slow cases, so each case stays under
/// about 20 s.
fn measure(
    b: &mut Bench,
    frames: usize,
    mut step: impl FnMut(&mut Bench, usize) -> Cost,
) -> Vec<Vec<Cost>> {
    let warm: Vec<Cost> = (0..3).map(|i| step(b, i)).collect();
    let per = warm
        .iter()
        .map(|c| c.ui + c.tess)
        .fold(0.0, f64::max)
        .max(0.1);
    let budget = (20_000.0 / 3.0 / per) as usize;
    let n = frames
        .min(budget)
        .max(3)
        .min(if quick() { 10 } else { usize::MAX });
    let mut k = 3;
    (0..3)
        .map(|_| {
            (0..n)
                .map(|_| {
                    k += 1;
                    step(b, k)
                })
                .collect()
        })
        .collect()
}

/// Move with the Move tool: `ids` selected, a base point, then the pointer
/// following a path. Checks the move is running, in its follow stage,
/// with the selection asked for, and that the elements moved.
fn bench_move(name: &str, mut b: Bench, ids: &[ElementId], snaps: bool) {
    b.app.plan.snap.on = snaps;
    b.select(ids);
    let before = first_point(&b.app, ids[0]);
    let ctx = b.ctx.clone();
    b.app
        .run_command(&ctx, crate::commands::Command::Move, None);
    let base = egui::Pos2::new(600.0, 500.0);
    b.click(base);
    let pts = path(base, 300);
    let reps = measure(&mut b, 40, |b, i| b.hover(pts[i % pts.len()]));
    let m = b.app.plan.modal.as_ref().expect("the move is running");
    assert_eq!(m.mode, crate::modal::Mode::Move, "{name}: Move");
    assert!(m.freezes_snaps(), "{name}: in its follow stage");
    assert_eq!(
        b.app.selection.len(),
        ids.len(),
        "{name}: the selection kept its size"
    );
    assert_ne!(
        first_point(&b.app, ids[0]),
        before,
        "{name}: the elements moved"
    );
    report(
        Report {
            scenario: name,
            elements: b.elements(),
            selected: ids.len(),
            snaps,
        },
        &reps,
    );
}

/// Move: 1 to 10,000 lines selected, in a project that holds just them
/// (selection = project) and in one with 10,000 lines (the rest of the
/// project counts); snaps off and on.
#[test]
#[ignore = "benchmark: run with --ignored in an optimised build"]
fn bench_move_2d_lines() {
    for snaps in [false, true] {
        for k in [1usize, 10, 100, 1000, 10000] {
            let mut b = Bench::new(creabim_core::sample::sample_house());
            let ids = add_lines(&mut b.app, k);
            bench_move(
                &format!("Sposta {k} linee, progetto piccolo"),
                b,
                &ids,
                snaps,
            );
            let mut b = Bench::new(creabim_core::starter::starter_house());
            let ids = add_lines(&mut b.app, 10000);
            bench_move(&format!("Sposta {k} linee su 10.000"), b, &ids[..k], snaps);
        }
    }
}

/// Move of BIM elements: one wall, the ground floor's walls.
#[test]
#[ignore = "benchmark: run with --ignored in an optimised build"]
fn bench_move_bim() {
    for snaps in [false, true] {
        let b = Bench::new(creabim_core::starter::starter_house());
        let one: Vec<ElementId> = b
            .app
            .doc
            .model()
            .iter()
            .filter(|e| e.name == "GF South")
            .map(|e| e.id)
            .collect();
        bench_move("Sposta 1 muro", b, &one, snaps);
        let b = Bench::new(creabim_core::starter::starter_house());
        let walls: Vec<ElementId> = b
            .app
            .doc
            .model()
            .iter()
            .filter(|e| e.name.starts_with("GF ") && matches!(e.kind, ElementKind::Wall(_)))
            .map(|e| e.id)
            .collect();
        assert!(walls.len() > 5);
        bench_move("Sposta i muri del piano terra", b, &walls, snaps);
    }
}

/// Rotate and Scale with their tools, and copies (Move with copies left
/// behind) of one wall.
#[test]
#[ignore = "benchmark: run with --ignored in an optimised build"]
fn bench_copy_rotate_scale() {
    for (name, cmd, mode) in [
        (
            "Ruota 1 muro",
            crate::commands::Command::Rotate,
            crate::modal::Mode::Rotate,
        ),
        (
            "Scala 1 muro",
            crate::commands::Command::Scale,
            crate::modal::Mode::Scale,
        ),
    ] {
        let mut b = Bench::new(creabim_core::starter::starter_house());
        let w = b
            .app
            .doc
            .model()
            .iter()
            .find(|e| e.name == "GF South")
            .unwrap()
            .id;
        b.select(&[w]);
        let ctx = b.ctx.clone();
        b.app.run_command(&ctx, cmd, None);
        let base = egui::Pos2::new(600.0, 500.0);
        if mode == crate::modal::Mode::Rotate {
            // the centre, then a reference direction
            b.click(base);
            b.click(base + egui::vec2(100.0, 0.0));
        } else {
            // Scale starts as the pointer comes into the view
            b.hover(base);
            b.hover(base + egui::vec2(5.0, 0.0));
        }
        let pts = path(base + egui::vec2(100.0, 20.0), 300);
        let reps = measure(&mut b, 40, |b, i| b.hover(pts[i % pts.len()]));
        let m = b
            .app
            .plan
            .modal
            .as_ref()
            .unwrap_or_else(|| panic!("{name} is running"));
        assert_eq!(m.mode, mode, "{name}");
        report(
            Report {
                scenario: name,
                elements: b.elements(),
                selected: 1,
                snaps: true,
            },
            &reps,
        );
    }
    // copies: Duplicate's copy follows the pointer
    let mut b = Bench::new(creabim_core::starter::starter_house());
    let w = b
        .app
        .doc
        .model()
        .iter()
        .find(|e| e.name == "GF South")
        .unwrap()
        .id;
    b.select(&[w]);
    let n = b.elements();
    let ctx = b.ctx.clone();
    b.app
        .run_command(&ctx, crate::commands::Command::Duplicate, None);
    let pts = path(egui::Pos2::new(600.0, 500.0), 300);
    let reps = measure(&mut b, 40, |b, i| b.hover(pts[i % pts.len()]));
    let m = b
        .app
        .plan
        .modal
        .as_ref()
        .expect("the copy follows the pointer");
    assert!(m.duplicated, "a duplicate");
    assert!(b.elements() > n, "the copy is there");
    report(
        Report {
            scenario: "Copia (Duplica) 1 muro",
            elements: b.elements(),
            selected: 1,
            snaps: true,
        },
        &reps,
    );
}

/// A wall dragged by its middle (a grip drag).
#[test]
#[ignore = "benchmark: run with --ignored in an optimised build"]
fn bench_grip_drag() {
    let mut b = Bench::new(creabim_core::starter::starter_house());
    let w = b
        .app
        .doc
        .model()
        .iter()
        .find(|e| e.name == "GF South")
        .unwrap()
        .id;
    b.select(&[w]);
    let mid = match &b.app.doc.model().get(w).unwrap().kind {
        ElementKind::Wall(x) => x.start + (x.end - x.start) * 0.3,
        _ => unreachable!(),
    };
    let at = b.on_screen(mid);
    let before = first_point(&b.app, w);
    b.hover(at);
    b.button(at, egui::PointerButton::Primary, true);
    let reps = measure(&mut b, 40, |b, i| {
        b.hover(at + egui::vec2((i % 150) as f32 * 2.0, 0.0))
    });
    assert!(b.app.plan.grip.is_some(), "the wall is being dragged");
    assert_ne!(first_point(&b.app, w), before, "it moved");
    report(
        Report {
            scenario: "Trascina un muro (maniglia)",
            elements: b.elements(),
            selected: 1,
            snaps: true,
        },
        &reps,
    );
}

/// A wall's rubber band after its first click, and a polyline's after
/// three points typed.
#[test]
#[ignore = "benchmark: run with --ignored in an optimised build"]
fn bench_draw() {
    let mut b = Bench::new(creabim_core::starter::starter_house());
    let i = crate::planview::Tool::ALL
        .iter()
        .position(|x| *x == crate::planview::Tool::Wall)
        .unwrap();
    let ctx = b.ctx.clone();
    b.app.run_action(&ctx, &format!("tool:{i}"));
    b.click(egui::Pos2::new(600.0, 450.0));
    assert!(
        b.app.plan.pending.is_some(),
        "a wall's first point is waiting"
    );
    let pts = path(egui::Pos2::new(600.0, 480.0), 300);
    let reps = measure(&mut b, 40, |b, i| b.hover(pts[i % pts.len()]));
    report(
        Report {
            scenario: "Disegna un muro (elastico)",
            elements: b.elements(),
            selected: 0,
            snaps: true,
        },
        &reps,
    );
    let mut b = Bench::new(creabim_core::starter::starter_house());
    let ctx = b.ctx.clone();
    b.app.cmd_start(&ctx, "PLINE");
    b.frame(vec![]);
    for p in ["0,0", "5,0", "5,4"] {
        b.app.cmd_enter(&ctx, p);
        b.frame(vec![]);
    }
    let picked = b.app.plan.cmd_pick.as_ref().map_or(0, |(_, p)| p.len());
    assert_eq!(picked, 3, "three polyline points taken");
    let pts = path(egui::Pos2::new(600.0, 480.0), 300);
    let reps = measure(&mut b, 40, |b, i| b.hover(pts[i % pts.len()]));
    report(
        Report {
            scenario: "Disegna una polilinea (elastico)",
            elements: b.elements(),
            selected: 0,
            snaps: true,
        },
        &reps,
    );
}

/// Pan with the middle button, zoom with the wheel, a window selection.
#[test]
#[ignore = "benchmark: run with --ignored in an optimised build"]
fn bench_navigate_and_select() {
    for lines in [0usize, 10000] {
        let mut b = Bench::new(creabim_core::starter::starter_house());
        add_lines(&mut b.app, lines);
        let view = b.app.active_view.unwrap();
        let cam = b.app.plan.cameras[&view];
        let p0 = egui::Pos2::new(800.0, 500.0);
        b.hover(p0);
        b.button(p0, egui::PointerButton::Middle, true);
        let reps = measure(&mut b, 40, |b, i| {
            b.hover(p0 + egui::vec2((i % 100) as f32 * 3.0, 0.0))
        });
        b.button(p0, egui::PointerButton::Middle, false);
        assert_ne!(b.app.plan.cameras[&view].0, cam.0, "the plan panned");
        report(
            Report {
                scenario: "Pan (tasto centrale)",
                elements: b.elements(),
                selected: 0,
                snaps: true,
            },
            &reps,
        );
        let cam = b.app.plan.cameras[&view];
        let reps = measure(&mut b, 40, |b, i| {
            b.frame(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, if (i / 10) % 2 == 0 { 1.0 } else { -1.0 }),
                modifiers: Default::default(),
                phase: egui::TouchPhase::Move,
            }])
        });
        assert_ne!(b.app.plan.cameras[&view].1, cam.1, "the plan zoomed");
        report(
            Report {
                scenario: "Zoom (rotellina)",
                elements: b.elements(),
                selected: 0,
                snaps: true,
            },
            &reps,
        );
        let p1 = egui::Pos2::new(300.0, 200.0);
        b.app.selection.clear();
        b.hover(p1);
        b.button(p1, egui::PointerButton::Primary, true);
        let reps = measure(&mut b, 40, |b, i| {
            b.hover(p1 + egui::vec2(10.0 + (i % 80) as f32 * 10.0, 10.0 + (i % 80) as f32 * 6.0))
        });
        let end = p1 + egui::vec2(800.0, 480.0);
        b.hover(end);
        b.button(end, egui::PointerButton::Primary, false);
        b.frame(vec![]);
        assert!(!b.app.selection.is_empty(), "the window selected something");
        report(
            Report {
                scenario: "Selezione a finestra",
                elements: b.elements(),
                selected: b.app.selection.len(),
                snaps: true,
            },
            &reps,
        );
    }
}

/// The 3D scene built from the model (on a worker in the app): its CPU
/// cost. Orbiting and moving in 3D need a real graphics card: measure them
/// in the app with Help › Performance diagnostics.
#[test]
#[ignore = "benchmark: run with --ignored in an optimised build"]
fn bench_3d_scene() {
    for lines in [0usize, 10000] {
        let mut b = Bench::new(creabim_core::starter::starter_house());
        add_lines(&mut b.app, lines);
        let mut reps = vec![];
        for _ in 0..3 {
            let mut r = vec![];
            for _ in 0..5 {
                let a = Instant::now();
                std::hint::black_box(creabim_geom::scene::build_scene(b.app.doc.model()));
                r.push(Cost {
                    ui: a.elapsed().as_secs_f64() * 1000.0,
                    tess: 0.0,
                    verts: 0,
                });
            }
            reps.push(r);
        }
        report(
            Report {
                scenario: "Scena 3D costruita dal modello (CPU)",
                elements: b.elements(),
                selected: 0,
                snaps: false,
            },
            &reps,
        );
    }
}

/// The benches check what they measure: a short run of each, so a change
/// that breaks a scenario (the selection not taken, the move not running)
/// fails here in ordinary test runs, not silently in a measurement.
#[test]
fn benches_do_what_they_say() {
    CHECK_ONLY.with(|c| c.set(true));
    let mut b = Bench::new(creabim_core::sample::sample_house());
    let ids = add_lines(&mut b.app, 100);
    bench_move("verifica: Sposta 100 linee", b, &ids[..10], true);
}

/// What Help › Performance diagnostics costs: the same Move with the
/// diagnostics off and on (each frame then recorded as the app does).
#[test]
#[ignore = "benchmark: run with --ignored in an optimised build"]
fn bench_diagnostics_overhead() {
    let _one = crate::diag::ONE.lock().unwrap_or_else(|e| e.into_inner());
    for lines in [0usize, 1000] {
        for on in [false, true] {
            crate::diag::set_enabled(on);
            let mut b = Bench::new(creabim_core::starter::starter_house());
            let ids = if lines == 0 {
                vec![
                    b.app
                        .doc
                        .model()
                        .iter()
                        .find(|e| e.name == "GF South")
                        .unwrap()
                        .id,
                ]
            } else {
                add_lines(&mut b.app, 10000)[..lines].to_vec()
            };
            b.select(&ids);
            let ctx = b.ctx.clone();
            b.app
                .run_command(&ctx, crate::commands::Command::Move, None);
            let base = egui::Pos2::new(600.0, 500.0);
            b.click(base);
            let pts = path(base, 300);
            let reps = measure(&mut b, 60, |b, i| {
                let r = b.app.doc.model().revision();
                crate::diag::frame_begin(1, true, false, Some(0.01));
                let c = b.hover(pts[i % pts.len()]);
                crate::diag::frame_end(true, b.app.doc.model().revision() != r);
                c
            });
            assert!(b.app.plan.modal.is_some(), "the move is running");
            if on {
                assert!(!crate::diag::recent(60.0).is_empty(), "frames recorded");
            }
            let what = if lines == 0 {
                "1 muro".to_string()
            } else {
                format!("{lines} linee su 10.000")
            };
            report(
                Report {
                    scenario: &format!(
                        "Diagnostica {}: Sposta {what}",
                        if on { "attiva" } else { "spenta" }
                    ),
                    elements: b.elements(),
                    selected: ids.len(),
                    snaps: true,
                },
                &reps,
            );
        }
    }
    crate::diag::set_enabled(false);
}

/// The diagnostics' own cost per call, off and on: a timed section, and a
/// frame's begin and end.
#[test]
#[ignore = "benchmark: run with --ignored in an optimised build"]
fn bench_diagnostics_per_call() {
    let _one = crate::diag::ONE.lock().unwrap_or_else(|e| e.into_inner());
    for on in [false, true] {
        crate::diag::set_enabled(on);
        crate::diag::frame_begin(1, true, false, None);
        let n = 1_000_000;
        let a = Instant::now();
        for _ in 0..n {
            crate::diag::section(std::hint::black_box("snap exclusion"), 0.1);
        }
        let section_ns = a.elapsed().as_secs_f64() * 1e9 / n as f64;
        crate::diag::frame_end(true, false);
        let k = 10_000;
        let a = Instant::now();
        for _ in 0..k {
            crate::diag::frame_begin(1, true, false, Some(0.01));
            crate::diag::section("snap exclusion", 0.1);
            crate::diag::frame_end(true, true);
        }
        let frame_us = a.elapsed().as_secs_f64() * 1e6 / k as f64;
        // the panel: the last 5 s summed up, every frame it is drawn
        let a = Instant::now();
        for _ in 0..100 {
            std::hint::black_box(crate::diag::summarize(&crate::diag::recent(5.0)));
        }
        let panel_ms = a.elapsed().as_secs_f64() * 1000.0 / 100.0;
        let kept = crate::diag::recent(5.0).len();
        println!(
            "BENCH Diagnostica {}: una sezione {section_ns:.1} ns; un fotogramma (inizio, una sezione, fine) {frame_us:.2} µs; riepilogo del pannello su {kept} fotogrammi {panel_ms:.3} ms",
            if on { "attiva" } else { "spenta" }
        );
    }
    crate::diag::set_enabled(false);
}
