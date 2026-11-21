//! AutoCAD's command line: type a command or its alias (L, C, TR…, in
//! English or in Italian), then points (`x,y`, `@dx,dy`, `@d<angle`, a
//! distance alone along the pointer) and values, each ended with Enter or
//! Space. Enter on an empty line finishes what is being drawn, or repeats
//! the last command. Esc cancels. Up and Down recall what was typed.
//!
//! While it is on, typing in a drawing goes to the command line, as in
//! AutoCAD: single-key shortcuts give way to the aliases.

use super::*;
use crate::planview::Tool;
use glam::DVec2;

/// What the command line holds between frames.
#[derive(Clone, Debug, Default)]
pub struct CmdLine {
    /// What is being typed.
    pub text: String,
    /// What was said, oldest first: commands, prompts, answers.
    pub log: Vec<String>,
    /// What was typed before, for Up and Down.
    pub recent: Vec<String>,
    /// Where Up has got to in `recent`.
    pub recall: Option<usize>,
    /// The command running and what it asks for now.
    pub running: Option<Running>,
    /// The last command entered, for Enter on an empty line.
    pub last: Option<String>,
    /// How many of a command's points it has dealt with already (those
    /// that act as they come: MATCHPROP, PLINE…).
    pub seen: usize,
    /// The linework a command works on (MATCHPROP's source, DIVIDE's).
    pub source: Option<ElementId>,
    /// A polyline's points so far, its arcs as chords, and whether the
    /// next stretch is an arc.
    pub pline: Vec<DVec2>,
    pub arc_mode: bool,
    /// Numbers a command asked for before its points (DONUT's diameters,
    /// TTR's picks).
    pub values: Vec<f64>,
    pub picks: Vec<DVec2>,
    /// The command a FROM or M2P interrupted, to go back to.
    pub resume: Option<Running>,
    /// Esc was pressed with nothing typed: the tool in use ends once the
    /// views have had it (AutoCAD's Esc ends any command).
    pub esc: bool,
    /// The text window (F2): everything the command line said.
    pub history: bool,
    /// Each view's camera before the last ZOOM, for ZOOM Previous.
    pub zoomed: Vec<(ElementId, (DVec2, f32))>,
}

/// A command waiting for more from the command line.
#[derive(Clone, Debug, PartialEq)]
pub struct Running {
    /// Its name, as AutoCAD writes it (LINE, CIRCLE…).
    pub name: &'static str,
    /// What it asks for, as the prompt says it.
    pub prompt: String,
    /// What the next answer means.
    pub wants: Wants,
}

/// What a running command takes next.
#[derive(Clone, Debug, PartialEq)]
pub enum Wants {
    /// Points for the tool that is drawing; Enter ends it.
    ToolPoints,
    /// Points of its own, up to `n` (0: until Enter), then `then`.
    Points { n: usize, then: Finish },
    /// A number: what it sets.
    Number(NumberFor),
    /// Objects to work on (clicked in the drawing), then Enter.
    Selection(crate::commands::Command),
    /// Objects to work on, then Enter: a command of the command line's own.
    SelectThen(&'static str),
    /// A word: a name, or an option.
    Word(WordFor),
}

/// What a word typed for a command is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordFor {
    UcsSave,
    UcsRestore,
    UcsDelete,
    Plan,
    UcsIcon,
}

/// What a command does with the points it gathered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    Distance,
    /// OFFSET's distance, from two points clicked.
    OffsetDist,
    Id,
    Area,
    Circle2P,
    Circle3P,
    /// An arc from its start, centre and end, drawn as a 3-point arc.
    ArcCenter,
    Break,
    BreakAt,
    Stretch,
    MatchProp,
    Donut,
    Point,
    DividePick,
    MeasurePick,
    Pline,
    HatchPick,
    Ttr,
    FromBase,
    Mid2,
    ZoomWindow,
    NorthPoints,
    Lengthen,
    Ucs,
    UcsObject,
}

impl Finish {
    /// Does it act on each point as it comes (until Enter)?
    pub fn streams(self) -> bool {
        matches!(
            self,
            Finish::MatchProp
                | Finish::Donut
                | Finish::Point
                | Finish::Pline
                | Finish::HatchPick
                | Finish::Lengthen
                | Finish::Ucs
        )
    }
}

/// What a number typed for a command sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberFor {
    PolygonSides,
    FilletRadius,
    ChamferDistance,
    OffsetDistance,
    LengthenDelta,
    DonutInner,
    DonutOuter,
    DivideCount,
    MeasureLength,
    TtrRadius,
    UcsZ,
    NorthAngle,
}

/// A command, by its name or one of its aliases: English and Italian
/// AutoCAD, with a leading `_` or `-` taken off.
pub fn find(word: &str) -> Option<&'static str> {
    let w = word
        .trim()
        .trim_start_matches(['_', '-', '.'])
        .to_uppercase();
    ALIASES
        .iter()
        .find(|(_, names)| names.split(' ').any(|n| n == w))
        .map(|(c, _)| *c)
}

