//! The tools row's tabs (brief 02, 2.1): Model, Annotate, View and Manage
//! each show their own tools and commands, icons only, while Select and
//! Align stay at the left on every tab. A tool picked by its key shows its
//! tab. A tool can be on two tabs where it belongs to both: the callout.

use crate::planview::Tool;

/// One button of a tab.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Entry {
    Tool(Tool),
    /// A command of the menus, by its action id.
    Act {
        id: &'static str,
        label: &'static str,
        icon: &'static str,
        hint: &'static str,
    },
    /// A short menu of commands (see [`menu`]).
    Menu {
        key: &'static str,
        label: &'static str,
        icon: &'static str,
        hint: &'static str,
    },
    /// A divider between groups.
    Sep,
    /// The current drafting layer, to pick another (AutoCAD's Layers panel).
    LayerCombo,
}

pub const TABS: [&str; 5] = ["Home", "Model", "Annotate", "View", "Manage"];

/// On every tab, before it.
pub const ALWAYS: [Tool; 2] = [Tool::Select, Tool::Align];

/// What tab `tab` holds.
pub fn entries(tab: usize) -> Vec<Entry> {
    use Entry::{Sep, Tool as T};
    let act = |id, label, icon, hint| Entry::Act {
        id,
        label,
        icon,
        hint,
    };
    match tab {
        // Home, as AutoCAD's: the Layers panel, drawing and editing
        0 => vec![
            Entry::LayerCombo,
            act("layers.manager", "Layer properties", "layers", "LAYER: the drafting layers as a tree, with sub-layers, colours, pens, line types, locks"),
            act("layer.fromsel", "Make the object's layer current", "check", "Make the layer of the selected linework the current one (LAYMCUR)"),
            act("layer.tocurrent", "Change to current layer", "layers-2", "Put the selected linework on the current layer (LAYCUR)"),
            act("layer.off", "Layer off", "eye-off", "Hide the layer of the selected linework (LAYOFF)"),
            act("layer.lock", "Lock layer", "square-dashed", "Lock the layer of the selected linework (LAYLCK): it shows and snaps, it can't be picked"),
            act("layer.allon", "All layers on", "eye", "Show every drafting layer again (LAYON) and unlock them all"),
            Sep,
            act("cmdline:LINE", "Line (L)", "slash", "LINE: lines through points"),
            act("cmdline:PLINE", "Polyline (PL)", "spline", "PLINE: a polyline, with arcs (A)"),
            act("cmdline:CIRCLE", "Circle (C)", "circle", "CIRCLE: centre and radius, 2P, 3P, TTR"),
            act("cmdline:ARC", "Arc (A)", "arc-length", "ARC: three points, or start, centre, end"),
            act("cmdline:RECTANG", "Rectangle (REC)", "rectangle-horizontal", "RECTANG: two corners"),
            act("cmdline:HATCH", "Hatch (H)", "hash", "HATCH: click inside an area"),
            act("cmdline:TEXT", "Text (T)", "text-tool", "TEXT: a note"),
            Sep,
            act("cmdline:MOVE", "Move (M)", "move", "MOVE"),
            act("cmdline:COPY", "Copy (CO)", "copy", "COPY: the copy follows the cursor"),
            act("cmdline:ROTATE", "Rotate (RO)", "rotate-cw", "ROTATE"),
            act("cmdline:MIRROR", "Mirror (MI)", "flip-horizontal-2", "MIRROR"),
            act("cmdline:SCALE", "Scale (SC)", "scaling", "SCALE"),
            act("cmdline:STRETCH", "Stretch (S)", "move-3d", "STRETCH"),
            act("cmdline:TRIM", "Trim (TR)", "scissors", "TRIM"),
            act("cmdline:EXTEND", "Extend (EX)", "arrow-right", "EXTEND"),
            act("cmdline:FILLET", "Fillet (F)", "corner-down-right", "FILLET"),
            act("cmdline:OFFSET", "Offset (O)", "offset", "OFFSET"),
            act("cmdline:EXPLODE", "Explode (X)", "box", "EXPLODE"),
            act("cmdline:ERASE", "Erase (E)", "trash-2", "ERASE"),
            Sep,
            act("cmdline:DIMLINEAR", "Dimension (DLI)", "dimension", "DIMLINEAR"),
        ],
        1 => vec![
            T(Tool::Wall),
            act(
                "tool.curtain",
                "Curtain wall",
                "curtain-wall",
                "The Wall tool drawing a curtain wall type: mullions and glass on a grid",
            ),
            T(Tool::Door),
            T(Tool::Window),
            T(Tool::Opening),
            Sep,
            T(Tool::Slab),
            T(Tool::Shaft),
            T(Tool::Roof),
            T(Tool::Stair),
            T(Tool::Railing),
            Sep,
            T(Tool::Column),
            T(Tool::Beam),
            T(Tool::Footing),
            Sep,
            T(Tool::Object),
            act(
                "catalogue",
                "Catalogue",
                "sofa",
                "Furniture and fixtures to place, and your own from Blender, FreeCAD or glTF",
            ),
            Sep,
            T(Tool::Mass),
            T(Tool::Extrusion),
            T(Tool::Sweep),
            T(Tool::Void),
            T(Tool::Sketch),
            Sep,
            T(Tool::Room),
            T(Tool::RoomSeparator),
            T(Tool::Ceiling),
            Sep,
            T(Tool::Terrain),
            T(Tool::Paving),
            T(Tool::Planting),
        ],
        2 => vec![
            T(Tool::Dimension),
            T(Tool::Angle),
            T(Tool::Radius),
            T(Tool::Diameter),
            T(Tool::ArcLength),
            T(Tool::SpotLevel),
            Sep,
            T(Tool::Tag),
            act(
                "tag.all",
                "Tag all",
                "tags",
                "Tag every door, window, fixture and room in the view that has no tag yet (Shift+T)",
            ),
            T(Tool::Note),
            Sep,
            T(Tool::Detail),
            T(Tool::Callout),
            Sep,
            act(
                "styles",
                "Annotation styles",
                "anno-styles",
                "Named styles for dimensions, notes and tags: text, ends, tag shape, colour",
            ),
            act(
                "marks",
                "Mark numbering",
                "hash",
                "How doors, windows and other elements are numbered; renumber, and check for marks used twice",
            ),
        ],
        3 => vec![
            // levels are what plans are cut from, so they sit with the
            // views that come from them (MU20)
            T(Tool::Level),
            T(Tool::Section),
            T(Tool::Elevation),
            T(Tool::Callout),
            T(Tool::Camera),
            Sep,
            act(
                "crop.add",
                "Crop box",
                "crop-box",
                "Add a crop box to the 3D view: it crops everything outside it",
            ),
            act(
                "view.reference",
                "Reference",
                "underlay",
                "Trace over another view, a picture or a DXF drawing: shown faintly under this view (Alt+F2 hides it)",
            ),
            act("new.sheet", "New sheet", "file-text", "A new drawing sheet"),
            Entry::Menu {
                key: "tab:schedules",
                label: "New schedule",
                icon: "table-2",
                hint: "A schedule of doors, windows, walls, rooms and more",
            },
            Sep,
            act(
                "graphicstyles",
                "Graphic styles",
                "align-left",
                "Style packs: pens by scale, object styles, hatch patterns and view templates",
            ),
            act(
                "templates",
                "Sheet templates",
                "sheet-template",
                "Title blocks for sheets",
            ),
        ],
        _ => vec![
            // the Levels manager is project set-up; the Level tool, which
            // makes a level view, is on the View tab (MU21)
            act(
                "levels",
                "Levels",
                "level",
                "Add, duplicate, rename, move and delete levels",
            ),
            T(Tool::Grid),
            T(Tool::Guide),
            Sep,
            act(
                "project.info",
                "Project information",
                "info",
                "Name, client, address, site and building: they fill the title blocks and the IFC export",
            ),

            act(
                "types.browser",
                "Types",
                "element-types",
                "Wall, curtain wall, door, window, slab, roof, column, beam and object types",
            ),
            act(
                "profiles.browser",
                "Profiles",
                "profile",
                "The profile library: presets and the project's own, for sweeps",
            ),
            act(
                "mat.browser",
                "Materials",
                "palette",
                "Add, edit, delete and assign the project's materials",
            ),
            act(
                "graphicstyles",
                "Style packs",
                "align-left",
                "Pens by scale, object styles, hatch patterns and view templates",
            ),
            act(
                "units",
                "Project units",
                "units",
                "Metric or imperial: how lengths are shown and typed",
            ),
            act(
                "cleanup.stacked",
                "Copies on originals",
                "copy",
                "Find and remove elements sitting exactly on a copy of themselves (left by a Duplicate that never moved)",
            ),
            act(
                "cleanup.overlap",
                "Overlapping walls",
                "walls-overlap",
                "Find walls running along each other and overlapping: the plan merges them, but in 3D they stay two solids. Merge keeps one wall covering both",
            ),
            Sep,
            Entry::Menu {
                key: "tab:ifc",
                label: "IFC",
                icon: "upload",
                hint: "Open an IFC file, or export this project as IFC4 or IFC5",
            },
        ],
    }
}

