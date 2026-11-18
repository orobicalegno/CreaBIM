//! Schedules: tables of doors, windows, walls or slabs read live from the
//! model, and counts of furniture and fixtures. The same table is drawn on
//! sheets, edited in the app and exported as CSV.

use crate::paper::Anchor;
use crate::plan::{Pen, PlanItem, PlanView};
use creabim_core::{Element, ElementId, ElementKind, Model, OpeningKind, ScheduleCategory};
use glam::DVec2;

/// An element property a schedule cell shows and can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    Width,
    Height,
    Sill,
    Thickness,
    WallHeight,
    Material,
    /// A material's take-off rate (a plain number, not a length).
    Rate,
    /// What a material's rate is priced per.
    RatePer,
}

impl Field {
    /// Lengths, shown and edited in the project's units.
    pub fn is_length(self) -> bool {
        matches!(
            self,
            Field::Width | Field::Height | Field::Sill | Field::Thickness | Field::WallHeight
        )
    }
}

#[derive(Clone, Debug)]
pub struct Column {
    pub title: &'static str,
    /// The editable property behind this column, if any.
    pub field: Option<Field>,
    /// Width on paper, mm.
    pub width: f64,
    pub numeric: bool,
}

#[derive(Clone, Debug)]
pub struct Row {
    /// The element listed, or for a group heading the type grouped.
    pub id: ElementId,
    pub cells: Vec<String>,
    /// A heading over the rows of one type, when grouped by type.
    pub group: bool,
}

#[derive(Clone, Debug)]
pub struct Schedule {
    pub category: ScheduleCategory,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    /// The last row: counts and sums.
    pub totals: Vec<String>,
    /// How many elements it lists (for furniture and fixtures, how many
    /// objects its counts add up to).
    pub listed: usize,
}

fn col(title: &'static str, field: Option<Field>, width: f64, numeric: bool) -> Column {
    Column {
        title,
        field,
        width,
        numeric,
    }
}

/// The current value of a numeric field, metres.
pub fn value(e: &Element, field: Field) -> Option<f64> {
    Some(match (&e.kind, field) {
        (ElementKind::Opening(o), Field::Width) => o.width,
        (ElementKind::Opening(o), Field::Height) => o.height,
        (ElementKind::Opening(o), Field::Sill) => o.sill,
        (ElementKind::Wall(w), Field::Thickness) => w.thickness,
        (ElementKind::Wall(w), Field::WallHeight) => w.height,
        (ElementKind::Slab(s), Field::Thickness) => s.thickness,
        _ => return None,
    })
}

/// Change a numeric field, metres.
pub fn set_value(e: &mut Element, field: Field, v: f64) {
    match (&mut e.kind, field) {
        (ElementKind::Opening(o), Field::Width) => o.width = v,
        (ElementKind::Opening(o), Field::Height) => o.height = v,
        (ElementKind::Opening(o), Field::Sill) => o.sill = v,
        (ElementKind::Wall(w), Field::Thickness) => w.thickness = v,
        (ElementKind::Wall(w), Field::WallHeight) => w.height = v,
        (ElementKind::Slab(s), Field::Thickness) => s.thickness = v,
        _ => {}
    }
}

/// Change what an element is made of.
pub fn set_material(e: &mut Element, material: Option<ElementId>) {
    match &mut e.kind {
        ElementKind::Wall(w) => w.material = material,
        ElementKind::Slab(s) => s.material = material,
        ElementKind::FreeMesh(m) => m.material = material,
        k => {
            if let Some(m) = creabim_core::members::material_mut(k) {
                *m = material;
            }
        }
    }
}

/// The schedule of one category, rows in mark or model order.
pub fn schedule(model: &Model, category: ScheduleCategory) -> Schedule {
    schedule_by(model, category, false)
}

