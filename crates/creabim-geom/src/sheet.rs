//! Sheets: views laid out on paper with a border and title block, exported
//! as SVG or a multi-page PDF. The same layout drives the app's sheet view.

use crate::paper::{self, Anchor, Canvas, PaperMap, PdfWriter, Stroke, SvgCanvas};
use crate::section::{view_drawing, Drawing};
use creabim_core::titleblock::{fill_fields, TextAlign, TitleItem};
use creabim_core::{ElementId, ElementKind, Model, Sheet, SheetTemplate, View, Viewport};
use glam::DVec2;
use std::collections::BTreeMap;

/// Text for the title block that does not live on the sheet itself.
#[derive(Clone, Debug, Default)]
pub struct SheetInfo {
    pub project: String,
    pub date: String,
}

/// A viewport resolved for drawing: its view's live drawing and where it lands.
#[derive(Clone, Debug)]
pub struct PlacedView {
    pub viewport: ElementId,
    pub view: ElementId,
    /// Position on the sheet, 1-based, in model order.
    pub number: usize,
    pub title: String,
    pub map: PaperMap,
    pub drawing: Drawing,
    /// The pens it is drawn with: its view's line weights.
    pub weights: creabim_core::LineWeights,
    /// Those pens with the object styles, patterns and overrides of the
    /// project's style pack and the view.
    pub looks: std::sync::Arc<crate::looks::Looks>,
    /// The drawing's extent on paper, mm.
    pub bounds: (DVec2, DVec2),
    /// How its title reads (MU27).
    pub title_look: creabim_core::SheetTitle,
    /// Its title's text style, resolved (MU28).
    pub title_text: crate::annotation::TextLook,
}

impl PlacedView {
    /// Where the view title sits, below the drawing.
    pub fn title_at(&self) -> DVec2 {
        DVec2::new(self.bounds.0.x, self.bounds.0.y - 7.0) + self.title_look.offset
    }

    /// The title's extent on paper, to pick and drag it.
    pub fn title_box(&self) -> Option<(DVec2, DVec2)> {
        if self.title_look.hidden || self.map.scale == 1000.0 {
            return None;
        }
        let at = self.title_at();
        let s = self.title_look.size;
        let w = 9.0 + paper::text_width(&self.title, s).max(30.0);
        Some((at - DVec2::new(0.5, s * 1.4), at + DVec2::new(w, s * 1.2)))
    }
}

/// Today's date as YYYY-MM-DD (UTC), for title blocks.
pub fn today() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    // days since 1970-01-01 to a civil date (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

pub const MARGIN: f64 = 10.0;
/// Extra margin on the binding (left) edge.
pub const BINDING: f64 = 20.0;
pub const TITLE_BLOCK: DVec2 = DVec2::new(170.0, 45.0);

pub fn sheet_of(model: &Model, id: ElementId) -> Option<&Sheet> {
    match &model.get(id)?.kind {
        ElementKind::Sheet(s) => Some(s),
        _ => None,
    }
}

fn view_of(model: &Model, id: ElementId) -> Option<&View> {
    match &model.get(id)?.kind {
        ElementKind::View(v) => Some(v),
        _ => None,
    }
}

/// Every sheet in the model, ordered by sheet number.
pub fn sheets(model: &Model) -> Vec<(ElementId, &Sheet)> {
    let mut v: Vec<(ElementId, &Sheet)> = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Sheet(s) => Some((e.id, s)),
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.1.number.cmp(&b.1.number));
    v
}

/// The viewports on a sheet with their drawings. `drawing` supplies each
/// view's drawing, so callers can cache them.
pub fn placed_views(
    model: &Model,
    sheet: ElementId,
    drawing: &mut dyn FnMut(ElementId) -> Option<Drawing>,
) -> Vec<PlacedView> {
    placed_views_with(model, sheet, drawing, &mut |id, v| {
        std::sync::Arc::new(crate::looks::Looks::for_view(model, id, v))
    })
}

/// [`placed_views`] with `looks` supplying each view's looks (given the
/// view as its template resolves it), so callers can cache those too.
pub fn placed_views_with(
    model: &Model,
    sheet: ElementId,
    drawing: &mut dyn FnMut(ElementId) -> Option<Drawing>,
    looks: &mut dyn FnMut(ElementId, &View) -> std::sync::Arc<crate::looks::Looks>,
) -> Vec<PlacedView> {
    let mut out = Vec::new();
    for e in model.iter() {
        let ElementKind::Viewport(vp) = &e.kind else {
            continue;
        };
        if vp.sheet != sheet {
            continue;
        }
        let (Some(view), Some(mut d)) = (view_of(model, vp.view), drawing(vp.view)) else {
            continue;
        };
        // its scale and pens as its view template gives them
        let view = creabim_core::styles::resolved(model, view);
        let looks = looks(vp.view, &view);
        if let Some([lo, hi]) = vp.crop {
            d = crate::crop::crop(&d, lo, hi);
        }
        let map = PaperMap {
            model: vp.target,
            paper: vp.center,
            scale: view.paper_scale(),
        };
        let bounds = d
            .bounds()
            .map(|(lo, hi)| (map.to_paper(lo), map.to_paper(hi)))
            .unwrap_or((vp.center, vp.center));
        out.push(PlacedView {
            viewport: e.id,
            view: vp.view,
            number: out.len() + 1,
            title: Some(view.sheet_title.text.trim())
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .or_else(|| model.get(vp.view).map(|v| v.name.clone()))
                .unwrap_or_default(),
            title_look: (*view.sheet_title).clone(),
            title_text: {
                let mut l = crate::annotate::look_of(model, view.sheet_title.style, None);
                l.size = view.sheet_title.size;
                l
            },
            map,
            drawing: d,
            weights: looks.weights,
            looks,
            bounds,
        });
    }
    out
}

