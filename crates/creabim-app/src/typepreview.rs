//! A type opened on its own (double-click it in the project browser, or
//! *Open preview* in its Properties): a sample instance in isolation, seen
//! in 3D (drag to turn it), in plan and in section, beside the type's
//! values and layers, so a change shows at once. *Show in model* lights
//! every instance of the type in the 3D view and the plans, the rest left
//! in context or faint, and frames them.

use super::typesui::{type_fields, TypeAsk};
use super::*;
use creabim_core::types::{self, TypeCategory, TypeParams};

/// The preview window's own state, for the session.
pub(super) struct TypePreview {
    /// The type shown, while the window is open.
    pub open: Option<ElementId>,
    /// Where the 3D preview is seen from, in degrees.
    azimuth: f64,
    altitude: f64,
    /// The type whose instances Show in model lights.
    pub spotlight: Option<ElementId>,
    /// The sample's drawings, and what they were made for: (type, model
    /// revision, azimuth, altitude).
    cache: Option<(SampleKey, Option<Sample>)>,
}

type SampleKey = (ElementId, u64, i64, i64);

impl Default for TypePreview {
    fn default() -> Self {
        Self {
            open: None,
            azimuth: 35.0,
            altitude: 28.0,
            spotlight: None,
            cache: None,
        }
    }
}

/// A sample instance's three drawings.
#[derive(Clone)]
pub(super) struct Sample {
    pub axo: creabim_geom::PlanView,
    pub plan: creabim_geom::PlanView,
    pub section: creabim_geom::PlanView,
    /// What the sample is: "A wall 4 m long", for the captions.
    pub what: &'static str,
}