/// Each command and the words that start it: its AutoCAD name, its
/// aliases, its Italian name and aliases.
pub const ALIASES: &[(&str, &str)] = &[
    ("LINE", "LINE L LINEA LI"),
    ("PLINE", "PLINE PL POLILINEA"),
    ("XLINE", "XLINE XL RAY LINEACOSTR LC SEMIRETTA"),
    ("CIRCLE", "CIRCLE C CERCHIO CE"),
    ("ARC", "ARC A ARCO"),
    ("RECTANG", "RECTANG REC RECTANGLE RETTANGOLO RET"),
    ("POLYGON", "POLYGON POL POLIGONO"),
    ("ELLIPSE", "ELLIPSE EL ELLISSE"),
    ("SPLINE", "SPLINE SPL"),
    ("HATCH", "HATCH H BH TRATTEGGIO TRA RETINO"),
    ("TEXT", "TEXT T DT MTEXT MT TESTO TESTOM"),
    (
        "DIMLINEAR",
        "DIMLINEAR DLI DIM DIMALIGNED DAL QUOTA QUOTALIN",
    ),
    ("DIMANGULAR", "DIMANGULAR DAN QUOTANG"),
    ("DIMRADIUS", "DIMRADIUS DRA QUOTARAG"),
    ("DIMDIAMETER", "DIMDIAMETER DDI QUOTADIA"),
    ("ERASE", "ERASE E CANCELLA CA DELETE"),
    ("MOVE", "MOVE M SPOSTA SP"),
    ("COPY", "COPY CO CP COPIA"),
    ("ROTATE", "ROTATE RO RUOTA"),
    ("MIRROR", "MIRROR MI SPECCHIO"),
    ("SCALE", "SCALE SC SCALA"),
    ("TRIM", "TRIM TR TAGLIA TA"),
    ("EXTEND", "EXTEND EX ESTENDI ES"),
    ("FILLET", "FILLET F RACCORDO RA"),
    ("CHAMFER", "CHAMFER CHA CIMA CI"),
    ("OFFSET", "OFFSET O"),
    ("ARRAY", "ARRAY AR ARRAYRECT SERIE SERIERETT"),
    ("ARRAYPOLAR", "ARRAYPOLAR SERIEPOLARE"),
    ("BREAK", "BREAK BR SPEZZA"),
    ("BREAKATPOINT", "BREAKATPOINT BRP SPEZZAINPUNTO"),
    ("JOIN", "JOIN J UNISCI"),
    ("LENGTHEN", "LENGTHEN LEN ALLUNGA"),
    ("STRETCH", "STRETCH S STIRA"),
    ("MATCHPROP", "MATCHPROP MA COPIAPROP"),
    ("DONUT", "DONUT DO CIAMBELLA"),
    ("POINT", "POINT PO PUNTO"),
    ("DIVIDE", "DIVIDE DIV DIVIDI"),
    ("MEASURE", "MEASURE ME MISURA"),
    ("EXPLODE", "EXPLODE X ESPLODI ESP"),
    ("BLOCK", "BLOCK B BLOCCO"),
    ("UNDO", "UNDO U ANNULLA"),
    ("REDO", "REDO RIPRISTINA"),
    ("DIST", "DIST DI DISTANZA"),
    ("ID", "ID"),
    ("AREA", "AREA AA"),
    ("LAYER", "LAYER LA"),
    ("OSNAP", "OSNAP OS DSETTINGS DS SE IMPOSTADIS"),
    ("ORTHO", "ORTHO ORTO"),
    ("ZOOM", "ZOOM Z"),
    ("PAN", "PAN P"),
    ("REGEN", "REGEN RE RIGEN"),
    ("WALL", "WALL MURO"),
    ("DOOR", "DOOR PORTA"),
    ("WINDOW", "WINDOW FINESTRA"),
    ("SLAB", "SLAB SOLAIO"),
    ("COLUMN", "COLUMN PILASTRO"),
    ("BEAM", "BEAM TRAVE"),
    ("ROOM", "ROOM LOCALE STANZA"),
    ("SECTION", "SECTION SEZIONE"),
    ("GRID", "GRID GRIGLIA ASSI"),
    ("NORTH", "NORTH NORD"),
    ("NORTHDIR", "NORTHDIR DIRNORD NORDVERO TRUENORTH"),
    ("UCS", "UCS SCU"),
    ("UCSMAN", "UCSMAN UC GESTSCU"),
    ("PLAN", "PLAN PIANTA"),
    ("UCSICON", "UCSICON ICONASCU"),
];

/// A point typed: `x,y` from the origin, `@dx,dy` from the last point,
/// `@d<a` and `d<a` by distance and angle (degrees from X, anticlockwise),
/// `#x,y` from the origin as well. Lengths read in the project's units.
#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_point(text: &str, last: Option<DVec2>) -> Option<DVec2> {
    parse_point_in(text, last, crate::planview::Ucs::default())
}

/// [`parse_point`] in a UCS, as AutoCAD reads points: coordinates and
/// angles in the UCS, `*x,y` (or `@*`) in the world's. The point is given
/// in the world's coordinates.
pub fn parse_point_in(text: &str, last: Option<DVec2>, ucs: crate::planview::Ucs) -> Option<DVec2> {
    let t = text.trim().replace(' ', "");
    let world = t.starts_with('*') || t.starts_with("@*");
    let ucs = if world {
        crate::planview::Ucs::default()
    } else {
        ucs
    };
    let t = t.replacen('*', "", 1);
    let local = parse_plain(&t, last.map(|l| ucs.local(l)))?;
    Some(ucs.world(local))
}

fn parse_plain(text: &str, last: Option<DVec2>) -> Option<DVec2> {
    let t = text.trim().replace(' ', "");
    let (rel, t) = match t.strip_prefix('@') {
        Some(r) => (true, r.to_string()),
        None => (false, t.trim_start_matches('#').to_string()),
    };
    let base = if rel { last? } else { DVec2::ZERO };
    if let Some((d, a)) = t.split_once('<') {
        let d = crate::units::parse(d)?;
        let a: f64 = a.trim().parse().ok()?;
        return Some(base + DVec2::from_angle(a.to_radians()) * d);
    }
    let (x, y) = t.split_once(',')?;
    // "@" alone, or "@0,0", is the last point itself
    let x = if x.is_empty() {
        0.0
    } else {
        crate::units::parse(x)?
    };
    let y = if y.is_empty() {
        0.0
    } else {
        crate::units::parse(y)?
    };
    Some(base + DVec2::new(x, y))
}

/// The centre and radius of the circle through three points.
pub fn circle_through(a: DVec2, b: DVec2, c: DVec2) -> Option<(DVec2, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-12 {
        return None;
    }
    let (a2, b2, c2) = (a.length_squared(), b.length_squared(), c.length_squared());
    let ux = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
    let uy = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
    let o = DVec2::new(ux, uy);
    Some((o, o.distance(a)))
}

/// The area an outline holds, and its perimeter.
pub fn area_of(pts: &[DVec2]) -> (f64, f64) {
    let n = pts.len();
    let mut a = 0.0;
    let mut p = 0.0;
    for i in 0..n {
        let (u, v) = (pts[i], pts[(i + 1) % n]);
        a += u.perp_dot(v);
        p += u.distance(v);
    }
    ((a / 2.0).abs(), p)
}

/// The draft shape of the Detail tool's list for each drawing command.
fn draft_index(name: &str) -> Option<usize> {
    use creabim_core::{DetailKind, DraftShape};
    let shape = match name {
        "LINE" | "PLINE" => DraftShape::Line,
        "RECTANG" => DraftShape::Rectangle,
        "POLYGON" => DraftShape::Polygon { sides: 6 },
        "CIRCLE" => DraftShape::Circle,
        "ARC" => DraftShape::Arc,
        "ELLIPSE" => DraftShape::Ellipse,
        "SPLINE" => DraftShape::Spline,
        _ => return None,
    };
    DetailKind::ALL
        .iter()
        .position(|k| k.same(DetailKind::draft(shape)))
}