/// A viewport for `view` centred on `at` (sheet mm), pinning the middle of
/// the view's current drawing there.
pub fn new_viewport(model: &Model, sheet: ElementId, view: ElementId, at: DVec2) -> Viewport {
    let target = view_drawing(model, view)
        .and_then(|d| d.bounds())
        .map_or(DVec2::ZERO, |(lo, hi)| (lo + hi) / 2.0);
    Viewport {
        sheet,
        view,
        center: at,
        target,
        crop: None,
    }
}

/// A spot on the sheet where `view` fits without covering the views
/// already there or the title block; the middle of the sheet if none does.
pub fn free_spot(model: &Model, sheet: ElementId, view: ElementId) -> DVec2 {
    let Some(s) = sheet_of(model, sheet) else {
        return DVec2::ZERO;
    };
    let middle = DVec2::new(s.width, s.height) / 2.0;
    let (Some(v), Some((lo, hi))) = (
        view_of(model, view),
        view_drawing(model, view).and_then(|d| d.bounds()),
    ) else {
        return middle;
    };
    let k = 1000.0 / v.paper_scale();
    // half size on paper, with room for the title below
    let half = (hi - lo) * k / 2.0 + DVec2::new(4.0, 6.0);
    let mut taken: Vec<(DVec2, DVec2)> =
        placed_views(model, sheet, &mut |v| view_drawing(model, v))
            .iter()
            .map(|p| {
                (
                    p.bounds.0 - DVec2::new(4.0, 14.0),
                    p.bounds.1 + DVec2::splat(4.0),
                )
            })
            .collect();
    taken.push(title_block_bounds(model, s));
    let (x0, x1) = (BINDING + half.x + 5.0, s.width - MARGIN - half.x - 5.0);
    let (y0, y1) = (MARGIN + half.y + 10.0, s.height - MARGIN - half.y - 5.0);
    let mut y = y1;
    while y >= y0 {
        let mut x = x0;
        while x <= x1 {
            let c = DVec2::new(x, y);
            let (a, b) = (c - half - DVec2::new(0.0, 8.0), c + half);
            if taken
                .iter()
                .all(|(p, q)| b.x < p.x || a.x > q.x || b.y < p.y || a.y > q.y)
            {
                return c;
            }
            x += 10.0;
        }
        y -= 10.0;
    }
    middle
}

/// The template a sheet's title block comes from, if it has one that
/// still exists.
pub fn template_of<'a>(model: &'a Model, sheet: &Sheet) -> Option<&'a SheetTemplate> {
    match &model.get(sheet.template?)?.kind {
        ElementKind::SheetTemplate(t) => Some(t),
        _ => None,
    }
}

/// The values a sheet's title block fields read: the built-in ones, then
/// its template's own (their defaults, unless the sheet sets them).
pub fn field_values(
    sheet: &Sheet,
    template: Option<&SheetTemplate>,
    title: &str,
    scales: &str,
    info: &SheetInfo,
) -> BTreeMap<String, String> {
    let mut v = BTreeMap::new();
    if let Some(t) = template {
        for (k, d) in &t.fields {
            v.insert(k.clone(), d.clone());
        }
    }
    for (k, val) in &sheet.fields {
        v.insert(k.clone(), val.clone());
    }
    v.insert("sheet".into(), title.to_string());
    v.insert("number".into(), sheet.number.clone());
    v.insert("project".into(), info.project.clone());
    v.insert("scale".into(), scales.to_string());
    v.insert("date".into(), info.date.clone());
    v
}

/// A sheet's title-block values with the project's information (A7-5)
/// filled in: over the template's defaults, under the sheet's own.
pub fn frame_values(
    model: &Model,
    sheet: &Sheet,
    template: Option<&SheetTemplate>,
    title: &str,
    scales: &str,
    info: &SheetInfo,
) -> BTreeMap<String, String> {
    let mut values = field_values(sheet, template, title, scales, info);
    if let Some((_, project)) = model.project_info() {
        for (k, v) in project.fields() {
            if !sheet.fields.contains_key(k) {
                values.insert(k.to_string(), v);
            }
        }
    }
    values
}

/// Where a sheet's title block sits, mm from its lower left: the
/// template's drawing, or the built-in block.
pub fn title_block_bounds(model: &Model, sheet: &Sheet) -> (DVec2, DVec2) {
    if let Some((lo, hi)) = template_of(model, sheet).and_then(|t| {
        let o = t.corner.at(sheet.width, sheet.height);
        t.bounds().map(|(lo, hi)| (lo + o, hi + o))
    }) {
        return (lo, hi);
    }
    let lo = DVec2::new(sheet.width - MARGIN - TITLE_BLOCK.x, MARGIN);
    (lo, lo + TITLE_BLOCK)
}

