//! HATCH's pick inside (AutoCAD's BOUNDARY): the smallest area round a
//! point closed by any lines, however many pieces they are in and wherever
//! they cross, with loose ends left out.

use glam::DVec2;
use std::collections::HashMap;

/// Nodes closer than this are one (metres): a tenth of a millimetre.
const SNAP: f64 = 1e-4;

/// The outline round `p` made by `segs`, anticlockwise; none when `p` is in
/// no closed area.
pub fn around(segs: &[(DVec2, DVec2)], p: DVec2) -> Option<Vec<DVec2>> {
    let segs: Vec<(DVec2, DVec2)> = segs
        .iter()
        .copied()
        .filter(|(a, b)| a.distance(*b) > SNAP && a.is_finite() && b.is_finite())
        .collect();
    // every piece split where another crosses or touches it
    let mut cuts: Vec<Vec<f64>> = vec![vec![0.0, 1.0]; segs.len()];
    let lo = |s: &(DVec2, DVec2)| s.0.x.min(s.1.x);
    let mut order: Vec<usize> = (0..segs.len()).collect();
    order.sort_by(|a, b| lo(&segs[*a]).total_cmp(&lo(&segs[*b])));
    for (k, &i) in order.iter().enumerate() {
        let s = segs[i];
        let x_hi = s.0.x.max(s.1.x) + SNAP;
        let (y_lo, y_hi) = (s.0.y.min(s.1.y) - SNAP, s.0.y.max(s.1.y) + SNAP);
        for &j in &order[k + 1..] {
            let t = segs[j];
            if lo(&t) > x_hi {
                break;
            }
            if t.0.y.max(t.1.y) < y_lo || t.0.y.min(t.1.y) > y_hi {
                continue;
            }
            if let Some((u, v)) = cross(s, t) {
                cuts[i].push(u);
                cuts[j].push(v);
            }
        }
    }
    // the graph: nodes by their rounded place, edges between them
    let key = |q: DVec2| ((q.x / SNAP).round() as i64, (q.y / SNAP).round() as i64);
    let mut nodes: Vec<DVec2> = vec![];
    let mut index: HashMap<(i64, i64), usize> = HashMap::new();
    let mut node = |q: DVec2, nodes: &mut Vec<DVec2>| {
        *index.entry(key(q)).or_insert_with(|| {
            nodes.push(q);
            nodes.len() - 1
        })
    };
    let mut edges: Vec<(usize, usize)> = vec![];
    for (i, (a, b)) in segs.iter().enumerate() {
        let mut ts = std::mem::take(&mut cuts[i]);
        ts.sort_by(f64::total_cmp);
        let mut prev: Option<usize> = None;
        for t in ts {
            let n = node(*a + (*b - *a) * t, &mut nodes);
            if let Some(m) = prev {
                if m != n {
                    edges.push((m.min(n), m.max(n)));
                }
            }
            prev = Some(n);
        }
    }
    edges.sort_unstable();
    edges.dedup();
    // loose ends can't close anything: pruned, again and again
    let mut adj: Vec<Vec<usize>> = vec![vec![]; nodes.len()];
    for (a, b) in &edges {
        adj[*a].push(*b);
        adj[*b].push(*a);
    }
    let mut stack: Vec<usize> = (0..nodes.len()).filter(|n| adj[*n].len() == 1).collect();
    while let Some(n) = stack.pop() {
        if adj[n].len() != 1 {
            continue;
        }
        let m = adj[n][0];
        adj[n].clear();
        adj[m].retain(|x| *x != n);
        if adj[m].len() == 1 {
            stack.push(m);
        }
    }
    // each node's edges anticlockwise
    let angle = |a: usize, b: usize| (nodes[b] - nodes[a]).to_angle();
    for (n, list) in adj.iter_mut().enumerate() {
        list.sort_by(|x, y| angle(n, *x).total_cmp(&angle(n, *y)));
    }
    // the edges a ray from `p` to the right meets, nearest first
    let mut hits: Vec<(f64, usize, usize)> = vec![];
    for (a, list) in adj.iter().enumerate() {
        for &b in list {
            if a >= b {
                continue;
            }
            let (pa, pb) = (nodes[a], nodes[b]);
            if (pa.y > p.y) == (pb.y > p.y) {
                continue;
            }
            let x = pa.x + (p.y - pa.y) * (pb.x - pa.x) / (pb.y - pa.y);
            if x > p.x {
                // the way up has `p` on its left
                let (u, v) = if pa.y < pb.y { (a, b) } else { (b, a) };
                hits.push((x - p.x, u, v));
            }
        }
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, u, v) in hits.into_iter().take(64) {
        let Some(face) = trace(&nodes, &adj, u, v) else {
            continue;
        };
        if signed_area(&face) > 0.0 && inside(&face, p) {
            return Some(face);
        }
    }
    None
}