impl CreaBimApp {
    /// The 2D view the command line works in: the one in front.
    pub(super) fn cmd_view(&self) -> Option<ElementId> {
        self.active_view.filter(|v| {
            matches!(
                self.view(*v).map(|v| &v.kind),
                Some(ViewKind::Plan { .. } | ViewKind::Section { .. })
            )
        })
    }

    pub(super) fn say(&mut self, line: impl Into<String>) {
        let line = line.into();
        self.status = line.clone();
        self.cmd.log.push(line);
        let n = self.cmd.log.len();
        if n > 200 {
            self.cmd.log.drain(..n - 200);
        }
    }

    pub(super) fn ask(&mut self, name: &'static str, prompt: impl Into<String>, wants: Wants) {
        let prompt = prompt.into();
        self.cmd.running = Some(Running {
            name,
            prompt: prompt.clone(),
            wants,
        });
        self.status = format!("{name} {prompt}");
    }

    /// Typing in a drawing goes to the command line: the keys are taken
    /// before the views and the shortcuts see them.
    pub(super) fn cmd_keys(&mut self, ctx: &egui::Context) {
        // Esc always ends the command: a field that had the keyboard
        // (Properties, a box of the strip) lets it go too, rather than
        // swallowing the Esc
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if ctx.egui_wants_keyboard_input() {
                ctx.memory_mut(|m| m.stop_text_input());
            }
            self.cmd.esc = true;
        }
        // a move, rotate or scale running takes its typed length itself
        if !self.shell.settings.command_line
            || ctx.egui_wants_keyboard_input()
            || self.plan.modal.is_some()
        {
            return;
        }
        let typing = !self.cmd.text.is_empty();
        let busy = self.cmd.running.is_some() || self.plan.details.choosing_cutters;
        let idle = self.plan.tool == Tool::Select;
        let mut text = String::new();
        let (mut enter, mut esc, mut back, mut up, mut down) = (false, false, false, false, false);
        ctx.input_mut(|i| {
            let plain = !i.modifiers.command && !i.modifiers.alt;
            i.events.retain(|e| match e {
                egui::Event::Text(t) if plain => {
                    // Space ends what was typed, as Enter does
                    if t == " " {
                        if typing || !text.is_empty() || busy {
                            enter = true;
                            return false;
                        }
                        return true;
                    }
                    if t.chars()
                        .all(|c| c.is_alphanumeric() || "@<,.-_#'\"/".contains(c))
                    {
                        text.push_str(t);
                        false
                    } else {
                        true
                    }
                }
                egui::Event::Key {
                    key,
                    pressed,
                    modifiers,
                    ..
                } if !modifiers.command && !modifiers.alt => {
                    use egui::Key as K;
                    let letter = key.symbol_or_name().chars().count() == 1
                        && key
                            .symbol_or_name()
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric());
                    match key {
                        // Enter alone: the drawing's own (a slab outline
                        // closes) unless a command runs or one can repeat
                        K::Enter if typing || busy || (idle && self.cmd.last.is_some()) => {
                            enter |= *pressed;
                            false
                        }
                        K::Escape if typing || busy => {
                            esc |= *pressed;
                            false
                        }
                        K::Backspace if typing => {
                            back |= *pressed;
                            false
                        }
                        K::ArrowUp if !self.cmd.recent.is_empty() && (typing || !busy) => {
                            up |= *pressed;
                            false
                        }
                        K::ArrowDown if typing => {
                            down |= *pressed;
                            false
                        }
                        // a letter's own key: no single-key shortcut while
                        // the command line takes typing
                        _ if letter => false,
                        K::Comma | K::Period | K::Minus => false,
                        _ => true,
                    }
                }
                _ => true,
            });
        });
        if !text.is_empty() {
            self.cmd.text.push_str(&text);
            self.cmd.recall = None;
        }
        if back {
            self.cmd.text.pop();
        }
        if up {
            let n = self.cmd.recent.len();
            let i = match self.cmd.recall {
                Some(i) if i > 0 => i - 1,
                Some(i) => i,
                None => n - 1,
            };
            self.cmd.recall = Some(i);
            self.cmd.text = self.cmd.recent[i].clone();
        }
        if down {
            if let Some(i) = self.cmd.recall {
                if i + 1 < self.cmd.recent.len() {
                    self.cmd.recall = Some(i + 1);
                    self.cmd.text = self.cmd.recent[i + 1].clone();
                } else {
                    self.cmd.recall = None;
                    self.cmd.text.clear();
                }
            }
        }
        if esc {
            self.cmd.text.clear();
            if let Some(r) = self.cmd.running.take() {
                self.say(format!("{} *Cancel*", r.name));
                self.plan.cmd_pick = None;
                self.plan.cmd_preview = None;
                self.plan.pline_arcs.clear();
                self.cmd.pline.clear();
                self.plan.details.path.clear();
            }
        }
        if enter {
            let t = std::mem::take(&mut self.cmd.text);
            self.cmd.recall = None;
            self.cmd_enter(ctx, t.trim());
        }
    }

    /// The command line's answer to a line typed (empty: Enter alone).
    pub(super) fn cmd_enter(&mut self, ctx: &egui::Context, t: &str) {
        // TRIM and EXTEND: Enter ends picking the cutting edges
        if t.is_empty() && self.plan.tool == Tool::Detail && self.plan.details.choosing_cutters {
            self.plan.details.choosing_cutters = false;
            let n = self.plan.details.cutters.len();
            let what = match self.plan.details.edit {
                Some(crate::detailtool::DraftEdit::Extend) => "extend",
                _ => "trim",
            };
            self.say(format!(
                "{}. Select the objects to {what}: click each, or drag a fence line across them. Enter or Esc ends.",
                if n == 0 {
                    "Every line cuts".to_string()
                } else {
                    format!("{n} cutting edges")
                }
            ));
            return;
        }
        if !t.is_empty() && self.cmd.recent.last().map(|s| s.as_str()) != Some(t) {
            self.cmd.recent.push(t.to_string());
        }
        // what the command running asks for
        if let Some(r) = self.cmd.running.clone() {
            if self.cmd_answer(ctx, &r, t) {
                return;
            }
        }
        // a tool picked from the bars, not typed, takes points too
        let drawing = self.plan.tool != Tool::Select;
        let mid_shape = self.plan.outline.is_some()
            || !self.plan.details.path.is_empty()
            || self.plan.pending.is_some();
        if drawing && ((t.is_empty() && mid_shape) || (!t.is_empty() && find(t).is_none())) {
            let r = Running {
                name: "POINT",
                prompt: String::new(),
                wants: Wants::ToolPoints,
            };
            if self.cmd_answer(ctx, &r, t) {
                return;
            }
        }
        if t.is_empty() {
            // Enter alone repeats the last command
            if let Some(c) = self.cmd.last.clone() {
                self.cmd_start(ctx, &c);
            }
            return;
        }
        match find(t) {
            Some(c) => self.cmd_start(ctx, c),
            None => self.say(format!(
                "Unknown command \"{}\". Type a command, such as L, C, TR or M.",
                t.to_uppercase()
            )),
        }
    }

    /// The answer to what a running command asks; false when it isn't one
    /// (a new command typed over it).
    fn cmd_answer(&mut self, ctx: &egui::Context, r: &Running, t: &str) -> bool {
        let view = self.cmd_view();
        let last = self
            .plan
            .last_point
            .filter(|(v, _)| Some(*v) == view)
            .map(|(_, p)| p);
        // a point: typed, or a distance alone along the pointer from the
        // last point (direct distance entry)
        let point = || -> Option<DVec2> {
            // in the UCS, in a plan
            let ucs = match view.and_then(|v| self.view(v)).map(|v| &v.kind) {
                Some(ViewKind::Plan { .. }) => self.plan.ucs,
                _ => crate::planview::Ucs::default(),
            };
            if let Some(p) = parse_point_in(t, last, ucs) {
                return Some(p);
            }
            let d = crate::units::parse(t)?;
            let from = last?;
            let towards = self
                .plan
                .cursor_at
                .filter(|(v, _)| Some(*v) == view)
                .map(|(_, p)| p)?;
            let dir = (towards - from).try_normalize()?;
            Some(from + dir * d)
        };
        match &r.wants {
            Wants::ToolPoints => {
                if t.is_empty() {
                    // Enter: the shape ends where it is
                    if self.plan.outline.is_some() {
                        self.plan.close_outline = true;
                    } else {
                        self.plan.cmd_finish = true;
                    }
                    self.cmd.running = None;
                    if r.name != "POINT" {
                        self.say(format!("{} done.", r.name));
                    }
                    return true;
                }
                // options of the drawing commands
                let up = t.to_uppercase();
                match (r.name, up.as_str()) {
                    ("CIRCLE", "2P") => {
                        self.cmd_pick_start(
                            r.name,
                            "Specify first end point of circle's diameter:",
                            2,
                            Finish::Circle2P,
                        );
                        return true;
                    }
                    ("CIRCLE", "TTR" | "T") => {
                        self.cmd_pick_start(
                            r.name,
                            "Specify point on object for first tangent, then on the second:",
                            2,
                            Finish::Ttr,
                        );
                        return true;
                    }
                    // FROM: a base point, then the offset from it
                    (_, "FROM" | "FRO" | "DA" | "_FROM") => {
                        self.cmd.resume = Some(r.clone());
                        self.cmd_pick_start(
                            r.name,
                            "Base point (then <Offset>: @dx,dy):",
                            1,
                            Finish::FromBase,
                        );
                        return true;
                    }
                    // the point halfway between two points
                    (_, "M2P" | "MTP" | "PM" | "_M2P") => {
                        self.cmd.resume = Some(r.clone());
                        self.cmd_pick_start(
                            r.name,
                            "First point of mid, then second point of mid:",
                            2,
                            Finish::Mid2,
                        );
                        return true;
                    }
                    ("CIRCLE", "3P") => {
                        self.cmd_pick_start(
                            r.name,
                            "Specify first point on circle:",
                            3,
                            Finish::Circle3P,
                        );
                        return true;
                    }
                    ("ARC", "C" | "CE") => {
                        self.cmd_pick_start(
                            r.name,
                            "Specify start point of arc, then its center and end point:",
                            3,
                            Finish::ArcCenter,
                        );
                        return true;
                    }
                    ("LINE" | "PLINE", "U") => {
                        self.plan.details.path.pop();
                        self.say("Undo: the last point is taken back.");
                        return true;
                    }
                    ("LINE" | "PLINE", "C") => {
                        // close: back to the first point
                        if let (Some(v), Some(first)) =
                            (view, self.plan.details.path.first().copied())
                        {
                            self.plan.forced_point = Some((v, first));
                        }
                        return true;
                    }
                    _ => {}
                }
                match (view, point()) {
                    (Some(v), Some(p)) => {
                        self.plan.forced_point = Some((v, p));
                        self.say(format!("{} {}", r.name, t));
                        true
                    }
                    (None, Some(_)) => {
                        self.say("Points go in a plan or section: open one first.");
                        true
                    }
                    _ => false,
                }
            }
            Wants::Points { n, then } => {
                let (n, then) = (*n, *then);
                let done = |app: &mut Self| {
                    let pts = app.plan.cmd_pick.take().map(|p| p.1).unwrap_or_default();
                    app.cmd.running = None;
                    app.cmd_finish_points(then, &pts);
                };
                // OFFSET: a distance typed, or Enter for the one shown
                if r.name == "OFFSET" {
                    let typed = if t.is_empty() {
                        Some(None)
                    } else {
                        crate::units::parse(t).map(Some)
                    };
                    if let Some(v) = typed {
                        self.plan.cmd_pick = None;
                        self.cmd_number_done(ctx, r.name, NumberFor::OffsetDistance, v);
                        return true;
                    }
                }
                if t.is_empty() {
                    if n == 0 {
                        done(self);
                    } else {
                        self.cmd.running = None;
                        self.plan.cmd_pick = None;
                        self.say(format!("{} *Cancel*", r.name));
                    }
                    return true;
                }
                match point() {
                    Some(p) => {
                        if let Some((v, pts)) = self.plan.cmd_pick.as_mut() {
                            if v.is_none() {
                                *v = view;
                            }
                            pts.push(p);
                        }
                        if let Some(v) = view {
                            self.plan.last_point = Some((v, p));
                        }
                        self.cmd_tick();
                        true
                    }
                    None => self.cmd_more_option(ctx, r, t),
                }
            }
            Wants::Number(what) => {
                let what = *what;
                let Some(v) = (if what == NumberFor::PolygonSides {
                    t.parse::<f64>().ok()
                } else {
                    crate::units::parse(t)
                }) else {
                    if t.is_empty() {
                        // Enter keeps the value shown
                        self.cmd_number_done(ctx, r.name, what, None);
                        return true;
                    }
                    return false;
                };
                self.cmd_number_done(ctx, r.name, what, Some(v));
                true
            }
            Wants::Word(what) => {
                let what = *what;
                self.cmd.running = None;
                self.cmd_word(what, t);
                true
            }
            Wants::SelectThen(what) => {
                if t.is_empty() {
                    let what = *what;
                    self.cmd.running = None;
                    self.cmd_selected(what);
                    return true;
                }
                false
            }
            Wants::Selection(c) => {
                if t.is_empty() {
                    let c = *c;
                    self.cmd.running = None;
                    if self.selection.is_empty() {
                        self.say("Nothing selected.");
                    } else {
                        self.run_command(ctx, c, None);
                    }
                    return true;
                }
                false
            }
        }
    }

    pub(super) fn cmd_pick_start(
        &mut self,
        name: &'static str,
        prompt: &str,
        n: usize,
        then: Finish,
    ) {
        self.cmd.seen = 0;
        self.plan.details.path.clear();
        self.plan.cmd_pick = Some((None, vec![]));
        self.ask(name, prompt, Wants::Points { n, then });
    }

    fn cmd_number_done(
        &mut self,
        ctx: &egui::Context,
        name: &'static str,
        what: NumberFor,
        v: Option<f64>,
    ) {
        use crate::detailtool::DraftEdit;
        if self.cmd_more_number(name, what, v) {
            return;
        }
        let d = &mut self.plan.details;
        let edit = match what {
            NumberFor::PolygonSides => {
                if let (
                    Some(n),
                    creabim_core::DetailKind::Draft {
                        shape: creabim_core::DraftShape::Polygon { sides },
                        ..
                    },
                ) = (v, &mut d.kind)
                {
                    *sides = (n.round() as i64).clamp(3, 64) as u8;
                }
                self.ask(
                    name,
                    "Specify center of polygon, then a corner:",
                    Wants::ToolPoints,
                );
                return;
            }
            NumberFor::FilletRadius => {
                if let Some(v) = v {
                    d.radius = v.max(0.0);
                }
                DraftEdit::Fillet
            }
            NumberFor::ChamferDistance => {
                if let Some(v) = v {
                    d.chamfer = v.max(0.0);
                }
                DraftEdit::Chamfer
            }
            NumberFor::OffsetDistance => {
                if let Some(v) = v {
                    d.offset = v.max(0.0);
                }
                DraftEdit::Offset
            }
            _ => return,
        };
        let _ = ctx;
        self.plan.details.pick_edit(edit);
        self.cmd.running = None;
        self.say(format!("{name}: {}", edit.hint()));
    }

    /// A command's points are in: what it does with them.
    pub(super) fn cmd_finish_points(&mut self, then: Finish, pts: &[DVec2]) {
        let len = crate::units::len;
        match (then, pts) {
            (Finish::OffsetDist, [a, b, ..]) => {
                let d = a.distance(*b);
                self.plan.details.offset = d.max(0.0);
                self.plan.cmd_pick = None;
                self.plan
                    .details
                    .pick_edit(crate::detailtool::DraftEdit::Offset);
                self.cmd.running = None;
                self.say(format!(
                    "OFFSET: {}. {}",
                    len(d),
                    crate::detailtool::DraftEdit::Offset.hint()
                ));
            }
            (Finish::Distance, [a, b, ..]) => {
                // in the UCS, as AutoCAD reports it
                let d = DVec2::from_angle(-self.plan.ucs.angle).rotate(*b - *a);
                self.say(format!(
                    "Distance = {}, Angle in XY plane = {:.4}°, Delta X = {}, Delta Y = {}",
                    len(d.length()),
                    d.y.atan2(d.x).to_degrees().rem_euclid(360.0),
                    len(d.x),
                    len(d.y)
                ));
            }
            (Finish::Id, [p, ..]) => {
                let q = self.plan.ucs.local(*p);
                self.say(format!("X = {}   Y = {}", len(q.x), len(q.y)))
            }
            (Finish::Area, pts) if pts.len() >= 3 => {
                let (a, p) = area_of(pts);
                self.say(format!("Area = {:.4} m², Perimeter = {}", a, len(p)));
            }
            (Finish::Circle2P, [a, b, ..]) => {
                let c = (*a + *b) / 2.0;
                self.cmd_add_circle(c, a.distance(*b) / 2.0);
            }
            (Finish::Circle3P, [a, b, c, ..]) => match circle_through(*a, *b, *c) {
                Some((o, r)) => self.cmd_add_circle(o, r),
                None => self.say("The three points are in line: no circle passes through them."),
            },
            (Finish::ArcCenter, [s, c, e, ..]) => {
                // anticlockwise from the start to the end, as AutoCAD's
                let r = s.distance(*c);
                let a0 = (*s - *c).to_angle();
                let mut a1 = (*e - *c).to_angle();
                if a1 <= a0 {
                    a1 += std::f64::consts::TAU;
                }
                let mid = *c + DVec2::from_angle((a0 + a1) / 2.0) * r;
                let end = *c + DVec2::from_angle(a1) * r;
                self.cmd_add_draft(creabim_core::DraftShape::Arc, vec![*s, mid, end]);
            }
            (then, pts) if self.cmd_more_finish(then, pts) => {}
            _ => self.say("Not enough points."),
        }
    }

    pub(super) fn cmd_add_circle(&mut self, c: DVec2, r: f64) {
        if r <= 1e-9 {
            self.say("A circle needs a radius.");
            return;
        }
        self.cmd_add_draft(creabim_core::DraftShape::Circle, vec![c, c + DVec2::X * r]);
    }

    /// New linework in the command line's view, with the pen, layer and
    /// line type the Detail tool would give it.
    pub(super) fn cmd_add_draft(&mut self, shape: creabim_core::DraftShape, points: Vec<DVec2>) {
        let Some(view) = self.cmd_view() else {
            self.say("Linework goes in a plan or section: open one first.");
            return;
        };
        let pen = self
            .plan
            .details
            .kind
            .as_draft()
            .map_or(creabim_core::LinePen::Medium, |d| d.1);
        let mut d = creabim_core::DetailItem::new(
            view,
            creabim_core::DetailKind::Draft { shape, pen },
            points,
        );
        d.layer = self.plan.details.layer.clone();
        d.linetype = self.plan.details.linetype;
        let e = Element::new(shape.label(), ElementKind::Detail(d));
        let id = e.id;
        match self.doc.commit(
            creabim_core::Transaction::new(format!("Add {}", shape.label().to_lowercase()))
                .insert(e),
        ) {
            Ok(_) => {
                self.selection.only(id);
                self.say(format!("{} drawn. Ctrl+Z to undo.", shape.label()));
            }
            Err(e) => self.say(format!("Couldn't do that: {e}")),
        }
    }

    /// Start command `c` (its AutoCAD name).
    pub(super) fn cmd_start(&mut self, ctx: &egui::Context, c: &str) {
        use crate::commands::Command;
        // a command typed over another ends it
        crate::detailtool::keep(&mut self.doc, &mut self.plan, &mut self.selection);
        self.plan.cmd_pick = None;
        self.cmd.running = None;
        let Some(name) = ALIASES.iter().map(|(n, _)| *n).find(|n| *n == c) else {
            return;
        };
        self.cmd.last = Some(name.to_string());
        self.cmd.log.push(format!("Command: {name}"));
        let detail = |app: &mut Self, ctx: &egui::Context| {
            app.pick_tool(ctx, Tool::Detail);
            if let Some(i) = draft_index(name) {
                app.run_action(ctx, &format!("detail.kind:{i}"));
            }
        };
        let edit = |app: &mut Self, ctx: &egui::Context, e: crate::detailtool::DraftEdit| {
            app.pick_tool(ctx, Tool::Detail);
            app.plan.details.pick_edit(e);
            app.say(format!("{name}: {}", e.hint()));
        };
        let selection_cmd = |app: &mut Self, ctx: &egui::Context, cmd: Command| {
            if app.selection.is_empty() && cmd.needs_selection() {
                app.pick_tool(ctx, Tool::Select);
                app.ask(name, "Select objects, then Enter:", Wants::Selection(cmd));
            } else {
                app.run_command(ctx, cmd, None);
            }
        };
        use crate::detailtool::DraftEdit as D;
        match name {
            _ if self.cmd_more_start(ctx, name) => {}
            "LINE" => {
                detail(self, ctx);
                self.ask(
                    name,
                    "Specify first point, then next points [Close/Undo]; Enter ends it:",
                    Wants::ToolPoints,
                );
            }
            "PLINE" => {
                detail(self, ctx);
                self.ask(
                    name,
                    "Specify start point, then next points [Close/Undo]; Enter ends it:",
                    Wants::ToolPoints,
                );
            }
            "CIRCLE" => {
                detail(self, ctx);
                self.ask(
                    name,
                    "Specify center point for circle or [3P/2P], then radius:",
                    Wants::ToolPoints,
                );
            }
            "ARC" => {
                detail(self, ctx);
                self.ask(
                    name,
                    "Specify start point of arc or [Center], then second and end point:",
                    Wants::ToolPoints,
                );
            }
            "RECTANG" => {
                detail(self, ctx);
                self.ask(
                    name,
                    "Specify first corner point, then other corner (@width,height):",
                    Wants::ToolPoints,
                );
            }
            "POLYGON" => {
                detail(self, ctx);
                let sides = match self.plan.details.kind {
                    creabim_core::DetailKind::Draft {
                        shape: creabim_core::DraftShape::Polygon { sides },
                        ..
                    } => sides,
                    _ => 6,
                };
                self.ask(
                    name,
                    format!("Enter number of sides <{sides}>:"),
                    Wants::Number(NumberFor::PolygonSides),
                );
            }
            "ELLIPSE" => {
                detail(self, ctx);
                self.ask(
                    name,
                    "Specify center, then axis endpoint, then other axis distance:",
                    Wants::ToolPoints,
                );
            }
            "SPLINE" => {
                detail(self, ctx);
                self.ask(name, "Specify points; Enter ends it:", Wants::ToolPoints);
            }
            "HATCH" => {
                self.pick_tool(ctx, Tool::Detail);
                if let Some(i) = creabim_core::DetailKind::ALL
                    .iter()
                    .position(|k| matches!(k, creabim_core::DetailKind::Region { .. }))
                {
                    self.run_action(ctx, &format!("detail.kind:{i}"));
                }
                self.ask(
                    name,
                    "Specify the corners of the area to hatch; Enter ends it:",
                    Wants::ToolPoints,
                );
            }
            "XLINE" => {
                self.pick_tool(ctx, Tool::Guide);
                self.ask(
                    name,
                    "Specify a point, then a point through which the line passes:",
                    Wants::ToolPoints,
                );
            }
            "TEXT" => {
                self.pick_tool(ctx, Tool::Note);
                self.say("TEXT: click where the text goes.");
            }
            "DIMLINEAR" => {
                self.pick_tool(ctx, Tool::Dimension);
                self.ask(name, "Specify first extension line origin, then second, then dimension line location:", Wants::ToolPoints);
            }
            "DIMANGULAR" => self.pick_tool(ctx, Tool::Angle),
            "DIMRADIUS" => self.pick_tool(ctx, Tool::Radius),
            "DIMDIAMETER" => self.pick_tool(ctx, Tool::Diameter),
            "ERASE" => selection_cmd(self, ctx, Command::Delete),
            "MOVE" => selection_cmd(self, ctx, Command::Move),
            "COPY" => selection_cmd(self, ctx, Command::Duplicate),
            "ROTATE" => selection_cmd(self, ctx, Command::Rotate),
            "MIRROR" => selection_cmd(self, ctx, Command::Mirror),
            "SCALE" => selection_cmd(self, ctx, Command::Scale),
            "TRIM" | "EXTEND" => {
                edit(self, ctx, if name == "TRIM" { D::Trim } else { D::Extend });
                self.selection.clear();
                self.say(format!(
                    "{name}: select the {} edges, then Enter (Enter alone: every line).",
                    if name == "TRIM" {
                        "cutting"
                    } else {
                        "boundary"
                    }
                ));
            }
            "FILLET" => {
                self.pick_tool(ctx, Tool::Detail);
                let r = crate::units::len(self.plan.details.radius);
                self.ask(
                    name,
                    format!("Specify fillet radius <{r}>:"),
                    Wants::Number(NumberFor::FilletRadius),
                );
            }
            "CHAMFER" => {
                self.pick_tool(ctx, Tool::Detail);
                let r = crate::units::len(self.plan.details.chamfer);
                self.ask(
                    name,
                    format!("Specify chamfer distance <{r}>:"),
                    Wants::Number(NumberFor::ChamferDistance),
                );
            }
            "OFFSET" => {
                self.pick_tool(ctx, Tool::Detail);
                let r = crate::units::len(self.plan.details.offset);
                // typed, or measured by two clicks on the drawing
                self.cmd_pick_start(
                    name,
                    &format!("Specify offset distance, or two points <{r}>:"),
                    2,
                    Finish::OffsetDist,
                );
            }
            "ARRAY" => edit(self, ctx, D::Array),
            "ARRAYPOLAR" => edit(self, ctx, D::Polar),
            "EXPLODE" => self.cmd_explode(ctx),
            "BLOCK" => self.make_symbol(),
            "UNDO" => self.run_command(ctx, Command::Undo, None),
            "REDO" => self.run_command(ctx, Command::Redo, None),
            "DIST" => self.cmd_pick_start(
                name,
                "Specify first point, then second point:",
                2,
                Finish::Distance,
            ),
            "ID" => self.cmd_pick_start(name, "Specify point:", 1, Finish::Id),
            "AREA" => self.cmd_pick_start(
                name,
                "Specify corner points; Enter totals them:",
                0,
                Finish::Area,
            ),
            "LAYER" => self.shell.layer_manager = true,
            "OSNAP" => self.shell.drafting_settings = true,
            "ORTHO" => self.run_action(ctx, "snap.ortho"),
            "ZOOM" | "PAN" | "REGEN" => {
                self.say(format!("{name}: the wheel zooms to the pointer, a middle drag pans; the Frame button shows everything."));
            }
            "WALL" => self.pick_tool(ctx, Tool::Wall),
            "DOOR" => self.pick_tool(ctx, Tool::Door),
            "WINDOW" => self.pick_tool(ctx, Tool::Window),
            "SLAB" => self.pick_tool(ctx, Tool::Slab),
            "COLUMN" => self.pick_tool(ctx, Tool::Column),
            "BEAM" => self.pick_tool(ctx, Tool::Beam),
            "ROOM" => self.pick_tool(ctx, Tool::Room),
            "SECTION" => self.pick_tool(ctx, Tool::Section),
            "GRID" => self.pick_tool(ctx, Tool::Grid),
            "NORTH" => self.run_action(ctx, "view.north"),
            _ => {}
        }
        // the tools that draw lines take typed points too
        if self.cmd.running.is_none()
            && matches!(
                name,
                "WALL" | "SLAB" | "BEAM" | "SECTION" | "GRID" | "COLUMN" | "ROOM"
            )
        {
            self.ask(
                name,
                "Specify points (x,y  @dx,dy  @dist<angle, or a distance); Enter ends it:",
                Wants::ToolPoints,
            );
        }
    }

    /// EXPLODE: the symbols selected back into linework.
    /// EXPLODE: what is selected broken into its parts, as AutoCAD's.
    fn cmd_explode(&mut self, ctx: &egui::Context) {
        if self.selection.is_empty() {
            self.pick_tool(ctx, Tool::Select);
            self.ask(
                "EXPLODE",
                "Select objects, then Enter:",
                Wants::SelectThen("EXPLODE"),
            );
            return;
        }
        self.explode_selection();
    }

    /// A command gathering points has them all (typed or clicked): it
    /// does what it does with them.
    fn cmd_tick(&mut self) {
        let Some(Running {
            wants: Wants::Points { n, then },
            ..
        }) = self.cmd.running.clone()
        else {
            return;
        };
        let got = self.plan.cmd_pick.as_ref().map_or(0, |p| p.1.len());
        if n == 0 && then.streams() && got > self.cmd.seen {
            let pts = self
                .plan
                .cmd_pick
                .as_ref()
                .map(|p| p.1.clone())
                .unwrap_or_default();
            let from = self.cmd.seen;
            self.cmd.seen = pts.len();
            self.cmd_stream(then, &pts, from);
        }
        if n > 0 && got >= n {
            let pts = self.plan.cmd_pick.take().map(|p| p.1).unwrap_or_default();
            self.cmd.running = None;
            self.cmd_finish_points(then, &pts);
        }
    }

    /// AutoCAD's text window (F2): every command and answer so far.
    pub(super) fn cmd_history_window(&mut self, ctx: &egui::Context) {
        if !self.cmd.history {
            return;
        }
        let mut open = true;
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        egui::Window::new("Command history")
            .open(&mut open)
            .collapsible(false)
            .constrain_to(area)
            .default_pos(area.left_top() + egui::vec2(40.0, 40.0))
            .default_size(egui::vec2(620.0, 360.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for l in &self.cmd.log {
                            ui.label(egui::RichText::new(l).monospace());
                        }
                    });
            });
        if !open {
            self.cmd.history = false;
        }
    }

    /// The command line, over the foot of the drawings: the last lines it
    /// said and the prompt with what is being typed.
    pub(super) fn cmdline_window(&mut self, ctx: &egui::Context) {
        self.cmd_tick();
        // STRETCH's second point is its window's other corner
        self.plan.pick_window = matches!(
            self.cmd.running.as_ref().map(|r| &r.wants),
            Some(Wants::Points {
                then: Finish::Stretch,
                ..
            })
        ) && self
            .plan
            .cmd_pick
            .as_ref()
            .is_some_and(|(_, p)| p.len() == 1);
        if let Some(t) = self.plan.tool_request.take() {
            self.cmd.running = None;
            self.plan.cmd_pick = None;
            self.pick_tool(ctx, t);
        }
        // a quick right-click: Enter for what runs, else the last command
        if std::mem::take(&mut self.plan.right_quick) && self.plan.modal.is_none() {
            let idle_tool = self.plan.tool != Tool::Select
                && self.plan.details.path.is_empty()
                && self.plan.details.edit.is_none()
                && self.plan.pending.is_none();
            if self.plan.tool == Tool::Detail && self.plan.details.edit.is_some() {
                // in TRIM, EXTEND and the other edits: Enter
                if self.plan.details.choosing_cutters {
                    self.cmd_enter(ctx, "");
                } else {
                    self.cmd.esc = true;
                }
            } else if (self.cmd.running.is_some() && !idle_tool) || self.plan.outline.is_some() {
                self.cmd_enter(ctx, "");
            } else if let Some(c) = self.cmd.last.clone() {
                // nothing being drawn: the last command again, as AutoCAD
                self.cmd.running = None;
                self.cmd_start(ctx, &c);
            } else if self.plan.tool != Tool::Select {
                self.cmd.esc = true;
            }
        }
        if std::mem::take(&mut self.plan.north_request) {
            self.selection.clear();
            self.cmd_start(ctx, "NORTHDIR");
        }
        // Esc ends the command: back to selecting, what is drawn kept, and
        // nothing left selected (the piece just drawn neither), as AutoCAD
        if std::mem::take(&mut self.cmd.esc) && self.plan.modal.is_none() {
            if self.plan.tool != Tool::Select || self.cmd.running.is_some() {
                crate::detailtool::keep(&mut self.doc, &mut self.plan, &mut self.selection);
                self.plan.cmd_pick = None;
                self.plan.cmd_preview = None;
                self.cmd.running = None;
                self.pick_tool(ctx, Tool::Select);
                self.say("*Cancel*");
            }
            self.selection.clear();
            self.plan.boxsel = None;
            // nothing left looking picked: edit mode, the hover, a face
            self.plan.edit = None;
            self.plan.hover = None;
            self.face = None;
            self.plan.nav_mesh = None;
            ctx.request_repaint();
        }
        if !self.shell.settings.command_line {
            return;
        }
        let Some(area) = self.mat.area else { return };
        let width = (area.width() - 120.0).clamp(260.0, 900.0);
        let at = egui::pos2(area.center().x - width / 2.0, area.bottom() - 74.0);
        let dark = self.dark_paper(ctx);
        let (bg, ink, faint) = if dark {
            (
                egui::Color32::from_rgba_unmultiplied(30, 33, 40, 225),
                egui::Color32::from_gray(230),
                egui::Color32::from_gray(150),
            )
        } else {
            (
                egui::Color32::from_rgba_unmultiplied(250, 250, 250, 230),
                egui::Color32::from_gray(25),
                egui::Color32::from_gray(110),
            )
        };
        let prompt = match &self.cmd.running {
            Some(r) => format!("{} {}", r.name, r.prompt),
            None => "Type a command".to_string(),
        };
        let caret = if (ctx.input(|i| i.time) * 2.0) as i64 % 2 == 0 {
            "|"
        } else {
            " "
        };
        egui::Area::new(egui::Id::new("cmdline"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(bg)
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .stroke(egui::Stroke::new(1.0, faint.gamma_multiply(0.5)))
                    .show(ui, |ui| {
                        ui.set_width(width);
                        let font = egui::FontId::monospace(12.0);
                        for l in self.cmd.log.iter().rev().take(2).rev() {
                            ui.label(egui::RichText::new(l).font(font.clone()).color(faint));
                        }
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("▶").font(font.clone()).color(faint));
                            ui.label(
                                egui::RichText::new(format!("{prompt}  {}{caret}", self.cmd.text))
                                    .font(font.clone())
                                    .color(ink),
                            );
                        });
                    });
            });
        // the caret blinks only while something is typed or asked for
        if !self.cmd.text.is_empty() || self.cmd.running.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_find_their_commands_in_english_and_italian() {
        assert_eq!(find("l"), Some("LINE"));
        assert_eq!(find("_LINE"), Some("LINE"));
        assert_eq!(find("tr"), Some("TRIM"));
        assert_eq!(find("TAGLIA"), Some("TRIM"));
        assert_eq!(find("Cerchio"), Some("CIRCLE"));
        assert_eq!(find("co"), Some("COPY"));
        assert_eq!(find("zz"), None);
    }

    #[test]
    fn points_are_read_as_autocad_reads_them() {
        let last = Some(DVec2::new(1.0, 1.0));
        // the project's units are millimetres by default
        let p = parse_point("@1000,0", last).unwrap();
        assert!((p - DVec2::new(2.0, 1.0)).length() < 1e-9);
        let p = parse_point("@1000<90", last).unwrap();
        assert!((p - DVec2::new(1.0, 2.0)).length() < 1e-9);
        let p = parse_point("500,250", last).unwrap();
        assert!((p - DVec2::new(0.5, 0.25)).length() < 1e-9);
        assert_eq!(parse_point("@", last), None);
        assert_eq!(parse_point("@0,0", last), last);
        assert!(parse_point("@1000,0", None).is_none());
        assert!(parse_point("hello", last).is_none());
    }

    #[test]
    fn points_typed_in_a_ucs_are_read_in_it() {
        use crate::planview::Ucs;
        // a UCS at (1, 1) turned 90°: its X runs along the world's Y
        let u = Ucs {
            origin: DVec2::new(1.0, 1.0),
            angle: std::f64::consts::FRAC_PI_2,
        };
        let p = parse_point_in("1000,0", None, u).unwrap();
        assert!((p - DVec2::new(1.0, 2.0)).length() < 1e-9);
        let p = parse_point_in("@1000,0", Some(DVec2::new(1.0, 2.0)), u).unwrap();
        assert!((p - DVec2::new(1.0, 3.0)).length() < 1e-9);
        let p = parse_point_in("@1000<90", Some(DVec2::ZERO), u).unwrap();
        assert!((p - DVec2::new(-1.0, 0.0)).length() < 1e-9);
        // a star types in the world's coordinates
        let p = parse_point_in("*1000,0", None, u).unwrap();
        assert!((p - DVec2::new(1.0, 0.0)).length() < 1e-9);
        assert!((u.local(u.world(DVec2::new(3.0, -2.0))) - DVec2::new(3.0, -2.0)).length() < 1e-9);
    }

    #[test]
    fn a_circle_through_three_points_and_an_outline_s_area() {
        let (o, r) = circle_through(
            DVec2::new(1.0, 0.0),
            DVec2::new(0.0, 1.0),
            DVec2::new(-1.0, 0.0),
        )
        .unwrap();
        assert!(o.length() < 1e-9 && (r - 1.0).abs() < 1e-9);
        let (a, p) = area_of(&[
            DVec2::ZERO,
            DVec2::new(2.0, 0.0),
            DVec2::new(2.0, 3.0),
            DVec2::new(0.0, 3.0),
        ]);
        assert!((a - 6.0).abs() < 1e-9 && (p - 10.0).abs() < 1e-9);
    }
}
