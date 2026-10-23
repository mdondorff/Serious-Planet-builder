//! The global coupled run (M8-1, ADR 0077 point 1, GEN-040): stream-power landscape evolution on the cube-sphere bake lattice. Per step the
//! depressions are filled (priority flood with epsilon, Barnes 2014), every cell takes the steepest receiver (a jittered D8 that removes most of
//! the grid-axis alignment, SPIKE-35), the drainage area is accumulated downstream and the heights take one implicit stream-power update
//! (Braun and Willett 2013; stable at a large step). The result fixes the river network the chunk stages incise along.
//!
//! Deterministic by construction (CON-14): one thread, fixed visiting orders (the flood's pop order, ties broken by cell index), `libm` for
//! the transcendentals, and a jitter that is a hash of the cell and the direction. Ported from SPIKE-33's prototype
//! (`spikes/landscape-evolution/results/*.patch`); the base heights, the uplift and the erodibility are inputs, so the run does not know where
//! they come from (the wiring into the world's sources waits for the field-chain refactor, R1).

use planet_core::cube::{direction_to_face, face_to_direction, Face, TangentWarp};
use planet_core::hash::mix64;
use planet_core::Vec3;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// "No cell": a receiver of an outlet, a neighbour off the lattice.
pub const NONE: u32 = u32::MAX;

