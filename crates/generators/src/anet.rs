//! A-net, the chunk stage of M8-3 (ADR 0077 points 1 and 3, SPIKE-33, SPIKE-35): stream-power incision along the river network the global
//! coupled run fixes, then hillslope processes, per chunk.
//!
//! This is the network in the chunk raster (GEN-047): from the stored coarse run, the corridor path of every river through a chunk's
//! region, the inflow where a river enters the region from outside, and the initial fine heights. Everything is a function of position
//! alone (the coarse segment, the distance along it, the direction), so two neighbouring chunks find the same path; `libm` only, fixed order
//! (CON-14).
//!
//! The coarse lattice is the chunk grid: the coupled run's 2048 cells per face edge and the level-11 tiles are the same squares, so a
//! fine cell `(face, i, j)` of a lattice of `cells_per_chunk` per chunk lies in the coarse cell `(face, i / cells_per_chunk, j / cells_per_chunk)`.

use crate::chunkfield::{Cell, ChunkLattice, Region};
use crate::coupled::{Lattice, NONE as COARSE_NONE};
use planet_core::cube::{direction_to_face, TangentWarp};
use planet_core::hash::mix64;
use planet_core::Vec3;
use std::collections::BTreeSet;

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
    /// What the coupled run's input held at the direction (the bake's elevation read there), metres.
    fn coarse_input(&self, dir: Vec3) -> f64;
    /// A hash of the base field and the input the run started from (their keys): part of the stage's definition hash.
    fn identity(&self) -> u64;
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
    /// The height a corridor cell is held at where the coarse run fixes it (NaN elsewhere): at every quarter of a coarse segment, the run's heights
    /// interpolated along the segment. Beds are tied to the run at these anchors, so a bed does not depend on the heights of a region's ring (the
    /// implicit step carries a downstream height far up a channel); the stage keeps the lower of this and the base height.
    pub anchor_z: Vec<f64>,
    /// The drainage area of a corridor cell in m2: the coarse areas of its segment interpolated along it (NaN off the corridor). A river's area is a
    /// function of position, not of how much of its catchment the region holds.
    pub river_area_m2: Vec<f64>,
}

/// "No next cell."
pub const NO_NEXT: u32 = u32::MAX;