/// A model of the type `type_id` alone: one sample instance on a level,
/// with the materials, assemblies and types it is built of, and a plan, a
/// section across it and a shaded 3D view of it. None when `type_id` is
/// no type.
pub(super) fn sample_model(
    model: &Model,
    type_id: ElementId,
    azimuth: f64,
    altitude: f64,
) -> Option<(Model, [ElementId; 3], &'static str)> {
    use creabim_core::{
        Beam, Column, Footing, FootingKind, Level, Opening, Profile, Roof, Slab, View, Wall,
    };
    let ElementKind::ElementType(t) = &model.get(type_id)?.kind else {
        return None;
    };
    // what the type is built of
    let mut els: Vec<Element> = model
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                ElementKind::Material(_)
                    | ElementKind::Assembly(_)
                    | ElementKind::ElementType(_)
                    | ElementKind::Units(_)
                    | ElementKind::TextStyle(_)
            )
        })
        .cloned()
        .collect();
    let level = Element::new(
        "Sample level",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let l = level.id;
    els.push(level);
    let typed = |mut e: Element| {
        types::stamp(model, &mut e, type_id);
        e
    };
    let v2 = DVec2::new;
    // the sample, and the line its section is cut along
    let (what, section): (&'static str, [DVec2; 2]) = match t.category() {
        TypeCategory::Wall => {
            els.push(typed(Element::new(
                "Sample wall",
                ElementKind::Wall(Wall::new(l, v2(0.0, 0.0), v2(4.0, 0.0), 0.2, 3.0)),
            )));
            ("A wall 4 m long", [v2(2.0, -1.2), v2(2.0, 1.2)])
        }
        TypeCategory::Door | TypeCategory::Window => {
            let door = t.category() == TypeCategory::Door;
            let wall = Element::new(
                "Host wall",
                ElementKind::Wall(Wall::new(l, v2(0.0, 0.0), v2(4.0, 0.0), 0.3, 3.0)),
            );
            let host = wall.id;
            els.push(wall);
            let o = typed(Element::new(
                if door { "Sample door" } else { "Sample window" },
                ElementKind::Opening(Opening {
                    host,
                    kind: if door {
                        creabim_core::OpeningKind::Door
                    } else {
                        creabim_core::OpeningKind::Window
                    },
                    offset: 1.5,
                    sill: if door { 0.0 } else { 0.9 },
                    width: 1.0,
                    height: if door { 2.1 } else { 1.2 },
                    material: None,
                    frame: Default::default(),
                    flip_hand: false,
                    flip_facing: false,
                    wrap: None,
                    frame_at: None,
                }),
            ));
            // the section through its middle, whatever its width
            let mid = match &o.kind {
                ElementKind::Opening(op) => op.offset + op.width / 2.0,
                _ => 2.0,
            };
            els.push(o);
            (
                if door {
                    "A door in a plain wall"
                } else {
                    "A window in a plain wall"
                },
                [v2(mid, -1.2), v2(mid, 1.2)],
            )
        }
        TypeCategory::Slab => {
            let square = vec![v2(0.0, 0.0), v2(4.0, 0.0), v2(4.0, 3.0), v2(0.0, 3.0)];
            els.push(typed(Element::new(
                "Sample slab",
                ElementKind::Slab(Slab::new(l, square, 0.25)),
            )));
            ("A slab 4 × 3 m", [v2(2.0, -0.6), v2(2.0, 3.6)])
        }
        TypeCategory::Roof => {
            let rect = vec![v2(0.0, 0.0), v2(5.0, 0.0), v2(5.0, 4.0), v2(0.0, 4.0)];
            els.push(typed(Element::new(
                "Sample roof",
                ElementKind::Roof(Roof::new(l, rect, creabim_core::RoofShape::Gable)),
            )));
            ("A gable roof 5 × 4 m", [v2(2.5, -1.0), v2(2.5, 5.0)])
        }
        TypeCategory::Column => {
            els.push(typed(Element::new(
                "Sample column",
                ElementKind::Column(Column::new(
                    l,
                    v2(0.0, 0.0),
                    Profile::Rectangle {
                        width: 0.3,
                        depth: 0.3,
                    },
                )),
            )));
            ("A column one storey high", [v2(0.0, -0.8), v2(0.0, 0.8)])
        }
        TypeCategory::Beam => {
            els.push(typed(Element::new(
                "Sample beam",
                ElementKind::Beam(Beam::new(l, v2(0.0, 0.0), v2(4.0, 0.0))),
            )));
            ("A beam 4 m long", [v2(2.0, -0.8), v2(2.0, 0.8)])
        }
        TypeCategory::Footing => {
            let pad = matches!(&t.params, TypeParams::Footing(f) if f.kind == FootingKind::Pad);
            let f = if pad {
                Footing::pad(l, v2(0.0, 0.0))
            } else {
                Footing::strip(l, v2(0.0, 0.0), v2(4.0, 0.0))
            };
            els.push(typed(Element::new(
                "Sample footing",
                ElementKind::Footing(f),
            )));
            if pad {
                ("A pad footing", [v2(0.0, -1.2), v2(0.0, 1.2)])
            } else {
                ("A strip footing 4 m long", [v2(2.0, -1.2), v2(2.0, 1.2)])
            }
        }
        TypeCategory::Object => {
            els.push(typed(Element::new(
                "Sample object",
                ElementKind::Object(creabim_core::objects::Object::new(l, v2(0.0, 0.0), 0.0)),
            )));
            ("The object on its own", [v2(0.0, -1.5), v2(0.0, 1.5)])
        }
    };
    let views = [
        View::plan(l),
        View::section(section[0], section[1], 6.0, true),
        View::axonometric(azimuth, altitude),
    ];
    let ids = views.map(|v| {
        let e = Element::new("Preview", ElementKind::View(v));
        let id = e.id;
        els.push(e);
        id
    });
    let m = Model::from_elements(els).ok()?;
    Some((m, ids, what))
}

/// The sample's drawings.
fn sample(model: &Model, type_id: ElementId, azimuth: f64, altitude: f64) -> Option<Sample> {
    let (m, [plan, section, axo], what) = sample_model(model, type_id, azimuth, altitude)?;
    let draw = |id| creabim_geom::section::view_drawing(&m, id);
    Some(Sample {
        plan: draw(plan)?,
        section: draw(section)?,
        axo: draw(axo)?,
        what,
    })
}

impl CreaBimApp {
    /// Open the preview of type `id`.
    pub(super) fn open_type_preview(&mut self, id: ElementId) {
        if matches!(
            self.doc.model().get(id).map(|e| &e.kind),
            Some(ElementKind::ElementType(_))
        ) {
            self.type_preview.open = Some(id);
            self.status = "The type on its own: drag the 3D preview to turn it; what you change here changes every instance.".into();
        }
    }