/// The schedule of one category; `by_type` groups the rows by element
/// type, each group under a heading row naming the type and its count.
pub fn schedule_by(model: &Model, category: ScheduleCategory, by_type: bool) -> Schedule {
    if category == ScheduleCategory::Materials {
        return takeoff_schedule(model, by_type);
    }
    let units = model.units();
    let mm =
        |v: f64| creabim_core::write_number(model, creabim_core::NumberKind::Schedules, None, v);
    let area = |v: f64| units.area(v);
    let name = |id: ElementId| model.get(id).map(|e| e.name.clone()).unwrap_or_default();
    let material = |e: &Element| {
        // A layered element is listed by its core layer's material.
        let core = creabim_core::assembly_of(model, e)
            .and_then(|(_, a)| a.layers.get(a.core().0)?.material)
            .and_then(|m| model.get(m));
        if let Some(m) = core {
            return m.name.clone();
        }
        creabim_core::material_of(model, e)
            .map(|(m, _)| m.name.clone())
            .unwrap_or_else(|| "-".into())
    };
    let level_of = |e: &Element| match &e.kind {
        ElementKind::Wall(w) => Some(w.level),
        ElementKind::Slab(s) => Some(s.level),
        ElementKind::Opening(o) => match model.get(o.host).map(|h| &h.kind) {
            Some(ElementKind::Wall(w)) => Some(w.level),
            _ => None,
        },
        k => creabim_core::members::level_of(k),
    };
    let elevation = |e: &Element| {
        level_of(e)
            .and_then(|l| model.level(l))
            .map_or(0.0, |l| l.elevation)
    };
    let mut rows: Vec<(f64, String, Row)> = Vec::new();
    let mut listed = None;
    let (columns, totals) = match category {
        ScheduleCategory::Furniture | ScheduleCategory::Fixtures => {
            let (columns, totals, n) = object_rows(model, category, by_type, &mut rows);
            listed = Some(n);
            (columns, totals)
        }
        ScheduleCategory::Doors | ScheduleCategory::Windows => {
            let kind = if category == ScheduleCategory::Doors {
                OpeningKind::Door
            } else {
                OpeningKind::Window
            };
            let marks = creabim_core::marks(model);
            let mut columns = vec![
                col("Mark", None, 16.0, false),
                col("Level", None, 28.0, false),
                col("Wall", None, 26.0, false),
                col("Width", Some(Field::Width), 18.0, true),
                col("Height", Some(Field::Height), 18.0, true),
                col("Sill", Some(Field::Sill), 16.0, true),
            ];
            let doors = kind == OpeningKind::Door;
            if doors {
                // hinge side seen from where it opens to, into or out of the building
                columns.push(col("Swing", None, 22.0, false));
            }
            for e in model.iter() {
                let ElementKind::Opening(o) = &e.kind else {
                    continue;
                };
                if o.kind != kind {
                    continue;
                }
                let mark = marks.get(&e.id).cloned().unwrap_or_default();
                let mut cells = vec![
                    mark.clone(),
                    level_of(e).map(name).unwrap_or_default(),
                    name(o.host),
                    mm(o.width),
                    mm(o.height),
                    mm(o.sill),
                ];
                if doors {
                    cells.push(match model.get(o.host).map(|h| &h.kind) {
                        Some(ElementKind::Wall(w)) => crate::opening::door_hand(o, w).into(),
                        _ => String::new(),
                    });
                }
                rows.push((
                    0.0,
                    mark,
                    Row {
                        id: e.id,
                        cells,
                        group: false,
                    },
                ));
            }
            let what = if kind == OpeningKind::Door {
                "doors"
            } else {
                "windows"
            };
            (columns, vec![format!("{} {what}", rows.len())])
        }
        ScheduleCategory::Walls => {
            // The assembly column shows up once any wall has layers, so
            // single-material projects keep their old table.
            let layered = model
                .iter()
                .any(|e| matches!(&e.kind, ElementKind::Wall(w) if w.assembly.is_some()));
            let mut columns = vec![
                col("Name", Some(Field::Name), 30.0, false),
                col("Level", None, 28.0, false),
                col("Material", Some(Field::Material), 30.0, false),
                col("Length", None, 18.0, true),
                col("Thickness", Some(Field::Thickness), 20.0, true),
                col("Height", Some(Field::WallHeight), 18.0, true),
                col("Area", None, 22.0, true),
            ];
            if layered {
                columns.insert(3, col("Assembly", None, 44.0, false));
            }
            let mut total_area = 0.0;
            let mut total_length = 0.0;
            for e in model.iter() {
                let ElementKind::Wall(w) = &e.kind else {
                    continue;
                };
                let openings: f64 = model
                    .dependents(e.id)
                    .into_iter()
                    .filter_map(|d| match &model.get(d)?.kind {
                        ElementKind::Opening(o) => Some(o.width * o.height),
                        _ => None,
                    })
                    .sum();
                let a = (w.length() * w.height - openings).max(0.0);
                total_area += a;
                total_length += w.length();
                let mut cells = vec![
                    e.name.clone(),
                    name(w.level),
                    material(e),
                    mm(w.length()),
                    mm(w.thickness),
                    mm(w.height),
                    area(a),
                ];
                if layered {
                    let asm = w
                        .assembly
                        .and_then(|id| model.get(id))
                        .map_or_else(|| "-".to_string(), |a| a.name.clone());
                    cells.insert(3, asm);
                }
                rows.push((
                    elevation(e),
                    String::new(),
                    Row {
                        id: e.id,
                        cells,
                        group: false,
                    },
                ));
            }
            let mut totals = vec![
                format!("{} walls", rows.len()),
                String::new(),
                String::new(),
                mm(total_length),
                String::new(),
                String::new(),
                area(total_area),
            ];
            if layered {
                totals.insert(3, String::new());
            }
            (columns, totals)
        }
        ScheduleCategory::Slabs => {
            let columns = vec![
                col("Name", Some(Field::Name), 30.0, false),
                col("Level", None, 28.0, false),
                col("Material", Some(Field::Material), 30.0, false),
                col("Thickness", Some(Field::Thickness), 20.0, true),
                col("Area", None, 22.0, true),
            ];
            let mut total = 0.0;
            for e in model.iter() {
                let ElementKind::Slab(s) = &e.kind else {
                    continue;
                };
                let a = s.area();
                total += a;
                rows.push((
                    elevation(e),
                    String::new(),
                    Row {
                        id: e.id,
                        group: false,
                        cells: vec![
                            e.name.clone(),
                            name(s.level),
                            material(e),
                            mm(s.thickness),
                            area(a),
                        ],
                    },
                ));
            }
            (
                columns,
                vec![
                    format!("{} slabs", rows.len()),
                    String::new(),
                    String::new(),
                    String::new(),
                    area(total),
                ],
            )
        }
        ScheduleCategory::Rooms => {
            let columns = vec![
                col("Number", None, 16.0, false),
                col("Name", Some(Field::Name), 34.0, false),
                col("Level", None, 28.0, false),
                col("Area", None, 20.0, true),
                col("Perimeter", None, 20.0, true),
                col("Height", None, 18.0, true),
                col("Floor finish", None, 34.0, false),
                col("Occupancy", None, 26.0, false),
            ];
            let mut total = 0.0;
            for e in model.iter() {
                let ElementKind::Room(r) = &e.kind else {
                    continue;
                };
                let shape = creabim_core::room::shape(model, e.id);
                let (a, p) = shape.as_ref().map_or((0.0, 0.0), |s| {
                    (creabim_core::room::net_area(model, r, s), s.perimeter())
                });
                total += a;
                let dash = |s: &str| {
                    if s.is_empty() {
                        "-".to_string()
                    } else {
                        s.to_string()
                    }
                };
                rows.push((
                    model.level(r.level).map_or(0.0, |l| l.elevation),
                    r.number.clone(),
                    Row {
                        id: e.id,
                        group: false,
                        cells: vec![
                            r.number.clone(),
                            e.name.clone(),
                            name(r.level),
                            if shape.is_some() {
                                area(a)
                            } else {
                                "Not enclosed".into()
                            },
                            mm(p),
                            mm(creabim_core::room::height(model, r)),
                            dash(&r.floor_finish),
                            dash(&r.occupancy),
                        ],
                    },
                ));
            }
            (
                columns,
                vec![
                    format!("{} rooms", rows.len()),
                    String::new(),
                    String::new(),
                    area(total),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ],
            )
        }
        ScheduleCategory::Structure => {
            let columns = vec![
                col("Name", Some(Field::Name), 30.0, false),
                col("Member", None, 16.0, false),
                col("Level", None, 28.0, false),
                col("Material", Some(Field::Material), 30.0, false),
                col("Section", None, 22.0, false),
                col("Length", None, 18.0, true),
            ];
            let mut total = 0.0;
            for e in model.iter() {
                let (kind, section, length) = match &e.kind {
                    ElementKind::Column(c) => {
                        ("Column", c.profile.label_in(&units), c.height_in(model))
                    }
                    ElementKind::Beam(b) => (
                        "Beam",
                        format!("{} × {}", mm(b.width), mm(b.depth)),
                        b.length(),
                    ),
                    ElementKind::Footing(f) => match f.kind {
                        creabim_core::FootingKind::Strip => (
                            "Strip footing",
                            format!("{} × {}", mm(f.width), mm(f.thickness)),
                            f.strip_length(model),
                        ),
                        // a pad's length is its longer side
                        creabim_core::FootingKind::Pad => (
                            "Pad footing",
                            format!("{} × {} × {}", mm(f.width), mm(f.length), mm(f.thickness)),
                            f.width.max(f.length),
                        ),
                    },
                    _ => continue,
                };
                total += length;
                rows.push((
                    elevation(e),
                    kind.to_string(),
                    Row {
                        id: e.id,
                        group: false,
                        cells: vec![
                            e.name.clone(),
                            kind.into(),
                            level_of(e).map(name).unwrap_or_default(),
                            material(e),
                            section,
                            mm(length),
                        ],
                    },
                ));
            }
            (
                columns,
                vec![
                    format!("{} members", rows.len()),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    mm(total),
                ],
            )
        }
        // answered above, before the element schedules
        ScheduleCategory::Materials => unreachable!("the take-off is built on its own"),
    };
    // marks sort as text (D01 < D02); walls and slabs by level, then model order
    rows.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut columns = columns;
    let mut totals = totals;
    // walls, slabs and structure get a mark column once one of them has a
    // mark, typed or numbered by the project's rules
    if matches!(
        category,
        ScheduleCategory::Walls | ScheduleCategory::Slabs | ScheduleCategory::Structure
    ) {
        let marks = creabim_core::marks(model);
        if rows.iter().any(|(_, _, r)| marks.contains_key(&r.id)) {
            columns.insert(1, col("Mark", None, 16.0, false));
            for (_, _, r) in &mut rows {
                r.cells
                    .insert(1, marks.get(&r.id).cloned().unwrap_or_default());
            }
            if totals.len() > 1 {
                totals.insert(1, String::new());
            }
        }
    }
    // the type column shows up once anything listed has a type
    let type_of = |id: ElementId| {
        model
            .get(id)
            .and_then(|e| creabim_core::types::type_of(model, e))
            .map(|(t, _)| (t.id, t.name.clone()))
    };
    let typed = rows.iter().any(|(_, _, r)| type_of(r.id).is_some());
    if typed {
        columns.insert(1, col("Type", None, 44.0, false));
        for (_, _, r) in &mut rows {
            let t = type_of(r.id).map_or_else(|| "-".to_string(), |(_, n)| n);
            r.cells.insert(1, t);
        }
        if totals.len() > 1 {
            totals.insert(1, String::new());
        }
    }
    let mut rows: Vec<Row> = rows.into_iter().map(|(_, _, r)| r).collect();
    if by_type {
        // typed groups in name order, untyped last, rows keeping their order
        let key = |r: &Row| match type_of(r.id) {
            Some((_, n)) => (0, n),
            None => (1, String::new()),
        };
        rows.sort_by_key(key);
        let id_of = |r: &Row| type_of(r.id).map(|(id, _)| id);
        let mut grouped = Vec::with_capacity(rows.len() * 2);
        for group in rows.chunk_by(|a, b| id_of(a) == id_of(b)) {
            let t = type_of(group[0].id);
            let mut cells = vec![String::new(); columns.len()];
            cells[0] = match &t {
                Some((_, name)) => format!("{name} ({})", group.len()),
                None => format!("No type ({})", group.len()),
            };
            grouped.push(Row {
                id: t.map_or(ElementId::NIL, |(id, _)| id),
                cells,
                group: true,
            });
            grouped.extend(group.iter().cloned());
        }
        rows = grouped;
    }
    if category == ScheduleCategory::Rooms {
        rows = with_level_totals(model, rows);
    }
    // feet and inches take more room than millimetres
    if units.is_imperial() {
        for c in columns.iter_mut().filter(|c| c.numeric) {
            c.width *= 1.4;
        }
    }
    let listed = listed.unwrap_or_else(|| rows.iter().filter(|r| !r.group).count());
    Schedule {
        category,
        columns,
        rows,
        totals,
        listed,
    }
}

