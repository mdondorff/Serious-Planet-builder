//! The distance to the coast of the field on a lattice (GEN-136; ADR 0107, ADR 0108): per cube-face lattice point the signed distance to the nearest sea-level crossing of the field and the margin
//! activity at that crossing, read bilinearly. The shelf of the ocean floor hangs from it ([`crate::shelf`]).
//!
//! The crossings are found on the lattice's cell edges (a sign change of the field, the point placed by linear interpolation), the nearest one to every lattice point by a 3D bucket search within the cap,
//! so that the distance is exact to the crossing points and does not depend on the field's gradient (which is not defined at the scale of its detail). Distances beyond the cap are the cap.
//!
//! GPU-free, `libm`, integer indices: the same bytes for any thread count (CON-14).

use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::par::{self, Par};
use planet_core::{Face, Vec3};

/// The narrowest bucket of the nearest-crossing search, in radians.
const MIN_BUCKET_RAD: f64 = 0.02;

/// The signed distance to the coast of a field on a lattice over the cube faces, and the margin activity at the nearest crossing.
#[derive(Debug)]
pub struct CoastLattice {
    cells: usize,
    cap_km: f64,
    /// Per face, row `j` outer: the distance in km (sea positive, land negative), at most the cap in size.
    dist: Vec<Vec<f32>>,
    /// Per face: the activity at the nearest crossing, in 1/255.
    act: Vec<Vec<u8>>,
}

impl CoastLattice {
    /// The lattice of `cells` cells per face edge (a power of two, so that a shared edge point is the same direction on both faces) of the field `signed` (land at 0 and above, sea below) on a
    /// planet of `radius_km`: distances up to `cap_km`, the activity read from `activity` at every crossing, on `threads` threads (the bytes do not depend on the count).
    pub fn build(
        signed: &(dyn Fn(Vec3) -> f64 + Sync),
        activity: &(dyn Fn(Vec3) -> f64 + Sync),
        cells: usize,
        cap_km: f64,
        radius_km: f64,
        threads: usize,
    ) -> CoastLattice {
        assert!(cells.is_power_of_two() && cells >= 4, "the coast lattice has a power of two cells per face edge, not {cells}");
        assert!(cap_km > 0.0 && cap_km.is_finite() && radius_km > 0.0, "the cap and the radius are positive");
        let n = cells;
        let map = TangentWarp;
        let par: &dyn Par = &threads;
        let dir = |f: usize, i: usize, j: usize| {
            face_to_direction(&map, Face(f as u8), 2.0 * i as f64 / n as f64 - 1.0, 2.0 * j as f64 / n as f64 - 1.0)
        };
        // The field at every lattice point.
        let rows = par::map(par, 6 * (n + 1), &|r| {
            let (f, j) = (r / (n + 1), r % (n + 1));
            (0..=n).map(|i| signed(dir(f, i, j))).collect::<Vec<f64>>()
        });
        let value = |f: usize, i: usize, j: usize| rows[f * (n + 1) + j][i];
        // The crossings on the cell edges, in the order face, row, column, edge direction.
        let crossings: Vec<Vec3> = par::map(par, 6 * (n + 1), &|r| {
            let (f, j) = (r / (n + 1), r % (n + 1));
            let mut out = Vec::new();
            for i in 0..=n {
                let v0 = value(f, i, j);
                for (di, dj) in [(1usize, 0usize), (0, 1)] {
                    if i + di > n || j + dj > n {
                        continue;
                    }
                    let v1 = value(f, i + di, j + dj);
                    if (v0 < 0.0) != (v1 < 0.0) {
                        let w = v0 / (v0 - v1);
                        let (a, b) = (dir(f, i, j), dir(f, i + di, j + dj));
                        out.push((a * (1.0 - w) + b * w).normalized());
                    }
                }
            }
            out
        })
        .into_iter()
        .flatten()
        .collect();
        let acts: Vec<u8> = par::map(par, crossings.len(), &|k| libm::round(255.0 * activity(crossings[k]).clamp(0.0, 1.0)) as u8);
        // A 3D grid of cells as wide as the cap's chord: every crossing within the cap of a point lies in the 27 cells around it.
        let cap_rad = cap_km / radius_km;
        // At least 0.02 rad (about 130 km): a smaller cap would need a grid of 1/cap^3 cells and a wider cell is always correct.
        let width = (cap_rad * 1.01).max(MIN_BUCKET_RAD);
        let m = (2.0 / width).ceil() as usize + 1;
        let cell_of = |p: Vec3| {
            (
                (((p.x + 1.0) / width) as usize).min(m - 1),
                (((p.y + 1.0) / width) as usize).min(m - 1),
                (((p.z + 1.0) / width) as usize).min(m - 1),
            )
        };
        let flat = |c: (usize, usize, usize)| (c.2 * m + c.1) * m + c.0;
        let mut start = vec![0u32; m * m * m + 1];
        for p in &crossings {
            start[flat(cell_of(*p)) + 1] += 1;
        }
        for c in 0..m * m * m {
            start[c + 1] += start[c];
        }
        let mut fill = start.clone();
        let mut items = vec![0u32; crossings.len()];
        for (k, p) in crossings.iter().enumerate() {
            let c = flat(cell_of(*p));
            items[fill[c] as usize] = k as u32;
            fill[c] += 1;
        }
        // The nearest crossing to every lattice point (the lowest index on ties), as a distance within the cap and its activity.
        let nodes = par::map(par, 6 * (n + 1), &|r| {
            let (f, j) = (r / (n + 1), r % (n + 1));
            (0..=n)
                .map(|i| {
                    let p = dir(f, i, j);
                    let c = cell_of(p);
                    let (mut best, mut at) = (-2.0f64, usize::MAX);
                    for z in c.2.saturating_sub(1)..=(c.2 + 1).min(m - 1) {
                        for y in c.1.saturating_sub(1)..=(c.1 + 1).min(m - 1) {
                            for x in c.0.saturating_sub(1)..=(c.0 + 1).min(m - 1) {
                                let cell = flat((x, y, z));
                                for &k in &items[start[cell] as usize..start[cell + 1] as usize] {
                                    let d = p.dot(crossings[k as usize]);
                                    if d > best || (d == best && (k as usize) < at) {
                                        (best, at) = (d, k as usize);
                                    }
                                }
                            }
                        }
                    }
                    // A crossing beyond the cap is not found, wherever the buckets fall: the result is a function of the field only.
                    let arc_km = if at == usize::MAX { f64::INFINITY } else { libm::acos(best.clamp(-1.0, 1.0)) * radius_km };
                    let km = arc_km.min(cap_km);
                    let signed_km = if value(f, i, j) < 0.0 { km } else { -km };
                    (signed_km as f32, if arc_km < cap_km { acts[at] } else { 0 })
                })
                .collect::<Vec<_>>()
        });
        let mut dist: Vec<Vec<f32>> = (0..6).map(|_| Vec::with_capacity((n + 1) * (n + 1))).collect();
        let mut act: Vec<Vec<u8>> = (0..6).map(|_| Vec::with_capacity((n + 1) * (n + 1))).collect();
        for (r, row) in nodes.into_iter().enumerate() {
            for (d, a) in row {
                dist[r / (n + 1)].push(d);
                act[r / (n + 1)].push(a);
            }
        }
        CoastLattice { cells: n, cap_km, dist, act }
    }

