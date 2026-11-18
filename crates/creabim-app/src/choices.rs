//! Choices with pictures (brief 02, 2.13): the lists of room tag layouts,
//! view marker styles and grip looks, each option with a picture of
//! itself, and the marker designer that mixes parts into a style of the
//! project's own.
//!
//! The pictures of markers are the real thing: the drawing items the
//! plans and sheets are made of, drawn small.

use creabim_core::styles::{
    CalloutEdge, MarkerColour, MarkerFamily, MarkerFill, MarkerHead, MarkerLine, MarkerParts,
    MarkerPointer, MarkerText, NamedMarkerStyle, RoomTagStyle, StylePack,
};
use creabim_core::ElementId;
use creabim_geom::plan::PlanItem;
use eframe::egui::{self, Color32, Painter, Rect, Sense, Stroke, Vec2};
use glam::DVec2;

/// What the marks page keeps between frames: the designer, when open,
/// with the mix and its name.
#[derive(Clone, Debug, Default)]
pub struct Designer {
    pub open: Option<(MarkerParts, String)>,
}

/// Which marks a picture shows.
#[derive(Clone, Copy, PartialEq)]
enum Marks {
    /// A section's two heads and its line.
    Section,
    /// All four: a section, an elevation, a callout and a detail callout.
    All,
}

/// The drawing items of `parts`' marks, in paper millimetres.
fn marker_items(parts: Option<&MarkerParts>, which: Marks) -> Vec<PlanItem> {
    let id = ElementId::NIL;
    let mm = 1.0;
    let Some(parts) = parts else {
        // the first markers: a small arrow and the view's letter
        let mut out = vec![];
        for x in [-12.0, 12.0] {
            out.push(PlanItem::Shade {
                id,
                points: vec![
                    DVec2::new(x - 1.6, 0.0),
                    DVec2::new(x + 1.6, 0.0),
                    DVec2::new(x, -3.0),
                ],
                rgb: [0.17, 0.19, 0.16],
            });
            out.push(PlanItem::Text {
                id,
                at: DVec2::new(x, 3.0),
                text: "A".into(),
                size: 3.0,
                align: creabim_geom::paper::Anchor::Middle,
                mm,
                rgb: None,
                bold: false,
                angle: 0.0,
            });
        }
        out.push(PlanItem::Line {
            id,
            pen: creabim_geom::plan::Pen::Grid,
            a: DVec2::new(-12.0, 0.0),
            b: DVec2::new(12.0, 0.0),
        });
        return out;
    };
    let m = creabim_geom::markers::section;
    let half = if which == Marks::Section { 9.0 } else { 18.0 };
    let mut out = m(
        id,
        "A",
        "A301",
        DVec2::new(half, 0.0),
        DVec2::new(-half, 0.0),
        true,
        parts,
        mm,
    );
    if which == Marks::All {
        out.extend(m(
            id,
            "1",
            "A201",
            DVec2::new(58.0, 0.0),
            DVec2::new(42.0, 0.0),
            false,
            parts,
            mm,
        ));
        out.extend(creabim_geom::markers::callout(
            id,
            "3",
            "A501",
            DVec2::new(72.0, -10.0),
            DVec2::new(96.0, 6.0),
            false,
            parts,
            mm,
        ));
        out.extend(creabim_geom::markers::callout(
            id,
            "4",
            "A502",
            DVec2::new(116.0, -10.0),
            DVec2::new(132.0, 6.0),
            true,
            parts,
            mm,
        ));
    }
    out
}

/// Draw drawing items fitted into `r`, on paper.
fn draw_fitted(painter: &Painter, r: Rect, items: &[PlanItem]) {
    painter.rect_filled(r, 4.0, Color32::from_rgb(250, 250, 248));
    let pts: Vec<(DVec2, DVec2)> = items.iter().filter_map(|i| i.bounds()).collect();
    let Some(first) = pts.first() else { return };
    let (lo, hi) = pts
        .iter()
        .fold(*first, |(lo, hi), (a, b)| (lo.min(*a), hi.max(*b)));
    let size = (hi - lo).max(DVec2::splat(1.0));
    let inner = r.shrink(4.0);
    let scale = (inner.width() as f64 / size.x).min(inner.height() as f64 / size.y) as f32;
    let xf = crate::planview::Xf {
        rect: inner,
        center: (lo + hi) / 2.0,
        scale,
    };
    let painter = painter.with_clip_rect(r);
    for i in items {
        crate::planview::draw_item(&painter, &xf, i, false, 1.0);
    }
}