impl Network {
    /// The network of `region` (a region of `lat`) from the run `run` of the coarse lattice, on a body of radius `radius_m`.
    pub fn build(run: &dyn CoarseRun, base: &dyn BaseDetail, lat: &ChunkLattice, region: &Region, radius_m: f64) -> Network {
        let per = lat.cells_per_chunk() as usize;
        let coarse = run.lattice();
        let n_coarse = coarse.n();
        assert_eq!(n_coarse * per, lat.n() as usize, "the fine lattice must be the coarse lattice cut into chunks of {per} cells");
        let coarse_of = |c: Cell| usize::from(c.face) * n_coarse * n_coarse + (c.j as usize / per) * n_coarse + (c.i as usize / per);

        // The coarse cells the region touches, and the segments of rivers through them (and into them from outside).
        let inside: BTreeSet<usize> = region.cells.iter().map(|&c| coarse_of(c)).collect();
        // One coarse ring more than the region touches: a meander of 700 m takes a segment of the next cell into the region's outer cells.
        let touch: BTreeSet<usize> =
            inside.iter().flat_map(|&c| core::iter::once(c).chain(coarse.neighbours(c).into_iter().map(|x| x as usize))).collect();
        let is_river = |c: usize| f64::from(run.area_m2(c)) > RIVER_AREA_M2 && run.receiver(c) != COARSE_NONE;
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

        let len = region.cells.len();
        let (mut corridor, mut next, mut inflow_m2) = (vec![false; len], vec![NO_NEXT; len], vec![0.0f64; len]);
        let mut anchor_z = vec![f64::NAN; len];
        let mut river_area_m2 = vec![f64::NAN; len];
        let half_cell_m = 0.5 * lat.cell_m(radius_m);
        let key = |x: usize| region.cells[x].key();
        for (from, to) in segments {
            let points = segment_points(coarse.direction(from), coarse.direction(to), from as u64, radius_m);
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
            // The anchors: the quarter points of the segment, held at the run's height there.
            for q in 0..=4usize {
                let t = q as f64 / 4.0;
                let p = points[(t * (points.len() - 1) as f64).round() as usize];
                if let Some(i) = region.index_of(lat.cell_at(p)) {
                    let h = f64::from(run.z(from)) * (1.0 - t) + f64::from(run.z(to)) * t;
                    if anchor_z[i].is_nan() || h < anchor_z[i] {
                        anchor_z[i] = h;
                    }
                }
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

        let z0 = region
            .cells
            .iter()
            .map(|&c| {
                let d = lat.direction(c);
                (coarse_z_at(run, d) + (base.base_height(d) - base.coarse_input(d))) as f32
            })
            .collect();
        Network { corridor, next, inflow_m2, z0, anchor_z, river_area_m2 }
    }
}

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
}

impl ANetParams {
    /// The values of SPIKE-33 and SPIKE-35 with a halo of `halo_m` metres at cells of `cell_m` metres.
    pub fn spike(halo_m: f64, cell_m: f64) -> ANetParams {
        ANetParams {
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
        }
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
    base: &dyn BaseDetail,
    lat: &ChunkLattice,
    region: &Region,
    radius_m: f64,
    cell_area: &dyn Fn(Cell) -> f64,
    p: &ANetParams,
) -> Solution {
    const NONE: u32 = u32::MAX;
    let len = region.cells.len();
    let key = |x: usize| region.cells[x].key();
    let dirs: Vec<Vec3> = region.cells.iter().map(|&c| lat.direction(c)).collect();
    let z_base: Vec<f64> = dirs.iter().map(|&d| base.base_height(d)).collect();
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
    for _ in 0..p.area_window {
        let before = area.clone();
        let mut changed = false;
        for c in 0..len {
            if donors[c].is_empty() {
                continue;
            }
            let mut a = base_area[c];
            for &d in &donors[c] {
                if rec[d as usize] == c as u32 {
                    a += before[d as usize];
                }
            }
            if a != area[c] {
                area[c] = a;
                changed = true;
            }
        }
        if !changed {
            break;
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
    for _ in 0..p.steps {
        for &c in order.iter().rev() {
            let r = rec[c];
            if r == NONE || fixed(c) {
                continue;
            }
            z[c] = (z[c] + coef[c] * z[r as usize]) / (1.0 + coef[c]);
        }
    }
    // No channel cell drains uphill (GEN-048). A corridor donor below a fixed receiver is lifted to it, a flat (the bed behind a dam fills up to it),
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
        } else {
            rec[c] = NONE;
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
}

impl ANet {
    /// The solve over a region (also what the tests look at).
    pub fn solve(&self, lat: &ChunkLattice, region: &Region) -> Solution {
        let net = Network::build(&*self.run, &*self.base, lat, region, self.radius_m);
        // The own area of a fine cell: the coarse cell's area over the cells it holds.
        let per = lat.cells_per_chunk() as usize;
        let n = self.run.lattice().n();
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
    fn compute(&self, lat: &ChunkLattice, region: &Region) -> Vec<f32> {
        let s = self.solve(lat, region);
        // The stage's own bound is a promise to the plan's bounds (GEN-039): a change beyond it is cut to it.
        let bound = self.bound_m();
        (0..region.cells.len()).map(|c| (s.z[c] - s.z_base[c]).clamp(-bound, bound) as f32).collect()
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
                self.run.identity(),
                self.base.identity(),
                self.radius_m.to_bits(),
                u64::from(ANET_VERSION),
            ],
        )
    }
}

/// Version of the stage's algorithm: part of its definition hash, so a change that changes results re-keys every chunk.
pub const ANET_VERSION: u32 = 1;
