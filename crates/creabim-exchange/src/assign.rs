//! Giving a modelled shape a building meaning: a mesh from Blender (or a
//! mass drawn in CreaBIM) becomes a wall, slab, door or window sized to fit
//! it, or keeps its shape and takes an IFC class.

use crate::shape;
use creabim_core::{Element, ElementId, ElementKind, Model, Opening, OpeningKind, Slab, Wall};
use creabim_geom::mesh::TriMesh;
use glam::{DVec2, DVec3};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Assign {
    Wall,
    Slab,
    Door,
    Window,
    /// Keep the mesh, and give it this IFC class (`IfcColumn`, `IfcRoof`...).
    Class(String),
}

impl Assign {
    /// What the "Assign as" menu offers.
    pub fn menu() -> Vec<Assign> {
        let mut v = vec![Assign::Wall, Assign::Slab, Assign::Door, Assign::Window];
        for c in [
            "IfcColumn",
            "IfcBeam",
            "IfcRoof",
            "IfcStair",
            "IfcRailing",
            "IfcCovering",
            "IfcFurniture",
            "IfcBuildingElementProxy",
        ] {
            v.push(Assign::Class(c.into()));
        }
        v
    }

    pub fn label(&self) -> String {
        match self {
            Assign::Wall => "Wall".into(),
            Assign::Slab => "Slab".into(),
            Assign::Door => "Door".into(),
            Assign::Window => "Window".into(),
            Assign::Class(c) => c.trim_start_matches("Ifc").to_string(),
        }
    }

    /// Read a hint like `wall`, `IfcWall` or `IfcColumn`, as Blender
    /// custom properties or FreeCAD IFC types give it.
    pub fn parse(s: &str) -> Option<Assign> {
        let t = s.trim();
        let l = t.to_ascii_lowercase();
        let l = l.strip_prefix("ifc").unwrap_or(&l);
        Some(match l {
            // a plain shape: no kind asked for
            "" | "mesh" | "free shape" | "none" => return None,
            "wall" | "wallstandardcase" => Assign::Wall,
            "slab" | "floor" => Assign::Slab,
            "door" => Assign::Door,
            "window" => Assign::Window,
            _ if t.len() > 3 && t[..3].eq_ignore_ascii_case("ifc") => {
                Assign::Class(format!("Ifc{}", &t[3..]))
            }
            _ => Assign::Class(format!("Ifc{}{}", t[..1].to_ascii_uppercase(), &t[1..])),
        })
    }
}

/// The element's shape in model coordinates, as the 3D view shows it.
pub fn shape_of(model: &Model, id: ElementId) -> Option<TriMesh> {
    if let Some(m) = shape::world_mesh(model, id) {
        return Some(shape::tri(&m));
    }
    creabim_geom::scene::element_meshes(&creabim_geom::scene::build_scene(model)).remove(&id)
}

/// Turn element `id` into `kind`, fitted to its current shape. It keeps its
/// id, name and source, so tags, links and reloads still find it.
pub fn assign(model: &Model, id: ElementId, kind: &Assign) -> Result<Element, String> {
    let e = model.get(id).ok_or("The element is gone.")?;
    if let Assign::Class(c) = kind {
        let mut out = e.clone();
        return match &mut out.kind {
            ElementKind::FreeMesh(f) => {
                f.ifc_class = c.clone();
                Ok(out)
            }
            _ => Err(
                "Only a mesh can take any IFC class. Use Wall, Slab, Door or Window here.".into(),
            ),
        };
    }
    let shape = shape_of(model, id).ok_or("This element has no shape to fit.")?;
    let mut out = e.clone();
    out.kind = fit(model, kind, &shape, material_of(&e.kind), Some(id))?;
    Ok(out)
}

fn material_of(k: &ElementKind) -> Option<ElementId> {
    match k {
        ElementKind::FreeMesh(f) => f.material,
        ElementKind::Wall(w) => w.material,
        ElementKind::Slab(s) => s.material,
        ElementKind::Opening(o) => o.material,
        _ => None,
    }
}