    pub fn cells(&self) -> usize {
        self.cells
    }

    pub fn cap_km(&self) -> f64 {
        self.cap_km
    }

    fn locate(&self, s: f64) -> (usize, f64) {
        let x = ((s + 1.0) * 0.5 * self.cells as f64).clamp(0.0, self.cells as f64);
        let i = (x as usize).min(self.cells - 1);
        (i, x - i as f64)
    }

    /// The signed distance to the coast in km (positive in the sea, negative on land, at most the cap in size) and the margin activity in `[0, 1]` at the unit direction `d`: bilinear, continuous
    /// across face edges.
    pub fn read(&self, d: Vec3) -> (f64, f64) {
        let (face, s, t) = direction_to_face(&TangentWarp, d);
        let ((i, fx), (j, fy)) = (self.locate(s), self.locate(t));
        let k = |di: usize, dj: usize| (j + dj) * (self.cells + 1) + i + di;
        let f = face.0 as usize;
        let bilinear = |value: &dyn Fn(usize) -> f64| {
            let (a, b) = (value(k(0, 0)) + (value(k(1, 0)) - value(k(0, 0))) * fx, value(k(0, 1)) + (value(k(1, 1)) - value(k(0, 1))) * fx);
            a + (b - a) * fy
        };
        (bilinear(&|p| f64::from(self.dist[f][p])), bilinear(&|p| f64::from(self.act[f][p]) / 255.0))
    }
}
