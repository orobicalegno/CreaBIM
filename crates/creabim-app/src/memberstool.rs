//! The Column, Beam, Stair and Railing tools (the Roof tool draws its
//! outline with the Slab tool's modes), their settings for new elements,
//! and the Properties panel for every element they make.

use crate::planview::{PlanState, Tool};
use crate::selection::Selection;
use creabim_core::members::{self, Landing, Profile, RoofShape};
use creabim_core::{
    Beam, Column, Document, Element, ElementId, ElementKind, Model, ModelError, Railing, Roof,
    Slab, Stair, Transaction,
};
use eframe::egui::{self, Pos2, Stroke};
use glam::{DVec2, DVec3};

/// Sizes and shapes for new elements, set in the tool bar.
#[derive(Clone, Debug)]
pub struct MemberSettings {
    pub column: Profile,
    pub beam_width: f64,
    pub beam_depth: f64,
    pub roof_shape: RoofShape,
    pub roof_pitch: f64,
    pub stair_width: f64,
    /// New stairs turn on a landing halfway up.
    pub stair_turn: bool,
    pub railing_height: f64,
    /// The railing path clicked so far.
    pub path: Vec<DVec2>,
    /// New footings are pads (else strips), unless the tool's type says.
    pub footing_pad: bool,
    /// Width of new strips, and the sides of new pads.
    pub footing_width: f64,
    pub footing_thickness: f64,
    /// New doors', windows' and bare holes' width, height and sill, set in
    /// the bar before placing (MU18); a type, when picked, sets its own.
    pub door: [f64; 3],
    pub window: [f64; 3],
    pub hole: [f64; 3],
    /// The library profile new columns and beams take; none, their own
    /// square, round or width × depth (MU40).
    pub column_section: Option<String>,
    pub beam_section: Option<String>,
    /// How a chain of beams meets at its corners (MU40).
    pub beam_corner: creabim_core::members::BeamCorner,
    /// Beams drawn in 3D lie on the face clicked (a roof slope, a wall),
    /// square to it, rather than level (MU40).
    pub beam_on_face: bool,
    /// The library profile new railings' top rail takes (MU40).
    pub railing_section: Option<String>,
    /// The beam this chain placed last: the next joins only it, and a new
    /// chain starts with none (MU40: a new Beam command joined the old one).
    pub last_beam: Option<ElementId>,
}

impl Default for MemberSettings {
    fn default() -> Self {
        Self {
            column: Profile::Rectangle {
                width: 0.3,
                depth: 0.3,
            },
            beam_width: 0.2,
            beam_depth: 0.4,
            roof_shape: RoofShape::Hip,
            roof_pitch: 30.0,
            stair_width: 1.0,
            stair_turn: false,
            railing_height: 1.0,
            path: vec![],
            footing_pad: false,
            footing_width: creabim_core::footing::STRIP_WIDTH,
            footing_thickness: creabim_core::footing::STRIP_THICKNESS,
            door: [0.9, 2.1, 0.0],
            window: [1.2, 1.3, 0.9],
            hole: [1.0, 1.0, 1.0],
            column_section: None,
            beam_section: None,
            beam_corner: Default::default(),
            beam_on_face: false,
            railing_section: None,
            last_beam: None,
        }
    }
}

impl MemberSettings {
    /// Width (or diameter) of new columns.
    pub fn column_size(&self) -> f64 {
        match self.column {
            Profile::Rectangle { width, .. } => width,
            Profile::Round { diameter } => diameter,
        }
    }

    pub fn set_column_size(&mut self, v: f64) {
        self.column = match self.column {
            Profile::Rectangle { .. } => Profile::Rectangle { width: v, depth: v },
            Profile::Round { .. } => Profile::Round { diameter: v },
        };
    }

    pub fn round_columns(&self) -> bool {
        matches!(self.column, Profile::Round { .. })
    }

    pub fn set_round(&mut self, round: bool) {
        self.column_section = None;
        let v = self.column_size();
        self.column = if round {
            Profile::Round { diameter: v }
        } else {
            Profile::Rectangle { width: v, depth: v }
        };
    }
}

/// A roof from an outline drawn with the Slab tool's modes.
pub fn roof_from_slab(slab: &Slab, shape: RoofShape, pitch: f64) -> Element {
    let mut r = Roof::new(slab.level, slab.outline(), shape);
    r.pitch = pitch;
    r.base_offset = slab.top_offset;
    Element::new("Roof", ElementKind::Roof(r))
}

/// What the Slab tool's modes make: a slab, or with the Roof tool a roof.
pub fn outline_element(
    model: &creabim_core::Model,
    state: &PlanState,
    slab: Slab,
) -> (Element, &'static str) {
    if state.tool == Tool::Roof {
        let m = &state.members;
        (
            state.typed(model, roof_from_slab(&slab, m.roof_shape, m.roof_pitch)),
            "Draw roof",
        )
    } else if state.tool == Tool::Shaft {
        (
            crate::outline::shaft_from_outline(model, slab.level, slab.outline()),
            "Draw shaft",
        )
    } else {
        (
            state.typed(model, Element::new("Slab", ElementKind::Slab(slab))),
            "Draw slab",
        )
    }
}

