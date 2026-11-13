//! Layered assemblies: named build-ups of material layers that walls,
//! slabs and roofs are made of, like Revit's compound structures or
//! ArchiCAD's composites.
//!
//! An assembly is an element of its own, so it saves with the project, has
//! a name, and many elements share it: editing it changes all of them. Its
//! layers run from the exterior face of a wall inwards, and from the top of
//! a slab or roof down. Each has a material from the project's materials
//! (none for an air gap), a thickness and a function. The structure layers,
//! and whatever lies between them, are the core: where walls meet at a T,
//! the core of the wall that ends runs on to the other wall's core.
//!
//! An element with an assembly keeps its own `thickness`, always equal to
//! the assembly's total: every edit here sets both, so code that only knows
//! thicknesses keeps working, and an element whose assembly goes missing
//! is simply a single-material element again.

use crate::{Element, ElementId, ElementKind, Model, Opening, OpeningKind, Transaction, Wall};
use serde::{Deserialize, Serialize};

/// What a layer does, as Revit and IFC name layer functions.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum LayerFunction {
    /// Load-bearing: the core.
    #[default]
    Structure,
    /// A backing for a finish, such as a screed or sheathing.
    Substrate,
    /// Thermal insulation.
    Insulation,
    /// A ventilated cavity, usually with no material.
    AirGap,
    /// A thin membrane: damp proofing, vapour control, roofing felt.
    Membrane,
    /// A surface layer: plaster, cladding, floor finish, tiles.
    Finish,
}

impl LayerFunction {
    pub const ALL: [LayerFunction; 6] = [
        LayerFunction::Structure,
        LayerFunction::Substrate,
        LayerFunction::Insulation,
        LayerFunction::AirGap,
        LayerFunction::Membrane,
        LayerFunction::Finish,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LayerFunction::Structure => "Structure",
            LayerFunction::Substrate => "Substrate",
            LayerFunction::Insulation => "Insulation",
            LayerFunction::AirGap => "Air gap",
            LayerFunction::Membrane => "Membrane",
            LayerFunction::Finish => "Finish",
        }
    }

    /// `IfcMaterialLayer.Category`, using IFC4's recommended values where
    /// there is one.
    pub fn ifc_category(self) -> &'static str {
        match self {
            LayerFunction::Structure => "LoadBearing",
            LayerFunction::Substrate => "Substrate",
            LayerFunction::Insulation => "Insulation",
            LayerFunction::AirGap => "AirGap",
            LayerFunction::Membrane => "Membrane",
            LayerFunction::Finish => "Finish",
        }
    }

    /// The function an IFC layer had, from its category and whether it is
    /// ventilated (an air layer).
    pub fn from_ifc(category: &str, ventilated: bool) -> LayerFunction {
        if ventilated {
            return LayerFunction::AirGap;
        }
        let c = category.trim().to_ascii_lowercase();
        LayerFunction::ALL
            .into_iter()
            .find(|f| {
                f.ifc_category().eq_ignore_ascii_case(&c) || f.label().eq_ignore_ascii_case(&c)
            })
            .unwrap_or(if c.contains("insul") {
                LayerFunction::Insulation
            } else if c.contains("finish") || c.contains("cladding") {
                LayerFunction::Finish
            } else if c.contains("air") {
                LayerFunction::AirGap
            } else if c.contains("membrane") {
                LayerFunction::Membrane
            } else {
                LayerFunction::Structure
            })
    }
}

/// One layer of an assembly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    #[serde(default)]
    pub function: LayerFunction,
    /// What it is made of; `None` for an air gap, or "by element".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<ElementId>,
    /// In metres.
    pub thickness: f64,
    /// Where elements meet, which layers run through which: lower runs
    /// through higher. `None` takes its function's
    /// ([`crate::joins::layer_priority`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    /// Does it turn into the reveals of doors and windows, where its
    /// assembly wraps on its side ([`Assembly::wrap_openings`])? `None`
    /// takes its function's: finishes do, the rest stop square.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wraps: Option<bool>,
}

impl Layer {
    pub fn new(function: LayerFunction, material: Option<ElementId>, thickness: f64) -> Self {
        Self {
            function,
            material,
            thickness,
            priority: None,
            wraps: None,
        }
    }

    /// Does it turn into the reveals of openings: its own choice, or its
    /// function's (a finish does).
    pub fn wraps_openings(&self) -> bool {
        self.wraps.unwrap_or(self.function == LayerFunction::Finish)
    }

    /// Its join priority: its own, or its function's.
    pub fn join_priority(&self) -> u8 {
        crate::joins::layer_priority(self.function, self.priority)
    }
}

/// What kind of element an assembly is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssemblyUse {
    #[default]
    Wall,
    Floor,
    Roof,
}

impl AssemblyUse {
    pub const ALL: [AssemblyUse; 3] = [AssemblyUse::Wall, AssemblyUse::Floor, AssemblyUse::Roof];

