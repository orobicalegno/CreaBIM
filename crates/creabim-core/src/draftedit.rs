//! Editing plain linework ([`crate::DetailKind::Draft`]) against other
//! linework, as a CAD program does: trim a line back to what crosses it,
//! extend it to what it would meet, round or cut off a corner, offset a
//! copy to one side, and repeat a piece in rows or round a centre.
//!
//! Every function here works in the view's drawing coordinates on the
//! line each piece draws ([`DetailItem::draft_path`], curves as chords),
//! and returns the new pieces; the caller puts them in the model.

use crate::detail::arc_through;
use crate::{DetailItem, DetailKind, DraftShape, LinePen};
use glam::DVec2;

/// Where a click lands on a piece of linework.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Which stretch of the drawn path, counted from its start.
    pub segment: usize,
    /// The nearest point on the path.
    pub at: DVec2,
    /// How far the click was from it.
    pub distance: f64,
    /// How far along the path that is.
    pub along: f64,
}

/// The path a piece draws, closed ones ending back on their start.
fn path_of(d: &DetailItem) -> Option<(Vec<DVec2>, bool)> {
    let (mut path, closed) = d.draft_path()?;
    if closed {
        path.push(*path.first()?);
    }
    (path.len() >= 2).then_some((path, closed))
}

fn pen_of(d: &DetailItem) -> LinePen {
    d.kind.as_draft().map_or(LinePen::Medium, |k| k.1)
}

/// A new piece like `like` (its view, its pen) of `shape` through `points`.
fn piece(like: &DetailItem, shape: DraftShape, points: Vec<DVec2>) -> DetailItem {
    let mut out = like.clone();
    out.kind = DetailKind::Draft {
        shape,
        pen: pen_of(like),
    };
    out.points = points;
    out
}

/// Where `p` lands on `d`'s line: the nearest point on it.
pub fn hit(d: &DetailItem, p: DVec2) -> Option<Hit> {
    let (path, _) = path_of(d)?;
    let mut best: Option<Hit> = None;
    let mut run = 0.0;
    for (i, w) in path.windows(2).enumerate() {
        let ab = w[1] - w[0];
        let len = ab.length();
        if len > 1e-12 {
            let t = ((p - w[0]).dot(ab) / (len * len)).clamp(0.0, 1.0);
            let at = w[0] + ab * t;
            let distance = at.distance(p);
            if best.is_none_or(|b| distance < b.distance) {
                best = Some(Hit {
                    segment: i,
                    at,
                    distance,
                    along: run + t * len,
                });
            }
        }
        run += len;
    }
    best
}

