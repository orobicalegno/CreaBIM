//! The starter house the app opens with: a two-storey family house with a
//! single-storey kitchen wing, drawn up as a small permit set.
//!
//! It is built the way a user would build it, through transactions, so
//! everything in it can be selected, edited and undone like anything
//! drawn by hand. It shows most of what CreaBIM models: four levels (with
//! a foundation level under the footings), layered walls, floors and
//! roofs with clean joins, element types, doors and windows with frames,
//! a curtain wall cutting its host wall, a stair with a landing, a shaft
//! and a slab hole, columns (one of them running up two storeys), beams,
//! railings, strip and pad footings, furniture and services fixtures from
//! the catalogue, grids, rooms whose tags and schedule read their areas
//! from the walls round them, ceilings that the upstairs partitions stop
//! at, dimensions, tags, sections, elevations, schedules and a set of
//! sheets with a title block.
//!
//! The house: a rendered two-storey main block 11 × 9.6 m (wall centres)
//! under a slate gable roof, with a two-storey loggia cut out of its
//! north-west corner, and a timber-clad kitchen wing on its east side
//! under a green flat roof that runs on over a garden terrace. Plan
//! coordinates are metres, x east and y north; the street is to the south.

use crate::curtain::{CurtainGrid, GridRule, PanelKind};
use crate::footing::FootingKind;
use crate::members::{Landing, Railing};
use crate::objects::Mount;
use crate::titleblock::{Corner, TextAlign, TitleItem};
use crate::types::{
    BeamType, ColumnType, ElementType, FootingType, OpeningType, SlabType, TypeParams, WallType,
};
use crate::*;
use glam::{DVec2, DVec3};
use std::collections::HashMap;

/// Thickness of the exterior walls (both assemblies are 400 mm).
const EXT: f64 = 0.4;
/// Thickness of the block partitions.
const BLOCK: f64 = 0.126;
/// How far the block of the rendered exterior wall sits in from its outer
/// face: the render and the insulation.
const CORE_OUT: f64 = 0.02 + 0.15;
/// Half the exterior wall, and half a block partition.
const H: f64 = EXT / 2.0;
const HB: f64 = BLOCK / 2.0;

/// The project's name, address and client, as the title block shows them.
pub const PROJECT: &str = "Maple Lane House";
const ADDRESS: &str = "12 Maple Lane, Greenfield";
const CLIENT: &str = "The Rivera family";

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

/// Builds the house step by step on one document.
struct Builder {
    doc: Document,
    /// Elements by the name they were given here (levels, walls, views…).
    named: HashMap<String, ElementId>,
}

impl Builder {
    fn id(&self, name: &str) -> ElementId {
        *self
            .named
            .get(name)
            .unwrap_or_else(|| panic!("the starter house has no {name}"))
    }

    fn commit(&mut self, tx: Transaction) {
        self.doc.commit(tx).expect("the starter house is valid");
    }

    /// Insert `e` under `key` in `tx`.
    fn keep(&mut self, key: &str, e: Element, tx: Transaction) -> Transaction {
        self.named.insert(key.to_string(), e.id);
        tx.insert(e)
    }

    /// A library material by name, added to the model if it is not there.
    fn material(&mut self, name: &str) -> ElementId {
        if let Some(e) = self
            .doc
            .model()
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Material(_)) && e.name == name)
        {
            return e.id;
        }
        let entry = crate::library::LIBRARY
            .iter()
            .find(|e| e.name == name)
            .unwrap_or_else(|| panic!("no library material {name}"));
        let e = entry.element();
        let id = e.id;
        self.commit(Transaction::new("Add material").insert(e));
        id
    }

    /// A type made of `params`, stamped onto elements later.
    fn add_type(&mut self, name: &str, description: &str, params: TypeParams) -> ElementId {
        let mut t = ElementType::new(params);
        t.description = description.into();
        let e = Element::new(name, ElementKind::ElementType(t));
        let id = e.id;
        self.named.insert(name.into(), id);
        self.commit(Transaction::new("Add type").insert(e));
        id
    }

    /// `e` stamped with the type named `t`.
    fn typed(&self, mut e: Element, t: &str) -> Element {
        let ok = types::stamp(self.doc.model(), &mut e, self.id(t));
        assert!(ok, "{t} fits {}", e.name);
        e
    }
}

/// A wall on `level` from `a` to `b`, `height` tall and topped at `top`.
fn wall(level: ElementId, a: DVec2, b: DVec2, height: f64, top: Option<ElementId>) -> Wall {
    let mut w = Wall::new(level, a, b, EXT, height);
    if let Some(t) = top {
        w.top = WallTop::UpToLevel {
            level: t,
            offset: 0.0,
        };
    }
    w
}

/// A door, window or hole in `host`, `offset` from its start.
fn opening(host: ElementId, kind: OpeningKind, offset: f64, width: f64) -> Opening {
    Opening {
        host,
        kind,
        offset,
        sill: 0.0,
        width,
        height: 2.1,
        material: None,
        frame: Default::default(),
        flip_hand: false,
        flip_facing: false,
        wrap: None,
    }
}

/// The starter house, with its views and sheets.
pub fn starter_house() -> Model {
    let mut b = Builder {
        doc: Document::new(),
        named: HashMap::new(),
    };
    levels(&mut b);
    materials_and_types(&mut b);
    structure(&mut b);
    openings(&mut b);
    footings(&mut b);
    furnish(&mut b);
    rooms_and_ceilings(&mut b);
    let views = views(&mut b);
    details(&mut b);
    annotate(&mut b, &views);
    sheets(&mut b, &views);
    b.doc.model().clone()
}

fn levels(b: &mut Builder) {
    let mut tx = Transaction::new("Starter house: levels");
    for (name, elevation, height) in [
        ("Foundation", -0.8, 0.8),
        ("Ground Floor", 0.0, 3.0),
        ("First Floor", 3.0, 2.8),
    ] {
        let e = Element::new(name, ElementKind::Level(Level { elevation, height }));
        tx = b.keep(name, e, tx);
    }
    b.commit(tx);
}

