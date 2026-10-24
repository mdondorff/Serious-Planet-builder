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
}

/// The arrays of a finished or stored run.
pub struct StoredRun {
    pub lat: Lattice,
    pub z: Vec<f32>,
    pub area_m2: Vec<f32>,
    pub receiver: Vec<u32>,
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
}

/// What the initial fine heights need besides the run: the base field the chunk stage changes, and the field the run's input was sampled from.
/// The fine surface starts as the run's height upsampled plus the base's detail, `base_height - coarse_input`, the part of the base the coarse
/// lattice cannot hold.
pub trait BaseDetail: Send + Sync {
    /// The height of the base field at a unit direction, metres.
    fn base_height(&self, dir: Vec3) -> f64;
    /// What the coupled run's input held at the direction (the bake's elevation read there), metres.
    fn coarse_input(&self, dir: Vec3) -> f64;
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
        let half_cell_m = 0.5 * lat.cell_m(radius_m);
        let key = |x: usize| region.cells[x].key();
        for (from, to) in segments {
            let points = segment_points(coarse.direction(from), coarse.direction(to), from as u64, radius_m);
            // The cells along the polyline in runs: a run ends where the polyline leaves the region, and a cell is visited once per run (a
            // meander that turns back within a cell must not make a loop of links).
            let mut runs: Vec<Vec<usize>> = vec![Vec::new()];
            let visit = |p: Vec3, runs: &mut Vec<Vec<usize>>| match region.index_of(lat.cell_at(p)) {
                Some(i) => {
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
            for w in points.windows(2) {
                let steps = (((w[1] - w[0]).length() * radius_m / half_cell_m).ceil() as usize).max(1);
                for s in 0..steps {
                    visit((w[0] * (1.0 - s as f64 / steps as f64) + w[1] * (s as f64 / steps as f64)).normalized(), &mut runs);
                }
            }
            if let Some(&last) = points.last() {
                visit(last, &mut runs);
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
        Network { corridor, next, inflow_m2, z0 }
    }
}
