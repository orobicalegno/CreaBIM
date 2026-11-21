//! Element types in the app: the Types window (the project's wall,
//! curtain wall, door, window, slab, roof, column and beam types: add,
//! duplicate, rename, delete, edit, select their instances, assign them to
//! the selection), the type picker at the top of Properties (with what the
//! element overrides, reset, update the type from it, make a type of it)
//! and the type each tool draws new elements as. Every change is an
//! ordinary transaction, and changing a type changes all its instances but
//! for what each overrides.

use super::*;
use crate::ui_model::Item;
use creabim_core::assembly::{self, AssemblyUse};
use creabim_core::curtain::{CurtainGrid, GridRule, PanelKind};
use creabim_core::types::{self, ElementType, TypeCategory, TypeParams};

/// The Types window's own state, for the session.
#[derive(Default)]
pub(super) struct TypesUi {
    pub open: bool,
    /// The type being edited.
    current: Option<ElementId>,
}

/// What a click asks for, done once the window or panel is drawn.
pub(super) enum TypeAsk {
    Pick(ElementId),
    New(TypeCategory, bool),
    Starters,
    Duplicate(ElementId),
    Delete(ElementId),
    Rename(ElementId, String),
    Edit(ElementId, &'static str, ElementType),
    /// The layers of a type's assembly changed (assembly, label, new).
    Layers(ElementId, &'static str, creabim_core::assembly::Assembly),
    /// Give the type a copy of the layers it shares, its own to change.
    OwnLayers(ElementId),
    SelectInstances(ElementId),
    /// Make the selected elements (those it fits) of this type, or of none.
    Assign(Option<ElementId>),
    /// An instance: drop its overrides.
    Reset(ElementId),
    /// An instance: write its values into its type.
    SaveToType(ElementId),
    /// An element: make a new type of it.
    FromElement(ElementId),
    /// Open the Catalogue window.
    Catalogue,
}

/// The groups of the Types window, in order: (heading, category, curtain).
const GROUPS: [(&str, TypeCategory, bool); 10] = [
    ("Walls", TypeCategory::Wall, false),
    ("Curtain walls", TypeCategory::Wall, true),
    ("Doors", TypeCategory::Door, false),
    ("Windows", TypeCategory::Window, false),
    ("Slabs", TypeCategory::Slab, false),
    ("Roofs", TypeCategory::Roof, false),
    ("Columns", TypeCategory::Column, false),
    ("Beams", TypeCategory::Beam, false),
    ("Footings", TypeCategory::Footing, false),
    ("Objects", TypeCategory::Object, false),
];

/// The tool that draws elements of a category.
pub(super) fn category_of_tool(tool: Tool) -> Option<TypeCategory> {
    Some(match tool {
        Tool::Wall => TypeCategory::Wall,
        Tool::Door => TypeCategory::Door,
        Tool::Window => TypeCategory::Window,
        Tool::Slab => TypeCategory::Slab,
        Tool::Roof => TypeCategory::Roof,
        Tool::Column => TypeCategory::Column,
        Tool::Beam => TypeCategory::Beam,
        Tool::Footing => TypeCategory::Footing,
        _ => return None,
    })
}

fn len_field(ui: &mut egui::Ui, text: &str, v: &mut f64, lo: f64, hi: f64) -> bool {
    crate::props::row(ui, text, |ui| {
        crate::props::fill(ui, crate::units::drag(v).range(lo..=hi)).changed()
    })
}

fn rule_field(ui: &mut egui::Ui, text: &str, salt: &str, r: &mut GridRule) -> bool {
    let mut changed = false;
    crate::props::flow(ui, |ui| {
        ui.label(text);
        let spacing = matches!(r, GridRule::Spacing(_));
        egui::ComboBox::from_id_salt(("curtain_rule", salt))
            .icon(crate::props::combo_icon)
            .selected_text(r.label())
            .width(ui.available_width().min(80.0))
            .show_ui(ui, |ui| {
                if ui.selectable_label(spacing, "Spacing").clicked() && !spacing {
                    *r = GridRule::Spacing(1.5);
                    changed = true;
                }
                if ui.selectable_label(!spacing, "Count").clicked() && spacing {
                    *r = GridRule::Count(4);
                    changed = true;
                }
            });
        match r {
            GridRule::Spacing(s) => {
                changed |= ui
                    .add(crate::units::drag(s).range(0.2..=20.0))
                    .on_hover_text("A grid line every so far; the last bay takes what is left")
                    .changed()
            }
            GridRule::Count(n) => {
                changed |= ui
                    .add(egui::DragValue::new(n).range(1..=60).suffix(" bays"))
                    .on_hover_text("So many equal bays")
                    .changed()
            }
        }
    });
    changed
}

/// A curtain wall's grid, mullions and panels. Returns the undo label of
/// what changed, or "".
pub(super) fn curtain_fields(ui: &mut egui::Ui, c: &mut CurtainGrid) -> &'static str {
    let mut label = "";
    if rule_field(ui, "Along", "v", &mut c.vertical) {
        label = "Curtain grid";
    }
    if rule_field(ui, "Up", "h", &mut c.horizontal) {
        label = "Curtain grid";
    }
    if len_field(ui, "Mullion width", &mut c.mullion_width, 0.01, 0.5) {
        label = "Mullion size";
    }
    if len_field(ui, "Mullion depth", &mut c.mullion_depth, 0.02, 1.0) {
        label = "Mullion size";
    }
    crate::props::flow(ui, |ui| {
        ui.label("Panels");
        for k in PanelKind::ALL {
            if ui.selectable_value(&mut c.panel, k, k.label()).changed() {
                label = "Curtain panels";
            }
        }
    });
    if len_field(ui, "Panel thickness", &mut c.panel_thickness, 0.004, 0.3) {
        label = "Panel thickness";
    }
    if !c.swapped.is_empty() {
        crate::props::flow(ui, |ui| {
            ui.label(format!("{} panels swapped", c.swapped.len()));
            if ui.small_button("Reset").clicked() {
                c.swapped.clear();
                label = "Curtain panels";
            }
        });
    }
    label
}

fn material_pick(
    ui: &mut egui::Ui,
    model: &Model,
    salt: &str,
    current: &mut Option<ElementId>,
) -> bool {
    let materials: Vec<(ElementId, String)> = model
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Material(_)))
        .map(|e| (e.id, e.name.clone()))
        .collect();
    let shown = current
        .and_then(|c| materials.iter().find(|m| m.0 == c))
        .map_or("By type", |m| m.1.as_str())
        .to_string();
    let mut changed = false;
    crate::props::flow(ui, |ui| {
        ui.label("Material");
        egui::ComboBox::from_id_salt(("type_material", salt))
            .icon(crate::props::combo_icon)
            .selected_text(shown)
            .width(ui.available_width().min(150.0))
            .truncate()
            .show_ui(ui, |ui| {
                changed |= ui.selectable_value(current, None, "By type").changed();
                for (id, n) in &materials {
                    changed |= ui.selectable_value(current, Some(*id), n).changed();
                }
            });
    });
    changed
}

