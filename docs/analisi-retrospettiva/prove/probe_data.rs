//! Prove esplorative (analisi retrospettiva, NON parte del progetto).
use creabim_core::{Document, ElementKind, Model, Transaction};

fn samples() -> Vec<(&'static str, Model)> {
    vec![
        ("starter", creabim_core::starter::starter_house()),
        ("sample", creabim_core::sample::sample_house()),
        ("layered", creabim_core::sample::layered_house()),
        ("typed", creabim_core::sample::typed_house()),
        ("furnished", creabim_core::sample::furnished_house()),
        ("garden", creabim_core::garden::garden_house()),
    ]
}

fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("probe-{}-{name}.creabim", std::process::id()))
}

/// P1: ogni campione salvato e riaperto e' identico; JSON idem.
#[test]
fn p1_samples_roundtrip() {
    let mut bad = vec![];
    for (n, m) in samples() {
        let doc = Document::from_model(m.clone());
        let p = tmp(n);
        creabim_store::save(&p, &doc).unwrap();
        let back = creabim_store::open(&p).unwrap();
        if back.model() != &m { bad.push(format!("{n}: .creabim differs")); }
        let j = m.to_json();
        match Model::from_json(&j) {
            Ok(b) if b == m => {}
            Ok(_) => bad.push(format!("{n}: json differs")),
            Err(e) => bad.push(format!("{n}: json err {e}")),
        }
        let _ = std::fs::remove_file(&p);
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// P2: la cronologia salvata: annulla tutto dopo la riapertura = annulla tutto prima.
#[test]
fn p2_history_roundtrip() {
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let ids: Vec<_> = doc.model().iter().filter(|e| matches!(e.kind, ElementKind::Wall(_))).map(|e| e.id).take(3).collect();
    for id in &ids {
        let mut e = doc.model().get(*id).unwrap().clone();
        if let ElementKind::Wall(w) = &mut e.kind { w.height += 0.1; }
        let old = doc.model().get(*id).unwrap().clone();
        doc.commit(Transaction::new("probe").replace(old, e)).unwrap();
    }
    doc.undo();
    let p = tmp("hist");
    creabim_store::save(&p, &doc).unwrap();
    let mut back = creabim_store::open(&p).unwrap();
    assert_eq!(back.model(), doc.model());
    assert_eq!(back.undo_steps().len(), doc.undo_steps().len());
    assert_eq!(back.redo_steps().len(), doc.redo_steps().len());
    assert!(back.redo());
    assert!(doc.redo());
    assert_eq!(back.model(), doc.model(), "redo after reopen");
    while back.undo() {}
    while doc.undo() {}
    assert_eq!(back.model(), doc.model(), "undo all after reopen");
    let _ = std::fs::remove_file(&p);
}

/// P3: un valore non finito (NaN/inf) nel modello: si salva? si riapre?
#[test]
fn p3_nan_in_model_save_and_reopen() {
    let mut report = vec![];
    for (label, v) in [("nan", f64::NAN), ("inf", f64::INFINITY)] {
        let mut doc = Document::from_model(creabim_core::starter::starter_house());
        let id = doc.model().iter().find(|e| matches!(e.kind, ElementKind::Wall(_))).unwrap().id;
        let old = doc.model().get(id).unwrap().clone();
        let mut e = old.clone();
        if let ElementKind::Wall(w) = &mut e.kind { w.height = v; }
        let c = doc.commit(Transaction::new("probe").replace(old, e));
        report.push(format!("{label}: commit {:?}", c.is_ok()));
        let p = tmp(label);
        let s = creabim_store::save(&p, &doc);
        report.push(format!("{label}: save {:?}", s.as_ref().map_err(|e| e.to_string())));
        if s.is_ok() {
            let r = creabim_store::open(&p);
            report.push(format!("{label}: reopen {:?}", r.as_ref().map(|_| ()).map_err(|e| e.to_string())));
        }
        let j = doc.model().to_json();
        report.push(format!("{label}: json reopen {:?}", Model::from_json(&j).map(|_| ())));
        let _ = std::fs::remove_file(&p);
    }
    println!("P3 {report:#?}");
}
