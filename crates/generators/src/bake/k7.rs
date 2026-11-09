//! K7, the local pass in the new chain (GEN-148, A3 sections 3.1 to 3.3, gate G-SURF): a tile of the local grid (102 m at Earth) starts from `B + C1(change)`, where `B` is the base read at every
//! cell (the store lattice's C1 read; the stride-4 window of pass 2 is retired) and the change is the regional solve's (a C1 read of the regional grid). The channels of K6 are burned in (the cells
//! along each segment are lowered to the segment's level profile, rasterised 8-connected at the local grid), then the breach of pass 1 runs at the local grid, the incising carve, thermal, the deposition
//! sweep and the pit spreading of pass 2. `L` is the result minus `B`, stored in steps of 0.25 m. The pit share and depth are reported after each stage, and G-SURF compares the runtime's `B + L`
//! with the bake's surface at the lattice nodes.

use super::carve::{carve, CarveParams};
use super::deposit::{deposit_on_own_route, spread_pits, DepositParams, Kc};
use super::flood::priority_flood;
use super::map::catmull_rom;
use super::thermal::thermal;
use super::tile::{sample_window, window_grid, Pass2Params, TileGrid, TileId, QUANT_STEP_M};
use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};

/// A field on a window of a face's grid (the regional grid at 814 m): the Catmull-Rom (C1) read of the cells' values, clamped to the four cells around the point as `map::change_in_face` is, the
/// window's edge clamped.
pub struct RegionField {
    face: Face,
    cells: usize,
    i0: i64,
    j0: i64,
    w: usize,
    h: usize,
    data: Vec<f32>,
}

impl RegionField {
    /// `data` row-major, `w x h`, cell `(0, 0)` the cell `(i0, j0)` of the face's grid of `cells` cells an edge.
    pub fn new(face: Face, cells: usize, (i0, j0): (i64, i64), w: usize, h: usize, data: Vec<f32>) -> RegionField {
        assert!(w >= 4 && h >= 4 && data.len() == w * h, "a window of at least 4 x 4 cells");
        RegionField { face, cells, i0, j0, w, h, data }
    }

    /// The value at a direction on this face's plane.
    pub fn at(&self, d: Vec3) -> f64 {
        let w = d.dot(self.face.normal());
        assert!(w > 0.0 && w.is_finite(), "direction {d:?} is not in front of face {}", self.face.0);
        let plane = |u: f64| libm::atan(u) / core::f64::consts::FRAC_PI_4;
        let (s, t) = (plane(d.dot(self.face.u_axis()) / w), plane(d.dot(self.face.v_axis()) / w));
        let n = self.cells as f64;
        let x = ((s + 1.0) * 0.5 * n - 0.5 - self.i0 as f64).clamp(0.0, (self.w - 1) as f64);
        let y = ((t + 1.0) * 0.5 * n - 0.5 - self.j0 as f64).clamp(0.0, (self.h - 1) as f64);
        let (i, j) = ((x as usize).min(self.w - 2), (y as usize).min(self.h - 2));
        let (fx, fy) = (x - i as f64, y - j as f64);
        let at =
            |a: i64, b: i64| f64::from(self.data[b.clamp(0, self.h as i64 - 1) as usize * self.w + a.clamp(0, self.w as i64 - 1) as usize]);
        let (ii, jj) = (i as i64, j as i64);
        let row = |dj: i64| catmull_rom([at(ii - 1, jj + dj), at(ii, jj + dj), at(ii + 1, jj + dj), at(ii + 2, jj + dj)], fx);
        let v = catmull_rom([row(-1), row(0), row(1), row(2)], fy);
        let around = [at(ii, jj), at(ii + 1, jj), at(ii, jj + 1), at(ii + 1, jj + 1)];
        v.clamp(around.iter().copied().fold(f64::INFINITY, f64::min), around.iter().copied().fold(f64::NEG_INFINITY, f64::max))
    }
}

/// A channel segment on the local grid: points in the face's cell-index space (cell `i` has its centre at `i`) and the bed level at each point (not rising downstream).
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub points: Vec<(f64, f64)>,
    pub levels: Vec<f64>,
}

