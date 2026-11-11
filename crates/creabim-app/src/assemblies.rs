//! The assembly editor: the project's layered wall, floor and roof
//! assemblies (add, duplicate, rename, delete, assign to the selection),
//! each a list of layers to add, remove, reorder and set: function,
//! material and thickness. The same layer table shows in Properties for an
//! assembly picked in the project browser. Every change is an ordinary
//! transaction (dragging a thickness is one undo step), and changing an
//! assembly changes the thickness of everything built of it.

use super::*;
use crate::ui_model::Item;
use creabim_core::assembly::{self, AssemblyUse, Layer, LayerFunction, WrapSides};
use creabim_core::Assembly;

/// The assembly editor's own state, for the session.
#[derive(Default)]
pub(super) struct AssembliesUi {
    pub open: bool,
    /// The assembly being edited.
    current: Option<ElementId>,
}

/// What a click in the editor asks for, done once it is drawn.
pub(super) enum AsmAsk {
    Pick(ElementId),
    New(AssemblyUse),
    Starters,
    Duplicate(ElementId),
    Delete(ElementId),
    Rename(ElementId, String),
    Edit(ElementId, &'static str, Assembly),
    SelectUsers(ElementId),
    Assign(Option<ElementId>),
}

/// The small drawn buttons of a layer row (the UI font has no arrows).
#[derive(Clone, Copy)]
enum Glyph {
    Up,
    Down,
    Remove,
}

struct GlyphButton(Glyph);

impl egui::Widget for GlyphButton {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
        let v = ui.style().interact(&resp);
        let p = ui.painter();
        p.rect(
            rect,
            3.0,
            v.weak_bg_fill,
            v.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let c = rect.center();
        let color = if ui.is_enabled() {
            v.fg_stroke.color
        } else {
            ui.visuals().weak_text_color()
        };
        let stroke = egui::Stroke::new(1.5, color);
        let r = 4.0;
        match self.0 {
            Glyph::Up | Glyph::Down => {
                let s = if matches!(self.0, Glyph::Up) {
                    -1.0
                } else {
                    1.0
                };
                p.line_segment(
                    [
                        c + Vec2::new(-r, -s * r / 2.0),
                        c + Vec2::new(0.0, s * r / 2.0),
                    ],
                    stroke,
                );
                p.line_segment(
                    [
                        c + Vec2::new(0.0, s * r / 2.0),
                        c + Vec2::new(r, -s * r / 2.0),
                    ],
                    stroke,
                );
            }
            Glyph::Remove => {
                p.line_segment([c + Vec2::new(-r, -r), c + Vec2::new(r, r)], stroke);
                p.line_segment([c + Vec2::new(-r, r), c + Vec2::new(r, -r)], stroke);
            }
        }
        resp
    }
}

/// The colour a layer is drawn in: its material's, or one for its function.
fn layer_fill(model: &Model, l: &Layer) -> (Color32, creabim_core::Hatch) {
    let m = l.material.and_then(|m| match &model.get(m)?.kind {
        ElementKind::Material(m) => Some(m.clone()),
        _ => None,
    });
    match m {
        Some(m) => {
            let c = m.color.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
            (Color32::from_rgb(c[0], c[1], c[2]), m.hatch)
        }
        None => match l.function {
            LayerFunction::AirGap => (Color32::from_rgb(245, 247, 250), Hatch::Empty),
            LayerFunction::Insulation => (Color32::from_rgb(236, 220, 120), Hatch::Empty),
            _ => (Color32::from_gray(200), Hatch::Diagonal),
        },
    }
}

/// The assembly drawn as a cut strip: a wall's layers side by side
/// (exterior on the left), a floor's or roof's stacked (top up), each
/// in its material's colour with its cut hatch.
pub(super) fn strip(ui: &mut egui::Ui, model: &Model, a: &Assembly, width: f32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter_at(rect);
    let total = a.total().max(1e-9);
    let vertical = a.usage == AssemblyUse::Wall;
    let ink = ui.visuals().text_color();
    let mut at = 0.0;
    for l in &a.layers {
        let k0 = (at / total) as f32;
        at += l.thickness;
        let k1 = (at / total) as f32;
        let r = if vertical {
            Rect::from_min_max(
                egui::pos2(rect.left() + rect.width() * k0, rect.top()),
                egui::pos2(rect.left() + rect.width() * k1, rect.bottom()),
            )
        } else {
            Rect::from_min_max(
                egui::pos2(rect.left(), rect.top() + rect.height() * k0),
                egui::pos2(rect.right(), rect.top() + rect.height() * k1),
            )
        };
        let (fill, hatch) = layer_fill(model, l);
        painter.rect_filled(r, 0.0, fill);
        let poly: Vec<DVec2> = [
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
        ]
        .iter()
        .map(|p| DVec2::new(p.x as f64, p.y as f64))
        .collect();
        let line = egui::Stroke::new(0.7, Color32::from_black_alpha(140));
        for (p, q) in creabim_geom::plan::hatch_lines(hatch, &poly, 6.0) {
            painter.line_segment(
                [
                    egui::pos2(p.x as f32, p.y as f32),
                    egui::pos2(q.x as f32, q.y as f32),
                ],
                line,
            );
        }
        painter.rect_stroke(
            r,
            0.0,
            egui::Stroke::new(0.8, ink),
            egui::StrokeKind::Inside,
        );
    }
    // the core, bracketed
    let (i, j) = a.core();
    let spans = a.spans(1.0);
    if let (Some(s0), Some(s1)) = (spans.get(i), spans.get(j)) {
        let (k0, k1) = (s0.0 as f32, s1.1 as f32);
        let accent = crate::theme::interaction();
        let stroke = egui::Stroke::new(1.5, accent);
        if vertical {
            let (x0, x1) = (
                rect.left() + rect.width() * k0,
                rect.left() + rect.width() * k1,
            );
            painter.line_segment(
                [
                    egui::pos2(x0, rect.bottom() - 2.0),
                    egui::pos2(x1, rect.bottom() - 2.0),
                ],
                stroke,
            );
        } else {
            let (y0, y1) = (
                rect.top() + rect.height() * k0,
                rect.top() + rect.height() * k1,
            );
            painter.line_segment(
                [
                    egui::pos2(rect.right() - 2.0, y0),
                    egui::pos2(rect.right() - 2.0, y1),
                ],
                stroke,
            );
        }
    }
}

/// The layer table: a row per layer with its function, material and
/// thickness, buttons to move it up or down or remove it, and Add layer.
/// Returns an edit when something changed: (undo label, new assembly).
pub(super) fn layer_table(
    ui: &mut egui::Ui,
    model: &Model,
    id: ElementId,
    a: &Assembly,
    compact: bool,
) -> Option<(&'static str, Assembly)> {
    let mats = creabim_core::library::materials(model);
    let mut edited = a.clone();
    let mut label = "";
    let mut remove = None;
    let mut swap = None;
    let weak = ui.visuals().weak_text_color();
    let n = a.layers.len();
    let (core0, core1) = a.core();
    // walls: which layers turn into the reveals of doors and windows
    let wall = a.usage == AssemblyUse::Wall;
    let wrapping = !a.wrap_openings.is_none();
    ui.label(
        egui::RichText::new(format!("{} face", a.usage.first_face()))
            .size(10.5)
            .color(weak),
    );
    egui::Grid::new(("layers", id))
        .num_columns(if wall { 6 } else { 5 })
        .spacing([6.0, 4.0])
        .show(ui, |ui| {
            for (i, l) in edited.layers.iter_mut().enumerate() {
                let (fill, _) = layer_fill(model, l);
                let (r, _) = ui.allocate_exact_size(Vec2::new(10.0, 18.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, fill);
                if (core0..=core1).contains(&i) {
                    ui.painter().line_segment(
                        [r.left_top(), r.left_bottom()],
                        egui::Stroke::new(2.0, crate::theme::interaction()),
                    );
                }
                egui::ComboBox::from_id_salt(("fn", id, i))
                    .icon(crate::props::combo_icon)
                    .width(if compact { 78.0 } else { 96.0 })
                    .selected_text(l.function.label())
                    .show_ui(ui, |ui| {
                        for f in LayerFunction::ALL {
                            if ui.selectable_value(&mut l.function, f, f.label()).changed() {
                                label = "Layer function";
                            }
                        }
                    });
                let shown = l
                    .material
                    .and_then(|m| mats.iter().find(|x| x.0 == m))
                    .map_or("None", |x| x.1)
                    .to_string();
                egui::ComboBox::from_id_salt(("mat", id, i))
                    .icon(crate::props::combo_icon)
                    .width(if compact { 110.0 } else { 190.0 })
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_value(&mut l.material, None, "None (air, or by type)")
                            .changed()
                        {
                            label = "Layer material";
                        }
                        for (mid, name, _) in &mats {
                            if ui
                                .selectable_value(&mut l.material, Some(*mid), *name)
                                .changed()
                            {
                                label = "Layer material";
                            }
                        }
                    });
                let mut t = l.thickness;
                if ui
                    .add(
                        crate::units::drag(&mut t)
                            .speed(0.001)
                            .range(assembly::MIN_LAYER..=2.0),
                    )
                    .on_hover_text("Thickness; type any unit")
                    .changed()
                {
                    l.thickness = t.max(assembly::MIN_LAYER);
                    label = "Layer thickness";
                }
                if wall {
                    let mut wraps = l.wraps_openings();
                    if ui
                        .add_enabled(wrapping, egui::Checkbox::new(&mut wraps, "Wraps"))
                        .on_hover_text("At doors and windows, the layer turns into the reveal and runs to the frame, on the faces chosen under Wrap at openings.")
                        .on_disabled_hover_text("Choose a face under Wrap at openings first.")
                        .changed()
                    {
                        // its function's own answer isn't kept: a finish wraps
                        let by_function = l.function == LayerFunction::Finish;
                        l.wraps = (wraps != by_function).then_some(wraps);
                        label = "Layer wraps at openings";
                    }
                }
                crate::props::flow(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if ui
                        .add_enabled(i > 0, GlyphButton(Glyph::Up))
                        .on_hover_text("Move the layer out")
                        .clicked()
                    {
                        swap = Some((i - 1, i));
                    }
                    if ui
                        .add_enabled(i + 1 < n, GlyphButton(Glyph::Down))
                        .on_hover_text("Move the layer in")
                        .clicked()
                    {
                        swap = Some((i, i + 1));
                    }
                    if ui
                        .add_enabled(n > 1, GlyphButton(Glyph::Remove))
                        .on_hover_text("Remove the layer")
                        .clicked()
                    {
                        remove = Some(i);
                    }
                });
                ui.end_row();
            }
        });
    ui.label(
        egui::RichText::new(format!("{} face", a.usage.last_face()))
            .size(10.5)
            .color(weak),
    );
    if let Some((i, j)) = swap {
        edited.layers.swap(i, j);
        label = "Reorder layers";
    }
    if let Some(i) = remove {
        edited.layers.remove(i);
        label = "Remove layer";
    }
    if wall {
        crate::props::flow(ui, |ui| {
            ui.label("Wrap at openings");
            egui::ComboBox::from_id_salt(("wrap", id))
                .icon(crate::props::combo_icon)
                .width(if compact { 110.0 } else { 140.0 })
                .selected_text(edited.wrap_openings.label())
                .show_ui(ui, |ui| {
                    for w in WrapSides::ALL {
                        if ui
                            .selectable_value(&mut edited.wrap_openings, w, w.label())
                            .changed()
                        {
                            label = "Wrap layers at openings";
                        }
                    }
                })
                .response
                .on_hover_text("At doors and windows, the layers ticked Wraps turn into the reveals from these faces of the wall and run to the frame; the layers they cover stop short. A door or window can go its own way in its Properties.");
        });
    }
    crate::props::flow(ui, |ui| {
        if ui
            .button("Add layer")
            .on_hover_text("A new layer on the inner (or lower) side")
            .clicked()
        {
            edited
                .layers
                .push(Layer::new(LayerFunction::Finish, None, 0.013));
            label = "Add layer";
        }
        ui.label(format!(
            "Total {}",
            crate::units::len(edited.layers.iter().map(|l| l.thickness).sum())
        ));
    });
    (!label.is_empty()).then_some((label, edited))
}