fn materials_and_types(b: &mut Builder) {
    // the starter types and assemblies, to swap in while tinkering
    let (tx, _) = types::add_starters(b.doc.model());
    b.commit(tx);
    let starters: Vec<(String, ElementId)> = types::types(b.doc.model())
        .into_iter()
        .map(|(id, name, _)| (name.to_string(), id))
        .collect();
    b.named.extend(starters);

    // a crisp white render and dark window frames of the house's own
    let mut tx = Transaction::new("Starter house: materials");
    let custom = [
        (
            "Render, white",
            Material {
                category: "plaster".into(),
                description: "White silicone render on insulation".into(),
                roughness: 0.9,
                ..Material::new(Hatch::Solid, [0.94, 0.93, 0.90])
            },
        ),
        (
            "Aluminium, anthracite",
            Material {
                category: "aluminium".into(),
                description: "Powder-coated aluminium, RAL 7016".into(),
                roughness: 0.45,
                metallic: 0.6,
                ..Material::new(Hatch::Solid, [0.21, 0.23, 0.25])
            },
        ),
        (
            "Steel, dark grey",
            Material {
                category: "steel".into(),
                description: "Structural steel, painted dark grey".into(),
                roughness: 0.5,
                metallic: 0.4,
                ..Material::new(Hatch::Solid, [0.25, 0.26, 0.28])
            },
        ),
    ];
    for (name, m) in custom {
        let e = Element::new(name, ElementKind::Material(m));
        tx = b.keep(name, e, tx);
    }
    b.commit(tx);
    let mut m = HashMap::new();
    for name in [
        "Plaster, gypsum",
        "Block, concrete",
        "Insulation, mineral wool",
        "Insulation, wood fibre",
        "Insulation, PIR",
        "Timber, oak",
        "Timber, softwood",
        "Plasterboard",
        "Parquet, oak",
        "Screed, cement",
        "Concrete, cast in place",
        "Roof slate",
        "Membrane, bitumen",
        "Membrane, EPDM",
        "Green roof",
        "Stone, slate",
        "Zinc",
        "Paint, white",
    ] {
        m.insert(name, b.material(name));
    }
    let own = |n: &str| Some(b.id(n));
    let lib = |n: &str| Some(m[n]);

    // the house's assemblies: (name, use, description, layers)
    use LayerFunction::*;
    let assemblies: Vec<(&str, AssemblyUse, &str, Vec<Layer>)> = vec![
        (
            "Exterior wall 400, rendered",
            AssemblyUse::Wall,
            "White render on mineral wool, a dense block inner leaf, plaster",
            vec![
                Layer::new(Finish, own("Render, white"), 0.02),
                Layer::new(Insulation, lib("Insulation, mineral wool"), 0.15),
                Layer::new(Structure, lib("Block, concrete"), 0.215),
                Layer::new(Finish, lib("Plaster, gypsum"), 0.015),
            ],
        ),
        (
            "Exterior wall 400, timber clad",
            AssemblyUse::Wall,
            "Oak boarding on battens, wood fibre insulation, block, plaster",
            vec![
                Layer::new(Finish, lib("Timber, oak"), 0.022),
                Layer::new(AirGap, None, 0.028),
                Layer::new(Insulation, lib("Insulation, wood fibre"), 0.135),
                Layer::new(Structure, lib("Block, concrete"), 0.2),
                Layer::new(Finish, lib("Plaster, gypsum"), 0.015),
            ],
        ),
        (
            "Stud partition 100",
            AssemblyUse::Wall,
            "Plasterboard both sides of an insulated timber stud",
            vec![
                Layer::new(Finish, lib("Plasterboard"), 0.0125),
                Layer::new(Structure, lib("Timber, softwood"), 0.075),
                Layer::new(Finish, lib("Plasterboard"), 0.0125),
            ],
        ),
        (
            "Ground floor 350, insulated",
            AssemblyUse::Floor,
            "Oak parquet on screed on PIR boards on a ground-bearing slab",
            vec![
                Layer::new(Finish, lib("Parquet, oak"), 0.015),
                Layer::new(Substrate, lib("Screed, cement"), 0.065),
                Layer::new(Insulation, lib("Insulation, PIR"), 0.1),
                Layer::new(Structure, lib("Concrete, cast in place"), 0.17),
            ],
        ),
        (
            "Upper floor 300, concrete",
            AssemblyUse::Floor,
            "Oak parquet on screed on acoustic insulation on a concrete slab",
            vec![
                Layer::new(Finish, lib("Parquet, oak"), 0.015),
                Layer::new(Substrate, lib("Screed, cement"), 0.06),
                Layer::new(Insulation, lib("Insulation, mineral wool"), 0.025),
                Layer::new(Structure, lib("Concrete, cast in place"), 0.2),
            ],
        ),
        (
            "Pitched roof 350, slate",
            AssemblyUse::Roof,
            "Slates on battens, membrane, wood fibre, rafters, plasterboard ceiling",
            vec![
                Layer::new(Finish, lib("Roof slate"), 0.012),
                Layer::new(AirGap, None, 0.038),
                Layer::new(Membrane, lib("Membrane, bitumen"), 0.002),
                Layer::new(Insulation, lib("Insulation, wood fibre"), 0.1),
                Layer::new(Structure, lib("Timber, softwood"), 0.185),
                Layer::new(Finish, lib("Plasterboard"), 0.013),
            ],
        ),
        (
            "Flat roof 400, green",
            AssemblyUse::Roof,
            "Sedum blanket on an EPDM membrane, PIR boards, concrete deck",
            vec![
                Layer::new(Finish, lib("Green roof"), 0.08),
                Layer::new(Membrane, lib("Membrane, EPDM"), 0.003),
                Layer::new(Insulation, lib("Insulation, PIR"), 0.137),
                Layer::new(Structure, lib("Concrete, cast in place"), 0.18),
            ],
        ),
    ];
    let mut tx = Transaction::new("Starter house: assemblies");
    let mut totals = HashMap::new();
    for (name, usage, description, layers) in assemblies {
        let mut a = Assembly::new(usage, layers);
        a.description = description.into();
        totals.insert(name, a.total());
        let e = Element::new(name, ElementKind::Assembly(a));
        tx = b.keep(name, e, tx);
    }
    b.commit(tx);

    let wall_type = |b: &Builder, a: &str| {
        TypeParams::Wall(WallType {
            thickness: totals[a],
            assembly: Some(b.id(a)),
            material: None,
            location: LocationLine::Centre,
            height: None,
            curtain: None,
            screen: None,
        })
    };
    let slab_type = |b: &Builder, a: &str| SlabType {
        thickness: totals[a],
        assembly: Some(b.id(a)),
        material: None,
    };
    let frames = Some(b.id("Aluminium, anthracite"));
    let window = |w: f64, h: f64, sill: f64, across: u32, up: u32| {
        TypeParams::Window(OpeningType {
            width: w,
            height: h,
            sill,
            frame: OpeningFrame {
                width: 0.06,
                depth: 0.09,
                panes_across: across,
                panes_up: up,
            },
            material: frames,
        })
    };
    let door = |w: f64, h: f64, material: Option<ElementId>, across: u32| {
        TypeParams::Door(OpeningType {
            width: w,
            height: h,
            sill: 0.0,
            frame: OpeningFrame {
                panes_across: across,
                ..Default::default()
            },
            material,
        })
    };
    let p = wall_type(b, "Exterior wall 400, rendered");
    b.add_type(
        "Exterior wall 400, rendered",
        "White render, the main walls",
        p,
    );
    let p = wall_type(b, "Exterior wall 400, timber clad");
    b.add_type(
        "Exterior wall 400, timber clad",
        "Oak boarding, the kitchen wing",
        p,
    );
    let p = wall_type(b, "Stud partition 100");
    b.add_type("Stud partition 100", "Bathroom partitions upstairs", p);
    b.add_type(
        "Garden glazing, 3 bays",
        "Three tall bays of glass in dark aluminium",
        TypeParams::Wall(WallType {
            thickness: 0.15,
            assembly: None,
            material: frames,
            location: LocationLine::Centre,
            height: None,
            curtain: Some(CurtainGrid {
                vertical: GridRule::Count(3),
                horizontal: GridRule::Count(1),
                mullion_width: 0.07,
                mullion_depth: 0.15,
                panel: PanelKind::Glass,
                ..Default::default()
            }),
            screen: None,
        }),
    );
    let p = TypeParams::Slab(slab_type(b, "Ground floor 350, insulated"));
    b.add_type("Ground floor 350, insulated", "", p);
    let p = TypeParams::Slab(slab_type(b, "Upper floor 300, concrete"));
    b.add_type("Upper floor 300, concrete", "", p);
    let paving = Some(m["Stone, slate"]);
    b.add_type(
        "Paving 150, slate",
        "Slate paving on a bed, for terraces and paths",
        TypeParams::Slab(SlabType {
            thickness: 0.15,
            assembly: None,
            material: paving,
        }),
    );
    let p = TypeParams::Roof(slab_type(b, "Pitched roof 350, slate"));
    b.add_type("Pitched roof 350, slate", "", p);
    let p = TypeParams::Roof(slab_type(b, "Flat roof 400, green"));
    b.add_type("Flat roof 400, green", "", p);
    let zinc = Some(m["Zinc"]);
    b.add_type(
        "Canopy 150, zinc",
        "A thin zinc-clad canopy",
        TypeParams::Roof(SlabType {
            thickness: 0.15,
            assembly: None,
            material: zinc,
        }),
    );
    let oak = Some(m["Timber, oak"]);
    let white = Some(m["Paint, white"]);
    b.add_type(
        "Front door 1000 × 2300",
        "Oak, solid",
        door(1.0, 2.3, oak, 1),
    );
    b.add_type(
        "Internal door 800 × 2100",
        "Painted",
        door(0.8, 2.1, white, 1),
    );
    b.add_type(
        "Glazed door 900 × 2300",
        "To the loggia and terrace",
        door(0.9, 2.3, frames, 1),
    );
    b.add_type("External door 900 × 2100", "", door(0.9, 2.1, frames, 1));
    b.add_type(
        "Window 1800 × 1500, three panes",
        "",
        window(1.8, 1.5, 0.75, 3, 1),
    );
    b.add_type(
        "Window 1200 × 1500, two panes",
        "",
        window(1.2, 1.5, 0.75, 2, 1),
    );
    b.add_type(
        "Window 1200 × 1000, kitchen",
        "Over a worktop",
        window(1.2, 1.0, 1.15, 2, 1),
    );
    b.add_type("Window 600 × 1500, slot", "", window(0.6, 1.5, 0.75, 1, 1));
    b.add_type(
        "Window 600 × 600, obscured",
        "WC and store",
        window(0.6, 0.6, 1.5, 1, 1),
    );
    b.add_type(
        "Glazed screen 1800 × 2300",
        "Floor to ceiling",
        window(1.8, 2.3, 0.05, 2, 1),
    );
    b.add_type(
        "Sidelight 500 × 2300",
        "Beside the front door",
        window(0.5, 2.3, 0.05, 1, 1),
    );
    let steel = Some(b.id("Steel, dark grey"));
    b.add_type(
        "Steel beam 200 × 300",
        "",
        TypeParams::Beam(BeamType {
            width: 0.2,
            depth: 0.3,
            material: steel,
        }),
    );
    b.add_type(
        "Steel column 250 × 250",
        "",
        TypeParams::Column(ColumnType {
            profile: Profile::Rectangle {
                width: 0.25,
                depth: 0.25,
            },
            material: steel,
        }),
    );
    let concrete = Some(m["Concrete, cast in place"]);
    let footing = |kind, width, length| {
        TypeParams::Footing(FootingType {
            kind,
            width,
            length,
            thickness: 0.45,
            material: concrete,
        })
    };
    b.add_type(
        "Strip footing 800 × 450",
        "Under the exterior walls",
        footing(FootingKind::Strip, 0.8, 0.0),
    );
    b.add_type(
        "Strip footing 600 × 450",
        "Under the block partitions",
        footing(FootingKind::Strip, 0.6, 0.0),
    );
    b.add_type(
        "Pad footing 1000 × 1000 × 450",
        "Under the loggia column",
        footing(FootingKind::Pad, 1.0, 1.0),
    );
    b.add_type(
        "Pad footing 600 × 600 × 450",
        "Under the terrace posts",
        footing(FootingKind::Pad, 0.6, 0.6),
    );
}

