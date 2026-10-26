//! A-net, the chunk stage of M8-3 (ADR 0077 points 1 and 3, SPIKE-33, SPIKE-35): stream-power incision along the river network the global
//! coupled run fixes, then hillslope processes, per chunk.
//!
//! This is the network in the chunk raster (GEN-047): from the stored coarse run, the corridor path of every river through a chunk's
//! region, the inflow where a river enters the region from outside, and the initial fine heights. Everything is a function of position
//! alone (the coarse segment, the distance along it, the direction), so two neighbouring chunks find the same path; `libm` only, fixed order
//! (CON-14).
//!
//! The coarse lattice is the chunk grid in production: the coupled run's 2048 cells per face edge and the level-11 tiles are the same squares (a coarser run cuts them into larger squares), so a
//! fine cell `(face, i, j)` of a lattice of `cells_per_chunk` per chunk lies in the coarse cell `(face, i / cells_per_chunk, j / cells_per_chunk)`.

use crate::chunkfield::{Cell, ChunkLattice, Region};
use crate::coupled::{Lattice, NONE as COARSE_NONE};
use planet_core::cube::{direction_to_face, TangentWarp};
use planet_core::hash::mix64;
use planet_core::Vec3;
use std::collections::{BTreeMap, BTreeSet};

/// The drainage area above which a coarse cell is a river of the network (ADR 0077): 100 km2.
pub const RIVER_AREA_M2: f64 = 1.0e8;
/// The sideways displacement of the coarse path in metres, its wavelength, and the spacing of the points of the polyline (SPIKE-35 item 1).
pub const MEANDER_M: f64 = 700.0;
pub const MEANDER_WAVELENGTH_M: f64 = 900.0;
pub const POINT_SPACING_M: f64 = 40.0;

/// The stored global coupled run as the chunk stage reads it: heights, drainage area and receiver per coarse cell.
pub trait CoarseRun: Send + Sync {
    fn lattice(&self) -> &Lattice;
    /// The height in metres of coarse cell `c` after the run.
    fn z(&self, c: usize) -> f32;
    /// The drainage area of `c` in m2.
    fn area_m2(&self, c: usize) -> f32;
    /// The receiver of `c` ([`crate::coupled::NONE`] for an outlet).
    fn receiver(&self, c: usize) -> u32;
    /// A hash of what the run is (its key): part of the stage's definition hash, so a new run re-keys every chunk.
    fn identity(&self) -> u64;
}

impl CoarseRun for crate::coupled::CoupledMaps {
    fn lattice(&self) -> &Lattice {
        crate::coupled::CoupledMaps::lattice(self)
    }
    fn z(&self, c: usize) -> f32 {
        crate::coupled::CoupledMaps::z(self, c)
    }
    fn area_m2(&self, c: usize) -> f32 {
        self.network_area_m2(c)
    }
    fn receiver(&self, c: usize) -> u32 {
        self.receivers()[c]
    }
    fn identity(&self) -> u64 {
        self.id()
    }
}

/// The arrays of a finished or stored run.
pub struct StoredRun {
    pub lat: Lattice,
    pub z: Vec<f32>,
    pub area_m2: Vec<f32>,
    pub receiver: Vec<u32>,
    /// The key of the stored run (its coupled-channel key), part of the stage's definition hash.
    pub identity: u64,
}

impl CoarseRun for StoredRun {
    fn lattice(&self) -> &Lattice {
        &self.lat
    }
    fn z(&self, c: usize) -> f32 {
        self.z[c]
    }
    fn area_m2(&self, c: usize) -> f32 {
        self.area_m2[c]
    }
    fn receiver(&self, c: usize) -> u32 {
        self.receiver[c]
    }
    fn identity(&self) -> u64 {
        self.identity
    }
}

/// What the initial fine heights need besides the run: the base field the chunk stage changes, and the field the run's input was sampled from.
/// The fine surface starts as the run's height upsampled plus the base's detail, `base_height - coarse_input`, the part of the base the coarse
/// lattice cannot hold.
pub trait BaseDetail: Send + Sync {
    /// The height of the base field at a unit direction, metres.
    fn base_height(&self, dir: Vec3) -> f64;
    /// [`BaseDetail::base_height`] at every direction of `dirs`, in order: a base that builds a field per call answers a region in one build.
    fn base_heights(&self, dirs: &[Vec3]) -> Vec<f64> {
        dirs.iter().map(|&d| self.base_height(d)).collect()
    }
    /// What the coupled run's input held at the direction (the bake's elevation read there), metres.
    fn coarse_input(&self, dir: Vec3) -> f64;
    /// A hash of the base field and the input the run started from (their keys): part of the stage's definition hash.
    fn identity(&self) -> u64;
    /// A number that differs whenever the base's heights may have changed (an edit under the chunk stage, GEN-082): the routing memo of one
    /// epoch is never read in another (default 0: a base that never changes).
    fn epoch(&self) -> u64 {
        0
    }
    /// Whether every answer so far was a real one (a base that lost its source answers zeros and says so here): a result made while it is not
    /// healthy is not to be stored.
    fn healthy(&self) -> bool {
        true
    }
}

/// A value in `[-1, 1]` that is a function of the segment and the knot alone.
fn knot(seg: u64, i: i64) -> f64 {
    let h = mix64(mix64(seg ^ 0x006d_6561_6e64_6572) ^ i as u64);
    2.0 * ((h >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)) - 1.0
}

/// The sideways displacement in metres of the point at the fraction `t` of a segment of `length_m` metres: smooth value noise along the
/// segment with a wavelength of [`MEANDER_WAVELENGTH_M`], times `sin(pi t)`, so it is zero at both ends (the coarse cell centres).
pub fn meander_m(seg: u64, t: f64, length_m: f64) -> f64 {
    let x = t * length_m / (0.5 * MEANDER_WAVELENGTH_M);
    let (i0, f) = (libm::floor(x), x - libm::floor(x));
    let s = f * f * (3.0 - 2.0 * f);
    let v = knot(seg, i0 as i64) * (1.0 - s) + knot(seg, i0 as i64 + 1) * s;
    MEANDER_M * v * libm::sin(core::f64::consts::PI * t)
}

/// The points of the coarse segment from direction `a` to direction `b`, every [`POINT_SPACING_M`] or less: on the chord between them, displaced
/// sideways by [`meander_m`]. The first point is `a` and the last is `b`. `seg` names the segment (the coarse cell it leaves).
pub fn segment_points(a: Vec3, b: Vec3, seg: u64, radius_m: f64) -> Vec<Vec3> {
    let length_m = (b - a).length() * radius_m;
    let n = ((length_m / POINT_SPACING_M).ceil() as usize).max(1);
    let mid = (a + b).normalized();
    let side = mid.cross(b - a).normalized();
    (0..=n)
        .map(|k| {
            let t = k as f64 / n as f64;
            let on = (a * (1.0 - t) + b * t).normalized();
            let shift = meander_m(seg, t, length_m) / radius_m;
            (on + side * shift).normalized()
        })
        .collect()
}

/// The run's height at a direction: bilinear over the four coarse cells around it in the face coordinates. A tap past the face edge is the
/// cell holding the direction beyond the edge (as the cell graph does), so the surface has no step at a face edge.
pub fn coarse_z_at(run: &dyn CoarseRun, dir: Vec3) -> f64 {
    let lat = run.lattice();
    let n = lat.n() as i64;
    let (face, s, t) = direction_to_face(&TangentWarp, dir);
    let (u, v) = ((s + 1.0) * 0.5 * n as f64 - 0.5, (t + 1.0) * 0.5 * n as f64 - 0.5);
    let (fu, fv) = (libm::floor(u), libm::floor(v));
    let (i0, j0, wx, wy) = (fu as i64, fv as i64, u - fu, v - fv);
    let at = |i: i64, j: i64| -> f64 {
        let c = if (0..n).contains(&i) && (0..n).contains(&j) {
            usize::from(face.0) * (n * n) as usize + (j * n + i) as usize
        } else {
            let beyond = planet_core::cube::face_to_direction(
                &TangentWarp,
                face,
                -1.0 + (2.0 * i as f64 + 1.0) / n as f64,
                -1.0 + (2.0 * j as f64 + 1.0) / n as f64,
            );
            lat.cell_at(beyond)
        };
        f64::from(run.z(c))
    };
    let top = at(i0, j0) * (1.0 - wx) + at(i0 + 1, j0) * wx;
    let bottom = at(i0, j0 + 1) * (1.0 - wx) + at(i0 + 1, j0 + 1) * wx;
    top * (1.0 - wy) + bottom * wy
}

/// The network of a chunk's region: the corridor, the inflow and the initial heights.
pub struct Network {
    /// Whether each cell of the region is on a river's corridor.
    pub corridor: Vec<bool>,
    /// The cell a corridor cell drains to along the corridor (the smallest cell, by face, `i`, `j`, among the segments through it), or `u32::MAX`.
    pub next: Vec<u32>,
    /// The drainage area in m2 that enters the region at each cell from a river upstream of the region (0 elsewhere).
    pub inflow_m2: Vec<f64>,
    /// The initial fine height of each cell in metres: the run's height upsampled plus the base's detail.
    pub z0: Vec<f32>,
    /// The height a corridor cell is held at (NaN off the corridor): the bed of its segment interpolated at the cell, from samples about 200 m apart of the
    /// base height minus the anchor law's depth for the river's area, as a running maximum from downstream with a least fall (`anchor_profiles`). Beds are
    /// tied to the ground, so a bed does not depend on the heights of a region's ring (the implicit step carries a downstream height far up a channel); the
    /// stage keeps the lower of this and the base height.
    pub anchor_z: Vec<f64>,
    /// The drainage area of a corridor cell in m2: the coarse areas of its segment interpolated along it (NaN off the corridor). A river's area is a
    /// function of position, not of how much of its catchment the region holds.
    pub river_area_m2: Vec<f64>,
    /// The base field's height at each cell in metres (read once for the network and the solve).
    pub base_z: Vec<f64>,
    /// The unit direction of each cell's centre (made once for the network and the solve).
    pub dirs: Vec<Vec3>,
}

