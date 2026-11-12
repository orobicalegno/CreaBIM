//! Dimensions and tags: where they sit in a view's drawing, what they
//! read, and the linework they are drawn with. Both follow their hosts,
//! because their positions are worked out from the model every time.

use crate::annotation::{end_marks, measure, runs_past, Annotation, Ends, Shape, TextLook};
use crate::plan::PlanItem;
use crate::section::{AxoFrame, SectionFrame};
use creabim_core::{
    DimensionKind, ElementId, ElementKind, HostPoint, Model, Note, OpeningKind, RoomField,
    TagShape, Terminator, View, ViewKind, Wall,
};
use glam::{DAffine3, DMat3, DVec2, DVec3};

/// Maps world points into a view's drawing coordinates.
pub enum Projector {
    Plan,
    Section(SectionFrame),
    Axo(AxoFrame),
}

impl Projector {
    pub fn of(model: &Model, view: &View) -> Option<Self> {
        Some(match view.kind {
            ViewKind::Plan { .. } => Projector::Plan,
            ViewKind::Section {
                start, end, depth, ..
            } => Projector::Section(SectionFrame::new(start, end, depth)),
            ViewKind::Axonometric { azimuth, altitude } => {
                Projector::Axo(AxoFrame::new(model, azimuth, altitude))
            }
            ViewKind::Schedule { .. } => return None,
        })
    }

    pub fn project(&self, p: DVec3) -> DVec2 {
        match self {
            Projector::Plan => p.truncate(),
            Projector::Section(f) => f.project(p).truncate(),
            Projector::Axo(f) => f.project(p).truncate(),
        }
    }
}

/// A wall's frame: a point `u` along it and `s` to its left.
fn on_wall(w: &Wall, u: f64, s: f64) -> DVec2 {
    w.start + w.direction() * u + w.direction().perp() * s
}

fn host_wall(model: &Model, id: ElementId) -> Option<(&Wall, f64)> {
    match &model.get(id)?.kind {
        ElementKind::Wall(w) => Some((w, 0.0)),
        ElementKind::Opening(o) => match &model.get(o.host)?.kind {
            ElementKind::Wall(w) => Some((w, o.offset)),
            _ => None,
        },
        _ => None,
    }
}

/// Where a dimension point sits in a view's drawing, if it can be drawn there.
pub fn resolve(model: &Model, view: &View, p: &HostPoint) -> Option<DVec2> {
    let Some(host) = p.host else {
        return Some(p.local);
    };
    match (&model.get(host)?.kind, &view.kind) {
        (ElementKind::Level(l), ViewKind::Section { .. }) => {
            Some(DVec2::new(p.local.x, l.elevation + p.local.y))
        }
        (ElementKind::Wall(_) | ElementKind::Opening(_), ViewKind::Plan { .. }) => {
            let (w, base) = host_wall(model, host)?;
            let u = if p.from_end {
                w.length() + p.local.x
            } else {
                base + p.local.x
            };
            Some(on_wall(w, u, p.local.y))
        }
        _ => {
            let body = p.body?;
            Some(
                Projector::of(model, view)?
                    .project(host_frame(model, host)?.transform_point3(body)),
            )
        }
    }
}

/// An element's own 3D axes in the model, for dimension points kept on it:
/// a mass's origin, turn and size; a slab's first corner and edge; a grid,
/// wall or guide's start and direction; a sketch's plane.
pub fn host_frame(model: &Model, id: ElementId) -> Option<DAffine3> {
    let elevation = |level: Option<ElementId>| {
        level
            .and_then(|l| model.level(l))
            .map_or(0.0, |l| l.elevation)
    };
    // axes along `x` (made level), across it and up, at `o`
    let along = |o: DVec3, x: DVec3| {
        let x = DVec3::new(x.x, x.y, 0.0)
            .try_normalize()
            .unwrap_or(DVec3::X);
        DAffine3::from_mat3_translation(DMat3::from_cols(x, DVec3::Z.cross(x), DVec3::Z), o)
    };
    Some(match &model.get(id)?.kind {
        ElementKind::Wall(w) => along(
            w.start.extend(elevation(Some(w.level)) + w.base_offset),
            (w.end - w.start).extend(0.0),
        ),
        ElementKind::Opening(o) => {
            let ElementKind::Wall(w) = &model.get(o.host)?.kind else {
                return None;
            };
            let z = elevation(Some(w.level)) + w.base_offset + o.sill;
            along(
                on_wall(w, o.offset, 0.0).extend(z),
                (w.end - w.start).extend(0.0),
            )
        }
        ElementKind::Slab(s) => {
            let a = *s.boundary.first()?;
            let b = s.boundary.get(1).copied().unwrap_or(a + DVec2::X);
            along(a.extend(elevation(Some(s.level))), (b - a).extend(0.0))
        }
        ElementKind::Grid(g) => along(g.start.extend(0.0), (g.end - g.start).extend(0.0)),
        ElementKind::Guide(g) => along(g.a + DVec3::Z * elevation(g.level), g.b - g.a),
        ElementKind::FreeMesh(m) => DAffine3::from_mat3_translation(
            DMat3::from_rotation_z(m.rotation.to_radians()) * m.scale,
            m.origin + DVec3::Z * elevation(m.level),
        ),
        ElementKind::Sketch(sk) => {
            let p = &sk.plane;
            DAffine3::from_mat3_translation(
                DMat3::from_cols(p.x, p.y, p.x.cross(p.y)),
                p.origin + DVec3::Z * elevation(sk.level),
            )
        }
        _ => return None,
    })
}

/// A drawing point of `view` back in the model: on the level in a plan, on
/// the cut plane in a section.
fn unproject(model: &Model, view: &View, at: DVec2) -> Option<DVec3> {
    Some(match view.kind {
        ViewKind::Plan { level, .. } => at.extend(model.level(level).map_or(0.0, |l| l.elevation)),
        ViewKind::Section {
            start, end, depth, ..
        } => {
            let f = SectionFrame::new(start, end, depth);
            (f.origin + f.along * at.x).extend(at.y)
        }
        _ => return None,
    })
}

/// A dimension point attached to `host`, expressed in the host's frame so
/// it follows the host. `at` is a drawing point in `view`.
pub fn attach(model: &Model, view: &View, host: Option<ElementId>, at: DVec2) -> HostPoint {
    let attached = host.and_then(|h| match (&model.get(h)?.kind, &view.kind) {
        (ElementKind::Level(l), ViewKind::Section { .. }) => Some(HostPoint {
            host: Some(h),
            local: DVec2::new(at.x, at.y - l.elevation),
            from_end: false,
            body: None,
        }),
        (ElementKind::Wall(_) | ElementKind::Opening(_), ViewKind::Plan { .. }) => {
            let (w, base) = host_wall(model, h)?;
            let d = w.direction();
            let u = (at - w.start).dot(d);
            let s = (at - w.start).dot(d.perp());
            let is_wall = matches!(model.get(h)?.kind, ElementKind::Wall(_));
            // points nearer a wall's end are kept relative to the end, so
            // they follow it when the wall is lengthened
            let from_end = is_wall && u > w.length() / 2.0;
            Some(HostPoint {
                host: Some(h),
                local: DVec2::new(if from_end { u - w.length() } else { u - base }, s),
                from_end,
                body: None,
            })
        }
        (ElementKind::Level(_), _) => None,
        _ => {
            let frame = host_frame(model, h)?;
            let world = unproject(model, view, at)?;
            Some(HostPoint {
                host: Some(h),
                local: at,
                from_end: false,
                body: Some(frame.inverse().transform_point3(world)),
            })
        }
    });
    attached.unwrap_or(HostPoint::free(at))
}

