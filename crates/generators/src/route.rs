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
use crate::chunkfield::{Cell, ChunkLattice, Region};
use crate::coupled::NONE as COARSE_NONE;
use planet_core::Face;
use planet_core::Vec3;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The points of a routed segment.
type Polyline = Arc<Vec<Vec3>>;

/// The most routed segments a [`RouteMemo`] keeps (about 125 points of 24 bytes each): over it the memo is cleared.
const MEMO_SEGMENTS: usize = 4096;

/// The most raster heights a [`RouteMemo`] keeps (a `u64` key and an `f64` value each, about 24 MB): over it the memo is cleared, like the lines.
pub const MEMO_HEIGHTS: usize = 1 << 20;

/// The key of the lattice point `(i, j)` of `face` in the memo of raster heights: the chart may reach past the face edge, so the indices carry an offset.
fn height_key(face: u8, i: i64, j: i64) -> u64 {
    const OFFSET: i64 = 1 << 28;
    u64::from(face) << 58 | ((i + OFFSET) as u64) << 29 | (j + OFFSET) as u64
}

/// A memo of the nodes, relief factors and routed polylines of one run, base and law (what a stage keeps between its chunks, so a segment the chunks share
/// is routed once). Every value is a pure function of its key under those three, so a hit is the value a miss would compute, bit for bit; a different law
/// empties it.
#[derive(Default)]
pub struct RouteMemo {
    law_key: Mutex<Option<u64>>,
    nodes: Mutex<BTreeMap<usize, Vec3>>,
    reliefs: Mutex<BTreeMap<usize, f64>>,
    lines: Mutex<BTreeMap<(usize, usize), Polyline>>,
    /// The routed polylines before smoothing (GEN-084), by segment.
    raw: Mutex<BTreeMap<(usize, usize), Polyline>>,
    /// The base heights at the raster points routed over so far, by lattice point (GEN-077): each value is the base field's height at the direction of
    /// the point, whichever chunk, thread or batch asked, so a hit is the value a miss would compute, bit for bit.
    heights: Mutex<HashMap<u64, f64>>,
    /// Raster points served by the region's own heights, by this memo, and evaluated (since the memo was made).
    served: [AtomicU64; 3],
    /// The most heights held (`None`: [`MEMO_HEIGHTS`]); a test makes a memo with a small cap.
    height_cap: Option<usize>,
}

