//! The Help panel: the guide's pages ([`crate::help`]) in a web pane at the
//! right of the window, opened from the Help menu, with F1 or with the ? by
//! the tool in use. F1 opens the page about what you are doing: the tool in
//! use, the running Move, or the selected element. With the classic panels
//! the pages show in an egui window instead.

use super::*;
use crate::help;
use crate::ui_model::Item;
use serde_json::{json, Value};

/// A labelled drop-down, as every Properties row is: the label at the left
/// and the box filling the rest ([`crate::props::row`]).
pub(crate) fn combo(_ui: &mut egui::Ui, label: &str) -> crate::props::Combo {
    crate::props::Combo::new(label)
}

#[derive(Default)]
pub(super) struct HelpState {
    pub open: bool,
    /// The page shown, as a link: `id` or `id#heading`.
    pub page: String,
    /// The pages shown before, for Back.
    back: Vec<String>,
    /// What the pane was last sent for (page, theme), so a page's pictures
    /// are encoded only when it changes.
    sent: Option<(String, String)>,
    /// The classic window's search.
    query: String,
}

impl HelpState {
    /// Send everything again (the page reloaded).
    pub fn resend(&mut self) {
        self.sent = None;
    }
}

impl CreaBimApp {
    /// The Help menu.
    pub(super) fn help_menu(&self) -> Vec<Item> {
        vec![
            Item::new("help", "Help for what you are doing")
                .key("F1")
                .icon("circle-help")
                .hint("The page about the tool in use, or about what is selected"),
            Item::new("help:contents", "Contents").icon("book-open"),
            Item::Separator,
            Item::new("help:start", "Getting started")
                .icon("rocket")
                .hint("A first house, step by step: levels, walls, doors, floors, views, sheets"),
            Item::sub(
                "Samples to learn from",
                crate::app::appmenu::SAMPLES
                    .iter()
                    .map(|s| Item::new(s.id, s.title).icon(s.icon).hint(s.line))
                    .collect(),
            )
            .icon("book-open"),
            Item::new("help:interface", "The window at a glance").icon("layout-dashboard"),
            Item::new("help:shortcuts", "Keyboard shortcuts").icon("keyboard"),
            Item::new("help:glossary", "BIM words explained").icon("info"),
            Item::Separator,
            Item::new("app.report", "Report a problem…")
                .icon("file-text")
                .hint("Save one file to attach to a bug report: what Volumen did lately, your settings and, if you like, the project. Nothing is sent anywhere"),
            Item::new("area.show:log", "Show the log")
                .icon("log")
                .hint("The activity log in a view of its own, problems in red"),
            Item::Separator,
            Item::new("help:about", "About Volumen").icon("info"),
        ]
    }

    /// Every menu with shortcuts, as (where it is, its items), for the
    /// shortcuts page.
    pub(crate) fn help_menus(&self) -> Vec<(&'static str, Vec<Item>)> {
        vec![
            ("Volumen", self.app_menu()),
            ("File", self.menu_items("menu:File")),
            ("Edit", self.menu_items("menu:Edit")),
            ("View", self.menu_items("menu:View")),
            ("Create", self.menu_items("menu:Create")),
            ("Materials", self.menu_items("menu:Materials")),
            ("Help", self.help_menu()),
            ("Right-click", self.menu_items("context")),
            ("Status bar › Snaps", self.menu_items("snaps")),
        ]
    }

    /// The page about what is being done: a running move, the tool in use,
    /// the selected element, else the contents.
    pub(super) fn help_context(&self) -> String {
        if self.plan.modal.is_some() || self.plan.armed.is_some() {
            return "move".into();
        }
        if self.plan.tool != Tool::Select {
            return help::tool_page(self.plan.tool).into();
        }
        let model = self.doc.model();
        if let Some(e) = self.selection.active.and_then(|id| model.get(id)) {
            return help::element_page(&e.kind).into();
        }
        help::CONTENTS.into()
    }

    /// Show a page (a link: `id` or `id#heading`).
    pub(super) fn open_help(&mut self, link: &str) {
        let link = if help::exists(link) {
            link
        } else {
            help::CONTENTS
        };
        let h = &mut self.shell.help;
        if h.open && h.page != link && !h.page.is_empty() {
            h.back.push(std::mem::take(&mut h.page));
        }
        h.page = link.to_string();
        h.open = true;
    }