/// The centre of an element's body, in world coordinates.
pub fn centre(model: &Model, id: ElementId) -> Option<DVec3> {
    let e = model.get(id)?;
    let elevation = |level: ElementId| model.level(level).map_or(0.0, |l| l.elevation);
    Some(match &e.kind {
        ElementKind::Wall(w) => on_wall(w, w.length() / 2.0, 0.0)
            .extend(elevation(w.level) + w.base_offset + w.height / 2.0),
        ElementKind::Opening(o) => {
            let ElementKind::Wall(w) = &model.get(o.host)?.kind else {
                return None;
            };
            on_wall(w, o.offset + o.width / 2.0, 0.0)
                .extend(elevation(w.level) + w.base_offset + o.sill + o.height / 2.0)
        }
        ElementKind::Slab(s) => {
            let c = s.boundary.iter().copied().sum::<DVec2>() / s.boundary.len().max(1) as f64;
            c.extend(s.top(elevation(s.level)) - s.thickness / 2.0)
        }
        ElementKind::FreeMesh(m) => {
            let mesh = m.evaluated().mesh;
            let n = mesh.vertices.len().max(1) as f64;
            let c = mesh.vertices.iter().copied().sum::<DVec3>() / n;
            c + m.origin + DVec3::Z * m.level.map_or(0.0, elevation)
        }
        // furniture and fixtures: the middle of their parts
        ElementKind::Object(_) => {
            let mut pts = creabim_core::objects::model_parts(model, e)
                .into_iter()
                .flat_map(|(m, _)| m.vertices);
            let first = pts.next()?;
            let (lo, hi) = pts.fold((first, first), |(lo, hi), v| (lo.min(v), hi.max(v)));
            (lo + hi) / 2.0
        }
        // a room: its point on its floor
        ElementKind::Room(r) => r.at.extend(elevation(r.level)),
        // a ceiling: the middle of its outline, on its underside
        ElementKind::Ceiling(c) => {
            let o = c.outline(model)?;
            let middle = o.iter().copied().sum::<DVec2>() / o.len().max(1) as f64;
            middle.extend(c.span(model).0)
        }
        // columns, beams, roofs, stairs and railings: the middle of their shape
        k if creabim_core::members::is_member(k) => {
            let (mesh, _) = crate::members::member_mesh(model, e)?;
            let (lo, hi) = mesh.bounds()?;
            ((lo + hi) / 2.0).as_dvec3()
        }
        _ => return None,
    })
}

/// What a material tag on `id` reads: its layers, one line each with
/// their thickness, outside to inside, or its one material's name.
pub fn material_text(model: &Model, id: ElementId) -> String {
    let Some(e) = model.get(id) else {
        return String::new();
    };
    let tag = |m: f64| creabim_core::write_number(model, creabim_core::NumberKind::Tags, None, m);
    if let Some((_, a)) = creabim_core::assembly_of(model, e) {
        if !a.layers.is_empty() {
            return a
                .layers
                .iter()
                .map(|l| {
                    let name = l
                        .material
                        .and_then(|m| model.get(m))
                        .map_or("(no material)".to_string(), |m| m.name.clone());
                    format!("{} {name}", tag(l.thickness))
                })
                .collect::<Vec<_>>()
                .join("\n");
        }
    }
    creabim_core::material_of(model, e).map_or("(no material)".into(), |(m, _)| m.name.clone())
}

/// What a tag on `id` reads, and whether it sits in a circle (doors) or a box.
pub fn tag_text(model: &Model, id: ElementId) -> (String, bool) {
    let Some(e) = model.get(id) else {
        return (String::new(), false);
    };
    let material = creabim_core::material_of(model, e).map(|(m, _)| m.name.clone());
    let units = creabim_core::units::number_format(model, creabim_core::NumberKind::Tags, None)
        .units(&model.units());
    let tag = |m: f64| creabim_core::write_number(model, creabim_core::NumberKind::Tags, None, m);
    let with_material = |t: f64| match &material {
        Some(m) => format!("{m} {}", tag(t)),
        None => tag(t),
    };
    // a mark, typed or numbered by the project's rules, is what it reads
    if !matches!(e.kind, ElementKind::Object(_)) {
        if let Some(m) = creabim_core::marks::mark_of(model, id) {
            let round = matches!(&e.kind, ElementKind::Opening(o) if o.kind == OpeningKind::Door);
            return (m, round);
        }
    }
    match &e.kind {
        ElementKind::Opening(o) => (e.name.clone(), o.kind == OpeningKind::Door),
        ElementKind::Wall(w) => (with_material(w.thickness), false),
        ElementKind::Slab(s) => (with_material(s.thickness), false),
        ElementKind::Column(c) => (c.profile.label_in(&units), false),
        ElementKind::Beam(b) => (format!("{} × {}", tag(b.width), tag(b.depth)), false),
        ElementKind::Roof(r) => (
            format!("{} {:.0}°", with_material(r.thickness), r.pitch),
            false,
        ),
        // furniture and fixtures: their type's mark, in a circle like doors
        ElementKind::Object(_) => match creabim_core::marks::object_mark(model, e) {
            Some(m) => (m, true),
            None => (e.name.clone(), false),
        },
        ElementKind::Footing(f) => (
            match f.kind {
                creabim_core::FootingKind::Strip => {
                    format!("{} × {}", tag(f.width), tag(f.thickness))
                }
                creabim_core::FootingKind::Pad => format!(
                    "{} × {} × {}",
                    tag(f.width),
                    tag(f.length),
                    tag(f.thickness)
                ),
            },
            false,
        ),
        ElementKind::Room(_) => (room_tag_text(model, id, &[]), false),
        // a ceiling: its height above the floor, as reflected ceiling plans write it
        ElementKind::Ceiling(c) => (format!("CLG +{}", tag(c.height)), false),
        _ => (e.name.clone(), false),
    }
}

/// What a room's tag reads: each of `fields` on its own line (the room's
/// name, number and area when there are none), its name in capitals and
/// its number and area side by side; fields the room has no value for are
/// left out.
pub fn room_tag_text(model: &Model, id: ElementId, fields: &[RoomField]) -> String {
    let Some(e) = model.get(id) else {
        return String::new();
    };
    if !matches!(e.kind, ElementKind::Room(_)) {
        return e.name.clone();
    }
    let fields = if fields.is_empty() {
        &RoomField::DEFAULT[..]
    } else {
        fields
    };
    let mut lines: Vec<String> = Vec::new();
    let mut joined = false;
    for (i, f) in fields.iter().enumerate() {
        let value = f.value(model, id);
        if value.is_empty() {
            continue;
        }
        let value = if *f == RoomField::Name {
            value.to_uppercase()
        } else {
            value
        };
        // the area goes beside the number written just before it
        if *f == RoomField::Area && joined && i > 0 && fields[i - 1] == RoomField::Number {
            if let Some(last) = lines.last_mut() {
                last.push_str("  ");
                last.push_str(&value);
                continue;
            }
        }
        joined = *f == RoomField::Number;
        lines.push(value);
    }
    lines.join("\n")
}

