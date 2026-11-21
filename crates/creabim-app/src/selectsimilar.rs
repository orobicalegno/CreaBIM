//! Select similar: from what is selected, every element of the same type
//! or the same category, in the view or the whole project; tags of the
//! same kind; and a filter that keeps only one category of a mixed
//! selection. In the Edit menu and the right-click menu.

use super::*;
use crate::ui_model::Item;
use creabim_core::visibility::{cutters, Category};
use std::collections::HashSet;

/// What "similar" means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Similar {
    /// The same type (or, for an element with none, the same sizes).
    Type,
    /// The same category: walls, doors, slabs…
    Category,
    /// Tags of what is the same kind: door tags, window tags…
    Tags,
}

/// What makes elements "the same type": their type if they have one,
/// else their kind and main sizes, so untyped walls of one thickness or
/// windows of one size go together. None for what has neither.
pub(super) fn type_key(model: &Model, e: &Element) -> Option<String> {
    if let Some(t) = e.of_type.as_ref().filter(|t| model.get(t.id).is_some()) {
        return Some(format!("type {}", t.id.0));
    }
    let mm = |v: f64| (v * 1000.0).round() as i64;
    Some(match &e.kind {
        ElementKind::Wall(w) => format!("wall {}", mm(w.thickness)),
        ElementKind::Slab(s) => format!("slab {}", mm(s.thickness)),
        ElementKind::Opening(o) => format!("{:?} {}x{}", o.kind, mm(o.width), mm(o.height)),
        ElementKind::Column(c) => format!("column {}", c.profile.label()),
        ElementKind::Beam(b) => format!("beam {}x{}", mm(b.width), mm(b.depth)),
        ElementKind::Roof(r) => format!("roof {:?} {}", r.shape, mm(r.thickness)),
        ElementKind::Footing(f) => format!("footing {:?} {}", f.kind, mm(f.width)),
        ElementKind::Tag(t) => format!("tag {:?}", host_category(model, t.host)),
        _ => return None,
    })
}

fn host_category(model: &Model, host: ElementId) -> Option<Category> {
    model
        .get(host)
        .and_then(|h| Category::of(h, &cutters(model)))
}

/// The elements of `model` like those in `seeds`, by `how`; with `within`,
/// only those among it (what a view shows).
pub(super) fn similar(
    model: &Model,
    seeds: &[ElementId],
    how: Similar,
    within: Option<&HashSet<ElementId>>,
) -> Vec<ElementId> {
    let cut = cutters(model);
    let seeds: Vec<&Element> = seeds.iter().filter_map(|id| model.get(*id)).collect();
    let keys: HashSet<String> = seeds.iter().filter_map(|e| type_key(model, e)).collect();
    let cats: HashSet<Category> = seeds.iter().filter_map(|e| Category::of(e, &cut)).collect();
    // tags: of the same kind of host, in the views the picked tags are in
    let tag_kinds: HashSet<(ElementId, Option<Category>)> = seeds
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Tag(t) => Some((t.view, host_category(model, t.host))),
            _ => None,
        })
        .collect();
    model
        .iter()
        .filter(|e| within.is_none_or(|w| w.contains(&e.id)))
        .filter(|e| match how {
            Similar::Type => type_key(model, e).is_some_and(|k| keys.contains(&k)),
            Similar::Category => Category::of(e, &cut).is_some_and(|c| cats.contains(&c)),
            Similar::Tags => match &e.kind {
                ElementKind::Tag(t) => tag_kinds.contains(&(t.view, host_category(model, t.host))),
                _ => false,
            },
        })
        .map(|e| e.id)
        .collect()
}

/// The categories of a selection, with how many of each, most first.
pub(super) fn categories_of(model: &Model, ids: &[ElementId]) -> Vec<(Category, usize)> {
    let cut = cutters(model);
    let mut out: Vec<(Category, usize)> = vec![];
    for e in ids.iter().filter_map(|id| model.get(*id)) {
        if let Some(c) = Category::of(e, &cut) {
            match out.iter_mut().find(|(k, _)| *k == c) {
                Some((_, n)) => *n += 1,
                None => out.push((c, 1)),
            }
        }
    }
    out.sort_by_key(|c| std::cmp::Reverse(c.1));
    out
}

