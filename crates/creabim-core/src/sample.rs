//! A small sample house used by the demo command, tests and the app's "New sample".

use crate::*;
use glam::{DVec2, DVec3};

pub fn sample_house() -> Model {
    let mut doc = Document::new();
    let l0 = Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let l1 = Element::new(
        "First Floor",
        ElementKind::Level(Level {
            elevation: 3.0,
            height: 3.0,
        }),
    );
    let (g, f) = (l0.id, l1.id);
    let mut tx = Transaction::new("Sample house").insert(l0).insert(l1);

    let material = |name: &str, hatch: Hatch, color: [f32; 3], category: &str| {
        Element::new(
            name,
            ElementKind::Material(Material {
                category: category.into(),
                ..Material::new(hatch, color)
            }),
        )
    };
    let brick = material("Brick", Hatch::Brick, [0.78, 0.55, 0.45], "brick");
    let concrete = material("Concrete", Hatch::Concrete, [0.70, 0.70, 0.70], "concrete");
    let stud = material("Stud partition", Hatch::Cross, [0.90, 0.89, 0.86], "gypsum");
    let timber = material("Timber", Hatch::Timber, [0.70, 0.56, 0.40], "wood");
    let (brick_id, concrete_id, stud_id, timber_id) = (brick.id, concrete.id, stud.id, timber.id);
    tx = tx
        .insert(brick)
        .insert(concrete)
        .insert(stud)
        .insert(timber);

    for (label, a, b) in [
        ("A", DVec2::new(0.0, -1.0), DVec2::new(0.0, 7.0)),
        ("B", DVec2::new(10.0, -1.0), DVec2::new(10.0, 7.0)),
        ("1", DVec2::new(-1.0, 0.0), DVec2::new(11.0, 0.0)),
        ("2", DVec2::new(-1.0, 6.0), DVec2::new(11.0, 6.0)),
    ] {
        tx = tx.insert(Element::new(
            format!("Grid {label}"),
            ElementKind::Grid(Grid {
                label: label.into(),
                start: a,
                end: b,
                group: None,
            }),
        ));
    }

    let corners = [
        DVec2::new(0.0, 0.0),
        DVec2::new(10.0, 0.0),
        DVec2::new(10.0, 6.0),
        DVec2::new(0.0, 6.0),
    ];
    let mut ground_walls = Vec::new();
    // doors and windows with the plan they are tagged in and the tag's offset
    let mut tagged: Vec<(ElementId, ElementId, DVec2)> = Vec::new();
    for (level, lname) in [(g, "GF"), (f, "FF")] {
        tx = tx.insert(Element::new(
            format!("{lname} Slab"),
            ElementKind::Slab(Slab {
                layer_edges: vec![],
                level,
                boundary: corners.to_vec(),
                thickness: 0.25,
                material: Some(concrete_id),
                top_offset: 0.0,
                bulges: vec![],
                assembly: None,
                holes: vec![],
            }),
        ));
        for i in 0..4 {
            let wall = Element::new(
                format!("{lname} Wall {}", i + 1),
                ElementKind::Wall(Wall {
                    level,
                    start: corners[i],
                    end: corners[(i + 1) % 4],
                    thickness: 0.3,
                    height: 3.0,
                    base_offset: 0.0,
                    material: Some(brick_id),
                    assembly: None,
                    flip: false,
                    location: Default::default(),
                    top: Default::default(),
                    disallow_join: [false; 2],
                    attach_top: vec![],
                    curtain: None,
                    layer_ends: vec![],
                    bulge: 0.0,
                }),
            );
            let id = wall.id;
            // the walls run counter-clockwise, so outside is to their right
            let d = (corners[(i + 1) % 4] - corners[i]).normalize();
            let outside = -d.perp();
            tx = tx.insert(wall);
            if level == g {
                ground_walls.push(id);
            }
            let windows: &[f64] = match i {
                0 | 2 => &[1.5, 6.5],
                _ => &[2.4],
            };
            for (k, &off) in windows.iter().enumerate() {
                if level == g && i == 0 && k == 0 {
                    continue; // the front door goes here instead
                }
                let window = Element::new(
                    format!("{lname} Window {}.{}", i + 1, k + 1),
                    ElementKind::Opening(Opening {
                        host: id,
                        kind: OpeningKind::Window,
                        offset: off,
                        sill: 0.9,
                        width: 1.2,
                        height: 1.3,
                        material: None,
                        // two casements side by side
                        frame: OpeningFrame {
                            panes_across: 2,
                            ..Default::default()
                        },
                        flip_hand: false,
                        flip_facing: false,
                        wrap: None,
                    }),
                );
                tagged.push((window.id, level, outside * 0.9));
                tx = tx.insert(window);
            }
        }
    }
    let front_door = Element::new(
        "Front Door",
        ElementKind::Opening(Opening {
            host: ground_walls[0],
            kind: OpeningKind::Door,
            offset: 1.6,
            sill: 0.0,
            width: 1.0,
            height: 2.2,
            material: None,
            frame: Default::default(),
            flip_hand: false,
            flip_facing: false,
            wrap: None,
        }),
    );
    let front_door_id = front_door.id;
    tagged.push((front_door.id, g, DVec2::new(0.0, 1.0)));
    tx = tx.insert(front_door);
    // An inner partition with a door.
    let part = Element::new(
        "GF Partition",
        ElementKind::Wall(Wall {
            level: g,
            start: DVec2::new(6.0, 0.15),
            end: DVec2::new(6.0, 5.85),
            thickness: 0.12,
            height: 3.0,
            base_offset: 0.0,
            material: Some(stud_id),
            assembly: None,
            flip: false,
            location: Default::default(),
            top: Default::default(),
            disallow_join: [false; 2],
            attach_top: vec![],
            curtain: None,
            layer_ends: vec![],
            bulge: 0.0,
        }),
    );
    let pid = part.id;
    let inner_door = Element::new(
        "GF Inner Door",
        ElementKind::Opening(Opening {
            host: pid,
            kind: OpeningKind::Door,
            offset: 3.5,
            sill: 0.0,
            width: 0.9,
            height: 2.1,
            material: None,
            frame: Default::default(),
            flip_hand: false,
            flip_facing: false,
            wrap: None,
        }),
    );
    tagged.push((inner_door.id, g, DVec2::new(0.8, 0.0)));
    tx = tx.insert(part).insert(inner_door);

    // A freeform canopy over the entrance, modelled as a mesh and classified as a roof.
    let mut canopy = EditMesh::cuboid(DVec3::new(3.0, 1.6, 0.15));
    canopy.scale_face(1, 0.85);
    tx = tx.insert(Element::new(
        "Entrance Canopy",
        ElementKind::FreeMesh(FreeMesh {
            ifc_class: "IfcRoof".into(),
            material: Some(timber_id),
            ..FreeMesh::new(Some(g), DVec3::new(2.1, -0.95, 2.6), canopy)
        }),
    ));

    // A garden pergola built with modifiers: one post repeated by two
    // arrays into four, and one rafter repeated along the beam.
    let array = |count, x, y| {
        Modifier::new(ModifierKind::Array {
            count,
            offset: DVec3::new(x, y, 0.0),
            relative: false,
            fit_length: None,
            turn: 0.0,
            grow: 1.0,
        })
    };
    let pergola = DVec3::new(6.5, -5.4, 0.0);
    tx = tx
        .insert(Element::new(
            "Pergola Posts",
            ElementKind::FreeMesh(FreeMesh {
                ifc_class: "IfcColumn".into(),
                material: Some(timber_id),
                modifiers: vec![array(2, 3.0, 0.0), array(2, 0.0, 2.0)],
                ..FreeMesh::new(
                    Some(g),
                    pergola,
                    EditMesh::cuboid(DVec3::new(0.15, 0.15, 2.4)),
                )
            }),
        ))
        .insert(Element::new(
            "Pergola Rafters",
            ElementKind::FreeMesh(FreeMesh {
                ifc_class: "IfcBeam".into(),
                material: Some(timber_id),
                modifiers: vec![array(7, 0.5, 0.0)],
                ..FreeMesh::new(
                    Some(g),
                    pergola + DVec3::new(0.0, 1.0, 2.4),
                    EditMesh::cuboid(DVec3::new(0.08, 2.8, 0.2)),
                )
            }),
        ));

    // Views: a plan per level, two sections, four elevations and a 3D view.
    let plan_g = Element::new("Ground Floor plan", ElementKind::View(View::plan(g)));
    let plan_f = Element::new("First Floor plan", ElementKind::View(View::plan(f)));
    let mut axo = View::axonometric(35.0, 30.0);
    axo.scale = 60.0;
    let axo = Element::new("3D View South-East", ElementKind::View(axo));
    let (plan_g_id, plan_f_id) = (plan_g.id, plan_f.id);
    // a cropped 1:50 plan of the entrance, for the details sheet
    let mut entrance = View::plan(g);
    entrance.scale = 50.0;
    let entrance = Element::new("Entrance plan", ElementKind::View(entrance));
    let entrance_id = entrance.id;
    let mut placements = vec![
        (
            "A101",
            plan_g.id,
            DVec2::new(160.0, 240.0),
            DVec2::new(5.0, 3.0),
        ),
        (
            "A101",
            plan_f.id,
            DVec2::new(400.0, 240.0),
            DVec2::new(5.0, 3.0),
        ),
        ("A001", axo.id, DVec2::new(200.0, 165.0), DVec2::ZERO),
    ];
    tx = tx
        .insert(plan_g)
        .insert(plan_f)
        .insert(entrance)
        .insert(axo);
    let mut section_a = None;
    for (name, a, b, cut) in [
        (
            "Section A",
            DVec2::new(2.1, -5.0),
            DVec2::new(2.1, 9.0),
            true,
        ),
        (
            "Section B",
            DVec2::new(-2.0, 3.0),
            DVec2::new(12.0, 3.0),
            true,
        ),
        (
            "South Elevation S",
            DVec2::new(-2.0, -4.0),
            DVec2::new(12.0, -4.0),
            false,
        ),
        (
            "East Elevation E",
            DVec2::new(14.0, -2.0),
            DVec2::new(14.0, 8.0),
            false,
        ),
        (
            "North Elevation N",
            DVec2::new(12.0, 10.0),
            DVec2::new(-2.0, 10.0),
            false,
        ),
        (
            "West Elevation W",
            DVec2::new(-4.0, 8.0),
            DVec2::new(-4.0, -2.0),
            false,
        ),
    ] {
        let mut view = View::section(a, b, 30.0, cut);
        if name == "South Elevation S" {
            view.style = ViewStyle::Shaded;
        }
        let e = Element::new(name, ElementKind::View(view));
        if name == "Section A" {
            section_a = Some(e.id);
        }
        // where each drawing sits on the sections sheet, and the drawing
        // point pinned there (along the line, height)
        let at = match name {
            "Section A" => Some((DVec2::new(105.0, 215.0), DVec2::new(8.0, 3.0))),
            "Section B" => Some((DVec2::new(290.0, 215.0), DVec2::new(7.0, 3.0))),
            "South Elevation S" => Some((DVec2::new(105.0, 110.0), DVec2::new(7.0, 3.0))),
            "East Elevation E" => Some((DVec2::new(290.0, 110.0), DVec2::new(5.0, 3.0))),
            _ => None,
        };
        if let Some((center, target)) = at {
            placements.push(("A201", e.id, center, target));
        }
        tx = tx.insert(e);
    }

    // Schedules, listed on their own sheet next to the entrance detail.
    for (i, category) in [
        ScheduleCategory::Doors,
        ScheduleCategory::Windows,
        ScheduleCategory::Walls,
    ]
    .into_iter()
    .enumerate()
    {
        let e = Element::new(
            format!(
                "{} Schedule",
                &category.label()[..category.label().len() - 1]
            ),
            ElementKind::View(View::schedule(category)),
        );
        // the schedule's top-left corner is its drawing origin
        let top = [280.0, 238.0, 142.0][i];
        placements.push(("A601", e.id, DVec2::new(30.0, top), DVec2::ZERO));
        tx = tx.insert(e);
    }
    placements.push((
        "A601",
        entrance_id,
        DVec2::new(300.0, 190.0),
        DVec2::new(2.1, 0.0),
    ));

    // Dimensions and tags.
    let dim = |view: ElementId, a: HostPoint, b: HostPoint, offset: f64| {
        Element::new(
            "Dimension",
            ElementKind::Dimension(Dimension::linear(view, a, b, offset)),
        )
    };
    let on = |host: ElementId, u: f64, s: f64, from_end: bool| HostPoint {
        host: Some(host),
        local: DVec2::new(u, s),
        from_end,
        body: None,
    };
    let (south, west) = (ground_walls[0], ground_walls[3]);
    let face = -0.15;
    for plan in [plan_g_id, entrance_id] {
        // overall length and depth to the outer faces
        tx = tx
            .insert(dim(
                plan,
                on(south, -0.15, face, false),
                on(south, 0.15, face, true),
                -2.7,
            ))
            .insert(dim(
                plan,
                on(west, -0.15, face, false),
                on(west, 0.15, face, true),
                -1.95,
            ));
        // the front wall's openings as a chain
        let chain = [
            on(south, -0.15, face, false),
            on(front_door_id, 0.0, face, false),
            on(front_door_id, 1.0, face, false),
            on(south, 0.15, face, true),
        ];
        for w in chain.windows(2) {
            tx = tx.insert(dim(plan, w[0], w[1], -2.0));
        }
    }
    for (opening, level, offset) in tagged {
        let plan = if level == g { plan_g_id } else { plan_f_id };
        tx = tx.insert(Element::new(
            "Tag",
            ElementKind::Tag(Tag::new(plan, opening, offset)),
        ));
        if opening == front_door_id {
            tx = tx.insert(Element::new(
                "Tag",
                ElementKind::Tag(Tag::new(entrance_id, opening, offset)),
            ));
        }
    }
    if let Some(section) = section_a {
        // storey heights beside the section
        let level = |id: ElementId, h: f64| HostPoint {
            host: Some(id),
            local: DVec2::new(3.6, h),
            from_end: false,
            body: None,
        };
        tx = tx
            .insert(dim(section, level(g, 0.0), level(f, 0.0), 0.0))
            .insert(dim(section, level(f, 0.0), level(f, 3.0), 0.0));
    }

    // Sheets with the views placed on them.
    let mut sheets = std::collections::HashMap::new();
    for (number, title) in [
        ("A001", "3D View"),
        ("A101", "Floor Plans"),
        ("A201", "Sections and Elevations"),
        ("A601", "Schedules and Details"),
    ] {
        let size = if number == "A101" { "A2" } else { "A3" };
        let sheet = Element::new(title, ElementKind::Sheet(Sheet::a(number, size)));
        sheets.insert(number, sheet.id);
        tx = tx.insert(sheet);
    }
    for (number, view, center, target) in placements {
        tx = tx.insert(Element::new(
            "Viewport",
            ElementKind::Viewport(Viewport {
                sheet: sheets[number],
                view,
                center,
                target,
                // the entrance detail shows only the front of the house
                crop: (view == entrance_id)
                    .then_some([DVec2::new(-0.8, -2.8), DVec2::new(5.0, 2.0)]),
            }),
        ));
    }

    doc.commit(tx).expect("sample house is valid");
    doc.model().clone()
}