/// A template's title block on a sheet, its fields filled in.
pub fn draw_template(
    c: &mut dyn Canvas,
    sheet: &Sheet,
    t: &SheetTemplate,
    values: &BTreeMap<String, String>,
) {
    let o = t.corner.at(sheet.width, sheet.height);
    if t.border {
        // where its margins put it (MU40: the border stretches)
        let (lo, hi) = t.border_box(DVec2::new(sheet.width, sheet.height));
        c.rect(lo, hi, Stroke::solid(0.5, 0.0));
    }
    // a logo is drawn as the shapes it holds, laid over its box
    let laid: Vec<TitleItem> = t
        .items
        .iter()
        .flat_map(|it| match it {
            TitleItem::Logo { .. } => it.logo_shapes(),
            _ => vec![it.clone()],
        })
        .collect();
    for it in &laid {
        match it {
            TitleItem::Line { points, width } => {
                let pts: Vec<DVec2> = points.iter().map(|p| *p + o).collect();
                c.line(&pts, Stroke::solid(*width, 0.0));
            }
            TitleItem::Fill { points, gray } => {
                let pts: Vec<DVec2> = points.iter().map(|p| *p + o).collect();
                c.fill(&pts, *gray);
            }
            TitleItem::Text {
                at,
                size,
                text,
                align,
                angle,
            } => {
                let anchor = match align {
                    TextAlign::Start => Anchor::Start,
                    TextAlign::Middle => Anchor::Middle,
                    TextAlign::End => Anchor::End,
                };
                let text = fill_fields(text, values);
                c.text_angle(*at + o, *size, &text, anchor, *angle);
            }
            TitleItem::Shape {
                points,
                closed,
                pen,
                colour,
                fill,
                opacity,
            } => {
                let mut pts: Vec<DVec2> = points.iter().map(|p| *p + o).collect();
                // see-through on white paper: the colour mixed toward white
                let wash = |c: [f32; 3]| c.map(|v| 1.0 - (1.0 - v) * *opacity as f32);
                if let (Some(f), true) = (fill, pts.len() >= 3) {
                    c.fill_rgb(&pts, wash(*f));
                }
                if let Some(w) = pen {
                    if *closed && pts.len() >= 2 && pts.first() != pts.last() {
                        pts.push(pts[0]);
                    }
                    c.line_rgb(&pts, Stroke::solid(*w, 0.0), wash(*colour));
                }
            }
            TitleItem::Image {
                lo,
                hi,
                width,
                height,
                rgba,
                opacity,
            } => {
                if let Some(r) = crate::raster::stored_image(*width, *height, rgba) {
                    let r = crate::raster::faded(&r, *opacity);
                    c.image(*lo + o, *hi + o, None, &r);
                }
            }
            TitleItem::Logo { .. } => {}
        }
    }
}

/// Border and title block: the sheet's template, or the built-in block.
pub fn draw_frame(
    c: &mut dyn Canvas,
    model: &Model,
    sheet: &Sheet,
    title: &str,
    scales: &str,
    info: &SheetInfo,
) {
    // the project's information (A7-5): over the template's defaults and
    // the file's name, under what this sheet sets itself
    let project = model
        .project_info()
        .map(|(_, p)| p.clone())
        .unwrap_or_default();
    let named;
    let info = if project.name.trim().is_empty() {
        info
    } else {
        named = SheetInfo {
            project: project.name.trim().to_string(),
            date: info.date.clone(),
        };
        &named
    };
    match template_of(model, sheet) {
        Some(t) => {
            let values = frame_values(model, sheet, Some(t), title, scales, info);
            draw_template(c, sheet, t, &values);
        }
        None => draw_builtin_frame(c, sheet, title, scales, info),
    }
}

/// The built-in border and title block.
pub fn draw_builtin_frame(
    c: &mut dyn Canvas,
    sheet: &Sheet,
    title: &str,
    scales: &str,
    info: &SheetInfo,
) {
    let (w, h) = (sheet.width, sheet.height);
    let lo = DVec2::new(BINDING, MARGIN);
    let hi = DVec2::new(w - MARGIN, h - MARGIN);
    c.rect(lo, hi, Stroke::solid(0.5, 0.0));

    let thin = Stroke::solid(0.18, 0.0);
    let tb_lo = DVec2::new(hi.x - TITLE_BLOCK.x, lo.y);
    let tb_hi = DVec2::new(hi.x, lo.y + TITLE_BLOCK.y);
    c.rect(tb_lo, tb_hi, Stroke::solid(0.35, 0.0));
    // rows from the top: project, sheet title, then number / scale / date
    let row = |k: f64| tb_hi.y - k;
    c.line(
        &[
            DVec2::new(tb_lo.x, row(12.0)),
            DVec2::new(tb_hi.x, row(12.0)),
        ],
        thin,
    );
    c.line(
        &[
            DVec2::new(tb_lo.x, row(27.0)),
            DVec2::new(tb_hi.x, row(27.0)),
        ],
        thin,
    );
    let split = tb_lo.x + 70.0;
    c.line(
        &[DVec2::new(split, tb_lo.y), DVec2::new(split, row(27.0))],
        thin,
    );
    let x = tb_lo.x + 3.0;
    let small = 2.0;
    c.text(DVec2::new(x, row(3.5)), small, "PROJECT", Anchor::Start);
    c.text(DVec2::new(x, row(8.5)), 4.0, &info.project, Anchor::Start);
    c.text(DVec2::new(x, row(15.5)), small, "SHEET", Anchor::Start);
    c.text(DVec2::new(x, row(21.5)), 5.0, title, Anchor::Start);
    c.text(DVec2::new(x, row(30.5)), small, "NUMBER", Anchor::Start);
    c.text(DVec2::new(x, row(38.5)), 8.0, &sheet.number, Anchor::Start);
    let x2 = split + 3.0;
    c.text(DVec2::new(x2, row(30.5)), small, "SCALE", Anchor::Start);
    c.text(DVec2::new(x2, row(35.5)), 3.0, scales, Anchor::Start);
    c.text(DVec2::new(x2, row(40.5)), small, &info.date, Anchor::Start);
    c.text(
        DVec2::new(tb_hi.x - 3.0, row(40.5)),
        small,
        "CreaBIM",
        Anchor::End,
    );
}

/// Records what is drawn on it as title block items, from `origin`.
struct Recorder {
    origin: DVec2,
    items: Vec<TitleItem>,
}

impl Canvas for Recorder {
    fn line(&mut self, pts: &[DVec2], stroke: Stroke) {
        self.items.push(TitleItem::Line {
            points: pts.iter().map(|p| *p - self.origin).collect(),
            width: stroke.width,
        });
    }