impl RouteMemo {
    /// A memo that holds at most `cap` raster heights (the default is [`MEMO_HEIGHTS`]).
    pub fn with_height_cap(cap: usize) -> RouteMemo {
        RouteMemo { height_cap: Some(cap), ..RouteMemo::default() }
    }

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
                law.smooth_m.to_bits(),
                law.smooth_max_m.to_bits(),
            ],
        );
        let mut current = self.law_key.lock().expect("memo");
        if *current != Some(key) {
            *current = Some(key);
            self.nodes.lock().expect("memo").clear();
            self.reliefs.lock().expect("memo").clear();
            self.lines.lock().expect("memo").clear();
            self.raw.lock().expect("memo").clear();
        }
    }

    /// How many raster points were served by the region's heights, by the memo and by evaluating the base field, since the memo was made.
    pub fn raster_counts(&self) -> [u64; 3] {
        [0, 1, 2].map(|k| self.served[k].load(Ordering::Relaxed))
    }

    /// The segments the memo holds a routed polyline for.
    pub fn routed_segments(&self) -> usize {
        self.raw.lock().expect("memo").len()
    }

    /// The points the memo of raster heights holds.
    pub fn raster_heights_held(&self) -> usize {
        self.heights.lock().expect("memo").len()
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
    /// The base heights of the cells of a region the build has already read (the raster points that fall in it need no evaluation).
    known: Option<(&'a Region, &'a [f64])>,
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
        Router { run, base, lat, radius_m, law, memo, known: None }
    }

    /// This router reading the base heights `heights` of the cells of `region` instead of evaluating the base field at raster points inside it.
    pub fn with_known(mut self, region: &'a Region, heights: &'a [f64]) -> Router<'a> {
        debug_assert_eq!(region.cells.len(), heights.len());
        self.known = Some((region, heights));
        self
    }

    /// The base heights at the raster points `(face, i, j)` of the lattice, in order: from the region's heights, else the memo, else evaluated in one batch.
    fn raster_heights(&self, face: u8, points: &[(i64, i64)]) -> Vec<f64> {
        let n = self.lat.n();
        let mut z = vec![0.0f64; points.len()];
        let mut miss: Vec<usize> = Vec::new();
        let mut counts = [0u64; 3];
        {
            let held = self.memo.heights.lock().expect("memo");
            for (k, &(i, j)) in points.iter().enumerate() {
                let inside = self.known.and_then(|(region, heights)| {
                    if i < 0 || j < 0 || i >= n || j >= n {
                        return None;
                    }
                    region.index_of(Cell { face, i: i as u32, j: j as u32 }).map(|x| heights[x])
                });
                if let Some(h) = inside {
                    z[k] = h;
                    counts[0] += 1;
                } else if let Some(&h) = held.get(&height_key(face, i, j)) {
                    z[k] = h;
                    counts[1] += 1;
                } else {
                    miss.push(k);
                }
            }
        }
        if !miss.is_empty() {
            let dirs: Vec<Vec3> = miss.iter().map(|&k| self.lat.direction_ext(face, points[k].0, points[k].1)).collect();
            let h = crate::anet::timed_base_heights(self.base, &dirs);
            let mut held = self.memo.heights.lock().expect("memo");
            let cap = self.memo.height_cap.unwrap_or(MEMO_HEIGHTS);
            if held.len() + miss.len() > cap {
                held.clear();
            }
            for (&k, &v) in miss.iter().zip(&h) {
                z[k] = v;
            }
            // A batch over the cap is not kept (the memo never holds more than the cap).
            if miss.len() <= cap {
                for (&k, &v) in miss.iter().zip(&h) {
                    held.insert(height_key(face, points[k].0, points[k].1), v);
                }
            }
            counts[2] += miss.len() as u64;
        }
        for (c, n) in self.memo.served.iter().zip(counts) {
            c.fetch_add(n, Ordering::Relaxed);
        }
        z
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
        let h = crate::anet::timed_base_heights(self.base, &ring);
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
        let mut points: Vec<(i64, i64)> = Vec::new();
        for a in -r..=r {
            for b in -r..=r {
                if ((a * a + b * b) as f64) * step_m * step_m <= radius_m * radius_m {
                    let (i, j) = (i64::from(cell.i) + a * s, i64::from(cell.j) + b * s);
                    dirs.push(self.lat.direction_ext(cell.face, i, j));
                    points.push((i, j));
                }
            }
        }
        let h = self.raster_heights(cell.face, &points);
        let mut best = 0;
        for k in 1..dirs.len() {
            if (highest && h[k] > h[best]) || (!highest && h[k] < h[best]) {
                best = k;
            }
        }
        dirs[best]
    }

    /// The coarse cells that drain into `c` through a river segment.
    fn donors(&self, c: usize) -> Vec<usize> {
        let coarse = self.run.lattice();
        coarse
            .neighbours(c)
            .into_iter()
            .map(|x| x as usize)
            .filter(|&x| x < coarse.len() && self.is_river(x) && self.run.receiver(x) as usize == c)
            .collect()
    }

    /// The polyline of the coarse segment `from` to `to`: the first point is near the node of `from`, the last near the node of `to` (GEN-084: the
    /// routed path averaged with the paths it continues from and into; a joint where the river continues through a node is smoothed from both sides,
    /// a source, a confluence and an outlet keep their node). A function of the segment alone, so two chunks that see it find the same polyline.
    pub fn polyline(&self, from: usize, to: usize) -> Polyline {
        if self.law.smooth_m <= 0.0 || !self.enabled() {
            return self.raw_polyline(from, to);
        }
        if let Some(l) = self.memo.lines.lock().expect("memo").get(&(from, to)) {
            return Arc::clone(l);
        }
        let own = self.raw_polyline(from, to);
        let donors = self.donors(from);
        let prev = (donors.len() == 1).then(|| self.raw_polyline(donors[0], from));
        let next = (self.is_river(to) && self.donors(to).len() == 1).then(|| self.raw_polyline(to, self.run.receiver(to) as usize));
        let line = Arc::new(smooth_chain(
            prev.as_deref().map(Vec::as_slice),
            &own,
            next.as_deref().map(Vec::as_slice),
            self.radius_m,
            self.law.smooth_m,
            self.law.smooth_max_m,
        ));
        let mut lines = self.memo.lines.lock().expect("memo");
        if lines.len() >= MEMO_SEGMENTS {
            lines.clear();
        }
        lines.insert((from, to), Arc::clone(&line));
        line
    }

    /// The routed polyline of the coarse segment `from` to `to` before smoothing: the first point is the node of `from`, the last the node of `to`.
    pub fn raw_polyline(&self, from: usize, to: usize) -> Polyline {
        if let Some(l) = self.memo.raw.lock().expect("memo").get(&(from, to)) {
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
        let mut raw = self.memo.raw.lock().expect("memo");
        if raw.len() >= MEMO_SEGMENTS {
            raw.clear();
        }
        raw.insert((from, to), Arc::clone(&line));
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
        let mut points: Vec<(i64, i64)> = Vec::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                if inband[at(x, y)] {
                    index[at(x, y)] = dirs.len();
                    dirs.push(self.lat.direction_ext(cell0.face, i0 + x * s, j0 + y * s));
                    points.push((i0 + x * s, j0 + y * s));
                }
            }
        }
        let z = self.raster_heights(cell0.face, &points);
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

