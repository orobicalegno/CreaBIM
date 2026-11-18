//! Cameras: a perspective view of the model, cut to what the camera sees,
//! hidden lines removed, inside its picture's frame.

use creabim_core::{Document, Element, ElementKind, Transaction, View, ViewKind, ViewStyle};
use creabim_geom::plan::PlanItem;
use creabim_geom::section::{view_drawing, CameraFrame};
use glam::DVec3;

fn camera(eye: DVec3, target: DVec3) -> (Document, creabim_core::ElementId) {
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let mut v = View::perspective(eye, target);
    v.style = ViewStyle::HiddenLine;
    let e = Element::new("Camera 1", ElementKind::View(v));
    let id = e.id;
    doc.commit(Transaction::new("camera").insert(e)).unwrap();
    (doc, id)
}

#[test]
fn a_camera_draws_what_it_sees_inside_its_picture() {
    // standing in the garden south-east of the house, at eye height,
    // looking at its middle
    let (doc, id) = camera(DVec3::new(18.0, -12.0, 1.6), DVec3::new(5.0, 4.0, 3.0));
    let d = view_drawing(doc.model(), id).expect("a camera draws");
    let lines: Vec<(glam::DVec2, glam::DVec2)> = d
        .items
        .iter()
        .filter_map(|i| match i {
            PlanItem::Line { a, b, .. } => Some((*a, *b)),
            _ => None,
        })
        .collect();
    assert!(lines.len() > 200, "{} lines", lines.len());
    let ElementKind::View(v) = &doc.model().get(id).unwrap().kind else {
        unreachable!()
    };
    let ViewKind::Perspective {
        eye,
        target,
        fov,
        aspect,
    } = v.kind
    else {
        unreachable!()
    };
    let frame = CameraFrame::new(eye, target, fov, aspect);
    let (lo, hi) = frame.picture();
    let pad = 1e-3 * (hi - lo).length();
    for (a, b) in &lines {
        for p in [a, b] {
            assert!(
                p.x >= lo.x - pad && p.x <= hi.x + pad && p.y >= lo.y - pad && p.y <= hi.y + pad,
                "{p} outside {lo} {hi}"
            );
        }
    }
    // the target is at the middle of the picture, true size at its depth
    let t = frame.project(target);
    assert!(t.truncate().length() < 1e-9);
    if let Ok(path) = std::env::var("CREABIM_PERSPECTIVE_SVG") {
        std::fs::write(path, d.to_svg(v.scale)).unwrap();
    }
}

#[test]
fn a_camera_inside_the_house_sees_only_what_is_in_front_of_it() {
    // in the living room, looking along it: nothing behind the eye drawn,
    // nothing blows up beside it
    let (doc, id) = camera(DVec3::new(3.0, 2.0, 1.6), DVec3::new(8.0, 3.0, 1.4));
    let d = view_drawing(doc.model(), id).expect("a camera draws");
    let (lo, hi) = d.bounds().expect("it sees something");
    let ElementKind::View(v) = &doc.model().get(id).unwrap().kind else {
        unreachable!()
    };
    let ViewKind::Perspective {
        eye,
        target,
        fov,
        aspect,
    } = v.kind
    else {
        unreachable!()
    };
    let (plo, phi) = CameraFrame::new(eye, target, fov, aspect).picture();
    let pad = 1e-3 * (phi - plo).length();
    assert!(
        lo.x >= plo.x - pad && hi.x <= phi.x + pad && lo.y >= plo.y - pad && hi.y <= phi.y + pad
    );
}

#[test]
fn a_camera_shows_in_the_plan_of_the_floor_it_stands_on() {
    let (doc, id) = camera(DVec3::new(18.0, -12.0, 1.6), DVec3::new(5.0, 4.0, 3.0));
    let plan = |name: &str| {
        let v = doc
            .model()
            .iter()
            .find(|e| e.name == name && matches!(e.kind, ElementKind::View(_)))
            .unwrap()
            .id;
        view_drawing(doc.model(), v).unwrap()
    };
    let mine = |d: &creabim_geom::PlanView| {
        d.items
            .iter()
            .filter(|i| matches!(i, PlanItem::Line { id: l, .. } if *l == id))
            .count()
    };
    let ground = plan("Ground Floor plan");
    // body, lens, the two edges of what it sees, the target's cross
    assert_eq!(mine(&ground), 4 + 4 + 2 + 2);
    assert_eq!(mine(&plan("First Floor plan")), 0);
    if let Ok(path) = std::env::var("CREABIM_CAMERA_PLAN_SVG") {
        std::fs::write(path, ground.to_svg(50.0)).unwrap();
    }
}
