//! The line of a channel at the local grid (GEN-186, REND-067; lead 2026-11-13): between two H1 vertices the line follows the lowest pass of the base `B` within a corridor around the straight step,
//! instead of the straight raster of the step. The bake burns the placed line and the drawing places it the same way from the bake's `B`, so the trench and the drawn river are one line. A function of
//! `B` and the two vertices only (partition independent, deterministic), with no format change: the stored graph keeps its H1 vertices.
//!
//! - A vertex is the lowest cell of `B` within [`SNAP_CELLS`] (Chebyshev) of the cell of its point, ties by `(y, x)`.
//! - A step from vertex `a` to vertex `b` runs through the cells whose centres lie within [`CORRIDOR_CELLS`] of the straight segment between the two points (and the two vertices themselves).
//! - Its cells minimise the highest `B` on the way (the lowest pass); among the paths under that pass, the summed height of `B` over the lowest of the corridor; ties by the cell order (`(y, x)`) of the
//!   predecessor. 8-connected.

use super::k7::Channel;

/// The half width of the corridor around the straight step, in local cells (half an H1 cell of 8 local cells; lead 2026-11-13).
pub const CORRIDOR_CELLS: f64 = 4.0;
/// The least cost of a step of the search, metres: a flat costs its length.
const STEP_COST_M: f64 = 1.0e-3;

/// How far a vertex may move from the cell of its point, in local cells (Chebyshev): within its H1 cell of 8.
pub const SNAP_CELLS: i64 = 3;

/// The placement of channel lines over a base `B` read at local face cells.
pub struct Placement<'a> {
    pub base: &'a (dyn Fn(i64, i64) -> f64 + Sync),
    pub corridor: f64,
    pub snap: i64,
}

fn cell_of(p: (f64, f64)) -> (i64, i64) {
    (libm::round(p.0) as i64, libm::round(p.1) as i64)
}

/// The order of the ties: `(y, x)`.
fn order(c: (i64, i64)) -> (i64, i64) {
    (c.1, c.0)
}