/// Burns the channels into the window `z` (`w x w`, cell `(0, 0)` the face cell `(i0, j0)`): along each segment between consecutive points the cells of the 8-connected line are lowered to the
/// level interpolated linearly along the segment (never raised). Returns the number of cells lowered.
pub fn burn_channels(z: &mut [f64], w: usize, (i0, j0): (i64, i64), channels: &[Channel]) -> usize {
    let mut lowered = 0;
    for ch in channels {
        assert!(ch.points.len() == ch.levels.len() && !ch.points.is_empty(), "a level for every point");
        let mut burn = |x: i64, y: i64, level: f64| {
            let (a, b) = (x - i0, y - j0);
            if a >= 0 && b >= 0 && (a as usize) < w && (b as usize) < w {
                let c = b as usize * w + a as usize;
                if level < z[c] {
                    z[c] = level;
                    lowered += 1;
                }
            }
        };
        if ch.points.len() == 1 {
            burn(libm::round(ch.points[0].0) as i64, libm::round(ch.points[0].1) as i64, ch.levels[0]);
        }
        for k in 0..ch.points.len().saturating_sub(1) {
            let ((x0, y0), (x1, y1)) = (ch.points[k], ch.points[k + 1]);
            let (l0, l1) = (ch.levels[k], ch.levels[k + 1]);
            let steps = libm::fabs(x1 - x0).max(libm::fabs(y1 - y0)).max(1.0).ceil() as i64;
            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                burn(libm::round(x0 + (x1 - x0) * t) as i64, libm::round(y0 + (y1 - y0) * t) as i64, l0 + (l1 - l0) * t);
            }
        }
    }
    lowered
}

/// The pits of a surface: land cells whose fill depth reaches `min_m`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PitStats {
    pub land_cells: usize,
    pub pit_cells: usize,
    pub p50_m: f64,
    pub p99_m: f64,
    pub max_m: f64,
}

impl PitStats {
    /// The share of the land cells that are pits.
    pub fn share(&self) -> f64 {
        self.pit_cells as f64 / self.land_cells.max(1) as f64
    }
}

/// The pits of `z` on `grid` (outlets: the sea and the window's border) at the depth `min_m`.
pub fn pit_stats(grid: &super::Grid, z: &[f64], sea_level: f64, epsilon: f64, min_m: f64) -> PitStats {
    let outlet = grid.outlets(z, sea_level);
    let filled = priority_flood(grid, z, &outlet, epsilon);
    let mut depths: Vec<f64> = Vec::new();
    let mut land = 0usize;
    for p in 0..z.len() {
        if z[p] > sea_level && !outlet[p] {
            land += 1;
            let d = filled[p] - z[p];
            if d >= min_m {
                depths.push(d);
            }
        }
    }
    depths.sort_by(|a, b| a.total_cmp(b));
    let at = |f: f64| if depths.is_empty() { 0.0 } else { depths[((depths.len() - 1) as f64 * f) as usize] };
    PitStats { land_cells: land, pit_cells: depths.len(), p50_m: at(0.5), p99_m: at(0.99), max_m: depths.last().copied().unwrap_or(0.0) }
}

/// The parameters of K7 besides the geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct K7Params {
    /// Pass 2's parameters; the local carve's breach is pass 1's rule at the local grid (`breach_max_m`, `breach_slope`).
    pub pass2: Pass2Params,
}

impl K7Params {
    /// Pass 2's start values with the breach of pass 1 (500 m limit, 0.1 % slope) in the local carve.
    pub fn standard(seed: u64, sea_level: f64) -> K7Params {
        let mut pass2 = Pass2Params::standard(seed, sea_level);
        pass2.carve.breach_max_m = CarveParams::default().breach_max_m;
        K7Params { pass2 }
    }
}