impl CreaBimApp {
    /// What the view the menu is about shows: the 3D view's scene less
    /// what is hidden there, or the active drawing's elements.
    fn ids_in_view(&mut self) -> HashSet<ElementId> {
        if self.shell.context_3d || self.active_view.is_none() {
            let hidden = self.hidden_in_3d();
            return self
                .scene
                .iter()
                .map(|s| s.id)
                .filter(|id| !hidden.contains(id) && !self.hidden.contains(id))
                .collect();
        }
        let Some(view) = self.active_view else {
            return HashSet::new();
        };
        let mut out: HashSet<ElementId> = self
            .drawing(view)
            .map(|d| d.items.iter().map(|i| i.id()).collect())
            .unwrap_or_default();
        // the view's own annotation
        for e in self.doc.model().iter() {
            let on = match &e.kind {
                ElementKind::Tag(t) => t.view == view,
                ElementKind::Dimension(d) => d.view == view,
                ElementKind::Note(n) => n.view == view,
                _ => false,
            };
            if on {
                out.insert(e.id);
            }
        }
        out
    }

    /// The Select similar menu, for the Edit and right-click menus.
    pub(super) fn similar_items(&self) -> Vec<Item> {
        let ids = self.selection.all();
        let model = self.doc.model();
        let some = !ids.is_empty();
        let typed = ids
            .iter()
            .filter_map(|id| model.get(*id))
            .any(|e| type_key(model, e).is_some());
        let tags = ids
            .iter()
            .any(|id| matches!(model.get(*id).map(|e| &e.kind), Some(ElementKind::Tag(_))));
        let mut items = vec![
            Item::new("similar:type.view", "All of this type in the view")
                .key("Shift+A")
                .enabled(typed)
                .hint("Every element of the same type as the selection that this view shows"),
            Item::new("similar:type.project", "All of this type in the project")
                .key("Ctrl+Shift+A")
                .enabled(typed)
                .hint("Every element of the same type as the selection, on every level"),
            Item::new("similar:cat.view", "All of this category in the view")
                .enabled(some)
                .hint("Every wall, door, slab… (as the selection) this view shows"),
            Item::new("similar:cat.project", "All of this category in the project")
                .enabled(some)
                .hint("Every wall, door, slab… (as the selection) in the project"),
            Item::new("similar:tags", "Similar tags")
                .enabled(tags)
                .hint("Tags of the same kind (door tags, window tags…) in the same view"),
        ];
        // linework: as AutoCAD's SELECTSIMILAR, the same kind on the same
        // layer, in the view it is drawn in
        let drafting = ids.iter().any(|id| {
            matches!(
                model.get(*id).map(|e| &e.kind),
                Some(ElementKind::Detail(_))
            )
        });
        if drafting {
            items.insert(
                0,
                Item::new("similar:draft", "Similar linework (same kind and layer)")
                    .hint("Every line, polyline, circle, hatch… like the selected ones, on the same drafting layer, in this view"),
            );
            items.insert(
                1,
                Item::new("similar:draftlayer", "All linework on the same layer").hint(
                    "Every piece of linework on the selection's drafting layers, in this view",
                ),
            );
        }
        let only = self.keep_only_items();
        if !only.is_empty() {
            items.push(Item::Separator);
            items.push(Item::Header("Keep only".into()));
            items.extend(only);
        }
        vec![Item::sub("Select similar", items).icon("square-dashed-mouse-pointer")]
    }

    /// One item per kind of element selected, each keeping only that kind;
    /// none when there is only one kind.
    pub(super) fn keep_only_items(&self) -> Vec<Item> {
        let model = self.doc.model();
        let cats = categories_of(model, &self.selection.all());
        if cats.len() < 2 {
            return vec![];
        }
        cats.into_iter()
            .map(|(c, n)| {
                let i = Category::ALL.iter().position(|k| *k == c).unwrap_or(0);
                Item::new(format!("similar:only.{i}"), format!("{} ({n})", c.label()))
            })
            .collect()
    }

