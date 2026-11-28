//! Help › Performance diagnostics: a small panel over the views with what
//! the last seconds of work cost, a 30 s recording saved as a report to
//! send, and test projects to measure with (docs/prestazioni/collaudo.md).
//! The numbers come from [`crate::diag`]; nothing here changes how a
//! command, a snap or a drawing works.

use super::*;

/// The test projects: (label, extra lines in the ground floor plan, the
/// small sample instead of the starter house).
pub(super) const TEST_PROJECTS: &[(&str, usize, bool)] = &[
    ("Starter house", 0, false),
    ("Starter house + 1,000 lines", 1000, false),
    ("Starter house + 10,000 lines", 10000, false),
    ("Small house + 10,000 lines", 10000, true),
];

/// The name the test lines carry, to select them.
pub(super) const TEST_LINE: &str = "Diagnostics test line";

/// The last report saved this session.
static LAST_REPORT: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

/// Where reports go: a "diagnostics" folder beside Volumen's settings (a
/// temporary folder in tests).
pub(super) fn reports_dir() -> Option<std::path::PathBuf> {
    if cfg!(test) {
        return Some(std::env::temp_dir().join("volumen-diagnostics-test"));
    }
    crate::files::settings_dir().map(|d| d.join("diagnostics"))
}

/// Something the panel asked for, done once the panel is drawn.
type PanelAction = Box<dyn FnOnce(&mut CreaBimApp, &egui::Context)>;

/// Seconds of work the panel sums up.
const PANEL_SECONDS: f64 = 5.0;

impl CreaBimApp {
    /// Help › Performance diagnostics: on or off. Off drops what was kept.
    pub(super) fn toggle_diagnostics(&mut self) {
        let on = !crate::diag::enabled();
        crate::diag::set_enabled(on);
        if on {
            crate::diag::find_machine();
        }
        self.status = if on {
            "Performance diagnostics on: the panel shows what the last seconds of work cost. Help › Performance diagnostics again turns it off.".into()
        } else {
            "Performance diagnostics off.".into()
        };
    }

    /// Open test project `i` of [`TEST_PROJECTS`], as a new project.
    pub(super) fn open_test_project(&mut self, i: usize) {
        let Some(&(label, lines, small)) = TEST_PROJECTS.get(i) else {
            return;
        };
        let model = if small {
            creabim_core::sample::sample_house()
        } else {
            creabim_core::starter::starter_house()
        };
        self.load_model(model);
        self.path.clear();
        let plan = self
            .doc
            .model()
            .iter()
            .find(|e| e.name == "Ground Floor plan")
            .map(|e| e.id);
        if let (Some(v), true) = (plan, lines > 0) {
            let mut tx = Transaction::new("Diagnostics: test lines");
            for k in 0..lines {
                let x = (k % 100) as f64 * 0.15;
                let y = (k / 100) as f64 * 0.15 - 3.0;
                tx = tx.insert(Element::new(
                    TEST_LINE,
                    ElementKind::Detail(creabim_core::DetailItem::new(
                        v,
                        creabim_core::DetailKind::draft(creabim_core::DraftShape::Line),
                        vec![DVec2::new(x, y), DVec2::new(x + 0.1, y + 0.05)],
                    )),
                ));
            }
            let r = self.doc.commit(tx);
            self.report(r, format!("Test project: {label}."));
        } else {
            self.status = format!("Test project: {label}.");
        }
        self.active_view = plan.or(self.active_view);
        self.frame_all();
    }

    /// Select the first `n` test lines; how many there were.
    pub(super) fn select_test_lines(&mut self, n: usize) -> usize {
        let ids: Vec<ElementId> = self
            .doc
            .model()
            .iter()
            .filter(|e| e.name == TEST_LINE)
            .take(n)
            .map(|e| e.id)
            .collect();
        self.selection.clear();
        self.selection.extend(ids.iter().copied(), false);
        self.face = None;
        self.status = format!(
            "{} test lines selected. Move them with Move (M) or by dragging.",
            self.selection.len()
        );
        ids.len()
    }

