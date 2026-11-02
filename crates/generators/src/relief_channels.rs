//! The relief channels of the bake (ADR 0088 points 1 and 2; GEN-097, GEN-099): the relief driver `k` and the ring mean `m` on the coupled lattice, f32, with a
//! min and max per 32 x 32 block.
//!
//! `k` is the class weights' blended driver (GEN-097) tapered to 1 towards the coast by `1 + smoothstep(0, 400 m, m) (k - 1)`, `m` the mean of the bake's
//! elevation at the lattice point and at 8 points of a ring of 10 km radius. Both are pure functions of the point's direction, so a point shared by two cube faces
//! gets the same value on both and the result is the same bits for any thread count (CON-14: `libm`, a fixed order of the 9 samples, rows as the unit of work).
//! The bake's elevation is an input (a function of the direction), so this module reads no store and the tests run it on synthetic surfaces.

use crate::classes::{blended_k, ClassLattice};
use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::par::{self, Par};
use planet_core::{smoothstep, Face, Vec3};

/// Bump whenever the channels' values or layout change (the stored channels carry it).
pub const RELIEF_CHANNELS_VERSION: u32 = 1;
/// Cells per face edge of the coupled lattice the channels live on (GEN-099, about 4.9 km at Earth's size).
pub const CHANNEL_CELLS: usize = 2048;
/// Cells per block edge of the min and max ranges (GEN-099).
pub const BLOCK_CELLS: usize = 32;
/// The radius of the ring the mean `m` is taken over, in metres (GEN-099).
pub const RING_RADIUS_M: f64 = 10_000.0;
/// The ring mean above which the taper has fully released `k` (GEN-097).
pub const TAPER_FULL_M: f64 = 400.0;
/// The default step limit of `k` between neighbouring lattice points (GEN-097: the 99th percentile of the steps is at most this; a bake parameter).
pub const DEFAULT_MAX_STEP: f64 = 2.0;

/// The `k` and `m` channels on one lattice, with their block ranges.
pub struct ReliefChannels {
    cells: usize,
    /// The tapered `k` per face, `(cells + 1)^2` points, row `j` of `i` (the lattice's own order).
    k: Vec<Vec<f32>>,
    /// The ring mean per face, the same layout.
    m: Vec<Vec<f32>>,
    /// Min and max of `k` and `m` per block of each face, `(cells / BLOCK_CELLS)^2` blocks, row-major: `[k_min, k_max, m_min, m_max]`.
    ranges: Vec<Vec<[f32; 4]>>,
}

/// The 8 compass points of the ring and its angular radius, computed once per radius (the trigonometry is the same for every lattice point).
pub struct Ring {
    cos_a: f64,
    sin_a: f64,
    compass: [(f64, f64); 8],
}

impl Ring {
    /// The ring of [`RING_RADIUS_M`] on a body of radius `radius_m` (more than the ring's radius).
    pub fn new(radius_m: f64) -> Ring {
        assert!(radius_m > RING_RADIUS_M, "the body's radius must exceed the ring's {RING_RADIUS_M} m, not {radius_m} m");
        let a = RING_RADIUS_M / radius_m;
        let mut compass = [(0.0, 0.0); 8];
        for (q, c) in compass.iter_mut().enumerate() {
            let theta = core::f64::consts::FRAC_PI_4 * q as f64;
            *c = (libm::cos(theta), libm::sin(theta));
        }
        Ring { cos_a: libm::cos(a), sin_a: libm::sin(a), compass }
    }
}

/// `k` and `m` at a direction: `classes` read bilinearly, `elevation` the bake's elevation in metres at a direction.
/// A pure function of the direction, so the same on both faces of a shared edge point.
pub fn point(classes: &ClassLattice, elevation: &dyn Fn(Vec3) -> f64, ring: &Ring, d: Vec3) -> (f64, f64) {
    let m = ring_mean_with(elevation, ring, d);
    debug_assert!(m.is_finite(), "the bake's elevation is not finite around {d:?}");
    let k = blended_k(&classes.weights_at(d));
    (1.0 + smoothstep(0.0, TAPER_FULL_M, m) * (k - 1.0), m)
}

/// The mean of the elevation at `d` and at the 8 compass points of the ring of [`RING_RADIUS_M`] around it, in a fixed order.
pub fn ring_mean(elevation: &dyn Fn(Vec3) -> f64, radius_m: f64, d: Vec3) -> f64 {
    ring_mean_with(elevation, &Ring::new(radius_m), d)
}

/// [`ring_mean`] for a prepared ring.
///
/// The compass is east and north (east is the Z axis crossed with `d`, north completes the frame): it is singular only at the two poles, so the ring's orientation
/// is continuous over every face edge and diagonal (a frame chosen from the smallest coordinate rotates by up to 30 degrees across the face diagonals, and
/// 8 samples do not cancel the angular harmonics of a rough surface under that). At a pole east is the Y axis, the same at every pole point.
pub fn ring_mean_with(elevation: &dyn Fn(Vec3) -> f64, ring: &Ring, d: Vec3) -> f64 {
    let east = {
        let e = Vec3::new(-d.y, d.x, 0.0);
        let l = e.length();
        if l > 1e-12 {
            e * (1.0 / l)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        }
    };
    let north = d.cross(east);
    let mut sum = elevation(d);
    for &(c, s) in &ring.compass {
        let dir = d * ring.cos_a + (east * c + north * s) * ring.sin_a;
        sum += elevation(dir.normalized());
    }
    sum / 9.0
}