/// A click with the Column, Beam, Stair or Railing tool on `level`.
pub fn click(
    doc: &mut Document,
    level: ElementId,
    tool: Tool,
    p: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let set = state.members.clone();
    let add = |doc: &mut Document, selection: &mut Selection, e: Element, label: &str| {
        selection.only(e.id);
        doc.commit(Transaction::new(label).insert(e))
    };
    match tool {
        Tool::Column => {
            let mut c = Column::new(level, p, set.column);
            // a profile from the library, if one is picked (MU40)
            let lib = set
                .column_section
                .as_deref()
                .and_then(|n| Some((n, creabim_core::profiles::find(doc.model(), n)?)));
            if let Some((n, pts)) = &lib {
                c.section = Some(creabim_core::members::Section {
                    name: n.to_string(),
                    points: pts.clone(),
                });
            }
            state.status = Some(format!(
                "Column {} placed. Its height runs to the next level; drag its top handle or set a height in Properties.",
                lib.map_or_else(|| set.column.label(), |(n, _)| n.to_string())
            ));
            let e = state.typed(doc.model(), Element::new("Column", ElementKind::Column(c)));
            add(doc, selection, e, "Place column")
        }
        Tool::Beam => {
            match state.pending {
                None => {
                    state.members.last_beam = None;
                    state.pending = Some(p);
                    state.status = Some("Click the beam's other end.".into());
                    Ok(())
                }
                Some(a) if a.distance(p) > 0.05 => {
                    let mut b = Beam::new(level, a, p);
                    b.width = set.beam_width;
                    b.depth = set.beam_depth;
                    // a profile from the library, if one is picked (MU40)
                    if let Some(n) = set.beam_section.as_deref() {
                        if let Some(pts) = creabim_core::profiles::find(doc.model(), n) {
                            b.section = Some(creabim_core::members::Section {
                                name: n.to_string(),
                                points: pts,
                            });
                        }
                    }
                    // under the slab above, not inside it
                    b.top_offset = creabim_core::members::beam_top_offset(doc.model(), level, a, p);
                    state.pending = Some(p);
                    state.status = Some(
                        "Beam added. Click on to chain another; Esc or right-click ends.".into(),
                    );
                    let e = state.typed(doc.model(), Element::new("Beam", ElementKind::Beam(b)));
                    // the beam before it in the chain, ending here: the two
                    // meet as the tool's Corners says (MU40)
                    let prev = state
                    .members
                    .last_beam
                    .and_then(|id| doc.model().get(id).cloned())
                    .filter(|pe| matches!(&pe.kind, ElementKind::Beam(pb) if pb.end.distance(a) < 1e-6));
                    let stood_on = state.members.last_beam;
                    let beam = e.id;
                    state.members.last_beam = Some(e.id);
                    let done = match prev {
                        Some(pe) => {
                            let (mut pe2, mut ne) = (pe.clone(), e);
                            if let (ElementKind::Beam(pb), ElementKind::Beam(nb)) =
                                (&mut pe2.kind, &mut ne.kind)
                            {
                                creabim_core::members::join_beams(pb, nb, set.beam_corner);
                            }
                            selection.only(ne.id);
                            doc.commit(Transaction::new("Draw beam").replace(pe, pe2).insert(ne))
                        }
                        None => add(doc, selection, e, "Draw beam"),
                    };
                    if done.is_ok() {
                        state.note_chain_piece(doc, a, p, stood_on, Some(beam));
                    }
                    done
                }
                Some(_) => Ok(()),
            }
        }
        Tool::Stair => match state.pending.take() {
            None => {
                state.pending = Some(p);
                state.status = Some("Click the way the stair climbs.".into());
                Ok(())
            }
            Some(a) if a.distance(p) > 0.05 => {
                let s = new_stair(doc.model(), level, a, p, &set);
                let lay = s.layout(s.rise(doc.model()));
                state.status = Some(format!(
                    "Stair added: {} risers of {}. Railing tool: click inside it to add its railings.",
                    lay.risers,
                    crate::units::len(lay.riser)
                ));
                add(
                    doc,
                    selection,
                    Element::new("Stair", ElementKind::Stair(s)),
                    "Draw stair",
                )
            }
            Some(_) => Ok(()),
        },
        Tool::Footing => place_footing(doc, level, p, state, selection),
        Tool::Railing => {
            if state.members.path.is_empty() {
                if let Some(stair) = stair_at(doc.model(), level, p) {
                    state.status = Some("Railings added along both sides of the stair.".into());
                    return add_stair_railings(doc, stair, set.railing_height, selection);
                }
            }
            let path = &mut state.members.path;
            if path.last().is_some_and(|q| q.distance(p) < 0.05) {
                // clicking the last point again finishes
                let pts = std::mem::take(path);
                state.pending = None;
                if pts.len() < 2 {
                    return Ok(());
                }
                let mut r = Railing::new(level, pts.iter().map(|q| q.extend(0.0)).collect());
                r.height = set.railing_height;
                r.rail_profile = set.railing_section.clone();
                state.status = Some("Railing added.".into());
                return add(
                    doc,
                    selection,
                    Element::new("Railing", ElementKind::Railing(r)),
                    "Draw railing",
                );
            }
            path.push(p);
            state.pending = Some(p);
            state.status = Some(
                "Click the next point; click the last one again to finish. Esc cancels.".into(),
            );
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The kind of footing the Footing tool places: its type's, or the bar's.
pub fn footing_kind(model: &Model, state: &PlanState) -> creabim_core::FootingKind {
    use creabim_core::{types::TypeParams, FootingKind, TypeCategory};
    let typed = state
        .types
        .get(&TypeCategory::Footing)
        .and_then(|t| match &model.get(*t)?.kind {
            ElementKind::ElementType(t) => match &t.params {
                TypeParams::Footing(f) => Some(f.kind),
                _ => None,
            },
            _ => None,
        });
    typed.unwrap_or(if state.members.footing_pad {
        FootingKind::Pad
    } else {
        FootingKind::Strip
    })
}

/// A new footing sized from the tool bar, of the tool's type if it fits.
fn sized_footing(model: &Model, state: &PlanState, mut f: creabim_core::Footing) -> Element {
    let m = &state.members;
    f.width = match (f.kind, f.host) {
        // a strip under a wall is at least as wide as the wall
        (creabim_core::FootingKind::Strip, Some(_)) => f.width.max(m.footing_width),
        _ => m.footing_width,
    };
    if f.kind == creabim_core::FootingKind::Pad {
        f.length = m.footing_width;
    }
    f.thickness = m.footing_thickness;
    let name = f.kind.label();
    state.typed(model, Element::new(name, ElementKind::Footing(f)))
}

/// A click with the Footing tool: a strip under the wall clicked, or the
/// ends of one on its own; a pad under the column clicked, or one on its own.
fn place_footing(
    doc: &mut Document,
    level: ElementId,
    p: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    use creabim_core::{Footing, FootingKind};
    let model = doc.model();
    let kind = footing_kind(model, state);
    // the host the pointer showed (Tab may have moved past the first)
    let shown = state
        .host_pick
        .filter(|h| hosts_at(model, level, Tool::Footing, state, p).contains(h));
    let host = match kind {
        FootingKind::Strip if state.pending.is_none() => shown.or_else(|| wall_at(model, level, p)),
        FootingKind::Pad => shown.or_else(|| column_at(model, level, p)),
        FootingKind::Strip => None,
    };
    if let Some(h) = host {
        if !creabim_core::footing::footings_under(model, h).is_empty() {
            state.status = Some("That already has a footing under it.".into());
            return Ok(());
        }
        let (tx, ids) = creabim_core::footing::under(model, &[h], &|e| {
            if let ElementKind::Footing(f) = &e.kind {
                *e = sized_footing(model, state, f.clone());
            }
        });
        let name = model.get(h).map_or(String::new(), |e| e.name.clone());
        state.status = Some(format!(
            "{} footing placed under {name}. Ctrl+Z to undo.",
            match kind {
                FootingKind::Strip => "Strip",
                FootingKind::Pad => "Pad",
            }
        ));
        state.footing_placed = true;
        if let Some(id) = ids.first() {
            selection.only(*id);
        }
        let mut tx = tx;
        tx.label = format!("Place {}", kind.label().to_lowercase());
        return doc.commit(tx);
    }
    let add = |doc: &mut Document, selection: &mut Selection, e: Element| {
        selection.only(e.id);
        let label = format!("Place {}", kind.label().to_lowercase());
        doc.commit(Transaction::new(label).insert(e))
    };
    match kind {
        FootingKind::Pad => {
            let e = sized_footing(model, state, Footing::pad(level, p));
            state.status = Some("Pad footing placed. Set its size and depth in Properties.".into());
            add(doc, selection, e)
        }
        FootingKind::Strip => match state.pending.take() {
            None => {
                state.pending = Some(p);
                state.status = Some("Click the strip's other end.".into());
                Ok(())
            }
            Some(a) if a.distance(p) > 0.05 => {
                let e = sized_footing(model, state, Footing::strip(level, a, p));
                state.status = Some("Strip footing placed.".into());
                add(doc, selection, e)
            }
            Some(_) => Ok(()),
        },
    }
}

/// The wall on `level` under `p` (on it, or within a few centimetres).
pub fn wall_at(model: &Model, level: ElementId, p: DVec2) -> Option<ElementId> {
    walls_at(model, level, p).first().copied()
}

/// Every wall on `level` under `p`, nearest its centre first: across the
/// wall's whole thickness about its centreline (`start` and `end`), and a
/// few centimetres round it, whatever line it was drawn on.
pub fn walls_at(model: &Model, level: ElementId, p: DVec2) -> Vec<ElementId> {
    let mut hits: Vec<(f64, ElementId)> = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Wall(w) if w.level == level && w.curtain.is_none() => {
                // round the arc on a curved wall
                let (_, s) = w.along(p);
                (s <= w.thickness / 2.0 + 0.05).then_some((s, e.id))
            }
            _ => None,
        })
        .collect();
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    hits.into_iter().map(|(_, id)| id).collect()
}

/// The project's foundation plan: a plan view whose name says Foundation.
pub fn foundation_plan(model: &Model) -> Option<ElementId> {
    model.iter().find_map(|e| match &e.kind {
        ElementKind::View(v)
            if matches!(v.kind, creabim_core::ViewKind::Plan { .. })
                && e.name.to_lowercase().contains("foundation") =>
        {
            Some(e.id)
        }
        _ => None,
    })
}

/// Does `tool` go on a host it is clicked onto (a footing under a wall or
/// column, a door or window in a wall)?
pub fn hosted(tool: Tool) -> bool {
    matches!(
        tool,
        Tool::Footing | Tool::Door | Tool::Window | Tool::Opening
    )
}

/// The hosts `tool` could go on at `p`, best first, before its first
/// click: these win over snaps (A3-2). None once a free strip has its
/// first point.
pub fn hosts_at(
    model: &Model,
    level: ElementId,
    tool: Tool,
    state: &PlanState,
    p: DVec2,
) -> Vec<ElementId> {
    use creabim_core::FootingKind;
    if state.pending.is_some() {
        return vec![];
    }
    match tool {
        Tool::Footing => match footing_kind(model, state) {
            FootingKind::Strip => walls_at(model, level, p),
            FootingKind::Pad => column_at(model, level, p).into_iter().collect(),
        },
        Tool::Door | Tool::Window | Tool::Opening => walls_at(model, level, p),
        _ => vec![],
    }
}

/// What a click would do on `host`, as the preview by the pointer says it:
/// "Strip 600 × 300 under W12", "Door in W12".
pub fn host_preview(model: &Model, state: &PlanState, tool: Tool, host: ElementId) -> String {
    let name = model.get(host).map_or(String::new(), |e| e.name.clone());
    match tool {
        Tool::Footing => {
            let kind = footing_kind(model, state);
            let mut f = match kind {
                creabim_core::FootingKind::Strip => {
                    creabim_core::Footing::strip(ElementId::NIL, DVec2::ZERO, DVec2::X)
                }
                creabim_core::FootingKind::Pad => {
                    creabim_core::Footing::pad(ElementId::NIL, DVec2::ZERO)
                }
            };
            f.host = Some(host);
            if let Some(ElementKind::Wall(w)) = model.get(host).map(|e| &e.kind) {
                f.width = w.thickness;
            }
            let ElementKind::Footing(f) = sized_footing(model, state, f).kind else {
                return name;
            };
            let mm = |v: f64| crate::units::len(v).trim_end_matches(" mm").to_string();
            let taken = !creabim_core::footing::footings_under(model, host).is_empty();
            if taken {
                return format!("{name} already has a footing");
            }
            match kind {
                creabim_core::FootingKind::Strip => {
                    format!("Strip {} × {} under {name}", mm(f.width), mm(f.thickness))
                }
                creabim_core::FootingKind::Pad => format!(
                    "Pad {} × {} × {} under {name}",
                    mm(f.width),
                    mm(f.length),
                    mm(f.thickness)
                ),
            }
        }
        Tool::Door => format!("Door in {name}"),
        Tool::Window => format!("Window in {name}"),
        _ => format!("Opening in {name}"),
    }
}

/// The column standing on `level` under `p`.
pub fn column_at(model: &Model, level: ElementId, p: DVec2) -> Option<ElementId> {
    model.iter().find_map(|e| match &e.kind {
        ElementKind::Column(c) if c.level == level => {
            let o = c.outline();
            (inside(&o, p) || c.position.distance(p) < 0.1).then_some(e.id)
        }
        _ => None,
    })
}

/// A stair from `a` climbing towards `b`, sized from the tool's settings.
pub fn new_stair(
    model: &Model,
    level: ElementId,
    a: DVec2,
    b: DVec2,
    set: &MemberSettings,
) -> Stair {
    let mut s = Stair::new(level, a, (b - a).to_angle().to_degrees());
    s.width = set.stair_width;
    if set.stair_turn {
        let n = members::riser_count(s.rise(model), s.max_riser);
        s.landing = Some(Landing {
            after: n / 2,
            left: true,
        });
    }
    s
}

/// The stair on `level` whose footprint holds `p`.
fn stair_at(model: &Model, level: ElementId, p: DVec2) -> Option<ElementId> {
    model.iter().find_map(|e| match &e.kind {
        ElementKind::Stair(s) if s.level == level => {
            let rise = s.rise(model);
            let mut shapes = s.flight_outlines(rise);
            if let Some((l, _)) = s.layout(rise).landing {
                shapes.push(l);
            }
            shapes.iter().any(|o| inside(o, p)).then_some(e.id)
        }
        _ => None,
    })
}

fn inside(poly: &[DVec2], p: DVec2) -> bool {
    let n = poly.len();
    let mut c = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            c = !c;
        }
    }
    c
}

