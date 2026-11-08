//! H0: one drainage structure over the whole planet (GEN-139, SPIKE-105 F6): the six cube faces as one graph of `6 n^2` cells, neighbours joined across the cube edges, one priority flood from the
//! sea, receivers and the true drainage area (the physical cell metric of GEN-138). A catchment that crosses a cube edge is one catchment, not two face-local ones joined afterwards.
//!
//! The cell `(face, i, j)` has the id `face n^2 + j n + i`. Inside a face a cell has its 8 neighbours; across a cube edge it has the one axis neighbour of the face it meets (the cells along a
//! shared edge pair 1:1, because the equiangular tangent coordinate along an edge is the same in both faces); a diagonal across an edge or a corner is not an edge of the graph. The kernels are
//! single-threaded with a fixed visiting order (the flood pops by height then cell id), so the result is the same bits on any machine (CON-14).

use super::metric::Metric;
use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// The cells of the whole planet at `n` cells a face edge.
#[derive(Clone, Debug)]
pub struct GlobalGrid {
    n: usize,
    radius_m: f64,
    metric: Metric,
    /// The tangent coordinate of each cell centre along an axis, and the face frames `(n, u, v)`: a direction is `(n + x u + y v)` normalised, no trigonometry per call.
    tan_c: Vec<f64>,
    frames: [[[f64; 3]; 3]; 6],
}

/// The routing of the whole planet.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalRouting {
    /// The receiver of each cell, or `u32::MAX` for an outlet.
    pub receiver: Vec<u32>,
    /// The true drainage area of each cell in square metres (its own area plus its donors').
    pub area: Vec<f64>,
    /// The outlet each cell drains to.
    pub outlet_of: Vec<u32>,
}

pub const NONE: u32 = u32::MAX;

impl GlobalGrid {
    pub fn new(n: usize, radius_m: f64) -> GlobalGrid {
        assert!(n >= 2, "a face has at least 2 cells an edge");
        let tan_c =
            (0..n).map(|k| planet_core::cube::FaceMapping::face_to_plane(&TangentWarp, 2.0 * (k as f64 + 0.5) / n as f64 - 1.0)).collect();
        // The face frame from the mapping itself: the direction at the plane point (0, 0) is the normal `n`, and at `x = 1` (resp. `y = 1`) it is `(n + u) / sqrt 2` (resp. `v`).
        let frames = std::array::from_fn(|f| {
            let face = Face(f as u8);
            let (c, a, b) = (
                face_to_direction(&TangentWarp, face, 0.0, 0.0),
                face_to_direction(&TangentWarp, face, 1.0, 0.0),
                face_to_direction(&TangentWarp, face, 0.0, 1.0),
            );
            let r = core::f64::consts::SQRT_2;
            [[c.x, c.y, c.z], [r * a.x - c.x, r * a.y - c.y, r * a.z - c.z], [r * b.x - c.x, r * b.y - c.y, r * b.z - c.z]]
        });
        GlobalGrid { n, radius_m, metric: Metric::cube_face(n, radius_m, (0, 0), n, n), tan_c, frames }
    }

    pub fn n(&self) -> usize {
        self.n
    }

    pub fn len(&self) -> usize {
        6 * self.n * self.n
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn id(&self, face: u8, i: usize, j: usize) -> u32 {
        (usize::from(face) * self.n * self.n + j * self.n + i) as u32
    }

    pub fn split(&self, id: u32) -> (u8, usize, usize) {
        let (nn, id) = (self.n * self.n, id as usize);
        ((id / nn) as u8, id % self.n, (id % nn) / self.n)
    }

    /// The plane coordinate (-1 to 1) of the centre of index `k`.
    fn centre(&self, k: usize) -> f64 {
        2.0 * (k as f64 + 0.5) / self.n as f64 - 1.0
    }

    /// The unit direction of the centre of cell `id`.
    pub fn direction(&self, id: u32) -> Vec3 {
        let (f, i, j) = self.split(id);
        let [n, u, v] = self.frames[usize::from(f)];
        let (x, y) = (self.tan_c[i], self.tan_c[j]);
        let (a, b, c) = (n[0] + x * u[0] + y * v[0], n[1] + x * u[1] + y * v[1], n[2] + x * u[2] + y * v[2]);
        let s = 1.0 / libm::sqrt(a * a + b * b + c * c);
        Vec3::new(a * s, b * s, c * s)
    }

    /// The area of cell `id` in square metres.
    pub fn area(&self, id: u32) -> f64 {
        let (_, i, j) = self.split(id);
        self.metric.area(i, j)
    }

    /// The cell of face `f` whose index along an axis holds the plane coordinate `s`.
    fn cell_of(&self, s: f64) -> usize {
        (((s + 1.0) * 0.5 * self.n as f64).floor().max(0.0) as usize).min(self.n - 1)
    }

    /// The axis neighbour of `id` in direction `(di, dj)`, across a cube edge when it leaves the face.
    fn axis_neighbour(&self, id: u32, di: i64, dj: i64) -> u32 {
        let (f, i, j) = self.split(id);
        let (ni, nj) = (i as i64 + di, j as i64 + dj);
        if ni >= 0 && nj >= 0 && (ni as usize) < self.n && (nj as usize) < self.n {
            return self.id(f, ni as usize, nj as usize);
        }
        // A point just past the edge at the cell's tangential centre, mapped into the face it lands in.
        let (mut s, mut t) = (self.centre(i), self.centre(j));
        const PAST: f64 = 1e-6;
        if di != 0 {
            s = di as f64 * (1.0 + PAST);
        } else {
            t = dj as f64 * (1.0 + PAST);
        }
        let (f2, s2, t2) = direction_to_face(&TangentWarp, face_to_direction(&TangentWarp, Face(f), s, t));
        let (a, b) = if s2.abs() >= t2.abs() {
            (if s2 > 0.0 { self.n - 1 } else { 0 }, self.cell_of(t2))
        } else {
            (self.cell_of(s2), if t2 > 0.0 { self.n - 1 } else { 0 })
        };
        self.id(f2.0, a, b)
    }

    /// The neighbours of `id` (no distances): the 8 of a cell inside a face, the axis ones across an edge. Returns how many are in `out`.
    pub fn neighbour_ids(&self, id: u32, out: &mut [u32; 8]) -> usize {
        let (f, i, j) = self.split(id);
        let mut k = 0;
        for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
            out[k] = self.axis_neighbour(id, di, dj);
            k += 1;
        }
        for (di, dj) in [(-1i64, -1i64), (1, -1), (-1, 1), (1, 1)] {
            let (ni, nj) = (i as i64 + di, j as i64 + dj);
            if ni >= 0 && nj >= 0 && (ni as usize) < self.n && (nj as usize) < self.n {
                out[k] = self.id(f, ni as usize, nj as usize);
                k += 1;
            }
        }
        k
    }