/// What K7 did on a tile.
pub struct K7Result {
    /// The window's heights after the pass, and `B` at the window's cells.
    pub z: Vec<f64>,
    pub b: Vec<f64>,
    /// The core's change against `B`, as stored (f32, before the 0.25 m quantisation).
    pub change: Vec<f32>,
    /// The cells lowered by the burn-in.
    pub burned: usize,
    /// The pits (at `pit_min_m`) after each stage: the start, the burn-in, the breach alone, the incising carve, the whole pass.
    pub pits: Vec<(&'static str, PitStats)>,
}

/// K7 on one tile. `base` is `B` (read at every cell of the window), `regional` the regional change (a C1 read of the regional solve, `RegionField::at`), `channels` the channel segments on the local
/// grid (cell-index space of the face, see [`Channel`]).
pub fn k7_tile(
    g: &TileGrid,
    id: TileId,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    regional: &(dyn Fn(Vec3) -> f64 + Sync),
    channels: &[Channel],
    p: &K7Params,
    threads: usize,
) -> K7Result {
    g.check().expect("a usable tile geometry");
    let q = &p.pass2;
    assert!(q.carve.round_rules.is_default() && !q.carve.cell_metric, "K7 runs the plane kernels of pass 2");
    let grid = window_grid(g, id);
    let b = sample_window(g, id, base, threads);
    let dr = sample_window(g, id, regional, threads);
    let mut z: Vec<f64> = b.iter().zip(&dr).map(|(b, c)| b + c).collect();
    drop(dr);
    let eps = q.carve.epsilon;
    let stats = |z: &[f64]| pit_stats(&grid, z, q.sea_level, eps, q.pit_min_m.max(0.5));
    let mut pits = vec![("base + regional change", stats(&z))];
    let (w, t, h) = (g.window(), g.tile as i64, g.halo as i64);
    let origin = (i64::from(id.ti) * t - h, i64::from(id.tj) * t - h);
    let burned = burn_channels(&mut z, w, origin, channels);
    pits.push(("channels burned in", stats(&z)));
    let z_burned = z.clone();
    let outlet = grid.outlets(&z, q.sea_level);
    let stream = super::face::face_stream(q.seed, id.face);
    let local = CarveParams { max_area: q.local_area_cells * g.dx() * g.dx(), ..q.carve };
    // The breach alone: the carve with no incision (`k_dt` 0) runs the breach of pass 1 and leaves the rest.
    let mut breach_only = z.clone();
    carve(&grid, &mut breach_only, &outlet, &CarveParams { k_dt: 0.0, steps: 1, reroute_every: 1, ..local }, None, stream);
    pits.push(("breach", stats(&breach_only)));
    drop(breach_only);
    let _ = carve(&grid, &mut z, &outlet, &local, None, stream);
    pits.push(("breach and incision", stats(&z)));
    thermal(&grid, &mut z, &q.thermal, threads);
    deposit_on_own_route(
        &grid,
        &mut z,
        &z_burned,
        eps,
        stream,
        &DepositParams { kc: Kc::Uniform(q.kc), zeta: q.zeta, sea_level: q.sea_level },
    );
    spread_pits(&grid, &mut z, q.sea_level, q.pit_min_m, q.pit_passes);
    pits.push(("the whole pass", stats(&z)));
    let (tile, halo) = (g.tile, g.halo);
    let mut change = Vec::with_capacity(tile * tile);
    for j in 0..tile {
        for i in 0..tile {
            let c = (j + halo) * w + i + halo;
            change.push((z[c] - b[c]) as f32);
        }
    }
    K7Result { z, b, change, burned, pits }
}

/// The result of G-SURF at the lattice nodes of a tile.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfaceGate {
    /// Lattice nodes read.
    pub nodes: usize,
    /// The largest `|runtime - bake|` before quantisation and with `L` quantised to steps of `QUANT_STEP_M`, over the nodes.
    pub max_unquantised_m: f64,
    pub max_quantised_m: f64,
    /// The same at the centres of the core's cells (where the C1 read returns the stored value).
    pub max_centres_unquantised_m: f64,
    pub max_centres_quantised_m: f64,
}