/// Railings on both sides of `stair`, as one undo step.
pub fn add_stair_railings(
    doc: &mut Document,
    stair: ElementId,
    height: f64,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let mut tx = Transaction::new("Add stair railings");
    let mut first = None;
    for left in [true, false] {
        if let Some(mut r) = Railing::along_stair(doc.model(), stair, left) {
            r.height = height;
            let e = Element::new("Railing", ElementKind::Railing(r));
            first.get_or_insert(e.id);
            tx = tx.insert(e);
        }
    }
    if let Some(id) = first {
        selection.only(id);
    }
    doc.commit(tx)
}

/// What the tool would make, drawn while its second point is chosen:
/// a beam's width, a stair's flights, a railing's path so far.
#[allow(clippy::too_many_arguments)]
pub fn preview(
    painter: &egui::Painter,
    project: &dyn Fn(DVec3) -> Option<Pos2>,
    model: &Model,
    level: Option<ElementId>,
    tool: Tool,
    state: &PlanState,
    a: DVec3,
    b: DVec3,
) {
    let stroke = Stroke::new(1.0, crate::theme::interaction());
    let z = a.z;
    let ring = |pts: &[DVec2]| {
        let scr: Option<Vec<Pos2>> = pts
            .iter()
            .chain(pts.first())
            .map(|p| project(p.extend(z)))
            .collect();
        if let Some(scr) = scr {
            painter.add(egui::Shape::line(scr, stroke));
        }
    };
    let set = &state.members;
    match (tool, level) {
        (Tool::Beam, Some(level)) if a.distance(b) > 1e-3 => {
            let mut bm = Beam::new(level, a.truncate(), b.truncate());
            bm.width = set.beam_width;
            ring(&bm.outline());
        }
        (Tool::Stair, Some(level)) if a.distance(b) > 1e-3 => {
            let s = new_stair(model, level, a.truncate(), b.truncate(), set);
            let rise = s.rise(model);
            for o in s.flight_outlines(rise) {
                ring(&o);
            }
            if let Some((l, _)) = s.layout(rise).landing {
                ring(&l);
            }
        }
        (Tool::Footing, Some(level)) if a.distance(b) > 1e-3 => {
            let mut f = creabim_core::Footing::strip(level, a.truncate(), b.truncate());
            f.width = set.footing_width;
            let n = (b - a).truncate().normalize_or_zero().perp() * f.width / 2.0;
            ring(&[
                a.truncate() - n,
                b.truncate() - n,
                b.truncate() + n,
                a.truncate() + n,
            ]);
        }
        (Tool::Railing, _) => {
            let scr: Option<Vec<Pos2>> = set.path.iter().map(|p| project(p.extend(z))).collect();
            if let Some(scr) = scr {
                painter.add(egui::Shape::line(scr, Stroke::new(2.0, stroke.color)));
            }
        }
        _ => {}
    }
}

