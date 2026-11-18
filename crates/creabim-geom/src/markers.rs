//! View marks in the Classic family: a split bubble with the view's number
//! over the sheet it is placed on, a solid wedge pointing the way the view
//! looks, heavy line ends and a dash-dot line between them; callouts are a
//! dashed rounded box (a circle in a section) with a leader to the bubble.
//!
//! Every size here is on paper, so the marks read the same at every scale:
//! `mm` is drawing units per paper millimetre.

use crate::plan::{Pen, PlanItem};
use creabim_core::styles::{
    CalloutEdge, MarkerColour, MarkerFill, MarkerHead, MarkerLine, MarkerParts, MarkerPointer,
    MarkerText,
};
use creabim_core::{ElementId, ElementKind, Model};
use glam::DVec2;
use std::f64::consts::{PI, TAU};

/// The bubble's radius: a 10 mm bubble.
pub const BUBBLE: f64 = 5.0;

/// The number of the sheet `view` is placed on, or "–" until it is.
pub fn sheet_ref(model: &Model, view: ElementId) -> String {
    model
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Viewport(vp) if vp.view == view => {
                crate::sheet::sheet_of(model, vp.sheet).map(|s| s.number.clone())
            }
            _ => None,
        })
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "–".into())
}

/// The number a view's marks carry: a section's letter ("Section A-A"
/// shows A), a detail's number, and an elevation's place among the
/// project's elevations.
pub fn number_of(model: &Model, view: ElementId) -> String {
    let Some(e) = model.get(view) else {
        return "–".into();
    };
    let elevation = |v: &creabim_core::View| {
        v.callout.is_none() && matches!(v.kind, creabim_core::ViewKind::Section { cut: false, .. })
    };
    if let ElementKind::View(v) = &e.kind {
        if elevation(v) {
            let n = model
                .iter()
                .filter(|x| matches!(&x.kind, ElementKind::View(w) if elevation(w)))
                .position(|x| x.id == view)
                .unwrap_or(0);
            return (n + 1).to_string();
        }
    }
    let tag = crate::detail::tag_of(&e.name);
    match tag.split_once('-') {
        Some((a, b)) if a == b && !a.is_empty() => a.to_string(),
        _ => tag,
    }
}

fn circle(id: ElementId, pen: Pen, c: DVec2, r: f64) -> PlanItem {
    PlanItem::Arc {
        id,
        pen,
        center: c,
        radius: r,
        start: 0.0,
        end: TAU,
    }
}

/// How a mark's text is written: its colour and weight.
#[derive(Clone, Copy)]
struct Ink {
    rgb: Option<[f32; 3]>,
    bold: bool,
}

/// The grey of a sheet number written beside its view's number.
const GREY: [f32; 3] = [0.42, 0.44, 0.41];
const WHITE: [f32; 3] = [1.0, 1.0, 1.0];

fn text_in(
    id: ElementId,
    at: DVec2,
    text: &str,
    size: f64,
    align: crate::paper::Anchor,
    ink: Ink,
    mm: f64,
) -> PlanItem {
    PlanItem::Text {
        id,
        at,
        text: text.to_string(),
        size,
        align,
        mm,
        rgb: ink.rgb,
        bold: ink.bold,
        angle: 0.0,
    }
}

/// The colour a mark's text takes: white on a filled head, the mark's
/// colour when it isn't ink, else the view's ink.
fn ink_of(parts: &MarkerParts, on_fill: bool) -> Ink {
    Ink {
        rgb: if on_fill {
            Some(WHITE)
        } else if parts.colour == MarkerColour::Ink {
            None
        } else {
            Some(parts.colour.rgb())
        },
        bold: on_fill,
    }
}

/// The points of a rounded box about `c`, `half` its half size and `k`
/// its corners' radius, counter-clockwise.
fn rounded_box(c: DVec2, half: DVec2, k: f64) -> Vec<DVec2> {
    let lo = c - half;
    let hi = c + half;
    let centres = [
        DVec2::new(hi.x - k, lo.y + k),
        hi - DVec2::splat(k),
        DVec2::new(lo.x + k, hi.y - k),
        lo + DVec2::splat(k),
    ];
    let mut pts = vec![];
    for (i, cc) in centres.into_iter().enumerate() {
        let a0 = -PI / 2.0 + i as f64 * PI / 2.0;
        for j in 0..=4 {
            pts.push(cc + DVec2::from_angle(a0 + PI / 2.0 * j as f64 / 4.0) * k);
        }
    }
    pts
}