    /// What is at work now, in a word, for the panel and the report.
    pub(super) fn diag_command(&self) -> String {
        if let Some(m) = &self.plan.modal {
            return format!("{:?}", m.mode);
        }
        if self.plan.grip.is_some() || self.handle_drag.is_some() {
            return "Drag".into();
        }
        if let Some(r) = &self.cmd.running {
            return r.name.to_string();
        }
        format!("{:?}", self.plan.tool)
    }

    /// Is a tool at work (something follows or waits for the pointer)?
    pub(super) fn diag_working(&self) -> bool {
        self.plan.modal.is_some()
            || self.plan.grip.is_some()
            || self.handle_drag.is_some()
            || self.plan.has_input()
            || self.pending_3d.is_some()
    }

    /// The report's context: no names, no paths.
    pub(super) fn diag_context(&self, ctx: &egui::Context) -> crate::diag::Context {
        let s = &self.shell.settings;
        let (window, monitor) = ctx.input(|i| {
            let v = i.viewport();
            let ppp = v.native_pixels_per_point.unwrap_or(1.0);
            let w = i.content_rect();
            (
                format!(
                    "{:.0} x {:.0} pixel, scala {ppp}",
                    w.width() * ppp,
                    w.height() * ppp
                ),
                v.monitor_size
                    .map(|m| format!("{:.0} x {:.0} pixel", m.x * ppp, m.y * ppp)),
            )
        });
        crate::diag::Context {
            gpu: crate::app::reportui::gpu(),
            os: crate::app::reportui::os_version(),
            window: match monitor {
                Some(m) => format!("{window}; schermo {m}"),
                None => window,
            },
            settings: vec![
                ("Stile 3D".into(), format!("{:?}", s.render_3d)),
                ("Ombre 3D".into(), s.shadows_3d.to_string()),
                ("Occlusione 3D".into(), s.occlusion_3d.to_string()),
                ("Una sola vista".into(), s.single_view.to_string()),
                ("Mirino".into(), s.crosshair.to_string()),
                ("Linee degli assi".into(), s.axis_lines.to_string()),
                ("Snap attivi".into(), self.plan.snap.on.to_string()),
                ("Tema".into(), s.theme.clone()),
            ],
            elements: crate::diag::kinds(self.doc.model()),
            selected: self.selection.len(),
            command: self.diag_command(),
        }
    }

    /// Save a recording as a report, where the user says.
    /// Save a recording as a report, at once, in the diagnostics folder
    /// (no dialog to answer): the panel then names the file and opens its
    /// folder.
    fn save_diag_report(&mut self, ctx: &egui::Context, frames: &[crate::diag::Frame]) {
        let text = crate::diag::report(&self.diag_context(ctx), frames);
        let saved = reports_dir()
            .ok_or_else(|| "no folder for it on this computer".to_string())
            .and_then(|dir| {
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let path = dir.join(crate::diag::report_name());
                std::fs::write(&path, text).map_err(|e| e.to_string())?;
                Ok(path)
            });
        self.status = match &saved {
            Ok(p) => format!(
                "Diagnostics report saved: {}. Diagnostics panel › Open the folder shows it.",
                p.display()
            ),
            Err(e) => format!("Couldn't save the diagnostics report: {e}"),
        };
        crate::actlog::line(format!(
            "diagnostics report: {}",
            saved
                .as_ref()
                .map_or("not saved".into(), |p| p.display().to_string())
        ));
        *LAST_REPORT.lock().unwrap_or_else(|e| e.into_inner()) = saved.ok();
    }