/// Samples of each side of the tent filter of [`smooth_chain`].
const SMOOTH_TAPS: i64 = 8;

/// The points of `own` moved to the average of the chain `prev` ++ `own` ++ `next` over a tent of half width `smooth_m` metres along it (GEN-084). `prev`
/// is the polyline of the segment `own` continues from and `next` of the one it continues into (the last point of `prev` and the first of `next` are the
/// ends of `own`); `None` is a river that begins, ends or meets another here, whose end stays where it is: the move fades to zero over `smooth_m` of
/// path towards it. A point moves at most `max_m`. The result of a point depends on the chain within `smooth_m` of it only, so the two segments of a joint
/// that continues through find the same point. A chain end is extended by its end point.
pub fn smooth_chain(prev: Option<&[Vec3]>, own: &[Vec3], next: Option<&[Vec3]>, radius_m: f64, smooth_m: f64, max_m: f64) -> Vec<Vec3> {
    if own.len() < 2 || smooth_m <= 0.0 {
        return own.to_vec();
    }
    let mut chain: Vec<Vec3> = Vec::with_capacity(own.len() + prev.map_or(0, <[Vec3]>::len) + next.map_or(0, <[Vec3]>::len));
    if let Some(p) = prev {
        chain.extend_from_slice(&p[..p.len().saturating_sub(1)]);
    }
    let first = chain.len();
    chain.extend_from_slice(own);
    let last = chain.len() - 1;
    if let Some(n) = next {
        chain.extend_from_slice(n.get(1..).unwrap_or(&[]));
    }
    let mut s = vec![0.0f64; chain.len()];
    for k in 1..chain.len() {
        s[k] = s[k - 1] + (chain[k] - chain[k - 1]).length() * radius_m;
    }
    let at = |x: f64| -> Vec3 {
        if x <= s[0] {
            return chain[0];
        }
        if x >= s[s.len() - 1] {
            return chain[chain.len() - 1];
        }
        let k = s.partition_point(|&v| v <= x).clamp(1, s.len() - 1);
        let span = s[k] - s[k - 1];
        let t = if span > 0.0 { (x - s[k - 1]) / span } else { 0.0 };
        chain[k - 1] * (1.0 - t) + chain[k] * t
    };
    // A segment shorter than the filter would read past its neighbour: not a river of this lattice (a coarse cell is kilometres).
    debug_assert!(
        (prev.is_none() && next.is_none()) || s[last] - s[first] >= smooth_m,
        "a segment of {} m is shorter than the smoothing's {smooth_m} m",
        s[last] - s[first]
    );
    let h = smooth_m / SMOOTH_TAPS as f64;
    let mut out = Vec::with_capacity(own.len());
    for k in first..=last {
        let mut sum = Vec3::new(0.0, 0.0, 0.0);
        for m in -SMOOTH_TAPS..=SMOOTH_TAPS {
            sum = sum + at(s[k] + m as f64 * h) * ((SMOOTH_TAPS + 1 - m.abs()) as f64);
        }
        let target = sum.normalized();
        let raw = chain[k];
        let mut fade = 1.0f64;
        if prev.is_none() {
            fade = fade.min(((s[k] - s[first]) / smooth_m).clamp(0.0, 1.0));
        }
        if next.is_none() {
            fade = fade.min(((s[last] - s[k]) / smooth_m).clamp(0.0, 1.0));
        }
        let mut v = (target - raw) * fade;
        let moved_m = v.length() * radius_m;
        if moved_m > max_m {
            v = v * (max_m / moved_m);
        }
        out.push((raw + v).normalized());
    }
    // The joint with a neighbour: from the points around it, measured from it, whichever segment asks, so both segments put it at the same bits.
    if next.is_some() {
        if let (Some(n), Some(l)) = (next, out.last_mut()) {
            *l = joint_point(own.iter().rev().copied(), n.iter().copied(), radius_m, smooth_m, max_m);
        }
    }
    if let (Some(p), Some(f)) = (prev, out.first_mut()) {
        *f = joint_point(p.iter().rev().copied(), own.iter().copied(), radius_m, smooth_m, max_m);
    }
    out
}