impl CreaBimApp {
    pub(super) fn open_assemblies(&mut self, current: Option<ElementId>) {
        self.asm.open = true;
        if current.is_some() {
            self.asm.current = current;
        }
    }

    /// The selected elements an assembly could go on.
    fn assembly_targets(&self) -> Vec<ElementId> {
        self.selection
            .all()
            .into_iter()
            .filter(|id| {
                self.doc
                    .model()
                    .get(*id)
                    .is_some_and(|e| assembly::assembly_slot(e).is_some())
            })
            .collect()
    }

    /// The Materials menu's assembly items.
    pub(super) fn assemblies_menu(&self) -> Vec<Item> {
        let targets = self.assembly_targets();
        let model = self.doc.model();
        let mut assign = vec![Item::new("asm.assign:none", "None (single material)")];
        for (id, name, a) in assembly::assemblies(model) {
            let fits = targets.iter().any(|t| {
                model
                    .get(*t)
                    .is_some_and(|e| AssemblyUse::of(&e.kind) == Some(a.usage))
            });
            assign.push(
                Item::new(format!("asm.assign:{}", id.0), name)
                    .hint(a.summary(model))
                    .enabled(fits),
            );
        }
        vec![
            Item::Separator,
            Item::new("asm.browser", "Assemblies…")
                .icon("layers-2")
                .hint("Layered wall, floor and roof types: add, edit their layers, assign"),
            Item::new("asm.starters", "Add starter assemblies")
                .icon("plus")
                .hint(
                "A cavity wall, a partition, a floor and a pitched roof, with the materials they use",
            ),
            Item::sub("Assign assembly", assign)
                .icon("layers-2")
                .enabled(!targets.is_empty()),
        ]
    }