/// Grids, walls, floors, roofs, the stair, columns, beams and railings.
fn structure(b: &mut Builder) {
    let (g, f) = (b.id("Ground Floor"), b.id("First Floor"));
    // the eaves, 2.8 m above the first floor
    let eaves = 2.8;
    let mut tx = Transaction::new("Starter house: grids");
    for (label, a, bb) in [
        ("A", v(0.0, -1.2), v(0.0, 11.8)),
        ("B", v(4.0, -1.2), v(4.0, 11.8)),
        ("C", v(7.4, -1.2), v(7.4, 11.8)),
        ("D", v(11.0, -1.2), v(11.0, 11.8)),
        ("E", v(15.6, -1.2), v(15.6, 11.8)),
        ("1", v(-1.2, 0.0), v(17.0, 0.0)),
        ("2", v(-1.2, 5.6), v(17.0, 5.6)),
        ("3", v(-1.2, 9.6), v(17.0, 9.6)),
    ] {
        tx = tx.insert(Element::new(
            format!("Grid {label}"),
            ElementKind::Grid(Grid {
                label: label.into(),
                start: a,
                end: bb,
                group: None,
            }),
        ));
    }
    b.commit(tx);

    // the roofs first, so the walls under them can be attached
    let mut tx = Transaction::new("Starter house: roofs");
    let mut main = Roof::new(
        f,
        polygon::rectangle(v(-H, -H), v(11.0 + H, 9.6 + H)),
        RoofShape::Gable,
    );
    main.pitch = 35.0;
    main.overhang = 0.4;
    main.base_offset = eaves;
    let e = b.typed(
        Element::new("Main roof", ElementKind::Roof(main)),
        "Pitched roof 350, slate",
    );
    tx = b.keep("Main roof", e, tx);
    // the wing's roof stops at the main block and runs on over the terrace
    // the lower roofs hang off the first floor too, so the first floor
    // plan shows them below it and the roof plan shows every roof
    let mut wing = Roof::new(
        f,
        polygon::rectangle(v(11.0 + H, 4.6 - H - 0.2), v(15.6 + H + 0.2, 11.4)),
        RoofShape::Flat,
    );
    wing.overhang = 0.0;
    wing.base_offset = 0.0;
    let e = b.typed(
        Element::new("Wing roof", ElementKind::Roof(wing)),
        "Flat roof 400, green",
    );
    tx = b.keep("Wing roof", e, tx);
    let mut canopy = Roof::new(
        f,
        polygon::rectangle(v(4.9, -1.5), v(7.3, -H)),
        RoofShape::Flat,
    );
    canopy.overhang = 0.0;
    canopy.base_offset = -0.4;
    let e = b.typed(
        Element::new("Entrance canopy", ElementKind::Roof(canopy)),
        "Canopy 150, zinc",
    );
    tx = b.keep("Entrance canopy", e, tx);
    b.commit(tx);
    let (main_roof, wing_roof) = (b.id("Main roof"), b.id("Wing roof"));

    // Walls. Exterior walls run round each storey as long runs meeting in
    // corners; partitions end on the middle of the walls they meet, so
    // every join is an L or a T. Where a partition meets the loggia's
    // corner it is drawn to that wall's face and told not to join.
    let ext = "Exterior wall 400, rendered";
    let timber = "Exterior wall 400, timber clad";
    let block = "Interior wall 126, block";
    let stud = "Stud partition 100";
    struct W {
        key: &'static str,
        level: ElementId,
        a: DVec2,
        b: DVec2,
        kind: &'static str,
        no_join: [bool; 2],
    }
    let w = |key, level, a, bb, kind| W {
        key,
        level,
        a,
        b: bb,
        kind,
        no_join: [false; 2],
    };
    let hall_w = 4.0 + H - HB; // the hall's west wall, flush with the loggia's inner face
    let spine = 5.6 - H + HB; // the wall north of the hall, likewise
    let mut walls = vec![
        // ground floor, main block
        w("GF South", g, v(0.0, 0.0), v(11.0, 0.0), ext),
        w("GF East", g, v(11.0, 0.0), v(11.0, 9.6), ext),
        // the garden wall runs on along the wing, holding its glazing
        w("GF North", g, v(15.6, 9.6), v(4.0, 9.6), ext),
        w("GF Loggia East", g, v(4.0, 9.6), v(4.0, 5.6), ext),
        w("GF Loggia South", g, v(4.0, 5.6), v(0.0, 5.6), ext),
        w("GF West", g, v(0.0, 5.6), v(0.0, 0.0), ext),
        // the kitchen wing
        w("GF Wing South", g, v(11.0, 4.6), v(15.6, 4.6), timber),
        w("GF Wing East", g, v(15.6, 4.6), v(15.6, 9.6), timber),
        // partitions
        W {
            no_join: [false, true],
            ..w("GF Hall West", g, v(hall_w, 0.0), v(hall_w, 5.6 - H), block)
        },
        W {
            no_join: [true, false],
            ..w("GF Spine", g, v(4.0 + H, spine), v(11.0, spine), block)
        },
        w("GF Hall East", g, v(7.4, 0.0), v(7.4, spine), block),
        w("GF Utility South", g, v(7.4, 2.4), v(11.0, 2.4), block),
        w("GF WC East", g, v(9.0, 0.0), v(9.0, 2.4), block),
        // first floor
        w("FF South", f, v(0.0, 0.0), v(11.0, 0.0), ext),
        w("FF East", f, v(11.0, 0.0), v(11.0, 9.6), ext),
        w("FF North", f, v(11.0, 9.6), v(4.0, 9.6), ext),
        w("FF Loggia East", f, v(4.0, 9.6), v(4.0, 5.6), ext),
        w("FF Loggia South", f, v(4.0, 5.6), v(0.0, 5.6), ext),
        w("FF West", f, v(0.0, 5.6), v(0.0, 0.0), ext),
        W {
            no_join: [false, true],
            ..w(
                "FF Landing West",
                f,
                v(hall_w, 0.0),
                v(hall_w, 5.6 - H),
                block,
            )
        },
        W {
            no_join: [true, false],
            ..w("FF Spine", f, v(4.0 + H, spine), v(11.0, spine), block)
        },
        w("FF Landing East", f, v(7.4, 0.0), v(7.4, spine), block),
        w("FF Bathroom North", f, v(0.0, 3.4), v(hall_w, 3.4), stud),
        w("FF En-suite West", f, v(8.0, spine), v(8.0, 9.6), stud),
    ];
    let mut tx = Transaction::new("Starter house: walls");
    for spec in walls.drain(..) {
        let on_first = spec.level == f;
        let (height, top) = if on_first {
            (eaves, None)
        } else {
            (3.0, Some(f))
        };
        let mut wl = wall(spec.level, spec.a, spec.b, height, top);
        wl.disallow_join = spec.no_join;
        let wing_wall = spec.key.starts_with("GF Wing") || spec.key == "GF North";
        if on_first {
            // the gables fill in up to the roof and the eaves follow its slope
            wl.attach_top = vec![main_roof];
        } else if wing_wall {
            wl.attach_top = vec![wing_roof];
        }
        let e = b.typed(Element::new(spec.key, ElementKind::Wall(wl)), spec.kind);
        // a stamped wall takes the type's thickness; keep the storey heights
        let mut e = e;
        if let ElementKind::Wall(wl) = &mut e.kind {
            wl.height = height;
        }
        tx = b.keep(spec.key, e, tx);
    }
    // the garden glazing sits in the wing's stretch of the garden wall and
    // cuts its own opening there
    let mut cw = Wall::new(g, v(15.2, 9.6), v(11.6, 9.6), 0.15, 2.6);
    cw.material = None;
    let e = b.typed(
        Element::new("Garden glazing", ElementKind::Wall(cw)),
        "Garden glazing, 3 bays",
    );
    tx = b.keep("Garden glazing", e, tx);
    b.commit(tx);

    // Floors. The slabs reach the outer faces of the walls; the walls cut
    // through them where they overlap.
    let mut tx = Transaction::new("Starter house: floors");
    let gf = Slab::new(
        g,
        vec![
            v(-H, -H),
            v(11.0 + H, -H),
            v(11.0 + H, 4.6 - H),
            v(15.6 + H, 4.6 - H),
            v(15.6 + H, 9.6 + H),
            v(-H, 9.6 + H),
        ],
        0.35,
    );
    let e = b.typed(
        Element::new("GF Slab", ElementKind::Slab(gf)),
        "Ground floor 350, insulated",
    );
    tx = b.keep("GF Slab", e, tx);
    // the upper floor bears on the block of the walls under it, inside
    // their insulation and render, which run on past it; over the loggia
    // it reaches out to the balcony's edge
    let core = H - CORE_OUT;
    let ff_outline = vec![
        v(-core, -core),
        v(11.0 + core, -core),
        v(11.0 + core, 9.6 + core),
        v(4.0 - H, 9.6 + core),
        v(4.0 - H, 9.6 + H),
        v(-H, 9.6 + H),
        v(-H, 5.6 + H),
        v(-core, 5.6 + H),
    ];
    let mut ff = Slab::new(f, ff_outline, 0.3);
    // a riser for the soil stack and ducts, in the en-suite's corner
    ff.holes = vec![polygon::rectangle(v(10.35, 8.95), v(10.75, 9.35))];
    let e = b.typed(
        Element::new("FF Slab", ElementKind::Slab(ff)),
        "Upper floor 300, concrete",
    );
    tx = b.keep("FF Slab", e, tx);
    for (name, lo, hi) in [
        (
            "Garden terrace",
            v(11.0 + H, 9.6 + H),
            v(15.6 + H + 0.2, 11.4),
        ),
        ("Loggia steps", v(-H, 9.6 + H), v(4.0 - H, 10.6)),
        ("Front path", v(5.0, -4.0), v(6.6, -H)),
    ] {
        let mut s = Slab::new(g, polygon::rectangle(lo, hi), 0.15);
        s.top_offset = -0.05;
        let e = b.typed(
            Element::new(name, ElementKind::Slab(s)),
            "Paving 150, slate",
        );
        tx = b.keep(name, e, tx);
    }
    b.commit(tx);

    // The stair: up the hall's east wall, a landing in its corner, then
    // west along its north wall to the landing upstairs.
    let mut tx = Transaction::new("Starter house: stair");
    let east_face = 7.4 - HB;
    let mut st = Stair::new(g, v(east_face - 0.5, 1.9), 90.0);
    st.width = 1.0;
    st.tread = 0.25;
    st.max_riser = 0.18;
    st.landing = Some(Landing {
        after: 11,
        left: true,
    });
    st.material = Some(b.material("Timber, oak"));
    tx = b.keep("Stair", Element::new("Stair", ElementKind::Stair(st)), tx);
    // the well it climbs through, cutting the first floor
    let well = vec![
        v(east_face - 1.0, 1.9),
        v(east_face, 1.9),
        v(east_face, 5.6 - H),
        v(5.087, 5.6 - H),
        v(5.087, 4.4),
        v(east_face - 1.0, 4.4),
    ];
    let shaft = Element::new(
        "Stair Well",
        ElementKind::Shaft(Shaft::new(g, well, Some(f))),
    );
    tx = b.keep("Stair Well", shaft, tx);
    b.commit(tx);
    let steel = Some(b.id("Steel, dark grey"));
    let model = b.doc.model().clone();
    let mut tx = Transaction::new("Starter house: railings");
    if let Some(mut rail) = Railing::along_stair(&model, b.id("Stair"), true) {
        rail.material = steel;
        tx = tx.insert(Element::new("Stair handrail", ElementKind::Railing(rail)));
    }
    let z = DVec3::ZERO;
    let at = |x: f64, y: f64| DVec3::new(x, y, 0.0) + z;
    let mut balustrade = Railing::new(
        f,
        vec![
            at(east_face, 1.9),
            at(east_face - 1.0, 1.9),
            at(east_face - 1.0, 4.4),
            at(5.087, 4.4),
        ],
    );
    balustrade.material = steel;
    tx = tx.insert(Element::new(
        "Landing balustrade",
        ElementKind::Railing(balustrade),
    ));
    let mut balcony = Railing::new(
        f,
        vec![
            at(-0.12, 5.6 + H),
            at(-0.12, 9.6 + 0.12),
            at(4.0 - H, 9.6 + 0.12),
        ],
    );
    balcony.height = 1.1;
    balcony.material = steel;
    tx = tx.insert(Element::new(
        "Balcony railing",
        ElementKind::Railing(balcony),
    ));
    b.commit(tx);

    // The loggia's corner: a steel column from the ground up to the eaves,
    // through the balcony, carrying edge beams under the balcony and the
    // roof. Two slim posts hold the end of the wing's roof over the terrace.
    let mut tx = Transaction::new("Starter house: columns and beams");
    let corner = v(-H + 0.125, 9.6 + H - 0.125);
    let mut col = Column::new(
        g,
        corner,
        Profile::Rectangle {
            width: 0.25,
            depth: 0.25,
        },
    );
    col.top = WallTop::UpToLevel {
        level: f,
        offset: eaves,
    };
    col.material = steel;
    tx = b.keep(
        "Loggia column",
        Element::new("Loggia column", ElementKind::Column(col)),
        tx,
    );
    for (i, x) in [11.0 + H + 0.2, 15.6 + H].into_iter().enumerate() {
        let mut post = Column::new(g, v(x, 11.2), Profile::Round { diameter: 0.15 });
        post.top = WallTop::UpToLevel {
            level: f,
            offset: 0.0,
        };
        post.material = steel;
        let key = format!("Terrace post {}", i + 1);
        tx = b.keep(
            &key,
            Element::new(key.clone(), ElementKind::Column(post)),
            tx,
        );
    }
    for (key, level, a, bb, top_offset) in [
        (
            "Balcony beam north",
            g,
            v(corner.x + 0.125, corner.y),
            v(4.0 - H, corner.y),
            0.3,
        ),
        (
            "Balcony beam west",
            g,
            v(corner.x, corner.y - 0.125),
            v(corner.x, 5.6 + H),
            0.3,
        ),
        (
            "Eaves beam north",
            f,
            v(corner.x + 0.125, corner.y),
            v(4.0 - H, corner.y),
            0.0,
        ),
        (
            "Eaves beam west",
            f,
            v(corner.x, corner.y - 0.125),
            v(corner.x, 5.6 + H),
            0.0,
        ),
    ] {
        let mut bm = Beam::new(level, a, bb);
        bm.top_offset = top_offset;
        let e = b.typed(
            Element::new(key, ElementKind::Beam(bm)),
            "Steel beam 200 × 300",
        );
        tx = b.keep(key, e, tx);
    }
    b.commit(tx);
}

