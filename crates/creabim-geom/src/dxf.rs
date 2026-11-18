//! Drawings written as DXF, for CAD programs: a [`Canvas`] that turns what
//! a view or sheet draws into lines, text and solid fills. The file is the
//! old plain R12 flavour (AC1009), which every CAD program reads.
//!
//! Line weights become layers (`HEAVY`, `MEDIUM`, `LIGHT`, `FINE`), dashed
//! lines go on `DASHED` with a dashed linetype, text on `TEXT` and solid
//! fills on `FILL`, so a drawing can be restyled by layer once opened.

use crate::paper::{Anchor, Canvas, Stroke};
use crate::raster::Raster;
use glam::DVec2;
use std::fmt::Write;
use std::sync::Arc;

/// The layers a drawing is written on, with their AutoCAD colour index.
const LAYERS: [(&str, u8, &str); 7] = [
    ("HEAVY", 7, "CONTINUOUS"),
    ("MEDIUM", 7, "CONTINUOUS"),
    ("LIGHT", 8, "CONTINUOUS"),
    ("FINE", 9, "CONTINUOUS"),
    ("DASHED", 8, "DASHED"),
    ("TEXT", 7, "CONTINUOUS"),
    ("FILL", 9, "CONTINUOUS"),
];

/// A drawing being written as DXF. Canvas points are paper millimetres;
/// they are written `k` times as large, so a view at 1:100 written with
/// `k` 100 is full size in millimetres.
pub struct DxfCanvas {
    k: f64,
    body: String,
}

impl DxfCanvas {
    pub fn new(k: f64) -> Self {
        Self {
            k,
            body: String::new(),
        }
    }

    fn pair(&mut self, code: u32, value: impl std::fmt::Display) {
        let _ = write!(self.body, "{code:>3}\n{value}\n");
    }

    fn point(&mut self, first: u32, p: DVec2) {
        let p = p * self.k;
        self.pair(first, format!("{:.4}", p.x));
        self.pair(first + 10, format!("{:.4}", p.y));
        self.pair(first + 20, "0.0");
    }

    /// The layer a stroke is drawn on.
    fn layer_of(s: Stroke) -> &'static str {
        if s.dash.is_some() {
            "DASHED"
        } else if s.width >= 0.45 {
            "HEAVY"
        } else if s.width >= 0.22 {
            "MEDIUM"
        } else if s.width >= 0.15 {
            "LIGHT"
        } else {
            "FINE"
        }
    }

    fn polyline(&mut self, pts: &[DVec2], layer: &str, colour: Option<u8>) {
        // points drawn twice over are one
        let mut path: Vec<DVec2> = Vec::with_capacity(pts.len());
        for p in pts {
            if path.last().is_none_or(|q| q.distance(*p) > 1e-7) {
                path.push(*p);
            }
        }
        if path.len() < 2 {
            return;
        }
        let entity = |c: &mut Self, name: &str| {
            c.pair(0, name);
            c.pair(8, layer);
            if let Some(i) = colour {
                c.pair(62, i);
            }
        };
        if let [a, b] = path[..] {
            entity(self, "LINE");
            self.point(10, a);
            self.point(11, b);
            return;
        }
        let closed = path.len() > 3 && path[0].distance(path[path.len() - 1]) < 1e-7;
        if closed {
            path.pop();
        }
        entity(self, "POLYLINE");
        self.pair(66, 1);
        self.pair(70, u8::from(closed));
        for p in path {
            entity(self, "VERTEX");
            self.point(10, p);
        }
        entity(self, "SEQEND");
    }

    /// The whole file.
    pub fn finish(self) -> String {
        let mut out = String::new();
        let mut pair = |code: u32, value: &dyn std::fmt::Display| {
            let _ = write!(out, "{code:>3}\n{value}\n");
        };
        for (c, v) in [
            (0, "SECTION"),
            (2, "HEADER"),
            (9, "$ACADVER"),
            (1, "AC1009"),
        ] {
            pair(c, &v);
        }
        for (c, v) in [(0, "ENDSEC"), (0, "SECTION"), (2, "TABLES")] {
            pair(c, &v);
        }
        // linetypes: continuous, and dashed 3 on 1.5 off (in drawing units)
        for (c, v) in [(0, "TABLE"), (2, "LTYPE")] {
            pair(c, &v);
        }
        pair(70, &2);
        for (c, v) in [(0, "LTYPE"), (2, "CONTINUOUS")] {
            pair(c, &v);
        }
        pair(70, &0);
        pair(3, &"Solid line");
        pair(72, &65);
        pair(73, &0);
        pair(40, &0.0);
        for (c, v) in [(0, "LTYPE"), (2, "DASHED")] {
            pair(c, &v);
        }
        pair(70, &0);
        pair(3, &"Dashed __ __ __");
        pair(72, &65);
        pair(73, &2);
        pair(40, &format!("{:.4}", 4.5 * self.k));
        pair(49, &format!("{:.4}", 3.0 * self.k));
        pair(49, &format!("{:.4}", -1.5 * self.k));
        for (c, v) in [(0, "ENDTAB"), (0, "TABLE"), (2, "LAYER")] {
            pair(c, &v);
        }
        pair(70, &LAYERS.len());
        for (name, colour, ltype) in LAYERS {
            pair(0, &"LAYER");
            pair(2, &name);
            pair(70, &0);
            pair(62, &colour);
            pair(6, &ltype);
        }
        for (c, v) in [
            (0, "ENDTAB"),
            (0, "ENDSEC"),
            (0, "SECTION"),
            (2, "ENTITIES"),
        ] {
            pair(c, &v);
        }
        out.push_str(&self.body);
        out.push_str("  0\nENDSEC\n  0\nEOF\n");
        out
    }
}