/// The take-off as a schedule: a row for each material with what it adds
/// up to and what it costs at its rate. `by_level` puts each level's
/// materials under a heading with the level's totals. Quantities are in
/// metres, square metres and cubic metres, the units rates are priced in.
fn takeoff_schedule(model: &Model, by_level: bool) -> Schedule {
    use creabim_core::takeoff;
    let columns = vec![
        col("Material", None, 40.0, false),
        col("Elements", None, 16.0, true),
        col("Area", None, 22.0, true),
        col("Volume", None, 22.0, true),
        col("Length", None, 20.0, true),
        col("Rate", Some(Field::Rate), 18.0, true),
        col("Per", Some(Field::RatePer), 16.0, false),
        col("Cost", None, 22.0, true),
    ];
    let name = |id: Option<ElementId>, none: &str| {
        id.and_then(|id| model.get(id))
            .map_or_else(|| none.to_string(), |e| e.name.clone())
    };
    let money = |v: Option<f64>| v.map_or_else(|| "-".to_string(), |v| format!("{v:.2}"));
    let cells = |label: String, t: &takeoff::Total| {
        vec![
            label,
            t.elements.to_string(),
            format!("{:.2} m²", t.quantity.area),
            format!("{:.2} m³", t.quantity.volume),
            format!("{:.2} m", t.quantity.length),
            t.rate
                .map_or_else(|| "-".to_string(), |r| format!("{:.2}", r.amount)),
            t.rate
                .map_or_else(|| "-".to_string(), |r| r.per.label().to_string()),
            money(t.cost),
        ]
    };
    let lines = takeoff::lines(model);
    let materials = takeoff::by_material(model, &lines);
    let mut rows = Vec::new();
    let material_rows = |lines: &[takeoff::Line], rows: &mut Vec<Row>| {
        for t in takeoff::by_material(model, lines) {
            rows.push(Row {
                id: t.key.unwrap_or(ElementId::NIL),
                cells: cells(name(t.key, "No material"), &t),
                group: false,
            });
        }
    };
    if by_level {
        for level in takeoff::by_level(model, &lines) {
            let mine: Vec<takeoff::Line> = lines
                .iter()
                .filter(|l| l.level == level.key)
                .cloned()
                .collect();
            let mut head = cells(name(level.key, "No level"), &level);
            // a level has no rate of its own
            head[5] = String::new();
            head[6] = String::new();
            rows.push(Row {
                id: level.key.unwrap_or(ElementId::NIL),
                cells: head,
                group: true,
            });
            material_rows(&mine, &mut rows);
        }
    } else {
        material_rows(&lines, &mut rows);
    }
    let sum =
        |f: fn(&takeoff::Quantity) -> f64| materials.iter().map(|t| f(&t.quantity)).sum::<f64>();
    let priced = materials.iter().any(|t| t.cost.is_some());
    let totals = vec![
        format!("{} materials", materials.len()),
        String::new(),
        format!("{:.2} m²", sum(|q| q.area)),
        format!("{:.2} m³", sum(|q| q.volume)),
        format!("{:.2} m", sum(|q| q.length)),
        String::new(),
        String::new(),
        if priced {
            money(Some(takeoff::total_cost(model, &lines)))
        } else {
            "-".to_string()
        },
    ];
    Schedule {
        category: ScheduleCategory::Materials,
        columns,
        rows,
        totals,
        listed: materials.len(),
    }
}

