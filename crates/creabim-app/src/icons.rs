//! Small drawn icons for the model list, one per kind of element, so
//! categories are easy to tell apart at a glance. They are painted with
//! lines and shapes rather than font glyphs, which the UI font lacks.

use creabim_core::{Element, ElementKind, OpeningKind, ViewKind};
use eframe::egui::{self, vec2, Color32, Painter, Pos2, Rect, Stroke};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Storey,
    Grid,
    Wall,
    Slab,
    Door,
    Window,
    Roof,
    Column,
    Beam,
    Mass,
    Material,
    Plan,
    Section,
    Elevation,
    View3d,
    Schedule,
    Sheet,
    Viewport,
    Dimension,
    Tag,
    Sketch,
    Guide,
    CropBox,
}

impl Icon {
    /// The icon for a group of the model list, by its IFC class.
    pub fn of_class(class: &str) -> Icon {
        match class {
            "IfcBuildingStorey" => Icon::Storey,
            "IfcGrid" => Icon::Grid,
            "IfcWall"
            | "IfcWallStandardCase"
            | "IfcCurtainWall"
            | "IfcWallType"
            | "IfcCurtainWallType" => Icon::Wall,
            "IfcSlab" | "IfcCovering" | "IfcPlate" | "IfcSlabType" => Icon::Slab,
            "IfcDoor" | "IfcDoorType" => Icon::Door,
            "IfcWindow" | "IfcWindowType" => Icon::Window,
            "IfcRoof" | "IfcRoofType" => Icon::Roof,
            "IfcColumn" | "IfcPile" | "IfcColumnType" => Icon::Column,
            "IfcBeam" | "IfcMember" | "IfcBeamType" => Icon::Beam,
            "IfcMaterial" => Icon::Material,
            "IfcMaterialLayerSet" => Icon::Slab,
            "View" => Icon::Plan,
            "Sheet" => Icon::Sheet,
            "Viewport" => Icon::Viewport,
            "Dimension" => Icon::Dimension,
            "Tag" | "Note" => Icon::Tag,
            "TextStyle" => Icon::Material,
            "Sketch" => Icon::Sketch,
            "Guide" => Icon::Guide,
            "CropBox" => Icon::CropBox,
            _ => Icon::Mass,
        }
    }

    /// The icon for one element: views get one per kind of drawing.
    pub fn of(e: &Element) -> Icon {
        match &e.kind {
            ElementKind::View(v) => match &v.kind {
                ViewKind::Plan { .. } => Icon::Plan,
                ViewKind::Section { cut: true, .. } => Icon::Section,
                ViewKind::Section { cut: false, .. } => Icon::Elevation,
                ViewKind::Axonometric { .. } | ViewKind::Perspective { .. } => Icon::View3d,
                ViewKind::Schedule { .. } => Icon::Schedule,
            },
            ElementKind::Opening(o) => match o.kind {
                OpeningKind::Door => Icon::Door,
                OpeningKind::Window => Icon::Window,
                OpeningKind::Hole => Icon::Mass,
            },
            _ => Icon::of_class(e.ifc_class()),
        }
    }

    fn color(self) -> Color32 {
        match self {
            Icon::Storey | Icon::Grid | Icon::Guide => Color32::from_rgb(70, 110, 170),
            Icon::Wall => Color32::from_rgb(170, 80, 50),
            Icon::Slab => Color32::from_rgb(120, 120, 120),
            Icon::Door => Color32::from_rgb(140, 90, 40),
            Icon::Window => Color32::from_rgb(40, 120, 190),
            Icon::Roof => Color32::from_rgb(180, 60, 60),
            Icon::Column | Icon::Beam => Color32::from_rgb(130, 95, 55),
            Icon::Mass => Color32::from_rgb(60, 130, 200),
            Icon::Material => Color32::from_rgb(200, 140, 40),
            Icon::Plan
            | Icon::Section
            | Icon::Elevation
            | Icon::View3d
            | Icon::Schedule
            | Icon::CropBox => Color32::from_rgb(40, 140, 110),
            Icon::Sheet | Icon::Viewport => Color32::from_rgb(110, 90, 160),
            Icon::Dimension | Icon::Tag => Color32::from_rgb(30, 60, 140),
            Icon::Sketch => crate::theme::interaction(),
        }
    }