/// Doors, windows, the kitchen's opening and the garden door.
fn openings(b: &mut Builder) {
    use OpeningKind::{Door, Hole, Window};
    // (name, host wall, kind, type, offset from the wall's start, hinge on
    // the other jamb, open or face the other way)
    let list: &[(&str, &str, OpeningKind, &str, f64, bool, bool)] = &[
        // ground floor, street side
        (
            "Front door",
            "GF South",
            Door,
            "Front door 1000 × 2300",
            5.3,
            false,
            false,
        ),
        (
            "Sidelight",
            "GF South",
            Window,
            "Sidelight 500 × 2300",
            6.45,
            false,
            false,
        ),
        (
            "Study window",
            "GF South",
            Window,
            "Window 1800 × 1500, three panes",
            1.1,
            false,
            false,
        ),
        (
            "WC window",
            "GF South",
            Window,
            "Window 600 × 600, obscured",
            7.9,
            false,
            false,
        ),
        (
            "Store window",
            "GF South",
            Window,
            "Window 600 × 600, obscured",
            9.7,
            false,
            false,
        ),
        (
            "Study side window",
            "GF West",
            Window,
            "Window 1200 × 1500, two panes",
            2.2,
            false,
            false,
        ),
        (
            "Utility door",
            "GF East",
            Door,
            "External door 900 × 2100",
            2.6,
            true,
            false,
        ),
        // ground floor, garden side
        (
            "Dining screen",
            "GF North",
            Window,
            "Glazed screen 1800 × 2300",
            5.8,
            false,
            false,
        ),
        (
            "Living screen",
            "GF North",
            Window,
            "Glazed screen 1800 × 2300",
            8.8,
            false,
            false,
        ),
        (
            "Loggia door",
            "GF Loggia East",
            Door,
            "Glazed door 900 × 2300",
            0.5,
            false,
            false,
        ),
        (
            "Loggia screen",
            "GF Loggia East",
            Window,
            "Glazed screen 1800 × 2300",
            1.7,
            false,
            false,
        ),
        (
            "Study garden door",
            "GF Loggia South",
            Door,
            "Glazed door 900 × 2300",
            1.4,
            true,
            false,
        ),
        (
            "Kitchen window",
            "GF Wing East",
            Window,
            "Window 1200 × 1000, kitchen",
            1.9,
            false,
            false,
        ),
        (
            "Kitchen side window",
            "GF Wing South",
            Window,
            "Window 1200 × 1500, two panes",
            2.0,
            false,
            false,
        ),
        // ground floor, inside
        (
            "Study door",
            "GF Hall West",
            Door,
            "Internal door 800 × 2100",
            0.7,
            false,
            false,
        ),
        // the WC door opens into the WC, out of the way of the front door
        (
            "WC door",
            "GF Hall East",
            Door,
            "Internal door 800 × 2100",
            0.75,
            true,
            true,
        ),
        // hinged by the sofa, so the switches sit by its latch
        (
            "Hall door",
            "GF Spine",
            Door,
            "Internal door 800 × 2100",
            0.15,
            true,
            false,
        ),
        // its leaf clears the utility's worktop when it opens
        (
            "Utility inner door",
            "GF Spine",
            Door,
            "Internal door 800 × 2100",
            5.05,
            true,
            true,
        ),
        (
            "Store door",
            "GF Utility South",
            Door,
            "Internal door 800 × 2100",
            2.2,
            false,
            false,
        ),
        // first floor
        (
            "Bedroom 2 window",
            "FF South",
            Window,
            "Window 1800 × 1500, three panes",
            1.1,
            false,
            false,
        ),
        (
            "Landing window",
            "FF South",
            Window,
            "Window 1200 × 1500, two panes",
            5.2,
            false,
            false,
        ),
        (
            "Bedroom 3 window",
            "FF South",
            Window,
            "Window 1800 × 1500, three panes",
            8.3,
            false,
            false,
        ),
        (
            "Bedroom 2 side window",
            "FF West",
            Window,
            "Window 1200 × 1500, two panes",
            3.6,
            false,
            false,
        ),
        (
            "Bathroom window",
            "FF West",
            Window,
            "Window 600 × 1500, slot",
            0.8,
            false,
            false,
        ),
        (
            "Bedroom 3 side window",
            "FF East",
            Window,
            "Window 1200 × 1500, two panes",
            2.0,
            false,
            false,
        ),
        (
            "En-suite window",
            "FF East",
            Window,
            "Window 600 × 1500, slot",
            7.5,
            false,
            false,
        ),
        (
            "Bedroom 1 window",
            "FF North",
            Window,
            "Window 1800 × 1500, three panes",
            4.2,
            false,
            false,
        ),
        (
            "En-suite north window",
            "FF North",
            Window,
            "Window 600 × 1500, slot",
            0.9,
            false,
            false,
        ),
        (
            "Balcony door",
            "FF Loggia East",
            Door,
            "Glazed door 900 × 2300",
            0.9,
            false,
            false,
        ),
        (
            "Bedroom 1 screen",
            "FF Loggia East",
            Window,
            "Glazed screen 1800 × 2300",
            2.0,
            false,
            false,
        ),
        (
            "Bedroom 2 door",
            "FF Landing West",
            Door,
            "Internal door 800 × 2100",
            2.5,
            true,
            false,
        ),
        (
            "Bathroom door",
            "FF Landing West",
            Door,
            "Internal door 800 × 2100",
            3.9,
            false,
            false,
        ),
        (
            "Bedroom 1 door",
            "FF Spine",
            Door,
            "Internal door 800 × 2100",
            0.12,
            false,
            false,
        ),
        (
            "Bedroom 3 door",
            "FF Landing East",
            Door,
            "Internal door 800 × 2100",
            0.7,
            false,
            true,
        ),
        (
            "En-suite door",
            "FF En-suite West",
            Door,
            "Internal door 800 × 2100",
            0.6,
            false,
            true,
        ),
    ];
    let mut tx = Transaction::new("Starter house: doors and windows");
    for &(name, host, kind, t, offset, hand, facing) in list {
        let mut o = opening(b.id(host), kind, offset, 1.0);
        o.flip_hand = hand;
        o.flip_facing = facing;
        let e = b.typed(Element::new(name, ElementKind::Opening(o)), t);
        tx = b.keep(name, e, tx);
    }
    // the dining room opens wide into the kitchen wing through the old
    // exterior wall
    let mut hole = opening(b.id("GF East"), Hole, 5.9, 3.2);
    hole.height = 2.4;
    tx = b.keep(
        "Kitchen opening",
        Element::new("Kitchen opening", ElementKind::Opening(hole)),
        tx,
    );
    // a garden door in the glazing's middle bay
    let bay = 3.6 / 3.0;
    let mut gd = opening(b.id("Garden glazing"), Door, bay + 0.035, bay - 0.07);
    gd.height = 2.3;
    gd.material = Some(b.id("Aluminium, anthracite"));
    tx = b.keep(
        "Garden door",
        Element::new("Garden door", ElementKind::Opening(gd)),
        tx,
    );
    b.commit(tx);
}

/// Strips under the ground floor's walls and pads under the columns, their
/// tops at the underside of the ground floor slab.
fn footings(b: &mut Builder) {
    let g = b.id("Ground Floor");
    let model = b.doc.model().clone();
    let hosts: Vec<ElementId> = model
        .iter()
        .filter(|e| match &e.kind {
            ElementKind::Wall(w) => {
                w.level == g && w.curtain.is_none() && !e.name.contains("Partition")
            }
            ElementKind::Column(_) => true,
            _ => false,
        })
        .map(|e| e.id)
        .collect();
    let ext = b.id("Strip footing 800 × 450");
    let int = b.id("Strip footing 600 × 450");
    let big = b.id("Pad footing 1000 × 1000 × 450");
    let small = b.id("Pad footing 600 × 600 × 450");
    let loggia = b.id("Loggia column");
    let (mut tx, _) = footing::under(&model, &hosts, &|e| {
        let host = match &e.kind {
            ElementKind::Footing(f) => f.host,
            _ => None,
        };
        let t = match (&e.kind, host.and_then(|h| model.get(h))) {
            (ElementKind::Footing(f), _) if f.kind == FootingKind::Pad => {
                if host == Some(loggia) {
                    big
                } else {
                    small
                }
            }
            (_, Some(h)) if matches!(&h.kind, ElementKind::Wall(w) if w.thickness > 0.3) => ext,
            _ => int,
        };
        types::stamp(&model, e, t);
        if let ElementKind::Footing(f) = &mut e.kind {
            f.below = 0.35;
        }
    });
    tx.label = "Starter house: footings".into();
    b.commit(tx);
}

/// Furniture and fixtures from the starter catalogue.
fn furnish(b: &mut Builder) {
    let (g, f) = (b.id("Ground Floor"), b.id("First Floor"));
    // (object, level, a point near where it goes, its turn when it is not
    // placed against the nearest wall)
    let plan: &[(&str, ElementId, DVec2, Option<f64>)] = &[
        // living, at the west end of the garden room
        ("sofa-3", g, v(6.4, 5.6), None),
        ("coffee-table", g, v(6.4, 7.0), Some(0.0)),
        ("armchair", g, v(4.75, 7.3), Some(90.0)),
        // dining, by the garden screen
        ("dining-table", g, v(8.85, 7.6), Some(0.0)),
        ("chair", g, v(8.25, 7.0), Some(180.0)),
        ("chair", g, v(9.45, 7.0), Some(180.0)),
        ("chair", g, v(8.25, 8.2), Some(0.0)),
        ("chair", g, v(9.45, 8.2), Some(0.0)),
        ("chair", g, v(7.75, 7.6), Some(90.0)),
        ("chair", g, v(9.95, 7.6), Some(270.0)),
        // kitchen in the wing: a run along the east wall, the sink under
        // its window, and an island
        ("fridge-freezer", g, v(15.3, 5.2), None),
        ("base-cabinet-600", g, v(15.3, 5.8), None),
        ("cooker-600", g, v(15.3, 6.4), None),
        ("base-cabinet-600", g, v(15.3, 7.0), None),
        ("sink-unit-1000", g, v(15.3, 7.8), None),
        ("base-cabinet-600", g, v(15.3, 8.6), None),
        ("wall-cabinet-600", g, v(15.3, 5.8), None),
        ("base-cabinet-600", g, v(13.0, 6.6), Some(90.0)),
        ("base-cabinet-600", g, v(13.0, 7.2), Some(90.0)),
        ("base-cabinet-600", g, v(13.0, 7.8), Some(90.0)),
        // study
        ("desk", g, v(2.0, 0.3), None),
        ("chair", g, v(2.0, 1.15), Some(0.0)),
        ("armchair", g, v(3.3, 4.6), Some(315.0)),
        ("wardrobe", g, v(0.3, 4.0), None),
        // WC and utility
        ("wc", g, v(8.2, 2.25), None),
        ("washbasin", g, v(8.85, 1.2), None),
        ("sink-unit-1000", g, v(10.75, 4.3), None),
        ("base-cabinet-600", g, v(10.75, 5.1), None),
        // bedroom 1 (north) and its en-suite
        ("double-bed", f, v(6.5, 5.6), None),
        ("bedside-table", f, v(5.45, 5.6), None),
        ("bedside-table", f, v(7.55, 5.6), None),
        ("wardrobe", f, v(7.85, 8.0), None),
        ("shower", f, v(10.3, 6.1), None),
        ("wc", f, v(8.2, 8.5), None),
        ("washbasin", f, v(8.9, 9.4), None),
        // bedroom 2 (south-west) and the bathroom
        ("double-bed", f, v(1.9, 3.3), None),
        ("bedside-table", f, v(0.85, 3.3), None),
        ("bedside-table", f, v(2.95, 3.3), None),
        ("wardrobe", f, v(3.5, 0.25), None),
        ("bathtub", f, v(1.2, 5.4), None),
        ("wc", f, v(2.9, 3.5), None),
        ("washbasin", f, v(3.6, 5.4), None),
        // bedroom 3 (east)
        ("single-bed", f, v(10.8, 4.4), None),
        ("bedside-table", f, v(10.8, 3.6), None),
        ("desk", f, v(8.2, 5.4), None),
        ("chair", f, v(8.2, 4.55), Some(180.0)),
        ("wardrobe", f, v(7.55, 2.05), None),
        // services: the board and boiler in the utility, switches by the
        // doors, sockets round the rooms, lights centred in them
        ("consumer-unit", g, v(9.2, 3.0), None),
        ("boiler", g, v(7.5, 3.4), None),
        ("switch-2-gang", g, v(5.0, 0.25), None),
        ("switch-1-gang", g, v(4.25, 1.75), None),
        ("switch-1-gang", g, v(4.27, 5.3), None),
        ("switch-2-gang", g, v(4.27, 5.6), None),
        ("switch-1-gang", g, v(7.3, 1.75), None),
        ("switch-1-gang", g, v(10.4, 5.4), None),
        ("socket-double", g, v(0.25, 1.0), None),
        ("socket-double", g, v(3.0, 0.25), None),
        ("socket-double", g, v(3.2, 5.35), None),
        ("socket-double", g, v(7.8, 5.6), None),
        ("socket-double", g, v(4.25, 5.8), None),
        ("socket-double", g, v(10.75, 5.7), None),
        ("socket-double", g, v(15.3, 7.3), None),
        ("socket-double", g, v(15.3, 9.0), None),
        ("socket-double", g, v(12.0, 4.8), None),
        ("socket-single", g, v(10.75, 3.7), None),
        ("ceiling-light", g, v(2.1, 2.7), Some(0.0)),
        ("ceiling-light", g, v(5.8, 7.5), Some(0.0)),
        ("ceiling-light", g, v(8.85, 7.6), Some(0.0)),
        ("ceiling-light", g, v(5.3, 3.0), Some(0.0)),
        ("downlight", g, v(13.0, 6.0), Some(0.0)),
        ("downlight", g, v(13.0, 8.4), Some(0.0)),
        ("downlight", g, v(14.6, 6.0), Some(0.0)),
        ("downlight", g, v(14.6, 8.4), Some(0.0)),
        ("downlight", g, v(8.2, 1.3), Some(0.0)),
        ("downlight", g, v(9.6, 3.8), Some(0.0)),
        ("smoke-detector", g, v(5.0, 4.0), Some(0.0)),
        ("smoke-detector", g, v(12.4, 7.2), Some(0.0)),
        ("radiator", g, v(2.0, 0.25), None),
        ("radiator", g, v(0.25, 2.8), None),
        ("radiator", g, v(7.4, 9.35), None),
        ("radiator", g, v(12.3, 4.8), None),
        ("radiator", g, v(4.25, 2.6), None),
        ("extract-fan", g, v(8.7, 0.25), None),
        ("extract-fan", g, v(15.3, 8.0), None),
        ("extract-fan", g, v(10.75, 2.6), None),
        ("outside-tap", g, v(15.85, 9.0), None),
        ("outside-tap", g, v(11.25, 3.8), None),
        ("switch-1-gang", f, v(4.25, 2.15), None),
        ("switch-1-gang", f, v(4.25, 4.85), None),
        ("switch-1-gang", f, v(5.22, 5.6), None),
        ("switch-1-gang", f, v(7.5, 1.6), None),
        ("switch-1-gang", f, v(5.3, 5.4), None),
        ("socket-double", f, v(5.5, 5.6), None),
        ("socket-double", f, v(7.4, 5.6), None),
        ("socket-double", f, v(0.6, 3.25), None),
        ("socket-double", f, v(3.4, 3.25), None),
        ("socket-double", f, v(10.8, 3.2), None),
        ("socket-double", f, v(9.2, 5.35), None),
        ("socket-double", f, v(6.0, 0.25), None),
        ("wall-light", f, v(5.45, 5.6), None),
        ("wall-light", f, v(7.55, 5.6), None),
        ("ceiling-light", f, v(6.5, 7.4), Some(0.0)),
        ("ceiling-light", f, v(2.0, 1.0), Some(0.0)),
        ("ceiling-light", f, v(9.2, 2.7), Some(0.0)),
        ("ceiling-light", f, v(5.4, 1.0), Some(0.0)),
        ("downlight", f, v(9.6, 6.6), Some(0.0)),
        ("downlight", f, v(9.6, 8.4), Some(0.0)),
        ("downlight", f, v(2.4, 4.3), Some(0.0)),
        ("smoke-detector", f, v(5.4, 2.8), Some(0.0)),
        ("extract-fan", f, v(10.8, 8.4), None),
        ("extract-fan", f, v(0.25, 4.0), None),
        ("floor-drain", f, v(10.3, 6.1), Some(0.0)),
        ("radiator", f, v(2.0, 0.25), None),
        ("radiator", f, v(9.2, 0.25), None),
        ("radiator", f, v(5.8, 9.35), None),
        ("radiator", f, v(1.1, 3.5), None),
        ("radiator", f, v(9.7, 5.7), None),
        ("supply-diffuser", f, v(5.0, 8.8), Some(0.0)),
        ("extract-grille", f, v(0.9, 4.2), Some(0.0)),
    ];
    for (key, lvl, at, turn) in plan {
        let c = crate::catalogue::starter(&format!("starter/{key}")).expect("a starter");
        let (p, r) = match turn {
            // lights and diffusers by their middle
            Some(r) if c.object.mount == Mount::Ceiling => {
                (crate::objects::centred(&c.object, *at, *r), *r)
            }
            Some(r) => (crate::objects::centred(&c.object, *at, *r), *r),
            // against the nearest wall, clear of its doors and windows
            None => {
                let spot = crate::objects::against_wall_clear(
                    b.doc.model(),
                    *lvl,
                    *at,
                    0.8,
                    &c.object,
                    c.object.offset,
                )
                .unwrap_or_else(|| panic!("a wall near {key} at {at}"));
                assert!(!spot.blocked, "{key} at {at} fits clear of the doors");
                (spot.at, spot.rotation)
            }
        };
        let (tx, _) = crate::objects::place(b.doc.model(), &c, *lvl, p, r);
        b.commit(tx);
    }
}

