//! The material browser: the project's materials (add, duplicate, rename,
//! edit, delete with their users reassigned, assign to the selection), the
//! built-in library, and materials imported from IFC, FreeCAD, glTF and
//! Blender files. Every change is an ordinary transaction, so it undoes
//! and saves like any edit.

use super::*;
use crate::ui_model::Item;
use creabim_core::library::{self, CATEGORIES, LIBRARY};
use creabim_core::takeoff::{Rate, RateUnit};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum MatTab {
    #[default]
    Project,
    Library,
}

/// The material browser's own state, for the session.
#[derive(Default)]
pub(super) struct MaterialsUi {
    pub open: bool,
    /// Where the views are, which the window keeps within (the web panes
    /// round them would hide it).
    pub area: Option<Rect>,
    tab: MatTab,
    filter: String,
    /// The material being edited.
    current: Option<ElementId>,
    /// Asked to delete a material that is in use: what its users get instead.
    delete: Option<(ElementId, Option<ElementId>)>,
}

/// File kinds materials are imported from.
pub(super) const MATERIAL_FILES: &[&str] = &["ifc", "FCMat", "fcmat", "gltf", "glb", "blend"];

/// Is this a file opened only for its materials (not a model)?
pub(super) fn is_material_file(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.ends_with(".fcmat") || p.ends_with(".gltf") || p.ends_with(".glb")
}

fn swatch(ui: &mut egui::Ui, m: &Material, size: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let c = m.color.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    let p = ui.painter();
    if m.opacity < 0.999 {
        // a checker shows through see-through materials
        let h = r.width() / 2.0;
        for q in [r.left_top(), r.left_top() + Vec2::new(h, h)] {
            p.rect_filled(Rect::from_min_size(q, Vec2::splat(h)), 0.0, Color32::GRAY);
        }
    }
    p.rect_filled(
        r,
        3.0,
        Color32::from_rgba_unmultiplied(c[0], c[1], c[2], (m.opacity * 255.0) as u8),
    );
    if m.metallic > 0.5 {
        // a sheen marks metals
        p.line_segment(
            [
                r.left_top() + Vec2::new(3.0, size * 0.7),
                r.left_top() + Vec2::new(size * 0.7, 3.0),
            ],
            egui::Stroke::new(1.5, Color32::from_white_alpha(170)),
        );
    }
    p.rect_stroke(
        r,
        3.0,
        egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );
    resp
}