/// The sample house built of layered assemblies: cavity outer walls, a
/// plastered block partition, concrete floors and a tiled gable roof.
pub fn layered_house() -> Model {
    let mut doc = Document::from_model(sample_house());
    let (tx, ids) = assembly::add_starters(doc.model());
    doc.commit(tx).expect("starter assemblies are valid");
    let [outer, inner, floor, roof] = ids[..] else {
        unreachable!("four starters")
    };
    let model = doc.model().clone();
    let pick = |f: &dyn Fn(&Element) -> bool| -> Vec<ElementId> {
        model.iter().filter(|e| f(e)).map(|e| e.id).collect()
    };
    let is_wall = |e: &Element| matches!(e.kind, ElementKind::Wall(_));
    let outer_walls = pick(&|e| is_wall(e) && !e.name.contains("Partition"));
    let partitions = pick(&|e| is_wall(e) && e.name.contains("Partition"));
    let slabs = pick(&|e| matches!(e.kind, ElementKind::Slab(_)));
    for (elements, a) in [(outer_walls, outer), (partitions, inner), (slabs, floor)] {
        let (tx, _) = assembly::assign(doc.model(), &elements, Some(a));
        doc.commit(tx).expect("assemblies fit");
    }
    // a gable roof on the first floor's walls
    let first = model
        .levels()
        .find(|(e, _)| e.name == "First Floor")
        .map(|(e, _)| e.id)
        .expect("a first floor");
    let mut r = Roof::new(
        first,
        vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 6.0),
            DVec2::new(0.0, 6.0),
        ],
        RoofShape::Gable,
    );
    r.base_offset = 3.0;
    r.pitch = 35.0;
    let mut e = Element::new("Roof", ElementKind::Roof(r));
    assembly::set_assembly(doc.model(), &mut e, Some(roof));
    let roof_id = e.id;
    doc.commit(Transaction::new("Roof").insert(e))
        .expect("the roof is valid");
    // the first floor's walls reach up to it: the gables fill in, the eave
    // walls are cut to the slope instead of poking through
    let mut tx = Transaction::new("Attach walls to the roof");
    for e in doc.model().iter() {
        if let ElementKind::Wall(w) = &e.kind {
            if w.level == first {
                let mut after = e.clone();
                if let ElementKind::Wall(w) = &mut after.kind {
                    w.attach_top = vec![roof_id];
                }
                tx = tx.replace(e.clone(), after);
            }
        }
    }
    doc.commit(tx).expect("attaching keeps the walls valid");
    doc.model().clone()
}

