//! Graphic styles in the app: the Graphic styles window, where the
//! project's style packs (pens, object styles, hatch patterns and view
//! templates) are edited, loaded from files and saved to them; and in a
//! view's Properties its view range, template, depth cues and graphic
//! overrides. Packs and views are project data, and every change is one
//! transaction, so it undoes like any edit.

use super::*;
use creabim_core::graphics::{PlanRange, RangeLimit};
use creabim_core::styles::{
    self, DepthCues, HatchLine, HatchPattern, LinePattern, ObjectStyle, Override, PatternUnits,
    PenName, Rgb, Shadows, StylePack, Target, ViewTemplate, Weight,
};
use creabim_core::visibility::Category;

/// The window's page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Page {
    #[default]
    Objects,
    Pens,
    Patterns,
    Templates,
    Marks,
}

/// The window's own state, for the session.
#[derive(Default)]
pub(super) struct StylesUi {
    pub open: bool,
    page: Page,
    /// The pattern and template picked in their lists.
    pattern: usize,
    template: usize,
    /// The marker designer.
    designer: crate::choices::Designer,
    /// While an option is previewed on the plan, the pack as it was.
    preview_base: Option<StylePack>,
}

/// The undo label of a previewed option, taken back when the pointer
/// leaves it.
const PREVIEW: &str = "Preview style";

/// The categories object styles are set for: those with linework.
pub(super) const STYLED: [Category; 20] = [
    Category::Walls,
    Category::Slabs,
    Category::Roofs,
    Category::Columns,
    Category::Beams,
    Category::Footings,
    Category::Doors,
    Category::Windows,
    Category::Stairs,
    Category::Railings,
    Category::Ceilings,
    Category::Objects,
    Category::FreeMeshes,
    Category::Parts,
    Category::Site,
    Category::Planting,
    Category::Openings,
    Category::Rooms,
    Category::Grids,
    Category::SectionMarks,
];

/// A weight picker: *As drawn*, one of the pens, or millimetres.
fn weight_combo(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    w: &mut Option<Weight>,
) -> bool {
    let mut changed = false;
    let text = w.map_or("As drawn".to_string(), |w| w.label());
    egui::ComboBox::from_id_salt(salt)
        .icon(crate::props::combo_icon)
        .width(
            ui.spacing()
                .combo_width
                .max(108.0)
                .min(ui.available_width()),
        )
        .selected_text(text)
        .show_ui(ui, |ui| {
            changed |= ui.selectable_value(w, None, "As drawn").changed();
            for p in PenName::ALL {
                changed |= ui
                    .selectable_value(w, Some(Weight::Pen(p)), p.label())
                    .changed();
            }
            if ui
                .selectable_label(matches!(w, Some(Weight::Mm(_))), "Millimetres")
                .clicked()
                && !matches!(w, Some(Weight::Mm(_)))
            {
                *w = Some(Weight::Mm(0.25));
                changed = true;
            }
        });
    if let Some(Weight::Mm(mm)) = w {
        changed |= ui
            .add(
                egui::DragValue::new(mm)
                    .speed(0.01)
                    .range(0.02..=2.0)
                    .max_decimals(2)
                    .suffix(" mm"),
            )
            .changed();
    }
    changed
}

fn pattern_combo(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    p: &mut Option<LinePattern>,
) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(salt)
        .icon(crate::props::combo_icon)
        .width(ui.spacing().combo_width.max(84.0).min(ui.available_width()))
        .selected_text(p.as_ref().map_or("As drawn", |p| p.label()))
        .show_ui(ui, |ui| {
            changed |= ui.selectable_value(p, None, "As drawn").changed();
            for n in LinePattern::NAMED {
                let label = n.label();
                changed |= ui.selectable_value(p, Some(n), label).changed();
            }
        });
    changed
}

/// A colour that can be left unset. A fill starts light, a line dark.
fn colour_pick(ui: &mut egui::Ui, c: &mut Option<Rgb>, fill: bool, hover: &str) -> bool {
    let mut on = c.is_some();
    let mut changed = ui.checkbox(&mut on, "").on_hover_text(hover).changed();
    if changed {
        let v = if fill { 0.85 } else { 0.2 };
        *c = on.then_some(Rgb([v; 3]));
    }
    if let Some(Rgb(rgb)) = c {
        // the picker rounds through its own colour space every frame: keep
        // what it gives back only when it was used
        let mut edit = *rgb;
        if ui.color_edit_button_rgb(&mut edit).changed() {
            *rgb = edit;
            changed = true;
        }
    }
    changed
}

fn name_combo(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    names: &[String],
    v: &mut Option<String>,
    none: &str,
) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(salt)
        .icon(crate::props::combo_icon)
        .width(
            ui.spacing()
                .combo_width
                .max(108.0)
                .min(ui.available_width()),
        )
        .selected_text(v.as_deref().unwrap_or(none))
        .show_ui(ui, |ui| {
            changed |= ui.selectable_value(v, None, none).changed();
            for n in names {
                changed |= ui.selectable_value(v, Some(n.clone()), n).changed();
            }
        });
    changed
}

/// A small square of a pattern, as it draws at 1:`scale`.
fn pattern_preview(ui: &mut egui::Ui, p: &HatchPattern, scale: f64) {
    let size = 64.0;
    let (r, painter) = ui.allocate_painter(egui::vec2(size, size), egui::Sense::hover());
    let rect = r.rect;
    let fill = p.fill.map_or([1.0; 3], |c| c.0);
    let c32 = |c: [f32; 3]| {
        egui::Color32::from_rgb(
            (c[0] * 255.0) as u8,
            (c[1] * 255.0) as u8,
            (c[2] * 255.0) as u8,
        )
    };
    painter.rect_filled(rect, 0.0, c32(fill));
    // the square is 20 mm of paper; drawing units are model metres
    let k = 1000.0 / scale;
    let side = 20.0 / k;
    let poly = [
        DVec2::ZERO,
        DVec2::new(side, 0.0),
        DVec2::new(side, side),
        DVec2::new(0.0, side),
    ];
    let to = |p: DVec2| {
        egui::pos2(
            rect.min.x + (p.x / side) as f32 * size,
            rect.max.y - (p.y / side) as f32 * size,
        )
    };
    let stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(50));
    for (a, b) in creabim_geom::looks::pattern_lines(p, &poly, k) {
        let (a, b) = (to(a), to(b));
        if a.distance(b) < 0.8 {
            painter.circle_filled(a, 0.7, stroke.color);
        } else {
            painter.line_segment([a, b], stroke);
        }
    }
    painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
}