    /// The great-circle distance from the direction `here` to the centre of cell `q`.
    fn arc(&self, here: Vec3, q: u32) -> f64 {
        let d = self.direction(q);
        let (dx, dy, dz) = (here.x - d.x, here.y - d.y, here.z - d.z);
        let chord = libm::sqrt(dx * dx + dy * dy + dz * dz);
        self.radius_m * 2.0 * libm::asin((0.5 * chord).min(1.0))
    }

    /// The neighbours of `id` with the distance to each (great circle between the centres).
    pub fn neighbours(&self, id: u32) -> Vec<(u32, f64)> {
        let mut ids = [0u32; 8];
        let k = self.neighbour_ids(id, &mut ids);
        let here = self.direction(id);
        ids[..k].iter().map(|&q| (q, self.arc(here, q))).collect()
    }

    /// Routes the whole planet: the priority flood from the `outlet` cells (the sea; `epsilon` over flats), the steepest receiver on the filled surface, and the true area accumulated in
    /// descending order of the filled height (ties by id).
    pub fn route(&self, z: &[f64], outlet: &[bool], epsilon: f64) -> GlobalRouting {
        let n = self.len();
        assert!(z.len() == n && outlet.len() == n, "one height and one outlet flag per cell");
        assert!(z.iter().all(|v| v.is_finite()), "the heights are finite");
        assert!(outlet.iter().any(|o| *o), "a planet needs an outlet");
        let key = |v: f64| {
            let b = v.to_bits();
            if b & (1 << 63) != 0 {
                !b
            } else {
                b | (1 << 63)
            }
        };
        let mut filled = vec![f64::INFINITY; n];
        let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
        for c in 0..n {
            if outlet[c] {
                filled[c] = z[c];
                heap.push(Reverse((key(z[c]), c as u32)));
            }
        }
        let mut done = vec![false; n];
        let mut order: Vec<u32> = Vec::with_capacity(n);
        while let Some(Reverse((_, c))) = heap.pop() {
            if done[c as usize] {
                continue;
            }
            done[c as usize] = true;
            order.push(c);
            let mut ids = [0u32; 8];
            let k = self.neighbour_ids(c, &mut ids);
            for &q in &ids[..k] {
                if !done[q as usize] && filled[q as usize].is_infinite() {
                    filled[q as usize] = z[q as usize].max(filled[c as usize] + epsilon);
                    heap.push(Reverse((key(filled[q as usize]), q)));
                }
            }
        }
        assert_eq!(order.len(), n, "the flood did not reach every cell");
        // Receivers: the steepest descent on the filled surface; the flood order guarantees a lower neighbour.
        let mut receiver = vec![NONE; n];
        for c in 0..n {
            if outlet[c] {
                continue;
            }
            let mut best = (0.0f64, NONE);
            let (mut ids, here) = ([0u32; 8], self.direction(c as u32));
            let k = self.neighbour_ids(c as u32, &mut ids);
            for &q in &ids[..k] {
                if filled[q as usize] >= filled[c] {
                    continue;
                }
                let d = self.arc(here, q);
                let s = (filled[c] - filled[q as usize]) / d;
                if s > best.0 {
                    best = (s, q);
                }
            }
            assert!(best.1 != NONE, "cell {c} has no lower neighbour after the flood");
            receiver[c] = best.1;
        }
        // Area down the flood order reversed (highest first), and the outlet each cell drains to by the flood order (a receiver is flooded before its donor).
        let mut area: Vec<f64> = (0..n as u32).map(|c| self.area(c)).collect();
        for &c in order.iter().rev() {
            let r = receiver[c as usize];
            if r != NONE {
                area[r as usize] += area[c as usize];
            }
        }
        let mut outlet_of = vec![NONE; n];
        for &c in &order {
            outlet_of[c as usize] = if outlet[c as usize] { c } else { outlet_of[receiver[c as usize] as usize] };
        }
        GlobalRouting { receiver, area, outlet_of }
    }
}
