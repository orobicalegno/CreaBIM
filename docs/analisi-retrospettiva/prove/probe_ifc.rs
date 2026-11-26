use creabim_core::ElementKind;
fn tally(m: &creabim_core::Model) -> std::collections::BTreeMap<String, usize> {
    let mut t = std::collections::BTreeMap::new();
    for e in m.iter() {
        let k = format!("{:?}", e.kind).split('(').next().unwrap().to_string();
        *t.entry(k).or_insert(0) += 1;
    }
    t
}
#[test]
fn probe_ifc_roundtrip_counts() {
    for (n, m) in [
        ("starter", creabim_core::starter::starter_house()),
        ("layered", creabim_core::sample::layered_house()),
        ("garden", creabim_core::garden::garden_house()),
    ] {
        let text = creabim_ifc::export_ifc4(&m, &Default::default());
        let r = creabim_ifc::import_ifc(&text).unwrap();
        let (a, b) = (tally(&m), tally(&r.model));
        let mut d = vec![];
        for k in a.keys().chain(b.keys()).collect::<std::collections::BTreeSet<_>>() {
            let (x, y) = (a.get(k).copied().unwrap_or(0), b.get(k).copied().unwrap_or(0));
            if x != y { d.push(format!("{k}: {x}->{y}")); }
        }
        let walls_h = |m: &creabim_core::Model| m.iter().filter_map(|e| match &e.kind { ElementKind::Wall(w) => Some((w.height * 1000.0).round() as i64), _ => None }).sum::<i64>();
        println!("IFC {n}: differenze conteggi {d:?}; somma altezze muri mm {} -> {}; avvisi {}", walls_h(&m), walls_h(&r.model), r.warnings.len());
        for w in r.warnings.iter().take(5) { println!("   avviso: {w}"); }
    }
}