/// A level and an offset from it, or none.
fn limit_ui(
    ui: &mut egui::Ui,
    salt: &str,
    label: &str,
    none: &str,
    levels: &[(ElementId, String)],
    l: &mut Option<RangeLimit>,
) -> bool {
    let mut changed = false;
    crate::props::flow(ui, |ui| {
        ui.label(label);
        let current = l
            .and_then(|l| levels.iter().find(|x| x.0 == l.level))
            .map_or(none.to_string(), |x| x.1.clone());
        egui::ComboBox::from_id_salt(salt)
            .icon(crate::props::combo_icon)
            .width(ui.available_width().min(130.0))
            .truncate()
            .selected_text(current)
            .show_ui(ui, |ui| {
                if ui.selectable_label(l.is_none(), none).clicked() {
                    *l = None;
                    changed = true;
                }
                for (id, name) in levels {
                    let on = l.is_some_and(|l| l.level == *id);
                    if ui.selectable_label(on, name).clicked() {
                        *l = Some(RangeLimit {
                            level: *id,
                            offset: l.map_or(0.0, |l| l.offset),
                        });
                        changed = true;
                    }
                }
            });
        if let Some(l) = l {
            ui.label("+");
            changed |= ui
                .add(crate::units::drag(&mut l.offset).range(-100.0..=100.0))
                .changed();
        }
    });
    changed
}

/// A plan's view range: its own level, the levels within a range, or a
/// view from above. Returns the undo label of a change.
pub(super) fn range_ui(
    ui: &mut egui::Ui,
    levels: &[(ElementId, String)],
    r: &mut PlanRange,
) -> Option<&'static str> {
    let mut label = None;
    #[derive(PartialEq)]
    enum Mode {
        Own,
        Range,
        Above,
    }
    let mode = if r.from_above {
        Mode::Above
    } else if r.spans_levels() {
        Mode::Range
    } else {
        Mode::Own
    };
    let mut pick = mode;
    crate::props::flow(ui, |ui| {
        ui.radio_value(&mut pick, Mode::Own, "Its level")
            .on_hover_text("Only the elements of this plan's own level, as plans always drew");
        ui.radio_value(&mut pick, Mode::Range, "Levels in range")
            .on_hover_text("Every level's elements between the bottom and the top: a site level under the floor, roofs dashed overhead");
        ui.radio_value(&mut pick, Mode::Above, "From above")
            .on_hover_text("Look down from the top with nothing cut; roofs and upper slabs hide what is under them. A roof or site plan.");
    });
    let was = if r.from_above {
        2
    } else if r.spans_levels() {
        1
    } else {
        0
    };
    let now = match pick {
        Mode::Own => 0,
        Mode::Range => 1,
        Mode::Above => 2,
    };
    if now != was {
        let depth = r.view_depth;
        *r = PlanRange {
            view_depth: depth,
            ..Default::default()
        };
        match now {
            1 => r.overhead = true,
            2 => r.from_above = true,
            _ => {}
        }
        label = Some("View range");
    }
    match now {
        1 => {
            if limit_ui(ui, "range_top", "Top", "Next level up", levels, &mut r.top) {
                label = Some("View range");
            }
            if limit_ui(
                ui,
                "range_bottom",
                "Bottom",
                "This level",
                levels,
                &mut r.bottom,
            ) {
                label = Some("View range");
            }
            if limit_ui(
                ui,
                "range_beyond",
                "View depth",
                "None",
                levels,
                &mut r.beyond,
            ) {
                label = Some("View range");
            }
            if crate::props::switch(ui, &mut r.overhead, "Overhead dashed")
                .on_hover_text("Draw what is above the cut, up to the top, in dashed lines: roof edges, upper slabs")
                .changed()
            {
                label = Some("View range");
            }
            ui.label(
                egui::RichText::new(
                    "Levels between the bottom and the cut are cut or seen below it; between the view depth and the bottom they draw lighter.",
                )
                .weak(),
            );
        }
        2 => {
            if limit_ui(
                ui,
                "above_top",
                "Top",
                "Above everything",
                levels,
                &mut r.top,
            ) {
                label = Some("View range");
            }
            if limit_ui(
                ui,
                "above_bottom",
                "Bottom",
                "Under everything",
                levels,
                &mut r.bottom,
            ) {
                label = Some("View range");
            }
        }
        _ => {}
    }
    label
}