/// The Properties panel rows for a column, beam, roof, stair or railing.
/// Returns the undo label when something changed.
pub fn properties(ui: &mut egui::Ui, model: &Model, kind: &mut ElementKind) -> &'static str {
    if matches!(kind, ElementKind::Part(_) | ElementKind::Site(_)) {
        return crate::partstool::properties(ui, model, kind);
    }
    let mut label = "";
    let num = |ui: &mut egui::Ui, text: &str, v: &mut f64, lo: f64, hi: f64| -> bool {
        crate::props::row(ui, text, |ui| {
            crate::props::fill(ui, crate::units::drag(v).range(lo..=hi)).changed()
        })
    };
    let deg = |ui: &mut egui::Ui, text: &str, v: &mut f64, lo: f64, hi: f64| -> bool {
        crate::props::row(ui, text, |ui| {
            crate::props::fill(
                ui,
                egui::DragValue::new(v)
                    .speed(0.5)
                    .range(lo..=hi)
                    .suffix("°"),
            )
            .changed()
        })
    };
    // under the fields, in their column, as Blender's notes (MU31)
    let weak = |ui: &mut egui::Ui, t: String| crate::props::note(ui, t);
    match kind {
        ElementKind::Column(c) => {
            let mut round = matches!(c.profile, Profile::Round { .. });
            if crate::props::choice(
                ui,
                "Profile",
                &mut round,
                &[(false, "Rectangular", ""), (true, "Round", "")],
            ) {
                label = "Column profile";
            }
            if label == "Column profile" {
                c.profile = match (round, c.profile) {
                    (true, Profile::Rectangle { width, .. }) => Profile::Round { diameter: width },
                    (false, Profile::Round { diameter }) => Profile::Rectangle {
                        width: diameter,
                        depth: diameter,
                    },
                    (_, p) => p,
                };
            }
            match &mut c.profile {
                Profile::Rectangle { width, depth } => {
                    if num(ui, "Width", width, 0.05, 5.0) | num(ui, "Depth", depth, 0.05, 5.0) {
                        label = "Column size"
                    }
                    if deg(ui, "Rotation", &mut c.rotation, -360.0, 360.0) {
                        label = "Turn column"
                    }
                }
                Profile::Round { diameter } => {
                    if num(ui, "Diameter", diameter, 0.05, 5.0) {
                        label = "Column size"
                    }
                }
            }
            if c.top.is_unconnected() {
                let mut fixed = c.height.is_some();
                if crate::props::switch(ui, &mut fixed, "Fixed height (else up to the next level)")
                    .changed()
                {
                    c.height = fixed.then(|| c.height_in(model));
                    label = "Column height";
                }
                if let Some(h) = &mut c.height {
                    if num(ui, "Height", h, 0.1, 100.0) {
                        label = "Column height"
                    }
                }
            }
            let l = column_levels(ui, model, c);
            if !l.is_empty() {
                label = l;
            }
            if num(ui, "X", &mut c.position.x, -1e4, 1e4)
                | num(ui, "Y", &mut c.position.y, -1e4, 1e4)
            {
                label = "Move column"
            }
            let (z0, z1) = c.span(model);
            weak(
                ui,
                format!(
                    "Base at {}, top at {}.",
                    crate::units::elevation(z0),
                    crate::units::elevation(z1)
                ),
            );
        }
        ElementKind::Footing(f) => {
            let l = footing_fields(ui, model, f);
            if !l.is_empty() {
                label = l;
            }
        }
        ElementKind::Beam(b) => {
            if num(ui, "Width", &mut b.width, 0.02, 3.0) {
                label = "Beam width"
            }
            if num(ui, "Depth", &mut b.depth, 0.02, 5.0) {
                label = "Beam depth"
            }
            if num(ui, "Below storey top", &mut b.top_offset, -10.0, 50.0) {
                label = "Beam top offset"
            }
            if num(ui, "Start X", &mut b.start.x, -1e4, 1e4)
                | num(ui, "Start Y", &mut b.start.y, -1e4, 1e4)
                | num(ui, "End X", &mut b.end.x, -1e4, 1e4)
                | num(ui, "End Y", &mut b.end.y, -1e4, 1e4)
            {
                label = "Move beam"
            }
            // a sloped beam: each end's top above the beam's top (MU40)
            if num(ui, "Start rise", &mut b.rise[0], -50.0, 50.0)
                | num(ui, "End rise", &mut b.rise[1], -50.0, 50.0)
            {
                label = "Beam slope"
            }
            // each end run past its point, or pulled back where a column
            // or wall runs through (MU40)
            if num(ui, "Start extension", &mut b.extend[0], -20.0, 20.0)
                | num(ui, "End extension", &mut b.extend[1], -20.0, 20.0)
            {
                label = "Beam extension"
            }
            let cuts = b.cuts.iter().filter(|c| c.is_some()).count();
            if cuts > 0 || b.up.is_some() {
                crate::props::row(ui, "Ends", |ui| {
                    ui.label(match cuts {
                        0 => "Square".to_string(),
                        1 => "One mitred".to_string(),
                        _ => "Both mitred".to_string(),
                    })
                });
                let (square, plumb) = crate::props::actions2(
                    ui,
                    "",
                    ("Square ends", "Take the mitre cuts off its ends", cuts > 0),
                    (
                        "Stand plumb",
                        "Its section upright again, not square to the face it was drawn on",
                        b.up.is_some(),
                    ),
                );
                if square {
                    b.cuts = [None, None];
                    label = "Square ends";
                }
                if plumb {
                    b.up = None;
                    label = "Beam plumb";
                }
            }
            weak(
                ui,
                format!(
                    "Length {}, top at {}. Plans show it dashed above the cut.",
                    crate::units::len(b.length()),
                    crate::units::elevation(b.top(model))
                ),
            );
        }
        ElementKind::Roof(r) => {
            // each shape with its own picture
            crate::props::Combo::new("Shape")
                .selected_text(r.shape.label())
                .show_ui(ui, |ui| {
                    for s in RoofShape::ALL {
                        let pick = crate::props::picture_option(
                            ui,
                            r.shape == s,
                            s.label(),
                            |p, rect, st| roof_glyph(p, rect, st, s),
                        );
                        if pick.clicked() && r.shape != s {
                            r.shape = s;
                            label = "Roof shape"
                        }
                    }
                });
            if r.shape != RoofShape::Flat && deg(ui, "Pitch", &mut r.pitch, 1.0, 75.0) {
                label = "Roof pitch"
            }
            if num(ui, "Overhang", &mut r.overhang, 0.0, 5.0) {
                label = "Roof overhang"
            }
            if num(ui, "Thickness", &mut r.thickness, 0.02, 2.0) {
                label = "Roof thickness"
            }
            if num(ui, "Base offset", &mut r.base_offset, -10.0, 100.0) {
                label = "Roof base offset"
            }
            // where the eaves end: a steep pitch with a long overhang hangs
            // them down past the walls, which then look gone (seen
            // replaying a 55° hip with 2 m eaves over 3 m walls)
            if r.shape != RoofShape::Flat && r.overhang > 0.0 {
                let drop = r.overhang * r.pitch.to_radians().tan();
                let floor = creabim_core::members::elevation(model, r.level);
                let eave = r.base(model) - drop - floor;
                let text = format!(
                    "Eaves {} above the floor: the overhang drops them {} below the base.",
                    crate::units::len(eave),
                    crate::units::len(drop)
                );
                if eave < 2.1 {
                    crate::props::note(
                        ui,
                        format!(
                            "{text} They hang below head height and hide the walls under them: shorten the overhang, lower the pitch or raise the base offset."
                        ),
                    );
                } else {
                    weak(ui, text);
                }
            }
            let n = r.boundary.len();
            if matches!(r.shape, RoofShape::Shed | RoofShape::Gable) && n > 0 {
                let what = if r.shape == RoofShape::Shed {
                    "Low edge"
                } else {
                    "Eave edge"
                };
                crate::props::row(ui, what, |ui| {
                    crate::props::flow(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 1.0;
                        let h = ui.spacing().interact_size.y;
                        let w = ((ui.available_width() - 2.0) / 3.0).max(24.0);
                        if ui
                            .add_sized([w, h], egui::Button::new("‹"))
                            .on_hover_text("The edge before")
                            .clicked()
                        {
                            r.edge = (r.edge + n - 1) % n;
                            label = "Roof edge";
                        }
                        ui.add_sized(
                            [w, h],
                            egui::Label::new(format!("{} of {n}", r.edge % n + 1)),
                        );
                        if ui
                            .add_sized([w, h], egui::Button::new("›"))
                            .on_hover_text("The next edge")
                            .clicked()
                        {
                            r.edge = (r.edge + 1) % n;
                            label = "Roof edge";
                        }
                    })
                });
            }
            if r.effective_shape() != r.shape {
                weak(
                    ui,
                    "Gable and hip roofs need a convex outline; this one is built flat.".into(),
                );
            }
            weak(
                ui,
                format!(
                    "{} corners, {} with the overhang, {} high.",
                    n,
                    crate::units::area(r.plan_area()),
                    crate::units::len(r.peak())
                ),
            );
        }
        ElementKind::Stair(s) => {
            if num(ui, "Width", &mut s.width, 0.5, 5.0) {
                label = "Stair width"
            }
            if num(ui, "Max riser", &mut s.max_riser, 0.1, 0.3) {
                label = "Stair riser"
            }
            if num(ui, "Tread", &mut s.tread, 0.15, 0.6) {
                label = "Stair tread"
            }
            if num(ui, "Waist", &mut s.waist, 0.05, 0.6) {
                label = "Stair waist"
            }
            if deg(ui, "Direction", &mut s.direction, -360.0, 360.0) {
                label = "Turn stair"
            }
            let mut fixed = s.height.is_some();
            if crate::props::switch(ui, &mut fixed, "Fixed rise (else up to the next level)")
                .changed()
            {
                s.height = fixed.then(|| s.rise(model));
                label = "Stair rise";
            }
            if let Some(h) = &mut s.height {
                if num(ui, "Rise", h, 0.1, 20.0) {
                    label = "Stair rise"
                }
            }
            let rise = s.rise(model);
            let n = members::riser_count(rise, s.max_riser);
            let mut turn = s.landing.is_some();
            if crate::props::switch(ui, &mut turn, "L-shaped, with a landing").changed() {
                s.landing = turn.then_some(Landing {
                    after: n / 2,
                    left: true,
                });
                label = "Stair landing";
            }
            if let Some(l) = &mut s.landing {
                // full rows in the label and field columns (MU31)
                crate::props::row(ui, "Landing after", |ui| {
                    let d = egui::DragValue::new(&mut l.after)
                        .range(1..=n.saturating_sub(1).max(1))
                        .suffix(" risers");
                    if crate::props::fill(ui, d).changed() {
                        label = "Stair landing";
                    }
                });
                if crate::props::choice(
                    ui,
                    "Turns",
                    &mut l.left,
                    &[(true, "Left", ""), (false, "Right", "")],
                ) {
                    label = "Stair turn";
                }
            }
            let lay = s.layout(rise);
            let going: f64 = lay.flights.iter().map(|f| f.run(s.tread)).sum();
            let r2g = 2.0 * lay.riser + s.tread;
            weak(
                ui,
                format!(
                    "{} risers of {}, rise {}, going {}. 2R + G = {}{}.",
                    lay.risers,
                    crate::units::len(lay.riser),
                    crate::units::len(rise),
                    crate::units::len(going),
                    crate::units::len(r2g),
                    if (0.55..=0.70).contains(&r2g) {
                        String::new()
                    } else {
                        format!(
                            " (outside the usual {}–{})",
                            crate::units::len(0.55),
                            crate::units::len(0.70)
                        )
                    }
                ),
            );
        }
        ElementKind::Railing(r) => {
            if num(ui, "Height", &mut r.height, 0.2, 3.0) {
                label = "Railing height"
            }
            if num(ui, "Post spacing", &mut r.post_spacing, 0.1, 5.0) {
                label = "Railing posts"
            }
            // its top rail from the profile library (MU36)
            crate::props::row(ui, "Top rail", |ui| {
                let w = ui.available_width();
                let mut names: Vec<String> = creabim_core::profiles::project_profiles(model)
                    .into_iter()
                    .map(|p| p.name)
                    .collect();
                names.extend(["Handrail", "Tube", "Bar"].iter().map(|s| s.to_string()));
                egui::ComboBox::from_id_salt("rail.profile")
                    .icon(crate::props::combo_icon)
                    .width(w)
                    .truncate()
                    .selected_text(
                        r.rail_profile
                            .clone()
                            .unwrap_or_else(|| "Square bar".into()),
                    )
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(r.rail_profile.is_none(), "Square bar")
                            .clicked()
                        {
                            r.rail_profile = None;
                            label = "Railing top rail";
                        }
                        for n in &names {
                            if ui
                                .selectable_label(r.rail_profile.as_deref() == Some(n.as_str()), n)
                                .clicked()
                            {
                                r.rail_profile = Some(n.clone());
                                label = "Railing top rail";
                            }
                        }
                    });
            });
            if let Some(side) = &mut r.stair {
                if crate::props::choice(
                    ui,
                    "Side",
                    &mut side.left,
                    &[(true, "Left", ""), (false, "Right", "")],
                ) {
                    label = "Railing side";
                }
                weak(ui, "Follows its stair.".into());
            }
            weak(
                ui,
                format!(
                    "Length {}, {} posts.",
                    crate::units::len(r.length(model)),
                    r.posts(model).len()
                ),
            );
        }
        _ => {}
    }
    label
}