/// The AutoCAD colour index nearest `rgb`, among the plain colours and
/// greys; black is 7, which CAD programs draw black on white.
fn aci([r, g, b]: [f32; 3]) -> u8 {
    const TABLE: [([f32; 3], u8); 9] = [
        ([1.0, 0.0, 0.0], 1),
        ([1.0, 1.0, 0.0], 2),
        ([0.0, 1.0, 0.0], 3),
        ([0.0, 1.0, 1.0], 4),
        ([0.0, 0.0, 1.0], 5),
        ([1.0, 0.0, 1.0], 6),
        ([0.0, 0.0, 0.0], 7),
        ([0.5, 0.5, 0.5], 8),
        ([0.75, 0.75, 0.75], 9),
    ];
    TABLE
        .iter()
        .min_by(|x, y| {
            let d = |t: &[f32; 3]| (t[0] - r).powi(2) + (t[1] - g).powi(2) + (t[2] - b).powi(2);
            d(&x.0).total_cmp(&d(&y.0))
        })
        .map_or(7, |t| t.1)
}

/// Is the outline convex, so it can be written as a fan of solids?
fn convex(pts: &[DVec2]) -> bool {
    let n = pts.len();
    let mut sign = 0.0;
    for i in 0..n {
        let c = (pts[(i + 1) % n] - pts[i]).perp_dot(pts[(i + 2) % n] - pts[(i + 1) % n]);
        if c.abs() > 1e-12 {
            if sign * c < 0.0 {
                return false;
            }
            sign = c;
        }
    }
    true
}

impl Canvas for DxfCanvas {
    fn line(&mut self, pts: &[DVec2], s: Stroke) {
        self.polyline(pts, Self::layer_of(s), None);
    }

    fn line_rgb(&mut self, pts: &[DVec2], s: Stroke, rgb: [f32; 3]) {
        self.polyline(pts, Self::layer_of(s), Some(aci(rgb)));
    }

    fn fill(&mut self, pts: &[DVec2], gray: f64) {
        self.fill_rgb(pts, [gray as f32; 3]);
    }

    fn fill_rgb(&mut self, pts: &[DVec2], rgb: [f32; 3]) {
        // white is a mask over what is behind: nothing to write
        if rgb.iter().all(|c| *c > 0.95) || pts.len() < 3 || !convex(pts) {
            return;
        }
        let colour = aci(rgb);
        for i in 1..pts.len() - 1 {
            self.pair(0, "SOLID");
            self.pair(8, "FILL");
            self.pair(62, colour);
            // a solid's third and fourth corners are written crossed over
            self.point(10, pts[0]);
            self.point(11, pts[i]);
            self.point(12, pts[i + 1]);
            self.point(13, pts[i + 1]);
        }
    }