/// A view's template, depth cues and overrides. Returns the undo label of
/// a change.
pub(super) fn look_ui(
    ui: &mut egui::Ui,
    model: &creabim_core::Model,
    v: &mut View,
    edit_styles: &mut bool,
) -> Option<&'static str> {
    let mut label = None;
    let pack = styles::active_pack(model);
    let names: Vec<String> = pack.templates.iter().map(|t| t.name.clone()).collect();
    let fallback = pack.default_templates.of(v).map(str::to_string);
    let none = match &fallback {
        Some(n) => format!("Pack default ({n})"),
        None => "None".to_string(),
    };
    let none = none.replace("Pack default", "Default");
    crate::props::row(ui, "Template", |ui| {
        ui.spacing_mut().combo_width = ui.available_width();
        if name_combo(ui, "view_template", &names, &mut v.look.template, &none) {
            label = Some("View template");
        }
    });
    // the pack the template comes from, and where it is edited (MU22)
    // as Blender's data rows (MU30): the pack's name in a field, its edit
    // button beside it, together filling the field column
    crate::props::row(ui, "Style pack", |ui| {
        let w = ui.available_width();
        let gap = ui.spacing().item_spacing.x;
        ui.add_sized(
            [(w - 28.0 - gap).max(40.0), 22.0],
            // the fill of the fields and lists beside it, in every theme
            // (MU31: a hover-only button took the theme's darkest fill)
            egui::Button::new(egui::RichText::new(pack.name.clone()))
                .fill(ui.visuals().widgets.inactive.weak_bg_fill)
                .stroke(ui.visuals().widgets.inactive.bg_stroke)
                .sense(egui::Sense::hover()),
        )
        .on_hover_text("A named bundle of scale, style, hidden categories, pens, overrides and cues from the project's style pack. Change the template and every view using it changes.");
        if crate::props::icon_button(ui, "pencil", "Edit the style pack")
            .on_hover_text(
                "Graphic styles: the pack's pens, object styles, hatches and view templates",
            )
            .clicked()
        {
            *edit_styles = true;
        }
    });
    // depth cues
    let c = &mut v.look.cues;
    crate::props::row(ui, "Fade", |ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 56.0).max(40.0);
        if ui
            .add(egui::Slider::new(&mut c.fade, 0.0..=0.9).max_decimals(2))
            .on_hover_text("Lines and fills further from the cut or the viewer draw lighter, up to this much at the far end of the view. Sections, elevations and plans from above.")
            .changed()
        {
            label = Some("Depth cues");
        }
    });
    let mut sun = c.shadows.is_some();
    if crate::props::check(ui, "Shadows", &mut sun, "Sun")
        .on_hover_text("Shadows cast on the ground, in plans from above")
        .changed()
    {
        c.shadows = sun.then_some(Shadows {
            direction: -50.0,
            length: 0.6,
            opacity: 0.35,
        });
        label = Some("Depth cues");
    }
    if let Some(s) = &mut c.shadows {
        crate::props::row(ui, "Toward", |ui| {
            label = label.or(crate::props::fill(
                ui,
                egui::DragValue::new(&mut s.direction)
                    .speed(1.0)
                    .suffix("°"),
            )
            .changed()
            .then_some("Depth cues"));
        });
        crate::props::row(ui, "Length", |ui| {
            label = label.or(crate::props::fill(
                ui,
                egui::DragValue::new(&mut s.length)
                    .speed(0.05)
                    .range(0.0..=5.0),
            )
            .changed()
            .then_some("Depth cues"));
        });
        crate::props::row(ui, "Strength", |ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - 56.0).max(40.0);
            label = label.or(ui
                .add(egui::Slider::new(&mut s.opacity, 0.05..=1.0))
                .changed()
                .then_some("Depth cues"));
        });
    }
    // overrides by category: a sub-section, as Blender's (MU22)
    let oid = egui::Id::new("props.view.overrides.open");
    let open = ui.ctx().data(|d| d.get_temp::<bool>(oid)).unwrap_or(true);
    let n = v.look.overrides.len();
    let title = if n == 0 {
        "Overrides".to_string()
    } else {
        format!("Overrides ({n})")
    };
    let (resp, _) = crate::props::header(ui, &title, open, true);
    if resp
        .on_hover_text("How a category draws in this view only: halftone, its weight, colour or line pattern, its hatch off")
        .clicked()
    {
        ui.ctx().data_mut(|d| d.insert_temp(oid, !open));
    }
    if !open {
        return label;
    }
    let mut remove = None;
    for (i, o) in v.look.overrides.iter_mut().enumerate() {
        let name = match o.target {
            Target::Category(c) => c.label().to_string(),
            Target::Element(id) => model
                .get(id)
                .map_or("(deleted element)".into(), |e| e.name.clone()),
        };
        ui.push_id(("override", i), |ui| {
            crate::props::flow(ui, |ui| {
                ui.label(egui::RichText::new(name).strong());
                if ui
                    .small_button("✕")
                    .on_hover_text("Remove this override")
                    .clicked()
                {
                    remove = Some(i);
                }
            });
            if override_fields(ui, o) {
                label = Some("Graphic override");
            }
        });
    }
    if let Some(i) = remove {
        v.look.overrides.remove(i);
        label = Some("Graphic override");
    }
    let mut add = None;
    crate::props::row(ui, "Add", |ui| {
        egui::ComboBox::from_id_salt("add_override")
            .icon(crate::props::combo_icon)
            .selected_text("A category…")
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for c in STYLED {
                    let taken = v
                        .look
                        .overrides
                        .iter()
                        .any(|o| o.target == Target::Category(c));
                    if !taken && ui.selectable_label(false, c.label()).clicked() {
                        add = Some(c);
                    }
                }
            });
    });
    if let Some(c) = add {
        v.look.override_mut(Target::Category(c)).halftone = true;
        label = Some("Graphic override");
    }
    label
}

/// The fields of one override. True when one changed.
pub(super) fn override_fields(ui: &mut egui::Ui, o: &mut Override) -> bool {
    let mut changed = false;
    // one setting a row, as Blender's (MU22)
    changed |= crate::props::check(ui, "Draw", &mut o.halftone, "Halftone")
        .on_hover_text("Drawn half-strength, mixed toward white")
        .changed();
    let mut hatch_off = o.hatch == Some(false);
    if crate::props::check(ui, "", &mut hatch_off, "No hatch")
        .on_hover_text("Leave out the hatch lines of its cut faces")
        .changed()
    {
        o.hatch = hatch_off.then_some(false);
        changed = true;
    }
    crate::props::row(ui, "Weight", |ui| {
        ui.spacing_mut().combo_width = ui.available_width();
        changed |= weight_combo(ui, "ov_weight", &mut o.weight);
    });
    crate::props::row(ui, "Line", |ui| {
        ui.spacing_mut().combo_width = ui.available_width();
        changed |= pattern_combo(ui, "ov_pattern", &mut o.pattern);
    });
    crate::props::row(ui, "Colour", |ui| {
        changed |= colour_pick(ui, &mut o.colour, false, "A colour of its own in this view");
    });
    changed
}