/// The face on the left of the edge `u` to `v`: at each node the next
/// edge turning most to the right... of those going on, which keeps the
/// face on the left.
fn trace(nodes: &[DVec2], adj: &[Vec<usize>], u: usize, v: usize) -> Option<Vec<DVec2>> {
    let (start, mut a, mut b) = ((u, v), u, v);
    let mut out = vec![nodes[u]];
    for _ in 0..200_000 {
        out.push(nodes[b]);
        let list = &adj[b];
        let back = list.iter().position(|x| *x == a)?;
        // the edge just clockwise of the way back
        let next = list[(back + list.len() - 1) % list.len()];
        a = b;
        b = next;
        if (a, b) == start {
            out.pop();
            return Some(out);
        }
    }
    None
}

/// Where two pieces meet, as fractions along each.
fn cross(s: (DVec2, DVec2), t: (DVec2, DVec2)) -> Option<(f64, f64)> {
    let (r, q) = (s.1 - s.0, t.1 - t.0);
    let d = r.perp_dot(q);
    if d.abs() < 1e-15 {
        return None;
    }
    let w = t.0 - s.0;
    let u = w.perp_dot(q) / d;
    let v = w.perp_dot(r) / d;
    let (eu, ev) = (SNAP / r.length(), SNAP / q.length());
    ((-eu..=1.0 + eu).contains(&u) && (-ev..=1.0 + ev).contains(&v))
        .then_some((u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)))
}

fn signed_area(poly: &[DVec2]) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| poly[i].perp_dot(poly[(i + 1) % n]))
        .sum::<f64>()
        / 2.0
}

fn inside(poly: &[DVec2], p: DVec2) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) {
            c = !c;
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(x: f64, y: f64) -> DVec2 {
        DVec2::new(x, y)
    }

    #[test]
    fn four_separate_lines_crossing_close_an_area() {
        // a # sign: the middle square is closed by four long lines
        let segs = [
            (d(-1.0, 0.0), d(4.0, 0.0)),
            (d(-1.0, 2.0), d(4.0, 2.0)),
            (d(0.0, -1.0), d(0.0, 3.0)),
            (d(3.0, -1.0), d(3.0, 3.0)),
        ];
        let f = around(&segs, d(1.0, 1.0)).unwrap();
        assert!((signed_area(&f) - 6.0).abs() < 1e-9);
        // outside it nothing is closed
        assert!(around(&segs, d(-0.5, 1.0)).is_none());
    }

    #[test]
    fn a_wall_splitting_a_room_gives_the_half_picked() {
        let segs = [
            (d(0.0, 0.0), d(10.0, 0.0)),
            (d(10.0, 0.0), d(10.0, 5.0)),
            (d(10.0, 5.0), d(0.0, 5.0)),
            (d(0.0, 5.0), d(0.0, 0.0)),
            // across, touching both sides, and a stub going nowhere
            (d(4.0, 0.0), d(4.0, 5.0)),
            (d(6.0, 2.0), d(8.0, 2.0)),
        ];
        let f = around(&segs, d(7.0, 4.0)).unwrap();
        assert!((signed_area(&f) - 30.0).abs() < 1e-9, "{}", signed_area(&f));
        let f = around(&segs, d(1.0, 1.0)).unwrap();
        assert!((signed_area(&f) - 20.0).abs() < 1e-9);
    }
}