/// A ball shaded the way the 3D view shades the material.
pub(super) fn preview(ui: &mut egui::Ui, m: &Material, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter_at(rect);
    // checker behind, for see-through materials
    let n = 6;
    let cell = size / n as f32;
    for i in 0..n {
        for j in 0..n {
            let c = if (i + j) % 2 == 0 {
                Color32::from_gray(205)
            } else {
                Color32::from_gray(150)
            };
            painter.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::new(i as f32 * cell, j as f32 * cell),
                    Vec2::splat(cell),
                ),
                0.0,
                c,
            );
        }
    }
    let steps = 48;
    let mut mesh = egui::Mesh::default();
    let radius = size * 0.45;
    let center = rect.center();
    for j in 0..=steps {
        for i in 0..=steps {
            let x = i as f32 / steps as f32 * 2.0 - 1.0;
            let y = j as f32 / steps as f32 * 2.0 - 1.0;
            let d2 = x * x + y * y;
            let (nx, ny) = if d2 > 1.0 {
                let d = d2.sqrt();
                (x / d, y / d)
            } else {
                (x, y)
            };
            let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
            // screen y down; the model's "up" is -y
            let shade = shade(m, [nx, -ny, nz]);
            let a = (m.opacity.clamp(0.05, 1.0) * 255.0) as u8;
            let col = shade.map(|c| (linear_to_srgb(c) * 255.0).round() as u8);
            mesh.colored_vertex(
                center + Vec2::new(nx, ny) * radius,
                Color32::from_rgba_unmultiplied(col[0], col[1], col[2], a),
            );
        }
    }
    let row = steps as u32 + 1;
    for j in 0..steps as u32 {
        for i in 0..steps as u32 {
            let k = j * row + i;
            mesh.add_triangle(k, k + 1, k + row);
            mesh.add_triangle(k + 1, k + row + 1, k + row);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn linear_to_srgb(c: f32) -> f32 {
    creabim_exchange::materials::srgb(c)
}

/// The 3D view's shading (see `render3d.rs`), for a normal in view space
/// seen straight on. Linear RGB.
fn shade(m: &Material, n: [f32; 3]) -> [f32; 3] {
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let norm = |a: [f32; 3]| {
        let l = dot(a, a).sqrt().max(1e-6);
        a.map(|x| x / l)
    };
    let v = [0.0, 0.0, 1.0];
    let l = norm([0.5, 0.6, 0.65]);
    let rough = m.roughness.clamp(0.04, 1.0);
    let metal = m.metallic.clamp(0.0, 1.0);
    let base = m.color.map(|c| c.clamp(0.0, 1.0).powf(2.2));
    let diffuse = dot(n, l).max(0.0);
    let sky = 0.5 + 0.5 * n[1];
    let ndv = dot(n, v).max(0.0);
    let fres = (1.0 - ndv).powi(5);
    let h = norm([l[0] + v[0], l[1] + v[1], l[2] + v[2]]);
    let a = rough * rough;
    let shine = (2.0 / (a * a) - 2.0).clamp(1.0, 2048.0);
    let spec = dot(n, h).max(0.0).powf(shine) * (shine + 2.0) / 8.0 * diffuse;
    // reflect(-v, n): its "up" component
    let r_up = 2.0 * ndv * n[1] - v[1];
    let t = 0.5 + 0.5 * r_up;
    let mut out = [0.0; 3];
    for k in 0..3 {
        let f0 = 0.04 * (1.0 - metal) + base[k] * metal;
        let f = f0 + (1.0 - f0) * fres * (1.0 - rough);
        let env = [0.30, 0.30, 0.32][k] * (1.0 - t) + [0.95, 0.97, 1.0][k] * t;
        let lit = base[k] * (1.0 - metal) * (0.30 + 0.25 * sky + 0.55 * diffuse);
        out[k] = lit + (f * spec).min(4.0) * (1.0 - 0.5 * rough) + f * env * (1.0 - 0.6 * rough);
    }
    out
}

/// The fields of a material. Returns the undo label of what changed, or "".
/// `patterns` are the style pack's own hatch patterns.
pub(super) fn material_fields(
    ui: &mut egui::Ui,
    m: &mut Material,
    patterns: &[creabim_core::styles::HatchPattern],
) -> &'static str {
    let mut label = "";
    egui::Grid::new(ui.id().with("material_fields"))
        .num_columns(2)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            ui.label("Category");
            egui::ComboBox::from_id_salt("mat_category")
                .icon(crate::props::combo_icon)
                .selected_text(library::category_label(&m.category))
                .width(ui.available_width().min(170.0))
                .truncate()
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(m.category.is_empty(), "Uncategorised")
                        .clicked()
                    {
                        m.category.clear();
                        label = "Material category";
                    }
                    for (value, text) in CATEGORIES {
                        if ui.selectable_label(m.category == *value, *text).clicked() {
                            m.category = value.to_string();
                            label = "Material category";
                        }
                    }
                })
                .response
                .on_hover_text("Written to IFC as IfcMaterial.Category");
            ui.end_row();

            ui.label("Description");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut m.description)
                        .vertical_align(egui::Align::Center)
                        .desired_width(170.0),
                )
                .changed()
            {
                label = "Material description";
            }
            ui.end_row();

            ui.label("Colour");
            // the colour is sRGB, as the picker shows it
            let mut srgb = m.color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
            if ui.color_edit_button_srgb(&mut srgb).changed() {
                m.color = srgb.map(|c| c as f32 / 255.0);
                label = "Material colour";
            }
            ui.end_row();

            ui.label("Cut hatch");
            ui.horizontal(|ui| {
                hatch_swatch(ui, m.hatch, 22.0);
                egui::ComboBox::from_id_salt("mat_hatch")
                    .icon(crate::props::combo_icon)
                    .selected_text(m.hatch.label())
                    .width(ui.available_width().min(150.0))
                    .truncate()
                    .show_ui(ui, |ui| {
                        // each with its picture: a hatch is chosen by eye
                        for h in Hatch::ALL {
                            ui.horizontal(|ui| {
                                hatch_swatch(ui, h, 18.0);
                                if ui.selectable_value(&mut m.hatch, h, h.label()).changed() {
                                    label = "Material hatch";
                                }
                            });
                        }
                    });
            });
            ui.end_row();

            // a pattern of the style pack's own, over the built-in one
            ui.label("Pattern");
            ui.horizontal(|ui| {
                match m.pattern.as_ref().and_then(|n| patterns.iter().find(|p| &p.name == n)) {
                    Some(p) => pattern_swatch(ui, p, 22.0),
                    None => {
                        hatch_swatch(ui, m.hatch, 22.0);
                    }
                }
                egui::ComboBox::from_id_salt("mat_pattern")
                    .icon(crate::props::combo_icon)
                    .selected_text(m.pattern.clone().unwrap_or_else(|| "As the cut hatch".into()))
                    .width(ui.available_width().min(150.0))
                    .truncate()
                    .show_ui(ui, |ui| {
                        ui.horizontal(|ui| {
                            hatch_swatch(ui, m.hatch, 18.0);
                            if ui.selectable_value(&mut m.pattern, None, "As the cut hatch").changed() {
                                label = "Material pattern";
                            }
                        });
                        for p in patterns {
                            ui.horizontal(|ui| {
                                pattern_swatch(ui, p, 18.0);
                                if ui
                                    .selectable_value(&mut m.pattern, Some(p.name.clone()), &p.name)
                                    .changed()
                                {
                                    label = "Material pattern";
                                }
                            });
                        }
                        // the whole library too: one picked joins the
                        // project's patterns by itself
                        let library: Vec<_> = creabim_core::styles::library_patterns()
                            .into_iter()
                            .filter(|l| !patterns.iter().any(|p| p.name == l.name))
                            .collect();
                        if !library.is_empty() {
                            ui.separator();
                            ui.label(egui::RichText::new("From the library").weak().small());
                            for l in library {
                                ui.horizontal(|ui| {
                                    pattern_swatch(ui, &l, 18.0);
                                    if ui
                                        .selectable_label(false, &l.name)
                                        .on_hover_text("Added to the project's patterns when picked")
                                        .clicked()
                                    {
                                        m.pattern = Some(l.name.clone());
                                        label = "Material pattern";
                                        PENDING_PATTERN.with(|c| *c.borrow_mut() = Some(l.clone()));
                                    }
                                });
                            }
                        }
                    })
                    .response
                    .on_hover_text(if patterns.is_empty() {
                        "Add patterns in Materials › Hatch patterns (from the library, or a .pat file) to choose them here"
                    } else {
                        "One of the style pack's patterns for its cut faces; As the cut hatch uses the built-in one"
                    });
            });
            ui.end_row();

            ui.label("Roughness");
            if ui
                .add(egui::Slider::new(&mut m.roughness, 0.0..=1.0).fixed_decimals(2))
                .on_hover_text("0 is mirror-smooth, 1 fully matte")
                .changed()
            {
                label = "Material roughness";
            }
            ui.end_row();

            ui.label("Metallic");
            if ui
                .add(egui::Slider::new(&mut m.metallic, 0.0..=1.0).fixed_decimals(2))
                .on_hover_text("1 for bare metal, 0 for everything else")
                .changed()
            {
                label = "Material metallic";
            }
            ui.end_row();

            ui.label("Opacity");
            if ui
                .add(egui::Slider::new(&mut m.opacity, 0.05..=1.0).fixed_decimals(2))
                .on_hover_text("1 is opaque; glass is about 0.3")
                .changed()
            {
                label = "Material opacity";
            }
            ui.end_row();

            ui.label("Rate");
            crate::props::flow(ui, |ui| {
                let per = m.rate.map_or(RateUnit::Area, |r| r.per);
                let mut amount = m.rate.map_or(0.0, |r| r.amount);
                if ui
                    .add(
                        egui::DragValue::new(&mut amount)
                            .range(0.0..=1.0e9)
                            .speed(0.5)
                            .max_decimals(2),
                    )
                    .on_hover_text(
                        "Its price for take-off, in the currency you price the project in",
                    )
                    .changed()
                {
                    m.rate = Some(Rate { amount, per });
                    label = "Material rate";
                }
                egui::ComboBox::from_id_salt("mat_rate_per")
                    .icon(crate::props::combo_icon)
                    .selected_text(per.label())
                    .width(ui.available_width().min(70.0))
                    .show_ui(ui, |ui| {
                        for u in RateUnit::ALL {
                            if ui.selectable_label(per == u, u.label()).clicked() {
                                m.rate = Some(Rate { amount, per: u });
                                label = "Material rate";
                            }
                        }
                    });
                if m.rate.is_some()
                    && ui
                        .small_button("Clear")
                        .on_hover_text("No rate: leave it out of the priced total")
                        .clicked()
                {
                    m.rate = None;
                    label = "Material rate";
                }
            });
            ui.end_row();
        });
    label
}