    /// Run an assembly menu action. False when the action isn't one.
    pub(super) fn assemblies_action(&mut self, head: &str, arg: &str) -> bool {
        match head {
            "asm.browser" => self.open_assemblies(None),
            "asm.starters" => self.run_asm(AsmAsk::Starters),
            "asm.assign" => {
                let a = self
                    .doc
                    .model()
                    .iter()
                    .find(|e| e.id.0.to_string() == arg)
                    .map(|e| e.id);
                self.run_asm(AsmAsk::Assign(a));
            }
            _ => return self.types_action(head, arg),
        }
        true
    }

    /// The assembly picker in Properties for a wall, slab or roof: which
    /// assembly it is built of, and its layers. True when it changed.
    pub(super) fn assembly_combo(
        &mut self,
        ui: &mut egui::Ui,
        usage: AssemblyUse,
        current: &mut Option<ElementId>,
    ) -> bool {
        let model = self.doc.model();
        let choices: Vec<(ElementId, String)> = assembly::assemblies(model)
            .into_iter()
            .filter(|(_, _, a)| a.usage == usage)
            .map(|(id, n, _)| (id, n.to_string()))
            .collect();
        let shown = current
            .and_then(|c| choices.iter().find(|x| x.0 == c))
            .map_or("None (single material)".to_string(), |x| x.1.clone());
        let mut changed = false;
        let mut create = false;
        let mut edit = false;
        crate::props::flow(ui, |ui| {
            crate::app::helpui::combo(ui, "Assembly")
                .selected_text(shown)
                .width(ui.available_width().min(150.0))
                .show_ui(ui, |ui| {
                    changed |= ui
                        .selectable_value(current, None, "None (single material)")
                        .changed();
                    for (id, n) in &choices {
                        changed |= ui.selectable_value(current, Some(*id), n).changed();
                    }
                    ui.separator();
                    create = ui.button("New assembly…").clicked();
                });
        });
        if let Some((_, a)) =
            current
                .and_then(|c| self.doc.model().get(c))
                .and_then(|e| match &e.kind {
                    ElementKind::Assembly(a) => Some((e, a.clone())),
                    _ => None,
                })
        {
            strip(
                ui,
                self.doc.model(),
                &a,
                ui.available_width().min(240.0),
                26.0,
            );
            ui.label(egui::RichText::new(a.summary(self.doc.model())).weak());
            edit = ui
                .button("Edit layers…")
                .on_hover_text("Change this assembly: every element built of it follows")
                .clicked();
        }
        if create {
            let a = Assembly::new(usage, vec![Layer::new(LayerFunction::Structure, None, 0.2)]);
            let e = Element::new(
                assembly::unique_name(self.doc.model(), "New assembly"),
                ElementKind::Assembly(a),
            );
            *current = Some(e.id);
            let r = self.doc.commit(Transaction::new("New assembly").insert(e));
            self.open_assemblies(*current);
            self.report(r, "New assembly. Set its layers in the assembly editor.");
            changed = true;
        }
        if edit {
            self.open_assemblies(*current);
        }
        changed
    }