fn circle_points(c: DVec2, r: f64) -> Vec<DVec2> {
    (0..32)
        .map(|i| c + DVec2::from_angle(TAU * i as f64 / 32.0) * r)
        .collect()
}

/// A closed outline as lines.
fn outline(id: ElementId, pen: Pen, pts: &[DVec2]) -> Vec<PlanItem> {
    (0..pts.len())
        .map(|i| PlanItem::Line {
            id,
            pen,
            a: pts[i],
            b: pts[(i + 1) % pts.len()],
        })
        .collect()
}

/// The pointer outside a head of radius `r` at `c`, pointing `look`.
fn pointer(id: ElementId, c: DVec2, r: f64, look: DVec2, parts: &MarkerParts, mm: f64) -> PlanItem {
    let rgb = match parts.pointer {
        MarkerPointer::AccentWedge => creabim_core::styles::ACCENT_RGB,
        _ => parts.colour.rgb(),
    };
    let points = match parts.pointer {
        MarkerPointer::Wedge | MarkerPointer::AccentWedge => {
            // from where its sides touch the head out to its tip,
            // following the head round so it never covers the text
            let tip = c + look * r * 2.0;
            let a0 = look.y.atan2(look.x);
            let half = (r / (2.0 * r)).acos(); // the tangents from the tip
            let mut pts = vec![tip];
            let n = 10;
            for i in 0..=n {
                let t = a0 - half + 2.0 * half * i as f64 / n as f64;
                pts.push(c + DVec2::from_angle(t) * r);
            }
            pts.reverse();
            pts
        }
        MarkerPointer::SmallArrow => {
            let base = c + look * (r + 0.8 * mm);
            let side = look.perp() * 1.3 * mm;
            vec![base - side, base + look * 2.0 * mm, base + side]
        }
    };
    PlanItem::Shade { id, points, rgb }
}

/// A head at `c`: `number` and `sheet` in its parts' way, and its pointer
/// toward `look` when there is one.
pub fn head(
    id: ElementId,
    c: DVec2,
    number: &str,
    sheet: &str,
    look: Option<DVec2>,
    parts: &MarkerParts,
    mm: f64,
) -> Vec<PlanItem> {
    use crate::paper::Anchor;
    let r = BUBBLE * mm;
    let mut out = vec![];
    let look = look.and_then(|l| l.try_normalize());
    let filled = parts.fill == MarkerFill::Filled && parts.head != MarkerHead::TextOnly;
    // the pointer first, so the head sits over it
    if let Some(look) = look {
        let reach = if parts.head == MarkerHead::TextOnly {
            0.0
        } else {
            r
        };
        out.push(pointer(id, c, reach.max(0.5 * mm), look, parts, mm));
    }
    let shape = match parts.head {
        MarkerHead::Circle => Some(circle_points(c, r)),
        MarkerHead::RoundedBox => Some(rounded_box(c, DVec2::new(r, r * 0.86), 1.4 * mm)),
        MarkerHead::TextOnly => None,
    };
    if let Some(pts) = &shape {
        if filled {
            out.push(PlanItem::Shade {
                id,
                points: pts.clone(),
                rgb: parts.colour.rgb(),
            });
        }
        match parts.head {
            MarkerHead::Circle => out.push(circle(id, Pen::Symbol, c, r)),
            _ => out.extend(outline(id, Pen::Symbol, pts)),
        }
    }
    let ink = ink_of(parts, filled);
    let long = number.chars().count() > 2;
    match parts.text {
        MarkerText::Split => {
            if shape.is_some() && !filled {
                out.push(PlanItem::Line {
                    id,
                    pen: Pen::Symbol,
                    a: c - DVec2::X * r,
                    b: c + DVec2::X * r,
                });
            }
            let big = if long { 2.6 } else { 3.2 };
            out.push(text_in(
                id,
                c + DVec2::Y * 2.1 * mm,
                number,
                big,
                Anchor::Middle,
                Ink {
                    bold: ink.bold || shape.is_none(),
                    ..ink
                },
                mm,
            ));
            let sheet_ink = if shape.is_none() && ink.rgb.is_none() {
                Ink {
                    rgb: Some(GREY),
                    bold: false,
                }
            } else {
                Ink { bold: false, ..ink }
            };
            out.push(text_in(
                id,
                c - DVec2::Y * 2.1 * mm,
                sheet,
                1.9,
                Anchor::Middle,
                sheet_ink,
                mm,
            ));
        }
        MarkerText::Inline => {
            if shape.is_some() {
                // "A · A301" across the head
                let size = if long { 1.9 } else { 2.2 };
                out.push(text_in(
                    id,
                    c,
                    &format!("{number} · {sheet}"),
                    size,
                    Anchor::Middle,
                    ink,
                    mm,
                ));
            } else {
                // the number in semibold, the sheet beside it in grey
                out.push(text_in(
                    id,
                    c - DVec2::X * 0.4 * mm,
                    number,
                    3.0,
                    Anchor::End,
                    Ink { bold: true, ..ink },
                    mm,
                ));
                out.push(text_in(
                    id,
                    c + DVec2::X * 0.4 * mm,
                    sheet,
                    1.9,
                    Anchor::Start,
                    Ink {
                        rgb: ink.rgb.or(Some(GREY)),
                        bold: false,
                    },
                    mm,
                ));
            }
        }
    }
    out
}