/// What a click in the browser asks for, done once the window is drawn.
enum Ask {
    Pick(ElementId),
    New,
    Duplicate(ElementId),
    Delete(ElementId),
    /// What the users of a material being deleted get instead.
    Replacement(ElementId, Option<ElementId>),
    ConfirmDelete(ElementId, Option<ElementId>),
    Rename(ElementId, String),
    Edit(ElementId, &'static str, Material),
    SelectUsers(ElementId),
    Assign(Option<ElementId>),
    AddLibrary(Vec<usize>),
    Import,
}

impl CreaBimApp {
    /// The project's material elements that can be assigned: (id, name).
    fn assignable(&self) -> Vec<ElementId> {
        self.selection
            .all()
            .into_iter()
            .filter(|id| {
                self.doc
                    .model()
                    .get(*id)
                    .is_some_and(|e| library::material_slot(e).is_some())
            })
            .collect()
    }

    pub(super) fn open_materials(&mut self, current: Option<ElementId>) {
        self.mat.open = true;
        self.mat.tab = MatTab::Project;
        if current.is_some() {
            self.mat.current = current;
        }
    }

    /// The Materials menu.
    pub(super) fn materials_menu(&self) -> Vec<Item> {
        let targets = self.assignable().len();
        let mut assign = vec![Item::new("mat.assign:none", "By type (no material)")];
        for (id, name, _) in library::materials(self.doc.model()) {
            assign.push(Item::new(format!("mat.assign:{}", id.0), name));
        }
        let lib: Vec<Item> = CATEGORIES
            .iter()
            .enumerate()
            .map(|(ci, (cat, label))| {
                let mut items: Vec<Item> = LIBRARY
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.category == *cat)
                    .map(|(i, e)| Item::new(format!("mat.lib:{i}"), e.name).hint(e.description))
                    .collect();
                items.push(Item::Separator);
                items.push(Item::new(format!("mat.libcat:{ci}"), "Add them all"));
                Item::sub(*label, items)
            })
            .collect();
        let mut items = vec![
            Item::new("mat.browser", "Material browser…")
                .icon("palette")
                .hint("Add, edit, delete and assign the project's materials"),
            Item::new("hatches", "Hatch patterns…")
                .icon("hash")
                .hint("Every hatch, with its picture: the built-in ones, AutoCAD's library, and patterns from .pat files"),
            Item::sub("Add from the library", lib).icon("plus"),
            Item::new("mat.import", "Import materials…").icon("download").hint(
                "Materials from an IFC file, FreeCAD material cards (.FCMat), glTF (.glb, .gltf, e.g. exported from Blender) or a .blend file",
            ),
        ];
        if self.shell.web.is_none() {
            items.push(
                Item::new(
                    "mat.import.path",
                    "Import materials from the path in the File box",
                )
                .icon("download")
                .hint("For typing or pasting a path instead of browsing."),
            );
        }
        items.extend([
            Item::Separator,
            Item::Header(match targets {
                0 => "Select walls, slabs or masses to assign".into(),
                1 => "Assign to the selected element".into(),
                n => format!("Assign to the {n} selected elements"),
            }),
            Item::sub("Assign material", assign)
                .icon("palette")
                .enabled(targets > 0),
        ]);
        items.extend(self.assemblies_menu());
        items.extend(self.types_menu());
        items
    }