/// The smoothed point at a joint: `back` runs from the joint backwards along one segment, `ahead` from the joint forwards along the next (both start with
/// the joint). The tent average of the two arms, arclength measured from the joint outward, moved at most `max_m` from the joint.
fn joint_point(back: impl Iterator<Item = Vec3>, ahead: impl Iterator<Item = Vec3>, radius_m: f64, smooth_m: f64, max_m: f64) -> Vec3 {
    // One arm as points with their distance from the joint, as far as the filter reaches.
    let arm = |points: &mut dyn Iterator<Item = Vec3>| -> (Vec<Vec3>, Vec<f64>) {
        let (mut pts, mut dist): (Vec<Vec3>, Vec<f64>) = (Vec::new(), Vec::new());
        for p in points {
            let d = pts.last().map_or(0.0, |&q: &Vec3| dist[dist.len() - 1] + (p - q).length() * radius_m);
            pts.push(p);
            dist.push(d);
            if d >= smooth_m {
                break;
            }
        }
        (pts, dist)
    };
    let (mut back, mut ahead) = (back, ahead);
    let (bp, bd) = arm(&mut back);
    let (ap, ad) = arm(&mut ahead);
    let joint = ap[0];
    let along = |pts: &[Vec3], dist: &[f64], x: f64| -> Vec3 {
        if x >= dist[dist.len() - 1] {
            return pts[pts.len() - 1];
        }
        let k = dist.partition_point(|&v| v <= x).clamp(1, dist.len() - 1);
        let span = dist[k] - dist[k - 1];
        let t = if span > 0.0 { (x - dist[k - 1]) / span } else { 0.0 };
        pts[k - 1] * (1.0 - t) + pts[k] * t
    };
    let h = smooth_m / SMOOTH_TAPS as f64;
    let mut sum = Vec3::new(0.0, 0.0, 0.0);
    for m in -SMOOTH_TAPS..=SMOOTH_TAPS {
        let x = m.abs() as f64 * h;
        let q = if m < 0 { along(&bp, &bd, x) } else { along(&ap, &ad, x) };
        sum = sum + q * ((SMOOTH_TAPS + 1 - m.abs()) as f64);
    }
    let v = sum.normalized() - joint;
    let moved_m = v.length() * radius_m;
    let v = if moved_m > max_m { v * (max_m / moved_m) } else { v };
    (joint + v).normalized()
}
