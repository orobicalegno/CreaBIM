//! The Log view and Report a problem (MU40): what testers send when
//! something goes wrong, so it can be rebuilt and fixed.
//!
//! The Log is a view kind like Notes: the activity log as it is written,
//! problems in red, a filter, copy and save. Report a problem writes one
//! zip the tester attaches to an issue or an email: a summary (version,
//! build, system, what is open, the last few hundred lines), the whole
//! activity log, the settings and, only if they say yes, the project as
//! it is now. Nothing is sent anywhere by the app.

use super::*;

/// The Log view between frames.
#[derive(Clone, Debug, Default)]
pub struct LogUi {
    pub filter: String,
    pub problems_only: bool,
}

/// A zip of `files` (name, bytes), stored without compression: plain to
/// read in any unzip tool.
pub fn zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out: Vec<u8> = vec![];
    let mut central: Vec<u8> = vec![];
    let le16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    let le32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    for (name, data) in files {
        let crc = creabim_geom::raster::crc32(data);
        let offset = out.len() as u32;
        let n = name.as_bytes();
        // local header
        le32(&mut out, 0x0403_4b50);
        le16(&mut out, 20);
        le16(&mut out, 0x0800); // names in UTF-8
        le16(&mut out, 0); // stored
        le16(&mut out, 0);
        le16(&mut out, 0x21);
        le32(&mut out, crc);
        le32(&mut out, data.len() as u32);
        le32(&mut out, data.len() as u32);
        le16(&mut out, n.len() as u16);
        le16(&mut out, 0);
        out.extend_from_slice(n);
        out.extend_from_slice(data);
        // its central directory entry
        le32(&mut central, 0x0201_4b50);
        le16(&mut central, 20);
        le16(&mut central, 20);
        le16(&mut central, 0x0800);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0x21);
        le32(&mut central, crc);
        le32(&mut central, data.len() as u32);
        le32(&mut central, data.len() as u32);
        le16(&mut central, n.len() as u16);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le32(&mut central, 0);
        le32(&mut central, offset);
        central.extend_from_slice(n);
    }
    let at = out.len() as u32;
    out.extend_from_slice(&central);
    le32(&mut out, 0x0605_4b50);
    le16(&mut out, 0);
    le16(&mut out, 0);
    le16(&mut out, files.len() as u16);
    le16(&mut out, files.len() as u16);
    le32(&mut out, central.len() as u32);
    le32(&mut out, at);
    le16(&mut out, 0);
    out
}

static GPU: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Note the graphics card, once, at start.
pub fn set_gpu(s: String) {
    let _ = GPU.set(s);
}

/// The graphics card, as the app found it.
pub fn gpu() -> String {
    GPU.get().cloned().unwrap_or_else(|| "unknown".into())
}

/// The system and its version: "Windows 10.0.26200", a Linux
/// distribution's name, or macOS's, and the processor kind.
pub fn os_version() -> String {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        let arch = std::env::consts::ARCH;
        let run = |cmd: &str, args: &[&str]| {
            std::process::Command::new(cmd)
                .args(args)
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty())
        };
        let name = if cfg!(windows) {
            run("cmd", &["/C", "ver"]).unwrap_or_else(|| "Windows".into())
        } else if cfg!(target_os = "macos") {
            run("sw_vers", &["-productVersion"]).map_or("macOS".into(), |v| format!("macOS {v}"))
        } else {
            std::fs::read_to_string("/etc/os-release")
                .ok()
                .and_then(|t| {
                    t.lines()
                        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
                        .map(|v| v.trim_matches('"').to_string())
                })
                .unwrap_or_else(|| "Linux".into())
        };
        format!("{name}, {arch}")
    })
    .clone()
}

impl CreaBimApp {
    /// The Log view's header: the buttons used most (MU40); the filter
    /// and counts are in its Properties tab.
    pub(super) fn log_controls(&self) -> Vec<serde_json::Value> {
        use serde_json::json;
        vec![
            json!({"kind": "toggle", "id": "log.problems", "icon": "problem", "label": "Problems only",
                   "on": self.log_ui.problems_only, "hint": "Only crashes, errors and what could not be done"}),
            json!({"kind": "button", "id": "log.copy", "icon": "copy", "label": "Copy",
                   "hint": "Copy what is shown, to paste in a message"}),
            json!({"kind": "button", "id": "app.report", "icon": "file-text", "label": "Report a problem",
                   "hint": "Save one file to attach to a bug report: this log, your settings, your system and, if you like, the project"}),
            json!({"kind": "button", "id": "app.log", "icon": "folder-open", "label": "Open the folder",
                   "hint": "Where the whole activity log is kept"}),
        ]
    }