    /// Right after a box selection: say what it took and, when it took
    /// several kinds, offer Keep only… in the status bar.
    pub(super) fn after_box_selection(&mut self) {
        let ids = self.selection.all();
        let cats = categories_of(self.doc.model(), &ids);
        if cats.len() < 2 {
            self.boxed = None;
            return;
        }
        let kinds: Vec<String> = cats
            .iter()
            .take(4)
            .map(|(c, n)| format!("{n} {}", c.label().to_lowercase()))
            .collect();
        let more = if cats.len() > 4 { " and more" } else { "" };
        self.status = format!(
            "{} selected: {}{more}. Keep only… narrows it to one kind.",
            ids.len(),
            kinds.join(", ")
        );
        self.boxed = Some(ids);
    }

    /// Is the selection still what the last box took?
    pub(super) fn offers_keep_only(&self) -> bool {
        self.boxed
            .as_ref()
            .is_some_and(|b| *b == self.selection.all())
    }

    /// Run a Select similar action; false when `head` isn't one.
    pub(super) fn similar_action(&mut self, head: &str, arg: &str) -> bool {
        if head != "similar" {
            return false;
        }
        self.select_similar(arg);
        true
    }

    /// `what`: `type.view`, `type.project`, `cat.view`, `cat.project`,
    /// `tags`, or `only.<category index>`.
    pub(super) fn select_similar(&mut self, what: &str) {
        let ids = self.selection.all();
        if ids.is_empty() {
            self.status = "Select something first, then pick Select similar.".into();
            return;
        }
        if what == "draft" || what == "draftlayer" {
            let model = self.doc.model();
            let keys: Vec<(ElementId, Option<String>, String)> = ids
                .iter()
                .filter_map(|id| match &model.get(*id)?.kind {
                    ElementKind::Detail(d) => {
                        Some((d.view, d.layer.clone(), d.kind.label().to_string()))
                    }
                    _ => None,
                })
                .collect();
            let same: Vec<ElementId> = model
                .iter()
                .filter(|e| match &e.kind {
                    ElementKind::Detail(d) => keys.iter().any(|(v, l, k)| {
                        d.view == *v
                            && d.layer == *l
                            && (what == "draftlayer" || d.kind.label() == k)
                    }),
                    _ => false,
                })
                .map(|e| e.id)
                .collect();
            let n = same.len();
            self.selection.clear();
            self.selection.extend(same, false);
            self.status = format!("{n} pieces of linework selected.");
            return;
        }
        if let Some(i) = what.strip_prefix("only.") {
            let Some(c) = i.parse::<usize>().ok().and_then(|i| Category::ALL.get(i)) else {
                return;
            };
            let model = self.doc.model();
            let cut = cutters(model);
            let keep: Vec<ElementId> = ids
                .into_iter()
                .filter(|id| {
                    model
                        .get(*id)
                        .and_then(|e| Category::of(e, &cut))
                        .is_some_and(|k| k == *c)
                })
                .collect();
            let n = keep.len();
            self.selection.extend(keep, false);
            self.face = None;
            self.status = format!("Kept only the {}: {n} selected.", c.label().to_lowercase());
            return;
        }
        let (how, in_view) = match what {
            "type.view" => (Similar::Type, true),
            "type.project" => (Similar::Type, false),
            "cat.view" => (Similar::Category, true),
            "cat.project" => (Similar::Category, false),
            "tags" => (Similar::Tags, false),
            _ => return,
        };
        let within = in_view.then(|| self.ids_in_view());
        let model = self.doc.model();
        let mut found = similar(model, &ids, how, within.as_ref());
        // hidden elements stay out, and what was picked stays in
        found.retain(|id| !self.hidden.contains(id));
        let n = found.len();
        self.selection.extend(found, false);
        self.face = None;
        let what = match how {
            Similar::Type => "of the same type",
            Similar::Category => "of the same category",
            Similar::Tags => "similar tags",
        };
        let where_ = if in_view && how != Similar::Tags {
            " in the view"
        } else if how == Similar::Tags {
            ""
        } else {
            " in the project"
        };
        self.status = match n {
            0 => format!("Nothing {what}{where_}."),
            n => format!("{n} selected: everything {what}{where_}. Shift+click takes one away."),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> CreaBimApp {
        let mut app = CreaBimApp::blank(None);
        app.load_model(creabim_core::starter::starter_house());
        app
    }

    fn count(app: &CreaBimApp, f: impl Fn(&ElementKind) -> bool) -> usize {
        app.doc.model().iter().filter(|e| f(&e.kind)).count()
    }

    #[test]
    fn a_box_that_takes_several_kinds_offers_keep_only() {
        let mut app = app();
        let ids: Vec<ElementId> = app
            .doc
            .model()
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::Wall(_) | ElementKind::Opening(_)))
            .map(|e| e.id)
            .collect();
        app.selection.extend(ids, false);
        app.plan.boxed = true;
        let ctx = egui::Context::default();
        app.after_box_selection();
        assert!(app.status.contains("Keep only"), "{}", app.status);
        let offered = |app: &CreaBimApp| {
            app.status_state()["controls"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["key"] == "keeponly")
        };
        assert!(offered(&app));
        let items = app.menu_items("keeponly");
        let Some(Item::Action { id, .. }) = items.first() else {
            panic!("{items:?}")
        };
        let id = id.clone();
        app.run_action(&ctx, &id);
        assert!(!offered(&app));
        let kinds = categories_of(app.doc.model(), &app.selection.all());
        assert_eq!(kinds.len(), 1);
    }