    fn fill(&mut self, pts: &[DVec2], gray: f64) {
        self.items.push(TitleItem::Fill {
            points: pts.iter().map(|p| *p - self.origin).collect(),
            gray,
        });
    }

    fn text_angle(&mut self, at: DVec2, size: f64, text: &str, anchor: Anchor, angle: f64) {
        self.items.push(TitleItem::Text {
            at: at - self.origin,
            size,
            text: text.to_string(),
            align: match anchor {
                Anchor::Start => TextAlign::Start,
                Anchor::Middle => TextAlign::Middle,
                Anchor::End => TextAlign::End,
            },
            angle,
        });
    }

    fn image(
        &mut self,
        _lo: DVec2,
        _hi: DVec2,
        _clip: Option<(DVec2, DVec2)>,
        _raster: &std::sync::Arc<crate::raster::Raster>,
    ) {
    }
}

/// The built-in title block as a template to change: its lines and text,
/// with fields where it writes the sheet's own values.
pub fn builtin_template() -> SheetTemplate {
    let sheet = Sheet::a("{number}", "A3");
    let info = SheetInfo {
        project: "{project}".into(),
        date: "{date}".into(),
    };
    let mut rec = Recorder {
        origin: DVec2::new(sheet.width, 0.0),
        items: vec![],
    };
    draw_builtin_frame(&mut rec, &sheet, "{sheet}", "{scale}", &info);
    // the border is the template's own switch
    rec.items.remove(0);
    SheetTemplate {
        corner: creabim_core::titleblock::Corner::BottomRight,
        items: rec.items,
        border: true,
        fields: vec![],
        paper: None,
        guides: vec![],
        margins: None,
    }
}

/// The scale text for a title block: one scale, or "As indicated".
pub fn scale_text(model: &Model, views: &[PlacedView]) -> String {
    let mut scales: Vec<i64> = views
        .iter()
        .filter_map(|v| view_of(model, v.view))
        .filter(|v| !matches!(v.kind, creabim_core::ViewKind::Schedule { .. }))
        .map(|v| v.scale.round() as i64)
        .collect();
    scales.sort();
    scales.dedup();
    match scales.as_slice() {
        [] => "-".into(),
        [s] => format!("1:{s}"),
        _ => "As indicated".into(),
    }
}

/// A view's number bubble, title and scale under its drawing.
pub fn draw_view_title(c: &mut dyn Canvas, v: &PlacedView) {
    // a schedule carries its own title
    if v.map.scale == 1000.0 {
        return;
    }
    let t = &v.title_look;
    if t.hidden {
        return;
    }
    let at = v.title_at();
    let k = t.size / 3.5;
    let thin = Stroke::solid(0.18, 0.0);
    let x0 = if t.show_number {
        c.circle(at + DVec2::new(3.5 * k, 0.0), 3.5 * k, thin);
        c.text(
            at + DVec2::new(3.5 * k, 0.0),
            3.0 * k,
            &v.number.to_string(),
            Anchor::Middle,
        );
        9.0 * k
    } else {
        0.0
    };
    c.styled_text(
        at + DVec2::new(x0, 1.2 * k),
        &v.title,
        Anchor::Start,
        0.0,
        &v.title_text,
    );
    let width = paper::text_width(&v.title, t.size).max(30.0 * k);
    c.line(
        &[at + DVec2::new(x0, -k), at + DVec2::new(x0 + width, -k)],
        Stroke::solid(0.35, 0.0),
    );
    if t.show_scale {
        c.text(
            at + DVec2::new(x0, -3.5 * k),
            2.5 * k,
            &format!("1:{:.0}", v.map.scale),
            Anchor::Start,
        );
    }
}

/// Draw a whole sheet: frame, every view and its title.
pub fn draw_sheet(
    c: &mut dyn Canvas,
    model: &Model,
    sheet: ElementId,
    views: &[PlacedView],
    info: &SheetInfo,
) {
    let Some(s) = sheet_of(model, sheet) else {
        return;
    };
    let title = model.get(sheet).map(|e| e.name.as_str()).unwrap_or("");
    draw_frame(c, model, s, title, &scale_text(model, views), info);
    for v in views {
        paper::draw_looks(c, &v.drawing, &v.map, &v.looks);
        draw_view_title(c, v);
    }
    draw_sheet_notes(c, model, sheet);
}

/// Paper millimetres as drawing units, for what is drawn on a sheet itself.
pub const PAPER: PaperMap = PaperMap {
    model: DVec2::ZERO,
    paper: DVec2::ZERO,
    scale: 1000.0,
};

/// The notes written on the sheet itself, outside any view.
pub fn draw_sheet_notes(c: &mut dyn Canvas, model: &Model, sheet: ElementId) {
    // linework drawn on the paper, then the notes over it
    let mut items = crate::annotate::sheet_linework(model, sheet);
    items.extend(crate::annotate::sheet_notes(model, sheet));
    let notes = crate::PlanView { items };
    paper::draw(c, &notes, &PAPER);
    draw_paper_marks(c, model, sheet);
}