    /// Run a Materials menu action. False when the action isn't one.
    pub(super) fn materials_action(&mut self, head: &str, arg: &str) -> bool {
        match head {
            "mat.browser" => self.open_materials(None),
            "mat.import" => self.import_materials_dialog(),
            "mat.import.path" => {
                let path = self.path.trim().to_string();
                self.import_materials(&[path]);
            }
            "mat.lib" => {
                if let Ok(i) = arg.parse::<usize>() {
                    self.add_library(&[i]);
                }
            }
            "mat.libcat" => {
                if let Some((cat, _)) = arg.parse::<usize>().ok().and_then(|i| CATEGORIES.get(i)) {
                    let all: Vec<usize> = (0..LIBRARY.len())
                        .filter(|i| LIBRARY[*i].category == *cat)
                        .collect();
                    self.add_library(&all);
                }
            }
            "mat.assign" => {
                let m = self
                    .doc
                    .model()
                    .iter()
                    .find(|e| e.id.0.to_string() == arg)
                    .map(|e| e.id);
                self.assign_material(m);
            }
            _ => return self.assemblies_action(head, arg),
        }
        true
    }

    fn add_library(&mut self, entries: &[usize]) {
        let incoming: Vec<(String, Material)> = entries
            .iter()
            .filter_map(|i| LIBRARY.get(*i))
            .map(|e| (e.name.to_string(), e.material()))
            .collect();
        let n = incoming.len();
        let (tx, r, ids) = library::merge(self.doc.model(), incoming, "Add materials");
        let res = self.doc.commit(tx);
        if n == 1 {
            self.open_materials(ids.first().copied());
        }
        self.report(
            res,
            match (r.added, r.updated) {
                (0, 0) => "Those materials are in the project already.".to_string(),
                (a, 0) => format!("Added {a} material{} from the library.", plural_s(a)),
                (a, u) => format!(
                    "Added {a} material{} from the library and reset {u} to the library's.",
                    plural_s(a)
                ),
            },
        );
    }

    fn assign_material(&mut self, material: Option<ElementId>) {
        let targets = self.assignable();
        if targets.is_empty() {
            self.status = "Select walls, slabs or masses first.".into();
            return;
        }
        let (tx, n) = library::assign(self.doc.model(), &targets, material);
        let name = material
            .and_then(|m| self.doc.model().get(m))
            .map_or("their type's default".to_string(), |e| e.name.clone());
        let r = self.doc.commit(tx);
        self.report(
            r,
            match n {
                0 => format!("They are already made of {name}."),
                n => format!("{n} element{} now made of {name}.", plural_s(n)),
            },
        );
    }