impl<'a> Placement<'a> {
    /// The standard placement (corridor 4 cells, snap 3 cells).
    pub fn new(base: &'a (dyn Fn(i64, i64) -> f64 + Sync)) -> Placement<'a> {
        Placement { base, corridor: CORRIDOR_CELLS, snap: SNAP_CELLS }
    }

    fn height(&self, c: (i64, i64)) -> f64 {
        let h = (self.base)(c.0, c.1);
        assert!(h.is_finite(), "the base at local cell ({}, {}) is {h}: not finite", c.0, c.1);
        h
    }

    /// The cell of a vertex: the lowest of `B` within `snap` cells of the point's cell, ties by `(y, x)`.
    pub fn vertex(&self, p: (f64, f64)) -> (i64, i64) {
        let (cx, cy) = cell_of(p);
        let mut best: Option<((i64, i64), f64)> = None;
        for y in cy - self.snap..=cy + self.snap {
            for x in cx - self.snap..=cx + self.snap {
                let h = self.height((x, y));
                if best.is_none_or(|(c, bh)| h < bh || (h == bh && order((x, y)) < order(c))) {
                    best = Some(((x, y), h));
                }
            }
        }
        best.expect("a cell").0
    }

    /// The cells of a step from vertex `a` to vertex `b` (both included), the step's straight segment from `p0` to `p1` (face cell-index space).
    pub fn step(&self, a: (i64, i64), b: (i64, i64), p0: (f64, f64), p1: (f64, f64)) -> Vec<(i64, i64)> {
        if a == b {
            return vec![a];
        }
        let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
        let len2 = dx * dx + dy * dy;
        let dist2 = |x: f64, y: f64| {
            let t = if len2 > 0.0 { (((x - p0.0) * dx + (y - p0.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let (ex, ey) = (x - (p0.0 + t * dx), y - (p0.1 + t * dy));
            ex * ex + ey * ey
        };
        let r = libm::ceil(self.corridor) as i64 + self.snap + 1;
        let (x0, x1) = (a.0.min(b.0).min(cell_of(p0).0).min(cell_of(p1).0) - r, a.0.max(b.0).max(cell_of(p0).0).max(cell_of(p1).0) + r);
        let (y0, y1) = (a.1.min(b.1).min(cell_of(p0).1).min(cell_of(p1).1) - r, a.1.max(b.1).max(cell_of(p0).1).max(cell_of(p1).1) + r);
        assert!(
            x1 - x0 <= 256 && y1 - y0 <= 256,
            "a step from {a:?} to {b:?} (points {p0:?} to {p1:?}) spans {} x {} cells: the vertices of a step are neighbouring H1 cells",
            x1 - x0 + 1,
            y1 - y0 + 1
        );
        let w = (x1 - x0 + 1) as usize;
        let n = w * (y1 - y0 + 1) as usize;
        let idx = |c: (i64, i64)| (c.1 - y0) as usize * w + (c.0 - x0) as usize;
        let cell = |i: usize| ((i % w) as i64 + x0, (i / w) as i64 + y0);
        let mut inside = vec![false; n];
        let mut h = vec![f64::INFINITY; n];
        for i in 0..n {
            let c = cell(i);
            if c == a || c == b || dist2(c.0 as f64, c.1 as f64) <= self.corridor * self.corridor {
                inside[i] = true;
                h[i] = self.height(c);
            }
        }
        const NB: [(i64, i64); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];
        let neighbours = |i: usize| {
            let c = cell(i);
            NB.iter().filter_map(move |d| {
                let q = (c.0 + d.0, c.1 + d.1);
                (q.0 >= x0 && q.0 <= x1 && q.1 >= y0 && q.1 <= y1).then(|| idx(q))
            })
        };
        // 1. The lowest pass: the smallest highest height of a path from a to b (a bottleneck search), ties by cell order.
        let mut pass = vec![f64::INFINITY; n];
        let mut heap = std::collections::BinaryHeap::new();
        let key = |v: f64, c: (i64, i64)| std::cmp::Reverse((OrdF(v), order(c)));
        pass[idx(a)] = h[idx(a)];
        heap.push(key(h[idx(a)], a));
        while let Some(std::cmp::Reverse((OrdF(v), (cy, cx)))) = heap.pop() {
            let i = idx((cx, cy));
            if v > pass[i] {
                continue;
            }
            for j in neighbours(i) {
                if !inside[j] {
                    continue;
                }
                let m = v.max(h[j]);
                if m < pass[j] {
                    pass[j] = m;
                    heap.push(key(m, cell(j)));
                }
            }
        }
        let top = pass[idx(b)];
        assert!(top.is_finite(), "the corridor connects the vertices");
        // 2. Under the pass, the path of the least summed height over the corridor's lowest, each step at least `STEP_COST_M` (so a cost rises strictly along a path: no predecessor cycle on a flat, and of
        // two paths of equal height the shorter), ties by the predecessor's cell order.
        let floor = h.iter().copied().filter(|v| v.is_finite()).fold(f64::INFINITY, f64::min);
        let mut cost = vec![f64::INFINITY; n];
        let mut prev = vec![usize::MAX; n];
        cost[idx(a)] = h[idx(a)] - floor + STEP_COST_M;
        heap.push(key(cost[idx(a)], a));
        while let Some(std::cmp::Reverse((OrdF(v), (cy, cx)))) = heap.pop() {
            let i = idx((cx, cy));
            if v > cost[i] {
                continue;
            }
            for j in neighbours(i) {
                if !inside[j] || h[j] > top {
                    continue;
                }
                let c = v + (h[j] - floor) + STEP_COST_M;
                if c < cost[j] || (c == cost[j] && prev[j] != usize::MAX && order(cell(i)) < order(cell(prev[j]))) {
                    let better = c < cost[j];
                    cost[j] = c;
                    prev[j] = i;
                    if better {
                        heap.push(key(c, cell(j)));
                    }
                }
            }
        }
        let mut out = vec![b];
        let mut at = idx(b);
        while at != idx(a) {
            at = prev[at];
            assert!(at != usize::MAX, "a path under the pass reaches the vertex");
            out.push(cell(at));
        }
        out.reverse();
        out
    }

    /// The channel with its line placed: one point per local cell of the placed line, the levels and discharges interpolated along each step by the cell's index in it. K7's burn, its fixed cells and
    /// its final pass, and the drawing, read the placed channel as they read any other.
    pub fn channel(&self, ch: &Channel) -> Channel {
        assert!(ch.points.len() == ch.levels.len() && !ch.points.is_empty(), "a level for every point");
        let verts: Vec<(i64, i64)> = ch.points.iter().map(|&p| self.vertex(p)).collect();
        let mut out = Channel { points: Vec::new(), levels: Vec::new(), q: Vec::new(), q_unit: ch.q_unit };
        let has_q = !ch.q.is_empty();
        let mut push = |c: (i64, i64), level: f64, q: f64| {
            let p = (c.0 as f64, c.1 as f64);
            // A cell the line already holds (the next step runs back through the last one's cells): the loop between the two visits is cut, the cell keeping the lowest level and the largest
            // discharge, so the line is simple and every cell of it is one step of the profile.
            if let Some(k) = out.points.iter().rposition(|&q| q == p) {
                let lowest = out.levels[k..].iter().copied().fold(level, f64::min);
                out.points.truncate(k + 1);
                out.levels.truncate(k + 1);
                out.levels[k] = lowest;
                if has_q {
                    let most = out.q[k..].iter().copied().fold(q, f64::max);
                    out.q.truncate(k + 1);
                    out.q[k] = most;
                }
            } else {
                out.points.push((c.0 as f64, c.1 as f64));
                out.levels.push(level);
                if has_q {
                    out.q.push(q);
                }
            }
        };
        let qk = |k: usize| if has_q { ch.q[k] } else { 0.0 };
        push(verts[0], ch.levels[0], qk(0));
        for k in 1..verts.len() {
            let cells = self.step(verts[k - 1], verts[k], ch.points[k - 1], ch.points[k]);
            let m = cells.len();
            for (i, c) in cells.into_iter().enumerate().skip(1) {
                let t = i as f64 / (m - 1) as f64;
                push(c, ch.levels[k - 1] + (ch.levels[k] - ch.levels[k - 1]) * t, qk(k - 1) + (qk(k) - qk(k - 1)) * t);
            }
        }
        out
    }
}

/// An f64 with a total order (`total_cmp`) for the search's heap.
#[derive(Clone, Copy, Debug, PartialEq)]
struct OrdF(f64);

impl Eq for OrdF {}

impl PartialOrd for OrdF {
    fn partial_cmp(&self, o: &OrdF) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for OrdF {
    fn cmp(&self, o: &OrdF) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0)
    }
}