fn assembly_pick(
    ui: &mut egui::Ui,
    model: &Model,
    usage: AssemblyUse,
    current: &mut Option<ElementId>,
) -> bool {
    let choices: Vec<(ElementId, String)> = assembly::assemblies(model)
        .into_iter()
        .filter(|(_, _, a)| a.usage == usage)
        .map(|(id, n, _)| (id, n.to_string()))
        .collect();
    let shown = current
        .and_then(|c| choices.iter().find(|x| x.0 == c))
        .map_or("None (single material)".to_string(), |x| x.1.clone());
    let mut changed = false;
    crate::props::flow(ui, |ui| {
        ui.label("Assembly");
        egui::ComboBox::from_id_salt("type_assembly")
            .icon(crate::props::combo_icon)
            .selected_text(shown)
            .width(ui.available_width().min(170.0))
            .truncate()
            .show_ui(ui, |ui| {
                changed |= ui
                    .selectable_value(current, None, "None (single material)")
                    .changed();
                for (id, n) in &choices {
                    changed |= ui.selectable_value(current, Some(*id), n).changed();
                }
            });
    });
    changed
}

/// The assembly a type is built of, when it is layered.
fn type_assembly(t: &ElementType) -> Option<ElementId> {
    match &t.params {
        TypeParams::Wall(w) if w.curtain.is_none() => w.assembly,
        TypeParams::Slab(s) | TypeParams::Roof(s) => s.assembly,
        _ => None,
    }
}

/// A layered type's layers, edited right here as in Revit's Edit Type ›
/// Structure; a note and a way out when other types share them.
fn type_layers(ui: &mut egui::Ui, model: &Model, id: ElementId, t: &ElementType) -> Vec<TypeAsk> {
    let mut asks = vec![];
    let Some(aid) = type_assembly(t) else {
        return asks;
    };
    let Some(ElementKind::Assembly(a)) = model.get(aid).map(|e| &e.kind) else {
        return asks;
    };
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Layers").strong());
    let others: Vec<String> = model
        .iter()
        .filter(|e| e.id != id)
        .filter_map(|e| match &e.kind {
            ElementKind::ElementType(o) if type_assembly(o) == Some(aid) => Some(e.name.clone()),
            _ => None,
        })
        .collect();
    if !others.is_empty() {
        crate::props::flow(ui, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "Shared with {}: changing them changes those too.",
                    others.join(", ")
                ))
                .weak()
                .small(),
            );
            if ui
                .small_button("Make them this type's own")
                .on_hover_text(
                    "A copy of the layers for this type only, as each Revit type has its own",
                )
                .clicked()
            {
                asks.push(TypeAsk::OwnLayers(id));
            }
        });
    }
    if let Some((label, new)) = super::assemblies::layer_table(ui, model, aid, a, true) {
        asks.push(TypeAsk::Layers(aid, label, new));
    }
    asks
}

/// The thickness an assembly gives, if it is one.
fn assembly_total(model: &Model, a: Option<ElementId>) -> Option<f64> {
    match a.and_then(|a| model.get(a)).map(|e| &e.kind) {
        Some(ElementKind::Assembly(a)) => Some(a.total()),
        _ => None,
    }
}