/// A room's tag as drawing text: its lines stacked and centred on `at`.
pub fn room_tag_item(id: ElementId, text: String, at: DVec2, look: TextLook, mm: f64) -> PlanItem {
    // its lines centred by their real widths (MU29)
    let mut look = look;
    if look.align == creabim_core::TextAlign::Usual {
        look.align = creabim_core::TextAlign::Centre;
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let width = lines.iter().map(|l| look.width(l)).fold(0.0, f64::max) * mm;
    let step = look.size * 1.5 * mm;
    let start = at + DVec2::new(-width / 2.0, step * (lines.len() - 1) as f64 / 2.0);
    PlanItem::Annotation {
        id,
        item: Box::new(Annotation {
            shape: Shape::Note {
                at: start,
                leader: None,
                elbow: None,
            },
            text,
            look,
            mm,
        }),
    }
}

/// A room's tag laid out as `style` says, centred on `at`: each line its
/// own drawing text. Stacked writes the name in capitals and semibold, the
/// area below it and the number small and last; Inline is
/// [`room_tag_text`] in one block. A tag moved out of its room (`room` is
/// its outline in the drawing) gets a leader back to `centre`.
#[allow(clippy::too_many_arguments)]
pub fn room_tag_items(
    model: &Model,
    id: ElementId,
    room: ElementId,
    fields: &[RoomField],
    at: DVec2,
    centre: DVec2,
    outline: Option<&[DVec2]>,
    look: TextLook,
    mm: f64,
    style: creabim_core::styles::RoomTagStyle,
) -> Vec<PlanItem> {
    use creabim_core::styles::RoomTagStyle;
    let outside = outline.is_some_and(|o| o.len() > 2 && !creabim_core::polygon::contains(o, at));
    let mut out = match style {
        RoomTagStyle::Inline => {
            vec![room_tag_item(
                id,
                room_tag_text(model, room, fields),
                at,
                look,
                mm,
            )]
        }
        RoomTagStyle::Stacked => {
            let order: &[RoomField] = if fields.is_empty() {
                &[RoomField::Name, RoomField::Area, RoomField::Number]
            } else {
                fields
            };
            let lines: Vec<(String, TextLook)> = order
                .iter()
                .filter_map(|f| {
                    let v = f.value(model, room);
                    if v.is_empty() {
                        return None;
                    }
                    let mut l = look;
                    Some(match f {
                        RoomField::Name => {
                            l.bold = true;
                            (v.to_uppercase(), l)
                        }
                        RoomField::Number => {
                            l.size *= 0.72;
                            (v, l)
                        }
                        _ => {
                            l.size *= 0.9;
                            (v, l)
                        }
                    })
                })
                .collect();
            let steps: Vec<f64> = lines.iter().map(|(_, l)| l.size * 1.45 * mm).collect();
            let total: f64 = steps.iter().sum::<f64>() - steps.last().copied().unwrap_or(0.0);
            let mut y = at.y + total / 2.0;
            let mut items = vec![];
            // left or right aligned: every line against the block's edge
            let block = lines
                .iter()
                .map(|(t, l)| l.width(t) * mm)
                .fold(0.0, f64::max);
            for ((text, l), step) in lines.into_iter().zip(steps) {
                let w = l.width(&text) * mm;
                let x = match look.align {
                    creabim_core::TextAlign::Left => at.x - block / 2.0 + w / 2.0,
                    creabim_core::TextAlign::Right => at.x + block / 2.0 - w / 2.0,
                    _ => at.x,
                };
                items.push(room_tag_item(id, text, DVec2::new(x, y), l, mm));
                y -= step;
            }
            items
        }
        RoomTagStyle::NumberBadge | RoomTagStyle::Boxed => {
            let value = |f: RoomField| {
                let shown = fields.is_empty() || fields.contains(&f);
                Some(f.value(model, room)).filter(|v| shown && !v.is_empty())
            };
            let (name, number, area) = (
                value(RoomField::Name),
                value(RoomField::Number),
                value(RoomField::Area),
            );
            let mut bold = look;
            bold.bold = true;
            let mut small = look;
            small.size *= 0.72;
            let mut mid = look;
            mid.size *= 0.9;
            let pen = crate::plan::Pen::Symbol;
            let rect = |lo: DVec2, hi: DVec2| PlanItem::Polygon {
                id,
                pen,
                points: vec![lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)],
            };
            let mut items = vec![];
            if style == RoomTagStyle::NumberBadge {
                // the number in a small outline on top, the name, the area
                let step = look.size * 1.45 * mm;
                let rows = [number.is_some(), name.is_some(), area.is_some()]
                    .iter()
                    .filter(|r| **r)
                    .count();
                let mut y = at.y + step * (rows.saturating_sub(1)) as f64 / 2.0;
                if let Some(n) = number {
                    let w = small.width(&n) * mm + 1.6 * mm;
                    let h = small.size * 1.5 * mm;
                    let c = DVec2::new(at.x, y);
                    items.push(PlanItem::Polygon {
                        id,
                        pen,
                        points: (0..20)
                            .map(|i| {
                                // a stadium round the number
                                let t = i as f64 / 20.0 * std::f64::consts::TAU;
                                let (cx, cy) = (t.cos(), t.sin());
                                let half = (w - h).max(0.0) / 2.0;
                                c + DVec2::new(cx * h / 2.0 + half * cx.signum(), cy * h / 2.0)
                            })
                            .collect(),
                    });
                    items.push(room_tag_item(id, n, c, small, mm));
                    y -= step;
                }
                if let Some(n) = name {
                    items.push(room_tag_item(
                        id,
                        n.to_uppercase(),
                        DVec2::new(at.x, y),
                        bold,
                        mm,
                    ));
                    y -= step;
                }
                if let Some(a) = area {
                    items.push(room_tag_item(id, a, DVec2::new(at.x, y), mid, mm));
                }
            } else {
                // a box: the name, and a footer with the number and area
                let pad = 1.2 * mm;
                let top = name
                    .as_ref()
                    .map_or(0.0, |n| bold.width(&n.to_uppercase()) * mm);
                // text drawn on screen runs a little wider than the
                // estimate, so the footer leaves room and a divider
                let foot_gap = 2.0 * pad;
                let nw = number.as_ref().map_or(0.0, |n| small.width(n) * mm * 1.15);
                let aw = area.as_ref().map_or(0.0, |a| mid.width(a) * mm * 1.15);
                let foot = nw + aw + foot_gap;
                let w = top.max(foot) + 2.0 * pad;
                let h_top = look.size * 1.6 * mm;
                let h_foot = look.size * 1.4 * mm;
                let lo = DVec2::new(at.x - w / 2.0, at.y - (h_top + h_foot) / 2.0);
                let hi = DVec2::new(at.x + w / 2.0, at.y + (h_top + h_foot) / 2.0);
                let split = hi.y - h_top;
                items.push(rect(lo, hi));
                items.push(PlanItem::Line {
                    id,
                    pen,
                    a: DVec2::new(lo.x, split),
                    b: DVec2::new(hi.x, split),
                });
                if let Some(n) = name {
                    items.push(room_tag_item(
                        id,
                        n.to_uppercase(),
                        DVec2::new(at.x, split + h_top / 2.0),
                        bold,
                        mm,
                    ));
                }
                let fy = lo.y + h_foot / 2.0;
                if number.is_some() && area.is_some() {
                    let x = lo.x + 2.0 * pad + nw;
                    items.push(PlanItem::Line {
                        id,
                        pen,
                        a: DVec2::new(x, lo.y),
                        b: DVec2::new(x, split),
                    });
                }
                if let Some(n) = number {
                    items.push(room_tag_item(
                        id,
                        n,
                        DVec2::new(lo.x + pad + nw / 2.0, fy),
                        small,
                        mm,
                    ));
                }
                if let Some(a) = area {
                    items.push(room_tag_item(
                        id,
                        a,
                        DVec2::new(hi.x - pad - aw / 2.0, fy),
                        mid,
                        mm,
                    ));
                }
            }
            items
        }
    };
    if outside {
        // a leader from the tag back to its room
        let gap = look.size * mm;
        let d = centre - at;
        if d.length() > gap * 2.0 {
            let from = at + d.normalize() * gap * 1.5;
            out.push(PlanItem::Line {
                id,
                pen: crate::plan::Pen::Symbol,
                a: from,
                b: centre,
            });
            out.push(PlanItem::Polygon {
                id,
                pen: crate::plan::Pen::Symbol,
                points: (0..12)
                    .map(|i| {
                        let t = i as f64 / 12.0 * std::f64::consts::TAU;
                        centre + DVec2::new(t.cos(), t.sin()) * 0.6 * mm
                    })
                    .collect(),
            });
        }
    }
    out
}

