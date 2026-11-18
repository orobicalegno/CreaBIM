//! CreaBIM's own parameters as IFC property sets, both ways: an element
//! type's shared parameters (on IfcWallType, IfcDoorType...) and a curtain
//! wall's grid (on IfcCurtainWall). Other tools show them as ordinary
//! properties; CreaBIM reads them back into the same types and grids.

use crate::step::{real, string};
use creabim_core::curtain::{CurtainGrid, GridRule, PanelKind};
use creabim_core::screen::ScreenPattern;
use creabim_core::types::FootingType;
use creabim_core::types::{
    BeamType, ColumnType, ElementType, OpeningType, SlabType, TypeCategory, TypeParams, WallType,
};
use creabim_core::{Footing, FootingKind, LocationLine, OpeningFrame, Profile};
use std::collections::HashMap;

/// The property set a type's parameters go in.
pub(crate) const TYPE_PSET: &str = "CreaBIM_TypeParameters";
/// The property set a curtain wall's grid goes in.
pub(crate) const CURTAIN_PSET: &str = "CreaBIM_CurtainWall";
/// The property set a footing's own values go in.
pub(crate) const FOOTING_PSET: &str = "CreaBIM_Footing";
/// The property sets a room's and a ceiling's own values go in.
pub(crate) const ROOM_PSET: &str = "CreaBIM_Room";
pub(crate) const CEILING_PSET: &str = "CreaBIM_Ceiling";

/// One property value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Prop {
    Length(f64),
    Count(i64),
    Real(f64),
    Label(String),
    Bool(bool),
    /// Text of any length (a label is at most 255 characters).
    Text(String),
}

impl Prop {
    /// As a STEP typed value, for `IfcPropertySingleValue.NominalValue`.
    pub(crate) fn step(&self) -> String {
        match self {
            Prop::Length(v) => format!("IFCLENGTHMEASURE({})", real(*v)),
            Prop::Count(n) => format!("IFCINTEGER({n})"),
            Prop::Real(v) => format!("IFCREAL({})", real(*v)),
            Prop::Label(s) => format!("IFCLABEL({})", string(s)),
            Prop::Bool(b) => format!("IFCBOOLEAN({})", if *b { ".T." } else { ".F." }),
            Prop::Text(s) => format!("IFCTEXT({})", string(s)),
        }
    }
}

/// Values read back, by property name.
pub(crate) type Props = HashMap<String, (Option<f64>, Option<String>)>;

fn len(p: &Props, k: &str) -> Option<f64> {
    p.get(k).and_then(|v| v.0).filter(|v| v.is_finite())
}

fn label<'a>(p: &'a Props, k: &str) -> Option<&'a str> {
    p.get(k).and_then(|v| v.1.as_deref())
}

fn rule_props(out: &mut Vec<(&'static str, Prop)>, keys: [&'static str; 2], r: GridRule) {
    match r {
        GridRule::Spacing(s) => out.push((keys[0], Prop::Length(s))),
        GridRule::Count(n) => out.push((keys[1], Prop::Count(n as i64))),
    }
}

fn rule_from(p: &Props, keys: [&str; 2], fallback: GridRule) -> GridRule {
    if let Some(s) = len(p, keys[0]).filter(|s| *s >= 0.05) {
        GridRule::Spacing(s)
    } else if let Some(n) = len(p, keys[1]).filter(|n| *n >= 1.0) {
        GridRule::Count(n as u32)
    } else {
        fallback
    }
}

/// A curtain grid's properties.
pub(crate) fn curtain_props(g: &CurtainGrid) -> Vec<(&'static str, Prop)> {
    let mut out = vec![];
    rule_props(&mut out, ["VerticalSpacing", "VerticalCount"], g.vertical);
    rule_props(
        &mut out,
        ["HorizontalSpacing", "HorizontalCount"],
        g.horizontal,
    );
    out.push(("MullionWidth", Prop::Length(g.mullion_width)));
    out.push(("MullionDepth", Prop::Length(g.mullion_depth)));
    out.push(("PanelThickness", Prop::Length(g.panel_thickness)));
    out.push(("Panels", Prop::Label(g.panel.label().into())));
    if !g.swapped.is_empty() {
        let cells: Vec<String> = g.swapped.iter().map(|(a, b)| format!("{a}:{b}")).collect();
        out.push(("SwappedPanels", Prop::Label(cells.join(" "))));
    }
    out
}