    pub fn label(self) -> &'static str {
        match self {
            AssemblyUse::Wall => "Walls",
            AssemblyUse::Floor => "Floors and slabs",
            AssemblyUse::Roof => "Roofs",
        }
    }

    /// The use that fits an element: walls, slabs, roofs.
    pub fn of(kind: &ElementKind) -> Option<AssemblyUse> {
        match kind {
            ElementKind::Wall(_) => Some(AssemblyUse::Wall),
            ElementKind::Slab(_) => Some(AssemblyUse::Floor),
            ElementKind::Roof(_) => Some(AssemblyUse::Roof),
            _ => None,
        }
    }

    /// Which face the first layer is on, for the editor.
    pub fn first_face(self) -> &'static str {
        match self {
            AssemblyUse::Wall => "Exterior",
            AssemblyUse::Floor | AssemblyUse::Roof => "Top",
        }
    }

    pub fn last_face(self) -> &'static str {
        match self {
            AssemblyUse::Wall => "Interior",
            AssemblyUse::Floor | AssemblyUse::Roof => "Underside",
        }
    }
}

/// A layered build-up. The element's name is the assembly's name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Assembly {
    #[serde(default)]
    pub usage: AssemblyUse,
    /// From the exterior face (walls) or the top (slabs, roofs).
    pub layers: Vec<Layer>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// At doors and windows, the faces its wrapping layers turn in from
    /// to line the reveals up to the frame (walls only).
    #[serde(default, skip_serializing_if = "WrapSides::is_none")]
    pub wrap_openings: WrapSides,
}

/// Which faces of a wall its layers turn in from at a door or window, to
/// line the reveal: Revit's "wrapping at inserts".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WrapSides {
    /// Every layer stops square at the opening.
    #[default]
    None,
    Exterior,
    Interior,
    Both,
}

impl WrapSides {
    pub const ALL: [WrapSides; 4] = [
        WrapSides::None,
        WrapSides::Exterior,
        WrapSides::Interior,
        WrapSides::Both,
    ];

    pub fn label(self) -> &'static str {
        match self {
            WrapSides::None => "Do not wrap",
            WrapSides::Exterior => "Exterior",
            WrapSides::Interior => "Interior",
            WrapSides::Both => "Both",
        }
    }

    pub fn is_none(&self) -> bool {
        *self == WrapSides::None
    }

    pub fn exterior(self) -> bool {
        matches!(self, WrapSides::Exterior | WrapSides::Both)
    }

    pub fn interior(self) -> bool {
        matches!(self, WrapSides::Interior | WrapSides::Both)
    }
}

/// A layer turned into an opening's reveal: a lining `thickness` thick
/// round the opening, `inset` back from its edge (behind the layers that
/// wrap outside it), running across the wall from `from` to `to`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WrapReturn {
    pub layer: usize,
    pub inset: f64,
    pub thickness: f64,
    pub from: f64,
    pub to: f64,
}

/// One face's wrapping: from the face in to where the returns stop.
#[derive(Clone, Debug, PartialEq)]
struct WrapZone {
    face: f64,
    stop: f64,
    /// Outermost first, each with the depth of its layer's inner face.
    returns: Vec<(WrapReturn, f64)>,
}

/// How a wall's layers wrap at one opening ([`opening_wrap`]). Across
/// the wall, positions are as [`Band`]'s: centreline 0, left positive.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpeningWrap {
    zones: Vec<WrapZone>,
}

impl OpeningWrap {
    /// Nothing wraps: every layer stops square.
    pub fn is_empty(&self) -> bool {
        self.zones.is_empty()
    }

    /// How far back from the opening's edge the wall's own material stops
    /// at `s` across the wall: the thickness of the returns passing there.
    pub fn inset_at(&self, s: f64) -> f64 {
        self.zones
            .iter()
            .filter(|z| (s - z.face) * (s - z.stop) < 0.0)
            .map(|z| {
                let depth = (s - z.face).abs();
                z.returns
                    .iter()
                    .filter(|(_, d)| *d <= depth + 1e-9)
                    .map(|(r, _)| r.thickness)
                    .sum::<f64>()
            })
            .sum()
    }

    /// The return of layer `layer`, if it wraps here.
    pub fn return_of(&self, layer: usize) -> Option<WrapReturn> {
        self.zones
            .iter()
            .flat_map(|z| &z.returns)
            .find(|(r, _)| r.layer == layer)
            .map(|(r, _)| *r)
    }

    /// Every return, for drawing.
    pub fn returns(&self) -> impl Iterator<Item = WrapReturn> + '_ {
        self.zones.iter().flat_map(|z| &z.returns).map(|(r, _)| *r)
    }

    /// Where across the wall the returns stop: the frame's faces.
    pub fn stops(&self) -> impl Iterator<Item = f64> + '_ {
        self.zones.iter().map(|z| z.stop)
    }

    /// The parts of the reveal the returns line, across the wall, each
    /// `(low, high)`.
    pub fn lined(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        self.zones
            .iter()
            .map(|z| (z.face.min(z.stop), z.face.max(z.stop)))
    }
}