/// A dimension's value as it is written in `units`.
pub fn dimension_text(units: &creabim_core::Units, a: DVec2, b: DVec2) -> String {
    units.dimension(a.distance(b))
}

/// A linear dimension from `a` to `b` measured along `axis`: where `b`
/// lands on the line through `a` that way, and the true points its
/// witness lines start from (none when aligned). None when the points
/// coincide along it, so there is nothing to measure.
pub fn squared(
    axis: creabim_core::DimAxis,
    a: DVec2,
    b: DVec2,
) -> Option<(DVec2, Option<[DVec2; 2]>)> {
    let u = axis.direction(a, b)?;
    if axis.is_aligned() {
        return (a.distance(b) > 1e-6).then_some((b, None));
    }
    let b2 = a + u * (b - a).dot(u);
    (a.distance(b2) > 1e-6).then_some((b2, Some([a, b])))
}

/// How an annotation with `style` and its own `color` looks.
pub fn look_of(model: &Model, style: Option<ElementId>, color: Option<[f32; 3]>) -> TextLook {
    creabim_core::text_style(model, style, color).into()
}

/// A dimension about a centre (angle, radius, diameter, arc length) as a
/// drawing item, if its points can be drawn in the view.
fn measured(
    model: &Model,
    view: &View,
    id: ElementId,
    d: &creabim_core::Dimension,
) -> Option<PlanItem> {
    let c = resolve(model, view, d.center.as_ref()?)?;
    let a = resolve(model, view, &d.a)?;
    let b = resolve(model, view, &d.b)?;
    if c.distance(a) < 1e-6 {
        return None;
    }
    let shape = match d.kind {
        DimensionKind::Linear | DimensionKind::Spot => return None,
        DimensionKind::Angular => {
            if c.distance(b) < 1e-6 {
                return None;
            }
            Shape::Angle {
                vertex: c,
                a,
                b,
                radius: d.offset,
            }
        }
        DimensionKind::Radius | DimensionKind::Diameter => Shape::Radius {
            center: c,
            at: a,
            extra: d.offset,
            diameter: d.kind == DimensionKind::Diameter,
        },
        DimensionKind::ArcLength => Shape::ArcLength {
            center: c,
            a,
            b,
            offset: d.offset,
        },
    };
    Some(PlanItem::Annotation {
        id,
        item: Box::new(Annotation {
            shape,
            text: measure(
                &creabim_core::units::number_format(
                    model,
                    creabim_core::NumberKind::Dimensions,
                    d.format,
                )
                .units(&model.units()),
                d.kind,
                c,
                a,
                b,
            ),
            look: look_of(model, d.style, d.color),
            mm: view.scale / 1000.0,
        }),
    })
}

/// The height a spot level reads at `at`: in a section or elevation the
/// point's own height; in a plan the top of the slab it was put on, or
/// else the plan's level (a level marker).
pub fn spot_height(
    model: &Model,
    view: &View,
    d: &creabim_core::Dimension,
    at: DVec2,
) -> Option<f64> {
    match view.kind {
        ViewKind::Section { .. } => Some(at.y),
        ViewKind::Plan { level, .. } => {
            let elevation = |l: ElementId| model.level(l).map(|l| l.elevation);
            match d.a.host.and_then(|h| model.get(h)).map(|e| &e.kind) {
                Some(ElementKind::Slab(s)) => Some(s.top(elevation(s.level)?)),
                _ => elevation(level),
            }
        }
        _ => None,
    }
}

/// A spot level as a drawing item, if it can be drawn in the view.
fn spot(
    model: &Model,
    view: &View,
    id: ElementId,
    d: &creabim_core::Dimension,
) -> Option<PlanItem> {
    let at = resolve(model, view, &d.a)?;
    let text_at = resolve(model, view, &d.b)?;
    let z = spot_height(model, view, d, at)?;
    let pattern = d
        .pattern
        .as_deref()
        .unwrap_or(creabim_core::annotation::SPOT_PATTERN);
    Some(PlanItem::Annotation {
        id,
        item: Box::new(Annotation {
            shape: Shape::Spot {
                at,
                text_at,
                plan: matches!(view.kind, ViewKind::Plan { .. }),
            },
            text: creabim_core::annotation::spot_text(model, pattern, z),
            look: look_of(model, d.style, d.color),
            mm: view.scale / 1000.0,
        }),
    })
}

/// A note as a drawing item; `leader` resolves its leader point.
pub fn note_item(
    model: &Model,
    id: ElementId,
    n: &Note,
    leader: Option<DVec2>,
    mm: f64,
) -> PlanItem {
    let mut look = look_of(model, n.style, n.color);
    look.leader_end = n.leader_end.or(look.leader_end);
    PlanItem::Annotation {
        id,
        item: Box::new(Annotation {
            shape: Shape::Note {
                at: n.at,
                leader,
                elbow: leader.and(n.elbow),
            },
            text: crate::annotation::lay_out_note(&n.text, &look, n.width, n.markdown),
            look,
            mm,
        }),
    }
}

/// The notes written straight on a sheet, in paper millimetres.
pub fn sheet_notes(model: &Model, sheet: ElementId) -> Vec<PlanItem> {
    model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Note(n) if n.view == sheet => {
                Some(note_item(model, e.id, n, n.leader.map(|p| p.local), 1.0))
            }
            _ => None,
        })
        .collect()
}

