//! The joints between like faces: where two faces of one material carry on
//! in one plane (a wall on the wall below, a slab against a slab), the edge
//! each element draws there is no edge of the whole. The model styles leave
//! those lines out.

use crate::SceneItem;
use glam::{DVec3, Vec3};
use std::collections::HashMap;

/// A triangle with its plane.
struct Tri {
    p: [DVec3; 3],
    n: DVec3,
    d: f64,
}

impl Tri {
    /// Is `q` (on its plane) inside it, edges included?
    fn holds(&self, q: DVec3) -> bool {
        let eps = 1e-6;
        let [a, b, c] = self.p;
        let s = |u: DVec3, v: DVec3| (v - u).cross(q - u).dot(self.n);
        s(a, b) >= -eps && s(b, c) >= -eps && s(c, a) >= -eps
    }

    fn on_plane(&self, q: DVec3, tol: f64) -> bool {
        (self.n.dot(q) - self.d).abs() < tol
    }
}

/// The scene's faces, by item and by where they are, to find joints.
pub struct FlushIndex {
    /// Each item's triangles, and a key that is the same for items of
    /// one material (colour and surface).
    items: Vec<(Vec<Tri>, [u32; 6])>,
    /// Triangles (item, triangle) by grid cell.
    grid: HashMap<(i32, i32, i32), Vec<(u32, u32)>>,
    /// Triangles too big for the grid, looked at every time.
    big: Vec<(u32, u32)>,
}

const CELL: f64 = 0.5;
const MAX_CELLS: i64 = 4096;

fn cell_of(p: DVec3) -> (i32, i32, i32) {
    (
        (p.x / CELL).floor() as i32,
        (p.y / CELL).floor() as i32,
        (p.z / CELL).floor() as i32,
    )
}

impl FlushIndex {
    pub fn new(scene: &[SceneItem]) -> Self {
        let mut items = Vec::with_capacity(scene.len());
        let mut grid: HashMap<(i32, i32, i32), Vec<(u32, u32)>> = HashMap::new();
        let mut big = Vec::new();
        for (k, s) in scene.iter().enumerate() {
            let key = [
                s.color[0].to_bits(),
                s.color[1].to_bits(),
                s.color[2].to_bits(),
                s.surface[0].to_bits(),
                s.surface[1].to_bits(),
                s.surface[2].to_bits(),
            ];
            let mut tris = Vec::new();
            if !s.ghost {
                for t in s.mesh.indices.chunks(3) {
                    if t.len() < 3 {
                        continue;
                    }
                    let p = |i: u32| Vec3::from(s.mesh.positions[i as usize]).as_dvec3();
                    let p = [p(t[0]), p(t[1]), p(t[2])];
                    let n = (p[1] - p[0]).cross(p[2] - p[0]);
                    if n.length() < 1e-12 {
                        continue;
                    }
                    let n = n.normalize();
                    let ti = tris.len() as u32;
                    let lo = p[0].min(p[1]).min(p[2]);
                    let hi = p[0].max(p[1]).max(p[2]);
                    let (a, b) = (cell_of(lo), cell_of(hi));
                    let count =
                        (b.0 - a.0 + 1) as i64 * (b.1 - a.1 + 1) as i64 * (b.2 - a.2 + 1) as i64;
                    if count > MAX_CELLS {
                        big.push((k as u32, ti));
                    } else {
                        for x in a.0..=b.0 {
                            for y in a.1..=b.1 {
                                for z in a.2..=b.2 {
                                    grid.entry((x, y, z)).or_default().push((k as u32, ti));
                                }
                            }
                        }
                    }
                    tris.push(Tri {
                        p,
                        n,
                        d: n.dot(p[0]),
                    });
                }
            }
            items.push((tris, key));
        }
        Self { items, grid, big }
    }