/// How far across `wall` from its centre line the middle of the frame of
/// opening `o` stands (left of start → end positive, as [`Band`]), with
/// the wall's layers `a` if it has them: 0 unless the opening says where
/// its frame goes ([`Opening::frame_at`]). The frame is kept inside the
/// wall where it fits.
pub fn frame_shift_in(wall: &Wall, a: Option<&Assembly>, o: &Opening) -> f64 {
    let Some(at) = o.frame_at else {
        return 0.0;
    };
    let t = wall.thickness / 2.0;
    let hd = o.frame.depth.clamp(0.02, wall.thickness.max(0.02)) / 2.0;
    // from the exterior to the interior across the wall
    let inwards = if wall.flip { -1.0 } else { 1.0 };
    let (ext, int) = match a.filter(|a| a.usage == AssemblyUse::Wall) {
        Some(a) => {
            let bands = wall_bands(a, wall);
            let b = bands[at.layer.min(bands.len() - 1)];
            if wall.flip {
                (b.s1, b.s0)
            } else {
                (b.s0, b.s1)
            }
        }
        // a wall of one material is one layer
        None => (-t * inwards, t * inwards),
    };
    let c = match at.face {
        crate::LayerFace::Exterior => ext + inwards * hd,
        crate::LayerFace::Interior => int - inwards * hd,
    } + inwards * at.offset;
    let room = (t - hd).max(0.0);
    c.clamp(-room, room)
}

/// [`frame_shift_in`] for opening `o` in its host wall in `model`.
pub fn frame_shift(model: &Model, o: &Opening) -> f64 {
    let Some(e) = model.get(o.host) else {
        return 0.0;
    };
    let ElementKind::Wall(w) = &e.kind else {
        return 0.0;
    };
    frame_shift_in(w, assembly_of(model, e).map(|x| x.1), o)
}

/// How the layers of `wall` wrap at opening `o`, whose frame is
/// `frame_depth` deep through the wall and stands `shift` off its centre
/// line ([`frame_shift_in`]): on each face
/// the assembly (or the opening itself, [`Opening::wrap`]) wraps, the
/// layers that wrap turn in and run to the frame, the outermost against
/// the opening and each next one behind it; what they cover stops short
/// by their thickness, so the opening keeps its size. A bare hole has no
/// frame: its returns stop at the core.
pub fn opening_wrap(
    a: &Assembly,
    wall: &Wall,
    o: &Opening,
    frame_depth: f64,
    shift: f64,
) -> OpeningWrap {
    let sides = o.wrap.unwrap_or(a.wrap_openings);
    if sides.is_none() || a.usage != AssemblyUse::Wall {
        return OpeningWrap::default();
    }
    let bands = wall_bands(a, wall);
    let t = wall.thickness / 2.0;
    let hd = (frame_depth / 2.0).clamp(0.0, t);
    let (core_lo, core_hi) = wall_core(a, wall);
    // the exterior face is on the wall's right, or its left when flipped
    let exterior = if wall.flip { t } else { -t };
    let mut zones = Vec::new();
    for (on, face) in [(sides.exterior(), exterior), (sides.interior(), -exterior)] {
        if !on {
            continue;
        }
        let stop = match (o.kind, face < 0.0) {
            (OpeningKind::Hole, true) => core_lo,
            (OpeningKind::Hole, false) => core_hi,
            (_, true) => shift - hd,
            (_, false) => shift + hd,
        };
        let reach = (stop - face).abs();
        // from the face inwards
        let mut order: Vec<&Band> = bands.iter().collect();
        order.sort_by(|x, y| {
            let d = |b: &Band| ((b.s0 + b.s1) / 2.0 - face).abs();
            d(x).total_cmp(&d(y))
        });
        let mut inset = 0.0;
        let mut returns = Vec::new();
        for b in order {
            let inner = if face < 0.0 { b.s1 } else { b.s0 };
            let depth = (inner - face).abs();
            if depth >= reach - 1e-6 {
                break;
            }
            if !a.layers[b.index].wraps_openings() || b.function == LayerFunction::AirGap {
                continue;
            }
            let thickness = b.s1 - b.s0;
            returns.push((
                WrapReturn {
                    layer: b.index,
                    inset,
                    thickness,
                    from: inner.min(stop),
                    to: inner.max(stop),
                },
                depth,
            ));
            inset += thickness;
        }
        if !returns.is_empty() {
            zones.push(WrapZone {
                face,
                stop,
                returns,
            });
        }
    }
    OpeningWrap { zones }
}

/// The thinnest layer the editor and validation accept: 1 mm.
pub const MIN_LAYER: f64 = 0.001;

impl Assembly {
    pub fn new(usage: AssemblyUse, layers: Vec<Layer>) -> Self {
        Self {
            usage,
            layers,
            description: String::new(),
            wrap_openings: WrapSides::None,
        }
    }

    /// Total thickness: the sum of the layers.
    pub fn total(&self) -> f64 {
        self.layers.iter().map(|l| l.thickness).sum()
    }

    /// The core's layers, `first..=last`: from the first structure layer
    /// to the last; every layer when none is structure.
    pub fn core(&self) -> (usize, usize) {
        let s = |l: &Layer| l.function == LayerFunction::Structure;
        match (
            self.layers.iter().position(s),
            self.layers.iter().rposition(s),
        ) {
            (Some(a), Some(b)) => (a, b),
            _ => (0, self.layers.len().saturating_sub(1)),
        }
    }

    pub fn is_core(&self, i: usize) -> bool {
        let (a, b) = self.core();
        (a..=b).contains(&i)
    }