    /// The heading of a section of Properties: click it to fold the section
    /// shut or open it again, remembered between sessions; a ? beside it
    /// opens a few lines from `help`'s page. Returns whether it is open.
    pub(super) fn prop_header(
        &mut self,
        ui: &mut egui::Ui,
        key: &str,
        title: &str,
        default_open: bool,
        help: Option<&str>,
    ) -> bool {
        let open = self
            .shell
            .settings
            .props_open
            .get(key)
            .copied()
            .unwrap_or(default_open);
        // a sub-section (a key with a dot, "view.pens") is set in
        // (visibility and a view's range and template are sections of
        // their own, boxed, not sub-sections: MU22)
        let indent = key.contains('.')
            && !key.starts_with("visibility.")
            && !key.starts_with("view.")
            && crate::props::box_open(ui);
        let (resp, help_at) = crate::props::header(ui, title, open, indent);
        let resp = resp.on_hover_text(if open {
            "Click to fold this section"
        } else {
            "Click to open this section"
        });
        let mut toggle = resp.clicked();
        if let Some(link) = help.filter(|_| help_at.is_positive()) {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(help_at));
            if self.quick_help_button(&mut child, link, title) {
                toggle = false;
            }
        }
        if toggle {
            let s = &mut self.shell.settings;
            if !open == default_open {
                s.props_open.remove(key);
            } else {
                s.props_open.insert(key.to_string(), !open);
            }
            s.store();
        }
        open != toggle
    }

    /// A small ? beside a heading in Properties: it opens a few lines from
    /// the page `link` and a link to read the rest in the Help panel.
    /// Returns whether it was clicked.
    pub(super) fn quick_help_button(&mut self, ui: &mut egui::Ui, link: &str, title: &str) -> bool {
        // "say:…" is the text itself, not a page (MU33: the Editing hint
        // as a ? rather than a block in the panel)
        let page = !link.starts_with("say:");
        let lines = match link.strip_prefix("say:") {
            Some(t) => vec![t.to_string()],
            None => help::quick(link),
        };
        if lines.is_empty() {
            return false;
        }
        let r = ui
            .add(
                egui::Button::new(egui::RichText::new("?").size(11.0).strong())
                    .small()
                    .corner_radius(8.0)
                    .min_size(egui::vec2(16.0, 16.0)),
            )
            .on_hover_text(format!(
                "About {}: a few lines, and its page in the guide",
                title.to_lowercase()
            ));
        let mut read = false;
        egui::Popup::from_toggle_button_response(&r)
            // opening leftward, under the ?: the Help panel to the right
            // is drawn over anything that runs into it (MU39)
            .align(egui::RectAlign::BOTTOM_END)
            .align_alternatives(&[egui::RectAlign::TOP_END, egui::RectAlign::LEFT_START])
            // above the views' zoom columns, which sit on the foreground
            // layer (MU22)
            .kind(egui::PopupKind::Tooltip)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_max_width(300.0);
                ui.label(egui::RichText::new(title).strong());
                ui.add_space(2.0);
                for l in &lines {
                    ui.label(help::plain(l));
                }
                ui.add_space(4.0);
                if page && ui.link("Read more in the guide →").clicked() {
                    read = true;
                    ui.close();
                }
            });
        if read {
            self.open_help(link);
        }
        r.clicked()
    }

    /// The Help menu's actions and the Help panel's buttons. Returns false
    /// for any other action.
    pub(super) fn help_action(&mut self, head: &str, arg: &str) -> bool {
        match head {
            // F1: the page about what you are doing, or close it again
            "help" if arg.is_empty() => {
                let here = self.help_context();
                if self.shell.help.open && self.shell.help.page == here {
                    self.shell.help.open = false;
                } else {
                    self.open_help(&here);
                }
            }
            "help" => self.open_help(arg),
            "help.close" => self.shell.help.open = false,
            "help.back" => {
                if let Some(p) = self.shell.help.back.pop() {
                    self.shell.help.page = p;
                }
            }
            _ => return false,
        }
        true
    }

    /// What the Help pane shows, when it changed since it was last sent.
    pub(super) fn help_state(&mut self) -> Option<Value> {
        let h = &self.shell.help;
        let key = (h.page.clone(), self.shell.theme.id().to_string());
        if h.sent.as_ref() == Some(&key) {
            return None;
        }
        let (id, anchor) = h.page.split_once('#').unwrap_or((&h.page, ""));
        // the pictures of this page only; the text of every page is in
        // the pane from the start
        let mut images = serde_json::Map::new();
        if let Some(md) = help::text(id) {
            for path in help::targets(md).1 {
                if let Some(url) = help::data_url_for(path, self.shell.theme.palette().dark) {
                    images.insert(path.to_string(), json!(url));
                }
            }
        }
        let state = json!({
            "theme": key.1,
            "page": id,
            "anchor": anchor,
            "back": !h.back.is_empty(),
            "images": images,
            // made from the menus, so it is always as they are
            "generated": {"shortcuts": help::shortcuts_page(&self.help_menus())},
        });
        self.shell.help.sent = Some(key);
        Some(state)
    }

    /// The Help panel with the classic panels: the page list and the page,
    /// drawn by egui.
    pub(super) fn help_window(&mut self, ctx: &egui::Context) {
        if !self.shell.help.open || (self.shell.web.is_some() && !self.shell.classic) {
            return;
        }
        let mut open = true;
        let mut go = None;
        let page = self.shell.help.page.clone();
        let (id, _) = page.split_once('#').unwrap_or((&page, ""));
        let shortcuts = (id == "shortcuts").then(|| help::shortcuts_page(&self.help_menus()));
        crate::app::shell::window("Help")
            .open(&mut open)
            .default_size([640.0, 520.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Search:");
                    ui.text_edit_singleline(&mut self.shell.help.query);
                });
                ui.separator();
                let q = self.shell.help.query.to_lowercase();
                ui.columns(2, |cols| {
                    egui::ScrollArea::vertical()
                        .id_salt("help_pages")
                        .show(&mut cols[0], |ui| {
                            let mut section = "";
                            for (s, pid, md) in help::PAGES {
                                if !q.is_empty() && !md.to_lowercase().contains(&q) {
                                    continue;
                                }
                                if *s != section {
                                    section = s;
                                    ui.label(egui::RichText::new(*s).weak());
                                }
                                if ui.selectable_label(*pid == id, help::title(md)).clicked() {
                                    go = Some(pid.to_string());
                                }
                            }
                        });
                    egui::ScrollArea::vertical()
                        .id_salt("help_page")
                        .show(&mut cols[1], |ui| {
                            let md = shortcuts
                                .clone()
                                .or_else(|| help::text(id).map(help::shown))
                                .unwrap_or_else(|| {
                                    "# Help\n\nPick a page on the left, or search.".into()
                                });
                            classic_page(ui, &md);
                        });
                });
            });
        if let Some(p) = go {
            self.open_help(&p);
        }
        if !open {
            self.shell.help.open = false;
        }
    }
}