/// A picture of room tags laid out `style`'s way.
fn paint_room_tag(painter: &Painter, r: Rect, style: RoomTagStyle) {
    painter.rect_filled(r, 4.0, Color32::from_rgb(238, 241, 246));
    let ink = Color32::from_rgb(40, 60, 110);
    let c = r.center();
    let f = |s: f32| egui::FontId::proportional(s);
    let mid = egui::Align2::CENTER_CENTER;
    match style {
        RoomTagStyle::Stacked => {
            painter.text(c - Vec2::Y * 9.0, mid, "LOGGIA", f(10.0), ink);
            painter.text(c, mid, "16.00 m²", f(8.5), ink);
            painter.text(
                c + Vec2::Y * 8.0,
                mid,
                "0.08",
                f(7.0),
                ink.gamma_multiply(0.75),
            );
        }
        RoomTagStyle::NumberBadge => {
            let badge = Rect::from_center_size(c - Vec2::Y * 10.0, Vec2::new(22.0, 9.0));
            painter.rect_stroke(badge, 4.5, Stroke::new(1.0, ink), egui::StrokeKind::Middle);
            painter.text(badge.center(), mid, "0.08", f(7.0), ink);
            painter.text(c, mid, "LOGGIA", f(10.0), ink);
            painter.text(c + Vec2::Y * 9.0, mid, "16.00 m²", f(8.5), ink);
        }
        RoomTagStyle::Boxed => {
            let b = Rect::from_center_size(c, Vec2::new(64.0, 26.0));
            painter.rect_stroke(b, 0.0, Stroke::new(1.0, ink), egui::StrokeKind::Middle);
            let split = b.top() + 14.0;
            painter.line_segment(
                [egui::pos2(b.left(), split), egui::pos2(b.right(), split)],
                Stroke::new(1.0, ink),
            );
            painter.text(egui::pos2(c.x, b.top() + 7.0), mid, "LOGGIA", f(9.5), ink);
            painter.text(
                egui::pos2(b.left() + 4.0, split + 6.0),
                egui::Align2::LEFT_CENTER,
                "0.08",
                f(7.0),
                ink,
            );
            painter.text(
                egui::pos2(b.right() - 4.0, split + 6.0),
                egui::Align2::RIGHT_CENTER,
                "16.00 m²",
                f(7.0),
                ink,
            );
        }
        RoomTagStyle::Inline => {
            painter.text(c - Vec2::Y * 6.0, mid, "LOGGIA", f(9.0), ink);
            painter.text(c + Vec2::Y * 5.0, mid, "0.08 16.00 m²", f(9.0), ink);
        }
    }
}

/// A picture of grips in `style`'s way on a selected outline.
pub fn paint_grips(painter: &Painter, r: Rect, style: crate::handles::GripStyle) {
    use crate::handles::GripStyle as G;
    let accent = crate::theme::interaction();
    let b = r.shrink2(Vec2::new(10.0, 8.0));
    painter.rect(
        b,
        0.0,
        Color32::from_rgb(250, 220, 195),
        Stroke::new(1.4, accent),
        egui::StrokeKind::Middle,
    );
    let corners = [
        b.left_top(),
        b.right_top(),
        b.right_bottom(),
        b.left_bottom(),
    ];
    let mids = [
        (b.center_top(), Vec2::X),
        (b.right_center(), Vec2::Y),
        (b.center_bottom(), Vec2::X),
        (b.left_center(), Vec2::Y),
    ];
    match style {
        G::Quiet => {
            for c in corners {
                painter.rect(
                    Rect::from_center_size(c, Vec2::splat(6.0)),
                    1.0,
                    Color32::WHITE,
                    Stroke::new(1.2, accent),
                    egui::StrokeKind::Middle,
                );
            }
            for (c, d) in mids {
                painter.line_segment([c - d * 4.0, c + d * 4.0], Stroke::new(3.5, accent));
            }
        }
        G::Edges => {
            for c in corners {
                painter.circle_filled(c, 2.0, accent);
            }
        }
        G::OneShape => {
            for c in corners.into_iter().chain(mids.map(|m| m.0)) {
                painter.circle(c, 3.2, Color32::WHITE, Stroke::new(1.2, accent));
            }
        }
    }
}