/// A column's base and top constraints: the level it stands on and how
/// far above it, and the level its top is tied to (or none) and how far
/// above that. Standing it on another level keeps its top where it was.
fn column_levels(ui: &mut egui::Ui, model: &Model, c: &mut Column) -> &'static str {
    let mut label = "";
    let levels = creabim_core::levels::sorted(model);
    let name = |id: Option<ElementId>| {
        id.and_then(|i| levels.iter().find(|l| l.0 == i))
            .map_or("Unconnected".to_string(), |l| l.1.clone())
    };
    let mut base = c.level;
    crate::app::helpui::combo(ui, "Base level")
        .selected_text(name(Some(c.level)))
        .show_ui(ui, |ui| {
            for (lid, n, _) in levels.iter().rev() {
                ui.selectable_value(&mut base, *lid, n);
            }
        });
    if base != c.level {
        c.set_base_level(model, base);
        label = "Column base level";
    }
    let num = |ui: &mut egui::Ui, text: &str, v: &mut f64| -> bool {
        crate::props::row(ui, text, |ui| {
            crate::props::fill(ui, crate::units::drag(v).range(-50.0..=50.0)).changed()
        })
    };
    if num(ui, "Base offset", &mut c.base_offset) {
        label = "Column base offset";
    }
    let mut top = c.top_level();
    crate::app::helpui::combo(ui, "Top level")
        .selected_text(name(top))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut top, None, "Unconnected")
                .on_hover_text("A fixed height, or up to the next level");
            for (lid, n, _) in levels.iter().rev() {
                ui.selectable_value(&mut top, Some(*lid), n);
            }
        });
    if top != c.top_level() {
        // tied to a level: the top lands on it, wherever it was
        c.set_top_level(model, top, 0.0);
        label = "Column top level";
    }
    if let creabim_core::WallTop::UpToLevel { offset, .. } = &mut c.top {
        if num(ui, "Top offset", offset) {
            label = "Column top offset";
        }
    }
    label
}