/// A type's parameters as fields. Returns the undo label of what changed,
/// or "".
pub(super) fn type_fields(ui: &mut egui::Ui, model: &Model, t: &mut ElementType) -> &'static str {
    let mut label = "";
    let usage = if matches!(t.params, TypeParams::Roof(_)) {
        AssemblyUse::Roof
    } else {
        AssemblyUse::Floor
    };
    let opening_kind = if matches!(t.params, TypeParams::Door(_)) {
        creabim_core::OpeningKind::Door
    } else {
        creabim_core::OpeningKind::Window
    };
    match &mut t.params {
        TypeParams::Wall(w) => {
            if let Some(c) = &mut w.curtain {
                let l = curtain_fields(ui, c);
                if !l.is_empty() {
                    label = l;
                    // a curtain wall is as thick as its mullions
                    w.thickness = c.mullion_depth;
                }
            } else {
                if assembly_pick(ui, model, AssemblyUse::Wall, &mut w.assembly) {
                    label = "Type assembly";
                }
                match assembly_total(model, w.assembly) {
                    Some(total) => {
                        w.thickness = total;
                        ui.label(format!(
                            "Thickness {}, from its layers",
                            crate::units::len(total)
                        ));
                    }
                    None => {
                        if len_field(ui, "Thickness", &mut w.thickness, 0.02, 2.0) {
                            label = "Type thickness";
                        }
                        if material_pick(ui, model, "wall", &mut w.material) {
                            label = "Type material";
                        }
                    }
                }
                crate::props::flow(ui, |ui| {
                    ui.label("Location line");
                    egui::ComboBox::from_id_salt("type_location")
                        .icon(crate::props::combo_icon)
                        .selected_text(w.location.label())
                        .show_ui(ui, |ui| {
                            for l in creabim_core::LocationLine::ALL {
                                if ui.selectable_value(&mut w.location, l, l.label()).changed() {
                                    label = "Type location line";
                                }
                            }
                        });
                });
            }
            crate::props::flow(ui, |ui| {
                let mut own = w.height.is_some();
                if crate::props::switch(ui, &mut own, "Default height")
                    .on_hover_text("New walls of this type are drawn this high; otherwise as high as their level")
                    .changed()
                {
                    w.height = own.then_some(3.0);
                    label = "Type height";
                }
                if let Some(h) = &mut w.height {
                    if ui.add(crate::units::drag(h).range(0.1..=100.0)).changed() {
                        label = "Type height";
                    }
                }
            });
        }
        TypeParams::Door(o) | TypeParams::Window(o) => {
            if len_field(ui, "Width", &mut o.width, 0.1, 20.0) {
                label = "Type width";
            }
            if len_field(ui, "Height", &mut o.height, 0.1, 20.0) {
                label = "Type height";
            }
            if len_field(ui, "Sill", &mut o.sill, 0.0, 20.0) {
                label = "Type sill";
            }
            if len_field(ui, "Frame width", &mut o.frame.width, 0.01, 0.5) {
                label = "Type frame";
            }
            if len_field(ui, "Frame depth", &mut o.frame.depth, 0.02, 1.0) {
                label = "Type frame";
            }
            let panes = |ui: &mut egui::Ui, text: &str, v: &mut u32| {
                crate::props::row(ui, text, |ui| {
                    crate::props::fill(ui, egui::DragValue::new(v).range(1..=12)).changed()
                })
            };
            if panes(ui, "Panes across", &mut o.frame.panes_across) {
                label = "Type panes";
            }
            if panes(ui, "Panes up", &mut o.frame.panes_up) {
                label = "Type panes";
            }
            if !super::wallsui::frame_drawing_fields(ui, opening_kind, &mut o.frame).is_empty() {
                label = "Type frame";
            }
            if material_pick(ui, model, "opening", &mut o.material) {
                label = "Type material";
            }
        }
        TypeParams::Slab(s) | TypeParams::Roof(s) => {
            if assembly_pick(ui, model, usage, &mut s.assembly) {
                label = "Type assembly";
            }
            match assembly_total(model, s.assembly) {
                Some(total) => {
                    s.thickness = total;
                    ui.label(format!(
                        "Thickness {}, from its layers",
                        crate::units::len(total)
                    ));
                }
                None => {
                    if len_field(ui, "Thickness", &mut s.thickness, 0.02, 3.0) {
                        label = "Type thickness";
                    }
                    if material_pick(ui, model, "slab", &mut s.material) {
                        label = "Type material";
                    }
                }
            }
        }
        TypeParams::Column(c) => {
            let mut round = matches!(c.profile, creabim_core::Profile::Round { .. });
            if crate::props::choice(
                ui,
                "Profile",
                &mut round,
                &[(false, "Rectangular", ""), (true, "Round", "")],
            ) {
                c.profile = if round {
                    creabim_core::Profile::Round { diameter: 0.3 }
                } else {
                    creabim_core::Profile::Rectangle {
                        width: 0.3,
                        depth: 0.3,
                    }
                };
                label = "Type profile";
            }
            match &mut c.profile {
                creabim_core::Profile::Rectangle { width, depth } => {
                    if len_field(ui, "Width", width, 0.05, 5.0)
                        | len_field(ui, "Depth", depth, 0.05, 5.0)
                    {
                        label = "Type profile";
                    }
                }
                creabim_core::Profile::Round { diameter } => {
                    if len_field(ui, "Diameter", diameter, 0.05, 5.0) {
                        label = "Type profile";
                    }
                }
            }
            if material_pick(ui, model, "column", &mut c.material) {
                label = "Type material";
            }
        }
        TypeParams::Beam(b) => {
            if len_field(ui, "Width", &mut b.width, 0.02, 5.0) {
                label = "Type width";
            }
            if len_field(ui, "Depth", &mut b.depth, 0.02, 5.0) {
                label = "Type depth";
            }
            if material_pick(ui, model, "beam", &mut b.material) {
                label = "Type material";
            }
        }
        TypeParams::Footing(f) => {
            ui.label(egui::RichText::new(f.kind.label()).weak());
            if len_field(ui, "Width", &mut f.width, 0.1, 10.0) {
                label = "Type width";
            }
            if f.kind == creabim_core::FootingKind::Pad
                && len_field(ui, "Length", &mut f.length, 0.1, 10.0)
            {
                label = "Type length";
            }
            if len_field(ui, "Thickness", &mut f.thickness, 0.05, 5.0) {
                label = "Type thickness";
            }
            if material_pick(ui, model, "footing", &mut f.material) {
                label = "Type material";
            }
        }
        TypeParams::Object(o) => {
            let l = super::objectsui::type_fields(ui, model, o);
            if !l.is_empty() {
                label = l;
            }
        }
    }
    label
}

