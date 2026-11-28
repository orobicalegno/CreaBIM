//! STRETCH for the model (AutoCAD's, on a plan): what lies wholly in the
//! window moves; walls, slabs, roofs, rooms and ceilings with only some of
//! their ends or corners in it have just those moved, so the rest stays
//! where it is. Doors and windows keep their place along a wall whose
//! start moved.

use crate::transform::{pivot, transformed, Xform};
use crate::{Element, ElementId, ElementKind, Model, Transaction};
use glam::DVec2;

/// The stretch of what is on `level` (none: every level) by `by`, for the
/// window `lo` to `hi`; with how many elements it changes.
pub fn stretch(
    model: &Model,
    level: Option<ElementId>,
    lo: DVec2,
    hi: DVec2,
    by: DVec2,
) -> (Transaction, usize) {
    let inside = |p: DVec2| p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y;
    let on_level =
        |e: &Element| level.is_none_or(|l| crate::levels::level_of(&e.kind).is_none_or(|x| x == l));
    let mut tx = Transaction::new("Stretch");
    let mut n = 0;
    let poly = |pts: &mut Vec<DVec2>| -> (bool, bool) {
        let mut any = false;
        let mut all = !pts.is_empty();
        for p in pts.iter_mut() {
            if inside(*p) {
                *p += by;
                any = true;
            } else {
                all = false;
            }
        }
        (any, all)
    };
    for e in model.iter() {
        if !on_level(e) {
            continue;
        }
        let mut after = e.clone();
        let changed = match &mut after.kind {
            ElementKind::Wall(w) => {
                let (s, t) = (inside(w.start), inside(w.end));
                if s && t {
                    w.start += by;
                    w.end += by;
                } else if s {
                    w.start += by;
                } else if t {
                    w.end += by;
                }
                (s || t) && w.start.distance(w.end) > 1e-6
            }
            ElementKind::Slab(s) => {
                let (any, _) = poly(&mut s.boundary);
                for h in &mut s.holes {
                    poly(h);
                }
                any
            }
            ElementKind::Roof(r) => poly(&mut r.boundary).0,
            ElementKind::Room(r) => {
                let moved = poly(&mut r.boundary).0;
                if inside(r.at) {
                    r.at += by;
                    true
                } else {
                    moved
                }
            }
            ElementKind::Ceiling(c) => poly(&mut c.boundary).0,
            ElementKind::RoomSeparator(s) => {
                let (a, b) = (inside(s.a), inside(s.b));
                if a {
                    s.a += by;
                }
                if b {
                    s.b += by;
                }
                a || b
            }
            // doors and windows go with their wall (below)
            ElementKind::Opening(_)
            | ElementKind::Level(_)
            | ElementKind::View(_)
            | ElementKind::Sheet(_)
            | ElementKind::Material(_)
            | ElementKind::Grid(_)
            | ElementKind::GridGroup(_)
            | ElementKind::Detail(_)
            | ElementKind::Reference(_) => false,
            _ => {
                // anything else placed at a point: moved whole when its
                // point is in the window
                match pivot(model, e.id) {
                    Some(p) if inside(p.truncate()) => {
                        if let Some(m) = transformed(model, e, &Xform::Move(by.extend(0.0))) {
                            after = m;
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
        };
        if changed && after != *e {
            tx = tx.replace(e.clone(), after);
            n += 1;
        }
    }
    // the doors and windows of a stretched wall: one in the window goes
    // with it, the rest keep their place; all stay within the wall
    for e in model.iter() {
        let ElementKind::Opening(o) = &e.kind else {
            continue;
        };
        let Some(ElementKind::Wall(before)) = model.get(o.host).map(|h| &h.kind) else {
            continue;
        };
        let Some(ElementKind::Wall(after)) = tx.ops.iter().rev().find_map(|op| match op {
            crate::Op::Replace { after, .. } if after.id == o.host => Some(&after.kind),
            _ => None,
        }) else {
            // its wall not stretched: moved whole when it sits in the window
            continue;
        };
        let d0 = (before.end - before.start).normalize_or_zero();
        let mid = before.start + d0 * (o.offset + o.width / 2.0);
        let moved = if inside(mid) { mid + by } else { mid };
        let d1 = (after.end - after.start).normalize_or_zero();
        let len = after.start.distance(after.end);
        let left = (moved - after.start).dot(d1) - o.width / 2.0;
        let offset = left.clamp(0.0, (len - o.width).max(0.0));
        if (offset - o.offset).abs() > 1e-9 {
            let mut a = e.clone();
            if let ElementKind::Opening(x) = &mut a.kind {
                x.offset = offset;
            }
            tx = tx.replace(e.clone(), a);
        }
    }
    (tx, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wall_shortened_from_its_start_keeps_its_door_in_place() {
        use crate::{Opening, OpeningKind};
        let level = Element::new(
            "L0",
            ElementKind::Level(crate::Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let lid = level.id;
        let wall = Element::new(
            "W",
            ElementKind::Wall(crate::Wall::new(
                lid,
                DVec2::ZERO,
                DVec2::new(6.0, 0.0),
                0.2,
                3.0,
            )),
        );
        let wid = wall.id;
        let mut o = Opening {
            host: wid,
            kind: OpeningKind::Door,
            offset: 3.0,
            sill: 0.0,
            width: 0.9,
            height: 2.1,
            ..serde_json::from_str::<Opening>(&format!(
                r#"{{"host":"{}","kind":"Door","offset":3.0,"sill":0.0,"width":0.9,"height":2.1}}"#,
                wid.0
            ))
            .unwrap()
        };
        o.offset = 3.0;
        let door = Element::new("D", ElementKind::Opening(o));
        let did = door.id;
        let model = Model::from_elements([level, wall, door]).unwrap();
        // the start pulled 2 m in: shorter, the door where it was
        let (tx, _) = stretch(
            &model,
            Some(lid),
            DVec2::new(-1.0, -1.0),
            DVec2::new(1.0, 1.0),
            DVec2::new(2.0, 0.0),
        );
        let mut doc = crate::Document::from_model(model);
        doc.commit(tx).unwrap();
        let ElementKind::Opening(o) = &doc.model().get(did).unwrap().kind else {
            panic!()
        };
        assert!((o.offset - 1.0).abs() < 1e-9, "{}", o.offset);
    }
    use crate::{Level, Slab, Wall};

    #[test]
    fn a_wall_end_in_the_window_moves_the_other_stays() {
        let level = Element::new(
            "L0",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let lid = level.id;
        let wall = Element::new(
            "W",
            ElementKind::Wall(Wall::new(lid, DVec2::ZERO, DVec2::new(5.0, 0.0), 0.2, 3.0)),
        );
        let wid = wall.id;
        let slab = Element::new(
            "S",
            ElementKind::Slab(Slab::new(
                lid,
                vec![
                    DVec2::ZERO,
                    DVec2::new(5.0, 0.0),
                    DVec2::new(5.0, 4.0),
                    DVec2::new(0.0, 4.0),
                ],
                0.2,
            )),
        );
        let sid = slab.id;
        let model = Model::from_elements([level, wall, slab]).unwrap();
        // a window round the right-hand side, stretched 1 m to the right
        let (tx, n) = stretch(
            &model,
            Some(lid),
            DVec2::new(4.0, -1.0),
            DVec2::new(6.0, 5.0),
            DVec2::X,
        );
        assert_eq!(n, 2);
        let mut doc = crate::Document::from_model(model);
        doc.commit(tx).unwrap();
        let ElementKind::Wall(w) = &doc.model().get(wid).unwrap().kind else {
            panic!()
        };
        assert_eq!((w.start, w.end), (DVec2::ZERO, DVec2::new(6.0, 0.0)));
        let ElementKind::Slab(s) = &doc.model().get(sid).unwrap().kind else {
            panic!()
        };
        assert_eq!(s.boundary[0], DVec2::ZERO);
        assert_eq!(s.boundary[1], DVec2::new(6.0, 0.0));
        assert_eq!(s.boundary[2], DVec2::new(6.0, 4.0));
    }
}