    /// The Log's header buttons. False for anything else.
    pub(super) fn log_action(&mut self, ctx: &egui::Context, head: &str) -> bool {
        match head {
            "log.problems" => self.log_ui.problems_only = !self.log_ui.problems_only,
            "log.copy" => {
                let shown = self.log_shown();
                ctx.copy_text(shown.join(
                    "
",
                ));
                self.status = format!("{} log lines copied.", shown.len());
            }
            _ => return false,
        }
        true
    }

    /// The lines the filter lets through.
    fn log_shown(&self) -> Vec<String> {
        let (lines, _) = crate::actlog::recent();
        let f = self.log_ui.filter.to_lowercase();
        lines
            .into_iter()
            .filter(|l| !self.log_ui.problems_only || crate::actlog::is_problem(l))
            .filter(|l| f.is_empty() || l.to_lowercase().contains(&f))
            .collect()
    }

    /// The Log tab of Properties: its filter and what is in it.
    pub(super) fn log_props(&mut self, ui: &mut egui::Ui) {
        if !self.prop_header(ui, "log_view", "Log", true, None) {
            return;
        }
        crate::props::row(ui, "Filter", |ui| {
            crate::props::fill(
                ui,
                egui::TextEdit::singleline(&mut self.log_ui.filter)
                    .vertical_align(egui::Align::Center)
                    .hint_text("words to find"),
            )
        });
        crate::props::check(ui, "Show", &mut self.log_ui.problems_only, "Problems only");
        let (lines, _) = crate::actlog::recent();
        let problems = lines
            .iter()
            .filter(|l| crate::actlog::is_problem(l))
            .count();
        crate::props::row(ui, "Lines", |ui| ui.label(format!("{}", lines.len())));
        crate::props::row(ui, "Problems", |ui| ui.label(format!("{problems}")));
        crate::props::row(ui, "System", |ui| ui.label(os_version()));
        crate::props::row(ui, "Graphics", |ui| ui.label(gpu()));
        crate::props::note(
            ui,
            "Report a problem saves this log, your settings and your system in one file; your project only if you say so. Nothing is sent anywhere.",
        );
    }