impl CreaBimApp {
    /// The pack being edited: the project's default, else the Standard
    /// pack it draws with until it has one.
    pub(super) fn edited_pack(&self) -> (Option<ElementId>, StylePack) {
        let model = self.doc.model();
        let all = styles::packs(model);
        let found = all.iter().find(|p| p.1.default).or(all.first());
        match found {
            Some((id, p)) => (Some(*id), (*p).clone()),
            None => (None, StylePack::standard()),
        }
    }

    /// Commit an edited pack: in place, or as the project's first pack.
    pub(super) fn commit_pack(&mut self, id: Option<ElementId>, pack: StylePack, label: &str) {
        let r = match id {
            Some(id) => self.doc.edit_merged(id, label, |e| {
                e.name = pack.name.clone();
                e.kind = ElementKind::StylePack(Box::new(pack.clone()));
            }),
            None => {
                let tx = styles::apply_pack(self.doc.model(), &pack);
                self.doc.commit(Transaction {
                    label: label.into(),
                    ..tx
                })
            }
        };
        self.report(r, format!("{label}."));
    }

    /// Properties of a view: its range (plans) and its graphics.
    pub(super) fn view_graphics_sections(&mut self, ui: &mut egui::Ui, id: ElementId) {
        let mut cut_box: Option<bool> = None;
        self.view_graphics_inner(ui, id, &mut cut_box);
        if let Some(up) = cut_box {
            self.cut_box(id, up);
        }
    }

    fn view_graphics_inner(
        &mut self,
        ui: &mut egui::Ui,
        id: ElementId,
        cut_box: &mut Option<bool>,
    ) {
        let Some(ElementKind::View(v)) = self.doc.model().get(id).map(|e| &e.kind) else {
            return;
        };
        if matches!(v.kind, ViewKind::Schedule { .. }) {
            return;
        }
        let mut v = v.clone();
        let mut label = None;
        if matches!(v.kind, ViewKind::Plan { .. })
            && self.prop_header(ui, "view.range", "View range", true, Some("view-range"))
        {
            let levels: Vec<(ElementId, String)> = creabim_core::levels::sorted(self.doc.model())
                .into_iter()
                .map(|(id, name, _)| (id, name))
                .collect();
            // the cut, as in the header (MU32: only the header had it)
            if let ViewKind::Plan { cut_height, .. } = &mut v.kind {
                crate::props::row(ui, "Cut height", |ui| {
                    let r =
                        crate::props::fill(ui, crate::units::drag(cut_height).range(0.0..=100.0))
                            .on_hover_text("How high above its level the plan is cut");
                    if r.changed() {
                        label = Some("Cut height");
                    }
                });
            }
            if let Some(l) = range_ui(ui, &levels, &mut v.graphics.range) {
                label = Some(l);
            }
            // the cut as a section box in 3D (MU34 A2)
            let (down, up) = crate::props::actions2(
                ui,
                "Section box",
                (
                    "Below the cut",
                    "Cut the 3D view as this plan sees it",
                    true,
                ),
                (
                    "Above (ceilings)",
                    "Keep what is above the cut, to look up at the ceilings",
                    true,
                ),
            );
            if down || up {
                *cut_box = Some(up);
            }
        }
        // a section's cut as a section box in 3D too (MU34 A2)
        if matches!(v.kind, ViewKind::Section { .. })
            && crate::props::action(
                ui,
                "Section box",
                "In 3D",
                "Cut the 3D view as this section sees it: from its line to its depth",
                true,
            )
            .clicked()
        {
            *cut_box = Some(false);
        }
        if self.prop_header(
            ui,
            "view.look",
            "Template and overrides",
            true,
            Some("graphic-styles#view-templates"),
        ) {
            let mut edit = false;
            if let Some(l) = look_ui(ui, self.doc.model(), &mut v, &mut edit) {
                label = Some(l);
            }
            if edit {
                self.gstyles.open = true;
            }
        }
        if let Some(label) = label {
            v.look.tidy();
            let r = self.doc.edit_merged(id, label, |e| {
                if let ElementKind::View(x) = &mut e.kind {
                    *x = v.clone();
                }
            });
            self.report(r, "View changed.");
        }
    }

    /// The selected element's override in the open view: "this boundary
    /// wall dashed here".
    pub(super) fn element_override_section(&mut self, ui: &mut egui::Ui, id: ElementId) {
        let Some(view) = self.active_view else { return };
        let model = self.doc.model();
        let Some(ElementKind::View(v)) = model.get(view).map(|e| &e.kind) else {
            return;
        };
        if matches!(v.kind, ViewKind::Schedule { .. }) || id == view {
            return;
        }
        let cutters = creabim_core::visibility::cutters(model);
        if model
            .get(id)
            .and_then(|e| Category::of(e, &cutters))
            .is_none()
        {
            return;
        }
        let view_name = model.get(view).map_or("", |e| e.name.as_str()).to_string();
        let mut v = v.clone();
        if !self.prop_header(
            ui,
            "element.override",
            &format!("In {view_name}"),
            false,
            Some("graphic-styles#overrides"),
        ) {
            return;
        }
        let mut o = v
            .look
            .overrides
            .iter()
            .find(|o| o.target == Target::Element(id))
            .cloned()
            .unwrap_or_else(|| Override::new(Target::Element(id)));
        if override_fields(ui, &mut o) {
            *v.look.override_mut(Target::Element(id)) = o;
            v.look.tidy();
            let r = self.doc.edit_merged(view, "Graphic override", |e| {
                if let ElementKind::View(x) = &mut e.kind {
                    *x = v.clone();
                }
            });
            self.report(r, "Override changed.");
        }
    }