/// The dimensions, tags, notes and detail components placed in a view,
/// and the boxes of the detail views called out from it, as drawing items.
pub fn annotations(model: &Model, view_id: ElementId, view: &View) -> Vec<PlanItem> {
    let Some(project) = Projector::of(model, view) else {
        return vec![];
    };
    let mut out = Vec::new();
    for e in model.iter() {
        match &e.kind {
            ElementKind::Detail(d) if d.view == view_id => {
                let look = creabim_core::styles::active_pack(model).membranes;
                out.extend(crate::detail::detail_items(
                    e.id,
                    d,
                    view.scale / 1000.0,
                    look,
                    &creabim_core::detail::layers(model),
                ));
            }
            ElementKind::View(v) => {
                if let Some(c) = v.callout.filter(|c| c.parent == view_id) {
                    let mm = view.scale / 1000.0;
                    match creabim_core::styles::active_pack(model).marker_parts() {
                        Some(parts) => {
                            // a detail called out of a section is ringed
                            let round = matches!(view.kind, ViewKind::Section { .. });
                            out.extend(crate::markers::callout(
                                e.id,
                                &crate::markers::number_of(model, e.id),
                                &crate::markers::sheet_ref(model, e.id),
                                c.lo,
                                c.hi,
                                round,
                                &parts,
                                mm,
                            ));
                        }
                        None => {
                            out.extend(crate::detail::callout_items(e.id, &e.name, c.lo, c.hi, mm))
                        }
                    }
                }
            }
            ElementKind::Dimension(d) if d.view == view_id && d.kind == DimensionKind::Spot => {
                out.extend(spot(model, view, e.id, d));
            }
            ElementKind::Dimension(d) if d.view == view_id && !d.kind.is_linear() => {
                out.extend(measured(model, view, e.id, d));
            }
            ElementKind::Note(n) if n.view == view_id => {
                let leader = n.leader.and_then(|p| resolve(model, view, &p));
                out.push(note_item(model, e.id, n, leader, view.scale / 1000.0));
            }
            ElementKind::Dimension(d) if d.view == view_id => {
                let (Some(a), Some(b)) = (resolve(model, view, &d.a), resolve(model, view, &d.b))
                else {
                    continue;
                };
                let Some((b2, witness)) = squared(d.axis, a, b) else {
                    continue;
                };
                out.push(PlanItem::Dimension {
                    id: e.id,
                    a,
                    b: b2,
                    offset: d.offset,
                    text: creabim_core::write_number(
                        model,
                        creabim_core::NumberKind::Dimensions,
                        d.format,
                        a.distance(b2),
                    ),
                    look: look_of(model, d.style, d.color),
                    witness,
                    breaks: vec![],
                });
            }
            ElementKind::Tag(t) if t.view == view_id => {
                let Some(c) = centre(model, t.host) else {
                    continue;
                };
                let centre = project.project(c);
                // a room's tag is its name, number and area, without a bubble
                if matches!(
                    model.get(t.host).map(|h| &h.kind),
                    Some(ElementKind::Room(_))
                ) {
                    if !view.shows_room_tags() {
                        continue;
                    }
                    let outline = match model.get(t.host).map(|h| &h.kind) {
                        Some(ElementKind::Room(r)) => {
                            creabim_core::room::shape_of(model, r).map(|s| {
                                s.outline
                                    .iter()
                                    .map(|p| project.project(p.extend(c.z)))
                                    .collect::<Vec<_>>()
                            })
                        }
                        _ => None,
                    };
                    out.extend(room_tag_items(
                        model,
                        e.id,
                        t.host,
                        &t.fields,
                        centre + t.offset,
                        centre,
                        outline.as_deref(),
                        look_of(model, t.style, t.color),
                        view.scale / 1000.0,
                        creabim_core::styles::active_pack(model).room_tags,
                    ));
                    continue;
                }
                // a material tag reads the build-up (Annotate step 5)
                let (text, round) = if t.material {
                    (material_text(model, t.host), false)
                } else {
                    tag_text(model, t.host)
                };
                let mut look = look_of(model, t.style, t.color);
                look.leader_end = t.leader_end.or(look.leader_end);
                out.push(PlanItem::Tag {
                    id: e.id,
                    anchor: centre + t.end.unwrap_or(DVec2::ZERO),
                    at: centre + t.offset,
                    elbow: t.elbow.map(|p| centre + p),
                    text,
                    round,
                    look,
                });
            }
            _ => {}
        }
    }
    out
}

/// Where a dimension line `p` → `q` crosses the box `corners` (grown by
/// `pad`), as fractions of the way along it.
fn crossing(p: DVec2, q: DVec2, corners: &[DVec2], pad: f64) -> Option<[f64; 2]> {
    let lo = corners.iter().copied().reduce(DVec2::min)? - DVec2::splat(pad);
    let hi = corners.iter().copied().reduce(DVec2::max)? + DVec2::splat(pad);
    let d = q - p;
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (o, v, a, b) in [(p.x, d.x, lo.x, hi.x), (p.y, d.y, lo.y, hi.y)] {
        if v.abs() < 1e-12 {
            if o < a || o > b {
                return None;
            }
            continue;
        }
        let (mut s0, mut s1) = ((a - o) / v, (b - o) / v);
        if s0 > s1 {
            std::mem::swap(&mut s0, &mut s1);
        }
        t0 = t0.max(s0);
        t1 = t1.min(s1);
    }
    (t1 - t0 > 1e-9).then_some([t0, t1])
}

/// Give each dimension whose style breaks at tags the gaps where the
/// tags' bubbles and the notes' text sit across its line; `mm` is drawing
/// units per paper mm.
pub fn break_dimensions(items: &mut [PlanItem], mm: f64) {
    let mut boxes: Vec<Vec<DVec2>> = Vec::new();
    for it in items.iter() {
        match it {
            PlanItem::Tag {
                anchor,
                at,
                elbow,
                text,
                round,
                look,
                ..
            } => boxes.push(tag_geometry(*anchor, *at, *elbow, text, *round, look, mm).outline),
            PlanItem::Annotation { item, .. } if matches!(item.shape, Shape::Note { .. }) => {
                boxes.extend(item.text_boxes().iter().map(|b| b.to_vec()))
            }
            _ => {}
        }
    }
    boxes.retain(|b| !b.is_empty());
    for it in items.iter_mut() {
        let PlanItem::Dimension {
            a,
            b,
            offset,
            look,
            breaks,
            ..
        } = it
        else {
            continue;
        };
        breaks.clear();
        if !look.breaks {
            continue;
        }
        let n = (*b - *a).normalize_or_zero().perp() * *offset;
        let (p, q) = (*a + n, *b + n);
        let mut gaps: Vec<[f64; 2]> = boxes
            .iter()
            .filter_map(|c| crossing(p, q, c, 0.8 * mm))
            .collect();
        gaps.sort_by(|x, y| x[0].total_cmp(&y[0]));
        for g in gaps {
            match breaks.last_mut() {
                Some(last) if g[0] <= last[1] => last[1] = last[1].max(g[1]),
                _ => breaks.push(g),
            }
        }
    }
}