    /// Each layer's span from the first face, scaled to fill `thickness`
    /// (the element's own, normally the total already).
    pub fn spans(&self, thickness: f64) -> Vec<(f64, f64)> {
        let total = self.total();
        let k = if total > 1e-9 { thickness / total } else { 0.0 };
        let mut at = 0.0;
        self.layers
            .iter()
            .map(|l| {
                let a = at;
                at += l.thickness * k;
                (a, at)
            })
            .collect()
    }

    /// A short summary: "Brick 102 / Air gap 50 / …", thicknesses in mm.
    pub fn summary(&self, model: &Model) -> String {
        self.layers
            .iter()
            .map(|l| {
                let name = l
                    .material
                    .and_then(|m| model.get(m))
                    .filter(|e| matches!(e.kind, ElementKind::Material(_)))
                    .map_or(l.function.label(), |e| e.name.as_str());
                format!("{name} {}", trim_mm(l.thickness))
            })
            .collect::<Vec<_>>()
            .join(" / ")
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.layers.is_empty() {
            return Err("an assembly needs at least one layer".into());
        }
        if self
            .layers
            .iter()
            .any(|l| !l.thickness.is_finite() || l.thickness < MIN_LAYER - 1e-12)
        {
            return Err("every layer needs a thickness of at least 1 mm".into());
        }
        Ok(())
    }
}

/// A length in whole millimetres, or tenths where it has them ("12.5").
fn trim_mm(m: f64) -> String {
    let mm = m * 1000.0;
    if (mm - mm.round()).abs() < 1e-6 {
        format!("{}", mm.round() as i64)
    } else {
        format!("{:.1}", mm)
    }
}

/// One layer of a wall, placed across it: `s0..s1` is its span across the
/// wall (left of start → end positive, centreline 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub index: usize,
    pub s0: f64,
    pub s1: f64,
    pub material: Option<ElementId>,
    pub function: LayerFunction,
    pub core: bool,
    /// Its join priority ([`Layer::join_priority`]).
    pub priority: u8,
    /// How far it runs past the wall's start and end, edited by hand
    /// (see [`crate::Wall::layer_ends`]); zero for slabs and roofs.
    pub ext: [f64; 2],
    /// How far it runs below the wall's base and above its top, edited by
    /// hand; zero for slabs and roofs.
    pub vext: [f64; 2],
}

/// A wall's layers across it, exterior first. The exterior is on the
/// wall's right (the outside of walls drawn anticlockwise round a
/// building), or its left when flipped.
pub fn wall_bands(a: &Assembly, wall: &Wall) -> Vec<Band> {
    let t = wall.thickness / 2.0;
    a.spans(wall.thickness)
        .into_iter()
        .zip(&a.layers)
        .enumerate()
        .map(|(i, ((d0, d1), l))| {
            let (s0, s1) = if wall.flip {
                (t - d1, t - d0)
            } else {
                (-t + d0, -t + d1)
            };
            Band {
                index: i,
                s0,
                s1,
                material: l.material,
                function: l.function,
                core: a.is_core(i),
                priority: l.join_priority(),
                ext: wall.layer_ext(i),
                vext: wall.layer_vext(i),
            }
        })
        .collect()
}

/// The span of a wall's core across it, `(low, high)`.
pub fn wall_core(a: &Assembly, wall: &Wall) -> (f64, f64) {
    let bands = wall_bands(a, wall);
    let (i, j) = a.core();
    let (lo, hi) = (bands[i], bands[j]);
    (lo.s0.min(hi.s0), lo.s1.max(hi.s1))
}

/// A slab's or roof's layers, top first: each layer's depth below the top,
/// `(from, to)`, scaled to `thickness`.
pub fn depth_bands(a: &Assembly, thickness: f64) -> Vec<Band> {
    a.spans(thickness)
        .into_iter()
        .zip(&a.layers)
        .enumerate()
        .map(|(i, ((d0, d1), l))| Band {
            index: i,
            s0: d0,
            s1: d1,
            material: l.material,
            function: l.function,
            core: a.is_core(i),
            priority: l.join_priority(),
            ext: [0.0; 2],
            vext: [0.0; 2],
        })
        .collect()
}

/// An element's assembly slot: `Some(None)` for a wall, slab or roof with
/// none, `None` for kinds that can't have one.
pub fn assembly_slot(e: &Element) -> Option<Option<ElementId>> {
    match &e.kind {
        ElementKind::Wall(w) => Some(w.assembly),
        ElementKind::Slab(s) => Some(s.assembly),
        ElementKind::Roof(r) => Some(r.assembly),
        _ => None,
    }
}

/// The assembly an element is built of, if it has one that still exists
/// and has layers.
pub fn assembly_of<'a>(model: &'a Model, e: &Element) -> Option<(&'a Element, &'a Assembly)> {
    let id = assembly_slot(e)??;
    let a = model.get(id)?;
    match &a.kind {
        ElementKind::Assembly(asm) if !asm.layers.is_empty() && asm.total() > 1e-9 => {
            Some((a, asm))
        }
        _ => None,
    }
}

