//! Edit mode in the app: opening the selected element for editing and
//! closing it again (see [`crate::editmode`] for its grips).

use super::*;

impl CreaBimApp {
    /// Open the selected element in Edit mode, or close it.
    pub(super) fn toggle_edit_mode(&mut self) {
        if self.plan.edit.take().is_some() {
            self.status = "Finished editing.".into();
            return;
        }
        let Some(id) = self.selection.active.filter(|_| self.selection.len() == 1) else {
            self.status = "Select one element to edit it.".into();
            return;
        };
        if self.plan.tool != Tool::Select {
            self.plan.tool = Tool::Select;
        }
        self.status = editmode::enter(self.doc.model(), id, &mut self.plan.edit);
    }

    /// Edit mode ends when its element is no longer the one selected, is
    /// gone, or a drawing tool is picked.
    pub(super) fn keep_edit_mode(&mut self) {
        match self.plan.grips.request.take() {
            Some(handles::GripRequest::Finish) => {
                self.plan.edit = None;
                self.status = "Finished editing.".into();
            }
            Some(handles::GripRequest::Enter) => {
                if let Some(id) = self.selection.active.filter(|_| self.selection.len() == 1) {
                    self.status = editmode::enter(self.doc.model(), id, &mut self.plan.edit);
                }
            }
            Some(handles::GripRequest::OffsetEdge(ring, i)) => {
                self.plan.armed = Some((crate::modal::Mode::Grip, false));
                self.plan.armed_grip = Some(handles::HandleKind::Edge { ring, i });
                self.status = "Offset edge: move the pointer to push the edge out or pull it in, or type a distance.".into();
            }
            Some(handles::GripRequest::OffsetAll) => {
                self.plan.armed = Some((crate::modal::Mode::Offset, false));
                self.plan.in_place = true;
                self.status = "Offset all: move the pointer out or in, or type a distance.".into();
            }
            Some(handles::GripRequest::InsertPoint(ring, i, at)) => {
                if let Some(id) = self.selection.active {
                    match editmode::add_corner(&mut self.doc, id, ring, i, Some(at)) {
                        Ok(_) => {
                            self.plan.armed = Some((crate::modal::Mode::Grip, false));
                            self.plan.armed_grip =
                                Some(handles::HandleKind::Vertex { ring, i: i + 1 });
                            self.status = "Point inserted: move it with the pointer and click, or type a distance. Esc leaves it where it is.".into();
                        }
                        Err(e) => self.status = e,
                    }
                }
            }
            None => {}
        }
        let Some(id) = self.plan.edit else {
            self.plan.grips.clear();
            handles::set_layers(false);
            return;
        };
        // as Blender's: a click elsewhere doesn't end Edit mode, the
        // element stays the one edited until Tab, Esc or Enter (MU39)
        if self.doc.model().get(id).is_some()
            && self.plan.tool == Tool::Select
            && (self.selection.active != Some(id) || self.selection.len() != 1)
        {
            self.selection.only(id);
        }
        if self.plan.tool != Tool::Select || self.doc.model().get(id).is_none() {
            self.plan.edit = None;
            self.plan.grips.clear();
            handles::set_layers(false);
        }
    }

    /// Delete or Backspace in Edit mode: remove the picked corners or
    /// holes, never the element itself.
    pub(super) fn delete_grips(&mut self) {
        let Some(id) = self.plan.edit else { return };
        self.status = match handles::delete_picked(&mut self.doc, id, &mut self.plan.grips) {
            Ok(s) | Err(s) => s,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> CreaBimApp {
        let mut app = CreaBimApp::blank(None);
        app.load_model(creabim_core::sample::typed_house());
        app
    }

    /// Rename the first wall: one undo step.
    fn rename_a_wall(app: &mut CreaBimApp, name: &str) {
        let id = app
            .doc
            .model()
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Wall(_)))
            .unwrap()
            .id;
        app.doc
            .edit(id, "Rename", |e| e.name = name.into())
            .unwrap();
    }

    fn wall_names(app: &CreaBimApp) -> Vec<String> {
        app.doc
            .model()
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
            .map(|e| e.name.clone())
            .collect()
    }