/// "No next cell."
pub const NO_NEXT: u32 = u32::MAX;

impl Network {
    /// The network of `region` (a region of `lat`) from the run `run` of the coarse lattice, on a body of radius `radius_m`.
    pub fn build(run: &dyn CoarseRun, base: &dyn BaseDetail, lat: &ChunkLattice, region: &Region, radius_m: f64) -> Network {
        Network::build_with(run, base, lat, region, radius_m, &AnchorLaw::default())
    }

    /// [`Network::build`] with the anchor law of the stage.
    pub fn build_with(
        run: &dyn CoarseRun,
        base: &dyn BaseDetail,
        lat: &ChunkLattice,
        region: &Region,
        radius_m: f64,
        law: &AnchorLaw,
    ) -> Network {
        Network::build_with_memo(run, base, lat, region, radius_m, law, &crate::route::RouteMemo::default())
    }

    /// [`Network::build_with`] keeping the routed segments in `memo` for the next build (a stage's chunks share most of them).
    pub fn build_with_memo(
        run: &dyn CoarseRun,
        base: &dyn BaseDetail,
        lat: &ChunkLattice,
        region: &Region,
        radius_m: f64,
        law: &AnchorLaw,
        memo: &crate::route::RouteMemo,
    ) -> Network {
        Network::build_pruned(run, base, lat, region, radius_m, law, memo, true)
    }

    /// [`Network::build_with_memo`] routing every segment of the ring of coarse cells, also those that cannot reach the region (the tests compare it
    /// with the pruned build; the two are bit for bit the same).
    #[doc(hidden)]
    pub fn build_unpruned(
        run: &dyn CoarseRun,
        base: &dyn BaseDetail,
        lat: &ChunkLattice,
        region: &Region,
        radius_m: f64,
        law: &AnchorLaw,
        memo: &crate::route::RouteMemo,
    ) -> Network {
        Network::build_pruned(run, base, lat, region, radius_m, law, memo, false)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_pruned(
        run: &dyn CoarseRun,
        base: &dyn BaseDetail,
        lat: &ChunkLattice,
        region: &Region,
        radius_m: f64,
        law: &AnchorLaw,
        memo: &crate::route::RouteMemo,
        prune: bool,
    ) -> Network {
        let is_river = |c: usize| f64::from(run.area_m2(c)) > law.river_area_m2 && run.receiver(c) != COARSE_NONE;
        let mut segments = region_segments(run, lat, region, law);

        let len = region.cells.len();
        let (mut corridor, mut next, mut inflow_m2) = (vec![false; len], vec![NO_NEXT; len], vec![0.0f64; len]);
        let mut anchor_z = vec![f64::NAN; len];
        let mut river_area_m2 = vec![f64::NAN; len];
        let mut relief_of = vec![0.0f64; len];
        let half_cell_m = 0.5 * lat.cell_m(radius_m);
        let key = |x: usize| region.cells[x].key();
        // The segments a region reads are those of its coarse cells and one ring more; a routed path leaves its coarse chord by the slide, the meander and the band.
        debug_assert!(
            law.route_band_m + law.slide_steep_m.max(law.slide_m).max(law.head_m) + MEANDER_M < 3_400.0,
            "the routing reaches past the coarse ring a region reads"
        );
        // The base heights of the region's cells, read before the routing: the raster of a segment is every few cells of this lattice, so the points of it
        // that lie in the region are these heights (GEN-077).
        let dirs: Vec<Vec3> = region.cells.iter().map(|&c| lat.direction(c)).collect();
        let heights = base.base_heights(&dirs);
        if prune {
            segments = segments_reaching(run, &dirs, &segments, lat, radius_m, law);
        }
        let router = crate::route::Router::new(run, base, lat, radius_m, law, memo).with_known(region, &heights);
        let anchors = anchor_profiles(run, base, &segments, radius_m, law, is_river, &router);
        for (from, to) in segments {
            let points = router.polyline(from, to);
            // The cells along the polyline in runs: a run ends where the polyline leaves the region, and a cell is visited once per run (a
            // meander that turns back within a cell must not make a loop of links).
            let mut runs: Vec<Vec<usize>> = vec![Vec::new()];
            let (area_from, area_to) = (f64::from(run.area_m2(from)), f64::from(run.area_m2(to)));
            let mut visit = |p: Vec3, t: f64, runs: &mut Vec<Vec<usize>>| match region.index_of(lat.cell_at(p)) {
                Some(i) => {
                    // The river's area at a cell is the coarse areas interpolated along the segment: a function of position alone.
                    let a = area_from + (area_to - area_from) * t;
                    if river_area_m2[i].is_nan() || a > river_area_m2[i] {
                        river_area_m2[i] = a;
                    }
                    // The bed of a corridor cell: the anchors of the segment interpolated along it (GEN-059), the lower where two rivers meet.
                    let (profile, rf) = &anchors[&(from, to)];
                    relief_of[i] = relief_of[i].max(*rf);
                    let k = profile.partition_point(|&(pt, _)| pt <= t).clamp(1, profile.len() - 1);
                    let ((t0, h0), (t1, h1)) = (profile[k - 1], profile[k]);
                    let bed = if t1 > t0 { h0 + (h1 - h0) * ((t - t0) / (t1 - t0)).clamp(0.0, 1.0) } else { h0 };
                    if anchor_z[i].is_nan() || bed < anchor_z[i] {
                        anchor_z[i] = bed;
                    }
                    let run = runs.last_mut().expect("a run");
                    if run.last() != Some(&i) && !run.contains(&i) {
                        run.push(i);
                    }
                }
                None => {
                    if !runs.last().is_some_and(Vec::is_empty) {
                        runs.push(Vec::new());
                    }
                }
            };
            for (k, w) in points.windows(2).enumerate() {
                let steps = (((w[1] - w[0]).length() * radius_m / half_cell_m).ceil() as usize).max(1);
                for s in 0..steps {
                    let u = s as f64 / steps as f64;
                    visit((w[0] * (1.0 - u) + w[1] * u).normalized(), (k as f64 + u) / (points.len() - 1) as f64, &mut runs);
                }
            }
            if let Some(&last) = points.last() {
                visit(last, 1.0, &mut runs);
            }
            // A river whose polyline starts outside the region enters it at its first cell in the region, carrying the coarse area it started with.
            if points.first().is_some_and(|&p| region.index_of(lat.cell_at(p)).is_none()) {
                if let Some(&first) = runs.iter().find(|r| !r.is_empty()).and_then(|r| r.first()) {
                    inflow_m2[first] += f64::from(run.area_m2(from));
                }
            }
            for path in runs {
                for (k, &i) in path.iter().enumerate() {
                    corridor[i] = true;
                    // Only to a neighbouring cell (a gap in the samples, as across a face corner, ends the links).
                    if let Some(&j) = path.get(k + 1) {
                        let is_neighbour = lat.neighbours(region.cells[i]).iter().any(|n| *n == region.cells[j]);
                        if is_neighbour && (next[i] == NO_NEXT || key(j) < key(next[i] as usize)) {
                            next[i] = j as u32;
                        }
                    }
                }
            }
        }
        // Segments that cross at a confluence may disagree on the order of two cells: cut every cycle of links at the link out of its largest cell,
        // walking the cells in key order, so the links end at an outlet and the cut is the same wherever the cycle is seen whole.
        let mut order: Vec<usize> = (0..len).filter(|&i| next[i] != NO_NEXT).collect();
        order.sort_unstable_by_key(|&i| key(i));
        let mut stamp = vec![usize::MAX; len];
        for (round, &start) in order.iter().enumerate() {
            let (mut at, mut walk) = (start, Vec::new());
            while stamp[at] != round && next[at] != NO_NEXT && stamp[at] == usize::MAX {
                stamp[at] = round;
                walk.push(at);
                at = next[at] as usize;
            }
            if stamp[at] == round && walk.contains(&at) {
                let cycle = &walk[walk.iter().position(|&x| x == at).expect("on the walk")..];
                let cut = *cycle.iter().max_by_key(|&&x| key(x)).expect("a cycle has cells");
                next[cut] = NO_NEXT;
            }
        }

        paint_band(&corridor, &river_area_m2, &relief_of, &mut anchor_z, &heights, &dirs, region, radius_m, law);
        let z0 = dirs.iter().zip(&heights).map(|(&d, &h)| (coarse_z_at(run, d) + (h - base.coarse_input(d))) as f32).collect();
        Network { corridor, next, inflow_m2, z0, anchor_z, river_area_m2, base_z: heights, dirs }
    }
}

/// How far under the drawn ground a corridor bed is anchored (GEN-047): `depth_m (A / area_ref_m2)^exponent`, at most `cap_m`, for a river of drainage
/// area `A`. The bed follows the ground the chunk is drawn over, never the coupled run's absolute height (the run's heights lie far from the base's
/// where the relief is high). `window` is how many coarse segments upstream along the main stem the bed profile looks (GEN-059): a bed is never higher than
/// the raw beds upstream of it within the window (a running minimum from the upstream end), and at most `cut_cap_m` under the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnchorLaw {
    pub depth_m: f64,
    pub area_ref_m2: f64,
    pub exponent: f64,
    pub cap_m: f64,
    pub window: u16,
    /// The least fall of the bed downstream (GEN-059): `slope_ks A^-slope_theta` per metre for a river of area `A` m2 (the law of the channel slope),
    /// at most `slope_cap` (a floor against flat reaches that does not lift a bed over the ground it runs on).
    pub slope_ks: f64,
    pub slope_theta: f64,
    pub slope_cap: f64,
    /// The bankfull channel (GEN-060): discharge `Q = q_per_km2 A` (m3/s, `A` in km2), width `w_coef Q^w_exp` m (Moody and Troutman: 7.2 and 0.5, a
    /// factor 2 of scatter), depth `d_coef Q^d_exp` m (Miller et al.: 0.18 and 0.47 at large `Q`; the exponent runs from 0.3 to 0.47).
    pub q_per_km2: f64,
    pub w_coef: f64,
    pub w_exp: f64,
    pub d_coef: f64,
    pub d_exp: f64,
    /// The valley (GEN-060), by the relief factor `rf = clamp(relief / relief_full_m, 0, 1)` of the base round the river: the valley floor is
    /// `lerp(floor_low, floor_high, rf)` bankfull widths wide (5 on a lowland is provisional, 1 where confined), the valley depth is the depth law
    /// capped at `lerp(cap_low_m, cap_m, rf)`, the sides rise at `lerp(side_low, side_high, rf)` (rise over run) up to the ground, and no band is painted
    /// further than `band_cap_m` from the river.
    pub relief_full_m: f64,
    pub floor_low: f64,
    pub floor_high: f64,
    pub cap_low_m: f64,
    pub side_low: f64,
    pub side_high: f64,
    pub band_cap_m: f64,
    /// The most a bed lies under the ground (GEN-059): a rise the river's path forces it through is cut at most this deep (a dam beyond), under the stage's
    /// bound on the change.
    pub cut_cap_m: f64,
    /// The drainage area in m2 above which a coarse cell is a river of the network and is routed like a main stem (GEN-068): the tributaries of the coarse run
    /// join the stems above it. [`RIVER_AREA_M2`] (100 km2) is the main stems alone.
    pub river_area_m2: f64,
    /// Local main-stem routing (GEN-066, ADR 0081; `crate::route`): the band in metres round the coarse guide within which the least-cost path runs (0: the
    /// coarse path), the weight of the length in the cost (metres of rise per metre), the raster stride in cells, the radius in metres within which a node
    /// slides to the lowest ground (on flat ground and on steep ground, by the relief factor) and the radius within which a head moves to the highest.
    pub route_band_m: f64,
    pub route_lambda: f64,
    pub route_stride: u32,
    pub slide_m: f64,
    pub slide_steep_m: f64,
    pub head_m: f64,
}