/// Where a sheet's title block is, as drawn (its template's or the built-in
/// one): the box round what its frame draws in the lower half of the
/// sheet, inside the border (MU22). Paper marks are placed clear of it.
pub fn title_block_box(model: &Model, sheet: ElementId) -> Option<(DVec2, DVec2)> {
    struct Points(Vec<DVec2>);
    impl Canvas for Points {
        fn line(&mut self, pts: &[DVec2], _: Stroke) {
            self.0.extend_from_slice(pts);
        }
        fn fill(&mut self, pts: &[DVec2], _: f64) {
            self.0.extend_from_slice(pts);
        }
        fn text_angle(&mut self, at: DVec2, size: f64, _: &str, _: Anchor, _: f64) {
            self.0.push(at);
            self.0.push(at + DVec2::new(0.0, size));
        }
        fn image(
            &mut self,
            lo: DVec2,
            hi: DVec2,
            _: Option<(DVec2, DVec2)>,
            _: &std::sync::Arc<crate::raster::Raster>,
        ) {
            self.0.push(lo);
            self.0.push(hi);
        }
    }
    let s = sheet_of(model, sheet)?;
    let mut c = Points(vec![]);
    let title = model.get(sheet).map(|e| e.name.as_str()).unwrap_or("");
    draw_frame(&mut c, model, s, title, "1:100", &SheetInfo::default());
    let (w, h) = (s.width, s.height);
    let on_border = |p: &DVec2| {
        (p.x - BINDING).abs() < 0.05
            || (p.x - (w - MARGIN)).abs() < 0.05
            || (p.y - MARGIN).abs() < 0.05
            || (p.y - (h - MARGIN)).abs() < 0.05
            || p.x <= 0.05
            || p.y <= 0.05
            || p.x >= w - 0.05
            || p.y >= h - 0.05
    };
    let inside: Vec<DVec2> =
        c.0.into_iter()
            .filter(|p| p.y < h * 0.5 && !on_border(p))
            .collect();
    let first = *inside.first()?;
    let (lo, hi) = inside
        .iter()
        .fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    Some((lo, hi))
}

/// The north arrows and scale bars on a sheet (MU22).
pub fn draw_paper_marks(c: &mut dyn Canvas, model: &Model, sheet: ElementId) {
    let north = model.project_info().map_or(0.0, |(_, p)| p.true_north);
    // the sheet's revisions, for its table
    let mut revs: Vec<(String, String, String)> = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::PaperMark(m) if m.sheet == sheet => match &m.kind {
                creabim_core::PaperMarkKind::Cloud {
                    rev, date, what, ..
                } => Some((rev.clone(), date.clone(), what.clone())),
                _ => None,
            },
            _ => None,
        })
        .collect();
    revs.sort();
    revs.dedup_by(|a, b| a.0 == b.0);
    for e in model.iter() {
        if let ElementKind::PaperMark(m) = &e.kind {
            if m.sheet == sheet {
                if matches!(m.kind, creabim_core::PaperMarkKind::RevisionTable) {
                    draw_revision_table(c, m, &revs);
                } else {
                    draw_paper_mark(c, m, north);
                }
            }
        }
    }
}

/// The revisions table: REV, DATE, DESCRIPTION, a row per revision.
fn draw_revision_table(
    c: &mut dyn Canvas,
    m: &creabim_core::PaperMark,
    revs: &[(String, String, String)],
) {
    let t = m.size;
    let row = t * 2.4;
    let cols = [0.0, 14.0, 40.0, 110.0];
    let thin = Stroke::solid(0.18, 0.0);
    let rows = revs.len() + 1;
    let top = m.at.y + row * rows as f64;
    for i in 0..=rows {
        let y = m.at.y + row * i as f64;
        c.line(
            &[DVec2::new(m.at.x, y), DVec2::new(m.at.x + cols[3], y)],
            thin,
        );
    }
    for x in cols {
        c.line(
            &[DVec2::new(m.at.x + x, m.at.y), DVec2::new(m.at.x + x, top)],
            thin,
        );
    }
    let cell = |c: &mut dyn Canvas, r: usize, col: usize, s: &str| {
        let y = top - row * (r as f64 + 1.0) + row * 0.32;
        c.text(DVec2::new(m.at.x + cols[col] + 1.5, y), t, s, Anchor::Start);
    };
    cell(c, 0, 0, "REV");
    cell(c, 0, 1, "DATE");
    cell(c, 0, 2, "DESCRIPTION");
    for (i, (rev, date, what)) in revs.iter().enumerate() {
        cell(c, i + 1, 0, rev);
        cell(c, i + 1, 1, date);
        cell(c, i + 1, 2, what);
    }
}