    pub(super) fn import_materials_dialog(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("Import materials")
            .add_filter("Materials (IFC, FreeCAD, glTF, Blender)", MATERIAL_FILES)
            .add_filter("IFC", &["ifc"])
            .add_filter("FreeCAD material card", &["FCMat", "fcmat"])
            .add_filter("glTF (from Blender or elsewhere)", &["glb", "gltf"])
            .add_filter("Blender", &["blend"])
            .pick_files();
        if let Some(files) = picked {
            let paths: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
            self.import_materials(&paths);
        }
    }

    /// Read the materials of each file and merge them into the project as
    /// one undo step: new names are added, known ones updated.
    pub(super) fn import_materials(&mut self, paths: &[String]) {
        let mut incoming: Vec<(String, Material)> = vec![];
        let mut problems: Vec<String> = vec![];
        for p in paths {
            let read = if p.to_ascii_lowercase().ends_with(".ifc") {
                std::fs::read(p).map_err(|e| e.to_string()).and_then(|b| {
                    creabim_ifc::import_ifc_materials(&String::from_utf8_lossy(&b))
                        .map_err(|e| e.to_string())
                })
            } else {
                creabim_exchange::materials::read_file(std::path::Path::new(p))
                    .map_err(|e| e.to_string())
            };
            match read {
                Ok(m) => incoming.extend(m),
                Err(e) => problems.push(format!("{}: {e}", file_label(p))),
            }
        }
        let found = incoming.len();
        let (tx, r, ids) = library::merge(self.doc.model(), incoming, "Import materials");
        let res = self.doc.commit(tx);
        if let Some(first) = ids.first() {
            self.open_materials(Some(*first));
        }
        let from = match paths {
            [one] => file_label(one),
            _ => format!("{} files", paths.len()),
        };
        let mut msg = format!(
            "Read {found} material{} from {from}: {} added, {} updated, {} unchanged.",
            plural_s(found),
            r.added,
            r.updated,
            r.unchanged
        );
        if !problems.is_empty() {
            msg = format!("{msg} Couldn't read {}", problems.join("; "));
        }
        self.report(res, msg);
    }