    #[test]
    fn select_similar_takes_the_type_the_category_and_tags() {
        let mut app = app();
        let model = app.doc.model();
        let wall = model
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Wall(_)))
            .unwrap();
        let key = type_key(model, wall).unwrap();
        let same: usize = model
            .iter()
            .filter(|e| type_key(model, e).as_ref() == Some(&key))
            .count();
        let id = wall.id;
        app.selection.only(id);
        app.select_similar("type.project");
        assert_eq!(app.selection.len(), same);
        assert!(app
            .selection
            .all()
            .iter()
            .all(|i| matches!(app.doc.model().get(*i).unwrap().kind, ElementKind::Wall(_))));
        // every wall in the project
        app.selection.only(id);
        app.select_similar("cat.project");
        assert_eq!(
            app.selection.len(),
            count(&app, |k| matches!(k, ElementKind::Wall(_)))
        );
        // in the view: a plan shows fewer walls than the whole house has
        let plan = app
            .views()
            .into_iter()
            .find(|(v, _, _)| matches!(app.view(*v).map(|v| &v.kind), Some(ViewKind::Plan { .. })))
            .unwrap()
            .0;
        app.active_view = Some(plan);
        app.shell.context_3d = false;
        app.selection.only(id);
        app.select_similar("cat.view");
        let in_view = app.selection.len();
        assert!(in_view > 0 && in_view <= count(&app, |k| matches!(k, ElementKind::Wall(_))));
        // a mixed selection keeps only one category
        let door = app
            .doc
            .model()
            .iter()
            .find(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == creabim_core::OpeningKind::Door))
            .unwrap()
            .id;
        app.selection.only(id);
        app.selection.toggle(door);
        let items = app.similar_items();
        let Item::Sub { items, .. } = &items[0] else {
            panic!()
        };
        let only_walls = Category::ALL
            .iter()
            .position(|c| *c == Category::Walls)
            .unwrap();
        assert!(items.iter().any(
            |i| matches!(i, Item::Action { id, .. } if *id == format!("similar:only.{only_walls}"))
        ));
        app.select_similar(&format!("only.{only_walls}"));
        assert_eq!(app.selection.all(), vec![id]);
    }

    #[test]
    fn similar_tags_are_tags_of_the_same_kind_in_the_same_view() {
        let mut app = app();
        let model = app.doc.model();
        let Some(tag) = model.iter().find_map(|e| match &e.kind {
            ElementKind::Tag(t) => Some((e.id, t.clone())),
            _ => None,
        }) else {
            return;
        };
        let kind = host_category(model, tag.1.host);
        let want = model
            .iter()
            .filter(|e| {
                matches!(&e.kind, ElementKind::Tag(t)
                    if t.view == tag.1.view && host_category(model, t.host) == kind)
            })
            .count();
        app.selection.only(tag.0);
        app.select_similar("tags");
        assert_eq!(app.selection.len(), want);
    }
}
