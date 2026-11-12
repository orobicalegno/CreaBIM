//! IFC5 exchange in the alpha IFCX JSON format (experimental).
//!
//! An IFCX file is a list of nodes, each naming a `path` and adding
//! `children` (name → path), `inherits` (name → path) and namespaced
//! `attributes`. Several entries may describe the same path; later ones win.
//! A node's composed view is what it inherits, overlaid with its own
//! entries. The tree under the roots carries USD transforms
//! (`usd::xformop`) and meshes (`usd::usdgeom::mesh`); objects are the nodes
//! with a `bsi::ifc::class`. This follows buildingSMART's IFC5-development
//! examples (`ifcxVersion` "ifcx_alpha"), which may still change.
//!
//! On export every element also carries a `creabim::element` attribute with
//! its exact CreaBIM parameters, so CreaBIM files round-trip losslessly
//! while other tools see ordinary classes, meshes and properties. On import
//! files without it are read from their geometry: straight walls with an
//! axis, slabs on a storey and doors and windows in such walls become native
//! elements, everything else a free mesh that keeps its class.

use crate::import::{
    build_model, hatch_for, opening_in_wall, prism_of_mesh, HostWall, ImportError, ImportResult,
};
use crate::shape::Warnings;
use crate::tess::{bounds, merge_coplanar, weld};
use creabim_core::{
    EditMesh, Element, ElementId, ElementKind, FreeMesh, Level, Material, Model, Opening,
    OpeningKind, Slab, Wall,
};
use glam::{DMat4, DVec2, DVec3, DVec4};
use indexmap::IndexMap;
use serde_json::{json, Map, Value as J};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const CLASS: &str = "bsi::ifc::class";
const MESH: &str = "usd::usdgeom::mesh";
const CURVES: &str = "usd::usdgeom::basiscurves";
const XFORM: &str = "usd::xformop";
const VISIBILITY: &str = "usd::usdgeom::visibility";
const MATERIAL: &str = "bsi::ifc::material";
const COLOR: &str = "bsi::ifc::presentation::diffuseColor";
const OPACITY: &str = "bsi::ifc::presentation::opacity";
/// CreaBIM's own parameters for an element: `{"name": …, "kind": …}`.
const CREABIM: &str = "creabim::element";
const CLASS_URI: &str = "https://identifier.buildingsmart.org/uri/buildingsmart/ifc/4.3/class/";

// ---------------------------------------------------------------- export