    /// Show the instances of type `id` in the model (or stop, with None):
    /// lit as if selected in the 3D view and the plans, and framed.
    pub(super) fn show_type_in_model(&mut self, id: Option<ElementId>) {
        self.type_preview.spotlight = id;
        self.refresh_spotlight();
        let Some(id) = id else {
            self.status = "The type's instances are no longer shown.".into();
            return;
        };
        let ids = self.plan.spotlight.clone();
        if ids.is_empty() {
            self.status = "This type has no instances in the model yet.".into();
            return;
        }
        // frame them in 3D, and in the plan shown if they are on it
        let scene = (*creabim_geom::shared_scene(self.doc.model())).clone();
        let bounds = scene
            .iter()
            .filter(|s| ids.contains(&s.id))
            .filter_map(|s| s.mesh.bounds())
            .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)));
        if let Some((lo, hi)) = bounds {
            self.camera.frame(lo, hi, self.aspect);
            if let (Some((view, _)), Some(r)) = (self.active_plan(), self.rect_2d) {
                let (lo, hi) = (lo.as_dvec3().truncate(), hi.as_dvec3().truncate());
                let size = (hi - lo).max(DVec2::splat(1.0));
                let zoom = (r.width() as f64 / size.x).min(r.height() as f64 / size.y) * 0.8;
                self.plan
                    .cameras
                    .insert(view, ((lo + hi) / 2.0, zoom.clamp(2.0, 400.0) as f32));
            }
        }
        let name = self
            .doc
            .model()
            .get(id)
            .map(|e| e.name.clone())
            .unwrap_or_default();
        self.status = format!(
            "{} of {name} shown in the 3D view and the plans. Untick Show in model to stop.",
            plural_word(ids.len(), "instance")
        );
    }

    /// Keep Show in model on the type's instances as they come and go.
    pub(super) fn refresh_spotlight(&mut self) {
        let model = self.doc.model();
        let now = match self.type_preview.spotlight {
            Some(t) if model.get(t).is_some() => types::instances(model, t),
            _ => {
                self.type_preview.spotlight = None;
                vec![]
            }
        };
        if now != self.plan.spotlight {
            self.plan.spotlight = now;
            self.geometry_key = None;
        }
    }

    /// The Show in model and faint-the-rest ticks, for a type's panels.
    pub(super) fn show_in_model_ui(&mut self, ui: &mut egui::Ui, id: ElementId) {
        let n = types::instances(self.doc.model(), id).len();
        let mut on = self.type_preview.spotlight == Some(id);
        let r = ui
            .add_enabled(n > 0, egui::Checkbox::new(&mut on, "Show in model"))
            .on_hover_text("Light every instance of this type in the 3D view and the plans, as if selected, and frame them")
            .on_disabled_hover_text("No instances yet");
        if r.changed() {
            self.show_type_in_model(on.then_some(id));
        }
        if on {
            let mut ghost = self.plan.spotlight_ghost;
            if crate::props::switch(ui, &mut ghost, "Fade the rest")
                .on_hover_text("Draw everything else faint, to see them on their own; untick to see them in their context")
                .changed()
            {
                self.plan.spotlight_ghost = ghost;
                self.geometry_key = None;
            }
        }
    }

    /// The preview window, when open.
    pub(super) fn type_preview_window(&mut self, ctx: &egui::Context) {
        let Some(id) = self.type_preview.open else {
            return;
        };
        let Some((name, t)) = self.doc.model().get(id).and_then(|e| match &e.kind {
            ElementKind::ElementType(t) => Some((e.name.clone(), t.clone())),
            _ => None,
        }) else {
            self.type_preview.open = None;
            return;
        };
        let key = (
            id,
            self.doc.model().revision(),
            self.type_preview.azimuth.round() as i64,
            self.type_preview.altitude.round() as i64,
        );
        if self.type_preview.cache.as_ref().map(|c| c.0) != Some(key) {
            let s = sample(
                self.doc.model(),
                id,
                self.type_preview.azimuth,
                self.type_preview.altitude,
            );
            self.type_preview.cache = Some((key, s));
        }
        let shown = self.type_preview.cache.as_ref().and_then(|c| c.1.clone());
        let mut open = true;
        let mut asks = vec![];
        let mut turn = egui::Vec2::ZERO;
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        crate::app::shell::window(format!("Type: {name}"))
            .id(egui::Id::new("type_preview"))
            .open(&mut open)
            .default_pos(area.min + Vec2::new(60.0, 40.0))
            .constrain_to(area)
            .default_size([900.0, (area.height() - 60.0).clamp(380.0, 600.0)])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(300.0);
                        egui::ScrollArea::vertical()
                            .id_salt("type_preview_fields")
                            .max_height(ui.available_height().max(300.0))
                            .show(ui, |ui| {
                                let model = self.doc.model();
                                let mut n = name.clone();
                                if ui
                                    .add(egui::TextEdit::singleline(&mut n).vertical_align(egui::Align::Center).font(egui::TextStyle::Heading))
                                    .changed()
                                {
                                    asks.push(TypeAsk::Rename(id, n));
                                }
                                ui.label(
                                    egui::RichText::new(format!("{} · {}", t.ifc_class(), t.summary(model)))
                                        .weak(),
                                );
                                ui.add_space(6.0);
                                let mut new = t.clone();
                                let label = type_fields(ui, model, &mut new);
                                if !label.is_empty() {
                                    asks.push(TypeAsk::Edit(id, label, new));
                                }
                                ui.add_space(8.0);
                                let users = types::instances(model, id);
                                ui.label(match users.len() {
                                    0 => "No instances yet.".to_string(),
                                    n => format!(
                                        "{}. What you change here changes them all, but for what each overrides.",
                                        plural_word(n, "instance")
                                    ),
                                });
                                if !users.is_empty() && ui.button("Select them").clicked() {
                                    asks.push(TypeAsk::SelectInstances(id));
                                }
                                self.show_in_model_ui(ui, id);
                            });
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        let Some(s) = &shown else {
                            ui.label(egui::RichText::new("This type can't be drawn on its own.").weak());
                            return;
                        };
                        let w = ui.available_width().max(300.0);
                        let h = ui.available_height().max(320.0);
                        let paper = Color32::from_rgb(252, 252, 249);
                        let pane = |ui: &mut egui::Ui, size: Vec2, d: &creabim_geom::PlanView, caption: &str, sense: Sense| {
                            let (rect, r) = ui.allocate_exact_size(size, sense);
                            let p = ui.painter_at(rect);
                            p.rect_filled(rect, 4.0, paper);
                            p.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, Color32::from_gray(200)), egui::StrokeKind::Inside);
                            sheetview::draw_fit(&p, rect.shrink(10.0), d);
                            p.text(
                                rect.left_top() + Vec2::new(8.0, 6.0),
                                egui::Align2::LEFT_TOP,
                                caption,
                                egui::FontId::proportional(11.5),
                                Color32::from_gray(90),
                            );
                            r
                        };
                        let r = pane(ui, Vec2::new(w, h * 0.56), &s.axo, &format!("3D · {} · drag to turn it", s.what), Sense::drag());
                        if r.dragged() {
                            turn = r.drag_delta();
                        }
                        ui.add_space(4.0);
                        crate::props::flow(ui, |ui| {
                            let half = Vec2::new((w - 8.0) / 2.0, h * 0.44 - 10.0);
                            pane(ui, half, &s.plan, "Plan", Sense::hover());
                            pane(ui, half, &s.section, "Section across it", Sense::hover());
                        });
                    });
                });
            });
        if !open {
            self.type_preview.open = None;
        }
        if turn != egui::Vec2::ZERO {
            self.type_preview.azimuth =
                (self.type_preview.azimuth - turn.x as f64 * 0.5).rem_euclid(360.0);
            self.type_preview.altitude =
                (self.type_preview.altitude + turn.y as f64 * 0.4).clamp(5.0, 85.0);
        }
        for a in asks {
            self.run_types(a);
        }
    }
}