/// The rooms, each found from the walls round its point (the loggia,
/// terrace and balcony, which no wall closes off, drawn), and the
/// ceilings: plasterboard over every room upstairs, which the partitions
/// there stop at instead of running up into the roof, and a suspended
/// tile ceiling over the ground floor's services rooms.
fn rooms_and_ceilings(b: &mut Builder) {
    let (g, f) = (b.id("Ground Floor"), b.id("First Floor"));
    let loggia = polygon::rectangle(v(-H, 5.6 + H), v(4.0 - H, 9.6 + H));
    // (level, number, name, its point, a drawn outline, occupancy, floor finish)
    type Entry<'a> = (
        ElementId,
        &'a str,
        &'a str,
        DVec2,
        Vec<DVec2>,
        &'a str,
        &'a str,
    );
    #[rustfmt::skip]
    let list: Vec<Entry> = vec![
        (g, "0.01", "Study", v(2.0, 3.5), vec![], "Study", "Oak boards"),
        (g, "0.02", "Hall", v(5.0, 1.7), vec![], "Circulation", "Slate tiles"),
        (g, "0.03", "WC", v(7.95, 0.75), vec![], "Sanitary", "Ceramic tiles"),
        (g, "0.04", "Store", v(10.0, 1.2), vec![], "Storage", "Screed, sealed"),
        (g, "0.05", "Utility", v(8.6, 4.4), vec![], "Utility", "Ceramic tiles"),
        (g, "0.06", "Living / Dining", v(9.0, 6.3), vec![], "Living", "Oak boards"),
        (g, "0.07", "Kitchen", v(13.4, 5.4), vec![], "Kitchen", "Slate tiles"),
        (g, "0.08", "Loggia", v(1.9, 7.6), loggia.clone(), "Outdoor", "Slate paving"),
        (
            g,
            "0.09",
            "Terrace",
            v(12.1, 10.75),
            polygon::rectangle(v(11.0 + H, 9.6 + H), v(15.6 + H + 0.2, 11.4)),
            "Outdoor",
            "Slate paving",
        ),
        (f, "1.01", "Bedroom 2", v(3.35, 1.9), vec![], "Bedroom", "Carpet"),
        (f, "1.02", "Bathroom", v(1.6, 3.95), vec![], "Sanitary", "Ceramic tiles"),
        (f, "1.03", "Landing", v(5.3, 3.2), vec![], "Circulation", "Oak boards"),
        (f, "1.04", "Bedroom 3", v(9.2, 1.5), vec![], "Bedroom", "Carpet"),
        (f, "1.05", "Bedroom 1", v(6.5, 8.7), vec![], "Bedroom", "Carpet"),
        (f, "1.06", "En-suite", v(9.4, 7.5), vec![], "Sanitary", "Ceramic tiles"),
        (f, "1.07", "Balcony", v(1.9, 7.6), loggia, "Outdoor", "Timber decking"),
    ];
    let mut tx = Transaction::new("Starter house: rooms");
    for (level, number, name, at, outline, occupancy, floor) in list {
        let outdoor = occupancy == "Outdoor";
        let mut r = Room::new(level, at);
        r.number = number.into();
        r.boundary = outline;
        r.occupancy = occupancy.into();
        r.floor_finish = floor.into();
        if outdoor {
            r.department = "Outdoor".into();
            // open to the sky, or to the balcony over it
            r.height = Some(if level == g { 2.7 } else { 2.4 });
        } else {
            r.wall_finish = if occupancy == "Sanitary" {
                "Ceramic tiles to 1200".into()
            } else {
                "Plaster, painted".into()
            };
            r.ceiling_finish = "Plaster, painted".into();
        }
        let key = format!("Room {number}");
        tx = b.keep(&key, Element::new(name, ElementKind::Room(r)), tx);
    }
    b.commit(tx);

    let board = b.material("Plasterboard");
    let mut tx = Transaction::new("Starter house: ceilings");
    let mut upstairs = vec![];
    for number in ["1.01", "1.02", "1.03", "1.04", "1.05", "1.06"] {
        let room = b.id(&format!("Room {number}"));
        let mut c = Ceiling::in_room(f, room, 2.5);
        c.material = Some(board);
        let e = Element::new("Plasterboard ceiling", ElementKind::Ceiling(c));
        upstairs.push(e.id);
        tx = tx.insert(e);
    }
    for number in ["0.03", "0.04", "0.05"] {
        let room = b.id(&format!("Room {number}"));
        let mut c = Ceiling::in_room(g, room, 2.4);
        c.thickness = 0.02;
        c.grid = 0.6;
        c.material = Some(board);
        tx = tx.insert(Element::new(
            "Suspended tile ceiling",
            ElementKind::Ceiling(c),
        ));
    }
    b.commit(tx);

    // the partitions upstairs stop at the ceilings
    for key in [
        "FF Landing West",
        "FF Spine",
        "FF Landing East",
        "FF Bathroom North",
        "FF En-suite West",
    ] {
        let id = b.id(key);
        b.doc
            .edit(id, "Starter house: partitions to the ceiling", |e| {
                if let ElementKind::Wall(w) = &mut e.kind {
                    w.attach_top = upstairs.clone();
                }
            })
            .expect("the partitions take the ceilings");
    }
}

/// The views of the set, by what the sheets call them.
struct Views {
    ground: ElementId,
    first: ElementId,
    foundation: ElementId,
    roof: ElementId,
    sections: [ElementId; 2],
    elevations: [ElementId; 4],
    street: ElementId,
    garden: ElementId,
    schedules: Vec<ElementId>,
}

fn views(b: &mut Builder) -> Views {
    use visibility::Category as C;
    let mut tx = Transaction::new("Starter house: views");
    let plan = |level: ElementId, scale: f64, order: u32, cut: f64, hide: &[C]| {
        let mut v = View::plan(level);
        v.scale = scale;
        v.browser_order = Some(order);
        if let ViewKind::Plan { cut_height, .. } = &mut v.kind {
            *cut_height = cut;
        }
        v.hidden.categories = hide.to_vec();
        v
    };
    let add = |b: &mut Builder, tx: Transaction, name: &str, v: View| {
        let e = Element::new(name, ElementKind::View(v));
        b.keep(name, e, tx)
    };
    tx = add(
        b,
        tx,
        "Ground Floor plan",
        plan(b.id("Ground Floor"), 50.0, 0, 1.2, &[C::Footings]),
    );
    tx = add(
        b,
        tx,
        "First Floor plan",
        plan(b.id("First Floor"), 50.0, 1, 1.2, &[]),
    );
    // the foundation plan cuts through the footings: everything else on
    // the ground floor shows faintly as an underlay to set them out from
    let mut foundation = plan(b.id("Foundation"), 50.0, 2, 0.25, &[]);
    foundation.underlay = Underlay::Level(b.id("Ground Floor"));
    tx = add(b, tx, "Foundation plan", foundation);
    // the roof plan looks down on the roofs from above the ridge
    let below = [
        C::Walls,
        C::Doors,
        C::Windows,
        C::Slabs,
        C::Objects,
        C::Railings,
        C::Stairs,
        C::Beams,
        C::Columns,
        C::Openings,
    ];
    tx = add(
        b,
        tx,
        "Roof plan",
        plan(b.id("First Floor"), 100.0, 3, 7.5, &below),
    );
    for (name, a, bb, depth, cut, style) in [
        (
            "Section A-A",
            v(6.6, 12.2),
            v(6.6, -4.6),
            10.0,
            true,
            ViewStyle::HiddenLine,
        ),
        (
            "Section B-B",
            v(-3.4, 7.4),
            v(18.0, 7.4),
            3.0,
            true,
            ViewStyle::HiddenLine,
        ),
        (
            "South Elevation",
            v(-1.5, -5.2),
            v(17.1, -5.2),
            30.0,
            false,
            ViewStyle::Shaded,
        ),
        (
            "East Elevation",
            v(17.8, -2.4),
            v(17.8, 11.6),
            30.0,
            false,
            ViewStyle::Shaded,
        ),
        (
            "North Elevation",
            v(17.5, 12.3),
            v(-1.1, 12.3),
            30.0,
            false,
            ViewStyle::Shaded,
        ),
        (
            "West Elevation",
            v(-3.6, 12.2),
            v(-3.6, -1.5),
            30.0,
            false,
            ViewStyle::Shaded,
        ),
    ] {
        let mut view = View::section(a, bb, depth, cut);
        view.scale = 100.0;
        view.style = style;
        if cut {
            view.scale = 50.0;
            // the stair well's outline would run through the drawing
            view.hidden.categories = vec![C::Openings];
        } else {
            // nothing below the ground in an elevation
            view.hidden.categories = vec![C::Openings, C::Footings];
        }
        tx = add(b, tx, name, view);
    }
    // the 3D views show the house above ground, without the stair well's
    // outline
    let mut street = View::axonometric(40.0, 28.0);
    street.scale = 100.0;
    street.hidden.categories = vec![C::Openings, C::Footings];
    tx = add(b, tx, "3D Street view", street);
    let mut garden = View::axonometric(212.0, 26.0);
    garden.scale = 100.0;
    garden.hidden.categories = vec![C::Openings, C::Footings];
    tx = add(b, tx, "3D Garden view", garden);
    let mut schedules = vec![];
    for category in [
        ScheduleCategory::Doors,
        ScheduleCategory::Windows,
        ScheduleCategory::Walls,
        ScheduleCategory::Structure,
        ScheduleCategory::Furniture,
        ScheduleCategory::Fixtures,
        ScheduleCategory::Rooms,
    ] {
        let name = category.schedule_name();
        tx = add(b, tx, &name, View::schedule(category));
        schedules.push(b.id(&name));
    }
    b.commit(tx);
    Views {
        ground: b.id("Ground Floor plan"),
        first: b.id("First Floor plan"),
        foundation: b.id("Foundation plan"),
        roof: b.id("Roof plan"),
        sections: [b.id("Section A-A"), b.id("Section B-B")],
        elevations: [
            b.id("South Elevation"),
            b.id("East Elevation"),
            b.id("North Elevation"),
            b.id("West Elevation"),
        ],
        street: b.id("3D Street view"),
        garden: b.id("3D Garden view"),
        schedules,
    }
}

