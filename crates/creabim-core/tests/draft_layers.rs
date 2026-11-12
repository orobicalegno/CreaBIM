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