/// The eight neighbours of a cell: the four edge neighbours first, then the four diagonals.
const OFFS: [(i64, i64); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

/// The cube sphere of `n` cells per face edge (cell-centred, tangent warp), neighbours across the face edges through the direction.
pub struct Lattice {
    n: usize,
    /// Area of a cell of one face in m2 (all faces are equal).
    area_m2: Vec<f64>,
}

fn solid(u: f64, v: f64) -> f64 {
    libm::atan2(u * v, libm::sqrt(1.0 + u * u + v * v))
}

impl Lattice {
    pub fn new(n: usize, radius_m: f64) -> Lattice {
        assert!(n > 0 && 6 * n * n < NONE as usize, "{n} cells per face edge: the cell indices must fit a u32 below NONE");
        let u: Vec<f64> = (0..=n).map(|k| libm::tan((-1.0 + 2.0 * k as f64 / n as f64) * core::f64::consts::FRAC_PI_4)).collect();
        let mut area_m2 = vec![0.0; n * n];
        for j in 0..n {
            for i in 0..n {
                let omega = solid(u[i + 1], u[j + 1]) - solid(u[i], u[j + 1]) - solid(u[i + 1], u[j]) + solid(u[i], u[j]);
                area_m2[j * n + i] = omega * radius_m * radius_m;
            }
        }
        Lattice { n, area_m2 }
    }

    /// Cells per face edge.
    pub fn n(&self) -> usize {
        self.n
    }

    /// The number of cells.
    pub fn len(&self) -> usize {
        6 * self.n * self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    fn split(&self, c: usize) -> (usize, usize, usize) {
        let n2 = self.n * self.n;
        let r = c % n2;
        (c / n2, r % self.n, r / self.n)
    }

    /// The unit direction of the centre of cell `c`.
    pub fn direction(&self, c: usize) -> Vec3 {
        let (f, i, j) = self.split(c);
        let n = self.n as f64;
        face_to_direction(&TangentWarp, Face(f as u8), -1.0 + (2.0 * i as f64 + 1.0) / n, -1.0 + (2.0 * j as f64 + 1.0) / n)
    }

    fn cell_of(&self, d: Vec3) -> usize {
        let (f, s, t) = direction_to_face(&TangentWarp, d);
        let n = self.n as i64;
        let i = (((s + 1.0) * 0.5 * n as f64) as i64).clamp(0, n - 1);
        let j = (((t + 1.0) * 0.5 * n as f64) as i64).clamp(0, n - 1);
        f.0 as usize * self.n * self.n + j as usize * self.n + i as usize
    }

    /// The area of cell `c` in m2.
    pub fn area_m2(&self, c: usize) -> f64 {
        self.area_m2[c % (self.n * self.n)]
    }

    /// The eight neighbours of `c` in the order of [`OFFS`]; across a face edge the neighbour is the cell holding the direction beyond the edge.
    pub fn neighbours(&self, c: usize) -> [u32; 8] {
        let (f, i, j) = self.split(c);
        let n = self.n as i64;
        let mut out = [NONE; 8];
        for (k, &(di, dj)) in OFFS.iter().enumerate() {
            let (ni, nj) = (i as i64 + di, j as i64 + dj);
            out[k] = if ni >= 0 && ni < n && nj >= 0 && nj < n {
                (f * self.n * self.n + nj as usize * self.n + ni as usize) as u32
            } else {
                let s = -1.0 + (2.0 * ni as f64 + 1.0) / n as f64;
                let t = -1.0 + (2.0 * nj as f64 + 1.0) / n as f64;
                self.cell_of(face_to_direction(&TangentWarp, Face(f as u8), s, t)) as u32
            };
        }
        out
    }

    /// The distance in metres from `c` to its neighbour `nb` (index `k` of [`OFFS`]), modelled from the cell sizes.
    fn spacing_m(&self, c: usize, k: usize, nb: usize) -> f64 {
        0.5 * (libm::sqrt(self.area_m2(c)) + libm::sqrt(self.area_m2(nb))) * if k < 4 { 1.0 } else { core::f64::consts::SQRT_2 }
    }
}

/// The parameters of the run.
#[derive(Clone, Copy, Debug)]
pub struct CoupledParams {
    /// The drainage-area exponent of the stream power law (the slope exponent is 1).
    pub area_exp: f64,
    /// The step in years.
    pub dt_years: f64,
    /// The jitter of the receiver choice in `[0, 1)`: the slope of each direction is scaled by `1 + j (2 h - 1)`, `h` a hash of the cell and
    /// the direction; 0 is plain D8 (SPIKE-35: 0.4 to 0.8 removes most of the axis alignment).
    pub jitter: f64,
    /// Seeds the jitter hash.
    pub seed: u64,
    /// The height land cells never fall below, in metres.
    pub min_height_m: f64,
}

impl Default for CoupledParams {
    fn default() -> Self {
        CoupledParams { area_exp: 0.45, dt_years: 1.0e5, jitter: 0.4, seed: 0, min_height_m: 0.5 }
    }
}

/// The inputs per cell: the heights the run starts from (sea at or below 0), the uplift in m/year and the erodibility of the stream power law.
pub struct CoupledInput {
    pub z: Vec<f32>,
    pub uplift: Vec<f32>,
    pub erodibility: Vec<f32>,
}

/// What a step changed.
#[derive(Clone, Copy, Debug)]
pub struct StepStat {
    pub mean_dz_m: f64,
    pub max_dz_m: f64,
}

/// The state after the run: the heights, the drainage area in m2 (a land cell's own area plus everything that drains through it), the receiver
/// per cell ([`NONE`] for an outlet) and the change per step.
pub struct CoupledState {
    pub z: Vec<f32>,
    pub area_m2: Vec<f32>,
    pub receiver: Vec<u32>,
    pub steps: Vec<StepStat>,
}

/// Priority flood with epsilon from the coast: the filled surface (a depression is raised to just above its lowest outlet) and the pop order,
/// ascending in filled height with ties by cell index, which is the order a receiver always precedes its donors in. Sea cells (`z <= 0`) are
/// not in the order.
pub fn priority_flood(lat: &Lattice, z: &[f32]) -> (Vec<f32>, Vec<u32>) {
    let len = lat.len();
    let mut filled = z.to_vec();
    let mut visited = vec![false; len];
    let mut heap: BinaryHeap<Reverse<u64>> = BinaryHeap::new();
    let mut land = 0usize;
    for c in 0..len {
        if z[c] <= 0.0 {
            visited[c] = true;
        }
    }
    for c in 0..len {
        if z[c] <= 0.0 {
            continue;
        }
        land += 1;
        if lat.neighbours(c).iter().any(|&n| z[n as usize] <= 0.0) {
            visited[c] = true;
            heap.push(Reverse((u64::from(z[c].to_bits()) << 32) | c as u64));
        }
    }
    // A world without sea has no coast to flood from: its lowest cell (ties by index) is the one outlet.
    if heap.is_empty() && land > 0 {
        let lowest = (0..len).min_by_key(|&c| (z[c].to_bits(), c)).expect("a lattice has cells");
        visited[lowest] = true;
        heap.push(Reverse((u64::from(z[lowest].to_bits()) << 32) | lowest as u64));
    }
    let mut order: Vec<u32> = Vec::with_capacity(land);
    while let Some(Reverse(key)) = heap.pop() {
        let c = (key & 0xffff_ffff) as usize;
        order.push(c as u32);
        let fc = filled[c];
        for &n in lat.neighbours(c).iter() {
            let n = n as usize;
            if visited[n] {
                continue;
            }
            visited[n] = true;
            if z[n] <= fc {
                filled[n] = f32::from_bits(fc.to_bits() + 1);
            }
            heap.push(Reverse((u64::from(filled[n].to_bits()) << 32) | n as u64));
        }
    }
    // Land the flood never reached (not expected: the neighbour relation is symmetric) keeps its height and is processed first.
    if order.len() < land {
        let mut seen = vec![false; len];
        for &c in &order {
            seen[c as usize] = true;
        }
        let mut extra: Vec<u32> = (0..len).filter(|&c| z[c] > 0.0 && !seen[c]).map(|c| c as u32).collect();
        extra.extend(order);
        order = extra;
    }
    (filled, order)
}

/// The jitter factor of direction `k` at cell `c`: `1 + j (2 h - 1)`, `h` in `[0, 1)` a hash of the seed, the cell and the direction.
fn jitter_factor(seed: u64, c: usize, k: usize, jitter: f64) -> f64 {
    if jitter == 0.0 {
        return 1.0;
    }
    let h = (mix64(seed ^ ((c as u64) << 3 | k as u64)) >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0);
    1.0 + jitter * (2.0 * h - 1.0)
}

/// The receiver of every land cell and the direction (index of [`OFFS`]) it lies in: the neighbour with the steepest jittered slope down the
/// filled surface; [`NONE`] when no neighbour is lower.
pub fn receivers(lat: &Lattice, filled: &[f32], order: &[u32], p: &CoupledParams) -> (Vec<u32>, Vec<u8>) {
    assert!((0.0..1.0).contains(&p.jitter), "jitter {} is not in [0, 1)", p.jitter);
    let mut rec = vec![NONE; lat.len()];
    let mut dir = vec![0u8; lat.len()];
    for &c in order {
        let c = c as usize;
        let nb = lat.neighbours(c);
        let mut best = (0.0f64, NONE, 0u8);
        for k in 0..8 {
            let n = nb[k] as usize;
            let drop = f64::from(filled[c]) - f64::from(filled[n]);
            if drop > 0.0 {
                let s = drop / lat.spacing_m(c, k, n) * jitter_factor(p.seed, c, k, p.jitter);
                if s > best.0 {
                    best = (s, n as u32, k as u8);
                }
            }
        }
        rec[c] = best.1;
        dir[c] = best.2;
    }
    (rec, dir)
}

/// The drainage area in m2 by walking the pop order backwards: a cell's own area plus what its donors bring; sea receives nothing.
pub fn accumulate(lat: &Lattice, z: &[f32], order: &[u32], rec: &[u32]) -> Vec<f64> {
    let mut a = vec![0.0f64; lat.len()];
    for &c in order {
        a[c as usize] = lat.area_m2(c as usize);
    }
    for &c in order.iter().rev() {
        let r = rec[c as usize];
        if r != NONE && z[r as usize] > 0.0 {
            a[r as usize] += a[c as usize];
        }
    }
    a
}

/// One step: the flood, the receivers, the area and the implicit update `z = (z + dt U + f z_r) / (1 + f)`, `f = K A^m dt / distance`.
pub fn step(lat: &Lattice, input: &CoupledInput, z: &mut [f32], p: &CoupledParams) -> (Vec<u32>, Vec<f64>, StepStat) {
    assert!(
        z.len() == lat.len() && input.uplift.len() == lat.len() && input.erodibility.len() == lat.len(),
        "the inputs must have one value per cell"
    );
    assert!(z.iter().chain(&input.uplift).chain(&input.erodibility).all(|v| v.is_finite()), "the inputs must be finite");
    let (filled, order) = priority_flood(lat, z);
    let (rec, dir) = receivers(lat, &filled, &order, p);
    let a = accumulate(lat, z, &order, &rec);
    let (mut sum, mut max) = (0.0f64, 0.0f64);
    for &c in &order {
        let c = c as usize;
        let old = f64::from(z[c]);
        let r = rec[c];
        let (zr, f) = if r == NONE {
            (0.0, 0.0)
        } else {
            let dist = lat.spacing_m(c, usize::from(dir[c]), r as usize);
            (f64::from(z[r as usize]).max(0.0), f64::from(input.erodibility[c]) * libm::pow(a[c], p.area_exp) * p.dt_years / dist)
        };
        let new = ((old + p.dt_years * f64::from(input.uplift[c]) + f * zr) / (1.0 + f)).max(p.min_height_m);
        z[c] = new as f32;
        let d = (new - old).abs();
        sum += d;
        max = max.max(d);
    }
    (rec, a, StepStat { mean_dz_m: sum / order.len().max(1) as f64, max_dz_m: max })
}

/// The run: `steps` steps from the input heights.
pub fn run(lat: &Lattice, input: &CoupledInput, p: &CoupledParams, steps: usize) -> CoupledState {
    let mut z = input.z.clone();
    let mut log = Vec::with_capacity(steps);
    let mut last = (vec![NONE; lat.len()], vec![0.0; lat.len()]);
    for _ in 0..steps {
        let (rec, a, stat) = step(lat, input, &mut z, p);
        log.push(stat);
        last = (rec, a);
    }
    CoupledState { z, area_m2: last.1.iter().map(|&a| a as f32).collect(), receiver: last.0, steps: log }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: f64 = planet_core::EARTH_RADIUS_M;
    const PINNED_CHECKSUM: u64 = 3_741_216_821_528_484_935;

    /// A smooth world with about 60 % land: heights from the direction, uplift and erodibility constant.
    fn world(lat: &Lattice) -> CoupledInput {
        let z: Vec<f32> = (0..lat.len())
            .map(|c| {
                let d = lat.direction(c);
                (1200.0 * (libm::sin(3.0 * d.x + 1.0) * libm::cos(2.0 * d.y) + 0.5 * libm::sin(5.0 * d.z)) + 200.0) as f32
            })
            .collect();
        CoupledInput { uplift: vec![1.0e-3; z.len()], erodibility: vec![2.0e-5; z.len()], z }
    }

    // spec: GEN-040
    #[test]
    fn the_lattice_covers_the_sphere_and_its_neighbours_are_cells() {
        let lat = Lattice::new(12, R);
        let total: f64 = (0..lat.len()).map(|c| lat.area_m2(c)).sum();
        assert!((total / (4.0 * std::f64::consts::PI * R * R) - 1.0).abs() < 1e-9, "{total}");
        for c in (0..lat.len()).step_by(7) {
            for n in lat.neighbours(c) {
                assert!((n as usize) < lat.len() && n as usize != c);
            }
        }
    }

    // spec: GEN-040
    #[test]
    fn every_receiver_is_downhill_on_the_filled_surface_and_every_chain_ends_at_an_outlet() {
        let lat = Lattice::new(12, R);
        let input = world(&lat);
        assert!(input.z.iter().any(|&z| z <= 0.0) && input.z.iter().any(|&z| z > 0.0));
        for jitter in [0.0, 0.8] {
            let p = CoupledParams { jitter, ..CoupledParams::default() };
            let (filled, order) = priority_flood(&lat, &input.z);
            let (rec, _) = receivers(&lat, &filled, &order, &p);
            for &c in &order {
                let r = rec[c as usize];
                if r != NONE {
                    assert!(filled[r as usize] < filled[c as usize], "cell {c} drains uphill (jitter {jitter})");
                }
                let (mut at, mut hops) = (c as usize, 0);
                while rec[at] != NONE && input.z[rec[at] as usize] > 0.0 {
                    at = rec[at] as usize;
                    hops += 1;
                    assert!(hops <= lat.len(), "a cycle from cell {c}");
                }
            }
        }
    }

    // spec: GEN-040
    #[test]
    fn the_drainage_area_of_the_outlets_is_the_land_area() {
        let lat = Lattice::new(12, R);
        let input = world(&lat);
        let (filled, order) = priority_flood(&lat, &input.z);
        let (rec, _) = receivers(&lat, &filled, &order, &CoupledParams::default());
        let a = accumulate(&lat, &input.z, &order, &rec);
        let land: f64 = order.iter().map(|&c| lat.area_m2(c as usize)).sum();
        let outlets: f64 =
            order.iter().filter(|&&c| rec[c as usize] == NONE || input.z[rec[c as usize] as usize] <= 0.0).map(|&c| a[c as usize]).sum();
        assert!((outlets / land - 1.0).abs() < 1e-9, "{outlets} against {land}");
    }

    // spec: GEN-040
    #[test]
    fn the_run_is_bit_identical_twice_settles_and_keeps_land_above_the_floor() {
        let lat = Lattice::new(12, R);
        let input = world(&lat);
        let p = CoupledParams::default();
        let a = run(&lat, &input, &p, 30);
        let b = run(&lat, &input, &p, 30);
        assert!(a.z.iter().zip(&b.z).all(|(x, y)| x.to_bits() == y.to_bits()));
        assert!(a.area_m2.iter().zip(&b.area_m2).all(|(x, y)| x.to_bits() == y.to_bits()));
        assert_eq!(a.receiver, b.receiver);
        assert!(a.steps[29].mean_dz_m < a.steps[2].mean_dz_m, "{:?} then {:?}", a.steps[2], a.steps[29]);
        for (c, &z) in a.z.iter().enumerate() {
            assert!(input.z[c] <= 0.0 && z == input.z[c] || f64::from(z) >= p.min_height_m, "cell {c}: {z}");
            assert!(z.is_finite());
        }
        // The jitter changes the network, not the determinism.
        let j0 = run(&lat, &input, &CoupledParams { jitter: 0.0, ..p }, 5);
        assert_ne!(j0.receiver, run(&lat, &input, &CoupledParams { jitter: 0.8, ..p }, 5).receiver);
    }

    /// FNV-1a over the bits of a run: the same on every platform and thread count, or the determinism promise is broken.
    fn checksum(s: &CoupledState) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut eat = |v: u32| {
            for b in v.to_le_bytes() {
                h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        s.z.iter().for_each(|z| eat(z.to_bits()));
        s.area_m2.iter().for_each(|a| eat(a.to_bits()));
        s.receiver.iter().for_each(|r| eat(*r));
        h
    }

    // spec: GEN-040
    #[test]
    fn a_fixed_run_has_the_same_checksum_on_every_platform() {
        let lat = Lattice::new(12, R);
        let state = run(&lat, &world(&lat), &CoupledParams::default(), 30);
        assert_eq!(checksum(&state), PINNED_CHECKSUM, "the run changed: platform, order or arithmetic");
    }

    // spec: GEN-040
    #[test]
    fn a_world_without_sea_drains_to_its_lowest_cell_and_every_receiver_comes_first() {
        let lat = Lattice::new(12, R);
        let mut input = world(&lat);
        input.z.iter_mut().for_each(|z| *z = z.abs() + 1.0);
        let (filled, order) = priority_flood(&lat, &input.z);
        assert_eq!(order.len(), lat.len());
        let (rec, _) = receivers(&lat, &filled, &order, &CoupledParams::default());
        let mut at = vec![usize::MAX; lat.len()];
        order.iter().enumerate().for_each(|(i, &c)| at[c as usize] = i);
        for &c in &order {
            let r = rec[c as usize];
            assert!(r == NONE || at[r as usize] < at[c as usize], "cell {c} comes before its receiver {r}");
        }
        assert_eq!(rec.iter().filter(|&&r| r == NONE).count(), 1, "one outlet: the lowest cell");
        let a = accumulate(&lat, &input.z, &order, &rec);
        let total: f64 = (0..lat.len()).map(|c| lat.area_m2(c)).sum();
        let outlet = (0..lat.len()).find(|&c| rec[c] == NONE).unwrap();
        assert!((a[outlet] / total - 1.0).abs() < 1e-9, "{} of {total}", a[outlet]);
    }

    // spec: GEN-040
    #[test]
    fn with_sea_every_land_cell_has_a_receiver_that_comes_before_it() {
        let lat = Lattice::new(12, R);
        let input = world(&lat);
        let (filled, order) = priority_flood(&lat, &input.z);
        let (rec, _) = receivers(&lat, &filled, &order, &CoupledParams::default());
        let mut at = vec![usize::MAX; lat.len()];
        order.iter().enumerate().for_each(|(i, &c)| at[c as usize] = i);
        for &c in &order {
            let r = rec[c as usize];
            assert!(r != NONE, "land cell {c} has no receiver");
            assert!(input.z[r as usize] <= 0.0 || at[r as usize] < at[c as usize], "cell {c} comes before its receiver {r}");
        }
    }
}