/// How the house is detailed: a wall section through the study and the
/// bedroom over it, three details called out from it at 1:10 (the wall's
/// base and foundation, the window's head and sill, the eaves), drawn
/// from the model with detail components for what it does not show, and
/// the junctions set by hand where the upper floor bears on the walls.
fn details(b: &mut Builder) {
    use visibility::Category as C;
    // the first floor bears on the block of the walls under it: the slab
    // cuts through them, where by layer priority their block would win
    let mut tx = Transaction::new("Starter house: junctions");
    for wall in ["GF South", "GF East", "GF North", "GF West"] {
        let mut rule = JoinRule::new(b.id("FF Slab"), b.id(wall));
        rule.winner = JoinWinner::First;
        tx = tx.insert(Element::new("Junction", ElementKind::Join(rule)));
    }
    b.commit(tx);

    // the wall section: north to south through the study window,
    // looking east, so the outside is on the right
    let mut tx = Transaction::new("Starter house: wall section");
    let mut section = View::section(v(2.0, 2.5), v(2.0, -2.5), 0.9, true);
    section.scale = 25.0;
    section.hidden.categories = vec![C::Openings, C::Objects];
    let parent = section.clone();
    tx = b.keep(
        "Section C-C",
        Element::new("Section C-C", ElementKind::View(section)),
        tx,
    );
    b.commit(tx);
    let sid = b.id("Section C-C");
    let mut tx = Transaction::new("Starter house: details");
    for (name, lo, hi) in DETAIL_BOXES {
        let d = View::callout(sid, &parent, v(lo.0, lo.1), v(hi.0, hi.1), 10.0);
        tx = b.keep(name, Element::new(*name, ElementKind::View(d)), tx);
    }
    b.commit(tx);

    // what the model doesn't show, drafted in each detail
    let mut tx = Transaction::new("Starter house: detail components");
    for (detail, kind, width, pts) in detail_components() {
        let mut item = DetailItem::new(b.id(detail), kind, pts);
        item.width = width;
        tx = tx.insert(Element::new(kind.label(), ElementKind::Detail(item)));
    }
    for (detail, at, text, leader) in detail_notes() {
        let mut n = Note::new(b.id(detail), at, text);
        n.leader = leader.map(HostPoint::free);
        tx = tx.insert(Element::new("Note", ElementKind::Note(n)));
    }
    b.commit(tx);
}

/// A detail view: its name and the corners of its callout box.
type DetailBox = (&'static str, (f64, f64), (f64, f64));
/// The details called out from the wall section, and their boxes in its
/// drawing (metres along the section, and height).
const DETAIL_BOXES: &[DetailBox] = &[
    ("Detail 1 Wall base", (1.85, -0.95), (3.35, 0.45)),
    ("Detail 2 Window", (1.95, 0.5), (3.15, 2.6)),
    ("Detail 3 Eaves", (1.85, 5.2), (3.45, 6.5)),
];

/// The detail components in each detail: (detail, kind, width, path).
/// In the section's drawing the wall's inner face is at 2.3 and its outer
/// face at 2.7 along it, the outside to the right.
fn detail_components() -> Vec<(&'static str, DetailKind, f64, Vec<DVec2>)> {
    use DetailKind as K;
    let (base, window, eaves) = (DETAIL_BOXES[0].0, DETAIL_BOXES[1].0, DETAIL_BOXES[2].0);
    let pts = |p: &[(f64, f64)]| p.iter().map(|&(x, y)| v(x, y)).collect::<Vec<_>>();
    let region = |h: Hatch| K::Region { hatch: h };
    vec![
        // the wall's base: a damp-proof course under it, a membrane under
        // the floor's insulation, rigid insulation down the slab's edge,
        // and paving outside
        (base, K::Membrane, 0.0, pts(&[(2.3, 0.0), (2.8, 0.0)])),
        (base, K::Membrane, 0.0, pts(&[(1.85, -0.18), (2.7, -0.18)])),
        (
            base,
            region(Hatch::Cross),
            0.0,
            pts(&[(2.7, 0.0), (2.8, 0.0), (2.8, -0.35), (2.7, -0.35)]),
        ),
        (
            base,
            K::Repeat {
                module: 0.3,
                joint: 0.01,
            },
            0.04,
            pts(&[(2.8, -0.13), (3.35, -0.13)]),
        ),
        (base, K::BreakLine, 0.0, pts(&[(1.88, 0.08), (1.88, -0.42)])),
        (base, K::BreakLine, 0.0, pts(&[(2.24, 0.42), (2.78, 0.42)])),
        // the window: a precast lintel over it, flashings at its head and
        // sill, an oak window board inside
        (
            window,
            region(Hatch::Concrete),
            0.0,
            pts(&[(2.315, 2.25), (2.53, 2.25), (2.53, 2.4), (2.315, 2.4)]),
        ),
        (
            window,
            K::Flashing,
            0.003,
            pts(&[(2.54, 2.252), (2.72, 2.252), (2.72, 2.22)]),
        ),
        (
            window,
            K::Flashing,
            0.003,
            pts(&[(2.54, 0.722), (2.77, 0.712), (2.77, 0.68)]),
        ),
        (
            window,
            K::Membrane,
            0.0,
            pts(&[(2.45, 0.735), (2.69, 0.735)]),
        ),
        (
            window,
            K::Blocking,
            0.022,
            pts(&[(2.27, 0.76), (2.45, 0.76)]),
        ),
        (
            window,
            K::BreakLine,
            0.0,
            pts(&[(2.24, 2.55), (2.78, 2.55)]),
        ),
        (
            window,
            K::BreakLine,
            0.0,
            pts(&[(2.24, 0.56), (2.78, 0.56)]),
        ),
        // the eaves: fascia, gutter and soffit, and insulation closing the
        // eaves over the wall's head
        (
            eaves,
            K::Blocking,
            0.025,
            pts(&[(3.115, 5.47), (3.115, 5.76)]),
        ),
        (
            eaves,
            K::Flashing,
            0.002,
            pts(&[(3.135, 5.63), (3.135, 5.5), (3.27, 5.5), (3.3, 5.64)]),
        ),
        (eaves, K::Blocking, 0.015, pts(&[(2.7, 5.49), (3.1, 5.49)])),
        (eaves, K::Batt, 0.08, pts(&[(2.72, 5.56), (2.9, 5.56)])),
        (eaves, K::BreakLine, 0.0, pts(&[(2.24, 5.26), (2.78, 5.26)])),
    ]
}

/// The notes in each detail: (detail, where, text, leader point).
fn detail_notes() -> Vec<(&'static str, DVec2, &'static str, Option<DVec2>)> {
    let (base, window, eaves) = (DETAIL_BOXES[0].0, DETAIL_BOXES[1].0, DETAIL_BOXES[2].0);
    vec![
        (
            base,
            v(3.0, 0.36),
            "20 RENDER\n150 MINERAL WOOL\n215 CONCRETE BLOCK\n15 PLASTER",
            Some(v(2.6, 0.3)),
        ),
        (base, v(3.0, 0.06), "DPC", Some(v(2.76, 0.0))),
        (
            base,
            v(3.0, -0.26),
            "PERIMETER INSULATION",
            Some(v(2.76, -0.25)),
        ),
        (base, v(3.0, -0.07), "PAVING", Some(v(3.1, -0.12))),
        (
            base,
            v(3.0, -0.6),
            "STRIP FOOTING\nC30/37, 800 × 450",
            Some(v(2.86, -0.6)),
        ),
        (
            base,
            v(1.9, 0.28),
            "PARQUET, SCREED\n100 PIR ON DPM\n170 SLAB",
            Some(v(2.0, -0.1)),
        ),
        (window, v(2.85, 2.45), "PRECAST LINTEL", Some(v(2.45, 2.33))),
        (window, v(2.85, 2.2), "HEAD FLASHING", Some(v(2.71, 2.245))),
        (
            window,
            v(2.85, 1.5),
            "TIMBER WINDOW\nTRIPLE GLAZED",
            Some(v(2.5, 1.5)),
        ),
        (
            window,
            v(2.85, 0.68),
            "SILL FLASHING\nWITH DRIP",
            Some(v(2.77, 0.69)),
        ),
        (
            window,
            v(1.98, 0.86),
            "OAK WINDOW BOARD",
            Some(v(2.33, 0.77)),
        ),
        (
            eaves,
            v(2.9, 6.42),
            "SLATE ON BATTENS\nBREATHER MEMBRANE\n100 WOOD FIBRE\n185 RAFTERS",
            Some(v(2.6, 6.2)),
        ),
        (eaves, v(3.25, 5.82), "TIMBER FASCIA", Some(v(3.12, 5.7))),
        (eaves, v(3.25, 5.4), "GUTTER", Some(v(3.25, 5.52))),
        (eaves, v(2.9, 5.32), "VENTED SOFFIT", Some(v(2.95, 5.48))),
        (eaves, v(1.9, 5.7), "EAVES INSULATION", Some(v(2.8, 5.56))),
    ]
}

/// Room tags, notes, door and window tags, and dimensions.
fn annotate(b: &mut Builder, views: &Views) {
    let mut tx = Transaction::new("Starter house: annotation styles");
    let room_style = TextStyle {
        size: 3.0,
        bold: true,
        purpose: StyleUse::Notes,
        ..Default::default()
    };
    tx = b.keep(
        "Room name",
        Element::new("Room name", ElementKind::TextStyle(room_style)),
        tx,
    );
    b.commit(tx);
    let room = Some(b.id("Room name"));

    // Rooms: a tag in the middle of each, its name, number and the floor
    // area inside the finishes, read live from the walls round it
    let mut tx = Transaction::new("Starter house: room tags");
    let model = b.doc.model().clone();
    for e in model.iter() {
        let ElementKind::Room(r) = &e.kind else {
            continue;
        };
        let view = if r.level == b.id("First Floor") {
            views.first
        } else {
            views.ground
        };
        let mut t = Tag::new(view, e.id, DVec2::ZERO);
        t.style = room;
        tx = tx.insert(Element::new("Room tag", ElementKind::Tag(t)));
    }
    b.commit(tx);

    // what the foundation and roof plans need saying, with leaders to
    // what they are about
    let notes: [(ElementId, DVec2, &str, Option<DVec2>); 7] = [
        (
            views.foundation,
            v(12.2, 2.9),
            "STRIP FOOTINGS\nC30/37 concrete, 450 deep\n800 wide under exterior walls\n600 wide under partitions\nTop -350, underside -800",
            Some(v(11.2, 3.4)),
        ),
        (
            views.foundation,
            v(-3.6, 11.6),
            "PAD 1000 × 1000\nunder the loggia column",
            Some(v(-0.4, 10.0)),
        ),
        (
            views.foundation,
            v(12.6, 12.4),
            "PADS 600 × 600\nunder the terrace posts",
            Some(v(11.8, 11.4)),
        ),
        // the slope arrows give the pitch and the fall
        (views.roof, v(2.4, 3.3), "SLATE\nnatural slate on battens", None),
        (views.roof, v(0.6, 8.8), "SLATE\nnatural slate on battens", None),
        (views.roof, v(12.4, 7.6), "GREEN ROOF\nsedum on EPDM\nfalls 1:60 to the east", None),
        (views.roof, v(4.4, -2.4), "ZINC CANOPY", Some(v(5.6, -0.9))),
    ];
    let mut tx = Transaction::new("Starter house: notes");
    for (view, at, text, leader) in notes {
        let mut n = Note::new(view, at, text);
        n.leader = leader.map(HostPoint::free);
        tx = tx.insert(Element::new("Note", ElementKind::Note(n)));
    }
    b.commit(tx);

    // Tags: every door and window in its plan, outside the house for the
    // exterior walls, in the room for the partitions.
    let model = b.doc.model().clone();
    let mut tx = Transaction::new("Starter house: tags");
    for e in model.iter() {
        let ElementKind::Opening(o) = &e.kind else {
            continue;
        };
        if o.kind == OpeningKind::Hole {
            continue;
        }
        let Some(ElementKind::Wall(w)) = model.get(o.host).map(|h| &h.kind) else {
            continue;
        };
        let view = if w.level == b.id("First Floor") {
            views.first
        } else {
            views.ground
        };
        // the exterior is to a wall's right; doors are tagged on the side
        // they open to, windows outside
        let right = -w.direction().perp();
        let exterior = w.thickness > 0.3 || w.curtain.is_some();
        let out = if exterior {
            right * 0.95
        } else {
            let left = !o.faces_left(w);
            (if left { right } else { -right }) * 0.55
        };
        tx = tx.insert(Element::new(
            "Tag",
            ElementKind::Tag(Tag::new(view, e.id, out)),
        ));
    }
    b.commit(tx);

    // Dimensions on the plans: the openings, the grids and the overall
    // size along the street and the east side, outside the walls.
    let on = |host: ElementId, u: f64, s: f64, from_end: bool| HostPoint {
        host: Some(host),
        local: DVec2::new(u, s),
        from_end,
        body: None,
    };
    let dim = |view: ElementId, a: HostPoint, bb: HostPoint, offset: f64| {
        Element::new(
            "Dimension",
            ElementKind::Dimension(Dimension::linear(view, a, bb, offset)),
        )
    };
    let face = -H;
    let mut tx = Transaction::new("Starter house: dimensions");
    for (view, south, east, prefix) in [
        (views.ground, b.id("GF South"), b.id("GF East"), "GF"),
        (views.first, b.id("FF South"), b.id("FF East"), "FF"),
    ] {
        // the street front's openings, as a chain
        let mut chain = vec![on(south, -H, face, false)];
        let mut on_south: Vec<(f64, ElementId, f64)> = model
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Opening(o) if o.host == south => Some((o.offset, e.id, o.width)),
                _ => None,
            })
            .collect();
        on_south.sort_by(|a, c| a.0.total_cmp(&c.0));
        for (_, id, w) in on_south {
            chain.push(on(id, 0.0, face, false));
            chain.push(on(id, w, face, false));
        }
        chain.push(on(south, H, face, true));
        for w in chain.windows(2) {
            tx = tx.insert(dim(view, w[0], w[1], -1.5));
        }
        // the grids along the front, then the whole length
        for (u0, u1) in [
            (-H, 0.0),
            (0.0, 4.0),
            (4.0, 7.4),
            (7.4, 11.0),
            (11.0, 11.0 + H),
        ] {
            let a = on(south, u0, face, false);
            let bb = on(south, u1, face, false);
            tx = tx.insert(dim(view, a, bb, -2.3));
        }
        tx = tx.insert(dim(
            view,
            on(south, -H, face, false),
            on(south, H, face, true),
            -3.1,
        ));
        // the east side: grids and overall depth
        for (u0, u1) in [(-H, 0.0), (0.0, 5.6), (5.6, 9.6), (9.6, 9.6 + H)] {
            let a = on(east, u0, face, false);
            let bb = on(east, u1, face, false);
            let off = if prefix == "GF" { -5.3 } else { -1.5 };
            tx = tx.insert(dim(view, a, bb, off));
        }
        let off = if prefix == "GF" { -6.1 } else { -2.3 };
        tx = tx.insert(dim(
            view,
            on(east, -H, face, false),
            on(east, H, face, true),
            off,
        ));
    }
    // the west side, up past the loggia to the column's outer face
    for (view, west) in [
        (views.ground, b.id("GF West")),
        (views.first, b.id("FF West")),
        (views.foundation, b.id("GF West")),
    ] {
        let free = |y: f64| HostPoint::free(v(-H, y));
        let chain = [
            on(west, H, face, true),
            on(west, 0.0, face, true),
            on(west, 0.0, face, false),
            free(9.6),
            free(9.6 + H),
        ];
        for w in chain.windows(2) {
            tx = tx.insert(dim(view, w[0], w[1], 1.5));
        }
        tx = tx.insert(dim(view, chain[0], chain[4], 2.3));
    }
    // the foundation plan: the grids and overall length along the front
    let south = b.id("GF South");
    for (u0, u1) in [
        (-H, 0.0),
        (0.0, 4.0),
        (4.0, 7.4),
        (7.4, 11.0),
        (11.0, 11.0 + H),
    ] {
        tx = tx.insert(dim(
            views.foundation,
            on(south, u0, face, false),
            on(south, u1, face, false),
            -1.5,
        ));
    }
    tx = tx.insert(dim(
        views.foundation,
        on(south, -H, face, false),
        on(south, H, face, true),
        -2.3,
    ));
    // the wing's length along its south wall
    let wing = b.id("GF Wing South");
    tx = tx.insert(dim(
        views.ground,
        on(wing, -H, face, false),
        on(wing, H, face, true),
        -1.5,
    ));
    // storey heights beside the sections
    for section in views.sections {
        let lv = |name: &str, h: f64| HostPoint {
            host: Some(b.id(name)),
            local: DVec2::new(0.8, h),
            from_end: false,
            body: None,
        };
        for (a, ha, c, hc) in [
            ("Foundation", 0.0, "Ground Floor", 0.0),
            ("Ground Floor", 0.0, "First Floor", 0.0),
            ("First Floor", 0.0, "First Floor", 2.8),
        ] {
            tx = tx.insert(dim(section, lv(a, ha), lv(c, hc), 0.0));
        }
    }
    b.commit(tx);
}