/// A curtain grid read back from its properties; `None` when there are none.
pub(crate) fn curtain_from(p: &Props) -> Option<CurtainGrid> {
    let d = CurtainGrid::default();
    len(p, "MullionWidth")?;
    let g = CurtainGrid {
        vertical: rule_from(p, ["VerticalSpacing", "VerticalCount"], d.vertical),
        horizontal: rule_from(p, ["HorizontalSpacing", "HorizontalCount"], d.horizontal),
        mullion_width: len(p, "MullionWidth")
            .filter(|v| *v > 0.0)
            .unwrap_or(d.mullion_width),
        mullion_depth: len(p, "MullionDepth")
            .filter(|v| *v > 0.0)
            .unwrap_or(d.mullion_depth),
        panel_thickness: len(p, "PanelThickness")
            .filter(|v| *v > 0.0)
            .unwrap_or(d.panel_thickness),
        panel: match label(p, "Panels") {
            Some(s) if s.eq_ignore_ascii_case("solid") => PanelKind::Solid,
            _ => PanelKind::Glass,
        },
        swapped: label(p, "SwappedPanels")
            .map(|s| {
                s.split_whitespace()
                    .filter_map(|c| {
                        let (a, b) = c.split_once(':')?;
                        Some((a.parse().ok()?, b.parse().ok()?))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    };
    g.validate().ok().map(|_| g)
}

/// A screen wall type's unit and holes: the holes as one label of
/// `x0 y0 x1 y1` quadruples in metres, separated by `;`.
fn screen_props(s: &ScreenPattern) -> Vec<(&'static str, Prop)> {
    let holes: Vec<String> = s
        .holes
        .iter()
        .map(|[a, b]| format!("{} {} {} {}", a.x, a.y, b.x, b.y))
        .collect();
    vec![
        ("ScreenUnitLength", Prop::Length(s.unit.x)),
        ("ScreenUnitHeight", Prop::Length(s.unit.y)),
        ("ScreenFrame", Prop::Length(s.frame)),
        ("ScreenStagger", Prop::Count(s.stagger as i64)),
        ("ScreenHoles", Prop::Label(holes.join("; "))),
    ]
}

/// A screen pattern read back from its properties; `None` when there is
/// none or it does not make sense.
fn screen_from(p: &Props) -> Option<ScreenPattern> {
    let unit = glam::DVec2::new(len(p, "ScreenUnitLength")?, len(p, "ScreenUnitHeight")?);
    let holes = label(p, "ScreenHoles")?
        .split(';')
        .filter_map(|h| {
            let v: Vec<f64> = h
                .split_whitespace()
                .filter_map(|n| n.parse().ok())
                .collect();
            (v.len() == 4).then(|| [glam::DVec2::new(v[0], v[1]), glam::DVec2::new(v[2], v[3])])
        })
        .collect();
    let s = ScreenPattern {
        unit,
        holes,
        frame: len(p, "ScreenFrame").unwrap_or(0.0).max(0.0),
        stagger: len(p, "ScreenStagger").is_some_and(|v| v > 0.5),
    };
    s.validate().ok().map(|_| s)
}

fn location_label(l: LocationLine) -> &'static str {
    match l {
        LocationLine::Centre => "Centre",
        LocationLine::CoreCentre => "CoreCentre",
        LocationLine::ExteriorFace => "ExteriorFace",
        LocationLine::InteriorFace => "InteriorFace",
        LocationLine::CoreExterior => "CoreExterior",
        LocationLine::CoreInterior => "CoreInterior",
        LocationLine::Boundary(_) => "LayerBoundary",
    }
}

/// A type's shared parameters as properties.
pub(crate) fn type_props(t: &ElementType) -> Vec<(&'static str, Prop)> {
    let mut out = vec![];
    match &t.params {
        TypeParams::Wall(w) => {
            out.push(("Thickness", Prop::Length(w.thickness)));
            out.push((
                "LocationLine",
                Prop::Label(location_label(w.location).into()),
            ));
            if let Some(h) = w.height {
                out.push(("DefaultHeight", Prop::Length(h)));
            }
            if let Some(g) = &w.curtain {
                out.extend(curtain_props(g));
            }
            if let Some(s) = &w.screen {
                out.extend(screen_props(s));
            }
        }
        TypeParams::Door(o) | TypeParams::Window(o) => {
            out.push(("Width", Prop::Length(o.width)));
            out.push(("Height", Prop::Length(o.height)));
            out.push(("Sill", Prop::Length(o.sill)));
            out.push(("FrameWidth", Prop::Length(o.frame.width)));
            out.push(("FrameDepth", Prop::Length(o.frame.depth)));
            out.push(("PanesAcross", Prop::Count(o.frame.panes_across as i64)));
            out.push(("PanesUp", Prop::Count(o.frame.panes_up as i64)));
        }
        TypeParams::Slab(s) | TypeParams::Roof(s) => {
            out.push(("Thickness", Prop::Length(s.thickness)));
        }
        TypeParams::Column(c) => match c.profile {
            Profile::Rectangle { width, depth } => {
                out.push(("ProfileWidth", Prop::Length(width)));
                out.push(("ProfileDepth", Prop::Length(depth)));
            }
            Profile::Round { diameter } => out.push(("Diameter", Prop::Length(diameter))),
        },
        TypeParams::Beam(b) => {
            out.push(("Width", Prop::Length(b.width)));
            out.push(("Depth", Prop::Length(b.depth)));
        }
        TypeParams::Footing(f) => {
            out.push(("Kind", Prop::Label(kind_label(f.kind).into())));
            out.push(("Width", Prop::Length(f.width)));
            if f.kind == FootingKind::Pad {
                out.push(("Length", Prop::Length(f.length)));
            }
            out.push(("Thickness", Prop::Length(f.thickness)));
        }
        TypeParams::Object(o) => {
            let s = o.size();
            out.push(("Width", Prop::Length(s.x)));
            out.push(("Depth", Prop::Length(s.y)));
            out.push(("Height", Prop::Length(s.z)));
            out.push(("Category", Prop::Label(o.category.label().into())));
            out.push(("Mount", Prop::Label(o.mount.label().into())));
            if o.offset != 0.0 {
                out.push(("DefaultOffset", Prop::Length(o.offset)));
            }
            if let Some(k) = &o.catalogue {
                out.push(("CatalogueKey", Prop::Label(k.clone())));
            }
        }
    }
    out
}

/// The category of an IFC type entity (upper case, as in STEP).
pub(crate) fn category_of(class: &str) -> Option<(TypeCategory, bool)> {
    Some(match class {
        "IFCWALLTYPE" => (TypeCategory::Wall, false),
        "IFCCURTAINWALLTYPE" => (TypeCategory::Wall, true),
        "IFCDOORTYPE" | "IFCDOORSTYLE" => (TypeCategory::Door, false),
        "IFCWINDOWTYPE" | "IFCWINDOWSTYLE" => (TypeCategory::Window, false),
        "IFCSLABTYPE" => (TypeCategory::Slab, false),
        "IFCROOFTYPE" => (TypeCategory::Roof, false),
        "IFCCOLUMNTYPE" => (TypeCategory::Column, false),
        "IFCBEAMTYPE" => (TypeCategory::Beam, false),
        "IFCFOOTINGTYPE" => (TypeCategory::Footing, false),
        _ => return None,
    })
}

/// A type's parameters read back from its properties, with sensible
/// defaults for what a file leaves out.
pub(crate) fn type_from(category: TypeCategory, curtain: bool, p: &Props) -> TypeParams {
    let l = |k: &str, d: f64| len(p, k).filter(|v| *v > 0.0).unwrap_or(d);
    match category {
        TypeCategory::Wall => {
            let curtain = curtain_from(p).or_else(|| curtain.then(CurtainGrid::default));
            let location = label(p, "LocationLine")
                .and_then(|s| {
                    LocationLine::ALL
                        .into_iter()
                        .find(|l| location_label(*l).eq_ignore_ascii_case(s))
                })
                .unwrap_or_default();
            TypeParams::Wall(WallType {
                thickness: curtain
                    .as_ref()
                    .map_or(l("Thickness", 0.2), |c| c.mullion_depth),
                assembly: None,
                material: None,
                location,
                height: len(p, "DefaultHeight").filter(|h| *h > 0.0),
                curtain,
                screen: screen_from(p).map(Box::new),
            })
        }
        TypeCategory::Door | TypeCategory::Window => {
            let door = category == TypeCategory::Door;
            let d = OpeningFrame::default();
            let count = |k: &str| len(p, k).map_or(1, |v| (v.round() as u32).clamp(1, 12));
            let o = OpeningType {
                width: l("Width", if door { 0.9 } else { 1.2 }),
                height: l("Height", if door { 2.1 } else { 1.3 }),
                sill: len(p, "Sill")
                    .filter(|v| *v >= 0.0)
                    .unwrap_or(if door { 0.0 } else { 0.9 }),
                frame: OpeningFrame {
                    width: l("FrameWidth", d.width),
                    depth: l("FrameDepth", d.depth),
                    panes_across: count("PanesAcross"),
                    panes_up: count("PanesUp"),
                    ..Default::default()
                },
                material: None,
            };
            if door {
                TypeParams::Door(o)
            } else {
                TypeParams::Window(o)
            }
        }
        TypeCategory::Slab | TypeCategory::Roof => {
            let s = SlabType {
                thickness: l(
                    "Thickness",
                    if category == TypeCategory::Slab {
                        0.2
                    } else {
                        0.25
                    },
                ),
                assembly: None,
                material: None,
            };
            if category == TypeCategory::Slab {
                TypeParams::Slab(s)
            } else {
                TypeParams::Roof(s)
            }
        }
        TypeCategory::Column => TypeParams::Column(ColumnType {
            profile: match len(p, "Diameter").filter(|v| *v > 0.0) {
                Some(diameter) => Profile::Round { diameter },
                None => Profile::Rectangle {
                    width: l("ProfileWidth", 0.3),
                    depth: l("ProfileDepth", 0.3),
                },
            },
            material: None,
        }),
        TypeCategory::Beam => TypeParams::Beam(BeamType {
            width: l("Width", 0.2),
            depth: l("Depth", 0.4),
            material: None,
        }),
        TypeCategory::Footing => {
            let kind = kind_from(p);
            TypeParams::Footing(FootingType {
                kind,
                width: l("Width", 0.6),
                length: match kind {
                    FootingKind::Pad => l("Length", 1.2),
                    FootingKind::Strip => 0.0,
                },
                thickness: l("Thickness", 0.3),
                material: None,
            })
        }
        // the shape comes from the type's representation maps
        TypeCategory::Object => TypeParams::Object(creabim_core::ObjectType {
            category: label(p, "Category")
                .map(creabim_core::ObjectCategory::from_label)
                .unwrap_or_default(),
            ifc: Default::default(),
            mount: label(p, "Mount")
                .map(creabim_core::objects::Mount::from_label)
                .unwrap_or_default(),
            offset: len(p, "DefaultOffset").unwrap_or(0.0),
            parts: vec![],
            symbol: vec![],
            catalogue: label(p, "CatalogueKey").map(str::to_string),
        }),
    }
}

fn kind_label(k: FootingKind) -> &'static str {
    match k {
        FootingKind::Strip => "Strip",
        FootingKind::Pad => "Pad",
    }
}

fn kind_from(p: &Props) -> FootingKind {
    match label(p, "Kind") {
        Some(s) if s.eq_ignore_ascii_case("pad") => FootingKind::Pad,
        _ => FootingKind::Strip,
    }
}

/// A footing's own values, as built: its line or centre in plan, its
/// size, how far below its level (or its host's base) it sits, and the
/// GlobalId of the wall or column it is under.
pub(crate) fn footing_props(f: &Footing, host: Option<String>) -> Vec<(&'static str, Prop)> {
    let mut out = vec![
        ("Kind", Prop::Label(kind_label(f.kind).into())),
        ("Width", Prop::Length(f.width)),
        ("Thickness", Prop::Length(f.thickness)),
        ("Below", Prop::Length(f.below)),
        ("StartX", Prop::Length(f.start.x)),
        ("StartY", Prop::Length(f.start.y)),
    ];
    match f.kind {
        FootingKind::Strip => {
            out.push(("EndX", Prop::Length(f.end.x)));
            out.push(("EndY", Prop::Length(f.end.y)));
        }
        FootingKind::Pad => {
            out.push(("Length", Prop::Length(f.length)));
            out.push(("Rotation", Prop::Real(f.rotation)));
        }
    }
    if let Some(h) = host {
        out.push(("Host", Prop::Label(h)));
    }
    out
}

/// A footing read back from its properties, on `level` (none when the
/// properties aren't there), with its host's GlobalId.
pub(crate) fn footing_from(
    p: &Props,
    level: creabim_core::ElementId,
) -> Option<(Footing, Option<String>)> {
    let (width, thickness) = (len(p, "Width")?, len(p, "Thickness")?);
    let at = |x: &str, y: &str| Some(glam::DVec2::new(len(p, x)?, len(p, y)?));
    let start = at("StartX", "StartY")?;
    let mut f = match kind_from(p) {
        FootingKind::Strip => Footing::strip(level, start, at("EndX", "EndY")?),
        FootingKind::Pad => {
            let mut f = Footing::pad(level, start);
            f.length = len(p, "Length")?;
            f.rotation = p.get("Rotation").and_then(|v| v.0).unwrap_or(0.0);
            f
        }
    };
    f.width = width;
    f.thickness = thickness;
    f.below = p.get("Below").and_then(|v| v.0).unwrap_or(0.0);
    Some((f, label(p, "Host").map(str::to_string)))
}

/// A room's own values, beyond its shape: where its point is and whether
/// its outline was drawn, so CreaBIM finds it from the walls again.
pub(crate) fn room_props(r: &creabim_core::Room) -> Vec<(&'static str, Prop)> {
    let mut out = vec![
        ("PointX", Prop::Length(r.at.x)),
        ("PointY", Prop::Length(r.at.y)),
        ("Drawn", Prop::Count(r.is_drawn() as i64)),
    ];
    if let Some(h) = r.height {
        out.push(("Height", Prop::Length(h)));
    }
    if !r.department.is_empty() {
        out.push(("Department", Prop::Label(r.department.clone())));
    }
    out
}

/// A ceiling's own values: the room it follows and its tile grid.
pub(crate) fn ceiling_props(
    c: &creabim_core::Ceiling,
    room: Option<String>,
) -> Vec<(&'static str, Prop)> {
    let mut out = vec![("Height", Prop::Length(c.height))];
    if c.grid > 0.0 {
        out.push(("Grid", Prop::Length(c.grid)));
    }
    if let Some(r) = room {
        out.push(("Room", Prop::Label(r)));
    }
    out
}

/// A room's point, whether it was drawn, its own height and department,
/// read back.
pub(crate) fn room_from(p: &Props) -> (Option<glam::DVec2>, bool, Option<f64>, String) {
    let at = len(p, "PointX")
        .zip(len(p, "PointY"))
        .map(|(x, y)| glam::DVec2::new(x, y));
    let drawn = p.get("Drawn").and_then(|v| v.0).is_none_or(|v| v != 0.0);
    let height = len(p, "Height").filter(|h| *h > 0.0);
    let department = label(p, "Department").unwrap_or("").to_string();
    (at, drawn, height, department)
}

/// A ceiling's grid and the GlobalId of the room it follows, read back.
pub(crate) fn ceiling_from(p: &Props) -> (f64, Option<String>) {
    (
        len(p, "Grid").filter(|g| *g >= 0.1).unwrap_or(0.0),
        label(p, "Room").map(str::to_string),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(props: &[(&'static str, Prop)]) -> Props {
        props
            .iter()
            .map(|(k, v)| {
                let r = match v {
                    Prop::Length(x) => (Some(*x), None),
                    Prop::Count(n) => (Some(*n as f64), None),
                    Prop::Real(x) => (Some(*x), None),
                    Prop::Label(s) | Prop::Text(s) => (None, Some(s.clone())),
                    Prop::Bool(b) => (Some(*b as i64 as f64), None),
                };
                (k.to_string(), r)
            })
            .collect()
    }

    #[test]
    fn every_starter_type_reads_back_from_its_properties() {
        let model = creabim_core::Model::new();
        let (tx, _) = creabim_core::types::add_starters(&model);
        for op in tx.ops {
            let creabim_core::Op::Insert(e) = op else {
                continue;
            };
            let creabim_core::ElementKind::ElementType(t) = &e.kind else {
                continue;
            };
            let p = read(&type_props(t));
            let mut back = type_from(t.category(), t.is_curtain(), &p);
            // materials and assemblies travel as material associations
            let mut want = t.params.clone();
            for params in [&mut back, &mut want] {
                match params {
                    TypeParams::Wall(w) => w.assembly = None,
                    TypeParams::Slab(s) | TypeParams::Roof(s) => s.assembly = None,
                    _ => {}
                }
            }
            assert_eq!(back, want, "{}", e.name);
        }
    }
}