/// One paper mark, in sheet millimetres.
pub fn draw_paper_mark(c: &mut dyn Canvas, m: &creabim_core::PaperMark, true_north: f64) {
    use creabim_core::PaperMarkKind;
    match &m.kind {
        PaperMarkKind::Picture {
            width,
            height,
            rgba,
            opacity,
            ..
        } => {
            let (lo, hi) = m.bounds();
            if let Some(r) = crate::raster::stored_image(*width, *height, rgba) {
                c.image(lo, hi, None, &crate::raster::faded(&r, *opacity));
            }
            return;
        }
        PaperMarkKind::Vector { shapes, .. } => {
            // drawn as a master's logo is, from the sheet's lower left
            let (lo, hi) = m.bounds();
            let t = SheetTemplate {
                corner: creabim_core::titleblock::Corner::BottomLeft,
                items: vec![TitleItem::Logo {
                    lo,
                    hi,
                    shapes: shapes.clone(),
                }],
                border: false,
                fields: vec![],
                paper: None,
                guides: vec![],
                margins: None,
            };
            draw_template(c, &Sheet::a("", "A3"), &t, &BTreeMap::new());
            return;
        }
        _ => {}
    }
    match &m.kind {
        PaperMarkKind::NorthArrow { .. } => {
            let s = m.size;
            let turn = DVec2::from_angle(m.north_angle(true_north).to_radians());
            let at = |p: DVec2| m.at + turn.rotate(p);
            // the arrow inside its circle, the tip just touching it (MU29)
            let tip = at(DVec2::new(0.0, 0.40 * s));
            let notch = at(DVec2::new(0.0, -0.14 * s));
            let left = at(DVec2::new(-0.21 * s, -0.31 * s));
            let right = at(DVec2::new(0.21 * s, -0.31 * s));
            c.circle(m.at, 0.42 * s, Stroke::solid(0.25, 0.0));
            // half black, half white, as drawn by hand
            c.fill(&[tip, notch, left], 0.0);
            c.polygon(&[tip, right, notch, left], Stroke::solid(0.3, 0.0));
            c.text(
                at(DVec2::new(0.0, 0.42 * s + 3.0)),
                3.5,
                "N",
                Anchor::Middle,
            );
        }
        PaperMarkKind::Cloud { rev, arc, .. } => {
            // scallops of about `arc` mm round the outline, bulging out
            let corners = m.cloud_outline();
            let stroke = Stroke::solid(0.35, 0.0);
            let ink = |c: &mut dyn Canvas, pts: &[DVec2]| match m.color {
                Some(rgb) => c.line_rgb(pts, stroke, rgb),
                None => c.line(pts, stroke),
            };
            let k = corners.len();
            for q in 0..k {
                let (a, b) = (corners[q], corners[(q + 1) % k]);
                let len = a.distance(b);
                if len < 1e-6 {
                    continue;
                }
                let n = (len / arc.max(1.0)).ceil().max(1.0) as usize;
                let dir = (b - a) / len;
                let out = DVec2::new(dir.y, -dir.x);
                for i in 0..n {
                    let (s, e) = (
                        a + (b - a) * (i as f64 / n as f64),
                        a + (b - a) * ((i + 1) as f64 / n as f64),
                    );
                    let mid = (s + e) / 2.0;
                    let r = s.distance(e) / 2.0;
                    let pts: Vec<DVec2> = (0..=8)
                        .map(|j| {
                            let t = std::f64::consts::PI * j as f64 / 8.0;
                            mid - dir * r * t.cos() + out * r * t.sin()
                        })
                        .collect();
                    ink(c, &pts);
                }
            }
            let hi = corners
                .iter()
                .fold(DVec2::splat(f64::MIN), |a, p| a.max(*p));
            // its revision in a triangle at the top right
            let tip = hi + DVec2::new(4.0, 4.0);
            let s = m.size * 1.6;
            let tri = [
                tip + DVec2::new(-s, -s * 0.6),
                tip + DVec2::new(s, -s * 0.6),
                tip + DVec2::new(0.0, s * 1.1),
            ];
            c.polygon(&tri, Stroke::solid(0.3, 0.0));
            c.text(
                tip + DVec2::new(0.0, -s * 0.25),
                m.size,
                rev,
                Anchor::Middle,
            );
        }
        PaperMarkKind::RevisionTable
        | PaperMarkKind::Picture { .. }
        | PaperMarkKind::Vector { .. } => {}
        PaperMarkKind::ScaleBar {
            scale,
            length,
            parts,
        } => {
            let parts = (*parts).max(1) as usize;
            let w = length * 1000.0 / scale.max(1.0);
            let h = m.size;
            let seg = w / parts as f64;
            let o = m.at;
            for i in 0..parts {
                let x0 = o.x + seg * i as f64;
                let box_ = [
                    DVec2::new(x0, o.y),
                    DVec2::new(x0 + seg, o.y),
                    DVec2::new(x0 + seg, o.y + h),
                    DVec2::new(x0, o.y + h),
                ];
                if i % 2 == 0 {
                    c.fill(&box_, 0.0);
                }
                c.polygon(&box_, Stroke::solid(0.2, 0.0));
            }
            let step = length / parts as f64;
            let num = |v: f64| {
                if (v - v.round()).abs() < 1e-6 {
                    format!("{v:.0}")
                } else {
                    format!("{v:.1}")
                }
            };
            for i in 0..=parts {
                let x = o.x + seg * i as f64;
                let t = if i == parts {
                    format!("{} m", num(step * i as f64))
                } else {
                    num(step * i as f64)
                };
                c.text(DVec2::new(x, o.y + h + 1.2), 2.2, &t, Anchor::Middle);
            }
            c.text(
                DVec2::new(o.x, o.y - 3.6),
                2.2,
                &format!("1:{:.0}", scale),
                Anchor::Start,
            );
        }
    }
}

pub fn sheet_svg(model: &Model, sheet: ElementId, info: &SheetInfo) -> Option<String> {
    let s = sheet_of(model, sheet)?;
    let views = placed_views(model, sheet, &mut |v| view_drawing(model, v));
    let mut svg = SvgCanvas::new(DVec2::new(s.width, s.height));
    draw_sheet(&mut svg, model, sheet, &views, info);
    Some(svg.finish())
}

/// A sheet as DXF for CAD programs, in paper millimetres.
pub fn sheet_dxf(model: &Model, sheet: ElementId, info: &SheetInfo) -> Option<String> {
    sheet_of(model, sheet)?;
    let views = placed_views(model, sheet, &mut |v| view_drawing(model, v));
    let mut dxf = crate::dxf::DxfCanvas::new(1.0);
    draw_sheet(&mut dxf, model, sheet, &views, info);
    Some(dxf.finish())
}

/// All sheets as one PDF, a page per sheet in sheet-number order.
pub fn sheets_pdf(model: &Model, info: &SheetInfo) -> Vec<u8> {
    let all: Vec<ElementId> = sheets(model).into_iter().map(|(id, _)| id).collect();
    sheets_pdf_of(model, info, &all)
}