    fn tint(&mut self, _pts: &[DVec2], _rgb: [f32; 3]) {}

    fn text_angle(&mut self, at: DVec2, size: f64, text: &str, anchor: Anchor, angle: f64) {
        let text: String = text
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        if text.trim().is_empty() {
            return;
        }
        self.pair(0, "TEXT");
        self.pair(8, "TEXT");
        self.point(10, at);
        // a font's size is its body; capitals stand about 0.7 of it
        self.pair(40, format!("{:.4}", size * 0.7 * self.k));
        self.pair(1, text);
        if angle.abs() > 1e-9 {
            self.pair(50, format!("{:.3}", angle.to_degrees()));
        }
        self.pair(
            72,
            match anchor {
                Anchor::Start => 0,
                Anchor::Middle => 1,
                Anchor::End => 2,
            },
        );
        self.pair(73, 2);
        self.point(11, at);
    }

    fn image(&mut self, _: DVec2, _: DVec2, _: Option<(DVec2, DVec2)>, _: &Arc<Raster>) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The (code, value) pairs of a DXF file.
    fn pairs(dxf: &str) -> Vec<(u32, String)> {
        let lines: Vec<&str> = dxf.lines().collect();
        assert_eq!(lines.len() % 2, 0, "codes and values come in pairs");
        lines
            .chunks(2)
            .map(|c| (c[0].trim().parse().expect("a group code"), c[1].to_string()))
            .collect()
    }

    #[test]
    fn a_drawing_is_written_as_lines_text_and_fills_on_layers() {
        let mut c = DxfCanvas::new(100.0);
        let p = DVec2::new;
        c.line(&[p(0.0, 0.0), p(10.0, 0.0)], Stroke::solid(0.5, 0.0));
        c.line(
            &[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 5.0), p(0.0, 0.0)],
            Stroke {
                width: 0.18,
                gray: 0.0,
                dash: Some([2.0, 1.0]),
            },
        );
        c.line_rgb(
            &[p(0.0, 1.0), p(1.0, 1.0), p(2.0, 3.0)],
            Stroke::solid(0.1, 0.0),
            [1.0, 0.0, 0.0],
        );
        c.fill(&[p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)], 0.5);
        c.fill(&[p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)], 1.0);
        c.text_angle(p(5.0, 5.0), 3.0, "Kitchen", Anchor::Middle, 0.0);
        let dxf = c.finish();
        let ps = pairs(&dxf);
        assert_eq!(ps.first().unwrap(), &(0, "SECTION".to_string()));
        assert_eq!(ps.last().unwrap(), &(0, "EOF".to_string()));
        let count = |name: &str| ps.iter().filter(|p| p.0 == 0 && p.1 == name).count();
        assert_eq!(count("LINE"), 1);
        assert_eq!(count("POLYLINE"), 2);
        assert_eq!(count("SEQEND"), 2);
        // the closed triangle is three vertices, closed; the open line three
        assert_eq!(count("VERTEX"), 6);
        // a grey square is two solids; the white mask is left out
        assert_eq!(count("SOLID"), 2);
        assert_eq!(count("TEXT"), 1);
        // full size in millimetres: 10 mm on paper at 1:100 is a metre
        assert!(ps.contains(&(11, "1000.0000".to_string())));
        // weights and dashes are layers, and red is colour 1
        let after = |name: &str| {
            let i = ps.iter().position(|p| p.0 == 0 && p.1 == name).unwrap();
            ps[i + 1].clone()
        };
        assert_eq!(after("LINE"), (8, "HEAVY".to_string()));
        assert_eq!(after("POLYLINE"), (8, "DASHED".to_string()));
        assert!(ps
            .windows(2)
            .any(|w| w[0] == (8, "FINE".to_string()) && w[1] == (62, "1".to_string())));
        for (name, ..) in LAYERS {
            assert!(ps.contains(&(2, name.to_string())), "{name}");
        }
    }
}