    /// The Graphic styles window, when open.
    pub(super) fn graphic_styles_window(&mut self, ctx: &egui::Context) {
        if !self.gstyles.open {
            return;
        }
        let mut open = true;
        let (id, in_model) = self.edited_pack();
        // while an option is previewed, the window shows the pack as it was
        let before = self
            .gstyles
            .preview_base
            .clone()
            .unwrap_or(in_model.clone());
        let mut pack = before.clone();
        let mut hover: Option<StylePack> = None;
        let mut action: Option<&'static str> = None;
        let mut label = "Graphic styles";
        let mut make_default = None;
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        let model = self.doc.model();
        let packs: Vec<(ElementId, String, bool)> = styles::packs(model)
            .iter()
            .map(|(i, p)| (*i, p.name.clone(), p.default))
            .collect();
        let views: Vec<ElementId> = self
            .selection
            .all()
            .iter()
            .copied()
            .filter(|i| matches!(model.get(*i).map(|e| &e.kind), Some(ElementKind::View(_))))
            .collect();
        let mut give_template: Option<String> = None;
        crate::app::shell::window("Graphic styles")
            .open(&mut open)
            .default_pos(area.min + Vec2::new(40.0, 64.0))
            .default_size(Vec2::new(940.0, 560.0))
            .constrain_to(area)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(
                        "How the project draws: pens, object styles, hatch patterns and view templates, kept together as a style pack. Load an office's pack to restyle every view and sheet; save this one to use in another project.",
                    )
                    .weak(),
                );
                crate::props::flow(ui, |ui| {
                    ui.label("Style pack");
                    egui::ComboBox::from_id_salt("pack_pick").icon(crate::props::combo_icon)
                        .selected_text(if id.is_some() {
                            pack.name.clone()
                        } else {
                            format!("{} (built in)", pack.name)
                        })
                        .show_ui(ui, |ui| {
                            for (pid, name, default) in &packs {
                                if ui.selectable_label(*default, name).clicked() {
                                    make_default = Some(*pid);
                                }
                            }
                        });
                    ui.label("Name");
                    if ui
                        .add(egui::TextEdit::singleline(&mut pack.name).vertical_align(egui::Align::Center).desired_width(140.0))
                        .changed()
                    {
                        label = "Rename style pack";
                    }
                    ui.menu_button("Add a shipped pack", |ui| {
                        for p in StylePack::shipped() {
                            if ui.button(&p.name).on_hover_text(&p.description).clicked() {
                                action = Some(if p.name == "Standard" { "standard" } else { "light" });
                                ui.close();
                            }
                        }
                    });
                    if ui.button("Load pack…").clicked() {
                        action = Some("load");
                    }
                    if ui.button("Save pack as…").clicked() {
                        action = Some("save");
                    }
                });
                if !pack.description.is_empty() {
                    ui.label(egui::RichText::new(&pack.description).weak());
                }
                ui.separator();
                crate::props::flow(ui, |ui| {
                    for (p, text) in [
                        (Page::Objects, "Object styles"),
                        (Page::Pens, "Pens"),
                        (Page::Patterns, "Patterns"),
                        (Page::Templates, "View templates"),
                        (Page::Marks, "Tags and marks"),
                    ] {
                        ui.selectable_value(&mut self.gstyles.page, p, text);
                    }
                });
                ui.separator();
                egui::ScrollArea::both().max_height(420.0).show(ui, |ui| match self.gstyles.page {
                    Page::Objects => {
                        if objects_page(ui, &mut pack) {
                            label = "Object style";
                        }
                    }
                    Page::Pens => {
                        if pens_page(ui, &mut pack) {
                            label = "Pens";
                        }
                    }
                    Page::Patterns => {
                        if patterns_page(ui, &mut pack, &mut self.gstyles.pattern) {
                            label = "Hatch pattern";
                        }
                    }
                    Page::Marks => {
                        let (changed, h) =
                            crate::choices::marks_page(ui, &mut pack, &mut self.gstyles.designer);
                        if changed {
                            label = "Tags and marks";
                        }
                        hover = h;
                    }
                    Page::Templates => {
                        if templates_page(ui, &mut pack, &mut self.gstyles.template, &views, &mut give_template) {
                            label = "View template";
                        }
                    }
                });
            });
        self.gstyles.open = open;
        if pack != before {
            self.end_preview();
            let (id, _) = self.edited_pack();
            self.commit_pack(id, pack.clone(), label);
        } else if let Some(mut h) = hover.filter(|_| open) {
            h.default = in_model.default;
            // the option under the pointer, shown on the plan until it leaves
            if h != in_model {
                if self.gstyles.preview_base.is_none() {
                    self.gstyles.preview_base = Some(before.clone());
                }
                self.commit_pack(id, h, PREVIEW);
                self.status = "Previewing on the drawings: click to keep it.".into();
            }
        } else {
            self.end_preview();
        }
        if let Some(p) = make_default {
            let tx = styles::make_default(self.doc.model(), p);
            let r = self.doc.commit(tx);
            self.report(r, "Default style pack changed.");
        }
        if let Some(name) = give_template {
            let tx = styles::set_template(self.doc.model(), &views, Some(&name));
            let n = tx.ops.len();
            let r = self.doc.commit(tx);
            self.report(r, format!("{n} views use the template {name}."));
        }
        match action {
            Some("standard") | Some("light") => {
                let p = StylePack::shipped()
                    .into_iter()
                    .find(|p| (p.name == "Standard") == (action == Some("standard")))
                    .expect("shipped");
                self.apply_style_pack(p);
            }
            Some("save") => self.save_style_pack(&pack),
            Some("load") => self.load_style_pack(),
            _ => {}
        }
    }

    /// Take back an option previewed on the plan.
    fn end_preview(&mut self) {
        if self.gstyles.preview_base.take().is_some() {
            // a first preview adds the project's pack, and the next ones
            // edit it: each its own step
            while self.doc.undo_label() == Some(PREVIEW) {
                self.doc.discard_last();
            }
            self.status.clear();
        }
    }

    pub(super) fn apply_style_pack(&mut self, p: StylePack) {
        let name = p.name.clone();
        let tx = styles::apply_pack(self.doc.model(), &p);
        let r = self.doc.commit(tx);
        self.report(
            r,
            format!("Style pack {name} applied: every view and sheet draws with it."),
        );
    }

    fn save_style_pack(&mut self, pack: &StylePack) {
        let file = format!("{}.json", pack.name.to_lowercase().replace(' ', "-"));
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save style pack")
            .set_file_name(file)
            .add_filter("Style pack", &["json"])
            .save_file()
        else {
            return;
        };
        let mut p = pack.clone();
        p.default = false;
        let r = std::fs::write(&path, p.to_json());
        self.report(r, format!("Saved {}.", path.display()));
    }

    fn load_style_pack(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Load style pack")
            .add_filter("Style pack", &["json"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| StylePack::from_json(&t))
        {
            Ok(p) => self.apply_style_pack(p),
            Err(e) => self.status = format!("Couldn't read {}: {e}", path.display()),
        }
    }
}