impl Default for AnchorLaw {
    fn default() -> AnchorLaw {
        AnchorLaw {
            depth_m: 3.0,
            area_ref_m2: RIVER_AREA_M2,
            exponent: 0.5,
            cap_m: 100.0,
            window: 16,
            slope_ks: 40.0,
            slope_theta: 0.45,
            slope_cap: 1.0e-4,
            q_per_km2: 0.01,
            w_coef: 7.2,
            w_exp: 0.5,
            d_coef: 0.18,
            d_exp: 0.47,
            relief_full_m: 1000.0,
            floor_low: 5.0,
            floor_high: 1.0,
            cap_low_m: 30.0,
            side_low: 0.15,
            side_high: 0.9,
            band_cap_m: 1600.0,
            cut_cap_m: 200.0,
            river_area_m2: RIVER_AREA_M2,
            route_band_m: 0.0,
            route_lambda: 0.02,
            route_stride: 4,
            slide_m: 0.0,
            slide_steep_m: 0.0,
            head_m: 0.0,
        }
    }
}

impl AnchorLaw {
    /// The relief factor of a base whose heights over about 10 km span `relief_m`.
    pub fn relief_factor(&self, relief_m: f64) -> f64 {
        (relief_m / self.relief_full_m).clamp(0.0, 1.0)
    }

    /// The bankfull width and depth in metres of a river of drainage area `area_m2` (GEN-060).
    pub fn bankfull_m(&self, area_m2: f64) -> (f64, f64) {
        let q = self.q_per_km2 * area_m2.max(0.0) / 1.0e6;
        (self.w_coef * libm::pow(q, self.w_exp), self.d_coef * libm::pow(q, self.d_exp))
    }

    /// The depth of the valley floor under the ground for a river of area `area_m2` in a base of relief factor `rf`.
    pub fn valley_depth_m(&self, area_m2: f64, rf: f64) -> f64 {
        let cap = self.cap_low_m + (self.cap_m - self.cap_low_m) * rf;
        (self.depth_m * libm::pow((area_m2 / self.area_ref_m2).max(0.0), self.exponent)).min(cap)
    }

    /// The width of the valley floor in metres.
    pub fn floor_width_m(&self, area_m2: f64, rf: f64) -> f64 {
        (self.floor_low + (self.floor_high - self.floor_low) * rf) * self.bankfull_m(area_m2).0
    }

    /// The slope of the valley sides (rise over run).
    pub fn side_slope(&self, rf: f64) -> f64 {
        self.side_low + (self.side_high - self.side_low) * rf
    }

    /// The least slope of the bed of a river of drainage area `area_m2` (rise over run, positive downhill).
    pub fn min_slope(&self, area_m2: f64) -> f64 {
        (self.slope_ks * libm::pow(area_m2.max(1.0), -self.slope_theta)).min(self.slope_cap)
    }

    /// The depth in metres of a river of drainage area `area_m2` (never negative, never over `cap_m`).
    pub fn depth_m(&self, area_m2: f64) -> f64 {
        (self.depth_m * libm::pow((area_m2 / self.area_ref_m2).max(0.0), self.exponent)).min(self.cap_m)
    }
}

/// What a corridor cell asks of the cells round it: (bed, bankfull width, depth, floor half width, side slope, reach in metres).
type BandSpec = (f64, f64, f64, f64, f64, f64);

/// The bed profile of a segment, `(t, height)` along it, and the relief factor round its start.
type SegmentBed = (Vec<(f64, f64)>, f64);

/// A segment, the chain of main-stem segments upstream of it (itself last) and the confluence its river meets as a tributary within the window.
type SegmentChains = (Chain, Option<Confluence>);

/// A segment and the chain of segments from it on up the main stem (itself last).
type Chain = ((usize, usize), Vec<(usize, usize)>);

/// A confluence a river enters as a tributary: the chain of the main stem that arrives there (farthest first) and the distance in metres from the end of
/// the segment to the confluence along the river.
type Confluence = (Vec<(usize, usize)>, f64);

/// The channel and the valley round the corridor (GEN-060): every cell within the reach of a corridor cell is held at the profile of a corridor cell (the
/// lower where it lies under the ground): the channel (`bankfull_m`: a depth `d` under the valley floor at the axis, the parabola `d (1 - (2x / w)^2)`
/// across the width `w`), the valley floor at the bed out to half the floor width, and the valley sides rising at the side slope up to the ground. The corridor
/// cells themselves are lowered to the channel floor (`bed - d`), so the axis is the lowest point of the cross-section. The sources are taken in four tiers
/// by reach (80 m, 400 m, 1.2 km, more), each tier relaxed on its own, a cell owned by the nearest source of the tier that the relaxation reaches (the distance is Euclidean to the owner, GEN-083), and a cell takes the lowest profile of the tiers:
/// the valley of a great river is not walled in by the narrower valley of a tributary that is nearer. A band cell is fixed like a corridor cell (its height
/// is `anchor_z`). The reach is `band_cap_m` at most (0 paints nothing): a core cell reads sources within that distance only, so the halo must hold
/// `band_cap_m / cell`, the drainage-area window and the settle's reach (`ANetParams::spike` derives the cap from the halo).
#[allow(clippy::too_many_arguments)]
pub fn paint_band(
    corridor: &[bool],
    area: &[f64],
    relief: &[f64],
    anchor_z: &mut [f64],
    ground: &[f64],
    dirs: &[Vec3],
    region: &Region,
    radius_m: f64,
    law: &AnchorLaw,
) {
    let len = corridor.len();
    let key = |x: usize| region.cells[x].key();
    let sources: Vec<usize> = (0..len).filter(|&c| corridor[c] && !anchor_z[c].is_nan()).collect();
    if sources.is_empty() || law.band_cap_m <= 0.0 {
        // No band; the corridor cells keep the bed (the channel floor of a river is only drawn with a band round it).
        return;
    }
    let specs: Vec<BandSpec> = sources
        .iter()
        .map(|&c| {
            let rf = relief[c];
            let (w, d) = law.bankfull_m(area[c]);
            let half_floor = 0.5 * law.floor_width_m(area[c], rf).max(w);
            let side = law.side_slope(rf).max(1e-3);
            let extent = ((ground[c] - anchor_z[c]).max(0.0) + d) / side;
            let reach = (half_floor + extent).min(law.band_cap_m);
            // The sides reach the ground inside the cap: a floor wider than the cap allows is cut back (the valley floor of a great lowland river is
            // wider than the halo of a chunk reads, so it is provisional, GEN-060).
            let half_floor = (reach - extent).max(0.5 * w).min(half_floor).min(reach);
            (anchor_z[c], w, d, half_floor, side, reach)
        })
        .collect();
    let tier_of = |reach: f64| usize::from(reach > 80.0) + usize::from(reach > 400.0) + usize::from(reach > 1200.0);
    let mut painted = vec![f64::NAN; len];
    let mut best = vec![f64::INFINITY; len];
    let mut from = vec![u32::MAX; len];
    let mut stamp = vec![usize::MAX; len];
    for tier in 0..4 {
        best.fill(f64::INFINITY);
        from.fill(u32::MAX);
        stamp.fill(usize::MAX);
        let mut changed: Vec<usize> = Vec::new();
        for (k, &c) in sources.iter().enumerate() {
            if tier_of(specs[k].5) == tier {
                best[c] = 0.0;
                from[c] = k as u32;
                changed.push(c);
            }
        }
        let mut round = 0usize;
        while !changed.is_empty() {
            let mut next: Vec<usize> = Vec::new();
            for &c in &changed {
                let src = from[c] as usize;
                let reach = specs[src].5;
                for k in 0..8 {
                    let q = region.neighbours[c][k] as usize;
                    if q >= len {
                        continue;
                    }
                    // The distance from `q` to the owner's source itself (a vector distance transform): summing edge lengths over the 8-neighbour graph
                    // is off by up to 8 % and unevenly cell to cell, which ripples a wall that runs off the grid axes.
                    let d = (dirs[q] - dirs[sources[src]]).length() * radius_m;
                    if d > reach || corridor[q] {
                        continue;
                    }
                    let take = d < best[q] || (d == best[q] && from[q] != src as u32 && key(sources[src]) < key(sources[from[q] as usize]));
                    if take {
                        best[q] = d;
                        from[q] = src as u32;
                        if stamp[q] != round {
                            stamp[q] = round;
                            next.push(q);
                        }
                    }
                }
            }
            changed = next;
            round += 1;
            debug_assert!(round < 1_000_000, "the band relaxation did not settle");
        }
        for c in 0..len {
            if corridor[c] || from[c] == u32::MAX {
                continue;
            }
            let (bed, w, d, half_floor, side, _) = specs[from[c] as usize];
            let x = best[c];
            let z = if x <= 0.5 * w {
                let u = 2.0 * x / w.max(1e-9);
                bed - d * (1.0 - u * u)
            } else if x <= half_floor {
                bed
            } else {
                bed + side * (x - half_floor)
            };
            if z < ground[c] && (painted[c].is_nan() || z < painted[c]) {
                painted[c] = z;
            }
        }
    }
    for (k, &c) in sources.iter().enumerate() {
        anchor_z[c] = specs[k].0 - specs[k].2;
    }
    for c in 0..len {
        if !painted[c].is_nan() && (anchor_z[c].is_nan() || painted[c] < anchor_z[c]) {
            anchor_z[c] = painted[c];
        }
    }
}