    /// Properties for an assembly picked in the project browser.
    pub(super) fn assembly_properties(&mut self, ui: &mut egui::Ui, id: ElementId) {
        let model = self.doc.model();
        let Some(ElementKind::Assembly(a)) = model.get(id).map(|e| &e.kind) else {
            return;
        };
        let a = a.clone();
        ui.label(egui::RichText::new(format!("For {}", a.usage.label().to_lowercase())).weak());
        strip(ui, model, &a, ui.available_width().min(240.0), 30.0);
        let edit = layer_table(ui, model, id, &a, true);
        let users = assembly::users(model, id).len();
        ui.label(
            egui::RichText::new(match users {
                0 => "Not used yet.".to_string(),
                n => format!("Used by {n} element{}.", if n == 1 { "" } else { "s" }),
            })
            .weak(),
        );
        let open = ui.button("Open in the assembly editor").clicked();
        if let Some((label, new)) = edit {
            self.run_asm(AsmAsk::Edit(id, label, new));
        }
        if open {
            self.open_assemblies(Some(id));
        }
    }

    /// The assembly editor window, when open.
    pub(super) fn assemblies_window(&mut self, ctx: &egui::Context) {
        if !self.asm.open {
            return;
        }
        let mut open = true;
        let mut asks = vec![];
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        crate::app::shell::window("Assemblies")
            .open(&mut open)
            .default_pos(area.min + Vec2::new(24.0, 24.0))
            .constrain_to(area)
            .default_size([760.0, (area.height() - 40.0).clamp(360.0, 600.0)])
            .min_width(620.0)
            .collapsible(false)
            .show(ctx, |ui| self.assemblies_body(ui, &mut asks));
        self.asm.open = open;
        for ask in asks {
            self.run_asm(ask);
        }
    }