/// Where the stretch `a`..`b` crosses `c`..`d`: how far along the first, 0 to 1.
fn crossing(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> Option<f64> {
    let (r, s) = (b - a, d - c);
    let den = r.perp_dot(s);
    if den.abs() < 1e-14 {
        return None;
    }
    let t = (c - a).perp_dot(s) / den;
    let u = (c - a).perp_dot(r) / den;
    let eps = 1e-9;
    (t >= -eps && t <= 1.0 + eps && u >= -eps && u <= 1.0 + eps).then_some(t.clamp(0.0, 1.0))
}

/// How far along `path` each crossing with `others` is, in order.
fn cuts(path: &[DVec2], others: &[&DetailItem]) -> Vec<f64> {
    let edges: Vec<(DVec2, DVec2)> = others
        .iter()
        .filter_map(|o| path_of(o))
        .flat_map(|(p, _)| p.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>())
        .collect();
    let mut out = Vec::new();
    let mut run = 0.0;
    for w in path.windows(2) {
        let len = w[0].distance(w[1]);
        for (c, d) in &edges {
            if let Some(t) = crossing(w[0], w[1], *c, *d) {
                out.push(run + t * len);
            }
        }
        run += len;
    }
    out.sort_by(|a, b| a.total_cmp(b));
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
    out
}

/// The point `s` along `path`.
fn point_at(path: &[DVec2], s: f64) -> DVec2 {
    let mut run = 0.0;
    for w in path.windows(2) {
        let len = w[0].distance(w[1]);
        if s <= run + len || len < 1e-12 && s <= run {
            return w[0] + (w[1] - w[0]) * if len > 1e-12 { (s - run) / len } else { 0.0 };
        }
        run += len;
    }
    *path.last().expect("a path has points")
}

/// The part of `path` from `from` to `to` along it.
fn between(path: &[DVec2], from: f64, to: f64) -> Vec<DVec2> {
    let mut out = vec![point_at(path, from)];
    let mut run = 0.0;
    for w in path.windows(2) {
        run += w[0].distance(w[1]);
        if run > from + 1e-9 && run < to - 1e-9 {
            out.push(w[1]);
        }
    }
    out.push(point_at(path, to));
    out
}

fn length(path: &[DVec2]) -> f64 {
    path.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// A stretch of `d`'s line as a piece of its own: of a circle or an arc
/// an arc, of anything else a line through the stretch's points.
fn stretch(d: &DetailItem, pts: Vec<DVec2>) -> DetailItem {
    match d.kind.as_draft().map(|k| k.0) {
        Some(DraftShape::Circle | DraftShape::Arc) if pts.len() >= 2 => {
            let mid = point_at(&pts, length(&pts) / 2.0);
            let (a, b) = (pts[0], pts[pts.len() - 1]);
            piece(d, DraftShape::Arc, vec![a, mid, b])
        }
        _ => piece(d, DraftShape::Line, pts),
    }
}

/// Trim: the stretch of `d` under `p` taken away, back to the linework
/// in `others` that crosses it on either side. What is left, as one or
/// two pieces; `None` when nothing crosses it (or a closed one is crossed
/// only once), so there is nothing to trim back to.
pub fn trim(d: &DetailItem, others: &[&DetailItem], p: DVec2) -> Option<Vec<DetailItem>> {
    let (path, closed) = path_of(d)?;
    let at = hit(d, p)?.along;
    let cuts = cuts(&path, others);
    let total = length(&path);
    let below = cuts.iter().copied().rfind(|c| *c < at - 1e-9);
    let above = cuts.iter().copied().find(|c| *c > at + 1e-9);
    let keep =
        |from: f64, to: f64| (to - from > 1e-7).then(|| stretch(d, between(&path, from, to)));
    if closed {
        // what is left runs from the cut after the click round to the one before
        let (lo, hi) = match (below, above) {
            (Some(lo), Some(hi)) => (lo, hi),
            (Some(lo), None) => (lo, *cuts.first()? + total),
            (None, Some(hi)) => (*cuts.last()? - total, hi),
            (None, None) => return None,
        };
        if hi - lo > total - 1e-7 {
            return None;
        }
        let mut round = between(&path, hi.rem_euclid(total), total);
        let lo = lo.rem_euclid(total);
        let hi = hi.rem_euclid(total);
        if lo > hi {
            round = between(&path, hi, lo);
        } else {
            round.pop();
            round.extend(between(&path, 0.0, lo));
        }
        return Some(vec![stretch(d, round)]);
    }
    if below.is_none() && above.is_none() {
        return None;
    }
    Some(
        [
            below.and_then(|lo| keep(0.0, lo)),
            above.and_then(|hi| keep(hi, total)),
        ]
        .into_iter()
        .flatten()
        .collect(),
    )
}

/// Extend: the end of line `d` nearest `p` run on, straight, to the first
/// linework in `others` it meets. `None` when it isn't an open line, or
/// meets nothing that way.
pub fn extend(d: &DetailItem, others: &[&DetailItem], p: DVec2) -> Option<DetailItem> {
    let (DraftShape::Line, _) = d.kind.as_draft()? else {
        return None;
    };
    let n = d.points.len();
    if n < 2 {
        return None;
    }
    let h = hit(d, p)?;
    let at_end = h.along > d.length() / 2.0;
    let (from, tip) = if at_end {
        (d.points[n - 2], d.points[n - 1])
    } else {
        (d.points[1], d.points[0])
    };
    let dir = (tip - from).try_normalize()?;
    // the ray from the tip, as a very long stretch
    let reach = 1e6;
    let far = tip + dir * reach;
    let nearest = others
        .iter()
        .filter_map(|o| path_of(o))
        .flat_map(|(q, _)| q.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>())
        .filter_map(|(c, e)| crossing(tip, far, c, e))
        .map(|t| t * reach)
        .filter(|t| *t > 1e-7)
        .min_by(|a, b| a.total_cmp(b))?;
    let mut out = d.clone();
    let i = if at_end { n - 1 } else { 0 };
    out.points[i] = tip + dir * nearest;
    Some(out)
}

/// Which corner two picked stretches make.
enum Corner {
    /// Two stretches of one line that meet at its point `vertex`.
    Within { vertex: usize },
    /// The ends of two lines: each line's end point index.
    Between { a: usize, b: usize },
}

/// How a corner is finished.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Join {
    /// Rounded with an arc of this radius; 0 makes a sharp corner.
    Fillet(f64),
    /// Cut off this far back from the corner along each line.
    Chamfer(f64),
}

/// The pieces a fillet or chamfer leaves: the lines changed, and the arc
/// or cut put between them (none within one line, or for a sharp corner).
#[derive(Clone, Debug, PartialEq)]
pub struct Joined {
    pub a: DetailItem,
    /// The second line, when it is another piece than the first.
    pub b: Option<DetailItem>,
    pub between: Option<DetailItem>,
}

fn straight(d: &DetailItem) -> Result<bool, String> {
    match d.kind.as_draft().map(|k| k.0) {
        Some(DraftShape::Line) => Ok(false),
        Some(DraftShape::Closed) => Ok(true),
        _ => Err("Pick straight lines: a line, a closed line, a rectangle or a polygon.".into()),
    }
}

/// Fillet or chamfer the corner between the stretch of `a` under `pa`
/// and the stretch of `b` under `pb`. `same` says `a` and `b` are one
/// piece: then the two stretches must share a corner. Otherwise each is
/// run on or cut back at the end nearer where they would meet.
pub fn join(
    a: &DetailItem,
    pa: DVec2,
    b: &DetailItem,
    pb: DVec2,
    same: bool,
    how: Join,
) -> Result<Joined, String> {
    let (closed_a, closed_b) = (straight(a)?, straight(b)?);
    let (ha, hb) = (
        hit(a, pa).ok_or("Nothing to join there.")?,
        hit(b, pb).ok_or("Nothing to join there.")?,
    );
    let corner = if same {
        let n = a.points.len();
        let (i, j) = (ha.segment.min(hb.segment), ha.segment.max(hb.segment));
        let segs = if closed_a { n } else { n - 1 };
        if j == i + 1 {
            Corner::Within { vertex: j }
        } else if closed_a && i == 0 && j == segs - 1 {
            Corner::Within { vertex: 0 }
        } else {
            return Err("Pick two sides that meet at a corner.".into());
        }
    } else {
        if closed_a || closed_b {
            return Err("Pick two open lines, or two sides of one outline's corner.".into());
        }
        // each line's end nearer where the two would meet
        let end = |d: &DetailItem, h: &Hit, x: DVec2| -> Result<usize, String> {
            let n = d.points.len();
            let (first, last) = (h.segment == 0, h.segment == n - 2);
            match (first, last) {
                (true, true) => Ok(if d.points[0].distance(x) <= d.points[n - 1].distance(x) {
                    0
                } else {
                    n - 1
                }),
                (true, false) => Ok(0),
                (false, true) => Ok(n - 1),
                _ => Err("Pick each line by its first or last stretch.".into()),
            }
        };
        let seg = |d: &DetailItem, h: &Hit| (d.points[h.segment], d.points[h.segment + 1]);
        let ((a0, a1), (b0, b1)) = (seg(a, &ha), seg(b, &hb));
        let (r, s) = (a1 - a0, b1 - b0);
        let den = r.perp_dot(s);
        if den.abs() < 1e-12 {
            return Err("The two lines are parallel: they have no corner.".into());
        }
        let x = a0 + r * ((b0 - a0).perp_dot(s) / den);
        Corner::Between {
            a: end(a, &ha, x)?,
            b: end(b, &hb, x)?,
        }
    };
    // the corner point and the two directions away from it
    let (x, ua, ub, room_a, room_b) = match corner {
        Corner::Within { vertex } => {
            let n = a.points.len();
            let x = a.points[vertex];
            let (p, q) = (a.points[(vertex + n - 1) % n], a.points[(vertex + 1) % n]);
            (x, p - x, q - x, p.distance(x), q.distance(x))
        }
        Corner::Between { a: ia, b: ib } => {
            let other =
                |d: &DetailItem, i: usize| if i == 0 { d.points[1] } else { d.points[i - 1] };
            let (fa, fb) = (other(a, ia), other(b, ib));
            let (r, s) = (a.points[ia] - fa, b.points[ib] - fb);
            let x = fa + r * ((fb - fa).perp_dot(s) / r.perp_dot(s));
            (x, fa - x, fb - x, fa.distance(x), fb.distance(x))
        }
    };
    let (ua, ub) = (
        ua.try_normalize().ok_or("A line there has no length.")?,
        ub.try_normalize().ok_or("A line there has no length.")?,
    );
    let half = ua.angle_to(ub).abs() / 2.0;
    if half < 1e-6 || (std::f64::consts::FRAC_PI_2 - half).abs() < 1e-9 {
        return Err("The two lines run on in one line: they have no corner.".into());
    }
    let back = match how {
        Join::Fillet(r) => r.max(0.0) / half.tan(),
        Join::Chamfer(d) => d.max(0.0),
    };
    if back > room_a - 1e-9 || back > room_b - 1e-9 {
        return Err("Too large for these lines: it would run past their ends.".into());
    }
    let (ta, tb) = (x + ua * back, x + ub * back);
    // what goes between the two cut-back ends
    let link: Vec<DVec2> = match how {
        _ if back < 1e-9 => vec![],
        Join::Chamfer(_) => vec![ta, tb],
        Join::Fillet(r) => {
            let bis = (ua + ub).normalize();
            let c = x + bis * (r / half.sin());
            arc_through(ta, c - bis * r, tb)
        }
    };
    Ok(match corner {
        Corner::Within { vertex } => {
            let mut out = a.clone();
            let with: Vec<DVec2> = if link.is_empty() { vec![x] } else { link };
            out.points.splice(vertex..=vertex, with);
            Joined {
                a: out,
                b: None,
                between: None,
            }
        }
        Corner::Between { a: ia, b: ib } => {
            let (mut na, mut nb) = (a.clone(), b.clone());
            na.points[ia] = ta;
            nb.points[ib] = tb;
            let between = match how {
                _ if link.is_empty() => None,
                Join::Chamfer(_) => Some(piece(a, DraftShape::Line, link)),
                Join::Fillet(_) => {
                    let mid = link[link.len() / 2];
                    Some(piece(a, DraftShape::Arc, vec![ta, mid, tb]))
                }
            };
            Joined {
                a: na,
                b: Some(nb),
                between,
            }
        }
    })
}

/// A closed outline grown `by` (shrunk when negative), corners mitred.
fn grow(pts: &[DVec2], by: f64) -> Vec<DVec2> {
    let n = pts.len();
    let area: f64 = (0..n).map(|i| pts[i].perp_dot(pts[(i + 1) % n])).sum();
    // out of a counter-clockwise outline is to its right
    let side = if area >= 0.0 { -by } else { by };
    (0..n)
        .map(|i| {
            let (p, x, q) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            let a = (x - p).normalize_or_zero().perp();
            let b = (q - x).normalize_or_zero().perp();
            let m = (a + b).normalize_or_zero();
            let c = m.dot(a);
            x + if c.abs() > 0.25 { m / c } else { a } * side
        })
        .collect()
}

/// Offset: a copy of `d` a distance `by` to the side of it that `side`
/// is on: beside a line, inside or outside a closed shape. `None` when
/// there is no such copy (a circle shrunk to nothing).
pub fn offset(d: &DetailItem, by: f64, side: DVec2) -> Option<DetailItem> {
    let (shape, _) = d.kind.as_draft()?;
    let by = by.abs();
    let p = &d.points;
    let mut out = d.clone();
    // about a centre: further out when `side` is further from it than the shape
    let radial = |c: DVec2, r: f64| if side.distance(c) >= r { by } else { -by };
    match shape {
        DraftShape::Line | DraftShape::Spline => {
            let h = hit(d, side)?;
            let (path, _) = path_of(d)?;
            let (a, b) = (path[h.segment], path[h.segment + 1]);
            let left = (b - a).perp_dot(side - a) >= 0.0;
            out.points = crate::offset::offset_path(p, if left { by } else { -by });
        }
        DraftShape::Closed | DraftShape::Rectangle | DraftShape::Polygon { .. } => {
            let (path, _) = d.draft_path()?;
            let inside = crate::polygon::contains(&path, side);
            out = piece(
                d,
                DraftShape::Closed,
                grow(&path, if inside { -by } else { by }),
            );
        }
        DraftShape::Circle => {
            let (c, r) = (p[0], p[0].distance(p[1]));
            let nr = r + radial(c, r);
            if nr <= 1e-9 {
                return None;
            }
            out.points[1] = c + (p[1] - c) * (nr / r);
        }
        DraftShape::Arc => {
            // the same arc on a circle about the same centre
            let chords = arc_through(p[0], p[1], p[2]);
            if chords.len() == 3 && (p[1] - p[0]).perp_dot(p[2] - p[0]).abs() < 1e-12 {
                return None;
            }
            let (ab, am) = (p[2] - p[0], p[1] - p[0]);
            let cross = am.perp_dot(ab);
            let (d1, d2) = (am.length_squared() / 2.0, ab.length_squared() / 2.0);
            let c = p[0] + DVec2::new(d1 * ab.y - d2 * am.y, d2 * am.x - d1 * ab.x) / cross;
            let r = c.distance(p[0]);
            let nr = r + radial(c, r);
            if nr <= 1e-9 {
                return None;
            }
            for q in &mut out.points {
                *q = c + (*q - c) * (nr / r);
            }
        }
        DraftShape::Ellipse => {
            let c = p[0];
            let u = p[1] - c;
            let along = u.try_normalize()?;
            let across = along.perp();
            let (ra, rb) = (u.length(), (p[2] - c).dot(across).abs());
            // inside when the point is inside the ellipse
            let q = side - c;
            let inside =
                (q.dot(along) / ra).powi(2) + (q.dot(across) / rb.max(1e-12)).powi(2) < 1.0;
            let by = if inside { -by } else { by };
            if ra + by <= 1e-9 || rb + by <= 1e-9 {
                return None;
            }
            out.points[1] = c + along * (ra + by);
            out.points[2] = c + across * (rb + by);
        }
    }
    Some(out)
}

/// Rows and columns: `d` and its copies, `across` by `up` of them, each
/// `step` from the last. The first is `d` itself, where it is.
pub fn array_grid(d: &DetailItem, across: u32, up: u32, step: DVec2) -> Vec<DetailItem> {
    let mut out = Vec::new();
    for j in 0..up.max(1) {
        for i in 0..across.max(1) {
            let by = DVec2::new(step.x * i as f64, step.y * j as f64);
            let mut c = d.clone();
            for p in &mut c.points {
                *p += by;
            }
            out.push(c);
        }
    }
    out
}

/// Round a centre: `d` and its copies, `count` of them spread evenly over
/// `angle` radians about `centre` (a full turn leaves no copy on the first).
pub fn array_polar(d: &DetailItem, centre: DVec2, count: u32, angle: f64) -> Vec<DetailItem> {
    let n = count.max(1);
    let full = (angle.abs() - std::f64::consts::TAU).abs() < 1e-9;
    let steps = if full || n == 1 { n } else { n - 1 };
    (0..n)
        .map(|i| {
            let turn = DVec2::from_angle(angle * i as f64 / steps as f64);
            let mut c = d.clone();
            for p in &mut c.points {
                *p = centre + turn.rotate(*p - centre);
            }
            c
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ElementId;

    fn p(x: f64, y: f64) -> DVec2 {
        DVec2::new(x, y)
    }

    fn item(shape: DraftShape, pts: &[(f64, f64)]) -> DetailItem {
        DetailItem::new(
            ElementId::new(),
            DetailKind::draft(shape),
            pts.iter().map(|&(x, y)| p(x, y)).collect(),
        )
    }

    fn line(pts: &[(f64, f64)]) -> DetailItem {
        item(DraftShape::Line, pts)
    }

    /// The same points, to within rounding.
    fn same(a: &[DVec2], b: &[DVec2]) {
        assert_eq!(a.len(), b.len(), "{a:?} {b:?}");
        for (x, y) in a.iter().zip(b) {
            assert!(x.distance(*y) < 1e-9, "{a:?} {b:?}");
        }
    }

    #[test]
    fn trim_takes_away_the_stretch_between_what_crosses_it() {
        let l = line(&[(0.0, 0.0), (10.0, 0.0)]);
        let (a, b) = (
            line(&[(3.0, -1.0), (3.0, 1.0)]),
            line(&[(7.0, -1.0), (7.0, 1.0)]),
        );
        // the middle goes, two ends stay
        let left = trim(&l, &[&a, &b], p(5.0, 0.1)).unwrap();
        assert_eq!(left.len(), 2);
        assert_eq!(left[0].points, vec![p(0.0, 0.0), p(3.0, 0.0)]);
        assert_eq!(left[1].points, vec![p(7.0, 0.0), p(10.0, 0.0)]);
        // an end goes back to the first crossing
        let left = trim(&l, &[&a, &b], p(9.0, 0.0)).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].points, vec![p(0.0, 0.0), p(7.0, 0.0)]);
        // nothing crosses: nothing to trim back to
        assert!(trim(&l, &[], p(5.0, 0.0)).is_none());
        // a line of several stretches keeps its corners
        let bent = line(&[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0)]);
        let cut = line(&[(4.0, 2.0), (6.0, 2.0)]);
        let left = trim(&bent, &[&cut], p(5.0, 4.0)).unwrap();
        assert_eq!(left[0].points, vec![p(0.0, 0.0), p(5.0, 0.0), p(5.0, 2.0)]);
    }

    #[test]
    fn trimming_a_circle_leaves_an_arc_and_a_closed_line_an_open_one() {
        let c = item(DraftShape::Circle, &[(0.0, 0.0), (2.0, 0.0)]);
        let cut = line(&[(-3.0, 0.0), (3.0, 0.0)]);
        // the top half goes: an arc under the line is left
        let left = trim(&c, &[&cut], p(0.0, 2.0)).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].kind.as_draft().unwrap().0, DraftShape::Arc);
        let (path, closed) = left[0].draft_path().unwrap();
        assert!(!closed);
        assert!(path
            .iter()
            .all(|q| (q.length() - 2.0).abs() < 1e-6 && q.y < 1e-6));
        assert!((left[0].length() - std::f64::consts::PI * 2.0).abs() < 0.01);
        // one crossing is not enough to open a closed shape
        let touch = line(&[(0.0, 0.0), (3.0, 0.0)]);
        assert!(trim(&c, &[&touch], p(0.0, 2.0)).is_none());
        // a square cut across: the side picked and what joins it go
        let sq = item(
            DraftShape::Closed,
            &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        );
        let cut = line(&[(-1.0, 2.0), (5.0, 2.0)]);
        let left = trim(&sq, &[&cut], p(2.0, 4.0)).unwrap();
        assert_eq!(left[0].kind.as_draft().unwrap().0, DraftShape::Line);
        assert_eq!(
            left[0].points,
            vec![p(0.0, 2.0), p(0.0, 0.0), p(4.0, 0.0), p(4.0, 2.0)]
        );
    }

    #[test]
    fn extend_runs_a_line_on_to_what_it_meets() {
        let l = line(&[(0.0, 0.0), (2.0, 0.0)]);
        let (near, far) = (
            line(&[(5.0, -1.0), (5.0, 1.0)]),
            line(&[(8.0, -1.0), (8.0, 1.0)]),
        );
        let out = extend(&l, &[&far, &near], p(1.8, 0.0)).unwrap();
        assert_eq!(out.points, vec![p(0.0, 0.0), p(5.0, 0.0)]);
        // the other end, the other way: nothing there
        assert!(extend(&l, &[&far, &near], p(0.1, 0.0)).is_none());
        // a circle is met at its near side
        let c = item(DraftShape::Circle, &[(10.0, 0.0), (12.0, 0.0)]);
        let out = extend(&l, &[&c], p(2.0, 0.0)).unwrap();
        assert!((out.points[1].x - 8.0).abs() < 1e-3);
    }

    #[test]
    fn a_fillet_rounds_the_corner_of_two_lines_or_of_one() {
        // two lines that would meet at the origin, square
        let a = line(&[(5.0, 0.0), (1.0, 0.0)]);
        let b = line(&[(0.0, 1.0), (0.0, 5.0)]);
        let j = join(&a, p(2.0, 0.0), &b, p(0.0, 2.0), false, Join::Fillet(0.5)).unwrap();
        same(&j.a.points, &[p(5.0, 0.0), p(0.5, 0.0)]);
        same(&j.b.as_ref().unwrap().points, &[p(0.0, 0.5), p(0.0, 5.0)]);
        let arc = j.between.unwrap();
        assert_eq!(arc.kind.as_draft().unwrap().0, DraftShape::Arc);
        let (path, _) = arc.draft_path().unwrap();
        assert!(path
            .iter()
            .all(|q| (q.distance(p(0.5, 0.5)) - 0.5).abs() < 1e-9));
        // radius 0 makes the corner sharp
        let j = join(&a, p(2.0, 0.0), &b, p(0.0, 2.0), false, Join::Fillet(0.0)).unwrap();
        same(&j.a.points[1..], &[p(0.0, 0.0)]);
        same(&j.b.unwrap().points[..1], &[p(0.0, 0.0)]);
        assert!(j.between.is_none());
        // a chamfer cuts it off
        let j = join(&a, p(2.0, 0.0), &b, p(0.0, 2.0), false, Join::Chamfer(0.3)).unwrap();
        same(&j.between.unwrap().points, &[p(0.3, 0.0), p(0.0, 0.3)]);
        // the corner of one rectangle: the outline gains the cut
        let sq = item(
            DraftShape::Closed,
            &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        );
        let j = join(&sq, p(2.0, 0.0), &sq, p(4.0, 2.0), true, Join::Chamfer(1.0)).unwrap();
        same(
            &j.a.points,
            &[
                p(0.0, 0.0),
                p(3.0, 0.0),
                p(4.0, 1.0),
                p(4.0, 4.0),
                p(0.0, 4.0),
            ],
        );
        // and the corner where it closes, rounded
        let j = join(&sq, p(0.0, 2.0), &sq, p(2.0, 0.0), true, Join::Fillet(1.0)).unwrap();
        assert!(j.a.points.len() > 5);
        assert!((j.a.length() - (16.0 - 2.0 + std::f64::consts::FRAC_PI_2)).abs() < 1e-3);
        // too large, parallel, or not a corner
        assert!(join(&a, p(2.0, 0.0), &b, p(0.0, 2.0), false, Join::Fillet(9.0)).is_err());
        let par = line(&[(0.0, 1.0), (5.0, 1.0)]);
        assert!(join(&a, p(2.0, 0.0), &par, p(2.0, 1.0), false, Join::Fillet(0.1)).is_err());
        assert!(join(&sq, p(2.0, 0.0), &sq, p(2.0, 4.0), true, Join::Fillet(0.1)).is_err());
    }

    #[test]
    fn offset_copies_to_the_side_clicked() {
        let l = line(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0)]);
        assert_eq!(
            offset(&l, 1.0, p(2.0, 3.0)).unwrap().points,
            vec![p(0.0, 1.0), p(3.0, 1.0), p(3.0, 4.0)]
        );
        assert_eq!(
            offset(&l, 1.0, p(2.0, -3.0)).unwrap().points[0],
            p(0.0, -1.0)
        );
        // a rectangle grows outside, shrinks inside, whichever way it was drawn
        for pts in [
            [(0.0, 0.0), (4.0, 0.0), (4.0, 2.0), (0.0, 2.0)],
            [(0.0, 2.0), (4.0, 2.0), (4.0, 0.0), (0.0, 0.0)],
        ] {
            let sq = item(DraftShape::Closed, &pts);
            assert!((offset(&sq, 0.5, p(9.0, 9.0)).unwrap().length() - 16.0).abs() < 1e-9);
            assert!((offset(&sq, 0.5, p(2.0, 1.0)).unwrap().length() - 8.0).abs() < 1e-9);
        }
        // a circle and an arc keep their centre
        let c = item(DraftShape::Circle, &[(1.0, 1.0), (3.0, 1.0)]);
        assert_eq!(offset(&c, 0.5, p(9.0, 1.0)).unwrap().points[1], p(3.5, 1.0));
        assert_eq!(offset(&c, 0.5, p(1.2, 1.0)).unwrap().points[1], p(2.5, 1.0));
        assert!(offset(&c, 5.0, p(1.2, 1.0)).is_none());
        let a = item(DraftShape::Arc, &[(-1.0, 0.0), (0.0, 1.0), (1.0, 0.0)]);
        let out = offset(&a, 1.0, p(0.0, 5.0)).unwrap();
        assert!(out.points.iter().all(|q| (q.length() - 2.0).abs() < 1e-9));
        // an ellipse grows on both axes
        let e = item(DraftShape::Ellipse, &[(0.0, 0.0), (3.0, 0.0), (0.0, 1.0)]);
        let out = offset(&e, 0.5, p(9.0, 0.0)).unwrap();
        assert_eq!((out.points[1], out.points[2]), (p(3.5, 0.0), p(0.0, 1.5)));
    }

    #[test]
    fn arrays_repeat_in_rows_and_round_a_centre() {
        let l = line(&[(0.0, 0.0), (1.0, 0.0)]);
        let grid = array_grid(&l, 3, 2, p(2.0, 5.0));
        assert_eq!(grid.len(), 6);
        assert_eq!(grid[0].points, l.points);
        assert_eq!(grid[5].points, vec![p(4.0, 5.0), p(5.0, 5.0)]);
        // four round a full turn: a quarter turn apart
        let round = array_polar(&l, p(0.0, 0.0), 4, std::f64::consts::TAU);
        assert_eq!(round.len(), 4);
        assert!(round[1].points[1].distance(p(0.0, 1.0)) < 1e-9);
        assert!(round[3].points[1].distance(p(0.0, -1.0)) < 1e-9);
        // three over a half turn: the last is at the half turn
        let half = array_polar(&l, p(0.0, 0.0), 3, std::f64::consts::PI);
        assert!(half[2].points[1].distance(p(-1.0, 0.0)) < 1e-9);
    }
}