/// How far a routed path can lie from the chord between the coarse centres of its segment, in metres (GEN-078): the band and the slide are discs in the
/// raster's grid units, and the chart stretches a grid step by up to 2/sqrt(3) at a cube corner (a little more on the extended chart past an edge), so
/// both count `CHART_STRETCH` times; the meander is in true metres; the raster's rounding adds a few steps.
pub fn routing_reach_m(law: &AnchorLaw, cell_m: f64) -> f64 {
    const CHART_STRETCH: f64 = 1.17;
    let raster_m = f64::from(law.route_stride.max(1)) * cell_m;
    let slide_m = law.slide_m.max(law.slide_steep_m).max(law.head_m);
    let band_m = if law.route_band_m > 0.0 { law.route_band_m } else { 0.0 };
    CHART_STRETCH * (band_m + slide_m + 5.0 * raster_m) + MEANDER_M
}

/// The coarse segments a region reads: the segments of rivers through the coarse cells the region touches and one ring more, and into them from outside
/// (a meander of 700 m takes a segment of the next cell into the region's outer cells). A function of the region, the run and the law only.
fn region_segments(run: &dyn CoarseRun, lat: &ChunkLattice, region: &Region, law: &AnchorLaw) -> BTreeSet<(usize, usize)> {
    let coarse = run.lattice();
    let n_coarse = coarse.n();
    // Fine cells per coarse cell along an edge: the chunk lattice cut by the coarse one (the production run is the chunk grid, 2048 cells).
    let ratio = lat.n() as usize / n_coarse.max(1);
    assert_eq!(n_coarse * ratio, lat.n() as usize, "the fine lattice must be the coarse lattice cut into squares of {ratio} cells");
    let coarse_of = |c: Cell| usize::from(c.face) * n_coarse * n_coarse + (c.j as usize / ratio) * n_coarse + (c.i as usize / ratio);
    let inside: BTreeSet<usize> = region.cells.iter().map(|&c| coarse_of(c)).collect();
    let touch: BTreeSet<usize> =
        inside.iter().flat_map(|&c| core::iter::once(c).chain(coarse.neighbours(c).into_iter().map(|x| x as usize))).collect();
    let is_river = |c: usize| f64::from(run.area_m2(c)) > law.river_area_m2 && run.receiver(c) != COARSE_NONE;
    let mut segments: BTreeSet<(usize, usize)> = BTreeSet::new();
    for &c in &touch {
        if is_river(c) {
            segments.insert((c, run.receiver(c) as usize));
        }
        for nb in coarse.neighbours(c) {
            let nb = nb as usize;
            if !touch.contains(&nb) && is_river(nb) && run.receiver(nb) as usize == c {
                segments.insert((nb, c));
            }
        }
    }
    segments
}

/// The donor of the largest area into the coarse cell `c` (the main stem), the lower cell index among equals.
fn main_donor_of(run: &dyn CoarseRun, is_river: &dyn Fn(usize) -> bool, c: usize) -> Option<usize> {
    let coarse = run.lattice();
    let mut best: Option<usize> = None;
    for nb in coarse.neighbours(c) {
        let nb = nb as usize;
        if nb < coarse.len() && is_river(nb) && run.receiver(nb) as usize == c {
            let better = match best {
                None => true,
                Some(b) => run.area_m2(nb) > run.area_m2(b) || (run.area_m2(nb) == run.area_m2(b) && nb < b),
            };
            if better {
                best = Some(nb);
            }
        }
    }
    best
}

/// A disc on the sphere: the centre direction and the angle in radians. An item whose reach touches it can change what a chunk reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReadDisc {
    pub centre: Vec3,
    pub angle: f64,
}

/// The read set of a chunk (GEN-081): discs outside which no item can change the chunk's bits. A function of the region, the global run and the law
/// only, built from the coarse guides and their bands, never from routed paths or items.
/// - The region: one disc round its centre holding every cell.
/// - The network: the segments the region reads and the pruning keeps (GEN-078), each with the chain upstream of it along the main stem up to
///   `law.window` back, the chain downstream of it up to `law.window` on (a bed is held under the stem it meets), and the chain upstream of the main
///   donor at every cell of the downstream chain (the confluence); every segment as discs along its chord (nine points) of the radius of
///   `routing_reach_m` and the half step between them, and the eight neighbours of every cell of those segments (the relief factor of a start cell).
pub fn chunk_read_set(run: &dyn CoarseRun, lat: &ChunkLattice, region: &Region, radius_m: f64, law: &AnchorLaw) -> Vec<ReadDisc> {
    let coarse = run.lattice();
    let dirs: Vec<Vec3> = region.cells.iter().map(|&c| lat.direction(c)).collect();
    let mut sum = Vec3::new(0.0, 0.0, 0.0);
    for &d in &dirs {
        sum = sum + d;
    }
    let centre = sum.normalized();
    let region_angle = dirs.iter().map(|&d| centre.angle_to(d)).fold(0.0, f64::max);
    let mut discs = vec![ReadDisc { centre, angle: region_angle + 1.0e-9 }];
    let is_river = |c: usize| f64::from(run.area_m2(c)) > law.river_area_m2 && run.receiver(c) != COARSE_NONE;
    let kept = segments_reaching(run, &dirs, &region_segments(run, lat, region, law), lat, radius_m, law);
    let mut all: BTreeSet<(usize, usize)> = kept.clone();
    let upstream_of = |all: &mut BTreeSet<(usize, usize)>, from: usize| {
        let mut at = from;
        // `window + 1` segments: the confluence chain of `anchor_profiles` is the segment into the cell and `window` more upstream of its donor.
        for _ in 0..=law.window {
            match main_donor_of(run, &is_river, at) {
                Some(d) => {
                    all.insert((d, at));
                    at = d;
                }
                None => break,
            }
        }
    };
    for &(from, to) in &kept {
        upstream_of(&mut all, from);
        let mut at = to;
        for _ in 0..law.window {
            if !is_river(at) {
                break;
            }
            let next = run.receiver(at) as usize;
            all.insert((at, next));
            upstream_of(&mut all, at);
            at = next;
        }
    }
    let reach_rad = routing_reach_m(law, lat.cell_m(radius_m)) / radius_m;
    for &(from, to) in &all {
        let (a, b) = (coarse.direction(from), coarse.direction(to));
        let half_step_rad = (a - b).length() / 16.0;
        for k in 0..=8 {
            let p = (a * (1.0 - f64::from(k) / 8.0) + b * (f64::from(k) / 8.0)).normalized();
            discs.push(ReadDisc { centre: p, angle: reach_rad + half_step_rad + 1.0e-9 });
        }
    }
    // The relief factor of a segment's start cell reads the base at the cell's centre and its eight neighbours (coarse spacing).
    let cells: BTreeSet<usize> = all.iter().flat_map(|&(from, to)| [from, to]).collect();
    let ring: BTreeSet<usize> = cells.iter().flat_map(|&c| coarse.neighbours(c).into_iter().map(|x| x as usize)).collect();
    for c in ring {
        discs.push(ReadDisc { centre: coarse.direction(c), angle: reach_rad + 1.0e-9 });
    }
    discs
}

/// The segments of `segments` whose routed path can touch the region whose cell directions are `dirs` (GEN-078). A path stays within the band of its guide,
/// the guide within the meander of the chord between its two nodes, and a node within the slide of its coarse centre, so a segment whose chord is farther
/// from the region than the region's radius plus those and the raster's rounding cannot give the region a single cell, a bed, an inflow or a link: leaving
/// it out changes nothing.
fn segments_reaching(
    run: &dyn CoarseRun,
    dirs: &[Vec3],
    segments: &BTreeSet<(usize, usize)>,
    lat: &ChunkLattice,
    radius_m: f64,
    law: &AnchorLaw,
) -> BTreeSet<(usize, usize)> {
    let mut sum = Vec3::new(0.0, 0.0, 0.0);
    for &d in dirs {
        sum = sum + d;
    }
    let centre = sum.normalized();
    let reach_m = dirs.iter().map(|&d| (d - centre).length()).fold(0.0, f64::max) * radius_m;
    let margin_m = reach_m + routing_reach_m(law, lat.cell_m(radius_m));
    let coarse = run.lattice();
    segments
        .iter()
        .copied()
        .filter(|&(from, to)| {
            let (a, b) = (coarse.direction(from), coarse.direction(to));
            let half_step_m = (a - b).length() * radius_m / 16.0;
            (0..=8).any(|k| {
                let p = (a * (1.0 - f64::from(k) / 8.0) + b * (f64::from(k) / 8.0)).normalized();
                (p - centre).length() * radius_m - half_step_m <= margin_m
            })
        })
        .collect()
}

