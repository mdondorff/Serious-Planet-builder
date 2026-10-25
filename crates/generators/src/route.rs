//! Local main-stem routing (GEN-066, GEN-067, ADR 0081): the polyline of a coarse river segment is routed over the base's ground instead of followed
//! as the coarse path. Every part of it is a function of position alone (the coarse cells, the run's receivers and areas, the base field), so two
//! chunks that see the same segment find the same polyline, and `libm` and integer costs only (CON-14).
//!
//! 1. The node of a river's coarse cell (its centre in the coarse path) slides to the lowest base ground within a radius of the centre (a head, the
//!    first cell of a river with no river upstream, to the highest ground within its own radius: a river does not begin in a pit). All segments
//!    that meet at a cell share its node, so a confluence and a chunk border agree without a rule about borders.
//! 2. The segment from node to node is the least-cost path over the cells within a band `W` of the coarse guide (the guide is the segment points of the
//!    two nodes, with the position-only meander), on a raster of every `stride`-th cell of the chart of the first node's face. The cost of a step
//!    is the rise in millimetres plus `lambda` times its length; ties go to the lower cell index.
//!
//! `route_band_m` zero and no slide is the coarse path of before (`segment_points` between the cell centres).

use crate::anet::{segment_points, AnchorLaw, BaseDetail, CoarseRun};
use crate::chunkfield::ChunkLattice;
use crate::coupled::NONE as COARSE_NONE;
use planet_core::Face;
use planet_core::Vec3;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::sync::{Arc, Mutex};

/// The points of a routed segment.
type Polyline = Arc<Vec<Vec3>>;

/// The most routed segments a [`RouteMemo`] keeps (about 125 points of 24 bytes each): over it the memo is cleared.
const MEMO_SEGMENTS: usize = 4096;

/// A memo of the nodes, relief factors and routed polylines of one run, base and law (what a stage keeps between its chunks, so a segment the chunks share
/// is routed once). Every value is a pure function of its key under those three, so a hit is the value a miss would compute, bit for bit; a different law
/// empties it.
#[derive(Default)]
pub struct RouteMemo {
    law_key: Mutex<Option<u64>>,
    nodes: Mutex<BTreeMap<usize, Vec3>>,
    reliefs: Mutex<BTreeMap<usize, f64>>,
    lines: Mutex<BTreeMap<(usize, usize), Polyline>>,
}

impl RouteMemo {
    /// Empties the memo when `law` is not the law it was filled under.
    fn for_law(&self, law: &AnchorLaw) {
        let key = planet_core::hash::hash_u64s(
            0x0072_6f75_7465,
            &[
                law.route_band_m.to_bits(),
                law.route_lambda.to_bits(),
                u64::from(law.route_stride),
                law.slide_m.to_bits(),
                law.slide_steep_m.to_bits(),
                law.head_m.to_bits(),
                law.river_area_m2.to_bits(),
                law.relief_full_m.to_bits(),
            ],
        );
        let mut current = self.law_key.lock().expect("memo");
        if *current != Some(key) {
            *current = Some(key);
            self.nodes.lock().expect("memo").clear();
            self.reliefs.lock().expect("memo").clear();
            self.lines.lock().expect("memo").clear();
        }
    }
}

/// The routing of the segments of one network build: a memo of pure functions of the coarse cell and the segment, so a segment is routed once however
/// many segments and chains ask for it.
pub struct Router<'a> {
    run: &'a dyn CoarseRun,
    base: &'a dyn BaseDetail,
    lat: &'a ChunkLattice,
    radius_m: f64,
    law: &'a AnchorLaw,
    memo: &'a RouteMemo,
}

