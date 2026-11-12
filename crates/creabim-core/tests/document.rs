use creabim_core::*;
use glam::DVec2;

fn level() -> Element {
    Element::new(
        "Level 0",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    )
}

fn wall(level: ElementId) -> Element {
    Element::new(
        "Wall",
        ElementKind::Wall(Wall {
            level,
            start: DVec2::ZERO,
            end: DVec2::new(5.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            base_offset: 0.0,
            material: None,
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
    )
}

fn door(host: ElementId) -> Element {
    Element::new(
        "Door",
        ElementKind::Opening(Opening {
            host,
            kind: OpeningKind::Door,
            offset: 1.0,
            sill: 0.0,
            width: 0.9,
            height: 2.1,
            material: None,
            frame: Default::default(),
            flip_hand: false,
            flip_facing: false,
            wrap: None,
        }),
    )
}

#[test]
fn commit_undo_redo_round_trip() {
    let mut doc = Document::new();
    let l = level();
    let w = wall(l.id);
    let (lid, wid) = (l.id, w.id);
    doc.commit(Transaction::new("Add level").insert(l)).unwrap();
    doc.commit(Transaction::new("Add wall").insert(w)).unwrap();
    assert_eq!(doc.model().len(), 2);

    doc.edit(wid, "Thicken wall", |e| {
        if let ElementKind::Wall(w) = &mut e.kind {
            w.thickness = 0.3;
        }
    })
    .unwrap();

    assert_eq!(doc.undo_label(), Some("Thicken wall"));
    assert!(doc.undo());
    match &doc.model().get(wid).unwrap().kind {
        ElementKind::Wall(w) => assert_eq!(w.thickness, 0.2),
        _ => unreachable!(),
    }
    assert!(doc.undo());
    assert!(doc.model().get(wid).is_none());
    assert!(doc.redo());
    assert!(doc.redo());
    match &doc.model().get(wid).unwrap().kind {
        ElementKind::Wall(w) => assert_eq!(w.thickness, 0.3),
        _ => unreachable!(),
    }
    assert!(doc.model().get(lid).is_some());
}

#[test]
fn dangling_reference_is_refused_and_nothing_changes() {
    let mut doc = Document::new();
    let l = level();
    let w = wall(ElementId::new());
    let err = doc
        .commit(Transaction::new("Bad").insert(l).insert(w))
        .unwrap_err();
    assert!(matches!(err, ModelError::DanglingReference { .. }));
    assert!(doc.model().is_empty(), "failed transaction must roll back");
    assert!(!doc.can_undo());
}

#[test]
fn removing_a_wall_cascades_to_its_doors_and_undo_restores_both() {
    let mut doc = Document::new();
    let l = level();
    let w = wall(l.id);
    let d = door(w.id);
    let (wid, did) = (w.id, d.id);
    doc.commit(Transaction::new("Build").insert(l).insert(w).insert(d))
        .unwrap();

    let plain = Transaction::new("Remove wall only");
    let plain = Transaction {
        ops: vec![Op::Remove(doc.model().get(wid).unwrap().clone())],
        ..plain
    };
    assert!(matches!(
        doc.commit(plain),
        Err(ModelError::StillReferenced { .. })
    ));

    let tx = Transaction::new("Delete wall").remove_cascade(doc.model(), wid);
    doc.commit(tx).unwrap();
    assert!(doc.model().get(wid).is_none() && doc.model().get(did).is_none());
    doc.undo();
    assert!(doc.model().get(wid).is_some() && doc.model().get(did).is_some());
}

#[test]
fn opening_host_must_be_a_wall() {
    let mut doc = Document::new();
    let l = level();
    let d = door(l.id);
    doc.commit(Transaction::new("Level").insert(l)).unwrap();
    assert!(matches!(
        doc.commit(Transaction::new("Door").insert(d)),
        Err(ModelError::Invalid(..))
    ));
}

#[test]
fn json_round_trip_keeps_every_element() {
    let mut doc = Document::new();
    let l = level();
    let w = wall(l.id);
    let d = door(w.id);
    doc.commit(Transaction::new("Build").insert(l).insert(w).insert(d))
        .unwrap();
    let json = doc.model().to_json();
    let back = Model::from_json(&json).unwrap();
    assert_eq!(back.len(), 3);
    for e in doc.model().iter() {
        assert_eq!(back.get(e.id), Some(e));
    }
}

#[test]
fn merged_edits_undo_in_one_step() {
    let mut doc = Document::new();
    let l = level();
    let w = wall(l.id);
    let wid = w.id;
    doc.commit(Transaction::new("Build").insert(l).insert(w))
        .unwrap();
    for t in [0.21, 0.22, 0.25] {
        doc.edit_merged(wid, "Wall thickness", |e| {
            if let ElementKind::Wall(w) = &mut e.kind {
                w.thickness = t;
            }
        })
        .unwrap();
    }
    let thickness = |doc: &Document| match &doc.model().get(wid).unwrap().kind {
        ElementKind::Wall(w) => w.thickness,
        _ => unreachable!(),
    };
    assert_eq!(thickness(&doc), 0.25);
    doc.undo();
    assert_eq!(thickness(&doc), 0.2);
    doc.redo();
    assert_eq!(thickness(&doc), 0.25);
}

fn set_thickness(t: f64) -> impl FnOnce(&mut Element) {
    move |e| {
        if let ElementKind::Wall(w) = &mut e.kind {
            w.thickness = t;
        }
    }
}

#[test]
fn from_parts_restores_history_and_into_parts_takes_it_apart() {
    let mut doc = Document::new();
    let l = level();
    let w = wall(l.id);
    let wid = w.id;
    doc.commit(Transaction::new("Build").insert(l).insert(w))
        .unwrap();
    doc.edit(wid, "Thicken", set_thickness(0.3)).unwrap();
    doc.edit(wid, "Thicken more", set_thickness(0.4)).unwrap();
    doc.undo();

    let (model, undo, redo) = doc.into_parts();
    assert_eq!((undo.len(), redo.len()), (2, 1));
    let mut back = Document::from_parts(model.clone(), undo, redo).unwrap();
    assert_eq!(back.model(), &model);
    assert_eq!(back.undo_label(), Some("Thicken"));
    assert_eq!(back.redo_label(), Some("Thicken more"));
    assert!(back.redo());
    assert!(back.undo() && back.undo() && back.undo());
    assert!(back.model().is_empty());
}

#[test]
fn from_parts_rejects_a_history_that_does_not_fit_the_model() {
    let l = level();
    // the model lacks the level the undo step claims to have inserted
    let undo = vec![Transaction::new("Add level").insert(l)];
    assert!(Document::from_parts(Model::new(), undo, vec![]).is_err());
}

#[test]
fn step_ids_change_on_commit_and_amend_but_not_on_undo() {
    let mut doc = Document::new();
    let l = level();
    let w = wall(l.id);
    let wid = w.id;
    doc.commit(Transaction::new("Build").insert(l).insert(w))
        .unwrap();
    doc.edit_merged(wid, "Drag", set_thickness(0.21)).unwrap();
    let first = doc.undo_steps().last().unwrap().id;
    doc.edit_merged(wid, "Drag", set_thickness(0.22)).unwrap();
    let amended = doc.undo_steps().last().unwrap().id;
    assert_ne!(first, amended, "amending must issue a new id");
    assert_eq!(doc.undo_steps().len(), 2);
    doc.undo();
    assert_eq!(doc.redo_steps().last().unwrap().id, amended);
}

#[test]
fn from_elements_keeps_order_and_checks_references() {
    let l = level();
    let w = wall(l.id);
    // a wall listed before its level is fine: references are checked at the end
    let m = Model::from_elements([w.clone(), l.clone()]).unwrap();
    let ids: Vec<_> = m.iter().map(|e| e.id).collect();
    assert_eq!(ids, vec![w.id, l.id]);
    assert!(matches!(
        Model::from_elements([w.clone()]),
        Err(ModelError::DanglingReference { .. })
    ));
    assert!(matches!(
        Model::from_elements([l.clone(), l]),
        Err(ModelError::Duplicate(_))
    ));
}

#[test]
fn model_equality_is_ordered_and_ignores_revision() {
    let (a, b) = (level(), level());
    let m1 = Model::from_elements([a.clone(), b.clone()]).unwrap();
    let mut doc = Document::new();
    doc.commit(Transaction::new("x").insert(a.clone()).insert(b.clone()))
        .unwrap();
    assert_ne!(m1.revision(), doc.model().revision());
    assert_eq!(&m1, doc.model());
    assert_ne!(m1, Model::from_elements([b, a]).unwrap());
}

#[test]
fn kind_name_matches_the_serde_tag() {
    for e in sample::sample_house().iter() {
        let v = serde_json::to_value(&e.kind).unwrap();
        let tag = v.as_object().unwrap().keys().next().unwrap().clone();
        assert_eq!(e.kind.name(), tag);
    }
}
