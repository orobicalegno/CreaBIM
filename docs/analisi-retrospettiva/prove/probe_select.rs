
// ===== PROVA ESPLORATIVA selezione (analisi; non nel progetto) =====
#[test]
fn probe_sel_click_on_door_window_tag() {
    let m = creabim_core::starter::starter_house();
    let level = m.iter().find_map(|e| match &e.kind { ElementKind::View(v) if e.name == "Ground Floor plan" => match v.kind { creabim_core::ViewKind::Plan { level, .. } => Some(level), _ => None }, _ => None }).unwrap();
    let mut probes: Vec<(String, DVec2)> = vec![];
    for e in m.iter() {
        if let ElementKind::Opening(o) = &e.kind {
            if let Some(ElementKind::Wall(w)) = m.get(o.host).map(|h| &h.kind) {
                if w.level != level { continue; }
                let dir = (w.end - w.start).normalize();
                let c = w.start + dir * (o.offset + o.width / 2.0);
                probes.push((format!("{:?} '{}' (centro, asse muro)", o.kind, e.name), c));
                probes.push((format!("{:?} '{}' (bordo telaio)", o.kind, e.name), w.start + dir * (o.offset + 0.03)));
            }
        }
        if probes.len() > 8 { break; }
    }
    for (what, p) in probes {
        let mut pane = Pane::open("Ground Floor plan", p, 150.0);
        let (center, scale) = pane.camera();
        let at = Xf { rect: pane.rect, center, scale, rot: 0.0 }.to_screen(p);
        pane.hover(at);
        let press = |pressed| Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        pane.frame(vec![press(true)]);
        pane.frame(vec![press(false)]);
        pane.frame(vec![]);
        let picked: Vec<String> = pane.sel.all().into_iter().filter_map(|id| pane.doc.model().get(id).map(|e| format!("{}:{}", match &e.kind { ElementKind::Wall(_) => "Muro", ElementKind::Opening(_) => "Apertura", ElementKind::Tag(_) => "Etichetta", ElementKind::Dimension(_) => "Quota", ElementKind::Grid(_) => "Griglia", _ => "Altro" }, e.name))).collect();
        println!("SEL clic su {what}: preso {picked:?}");
    }
}