    fn assemblies_body(&self, ui: &mut egui::Ui, asks: &mut Vec<AsmAsk>) {
        let model = self.doc.model();
        let all = assembly::assemblies(model);
        let current = self
            .asm
            .current
            .filter(|c| all.iter().any(|(id, _, _)| id == c))
            .or_else(|| all.first().map(|a| a.0));
        let height = ui.available_height().max(320.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(230.0);
                egui::ScrollArea::vertical()
                    .id_salt("asm_list")
                    .max_height(height - 70.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for usage in AssemblyUse::ALL {
                            let rows: Vec<_> = all.iter().filter(|a| a.2.usage == usage).collect();
                            // a bold heading that folds, so a long list stays
                            // easy to scan (MU18)
                            let head = egui::RichText::new(format!(
                                "{}  ({})",
                                usage.label(),
                                rows.len()
                            ))
                            .size(13.0)
                            .strong();
                            let open = rows.iter().any(|(id, _, _)| current == Some(*id))
                                || rows.len() <= 6;
                            let fold = egui::CollapsingHeader::new(head)
                                .id_salt(("asm_group", usage.label()))
                                .default_open(open)
                                .show(ui, |ui| {
                            for (id, name, a) in rows {
                                let users = assembly::users(model, *id).len();
                                let text = format!(
                                    "{name}  ·  {}{}",
                                    crate::units::len(a.total()),
                                    if users > 0 {
                                        format!("  ·  {users}")
                                    } else {
                                        String::new()
                                    }
                                );
                                if ui
                                    .selectable_label(current == Some(*id), text)
                                    .on_hover_text(a.summary(model))
                                    .clicked()
                                {
                                    asks.push(AsmAsk::Pick(*id));
                                }
                            }
                            if ui.small_button("+ New").clicked() {
                                asks.push(AsmAsk::New(usage));
                            }
                                });
                            let _ = fold;
                            ui.separator();
                        }
                        if all.is_empty() {
                            ui.label(
                                egui::RichText::new(
                                    "No assemblies yet. Add the starters, or make a new one.",
                                )
                                .weak(),
                            );
                        }
                    });
                crate::props::flow(ui, |ui| {
                    if ui
                        .button("Add starters")
                        .on_hover_text("A cavity wall, a partition, a floor and a pitched roof")
                        .clicked()
                    {
                        asks.push(AsmAsk::Starters);
                    }
                    if let Some(c) = current {
                        if ui.button("Duplicate").clicked() {
                            asks.push(AsmAsk::Duplicate(c));
                        }
                        if ui
                            .button("Delete")
                            .on_hover_text("What is built of it keeps its thickness, in one material. Ctrl+Z brings it back.")
                            .clicked()
                        {
                            asks.push(AsmAsk::Delete(c));
                        }
                    }
                });
            });
            ui.separator();
            ui.vertical(|ui| {
                let Some(id) = current else { return };
                let Some(e) = model.get(id) else { return };
                let ElementKind::Assembly(a) = &e.kind else {
                    return;
                };
                let mut name = e.name.clone();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut name).vertical_align(egui::Align::Center)
                            .font(egui::TextStyle::Heading)
                            .desired_width(320.0),
                    )
                    .changed()
                {
                    asks.push(AsmAsk::Rename(id, name));
                }
                ui.label(
                    egui::RichText::new(format!(
                        "IfcMaterialLayerSet · {} · {} total",
                        a.usage.label(),
                        crate::units::len(a.total())
                    ))
                    .weak(),
                );
                let mut description = a.description.clone();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut description).vertical_align(egui::Align::Center)
                            .hint_text("Description")
                            .desired_width(420.0),
                    )
                    .changed()
                {
                    let mut new = a.clone();
                    new.description = description;
                    asks.push(AsmAsk::Edit(id, "Assembly description", new));
                }
                ui.add_space(6.0);
                strip(ui, model, a, ui.available_width().min(460.0), 44.0);
                ui.add_space(6.0);
                if let Some((label, new)) = layer_table(ui, model, id, a, false) {
                    asks.push(AsmAsk::Edit(id, label, new));
                }
                ui.add_space(6.0);
                let (c0, c1) = a.core();
                ui.label(
                    egui::RichText::new(format!(
                        "Core: layer{} {}{} (orange). Layers run from the {} face to the {}. {}",
                        if c0 == c1 { "" } else { "s" },
                        c0 + 1,
                        if c0 == c1 {
                            String::new()
                        } else {
                            format!("–{}", c1 + 1)
                        },
                        a.usage.first_face().to_lowercase(),
                        a.usage.last_face().to_lowercase(),
                        if a.usage == AssemblyUse::Wall {
                            "Where layered walls meet at a T, the core runs on to the other wall's core. A wall's exterior is on its right as drawn (Flip in Properties swaps it)."
                        } else {
                            ""
                        }
                    ))
                    .weak(),
                );
                ui.add_space(6.0);
                let users = assembly::users(model, id);
                crate::props::flow(ui, |ui| {
                    ui.label(match users.len() {
                        0 => "Not used yet.".to_string(),
                        n => format!("Used by {n} element{}.", if n == 1 { "" } else { "s" }),
                    });
                    if !users.is_empty() && ui.small_button("Select them").clicked() {
                        asks.push(AsmAsk::SelectUsers(id));
                    }
                });
                let fits = self
                    .assembly_targets()
                    .iter()
                    .filter(|t| {
                        model
                            .get(**t)
                            .is_some_and(|e| AssemblyUse::of(&e.kind) == Some(a.usage))
                    })
                    .count();
                let r = ui.add_enabled(
                    fits > 0,
                    egui::Button::new(match fits {
                        0 => "Assign to the selection".to_string(),
                        1 => "Assign to the selected element".to_string(),
                        n => format!("Assign to the {n} selected elements"),
                    }),
                );
                if r.on_disabled_hover_text(format!(
                    "Select {} first",
                    a.usage.label().to_lowercase()
                ))
                .clicked()
                {
                    asks.push(AsmAsk::Assign(Some(id)));
                }
            });
        });
    }

    pub(super) fn run_asm(&mut self, ask: AsmAsk) {
        match ask {
            AsmAsk::Pick(id) => self.asm.current = Some(id),
            AsmAsk::New(usage) => {
                let a = Assembly::new(usage, vec![Layer::new(LayerFunction::Structure, None, 0.2)]);
                let e = Element::new(
                    assembly::unique_name(self.doc.model(), "New assembly"),
                    ElementKind::Assembly(a),
                );
                let id = e.id;
                let r = self.doc.commit(Transaction::new("New assembly").insert(e));
                self.asm.current = Some(id);
                self.report(r, "New assembly. Name it and add its layers.");
            }
            AsmAsk::Starters => {
                let (tx, ids) = assembly::add_starters(self.doc.model());
                if tx.ops.is_empty() {
                    self.status = "The starter assemblies are in the project already.".into();
                } else {
                    let r = self.doc.commit(tx);
                    self.report(
                        r,
                        "Added starter assemblies: a cavity wall, a partition, a floor and a pitched roof.",
                    );
                }
                self.open_assemblies(ids.first().copied());
            }
            AsmAsk::Duplicate(id) => {
                if let Some(e) = assembly::duplicate(self.doc.model(), id) {
                    let new = e.id;
                    let r = self
                        .doc
                        .commit(Transaction::new("Duplicate assembly").insert(e));
                    self.asm.current = Some(new);
                    self.report(r, "Duplicated the assembly.");
                }
            }
            AsmAsk::Delete(id) => {
                let name = self
                    .doc
                    .model()
                    .get(id)
                    .map(|e| e.name.clone())
                    .unwrap_or_default();
                let tx = assembly::delete(self.doc.model(), id);
                let r = self.doc.commit(tx);
                self.asm.current = None;
                self.selection.retain(|s| s != id);
                self.report(r, format!("Deleted {name}. Ctrl+Z brings it back."));
            }
            AsmAsk::Rename(id, name) => {
                let r = self
                    .doc
                    .edit_merged(id, "Rename assembly", |e| e.name = name.clone());
                self.report(r, "Renamed.");
            }
            AsmAsk::Edit(id, label, a) => {
                if let Err(e) = a.validate() {
                    self.status = format!("Can't: {e}.");
                    return;
                }
                if let Some(tx) = assembly::edit(self.doc.model(), id, a, label) {
                    let n = tx.ops.len().saturating_sub(1);
                    let r = self.doc.commit_merged(tx);
                    self.report(
                        r,
                        match n {
                            0 => format!("{label}."),
                            n => format!(
                                "{label}: {n} element{} built of it follow.",
                                if n == 1 { "" } else { "s" }
                            ),
                        },
                    );
                }
            }
            AsmAsk::SelectUsers(id) => {
                let users = assembly::users(self.doc.model(), id);
                self.selection.extend(users.iter().copied(), false);
                self.status = format!("Selected {} element(s).", users.len());
            }
            AsmAsk::Assign(a) => {
                let targets = self.assembly_targets();
                if targets.is_empty() {
                    self.status = "Select walls, slabs or roofs first.".into();
                    return;
                }
                let (tx, n) = assembly::assign(self.doc.model(), &targets, a);
                let name = a
                    .and_then(|a| self.doc.model().get(a))
                    .map_or("one material".to_string(), |e| e.name.clone());
                let r = self.doc.commit(tx);
                self.report(
                    r,
                    match n {
                        0 => format!("Nothing selected takes {name}."),
                        n => format!(
                            "{n} element{} now built of {name}.",
                            if n == 1 { "" } else { "s" }
                        ),
                    },
                );
            }
        }
    }
}