/// Rooms under a heading for each level, which gives the level's room
/// count and its rooms' area added up (in the area column).
fn with_level_totals(model: &Model, rows: Vec<Row>) -> Vec<Row> {
    let level = |r: &Row| match model.get(r.id).map(|e| &e.kind) {
        Some(ElementKind::Room(room)) => room.level,
        _ => ElementId::NIL,
    };
    let mut out = Vec::with_capacity(rows.len() + 4);
    for group in rows.chunk_by(|a, b| level(a) == level(b)) {
        let l = level(&group[0]);
        let total: f64 = group
            .iter()
            .filter_map(|r| match model.get(r.id).map(|e| &e.kind) {
                Some(ElementKind::Room(room)) => creabim_core::room::shape(model, r.id)
                    .map(|s| creabim_core::room::net_area(model, room, &s)),
                _ => None,
            })
            .sum();
        let mut cells = vec![String::new(); group[0].cells.len()];
        let level_name = model.get(l).map_or("No level".into(), |e| e.name.clone());
        let n = group.len();
        cells[0] = format!(
            "{level_name} ({n} {})",
            if n == 1 { "room" } else { "rooms" }
        );
        if let Some(c) = cells.get_mut(3) {
            *c = model.units().area(total);
        }
        out.push(Row {
            id: l,
            cells,
            group: true,
        });
        out.extend(group.iter().cloned());
    }
    out
}

