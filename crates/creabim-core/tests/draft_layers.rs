//! Drafting layers: the project's table of them, and linework that takes
//! its colour, line type and visibility from the layer it is on.

use creabim_core::detail::{layers, rename_layer, set_layers};
use creabim_core::{
    DetailItem, DetailKind, Document, DraftLayer, DraftLayers, DraftShape, Element, ElementKind,
    LinePen, LineType, Transaction, View,
};
use glam::DVec2;

#[test]
fn layers_are_kept_in_the_project_and_renamed_with_their_linework() {
    let mut doc = Document::new();
    let view = Element::new(
        "Section A",
        ElementKind::View(View::section(DVec2::ZERO, DVec2::new(5.0, 0.0), 3.0, true)),
    );
    let mut line = DetailItem::new(
        view.id,
        DetailKind::draft(DraftShape::Line),
        vec![DVec2::ZERO, DVec2::X],
    );
    line.layer = Some("Axes".into());
    let line = Element::new("Line", ElementKind::Detail(line));
    let line_id = line.id;
    doc.commit(Transaction::new("setup").insert(view).insert(line))
        .unwrap();
    // no table yet: no layers, and the line draws as its pen says
    assert!(layers(doc.model()).layers.is_empty());
    let table = DraftLayers {
        layers: vec![DraftLayer {
            name: "Axes".into(),
            color: Some([200, 0, 0]),
            linetype: LineType::DashDot,
            hidden: false,
        }],
    };
    let tx = set_layers(doc.model(), table.clone());
    doc.commit(tx).unwrap();
    assert_eq!(layers(doc.model()), table);
    // setting the same table again changes nothing
    assert!(set_layers(doc.model(), table.clone()).ops.is_empty());
    let item = |doc: &Document| match &doc.model().get(line_id).unwrap().kind {
        ElementKind::Detail(d) => d.clone(),
        _ => unreachable!(),
    };
    let look = item(&doc).look(&layers(doc.model())).unwrap();
    assert_eq!(
        (look.weight, look.rgb, look.linetype),
        (LinePen::Medium, Some([200, 0, 0]), LineType::DashDot)
    );
    // renamed, the line goes with it; one undo step puts both back
    let tx = rename_layer(doc.model(), "Axes", "Centre lines");
    doc.commit(tx).unwrap();
    assert_eq!(item(&doc).layer.as_deref(), Some("Centre lines"));
    assert!(layers(doc.model()).get("Centre lines").is_some());
    assert!(doc.undo());
    assert_eq!(item(&doc).layer.as_deref(), Some("Axes"));
    // a name that is blank or taken is refused
    assert!(rename_layer(doc.model(), "Axes", "  ").ops.is_empty());
    // two layers of one name are not a valid table
    let twice = DraftLayers {
        layers: vec![table.layers[0].clone(), table.layers[0].clone()],
    };
    let tx = set_layers(doc.model(), twice);
    assert!(doc.commit(tx).is_err());
    // an old file's linework, with none of the new fields, still reads
    let old = r#"{"view":"00000000-0000-0000-0000-000000000000","kind":{"Draft":{"shape":"Line","pen":"Heavy"}},"points":[[0.0,0.0],[1.0,0.0]]}"#;
    if let Ok(d) = serde_json::from_str::<DetailItem>(old) {
        assert!(d.layer.is_none() && d.color.is_none() && d.fill.is_none());
    }
}

#[test]
fn linework_is_made_into_a_symbol_placed_and_exploded() {
    use creabim_core::detail::{explode, make_symbol, symbols};
    let mut doc = Document::new();
    let view = Element::new(
        "Section A",
        ElementKind::View(View::section(DVec2::ZERO, DVec2::new(5.0, 0.0), 3.0, true)),
    );
    let view_id = view.id;
    let p = DVec2::new;
    let piece = |shape: DraftShape, pts: Vec<DVec2>| {
        Element::new(
            "Line",
            ElementKind::Detail(DetailItem::new(view_id, DetailKind::draft(shape), pts)),
        )
    };
    let (a, b) = (
        piece(DraftShape::Line, vec![p(2.0, 1.0), p(3.0, 1.0)]),
        piece(DraftShape::Circle, vec![p(2.5, 1.5), p(3.0, 1.5)]),
    );
    let ids = [a.id, b.id];
    doc.commit(Transaction::new("setup").insert(view).insert(a).insert(b))
        .unwrap();
    let details = |doc: &Document| -> Vec<(creabim_core::ElementId, DetailItem)> {
        doc.model()
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Detail(d) => Some((e.id, d.clone())),
                _ => None,
            })
            .collect()
    };
    // the two become one placed symbol, its origin at their lower left
    let (tx, placed) = make_symbol(doc.model(), &ids, "Tap").unwrap();
    doc.commit(tx).unwrap();
    let now = details(&doc);
    assert_eq!(now.len(), 1);
    assert_eq!(now[0].0, placed);
    assert_eq!(now[0].1.points, vec![p(2.0, 1.0), p(3.0, 1.0)]);
    let all = symbols(doc.model());
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].1, "Tap");
    let def = all[0].0;
    // nothing selected, or no linework, makes no symbol
    assert!(make_symbol(doc.model(), &[], "X").is_none());
    assert!(make_symbol(doc.model(), &[placed], "X").is_none());
    // a second one placed elsewhere, turned a quarter and twice the size
    let second = Element::new(
        "Tap",
        ElementKind::Detail(DetailItem::new(
            view_id,
            DetailKind::Symbol { def },
            vec![p(10.0, 0.0), p(10.0, 2.0)],
        )),
    );
    let second_id = second.id;
    doc.commit(Transaction::new("place").insert(second))
        .unwrap();
    // exploded, it is linework again, where the symbol put it
    let tx = explode(doc.model(), second_id).unwrap();
    doc.commit(tx).unwrap();
    let now = details(&doc);
    assert_eq!(now.len(), 3);
    let line = now
        .iter()
        .find(|d| d.1.kind == DetailKind::draft(DraftShape::Line))
        .unwrap();
    // the symbol's line ran from (0, 0) to (1, 0): now up from (10, 0), 2 long
    assert!(line.1.points[0].distance(p(10.0, 0.0)) < 1e-9);
    assert!(line.1.points[1].distance(p(10.0, 2.0)) < 1e-9);
    // deleting the symbol takes its placings with it, not the exploded linework
    let tx = Transaction::new("Delete symbol").remove_cascade(doc.model(), def);
    doc.commit(tx).unwrap();
    assert_eq!(details(&doc).len(), 2);
    assert!(symbols(doc.model()).is_empty());
}