impl<'a> Router<'a> {
    pub fn new(
        run: &'a dyn CoarseRun,
        base: &'a dyn BaseDetail,
        lat: &'a ChunkLattice,
        radius_m: f64,
        law: &'a AnchorLaw,
        memo: &'a RouteMemo,
    ) -> Router<'a> {
        memo.for_law(law);
        Router { run, base, lat, radius_m, law, memo }
    }

    /// Whether the routing changes anything (a band or a slide is set).
    pub fn enabled(&self) -> bool {
        self.law.route_band_m > 0.0 || self.law.slide_m > 0.0 || self.law.slide_steep_m > 0.0 || self.law.head_m > 0.0
    }

    fn is_river(&self, c: usize) -> bool {
        f64::from(self.run.area_m2(c)) > self.law.river_area_m2 && self.run.receiver(c) != COARSE_NONE
    }

    fn is_head(&self, c: usize) -> bool {
        !self.run.lattice().neighbours(c).into_iter().any(|nb| self.is_river(nb as usize) && self.run.receiver(nb as usize) as usize == c)
    }

    /// The relief factor round the coarse cell `c`: the base at its centre and the eight round it.
    pub fn relief_factor(&self, c: usize) -> f64 {
        if let Some(&r) = self.memo.reliefs.lock().expect("memo").get(&c) {
            return r;
        }
        let coarse = self.run.lattice();
        let mut ring: Vec<Vec3> = vec![coarse.direction(c)];
        ring.extend(coarse.neighbours(c).into_iter().filter(|&x| (x as usize) < coarse.len()).map(|x| coarse.direction(x as usize)));
        let h = self.base.base_heights(&ring);
        let (lo, hi) = h.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, u), &x| (l.min(x), u.max(x)));
        let rf = self.law.relief_factor(hi - lo);
        self.memo.reliefs.lock().expect("memo").insert(c, rf);
        rf
    }

    /// The direction of the node of coarse cell `c` (see the module comment).
    pub fn node(&self, c: usize) -> Vec3 {
        if let Some(&d) = self.memo.nodes.lock().expect("memo").get(&c) {
            return d;
        }
        let centre = self.run.lattice().direction(c);
        let d = if !self.enabled() || !self.is_river(c) {
            centre
        } else if self.is_head(c) {
            if self.law.head_m > 0.0 {
                self.extreme(centre, self.law.head_m, true)
            } else {
                centre
            }
        } else {
            let rf = self.relief_factor(c);
            let radius = self.law.slide_m + (self.law.slide_steep_m - self.law.slide_m) * rf;
            if radius > 0.0 {
                self.extreme(centre, radius, false)
            } else {
                centre
            }
        };
        self.memo.nodes.lock().expect("memo").insert(c, d);
        d
    }

    /// The raster step in cells and in metres.
    fn stride(&self) -> (i64, f64) {
        let s = i64::from(self.law.route_stride.max(1));
        (s, s as f64 * self.lat.cell_m(self.radius_m))
    }

    /// The centre of the cell of the lowest (or highest) base ground among the raster points within `radius_m` of `d`: the first of equals in the order
    /// of the raster.
    fn extreme(&self, d: Vec3, radius_m: f64, highest: bool) -> Vec3 {
        let cell = self.lat.cell_at(d);
        let (s, step_m) = self.stride();
        let r = (radius_m / step_m).floor() as i64;
        let mut dirs: Vec<Vec3> = Vec::new();
        for a in -r..=r {
            for b in -r..=r {
                if ((a * a + b * b) as f64) * step_m * step_m <= radius_m * radius_m {
                    dirs.push(self.lat.direction_ext(cell.face, i64::from(cell.i) + a * s, i64::from(cell.j) + b * s));
                }
            }
        }
        let h = self.base.base_heights(&dirs);
        let mut best = 0;
        for k in 1..dirs.len() {
            if (highest && h[k] > h[best]) || (!highest && h[k] < h[best]) {
                best = k;
            }
        }
        dirs[best]
    }

    /// The polyline of the coarse segment `from` to `to`: the first point is the node of `from`, the last the node of `to`.
    pub fn polyline(&self, from: usize, to: usize) -> Polyline {
        if let Some(l) = self.memo.lines.lock().expect("memo").get(&(from, to)) {
            return Arc::clone(l);
        }
        let coarse = self.run.lattice();
        let line = if !self.enabled() {
            segment_points(coarse.direction(from), coarse.direction(to), from as u64, self.radius_m)
        } else {
            let (a, b) = (self.node(from), self.node(to));
            let guide = segment_points(a, b, from as u64, self.radius_m);
            if self.law.route_band_m > 0.0 {
                self.route(&guide)
            } else {
                guide
            }
        };
        let line = Arc::new(line);
        let mut lines = self.memo.lines.lock().expect("memo");
        if lines.len() >= MEMO_SEGMENTS {
            lines.clear();
        }
        lines.insert((from, to), Arc::clone(&line));
        line
    }

    /// The chart coordinates (in cells, fractional) of `d` on `face`: the gnomonic plane of the face read in the lattice's tangent warp, beyond the edge too.
    fn chart(&self, face: u8, d: Vec3) -> (f64, f64) {
        let f = Face(face);
        let w = d.dot(f.normal());
        let (u, v) = (d.dot(f.u_axis()) / w, d.dot(f.v_axis()) / w);
        let to_s = |p: f64| libm::atan(p) * (4.0 / core::f64::consts::PI);
        let n = self.lat.n() as f64;
        (((to_s(u) + 1.0) * n - 1.0) * 0.5, ((to_s(v) + 1.0) * n - 1.0) * 0.5)
    }

    /// The least-cost path over the band round `guide` from its first to its last point.
    fn route(&self, guide: &[Vec3]) -> Vec<Vec3> {
        let (first, last) = (guide[0], guide[guide.len() - 1]);
        let cell0 = self.lat.cell_at(first);
        let (s, step_m) = self.stride();
        let (i0, j0) = (i64::from(cell0.i), i64::from(cell0.j));
        let grid = |d: Vec3| {
            let (fi, fj) = self.chart(cell0.face, d);
            (((fi - i0 as f64) / s as f64), ((fj - j0 as f64) / s as f64))
        };
        let g: Vec<(f64, f64)> = guide.iter().map(|&d| grid(d)).collect();
        let reach = (self.law.route_band_m / step_m).ceil() as i64;
        let (mut x0, mut x1, mut y0, mut y1) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
        for &(x, y) in &g {
            x0 = x0.min(libm::floor(x) as i64);
            x1 = x1.max(libm::ceil(x) as i64);
            y0 = y0.min(libm::floor(y) as i64);
            y1 = y1.max(libm::ceil(y) as i64);
        }
        let (x0, x1, y0, y1) = (x0 - reach - 1, x1 + reach + 1, y0 - reach - 1, y1 + reach + 1);
        let (w, h) = ((x1 - x0 + 1) as usize, (y1 - y0 + 1) as usize);
        let at = |x: i64, y: i64| (y - y0) as usize * w + (x - x0) as usize;
        // The band: every grid point within the band of a guide point.
        let mut inband = vec![false; w * h];
        let r2 = (self.law.route_band_m / step_m) * (self.law.route_band_m / step_m);
        for &(gx, gy) in &g {
            let (cx, cy) = (libm::round(gx) as i64, libm::round(gy) as i64);
            for y in cy - reach..=cy + reach {
                for x in cx - reach..=cx + reach {
                    let (dx, dy) = (x as f64 - gx, y as f64 - gy);
                    if dx * dx + dy * dy <= r2 {
                        inband[at(x, y)] = true;
                    }
                }
            }
        }
        let (sx, sy) = (0i64, 0i64);
        let (ex, ey) = {
            let (x, y) = grid(last);
            (libm::round(x) as i64, libm::round(y) as i64)
        };
        inband[at(sx, sy)] = true;
        inband[at(ex, ey)] = true;
        // The heights of the band in one batch.
        let mut index = vec![usize::MAX; w * h];
        let mut dirs: Vec<Vec3> = Vec::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                if inband[at(x, y)] {
                    index[at(x, y)] = dirs.len();
                    dirs.push(self.lat.direction_ext(cell0.face, i0 + x * s, j0 + y * s));
                }
            }
        }
        let z = self.base.base_heights(&dirs);
        // Dijkstra with integer costs (millimetres): the rise plus `lambda` times the length, ties to the lower cell index.
        let mut cost = vec![i64::MAX; w * h];
        let mut from = vec![usize::MAX; w * h];
        let mut heap: BinaryHeap<Reverse<(i64, usize)>> = BinaryHeap::new();
        let start = at(sx, sy);
        let goal = at(ex, ey);
        cost[start] = 0;
        heap.push(Reverse((0, start)));
        let len_mm = |diag: bool| (step_m * if diag { core::f64::consts::SQRT_2 } else { 1.0 } * 1000.0).round() as i64;
        let lambda_mm = |len: i64| (len as f64 * self.law.route_lambda).round() as i64;
        let (straight, diagonal) = (len_mm(false), len_mm(true));
        while let Some(Reverse((c, u))) = heap.pop() {
            if c > cost[u] {
                continue;
            }
            if u == goal {
                break;
            }
            let (ux, uy) = ((u % w) as i64 + x0, (u / w) as i64 + y0);
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let (vx, vy) = (ux + dx, uy + dy);
                    if vx < x0 || vx > x1 || vy < y0 || vy > y1 || !inband[at(vx, vy)] {
                        continue;
                    }
                    let v = at(vx, vy);
                    let len = if dx != 0 && dy != 0 { diagonal } else { straight };
                    let rise = libm::round((z[index[v]] - z[index[u]]).max(0.0) * 1000.0) as i64;
                    let next = c + rise + lambda_mm(len);
                    if next < cost[v] {
                        cost[v] = next;
                        from[v] = u;
                        heap.push(Reverse((next, v)));
                    }
                }
            }
        }
        if cost[goal] == i64::MAX {
            return guide.to_vec();
        }
        let mut cells = vec![goal];
        while *cells.last().expect("a cell") != start {
            let p = from[*cells.last().expect("a cell")];
            cells.push(p);
        }
        cells.reverse();
        let mut out: Vec<Vec3> = Vec::with_capacity(cells.len() + 1);
        out.push(first);
        if cells.len() > 2 {
            for &u in &cells[1..cells.len() - 1] {
                out.push(dirs[index[u]]);
            }
        }
        out.push(last);
        out
    }
}