    /// The material browser window, when open.
    pub(super) fn materials_window(&mut self, ctx: &egui::Context) {
        // Save or Cancel pressed last frame: kept, or put back
        if let Some(keep) = self.take_dialog_close("materials") {
            self.dialog_end("materials", keep);
            self.mat.open = false;
        }
        if !self.mat.open {
            self.dialog_end("materials", true);
            return;
        }
        self.dialog_begin("materials");
        let mut dlg_footer: Option<bool> = None;
        let mut open = true;
        let mut asks = vec![];
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        crate::app::shell::window("Materials")
            .open(&mut open)
            .default_pos(area.min + Vec2::new(16.0, 16.0))
            .constrain_to(area)
            .default_size([780.0, (area.height() - 40.0).clamp(360.0, 640.0)])
            .min_width(640.0)
            .collapsible(false)
            .show(ctx, |ui| {
                crate::props::flow(ui, |ui| {
                    ui.selectable_value(&mut self.mat.tab, MatTab::Project, "In this project");
                    ui.selectable_value(&mut self.mat.tab, MatTab::Library, "Library");
                    ui.separator();
                    ui.add(
                        egui::TextEdit::singleline(&mut self.mat.filter)
                            .vertical_align(egui::Align::Center)
                            .hint_text("Find")
                            .desired_width(140.0),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("Import…")
                            .on_hover_text("IFC, FreeCAD .FCMat, glTF/.glb or .blend")
                            .clicked()
                        {
                            asks.push(Ask::Import);
                        }
                    });
                });
                ui.separator();
                match self.mat.tab {
                    MatTab::Project => self.project_tab(ui, &mut asks),
                    MatTab::Library => self.library_tab(ui, &mut asks),
                }
                dlg_footer = Self::dialog_footer(ui).or(dlg_footer);
            });
        if let Some(keep) = dlg_footer {
            self.dialog_close.push(("materials", keep));
        }
        self.mat.open = open;
        for ask in asks {
            self.run_ask(ask);
        }
    }

    fn project_tab(&self, ui: &mut egui::Ui, asks: &mut Vec<Ask>) {
        let model = self.doc.model();
        let mats = library::materials(model);
        let current = self
            .mat
            .current
            .filter(|c| mats.iter().any(|(id, _, _)| id == c))
            .or_else(|| mats.first().map(|m| m.0));
        let f = self.mat.filter.trim().to_lowercase();
        let height = ui.available_height().max(360.0);
        ui.horizontal_top(|ui| {
            // the list, by category
            ui.vertical(|ui| {
                ui.set_width(250.0);
                egui::ScrollArea::vertical()
                    .id_salt("mat_list")
                    .max_height(height - 40.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut cats: Vec<String> =
                            mats.iter().map(|(_, _, m)| m.category.clone()).collect();
                        cats.sort_by_key(|c| {
                            (
                                CATEGORIES
                                    .iter()
                                    .position(|(v, _)| v == c)
                                    .unwrap_or(usize::MAX),
                                c.clone(),
                            )
                        });
                        cats.dedup();
                        for cat in cats {
                            let rows: Vec<_> = mats
                                .iter()
                                .filter(|(_, n, m)| {
                                    m.category == cat
                                        && (f.is_empty() || n.to_lowercase().contains(&f))
                                })
                                .collect();
                            if rows.is_empty() {
                                continue;
                            }
                            ui.label(
                                egui::RichText::new(library::category_label(&cat).to_uppercase())
                                    .size(10.5)
                                    .strong()
                                    .color(ui.visuals().weak_text_color()),
                            );
                            for (id, name, m) in rows {
                                let users = library::users(model, *id).len();
                                crate::props::flow(ui, |ui| {
                                    if swatch(ui, m, 16.0).clicked() {
                                        asks.push(Ask::Pick(*id));
                                    }
                                    let text = if users > 0 {
                                        format!("{name}  ·  {users}")
                                    } else {
                                        name.to_string()
                                    };
                                    let r = ui
                                        .selectable_label(current == Some(*id), text)
                                        .on_hover_text(match users {
                                            0 => "Not used".to_string(),
                                            n => format!("Used by {n} element{}", plural_s(n)),
                                        });
                                    if r.clicked() {
                                        asks.push(Ask::Pick(*id));
                                    }
                                });
                            }
                            ui.add_space(4.0);
                        }
                        if mats.is_empty() {
                            ui.label(
                                egui::RichText::new(
                                    "No materials yet. Add some from the Library tab, or import them.",
                                )
                                .weak(),
                            );
                        }
                    });
                crate::props::flow(ui, |ui| {
                    if ui.button("New").clicked() {
                        asks.push(Ask::New);
                    }
                    if let Some(c) = current {
                        if ui.button("Duplicate").clicked() {
                            asks.push(Ask::Duplicate(c));
                        }
                        if ui.button("Delete").clicked() {
                            asks.push(Ask::Delete(c));
                        }
                    }
                });
            });
            ui.separator();
            // the editor
            ui.vertical(|ui| {
                let Some(id) = current else {
                    return;
                };
                let Some(e) = model.get(id) else { return };
                let ElementKind::Material(m) = &e.kind else {
                    return;
                };
                crate::props::flow(ui, |ui| {
                    preview(ui, m, 96.0);
                    ui.vertical(|ui| {
                        let mut name = e.name.clone();
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut name).vertical_align(egui::Align::Center)
                                    .font(egui::TextStyle::Heading)
                                    .desired_width(240.0),
                            )
                            .changed()
                        {
                            asks.push(Ask::Rename(id, name));
                        }
                        ui.label(
                            egui::RichText::new(format!(
                                "IfcMaterial · {}",
                                library::category_label(&m.category)
                            ))
                            .weak(),
                        );
                        let users = library::users(model, id);
                        crate::props::flow(ui, |ui| {
                            ui.label(match users.len() {
                                0 => "Not used yet.".to_string(),
                                n => format!("Used by {n} element{}.", plural_s(n)),
                            });
                            if !users.is_empty() && ui.small_button("Select them").clicked() {
                                asks.push(Ask::SelectUsers(id));
                            }
                        });
                        let targets = self.assignable().len();
                        let r = ui.add_enabled(
                            targets > 0,
                            egui::Button::new(match targets {
                                0 => "Assign to the selection".to_string(),
                                1 => "Assign to the selected element".to_string(),
                                n => format!("Assign to the {n} selected elements"),
                            }),
                        );
                        if r.on_disabled_hover_text("Select walls, slabs or masses first")
                            .clicked()
                        {
                            asks.push(Ask::Assign(Some(id)));
                        }
                    });
                });
                ui.add_space(8.0);
                let mut edited = m.clone();
                let pats: Vec<creabim_core::styles::HatchPattern> =
                    creabim_core::styles::active_pack(self.doc.model()).patterns.clone();
                let label = material_fields(ui, &mut edited, &pats);
                if !label.is_empty() {
                    asks.push(Ask::Edit(id, label, edited));
                }
                if let Some((del, replacement)) = self.mat.delete.filter(|(d, _)| *d == id) {
                    ui.add_space(10.0);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        let n = library::users(model, del).len();
                        ui.label(format!(
                            "{} is used by {n} element{}. What should they be made of instead?",
                            e.name,
                            plural_s(n)
                        ));
                        let mut choice = replacement;
                        let shown = choice
                            .and_then(|c| model.get(c))
                            .map_or("By type (no material)".to_string(), |e| e.name.clone());
                        egui::ComboBox::from_id_salt("mat_replace").icon(crate::props::combo_icon)
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut choice, None, "By type (no material)");
                                for (other, name, _) in &mats {
                                    if *other != del {
                                        ui.selectable_value(&mut choice, Some(*other), *name);
                                    }
                                }
                            });
                        if choice != replacement {
                            asks.push(Ask::Replacement(del, choice));
                        }
                        crate::props::flow(ui, |ui| {
                            if ui.button("Delete and reassign").clicked() {
                                asks.push(Ask::ConfirmDelete(del, choice));
                            }
                            if ui.button("Cancel").clicked() {
                                asks.push(Ask::Pick(id));
                            }
                        });
                    });
                }
            });
        });
    }

    fn library_tab(&self, ui: &mut egui::Ui, asks: &mut Vec<Ask>) {
        let model = self.doc.model();
        let have: Vec<&str> = library::materials(model)
            .into_iter()
            .map(|(_, n, _)| n)
            .collect();
        let f = self.mat.filter.trim().to_lowercase();
        ui.label(
            egui::RichText::new(format!(
                "{} common building materials, with IFC categories. Adding one that the project has already resets it to the library's.",
                LIBRARY.len()
            ))
            .weak(),
        );
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .id_salt("mat_library")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // one grid, so the columns line up across categories
                egui::Grid::new("lib_grid")
                    .num_columns(5)
                    .spacing([10.0, 5.0])
                    .show(ui, |ui| {
                        for (cat, label) in CATEGORIES {
                            let rows: Vec<usize> = (0..LIBRARY.len())
                                .filter(|i| {
                                    let e = &LIBRARY[*i];
                                    e.category == *cat
                                        && (f.is_empty()
                                            || e.name.to_lowercase().contains(&f)
                                            || e.description.to_lowercase().contains(&f))
                                })
                                .collect();
                            if rows.is_empty() {
                                continue;
                            }
                            ui.label("");
                            ui.label(
                                egui::RichText::new(label.to_uppercase())
                                    .size(10.5)
                                    .strong()
                                    .color(ui.visuals().weak_text_color()),
                            );
                            ui.label(egui::RichText::new(format!("IFC category: {cat}")).weak());
                            ui.label("");
                            if rows.len() > 1 && ui.small_button("Add all").clicked() {
                                asks.push(Ask::AddLibrary(rows.clone()));
                            }
                            ui.end_row();
                            for i in rows {
                                let e = &LIBRARY[i];
                                swatch(ui, &e.material(), 16.0);
                                ui.label(e.name);
                                ui.label(egui::RichText::new(e.description).weak());
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{}{}",
                                        e.hatch.label(),
                                        match (e.metallic > 0.5, e.opacity < 1.0) {
                                            (true, _) => ", metal",
                                            (_, true) => ", see-through",
                                            _ => "",
                                        }
                                    ))
                                    .weak(),
                                );
                                let inside = have.contains(&e.name);
                                if ui
                                    .small_button(if inside { "In project ↺" } else { "Add" })
                                    .on_hover_text(if inside {
                                        "Reset the project's material to the library's"
                                    } else {
                                        "Add it to the project"
                                    })
                                    .clicked()
                                {
                                    asks.push(Ask::AddLibrary(vec![i]));
                                }
                                ui.end_row();
                            }
                        }
                    });
            });
    }

    fn run_ask(&mut self, ask: Ask) {
        match ask {
            Ask::Pick(id) => {
                self.mat.current = Some(id);
                self.mat.delete = None;
            }
            Ask::New => {
                let name = library::unique_name(self.doc.model(), "New material");
                let e = Element::new(
                    name,
                    ElementKind::Material(Material::new(Hatch::Diagonal, [0.8, 0.8, 0.8])),
                );
                let id = e.id;
                let r = self.doc.commit(Transaction::new("New material").insert(e));
                self.mat.current = Some(id);
                self.report(r, "New material. Name it and set its look.");
            }
            Ask::Duplicate(id) => {
                if let Some(e) = library::duplicate(self.doc.model(), id) {
                    let new = e.id;
                    let r = self
                        .doc
                        .commit(Transaction::new("Duplicate material").insert(e));
                    self.mat.current = Some(new);
                    self.report(r, "Duplicated the material.");
                }
            }
            Ask::Delete(id) => {
                if library::users(self.doc.model(), id).is_empty() {
                    self.run_ask(Ask::ConfirmDelete(id, None));
                } else {
                    self.mat.delete = Some((id, None));
                }
            }
            Ask::Replacement(id, replacement) => self.mat.delete = Some((id, replacement)),
            Ask::ConfirmDelete(id, replacement) => {
                let name = self
                    .doc
                    .model()
                    .get(id)
                    .map(|e| e.name.clone())
                    .unwrap_or_default();
                let tx = library::delete(self.doc.model(), id, replacement);
                let r = self.doc.commit(tx);
                self.mat.delete = None;
                self.mat.current = None;
                self.selection.retain(|s| s != id);
                self.report(r, format!("Deleted {name}. Ctrl+Z brings it back."));
            }
            Ask::Rename(id, name) => {
                let r = self
                    .doc
                    .edit_merged(id, "Rename material", |e| e.name = name.clone());
                self.report(r, "Renamed.");
            }
            Ask::Edit(id, label, m) => {
                let r = self.doc.edit_merged(id, label, |e| {
                    e.kind = ElementKind::Material(m.clone());
                });
                self.report(r, format!("{label}."));
            }
            Ask::SelectUsers(id) => {
                let users = library::users(self.doc.model(), id);
                self.selection.extend(users.iter().copied(), false);
                self.status = format!("Selected {} element{}.", users.len(), plural_s(users.len()));
            }
            Ask::Assign(m) => {
                // once assigned, the window is done with: the model shows
                let any = !self.assignable().is_empty();
                self.assign_material(m);
                if any {
                    self.mat.open = false;
                }
            }
            Ask::AddLibrary(entries) => {
                self.add_library(&entries);
                self.mat.tab = MatTab::Library;
            }
            Ask::Import => self.import_materials_dialog(),
        }
    }
}