/// A page drawn plainly: headings, list items and paragraphs, without the
/// Markdown marks.
fn classic_page(ui: &mut egui::Ui, md: &str) {
    let plain = |s: &str| {
        let mut out = String::new();
        let mut rest = s;
        // [text](link) reads as its text
        while let Some(i) = rest.find('[') {
            out.push_str(&rest[..i]);
            let after = &rest[i + 1..];
            match (after.find("]("), after.find(')')) {
                (Some(a), Some(b)) if a < b => {
                    out.push_str(&after[..a]);
                    rest = &after[b + 1..];
                }
                _ => {
                    out.push('[');
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        out.replace(['*', '`'], "")
    };
    for line in md.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with("<!--") || l.starts_with("![") || l.starts_with("| ---") {
            continue;
        }
        if let Some(h) = l.strip_prefix("# ") {
            ui.heading(plain(h));
        } else if let Some(h) = l.strip_prefix("## ").or_else(|| l.strip_prefix("### ")) {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(plain(h)).strong());
        } else if let Some(t) = l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")) {
            ui.label(format!("•  {}", plain(t)));
        } else if l.starts_with('|') {
            let cells: Vec<String> = l
                .trim_matches('|')
                .split('|')
                .map(|c| plain(c.trim()))
                .collect();
            ui.label(cells.join("  —  "));
        } else {
            ui.label(plain(l.trim_start_matches("> ")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f1_opens_the_page_about_the_tool_in_use_and_closes_it_again() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        app.plan.tool = Tool::Wall;
        app.run_action(&ctx, "help");
        assert!(app.shell.help.open);
        assert_eq!(app.shell.help.page, "walls");
        app.run_action(&ctx, "help");
        assert!(!app.shell.help.open, "F1 again closes it");
        // a bare opening's page is a part of the holes page
        app.plan.tool = Tool::Opening;
        app.run_action(&ctx, "help");
        assert_eq!(app.shell.help.page, "holes#wall-openings");
        // with the Select tool, the page about what is selected
        app.plan.tool = Tool::Select;
        let wall = app
            .doc
            .model()
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Wall(_)))
            .unwrap()
            .id;
        app.selection.only(wall);
        app.run_action(&ctx, "help");
        assert_eq!(app.shell.help.page, "walls");
        app.selection.clear();
        app.shell.help.open = false;
        app.run_action(&ctx, "help");
        assert_eq!(app.shell.help.page, help::CONTENTS);
    }

    #[test]
    fn back_returns_to_the_page_before() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        app.run_action(&ctx, "help:start");
        app.run_action(&ctx, "help:walls#location-line");
        assert_eq!(app.shell.help.page, "walls#location-line");
        app.run_action(&ctx, "help.back");
        assert_eq!(app.shell.help.page, "start");
        // a link to no page shows the contents rather than nothing
        app.run_action(&ctx, "help:no-such-page");
        assert_eq!(app.shell.help.page, help::CONTENTS);
        app.run_action(&ctx, "help.close");
        assert!(!app.shell.help.open);
    }

    /// Every action that opens a page, on any menu, the palette or a tool's
    /// ?, goes to a page that exists.
    #[test]
    fn every_help_link_goes_to_a_page() {
        let app = test_app();
        fn links(items: &[Item], out: &mut Vec<String>) {
            for i in items {
                match i {
                    Item::Action { id, .. } => {
                        if let Some(l) = id.strip_prefix("help:") {
                            out.push(l.to_string());
                        }
                    }
                    Item::Sub { items, .. } => links(items, out),
                    _ => {}
                }
            }
        }
        let mut all = vec![];
        for (_, items) in app.help_menus() {
            links(&items, &mut all);
        }
        links(&app.menu_items("palette"), &mut all);
        assert!(
            all.len() > help::PAGES.len(),
            "the palette lists every page"
        );
        for t in Tool::ALL {
            all.push(help::tool_page(t).to_string());
        }
        for e in app.doc.model().iter() {
            all.push(help::element_page(&e.kind).to_string());
        }
        for l in all {
            assert!(help::exists(&l), "{l} isn't a page or a heading on one");
        }
    }

    #[test]
    fn the_pane_gets_a_pages_pictures_and_only_when_it_changes() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        app.run_action(&ctx, "help:interface");
        let s = app.help_state().unwrap();
        assert_eq!(s["page"], "interface");
        let img = s["images"]["img/interface.jpg"].as_str().unwrap();
        assert!(img.starts_with("data:image/jpeg;base64,"));
        assert!(s["generated"]["shortcuts"]
            .as_str()
            .unwrap()
            .contains("| `Ctrl+K` |"));
        assert!(app.help_state().is_none(), "nothing new to send");
        app.run_action(&ctx, "help:walls#joins");
        let s = app.help_state().unwrap();
        assert_eq!(
            (s["page"].as_str(), s["anchor"].as_str()),
            (Some("walls"), Some("joins"))
        );
        assert_eq!(s["back"], true);
    }

    #[test]
    fn the_tool_strip_offers_help_for_the_tool() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        app.plan.tool = Tool::Slab;
        let (head, _) = app.tool_strip(&ctx);
        assert_eq!(head["help"], "slabs");
    }
}