/// The bed of every segment of `segments` (GEN-059), sampled every `PROFILE_SAMPLES` polyline points: `(t, height)` along the segment, the height of
/// the bed `t` of the way along it. The raw bed at a point is the base height there minus the law's valley depth for the river's area. The bed is the
/// running minimum from upstream of the raw beds along the river's main stem (the donor of the largest area at every cell) for up to `law.window` segments
/// back, each point at least the least fall under the one above it and at most `cut_cap_m` under the ground: the bed follows the ground and cuts through a
/// rise only where the river's path forces it, never lying over the ground it runs on. A tributary's bed, back to the window, is held no lower than the main
/// stem's bed that arrives at the confluence it flows into (rising at the least fall: a tributary hangs above the stem, never under it). A function of the run
/// and the base at positions alone: the same in every chunk. The window is the limit: a low carried from the segment `window` above one segment is not carried by
/// the next, which releases it as a step there (rare; a link that climbs by more than the bed tolerance makes its donor an outlet in the solve).
fn anchor_profiles(
    run: &dyn CoarseRun,
    base: &dyn BaseDetail,
    segments: &BTreeSet<(usize, usize)>,
    radius_m: f64,
    law: &AnchorLaw,
    is_river: impl Fn(usize) -> bool,
    router: &crate::route::Router,
) -> BTreeMap<(usize, usize), SegmentBed> {
    let coarse = run.lattice();
    let main_donor = |c: usize| main_donor_of(run, &is_river, c);
    // The main stem back from a segment: the segments upstream of it along the main donors, farthest first, ending with the segment itself.
    let upstream = |seg: (usize, usize)| -> Vec<(usize, usize)> {
        let mut chain = vec![seg];
        let mut at = seg.0;
        for _ in 0..law.window {
            match main_donor(at) {
                Some(d) => {
                    chain.push((d, at));
                    at = d;
                }
                None => break,
            }
        }
        chain.reverse();
        chain
    };
    // For every requested segment: its own chain, and the confluence where its river meets a main stem as a tributary within the window downstream.
    let mut chains: Vec<SegmentChains> = Vec::new();
    let mut needed: BTreeSet<(usize, usize)> = BTreeSet::new();
    // The length of a segment on the way to a confluence: the distance between the two coarse centres (the lift is a floor, a rough distance serves, and
    // the segments downstream are not routed for it).
    let length_m = |seg: (usize, usize)| -> f64 { (coarse.direction(seg.0) - coarse.direction(seg.1)).length() * radius_m };
    for &(from, to) in segments {
        let own = upstream((from, to));
        let (mut at, mut last, mut run_m, mut confluence) = (to, (from, to), 0.0, None);
        for _ in 0..law.window {
            if !is_river(at) {
                break;
            }
            match main_donor(at) {
                Some(d) if d != last.0 => {
                    confluence = Some((upstream((d, at)), run_m));
                    break;
                }
                _ => {}
            }
            let next = run.receiver(at) as usize;
            run_m += length_m((at, next));
            last = (at, next);
            at = next;
        }
        needed.extend(own.iter().copied());
        if let Some((st, _)) = &confluence {
            needed.extend(st.iter().copied());
        }
        chains.push((((from, to), own), confluence));
    }
    // The sample points of every needed segment, and one batch of base heights for all of them.
    struct Samples {
        ts: Vec<f64>,
        spots: Vec<Vec3>,
        areas: Vec<f64>,
    }
    let mut samples: BTreeMap<(usize, usize), Samples> = BTreeMap::new();
    let mut dirs: Vec<Vec3> = Vec::new();
    for &(from, to) in &needed {
        let line = router.polyline(from, to);
        let mut idx: Vec<usize> = (0..line.len()).step_by(PROFILE_SAMPLES).collect();
        if idx.last() != Some(&(line.len() - 1)) {
            idx.push(line.len() - 1);
        }
        let last = idx[idx.len() - 1].max(1) as f64;
        let (area_from, area_to) = (f64::from(run.area_m2(from)), f64::from(run.area_m2(to)));
        let ts: Vec<f64> = idx.iter().map(|&i| i as f64 / last).collect();
        let spots: Vec<Vec3> = idx.iter().map(|&i| line[i]).collect();
        let areas: Vec<f64> = ts.iter().map(|&t| area_from + (area_to - area_from) * t).collect();
        dirs.extend(spots.iter().copied());
        samples.insert((from, to), Samples { ts, spots, areas });
    }
    let heights = base.base_heights(&dirs);
    let mut ground: BTreeMap<(usize, usize), Vec<f64>> = BTreeMap::new();
    let mut at = 0;
    for (seg, smp) in &samples {
        ground.insert(*seg, heights[at..at + smp.spots.len()].to_vec());
        at += smp.spots.len();
    }
    // The relief factor round the start of every needed segment.
    let froms: BTreeSet<usize> = needed.iter().map(|&(from, _)| from).collect();
    let rf_of: BTreeMap<usize, f64> = froms.into_iter().map(|c| (c, router.relief_factor(c))).collect();
    // The bed along a chain of segments (farthest upstream first): the running minimum with the least fall, floored at the cut cap under the ground.
    let along = |chain: &[(usize, usize)]| -> Vec<Vec<f64>> {
        let mut out: Vec<Vec<f64>> = Vec::new();
        let mut v = f64::INFINITY;
        let mut prev: Option<Vec3> = None;
        for seg in chain {
            let smp = &samples[seg];
            let (g, rf) = (&ground[seg], rf_of[&seg.0]);
            let mut bed = Vec::with_capacity(smp.spots.len());
            for (k, &gk) in g.iter().enumerate() {
                debug_assert!(gk.is_finite(), "a base height that is not a number");
                let raw = gk - law.valley_depth_m(smp.areas[k], rf);
                let fall = prev.map_or(0.0, |p| law.min_slope(smp.areas[k]) * (smp.spots[k] - p).length() * radius_m);
                v = raw.min(v - fall).max(gk - law.cut_cap_m);
                bed.push(v);
                prev = Some(smp.spots[k]);
            }
            out.push(bed);
        }
        out
    };
    let mut out = BTreeMap::new();
    for (((from, to), own), confluence) in chains {
        let seg = (from, to);
        let mut bed = along(&own).pop().expect("the segment's own bed");
        let smp = &samples[&seg];
        if let Some((st, downstream_m)) = confluence {
            // The main stem's bed where it arrives at the confluence: the last sample of its chain. A tributary is held no lower than this, rising at
            // the least fall from the confluence back up the river, so the whole of its reach above the confluence meets the stem from above.
            let arriving = *along(&st).last().and_then(|b| b.last()).expect("a bed");
            let last = bed.len() - 1;
            let slope = law.min_slope(smp.areas[last]);
            for (k, b) in bed.iter_mut().enumerate() {
                let rest: f64 = (k..last).map(|m| (smp.spots[m + 1] - smp.spots[m]).length() * radius_m).sum::<f64>() + downstream_m;
                // Never over the ground: a hollow on the way holds the bed at the ground (a flat at worst).
                *b = b.max(arriving + slope * rest).min(ground[&seg][k]);
            }
        }
        out.insert(seg, (smp.ts.iter().copied().zip(bed.iter().copied()).collect(), rf_of[&from]));
    }
    out
}

/// A link between two fixed cells that climbs by no more than this is a flat, not a reason to make the donor an outlet (the beds of two linked cells are
/// interpolated along the polyline in an order that can differ from the link order by millimetres). A larger climb (the ground rises beyond the window of
/// the bed profile: a step of the profile at a segment boundary) cuts the river there, locally: lifting the donors instead would raise the river over
/// the whole region and leave a seam at the next chunk, which does not see the step.
const BED_TOLERANCE_M: f64 = 0.05;

/// Polyline points between two samples of a segment's bed (about 200 m at the point spacing).
const PROFILE_SAMPLES: usize = 1;

/// The parameters of the incision (SPIKE-33, SPIKE-35): stream power `K A^m S` with the implicit step, N steps along the frozen network.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ANetParams {
    /// Erodibility `K` and the drainage-area exponent `m` (the coarse run's).
    pub k: f64,
    pub m: f64,
    /// The step in years.
    pub dt_years: f64,
    /// The step cap `N`.
    pub steps: u32,
    /// The jitter of the receiver choice in `[0, 1)` and its seed (the coupled run's rule, GEN-050).
    pub jitter: f64,
    pub seed: u64,
    /// Drainage area above which a cell is a channel, m2.
    pub channel_area_m2: f64,
    /// The halo in rings (the stage states it, GEN-049): 3 km over the cell size.
    pub halo_rings: u16,
    /// The least a receiver lies below its donor per metre of distance after the breach.
    pub breach_slope: f64,
    /// How many links up the corridor the breach and how many links up a catchment the drainage area reach (bounded, so a chunk does not depend on
    /// where its region ends).
    pub breach_window: u16,
    pub area_window: u16,
    /// The most a cell off the corridor takes of the step (`f` is capped here): a tributary that took all of it would collapse to its outlet level
    /// along its whole length and depend on the height at the ring. The corridor, held at its anchors, is not capped.
    pub tributary_f_max: f64,
    /// Hillslope processes (GEN-051): diffusivity in m2 per year, the angle of repose as a slope, the share of the excess slumped per pass; `diffusion`
    /// zero and `slump_rate` zero switch them off.
    pub diffusion: f64,
    pub talus: f64,
    pub slump_rate: f64,
    /// How many rings the banks settle to the repose slope round the channel cells (0: off, GEN-058).
    pub bank_rounds: u16,
    /// How far under the drawn ground the corridor beds are anchored.
    pub anchor: AnchorLaw,
    /// The constants of the driver channels of a chunk (GEN-070, GEN-071); `drivers.seed` is the world's seed, which the lithology noise reads (a local
    /// edit of the base does not touch it).
    pub drivers: crate::drivers::DriverParams,
}