    #[test]
    fn cancel_in_a_window_never_takes_back_work_done_elsewhere() {
        let mut app = app();
        app.dialog_begin("asm");
        // the window's own change, then work elsewhere while it stays open
        app.in_dialog("asm", |s| rename_a_wall(s, "In the window"));
        rename_a_wall(&mut app, "Drawn meanwhile");
        let steps = app.doc.undo_steps().len();
        app.dialog_end("asm", false);
        // the work done elsewhere stays, and so (under it) the window's
        assert_eq!(app.doc.undo_steps().len(), steps);
        assert!(wall_names(&app).contains(&"Drawn meanwhile".to_string()));
        // a window's change on top is taken back, and only that
        let before = app.doc.undo_steps().len();
        app.dialog_begin("asm");
        app.in_dialog("asm", |s| rename_a_wall(s, "Second"));
        app.dialog_end("asm", false);
        assert_eq!(app.doc.undo_steps().len(), before);
        assert!(wall_names(&app).contains(&"Drawn meanwhile".to_string()));
    }

    #[test]
    fn undo_never_repeats_on_enter_or_a_quick_right_click() {
        let mut app = app();
        let ctx = egui::Context::default();
        rename_a_wall(&mut app, "One");
        rename_a_wall(&mut app, "Two");
        rename_a_wall(&mut app, "Three");
        app.cmd_start(&ctx, "UNDO");
        assert!(wall_names(&app).contains(&"Two".to_string()));
        // Enter on an empty line (and a quick right-click, which repeats
        // the same last command): no more undoing
        assert!(app.cmd.last.as_deref() != Some("UNDO"));
        app.cmd_enter(&ctx, "");
        assert!(wall_names(&app).contains(&"Two".to_string()));
    }