/// A slab's or roof's own holes, with a button to draw another and one to
/// remove each; `shafts` is how many shafts cut it too. Returns the undo
/// label of what changed (or "") and whether to start drawing a hole.
pub(super) fn holes_fields(
    ui: &mut egui::Ui,
    holes: &mut Vec<Vec<glam::DVec2>>,
    shafts: usize,
) -> (&'static str, bool) {
    let mut label = "";
    let mut draw = false;
    // Blender's rows (MU31): the operator in the field column
    if crate::props::action(
        ui,
        "Holes",
        "Draw a hole",
        "Click the hole's corners in a plan, then the first again or Enter",
        true,
    )
    .clicked()
    {
        draw = true;
    }
    let mut remove = None;
    for (i, h) in holes.iter().enumerate() {
        let area = creabim_core::polygon::signed_area2(h).abs() / 2.0;
        crate::props::row(ui, &format!("Hole {}", i + 1), |ui| {
            crate::props::flow(ui, |ui| {
                ui.label(format!("{} corners, {}", h.len(), crate::units::area(area)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::props::icon_button(ui, "x", "Remove this hole").clicked() {
                        remove = Some(i);
                    }
                });
            })
        });
    }
    if let Some(i) = remove {
        holes.remove(i);
        label = "Remove hole";
    }
    if holes.is_empty() && shafts == 0 {
        crate::props::note(ui, "None yet. Draw one, or a shaft through several floors.");
    } else if !holes.is_empty() {
        crate::props::note(
            ui,
            "Edit (Enter) shows each hole's corners and edges to reshape it.",
        );
    }
    if shafts > 0 {
        ui.label(
            egui::RichText::new(format!(
                "Cut by {} too: move or reshape the shaft to change it.",
                plural(shafts, "shaft")
            ))
            .weak(),
        );
    }
    (label, draw)
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

impl CreaBimApp {
    pub(super) fn open_types(&mut self, current: Option<ElementId>) {
        self.types.open = true;
        if current.is_some() {
            self.types.current = current;
        }
    }

    /// The selected elements that could have a type.
    fn type_targets(&self) -> Vec<ElementId> {
        self.selection
            .all()
            .into_iter()
            .filter(|id| {
                self.doc
                    .model()
                    .get(*id)
                    .is_some_and(|e| TypeCategory::of(&e.kind).is_some())
            })
            .collect()
    }

    /// The Materials menu's type items.
    pub(super) fn types_menu(&self) -> Vec<Item> {
        vec![
            Item::Separator,
            Item::new("types.browser", "Types…")
                .icon("element-types")
                .hint("Wall, curtain wall, door, window, slab, roof, column, beam and object types: add, edit, assign"),
            Item::new("types.starters", "Add starter types")
                .icon("plus")
                .hint("Walls, curtain walls, doors, windows, slabs, roofs, columns and beams to start from"),
        ]
    }

    /// The option strip's type menu for the tool in use.
    pub(super) fn tool_type_items(&self) -> Vec<Item> {
        let Some(cat) = category_of_tool(self.plan.tool) else {
            return vec![];
        };
        let model = self.doc.model();
        let now = self.plan.types.get(&cat).copied();
        let mut items = vec![Item::new(format!("type.tool:{cat:?}:none"), "No type")
            .checked(now.is_none())
            .hint("Draw with the sizes in this bar")];
        let all = types::types(model);
        for (heading, c, curtain) in GROUPS {
            if c != cat {
                continue;
            }
            let rows: Vec<_> = all
                .iter()
                .filter(|(_, _, t)| t.category() == cat && t.is_curtain() == curtain)
                .collect();
            if rows.is_empty() {
                continue;
            }
            items.push(Item::Header(heading.into()));
            for (id, name, t) in rows {
                items.push(
                    Item::new(format!("type.tool:{cat:?}:{}", id.0), *name)
                        .checked(now == Some(*id))
                        .hint(t.summary(model)),
                );
            }
        }
        items.push(Item::Separator);
        if all.is_empty() {
            items.push(Item::new("types.starters", "Add starter types").icon("plus"));
        }
        items.push(Item::new("types.browser", "Types…").icon("element-types"));
        items
    }

    /// The name of the type the tool in use draws, for its option strip.
    pub(super) fn tool_type_label(&self) -> Option<String> {
        let cat = category_of_tool(self.plan.tool)?;
        Some(
            self.plan
                .types
                .get(&cat)
                .and_then(|id| self.doc.model().get(*id))
                .map_or("No type".to_string(), |e| e.name.clone()),
        )
    }

    /// Draw new elements of a category as a type (or as none).
    pub(super) fn set_tool_type(&mut self, cat: TypeCategory, id: Option<ElementId>) {
        let model = self.doc.model();
        let t = id.and_then(|id| match model.get(id).map(|e| &e.kind) {
            Some(ElementKind::ElementType(t)) if t.category() == cat => Some((id, t.clone())),
            _ => None,
        });
        match &t {
            Some((id, t)) => {
                self.plan.types.insert(cat, *id);
                // the bar shows the type's sizes
                match &t.params {
                    TypeParams::Wall(w) => {
                        self.plan.wall_thickness = w.thickness;
                        self.plan.wall_location = w.location;
                    }
                    TypeParams::Column(c) => self.plan.members.column = c.profile,
                    TypeParams::Beam(b) => {
                        self.plan.members.beam_width = b.width;
                        self.plan.members.beam_depth = b.depth;
                    }
                    _ => {}
                }
            }
            None => {
                self.plan.types.remove(&cat);
            }
        }
        let name = id
            .and_then(|i| self.doc.model().get(i))
            .map(|e| e.name.clone());
        self.status = match name {
            Some(n) if t.is_some() => format!("New {}s are {n}.", cat.word()),
            _ => format!("New {}s have no type.", cat.word()),
        };
    }

    /// Run a type menu action. False when the action isn't one.
    pub(super) fn types_action(&mut self, head: &str, arg: &str) -> bool {
        match head {
            "types.browser" => self.open_types(None),
            "types.starters" => self.run_types(TypeAsk::Starters),
            "type.tool" => {
                let (cat, id) = arg.split_once(':').unwrap_or((arg, "none"));
                let Some(cat) = TypeCategory::ALL
                    .into_iter()
                    .find(|c| format!("{c:?}") == cat)
                else {
                    return true;
                };
                let id = self
                    .doc
                    .model()
                    .iter()
                    .find(|e| e.id.0.to_string() == id)
                    .map(|e| e.id);
                self.set_tool_type(cat, id);
            }
            "tool.curtain" => {
                // the Wall tool, drawing the first curtain wall type
                let mut id = types::first_of(self.doc.model(), TypeCategory::Wall, true);
                if id.is_none() {
                    self.run_types(TypeAsk::Starters);
                    id = types::first_of(self.doc.model(), TypeCategory::Wall, true);
                }
                self.plan.tool = Tool::Wall;
                self.plan.cancel();
                self.set_tool_type(TypeCategory::Wall, id);
                self.status = "Curtain wall: click to start, click again to place each wall. Pick another type, or No type, in the bar.".into();
            }
            "tool.screen" => {
                // the Wall tool, drawing the first screen wall type
                let first = |m: &creabim_core::Model| {
                    types::types(m)
                        .into_iter()
                        .find(|(_, _, t)| {
                            matches!(&t.params, types::TypeParams::Wall(w) if w.screen.is_some())
                        })
                        .map(|(id, ..)| id)
                };
                let mut id = first(self.doc.model());
                if id.is_none() {
                    self.run_types(TypeAsk::Starters);
                    id = first(self.doc.model());
                }
                self.plan.tool = Tool::Wall;
                self.plan.cancel();
                self.set_tool_type(TypeCategory::Wall, id);
                self.status = "Screen wall: click to start, click again to place each wall. Its type sets the unit and holes; pick another in the bar.".into();
            }
            _ => return self.objects_action(head, arg),
        }
        true
    }

    /// The type picker at the top of Properties for an element that can
    /// have a type: which type it is (changing it changes every selected
    /// element it fits), what it overrides, and buttons to reset, update
    /// the type, make a type of it and select the type's instances.
    pub(super) fn type_picker(&mut self, ui: &mut egui::Ui, id: ElementId) {
        if self
            .doc
            .model()
            .get(id)
            .and_then(|e| TypeCategory::of(&e.kind))
            .is_none()
        {
            return;
        }
        // a folding header as every other section's (MU31: a bare title)
        if !self.prop_header(ui, "type", "Type", true, None) {
            return;
        }
        let model = self.doc.model();
        let Some(e) = model.get(id) else { return };
        let Some(cat) = TypeCategory::of(&e.kind) else {
            return;
        };
        let choices: Vec<(ElementId, String, bool)> = types::types_for(model, &e.kind)
            .into_iter()
            .map(|(id, n, t)| (id, n.to_string(), t.is_curtain()))
            .collect();
        let current = types::type_of(model, e).map(|(t, _)| t.id);
        let overrides = e
            .of_type
            .as_ref()
            .map(|r| r.overrides.clone())
            .unwrap_or_default();
        let targets = self.type_targets();
        let same: Vec<ElementId> = targets
            .iter()
            .copied()
            .filter(|t| {
                model
                    .get(*t)
                    .is_some_and(|x| TypeCategory::of(&x.kind) == Some(cat))
            })
            .collect();
        let shown = current
            .and_then(|c| choices.iter().find(|x| x.0 == c))
            .map_or("No type".to_string(), |x| x.1.clone());
        let mut asks = vec![];
        let mut chosen = current;
        crate::props::flow(ui, |ui| {
            crate::app::helpui::combo(ui, "Type")
                .selected_text(shown)
                .width(ui.available_width().min(180.0))
                .show_ui(ui, |ui| {
                    // an object is its type: it can change type, not lose it
                    if cat != TypeCategory::Object {
                        ui.selectable_value(&mut chosen, None, "No type");
                    }
                    let mut curtain_header = false;
                    for (tid, n, curtain) in &choices {
                        if *curtain && !curtain_header {
                            curtain_header = true;
                            ui.separator();
                        }
                        ui.selectable_value(&mut chosen, Some(*tid), n);
                    }
                    if choices.is_empty() {
                        ui.label(egui::RichText::new("No types yet: add the starters").weak());
                    }
                })
                .response
                .on_hover_text(if same.len() > 1 {
                    format!(
                        "Changes the type of all {} selected",
                        plural(same.len(), cat.word())
                    )
                } else {
                    "Its type: what it shares with the other instances".to_string()
                });
        });
        if chosen != current {
            asks.push(TypeAsk::Assign(chosen));
        }
        if let Some(t) = current {
            let summary = model
                .get(t)
                .and_then(|te| match &te.kind {
                    ElementKind::ElementType(ty) => Some(ty.summary(model)),
                    _ => None,
                })
                .unwrap_or_default();
            let n = types::instances(model, t).len();
            // one line, as Blender packs it (MU22): how many, then the
            // type's actions as links; what it is on hover
            let ink = ui.visuals().strong_text_color();
            let link = |ui: &mut egui::Ui, text: &str| {
                let r = ui.add(
                    egui::Button::new(egui::RichText::new(text).color(ink))
                        .frame(false)
                        .small(),
                );
                if r.hovered() {
                    let b = r.rect;
                    ui.painter()
                        .hline(b.x_range(), b.bottom() - 2.0, egui::Stroke::new(1.0, ink));
                }
                r
            };
            // as Blender's rows (MU29): the count, then real buttons the
            // width of the field column, side by side
            let _ = &link;
            crate::props::row(ui, "Instances", |ui| {
                crate::props::flow(ui, |ui| {
                    ui.label(n.to_string()).on_hover_text(summary.clone());
                    let w = ui.available_width();
                    if ui
                        .add_sized([w, 22.0], egui::Button::new("Select all"))
                        .on_hover_text("Select every element of this type")
                        .clicked()
                    {
                        asks.push(TypeAsk::SelectInstances(t));
                    }
                })
            });
            crate::props::row(ui, "", |ui| {
                crate::props::flow(ui, |ui| {
                    let two = cat != TypeCategory::Object;
                    let gap = ui.spacing().item_spacing.x;
                    let w = if two {
                        (ui.available_width() - gap) / 2.0
                    } else {
                        ui.available_width()
                    };
                    if ui
                        .add_sized([w, 22.0], egui::Button::new("Edit\u{2026}"))
                        .on_hover_text(format!(
                            "Edit the type \u{201c}{}\u{201d}: every instance follows",
                            self.doc
                                .model()
                                .get(t)
                                .map_or(String::new(), |e| e.name.clone())
                        ))
                        .clicked()
                    {
                        self.types.open = true;
                        self.types.current = Some(t);
                    }
                    if two
                        && ui
                            .add_sized([w, 22.0], egui::Button::new("New"))
                            .on_hover_text(
                                "A new type with its values, which it becomes an instance of",
                            )
                            .clicked()
                    {
                        asks.push(TypeAsk::FromElement(id));
                    }
                })
            });
            if !overrides.is_empty() {
                let list: Vec<&str> = overrides.iter().map(|p| p.label()).collect();
                crate::props::row(ui, "Overrides", |ui| {
                    crate::props::flow(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(list.join(", "))
                                    .color(Color32::from_rgb(220, 130, 40)),
                            )
                            .truncate(),
                        )
                        .on_hover_text(
                            "What this element sets for itself: a change to its type leaves these alone",
                        );
                    })
                });
                crate::props::row(ui, "", |ui| {
                    crate::props::flow(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        if link(ui, "Reset to type")
                            .on_hover_text("Drop its overrides: take every value from its type")
                            .clicked()
                        {
                            asks.push(TypeAsk::Reset(id));
                        }
                        ui.label(egui::RichText::new("·").weak());
                        if link(ui, "Update type")
                            .on_hover_text(
                                "Make its values the type's: every other instance follows",
                            )
                            .clicked()
                        {
                            asks.push(TypeAsk::SaveToType(id));
                        }
                    })
                });
            }
        } else if cat != TypeCategory::Object
            && crate::props::action(
                ui,
                "",
                "Make a type of it",
                "A new type with its values, which it becomes an instance of",
                true,
            )
            .clicked()
        {
            asks.push(TypeAsk::FromElement(id));
        }
        for a in asks {
            self.run_types(a);
        }
    }

    /// Properties for a type picked in the project browser.
    pub(super) fn type_properties(&mut self, ui: &mut egui::Ui, id: ElementId) {
        let model = self.doc.model();
        let Some(ElementKind::ElementType(t)) = model.get(id).map(|e| &e.kind) else {
            return;
        };
        let mut t = t.clone();
        ui.label(egui::RichText::new(format!("{} · {}", t.ifc_class(), t.summary(model))).weak());
        let label = type_fields(ui, model, &mut t);
        let n = types::instances(model, id).len();
        let mut asks = type_layers(ui, model, id, &t);
        crate::props::flow(ui, |ui| {
            ui.label(match n {
                0 => "No instances yet.".to_string(),
                n => format!("{}.", plural(n, "instance")),
            });
            if n > 0 && ui.small_button("Select them").clicked() {
                asks.push(TypeAsk::SelectInstances(id));
            }
        });
        self.show_in_model_ui(ui, id);
        crate::props::flow(ui, |ui| {
            if ui
                .button("Open preview")
                .on_hover_text("The type on its own, in 3D, plan and section, beside its values (or double-click it in the project browser)")
                .clicked()
            {
                self.open_type_preview(id);
            }
            if ui.button("Types window").clicked() {
                self.open_types(Some(id));
            }
        });
        if !label.is_empty() {
            asks.push(TypeAsk::Edit(id, label, t));
        }
        for a in asks {
            self.run_types(a);
        }
    }

    /// The Types window, when open.
    pub(super) fn types_window(&mut self, ctx: &egui::Context) {
        if !self.types.open {
            return;
        }
        let mut open = true;
        let mut asks = vec![];
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        crate::app::shell::window("Types")
            .open(&mut open)
            .default_pos(area.min + Vec2::new(36.0, 36.0))
            .constrain_to(area)
            .default_size([
                920.0_f32.min(area.width() - 60.0),
                (area.height() - 40.0).clamp(360.0, 620.0),
            ])
            .min_width(600.0)
            .collapsible(false)
            .show(ctx, |ui| self.types_body(ui, &mut asks));
        self.types.open = open;
        for ask in asks {
            self.run_types(ask);
        }
    }

    fn types_body(&self, ui: &mut egui::Ui, asks: &mut Vec<TypeAsk>) {
        let model = self.doc.model();
        let all = types::types(model);
        let current = self
            .types
            .current
            .filter(|c| all.iter().any(|(id, _, _)| id == c))
            .or_else(|| all.first().map(|t| t.0));
        let height = ui.available_height().max(320.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(250.0);
                egui::ScrollArea::vertical()
                    .id_salt("types_list")
                    .max_height(height - 70.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (heading, cat, curtain) in GROUPS {
                            ui.label(
                                egui::RichText::new(heading.to_uppercase())
                                    .size(10.5)
                                    .strong()
                                    .color(ui.visuals().weak_text_color()),
                            );
                            for (id, name, t) in all
                                .iter()
                                .filter(|(_, _, t)| t.category() == cat && t.is_curtain() == curtain)
                            {
                                let n = types::instances(model, *id).len();
                                let text = if n > 0 {
                                    format!("{name}  ·  {n}")
                                } else {
                                    name.to_string()
                                };
                                if ui
                                    .selectable_label(current == Some(*id), text)
                                    .on_hover_text(t.summary(model))
                                    .clicked()
                                {
                                    asks.push(TypeAsk::Pick(*id));
                                }
                            }
                            if cat == TypeCategory::Object {
                                // objects come from the catalogue, not from sizes
                                if ui
                                    .small_button("Catalogue…")
                                    .on_hover_text("Place furniture and fixtures from the catalogue; their types appear here")
                                    .clicked()
                                {
                                    asks.push(TypeAsk::Catalogue);
                                }
                            } else if ui.small_button("+ New").clicked() {
                                asks.push(TypeAsk::New(cat, curtain));
                            }
                            ui.add_space(6.0);
                        }
                    });
                crate::props::flow(ui, |ui| {
                    if ui
                        .button("Add starters")
                        .on_hover_text("Walls, curtain walls, doors, windows, slabs, roofs, columns and beams, with the starter assemblies they are built of")
                        .clicked()
                    {
                        asks.push(TypeAsk::Starters);
                    }
                    if let Some(c) = current {
                        if ui.button("Duplicate").clicked() {
                            asks.push(TypeAsk::Duplicate(c));
                        }
                        let object = all.iter().any(|(id, _, t)| {
                            *id == c && t.category() == TypeCategory::Object
                        });
                        if ui
                            .button("Delete")
                            .on_hover_text(if object {
                                "Deletes the objects of this type too. Ctrl+Z brings them back."
                            } else {
                                "Its instances keep their values, with no type. Ctrl+Z brings it back."
                            })
                            .clicked()
                        {
                            asks.push(TypeAsk::Delete(c));
                        }
                    }
                });
            });
            ui.separator();
            ui.vertical(|ui| {
                let Some(id) = current else {
                    ui.label(
                        egui::RichText::new(
                            "No types yet. Add the starters, or make a new one with + New.",
                        )
                        .weak(),
                    );
                    return;
                };
                let Some(e) = model.get(id) else { return };
                let ElementKind::ElementType(t) = &e.kind else {
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
                    asks.push(TypeAsk::Rename(id, name));
                }
                ui.label(
                    egui::RichText::new(format!("{} · {}", t.ifc_class(), t.summary(model)))
                        .weak(),
                );
                let mut description = t.description.clone();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut description).vertical_align(egui::Align::Center)
                            .hint_text("Description")
                            .desired_width(420.0),
                    )
                    .changed()
                {
                    let mut new = t.clone();
                    new.description = description;
                    asks.push(TypeAsk::Edit(id, "Type description", new));
                }
                ui.add_space(6.0);
                let mut new = t.clone();
                let label = type_fields(ui, model, &mut new);
                asks.extend(type_layers(ui, model, id, &new));
                if !label.is_empty() {
                    asks.push(TypeAsk::Edit(id, label, new));
                }
                ui.add_space(8.0);
                let users = types::instances(model, id);
                crate::props::flow(ui, |ui| {
                    ui.label(match users.len() {
                        0 => "No instances yet.".to_string(),
                        n => format!("{}. Changing the type changes them all, but for what each overrides.", plural(n, "instance")),
                    });
                });
                if !users.is_empty() && ui.button("Select all instances").clicked() {
                    asks.push(TypeAsk::SelectInstances(id));
                }
                let fits = self
                    .type_targets()
                    .iter()
                    .filter(|x| {
                        model
                            .get(**x)
                            .is_some_and(|e| types::fits(&t.params, &e.kind))
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
                    t.category().label().to_lowercase()
                ))
                .clicked()
                {
                    asks.push(TypeAsk::Assign(Some(id)));
                }
            });
        });
    }

    pub(super) fn run_types(&mut self, ask: TypeAsk) {
        match ask {
            TypeAsk::Pick(id) => self.types.current = Some(id),
            TypeAsk::Catalogue => self.catalogue.open = true,
            TypeAsk::New(cat, curtain) => {
                let base = if curtain {
                    "New curtain wall type".to_string()
                } else {
                    format!("New {} type", cat.word())
                };
                let e = Element::new(
                    types::unique_name(self.doc.model(), &base),
                    ElementKind::ElementType(ElementType::new(types::default_params(cat, curtain))),
                );
                let id = e.id;
                let r = self.doc.commit(Transaction::new("New type").insert(e));
                self.types.current = Some(id);
                self.report(r, "New type. Name it and set what its instances share.");
            }
            TypeAsk::Starters => {
                let (tx, ids) = types::add_starters(self.doc.model());
                if tx.ops.is_empty() {
                    self.status = "The starter types are in the project already.".into();
                } else {
                    let r = self.doc.commit(tx);
                    self.report(
                        r,
                        "Added starter types: walls, curtain walls, doors, windows, slabs, roofs, columns and beams.",
                    );
                }
                if self.types.current.is_none() {
                    self.types.current = ids.first().copied();
                }
            }
            TypeAsk::Duplicate(id) => {
                if let Some(mut e) = types::duplicate(self.doc.model(), id) {
                    let new = e.id;
                    let mut tx = Transaction::new("Duplicate type");
                    // its layers are copied with it, as Revit's types each
                    // have their own: the copy's can change on their own
                    if let ElementKind::ElementType(t) = &mut e.kind {
                        if let Some(ae) = type_assembly(t).and_then(|a| self.doc.model().get(a)) {
                            let copy = Element::new(format!("{} layers", e.name), ae.kind.clone());
                            let cid = copy.id;
                            match &mut t.params {
                                TypeParams::Wall(w) => w.assembly = Some(cid),
                                TypeParams::Slab(s) | TypeParams::Roof(s) => s.assembly = Some(cid),
                                _ => {}
                            }
                            tx = tx.insert(copy);
                        }
                    }
                    let r = self.doc.commit(tx.insert(e));
                    self.types.current = Some(new);
                    self.report(r, "Duplicated the type.");
                }
            }
            TypeAsk::Delete(id) => {
                let name = self
                    .doc
                    .model()
                    .get(id)
                    .map(|e| e.name.clone())
                    .unwrap_or_default();
                let tx = types::delete(self.doc.model(), id);
                let r = self.doc.commit(tx);
                self.types.current = None;
                self.selection.retain(|s| s != id);
                self.plan.types.retain(|_, t| *t != id);
                self.report(
                    r,
                    format!(
                        "Deleted {name}. Its instances keep their values. Ctrl+Z brings it back."
                    ),
                );
            }
            TypeAsk::Rename(id, name) => {
                let r = self
                    .doc
                    .edit_merged(id, "Rename type", |e| e.name = name.clone());
                self.report(r, "Renamed.");
            }
            TypeAsk::Edit(id, label, t) => {
                if let Err(e) = t.validate() {
                    self.status = format!("Can't: {e}.");
                    return;
                }
                if let Some(tx) = types::edit(self.doc.model(), id, t, label) {
                    let n = tx.ops.len().saturating_sub(1);
                    let r = self.doc.commit_merged(tx);
                    self.report(
                        r,
                        match n {
                            0 => format!("{label}."),
                            1 => format!("{label}: 1 instance follows."),
                            n => format!("{label}: {} follow.", plural(n, "instance")),
                        },
                    );
                }
            }
            TypeAsk::Layers(aid, label, a) => {
                if let Err(e) = a.validate() {
                    self.status = format!("Can't: {e}.");
                    return;
                }
                if let Some(tx) = creabim_core::assembly::edit(self.doc.model(), aid, a, label) {
                    let r = self.doc.commit_merged(tx);
                    self.report(r, format!("{label}."));
                }
            }
            TypeAsk::OwnLayers(id) => {
                let model = self.doc.model();
                let Some(te) = model.get(id).cloned() else {
                    return;
                };
                let ElementKind::ElementType(t) = &te.kind else {
                    return;
                };
                let Some(aid) = type_assembly(t) else { return };
                let Some(ae) = model.get(aid) else { return };
                let mut copy = Element::new(format!("{} layers", te.name), ae.kind.clone());
                copy.name = format!("{} layers", te.name);
                let cid = copy.id;
                let mut nt = t.clone();
                match &mut nt.params {
                    TypeParams::Wall(w) => w.assembly = Some(cid),
                    TypeParams::Slab(s) | TypeParams::Roof(s) => s.assembly = Some(cid),
                    _ => {}
                }
                let r = self.doc.commit(Transaction::new("Own layers").insert(copy));
                if r.is_ok() {
                    if let Some(tx) = types::edit(self.doc.model(), id, nt, "Own layers") {
                        let r = self.doc.commit_merged(tx);
                        self.report(r, "The type has its own layers now.");
                    }
                }
            }
            TypeAsk::SelectInstances(id) => {
                let users = types::instances(self.doc.model(), id);
                self.selection.extend(users.iter().copied(), false);
                self.status = format!("Selected {}.", plural(users.len(), "instance"));
            }
            TypeAsk::Assign(t) => {
                let targets = self.type_targets();
                if targets.is_empty() {
                    self.status =
                        "Select walls, doors, windows, slabs, roofs, columns or beams first."
                            .into();
                    return;
                }
                let (tx, n) = types::assign(self.doc.model(), &targets, t);
                let name = t
                    .and_then(|a| self.doc.model().get(a))
                    .map_or("no type".to_string(), |e| e.name.clone());
                let r = self.doc.commit(tx);
                self.report(
                    r,
                    match n {
                        0 => format!("Nothing selected changes to {name}."),
                        n => format!("{} now {name}.", plural(n, "element")),
                    },
                );
            }
            TypeAsk::Reset(id) => {
                if let Some(tx) = types::reset(self.doc.model(), id) {
                    let r = self.doc.commit(tx);
                    self.report(r, "Back to its type.");
                }
            }
            TypeAsk::SaveToType(id) => {
                if let Some(tx) = types::save_to_type(self.doc.model(), id) {
                    let n = tx.ops.len().saturating_sub(2);
                    let r = self.doc.commit(tx);
                    self.report(
                        r,
                        match n {
                            0 => "Type updated from it.".to_string(),
                            1 => "Type updated from it: 1 other instance follows.".to_string(),
                            n => format!("Type updated from it: {n} other instances follow."),
                        },
                    );
                }
            }
            TypeAsk::FromElement(id) => {
                let model = self.doc.model();
                let base = model.get(id).map_or("New type".to_string(), |e| {
                    let what = TypeCategory::of(&e.kind).map_or("element", |c| c.word());
                    format!("{} {what} type", e.name)
                });
                if let Some((t, after)) = types::from_element(model, id, &base) {
                    let Some(before) = model.get(id).cloned() else {
                        return;
                    };
                    let tid = t.id;
                    let name = t.name.clone();
                    let r = self.doc.commit(
                        Transaction::new("Make a type")
                            .insert(t)
                            .replace(before, after),
                    );
                    self.types.current = Some(tid);
                    self.report(r, format!("Made {name}: it is the type's first instance."));
                }
            }
        }
    }
}