impl DimensionGeometry {
    /// Leave the stretches `breaks` (fractions of the way from `pa` to
    /// `pb`, the dimension line's measured ends) out of its line.
    pub fn break_line(&mut self, breaks: &[[f64; 2]]) {
        if breaks.is_empty() || self.lines.is_empty() {
            return;
        }
        let (s, e) = self.lines[0];
        let (pa, pb) = self.ends_at;
        let len = pa.distance(pb);
        if len < 1e-12 {
            return;
        }
        let u = (pb - pa) / len;
        let t = |p: DVec2| (p - pa).dot(u) / len;
        let at = |f: f64| pa + (pb - pa) * f;
        let mut pieces = vec![];
        let mut from = t(s);
        for br in breaks {
            if br[0] > from {
                pieces.push((at(from), at(br[0].min(t(e)))));
            }
            from = from.max(br[1]);
        }
        if t(e) > from {
            pieces.push((at(from), e));
        }
        pieces.retain(|(p, q)| p.distance(*q) > 1e-9);
        self.lines.splice(0..1, pieces);
    }
}

/// Linework of a dimension, in drawing units; `mm` is drawing units per
/// paper millimetre, so ticks and gaps are a fixed size on paper.
pub struct DimensionGeometry {
    pub lines: Vec<(DVec2, DVec2)>,
    /// The ends of the dimension line: ticks (drawn heavier than the
    /// line), arrows, dots, as its style says.
    pub ends: Ends,
    pub text_at: DVec2,
    /// Text angle in radians, kept readable (never upside down).
    pub angle: f64,
    /// Where the dimension line meets its witness lines.
    pub ends_at: (DVec2, DVec2),
}

/// A linear dimension's linework: its line `offset` to the left of `a` →
/// `b`, witness lines back to the measured points (`witness` when they are
/// not `a` and `b`, as for a horizontal or vertical dimension), and its ends.
pub fn dimension_geometry(
    a: DVec2,
    b: DVec2,
    offset: f64,
    witness: Option<[DVec2; 2]>,
    look: &TextLook,
    mm: f64,
) -> DimensionGeometry {
    let u = (b - a).normalize_or_zero();
    let n = u.perp();
    let off = n * offset;
    let (pa, pb) = (a + off, b + off);
    let end = look.end.unwrap_or(Terminator::Tick);
    let run = if runs_past(end) { 1.5 * mm } else { 0.0 };
    let mut lines = vec![(pa - u * run, pb + u * run)];
    let [wa, wb] = witness.unwrap_or([a, b]);
    // witness lines from just off the measured points to just past the line
    for (from, to) in [(wa, pa), (wb, pb)] {
        let reach = (to - from).dot(n);
        if reach.abs() > 2.0 * mm {
            let s = n * reach.signum();
            lines.push((from + s * 1.0 * mm, to + s * 1.5 * mm));
        }
    }
    let mut ends = end_marks(end, pa, -u, look.end_size, mm);
    ends.extend(end_marks(end, pb, u, look.end_size, mm));
    let mut angle = u.to_angle();
    let mut up = n;
    if angle > std::f64::consts::FRAC_PI_2 + 1e-6 || angle <= -std::f64::consts::FRAC_PI_2 + 1e-6 {
        angle += std::f64::consts::PI;
        up = -n;
    }
    if angle > std::f64::consts::PI {
        angle -= std::f64::consts::TAU;
    }
    DimensionGeometry {
        lines,
        ends,
        // 1.8 mm off the line for the default 2.5 mm text
        text_at: (pa + pb) / 2.0 + up * (0.55 + look.size / 2.0) * mm,
        angle,
        ends_at: (pa, pb),
    }
}

/// A tag's bubble and leader, in drawing units.
pub struct TagGeometry {
    pub outline: Vec<DVec2>,
    /// The outline is a closed shape, filled white under the text (else an
    /// underline, or nothing for text alone).
    pub closed: bool,
    /// The leader from the bubble's edge (through its elbow) to the host,
    /// as a polyline; empty when the tag sits on its host.
    pub leader: Vec<DVec2>,
    /// The mark at the leader's end: an arrow, a dot, or nothing.
    pub ends: Ends,
}

/// Text height of tags and dimensions, paper mm.
pub const TEXT: f64 = 2.5;

/// A tag's outline of `shape` about `at`, its text half `half` wide and high.
fn tag_outline(shape: TagShape, at: DVec2, half: DVec2) -> (Vec<DVec2>, bool) {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};
    let ring = |n: usize, r: DVec2, turn: f64| -> Vec<DVec2> {
        (0..n)
            .map(|i| {
                let t = turn + i as f64 / n as f64 * TAU;
                at + DVec2::new(t.cos() * r.x, t.sin() * r.y)
            })
            .collect()
    };
    // rounded corners: quarter circles of `r` inside a box `h`
    let rounded = |h: DVec2, r: f64| -> Vec<DVec2> {
        let r = r.min(h.x).min(h.y);
        let mut pts = vec![];
        for (k, c) in [
            DVec2::new(h.x - r, h.y - r),
            DVec2::new(-h.x + r, h.y - r),
            DVec2::new(-h.x + r, -h.y + r),
            DVec2::new(h.x - r, -h.y + r),
        ]
        .into_iter()
        .enumerate()
        {
            for i in 0..=6 {
                let t = k as f64 * FRAC_PI_2 + FRAC_PI_2 * i as f64 / 6.0;
                pts.push(at + c + DVec2::from_angle(t) * r);
            }
        }
        pts
    };
    let boxed = |h: DVec2| {
        vec![
            at - h,
            at + DVec2::new(h.x, -h.y),
            at + h,
            at + DVec2::new(-h.x, h.y),
        ]
    };
    match shape {
        TagShape::Auto | TagShape::Box => (boxed(half), true),
        TagShape::Circle => (ring(24, DVec2::splat(half.x.max(half.y)), 0.0), true),
        TagShape::RoundedBox => (rounded(half, half.y * 0.45), true),
        TagShape::Pill => (rounded(half + DVec2::X * half.y * 0.4, half.y), true),
        TagShape::Hexagon => {
            // flat top and bottom, points left and right
            let w = half.x + half.y * 0.6;
            (
                vec![
                    at + DVec2::new(w, 0.0),
                    at + DVec2::new(half.x, half.y),
                    at + DVec2::new(-half.x, half.y),
                    at + DVec2::new(-w, 0.0),
                    at + DVec2::new(-half.x, -half.y),
                    at + DVec2::new(half.x, -half.y),
                ],
                true,
            )
        }
        TagShape::Octagon => {
            let r = half.x.max(half.y) / (PI / 8.0).cos();
            (ring(8, DVec2::splat(r), PI / 8.0), true)
        }
        TagShape::Diamond => {
            let h = half * 1.45;
            (
                vec![
                    at + DVec2::new(h.x, 0.0),
                    at + DVec2::new(0.0, h.y),
                    at + DVec2::new(-h.x, 0.0),
                    at + DVec2::new(0.0, -h.y),
                ],
                true,
            )
        }
        TagShape::Underline => (
            vec![
                at + DVec2::new(-half.x, -half.y * 0.75),
                at + DVec2::new(half.x, -half.y * 0.75),
            ],
            false,
        ),
        TagShape::TextOnly => (vec![], false),
    }
}