/// One option of a list: its picture, its name and a line under it, and
/// a tick when it is the one in use. Click it to choose it; resting on it
/// previews it.
/// A picture of the interaction colour: a selected box and its grips.
pub fn paint_interaction(painter: &Painter, r: Rect, ink: Color32) {
    let b = r.shrink2(Vec2::new(16.0, 9.0));
    painter.rect(
        b,
        0.0,
        ink.gamma_multiply(0.25),
        Stroke::new(1.4, ink),
        egui::StrokeKind::Middle,
    );
    for c in [
        b.left_top(),
        b.right_top(),
        b.left_bottom(),
        b.right_bottom(),
    ] {
        painter.rect_filled(Rect::from_center_size(c, Vec2::splat(5.0)), 1.0, ink);
    }
}

/// A picture of where the Move bar sits: a slim pill at the top of a
/// view, or a small palette by the pointer.
pub fn paint_edit_bar(painter: &Painter, r: Rect, by_pointer: bool) {
    let accent = crate::theme::interaction();
    let ink = Color32::from_gray(120);
    // the view, and a box being moved in it
    painter.rect_stroke(r, 3.0, Stroke::new(1.0, ink), egui::StrokeKind::Inside);
    let block = Rect::from_min_size(
        r.left_bottom() + Vec2::new(14.0, -16.0),
        Vec2::new(16.0, 10.0),
    );
    painter.rect(
        block,
        0.0,
        Color32::from_rgb(250, 220, 195),
        Stroke::new(1.2, accent),
        egui::StrokeKind::Middle,
    );
    let bar = if by_pointer {
        Rect::from_min_size(
            block.right_top() + Vec2::new(8.0, -10.0),
            Vec2::new(38.0, 9.0),
        )
    } else {
        Rect::from_center_size(
            egui::pos2(r.center().x, r.top() + 7.0),
            Vec2::new(60.0, 7.0),
        )
    };
    painter.rect(
        bar,
        4.0,
        Color32::from_gray(70),
        Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    // its chips
    let n = if by_pointer { 4 } else { 6 };
    for i in 0..n {
        let c = egui::pos2(
            bar.left() + 6.0 + i as f32 * (bar.width() - 12.0) / (n - 1) as f32,
            bar.center().y,
        );
        painter.circle_filled(
            c,
            1.6,
            if i == n - 1 {
                accent
            } else {
                Color32::from_gray(200)
            },
        );
    }
}

pub fn choice_row(
    ui: &mut egui::Ui,
    selected: bool,
    name: &str,
    note: &str,
    paint: impl FnOnce(&Painter, Rect),
) -> egui::Response {
    let w = ui.available_width().min(420.0);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 46.0), Sense::click());
    let p = crate::theme::Palette::of(ui.ctx());
    if selected {
        ui.painter()
            .rect_filled(r, 6.0, p.accent.gamma_multiply(0.16));
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 6.0, p.raise);
    }
    let pic = Rect::from_min_size(r.min + Vec2::new(6.0, 5.0), Vec2::new(86.0, 36.0));
    paint(ui.painter(), pic);
    let x = pic.right() + 12.0;
    let at = egui::pos2(x, r.top() + if note.is_empty() { 23.0 } else { 15.0 });
    let t = ui.painter().text(
        at,
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(13.0),
        p.text,
    );
    if selected {
        // a tick after the name
        let o = egui::pos2(t.right() + 8.0, at.y);
        ui.painter().add(egui::Shape::line(
            vec![
                o + Vec2::new(0.0, 0.0),
                o + Vec2::new(3.5, 3.5),
                o + Vec2::new(9.0, -4.0),
            ],
            Stroke::new(1.8, p.accent),
        ));
    }
    if !note.is_empty() {
        ui.painter().text(
            egui::pos2(x, r.top() + 32.0),
            egui::Align2::LEFT_CENTER,
            note,
            egui::FontId::proportional(11.0),
            p.muted,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn room_tag_note(s: RoomTagStyle) -> &'static str {
    match s {
        RoomTagStyle::Stacked => "Default. Name, area, then the number small",
        RoomTagStyle::NumberBadge => "The number leads, in a small outline",
        RoomTagStyle::Boxed => "A box with a split footer",
        RoomTagStyle::Inline => "The first tag: number and area on one line",
    }
}

fn family_note(f: MarkerFamily) -> &'static str {
    match f {
        MarkerFamily::Classic => "Default. Split bubble, solid wedge",
        MarkerFamily::Minimal => "No bubbles: a small arrow and set type",
        MarkerFamily::Bold => "Filled heads, an accent wedge",
        MarkerFamily::Plain => "The first markers",
    }
}