    /// The diagnostics panel, while diagnostics are on.
    pub(super) fn diagnostics_window(&mut self, ctx: &egui::Context) {
        if !crate::diag::enabled() {
            return;
        }
        if let Some(frames) = crate::diag::take_recording(false) {
            self.save_diag_report(ctx, &frames);
        }
        let s = crate::diag::summarize(&crate::diag::recent(PANEL_SECONDS));
        let mut open = true;
        let mut action: Option<PanelAction> = None;
        let ms = |v: Option<crate::diag::Spread>| match v {
            Some(v) => format!("{:.1} / {:.1} / {:.1} / {:.1}", v.p50, v.p95, v.p99, v.max),
            None => "-".into(),
        };
        egui::Window::new("Performance diagnostics")
            .id(egui::Id::new("diagnostics"))
            .open(&mut open)
            .default_pos(ctx.content_rect().right_top() + egui::vec2(-390.0, 40.0))
            .default_width(360.0)
            .resizable(false)
            .collapsible(true)
            .show(ctx, |ui| {
                let state = if s.interactive == 0 {
                    "Idle: no frames needed (normal)"
                } else {
                    "Working"
                };
                ui.label(format!(
                    "{state}. Last {PANEL_SECONDS:.0} s: {} frames, {} while working.",
                    s.frames, s.interactive
                ));
                ui.label(format!(
                    "Command: {} · selected: {}",
                    self.diag_command(),
                    self.selection.len()
                ));
                ui.horizontal(|ui| match crate::diag::recording_left() {
                    Some(left) => {
                        ui.label(format!("Recording… {left:.0} s left"));
                        if ui.button("Stop and save").clicked() {
                            action = Some(Box::new(|app: &mut Self, ctx: &egui::Context| {
                                if let Some(f) = crate::diag::take_recording(true) {
                                    app.save_diag_report(ctx, &f);
                                }
                            }));
                        }
                    }
                    None => {
                        if ui
                            .button("Record 30 s")
                            .on_hover_text("Work as usual for 30 seconds; then save the report and send it.")
                            .clicked()
                        {
                            crate::diag::start_recording();
                        }
                    }
                });
                let last = LAST_REPORT.lock().unwrap_or_else(|e| e.into_inner()).clone();
                if let Some(p) = last {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "Saved: {}",
                            p.file_name().map_or(String::new(), |n| n.to_string_lossy().into())
                        ));
                        if ui.small_button("Open the folder").clicked() {
                            if let Some(dir) = p.parent() {
                                let _ = crate::files::open_folder(dir);
                            }
                        }
                    });
                }
                ui.label("Test project (opens as a new project):");
                ui.horizontal_wrapped(|ui| {
                    for (i, (label, _, _)) in TEST_PROJECTS.iter().enumerate() {
                        if ui.small_button(*label).clicked() {
                            action = Some(Box::new(move |app: &mut Self, _: &egui::Context| {
                                app.open_test_project(i)
                            }));
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Select test lines:");
                    for n in [1usize, 10, 100, 1000, 10000] {
                        if ui.small_button(format!("{n}")).clicked() {
                            action = Some(Box::new(move |app: &mut Self, _: &egui::Context| {
                                app.select_test_lines(n);
                            }));
                        }
                    }
                });
                ui.separator();
                egui::Grid::new("diag-grid").num_columns(2).show(ui, |ui| {
                    ui.label("FPS while working (measured)");
                    ui.label(s.fps.map_or("-".into(), |f| format!("{f:.1}")));
                    ui.end_row();
                    ui.label("ms between frames (measured)");
                    ui.label(ms(s.interval));
                    ui.end_row();
                    ui.label("ms CPU per frame = internal latency (measured)")
                        .on_hover_text("From the frame's start, when its input is read, to its rendering handed to the graphics card. Not the time to the light on the monitor.");
                    ui.label(ms(s.cpu));
                    ui.end_row();
                    ui.label("ms waiting for the screen (estimate)")
                        .on_hover_text("Time between frames less CPU time: VSync and frames queued.");
                    ui.label(ms(s.wait));
                    ui.end_row();
                    ui.label("GPU time");
                    ui.label("not available");
                    ui.end_row();
                    ui.label("Model updates with nothing moving");
                    ui.label(s.idle_updates.to_string());
                    ui.end_row();
                });
                ui.small("Times: median / p95 / p99 / max, while working.");
                ui.separator();
                egui::Grid::new("diag-sections").num_columns(3).show(ui, |ui| {
                    ui.small("Section");
                    ui.small("ms per frame (median / max)");
                    ui.small("times");
                    ui.end_row();
                    for (name, _what, frames, sp, times) in &s.sections {
                        if *frames == 0 {
                            continue;
                        }
                        ui.small(*name);
                        ui.small(match sp {
                            Some(v) if !crate::diag::is_count(name) => {
                                format!("{:.2} / {:.2}", v.p50, v.max)
                            }
                            _ => "-".into(),
                        });
                        ui.small(times.to_string());
                        ui.end_row();
                    }
                });
                ui.separator();
            });
        if let Some(a) = action {
            a(self, ctx);
        }
        if !open {
            crate::diag::set_enabled(false);
            self.status = "Performance diagnostics off.".into();
        }
        // the panel follows the work: it is drawn again while frames come
        // anyway; when idle it waits like the rest of the app
        if crate::diag::recording_left().is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_project_has_its_lines_in_the_ground_floor_plan_and_they_select() {
        let mut app = crate::app::test_app();
        app.open_test_project(1);
        let lines: Vec<_> = app
            .doc
            .model()
            .iter()
            .filter(|e| e.name == TEST_LINE)
            .collect();
        assert_eq!(lines.len(), 1000);
        let plan = app
            .doc
            .model()
            .iter()
            .find(|e| e.name == "Ground Floor plan")
            .unwrap()
            .id;
        assert!(lines
            .iter()
            .all(|e| matches!(&e.kind, ElementKind::Detail(d) if d.view == plan)));
        assert!(app.path.is_empty(), "a new project, not saved over a file");
        for n in [1usize, 10, 100, 1000] {
            assert_eq!(app.select_test_lines(n), n);
            assert_eq!(app.selection.len(), n, "{n} selected");
        }
        assert_eq!(
            app.select_test_lines(10000),
            1000,
            "only as many as there are"
        );
    }

    #[test]
    fn the_panel_draws_while_on_and_not_while_off() {
        let _one = crate::diag::ONE.lock().unwrap_or_else(|e| e.into_inner());
        let mut app = crate::app::test_app();
        let ctx = egui::Context::default();
        let draw = |app: &mut CreaBimApp, events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 1000.0),
                )),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(raw, |ui| app.diagnostics_window(ui.ctx()));
            out.textures_delta.clear();
            out.shapes.len()
        };
        crate::diag::set_enabled(false);
        assert_eq!(draw(&mut app, vec![]), 0, "off: no panel");
        crate::diag::set_enabled(true);
        for _ in 0..5 {
            crate::diag::frame_begin(1, true, false, Some(0.01));
            crate::diag::section("snap exclusion", 1.0);
            crate::diag::frame_end(true, false);
        }
        assert!(draw(&mut app, vec![]) > 0, "on: the panel shows");
        assert!(crate::diag::enabled());
        crate::diag::set_enabled(false);
    }

    #[test]
    fn the_help_menu_turns_diagnostics_on_and_off() {
        let _one = crate::diag::ONE.lock().unwrap_or_else(|e| e.into_inner());
        let mut app = crate::app::test_app();
        let ctx = egui::Context::default();
        assert!(app.help_menu().iter().any(
            |i| matches!(i, crate::ui_model::Item::Action { id, .. } if id == "app.diagnostics")
        ));
        let was = crate::diag::enabled();
        app.run_action(&ctx, "app.diagnostics");
        assert_ne!(crate::diag::enabled(), was);
        app.run_action(&ctx, "app.diagnostics");
        assert_eq!(crate::diag::enabled(), was);
    }

    #[test]
    fn a_recording_is_saved_at_once_without_a_dialog() {
        let _one = crate::diag::ONE.lock().unwrap_or_else(|e| e.into_inner());
        let mut app = crate::app::test_app();
        app.open_test_project(1);
        let ctx = egui::Context::default();
        crate::diag::set_enabled(true);
        let mut frames = Vec::new();
        for _ in 0..3 {
            crate::diag::frame_begin(1, true, false, Some(0.01));
            crate::diag::frame_end(true, false);
            frames = crate::diag::recent(60.0);
        }
        crate::diag::set_enabled(false);
        app.save_diag_report(&ctx, &frames);
        let path = LAST_REPORT.lock().unwrap().clone().expect("saved");
        assert!(path.starts_with(reports_dir().unwrap()));
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            name.starts_with("Volumen-diagnostica-20") && name.ends_with(".txt"),
            "{name}"
        );
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("1000"), "the project's size is in it");
        assert!(!text.contains(TEST_LINE), "no element names");
        assert!(app.status.contains("saved"), "{}", app.status);
        let _ = std::fs::remove_file(path);
    }
}