/// A footing's Properties rows: its size, how far below its level (or its
/// host's base) it sits, where it is when it stands free, and its host.
fn footing_fields(ui: &mut egui::Ui, model: &Model, f: &mut creabim_core::Footing) -> &'static str {
    use creabim_core::FootingKind;
    let mut label = "";
    let num = |ui: &mut egui::Ui, text: &str, v: &mut f64, lo: f64, hi: f64| -> bool {
        crate::props::row(ui, text, |ui| {
            crate::props::fill(ui, crate::units::drag(v).range(lo..=hi)).changed()
        })
    };
    // under the fields, in their column, as Blender's notes (MU31)
    let weak = |ui: &mut egui::Ui, t: String| crate::props::note(ui, t);
    if num(ui, "Width", &mut f.width, 0.1, 10.0) {
        label = "Footing width";
    }
    if f.kind == FootingKind::Pad && num(ui, "Length", &mut f.length, 0.1, 10.0) {
        label = "Footing length";
    }
    if num(ui, "Thickness", &mut f.thickness, 0.05, 5.0) {
        label = "Footing thickness";
    }
    let host = f.host.and_then(|h| model.get(h));
    let below = if host.is_some() {
        match f.kind {
            FootingKind::Strip => "Top below wall base",
            FootingKind::Pad => "Top below column base",
        }
    } else {
        "Top below level"
    };
    if num(ui, below, &mut f.below, -50.0, 50.0) {
        label = "Footing depth";
    }
    match host {
        Some(h) => {
            weak(
                ui,
                format!(
                    "Under {}: it follows it when it moves, and goes when it is deleted.",
                    h.name
                ),
            );
            if ui
                .button("Let go of it")
                .on_hover_text("Keep the footing where it is, on its own")
                .clicked()
            {
                *f = creabim_core::footing::detached(model, f);
                label = "Let go of host";
            }
        }
        None => {
            let levels = creabim_core::levels::sorted(model);
            let current = levels
                .iter()
                .find(|l| l.0 == f.level)
                .map_or(String::new(), |l| l.1.clone());
            crate::app::helpui::combo(ui, "Level")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (lid, n, _) in levels.iter().rev() {
                        if ui.selectable_value(&mut f.level, *lid, n).changed() {
                            label = "Footing level";
                        }
                    }
                });
            match f.kind {
                FootingKind::Strip => {
                    if num(ui, "Start X", &mut f.start.x, -1e4, 1e4)
                        | num(ui, "Start Y", &mut f.start.y, -1e4, 1e4)
                        | num(ui, "End X", &mut f.end.x, -1e4, 1e4)
                        | num(ui, "End Y", &mut f.end.y, -1e4, 1e4)
                    {
                        label = "Move footing";
                    }
                }
                FootingKind::Pad => {
                    if num(ui, "X", &mut f.start.x, -1e4, 1e4)
                        | num(ui, "Y", &mut f.start.y, -1e4, 1e4)
                    {
                        f.end = f.start;
                        label = "Move footing";
                    }
                    let r = crate::props::flow(ui, |ui| {
                        ui.label("Rotation");
                        ui.add(
                            egui::DragValue::new(&mut f.rotation)
                                .speed(0.5)
                                .range(-360.0..=360.0)
                                .suffix("°"),
                        )
                        .changed()
                    });
                    if r.inner {
                        label = "Turn footing";
                    }
                }
            }
        }
    }
    let top = f.top(model);
    weak(
        ui,
        format!(
            "{}: top at {}, underside at {}.",
            match f.kind {
                FootingKind::Strip =>
                    format!("Strip {} long", crate::units::len(f.strip_length(model))),
                FootingKind::Pad => "Pad".to_string(),
            },
            crate::units::elevation(top),
            crate::units::elevation(top - f.thickness)
        ),
    );
    label
}

/// The tool bar's settings for the active tool: sizes as numbers, shapes
/// as toggles (`member:<what>` actions).
pub fn tool_options(state: &PlanState) -> Vec<serde_json::Value> {
    use serde_json::json;
    let m = &state.members;
    let num = |key: &str,
               label: &str,
               value: f64,
               unit: &str,
               step: f64,
               min: f64,
               max: f64,
               hint: &str| {
        json!({
            "kind": "number", "key": format!("member.{key}"), "label": label, "value": value,
            "unit": unit, "step": step, "min": min, "max": max, "hint": hint,
        })
    };
    let toggle = |id: &str, label: &str, on: bool, icon: &str, hint: &str| {
        json!({
            "kind": "toggle", "id": format!("member:{id}"), "label": label, "on": on,
            "icon": icon, "hint": hint,
        })
    };
    let opening = |key: &str, s: [f64; 3], what: &str| {
        vec![
            num(
                &format!("{key}_width"),
                "Width",
                s[0],
                "m",
                0.01,
                0.1,
                20.0,
                &format!("Width of new {what}s. Typing a size here places it without a type."),
            ),
            num(
                &format!("{key}_height"),
                "Height",
                s[1],
                "m",
                0.01,
                0.1,
                20.0,
                &format!("Height of new {what}s"),
            ),
            num(
                &format!("{key}_sill"),
                "Sill",
                s[2],
                "m",
                0.01,
                0.0,
                20.0,
                &format!("How high new {what}s start above the wall's base"),
            ),
        ]
    };
    let mut out = match state.tool {
        Tool::Door => opening("door", m.door, "door"),
        Tool::Window => opening("window", m.window, "window"),
        Tool::Opening => opening("hole", m.hole, "opening"),
        Tool::Column => vec![
            toggle(
                "square",
                "Square",
                m.column_section.is_none() && !m.round_columns(),
                "square-dashed",
                "Rectangular columns",
            ),
            toggle(
                "round",
                "Round",
                m.column_section.is_none() && m.round_columns(),
                "circle-dot",
                "Round columns",
            ),
            // or any profile of the library: an I, a T, an L, your own (MU40)
            json!({
                "kind": "menu", "key": "column.library", "label": "Profile",
                "value": m.column_section.clone().unwrap_or_else(|| "Library".into()), "icon": "profile",
                "on": m.column_section.is_some(),
                "hint": "Columns of a profile from the library: presets and the project's own",
            }),
            num(
                "column",
                if m.round_columns() {
                    "Diameter"
                } else {
                    "Size"
                },
                m.column_size(),
                "m",
                0.01,
                0.05,
                3.0,
                "Size of new columns",
            ),
        ],
        Tool::Beam => vec![
            json!({
                "kind": "menu", "key": "beam.corner", "label": "Corners",
                "value": m.beam_corner.label(), "icon": m.beam_corner.icon(),
                "hint": "How the beams of a chain meet: Separate (concrete pieces), Mitred (welded steel) or Butt (one runs through, the next stops against it)",
            }),
            toggle(
                "beam.onface",
                "On face",
                m.beam_on_face,
                "scan",
                "In 3D, beams lie on the face clicked (a roof slope, a wall), square to it: purlins, rafters, braces",
            ),
            json!({
                "kind": "menu", "key": "beam.library", "label": "Profile",
                "value": m.beam_section.clone().unwrap_or_else(|| "Its own".into()), "icon": "profile",
                "on": m.beam_section.is_some(),
                "hint": "Beams of a profile from the library (an I, a T, an L, your own), or their own width × depth",
            }),
            num(
                "beam_width",
                "Width",
                m.beam_width,
                "m",
                0.01,
                0.05,
                2.0,
                "Width of new beams",
            ),
            num(
                "beam_depth",
                "Depth",
                m.beam_depth,
                "m",
                0.01,
                0.05,
                3.0,
                "Depth of new beams",
            ),
        ],
        Tool::Roof => {
            let mut v: Vec<serde_json::Value> = RoofShape::ALL
                .iter()
                .map(|s| {
                    toggle(
                        &format!("roof.{}", s.label()),
                        s.label(),
                        m.roof_shape == *s,
                        roof_icon(*s),
                        "Shape of new roofs",
                    )
                })
                .collect();
            if m.roof_shape != RoofShape::Flat {
                v.push(num(
                    "pitch",
                    "Pitch",
                    m.roof_pitch,
                    "°",
                    1.0,
                    1.0,
                    75.0,
                    "Slope of new roofs",
                ));
            }
            v
        }
        Tool::Stair => vec![
            num(
                "stair_width",
                "Width",
                m.stair_width,
                "m",
                0.05,
                0.5,
                5.0,
                "Width of new stairs",
            ),
            toggle(
                "lshape",
                "L-shaped",
                m.stair_turn,
                "corner-down-right",
                "New stairs turn left on a landing halfway up",
            ),
        ],
        Tool::Footing => vec![
            toggle(
                "footing.strip",
                "Strip",
                !m.footing_pad,
                "footing",
                "Strip footings: click a wall, or two points",
            ),
            toggle(
                "footing.pad",
                "Pad",
                m.footing_pad,
                "square-dashed",
                "Pad footings: click a column, or anywhere",
            ),
            num(
                "footing_width",
                if m.footing_pad { "Size" } else { "Width" },
                m.footing_width,
                "m",
                0.05,
                0.1,
                10.0,
                "Width of new strips, or the sides of new pads",
            ),
            num(
                "footing_thickness",
                "Thickness",
                m.footing_thickness,
                "m",
                0.05,
                0.05,
                5.0,
                "Thickness of new footings",
            ),
        ],
        Tool::Railing => vec![
            json!({
                "kind": "menu", "key": "railing.library", "label": "Rail",
                "value": m.railing_section.clone().unwrap_or_else(|| "Its own".into()), "icon": "profile",
                "on": m.railing_section.is_some(),
                "hint": "The top rail's profile from the library (a handrail, a tube, your own), or its own square bar",
            }),
            num(
                "railing_height",
                "Height",
                m.railing_height,
                "m",
                0.05,
                0.3,
                3.0,
                "Height of new railings",
            ),
        ],
        _ => vec![],
    };
    if !out.is_empty() {
        out.push(json!({"kind": "sep"}));
    }
    out
}