/// Is a placed object listed in the furniture schedule, rather than the
/// fixtures one? Those IFC calls furniture are.
pub fn is_furniture(t: &creabim_core::ObjectType) -> bool {
    matches!(
        t.ifc.normalized().class.as_str(),
        "IfcFurniture" | "IfcSystemFurnitureElement"
    )
}

/// How high an object is mounted, as a schedule reads it.
fn mounting(units: &creabim_core::Units, hung: bool, offset: f64) -> String {
    match (hung, offset.abs() < 1e-6) {
        (true, true) => "At the ceiling".into(),
        (true, false) => format!("{} below ceiling", units.dimension(offset)),
        (false, true) => "On the floor".into(),
        (false, false) => format!("{} above floor", units.dimension(offset)),
    }
}

/// The rows of a furniture or fixture schedule: with `by_type` one row an
/// object (the caller groups them by type), else one row for each type on
/// each level with how many there are, under a heading for each category.
/// Returns the columns, the totals and how many objects are listed.
fn object_rows(
    model: &Model,
    category: ScheduleCategory,
    by_type: bool,
    rows: &mut Vec<(f64, String, Row)>,
) -> (Vec<Column>, Vec<String>, usize) {
    use creabim_core::objects::{self, ObjectCategory};
    let units = model.units();
    let marks = creabim_core::marks::object_marks(model);
    let level_name = |l: ElementId| model.get(l).map(|e| e.name.clone()).unwrap_or_default();
    let elevation = |l: ElementId| model.level(l).map_or(0.0, |l| l.elevation);
    let furniture = category == ScheduleCategory::Furniture;
    // (category, type, its element, the object, its element)
    let mut found = vec![];
    for e in model.iter() {
        let ElementKind::Object(o) = &e.kind else {
            continue;
        };
        let Some(r) = &e.of_type else { continue };
        let Some(t) = objects::type_of_id(model, r.id) else {
            continue;
        };
        if is_furniture(t) != furniture {
            continue;
        }
        found.push((t, r.id, o, e));
    }
    let ifc = |t: &creabim_core::ObjectType| {
        let k = t.ifc.normalized();
        format!("{} {}", k.class, k.predefined)
    };
    let mark = |id: ElementId| marks.get(&id).cloned().unwrap_or_default();
    let n = found.len();
    if by_type {
        let columns = vec![
            col("Mark", None, 16.0, false),
            col("Level", None, 28.0, false),
            col("Mounting", None, 34.0, false),
            col("IFC", None, 50.0, false),
        ];
        for (t, tid, o, e) in found {
            // an object with a mark of its own shows it
            let m = creabim_core::marks::manual(e).map_or_else(|| mark(tid), str::to_string);
            rows.push((
                elevation(o.level),
                m.clone(),
                Row {
                    id: e.id,
                    group: false,
                    cells: vec![
                        m,
                        level_name(o.level),
                        mounting(&units, t.mount == objects::Mount::Ceiling, o.offset),
                        ifc(t),
                    ],
                },
            ));
        }
        let totals = vec![
            format!("{n} objects"),
            String::new(),
            String::new(),
            String::new(),
        ];
        return (columns, totals, n);
    }
    let columns = vec![
        col("Mark", None, 16.0, false),
        col("Type", None, 44.0, false),
        col("Level", None, 28.0, false),
        col("Mounting", None, 34.0, false),
        col("IFC", None, 50.0, false),
        col("Count", None, 14.0, true),
    ];
    // one line a type and level: category, mark, level, then the line
    let mut lines: Vec<(usize, String, f64, ElementId, ElementId, Vec<f64>)> = vec![];
    for (t, tid, o, _) in &found {
        let c = ObjectCategory::ALL
            .iter()
            .position(|c| *c == t.category)
            .unwrap_or(0);
        match lines.iter_mut().find(|l| l.3 == *tid && l.4 == o.level) {
            Some(l) => l.5.push(o.offset),
            None => lines.push((
                c,
                mark(*tid),
                elevation(o.level),
                *tid,
                o.level,
                vec![o.offset],
            )),
        }
    }
    lines.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then(a.2.total_cmp(&b.2))
    });
    let mut out: Vec<Row> = vec![];
    for group in lines.chunk_by(|a, b| a.0 == b.0) {
        let count: usize = group.iter().map(|l| l.5.len()).sum();
        let mut cells = vec![String::new(); columns.len()];
        cells[0] = format!("{} ({count})", ObjectCategory::ALL[group[0].0].label());
        out.push(Row {
            id: ElementId::NIL,
            cells,
            group: true,
        });
        for (_, m, _, tid, level, offsets) in group {
            let Some(t) = objects::type_of_id(model, *tid) else {
                continue;
            };
            let same = offsets.iter().all(|v| (v - offsets[0]).abs() < 1e-6);
            let hung = t.mount == objects::Mount::Ceiling;
            out.push(Row {
                id: *tid,
                group: false,
                cells: vec![
                    m.clone(),
                    model.get(*tid).map(|e| e.name.clone()).unwrap_or_default(),
                    level_name(*level),
                    if same {
                        mounting(&units, hung, offsets[0])
                    } else {
                        "Varies".into()
                    },
                    ifc(t),
                    offsets.len().to_string(),
                ],
            });
        }
    }
    // in this order: the shared sort below keeps it
    for (i, r) in out.into_iter().enumerate() {
        rows.push((0.0, format!("{i:06}"), r));
    }
    let what = if furniture { "pieces" } else { "fixtures" };
    let totals = vec![
        format!("{n} {what}"),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        n.to_string(),
    ];
    (columns, totals, n)
}