/// The object styles, a row per category. True when one changed.
fn objects_page(ui: &mut egui::Ui, pack: &mut StylePack) -> bool {
    let mut changed = false;
    let names = pack.pattern_names();
    egui::Grid::new("object_styles")
        .striped(true)
        .num_columns(8)
        .show(ui, |ui| {
            for h in [
                "Category",
                "Projection",
                "Cut",
                "Colour",
                "Line",
                "Cut fill",
                "Cut pattern",
                "Surface fill",
            ] {
                ui.label(egui::RichText::new(h).strong());
            }
            ui.end_row();
            for c in STYLED {
                let i = pack
                    .objects
                    .iter()
                    .position(|s| s.category == c && s.material.is_none());
                let mut s = i.map_or_else(|| ObjectStyle::new(c), |i| pack.objects[i].clone());
                let before = s.clone();
                let k = c.label();
                ui.label(k);
                weight_combo(ui, (k, "proj"), &mut s.projection_weight);
                weight_combo(ui, (k, "cut"), &mut s.cut_weight);
                crate::props::flow(ui, |ui| colour_pick(ui, &mut s.colour, false, "A line colour of its own"));
                pattern_combo(ui, (k, "pattern"), &mut s.pattern);
                crate::props::flow(ui, |ui| {
                    colour_pick(ui, &mut s.cut_fill, true, "A fill colour behind its cut hatch")
                });
                name_combo(ui, (k, "cut_pattern"), &names, &mut s.cut_pattern, "Material's");
                crate::props::flow(ui, |ui| {
                    colour_pick(
                        ui,
                        &mut s.surface_fill,
                        true,
                        "A fill for its surfaces seen from above: roofs in a roof plan, hard landscape areas",
                    )
                });
                ui.end_row();
                if s != before {
                    changed = true;
                    let empty = s == ObjectStyle::new(c);
                    match (i, empty) {
                        (Some(i), true) => {
                            pack.objects.remove(i);
                        }
                        (Some(i), false) => pack.objects[i] = s,
                        (None, false) => pack.objects.push(s),
                        (None, true) => {}
                    }
                }
            }
        });
    ui.label(
        egui::RichText::new(
            "As drawn keeps Volumen's pens. Styles for one material, surface patterns and draw order are set in the pack's file.",
        )
        .weak(),
    );
    changed
}

/// The pens for each scale. True when one changed.
fn pens_page(ui: &mut egui::Ui, pack: &mut StylePack) -> bool {
    let mut changed = false;
    if pack.pens.is_empty() {
        pack.pens = StylePack::standard().pens;
        changed = true;
    }
    let mut remove = None;
    let n = pack.pens.len();
    for (i, t) in pack.pens.iter_mut().enumerate() {
        ui.push_id(("pens", i), |ui| {
            crate::props::flow(ui, |ui| {
                ui.label(egui::RichText::new("Up to 1:").strong());
                changed |= ui
                    .add(egui::DragValue::new(&mut t.up_to).range(1.0..=100000.0))
                    .changed();
                if n > 1 && ui.small_button("✕").clicked() {
                    remove = Some(i);
                }
            });
            egui::Grid::new("pen_widths").num_columns(6).show(ui, |ui| {
                for (name, v) in t.weights.fields_mut() {
                    ui.label(name);
                    changed |= ui
                        .add(
                            egui::DragValue::new(v)
                                .speed(0.01)
                                .range(0.02..=2.0)
                                .max_decimals(2)
                                .suffix(" mm"),
                        )
                        .changed();
                    ui.end_row();
                }
            });
        });
        ui.separator();
    }
    if let Some(i) = remove {
        pack.pens.remove(i);
        changed = true;
    }
    if ui.button("Add a scale").clicked() {
        let last = pack.pens.last().cloned().expect("at least one");
        pack.pens.push(styles::PenTable {
            up_to: last.up_to * 2.0,
            ..last
        });
        changed = true;
    }
    ui.label(
        egui::RichText::new(
            "A drawing takes the first table whose scale it is at or larger than: 1:50 takes Up to 1:75.",
        )
        .weak(),
    );
    changed
}

