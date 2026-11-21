//! Detail components and callouts drawn: the 2D drafting of
//! [`creabim_core::detail`], and the box a detail view is called out by on
//! its parent.

use crate::plan::{Pen, PlanItem};
use creabim_core::{DetailItem, DetailKind, Drafting, ElementId, Hatch, MembraneLook};
use glam::DVec2;

/// How far off the line it follows a membrane's heavy line lies, in paper
/// millimetres, and its partner beyond it.
pub const MEMBRANE_OFFSET: f64 = 2.0;
pub const MEMBRANE_PARTNER: f64 = 3.5;

/// `pts` moved `by` to their left (right when negative), corners mitred.
pub fn offset_path(pts: &[DVec2], by: f64) -> Vec<DVec2> {
    creabim_core::offset::offset_path(pts, by)
}

/// The linework of a detail component, in its view's drawing coordinates;
/// `mm` is drawing units per paper millimetre (a membrane stands off its
/// line by paper millimetres), `look` how membranes are drawn, `drafting`
/// the project's drafting layers, which colour, break and hide plain
/// linework, and its symbols.
pub fn detail_items(
    id: ElementId,
    d: &DetailItem,
    mm: f64,
    look: MembraneLook,
    drafting: &Drafting,
) -> Vec<PlanItem> {
    let layers = &drafting.layers;
    let mut out = Vec::new();
    let line = |out: &mut Vec<PlanItem>, pen: Pen, a: DVec2, b: DVec2| {
        out.push(PlanItem::Line { id, pen, a, b });
    };
    let segs: Vec<(DVec2, DVec2)> = d
        .points
        .windows(2)
        .map(|w| (w[0], w[1]))
        .filter(|(a, b)| a.distance(*b) > 1e-9)
        .collect();
    let w = d.width.max(0.0);
    match d.kind {
        DetailKind::Batt => {
            // a quilt: a wave from face to face, a loop every width along
            let half = w / 2.0;
            for &(a, b) in &segs {
                let len = a.distance(b);
                let dir = (b - a) / len;
                let n = dir.perp();
                for s in [-half, half] {
                    line(&mut out, Pen::Fine, a + n * s, b + n * s);
                }
                if half < 1e-6 {
                    continue;
                }
                let pitch = (w * 0.6).max(1e-3);
                let steps = ((len / pitch) * 16.0).ceil().max(2.0) as usize;
                let at = |t: f64| {
                    let u = t * len;
                    // a figure-of-eight-ish loop: the point runs along and
                    // swings across, doubling back at each face
                    let phase = u / pitch * std::f64::consts::TAU;
                    a + dir * (u - pitch * 0.3 * phase.sin()) + n * (half * 0.95 * phase.cos())
                };
                let mut prev = at(0.0);
                for i in 1..=steps {
                    let p = at(i as f64 / steps as f64);
                    line(&mut out, Pen::Symbol, prev, p);
                    prev = p;
                }
            }
        }
        DetailKind::Membrane => {
            // never on the line it follows: a heavy line just off it, on
            // the side it was clicked, and a partner beyond
            let pts: Vec<DVec2> = d.points.clone();
            let side = d.side() * mm;
            let heavy = offset_path(&pts, side * MEMBRANE_OFFSET);
            let partner = offset_path(&pts, side * MEMBRANE_PARTNER);
            for w in heavy.windows(2) {
                line(&mut out, Pen::Cut, w[0], w[1]);
            }
            match look {
                MembraneLook::Dashed => {
                    for w in partner.windows(2) {
                        line(&mut out, Pen::Hidden, w[0], w[1]);
                    }
                }
                MembraneLook::Double => {
                    for w in partner.windows(2) {
                        line(&mut out, Pen::Fine, w[0], w[1]);
                    }
                }
                MembraneLook::ZigZag => {
                    let pitch = 1.5 * mm;
                    for (h, p) in heavy.windows(2).zip(partner.windows(2)) {
                        let len = h[0].distance(h[1]);
                        let steps = ((len / pitch).ceil() as usize).max(1);
                        let mut prev = h[0];
                        for k in 1..=steps {
                            let t = k as f64 / steps as f64;
                            let q = if k % 2 == 1 {
                                p[0].lerp(p[1], t)
                            } else {
                                h[0].lerp(h[1], t)
                            };
                            line(&mut out, Pen::Symbol, prev, q);
                            prev = q;
                        }
                    }
                }
            }
        }
        DetailKind::Flashing => {
            let half = w.max(1e-4) / 2.0;
            for &(a, b) in &segs {
                let n = (b - a).normalize().perp() * half;
                out.push(PlanItem::Polygon {
                    id,
                    pen: Pen::Cut,
                    points: vec![a - n, b - n, b + n, a + n],
                });
            }
        }
        DetailKind::Blocking => {
            let half = w / 2.0;
            for &(a, b) in &segs {
                let n = (b - a).normalize().perp() * half;
                let c = [a - n, b - n, b + n, a + n];
                out.push(PlanItem::Polygon {
                    id,
                    pen: Pen::Projection,
                    points: c.to_vec(),
                });
                line(&mut out, Pen::Symbol, c[0], c[2]);
                line(&mut out, Pen::Symbol, c[1], c[3]);
            }
        }
        DetailKind::Repeat { module, joint } => {
            let half = w / 2.0;
            let step = (module + joint).max(1e-3);
            for &(a, b) in &segs {
                let len = a.distance(b);
                let dir = (b - a) / len;
                let n = dir.perp() * half;
                let mut u = 0.0;
                while u < len - 1e-6 {
                    let e = (u + module).min(len);
                    let (p, q) = (a + dir * u, a + dir * e);
                    out.push(PlanItem::Polygon {
                        id,
                        pen: Pen::Projection,
                        points: vec![p - n, q - n, q + n, p + n],
                    });
                    u += step;
                }
            }
        }
        DetailKind::Region { hatch } => {
            let pts = d.points.clone();
            if pts.len() >= 3 {
                // a region masks what is under it, then takes its hatch
                out.push(PlanItem::Shade {
                    id,
                    points: pts.clone(),
                    rgb: [1.0, 1.0, 1.0],
                });
                if hatch != Hatch::Empty {
                    push_hatch(&mut out, id, &pts, hatch, d, mm);
                }
                for i in 0..pts.len() {
                    line(&mut out, Pen::Fine, pts[i], pts[(i + 1) % pts.len()]);
                }
            }
        }
        DetailKind::Draft { .. } => {
            // on a hidden layer it isn't drawn
            let (Some(l), Some((mut path, closed))) = (d.look(layers), d.draft_path()) else {
                return out;
            };
            if let (Some(hatch), true) = (d.fill, closed) {
                if hatch != Hatch::Empty && path.len() >= 3 {
                    push_hatch(&mut out, id, &path, hatch, d, mm);
                }
            }
            if closed {
                path.extend(path.first().copied());
            }
            let pen = Pen::Draft {
                weight: l.weight,
                rgb: l.rgb,
            };
            // a broken line is drawn dash by dash, so its pattern runs on
            // round corners and curves and is the same on screen, on
            // paper and in a CAD file
            // a zigzag and batting are drawn as their shape along the line
            if matches!(
                l.linetype,
                creabim_core::LineType::Zigzag | creabim_core::LineType::Batting
            ) {
                path = shaped_path(&path, l.linetype == creabim_core::LineType::Batting, mm);
            }
            let runs = match l.linetype.pattern() {
                Some(dashes) => {
                    let dashes: Vec<f64> = dashes.iter().map(|x| x * mm).collect();
                    crate::looks::dash_polyline(&path, &dashes, 0.0)
                }
                None => vec![path],
            };
            for run in runs {
                for w in run.windows(2) {
                    if w[0].distance(w[1]) > 1e-12 {
                        line(&mut out, pen, w[0], w[1]);
                    }
                }
            }
        }
        DetailKind::Symbol { def } => {
            // hidden with its layer; else each piece where the symbol puts it
            if d.layer
                .as_deref()
                .and_then(|n| layers.effective(n))
                .is_some_and(|l| l.hidden)
            {
                return out;
            }
            match drafting.symbols.get(&def) {
                Some(symbol) => {
                    for piece in d.symbol_pieces(symbol) {
                        out.extend(detail_items(id, &piece, mm, look, drafting));
                    }
                }
                // its symbol is gone: a cross where it was
                None => {
                    let (Some(o), Some(x)) = (d.points.first(), d.points.get(1)) else {
                        return out;
                    };
                    let u = (*x - *o) * 0.5;
                    line(&mut out, Pen::Symbol, *o - u, *o + u);
                    line(&mut out, Pen::Symbol, *o - u.perp(), *o + u.perp());
                }
            }
        }
        DetailKind::BreakLine => {
            for &(a, b) in &segs {
                let len = a.distance(b);
                let dir = (b - a) / len;
                let n = dir.perp();
                let h = (len * 0.06).clamp(0.02, 0.3);
                let m = (a + b) / 2.0;
                let zig = [
                    a,
                    m - dir * h * 0.5,
                    m - dir * h * 0.15 + n * h,
                    m + dir * h * 0.15 - n * h,
                    m + dir * h * 0.5,
                    b,
                ];
                for z in zig.windows(2) {
                    line(&mut out, Pen::Symbol, z[0], z[1]);
                }
            }
        }
    }
    out
}

