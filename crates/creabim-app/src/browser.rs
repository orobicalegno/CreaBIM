//! How the project browser lists views: in groups by kind (floor plans, 3D
//! views, sections, elevations, details, schedules), floor plans in the order of
//! their levels, from the lowest up. A view the user drags to another place
//! in its group keeps that place (`View::browser_order`); "Sort by level"
//! (or "Sort automatically") gives the group back its automatic order.

use creabim_core::{levels, ElementId, ElementKind, Model, Transaction, ViewKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewGroup {
    Plans,
    Views3d,
    Sections,
    Elevations,
    Details,
    Schedules,
}

impl ViewGroup {
    pub const ALL: [ViewGroup; 6] = [
        ViewGroup::Plans,
        ViewGroup::Views3d,
        ViewGroup::Sections,
        ViewGroup::Elevations,
        ViewGroup::Details,
        ViewGroup::Schedules,
    ];

    /// A view's group: a callout's detail view is in Details, whatever
    /// it draws.
    pub fn of_view(v: &creabim_core::View) -> Self {
        if v.callout.is_some() {
            ViewGroup::Details
        } else {
            Self::of(&v.kind)
        }
    }

    pub fn of(kind: &ViewKind) -> Self {
        match kind {
            ViewKind::Plan { .. } => ViewGroup::Plans,
            ViewKind::Axonometric { .. } | ViewKind::Perspective { .. } => ViewGroup::Views3d,
            ViewKind::Section { cut: true, .. } => ViewGroup::Sections,
            ViewKind::Section { cut: false, .. } => ViewGroup::Elevations,
            ViewKind::Schedule { .. } => ViewGroup::Schedules,
        }
    }

    /// The group's key in the browser page, also what its messages name.
    pub fn key(self) -> &'static str {
        match self {
            ViewGroup::Plans => "views:plans",
            ViewGroup::Views3d => "views:3d",
            ViewGroup::Sections => "views:sections",
            ViewGroup::Elevations => "views:elevations",
            ViewGroup::Details => "views:details",
            ViewGroup::Schedules => "views:schedules",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.key() == key)
    }

    pub fn title(self) -> &'static str {
        match self {
            ViewGroup::Plans => "Floor plans",
            ViewGroup::Views3d => "3D views",
            ViewGroup::Sections => "Sections",
            ViewGroup::Elevations => "Elevations",
            ViewGroup::Details => "Details",
            ViewGroup::Schedules => "Schedules",
        }
    }

    /// What the automatic order of the group is called.
    pub fn auto_label(self) -> &'static str {
        match self {
            ViewGroup::Plans => "Sort by level",
            _ => "Sort automatically",
        }
    }
}

/// Every view of the model, grouped by kind in browser order, leaving out
/// empty groups.
pub fn view_groups(model: &Model) -> Vec<(ViewGroup, Vec<ElementId>)> {
    let elevations: Vec<(ElementId, f64)> = levels::sorted(model)
        .into_iter()
        .map(|(id, _, l)| (id, l.elevation))
        .collect();
    let level_rank = |level: ElementId| {
        elevations
            .iter()
            .position(|(id, _)| *id == level)
            .unwrap_or(usize::MAX)
    };
    let mut out = vec![];
    for g in ViewGroup::ALL {
        // (manual place, automatic place, model order, id)
        let mut rows: Vec<(u32, usize, usize, ElementId)> = model
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match &e.kind {
                ElementKind::View(v) if ViewGroup::of_view(v) == g => {
                    let auto = match v.kind {
                        ViewKind::Plan { level, .. } => level_rank(level),
                        _ => 0,
                    };
                    // views never moved go after those placed by hand
                    Some((v.browser_order.unwrap_or(u32::MAX), auto, i, e.id))
                }
                _ => None,
            })
            .collect();
        if rows.is_empty() {
            continue;
        }
        rows.sort();
        out.push((g, rows.into_iter().map(|r| r.3).collect()));
    }
    out
}

/// Does any view of the group have a place set by hand?
pub fn is_manual(model: &Model, group: ViewGroup) -> bool {
    model.iter().any(|e| match &e.kind {
        ElementKind::View(v) => ViewGroup::of_view(v) == group && v.browser_order.is_some(),
        _ => false,
    })
}