/// A tag's outline and leader. `round` is the tag's usual shape (doors
/// and windows) when its style leaves the shape to the tag.
pub fn tag_geometry(
    anchor: DVec2,
    at: DVec2,
    elbow: Option<DVec2>,
    text: &str,
    round: bool,
    look: &TextLook,
    mm: f64,
) -> TagGeometry {
    // the default 2.5 mm text sits in a bubble 4.4 mm tall
    let k = look.size / TEXT;
    let half = DVec2::new((look.width(text) / 2.0 + 1.2 * k).max(2.2 * k), 2.2 * k) * mm;
    let shape = match (look.shape, round) {
        (TagShape::Auto, true) => TagShape::Circle,
        (s, _) => s,
    };
    let (outline, closed) = tag_outline(shape, at, half);
    // a leader back to the host, from the edge of the bubble (or of the
    // text's box when there is no bubble), towards its elbow if it has one
    let first = elbow.unwrap_or(anchor);
    let d = first - at;
    let edge = if closed {
        outline.clone()
    } else {
        tag_outline(TagShape::Box, at, half).0
    };
    let mut t = 1.0f64;
    for i in 0..edge.len() {
        let (p, q) = (edge[i], edge[(i + 1) % edge.len()]);
        let e = q - p;
        let den = d.perp_dot(e);
        if den.abs() < 1e-18 {
            continue;
        }
        let s = (p - at).perp_dot(e) / den;
        let v = (p - at).perp_dot(d) / den;
        if s > 0.0 && (-1e-9..=1.0 + 1e-9).contains(&v) {
            t = t.min(s);
        }
    }
    let mut leader = vec![];
    if t < 1.0 && d.length() * (1.0 - t) > 1.0 * mm {
        leader.push(at + d * t);
        leader.extend(elbow);
        leader.push(anchor);
    } else if elbow.is_some() {
        // the elbow is inside the bubble: run straight on from it
        leader = vec![first, anchor];
    }
    leader.dedup_by(|a, b| a.distance(*b) < 1e-9);
    let mut ends = Ends::default();
    if let (Some(end), [.., from, tip]) = (look.leader_end, leader.as_slice()) {
        ends = end_marks(end, *tip, *tip - *from, look.end_size, mm);
    }
    if leader.len() < 2 {
        leader.clear();
    }
    TagGeometry {
        outline,
        closed,
        leader,
        ends,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::section::view_drawing;
    use creabim_core::sample::sample_house;

    fn named(m: &Model, name: &str) -> ElementId {
        m.iter().find(|e| e.name == name).unwrap().id
    }

    #[test]
    fn a_material_tag_reads_the_layers_outside_to_inside() {
        // Annotate step 5: a layered wall reads one line a layer
        let m = creabim_core::starter::starter_house();
        let wall = m
            .iter()
            .find(|e| {
                matches!(e.kind, ElementKind::Wall(_))
                    && creabim_core::assembly_of(&m, e).is_some_and(|(_, a)| a.layers.len() > 1)
            })
            .expect("a layered wall");
        let (_, a) = creabim_core::assembly_of(&m, wall).unwrap();
        let text = material_text(&m, wall.id);
        assert_eq!(text.lines().count(), a.layers.len(), "{text}");
    }

    #[test]
    fn room_tags_show_the_fields_they_list() {
        let m = creabim_core::starter::starter_house();
        let study = named(&m, "Study");
        let text = |f: &[RoomField]| room_tag_text(&m, study, f);
        let area = RoomField::Area.value(&m, study);
        assert_eq!(text(&[]), format!("STUDY\n0.01  {area}"));
        assert_eq!(
            text(&[
                RoomField::Name,
                RoomField::Area,
                RoomField::FloorFinish,
                RoomField::Level
            ]),
            format!("STUDY\n{area}\nOak boards\nGround Floor")
        );
        // a field the room has nothing for is left out
        assert_eq!(text(&[RoomField::Department, RoomField::Number]), "0.01");
        // and the tag's own list is what its drawing writes
        let mut doc = creabim_core::Document::from_model(m.clone());
        let tag = m
            .iter()
            .find(|e| matches!(&e.kind, ElementKind::Tag(t) if t.host == study))
            .unwrap()
            .id;
        doc.edit(tag, "Fields", |e| {
            if let ElementKind::Tag(t) = &mut e.kind {
                t.fields = vec![RoomField::Name, RoomField::CeilingFinish];
            }
        })
        .unwrap();
        let ElementKind::Tag(t) = doc.model().get(tag).unwrap().kind.clone() else {
            unreachable!()
        };
        let ElementKind::View(view) = doc.model().get(t.view).unwrap().kind.clone() else {
            unreachable!()
        };
        let items = annotations(doc.model(), t.view, &view);
        let shown: Vec<String> = items
            .iter()
            .filter_map(|i| match i {
                PlanItem::Annotation { id, item } if *id == tag => Some(item.text.clone()),
                _ => None,
            })
            .collect();
        let ceiling = RoomField::CeilingFinish.value(doc.model(), study);
        let want: Vec<String> = ["STUDY".to_string(), ceiling]
            .into_iter()
            .filter(|l| !l.is_empty())
            .collect();
        assert_eq!(shown, want);
        // with no list, Stacked: the name in semibold, the area, the number small
        doc.edit(tag, "Fields", |e| {
            if let ElementKind::Tag(t) = &mut e.kind {
                t.fields = vec![];
            }
        })
        .unwrap();
        let items = annotations(doc.model(), t.view, &view);
        let lines: Vec<(String, f64, bool)> = items
            .iter()
            .filter_map(|i| match i {
                PlanItem::Annotation { id, item } if *id == tag => {
                    Some((item.text.clone(), item.look.size, item.look.bold))
                }
                _ => None,
            })
            .collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].0, "STUDY");
        assert!(lines[0].2);
        assert_eq!(lines[1].0, area);
        assert_eq!(lines[2].0, "0.01");
        assert!(lines[2].1 < lines[1].1 && lines[1].1 < lines[0].1);
    }

    #[test]
    fn dimensions_follow_their_walls() {
        let mut doc = creabim_core::Document::from_model(sample_house());
        let plan = named(doc.model(), "Ground Floor plan");
        let overall = |m: &Model| {
            view_drawing(m, plan)
                .unwrap()
                .items
                .into_iter()
                .filter_map(|i| match i {
                    PlanItem::Dimension { text, .. } => text.parse::<f64>().ok(),
                    _ => None,
                })
                .fold(0.0, f64::max)
        };
        assert_eq!(overall(doc.model()), 10300.0);
        // stretch the house: move the east walls out by a metre
        for name in ["GF Wall 1", "GF Wall 2", "GF Wall 3"] {
            let id = named(doc.model(), name);
            doc.edit(id, "Stretch", |e| {
                if let ElementKind::Wall(w) = &mut e.kind {
                    for p in [&mut w.start, &mut w.end] {
                        if p.x > 9.0 {
                            p.x += 1.0;
                        }
                    }
                }
            })
            .unwrap();
        }
        assert_eq!(overall(doc.model()), 11300.0);
    }

    #[test]
    fn dimension_points_follow_the_masses_they_sit_on() {
        use creabim_core::transform::transformed;
        use creabim_core::{EditMesh, Element, FreeMesh, Transaction, Xform};
        let mut model = sample_house();
        let level = model.levels().next().unwrap().0.id;
        let mass = Element::new(
            "Mass",
            ElementKind::FreeMesh(FreeMesh::new(
                Some(level),
                DVec3::new(20.0, 5.0, 0.0),
                EditMesh::cuboid(DVec3::new(2.0, 4.0, 3.0)),
            )),
        );
        let id = mass.id;
        Transaction::new("Add mass")
            .insert(mass)
            .apply(&mut model)
            .unwrap();
        let plan = View::plan(level);
        let section = View::section(DVec2::new(15.0, 0.0), DVec2::new(25.0, 0.0), 20.0, true);
        // a plan corner and a section corner (top of the near face)
        let corner = DVec2::new(21.0, 7.0);
        let top = DVec2::new(6.0, 3.0);
        let in_plan = attach(&model, &plan, Some(id), corner);
        let in_section = attach(&model, &section, Some(id), top);
        assert_eq!(in_plan.host, Some(id));
        assert!(in_plan.body.is_some() && in_section.body.is_some());
        assert!(resolve(&model, &plan, &in_plan).unwrap().distance(corner) < 1e-9);
        assert!(
            resolve(&model, &section, &in_section)
                .unwrap()
                .distance(top)
                < 1e-9
        );
        // move the mass, then turn it a quarter about a point
        let moves = [
            Xform::Move(DVec3::new(1.0, -2.0, 0.5)),
            Xform::Rotate {
                center: DVec2::new(22.0, 5.0),
                degrees: 90.0,
            },
        ];
        let mut e = model.get(id).unwrap().clone();
        for t in &moves {
            e = transformed(&model, &e, t).unwrap();
        }
        let before = model.get(id).unwrap().clone();
        Transaction::new("Move mass")
            .replace(before, e)
            .apply(&mut model)
            .unwrap();
        let expect = moves.iter().fold(corner, |p, t| t.point2(p));
        let now = resolve(&model, &plan, &in_plan).unwrap();
        assert!(now.distance(expect) < 1e-9, "{now} vs {expect}");
        // the section point rose with the mass
        let now = resolve(&model, &section, &in_section).unwrap();
        assert!((now.y - 3.5).abs() < 1e-9, "{now}");
    }

    #[test]
    fn tags_read_marks_and_follow_doors() {
        let mut doc = creabim_core::Document::from_model(sample_house());
        let plan = named(doc.model(), "Ground Floor plan");
        let door = named(doc.model(), "Front Door");
        let tag_at = |m: &Model| {
            view_drawing(m, plan)
                .unwrap()
                .items
                .into_iter()
                .find_map(|i| match i {
                    // its leader's end, which follows the door (the text
                    // may be nudged clear of other lines)
                    PlanItem::Tag { anchor, text, .. } if text == "D01" => Some(anchor),
                    _ => None,
                })
                .expect("front door is tagged D01")
        };
        let before = tag_at(doc.model());
        doc.edit(door, "Move door", |e| {
            if let ElementKind::Opening(o) = &mut e.kind {
                o.offset += 2.0;
            }
        })
        .unwrap();
        let after = tag_at(doc.model());
        assert!((after.x - before.x - 2.0).abs() < 1e-9);
    }

    #[test]
    fn dimension_text_reads_upright() {
        let g = dimension_geometry(
            DVec2::new(0.0, 5.0),
            DVec2::ZERO,
            1.0,
            None,
            &TextLook::default(),
            0.1,
        );
        assert!(g.angle.abs() <= std::f64::consts::FRAC_PI_2 + 1e-9);
    }

    #[test]
    fn a_horizontal_dimension_measures_square_across() {
        use creabim_core::{DimAxis, Dimension, Element, Transaction};
        let mut doc = creabim_core::Document::from_model(sample_house());
        let plan = named(doc.model(), "Ground Floor plan");
        let (a, b) = (DVec2::new(1.0, 1.0), DVec2::new(1.47, 0.4));
        let mut d = Dimension::linear(plan, HostPoint::free(a), HostPoint::free(b), 0.5);
        d.axis = DimAxis::Horizontal;
        let e = Element::new("Dimension", ElementKind::Dimension(d));
        let id = e.id;
        doc.commit(Transaction::new("Add").insert(e)).unwrap();
        let item = view_drawing(doc.model(), plan)
            .unwrap()
            .items
            .into_iter()
            .find(|i| i.id() == id)
            .unwrap();
        let PlanItem::Dimension {
            a: pa,
            b: pb,
            text,
            witness,
            offset,
            look,
            ..
        } = item
        else {
            panic!("a linear dimension")
        };
        // 470 across, not the 760 along the diagonal
        assert_eq!(text, "470");
        assert!((pb - DVec2::new(1.47, 1.0)).length() < 1e-9 && pa == a);
        assert_eq!(witness, Some([a, b]));
        // the line is level, and the witness from the lower point reaches it
        let g = dimension_geometry(pa, pb, offset, witness, &look, 0.01);
        let (p, q) = g.lines[0];
        assert!((p.y - q.y).abs() < 1e-9);
        assert!(g.lines.iter().any(|(s, t)| (s.x - 1.47).abs() < 1e-9
            && (t.x - 1.47).abs() < 1e-9
            && s.y < 0.45
            && t.y > 1.5));
        // points level with each other measure nothing vertically
        assert!(squared(DimAxis::Vertical, a, DVec2::new(3.0, 1.0)).is_none());
    }

    #[test]
    fn styles_change_dimension_ends_and_tag_shapes() {
        let mut look = TextLook::default();
        let tick = dimension_geometry(DVec2::ZERO, DVec2::X * 3.0, 1.0, None, &look, 0.01);
        assert_eq!(tick.ends.heavy.len(), 2, "two ticks by default");
        look.end = Some(Terminator::Dot);
        let dots = dimension_geometry(DVec2::ZERO, DVec2::X * 3.0, 1.0, None, &look, 0.01);
        assert!(dots.ends.heavy.is_empty() && dots.ends.fills.len() == 2);
        // a tick.s line runs on past it; a dot.s or an arrow.s stops on its point
        assert!(tick.lines[0].0.x < 0.0);
        assert_eq!(dots.lines[0].0.x, 0.0);
        look.end = Some(Terminator::ArrowOpen);
        let open = dimension_geometry(DVec2::ZERO, DVec2::X * 3.0, 1.0, None, &look, 0.01);
        assert_eq!(open.lines[0].0.x, 0.0);
        assert_eq!(open.ends.thin.len(), 2);
        // every end draws something but None
        for t in Terminator::ALL {
            let e = end_marks(t, DVec2::ZERO, DVec2::X, 2.5, 0.01);
            let n = e.thin.len() + e.heavy.len() + e.fills.len();
            assert_eq!(n == 0, t == Terminator::None, "{t:?}");
        }
        // tags: shapes, and the leader leaving each from its edge
        let anchor = DVec2::new(0.0, -1.0);
        for shape in TagShape::ALL {
            look.shape = shape;
            let g = tag_geometry(anchor, DVec2::ZERO, None, "D01", false, &look, 0.01);
            assert_eq!(
                g.closed,
                !matches!(shape, TagShape::Underline | TagShape::TextOnly)
            );
            let [from, to] = g.leader[..] else {
                panic!("a leader to the far anchor")
            };
            assert_eq!(to, anchor);
            assert!(from.y < 0.0 && from.y > -0.05, "{shape:?} {from}");
        }
        look.shape = TagShape::Auto;
        let round = tag_geometry(anchor, DVec2::ZERO, None, "D01", true, &look, 0.01);
        assert_eq!(round.outline.len(), 24, "openings stay round");
    }
}