/// The short reference a callout shows: the first word of the view's name
/// with a digit in it ("Detail 3 Eaves" shows 3), else its last word.
pub fn tag_of(name: &str) -> String {
    name.split_whitespace()
        .find(|w| w.chars().any(|c| c.is_ascii_digit()))
        .or_else(|| name.rsplit(' ').next())
        .unwrap_or(name)
        .to_string()
}

/// A callout's box on its parent view, and its number in a bubble at the
/// box's top right corner; `mm` is drawing units per paper millimetre.
pub fn callout_items(id: ElementId, name: &str, lo: DVec2, hi: DVec2, mm: f64) -> Vec<PlanItem> {
    let c = [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)];
    let mut out: Vec<PlanItem> = (0..4)
        .map(|i| PlanItem::Line {
            id,
            pen: Pen::Symbol,
            a: c[i],
            b: c[(i + 1) % 4],
        })
        .collect();
    // a short leader out of the corner to the bubble
    let bubble = hi + DVec2::splat(4.5 * mm);
    out.push(PlanItem::Line {
        id,
        pen: Pen::Symbol,
        a: hi,
        b: bubble - DVec2::splat(2.5 * mm * std::f64::consts::FRAC_1_SQRT_2),
    });
    out.push(PlanItem::Label {
        id,
        at: bubble,
        text: tag_of(name),
    });
    out
}