/// The element's thickness set to `t` the way each kind keeps it: a slab
/// keeps its top where it is.
fn set_thickness(e: &mut Element, t: f64) {
    match &mut e.kind {
        ElementKind::Wall(w) => w.thickness = t,
        ElementKind::Slab(s) => s.set_thickness(t, false),
        ElementKind::Roof(r) => r.thickness = t,
        _ => {}
    }
}

fn thickness(e: &Element) -> Option<f64> {
    match &e.kind {
        ElementKind::Wall(w) => Some(w.thickness),
        ElementKind::Slab(s) => Some(s.thickness),
        ElementKind::Roof(r) => Some(r.thickness),
        _ => None,
    }
}

/// Build `e` of `assembly` (`None`: back to a single material, keeping its
/// thickness). False for kinds that can't have one.
pub fn set_assembly(model: &Model, e: &mut Element, assembly: Option<ElementId>) -> bool {
    match &mut e.kind {
        ElementKind::Wall(w) => w.assembly = assembly,
        ElementKind::Slab(s) => s.assembly = assembly,
        ElementKind::Roof(r) => r.assembly = assembly,
        _ => return false,
    }
    let layers = assembly_of(model, e).map(|(_, a)| {
        set_thickness(e, a.total());
        a.layers.len()
    });
    // a layer the new type doesn't have drops its edited ends and edges
    let keep = layers.unwrap_or(0);
    match &mut e.kind {
        ElementKind::Wall(w) => w.layer_ends.retain(|l| l.layer < keep),
        ElementKind::Slab(s) => s.layer_edges.retain(|l| l.layer < keep),
        ElementKind::Roof(r) => r.layer_edges.retain(|l| l.layer < keep),
        _ => {}
    }
    true
}

/// Every assembly of the model: (id, name, assembly), in model order.
pub fn assemblies(model: &Model) -> Vec<(ElementId, &str, &Assembly)> {
    model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Assembly(a) => Some((e.id, e.name.as_str(), a)),
            _ => None,
        })
        .collect()
}

/// The elements built of an assembly.
pub fn users(model: &Model, assembly: ElementId) -> Vec<ElementId> {
    model
        .iter()
        .filter(|e| assembly_slot(e) == Some(Some(assembly)))
        .map(|e| e.id)
        .collect()
}

/// Make `assembly` what each of `elements` is built of (those it fits).
/// Returns the transaction and how many elements it changes.
pub fn assign(
    model: &Model,
    elements: &[ElementId],
    assembly: Option<ElementId>,
) -> (Transaction, usize) {
    let fits = |e: &Element| match assembly.and_then(|a| model.get(a)).map(|a| &a.kind) {
        Some(ElementKind::Assembly(a)) => AssemblyUse::of(&e.kind) == Some(a.usage),
        _ => assembly.is_none(),
    };
    let mut tx = Transaction::new("Assign assembly");
    let mut n = 0;
    for id in elements {
        let Some(e) = model.get(*id) else { continue };
        if assembly_slot(e).is_none() || !fits(e) {
            continue;
        }
        let mut after = e.clone();
        set_assembly(model, &mut after, assembly);
        if after != *e {
            tx = tx.replace(e.clone(), after);
            n += 1;
        }
    }
    (tx, n)
}

/// Change an assembly, and the thickness of everything built of it to
/// its new total. One undo step.
pub fn edit(model: &Model, id: ElementId, new: Assembly, label: &str) -> Option<Transaction> {
    let e = model.get(id)?;
    let ElementKind::Assembly(_) = &e.kind else {
        return None;
    };
    let mut after = e.clone();
    let total = new.total();
    after.kind = ElementKind::Assembly(new);
    let mut tx = Transaction::new(label).replace(e.clone(), after);
    if total > 1e-9 {
        for u in users(model, id) {
            let Some(ue) = model.get(u) else { continue };
            if thickness(ue).is_some_and(|t| (t - total).abs() > 1e-9) {
                let mut ua = ue.clone();
                set_thickness(&mut ua, total);
                tx = tx.replace(ue.clone(), ua);
            }
        }
    }
    Some(tx)
}

/// Delete an assembly; what was built of it keeps its thickness, as a
/// single-material element. One undo step.
pub fn delete(model: &Model, id: ElementId) -> Transaction {
    let (mut tx, _) = assign(model, &users(model, id), None);
    tx.label = "Delete assembly".into();
    tx.remove_cascade(model, id)
}

/// A name no other assembly of the model has: `base`, else `base 2`...
pub fn unique_name(model: &Model, base: &str) -> String {
    let taken: Vec<&str> = assemblies(model).into_iter().map(|(_, n, _)| n).collect();
    if !taken.contains(&base) {
        return base.to_string();
    }
    (2..)
        .map(|i| format!("{base} {i}"))
        .find(|n| !taken.contains(&n.as_str()))
        .expect("an unused name exists")
}

/// A copy of an assembly under an unused name, not yet in the model.
pub fn duplicate(model: &Model, id: ElementId) -> Option<Element> {
    let e = model.get(id)?;
    let ElementKind::Assembly(a) = &e.kind else {
        return None;
    };
    let name = unique_name(model, &format!("{} copy", e.name));
    Some(Element::new(name, ElementKind::Assembly(a.clone())))
}