pub const ROW: f64 = 6.0;
const TITLE_ROW: f64 = 8.0;
const PAD: f64 = 1.5;

/// The schedule as a table drawn in paper millimetres, its top-left corner
/// at the origin. Row text carries the row's element, so clicking a row on a
/// sheet selects that element.
pub fn schedule_drawing(view: ElementId, title: &str, s: &Schedule) -> PlanView {
    let width: f64 = s.columns.iter().map(|c| c.width).sum();
    let mut items = Vec::new();
    let line = |items: &mut Vec<PlanItem>, a: DVec2, b: DVec2, heavy: bool| {
        items.push(PlanItem::Line {
            id: view,
            pen: if heavy { Pen::Cut } else { Pen::Symbol },
            a,
            b,
        })
    };
    let text = |items: &mut Vec<PlanItem>, id, at, t: &str, size, align| {
        items.push(PlanItem::Text {
            id,
            at,
            text: t.to_string(),
            size,
            align,
            mm: 1.0,
            rgb: None,
            bold: false,
            angle: 0.0,
        })
    };
    text(
        &mut items,
        view,
        DVec2::new(width / 2.0, -TITLE_ROW / 2.0),
        &title.to_uppercase(),
        3.5,
        Anchor::Middle,
    );
    let rows = s.rows.len() + 2; // header, rows, totals
    let bottom = -TITLE_ROW - rows as f64 * ROW;
    let mut x = 0.0;
    let cell = |c: &Column, x: f64| {
        if c.numeric {
            (x + c.width - PAD, Anchor::End)
        } else {
            (x + PAD, Anchor::Start)
        }
    };
    for (i, c) in s.columns.iter().enumerate() {
        let (tx, align) = cell(c, x);
        text(
            &mut items,
            view,
            DVec2::new(tx, -TITLE_ROW - ROW / 2.0),
            c.title,
            2.5,
            align,
        );
        for (r, row) in s.rows.iter().enumerate() {
            let y = -TITLE_ROW - (r as f64 + 1.5) * ROW;
            text(
                &mut items,
                row.id,
                DVec2::new(tx, y),
                &row.cells[i],
                2.5,
                align,
            );
        }
        if let Some(t) = s.totals.get(i).filter(|t| !t.is_empty()) {
            text(
                &mut items,
                view,
                DVec2::new(tx, bottom + ROW / 2.0),
                t,
                2.5,
                align,
            );
        }
        if i > 0 {
            // a group heading runs on across the empty cells after it, so
            // the column lines break there
            let spans = |row: &Row| row.group && row.cells[1..=i].iter().all(|c| c.is_empty());
            let mut top = -TITLE_ROW;
            for (r, row) in s.rows.iter().enumerate() {
                if spans(row) {
                    let y = -TITLE_ROW - (r as f64 + 1.0) * ROW;
                    if y < top {
                        line(&mut items, DVec2::new(x, top), DVec2::new(x, y), false);
                    }
                    top = y - ROW;
                }
            }
            line(&mut items, DVec2::new(x, top), DVec2::new(x, bottom), false);
        }
        x += c.width;
    }
    for r in 0..=rows {
        let y = -TITLE_ROW - r as f64 * ROW;
        let heavy = r == 0 || r == 1 || r == rows - 1 || r == rows;
        line(&mut items, DVec2::new(0.0, y), DVec2::new(width, y), heavy);
    }
    // outer frame
    for (a, b) in [
        (DVec2::ZERO, DVec2::new(width, 0.0)),
        (DVec2::ZERO, DVec2::new(0.0, bottom)),
        (DVec2::new(width, 0.0), DVec2::new(width, bottom)),
    ] {
        line(&mut items, a, b, true);
    }
    PlanView { items }
}

