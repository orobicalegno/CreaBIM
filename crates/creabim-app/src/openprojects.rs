//! More than one project open at once (Volumen round 2, phase E): the one
//! shown is the app's own document; the others wait here with their
//! selection, views, camera and save state, and the Window menu brings one
//! back. Settings (snaps, tools, the panels) stay the app's, shared by all.

use super::*;

/// A project open but not shown.
pub(super) struct OpenProject {
    doc: Document,
    selection: Selection,
    active_view: Option<ElementId>,
    sheet: SheetState,
    sheets: HashMap<usize, SheetState>,
    camera: OrbitCamera,
    path: String,
    saved_revision: Option<u64>,
    saved_mark: (usize, Option<creabim_core::document::StepId>),
    watch: crate::files::Watch,
    hidden: std::collections::HashSet<ElementId>,
    linked_camera: Option<(ElementId, [f64; 7])>,
    ucs: crate::planview::Ucs,
}

/// A project's name for the Window menu: its file name, without folders.
pub(super) fn project_name(path: &str) -> String {
    let name = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path);
    if name.is_empty() {
        "Untitled".into()
    } else {
        name.to_string()
    }
}

impl CreaBimApp {
    /// Before the shown project changes (another one shown, opened, made or
    /// closed): what is not confirmed ends here, in this project. An edit
    /// following the pointer or a drag takes its preview back; points
    /// clicked, a chain, a command's picks, a move armed and Edit mode
    /// stop. Everything confirmed stays as it is, and nothing of this goes
    /// on in the next project (V-10, V-11).
    pub(super) fn end_unconfirmed(&mut self) {
        if let Some(mut m) = self.plan.modal.take() {
            m.discard(&mut self.doc);
        }
        if let Some(mut d) = self.plan.grip.take() {
            d.cancel(&mut self.doc);
        }
        if let Some(mut d) = self.handle_drag.take() {
            d.cancel(&mut self.doc);
        }
        self.plan.cancel();
        self.plan.wall_arc_end = None;
        self.plan.cmd_pick = None;
        self.plan.armed = None;
        self.plan.armed_follow = false;
        self.plan.armed_grip = None;
        self.plan.in_place = false;
        self.plan.edit = None;
        self.plan.grips.clear();
        self.pending_3d = None;
        self.cmd.running = None;
        // a style option previewed on the drawings
        self.end_preview();
    }

    /// Take the shown project out, leaving the app's fields as they were
    /// (the caller loads or brings in another one at once).
    fn take_current(&mut self) -> OpenProject {
        self.end_unconfirmed();
        OpenProject {
            doc: std::mem::take(&mut self.doc),
            selection: std::mem::take(&mut self.selection),
            active_view: self.active_view.take(),
            sheet: std::mem::take(&mut self.sheet),
            sheets: std::mem::take(&mut self.sheets),
            camera: self.camera.clone(),
            path: std::mem::take(&mut self.path),
            saved_revision: self.saved_revision.take(),
            saved_mark: std::mem::take(&mut self.saved_mark),
            watch: std::mem::take(&mut self.watch),
            hidden: std::mem::take(&mut self.hidden),
            linked_camera: self.linked_camera.take(),
            ucs: self.plan.ucs,
        }
    }

    /// Show `p`, dropping what was drawn for the project shown before.
    fn bring_in(&mut self, p: OpenProject) {
        self.doc = p.doc;
        self.selection = p.selection;
        self.active_view = p.active_view;
        self.sheet = p.sheet;
        self.sheets = p.sheets;
        self.camera = p.camera;
        self.path = p.path;
        self.saved_revision = p.saved_revision;
        self.saved_mark = p.saved_mark;
        self.watch = p.watch;
        self.hidden = p.hidden;
        self.linked_camera = p.linked_camera;
        self.plan.ucs = p.ucs;
        self.forget_drawn();
    }

    /// Caches built from the project shown before: drawings, looks, picks.
    pub(super) fn forget_drawn(&mut self) {
        self.hidden_rev += 1;
        self.face = None;
        self.crop_pick = None;
        self.drawings.clear();
        self.looks.clear();
        self.xray_drawings.clear();
        self.plan.xray.clear();
        self.plan.hover = None;
        self.geometry_key = None;
        self.selection_key = None;
        self.snaps_3d = None;
        self.workplane = None;
        self.scene.clear();
        self.drop_stale_selection();
    }

    /// Before another project is opened or made: keep this one open in the
    /// Window menu, unless it is the untouched project the app started with.
    pub(super) fn keep_open(&mut self) {
        if !self.multi || (self.path.is_empty() && self.doc.undo_steps().is_empty()) {
            return;
        }
        let p = self.take_current();
        self.others.push(p);
    }

    /// The open projects, the shown one first, as (name, unsaved).
    pub(super) fn open_projects(&self) -> Vec<(String, bool)> {
        let mut v = vec![(project_name(&self.path), self.unsaved())];
        for p in &self.others {
            v.push((
                project_name(&p.path),
                p.saved_revision != Some(p.doc.model().revision()),
            ));
        }
        v
    }

    /// Show the `i`th of the other open projects (as [`Self::open_projects`]
    /// numbers them, from 1).
    pub(super) fn switch_project(&mut self, i: usize) {
        if i == 0 || i > self.others.len() {
            return;
        }
        let next = self.others.remove(i - 1);
        let shown = self.take_current();
        self.others.insert(i - 1, shown);
        self.bring_in(next);
        self.status = format!("Window: {}", project_name(&self.path));
    }

    /// Where `path` waits among the open projects, numbered as
    /// [`Self::switch_project`] takes them.
    pub(super) fn open_index(&self, path: &str) -> Option<usize> {
        self.others
            .iter()
            .position(|p| !p.path.is_empty() && p.path == path)
            .map(|i| i + 1)
    }

    /// Next open project, round (Ctrl+Tab).
    pub(super) fn next_project(&mut self) {
        if self.others.is_empty() {
            return;
        }
        // the shown one goes to the end, the first waiting comes in
        let next = self.others.remove(0);
        let shown = self.take_current();
        self.others.push(shown);
        self.bring_in(next);
        self.status = format!("Window: {}", project_name(&self.path));
    }

    /// Close the shown project and show the next open one. False when it is
    /// the only one: closing it then is closing the app.
    pub(super) fn close_project(&mut self) -> bool {
        if self.others.is_empty() {
            return false;
        }
        let was = project_name(&self.path);
        self.end_unconfirmed();
        let next = self.others.remove(0);
        self.bring_in(next);
        self.status = format!("Closed {was}. Window: {}", project_name(&self.path));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_another_keeps_the_first_in_the_window_menu() {
        let mut app = CreaBimApp::blank(None);
        app.load_model(creabim_core::sample::sample_house());
        app.multi = true;
        app.path = "a.creabim".into();
        app.mark_saved();
        app.load_model(creabim_core::starter::starter_house());
        app.path = "b.creabim".into();
        let names: Vec<String> = app.open_projects().into_iter().map(|p| p.0).collect();
        assert_eq!(names, ["b", "a"]);
        app.switch_project(1);
        assert_eq!(app.path, "a.creabim");
        assert!(!app.unsaved());
        assert_eq!(app.open_index("b.creabim"), Some(1));
        assert!(app.close_project());
        assert_eq!(app.path, "b.creabim");
        assert!(!app.close_project());
    }
}