impl ANetParams {
    /// The values of SPIKE-33 and SPIKE-35 with a halo of `halo_m` metres at cells of `cell_m` metres.
    pub fn spike(halo_m: f64, cell_m: f64) -> ANetParams {
        let mut params = ANetParams {
            k: 2.0e-5,
            m: 0.45,
            dt_years: 1.0e4,
            steps: 30,
            jitter: 0.4,
            seed: 0,
            channel_area_m2: 5.0e4,
            halo_rings: (halo_m / cell_m).ceil().clamp(1.0, f64::from(u16::MAX)) as u16,
            breach_slope: 0.002,
            breach_window: ((halo_m / cell_m).ceil() - 2.0).clamp(1.0, 64.0) as u16,
            area_window: ((halo_m / cell_m).ceil() - 2.0).clamp(1.0, 128.0) as u16,
            tributary_f_max: 0.5,
            diffusion: 0.005,
            talus: 0.9,
            slump_rate: 0.09,
            bank_rounds: ((300.0 / (0.9 * cell_m)).ceil() + 6.0).clamp(1.0, 96.0) as u16,
            anchor: AnchorLaw::default(),
            drivers: crate::drivers::DriverParams::default(),
        };
        // The valley band reads sources within `band_cap_m` and the solve reads `area_window + 1` rings further: the band reaches at most what the halo
        // holds besides that (GEN-049, GEN-060).
        let spare_rings = f64::from(params.halo_rings) - f64::from(params.area_window) - 2.0;
        params.anchor.band_cap_m = params.anchor.band_cap_m.min(spare_rings.max(0.0) * cell_m);
        // Local main-stem routing (GEN-066, ADR 0081): nodes slide 0.5 km (1 km on steep ground), heads move up to 1 km, the path runs within 1 km of the guide.
        params.anchor.route_band_m = 1000.0;
        params.anchor.route_stride = 8;
        params.anchor.slide_m = 500.0;
        params.anchor.slide_steep_m = 1000.0;
        params.anchor.head_m = 1000.0;
        // The tributaries of the coarse run (GEN-068): every coarse link of two cells' area and over is routed like a stem.
        params.anchor.river_area_m2 = 5.0e7;
        params
    }
}

/// What a solve leaves, per cell of the region: the receiver (a region index or `u32::MAX`), the final height, the drainage area and the base
/// height the incision started from.
pub struct Solution {
    pub receiver: Vec<u32>,
    pub z: Vec<f64>,
    pub area_m2: Vec<f64>,
    pub z_base: Vec<f64>,
}