/// The sample house with a timber beam buried in the ground floor's south
/// wall (GF Wall 1), under the first floor: for trying and testing how
/// elements hidden inside others show and are picked. Returns the beam.
pub fn sample_with_buried_beam() -> (Model, ElementId) {
    let mut doc = Document::from_model(sample_house());
    let model = doc.model();
    let level = model
        .levels()
        .find(|(e, _)| e.name == "Ground Floor")
        .map(|(e, _)| e.id);
    let timber = model.iter().find(|e| e.name == "Timber").map(|e| e.id);
    let mut beam = Beam::new(
        level.expect("the sample has a ground floor"),
        DVec2::new(1.0, 0.0),
        DVec2::new(4.0, 0.0),
    );
    beam.width = 0.16;
    beam.depth = 0.3;
    beam.top_offset = 0.4;
    beam.material = timber;
    let e = Element::new("Lintel beam", ElementKind::Beam(beam));
    let id = e.id;
    doc.commit(Transaction::new("Add beam").insert(e))
        .expect("the beam goes in");
    (doc.model().clone(), id)
}

/// The sample house built of element types: typed walls, doors, windows
/// and slabs, a storefront curtain wall at the back of the ground floor
/// with a garden door in one bay, a bare hole through a wall, a stair
/// shaft through the first floor and a hole in it for a flue.
pub fn typed_house() -> Model {
    let mut doc = Document::from_model(sample_house());
    let (tx, ids) = types::add_starters(doc.model());
    doc.commit(tx).expect("starter types are valid");
    let starter = |name: &str| {
        let i = types::STARTERS
            .iter()
            .position(|(n, _, _)| *n == name)
            .expect("a starter of that name");
        ids[i]
    };
    let model = doc.model().clone();
    let named = |name: &str| {
        model
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.id)
            .expect("the sample has it")
    };
    let pick = |f: &dyn Fn(&Element) -> bool| -> Vec<ElementId> {
        model.iter().filter(|e| f(e)).map(|e| e.id).collect()
    };
    let back = named("GF Wall 3");
    let is_wall = |e: &Element| matches!(e.kind, ElementKind::Wall(_));
    let outer = pick(&|e| is_wall(e) && !e.name.contains("Partition") && e.id != back);
    let opening = |e: &Element, kind: OpeningKind| matches!(&e.kind, ElementKind::Opening(o) if o.kind == kind);
    let on_back = |e: &Element| matches!(&e.kind, ElementKind::Opening(o) if o.host == back);
    let back_openings = pick(&on_back);
    let windows = pick(&|e| opening(e, OpeningKind::Window) && !on_back(e));
    let doors = pick(&|e| opening(e, OpeningKind::Door) && e.name != "Front Door");
    let slabs = pick(&|e| matches!(e.kind, ElementKind::Slab(_)));
    // the storefront replaces the back windows
    doc.commit(Transaction::new("Storefront").remove_all_cascade(doc.model(), &back_openings))
        .expect("removing windows is valid");
    for (elements, t) in [
        (outer, "Exterior wall 365, brick cavity"),
        (vec![named("GF Partition")], "Interior wall 126, block"),
        (vec![back], "Storefront, 4 bays"),
        (windows, "Window 1200 × 1300, two panes"),
        (doors, "Single door 900 × 2100"),
        (vec![named("Front Door")], "Wide door 1200 × 2100"),
        (slabs, "Floor 300, concrete"),
    ] {
        let (tx, _) = types::assign(doc.model(), &elements, Some(starter(t)));
        doc.commit(tx).expect("types fit");
    }

    // a door in the storefront's second bay
    let mut door = Element::new(
        "GF Garden Door",
        ElementKind::Opening(Opening {
            host: back,
            kind: OpeningKind::Door,
            offset: 3.3,
            sill: 0.0,
            width: 0.9,
            height: 2.1,
            material: None,
            frame: Default::default(),
            flip_hand: false,
            flip_facing: false,
            wrap: None,
        }),
    );
    types::stamp(doc.model(), &mut door, starter("Single door 900 × 2100"));
    // a bare hole through the east wall, for a duct
    let hole = Element::new(
        "GF Duct Opening",
        ElementKind::Opening(Opening {
            host: named("GF Wall 2"),
            kind: OpeningKind::Hole,
            offset: 4.4,
            sill: 2.2,
            width: 0.6,
            height: 0.4,
            material: None,
            frame: Default::default(),
            flip_hand: false,
            flip_facing: false,
            wrap: None,
        }),
    );
    let (g, f) = (named("Ground Floor"), named("First Floor"));
    // a stair shaft from the ground floor up to the first, cutting its slab
    let shaft = Element::new(
        "Stair Shaft",
        ElementKind::Shaft(Shaft::new(
            g,
            polygon::rectangle(DVec2::new(1.0, 3.5), DVec2::new(3.6, 5.5)),
            Some(f),
        )),
    );
    let mut tx = Transaction::new("Typed house")
        .insert(door)
        .insert(hole)
        .insert(shaft);
    // and a hole of the first floor slab's own, for a flue
    let ff = doc
        .model()
        .get(named("FF Slab"))
        .expect("a first floor slab");
    let mut after = ff.clone();
    if let ElementKind::Slab(s) = &mut after.kind {
        s.holes = vec![polygon::rectangle(
            DVec2::new(7.5, 4.2),
            DVec2::new(8.1, 4.8),
        )];
    }
    tx = tx.replace(ff.clone(), after);
    doc.commit(tx).expect("the typed house is valid");

    // a porch column by the front door, and footings: strips under the
    // ground floor's walls, their tops at the underside of its slab, and
    // a pad under the column
    // (left untyped: columns come back from IFC4 as meshes, without types)
    let column = Element::new(
        "Porch Column",
        ElementKind::Column(Column::new(
            g,
            DVec2::new(1.6, -1.5),
            Profile::Rectangle {
                width: 0.3,
                depth: 0.3,
            },
        )),
    );
    let porch = column.id;
    doc.commit(Transaction::new("Porch column").insert(column))
        .expect("the column is valid");
    let ground: Vec<ElementId> = doc
        .model()
        .iter()
        .filter(|e| matches!(&e.kind, ElementKind::Wall(w) if w.level == g))
        .map(|e| e.id)
        .chain([porch])
        .collect();
    let (strip, pad) = (
        starter("Strip footing 600 × 300"),
        starter("Pad footing 1200 × 1200 × 400"),
    );
    let model = doc.model().clone();
    let (mut tx, _) = footing::under(&model, &ground, &|e| {
        let t = match &e.kind {
            ElementKind::Footing(f) if f.kind == FootingKind::Pad => pad,
            _ => strip,
        };
        types::stamp(&model, e, t);
        if let ElementKind::Footing(f) = &mut e.kind {
            f.below = if f.kind == FootingKind::Strip {
                0.3
            } else {
                0.5
            };
        }
    });
    tx.label = "Footings".into();
    doc.commit(tx).expect("the footings are valid");
    doc.model().clone()
}