/// The pack's hatch patterns. True when one changed.
fn patterns_page(ui: &mut egui::Ui, pack: &mut StylePack, pick: &mut usize) -> bool {
    let mut changed = false;
    crate::props::flow(ui, |ui| {
        if ui.button("New pattern").clicked() {
            let mut p = styles::builtin_pattern(creabim_core::Hatch::Diagonal);
            p.name = format!("Pattern {}", pack.patterns.len() + 1);
            pack.patterns.push(p);
            *pick = pack.patterns.len() - 1;
            changed = true;
        }
        // Volumen's library: concrete, sand, gravel, earth, brick…
        ui.menu_button("From the library", |ui| {
            egui::ScrollArea::vertical()
                .max_height(420.0)
                .show(ui, |ui| {
                    for lib in styles::library_patterns() {
                        let picked = ui
                            .horizontal(|ui| {
                                pattern_preview(ui, &lib, 100.0);
                                ui.button(&lib.name).clicked()
                            })
                            .inner;
                        if picked {
                            let mut p = lib.clone();
                            if pack.patterns.iter().any(|x| x.name == p.name) {
                                p.name = format!("{} copy", p.name);
                            }
                            pack.patterns.push(p);
                            *pick = pack.patterns.len() - 1;
                            changed = true;
                            ui.close();
                        }
                    }
                });
        });
        // AutoCAD's own: any .pat file, acadiso.pat with all its patterns
        if ui
            .button("Import .pat…")
            .on_hover_text("Read every pattern of an AutoCAD .pat file (acadiso.pat, or any other): sizes in millimetres")
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Import hatch patterns")
                .add_filter("AutoCAD hatch patterns", &["pat", "PAT"])
                .pick_file()
            {
                if let Ok(text) = std::fs::read_to_string(&path)
                    .or_else(|_| std::fs::read(&path).map(|b| b.iter().map(|c| *c as char).collect()))
                {
                    for mut p in styles::parse_pat(&text) {
                        if pack.patterns.iter().any(|x| x.name == p.name) {
                            continue;
                        }
                        p.name = p.name.clone();
                        pack.patterns.push(p);
                        changed = true;
                    }
                    *pick = pack.patterns.len().saturating_sub(1);
                }
            }
        }
        ui.menu_button("Copy a built-in", |ui| {
            for h in creabim_core::Hatch::ALL {
                if ui.button(h.label()).clicked() {
                    let mut p = styles::builtin_pattern(h);
                    if pack.patterns.iter().any(|x| x.name == p.name) {
                        p.name = format!("{} copy", p.name);
                    }
                    pack.patterns.push(p);
                    *pick = pack.patterns.len() - 1;
                    changed = true;
                    ui.close();
                }
            }
        });
    });
    ui.label(
        egui::RichText::new(
            "A pattern named like a built-in one (Brick, Concrete, Diagonal…) replaces it for every material with that hatch.",
        )
        .weak(),
    );
    if pack.patterns.is_empty() {
        return changed;
    }
    *pick = (*pick).min(pack.patterns.len() - 1);
    ui.columns(2, |cols| {
        for (i, p) in pack.patterns.iter().enumerate() {
            cols[0].horizontal(|ui| {
                pattern_preview(ui, p, 100.0);
                if ui.selectable_label(*pick == i, &p.name).clicked() {
                    *pick = i;
                }
            });
        }
        let ui = &mut cols[1];
        let mut delete = false;
        let p = &mut pack.patterns[*pick];
        crate::props::flow(ui, |ui| {
            ui.label("Name");
            changed |= ui.text_edit_singleline(&mut p.name).changed();
            delete = ui.small_button("Delete").clicked();
        });
        crate::props::flow(ui, |ui| {
            ui.label("Sizes");
            changed |= ui
                .radio_value(&mut p.units, PatternUnits::Paper, "on paper (mm)")
                .changed();
            changed |= ui
                .radio_value(&mut p.units, PatternUnits::Model, "true size (m)")
                .changed();
        });
        crate::props::flow(ui, |ui| {
            ui.label("Background");
            changed |= colour_pick(
                ui,
                &mut p.fill,
                true,
                "A fill behind the lines; none is white",
            );
        });
        let mut remove = None;
        for (j, l) in p.lines.iter_mut().enumerate() {
            ui.push_id(("line", j), |ui| {
                crate::props::flow(ui, |ui| {
                    ui.label("Angle");
                    changed |= ui
                        .add(egui::DragValue::new(&mut l.angle).speed(1.0).suffix("°"))
                        .changed();
                    ui.label("Spacing");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut l.spacing)
                                .speed(0.01)
                                .range(0.001..=100.0),
                        )
                        .changed();
                    ui.label("Offset");
                    changed |= ui
                        .add(egui::DragValue::new(&mut l.offset).speed(0.01))
                        .changed();
                    ui.label("Stagger");
                    changed |= ui
                        .add(egui::DragValue::new(&mut l.shift).speed(0.01))
                        .changed();
                    let mut dashes = l
                        .dashes
                        .iter()
                        .map(|d| d.to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    ui.label("Dashes");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut dashes)
                                .vertical_align(egui::Align::Center)
                                .desired_width(80.0),
                        )
                        .on_hover_text("Dash, gap, dash, gap…, spaced; 0 is a dot. Empty is solid.")
                        .changed()
                    {
                        l.dashes = dashes
                            .split_whitespace()
                            .filter_map(|t| t.parse::<f64>().ok())
                            .filter(|d| *d >= 0.0)
                            .collect();
                        changed = true;
                    }
                    if ui
                        .small_button("x")
                        .on_hover_text("Take this line family away")
                        .clicked()
                    {
                        remove = Some(j);
                    }
                });
            });
        }
        if let Some(j) = remove {
            p.lines.remove(j);
            changed = true;
        }
        if ui.button("Add a line family").clicked() {
            p.lines.push(HatchLine {
                angle: 45.0,
                spacing: if p.units == PatternUnits::Model {
                    0.1
                } else {
                    1.5
                },
                offset: 0.0,
                shift: 0.0,
                phase: 0.0,
                dashes: vec![],
                along_longest: false,
            });
            changed = true;
        }
        pattern_preview(ui, p, 50.0);
        if delete {
            pack.patterns.remove(*pick);
            *pick = 0;
            changed = true;
        }
    });
    changed
}

