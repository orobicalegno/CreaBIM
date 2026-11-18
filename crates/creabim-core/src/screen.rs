//! Screen walls: walls built from a repeating unit with holes through it,
//! such as breeze blocks, louvres or a lattice.
//!
//! The pattern belongs to the wall type (see [`crate::types::WallType`]),
//! so every wall of the type shares it and editing the type changes them
//! all. A pattern is a unit size, the rectangles that are open in each
//! unit, a solid frame round the edge and whether courses are staggered;
//! what makes it a breeze block or a louvre is only those numbers. Holes
//! that run to a unit's edge meet the next unit's, so slots and slats
//! come from the same mechanism.
//!
//! The holes are cut through the wall like bare openings
//! ([`crate::curtain::wall_openings`]), so plans, sections, 3D and exports
//! show them wherever they show a wall's openings.

use crate::{Element, ElementId, ElementKind, Model, Opening, OpeningFrame, OpeningKind, Wall};
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// The repeating unit of a screen wall.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenPattern {
    /// The unit's size, joints included: along the wall and up.
    pub unit: DVec2,
    /// The open rectangles of one unit, (lower left, upper right), from
    /// the unit's lower left corner.
    pub holes: Vec<[DVec2; 2]>,
    /// A solid border this wide round the screen: at its ends, its base
    /// and its top.
    #[serde(default)]
    pub frame: f64,
    /// Every other course shifted half a unit along.
    #[serde(default)]
    pub stagger: bool,
}

/// Limits a pattern to a sane number of holes per wall.
const MAX_HOLES: usize = 4000;

impl ScreenPattern {
    /// Units of `unit` with one centred hole of `hole` in each.
    pub fn blocks(unit: DVec2, hole: DVec2, frame: f64) -> Self {
        let lo = (unit - hole) / 2.0;
        Self {
            unit,
            holes: vec![[lo, lo + hole]],
            frame,
            stagger: false,
        }
    }

    /// Horizontal slats `slat` deep every `pitch` up, open between: one
    /// slot per course, running the wall's length.
    pub fn louvres(pitch: f64, slat: f64, frame: f64) -> Self {
        let unit = DVec2::new(1.0, pitch);
        Self {
            unit,
            holes: vec![[DVec2::new(0.0, slat), unit]],
            frame,
            stagger: false,
        }
    }

    /// Bars `bar` wide at `pitch` both ways: square holes between.
    pub fn lattice(pitch: f64, bar: f64, frame: f64) -> Self {
        Self::blocks(
            DVec2::splat(pitch),
            DVec2::splat((pitch - bar).max(0.0)),
            frame,
        )
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(self.unit.is_finite() && self.unit.min_element() > 0.01) {
            return Err("a screen unit must be at least 10 mm each way".into());
        }
        if !(self.frame.is_finite() && self.frame >= 0.0) {
            return Err("a screen's frame can't be negative".into());
        }
        for [lo, hi] in &self.holes {
            if !(lo.is_finite() && hi.is_finite() && hi.cmpgt(*lo).all()) {
                return Err("a screen hole needs a positive size".into());
            }
        }
        Ok(())
    }

    /// The share of the screen's face that is open, ignoring the frame.
    pub fn openness(&self) -> f64 {
        let area: f64 = self
            .holes
            .iter()
            .map(|[lo, hi]| {
                let lo = lo.max(DVec2::ZERO);
                let hi = hi.min(self.unit);
                ((hi - lo).max(DVec2::ZERO)).element_product()
            })
            .sum();
        (area / self.unit.element_product()).min(1.0)
    }

    /// The holes through a wall `length` long and `height` high, as
    /// rectangles (u0, v0, u1, v1) in the wall's face: `u` along from its
    /// start, `v` up from its base. Holes are clipped to inside the frame.
    pub fn cuts(&self, length: f64, height: f64) -> Vec<[f64; 4]> {
        let f = self.frame;
        let (u_lo, u_hi, v_lo, v_hi) = (f, length - f, f, height - f);
        if u_hi <= u_lo || v_hi <= v_lo || self.validate().is_err() {
            return vec![];
        }
        let mut out = vec![];
        let rows = ((v_hi - v_lo) / self.unit.y).ceil() as i64;
        for row in 0..rows {
            let v0 = v_lo + row as f64 * self.unit.y;
            let shift = if self.stagger && row % 2 == 1 {
                self.unit.x / 2.0
            } else {
                0.0
            };
            // a half unit at the start of a staggered course
            let first = if shift > 0.0 { -1 } else { 0 };
            let cols = ((u_hi - u_lo + shift) / self.unit.x).ceil() as i64;
            for col in first..cols {
                let u0 = u_lo + col as f64 * self.unit.x + shift;
                for [lo, hi] in &self.holes {
                    let a = (u0 + lo.x).max(u_lo);
                    let b = (u0 + hi.x).min(u_hi);
                    let c = (v0 + lo.y).max(v_lo);
                    let d = (v0 + hi.y).min(v_hi);
                    if b - a > 1e-4 && d - c > 1e-4 {
                        out.push([a, c, b, d]);
                        if out.len() >= MAX_HOLES {
                            return out;
                        }
                    }
                }
            }
        }
        out
    }
}