/// The sheet set and its title block.
fn sheets(b: &mut Builder, views: &Views) {
    let mut tx = Transaction::new("Starter house: title block");
    tx = b.keep(
        "Maple Lane title block",
        Element::new(
            "Maple Lane title block",
            ElementKind::SheetTemplate(title_block()),
        ),
        tx,
    );
    b.commit(tx);
    let template = b.id("Maple Lane title block");

    // (number, title, paper, views: (view, where on the sheet (mm), the
    // drawing point pinned there, crop))
    type Place = (ElementId, DVec2, DVec2, Option<[DVec2; 2]>);
    let crop = |lo: (f64, f64), hi: (f64, f64)| Some([v(lo.0, lo.1), v(hi.0, hi.1)]);
    let plan_crop = crop((-5.0, -5.6), (19.5, 13.4));
    let sched = &views.schedules;
    let set: Vec<(&str, &str, &str, Vec<Place>)> = vec![
        (
            "A000",
            "Cover",
            "A3",
            vec![(views.garden, v(235.0, 200.0), v(0.0, 0.0), None)],
        ),
        (
            "A101",
            "Ground Floor Plan",
            "A2",
            vec![(views.ground, v(287.0, 222.0), v(7.8, 4.0), plan_crop)],
        ),
        (
            "A102",
            "First Floor Plan",
            "A2",
            vec![(views.first, v(287.0, 222.0), v(7.8, 4.0), plan_crop)],
        ),
        (
            "A103",
            "Foundation Plan",
            "A2",
            vec![(views.foundation, v(287.0, 222.0), v(7.8, 4.0), plan_crop)],
        ),
        (
            "A104",
            "Roof Plan",
            "A3",
            vec![(
                views.roof,
                v(160.0, 172.0),
                v(7.8, 4.8),
                crop((-4.0, -4.0), (19.0, 13.0)),
            )],
        ),
        (
            "A201",
            "Section A-A",
            "A2",
            vec![(views.sections[0], v(280.0, 235.0), v(9.6, 4.8), None)],
        ),
        (
            "A202",
            "Section B-B",
            "A2",
            vec![(views.sections[1], v(290.0, 245.0), v(12.2, 3.6), None)],
        ),
        (
            "A301",
            "Elevations",
            "A2",
            vec![
                (views.elevations[0], v(150.0, 300.0), v(10.3, 3.0), None),
                (views.elevations[2], v(400.0, 300.0), v(10.3, 3.0), None),
                (views.elevations[1], v(150.0, 140.0), v(9.0, 3.0), None),
                (views.elevations[3], v(400.0, 140.0), v(9.0, 3.0), None),
            ],
        ),
        (
            "A501",
            "Details",
            "A2",
            vec![
                (
                    b.id("Section C-C"),
                    v(100.0, 235.0),
                    v(2.3, 2.8),
                    crop((0.8, -1.0), (3.8, 6.7)),
                ),
                (
                    b.id(DETAIL_BOXES[0].0),
                    v(290.0, 115.0),
                    v(2.6, -0.25),
                    None,
                ),
                (
                    b.id(DETAIL_BOXES[1].0),
                    v(470.0, 255.0),
                    v(2.55, 1.55),
                    None,
                ),
                (
                    b.id(DETAIL_BOXES[2].0),
                    v(290.0, 320.0),
                    v(2.65, 5.85),
                    None,
                ),
            ],
        ),
        (
            "A601",
            "Door, Window and Structure Schedules",
            "A2",
            vec![
                (sched[0], v(35.0, 400.0), DVec2::ZERO, None),
                (sched[1], v(35.0, 270.0), DVec2::ZERO, None),
                (sched[3], v(320.0, 400.0), DVec2::ZERO, None),
            ],
        ),
        (
            "A602",
            "Furniture, Fixture and Wall Schedules",
            "A2",
            vec![
                (sched[4], v(35.0, 400.0), DVec2::ZERO, None),
                (sched[2], v(35.0, 245.0), DVec2::ZERO, None),
                (sched[5], v(320.0, 400.0), DVec2::ZERO, None),
            ],
        ),
        (
            "A603",
            "Room Schedule",
            "A3",
            vec![(sched[6], v(35.0, 270.0), DVec2::ZERO, None)],
        ),
        (
            "A901",
            "3D Views",
            "A2",
            vec![
                (views.street, v(150.0, 215.0), DVec2::ZERO, None),
                (views.garden, v(420.0, 225.0), DVec2::ZERO, None),
            ],
        ),
    ];
    let _ = views.elevations;
    let mut tx = Transaction::new("Starter house: sheets");
    for (number, title, paper, placed) in set {
        let mut sheet = Sheet::a(number, paper);
        sheet.template = Some(template);
        let e = Element::new(title, ElementKind::Sheet(sheet));
        let id = e.id;
        tx = b.keep(number, e, tx);
        for (view, center, target, crop) in placed {
            tx = tx.insert(Element::new(
                "Viewport",
                ElementKind::Viewport(Viewport {
                    sheet: id,
                    view,
                    center,
                    target,
                    crop,
                }),
            ));
        }
    }
    b.commit(tx);

    // the cover: the project's name large, the drawing list and a few
    // words on the house, written on the sheet clear of the 3D view
    let mut tx = Transaction::new("Starter house: cover styles");
    for (name, size) in [("Cover title", 9.0), ("Cover heading", 3.5)] {
        let style = TextStyle {
            size,
            bold: true,
            purpose: StyleUse::Notes,
            ..Default::default()
        };
        tx = b.keep(name, Element::new(name, ElementKind::TextStyle(style)), tx);
    }
    b.commit(tx);
    let cover = b.id("A000");
    let title = Some(b.id("Cover title"));
    let heading = Some(b.id("Cover heading"));
    let notes = [
        (v(30.0, 272.0), PROJECT.to_string(), title),
        (
            v(30.0, 259.0),
            format!("{ADDRESS}  ·  a new family house for {CLIENT}"),
            heading,
        ),
        (v(30.0, 92.0), "DRAWINGS".to_string(), heading),
        (
            v(30.0, 84.0),
            "A000  Cover\nA101  Ground Floor Plan  1:50\nA102  First Floor Plan  1:50\n\
             A103  Foundation Plan  1:50\nA104  Roof Plan  1:100\nA201  Section A-A  1:50\nA202  Section B-B  1:50\n\
             A301  Elevations  1:100\nA501  Details  1:25, 1:10\nA601  Door, Window and Structure Schedules\nA602  Furniture, Fixture and Wall Schedules\nA603  Room Schedule\nA901  3D Views"
                .to_string(),
            None,
        ),
        (v(115.0, 92.0), "THE HOUSE".to_string(), heading),
        (
            v(115.0, 84.0),
            "Two storeys and a single-storey kitchen wing\n\
             Gross floor area about 220 m²\n\
             Walls: rendered and timber-clad block, 400 mm\n\
             Roofs: slate at 35°, green flat roof on the wing\n\
             Every drawing here follows the model:\n\
             change it and the sheets update"
                .to_string(),
            None,
        ),
    ];
    let mut tx = Transaction::new("Starter house: cover notes");
    for (at, text, style) in notes {
        let mut note = Note::new(cover, at, text);
        note.style = style;
        tx = tx.insert(Element::new("Note", ElementKind::Note(note)));
    }
    b.commit(tx);
}