/// A path drawn as a zigzag, or as batting's loops (insulation), along
/// it: `mm` is drawing units per paper millimetre.
pub fn shaped_path(path: &[DVec2], loops: bool, mm: f64) -> Vec<DVec2> {
    if path.len() < 2 {
        return path.to_vec();
    }
    let mut cum = vec![0.0];
    for w in path.windows(2) {
        cum.push(cum.last().unwrap() + w[0].distance(w[1]));
    }
    let total = *cum.last().unwrap();
    if total < 1e-9 {
        return path.to_vec();
    }
    // a point and the way the path runs, `s` along it
    let at = |s: f64| -> (DVec2, DVec2) {
        let s = s.clamp(0.0, total);
        let i = cum.partition_point(|c| *c < s).clamp(1, path.len() - 1);
        let (a, b) = (path[i - 1], path[i]);
        let len = (cum[i] - cum[i - 1]).max(1e-12);
        let t = ((s - cum[i - 1]) / len).clamp(0.0, 1.0);
        (a + (b - a) * t, (b - a).normalize_or_zero())
    };
    let (wave, amp) = if loops {
        (2.6 * mm, 1.3 * mm)
    } else {
        (2.0 * mm, 0.8 * mm)
    };
    let steps = ((total / wave) * if loops { 24.0 } else { 2.0 })
        .ceil()
        .clamp(2.0, 40_000.0) as usize;
    let mut out = Vec::with_capacity(steps + 1);
    for k in 0..=steps {
        let s = total * k as f64 / steps as f64;
        let phi = std::f64::consts::TAU * s / wave;
        let (q, d) = if loops {
            // a prolate cycloid: running back a little at each turn makes
            // the loops
            let back = -amp * 0.9 * phi.sin();
            let (p, d) = at(s + back);
            (p + d.perp() * amp * phi.cos(), d)
        } else {
            let (p, d) = at(s);
            let tri = if k % 2 == 0 { -amp } else { amp };
            (p + d.perp() * tri, d)
        };
        let _ = d;
        out.push(q);
    }
    out
}