/// The screen pattern of a wall: its type's, if its type has one.
pub fn screen_of<'a>(model: &'a Model, e: &Element) -> Option<&'a ScreenPattern> {
    let ElementKind::Wall(w) = &e.kind else {
        return None;
    };
    if w.curtain.is_some() {
        return None;
    }
    let (_, t) = crate::types::type_of(model, e)?;
    match &t.params {
        crate::types::TypeParams::Wall(wt) => wt.screen.as_deref(),
        _ => None,
    }
}

/// Is wall `id` a screen wall?
pub fn is_screen(model: &Model, id: ElementId) -> bool {
    model.get(id).is_some_and(|e| screen_of(model, e).is_some())
}

/// Is element `id` a screen wall type?
pub fn is_screen_type(model: &Model, id: ElementId) -> bool {
    model.get(id).is_some_and(|e| {
        matches!(&e.kind, ElementKind::ElementType(t)
            if matches!(&t.params, crate::types::TypeParams::Wall(w) if w.screen.is_some()))
    })
}

/// The holes a screen wall's pattern cuts in it, as bare openings.
pub fn screen_openings(model: &Model, id: ElementId) -> Vec<Opening> {
    let Some(e) = model.get(id) else {
        return vec![];
    };
    let (Some(p), ElementKind::Wall(w)) = (screen_of(model, e), &e.kind) else {
        return vec![];
    };
    holes(id, w, p)
}

/// The holes `pattern` cuts in wall `w` (whose id is `host`).
pub fn holes(host: ElementId, w: &Wall, pattern: &ScreenPattern) -> Vec<Opening> {
    pattern
        .cuts(w.length(), w.height)
        .into_iter()
        .map(|[u0, v0, u1, v1]| Opening {
            host,
            kind: OpeningKind::Hole,
            offset: u0,
            sill: v0,
            width: u1 - u0,
            height: v1 - v0,
            material: None,
            frame: OpeningFrame::default(),
            flip_hand: false,
            flip_facing: false,
            wrap: None,
            frame_at: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breeze_blocks_tile_inside_the_frame() {
        let p = ScreenPattern::blocks(DVec2::new(0.4, 0.2), DVec2::new(0.3, 0.1), 0.1);
        // 2.0 m long, 1.2 m high: 1.8 x 1.0 inside the frame
        let cuts = p.cuts(2.0, 1.2);
        // 5 columns (the last one cut short) by 5 courses
        assert_eq!(cuts.len(), 25);
        assert!(cuts
            .iter()
            .all(|c| c[0] >= 0.1 && c[2] <= 1.9 && c[1] >= 0.1 && c[3] <= 1.1));
        assert!((p.openness() - 0.375).abs() < 1e-9);
    }

    #[test]
    fn louvres_are_slots_the_length_of_the_wall() {
        let p = ScreenPattern::louvres(0.15, 0.05, 0.0);
        let cuts = p.cuts(3.0, 0.9);
        // six courses, each slot made of three 1 m units meeting end to end
        assert_eq!(cuts.len(), 18);
        let row: Vec<_> = cuts.iter().filter(|c| c[1] == 0.05).collect();
        assert_eq!(row.len(), 3);
        assert!((row[0][2] - row[1][0]).abs() < 1e-12);
    }

    #[test]
    fn staggered_courses_shift_half_a_unit() {
        let mut p = ScreenPattern::lattice(0.2, 0.05, 0.0);
        p.stagger = true;
        let cuts = p.cuts(1.0, 0.4);
        let first = |v: f64| {
            cuts.iter()
                .filter(|c| (c[1] - v).abs() < 1e-9)
                .map(|c| c[0])
                .fold(f64::INFINITY, f64::min)
        };
        assert!((first(0.025) - 0.025).abs() < 1e-9);
        // the second course starts with a half unit clipped at the wall end
        assert!(first(0.225) < 0.025 + 1e-9);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn wall_types_from_before_screens_still_read() {
        let old = r#"{"thickness":0.2,"location":"Centre"}"#;
        let t: crate::types::WallType = serde_json::from_str(old).unwrap();
        assert!(t.screen.is_none());
        // and a plain type is written without the field
        assert!(!serde_json::to_string(&t).unwrap().contains("screen"));
        // a file of the sample house from before reads, and none of its
        // walls are screens
        let m = crate::sample::typed_house();
        let back = Model::from_json(&m.to_json()).unwrap();
        assert!(back.iter().all(|e| !is_screen(&back, e.id)));
    }

    #[test]
    fn a_screen_type_pierces_its_walls_and_saves_with_them() {
        let m = crate::garden::garden_house();
        let screen = m.iter().find(|e| e.name == "Garden screen").unwrap();
        let ElementKind::Wall(w) = &screen.kind else {
            panic!()
        };
        let holes = screen_openings(&m, screen.id);
        // 10 m by 1.8 m of 400 x 200 units, two holes in each
        assert_eq!(holes.len(), 25 * 9 * 2);
        assert!(holes
            .iter()
            .all(|o| o.offset >= 0.0 && o.offset + o.width <= w.length() + 1e-9));
        let back = Model::from_json(&m.to_json()).unwrap();
        assert_eq!(screen_openings(&back, screen.id), holes);
    }
}