    fn shaft(app: &CreaBimApp) -> ElementId {
        app.doc
            .model()
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Shaft(_)))
            .unwrap()
            .id
    }

    #[test]
    fn a_windows_sizes_set_before_placing_reach_the_window() {
        let mut app = app();
        app.plan.tool = Tool::Window;
        let ctx = egui::Context::default();
        let (_, options) = app.tool_strip(&ctx);
        let keys: Vec<&str> = options.iter().filter_map(|o| o["key"].as_str()).collect();
        for k in [
            "member.window_width",
            "member.window_height",
            "member.window_sill",
        ] {
            assert!(keys.contains(&k), "{k} in {keys:?}");
        }
        app.set_tool_number("member.window_width", 0.6);
        app.set_tool_number("member.window_sill", 1.5);
        assert_eq!(app.plan.members.window, [0.6, 1.3, 1.5]);
    }

    #[test]
    fn edit_mode_opens_on_the_selection_and_stays_until_left() {
        let mut app = app();
        // nothing selected: nothing to edit
        app.toggle_edit_mode();
        assert_eq!(app.plan.edit, None);
        let id = shaft(&app);
        app.selection.only(id);
        app.toggle_edit_mode();
        assert_eq!(app.plan.edit, Some(id));
        assert!(app.status.contains("Editing"), "{}", app.status);
        // Esc doesn't leave it, as Blender's (MU39); Tab does, keeping the
        // selection
        assert!(app.plan.step_back());
        assert_eq!(app.plan.edit, Some(id));
        app.toggle_edit_mode();
        assert_eq!(app.plan.edit, None);
        assert!(app.selection.contains(id));
        // and a click elsewhere doesn't end it
        app.toggle_edit_mode();
        let slab = app
            .doc
            .model()
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Slab(_)))
            .unwrap()
            .id;
        app.selection.only(slab);
        app.keep_edit_mode();
        assert_eq!(app.plan.edit, Some(id));
        assert_eq!(app.selection.active, Some(id));
        app.selection.clear();
        app.keep_edit_mode();
        assert_eq!(app.plan.edit, Some(id));
    }

    #[test]
    fn a_shaft_shows_and_is_picked_on_every_plan_it_reaches() {
        let app = app();
        let m = app.doc.model();
        let id = shaft(&app);
        let ElementKind::Shaft(s) = &m.get(id).unwrap().kind else {
            unreachable!()
        };
        let (z0, z1) = s.span(m);
        let mut seen = 0;
        for (l, lv) in m.levels() {
            let plan = creabim_geom::plan_view(m, l.id, 1.2);
            let drawn = plan.items.iter().any(|it| it.id() == id);
            let reaches = lv.elevation >= z0 - 1e-6 && lv.elevation <= z1 + 1e-6;
            assert_eq!(drawn, reaches, "{}", l.name);
            if drawn {
                seen += 1;
                // a click inside it takes the shaft, not the floor round it
                let c = s.boundary.iter().copied().sum::<glam::DVec2>() / s.boundary.len() as f64;
                assert_eq!(planview::hit_test(&plan.items, c, 0.05, 0.01), Some(id));
            }
        }
        // its own level and the one its top reaches
        assert!(seen >= 2, "{seen}");
    }

    #[test]
    fn the_3d_view_and_views_change_how_they_draw() {
        let mut app = app();
        let ctx = egui::Context::default();
        app.run_action(&ctx, "render3d:3");
        assert_eq!(app.shell.settings.render_3d, RenderMode::Wireframe);
        let items = app.menu_items("render3d");
        assert!(items.iter().any(|i| matches!(i,
            crate::ui_model::Item::Action { id, checked: Some(true), .. } if id == "render3d:3")));
        // a section draws in wireframe, on its sheets too
        let section = app
            .doc
            .model()
            .iter()
            .find(|e| matches!(&e.kind, ElementKind::View(v) if matches!(v.kind, ViewKind::Section { .. })))
            .unwrap()
            .id;
        app.active_view = Some(section);
        app.run_action(&ctx, "viewstyle:2");
        assert_eq!(app.view(section).unwrap().style, ViewStyle::Wireframe);
        // and shows the shaft passing through it, dashed, to click
        let d = creabim_geom::section::view_drawing(app.doc.model(), section).unwrap();
        let sh = shaft(&app);
        assert!(d.items.iter().any(|it| it.id() == sh));
    }

    fn slab(app: &CreaBimApp) -> (ElementId, usize) {
        app.doc
            .model()
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Slab(s) => Some((e.id, s.boundary.len())),
                _ => None,
            })
            .unwrap()
    }

    fn corners(app: &CreaBimApp, id: ElementId) -> usize {
        match &app.doc.model().get(id).unwrap().kind {
            ElementKind::Slab(s) => s.boundary.len(),
            _ => panic!(),
        }
    }

    #[test]
    fn delete_in_edit_mode_removes_the_picked_corner_never_the_element() {
        let mut app = app();
        let ctx = egui::Context::default();
        let (id, n) = slab(&app);
        app.selection.only(id);
        app.toggle_edit_mode();
        assert_eq!(app.plan.edit, Some(id));
        app.keep_edit_mode();
        // nothing picked: Delete says what to do and deletes nothing
        app.run_command(&ctx, Command::Delete, None);
        assert!(app.doc.model().get(id).is_some());
        assert!(app.status.contains("Select a corner"), "{}", app.status);
        assert_eq!(corners(&app, id), n);
        // a corner picked (twice, the second in the hole of one more): one
        // step removes them both
        app.plan.grips.id = Some(id);
        app.plan.grips.pick(handles::Ring::Outline, 0, false);
        app.plan.grips.pick(handles::Ring::Outline, 1, true);
        app.doc
            .edit(id, "more corners", |e| {
                if let ElementKind::Slab(s) = &mut e.kind {
                    s.split_edge(0);
                    s.split_edge(2);
                }
            })
            .unwrap();
        let n = corners(&app, id);
        app.run_command(&ctx, Command::Delete, None);
        assert!(app.doc.model().get(id).is_some(), "the slab stays");
        assert_eq!(corners(&app, id), n - 2);
        assert_eq!(
            app.doc.undo_label(),
            Some(
                format!(
                    "Remove 2 corners of {}",
                    app.doc.model().get(id).unwrap().name
                )
                .as_str()
            )
        );
        assert!(app.plan.grips.is_empty());
        // and still in Edit mode
        assert_eq!(app.plan.edit, Some(id));
        // Esc drops picks, and stays in Edit mode; Tab finishes
        app.plan.grips.pick(handles::Ring::Outline, 0, false);
        assert!(app.plan.step_back());
        assert_eq!(app.plan.edit, Some(id));
        assert!(app.plan.step_back());
        assert_eq!(app.plan.edit, Some(id));
        app.toggle_edit_mode();
        assert_eq!(app.plan.edit, None);
        // out of Edit mode Delete deletes the element, as it always did
        app.run_command(&ctx, Command::Delete, None);
        assert!(app.doc.model().get(id).is_none());
    }

    #[test]
    fn the_grips_menu_can_finish_editing() {
        let mut app = app();
        let (id, _) = slab(&app);
        app.selection.only(id);
        app.toggle_edit_mode();
        app.plan.grips.request = Some(handles::GripRequest::Finish);
        app.keep_edit_mode();
        assert_eq!(app.plan.edit, None);
        app.plan.grips.request = Some(handles::GripRequest::Enter);
        app.keep_edit_mode();
        assert_eq!(app.plan.edit, Some(id));
    }
}