/// A number typed in the tool bar.
pub fn set_option(state: &mut PlanState, key: &str, v: f64) {
    let m = &mut state.members;
    match key.trim_start_matches("member.") {
        "column" => m.set_column_size(v.clamp(0.05, 3.0)),
        "beam_width" => m.beam_width = v.clamp(0.05, 2.0),
        "beam_depth" => m.beam_depth = v.clamp(0.05, 3.0),
        "pitch" => m.roof_pitch = v.clamp(1.0, 75.0),
        "stair_width" => m.stair_width = v.clamp(0.5, 5.0),
        "railing_height" => m.railing_height = v.clamp(0.3, 3.0),
        "footing_width" => m.footing_width = v.clamp(0.1, 10.0),
        "footing_thickness" => m.footing_thickness = v.clamp(0.05, 5.0),
        k if k.starts_with("door_") || k.starts_with("window_") || k.starts_with("hole_") => {
            let (kind, what) = k.split_once('_').unwrap_or((k, ""));
            let s = match kind {
                "door" => &mut m.door,
                "window" => &mut m.window,
                _ => &mut m.hole,
            };
            match what {
                "width" => s[0] = v.clamp(0.1, 20.0),
                "height" => s[1] = v.clamp(0.1, 20.0),
                "sill" => s[2] = v.clamp(0.0, 20.0),
                _ => {}
            }
        }
        _ => {}
    }
}

/// A toggle clicked in the tool bar.
pub fn toggle_option(state: &mut PlanState, what: &str) {
    let m = &mut state.members;
    match what {
        "beam.onface" => m.beam_on_face = !m.beam_on_face,
        "square" => m.set_round(false),
        "round" => m.set_round(true),
        "lshape" => m.stair_turn = !m.stair_turn,
        "footing.strip" | "footing.pad" => {
            let pad = what == "footing.pad";
            if pad != m.footing_pad {
                m.footing_pad = pad;
                // each kind starts at its usual size
                (m.footing_width, m.footing_thickness) = if pad {
                    (
                        creabim_core::footing::PAD_SIZE,
                        creabim_core::footing::PAD_THICKNESS,
                    )
                } else {
                    (
                        creabim_core::footing::STRIP_WIDTH,
                        creabim_core::footing::STRIP_THICKNESS,
                    )
                };
            }
        }
        w => {
            if let Some(s) = RoofShape::ALL
                .into_iter()
                .find(|s| w == format!("roof.{}", s.label()))
            {
                m.roof_shape = s;
            }
        }
    }
}

/// The interface icon of a roof shape: each its own.
pub(crate) fn roof_icon(s: RoofShape) -> &'static str {
    match s {
        RoofShape::Flat => "roof-flat",
        RoofShape::Shed => "roof-mono",
        RoofShape::Gable => "roof-gable",
        RoofShape::Hip => "roof-hip",
    }
}