/// A hatch over `pts`: as the drawing style draws hatches, or, scaled or
/// turned (the piece's hatch scale and angle, as AutoCAD's), its lines
/// drawn here at that size and angle over the hatch's fill.
fn push_hatch(
    out: &mut Vec<PlanItem>,
    id: ElementId,
    pts: &[DVec2],
    hatch: Hatch,
    d: &DetailItem,
    mm: f64,
) {
    let (k, a) = (d.hatch_scale, d.hatch_angle);
    if (k - 1.0).abs() < 1e-9 && a.abs() < 1e-9 || hatch == Hatch::Solid {
        out.push(PlanItem::Fill {
            id,
            points: pts.to_vec(),
            hatch,
            rgb: None,
        });
        return;
    }
    // the fill behind, then the pattern's lines turned about the outline's
    // first corner
    out.push(PlanItem::Shade {
        id,
        points: pts.to_vec(),
        rgb: [crate::paper::hatch_fill(hatch) as f32; 3],
    });
    let o = pts[0];
    let turn = DVec2::from_angle(a.to_radians());
    let back = DVec2::from_angle(-a.to_radians());
    let local: Vec<DVec2> = pts.iter().map(|p| o + back.rotate(*p - o)).collect();
    let spacing = crate::paper::HATCH_SPACING * mm * k.max(0.01);
    for (p, q) in crate::plan::hatch_lines(hatch, &local, spacing) {
        out.push(PlanItem::Line {
            id,
            pen: Pen::Fine,
            a: o + turn.rotate(p - o),
            b: o + turn.rotate(q - o),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::LinePen;

    fn item(kind: DetailKind, pts: &[(f64, f64)]) -> DetailItem {
        DetailItem::new(
            ElementId::new(),
            kind,
            pts.iter().map(|&(x, y)| DVec2::new(x, y)).collect(),
        )
    }

    #[test]
    fn each_kind_draws_within_its_band() {
        for kind in DetailKind::ALL {
            let pts: &[(f64, f64)] = if kind.as_draft().is_some() {
                // a circle, an ellipse or a polygon about (0.5, 0)
                &[(0.5, 0.0), (1.0, 0.0), (0.7, 0.3)]
            } else if kind.is_area() {
                &[(0.0, 0.0), (1.0, 0.0), (1.0, 0.5)]
            } else {
                &[(0.0, 0.0), (1.0, 0.0)]
            };
            let d = item(kind, pts);
            let items = detail_items(
                ElementId::new(),
                &d,
                0.01,
                MembraneLook::default(),
                &Drafting::default(),
            );
            assert!(!items.is_empty(), "{kind:?} draws something");
            let (lo, hi) = crate::plan::PlanView { items }.bounds().unwrap();
            let reach = d.width.max(0.06) + 0.6;
            assert!(lo.y > -reach && hi.y < reach, "{kind:?}: {lo} {hi}");
            assert!(lo.x > -0.2 && hi.x < 1.2, "{kind:?}: {lo} {hi}");
        }
    }

    #[test]
    fn a_membrane_never_lies_on_the_line_it_follows() {
        let mut d = item(DetailKind::Membrane, &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]);
        // 1:10, a paper millimetre is 10 mm
        for look in MembraneLook::ALL {
            let items = detail_items(ElementId::new(), &d, 0.01, look, &Drafting::default());
            let lines: Vec<(Pen, DVec2, DVec2)> = items
                .iter()
                .filter_map(|i| match i {
                    PlanItem::Line { pen, a, b, .. } => Some((*pen, *a, *b)),
                    _ => None,
                })
                .collect();
            // the heavy line is 2 mm on paper to the left, mitred round the corner
            let heavy: Vec<_> = lines.iter().filter(|l| l.0 == Pen::Cut).collect();
            assert_eq!(heavy.len(), 2);
            assert!(heavy[0].1.distance(DVec2::new(0.0, 0.02)) < 1e-9);
            assert!(heavy[0].2.distance(DVec2::new(0.98, 0.02)) < 1e-9);
            assert!(
                lines.iter().all(|l| l.1.y > 1e-6 || l.1.x < 1.0 - 1e-6),
                "{look:?}"
            );
            assert!(
                lines.iter().any(|l| l.0 != Pen::Cut),
                "{look:?} has a partner"
            );
        }
        // flipped, it goes the other way
        d.flip = true;
        let items = detail_items(
            ElementId::new(),
            &d,
            0.01,
            MembraneLook::Dashed,
            &Drafting::default(),
        );
        assert!(items.iter().any(|i| matches!(i,
            PlanItem::Line { pen: Pen::Cut, a, .. } if a.distance(DVec2::new(0.0, -0.02)) < 1e-9)));
    }

    #[test]
    fn plain_linework_draws_its_shape_in_its_pen() {
        use creabim_core::DraftShape as S;
        let lines = |shape: S, pen: LinePen, pts: &[(f64, f64)]| -> Vec<(DVec2, DVec2)> {
            let d = item(DetailKind::Draft { shape, pen }, pts);
            detail_items(
                ElementId::new(),
                &d,
                0.01,
                MembraneLook::default(),
                &Drafting::default(),
            )
            .into_iter()
            .map(|i| match i {
                PlanItem::Line { pen: p, a, b, .. } => {
                    assert!(matches!(p, Pen::Draft { .. }));
                    (a, b)
                }
                other => panic!("{other:?}"),
            })
            .collect()
        };
        let p = DVec2::new;
        // a line of two stretches; closed, a third back to its start
        assert_eq!(
            lines(S::Line, LinePen::Medium, &[(0., 0.), (1., 0.), (1., 1.)]).len(),
            2
        );
        assert_eq!(
            lines(S::Closed, LinePen::Heavy, &[(0., 0.), (1., 0.), (1., 1.)]).len(),
            3
        );
        // a rectangle from two corners, a hexagon from centre and corner
        assert_eq!(
            lines(S::Rectangle, LinePen::Fine, &[(0., 0.), (2., 1.)]).len(),
            4
        );
        let hex = lines(
            S::Polygon { sides: 6 },
            LinePen::Light,
            &[(0., 0.), (1., 0.)],
        );
        assert_eq!(hex.len(), 6);
        assert!(hex.iter().all(|(a, b)| (a.distance(*b) - 1.0).abs() < 1e-9));
        // every chord of a circle ends on it
        let circle = lines(S::Circle, LinePen::Fine, &[(1., 1.), (3., 1.)]);
        assert!(circle
            .iter()
            .all(|(a, _)| (a.distance(p(1., 1.)) - 2.0).abs() < 1e-9));
        assert!(circle[0].0.distance(circle.last().unwrap().1) < 1e-9);
        // an arc runs from its start through its middle point to its end,
        // whichever way round that is
        for mid in [1.0, -1.0] {
            let arc = lines(S::Arc, LinePen::Medium, &[(-1., 0.), (0., mid), (1., 0.)]);
            assert!(arc[0].0.distance(p(-1., 0.)) < 1e-9);
            assert!(arc.last().unwrap().1.distance(p(1., 0.)) < 1e-9);
            assert!(arc.iter().all(|(a, _)| (a.length() - 1.0).abs() < 1e-9));
            assert!(arc.iter().any(|(a, _)| a.distance(p(0., mid)) < 0.05));
            assert!(arc.iter().all(|(a, _)| a.y * mid > -1e-9));
        }
        // three points in a row are a straight line
        assert_eq!(
            lines(S::Arc, LinePen::Medium, &[(0., 0.), (1., 0.), (2., 0.)]).len(),
            2
        );
        // an ellipse 2 by 1 about the origin, turned 90 degrees
        let ell = lines(S::Ellipse, LinePen::Medium, &[(0., 0.), (0., 2.), (1., 5.)]);
        assert!(ell
            .iter()
            .all(|(a, _)| ((a.y / 2.0).powi(2) + a.x.powi(2) - 1.0).abs() < 1e-9));
        // a spline passes through every point it was given
        let pts = [(0., 0.), (1., 1.), (2., 0.), (3., 1.)];
        let sp = lines(S::Spline, LinePen::Medium, &pts);
        for (x, y) in pts {
            assert!(sp
                .iter()
                .any(|(a, b)| a.distance(p(x, y)) < 1e-9 || b.distance(p(x, y)) < 1e-9));
        }
        // a rectangle is kept as a closed outline that can turn
        let baked = item(DetailKind::draft(S::Rectangle), &[(0., 0.), (2., 1.)]).baked();
        assert_eq!(baked.kind, DetailKind::draft(S::Closed));
        assert_eq!(baked.points.len(), 4);
        assert!((baked.length() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn linework_takes_its_colour_dashes_fill_and_hiding_from_itself_and_its_layer() {
        use creabim_core::{DraftLayer, DraftShape as S, LineType};
        let layers = creabim_core::DraftLayers {
            layers: vec![
                DraftLayer {
                    name: "Axes".into(),
                    color: Some([200, 0, 0]),
                    linetype: LineType::DashDot,
                    hidden: false,
                    parent: None,
                    locked: false,
                    pen: None,
                },
                DraftLayer {
                    name: "Notes".into(),
                    color: None,
                    linetype: LineType::Continuous,
                    hidden: true,
                    parent: None,
                    locked: false,
                    pen: None,
                },
            ],
        };
        let drafting = Drafting {
            layers,
            ..Default::default()
        };
        let draw = |d: &DetailItem| {
            detail_items(ElementId::new(), d, 0.1, MembraneLook::default(), &drafting)
        };
        let pens = |items: &[PlanItem]| -> Vec<Pen> {
            items
                .iter()
                .filter_map(|i| match i {
                    PlanItem::Line { pen, .. } => Some(*pen),
                    _ => None,
                })
                .collect()
        };
        let mut d = item(DetailKind::draft(S::Line), &[(0.0, 0.0), (10.0, 0.0)]);
        // on no layer: one black line of its weight
        assert_eq!(
            pens(&draw(&d)),
            vec![Pen::Draft {
                weight: LinePen::Medium,
                rgb: None
            }]
        );
        // on the axes layer: red, in dashes and dots (6 + 0.3 mm every
        // 9.3 mm of paper, at 100 mm a paper mm: about 11 of each)
        d.layer = Some("Axes".into());
        let on = pens(&draw(&d));
        assert!(on.len() > 15 && on.len() < 25, "{}", on.len());
        assert!(on.iter().all(|p| matches!(
            p,
            Pen::Draft {
                rgb: Some([200, 0, 0]),
                ..
            }
        )));
        // its own colour and line type win over the layer's
        d.color = Some([0, 0, 255]);
        d.linetype = Some(LineType::Continuous);
        assert_eq!(
            pens(&draw(&d)),
            vec![Pen::Draft {
                weight: LinePen::Medium,
                rgb: Some([0, 0, 255])
            }]
        );
        // on a hidden layer nothing is drawn
        d.layer = Some("Notes".into());
        assert!(draw(&d).is_empty());
        // the dashed pen is a light dashed line
        let dashed = item(
            DetailKind::Draft {
                shape: S::Line,
                pen: LinePen::Dashed,
            },
            &[(0.0, 0.0), (10.0, 0.0)],
        );
        let dp = pens(&draw(&dashed));
        assert!(
            dp.len() > 5
                && dp.iter().all(|p| matches!(
                    p,
                    Pen::Draft {
                        weight: LinePen::Light,
                        ..
                    }
                ))
        );
        // a closed shape can be filled; an open one can't
        let mut c = item(DetailKind::draft(S::Circle), &[(0.0, 0.0), (1.0, 0.0)]);
        c.fill = Some(Hatch::Diagonal);
        assert!(matches!(
            draw(&c)[0],
            PlanItem::Fill {
                hatch: Hatch::Diagonal,
                ..
            }
        ));
        let mut l = item(
            DetailKind::draft(S::Line),
            &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)],
        );
        l.fill = Some(Hatch::Diagonal);
        assert!(!draw(&l).iter().any(|i| matches!(i, PlanItem::Fill { .. })));
    }

    #[test]
    fn a_placed_symbol_draws_its_pieces_turned_and_sized() {
        use creabim_core::{DraftShape as S, DraftSymbol};
        // a symbol of one line along its x axis, two units long
        let def = ElementId::new();
        let mut drafting = Drafting::default();
        drafting.symbols.insert(
            def,
            DraftSymbol {
                pieces: vec![item(DetailKind::draft(S::Line), &[(0.0, 0.0), (2.0, 0.0)])],
            },
        );
        // placed at (5, 5), its x axis pointing up and half size
        let placed = item(DetailKind::Symbol { def }, &[(5.0, 5.0), (5.0, 5.5)]);
        let items = detail_items(
            ElementId::new(),
            &placed,
            0.01,
            MembraneLook::default(),
            &drafting,
        );
        assert_eq!(items.len(), 1);
        let PlanItem::Line { a, b, .. } = &items[0] else {
            panic!()
        };
        assert!(a.distance(DVec2::new(5.0, 5.0)) < 1e-9 && b.distance(DVec2::new(5.0, 6.0)) < 1e-9);
        // without its symbol it is a cross, not nothing
        let lost = detail_items(
            ElementId::new(),
            &placed,
            0.01,
            MembraneLook::default(),
            &Drafting::default(),
        );
        assert_eq!(lost.len(), 2);
    }

    #[test]
    fn bricks_repeat_along_the_path() {
        let d = item(
            DetailKind::Repeat {
                module: 0.215,
                joint: 0.01,
            },
            &[(0.0, 0.0), (0.9, 0.0)],
        );
        let units = detail_items(
            ElementId::new(),
            &d,
            0.01,
            MembraneLook::default(),
            &Drafting::default(),
        )
        .iter()
        .filter(|i| matches!(i, PlanItem::Polygon { .. }))
        .count();
        assert_eq!(units, 4);
    }
}