fn plural_word(n: usize, word: &str) -> String {
    if n == 1 {
        format!("1 {word}")
    } else {
        format!("{n} {word}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_of_type_draws_a_sample_on_its_own() {
        let model = creabim_core::starter::starter_house();
        let mut kinds = vec![];
        for (id, name, t) in types::types(&model) {
            let s = sample(&model, id, 35.0, 28.0);
            let s = s.unwrap_or_else(|| panic!("{name} has no preview"));
            for d in [&s.plan, &s.section, &s.axo] {
                assert!(d.bounds().is_some(), "{name}: an empty drawing");
            }
            if !kinds.contains(&t.category()) {
                kinds.push(t.category());
            }
        }
        // the starter house has types of every kind
        assert!(kinds.len() >= 8, "{kinds:?}");
    }

    #[test]
    fn the_sample_is_the_type_and_only_it() {
        let model = creabim_core::starter::starter_house();
        let (id, _, _) = types::types(&model)
            .into_iter()
            .find(|(_, _, t)| t.category() == TypeCategory::Wall && !t.is_curtain())
            .unwrap();
        let (m, _, _) = sample_model(&model, id, 35.0, 28.0).unwrap();
        let walls: Vec<&Element> = m
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
            .collect();
        assert_eq!(walls.len(), 1);
        assert_eq!(walls[0].of_type.as_ref().map(|t| t.id), Some(id));
        // nothing of the house but what the type is built of
        assert!(!m
            .iter()
            .any(|e| matches!(e.kind, ElementKind::Slab(_) | ElementKind::Opening(_))));
    }
}