/// The sheets `ids` as one PDF, a page each in sheet-number order (MU22).
pub fn sheets_pdf_of(model: &Model, info: &SheetInfo, ids: &[ElementId]) -> Vec<u8> {
    let mut pdf = PdfWriter::new();
    for (id, s) in sheets(model).into_iter().filter(|(id, _)| ids.contains(id)) {
        let views = placed_views(model, id, &mut |v| view_drawing(model, v));
        let mut page = pdf.page(DVec2::new(s.width, s.height));
        draw_sheet(&mut page, model, id, &views, info);
    }
    pdf.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::sample::sample_house;

    #[test]
    fn project_information_fills_the_title_block_under_the_sheets_own() {
        let mut m = sample_house();
        let info = creabim_core::ProjectInfo {
            name: "Maple Lane House".into(),
            client: "A. Client".into(),
            status: "Tender".into(),
            ..Default::default()
        };
        let mut doc = creabim_core::Document::from_model(m.clone());
        doc.commit(creabim_core::set_project(doc.model(), info))
            .unwrap();
        m = doc.model().clone();
        let t = SheetTemplate {
            corner: Default::default(),
            items: vec![],
            border: true,
            fields: vec![
                ("client".into(), "Client".into()),
                ("status".into(), "Draft".into()),
            ],
            paper: None,
            guides: vec![],
            margins: None,
        };
        let (_, s) = sheets(&m).into_iter().next().unwrap();
        let mut s = s.clone();
        s.fields.insert("status".into(), "Construction".into());
        let base = SheetInfo {
            project: "Starter house".into(),
            date: String::new(),
        };
        let v = frame_values(&m, &s, Some(&t), "Plan", "1:100", &base);
        assert_eq!(
            v["client"], "A. Client",
            "the project's client over the template's default"
        );
        assert_eq!(v["status"], "Construction", "the sheet's own value wins");
        assert_eq!(v["job"], "Maple Lane House");
    }

    #[test]
    fn sample_sheets_place_their_views_inside_the_border() {
        let m = sample_house();
        let all = sheets(&m);
        let numbers: Vec<&str> = all.iter().map(|(_, s)| s.number.as_str()).collect();
        assert_eq!(numbers, ["A001", "A101", "A201", "A601"]);
        for (id, s) in all {
            let views = placed_views(&m, id, &mut |v| view_drawing(&m, v));
            assert!(!views.is_empty());
            for v in &views {
                let (lo, hi) = v.bounds;
                assert!(
                    lo.x >= BINDING
                        && lo.y >= MARGIN
                        && hi.x <= s.width - MARGIN
                        && hi.y <= s.height - MARGIN,
                    "{} on {} spills off the sheet: {lo} {hi}",
                    v.title,
                    s.number
                );
            }
        }
    }

    #[test]
    fn free_spot_avoids_views_already_placed() {
        let mut doc = creabim_core::Document::from_model(sample_house());
        let plan = |name: &str, m: &Model| m.iter().find(|e| e.name == name).unwrap().id;
        let (g, f) = (
            plan("Ground Floor plan", doc.model()),
            plan("First Floor plan", doc.model()),
        );
        let sheet = creabim_core::Element::new("Empty", ElementKind::Sheet(Sheet::a("X1", "A1")));
        let sid = sheet.id;
        let vp = new_viewport(doc.model(), sid, g, DVec2::new(200.0, 400.0));
        doc.commit(
            creabim_core::Transaction::new("t")
                .insert(sheet)
                .insert(creabim_core::Element::new("vp", ElementKind::Viewport(vp))),
        )
        .unwrap();
        let at = free_spot(doc.model(), sid, f);
        let placed = &placed_views(doc.model(), sid, &mut |v| view_drawing(doc.model(), v))[0];
        let (lo, hi) = placed.bounds;
        // the new plan's own half width keeps it clear of the first
        let half = 90.0;
        assert!(
            at.x - half > hi.x || at.x + half < lo.x || at.y + 70.0 < lo.y || at.y - 70.0 > hi.y,
            "{at} overlaps the plan at {lo} {hi}"
        );
    }

    #[test]
    fn styled_annotations_reach_svg_and_pdf() {
        use creabim_core::{
            Dimension, DimensionKind, Element, Font, HostPoint, Note, TextStyle, Transaction,
        };
        let mut doc = creabim_core::Document::from_model(sample_house());
        let m = doc.model();
        let (sheet, _) = sheets(m)[1];
        let plan = m.iter().find(|e| e.name == "Ground Floor plan").unwrap().id;
        let style = Element::new(
            "Callouts",
            ElementKind::TextStyle(TextStyle {
                font: Font::Serif,
                size: 3.5,
                color: Some([0.8, 0.0, 0.0]),
                bold: true,
                italic: false,
                ..TextStyle::default()
            }),
        );
        let mut on_sheet = Note::new(sheet, DVec2::new(60.0, 60.0), "General note\nSecond line");
        on_sheet.style = Some(style.id);
        on_sheet.leader = Some(HostPoint::free(DVec2::new(40.0, 40.0)));
        let free = |x: f64, y: f64| HostPoint::free(DVec2::new(x, y));
        let angle = Dimension::about(
            DimensionKind::Angular,
            plan,
            free(0.0, 0.0),
            free(3.0, 0.0),
            free(0.0, 3.0),
            1.5,
        );
        let tx = Transaction::new("Annotate")
            .insert(Element::new("Note", ElementKind::Note(on_sheet)))
            .insert(Element::new("Angle", ElementKind::Dimension(angle)))
            .insert(style);
        doc.commit(tx).unwrap();
        let m = doc.model();
        let info = SheetInfo::default();
        let svg = sheet_svg(m, sheet, &info).unwrap();
        assert!(svg.contains(">General note</text>") && svg.contains(">Second line</text>"));
        assert!(svg.contains("font-weight=\"bold\"") && svg.contains("fill=\"#cc0000\""));
        assert!(svg.contains("Times"));
        // the angle in the plan, which sits on this sheet
        assert!(svg.contains(">90°</text>"), "angle missing");
        let pdf = String::from_utf8_lossy(&sheets_pdf(m, &info)).into_owned();
        assert!(pdf.contains("/BaseFont /Times-Bold"));
        assert!(pdf.contains("0.800 0.000 0.000 rg BT /F6"));
        assert!(pdf.contains("(General note) Tj"));
    }

    #[test]
    fn pdf_has_a_page_per_sheet() {
        let m = sample_house();
        let pdf = sheets_pdf(
            &m,
            &SheetInfo {
                project: "Sample house".into(),
                date: "2026-10-10".into(),
            },
        );
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.contains("/Count 4"));
        assert!(
            s.contains("/Subtype /Image"),
            "shaded views are embedded as images"
        );
        assert!(s.contains("(Floor Plans) Tj"));
        assert!(s.contains("(Ground Floor plan) Tj"));
    }

    #[test]
    fn cropped_viewports_show_only_their_crop() {
        let m = sample_house();
        let (sheet, _) = sheets(&m)
            .into_iter()
            .find(|(_, s)| s.number == "A601")
            .unwrap();
        let views = placed_views(&m, sheet, &mut |v| view_drawing(&m, v));
        let entrance = views.iter().find(|v| v.title == "Entrance plan").unwrap();
        let (lo, hi) = entrance.drawing.bounds().unwrap();
        assert!(
            lo.x >= -0.8 - 1e-9 && hi.x <= 5.0 + 1e-9 && lo.y >= -2.8 - 1e-9 && hi.y <= 2.0 + 1e-9
        );
        // the uncropped plan is much bigger
        let full = view_drawing(&m, entrance.view).unwrap().bounds().unwrap();
        assert!(full.1.x > 9.0);
    }

    #[test]
    fn viewports_follow_model_edits() {
        let mut doc = creabim_core::Document::from_model(sample_house());
        let (sheet, _) = sheets(doc.model())[1];
        let before = placed_views(doc.model(), sheet, &mut |v| view_drawing(doc.model(), v));
        let wall = doc
            .model()
            .iter()
            .find(|e| e.name == "GF Wall 1")
            .unwrap()
            .id;
        doc.edit(wall, "Thicken", |e| {
            if let ElementKind::Wall(w) = &mut e.kind {
                w.thickness = 0.5;
            }
        })
        .unwrap();
        let after = placed_views(doc.model(), sheet, &mut |v| view_drawing(doc.model(), v));
        assert_ne!(before[0].drawing, after[0].drawing);
        // the viewport stays pinned: same mapping
        assert_eq!(before[0].map, after[0].map);
    }

    #[test]
    fn the_built_in_block_as_a_template_draws_the_same() {
        let m = sample_house();
        let (id, s) = sheets(&m)[0];
        let views = placed_views(&m, id, &mut |v| view_drawing(&m, v));
        let info = SheetInfo {
            project: "House".into(),
            date: "2026-10-21".into(),
        };
        let title = m.get(id).unwrap().name.clone();
        let scales = scale_text(&m, &views);
        let mut a = SvgCanvas::new(DVec2::new(s.width, s.height));
        draw_builtin_frame(&mut a, s, &title, &scales, &info);
        let t = builtin_template();
        assert!(t.used_fields().contains(&"project".to_string()));
        let values = field_values(s, Some(&t), &title, &scales, &info);
        let mut b = SvgCanvas::new(DVec2::new(s.width, s.height));
        draw_template(&mut b, s, &t, &values);
        assert_eq!(a.finish(), b.finish());
    }

    #[test]
    fn a_template_title_block_fills_its_fields_on_every_sheet() {
        use creabim_core::titleblock::{Corner, SheetTemplate, TextAlign, TitleItem};
        use creabim_core::{Element, Transaction};
        let mut doc = creabim_core::Document::from_model(sample_house());
        let t = SheetTemplate {
            corner: Corner::BottomRight,
            items: vec![
                TitleItem::Line {
                    points: vec![DVec2::new(-100.0, 10.0), DVec2::new(-10.0, 10.0)],
                    width: 0.35,
                },
                TitleItem::Text {
                    at: DVec2::new(-95.0, 20.0),
                    size: 4.0,
                    text: "{number} · {client}".into(),
                    align: TextAlign::Start,
                    angle: 0.0,
                },
            ],
            border: false,
            fields: vec![("client".into(), "Client".into())],
            paper: None,
            guides: vec![],
            margins: None,
        };
        let te = Element::new("Office", ElementKind::SheetTemplate(t));
        let tid = te.id;
        doc.commit(Transaction::new("Template").insert(te)).unwrap();
        let (sid, _) = sheets(doc.model())[0];
        let info = SheetInfo::default();
        let before = sheet_svg(doc.model(), sid, &info).unwrap();
        assert!(before.contains("PROJECT"), "the built-in block");
        doc.edit(sid, "Use template", |e| {
            if let ElementKind::Sheet(s) = &mut e.kind {
                s.template = Some(tid);
                s.fields.insert("client".into(), "Ana Lopes".into());
            }
        })
        .unwrap();
        let svg = sheet_svg(doc.model(), sid, &info).unwrap();
        assert!(!svg.contains("PROJECT"), "the template replaces it");
        let s = sheet_of(doc.model(), sid).unwrap();
        assert!(
            svg.contains(&format!("{} · Ana Lopes", s.number)),
            "fields filled"
        );
        // its bounds are measured from the corner, and views keep clear of it
        let (lo, hi) = title_block_bounds(doc.model(), s);
        assert!(
            (hi.x - (s.width - 10.0)).abs() < 1e-9 && lo.y > 9.0,
            "{lo} {hi}"
        );
        // deleting the template brings the built-in block back
        doc.commit(Transaction::new("Delete").remove_cascade(doc.model(), tid))
            .unwrap();
        assert!(sheet_svg(doc.model(), sid, &info)
            .unwrap()
            .contains("PROJECT"));
    }
}