    /// Is the edge `a`–`b` of item `item` a joint: along it, a face of the
    /// item carries on, in its plane, into a face of another item of the
    /// same material?
    pub fn is_joint(&self, item: usize, a: DVec3, b: DVec3) -> bool {
        let Some((own, key)) = self.items.get(item) else {
            return false;
        };
        let along = b - a;
        if along.length() < 1e-6 {
            return false;
        }
        // the item's faces the edge runs along
        let m = (a + b) / 2.0;
        let faces: Vec<&Tri> = own
            .iter()
            .filter(|t| t.on_plane(a, 1e-4) && t.on_plane(b, 1e-4) && t.holds(m))
            .collect();
        let step = 0.01;
        faces.iter().any(|f| {
            let mut across = f.n.cross(along).normalize_or_zero();
            if !f.holds(m - across * step) {
                across = -across;
            }
            // all along the edge, the plane carries on into a like face
            [0.2, 0.5, 0.8].iter().all(|t| {
                let q = a + along * *t + across * step;
                // not the item's own face carrying on (no edge there)
                if own
                    .iter()
                    .any(|o| o.n.dot(f.n) > 0.999 && o.on_plane(q, 2e-3) && o.holds(q))
                {
                    return false;
                }
                self.near(q).any(|(k, ti)| {
                    let (tris, other) = &self.items[k as usize];
                    if k as usize == item || other != key {
                        return false;
                    }
                    let o = &tris[ti as usize];
                    o.n.dot(f.n) > 0.999 && o.on_plane(q, 2e-3) && o.holds(q)
                })
            })
        })
    }

    fn near(&self, q: DVec3) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.grid
            .get(&cell_of(q))
            .into_iter()
            .flatten()
            .chain(self.big.iter())
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::TriMesh;

    fn block(lo: [f32; 3], hi: [f32; 3], color: [f32; 3]) -> SceneItem {
        let (l, h) = (Vec3::from(lo).as_dvec3(), Vec3::from(hi).as_dvec3());
        let c = |x: f64, y: f64, z: f64| DVec3::new(x, y, z);
        let mut mesh = TriMesh::default();
        mesh.add_quad(
            [
                c(l.x, l.y, l.z),
                c(h.x, l.y, l.z),
                c(h.x, l.y, h.z),
                c(l.x, l.y, h.z),
            ],
            -DVec3::Y,
        );
        mesh.add_quad(
            [
                c(l.x, h.y, l.z),
                c(l.x, h.y, h.z),
                c(h.x, h.y, h.z),
                c(h.x, h.y, l.z),
            ],
            DVec3::Y,
        );
        mesh.add_quad(
            [
                c(l.x, l.y, l.z),
                c(l.x, l.y, h.z),
                c(l.x, h.y, h.z),
                c(l.x, h.y, l.z),
            ],
            -DVec3::X,
        );
        mesh.add_quad(
            [
                c(h.x, l.y, l.z),
                c(h.x, h.y, l.z),
                c(h.x, h.y, h.z),
                c(h.x, l.y, h.z),
            ],
            DVec3::X,
        );
        mesh.add_quad(
            [
                c(l.x, l.y, l.z),
                c(l.x, h.y, l.z),
                c(h.x, h.y, l.z),
                c(h.x, l.y, l.z),
            ],
            -DVec3::Z,
        );
        mesh.add_quad(
            [
                c(l.x, l.y, h.z),
                c(h.x, l.y, h.z),
                c(h.x, h.y, h.z),
                c(l.x, h.y, h.z),
            ],
            DVec3::Z,
        );
        SceneItem {
            id: creabim_core::ElementId::new(),
            mesh,
            color,
            surface: crate::scene::PLAIN_SURFACE,
            ghost: false,
            layer: None,
            finish: None,
        }
    }

    #[test]
    fn a_wall_on_a_like_wall_has_no_joint_but_a_different_one_has() {
        let white = [0.9, 0.9, 0.9];
        let low = block([0.0, 0.0, 0.0], [5.0, 0.3, 3.0], white);
        let high = block([0.0, 0.0, 3.0], [5.0, 0.3, 6.0], white);
        let other = block([0.0, 0.0, 3.0], [5.0, 0.3, 6.0], [0.5, 0.3, 0.2]);
        // the top front edge of the lower wall, on its outer face
        let (a, b) = (DVec3::new(0.0, 0.0, 3.0), DVec3::new(5.0, 0.0, 3.0));
        let like = FlushIndex::new(&[low.clone(), high]);
        assert!(like.is_joint(0, a, b));
        assert!(like.is_joint(1, a, b));
        let unlike = FlushIndex::new(&[low.clone(), other]);
        assert!(!unlike.is_joint(0, a, b));
        // the wall's own corner is no joint
        let alone = FlushIndex::new(&[low]);
        assert!(!alone.is_joint(0, DVec3::ZERO, DVec3::new(0.0, 0.0, 3.0)));
    }
}