/// A split bubble at `c` in the Classic family: `number` above the line,
/// `sheet` below, and a wedge pointing `look` when there is one.
pub fn bubble(
    id: ElementId,
    c: DVec2,
    number: &str,
    sheet: &str,
    look: Option<DVec2>,
    mm: f64,
) -> Vec<PlanItem> {
    head(id, c, number, sheet, look, &MarkerParts::default(), mm)
}

/// A section line from `a` to `b` looking to its left (or an elevation
/// mark at its middle when `cut` is off), drawn with `parts`.
#[allow(clippy::too_many_arguments)]
pub fn section(
    id: ElementId,
    number: &str,
    sheet: &str,
    a: DVec2,
    b: DVec2,
    cut: bool,
    parts: &MarkerParts,
    mm: f64,
) -> Vec<PlanItem> {
    let along = (b - a).normalize_or_zero();
    let look = along.perp();
    let mut out = vec![];
    let text_only = parts.head == MarkerHead::TextOnly;
    let r = BUBBLE * mm;
    if !cut {
        let c = (a + b) / 2.0;
        if text_only {
            // the number over a small arrow, the sheet below it
            out.push(pointer(id, c, 0.5 * mm, look, parts, mm));
            let ink = ink_of(parts, false);
            out.push(text_in(
                id,
                c - look * 2.6 * mm,
                number,
                3.0,
                crate::paper::Anchor::Middle,
                Ink { bold: true, ..ink },
                mm,
            ));
            out.push(text_in(
                id,
                c + look * 5.6 * mm,
                sheet,
                1.9,
                crate::paper::Anchor::Middle,
                Ink {
                    rgb: ink.rgb.or(Some(GREY)),
                    bold: false,
                },
                mm,
            ));
        } else {
            out.extend(head(id, c, number, sheet, Some(look), parts, mm));
        }
        return out;
    }
    let stub = 6.0 * mm;
    let len = a.distance(b);
    let heavy_ends = matches!(parts.line, MarkerLine::FullHeavyEnds | MarkerLine::EndsOnly);
    if heavy_ends {
        for (p, d) in [(a, along), (b, -along)] {
            out.push(PlanItem::Line {
                id,
                pen: Pen::Cut,
                a: p,
                b: p + d * stub.min(len / 2.0),
            });
        }
    }
    match parts.line {
        MarkerLine::FullHeavyEnds if len > stub * 2.0 => out.push(PlanItem::Line {
            id,
            pen: Pen::Grid,
            a: a + along * stub,
            b: b - along * stub,
        }),
        MarkerLine::EndsOnly if len > stub * 2.0 => out.push(PlanItem::Line {
            id,
            pen: Pen::Hidden,
            a: a + along * stub,
            b: b - along * stub,
        }),
        MarkerLine::Thin => out.push(PlanItem::Line {
            id,
            pen: Pen::Grid,
            a,
            b,
        }),
        _ => {}
    }
    for p in [a, b] {
        if text_only {
            // the label above the line's end, the arrow below it
            out.push(pointer(id, p, 0.5 * mm, look, parts, mm));
            out.extend(head(
                id,
                p - look * 2.8 * mm,
                number,
                sheet,
                None,
                &MarkerParts {
                    text: MarkerText::Inline,
                    ..*parts
                },
                mm,
            ));
        } else {
            let d = if p == a { -along } else { along };
            out.extend(head(id, p + d * r, number, sheet, Some(look), parts, mm));
        }
    }
    out
}