    /// Paint the icon filling `r` (best at 14–16 px).
    pub fn paint(self, painter: &Painter, r: Rect) {
        let c = self.color();
        let s = Stroke::new(1.3, c);
        let thin = Stroke::new(1.0, c);
        // points in a unit square, y down
        let p = |x: f32, y: f32| -> Pos2 { r.min + vec2(x * r.width(), y * r.height()) };
        let line = |a: (f32, f32), b: (f32, f32), st: Stroke| {
            painter.line_segment([p(a.0, a.1), p(b.0, b.1)], st)
        };
        let poly = |pts: &[(f32, f32)], fill: Color32, st: Stroke| {
            painter.add(egui::Shape::convex_polygon(
                pts.iter().map(|(x, y)| p(*x, *y)).collect(),
                fill,
                st,
            ));
        };
        let soft = c.gamma_multiply(0.35);
        match self {
            Icon::Storey => {
                for y in [0.25, 0.5, 0.75] {
                    line((0.1, y), (0.9, y), s);
                }
                poly(&[(0.1, 0.12), (0.3, 0.25), (0.1, 0.38)], c, Stroke::NONE);
            }
            Icon::Grid => {
                for t in [0.3, 0.7] {
                    line((t, 0.05), (t, 0.95), thin);
                    line((0.05, t), (0.95, t), thin);
                }
            }
            Icon::Wall => {
                painter.rect_filled(Rect::from_two_pos(p(0.05, 0.2), p(0.95, 0.8)), 0.0, soft);
                painter.rect_stroke(
                    Rect::from_two_pos(p(0.05, 0.2), p(0.95, 0.8)),
                    0.0,
                    thin,
                    egui::StrokeKind::Inside,
                );
                line((0.05, 0.5), (0.95, 0.5), thin);
                line((0.5, 0.2), (0.5, 0.5), thin);
                line((0.28, 0.5), (0.28, 0.8), thin);
                line((0.72, 0.5), (0.72, 0.8), thin);
            }
            Icon::Slab => {
                poly(
                    &[(0.05, 0.55), (0.35, 0.3), (0.95, 0.3), (0.65, 0.55)],
                    soft,
                    thin,
                );
                poly(
                    &[(0.05, 0.55), (0.65, 0.55), (0.65, 0.72), (0.05, 0.72)],
                    c,
                    Stroke::NONE,
                );
            }
            Icon::Door => {
                line((0.15, 0.9), (0.85, 0.9), s);
                line((0.25, 0.9), (0.25, 0.15), s);
                painter.add(egui::Shape::line(
                    (0..=8)
                        .map(|i| {
                            let a = std::f32::consts::FRAC_PI_2 * i as f32 / 8.0;
                            p(0.25 + 0.6 * a.sin(), 0.9 - 0.75 * a.cos())
                        })
                        .collect(),
                    thin,
                ));
            }
            Icon::Window => {
                let f = Rect::from_two_pos(p(0.12, 0.12), p(0.88, 0.88));
                painter.rect_filled(f, 1.0, soft);
                painter.rect_stroke(f, 1.0, s, egui::StrokeKind::Inside);
                line((0.5, 0.12), (0.5, 0.88), thin);
                line((0.12, 0.5), (0.88, 0.5), thin);
            }
            Icon::Roof => {
                poly(&[(0.05, 0.6), (0.5, 0.15), (0.95, 0.6)], soft, s);
                line((0.2, 0.6), (0.2, 0.9), thin);
                line((0.8, 0.6), (0.8, 0.9), thin);
                line((0.2, 0.9), (0.8, 0.9), thin);
            }
            Icon::Column => {
                painter.rect_filled(Rect::from_two_pos(p(0.35, 0.15), p(0.65, 0.85)), 0.0, c);
                line((0.2, 0.12), (0.8, 0.12), s);
                line((0.2, 0.88), (0.8, 0.88), s);
            }
            Icon::Beam => {
                painter.rect_filled(Rect::from_two_pos(p(0.05, 0.4), p(0.95, 0.6)), 0.0, c);
                line((0.15, 0.6), (0.15, 0.9), thin);
                line((0.85, 0.6), (0.85, 0.9), thin);
            }
            Icon::Mass => {
                // an isometric cube
                poly(
                    &[(0.5, 0.08), (0.9, 0.3), (0.5, 0.52), (0.1, 0.3)],
                    c.gamma_multiply(0.55),
                    thin,
                );
                poly(
                    &[(0.1, 0.3), (0.5, 0.52), (0.5, 0.95), (0.1, 0.72)],
                    soft,
                    thin,
                );
                poly(
                    &[(0.5, 0.52), (0.9, 0.3), (0.9, 0.72), (0.5, 0.95)],
                    c,
                    thin,
                );
            }
            Icon::Material => {
                painter.circle_filled(p(0.5, 0.5), r.width() * 0.38, soft);
                painter.circle_stroke(p(0.5, 0.5), r.width() * 0.38, s);
                line((0.25, 0.75), (0.75, 0.25), thin);
                line((0.42, 0.85), (0.85, 0.42), thin);
                line((0.15, 0.58), (0.58, 0.15), thin);
            }
            Icon::Plan => {
                painter.rect_stroke(
                    Rect::from_two_pos(p(0.1, 0.1), p(0.9, 0.9)),
                    0.0,
                    s,
                    egui::StrokeKind::Inside,
                );
                line((0.5, 0.1), (0.5, 0.55), thin);
                line((0.1, 0.55), (0.7, 0.55), thin);
            }
            Icon::Section => {
                line(
                    (0.1, 0.5),
                    (0.9, 0.5),
                    Stroke::new(1.0, c.gamma_multiply(0.6)),
                );
                line((0.1, 0.5), (0.1, 0.2), s);
                line((0.9, 0.5), (0.9, 0.2), s);
                poly(&[(0.0, 0.25), (0.1, 0.05), (0.2, 0.25)], c, Stroke::NONE);
                poly(&[(0.8, 0.25), (0.9, 0.05), (1.0, 0.25)], c, Stroke::NONE);
                painter.rect_filled(Rect::from_two_pos(p(0.3, 0.62), p(0.7, 0.9)), 0.0, soft);
            }
            Icon::Elevation => {
                painter.circle_stroke(p(0.4, 0.5), r.width() * 0.25, s);
                poly(&[(0.62, 0.3), (0.95, 0.5), (0.62, 0.7)], c, Stroke::NONE);
            }
            Icon::View3d => {
                poly(
                    &[(0.5, 0.08), (0.9, 0.3), (0.5, 0.52), (0.1, 0.3)],
                    soft,
                    thin,
                );
                line((0.1, 0.3), (0.1, 0.72), thin);
                line((0.9, 0.3), (0.9, 0.72), thin);
                line((0.5, 0.52), (0.5, 0.95), thin);
                line((0.1, 0.72), (0.5, 0.95), thin);
                line((0.9, 0.72), (0.5, 0.95), thin);
            }
            Icon::Schedule => {
                let f = Rect::from_two_pos(p(0.08, 0.15), p(0.92, 0.85));
                painter.rect_stroke(f, 0.0, s, egui::StrokeKind::Inside);
                painter.rect_filled(Rect::from_two_pos(p(0.08, 0.15), p(0.92, 0.35)), 0.0, soft);
                line((0.08, 0.6), (0.92, 0.6), thin);
                line((0.4, 0.15), (0.4, 0.85), thin);
            }
            Icon::Sheet => {
                let f = Rect::from_two_pos(p(0.05, 0.18), p(0.95, 0.82));
                painter.rect_stroke(f, 0.0, s, egui::StrokeKind::Inside);
                painter.rect_filled(Rect::from_two_pos(p(0.6, 0.62), p(0.95, 0.82)), 0.0, c);
            }
            Icon::Viewport => {
                let f = Rect::from_two_pos(p(0.1, 0.15), p(0.9, 0.85));
                painter.add(egui::Shape::dashed_line(
                    &[
                        f.left_top(),
                        f.right_top(),
                        f.right_bottom(),
                        f.left_bottom(),
                        f.left_top(),
                    ],
                    thin,
                    2.0,
                    2.0,
                ));
            }
            Icon::Dimension => {
                line((0.1, 0.55), (0.9, 0.55), thin);
                line((0.1, 0.3), (0.1, 0.8), thin);
                line((0.9, 0.3), (0.9, 0.8), thin);
                line((0.03, 0.62), (0.17, 0.48), s);
                line((0.83, 0.62), (0.97, 0.48), s);
            }
            Icon::Tag => {
                let f = Rect::from_two_pos(p(0.15, 0.3), p(0.9, 0.7));
                painter.rect_stroke(f, 1.0, s, egui::StrokeKind::Inside);
                line((0.15, 0.7), (0.0, 0.95), thin);
            }
            Icon::Sketch => {
                painter.add(egui::Shape::line(
                    vec![p(0.1, 0.85), p(0.3, 0.35), p(0.6, 0.6), p(0.9, 0.15)],
                    s,
                ));
                painter.circle_filled(p(0.9, 0.15), 1.6, c);
            }
            Icon::Guide => {
                painter.add(egui::Shape::dashed_line(
                    &[p(0.05, 0.95), p(0.95, 0.05)],
                    s,
                    2.5,
                    2.0,
                ));
            }
            Icon::CropBox => {
                // a dashed box with a grip on each side
                let corners = [p(0.15, 0.15), p(0.85, 0.15), p(0.85, 0.85), p(0.15, 0.85)];
                for i in 0..4 {
                    painter.add(egui::Shape::dashed_line(
                        &[corners[i], corners[(i + 1) % 4]],
                        thin,
                        2.0,
                        1.5,
                    ));
                }
                for (x, y) in [(0.5, 0.15), (0.85, 0.5), (0.5, 0.85), (0.15, 0.5)] {
                    painter.rect_filled(Rect::from_center_size(p(x, y), vec2(3.0, 3.0)), 0.0, c);
                }
            }
        }
    }
}

/// A row of the model list: the icon, then a selectable name.
pub fn row(ui: &mut egui::Ui, icon: Icon, selected: bool, text: &str) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), egui::Sense::hover());
        icon.paint(ui.painter(), r);
        ui.selectable_label(selected, text)
    })
    .inner
}
