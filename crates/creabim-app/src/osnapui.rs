//! AutoCAD's Drafting Settings (DSETTINGS): the object snap modes with
//! their markers, polar tracking and its increment, Ortho, object snap
//! tracking, the snap spacing, and AutoSnap's marker, tooltip and
//! aperture.

use super::*;

impl CreaBimApp {
    pub(super) fn drafting_settings_window(&mut self, ctx: &egui::Context) {
        if !self.shell.drafting_settings {
            return;
        }
        let mut open = true;
        let mut s = self.plan.snap;
        let mut angle = self.plan.guide_step;
        let mut marker = self.shell.settings.marker_size;
        let mut aperture = self.shell.settings.aperture;
        let mut tip = self.shell.settings.snap_tooltip;
        // over the drawings, never under the web panels round them
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        egui::Window::new("Drafting settings")
            .open(&mut open)
            .constrain_to(area)
            .default_pos(area.left_top() + egui::vec2(24.0, 24.0))
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height((area.height() - 70.0).max(200.0))
                    .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut s.on, "Object snap on (F3)");
                    ui.add_space(16.0);
                    ui.checkbox(&mut s.references, "Object snap tracking on (F11)");
                });
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Object snap modes").strong());
                let kinds = crate::snaps::SnapSettings::SWITCH_KINDS;
                let mut all = None;
                egui::Grid::new("osnap.modes").num_columns(4).spacing([10.0, 4.0]).show(ui, |ui| {
                    for (i, (label, v)) in s.switches().into_iter().enumerate() {
                        let (r, painter) = ui.allocate_painter(egui::vec2(18.0, 18.0), egui::Sense::hover());
                        crate::snaps::draw_marker_icon(&painter, kinds[i], r.rect.center());
                        ui.checkbox(v, label);
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Select all").clicked() {
                        all = Some(true);
                    }
                    if ui.button("Clear all").clicked() {
                        all = Some(false);
                    }
                });
                if let Some(on) = all {
                    s.switches().into_iter().for_each(|(_, v)| *v = on);
                }
                ui.label(
                    egui::RichText::new("Shift + right-click in a drawing gives one snap for the next point; hold Alt to pick free of snaps; Tab steps through the points under the pointer.")
                        .weak()
                        .small(),
                );
                ui.separator();
                ui.label(egui::RichText::new("Polar tracking").strong());
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut s.axis, "Polar tracking on (F10)").changed() && s.axis {
                        s.ortho = false;
                    }
                    ui.label("Increment angle");
                    egui::ComboBox::from_id_salt("polar.inc")
                        .selected_text(format!("{angle}°"))
                        .show_ui(ui, |ui| {
                            for a in [90.0, 45.0, 30.0, 22.5, 18.0, 15.0, 10.0, 5.0] {
                                ui.selectable_value(&mut angle, a, format!("{a}°"));
                            }
                        });
                });
                if ui.checkbox(&mut s.ortho, "Ortho (F8): from the last point only along X or Y").changed() && s.ortho {
                    s.axis = false;
                }
                ui.separator();
                ui.label(egui::RichText::new("Snap and grid").strong());
                ui.horizontal(|ui| {
                    ui.checkbox(&mut s.increment, "Snap on (F9), every");
                    let mut mm = s.step_um as f64 / 1000.0;
                    if ui
                        .add(egui::DragValue::new(&mut mm).range(0.1..=100_000.0).speed(1.0).suffix(" mm"))
                        .changed()
                    {
                        s.step_um = (mm * 1000.0).round().max(100.0) as u32;
                    }
                });
                let mut grid = self.shell.settings.plan_grid;
                if ui.checkbox(&mut grid, "Grid behind drawings (F7)").changed() {
                    self.shell.settings.plan_grid = grid;
                    self.shell.settings.store();
                }
                ui.separator();
                ui.label(egui::RichText::new("AutoSnap").strong());
                ui.checkbox(&mut tip, "Display AutoSnap tooltip");
                ui.horizontal(|ui| {
                    ui.label("Marker size");
                    ui.add(egui::Slider::new(&mut marker, 3..=15).suffix(" px"));
                });
                ui.horizontal(|ui| {
                    ui.label("Aperture size");
                    ui.add(egui::Slider::new(&mut aperture, 4..=40).suffix(" px"));
                });
                ui.separator();
                let mut extras = s;
                ui.collapsing("Volumen's own snaps", |ui| {
                    for (label, v) in extras.extras() {
                        ui.checkbox(v, label);
                    }
                });
                s.grid_lines = extras.grid_lines;
                s.guides = extras.guides;
                s.tracking = extras.tracking;
                ui.horizontal(|ui| {
                    if ui.button("AutoCAD defaults").clicked() {
                        s = crate::snaps::SnapSettings {
                            step_um: s.step_um,
                            marks: s.marks,
                            ..crate::snaps::SnapSettings::AUTOCAD
                        };
                        angle = 90.0;
                        marker = 6;
                        aperture = 10;
                        tip = true;
                    }
                });
                    });
            });
        if s != self.plan.snap {
            self.plan.snap = s;
            self.plan.forget_snaps();
            self.snaps_3d = None;
        }
        self.plan.guide_step = angle;
        let st = &mut self.shell.settings;
        if (st.marker_size, st.aperture, st.snap_tooltip) != (marker, aperture, tip) {
            st.marker_size = marker;
            st.aperture = aperture;
            st.snap_tooltip = tip;
            st.store();
        }
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.shell.drafting_settings = false;
        }
    }
}

impl CreaBimApp {
    /// AutoCAD's layer manager for the drafting layers: the tree of layers
    /// and sub-layers, over the drawings.
    pub(super) fn layer_manager_window(&mut self, ctx: &egui::Context) {
        if !self.shell.layer_manager {
            return;
        }
        let mut open = true;
        let table = creabim_core::detail::layers(self.doc.model());
        let mut edit = None;
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        egui::Window::new("Drafting layers")
            .open(&mut open)
            .collapsible(false)
            .constrain_to(area)
            .default_pos(area.right_top() + egui::vec2(-460.0, 24.0))
            .default_width(420.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height((area.height() - 90.0).max(160.0))
                    .show(ui, |ui| {
                        crate::detailtool::layer_tree(ui, &table, &mut edit);
                    });
                ui.label(
                    egui::RichText::new(
                        "A sub-layer is hidden with the layer it is under, and takes its colour and line type unless it has its own. Put linework on a layer in its Properties.",
                    )
                    .weak()
                    .small(),
                );
            });
        let tx = match edit {
            Some(crate::detailtool::LayerEdit::Set(t)) => {
                Some(creabim_core::detail::set_layers(self.doc.model(), t))
            }
            Some(crate::detailtool::LayerEdit::Rename { old, new }) => Some(
                creabim_core::detail::rename_layer(self.doc.model(), &old, &new),
            ),
            _ => None,
        };
        if let Some(tx) = tx.filter(|t| !t.ops.is_empty()) {
            match self.doc.commit(tx) {
                Ok(_) => self.status = "Drafting layers changed. Ctrl+Z to undo.".into(),
                Err(e) => self.status = format!("Couldn't do that: {e}"),
            }
        }
        if !open {
            self.shell.layer_manager = false;
        }
    }
}