/// A roof shape's outline, drawn small in `r` for its drop-down: the
/// same pictures as the icons.
fn roof_glyph(p: &egui::Painter, r: egui::Rect, st: egui::Stroke, s: RoofShape) {
    let at = |x: f32, y: f32| egui::pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let line = |pts: &[(f32, f32)]| {
        let pts: Vec<egui::Pos2> = pts.iter().map(|&(x, y)| at(x, y)).collect();
        p.add(egui::Shape::line(pts, st));
    };
    match s {
        RoofShape::Flat => {
            line(&[(0.1, 0.4), (0.9, 0.4), (0.9, 0.55), (0.1, 0.55), (0.1, 0.4)]);
            line(&[(0.2, 0.55), (0.2, 0.85)]);
            line(&[(0.8, 0.55), (0.8, 0.85)]);
        }
        RoofShape::Shed => {
            line(&[(0.1, 0.85), (0.1, 0.6), (0.9, 0.3), (0.9, 0.85)]);
        }
        RoofShape::Gable => {
            line(&[(0.1, 0.55), (0.5, 0.2), (0.9, 0.55)]);
            line(&[(0.2, 0.47), (0.2, 0.85), (0.8, 0.85), (0.8, 0.47)]);
        }
        RoofShape::Hip => {
            line(&[
                (0.1, 0.65),
                (0.3, 0.3),
                (0.7, 0.3),
                (0.9, 0.65),
                (0.1, 0.65),
            ]);
            line(&[(0.2, 0.65), (0.2, 0.85), (0.8, 0.85), (0.8, 0.65)]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::sample::sample_house;

    #[test]
    fn a_wall_is_hit_across_its_whole_thickness_whatever_its_location_line() {
        let (doc, level, _, _) = setup();
        let m = doc.model();
        // the starter house's walls are drawn on several location lines
        let walls: Vec<_> = m
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Wall(w) if w.level == level && w.curtain.is_none() => {
                    Some((e.id, w.clone()))
                }
                _ => None,
            })
            .collect();
        assert!(!walls.is_empty());
        for (id, w) in walls {
            let n = w.direction().perp();
            let mid = (w.start + w.end) / 2.0;
            for s in [-0.45, 0.0, 0.45] {
                let p = mid + n * s * w.thickness;
                assert!(
                    walls_at(m, level, p).contains(&id),
                    "{s} of its thickness off the centre"
                );
            }
            assert!(!walls_at(m, level, mid + n * (w.thickness / 2.0 + 0.2)).contains(&id));
        }
    }

    #[test]
    fn a_host_tool_finds_its_host_before_the_first_click_only() {
        let (doc, level, mut state, _) = setup();
        let m = doc.model();
        let (id, w) = m
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Wall(w) if w.level == level && w.curtain.is_none() => {
                    Some((e.id, w.clone()))
                }
                _ => None,
            })
            .unwrap();
        let p = (w.start + w.end) / 2.0;
        for tool in [Tool::Footing, Tool::Door, Tool::Window] {
            assert_eq!(
                hosts_at(m, level, tool, &state, p).first(),
                Some(&id),
                "{tool:?}"
            );
        }
        assert!(host_preview(m, &state, Tool::Footing, id).contains("under"));
        // a free strip with its first point snaps as usual
        state.pending = Some(DVec2::ZERO);
        assert!(hosts_at(m, level, Tool::Footing, &state, p).is_empty());
        assert!(!hosted(Tool::Wall));
    }

    fn setup() -> (Document, ElementId, PlanState, Selection) {
        let doc = Document::from_model(sample_house());
        let level = doc
            .model()
            .levels()
            .min_by(|a, b| a.1.elevation.total_cmp(&b.1.elevation))
            .unwrap()
            .0
            .id;
        (doc, level, PlanState::default(), Selection::default())
    }

    fn count(doc: &Document, f: fn(&ElementKind) -> bool) -> usize {
        doc.model().iter().filter(|e| f(&e.kind)).count()
    }

    fn active<'a>(doc: &'a Document, sel: &Selection) -> Option<&'a ElementKind> {
        sel.active
            .and_then(|id| doc.model().get(id))
            .map(|e| &e.kind)
    }

    #[test]
    fn tools_place_each_member_and_undo() {
        let (mut doc, level, mut st, mut sel) = setup();
        let mut go = |doc: &mut Document, st: &mut PlanState, tool, x, y| {
            click(doc, level, tool, DVec2::new(x, y), st, &mut sel).unwrap();
        };
        go(&mut doc, &mut st, Tool::Column, 1.0, 1.0);
        assert_eq!(count(&doc, |k| matches!(k, ElementKind::Column(_))), 1);
        for (x, y) in [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)] {
            go(&mut doc, &mut st, Tool::Beam, x, y);
        }
        assert_eq!(count(&doc, |k| matches!(k, ElementKind::Beam(_))), 2);
        st.cancel();
        st.members.stair_turn = true;
        go(&mut doc, &mut st, Tool::Stair, 20.0, 0.0);
        go(&mut doc, &mut st, Tool::Stair, 21.0, 0.0);
        assert_eq!(
            count(
                &doc,
                |k| matches!(k, ElementKind::Stair(s) if s.landing.is_some())
            ),
            1
        );
        // a click on the stair rails both its sides
        go(&mut doc, &mut st, Tool::Railing, 20.5, 0.0);
        assert_eq!(count(&doc, |k| matches!(k, ElementKind::Railing(_))), 2);
        // a free path: click the last point again to finish
        for (x, y) in [(30.0, 0.0), (33.0, 0.0), (33.0, 0.0)] {
            go(&mut doc, &mut st, Tool::Railing, x, y);
        }
        assert_eq!(count(&doc, |k| matches!(k, ElementKind::Railing(_))), 3);
        assert!(st.members.path.is_empty());
        doc.undo();
        assert_eq!(count(&doc, |k| matches!(k, ElementKind::Railing(_))), 2);
    }

    #[test]
    fn the_footing_tool_puts_strips_under_walls_and_pads_under_columns() {
        use creabim_core::{footing, FootingKind};
        let (mut doc, level, mut st, mut sel) = setup();
        let mut go = |doc: &mut Document, st: &mut PlanState, x: f64, y: f64| {
            click(doc, level, Tool::Footing, DVec2::new(x, y), st, &mut sel).unwrap();
        };
        let (wall, mid) = doc
            .model()
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Wall(w) if w.level == level && w.curtain.is_none() => {
                    Some((e.id, (w.start + w.end) / 2.0))
                }
                _ => None,
            })
            .unwrap();
        // a click on a wall: a strip under it, hosted
        go(&mut doc, &mut st, mid.x, mid.y);
        assert_eq!(footing::footings_under(doc.model(), wall).len(), 1);
        // a second click there adds none
        go(&mut doc, &mut st, mid.x, mid.y);
        assert_eq!(footing::footings_under(doc.model(), wall).len(), 1);
        assert!(st.status.as_deref().unwrap_or("").contains("already"));
        // two clicks away from walls: a strip on its own
        go(&mut doc, &mut st, 50.0, 50.0);
        go(&mut doc, &mut st, 54.0, 50.0);
        let free = |doc: &Document, kind: FootingKind| {
            doc.model()
                .iter()
                .filter(|e| matches!(&e.kind, ElementKind::Footing(f) if f.kind == kind && f.host.is_none()))
                .count()
        };
        assert_eq!(free(&doc, FootingKind::Strip), 1);
        // pads: under a column, and on their own, at the pad size
        click(
            &mut doc,
            level,
            Tool::Column,
            DVec2::new(70.0, 70.0),
            &mut st,
            &mut sel,
        )
        .unwrap();
        let column = sel.active.unwrap();
        toggle_option(&mut st, "footing.pad");
        assert!(st.members.footing_pad);
        let mut go = |doc: &mut Document, st: &mut PlanState, x: f64, y: f64| {
            click(doc, level, Tool::Footing, DVec2::new(x, y), st, &mut sel).unwrap();
        };
        go(&mut doc, &mut st, 70.0, 70.0);
        let under = footing::footings_under(doc.model(), column);
        assert_eq!(under.len(), 1);
        assert!(matches!(
            doc.model().get(under[0]).map(|e| &e.kind),
            Some(ElementKind::Footing(f)) if (f.width - footing::PAD_SIZE).abs() < 1e-9
        ));
        go(&mut doc, &mut st, 80.0, 80.0);
        assert_eq!(free(&doc, FootingKind::Pad), 1);
        doc.undo();
        assert_eq!(free(&doc, FootingKind::Pad), 0);
    }

    #[test]
    fn the_roof_tool_draws_rectangles_and_outlines() {
        let (mut doc, level, mut st, mut sel) = setup();
        st.tool = Tool::Roof;
        st.members.roof_shape = RoofShape::Gable;
        st.slab_mode = crate::slabtool::SlabMode::Rectangle;
        crate::slabtool::click(&mut doc, level, DVec2::ZERO, &mut st, &mut sel).unwrap();
        crate::slabtool::click(&mut doc, level, DVec2::new(8.0, 5.0), &mut st, &mut sel).unwrap();
        assert!(matches!(
            active(&doc, &sel),
            Some(ElementKind::Roof(r)) if r.shape == RoofShape::Gable
        ));
        st.slab_mode = crate::slabtool::SlabMode::Polygon;
        crate::slabtool::click(&mut doc, level, DVec2::new(20.0, 0.0), &mut st, &mut sel).unwrap();
        let mut o = st.outline.take().unwrap();
        assert!(o.roof.is_some());
        for (x, y) in [(24.0, 0.0), (24.0, 4.0), (20.0, 4.0)] {
            o.click(DVec2::new(x, y), false, false);
        }
        o.click(DVec2::new(20.0, 0.0), true, false);
        let (_, id) = o.finish(&mut doc, true).unwrap();
        assert!(matches!(
            doc.model().get(id.unwrap()).map(|e| &e.kind),
            Some(ElementKind::Roof(_))
        ));
    }
}