/// Write a model as an IFCX (IFC5 alpha) JSON file: project → site →
/// building → storeys → elements, each with its class, mesh, transform,
/// name and a few properties.
pub fn export_ifcx(model: &Model) -> String {
    let scene: HashMap<ElementId, creabim_geom::TriMesh> =
        creabim_geom::scene::element_meshes(&creabim_geom::scene::build_scene_for_exchange(model));
    let mut nodes: Vec<J> = Vec::new();
    // ids for nodes CreaBIM has no element for, stable across exports
    let base = model.levels().next().map_or(ElementId::NIL, |(e, _)| e.id);
    let project = derived(base, 0xA1);
    let site = derived(base, 0xA2);
    let building = derived(base, 0xA3);

    let mut building_children = Names::default();
    let mut level_children: HashMap<ElementId, Names> = HashMap::new();
    let mut wall_children: HashMap<ElementId, Names> = HashMap::new();
    let mut group_children: HashMap<ElementId, Names> = HashMap::new();
    let elevation = |level: Option<ElementId>| {
        level
            .and_then(|l| model.level(l))
            .map_or(0.0, |l| l.elevation)
    };

    for e in model.iter() {
        let path = path_of(e.id);
        let mut attrs = Map::new();
        let mut children = Names::default();
        let mut inherits = Map::new();
        // a grid group is the IfcGrid; its lines are its axes
        let grouped = match &e.kind {
            ElementKind::Grid(g) => creabim_core::grids::group_of(model, g).map(|(ge, _)| ge.id),
            _ => None,
        };
        let class = match grouped {
            Some(_) => "IfcGridAxis".to_string(),
            None => e.ifc_class().to_string(),
        };
        let mut creabim = json!({ "name": e.name, "kind": e.kind });
        if let Some(t) = &e.of_type {
            creabim["type"] = json!(t);
        }
        let (parent, frame_z) = match &e.kind {
            ElementKind::Level(l) => {
                attrs.insert(XFORM.into(), xform(DVec3::new(0.0, 0.0, l.elevation)));
                attrs.insert("bsi::ifc::prop::Elevation".into(), json!(l.elevation));
                (Parent::Building, None)
            }
            ElementKind::Wall(w) => {
                let base = w.base_offset;
                let axis = derived(e.id, 0x2);
                children.add("Axis", &axis);
                nodes.push(node(
                    &axis,
                    Names::default(),
                    Map::new(),
                    attributes([(
                        CURVES,
                        json!({ "points": [
                            [w.start.x, w.start.y, base],
                            [w.end.x, w.end.y, base]
                        ] }),
                    )]),
                ));
                attrs.insert("bsi::ifc::prop::Height".into(), json!(w.height));
                attrs.insert("bsi::ifc::prop::Width".into(), json!(w.thickness));
                attrs.insert("bsi::ifc::prop::Length".into(), json!(w.length()));
                (Parent::Level(w.level), Some(elevation(Some(w.level))))
            }
            ElementKind::Slab(s) => {
                attrs.insert("bsi::ifc::prop::Depth".into(), json!(s.thickness));
                (Parent::Level(s.level), Some(elevation(Some(s.level))))
            }
            ElementKind::Opening(o) => {
                attrs.insert("bsi::ifc::prop::Height".into(), json!(o.height));
                attrs.insert("bsi::ifc::prop::Width".into(), json!(o.width));
                let z = match model.get(o.host).map(|h| &h.kind) {
                    Some(ElementKind::Wall(w)) => elevation(Some(w.level)),
                    _ => 0.0,
                };
                (Parent::Wall(o.host), Some(z))
            }
            ElementKind::FreeMesh(f) => {
                attrs.insert(XFORM.into(), xform(f.origin));
                let z = elevation(f.level) + f.origin.z;
                let at = DVec3::new(f.origin.x, f.origin.y, z);
                // the body is relative to the element's own transform
                if let Some(mesh) = scene.get(&e.id) {
                    add_body(&mut nodes, &mut children, e.id, mesh, at);
                }
                (f.level.map_or(Parent::Building, Parent::Level), None)
            }
            ElementKind::GridGroup(_) => (Parent::Building, None),
            // rooms and separators have no body; a ceiling has its slab
            ElementKind::Room(r) => (Parent::Level(r.level), None),
            ElementKind::RoomSeparator(s) => (Parent::Level(s.level), None),
            ElementKind::Ceiling(c) => (Parent::Level(c.level), Some(elevation(Some(c.level)))),
            ElementKind::Grid(g) => {
                attrs.insert(
                    "bsi::ifc::prop::AxisTag".into(),
                    json!(creabim_core::grids::display_label(model, g)),
                );
                let axis = derived(e.id, 0x2);
                children.add("Axis", &axis);
                nodes.push(node(
                    &axis,
                    Names::default(),
                    Map::new(),
                    attributes([(
                        CURVES,
                        json!({ "points": [[g.start.x, g.start.y, 0.0], [g.end.x, g.end.y, 0.0]] }),
                    )]),
                ));
                (grouped.map_or(Parent::Building, Parent::Group), None)
            }
            ElementKind::Material(m) => {
                attrs.insert(MATERIAL.into(), json!({ "code": e.name }));
                attrs.insert(COLOR.into(), json!(m.color));
                if m.opacity < 1.0 {
                    attrs.insert(OPACITY.into(), json!(m.opacity));
                }
                attrs.insert(CREABIM.into(), creabim);
                nodes.push(node(&path, Names::default(), Map::new(), attrs));
                continue; // materials are roots, inherited by what is made of them
            }
            ElementKind::Assembly(_) | ElementKind::ElementType(_) | ElementKind::Shaft(_) => {
                // a root of its own, which the layered (or typed) elements
                // name in their CreaBIM parameters; a shaft is only a void
                attrs.insert(CREABIM.into(), creabim);
                nodes.push(node(&path, Names::default(), Map::new(), attrs));
                continue;
            }
            k if creabim_core::members::is_member(k) => {
                let level = creabim_core::members::level_of(k);
                (
                    level.map_or(Parent::Building, Parent::Level),
                    Some(elevation(level)),
                )
            }
            _ => continue, // views, sheets and annotations are not building data
        };
        if let Some(z) = frame_z {
            if let Some(mesh) = scene.get(&e.id) {
                add_body(
                    &mut nodes,
                    &mut children,
                    e.id,
                    mesh,
                    DVec3::new(0.0, 0.0, z),
                );
            }
        }
        if let Some((m, _)) = creabim_core::material_of(model, e) {
            inherits.insert("material".into(), json!(path_of(m.id)));
        }
        attrs.insert(
            CLASS.into(),
            json!({ "code": class, "uri": format!("{CLASS_URI}{class}") }),
        );
        attrs.insert(CREABIM.into(), creabim);
        let siblings = match parent {
            Parent::Building => &mut building_children,
            Parent::Level(l) => level_children.entry(l).or_default(),
            Parent::Wall(w) => wall_children.entry(w).or_default(),
            Parent::Group(g) => group_children.entry(g).or_default(),
        };
        siblings.add(&e.name, &path);
        nodes.push(node(&path, children, inherits, attrs));
    }
    // children found after their parent was written go in extra entries,
    // which IFCX composes onto the parent
    for (id, kids) in level_children
        .into_iter()
        .chain(wall_children)
        .chain(group_children)
    {
        nodes.push(node(&path_of(id), kids, Map::new(), Map::new()));
    }
    let class = |c: &str| {
        attributes([(
            CLASS,
            json!({ "code": c, "uri": format!("{CLASS_URI}{c}") }),
        )])
    };
    let mut root = Names::default();
    root.add("Site", &site);
    nodes.push(node(&project, root, Map::new(), class("IfcProject")));
    let mut s = Names::default();
    s.add("Building", &building);
    nodes.push(node(&site, s, Map::new(), class("IfcSite")));
    nodes.push(node(
        &building,
        building_children,
        Map::new(),
        class("IfcBuilding"),
    ));

    let header = json!({
        "header": {
            "id": "creabim-export.ifcx",
            "ifcxVersion": "ifcx_alpha",
            "dataVersion": "1.0.0",
            "author": concat!("CreaBIM ", env!("CARGO_PKG_VERSION")),
            "timestamp": today(),
        },
        "imports": [
            { "uri": "https://ifcx.dev/@standards.buildingsmart.org/ifc/core/ifc@v5a.ifcx" },
            { "uri": "https://ifcx.dev/@standards.buildingsmart.org/ifc/core/prop@v5a.ifcx" },
            { "uri": "https://ifcx.dev/@openusd.org/usd@v1.ifcx" },
        ],
        "schemas": {
            CREABIM: { "value": { "dataType": "Object" } },
        },
    });
    // one node per line keeps big files diffable and still valid JSON
    let mut out = header.to_string();
    out.pop(); // the closing brace
    out.push_str(",\"data\":[\n");
    for (i, n) in nodes.iter().enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        out.push_str(&n.to_string());
    }
    out.push_str("\n]}\n");
    out
}