/// The pack's view templates. True when one changed.
fn templates_page(
    ui: &mut egui::Ui,
    pack: &mut StylePack,
    pick: &mut usize,
    views: &[ElementId],
    give: &mut Option<String>,
) -> bool {
    let mut changed = false;
    if ui.button("New template").clicked() {
        pack.templates.push(ViewTemplate::new(&format!(
            "Template {}",
            pack.templates.len() + 1
        )));
        *pick = pack.templates.len() - 1;
        changed = true;
    }
    if pack.templates.is_empty() {
        return changed;
    }
    *pick = (*pick).min(pack.templates.len() - 1);
    let names: Vec<String> = pack.templates.iter().map(|t| t.name.clone()).collect();
    ui.columns(2, |cols| {
        for (i, n) in names.iter().enumerate() {
            if cols[0].selectable_label(*pick == i, n).clicked() {
                *pick = i;
            }
        }
        cols[0].add_space(8.0);
        cols[0].label(egui::RichText::new("Views without a template take").strong());
        let d = &mut pack.default_templates;
        for (text, slot) in [
            ("Plans", &mut d.plan),
            ("Roof and site plans", &mut d.plan_from_above),
            ("Sections", &mut d.section),
            ("Elevations", &mut d.elevation),
            ("3D views", &mut d.axonometric),
            ("Detail views", &mut d.detail),
        ] {
            cols[0].horizontal(|ui| {
                ui.label(text);
                changed |= name_combo(ui, ("kind", text), &names, slot, "None");
            });
        }
        let ui = &mut cols[1];
        let mut delete = false;
        let t = &mut pack.templates[*pick];
        crate::props::flow(ui, |ui| {
            ui.label("Name");
            changed |= ui.text_edit_singleline(&mut t.name).changed();
            delete = ui.small_button("Delete").clicked();
        });
        crate::props::flow(ui, |ui| {
            let mut on = t.scale.is_some();
            if ui.checkbox(&mut on, "Scale 1:").changed() {
                t.scale = on.then_some(100.0);
                changed = true;
            }
            if let Some(s) = &mut t.scale {
                changed |= ui
                    .add(egui::DragValue::new(s).range(1.0..=5000.0))
                    .changed();
            }
        });
        crate::props::flow(ui, |ui| {
            let mut on = t.style.is_some();
            if ui.checkbox(&mut on, "Style").changed() {
                t.style = on.then_some(creabim_core::ViewStyle::HiddenLine);
                changed = true;
            }
            if let Some(s) = &mut t.style {
                changed |= view_style_combo(ui, "template_style", s);
            }
        });
        let mut weights = t.weights.is_some();
        if crate::props::switch(ui, &mut weights, "Pens of its own")
            .on_hover_text("Fixed pen widths whatever the scale, not the pack's")
            .changed()
        {
            t.weights = weights.then(|| creabim_core::LineWeights::for_scale(100.0));
            changed = true;
        }
        if let Some(w) = &mut t.weights {
            for (name, v) in w.fields_mut() {
                crate::props::flow(ui, |ui| {
                    ui.label(name);
                    changed |= ui
                        .add(
                            egui::DragValue::new(v)
                                .speed(0.01)
                                .range(0.02..=2.0)
                                .suffix(" mm"),
                        )
                        .changed();
                });
            }
        }
        ui.label(egui::RichText::new("Leaves out").strong());
        crate::props::flow(ui, |ui| {
            for c in STYLED {
                let mut on = t.hidden.contains(&c);
                if ui.checkbox(&mut on, c.label()).changed() {
                    t.hidden.retain(|x| *x != c);
                    if on {
                        t.hidden.push(c);
                    }
                    changed = true;
                }
            }
        });
        let mut cues = t.cues.unwrap_or_default();
        crate::props::flow(ui, |ui| {
            ui.label("Fade with distance");
            changed |= ui
                .add(egui::Slider::new(&mut cues.fade, 0.0..=0.9))
                .changed();
        });
        let mut sun = cues.shadows.is_some();
        if crate::props::switch(ui, &mut sun, "Sun shadows in plans from above").changed() {
            cues.shadows = sun.then_some(Shadows {
                direction: -50.0,
                length: 0.6,
                opacity: 0.35,
            });
            changed = true;
        }
        t.cues = (cues != DepthCues::default()).then_some(cues);
        ui.label(
            egui::RichText::new(format!(
                "{} overrides, set in the pack's file.",
                t.overrides.len()
            ))
            .weak(),
        );
        if !views.is_empty()
            && ui
                .button(format!("Give it to the {} selected views", views.len()))
                .clicked()
        {
            *give = Some(t.name.clone());
        }
        if delete {
            pack.templates.remove(*pick);
            *pick = 0;
            changed = true;
        }
    });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shipped_pack_applies_and_undoes_in_one_step() {
        let mut app = crate::app::test_app();
        let before = app.doc.model().clone();
        let light = StylePack::shipped().remove(1);
        app.apply_style_pack(light);
        assert_eq!(styles::active_pack(app.doc.model()).name, "Light linework");
        // editing the pack edits the project's copy, merged per label
        let (id, mut pack) = app.edited_pack();
        pack.pens[0].weights.cut = 0.6;
        app.commit_pack(id, pack.clone(), "Pens");
        pack.pens[0].weights.cut = 0.65;
        app.commit_pack(id, pack, "Pens");
        assert_eq!(
            styles::active_pack(app.doc.model()).pens[0].weights.cut,
            0.65
        );
        app.doc.undo();
        assert_eq!(
            styles::active_pack(app.doc.model()).pens[0].weights.cut,
            0.5
        );
        app.doc.undo();
        assert_eq!(app.doc.model(), &before);
    }
}