/// A callout's box from `lo` to `hi` (a circle when `round`), edged in
/// `parts`' way, with its head: on a leader (outline heads), sitting on
/// the edge (filled heads), or as a dark label tab (text only).
#[allow(clippy::too_many_arguments)]
pub fn callout(
    id: ElementId,
    number: &str,
    sheet: &str,
    lo: DVec2,
    hi: DVec2,
    round: bool,
    parts: &MarkerParts,
    mm: f64,
) -> Vec<PlanItem> {
    let mut out = vec![];
    let (line_pen, arc_pen) = match parts.edge {
        CalloutEdge::Dashed => (Pen::Hidden, Pen::Symbol),
        CalloutEdge::SolidThin => (Pen::Symbol, Pen::Symbol),
        CalloutEdge::Solid2 => (Pen::Cut, Pen::Cut),
    };
    let c = (lo + hi) / 2.0;
    let rad = (hi - lo).abs().min_element() / 2.0;
    // where a head on a leader starts, and where one on the edge sits
    let (corner, top_left) = if round {
        let n = 24;
        let step = if parts.edge == CalloutEdge::Dashed {
            2
        } else {
            1
        };
        for i in (0..n).step_by(step) {
            out.push(PlanItem::Arc {
                id,
                pen: arc_pen,
                center: c,
                radius: rad,
                start: TAU * i as f64 / n as f64,
                end: TAU * (i + 1) as f64 / n as f64,
            });
        }
        (
            c + DVec2::from_angle(PI / 4.0) * rad,
            c + DVec2::from_angle(3.0 * PI / 4.0) * rad,
        )
    } else {
        let size = hi - lo;
        let k = (3.0 * mm).min(size.min_element() / 3.0);
        let cs = [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)];
        for i in 0..4 {
            let (p, q) = (cs[i], cs[(i + 1) % 4]);
            let d = (q - p).normalize_or_zero();
            out.push(PlanItem::Line {
                id,
                pen: line_pen,
                a: p + d * k,
                b: q - d * k,
            });
        }
        let centres = [
            lo + DVec2::splat(k),
            DVec2::new(hi.x - k, lo.y + k),
            hi - DVec2::splat(k),
            DVec2::new(lo.x + k, hi.y - k),
        ];
        for (i, cc) in centres.into_iter().enumerate() {
            let start = PI + i as f64 * PI / 2.0;
            out.push(PlanItem::Arc {
                id,
                pen: arc_pen,
                center: cc,
                radius: k,
                start,
                end: start + PI / 2.0,
            });
        }
        (
            hi - DVec2::splat(k) + DVec2::splat(k * std::f64::consts::FRAC_1_SQRT_2),
            DVec2::new(lo.x, hi.y),
        )
    };
    let r = BUBBLE * mm;
    if parts.head == MarkerHead::TextOnly {
        // a dark tab on the top edge: "3 · A501" in white
        let label = format!("{number} · {sheet}");
        let w = crate::paper::text_width(&label, 2.4) * mm + 3.0 * mm;
        let h = 4.2 * mm;
        let at = if round {
            top_left + DVec2::new(rad * 0.9, h * 0.2)
        } else {
            top_left + DVec2::new(2.0 * mm + w / 2.0, h * 0.25)
        };
        let pts = rounded_box(at, DVec2::new(w / 2.0, h / 2.0), 0.8 * mm);
        out.push(PlanItem::Shade {
            id,
            points: pts,
            rgb: parts.colour.rgb(),
        });
        out.push(text_in(
            id,
            at,
            &label,
            2.4,
            crate::paper::Anchor::Middle,
            Ink {
                rgb: Some(WHITE),
                bold: true,
            },
            mm,
        ));
    } else if parts.fill == MarkerFill::Filled {
        // the head sits on the edge
        let at = if round {
            c + DVec2::from_angle(PI / 3.0) * rad
        } else {
            DVec2::new(hi.x, hi.y)
        };
        out.extend(head(id, at, number, sheet, None, parts, mm));
    } else {
        let at = corner + DVec2::new(9.0, 6.0) * mm;
        let d = (at - corner).normalize_or_zero();
        out.push(PlanItem::Line {
            id,
            pen: Pen::Symbol,
            a: corner,
            b: at - d * r,
        });
        out.extend(head(id, at, number, sheet, None, parts, mm));
    }
    out
}