enum Parent {
    Building,
    Level(ElementId),
    Wall(ElementId),
    /// A grid line, under its grid group.
    Group(ElementId),
}

/// Children by unique, IFCX-friendly name.
#[derive(Default)]
struct Names(Map<String, J>);

impl Names {
    fn add(&mut self, name: &str, path: &str) {
        let mut base: String = name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        if base.is_empty() {
            base = "Element".into();
        }
        let mut key = base.clone();
        let mut n = 1;
        while self.0.contains_key(&key) {
            n += 1;
            key = format!("{base}_{n}");
        }
        self.0.insert(key, json!(path));
    }
}

fn path_of(id: ElementId) -> String {
    id.0.hyphenated().to_string()
}

/// A stable id for a node that belongs to an element (its body, its axis).
fn derived(id: ElementId, role: u128) -> String {
    Uuid::from_u128(id.0.as_u128() ^ (role << 64) ^ 0x5bd1_e995)
        .hyphenated()
        .to_string()
}

fn attributes<const N: usize>(items: [(&str, J); N]) -> Map<String, J> {
    items.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

fn node(path: &str, children: Names, inherits: Map<String, J>, attrs: Map<String, J>) -> J {
    let mut n = Map::new();
    n.insert("path".into(), json!(path));
    if !children.0.is_empty() {
        n.insert("children".into(), J::Object(children.0));
    }
    if !inherits.is_empty() {
        n.insert("inherits".into(), J::Object(inherits));
    }
    if !attrs.is_empty() {
        n.insert("attributes".into(), J::Object(attrs));
    }
    J::Object(n)
}

/// A USD transform: row-major, translation in the last row.
fn xform(t: DVec3) -> J {
    json!({ "transform": [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [t.x, t.y, t.z, 1.0]] })
}

/// A "Body" child holding the element's triangles, relative to `at`.
fn add_body(
    nodes: &mut Vec<J>,
    children: &mut Names,
    id: ElementId,
    mesh: &creabim_geom::TriMesh,
    at: DVec3,
) {
    let mut index: HashMap<[i64; 3], u32> = HashMap::new();
    let mut points: Vec<[f64; 3]> = Vec::new();
    let mut faces: Vec<u32> = Vec::with_capacity(mesh.indices.len());
    for &i in &mesh.indices {
        let p = DVec3::from(mesh.positions[i as usize].map(f64::from)) - at;
        // round off f32 noise so shared corners weld
        let r = |v: f64| (v * 1e5).round() / 1e5;
        let p = [r(p.x), r(p.y), r(p.z)];
        let key = p.map(|v| (v * 1e5).round() as i64);
        let k = *index.entry(key).or_insert_with(|| {
            points.push(p);
            (points.len() - 1) as u32
        });
        faces.push(k);
    }
    if faces.is_empty() {
        return;
    }
    let body = derived(id, 0x1);
    children.add("Body", &body);
    nodes.push(node(
        &body,
        Names::default(),
        Map::new(),
        attributes([(
            MESH,
            json!({ "points": points, "faceVertexIndices": faces }),
        )]),
    ));
}

/// Today's date as YYYY-MM-DD, for the header.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    // civil from days (Howard Hinnant's algorithm)
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

// ---------------------------------------------------------------- import

#[derive(Default)]
struct Raw {
    children: IndexMap<String, Option<String>>,
    inherits: IndexMap<String, Option<String>>,
    attributes: Map<String, J>,
}

#[derive(Clone, Default)]
struct Composed {
    children: IndexMap<String, String>,
    inherits: Vec<String>,
    attributes: Map<String, J>,
}

/// One object (a node with an IFC class) found in the composed tree.
struct Object {
    path: String,
    name: String,
    class: String,
    attributes: Map<String, J>,
    world: DMat4,
    parent: Option<usize>,
    /// Visible geometry, in world coordinates.
    mesh: EditMesh,
    /// Invisible geometry, such as an opening's void.
    hidden: EditMesh,
    axis: Option<(DVec3, DVec3)>,
    material: Option<String>,
}

struct Reader {
    raw: IndexMap<String, Raw>,
    composed: HashMap<String, Composed>,
    warnings: Warnings,
    visits: usize,
}

/// Composition stops this deep, and after this many node visits, so a
/// cyclic or exploding file cannot hang the import.
const MAX_DEPTH: usize = 64;
const MAX_VISITS: usize = 2_000_000;

impl Reader {
    fn compose(&mut self, path: &str, depth: usize) -> Composed {
        if let Some(c) = self.composed.get(path) {
            return c.clone();
        }
        if depth > MAX_DEPTH {
            self.warnings.add("inheritance nested too deeply or cyclic");
            return Composed::default();
        }
        // guard against cycles while this node is being composed
        self.composed.insert(path.to_string(), Composed::default());
        let mut out = Composed::default();
        let (inherits, own_children, own_attrs) = match self.raw.get(path) {
            Some(r) => (
                r.inherits.values().flatten().cloned().collect::<Vec<_>>(),
                r.children.clone(),
                r.attributes.clone(),
            ),
            None => Default::default(),
        };
        for target in inherits {
            let Some(t) = self.resolve(&target, depth + 1) else {
                self.warnings
                    .add(format!("inherited path {target} not found"));
                continue;
            };
            let c = self.compose(&t, depth + 1);
            out.attributes.extend(c.attributes);
            out.children.extend(c.children);
            out.inherits.push(t);
        }
        for (k, v) in own_attrs {
            if v.is_null() {
                out.attributes.remove(&k);
            } else {
                out.attributes.insert(k, v);
            }
        }
        for (name, child) in own_children {
            match child.and_then(|c| self.resolve(&c, depth + 1)) {
                Some(c) => {
                    out.children.insert(name, c);
                }
                None => {
                    out.children.shift_remove(&name);
                }
            }
        }
        self.composed.insert(path.to_string(), out.clone());
        out
    }

    /// A path to a node, possibly through children: `uuid/Body/Frame`.
    fn resolve(&mut self, path: &str, depth: usize) -> Option<String> {
        if self.raw.contains_key(path) {
            return Some(path.to_string());
        }
        let mut parts = path.split('/').filter(|p| !p.is_empty());
        let mut cur = parts.next()?.to_string();
        if !self.raw.contains_key(&cur) {
            return None;
        }
        for part in parts {
            cur = self.compose(&cur, depth + 1).children.get(part)?.clone();
        }
        Some(cur)
    }

    fn visit(
        &mut self,
        path: &str,
        name: &str,
        parent_world: DMat4,
        owner: Option<usize>,
        objects: &mut Vec<Object>,
        depth: usize,
    ) {
        self.visits += 1;
        if depth > MAX_DEPTH || self.visits > MAX_VISITS {
            self.warnings
                .add("the node tree is too deep or too large; part of it was skipped");
            return;
        }
        let c = self.compose(path, 0);
        let world = parent_world * transform(c.attributes.get(XFORM));
        let mut owner = owner;
        if let Some(class) = class_of(&c.attributes) {
            let material = c
                .inherits
                .iter()
                .find(|t| {
                    self.composed
                        .get(*t)
                        .is_some_and(|m| m.attributes.contains_key(MATERIAL))
                })
                .cloned();
            objects.push(Object {
                path: path.to_string(),
                name: name.to_string(),
                class,
                attributes: c.attributes.clone(),
                world,
                parent: owner,
                mesh: EditMesh::default(),
                hidden: EditMesh::default(),
                axis: None,
                material,
            });
            owner = Some(objects.len() - 1);
        }
        let hidden = c
            .attributes
            .get(VISIBILITY)
            .and_then(|v| v.get("visibility"))
            .and_then(J::as_str)
            == Some("invisible");
        if let Some(m) = c.attributes.get(MESH) {
            let mesh = usd_mesh(m, &world, &mut self.warnings);
            if !mesh.faces.is_empty() {
                let o = match owner {
                    Some(o) => o,
                    None => {
                        // loose geometry still shows up, as a proxy
                        objects.push(Object {
                            path: path.to_string(),
                            name: name.to_string(),
                            class: "IfcBuildingElementProxy".into(),
                            attributes: Map::new(),
                            world,
                            parent: None,
                            mesh: EditMesh::default(),
                            hidden: EditMesh::default(),
                            axis: None,
                            material: None,
                        });
                        objects.len() - 1
                    }
                };
                let target = if hidden {
                    &mut objects[o].hidden
                } else {
                    &mut objects[o].mesh
                };
                crate::shape::append(target, mesh);
            }
        }
        if let (Some(o), Some(curve)) = (owner, c.attributes.get(CURVES)) {
            if name.eq_ignore_ascii_case("Axis") {
                let pts: Vec<DVec3> = curve
                    .get("points")
                    .and_then(J::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(vec3)
                    .map(|p| world.transform_point3(p))
                    .collect();
                if let (Some(a), Some(b)) = (pts.first(), pts.last()) {
                    objects[o].axis = Some((*a, *b));
                }
            }
        }
        for (child_name, child) in c.children.clone() {
            self.visit(&child, &child_name, world, owner, objects, depth + 1);
        }
    }
}

fn class_of(attrs: &Map<String, J>) -> Option<String> {
    // older alpha files spell it bsi::ifc::v5a::class
    let v = attrs.get(CLASS).or_else(|| {
        attrs
            .iter()
            .find(|(k, _)| k.starts_with("bsi::ifc") && k.ends_with("::class"))
            .map(|(_, v)| v)
    })?;
    v.get("code")
        .and_then(J::as_str)
        .or_else(|| v.as_str())
        .map(str::to_string)
}

fn vec3(v: &J) -> Option<DVec3> {
    let a = v.as_array()?;
    let c = |i: usize| a.get(i).and_then(J::as_f64);
    let p = DVec3::new(c(0)?, c(1)?, c(2).unwrap_or(0.0));
    p.is_finite().then_some(p)
}

/// A USD transform (row-major, translation in the last row) as a matrix.
fn transform(v: Option<&J>) -> DMat4 {
    let Some(rows) = v.and_then(|v| v.get("transform")).and_then(J::as_array) else {
        return DMat4::IDENTITY;
    };
    let mut cols = [DVec4::ZERO; 4];
    for (i, row) in rows.iter().take(4).enumerate() {
        let r: Vec<f64> = row
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(J::as_f64)
            .collect();
        if r.len() != 4 {
            return DMat4::IDENTITY;
        }
        // USD's row i is glam's column i (row vectors vs column vectors)
        cols[i] = DVec4::new(r[0], r[1], r[2], r[3]);
    }
    let m = DMat4::from_cols(cols[0], cols[1], cols[2], cols[3]);
    if m.is_finite() {
        m
    } else {
        DMat4::IDENTITY
    }
}

fn usd_mesh(m: &J, world: &DMat4, warnings: &mut Warnings) -> EditMesh {
    let points: Vec<DVec3> = m
        .get("points")
        .and_then(J::as_array)
        .into_iter()
        .flatten()
        .filter_map(vec3)
        .map(|p| world.transform_point3(p))
        .collect();
    let indices: Vec<u32> = m
        .get("faceVertexIndices")
        .and_then(J::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_u64().map(|i| i as u32))
        .collect();
    let counts: Vec<usize> = match m.get("faceVertexCounts").and_then(J::as_array) {
        Some(c) => c
            .iter()
            .filter_map(|v| v.as_u64().map(|n| n as usize))
            .collect(),
        None => vec![3; indices.len() / 3], // the alpha examples are all triangles
    };
    let mut mesh = EditMesh {
        vertices: points,
        faces: vec![],
    };
    let mut at = 0;
    for n in counts {
        let Some(face) = indices.get(at..at + n) else {
            warnings.add("a mesh's face counts do not match its indices");
            break;
        };
        at += n;
        if n >= 3 && face.iter().all(|&i| (i as usize) < mesh.vertices.len()) {
            mesh.faces.push(face.to_vec());
        }
    }
    mesh
}

/// Read an IFCX (IFC5 alpha) JSON file into a CreaBIM model.
pub fn import_ifcx(json: &str) -> Result<ImportResult, ImportError> {
    let root: J = serde_json::from_str(json).map_err(|e| ImportError::Ifcx(e.to_string()))?;
    let data = root
        .get("data")
        .and_then(J::as_array)
        .ok_or_else(|| ImportError::Ifcx("no \"data\" list".into()))?;
    let schema = root
        .pointer("/header/ifcxVersion")
        .and_then(J::as_str)
        .unwrap_or("ifcx")
        .to_string();
    let mut r = Reader {
        raw: IndexMap::new(),
        composed: HashMap::new(),
        warnings: Warnings::default(),
        visits: 0,
    };
    if schema != "ifcx_alpha" {
        r.warnings.add(format!(
            "ifcxVersion {schema:?} is not the alpha CreaBIM reads; trying anyway"
        ));
    }
    let names = |v: Option<&J>| -> IndexMap<String, Option<String>> {
        v.and_then(J::as_object)
            .into_iter()
            .flatten()
            .map(|(k, v)| (k.clone(), v.as_str().map(str::to_string)))
            .collect()
    };
    for entry in data {
        let Some(path) = entry.get("path").and_then(J::as_str) else {
            r.warnings.add("an entry without a path was skipped");
            continue;
        };
        let raw = r.raw.entry(path.to_string()).or_default();
        raw.children.extend(names(entry.get("children")));
        raw.inherits.extend(names(entry.get("inherits")));
        if let Some(a) = entry.get("attributes").and_then(J::as_object) {
            for (k, v) in a {
                raw.attributes.insert(k.clone(), v.clone());
            }
        }
    }
    // roots: nodes nobody lists as a child or inherits from
    let mut referenced: HashSet<String> = HashSet::new();
    for raw in r.raw.values() {
        for p in raw.children.values().chain(raw.inherits.values()).flatten() {
            referenced.insert(p.split('/').next().unwrap_or(p).to_string());
        }
    }
    let roots: Vec<String> = r
        .raw
        .keys()
        .filter(|p| !referenced.contains(*p))
        .cloned()
        .collect();
    let mut objects = Vec::new();
    for root in &roots {
        r.visit(root, root, DMat4::IDENTITY, None, &mut objects, 0);
    }
    let mut b = Builder {
        warnings: r.warnings,
        ..Builder::default()
    };
    // materials: the inherited nodes that name one, in order of first use
    let mut seen = HashSet::new();
    for p in objects.iter().filter_map(|o| o.material.as_ref()) {
        if seen.insert(p.clone()) {
            if let Some(c) = r.composed.get(p) {
                b.material(p, &c.attributes);
            }
        }
    }
    // assemblies, and the materials of their layers that nothing else uses
    // (and element types, made of them or of a material, and shafts)
    let (mut assemblies, mut types, mut shafts) = (vec![], vec![], vec![]);
    for (path, raw) in &r.raw {
        let Some((name, kind)) = creabim_element(&raw.attributes) else {
            continue;
        };
        let materials: Vec<ElementId> = match &kind {
            ElementKind::Assembly(a) => a.layers.iter().filter_map(|l| l.material).collect(),
            ElementKind::ElementType(t) => t.material().into_iter().collect(),
            ElementKind::Shaft(_) => vec![],
            _ => continue,
        };
        {
            for m in materials {
                let mp = path_of(m);
                if b.material_ids.contains_key(&mp) {
                    continue;
                }
                if let Some(mr) = r.raw.get(&mp) {
                    let attrs = mr.attributes.clone();
                    b.material(&mp, &attrs);
                }
            }
        }
        let id = b.id(path);
        let list = match &kind {
            ElementKind::Assembly(_) => &mut assemblies,
            ElementKind::ElementType(_) => &mut types,
            _ => &mut shafts,
        };
        list.push(Element {
            mark: None,
            id,
            name,
            kind,
            source: None,
            of_type: None,
        });
    }
    b.objects(objects);
    let mut warnings = b.warnings;
    let elements = [
        b.materials,
        assemblies,
        types,
        b.levels,
        shafts,
        b.grids,
        b.main,
        b.openings,
    ]
    .concat();
    let model = build_model(elements, &mut warnings)?;
    let (warnings, details) = warnings.into_parts();
    Ok(ImportResult {
        model,
        warnings,
        details,
        schema,
    })
}

#[derive(Default)]
struct Builder {
    warnings: Warnings,
    used: HashSet<ElementId>,
    material_ids: HashMap<String, ElementId>,
    materials: Vec<Element>,
    levels: Vec<Element>,
    grids: Vec<Element>,
    main: Vec<Element>,
    openings: Vec<Element>,
}

impl Builder {
    fn id(&mut self, path: &str) -> ElementId {
        let id = Uuid::parse_str(path)
            .ok()
            .map(ElementId)
            .filter(|id| !self.used.contains(id))
            .unwrap_or_default();
        self.used.insert(id);
        id
    }

    fn material(&mut self, path: &str, attrs: &Map<String, J>) {
        let ours = creabim_element(attrs);
        let id = self.id(path);
        let element = match ours {
            Some((name, kind @ ElementKind::Material(_))) => Element {
                mark: None,
                id,
                name,
                kind,
                source: None,
                of_type: None,
            },
            _ => {
                let name = attrs
                    .get(MATERIAL)
                    .and_then(|m| m.get("code"))
                    .and_then(J::as_str)
                    .unwrap_or("Material")
                    .to_string();
                let color = attrs
                    .get(COLOR)
                    .and_then(J::as_array)
                    .map(|c| {
                        let ch = |i: usize| {
                            c.get(i).and_then(J::as_f64).unwrap_or(0.75).clamp(0.0, 1.0) as f32
                        };
                        [ch(0), ch(1), ch(2)]
                    })
                    .unwrap_or([0.75; 3]);
                Element {
                    mark: None,
                    source: None,
                    id,
                    kind: ElementKind::Material(creabim_core::library::imported(
                        &name,
                        Material {
                            hatch: hatch_for(&name, "", ""),
                            color,
                            opacity: attrs.get(OPACITY).and_then(J::as_f64).unwrap_or(1.0) as f32,
                            ..Default::default()
                        },
                        false,
                    )),
                    name,
                    of_type: None,
                }
            }
        };
        self.material_ids.insert(path.to_string(), element.id);
        self.materials.push(element);
    }

    fn objects(&mut self, mut objects: Vec<Object>) {
        // storeys first, so everything else can find its level
        let mut storeys: Vec<(f64, usize)> = objects
            .iter()
            .enumerate()
            .filter(|(_, o)| o.class == "IfcBuildingStorey")
            .map(|(i, o)| {
                let z = match creabim_element(&o.attributes) {
                    Some((_, ElementKind::Level(l))) => l.elevation,
                    _ => o.world.w_axis.z,
                };
                (z, i)
            })
            .collect();
        storeys.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut level_of: HashMap<usize, (ElementId, f64)> = HashMap::new();
        for (k, &(z, i)) in storeys.iter().enumerate() {
            let o = &objects[i];
            let id = self.id(&o.path.clone());
            let (name, height) = match creabim_element(&o.attributes) {
                Some((n, ElementKind::Level(l))) => (n, l.height),
                _ => (
                    o.name.replace('_', " "),
                    storeys
                        .get(k + 1)
                        .map(|n| n.0 - z)
                        .filter(|h| *h > 0.01)
                        .unwrap_or(3.0),
                ),
            };
            self.levels.push(Element {
                mark: None,
                source: None,
                id,
                name,
                kind: ElementKind::Level(Level {
                    elevation: z,
                    height,
                }),
                of_type: None,
            });
            level_of.insert(i, (id, z));
        }
        if storeys.is_empty() {
            let e = Element::new(
                "Level 0",
                ElementKind::Level(Level {
                    elevation: 0.0,
                    height: 3.0,
                }),
            );
            self.warnings
                .add("the file has no storeys; everything was put on one level at 0");
            level_of.insert(usize::MAX, (e.id, 0.0));
            self.levels.push(e);
        }
        let levels: Vec<(f64, ElementId)> = level_of.values().map(|&(id, z)| (z, id)).collect();
        let level_for = |objects: &[Object], i: usize, z: f64| -> (ElementId, f64) {
            let mut cur = objects[i].parent;
            while let Some(p) = cur {
                if let Some(l) = level_of.get(&p) {
                    return *l;
                }
                cur = objects[p].parent;
            }
            let mut best = levels
                .iter()
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|l| (l.1, l.0))
                .unwrap_or_default();
            for &(e, id) in &levels {
                if e <= z + 1e-3 && e >= best.1 {
                    best = (id, e);
                }
            }
            best
        };

        let mut walls: HashMap<usize, HostWall> = HashMap::new();
        let mut spaces = 0;
        for i in 0..objects.len() {
            let o = &objects[i];
            if o.class == "IfcBuildingStorey" {
                continue;
            }
            let material = o
                .material
                .as_ref()
                .and_then(|m| self.material_ids.get(m))
                .copied();
            if let Some((name, kind)) = creabim_element(&o.attributes) {
                let id = self.id(&o.path.clone());
                let e = Element {
                    mark: None,
                    id,
                    name,
                    kind,
                    source: None,
                    of_type: o
                        .attributes
                        .get(CREABIM)
                        .and_then(|v| v.get("type"))
                        .and_then(|t| serde_json::from_value(t.clone()).ok()),
                };
                match &e.kind {
                    // hosted elements go after their hosts
                    ElementKind::Opening(_) => self.openings.push(e),
                    ElementKind::Railing(r) if r.stair.is_some() => self.openings.push(e),
                    ElementKind::Footing(f) if f.host.is_some() => self.openings.push(e),
                    ElementKind::Part(p) if p.host().is_some() => self.openings.push(e),
                    // groups first, so their lines find them
                    ElementKind::GridGroup(_) => self.grids.insert(0, e),
                    ElementKind::Grid(_) => self.grids.push(e),
                    ElementKind::Level(_)
                    | ElementKind::Material(_)
                    | ElementKind::Assembly(_)
                    | ElementKind::ElementType(_)
                    | ElementKind::Shaft(_) => {}
                    _ => self.main.push(e),
                }
                continue;
            }
            match o.class.as_str() {
                "IfcProject" | "IfcSite" | "IfcBuilding" if o.mesh.faces.is_empty() => continue,
                "IfcOpeningElement" | "IfcGrid" | "IfcGridAxis" | "IfcAnnotation" => continue,
                _ => {}
            }
            let name = o.name.replace('_', " ");
            let class = o.class.clone();
            let Some((lo, _)) = bounds(o.mesh.vertices.iter().copied()) else {
                if class == "IfcSpace" {
                    spaces += 1;
                } else if !o.class.starts_with("IfcBuilding") {
                    self.warnings.add(format!(
                        "{} {name:?} has no geometry CreaBIM can read",
                        o.class
                    ));
                }
                continue;
            };
            let (level, elev) = level_for(&objects, i, lo.z);
            // a space from another tool: a room with the outline of its body
            if class == "IfcSpace" {
                match prism_of_mesh(o.mesh.clone()) {
                    Some(p) if p.footprint.len() >= 3 => {
                        let mut r = creabim_core::Room::new(
                            level,
                            p.footprint.iter().copied().sum::<DVec2>() / p.footprint.len() as f64,
                        );
                        if !creabim_core::polygon::contains(&p.footprint, r.at) {
                            if let Some(t) =
                                creabim_core::polygon::triangulate(&p.footprint).first()
                            {
                                r.at = (p.footprint[t[0]] + p.footprint[t[1]] + p.footprint[t[2]])
                                    / 3.0;
                            }
                        }
                        r.boundary = p.footprint;
                        r.height = Some(p.z1 - elev).filter(|h| *h > 0.1);
                        let id = self.id(&o.path.clone());
                        self.main.push(Element {
                            mark: None,
                            source: None,
                            id,
                            name,
                            kind: ElementKind::Room(r),
                            of_type: None,
                        });
                    }
                    _ => spaces += 1,
                }
                continue;
            }
            // native walls: an axis and every vertex on the two faces along it
            if class.starts_with("IfcWall") {
                if let Some((start, end, thickness, z0, z1)) = o
                    .axis
                    .and_then(|(a, b)| wall_from_mesh(&o.mesh, a.truncate(), b.truncate()))
                {
                    let id = self.id(&o.path.clone());
                    walls.insert(
                        i,
                        HostWall {
                            id,
                            start,
                            end,
                            thickness,
                            base_z: z0,
                            top_z: z1,
                        },
                    );
                    self.main.push(Element {
                        mark: None,
                        source: None,
                        id,
                        name,
                        kind: ElementKind::Wall(Wall {
                            level,
                            start,
                            end,
                            thickness,
                            height: z1 - z0,
                            base_offset: z0 - elev,
                            material,
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
                        of_type: None,
                    });
                    continue;
                }
            }
            if class.starts_with("IfcSlab") {
                if let Some(p) = prism_of_mesh(o.mesh.clone()) {
                    let top = levels.iter().find(|(z, _)| (z - p.z1).abs() < 0.01);
                    if let Some(&(_, level)) = top {
                        let id = self.id(&o.path.clone());
                        self.main.push(Element {
                            mark: None,
                            source: None,
                            id,
                            name,
                            kind: ElementKind::Slab(Slab {
                                layer_edges: vec![],
                                level,
                                boundary: p.footprint,
                                thickness: p.z1 - p.z0,
                                material,
                                top_offset: 0.0,
                                bulges: vec![],
                                assembly: None,
                                holes: vec![],
                            }),
                            of_type: None,
                        });
                        continue;
                    }
                }
            }
            if class.starts_with("IfcDoor") || class.starts_with("IfcWindow") {
                let host = o.parent.and_then(|p| walls.get(&p)).copied();
                let hole = if o.hidden.faces.is_empty() {
                    &o.mesh
                } else {
                    &o.hidden
                };
                if let Some((wall, (offset, sill, width, height))) =
                    host.and_then(|w| Some((w, opening_in_wall(&w, hole, None)?)))
                {
                    let id = self.id(&o.path.clone());
                    let kind = if class.starts_with("IfcDoor") {
                        OpeningKind::Door
                    } else {
                        OpeningKind::Window
                    };
                    self.openings.push(Element {
                        mark: None,
                        source: None,
                        id,
                        name,
                        kind: ElementKind::Opening(Opening {
                            host: wall.id,
                            kind,
                            offset: offset.max(0.0),
                            sill: sill.max(0.0),
                            width,
                            height,
                            material,
                            frame: Default::default(),
                            flip_hand: false,
                            flip_facing: false,
                            wrap: None,
                        }),
                        of_type: None,
                    });
                    continue;
                }
            }
            // everything else: a free mesh at the object's own origin
            let mut mesh = std::mem::take(&mut objects[i].mesh);
            let o = &objects[i];
            weld(&mut mesh, 1e-6);
            merge_coplanar(&mut mesh);
            let at = o.world.w_axis.truncate();
            for v in &mut mesh.vertices {
                *v -= at;
            }
            let id = self.id(&o.path.clone());
            self.main.push(Element {
                mark: None,
                source: None,
                id,
                name,
                kind: ElementKind::FreeMesh(FreeMesh {
                    ifc_class: class,
                    material,
                    ..FreeMesh::new(Some(level), DVec3::new(at.x, at.y, at.z - elev), mesh)
                }),
                of_type: None,
            });
        }
        if spaces > 0 {
            self.warnings.add(format!(
                "{spaces} spaces have no flat outline CreaBIM can read and were left out"
            ));
        }
    }
}

/// CreaBIM's own parameters, if this element was written by CreaBIM.
fn creabim_element(attrs: &Map<String, J>) -> Option<(String, ElementKind)> {
    let v = attrs.get(CREABIM)?;
    let name = v.get("name").and_then(J::as_str).unwrap_or("").to_string();
    let kind: ElementKind = serde_json::from_value(v.get("kind")?.clone()).ok()?;
    Some((name, kind))
}

/// A straight wall from a mesh and its axis: every vertex must lie on one of
/// two vertical planes parallel to the axis (holes for windows are fine),
/// and both faces must be rectangles in elevation. Returns the centreline,
/// thickness and bottom and top heights.
fn wall_from_mesh(mesh: &EditMesh, a: DVec2, b: DVec2) -> Option<(DVec2, DVec2, f64, f64, f64)> {
    let d = (b - a).normalize_or_zero();
    if d == DVec2::ZERO || mesh.vertices.is_empty() {
        return None;
    }
    let n = d.perp();
    let tol = 1e-3;
    let s: Vec<f64> = mesh
        .vertices
        .iter()
        .map(|v| (v.truncate() - a).dot(n))
        .collect();
    let smin = s.iter().copied().fold(f64::INFINITY, f64::min);
    let smax = s.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if smax - smin < 2.0 * tol
        || s.iter()
            .any(|&v| (v - smin).abs() > tol && (v - smax).abs() > tol)
    {
        return None;
    }
    let (lo, hi) = bounds(mesh.vertices.iter().copied())?;
    let (z0, z1) = (lo.z, hi.z);
    for face in [smin, smax] {
        let on: Vec<DVec3> = mesh
            .vertices
            .iter()
            .zip(&s)
            .filter(|(_, sv)| (**sv - face).abs() <= tol)
            .map(|(v, _)| *v)
            .collect();
        let u = |v: &DVec3| (v.truncate() - a).dot(d);
        let u0 = on.iter().map(u).fold(f64::INFINITY, f64::min);
        let u1 = on.iter().map(u).fold(f64::NEG_INFINITY, f64::max);
        for (uu, zz) in [(u0, z0), (u0, z1), (u1, z0), (u1, z1)] {
            if !on
                .iter()
                .any(|v| (u(v) - uu).abs() <= tol && (v.z - zz).abs() <= tol)
            {
                return None; // a sloped top or a stepped face: not a simple wall
            }
        }
    }
    let c = (smin + smax) / 2.0;
    Some((a + n * c, b + n * c, smax - smin, z0, z1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layered_nodes_compose_and_inherit() {
        let file = r#"{
            "header": {"ifcxVersion": "ifcx_alpha"},
            "data": [
                {"path": "root", "children": {"Storey": "s"}},
                {"path": "s", "attributes": {"bsi::ifc::class": {"code": "IfcBuildingStorey"},
                    "usd::xformop": {"transform": [[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,3,1]]}},
                    "children": {"Thing": "t"}},
                {"path": "type", "children": {"Body": "b"}},
                {"path": "b", "attributes": {"usd::usdgeom::mesh": {
                    "points": [[0,0,0],[1,0,0],[0,1,0],[0,0,1]],
                    "faceVertexIndices": [0,2,1, 0,1,3, 1,2,3, 0,3,2]}}},
                {"path": "t", "inherits": {"type": "type"},
                    "attributes": {"bsi::ifc::class": {"code": "IfcFurniture"}}},
                {"path": "t", "attributes": {"usd::xformop":
                    {"transform": [[1,0,0,0],[0,1,0,0],[0,0,1,0],[5,0,0,1]]}}}
            ]}"#;
        let r = import_ifcx(file).unwrap();
        let m = &r.model;
        assert_eq!(m.levels().count(), 1);
        let (_, f) = m
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::FreeMesh(f) => Some((e, f)),
                _ => None,
            })
            .expect("the inherited body becomes a mesh");
        assert_eq!(f.ifc_class, "IfcFurniture");
        assert!(
            (f.origin - DVec3::new(5.0, 0.0, 0.0)).length() < 1e-9,
            "{}",
            f.origin
        );
        assert!((f.mesh.volume() - 1.0 / 6.0).abs() < 1e-9);
    }

    #[test]
    fn bad_json_is_an_error() {
        assert!(import_ifcx("not json").is_err());
        assert!(import_ifcx("{}").is_err());
        // a child cycle terminates
        let cyclic = r#"{"data": [{"path": "a", "children": {"b": "b"}},
            {"path": "b", "children": {"a": "a"}, "inherits": {"x": "b"}}]}"#;
        assert!(import_ifcx(cyclic).is_ok());
    }

    #[test]
    fn wall_with_window_holes_is_recognised() {
        // the faces of a 4 m wall, 0.2 thick, 3 high, with a hole's corners on both faces
        let mut vertices = vec![];
        for s in [0.0, 0.2] {
            for (u, z) in [
                (0.0, 0.0),
                (4.0, 0.0),
                (4.0, 3.0),
                (0.0, 3.0),
                (1.0, 1.0),
                (2.0, 1.0),
                (2.0, 2.0),
                (1.0, 2.0),
            ] {
                vertices.push(DVec3::new(u, s, z));
            }
        }
        let mesh = EditMesh {
            vertices,
            faces: vec![vec![0, 1, 2]],
        };
        let (a, b, t, z0, z1) = wall_from_mesh(&mesh, DVec2::ZERO, DVec2::new(4.0, 0.0)).unwrap();
        assert!(a.distance(DVec2::new(0.0, 0.1)) < 1e-9 && b.distance(DVec2::new(4.0, 0.1)) < 1e-9);
        assert!((t - 0.2).abs() < 1e-9 && z0 == 0.0 && z1 == 3.0);
    }
}