/// The number of lattice cells a class paint item's `k` change `delta_k` is feathered over so that no step exceeds `max_step` (GEN-097).
pub fn feather_cells(delta_k: f64, max_step: f64) -> usize {
    assert!(max_step > 0.0, "the step limit of k is positive, not {max_step}");
    libm::ceil(libm::fabs(delta_k) / max_step) as usize
}

impl ReliefChannels {
    /// Computes both channels on `cells` cells per face edge (a power of two, a multiple of [`BLOCK_CELLS`]).
    pub fn compute(
        classes: &ClassLattice,
        elevation: &(dyn Fn(Vec3) -> f64 + Sync),
        radius_m: f64,
        cells: usize,
        threads: usize,
    ) -> ReliefChannels {
        assert!(
            cells.is_power_of_two() && cells >= BLOCK_CELLS,
            "the channels have a power of two cells per face edge of at least {BLOCK_CELLS}, not {cells}"
        );
        let n = cells;
        let ring = Ring::new(radius_m);
        let map = TangentWarp;
        let par: &dyn Par = &threads;
        // One row of one face per piece of work: every value is independent of the others.
        let rows = par::map(par, 6 * (n + 1), &|idx| {
            let (f, j) = (idx / (n + 1), idx % (n + 1));
            let t = 2.0 * j as f64 / n as f64 - 1.0;
            (0..=n)
                .map(|i| {
                    let s = 2.0 * i as f64 / n as f64 - 1.0;
                    let (k, m) = point(classes, elevation, &ring, face_to_direction(&map, Face(f as u8), s, t));
                    (k as f32, m as f32)
                })
                .collect::<Vec<_>>()
        });
        let new_face = || Vec::<f32>::with_capacity((n + 1) * (n + 1));
        let (mut k, mut m): (Vec<Vec<f32>>, Vec<Vec<f32>>) = ((0..6).map(|_| new_face()).collect(), (0..6).map(|_| new_face()).collect());
        for (idx, row) in rows.into_iter().enumerate() {
            for (kv, mv) in row {
                k[idx / (n + 1)].push(kv);
                m[idx / (n + 1)].push(mv);
            }
        }
        let b = n / BLOCK_CELLS;
        // A block holds the points of its cells, so the shared row and column of the next block are in both: a bilinear read inside a block stays inside its range
        // (in exact arithmetic: a reader must interpolate in f64, or allow one f32 ulp, since `a + t (b - a)` in f32 can overshoot).
        let ranges = (0..6)
            .map(|f| {
                let mut out = Vec::with_capacity(b * b);
                for bj in 0..b {
                    for bi in 0..b {
                        let mut r = [f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY];
                        for j in bj * BLOCK_CELLS..=(bj + 1) * BLOCK_CELLS {
                            for i in bi * BLOCK_CELLS..=(bi + 1) * BLOCK_CELLS {
                                let at = j * (n + 1) + i;
                                r[0] = r[0].min(k[f][at]);
                                r[1] = r[1].max(k[f][at]);
                                r[2] = r[2].min(m[f][at]);
                                r[3] = r[3].max(m[f][at]);
                            }
                        }
                        out.push(r);
                    }
                }
                out
            })
            .collect();
        ReliefChannels { cells: n, k, m, ranges }
    }

    pub fn cells(&self) -> usize {
        self.cells
    }

    /// The tapered `k` and the ring mean `m` at lattice point `(i, j)` of `face`.
    pub fn at(&self, face: Face, i: usize, j: usize) -> (f32, f32) {
        let at = j * (self.cells + 1) + i;
        (self.k[face.0 as usize][at], self.m[face.0 as usize][at])
    }

    /// `[k_min, k_max, m_min, m_max]` of block `(bi, bj)` of `face`.
    pub fn block_range(&self, face: Face, bi: usize, bj: usize) -> [f32; 4] {
        self.ranges[face.0 as usize][bj * (self.cells / BLOCK_CELLS) + bi]
    }

    /// The share of neighbouring lattice point pairs (within a face) whose TAPERED `k` differs by more than `max_step`: a diagnostic of the taper. GEN-097's gate is on the
    /// untapered blended `k` (`ClassLattice::k_step_quantile`).
    pub fn k_steps_over(&self, max_step: f64) -> f64 {
        let n = self.cells;
        let (mut over, mut total) = (0u64, 0u64);
        for face in &self.k {
            for j in 0..=n {
                for i in 0..=n {
                    for (di, dj) in [(1, 0), (0, 1)] {
                        if i + di <= n && j + dj <= n {
                            total += 1;
                            if libm::fabs(f64::from(face[(j + dj) * (n + 1) + i + di]) - f64::from(face[j * (n + 1) + i])) > max_step {
                                over += 1;
                            }
                        }
                    }
                }
            }
        }
        over as f64 / total as f64
    }

    /// The channels' bytes, for the identity checks of the tests and the stored form's key: the version, the cells, then `k` and `m` of every face, f32 little-endian.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + 8 * 6 * (self.cells + 1) * (self.cells + 1));
        out.extend(RELIEF_CHANNELS_VERSION.to_le_bytes());
        out.extend((self.cells as u32).to_le_bytes());
        for f in 0..6 {
            for v in self.k[f].iter().chain(&self.m[f]) {
                out.extend(v.to_le_bytes());
            }
        }
        out
    }
}