/// In a plan's items, the section and elevation marks redrawn with
/// `parts`: each section view's own items are taken out and its marks put
/// in their place.
pub fn restyle_sections(model: &Model, items: &mut Vec<PlanItem>, parts: &MarkerParts, mm: f64) {
    let mut seen: Vec<ElementId> = vec![];
    items.retain(|it| {
        let id = it.id();
        let is_section = matches!(
            model.get(id).map(|e| &e.kind),
            Some(ElementKind::View(v)) if v.callout.is_none()
                && matches!(v.kind, creabim_core::ViewKind::Section { .. })
        );
        if is_section && !seen.contains(&id) {
            seen.push(id);
        }
        !is_section
    });
    for id in seen {
        let Some(e) = model.get(id) else { continue };
        let ElementKind::View(v) = &e.kind else {
            continue;
        };
        if let creabim_core::ViewKind::Section {
            start, end, cut, ..
        } = v.kind
        {
            let number = number_of(model, id);
            let sheet = sheet_ref(model, id);
            items.extend(section(id, &number, &sheet, start, end, cut, parts, mm));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_classic_section_has_two_split_bubbles_and_heavy_ends() {
        let id = ElementId::new();
        let items = section(
            id,
            "A",
            "A301",
            DVec2::ZERO,
            DVec2::new(10.0, 0.0),
            true,
            &MarkerParts::default(),
            0.1,
        );
        let texts: Vec<&str> = items
            .iter()
            .filter_map(|i| match i {
                PlanItem::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["A", "A301", "A", "A301"]);
        let heavy = items
            .iter()
            .filter(|i| matches!(i, PlanItem::Line { pen: Pen::Cut, .. }))
            .count();
        assert_eq!(heavy, 2);
        // the wedges point the way the view looks: to the left of the line
        for i in &items {
            if let PlanItem::Shade { points, .. } = i {
                assert!(points.iter().any(|p| p.y > 0.9), "{points:?}");
                assert!(points.iter().all(|p| p.y > -1e-9));
            }
        }
    }

    #[test]
    fn the_sheet_reference_waits_for_a_sheet() {
        let m = creabim_core::sample::sample_house();
        let placed = m.iter().find_map(|e| match &e.kind {
            ElementKind::Viewport(vp) => Some(vp.view),
            _ => None,
        });
        if let Some(v) = placed {
            assert_ne!(sheet_ref(&m, v), "–");
        }
        assert_eq!(sheet_ref(&m, ElementId::new()), "–");
    }

    #[test]
    fn a_bubbles_text_fits_inside_the_bubble() {
        // at 1:50 in metres, 0.05 units to a paper millimetre
        let mm = 0.05;
        let c = DVec2::new(3.0, 4.0);
        let r = BUBBLE * mm;
        let items = bubble(ElementId::new(), c, "12", "A-101", None, mm);
        let texts: Vec<_> = items
            .iter()
            .filter(|i| matches!(i, PlanItem::Text { .. }))
            .collect();
        assert_eq!(texts.len(), 2);
        for t in texts {
            let (lo, hi) = t.bounds().unwrap();
            for p in [lo, hi, DVec2::new(lo.x, hi.y), DVec2::new(hi.x, lo.y)] {
                assert!(
                    p.distance(c) <= r * 1.05,
                    "{t:?} reaches {} from the centre of a bubble of {r}",
                    p.distance(c)
                );
            }
        }
    }
}