/// The project's room tags and view markers, each a list with pictures,
/// and the marker designer. Returns whether `pack` changed, and the pack
/// as it would be with the option the pointer rests on, to preview.
pub fn marks_page(
    ui: &mut egui::Ui,
    pack: &mut StylePack,
    designer: &mut Designer,
) -> (bool, Option<StylePack>) {
    let mut changed = false;
    let mut hover: Option<StylePack> = None;
    ui.label(
        egui::RichText::new(
            "What prints on sheets, for the whole project: every tag and mark follows the choice. Rest on an option to see it on the plan; click to keep it.",
        )
        .weak(),
    );
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        crate::props::section(ui, "Room tags");
        for s in RoomTagStyle::ALL {
            let r = choice_row(
                ui,
                pack.room_tags == s,
                s.label(),
                room_tag_note(s),
                |p, r| paint_room_tag(p, r, s),
            );
            if r.clicked() && pack.room_tags != s {
                pack.room_tags = s;
                changed = true;
            } else if r.hovered() && pack.room_tags != s {
                hover = Some(StylePack {
                    room_tags: s,
                    ..pack.clone()
                });
            }
        }
        crate::props::section(ui, "Membranes in details");
        crate::props::Combo::new("Drawn as")
            .selected_text(pack.membranes.label())
            .show_ui(ui, |ui| {
                for m in creabim_core::MembraneLook::ALL {
                    changed |= ui
                        .selectable_value(&mut pack.membranes, m, m.label())
                        .changed();
                }
            });

        let ui = &mut cols[1];
        crate::props::section(ui, "View markers");
        let own = pack
            .marker_style
            .clone()
            .filter(|n| pack.marker_styles.iter().any(|s| &s.name == n));
        for f in MarkerFamily::ALL {
            let sel = own.is_none() && pack.markers == f;
            let parts = f.parts();
            let r = choice_row(ui, sel, f.label(), family_note(f), |p, r| {
                draw_fitted(p, r, &marker_items(parts.as_ref(), Marks::Section))
            });
            let with = || StylePack {
                markers: f,
                marker_style: None,
                ..pack.clone()
            };
            if r.clicked() && !sel {
                *pack = with();
                changed = true;
            } else if r.hovered() && !sel {
                hover = Some(with());
            }
        }
        if !pack.marker_styles.is_empty() {
            ui.label(egui::RichText::new("YOUR STYLES").small().weak());
        }
        let mut remove = None;
        let mut edit = None;
        for (i, st) in pack.marker_styles.clone().iter().enumerate() {
            let sel = own.as_deref() == Some(st.name.as_str());
            let r = choice_row(ui, sel, &st.name, "Your own mix", |p, r| {
                draw_fitted(p, r, &marker_items(Some(&st.parts), Marks::Section))
            });
            let with = || StylePack {
                marker_style: Some(st.name.clone()),
                ..pack.clone()
            };
            if r.clicked() && !sel {
                *pack = with();
                changed = true;
            } else if r.hovered() && !sel {
                hover = Some(with());
            }
            ui.horizontal(|ui| {
                ui.add_space(104.0);
                if ui.small_button("Edit…").clicked() {
                    edit = Some(st.clone());
                }
                if ui.small_button("Delete").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            let gone = pack.marker_styles.remove(i);
            if pack.marker_style.as_deref() == Some(gone.name.as_str()) {
                pack.marker_style = None;
            }
            changed = true;
        }
        if let Some(st) = edit {
            designer.open = Some((st.parts, st.name));
        }
        if designer.open.is_none()
            && ui
                .button(format!("+ New style from {}…", pack.marker_label()))
                .clicked()
        {
            let parts = pack.marker_parts().unwrap_or_default();
            designer.open = Some((parts, "My markers".into()));
        }
    });
    if let Some((parts, name)) = &mut designer.open {
        ui.separator();
        crate::props::section(ui, "Marker designer: mix any parts, save as a style");
        part_row(
            ui,
            "Head",
            &mut parts.head,
            &[
                (MarkerHead::Circle, "Circle"),
                (MarkerHead::RoundedBox, "Rounded box"),
                (MarkerHead::TextOnly, "Text only"),
            ],
        );
        part_row(
            ui,
            "Fill",
            &mut parts.fill,
            &[
                (MarkerFill::Outline, "Outline"),
                (MarkerFill::Filled, "Filled"),
            ],
        );
        part_row(
            ui,
            "Text",
            &mut parts.text,
            &[
                (MarkerText::Split, "Split"),
                (MarkerText::Inline, "Inline \"A · A301\""),
            ],
        );
        part_row(
            ui,
            "Pointer",
            &mut parts.pointer,
            &[
                (MarkerPointer::Wedge, "Wedge"),
                (MarkerPointer::SmallArrow, "Small arrow"),
                (MarkerPointer::AccentWedge, "Accent wedge"),
            ],
        );
        part_row(
            ui,
            "Line",
            &mut parts.line,
            &[
                (MarkerLine::FullHeavyEnds, "Full, heavy ends"),
                (MarkerLine::Thin, "Thin"),
                (MarkerLine::EndsOnly, "Ends only"),
            ],
        );
        part_row(
            ui,
            "Callout edge",
            &mut parts.edge,
            &[
                (CalloutEdge::Dashed, "Dashed"),
                (CalloutEdge::SolidThin, "Solid thin"),
                (CalloutEdge::Solid2, "Solid 2 px"),
            ],
        );
        ui.horizontal(|ui| {
            ui.add_sized([96.0, 20.0], egui::Label::new("Colour"));
            for c in MarkerColour::ALL {
                let [r, g, b] = c.rgb();
                let col =
                    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                ui.painter().circle_filled(rect.center(), 8.0, col);
                if parts.colour == c {
                    ui.painter().circle_stroke(
                        rect.center(),
                        10.0,
                        Stroke::new(1.5, crate::theme::Palette::of(ui.ctx()).text),
                    );
                }
                if resp.on_hover_text(c.label()).clicked() {
                    parts.colour = c;
                }
            }
            ui.label(egui::RichText::new("ink · accent · annotation blue").weak());
        });
        ui.label(
            egui::RichText::new(format!("PREVIEW: \"{}\"", name.to_uppercase()))
                .small()
                .weak(),
        );
        let (r, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width().min(640.0), 120.0),
            Sense::hover(),
        );
        draw_fitted(ui.painter(), r, &marker_items(Some(parts), Marks::All));
        let mut save = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(name)
                    .vertical_align(egui::Align::Center)
                    .desired_width(240.0),
            );
            save = ui
                .add_enabled(!name.trim().is_empty(), egui::Button::new("Save as style"))
                .clicked();
            cancel = ui.button("Cancel").clicked();
        });
        if save {
            let n = name.trim().to_string();
            match pack.marker_styles.iter_mut().find(|s| s.name == n) {
                Some(s) => s.parts = *parts,
                None => pack.marker_styles.push(NamedMarkerStyle {
                    name: n.clone(),
                    parts: *parts,
                }),
            }
            pack.marker_style = Some(n);
            changed = true;
            designer.open = None;
        } else if cancel {
            designer.open = None;
        }
    }
    (changed, hover)
}

/// A row of buttons, one per value of a marker part.
fn part_row<T: Copy + PartialEq>(ui: &mut egui::Ui, label: &str, v: &mut T, options: &[(T, &str)]) {
    ui.horizontal(|ui| {
        ui.add_sized([96.0, 20.0], egui::Label::new(label));
        for (o, text) in options {
            ui.selectable_value(v, *o, *text);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_marker_choice_has_a_picture() {
        for f in MarkerFamily::ALL {
            let items = marker_items(f.parts().as_ref(), Marks::All);
            assert!(!items.is_empty(), "{f:?}");
        }
        let mine = MarkerParts {
            head: MarkerHead::RoundedBox,
            fill: MarkerFill::Filled,
            pointer: MarkerPointer::SmallArrow,
            ..Default::default()
        };
        let items = marker_items(Some(&mine), Marks::All);
        // filled heads write in white
        assert!(items
            .iter()
            .any(|i| matches!(i, PlanItem::Text { rgb: Some(c), .. } if *c == [1.0, 1.0, 1.0])));
        assert_eq!(
            MarkerFamily::default().parts(),
            Some(MarkerParts::default())
        );
    }
}