/// A layer of a starter assembly: function, library material, millimetres.
type Starter = (LayerFunction, Option<&'static str>, f64);

/// The starter assemblies: (name, use, description, layers).
pub const STARTERS: &[(&str, AssemblyUse, &str, &[Starter])] = &[
    (
        "Exterior wall 365, brick cavity",
        AssemblyUse::Wall,
        "Brick outer leaf, ventilated cavity, insulation, block inner leaf, plaster",
        &[
            (LayerFunction::Finish, Some("Brick, common red"), 102.0),
            (LayerFunction::AirGap, None, 50.0),
            (
                LayerFunction::Insulation,
                Some("Insulation, mineral wool"),
                100.0,
            ),
            (LayerFunction::Structure, Some("Block, concrete"), 100.0),
            (LayerFunction::Finish, Some("Plaster, gypsum"), 13.0),
        ],
    ),
    (
        "Interior wall 126, block",
        AssemblyUse::Wall,
        "Plastered blockwork partition",
        &[
            (LayerFunction::Finish, Some("Plaster, gypsum"), 13.0),
            (LayerFunction::Structure, Some("Block, concrete"), 100.0),
            (LayerFunction::Finish, Some("Plaster, gypsum"), 13.0),
        ],
    ),
    (
        "Floor 300, concrete",
        AssemblyUse::Floor,
        "Tiles on screed on insulation on a concrete slab",
        &[
            (LayerFunction::Finish, Some("Ceramic tile"), 10.0),
            (LayerFunction::Substrate, Some("Screed, cement"), 65.0),
            (LayerFunction::Insulation, Some("Insulation, EPS"), 25.0),
            (
                LayerFunction::Structure,
                Some("Concrete, cast in place"),
                200.0,
            ),
        ],
    ),
    (
        "Pitched roof 288, tiled",
        AssemblyUse::Roof,
        "Clay tiles on battens, membrane, insulation, rafters, plasterboard ceiling",
        &[
            (LayerFunction::Finish, Some("Roof tile, clay"), 25.0),
            (LayerFunction::AirGap, None, 38.0),
            (LayerFunction::Membrane, Some("Membrane, bitumen"), 2.0),
            (
                LayerFunction::Insulation,
                Some("Insulation, wood fibre"),
                60.0,
            ),
            (LayerFunction::Structure, Some("Timber, softwood"), 150.0),
            (LayerFunction::Finish, Some("Plasterboard"), 13.0),
        ],
    ),
];

/// Add the starter assemblies the model doesn't have yet (by name), with
/// the library materials they use that it doesn't have. Returns the
/// transaction and the ids of the starters (new or already there).
pub fn add_starters(model: &Model) -> (Transaction, Vec<ElementId>) {
    let mut tx = Transaction::new("Add starter assemblies");
    let mut materials: Vec<(String, ElementId)> = crate::library::materials(model)
        .into_iter()
        .map(|(id, n, _)| (n.to_string(), id))
        .collect();
    let mut ids = vec![];
    for (name, usage, description, layers) in STARTERS {
        if let Some((id, _, _)) = assemblies(model).into_iter().find(|(_, n, _)| n == name) {
            ids.push(id);
            continue;
        }
        let mut out = vec![];
        for (function, material, mm) in layers.iter() {
            let mut m = None;
            if let Some(name) = material {
                if let Some((_, id)) = materials.iter().find(|(n, _)| n == name) {
                    m = Some(*id);
                } else if let Some(entry) = crate::library::LIBRARY.iter().find(|e| e.name == *name)
                {
                    let e = entry.element();
                    m = Some(e.id);
                    materials.push((name.to_string(), e.id));
                    tx = tx.insert(e);
                }
            }
            out.push(Layer::new(*function, m, mm / 1000.0));
        }
        let mut a = Assembly::new(*usage, out);
        a.description = description.to_string();
        let e = Element::new(*name, ElementKind::Assembly(a));
        ids.push(e.id);
        tx = tx.insert(e);
    }
    (tx, ids)
}

/// When a material is deleted, the layers made of it take `replacement`.
pub fn replace_material(
    model: &Model,
    material: ElementId,
    replacement: Option<ElementId>,
) -> Vec<(Element, Element)> {
    model
        .iter()
        .filter_map(|e| {
            let ElementKind::Assembly(a) = &e.kind else {
                return None;
            };
            if !a.layers.iter().any(|l| l.material == Some(material)) {
                return None;
            }
            let mut a = a.clone();
            for l in &mut a.layers {
                if l.material == Some(material) {
                    l.material = replacement;
                }
            }
            let mut after = e.clone();
            after.kind = ElementKind::Assembly(a);
            Some((e.clone(), after))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_frame_stands_against_the_face_of_the_layer_it_is_set_to() {
        use crate::{FrameAt, LayerFace, OpeningFrame};
        use glam::DVec2;
        // 20 render, 100 insulation, 160 block, 20 plaster: 300 in all,
        // exterior on the wall's right (negative across it)
        let a = Assembly::new(
            AssemblyUse::Wall,
            vec![
                Layer::new(LayerFunction::Finish, None, 0.02),
                Layer::new(LayerFunction::Insulation, None, 0.10),
                Layer::new(LayerFunction::Structure, None, 0.16),
                Layer::new(LayerFunction::Finish, None, 0.02),
            ],
        );
        let mut w = Wall::new(
            ElementId::new(),
            DVec2::ZERO,
            DVec2::new(5.0, 0.0),
            0.3,
            3.0,
        );
        let mut o = Opening {
            host: ElementId::new(),
            kind: OpeningKind::Window,
            offset: 1.0,
            sill: 0.9,
            width: 1.2,
            height: 1.2,
            material: None,
            frame: OpeningFrame {
                depth: 0.08,
                ..Default::default()
            },
            flip_hand: false,
            flip_facing: false,
            wrap: None,
            frame_at: None,
        };
        let near = |x: f64, y: f64| assert!((x - y).abs() < 1e-9, "{x} {y}");
        let at = |layer, face, offset| {
            Some(FrameAt {
                layer,
                face,
                offset,
            })
        };
        // unset: the middle of the wall
        near(frame_shift_in(&w, Some(&a), &o), 0.0);
        // the block's exterior face is 120 in from the outside (-0.03):
        // the frame's outer face on it, the frame inwards of it
        o.frame_at = at(2, LayerFace::Exterior, 0.0);
        near(frame_shift_in(&w, Some(&a), &o), -0.03 + 0.04);
        // the insulation's interior face is the same plane: the frame's
        // inner face on it, the frame in the insulation
        o.frame_at = at(1, LayerFace::Interior, 0.0);
        near(frame_shift_in(&w, Some(&a), &o), -0.03 - 0.04);
        // flush with the outside of the wall, and with the inside
        o.frame_at = at(0, LayerFace::Exterior, 0.0);
        near(frame_shift_in(&w, Some(&a), &o), -0.15 + 0.04);
        o.frame_at = at(3, LayerFace::Interior, 0.0);
        near(frame_shift_in(&w, Some(&a), &o), 0.15 - 0.04);
        // an offset moves it towards the interior; it never leaves the wall
        o.frame_at = at(0, LayerFace::Exterior, 0.05);
        near(frame_shift_in(&w, Some(&a), &o), -0.15 + 0.04 + 0.05);
        o.frame_at = at(0, LayerFace::Exterior, -1.0);
        near(frame_shift_in(&w, Some(&a), &o), -0.11);
        // a flipped wall has its exterior on the other side
        w.flip = true;
        o.frame_at = at(0, LayerFace::Exterior, 0.0);
        near(frame_shift_in(&w, Some(&a), &o), 0.15 - 0.04);
        w.flip = false;
        // a wall of one material is one layer
        o.frame_at = at(0, LayerFace::Interior, 0.0);
        near(frame_shift_in(&w, None, &o), 0.11);
        // the wrapping layers run to the frame's faces, wherever it stands
        let mut wrapping = a.clone();
        wrapping.wrap_openings = WrapSides::Both;
        o.frame_at = at(2, LayerFace::Exterior, 0.0);
        let shift = frame_shift_in(&w, Some(&wrapping), &o);
        let wrap = opening_wrap(&wrapping, &w, &o, 0.08, shift);
        let mut stops: Vec<f64> = wrap.stops().collect();
        stops.sort_by(|x, y| x.total_cmp(y));
        near(stops[0], -0.03);
        near(stops[1], 0.05);
    }

    use super::*;
    use crate::Document;

    fn house_with_starters() -> (Document, Vec<ElementId>) {
        let mut doc = Document::from_model(crate::sample::sample_house());
        let (tx, ids) = add_starters(doc.model());
        doc.commit(tx).unwrap();
        (doc, ids)
    }

    #[test]
    fn starters_add_once_with_their_materials() {
        let (doc, ids) = house_with_starters();
        assert_eq!(ids.len(), STARTERS.len());
        let wall = match &doc.model().get(ids[0]).unwrap().kind {
            ElementKind::Assembly(a) => a.clone(),
            _ => unreachable!(),
        };
        assert!((wall.total() - 0.365).abs() < 1e-9);
        assert_eq!(wall.core(), (3, 3));
        assert!(wall.layers[1].material.is_none());
        assert!(wall.layers.iter().enumerate().all(|(i, l)| {
            l.material.is_none() || doc.model().get(l.material.unwrap()).is_some() || i == 1
        }));
        // a second time adds nothing
        let (tx, again) = add_starters(doc.model());
        assert!(tx.ops.is_empty());
        assert_eq!(again, ids);
    }

    #[test]
    fn assigning_and_editing_keep_thicknesses_in_step() {
        let (mut doc, ids) = house_with_starters();
        let walls: Vec<ElementId> = doc
            .model()
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
            .map(|e| e.id)
            .collect();
        let slab = doc
            .model()
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Slab(_)))
            .unwrap()
            .clone();
        let mut all = walls.clone();
        all.push(slab.id);
        // a wall assembly only goes on walls
        let (tx, n) = assign(doc.model(), &all, Some(ids[0]));
        assert_eq!(n, walls.len());
        doc.commit(tx).unwrap();
        for w in &walls {
            let ElementKind::Wall(w) = &doc.model().get(*w).unwrap().kind else {
                unreachable!()
            };
            assert!((w.thickness - 0.365).abs() < 1e-9);
        }
        // a thicker layer thickens every wall built of it, in one step
        let ElementKind::Assembly(mut a) = doc.model().get(ids[0]).unwrap().kind.clone() else {
            unreachable!()
        };
        a.layers[2].thickness = 0.150;
        let tx = edit(doc.model(), ids[0], a, "Layer thickness").unwrap();
        doc.commit(tx).unwrap();
        let ElementKind::Wall(w) = &doc.model().get(walls[0]).unwrap().kind else {
            unreachable!()
        };
        assert!((w.thickness - 0.415).abs() < 1e-9);
        // the slab keeps its top when it takes the floor
        let (tx, n) = assign(doc.model(), &[slab.id], Some(ids[2]));
        assert_eq!(n, 1);
        doc.commit(tx).unwrap();
        let (ElementKind::Slab(before), ElementKind::Slab(after)) =
            (&slab.kind, &doc.model().get(slab.id).unwrap().kind)
        else {
            unreachable!()
        };
        assert!((after.thickness - 0.3).abs() < 1e-9);
        assert!((after.top_offset - before.top_offset).abs() < 1e-9);
        // deleting the assembly leaves single-material walls as thick as they were
        let model = doc.model().clone();
        doc.commit(delete(&model, ids[0])).unwrap();
        let ElementKind::Wall(w) = &doc.model().get(walls[0]).unwrap().kind else {
            unreachable!()
        };
        assert_eq!(w.assembly, None);
        assert!((w.thickness - 0.415).abs() < 1e-9);
        doc.undo();
        assert_eq!(users(doc.model(), ids[0]).len(), walls.len());
    }

    #[test]
    fn bands_run_from_the_exterior_on_the_right() {
        let (doc, ids) = house_with_starters();
        let ElementKind::Assembly(a) = &doc.model().get(ids[0]).unwrap().kind else {
            unreachable!()
        };
        let mut w = Wall {
            level: ElementId::new(),
            start: glam::DVec2::ZERO,
            end: glam::DVec2::new(4.0, 0.0),
            thickness: 0.365,
            height: 3.0,
            base_offset: 0.0,
            material: None,
            assembly: Some(ids[0]),
            flip: false,
            location: Default::default(),
            top: Default::default(),
            disallow_join: [false; 2],
            attach_top: vec![],
            curtain: None,
            layer_ends: vec![],
            bulge: 0.0,
        };
        let b = wall_bands(a, &w);
        assert!((b[0].s0 + 0.1825).abs() < 1e-9 && (b[0].s1 + 0.0805).abs() < 1e-9);
        assert!((b[4].s1 - 0.1825).abs() < 1e-9);
        assert!(b[3].core && !b[2].core);
        let (c0, c1) = wall_core(a, &w);
        assert!((c1 - c0 - 0.1).abs() < 1e-9);
        w.flip = true;
        let b = wall_bands(a, &w);
        assert!((b[0].s1 - 0.1825).abs() < 1e-9);
        assert_eq!(a.summary(doc.model()).split(" / ").count(), 5);
    }

    #[test]
    fn functions_read_back_from_ifc_categories() {
        for f in LayerFunction::ALL {
            assert_eq!(LayerFunction::from_ifc(f.ifc_category(), false), f);
        }
        assert_eq!(
            LayerFunction::from_ifc("anything", true),
            LayerFunction::AirGap
        );
        assert_eq!(
            LayerFunction::from_ifc("Thermal insulation", false),
            LayerFunction::Insulation
        );
    }

    #[test]
    fn single_material_elements_from_older_files_still_read() {
        let level = "00000000-0000-0000-0000-000000000001";
        let wall = format!(
            r#"{{"level":"{level}","start":[0.0,0.0],"end":[4.0,0.0],"thickness":0.2,"height":3.0,"base_offset":0.0,"material":null}}"#
        );
        let w: Wall = serde_json::from_str(&wall).unwrap();
        assert_eq!((w.assembly, w.flip), (None, false));
        let slab = format!(
            r#"{{"level":"{level}","boundary":[[0.0,0.0],[4.0,0.0],[4.0,3.0]],"thickness":0.2}}"#
        );
        let s: crate::Slab = serde_json::from_str(&slab).unwrap();
        assert_eq!(s.assembly, None);
        let roof = format!(
            r#"{{"level":"{level}","boundary":[[0.0,0.0],[4.0,0.0],[4.0,3.0]],"shape":"Hip","pitch":30.0,"overhang":0.5,"thickness":0.25}}"#
        );
        let r: crate::Roof = serde_json::from_str(&roof).unwrap();
        assert_eq!(r.assembly, None);
        // and without one, nothing new is written: older CreaBIMs read them
        let out = serde_json::to_string(&w).unwrap();
        assert!(!out.contains("assembly") && !out.contains("flip"), "{out}");
        assert!(!serde_json::to_string(&s).unwrap().contains("assembly"));
        // a whole saved project, assemblies and all, reads back the same
        let (mut doc, ids) = house_with_starters();
        let walls: Vec<ElementId> = doc
            .model()
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
            .map(|e| e.id)
            .collect();
        let (tx, _) = assign(doc.model(), &walls, Some(ids[0]));
        doc.commit(tx).unwrap();
        let back = Model::from_json(&doc.model().to_json()).unwrap();
        assert_eq!(&back, doc.model());
    }
}