/// The receivers, the breach and the implicit incision on `region` (A-net, SPIKE-35 variant c4 with the corridor of GEN-047). The surface the
/// incision starts from is the base field's: the change is what the incision does to it.
pub fn solve(
    net: &Network,
    _base: &dyn BaseDetail,
    lat: &ChunkLattice,
    region: &Region,
    radius_m: f64,
    cell_area: &dyn Fn(Cell) -> f64,
    p: &ANetParams,
) -> Solution {
    const NONE: u32 = u32::MAX;
    let len = region.cells.len();
    let key = |x: usize| region.cells[x].key();
    let dirs: &[Vec3] = &net.dirs;
    let z_base: Vec<f64> = net.base_z.clone();
    let mut z = z_base.clone();
    for (h, &anchor) in z.iter_mut().zip(&net.anchor_z) {
        if !anchor.is_nan() {
            *h = h.min(anchor);
        }
    }
    let ring = |c: usize| region.depth[c] >= region.halo;
    // A cell the incision does not move: the outer ring (the base height) and the anchors of the corridor (the lower of the run's height and the base).
    let fixed = |c: usize| ring(c) || !net.anchor_z[c].is_nan();
    let dist = |c: usize, r: usize| (dirs[c] - dirs[r]).length() * radius_m;

    // Receivers on the initial surface: the corridor's links, else the steepest jittered neighbour down (a function of position only).
    let mut rec = vec![NONE; len];
    for c in 0..len {
        if ring(c) {
            continue;
        }
        if net.corridor[c] && net.next[c] != NO_NEXT {
            rec[c] = net.next[c];
            continue;
        }
        let mut best = (0.0f64, NONE);
        for (k, &nb) in region.neighbours[c].iter().enumerate() {
            let drop = z[c] - z[nb as usize];
            if drop > 0.0 {
                let s = drop / dist(c, nb as usize) * crate::coupled::jitter_of(p.seed, key(c), k, p.jitter);
                if s > best.0 {
                    best = (s, nb);
                }
            }
        }
        rec[c] = best.1;
    }

    // Donors before receivers: Kahn's queue, seeded in cell-key order, so the order does not depend on the region's numbering.
    let mut donors: Vec<Vec<u32>> = vec![Vec::new(); len];
    for c in 0..len {
        if rec[c] != NONE {
            donors[rec[c] as usize].push(c as u32);
        }
    }
    for d in &mut donors {
        d.sort_unstable_by_key(|&x| key(x as usize));
    }
    let mut indeg: Vec<u32> = donors.iter().map(|d| d.len() as u32).collect();
    let mut order: Vec<usize> = (0..len).filter(|&c| indeg[c] == 0).collect();
    order.sort_unstable_by_key(|&c| key(c));
    let mut at = 0;
    while at < order.len() {
        let c = order[at];
        at += 1;
        let r = rec[c];
        if r != NONE {
            indeg[r as usize] -= 1;
            if indeg[r as usize] == 0 {
                order.push(r as usize);
            }
        }
    }
    // A cell left over lies on a cycle of corridor links that survived: it is an outlet.
    if order.len() < len {
        let mut seen = vec![false; len];
        for &c in &order {
            seen[c] = true;
        }
        for c in 0..len {
            if !seen[c] {
                rec[c] = NONE;
            }
        }
        // Its donors were never ordered either; order again without the cut links.
        donors = vec![Vec::new(); len];
        for c in 0..len {
            if rec[c] != NONE {
                donors[rec[c] as usize].push(c as u32);
            }
        }
        for d in &mut donors {
            d.sort_unstable_by_key(|&x| key(x as usize));
        }
        indeg = donors.iter().map(|d| d.len() as u32).collect();
        order = (0..len).filter(|&c| indeg[c] == 0).collect();
        order.sort_unstable_by_key(|&c| key(c));
        at = 0;
        while at < order.len() {
            let c = order[at];
            at += 1;
            let r = rec[c];
            if r != NONE {
                indeg[r as usize] -= 1;
                if indeg[r as usize] == 0 {
                    order.push(r as usize);
                }
            }
        }
    }

    // The breach, local: along the corridor's links a cell is cut to at least `breach_slope` below the donors within `breach_window` links
    // upstream of it (a running minimum over a bounded reach, not over the whole chain, so it does not depend on where the region ends). A link to
    // a fixed cell is never cut: the donor settles up to it in the implicit step (a flat, not a pit); a link to a ring cell that the donor lies
    // below is the one cut, the donor becoming an outlet (no cascade up the channel).
    let corridor_link = |d: usize| net.corridor[d] && rec[d] != NONE && rec[d] == net.next[d];
    let linked: Vec<usize> = (0..len).filter(|&c| corridor_link(c)).collect();
    let mut is_link = vec![false; len];
    for &c in &linked {
        is_link[c] = true;
    }
    let mut cut = z.clone();
    for _ in 0..p.breach_window {
        let before = cut.clone();
        let mut changed = false;
        for &d in &linked {
            let x = rec[d] as usize;
            if fixed(x) {
                continue;
            }
            let candidate = before[d] - p.breach_slope * dist(d, x);
            if candidate < cut[x] {
                cut[x] = candidate;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    z = cut;
    for &d in &linked {
        let r = rec[d] as usize;
        if ring(r) && z[r] > z[d] - p.breach_slope * dist(d, r) {
            rec[d] = NONE;
        }
    }

    // Drainage area: a cell's own area, the inflow of the corridor and the area of its donors in key order, over the upstream cells within
    // `area_window` links (a bounded reach: a catchment longer than the window is cut at the same distance whatever the region, so two chunks
    // agree; the corridor's rivers carry the area from further up as inflow).
    let base_area: Vec<f64> = (0..len).map(|c| cell_area(region.cells[c]) + net.inflow_m2[c]).collect();
    let mut area = base_area.clone();
    // Round by round from the values of the round before; only a cell with a donor that changed in the round before can change, so each round
    // looks at the receivers of the cells that changed (the same values as a sweep over every cell, in far less time once the short catchments
    // have settled).
    let mut changed: Vec<usize> = (0..len).filter(|&c| rec[c] != NONE).collect();
    let mut queued = vec![usize::MAX; len];
    for round in 0..p.area_window as usize {
        let mut touched: Vec<usize> = Vec::new();
        for &d in &changed {
            let c = rec[d];
            if c != NONE && queued[c as usize] != round {
                queued[c as usize] = round;
                touched.push(c as usize);
            }
        }
        let mut next: Vec<(usize, f64)> = Vec::new();
        for &c in &touched {
            if donors[c].is_empty() {
                continue;
            }
            let mut a = base_area[c];
            for &d in &donors[c] {
                if rec[d as usize] == c as u32 {
                    a += area[d as usize];
                }
            }
            if a != area[c] {
                next.push((c, a));
            }
        }
        if next.is_empty() {
            break;
        }
        changed.clear();
        for (c, a) in next {
            area[c] = a;
            changed.push(c);
        }
    }
    // The corridor carries its coarse area, a function of position.
    for c in 0..len {
        if !net.river_area_m2[c].is_nan() {
            area[c] = net.river_area_m2[c].max(base_area[c]);
        }
    }

    // The implicit step along the frozen network, receivers first: z = (z + f z_r) / (1 + f), f = K A^m dt / distance.
    let coef: Vec<f64> = (0..len)
        .map(|c| match rec[c] {
            NONE => 0.0,
            _ if fixed(c) => 0.0,
            r => {
                let f = p.k * libm::pow(area[c], p.m) * p.dt_years / dist(c, r as usize);
                if net.river_area_m2[c].is_nan() {
                    f.min(p.tributary_f_max)
                } else {
                    f
                }
            }
        })
        .collect();
    // Hillslope processes (GEN-051) after each incision sweep, on the cells within `steps - t` rings of the chunk (the reach left to a core cell at step
    // `t`), so they do not depend on the halo; channels, the corridor and fixed cells are exempt.
    let hill = p.diffusion > 0.0 || p.slump_rate > 0.0;
    let reach0 = (p.steps as u16).min(region.halo.saturating_sub(2));
    let inner = if region.halo < 2 { 0 } else { region.depth.partition_point(|&d| d <= reach0) };
    let nd: Vec<[f64; 8]> =
        if hill { (0..inner).map(|c| core::array::from_fn(|k| dist(c, region.neighbours[c][k] as usize))).collect() } else { Vec::new() };
    let protected = |c: usize| fixed(c) || net.corridor[c] || area[c] > p.channel_area_m2;
    // The cells a bank slumps towards: the protected cells inside the region's core reach, not the outer ring (GEN-058).
    let sink = |c: usize| protected(c) && !ring(c);
    let cf = (p.diffusion * p.dt_years / (lat.cell_m(radius_m) * lat.cell_m(radius_m))).min(0.12);
    // The cells the sweep moves, receivers first, as arrays read in sequence (the cell, its receiver, the coefficient and its divisor).
    let (mut sw_cell, mut sw_rec, mut sw_f, mut sw_den) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for &c in order.iter().rev() {
        if rec[c] != NONE && !fixed(c) {
            sw_cell.push(c as u32);
            sw_rec.push(rec[c]);
            sw_f.push(coef[c]);
            sw_den.push(1.0 + coef[c]);
        }
    }
    for t in 0..p.steps {
        for i in 0..sw_cell.len() {
            let c = sw_cell[i] as usize;
            z[c] = (z[c] + sw_f[i] * z[sw_rec[i] as usize]) / sw_den[i];
        }
        if hill {
            let cells = region.depth.partition_point(|&d| d <= (p.steps - t) as u16).min(inner);
            if p.diffusion > 0.0 {
                z = diffuse(&z, &region.neighbours, cells, cf, &protected);
            }
            if p.slump_rate > 0.0 {
                let delta = slump_deltas(&z, &region.neighbours, &nd, cells, p.talus, p.slump_rate, &protected, &key);
                for c in 0..cells {
                    z[c] += delta[c];
                }
            }
        }
    }
    // No channel cell drains uphill (GEN-048). A free corridor donor below a fixed receiver is lifted to it, a flat (the bed behind a dam fills up to it),
    // receivers first; a free corridor receiver above its donor is cut down to the donor, donors first (it ends at the next anchor); a link that
    // joins two fixed cells the wrong way, or a donor off the corridor, becomes an outlet. Each rule reaches the cell and the corridor next to it.
    let channel = |c: usize, area: &[f64]| area[c] > p.channel_area_m2;
    for &c in order.iter().rev() {
        let r = rec[c];
        if r != NONE && !fixed(c) && channel(c, &area) && z[c] < z[r as usize] {
            if is_link[c] {
                z[c] = z[r as usize];
            } else {
                rec[c] = NONE;
            }
        }
    }
    for &c in &order {
        let r = rec[c];
        if r == NONE || !channel(c, &area) || z[r as usize] <= z[c] {
            continue;
        }
        if !fixed(r as usize) {
            z[r as usize] = z[c];
        } else if z[r as usize] > z[c] + BED_TOLERANCE_M {
            rec[c] = NONE;
        }
    }
    // The banks settle (GEN-058): a free cell stands at most the angle of repose over the protected cells near it, as the distance allows: the lower
    // cone of slope `talus` from every sink (a channel, the corridor, an anchor; they do not move), `bank_rounds` rings far (a wall of `bound_m` at the
    // repose slope is `bound_m / (talus cell)` cells wide; a base cliff steeper than that is cut at the edge of the reach). Only cells within `rounds`
    // rings of the core are lowered, by sinks at most one ring further out; the reach is read from the stated halo, not the region's, so a wider
    // region gives the same core. A cell is never lowered under a protected neighbour by more than the repose slope (no new wall below a higher bed).
    if p.bank_rounds > 0 {
        let rounds = usize::from(p.bank_rounds).min(usize::from(p.halo_rings.saturating_sub(2)) / 2);
        let cover = region.depth.partition_point(|&d| usize::from(d) <= rounds);
        let seeds = region.depth.partition_point(|&d| usize::from(d) <= rounds + 1);
        let mut env = vec![f64::INFINITY; len];
        let mut changed: Vec<usize> = Vec::new();
        for c in 0..seeds {
            if sink(c) {
                env[c] = z[c];
                changed.push(c);
            }
        }
        let mut stamp = vec![usize::MAX; len];
        for round in 0..rounds {
            let mut next: Vec<usize> = Vec::new();
            for &s in &changed {
                for k in 0..8 {
                    let q = region.neighbours[s][k] as usize;
                    if q >= cover || protected(q) {
                        continue;
                    }
                    let candidate = env[s] + p.talus * dist(s, q);
                    if candidate < env[q] {
                        env[q] = candidate;
                        if stamp[q] != round {
                            stamp[q] = round;
                            next.push(q);
                        }
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            changed = next;
        }
        for c in 0..cover {
            if !protected(c) && env[c] < z[c] {
                let mut lowered = env[c];
                for k in 0..8 {
                    let q = region.neighbours[c][k] as usize;
                    if protected(q) {
                        lowered = lowered.max(z[q] - p.talus * dist(c, q));
                    }
                }
                z[c] = lowered.min(z[c]);
            }
        }
    }
    Solution { receiver: rec, z, area_m2: area, z_base }
}

/// The A-net chunk stage: the network of a stored coupled run, the implicit incision along it with the breach rule, jittered receivers.
pub struct ANet {
    pub run: std::sync::Arc<dyn CoarseRun>,
    pub base: std::sync::Arc<dyn BaseDetail>,
    pub radius_m: f64,
    pub params: ANetParams,
    /// The routed segments of this stage's chunks (GEN-066): a memo of pure functions of the run, the base and the law, emptied when the law changes.
    pub memo: EpochMemo,
}

/// The routing memo of the stage by the epoch of its base (GEN-082): the memo holds heights of the base, so a memo made before an edit is never read
/// after it, and a job that began before the edit keeps filling the memo it took, which no later job reads.
#[derive(Default)]
pub struct EpochMemo {
    current: std::sync::Mutex<(u64, std::sync::Arc<crate::route::RouteMemo>)>,
}

impl EpochMemo {
    /// The memo of `epoch`: the current one, or a new one when the epoch moved.
    pub fn get(&self, epoch: u64) -> std::sync::Arc<crate::route::RouteMemo> {
        let mut cur = self.current.lock().expect("epoch memo");
        if cur.0 != epoch {
            *cur = (epoch, std::sync::Arc::default());
        }
        cur.1.clone()
    }
}

impl ANet {
    /// The change of every cell of `region` as the stage reports it: the solve's change cut to the stage's bound (GEN-039).
    fn clamped_change(&self, region: &Region, s: &Solution) -> Vec<f32> {
        let bound = crate::chunkfield::ChunkStage::bound_m(self);
        (0..region.cells.len()).map(|c| (s.z[c] - s.z_base[c]).clamp(-bound, bound) as f32).collect()
    }

    /// The driver channels of the chunk's own cells (GEN-070) from the final heights (the base plus the stage's clamped change, over the reference surface: sea level 0), the
    /// drainage area of the solve and the cells' own geometry: the spacing along each axis, the compass turned into the gradient's east and north parts.
    pub fn chunk_drivers(&self, lat: &ChunkLattice, region: &Region, s: &Solution, change: &[f32]) -> crate::drivers::ChunkDrivers {
        use crate::drivers::{chunk_drivers, ChunkRaster};
        assert!(region.halo >= 1, "the drivers read one ring round the chunk");
        let n = lat.cells_per_chunk() as usize;
        let first = region.cells[0];
        let (face, i0, j0) = (first.face, i64::from(first.i), i64::from(first.j));
        let n_face = lat.n();
        // The height at a cell of the core's ring (position relative to the core's origin): the final height of the region's cell there.
        // The cell at a position that may lie beyond the face edge: the cell holding the direction there, whose centre is not at that point.
        let real_cell = |i: i64, j: i64| -> Cell {
            if (0..n_face).contains(&i) && (0..n_face).contains(&j) {
                Cell { face, i: i as u32, j: j as u32 }
            } else {
                lat.cell_at(lat.direction_ext(face, i, j))
            }
        };
        let height_at = |i: i64, j: i64| -> f64 {
            let c = region.index_of(real_cell(i, j)).expect("the ring of the chunk lies in its region");
            s.z_base[c] + f64::from(change[c])
        };
        let mut heights = Vec::with_capacity((n + 2) * (n + 2));
        for rj in 0..n + 2 {
            for ri in 0..n + 2 {
                heights.push(height_at(i0 + ri as i64 - 1, j0 + rj as i64 - 1));
            }
        }
        let (mut area, mut dirs, mut spacing, mut compass) =
            (Vec::with_capacity(n * n), Vec::with_capacity(n * n), Vec::with_capacity(n * n), Vec::with_capacity(n * n));
        let pole = Vec3::new(0.0, 0.0, 1.0);
        for j in 0..n {
            for i in 0..n {
                let c = j * n + i;
                let (ci, cj) = (i0 + i as i64, j0 + j as i64);
                let d = lat.direction(real_cell(ci, cj));
                // The neighbours the heights belong to, at their own centres (across a face edge those are not on the regular grid).
                let at = |i: i64, j: i64| lat.direction(real_cell(i, j));
                let (im, ip, jm, jp) = (at(ci - 1, cj), at(ci + 1, cj), at(ci, cj - 1), at(ci, cj + 1));
                let (di, dj) = (ip - im, jp - jm);
                let (ei, ej) = (di.normalized(), dj.normalized());
                // The local north and east (COORD: +z the north pole, +y 90 degrees east of +x): north is the pole's part across `d`, east is north x up.
                let north = (pole - d * pole.dot(d)).normalized();
                let east = north.cross(d);
                // The gradient is `alpha ei + beta ej` with `(alpha, beta)` the dual of the slopes `(p, q)` along the axes: the Gram matrix of the axes inverted.
                let (g11, g12, g22) = (ei.dot(ei), ei.dot(ej), ej.dot(ej));
                let det = g11 * g22 - g12 * g12;
                let part = |v: Vec3| {
                    let (a, b) = (v.dot(ei), v.dot(ej));
                    [(a * g22 - b * g12) / det, (-a * g12 + b * g11) / det]
                };
                let (pe, pn) = (part(east), part(north));
                compass.push([pe[0], pe[1], pn[0], pn[1]]);
                spacing.push([
                    (d - im).length() * self.radius_m,
                    (ip - d).length() * self.radius_m,
                    (d - jm).length() * self.radius_m,
                    (jp - d).length() * self.radius_m,
                ]);
                dirs.push(d);
                area.push(s.area_m2[c]);
            }
        }
        let centre = lat.direction_ext(face, i0 + (n / 2) as i64, j0 + (n / 2) as i64);
        let params = self.params.drivers;
        let raster = ChunkRaster {
            n,
            heights: &heights,
            area_m2: &area,
            dirs: &dirs,
            spacing_m: &spacing,
            compass: &compass,
            lat_rad: libm::asin(centre.z.clamp(-1.0, 1.0)),
        };
        chunk_drivers(&raster, &params)
    }

    /// The solve over a region (also what the tests look at).
    pub fn solve(&self, lat: &ChunkLattice, region: &Region) -> Solution {
        let net = Network::build_with_memo(
            &*self.run,
            &*self.base,
            lat,
            region,
            self.radius_m,
            &self.params.anchor,
            &self.memo.get(self.base.epoch()),
        );
        // The own area of a fine cell: the coarse cell's area over the cells it holds.
        let n = self.run.lattice().n();
        let per = lat.n() as usize / n.max(1);
        let cells = (per * per) as f64;
        let area = |c: Cell| {
            let coarse = usize::from(c.face) * n * n + (c.j as usize / per) * n + (c.i as usize / per);
            self.run.lattice().area_m2(coarse) / cells
        };
        solve(&net, &*self.base, lat, region, self.radius_m, &area, &self.params)
    }
}

impl crate::chunkfield::ChunkStage for ANet {
    fn step_cap(&self) -> u32 {
        self.params.steps
    }
    /// A bound on the change in metres: the breach cuts at most 250 m (SPIKE-35: 180 m maximum at the spike's site).
    fn bound_m(&self) -> f64 {
        300.0
    }
    fn halo_rings(&self) -> u16 {
        self.params.halo_rings
    }
    fn healthy(&self) -> bool {
        self.base.healthy()
    }
    fn epoch(&self) -> u64 {
        self.base.epoch()
    }
    fn read_discs(&self, lat: &ChunkLattice, region: &Region) -> Vec<ReadDisc> {
        chunk_read_set(&*self.run, lat, region, self.radius_m, &self.params.anchor)
    }
    fn compute(&self, lat: &ChunkLattice, region: &Region) -> Vec<f32> {
        let s = self.solve(lat, region);
        self.clamped_change(region, &s)
    }
    fn compute_full(&self, lat: &ChunkLattice, region: &Region) -> crate::chunkfield::StageOutput {
        let s = self.solve(lat, region);
        // The stage's own bound is a promise to the plan's bounds (GEN-039): a change beyond it is cut to it.
        let change = self.clamped_change(region, &s);
        let drivers = Some(self.chunk_drivers(lat, region, &s, &change));
        crate::chunkfield::StageOutput { change, drivers }
    }
    fn definition_hash(&self) -> u64 {
        let p = &self.params;
        planet_core::hash::hash_u64s(
            0x0000_616e_6574,
            &[
                p.k.to_bits(),
                p.m.to_bits(),
                p.dt_years.to_bits(),
                u64::from(p.steps),
                p.jitter.to_bits(),
                p.seed,
                p.channel_area_m2.to_bits(),
                u64::from(p.halo_rings),
                p.breach_slope.to_bits(),
                u64::from(p.breach_window),
                u64::from(p.area_window),
                p.tributary_f_max.to_bits(),
                p.diffusion.to_bits(),
                p.talus.to_bits(),
                p.slump_rate.to_bits(),
                u64::from(p.bank_rounds),
                p.anchor.depth_m.to_bits(),
                p.anchor.area_ref_m2.to_bits(),
                p.anchor.exponent.to_bits(),
                p.anchor.cap_m.to_bits(),
                u64::from(p.anchor.window),
                p.anchor.slope_ks.to_bits(),
                p.anchor.slope_theta.to_bits(),
                p.anchor.slope_cap.to_bits(),
                p.anchor.q_per_km2.to_bits(),
                p.anchor.w_coef.to_bits(),
                p.anchor.w_exp.to_bits(),
                p.anchor.d_coef.to_bits(),
                p.anchor.d_exp.to_bits(),
                p.anchor.relief_full_m.to_bits(),
                p.anchor.floor_low.to_bits(),
                p.anchor.floor_high.to_bits(),
                p.anchor.cap_low_m.to_bits(),
                p.anchor.side_low.to_bits(),
                p.anchor.side_high.to_bits(),
                p.anchor.band_cap_m.to_bits(),
                p.anchor.cut_cap_m.to_bits(),
                p.anchor.river_area_m2.to_bits(),
                p.anchor.route_band_m.to_bits(),
                p.anchor.route_lambda.to_bits(),
                u64::from(p.anchor.route_stride),
                p.anchor.slide_m.to_bits(),
                p.anchor.slide_steep_m.to_bits(),
                p.anchor.head_m.to_bits(),
                planet_core::hash::hash_u64s(0x6472_6976, &p.drivers.hash_words()),
                self.run.identity(),
                self.base.identity(),
                self.radius_m.to_bits(),
                u64::from(ANET_VERSION),
            ],
        )
    }
}

/// Version of the stage's algorithm: part of its definition hash, so a change that changes results re-keys every chunk.
pub const ANET_VERSION: u32 = 9;

/// The change of height each cell takes in one pass of thermal slumping (GEN-051): a cell steeper than `talus` over a neighbour gives that
/// neighbour `rate x (slope - talus) x distance`. `dist[c][k]` is the distance to the `k`-th neighbour of `c` (the order of the region's neighbour
/// table); a pair is skipped when either cell is `protected`. What a cell gives is added to the neighbour it goes to, so the sum of the deltas of the
/// cells the pass covers is zero whatever the neighbour table looks like (near a cube corner it is not symmetric), as far as the neighbour is
/// covered too. Every sum is in a fixed order, a cell's own gifts in table order and what it takes in order of the giver's `key`, so the result does
/// not depend on how the region numbers its cells. `cells` limits the pass to the first `cells` entries of `z`.
#[allow(clippy::too_many_arguments)]
pub fn slump_deltas(
    z: &[f64],
    neighbours: &[[u32; 8]],
    dist: &[[f64; 8]],
    cells: usize,
    talus: f64,
    rate: f64,
    protected: &dyn Fn(usize) -> bool,
    key: &dyn Fn(usize) -> u64,
) -> Vec<f64> {
    let mut delta = vec![0.0f64; z.len()];
    // (receiver, giver's key, amount), sorted before they are added.
    let mut gifts: Vec<(usize, u64, f64)> = Vec::new();
    for c in 0..cells {
        if protected(c) {
            continue;
        }
        let mut given = 0.0;
        for k in 0..8 {
            let q = neighbours[c][k] as usize;
            if protected(q) {
                continue;
            }
            let slope = (z[c] - z[q]) / dist[c][k];
            if slope > talus {
                let m = rate * (slope - talus) * dist[c][k];
                given += m;
                gifts.push((q, key(c), m));
            }
        }
        delta[c] = -given;
    }
    gifts.sort_unstable_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)).then(a.2.total_cmp(&b.2)));
    for (q, _, m) in gifts {
        delta[q] += m;
    }
    delta
}

/// One pass of diffusion on the first `cells` cells: `z + cf (sum of the four edge neighbours - 4 z)`, from the heights before the pass.
pub fn diffuse(z: &[f64], neighbours: &[[u32; 8]], cells: usize, cf: f64, skip: &dyn Fn(usize) -> bool) -> Vec<f64> {
    let mut out = z.to_vec();
    for c in 0..cells {
        if skip(c) {
            continue;
        }
        let n = &neighbours[c];
        out[c] = z[c] + cf * (z[n[0] as usize] + z[n[1] as usize] + z[n[2] as usize] + z[n[3] as usize] - 4.0 * z[c]);
    }
    out
}