/// Move `moved` to just before `before` in its group (to the end with
/// `None`). Every view of the group then keeps its place by hand.
pub fn reorder(model: &Model, moved: ElementId, before: Option<ElementId>) -> Transaction {
    let mut tx = Transaction::new("Reorder views");
    let groups = view_groups(model);
    let Some((_, ids)) = groups.iter().find(|(_, ids)| ids.contains(&moved)) else {
        return tx;
    };
    if before == Some(moved) || before.is_some_and(|b| !ids.contains(&b)) {
        return tx;
    }
    let mut order: Vec<ElementId> = ids.iter().copied().filter(|id| *id != moved).collect();
    let at = before
        .and_then(|b| order.iter().position(|id| *id == b))
        .unwrap_or(order.len());
    order.insert(at, moved);
    for (i, id) in order.into_iter().enumerate() {
        tx = set_order(model, tx, id, Some(i as u32));
    }
    tx
}

/// Give a group back its automatic order.
pub fn sort_auto(model: &Model, group: ViewGroup) -> Transaction {
    let mut tx = Transaction::new(group.auto_label());
    if let Some((_, ids)) = view_groups(model).into_iter().find(|(g, _)| *g == group) {
        for id in ids {
            tx = set_order(model, tx, id, None);
        }
    }
    tx
}

fn set_order(model: &Model, tx: Transaction, id: ElementId, order: Option<u32>) -> Transaction {
    let Some(e) = model.get(id) else { return tx };
    let mut after = e.clone();
    if let ElementKind::View(v) = &mut after.kind {
        v.browser_order = order;
    }
    if after == *e {
        tx
    } else {
        tx.replace(e.clone(), after)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{sample::sample_house, Document, Element, View};

    fn names(model: &Model, group: ViewGroup) -> Vec<String> {
        view_groups(model)
            .into_iter()
            .find(|(g, _)| *g == group)
            .map(|(_, ids)| {
                ids.iter()
                    .map(|id| model.get(*id).unwrap().name.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn plans_follow_their_levels_and_views_group_by_kind() {
        let mut doc = Document::from_model(sample_house());
        // a level below the others, added last: its plan still comes first
        let (tx, _, _) = levels::add(doc.model(), "Basement", -3.0, 3.0, true);
        doc.commit(tx).unwrap();
        let plans = names(doc.model(), ViewGroup::Plans);
        assert_eq!(plans.first().unwrap(), "Basement plan");
        let groups: Vec<ViewGroup> = view_groups(doc.model()).iter().map(|g| g.0).collect();
        assert_eq!(groups[0], ViewGroup::Plans);
        assert!(groups.contains(&ViewGroup::Sections) && groups.contains(&ViewGroup::Elevations));
        // nothing is in two groups
        let n: usize = view_groups(doc.model()).iter().map(|g| g.1.len()).sum();
        let views = doc
            .model()
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::View(_)))
            .count();
        assert_eq!(n, views);
    }

    #[test]
    fn a_plan_dragged_keeps_its_place_until_sorted_by_level() {
        let mut doc = Document::from_model(sample_house());
        let before = names(doc.model(), ViewGroup::Plans);
        let ids = view_groups(doc.model())[0].1.clone();
        // the last plan to the top
        let last = *ids.last().unwrap();
        doc.commit(reorder(doc.model(), last, Some(ids[0])))
            .unwrap();
        let moved = names(doc.model(), ViewGroup::Plans);
        assert_eq!(moved[0], *before.last().unwrap());
        assert!(is_manual(doc.model(), ViewGroup::Plans));
        // it survives saving: the order is in the model
        let text = serde_json::to_string(doc.model()).unwrap();
        let read: Model = serde_json::from_str(&text).unwrap();
        assert_eq!(names(&read, ViewGroup::Plans), moved);
        // a plan made now goes after the ones placed by hand
        let lvl = levels::sorted(doc.model())[0].0;
        let extra = Element::new("Extra plan", ElementKind::View(View::plan(lvl)));
        doc.commit(Transaction::new("Add").insert(extra)).unwrap();
        assert_eq!(
            names(doc.model(), ViewGroup::Plans).last().unwrap(),
            "Extra plan"
        );
        // back to the levels' order, and undo brings the hand order back
        doc.commit(sort_auto(doc.model(), ViewGroup::Plans))
            .unwrap();
        assert!(!is_manual(doc.model(), ViewGroup::Plans));
        assert_eq!(names(doc.model(), ViewGroup::Plans)[0], before[0]);
        doc.undo();
        assert_eq!(
            names(doc.model(), ViewGroup::Plans)[0],
            *before.last().unwrap()
        );
    }

    #[test]
    fn reordering_across_groups_does_nothing() {
        let doc = Document::from_model(sample_house());
        let groups = view_groups(doc.model());
        let plan = groups[0].1[0];
        let other = groups[1].1[0];
        assert!(reorder(doc.model(), plan, Some(other)).ops.is_empty());
    }
}