/// A C1 read of the window field `v` (`w x w`, centres at the integer coordinates) at `(x, y)`, with the clamp of [`RegionField`].
fn read_c1(v: &[f64], w: usize, x: f64, y: f64) -> f64 {
    let (i, j) = ((x as usize).min(w - 2), (y as usize).min(w - 2));
    let (fx, fy) = (x - i as f64, y - j as f64);
    let at = |a: i64, b: i64| v[b.clamp(0, w as i64 - 1) as usize * w + a.clamp(0, w as i64 - 1) as usize];
    let (ii, jj) = (i as i64, j as i64);
    let row = |dj: i64| catmull_rom([at(ii - 1, jj + dj), at(ii, jj + dj), at(ii + 1, jj + dj), at(ii + 2, jj + dj)], fx);
    let r = catmull_rom([row(-1), row(0), row(1), row(2)], fy);
    let around = [at(ii, jj), at(ii + 1, jj), at(ii, jj + 1), at(ii + 1, jj + 1)];
    r.clamp(around.iter().copied().fold(f64::INFINITY, f64::min), around.iter().copied().fold(f64::NEG_INFINITY, f64::max))
}

/// G-SURF on a tile: the runtime's surface `B + C1(L)` against the bake's `C1(z)` at the lattice nodes (every `lattice_step` local cells, on the cell corners) inside the core, two cells in from its
/// edge (the read needs its neighbours), with `L = z - B` as stored (quantised to `QUANT_STEP_M`) and unquantised; and at the centres of the same region's cells.
pub fn surface_gate(g: &TileGrid, id: TileId, base: &(dyn Fn(Vec3) -> f64 + Sync), r: &K7Result, lattice_step: usize) -> SurfaceGate {
    let (w, t, h) = (g.window(), g.tile as i64, g.halo as i64);
    let (i0, j0) = (i64::from(id.ti) * t - h, i64::from(id.tj) * t - h);
    let l: Vec<f64> = r.z.iter().zip(&r.b).map(|(z, b)| z - b).collect();
    let step = f64::from(QUANT_STEP_M);
    let lq: Vec<f64> = l.iter().map(|v| (libm::round(v / step).clamp(f64::from(i16::MIN), f64::from(i16::MAX))) * step).collect();
    let mut out = SurfaceGate::default();
    // Nodes: the corners E = lattice_step * a of the face's cells; the node is at the cell-index coordinate E - 0.5.
    let (lo, hi) = (h + 2, h + t - 3);
    let n = g.cells as f64;
    let e_first = ((i0 + lo) as f64 / lattice_step as f64).ceil() as i64;
    let e_last = ((i0 + hi) as f64 / lattice_step as f64).floor() as i64;
    let f_first = ((j0 + lo) as f64 / lattice_step as f64).ceil() as i64;
    let f_last = ((j0 + hi) as f64 / lattice_step as f64).floor() as i64;
    for fb in f_first..=f_last {
        for ea in e_first..=e_last {
            let (ex, ey) = ((ea * lattice_step as i64) as f64, (fb * lattice_step as i64) as f64);
            let (x, y) = (ex - 0.5 - i0 as f64, ey - 0.5 - j0 as f64);
            let d = face_to_direction(&TangentWarp, id.face, 2.0 * ex / n - 1.0, 2.0 * ey / n - 1.0);
            let b_node = base(d);
            let bake = read_c1(&r.z, w, x, y);
            out.nodes += 1;
            out.max_unquantised_m = out.max_unquantised_m.max(libm::fabs(b_node + read_c1(&l, w, x, y) - bake));
            out.max_quantised_m = out.max_quantised_m.max(libm::fabs(b_node + read_c1(&lq, w, x, y) - bake));
        }
    }
    for j in lo..=hi {
        for i in lo..=hi {
            let c = j as usize * w + i as usize;
            out.max_centres_unquantised_m = out.max_centres_unquantised_m.max(libm::fabs(r.b[c] + l[c] - r.z[c]));
            out.max_centres_quantised_m = out.max_centres_quantised_m.max(libm::fabs(r.b[c] + lq[c] - r.z[c]));
        }
    }
    let _ = direction_to_face;
    out
}