/// The schedule as CSV, header first, totals last.
pub fn to_csv(s: &Schedule) -> String {
    let quote = |v: &str| {
        if v.contains([',', '"', '\n']) {
            format!("\"{}\"", v.replace('"', "\"\""))
        } else {
            v.to_string()
        }
    };
    let line = |cells: Vec<&str>| cells.into_iter().map(quote).collect::<Vec<_>>().join(",");
    let mut out = line(s.columns.iter().map(|c| c.title).collect()) + "\n";
    for r in &s.rows {
        out += &(line(r.cells.iter().map(String::as_str).collect()) + "\n");
    }
    let mut totals: Vec<&str> = s.totals.iter().map(String::as_str).collect();
    totals.resize(s.columns.len(), "");
    out += &(line(totals) + "\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::sample::sample_house;

    #[test]
    fn door_schedule_lists_marks_in_order_and_follows_edits() {
        let mut doc = creabim_core::Document::from_model(sample_house());
        let doors = schedule(doc.model(), ScheduleCategory::Doors);
        let marks: Vec<&str> = doors.rows.iter().map(|r| r.cells[0].as_str()).collect();
        assert_eq!(marks, ["D01", "D02"]);
        assert_eq!(doors.totals[0], "2 doors");
        let front = doors.rows[0].id;
        assert_eq!(doors.rows[0].cells[3], "1000");
        doc.edit(front, "Widen", |e| set_value(e, Field::Width, 1.2))
            .unwrap();
        let doors = schedule(doc.model(), ScheduleCategory::Doors);
        assert_eq!(doors.rows[0].cells[3], "1200");
    }

    #[test]
    fn wall_schedule_totals_areas_and_exports_csv() {
        let m = sample_house();
        let walls = schedule(&m, ScheduleCategory::Walls);
        assert_eq!(walls.rows.len(), 9);
        let csv = to_csv(&walls);
        assert!(csv.starts_with("Name,Level,Material,Length"));
        assert!(csv.contains("GF Wall 1,Ground Floor,Brick,10000,300,3000"));
        assert!(csv.trim_end().ends_with("m²"));
    }

    #[test]
    fn layered_walls_list_their_assembly() {
        let m = creabim_core::sample::layered_house();
        let walls = schedule(&m, ScheduleCategory::Walls);
        assert_eq!(walls.columns[3].title, "Assembly");
        let row = walls
            .rows
            .iter()
            .find(|r| r.cells[0] == "GF Wall 1")
            .unwrap();
        assert_eq!(row.cells[2], "Block, concrete", "listed by its core");
        assert_eq!(row.cells[3], "Exterior wall 365, brick cavity");
        assert_eq!(row.cells[5], "365");
        assert_eq!(walls.totals.len(), walls.columns.len());
    }

    #[test]
    fn an_imperial_project_schedules_in_feet_and_inches() {
        let mut doc = creabim_core::Document::from_model(sample_house());
        let tx = creabim_core::units::set_units(doc.model(), creabim_core::Units::imperial());
        doc.commit(tx).unwrap();
        let walls = schedule(doc.model(), ScheduleCategory::Walls);
        // 10 m long, 300 thick, 3 m high
        assert_eq!(
            walls.rows[0].cells[..6],
            [
                "GF Wall 1",
                "Ground Floor",
                "Brick",
                "32'-9 3/4\"",
                "0'-11 3/4\"",
                "9'-10 1/8\""
            ]
        );
        assert!(walls.totals.last().unwrap().ends_with("ft²"));
        // inch marks are quoted in the CSV
        assert!(to_csv(&walls).contains(",\"32'-9 3/4\"\"\","));
    }

    #[test]
    fn typed_elements_list_and_group_by_their_type() {
        let m = creabim_core::sample::typed_house();
        let windows = schedule(&m, ScheduleCategory::Windows);
        assert_eq!(windows.columns[1].title, "Type");
        assert!(windows
            .rows
            .iter()
            .all(|r| r.cells[1] == "Window 1200 × 1300, two panes"));
        let walls = schedule_by(&m, ScheduleCategory::Walls, true);
        assert_eq!(walls.totals.len(), walls.columns.len());
        let groups: Vec<&str> = walls
            .rows
            .iter()
            .filter(|r| r.group)
            .map(|r| r.cells[0].as_str())
            .collect();
        assert_eq!(
            groups,
            [
                "Exterior wall 365, brick cavity (7)",
                "Interior wall 126, block (1)",
                "Storefront, 4 bays (1)"
            ]
        );
        // each group heading names its type, the rows under it its instances
        let first = &walls.rows[0];
        assert!(matches!(
            m.get(first.id).map(|e| &e.kind),
            Some(ElementKind::ElementType(_))
        ));
        assert_eq!(walls.rows.len(), 9 + 3);
        // an untyped project keeps its old table
        let plain = schedule(&sample_house(), ScheduleCategory::Walls);
        assert!(plain.columns.iter().all(|c| c.title != "Type"));
    }

    #[test]
    fn furniture_and_fixtures_are_counted_by_type_and_level() {
        let m = creabim_core::sample::furnished_house();
        let s = schedule(&m, ScheduleCategory::Fixtures);
        let titles: Vec<&str> = s.columns.iter().map(|c| c.title).collect();
        assert_eq!(
            titles,
            ["Mark", "Type", "Level", "Mounting", "IFC", "Count"]
        );
        // a heading for each category, in catalogue order
        let headings: Vec<&str> = s
            .rows
            .iter()
            .filter(|r| r.group)
            .map(|r| r.cells[0].split(" (").next().unwrap())
            .collect();
        assert_eq!(
            headings,
            [
                "Bathroom",
                "Kitchen",
                "Electrical",
                "Lighting",
                "Plumbing",
                "Heating and ventilation"
            ]
        );
        let line = |ty: &str, level: &str| {
            s.rows
                .iter()
                .find(|r| !r.group && r.cells[1] == ty && r.cells[2] == level)
                .unwrap_or_else(|| panic!("{ty} on {level}"))
        };
        let sockets = line("Double socket", "Ground Floor");
        assert_eq!(sockets.cells[5], "5");
        assert!(sockets.cells[0].starts_with('E'), "{:?}", sockets.cells);
        assert_eq!(sockets.cells[3], "300 above floor");
        assert_eq!(sockets.cells[4], "IfcOutlet POWEROUTLET");
        assert_eq!(line("Double socket", "First Floor").cells[5], "3");
        let lights = line("Recessed downlight", "Ground Floor");
        assert_eq!(
            (lights.cells[3].as_str(), lights.cells[5].as_str()),
            ("At the ceiling", "3")
        );
        assert!(lights.cells[0].starts_with('L'));
        // the counts add up to every fixture, and furniture is apart
        let counted: usize = s
            .rows
            .iter()
            .filter(|r| !r.group)
            .map(|r| r.cells[5].parse::<usize>().unwrap())
            .sum();
        assert_eq!(counted, s.listed);
        assert_eq!(s.totals[5], s.listed.to_string());
        let f = schedule(&m, ScheduleCategory::Furniture);
        let objects = m
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::Object(_)))
            .count();
        assert_eq!(f.listed + s.listed, objects);
        assert!(f
            .rows
            .iter()
            .all(|r| r.group || r.cells[4].starts_with("IfcFurniture")));
        // grouped by type instead: one row an object, under its type
        let by = schedule_by(&m, ScheduleCategory::Fixtures, true);
        assert_eq!(by.rows.iter().filter(|r| !r.group).count(), s.listed);
        assert!(by
            .rows
            .iter()
            .any(|r| r.group && r.cells[0] == "Double socket (8)"));
        assert_eq!(by.totals.len(), by.columns.len());
        assert!(to_csv(&s).starts_with("Mark,Type,Level,Mounting,IFC,Count"));
    }

    #[test]
    fn a_group_heading_runs_on_across_the_empty_cells_after_it() {
        let m = creabim_core::starter::starter_house();
        let s = schedule(&m, ScheduleCategory::Rooms);
        let r = s.rows.iter().position(|r| r.group).unwrap();
        let d = schedule_drawing(ElementId::NIL, "Room Schedule", &s);
        let x1 = s.columns[0].width;
        let mid = -TITLE_ROW - (r as f64 + 1.5) * ROW;
        let crosses = |x: f64| {
            d.items.iter().any(|it| {
                matches!(it, PlanItem::Line { a, b, .. }
                if (a.x - x).abs() < 1e-9 && (b.x - x).abs() < 1e-9
                    && a.y.max(b.y) > mid && a.y.min(b.y) < mid)
            })
        };
        assert!(
            !crosses(x1),
            "the level heading is cut by the first column line"
        );
        // the area column's line still runs through it
        let area = s.columns.iter().position(|c| c.title == "Area").unwrap();
        let xa: f64 = s.columns[..area].iter().map(|c| c.width).sum();
        assert!(crosses(xa));
    }

    #[test]
    fn schedule_drawing_is_a_table() {
        let m = sample_house();
        let s = schedule(&m, ScheduleCategory::Windows);
        let d = schedule_drawing(ElementId::NIL, "Window Schedule", &s);
        let (lo, hi) = d.bounds().unwrap();
        let width: f64 = s.columns.iter().map(|c| c.width).sum();
        assert!((hi.x - lo.x - width).abs() < 1e-9);
        assert!((lo.y + TITLE_ROW + (s.rows.len() + 2) as f64 * ROW).abs() < 1e-9);
    }
}