/// A `kind` sized to fit `shape` (triangles in model coordinates).
/// `itself` is left out when looking for a door's or window's wall.
pub fn fit(
    model: &Model,
    kind: &Assign,
    shape: &TriMesh,
    material: Option<ElementId>,
    itself: Option<ElementId>,
) -> Result<ElementKind, String> {
    let pts = shape::points(shape);
    let (lo, hi) = shape::bounds(&pts).ok_or("The shape is empty.")?;
    let plan: Vec<DVec2> = pts.iter().map(|p| p.truncate()).collect();
    match kind {
        Assign::Wall => {
            let (level, elev) = shape::level_at(model, lo.z).ok_or("Add a level first.")?;
            let r = shape::min_rect(&shape::hull(&plan)).ok_or("The shape is too small.")?;
            if r.half_len < 0.025 || r.half_width < 0.0025 || hi.z - lo.z < 0.05 {
                return Err("The shape is too small for a wall.".into());
            }
            Ok(ElementKind::Wall(Wall {
                level,
                start: r.center - r.along * r.half_len,
                end: r.center + r.along * r.half_len,
                thickness: r.half_width * 2.0,
                height: hi.z - lo.z,
                base_offset: lo.z - elev,
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
            }))
        }
        Assign::Slab => {
            let (level, elevation) =
                shape::nearest_level(model, hi.z).ok_or("Add a level first.")?;
            let mid = (lo.z + hi.z) / 2.0;
            let mut boundary = creabim_geom::slice::slice(shape, mid)
                .into_iter()
                .max_by(|a, b| {
                    creabim_core::polygon::signed_area2(a)
                        .total_cmp(&creabim_core::polygon::signed_area2(b))
                })
                .unwrap_or_else(|| shape::hull(&plan));
            if boundary.len() < 3 {
                boundary = shape::hull(&plan);
            }
            if boundary.len() < 3 || hi.z - lo.z < 0.005 {
                return Err("The shape is too small for a slab.".into());
            }
            if creabim_core::polygon::signed_area2(&boundary) < 0.0 {
                boundary.reverse();
            }
            Ok(ElementKind::Slab(Slab {
                layer_edges: vec![],
                level,
                boundary,
                thickness: hi.z - lo.z,
                material,
                top_offset: hi.z - elevation,
                bulges: vec![],
                assembly: None,
                holes: vec![],
            }))
        }
        Assign::Door | Assign::Window => {
            let c = (lo + hi) / 2.0;
            let (host, w, base) = host_wall(model, c, itself)
                .ok_or("Put it in a wall first: no wall runs through it.")?;
            let d = w.direction();
            let us: Vec<f64> = plan.iter().map(|p| (*p - w.start).dot(d)).collect();
            let u0 = us.iter().copied().fold(f64::MAX, f64::min).max(0.0);
            let u1 = us.iter().copied().fold(f64::MIN, f64::max).min(w.length());
            if u1 - u0 < 0.1 {
                return Err("The shape is too narrow for an opening.".into());
            }
            let sill = (lo.z - base).max(0.0);
            let height = (hi.z - base - sill).min(w.height - sill);
            if height < 0.05 {
                return Err("The shape is too short for an opening in its wall.".into());
            }
            Ok(ElementKind::Opening(Opening {
                host,
                kind: if *kind == Assign::Door {
                    OpeningKind::Door
                } else {
                    OpeningKind::Window
                },
                offset: u0,
                sill,
                width: u1 - u0,
                height,
                material,
                frame: Default::default(),
                flip_hand: false,
                flip_facing: false,
                wrap: None,
                frame_at: None,
            }))
        }
        Assign::Class(_) => Err("A class is not a shape.".into()),
    }
}

/// The wall that `c` lies in: its id, the wall, and its base height.
fn host_wall(model: &Model, c: DVec3, itself: Option<ElementId>) -> Option<(ElementId, Wall, f64)> {
    let mut best: Option<(f64, ElementId, Wall, f64)> = None;
    for e in model.iter() {
        let ElementKind::Wall(w) = &e.kind else {
            continue;
        };
        if Some(e.id) == itself {
            continue;
        }
        let base = model.level(w.level).map_or(0.0, |l| l.elevation) + w.base_offset;
        let d = w.direction();
        let u = (c.truncate() - w.start).dot(d);
        let v = (c.truncate() - w.start).perp_dot(d).abs();
        let inside = u > -0.01
            && u < w.length() + 0.01
            && v <= w.thickness / 2.0 + 0.3
            && c.z > base - 0.01
            && c.z < base + w.height + 0.01;
        if inside && best.as_ref().is_none_or(|b| v < b.0) {
            best = Some((v, e.id, w.clone(), base));
        }
    }
    best.map(|(_, id, w, b)| (id, w, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{Document, EditMesh, FreeMesh, Level, Transaction};

    fn model_with_mass(size: DVec3, at: DVec3) -> (Model, ElementId) {
        let l = Element::new(
            "L0",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let m = Element::new(
            "Box",
            ElementKind::FreeMesh(FreeMesh::new(Some(l.id), at, EditMesh::cuboid(size))),
        );
        let id = m.id;
        let mut doc = Document::new();
        doc.commit(Transaction::new("t").insert(l).insert(m))
            .unwrap();
        (doc.model().clone(), id)
    }

    #[test]
    fn a_long_thin_box_becomes_a_wall_that_fits_it() {
        let (m, id) = model_with_mass(DVec3::new(4.0, 0.25, 2.8), DVec3::new(2.0, 1.0, 0.0));
        let e = assign(&m, id, &Assign::Wall).unwrap();
        let ElementKind::Wall(w) = &e.kind else {
            panic!("{:?}", e.kind)
        };
        assert_eq!(e.id, id);
        assert!((w.length() - 4.0).abs() < 1e-6);
        assert!((w.thickness - 0.25).abs() < 1e-6);
        assert!((w.height - 2.8).abs() < 1e-6);
        assert!(w.start.distance(DVec2::new(0.0, 1.0)) < 1e-6);
    }

    #[test]
    fn a_flat_box_becomes_a_slab() {
        let (m, id) = model_with_mass(DVec3::new(5.0, 3.0, 0.2), DVec3::new(0.0, 0.0, -0.2));
        let e = assign(&m, id, &Assign::Slab).unwrap();
        let ElementKind::Slab(s) = &e.kind else {
            panic!()
        };
        assert_eq!(s.boundary.len(), 4);
        assert!((s.thickness - 0.2).abs() < 1e-6);
        assert!((creabim_core::polygon::signed_area2(&s.boundary) / 2.0 - 15.0).abs() < 1e-6);
    }

    #[test]
    fn hints_read_as_kinds_or_classes() {
        assert_eq!(Assign::parse("IfcWall"), Some(Assign::Wall));
        assert_eq!(Assign::parse("door"), Some(Assign::Door));
        assert_eq!(
            Assign::parse("column"),
            Some(Assign::Class("IfcColumn".into()))
        );
        assert_eq!(
            Assign::parse("IfcRoof"),
            Some(Assign::Class("IfcRoof".into()))
        );
    }
}