/// Does tab `tab` show `tool`?
pub fn holds(tab: usize, tool: Tool) -> bool {
    ALWAYS.contains(&tool) || entries(tab).contains(&Entry::Tool(tool))
}

/// The first tab that shows `tool`.
pub fn home_of(tool: Tool) -> usize {
    (0..TABS.len()).find(|t| holds(*t, tool)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_buttons_on_a_tab_share_an_icon() {
        // A5-1: two buttons that look the same read as one
        for (tab, tab_name) in TABS.iter().enumerate() {
            let mut seen: Vec<(&str, String)> = vec![];
            for e in entries(tab) {
                let (icon, name) = match e {
                    Entry::Tool(t) => (crate::app::shell::tool_icon(t), t.label().to_string()),
                    Entry::Act { icon, label, .. } | Entry::Menu { icon, label, .. } => {
                        (icon, label.to_string())
                    }
                    Entry::Sep | Entry::LayerCombo => continue,
                };
                if let Some((_, other)) = seen.iter().find(|(i, _)| *i == icon) {
                    panic!("{tab_name}: {name} and {other} both use the {icon} icon");
                }
                seen.push((icon, name));
            }
        }
    }

    #[test]
    fn every_tool_is_on_a_tab() {
        for t in Tool::ALL {
            assert!(
                (0..TABS.len()).any(|tab| holds(tab, t)),
                "{} is on no tab",
                t.label()
            );
        }
        assert_eq!(home_of(Tool::Wall), 1);
        assert_eq!(home_of(Tool::Dimension), 2);
        assert_eq!(home_of(Tool::Section), 3);
        assert_eq!(home_of(Tool::Grid), 4);
        // the callout is on both of its tabs
        assert!(holds(2, Tool::Callout) && holds(3, Tool::Callout));
        // Select is on every one
        assert!((0..TABS.len()).all(|t| holds(t, Tool::Select)));
    }
}