    /// The Log view: the activity log as it is written.
    pub(super) fn log_area(&mut self, ui: &mut egui::Ui, body: Rect) {
        let pal = self.shell.theme.palette();
        ui.painter().rect_filled(body, 0.0, pal.panel2);
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(body.shrink2(egui::vec2(12.0, 8.0)))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        let ui = &mut child;
        let shown = self.log_shown();
        let red = egui::Color32::from_rgb(214, 72, 72);
        egui::ScrollArea::both()
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if shown.is_empty() {
                    ui.label(egui::RichText::new("Nothing yet.").color(pal.muted));
                }
                for l in &shown {
                    let ink = if crate::actlog::is_problem(l) {
                        red
                    } else {
                        pal.text
                    };
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(l.as_str())
                                .monospace()
                                .size(11.5)
                                .color(ink),
                        )
                        .extend(),
                    );
                }
            });
    }

    /// Help › Report a problem: asks in a window of CreaBIM's own
    /// (MU40: not the system's box) whether to put the project in.
    pub(super) fn report_problem(&mut self) {
        self.report_ask = Some(false);
    }

    /// The Report a problem window, while it is asked for.
    pub(super) fn report_window(&mut self, ctx: &egui::Context) {
        let Some(mut with_project) = self.report_ask else {
            return;
        };
        let mut go = false;
        let mut close = false;
        let pal = self.shell.theme.palette();
        egui::Modal::new(egui::Id::new("report-problem")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading("Report a problem");
            ui.add_space(6.0);
            ui.label("Volumen saves one file to attach to a GitHub issue or an email: what it did lately, the steps that were slow, any crash, your settings, your system and graphics card. Nothing is sent anywhere.");
            ui.add_space(8.0);
            ui.checkbox(&mut with_project, "Include the project as it is now");
            ui.label(
                egui::RichText::new("It helps rebuild the problem exactly, but it is your work: leave it off if it is private.")
                    .small()
                    .color(pal.muted),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                go = ui.button("Save the report…").clicked();
                close = ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape));
            });
        });
        self.report_ask = if go || close {
            None
        } else {
            Some(with_project)
        };
        if !go {
            return;
        }
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let Some(to) = rfd::FileDialog::new()
            .set_title("Save the problem report")
            .add_filter("Zip", &["zip"])
            .set_file_name(format!("Volumen-report-{secs}.zip"))
            .save_file()
        else {
            return;
        };
        let bytes = zip(&self.report_files(with_project));
        self.status = match std::fs::write(&to, bytes) {
            Ok(()) => {
                let _ = crate::files::open_folder(&to);
                format!(
                    "Report saved: {}. Attach it to a GitHub issue or an email{}.",
                    to.display(),
                    if with_project {
                        "; it holds your project"
                    } else {
                        ""
                    }
                )
            }
            Err(e) => format!("Couldn't save the report: {e}"),
        };
    }

    /// The report's files: a summary, the activity log, the settings and,
    /// if asked, the project.
    fn report_files(&self, with_project: bool) -> Vec<(String, Vec<u8>)> {
        let model = self.doc.model();
        let (lines, _) = crate::actlog::recent();
        let tail: Vec<&String> = lines.iter().rev().take(400).collect();
        let problems: Vec<&String> = lines
            .iter()
            .filter(|l| crate::actlog::is_problem(l))
            .collect();
        let areas: Vec<String> = self
            .areas
            .root
            .areas()
            .iter()
            .map(|a| format!("{a:?}"))
            .collect();
        let mut summary = String::new();
        let mut put = |k: &str, v: String| summary.push_str(&format!("{k}: {v}\n"));
        put("Volumen", env!("CARGO_PKG_VERSION").to_string());
        put("Build", env!("CREABIM_COMMIT").to_string());
        put("System", os_version());
        put("Graphics", gpu());
        put("Theme", self.shell.theme.id().to_string());
        put("Tool", self.plan.tool.label().to_string());
        put("Elements", model.iter().count().to_string());
        put("Undo steps", self.doc.can_undo().to_string());
        put(
            "Last edit",
            self.doc.undo_label().unwrap_or("none").to_string(),
        );
        put("Areas", areas.join(", "));
        put("Status", self.status.clone());
        put("Project included", with_project.to_string());
        summary.push_str(
            "
--- Slow steps in this run (lag) ---
",
        );
        summary.push_str(&crate::actlog::slow_table());
        summary.push_str(
            "
--- Problems in this run ---
",
        );
        for p in &problems {
            summary.push_str(p);
            summary.push('\n');
        }
        summary.push_str("\n--- The last 400 lines, newest last ---\n");
        for l in tail.iter().rev() {
            summary.push_str(l);
            summary.push('\n');
        }
        let mut files = vec![("summary.txt".to_string(), summary.into_bytes())];
        // the end of the log: what led up to now, light enough to attach
        let tail = |b: Vec<u8>| {
            const KEEP: usize = 2 * 1024 * 1024;
            if b.len() > KEEP {
                b[b.len() - KEEP..].to_vec()
            } else {
                b
            }
        };
        if let Some(log) = crate::actlog::path().and_then(|p| std::fs::read(p).ok()) {
            files.push(("activity.log".into(), tail(log)));
        }
        if let Some(s) =
            crate::files::settings_dir().and_then(|d| std::fs::read(d.join("settings.json")).ok())
        {
            files.push(("settings.json".into(), s));
        }
        if with_project {
            // as it is now, unsaved changes and all
            files.push(("project.json".into(), model.to_json().into_bytes()));
        }
        files
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_zip_has_its_files_whole() {
        let files = vec![
            ("summary.txt".to_string(), b"Volumen: 1.1.0".to_vec()),
            ("activity.log".to_string(), vec![7u8; 1000]),
        ];
        let z = zip(&files);
        // its end record counts both, and each file sits whole after its
        // header, with its checksum
        let end = z.len() - 22;
        assert_eq!(&z[end..end + 4], &[0x50, 0x4b, 0x05, 0x06]);
        assert_eq!(u16::from_le_bytes([z[end + 10], z[end + 11]]), 2);
        let i = z
            .windows(11)
            .position(|w| w == b"summary.txt")
            .expect("its name");
        assert_eq!(&z[i + 11..i + 11 + 14], b"Volumen: 1.1.0");
        let crc = u32::from_le_bytes([z[14], z[15], z[16], z[17]]);
        assert_eq!(crc, creabim_geom::raster::crc32(b"Volumen: 1.1.0"));
    }
}