/// The sample house furnished from the starter catalogue: a living room
/// and a kitchen downstairs, a bedroom and a bathroom upstairs, with their
/// sockets, switches, lights, radiators, ventilation and plumbing points.
pub fn furnished_house() -> Model {
    let mut doc = Document::from_model(sample_house());
    let level = |name: &str| {
        doc.model()
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.id)
            .expect("the sample has its levels")
    };
    let (g, f) = (level("Ground Floor"), level("First Floor"));
    // (object, level, a point near where it goes, turn when free-standing)
    let plan: &[(&str, ElementId, DVec2, Option<f64>)] = &[
        // living room, west of the partition
        ("sofa-3", g, DVec2::new(0.4, 3.0), None),
        ("coffee-table", g, DVec2::new(1.55, 3.0), Some(90.0)),
        ("armchair", g, DVec2::new(3.6, 3.0), Some(270.0)),
        ("desk", g, DVec2::new(4.5, 5.6), None),
        ("chair", g, DVec2::new(4.5, 4.75), Some(180.0)),
        // kitchen along the east wall, the sink under its window
        ("fridge-freezer", g, DVec2::new(9.6, 0.5), None),
        ("base-cabinet-600", g, DVec2::new(9.6, 1.1), None),
        ("cooker-600", g, DVec2::new(9.6, 1.7), None),
        ("base-cabinet-600", g, DVec2::new(9.6, 2.3), None),
        ("sink-unit-1000", g, DVec2::new(9.6, 3.1), None),
        ("base-cabinet-600", g, DVec2::new(9.6, 3.9), None),
        ("wall-cabinet-600", g, DVec2::new(9.6, 1.1), None),
        ("wall-cabinet-600", g, DVec2::new(9.6, 3.9), None),
        ("dining-table", g, DVec2::new(7.4, 3.4), Some(90.0)),
        ("chair", g, DVec2::new(6.95, 2.95), Some(90.0)),
        ("chair", g, DVec2::new(6.95, 3.85), Some(90.0)),
        ("chair", g, DVec2::new(8.75, 2.95), Some(270.0)),
        ("chair", g, DVec2::new(8.75, 3.85), Some(270.0)),
        // bedroom upstairs, west
        ("double-bed", f, DVec2::new(2.5, 5.6), None),
        ("bedside-table", f, DVec2::new(1.4, 5.6), None),
        ("bedside-table", f, DVec2::new(3.6, 5.6), None),
        ("wardrobe", f, DVec2::new(0.4, 1.1), None),
        ("single-bed", f, DVec2::new(5.2, 5.6), None),
        // bathroom upstairs, east
        ("bathtub", f, DVec2::new(9.6, 1.25), None),
        ("wc", f, DVec2::new(8.3, 0.4), None),
        ("washbasin", f, DVec2::new(7.1, 0.4), None),
        ("shower", f, DVec2::new(9.4, 5.6), None),
        // services downstairs: the board and switches by the front door
        ("consumer-unit", g, DVec2::new(3.3, 0.3), None),
        ("switch-2-gang", g, DVec2::new(2.85, 0.3), None),
        ("switch-1-gang", g, DVec2::new(5.85, 3.3), None),
        ("switch-1-gang", g, DVec2::new(6.15, 4.6), None),
        ("socket-double", g, DVec2::new(0.3, 1.2), None),
        ("socket-double", g, DVec2::new(0.3, 4.8), None),
        ("socket-double", g, DVec2::new(4.5, 0.3), None),
        ("socket-double", g, DVec2::new(5.3, 5.7), None),
        ("socket-double", g, DVec2::new(6.15, 1.5), None),
        ("socket-single", g, DVec2::new(6.15, 5.2), None),
        ("ceiling-light", g, DVec2::new(3.0, 2.0), Some(0.0)),
        ("ceiling-light", g, DVec2::new(7.4, 3.4), Some(0.0)),
        ("downlight", g, DVec2::new(8.9, 1.0), Some(0.0)),
        ("downlight", g, DVec2::new(8.9, 2.2), Some(0.0)),
        ("downlight", g, DVec2::new(8.9, 3.4), Some(0.0)),
        ("smoke-detector", g, DVec2::new(4.6, 3.0), Some(0.0)),
        ("radiator", g, DVec2::new(7.1, 0.3), None),
        ("radiator", g, DVec2::new(2.9, 5.7), None),
        ("radiator", g, DVec2::new(7.9, 5.7), None),
        ("boiler", g, DVec2::new(9.35, 5.7), None),
        ("extract-fan", g, DVec2::new(9.6, 4.7), None),
        ("split-ac", g, DVec2::new(4.0, 5.7), None),
        ("outside-tap", g, DVec2::new(10.3, 5.0), None),
        // and upstairs
        ("switch-1-gang", f, DVec2::new(5.0, 0.3), None),
        ("socket-double", f, DVec2::new(0.85, 5.7), None),
        ("socket-double", f, DVec2::new(4.15, 5.7), None),
        ("socket-double", f, DVec2::new(0.3, 4.0), None),
        ("wall-light", f, DVec2::new(1.4, 5.7), None),
        ("wall-light", f, DVec2::new(3.6, 5.7), None),
        ("ceiling-light", f, DVec2::new(3.5, 2.5), Some(0.0)),
        ("downlight", f, DVec2::new(8.3, 1.2), Some(0.0)),
        ("downlight", f, DVec2::new(8.3, 3.6), Some(0.0)),
        ("smoke-detector", f, DVec2::new(4.6, 4.2), Some(0.0)),
        ("supply-diffuser", f, DVec2::new(1.6, 2.0), Some(0.0)),
        ("extract-grille", f, DVec2::new(8.0, 2.4), Some(0.0)),
        ("floor-drain", f, DVec2::new(8.6, 4.4), Some(0.0)),
        ("radiator", f, DVec2::new(2.1, 0.3), None),
        ("radiator", f, DVec2::new(7.9, 5.7), None),
        ("extract-fan", f, DVec2::new(9.6, 4.2), None),
        ("split-ac", f, DVec2::new(0.3, 3.0), None),
    ];
    for (key, lvl, at, turn) in plan {
        let c = crate::catalogue::starter(&format!("starter/{key}")).expect("a starter");
        let (p, r) = match turn {
            // lights and diffusers by their middle
            Some(r) if c.object.mount == crate::objects::Mount::Ceiling => {
                (crate::objects::centred(&c.object, *at, *r), *r)
            }
            Some(r) => (*at, *r),
            None => {
                let spot = crate::objects::against_wall_clear(
                    doc.model(),
                    *lvl,
                    *at,
                    0.8,
                    &c.object,
                    c.object.offset,
                )
                .expect("a wall near the point");
                (spot.at, spot.rotation)
            }
        };
        let (tx, _) = crate::objects::place(doc.model(), &c, *lvl, p, r);
        doc.commit(tx).expect("starter objects are valid");
    }
    doc.model().clone()
}