fn plural_s(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// A small square of a built-in hatch, as it fills a cut.
pub(crate) fn hatch_swatch(ui: &mut egui::Ui, h: Hatch, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let p = ui.painter_at(rect);
    let g = (creabim_geom::paper::hatch_fill(h) * 255.0) as u8;
    p.rect_filled(rect, 2.0, egui::Color32::from_gray(g));
    let side = 1.0;
    let poly = [
        glam::DVec2::ZERO,
        glam::DVec2::new(side, 0.0),
        glam::DVec2::new(side, side),
        glam::DVec2::new(0.0, side),
    ];
    let to = |q: glam::DVec2| {
        egui::pos2(
            rect.min.x + q.x as f32 * size,
            rect.max.y - q.y as f32 * size,
        )
    };
    let stroke = egui::Stroke::new(0.8, egui::Color32::from_gray(60));
    for (a, b) in creabim_geom::plan::hatch_lines(h, &poly, 0.18) {
        p.line_segment([to(a), to(b)], stroke);
    }
    p.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(140)),
        egui::StrokeKind::Inside,
    );
    resp
}

/// A small square of a style pack's pattern.
pub(crate) fn pattern_swatch(
    ui: &mut egui::Ui,
    pat: &creabim_core::styles::HatchPattern,
    size: f32,
) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let p = ui.painter_at(rect);
    let fill = pat.fill.map_or([1.0; 3], |c| c.0);
    p.rect_filled(
        rect,
        2.0,
        egui::Color32::from_rgb(
            (fill[0] * 255.0) as u8,
            (fill[1] * 255.0) as u8,
            (fill[2] * 255.0) as u8,
        ),
    );
    // the square stands for 10 mm of paper
    let side = 0.5;
    let k = 20.0;
    let poly = [
        glam::DVec2::ZERO,
        glam::DVec2::new(side, 0.0),
        glam::DVec2::new(side, side),
        glam::DVec2::new(0.0, side),
    ];
    let to = |q: glam::DVec2| {
        egui::pos2(
            rect.min.x + (q.x / side) as f32 * size,
            rect.max.y - (q.y / side) as f32 * size,
        )
    };
    let stroke = egui::Stroke::new(0.8, egui::Color32::from_gray(60));
    for (a, b) in creabim_geom::looks::pattern_lines(pat, &poly, k) {
        let (a, b) = (to(a), to(b));
        if a.distance(b) < 0.8 {
            p.circle_filled(a, 0.6, stroke.color);
        } else {
            p.line_segment([a, b], stroke);
        }
    }
    p.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(140)),
        egui::StrokeKind::Inside,
    );
}

thread_local! {
    /// A library pattern picked for a material, still to join the project.
    static PENDING_PATTERN: std::cell::RefCell<Option<creabim_core::styles::HatchPattern>> =
        const { std::cell::RefCell::new(None) };
}

impl CreaBimApp {
    /// A library pattern a material was just given joins the style pack.
    pub(super) fn add_pending_pattern(&mut self) {
        let Some(p) = PENDING_PATTERN.with(|c| c.borrow_mut().take()) else {
            return;
        };
        let (id, mut pack) = self.edited_pack();
        if !pack.patterns.iter().any(|x| x.name == p.name) {
            pack.patterns.push(p);
            self.commit_pack(id, pack, "Add hatch pattern");
        }
    }
}