/// A title block 180 × 62 mm in the bottom right corner, with the
/// project's own fields.
fn title_block() -> SheetTemplate {
    let (x0, x1, y0, y1) = (-190.0, -10.0, 10.0, 72.0);
    let line = |pts: &[(f64, f64)], width: f64| TitleItem::Line {
        points: pts.iter().map(|&(x, y)| v(x, y)).collect(),
        width,
    };
    let text = |x: f64, y: f64, size: f64, t: &str| TitleItem::Text {
        at: v(x, y),
        size,
        text: t.into(),
        align: TextAlign::Start,
        angle: 0.0,
    };
    let mut items = vec![
        line(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)], 0.5),
        // a light band across the top with the project's name
        TitleItem::Fill {
            points: vec![v(x0, y1 - 14.0), v(x1, y1 - 14.0), v(x1, y1), v(x0, y1)],
            gray: 0.9,
        },
        line(&[(x0, y1 - 14.0), (x1, y1 - 14.0)], 0.25),
        line(&[(x0, 46.0), (x1, 46.0)], 0.25),
        line(&[(x0, 30.0), (x1, 30.0)], 0.25),
        line(&[(x0 + 60.0, 46.0), (x0 + 60.0, y1 - 14.0)], 0.18),
        line(&[(x0 + 60.0, y0), (x0 + 60.0, 30.0)], 0.18),
        line(&[(x0 + 100.0, y0), (x0 + 100.0, 30.0)], 0.18),
        line(&[(x0 + 140.0, y0), (x0 + 140.0, 30.0)], 0.18),
        line(&[(x0 + 100.0, 20.0), (x1, 20.0)], 0.18),
    ];
    let label = 1.8;
    items.extend([
        text(x0 + 3.0, y1 - 5.0, 4.5, "{job}"),
        text(x0 + 3.0, y1 - 10.5, 2.5, "{address}"),
        text(x0 + 3.0, 54.0, label, "CLIENT"),
        text(x0 + 3.0, 49.5, 2.8, "{client}"),
        text(x0 + 63.0, 54.0, label, "STATUS"),
        text(x0 + 63.0, 49.5, 2.8, "{status}"),
        text(x0 + 3.0, 42.5, label, "SHEET"),
        text(x0 + 3.0, 35.5, 5.0, "{sheet}"),
        text(x0 + 3.0, 26.5, label, "NUMBER"),
        text(x0 + 3.0, 17.0, 8.0, "{number}"),
        text(x0 + 63.0, 26.5, label, "SCALE"),
        text(x0 + 63.0, 20.0, 3.0, "{scale}"),
        text(x0 + 63.0, 13.5, 2.2, "{date}"),
        text(x0 + 103.0, 26.5, label, "REVISION"),
        text(x0 + 103.0, 22.5, 2.8, "{revision}"),
        text(x0 + 103.0, 16.5, label, "DRAWN"),
        text(x0 + 103.0, 12.5, 2.5, "{drawn}"),
        text(x0 + 143.0, 26.5, label, "PAPER"),
        text(x0 + 143.0, 22.5, 2.8, "ISO A"),
        text(x0 + 143.0, 14.5, 3.0, "CreaBIM"),
    ]);
    SheetTemplate {
        corner: Corner::BottomRight,
        items,
        border: true,
        fields: vec![
            ("job".into(), PROJECT.into()),
            ("address".into(), ADDRESS.into()),
            ("client".into(), CLIENT.into()),
            ("status".into(), "PLANNING APPLICATION".into()),
            ("revision".into(), "A".into()),
            ("drawn".into(), "CreaBIM starter".into()),
        ],
        paper: None,
        guides: vec![],
        margins: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_starter_house_builds() {
        let m = starter_house();
        assert!(m.len() > 300);
        for e in m.iter() {
            assert!(e.validate().is_ok(), "{}: {:?}", e.name, e.validate());
        }
    }

    fn count(m: &Model, f: impl Fn(&ElementKind) -> bool) -> usize {
        m.iter().filter(|e| f(&e.kind)).count()
    }

    #[test]
    fn the_starter_house_shows_the_features_a_new_user_looks_for() {
        use ElementKind as K;
        let m = starter_house();
        assert_eq!(count(&m, |k| matches!(k, K::Level(_))), 3);
        assert_eq!(count(&m, |k| matches!(k, K::Grid(_))), 8);
        assert_eq!(
            count(&m, |k| matches!(k, K::Wall(w) if w.curtain.is_none())),
            24
        );
        assert_eq!(
            count(&m, |k| matches!(k, K::Wall(w) if w.curtain.is_some())),
            1
        );
        let openings =
            |kind: OpeningKind| count(&m, |k| matches!(k, K::Opening(o) if o.kind == kind));
        assert_eq!(openings(OpeningKind::Door), 16);
        assert_eq!(openings(OpeningKind::Window), 20);
        assert_eq!(openings(OpeningKind::Hole), 1);
        assert!(count(&m, |k| matches!(k, K::Opening(o) if o.flip_hand)) > 0);
        assert!(count(&m, |k| matches!(k, K::Opening(o) if o.flip_facing)) > 0);
        assert_eq!(count(&m, |k| matches!(k, K::Roof(_))), 3);
        assert_eq!(count(&m, |k| matches!(k, K::Slab(_))), 5);
        assert!(count(&m, |k| matches!(k, K::Slab(s) if !s.holes.is_empty())) > 0);
        assert_eq!(count(&m, |k| matches!(k, K::Stair(_))), 1);
        assert_eq!(count(&m, |k| matches!(k, K::Shaft(_))), 1);
        assert_eq!(count(&m, |k| matches!(k, K::Railing(_))), 3);
        assert_eq!(count(&m, |k| matches!(k, K::Column(_))), 3);
        assert_eq!(count(&m, |k| matches!(k, K::Beam(_))), 4);
        let footings =
            |kind: FootingKind| count(&m, |k| matches!(k, K::Footing(f) if f.kind == kind));
        assert_eq!(footings(FootingKind::Pad), 3);
        assert_eq!(footings(FootingKind::Strip), 13);
        assert!(count(&m, |k| matches!(k, K::Object(_))) >= 100);
        assert!(count(&m, |k| matches!(k, K::Dimension(_))) >= 40);
        assert!(count(&m, |k| matches!(k, K::Tag(_))) >= 36);
        // every door and window carries a type
        for e in m.iter() {
            if let K::Opening(o) = &e.kind {
                if o.kind != OpeningKind::Hole && e.name != "Garden door" {
                    assert!(e.of_type.is_some(), "{} has no type", e.name);
                }
            }
        }
    }

    #[test]
    fn the_starter_rooms_are_closed_off_by_their_walls() {
        let m = starter_house();
        let rooms: Vec<(&Element, &Room)> = m
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Room(r) => Some((e, r)),
                _ => None,
            })
            .collect();
        assert_eq!(rooms.len(), 16);
        let area = |name: &str| {
            let (e, _) = rooms.iter().find(|(e, _)| e.name == name).unwrap();
            room::area(&m, e.id)
        };
        for (e, r) in &rooms {
            let s = room::shape_of(&m, r).unwrap_or_else(|| panic!("{} is enclosed", e.name));
            assert!(s.area() > 1.0, "{} has an area", e.name);
            // the walls bound every room but the outdoor ones
            assert_eq!(
                r.is_drawn(),
                r.occupancy == "Outdoor",
                "{} is drawn only if no wall closes it off",
                e.name
            );
            if !r.is_drawn() {
                assert!(s.bounded_by.iter().all(|b| b.is_some()), "{}", e.name);
            }
        }
        // to the inner faces of the walls: 3.937 × 5.0 for the study
        assert!((area("Study") - (4.0 + H - BLOCK - H) * (5.6 - 2.0 * H)).abs() < 1e-6);
        // the landing less the stair well
        let landing = (7.4 - HB - 4.0 - H) * (5.6 - 2.0 * H);
        assert!(area("Landing") < landing - 2.0);
        // ceilings: over every room upstairs and the services rooms below,
        // and the upstairs partitions stop at them
        assert_eq!(
            count(&m, |k| matches!(k, ElementKind::Ceiling(_))),
            9,
            "ceilings"
        );
        let spine = m.iter().find(|e| e.name == "FF Spine").unwrap();
        let ElementKind::Wall(w) = &spine.kind else {
            unreachable!()
        };
        let top = walls::top_profile(&m, w).expect("attached to the ceilings");
        let z = top.z_at((w.start + w.end) / 2.0).unwrap();
        assert!((z - (3.0 + 2.5)).abs() < 1e-9, "{z}");
        // lights upstairs hang from the ceilings
        let light = m
            .iter()
            .find(|e| {
                matches!(&e.kind, ElementKind::Object(o) if o.level == b_id(&m, "First Floor"))
                    && objects::hung(&m, e)
            })
            .unwrap();
        let (_, hi) = objects::type_of(&m, light).unwrap().bounds();
        let top = objects::base_z(&m, light).unwrap() + hi.z;
        assert!((top - 5.5).abs() < 0.2, "{top}");
    }

    fn b_id(m: &Model, name: &str) -> ElementId {
        m.iter().find(|e| e.name == name).unwrap().id
    }

    #[test]
    fn nothing_in_the_starter_house_stands_in_a_doorway() {
        let m = starter_house();
        let clashes = checks::objects_in_openings(&m);
        let named: Vec<(String, String)> = clashes
            .iter()
            .map(|c| {
                (
                    m.get(c.object).unwrap().name.clone(),
                    m.get(c.opening).unwrap().name.clone(),
                )
            })
            .collect();
        assert!(named.is_empty(), "{named:?}");
    }

    #[test]
    fn every_starter_sheet_has_its_title_block_and_views() {
        let m = starter_house();
        let sheets: Vec<(ElementId, &Sheet)> = m
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Sheet(s) => Some((e.id, s)),
                _ => None,
            })
            .collect();
        let numbers: Vec<&str> = sheets.iter().map(|(_, s)| s.number.as_str()).collect();
        for n in [
            "A000", "A101", "A102", "A103", "A104", "A201", "A202", "A301", "A501", "A601", "A602",
            "A603", "A901",
        ] {
            assert!(numbers.contains(&n), "no sheet {n}");
        }
        for (id, s) in &sheets {
            assert!(s.template.is_some(), "{} has no title block", s.number);
            let placed = m
                .iter()
                .filter(|e| matches!(&e.kind, ElementKind::Viewport(v) if v.sheet == *id))
                .count();
            assert!(placed > 0, "{} has no views", s.number);
        }
    }
}
