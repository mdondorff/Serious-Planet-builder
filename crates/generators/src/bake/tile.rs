//! Pass 2 of the bake on one tile of the layer grid (GEN-101; m9-bake PR 5): the base plus the face carve's change, a local carve that only incises, thermal on active cells and one
//! deposition sweep, and the change of the tile's core against the base field.
//!
//! The layer grid has `cells` cells per face edge (98,304 for 101.8 m at Earth's size; fewer for a coarse planet pass), cut into square tiles of `tile` core cells (3,072) with a halo of
//! `halo` cells (128) around each. A tile's window is `tile + 2 halo` cells a side; a window may reach past the face's edge, where the plane of the face continues over the neighbouring
//! ground (the base and the face change are functions of the direction). The routing's jitter and the thermal's pair hash follow the GLOBAL cell of the face's grid, so two windows that
//! hold a cell treat it alike. A window's border still matters: the local carve caps the catchment area, but the deposition sweep routes the whole window, so its loads and capacities
//! depend on where the border falls; the seam this leaves is measured (TEST-084), not assumed away.

use super::carve::{carve, CarveParams};
use super::deposit::{deposit_on_own_route, spread_pits, DepositParams, Kc};
use super::face::{face_stream, fill_code, fill_m, FILL_MIN_M};
use super::flood::fill_depth;
use super::map::catmull_rom;
use super::thermal::{thermal, ThermalParams};
use super::Grid;
use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::par::{self, Par};
use planet_core::{Face, Vec3};

/// The quantisation step of a stored change in metres (a change is stored as an `i16` count of steps: `+-8 km` at 0.25 m).
pub const QUANT_STEP_M: f32 = 0.25;
/// The base field is evaluated on every `BASE_STRIDE`-th cell of a window and interpolated between (Catmull-Rom): the base is 86 % of a tile's cost (SPIKE-98), and the stored change is taken
/// against the interpolated window, so the interpolation error cancels in it (measured against the every-cell window: rms 4.0 m on a mountain tile, 1.0 m on hills).
pub const BASE_STRIDE: usize = 4;
/// Added to a window's cell indices in the global cell id so that a halo past the face's edge (a negative index) stays a `u32`.
const ID_OFFSET: i64 = 1 << 20;

/// The geometry of the layer grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileGrid {
    /// Cells per face edge.
    pub cells: usize,
    /// Core cells per tile edge.
    pub tile: usize,
    /// Halo cells around the core.
    pub halo: usize,
    pub radius_m: f64,
}

/// A tile: the cube face and the tile's index along the face's two axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileId {
    pub face: Face,
    pub ti: u32,
    pub tj: u32,
}

impl TileGrid {
    pub fn dx(&self) -> f64 {
        self.radius_m * core::f64::consts::FRAC_PI_2 / self.cells as f64
    }

    /// Tiles per face edge.
    pub fn tiles_per_edge(&self) -> usize {
        self.cells / self.tile
    }

    /// Cells per window edge.
    pub fn window(&self) -> usize {
        self.tile + 2 * self.halo
    }

    /// The direction of the centre of cell `(i, j)` of the face's grid (the plane continues past `0..cells`).
    pub fn direction(&self, face: Face, i: i64, j: i64) -> Vec3 {
        let n = self.cells as f64;
        face_to_direction(&TangentWarp, face, 2.0 * (i as f64 + 0.5) / n - 1.0, 2.0 * (j as f64 + 0.5) / n - 1.0)
    }

    /// Refuses a geometry that does not tile the face, has a halo of half a tile or more, or is too large for the cell index.
    pub fn check(&self) -> Result<(), String> {
        if self.tile == 0 || !self.cells.is_multiple_of(self.tile) {
            return Err(format!("the tile ({} cells) does not divide the face edge ({} cells)", self.tile, self.cells));
        }
        if !self.tile.is_multiple_of(BASE_STRIDE) {
            return Err(format!("the tile ({} cells) must be a multiple of the base stride ({BASE_STRIDE}): the sparse base lattice of every window falls on the same global cells", self.tile));
        }
        if self.halo * 2 >= self.tile {
            return Err(format!("the halo ({} cells) must be under half the tile ({} cells)", self.halo, self.tile));
        }
        if self.halo * 8 > self.cells {
            return Err(format!("the halo ({} cells) must be at most an eighth of the face edge ({} cells): beyond that the extended plane stretches the cells", self.halo, self.cells));
        }
        if self.window() < 3 || self.window().checked_mul(self.window()).is_none_or(|n| n >= u32::MAX as usize) {
            return Err(format!("a window of {} cells a side is too small or too large", self.window()));
        }
        if (self.cells as i64 + self.halo as i64 + ID_OFFSET) >= u32::MAX as i64 {
            return Err(format!("{} cells a face edge is too many for the global cell id", self.cells));
        }
        Ok(())
    }

    /// Every tile of every face, face by face, row by row.
    pub fn all_tiles(&self) -> Vec<TileId> {
        let n = self.tiles_per_edge() as u32;
        (0..6u8).flat_map(|f| (0..n).flat_map(move |tj| (0..n).map(move |ti| TileId { face: Face(f), ti, tj }))).collect()
    }
}

/// The parameters of pass 2 besides the geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pass2Params {
    /// The local carve: only incises, catchments up to 1 km^2 (see [`Pass2Params::standard`]).
    pub carve: CarveParams,
    pub thermal: ThermalParams,
    /// The deposition's `kc` when no per-cell value is given, and its `zeta`.
    pub kc: f64,
    pub zeta: f64,
    /// The local carve's catchments are capped at this many cells of area (96 is about 1 km^2 at 101.8 m); the cap in square metres follows the grid's cell size, so a coarse planet pass
    /// keeps a cap of the same number of cells. `carve.max_area` is overwritten by it.
    pub local_area_cells: f64,
    pub sea_level: f64,
    pub seed: u64,
    /// A pit deeper than this many metres (a land cell lower than its 8 neighbours) is spread over its neighbours after the sweep, with the volume kept (GEN-101); 0 and no passes switch it off.
    pub pit_min_m: f64,
    /// At most this many passes of the pit spreading.
    pub pit_passes: usize,
}

impl Pass2Params {
    /// GEN-101's start values: the local carve of 10 steps without uplift, breach or fill and with the area capped at 1 km^2; thermal at 300 passes of 1/8 of the excess (SPIKE-98: the 100-pass cap left 73 m of excess on a mountain tile); `kc` 1e5 and `zeta` 0.5.
    pub fn standard(seed: u64, sea_level: f64) -> Pass2Params {
        Pass2Params {
            carve: CarveParams {
                u_over_k: 0.0,
                steps: 10,
                breach_max_m: 0.0,
                sea_level,
                max_area: 0.0,
                incise_only: true,
                ..CarveParams::default()
            },
            thermal: ThermalParams { sea_level, passes: 300, k: 1.0 / 8.0, ..ThermalParams::default() },
            kc: 1.0e5,
            zeta: 0.5,
            local_area_cells: 96.0,
            sea_level,
            seed,
            pit_min_m: 0.5,
            pit_passes: 32,
        }
    }
}

/// What a tile's pass 2 did (bake statistics, STRM-065).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileStats {
    /// Land cells of the core.
    pub land_cells: usize,
    pub thermal_passes: usize,
    /// Cells that still gave in the last thermal pass: the unsettled slopes at the pass cap.
    pub thermal_moving_at_cap: usize,
    /// Volumes of the deposition sweep over the whole window (the halo included), in cubic metres: eroded = deposited + exported.
    pub eroded_m3: f64,
    pub deposited_m3: f64,
    pub exported_m3: f64,
    /// Cells of the core whose change is beyond the quantisation range (+-8,191.75 m) and is stored clamped.
    pub clamped_cells: usize,
}

/// A tile's result: the core's change against the base field (final minus base, f32, row-major `tile x tile`; it contains the face carve's change), the deposited depth (GEN-111) and
/// the statistics. `deposited` holds codes of [`depth_code`] per core cell, or is empty when the result was read from a record that holds no channel.
#[derive(Clone, Debug, PartialEq)]
pub struct TileResult {
    pub id: TileId,
    pub core: Vec<f32>,
    pub deposited: Vec<u8>,
    /// The pit depth of the core (GEN-120, codes of [`fill_code`]): how far each cell lies below the flood of the final surface, or of pass 1's flood where that holds a deeper basin. Empty when the result was read from a record that holds no such plane.
    pub fill: Vec<u8>,
    pub stats: TileStats,
}

/// A depth in metres as a byte (GEN-111): `round(m)`, so 0 to 255 m in steps of 1 m; deeper is stored as 255.
pub fn depth_code(m: f64) -> u8 {
    m.max(0.0).round().min(255.0) as u8
}

/// The depth in metres of a code of [`depth_code`].
pub fn depth_m(code: u8) -> f32 {
    f32::from(code)
}

/// A field evaluated on every cell of the tile's window, row-major, on `threads` threads (a value per cell: the same bits for any count).
fn sample_window(g: &TileGrid, id: TileId, field: &(dyn Fn(Vec3) -> f64 + Sync), threads: usize) -> Vec<f64> {
    let par: &dyn Par = &threads;
    let (w, t, h) = (g.window(), g.tile as i64, g.halo as i64);
    let (i0, j0) = (i64::from(id.ti) * t - h, i64::from(id.tj) * t - h);
    par::map(par, w, &|j| (0..w).map(|i| field(g.direction(id.face, i0 + i as i64, j0 + j as i64))).collect::<Vec<f64>>())
        .into_iter()
        .flatten()
        .collect()
}

/// `field` evaluated every `s` cells of the window (on a lattice that reaches one node beyond the window on each side) and interpolated between the nodes with Catmull-Rom, row-major.
/// A node is the field at its cell's direction, so a window cell on a node holds the field exactly. The lattice is anchored at the window's origin; with a tile that is a multiple of
/// [`BASE_STRIDE`] every window's nodes fall on the same global cells (`TileGrid::check` refuses another tile). The interpolation error is cancelled by taking the change against this
/// window. A change of the stride moves bits: bump `PASS2_KERNEL_VERSION`.
pub fn sample_window_coarse(g: &TileGrid, id: TileId, field: &(dyn Fn(Vec3) -> f64 + Sync), threads: usize, s: usize) -> Vec<f64> {
    assert!(s > 0, "the stride is at least 1");
    let par: &dyn Par = &threads;
    let (w, t, h) = (g.window(), g.tile as i64, g.halo as i64);
    let (i0, j0) = (i64::from(id.ti) * t - h, i64::from(id.tj) * t - h);
    let n = (w - 1) / s + 4; // nodes -1 ..= (w-1)/s + 2: the last cell reads one node past the cell after its own
    let coarse: Vec<f64> = par::map(par, n, &|b| {
        (0..n).map(|a| field(g.direction(id.face, i0 + (a as i64 - 1) * s as i64, j0 + (b as i64 - 1) * s as i64))).collect::<Vec<f64>>()
    })
    .into_iter()
    .flatten()
    .collect();
    let at = |a: i64, b: i64| coarse[(b + 1) as usize * n + (a + 1) as usize];
    par::map(par, w, &|j| {
        let (cb, fy) = ((j / s) as i64, (j % s) as f64 / s as f64);
        (0..w)
            .map(|i| {
                let (ca, fx) = ((i / s) as i64, (i % s) as f64 / s as f64);
                let row = |db: i64| catmull_rom([at(ca - 1, cb + db), at(ca, cb + db), at(ca + 1, cb + db), at(ca + 2, cb + db)], fx);
                // Kept within the range of the four nodes around the point, as the pass-1 read is (map.rs): a spline overshoots beside a sharp feature, which would put a coastal plain below the sea.
                let four = [at(ca, cb), at(ca + 1, cb), at(ca, cb + 1), at(ca + 1, cb + 1)];
                let (lo, hi) = four.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, u), v| (l.min(*v), u.max(*v)));
                catmull_rom([row(-1), row(0), row(1), row(2)], fy).clamp(lo, hi)
            })
            .collect::<Vec<f64>>()
    })
    .into_iter()
    .flatten()
    .collect()
}

/// The window's grid: cell `(0, 0)` is the global cell `(i0 + offset, j0 + offset)` of the face's grid, so a cell hashes alike in every window.
fn window_grid(g: &TileGrid, id: TileId) -> Grid {
    let (t, h) = (g.tile as i64, g.halo as i64);
    let origin = |a: u32| (i64::from(a) * t - h + ID_OFFSET) as u32;
    Grid::new(g.window(), g.window(), g.dx()).with_origin((origin(id.ti), origin(id.tj)))
}

/// Whether the tile's core holds land in the base plus the face change: sampled every `stride` cells (a coarse look: an island smaller than `stride` cells can be missed).
pub fn tile_has_land(g: &TileGrid, id: TileId, surface: &(dyn Fn(Vec3) -> f64 + Sync), sea_level: f64, stride: usize) -> bool {
    let (t, stride) = (g.tile as i64, stride.max(1));
    let (i0, j0) = (i64::from(id.ti) * t, i64::from(id.tj) * t);
    (0..g.tile)
        .step_by(stride)
        .any(|j| (0..g.tile).step_by(stride).any(|i| surface(g.direction(id.face, i0 + i as i64, j0 + j as i64)) > sea_level))
}

/// [`pass2_tile_with_fill`] without pass 1's fill depth: the pits are those the tile's own window floods.
pub fn pass2_tile(
    g: &TileGrid,
    id: TileId,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    face_change: &(dyn Fn(Vec3) -> f64 + Sync),
    factor: Option<&(dyn Fn(Vec3) -> f64 + Sync)>,
    p: &Pass2Params,
    threads: usize,
) -> TileResult {
    pass2_tile_with_fill(g, id, base, face_change, None, factor, p, threads)
}

/// Pass 2 on one tile. `base` is the base field, `face_change` the face carve's change at a direction (the blended reader, GEN-100), `factor` the optional carve factor of GEN-097.
/// The window is `base + face_change`; the local carve incises it; thermal and the deposition sweep follow; the result is the core's change against `base`.
#[allow(clippy::too_many_arguments)]
pub fn pass2_tile_with_fill(
    g: &TileGrid,
    id: TileId,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    face_change: &(dyn Fn(Vec3) -> f64 + Sync),
    face_fill: Option<&(dyn Fn(Vec3) -> (f64, Vec3) + Sync)>,
    factor: Option<&(dyn Fn(Vec3) -> f64 + Sync)>,
    p: &Pass2Params,
    threads: usize,
) -> TileResult {
    g.check().expect("a usable tile geometry");
    assert!(
        (id.ti as usize) < g.tiles_per_edge() && (id.tj as usize) < g.tiles_per_edge(),
        "tile ({}, {}) is outside the face",
        id.ti,
        id.tj
    );
    let grid = window_grid(g, id);
    let base_w = sample_window_coarse(g, id, base, threads, BASE_STRIDE);
    let fc_w = sample_window(g, id, face_change, threads);
    let z0: Vec<f64> = base_w.iter().zip(&fc_w).map(|(b, c)| b + c).collect();
    // The surface the deposited depth counts from: the base plus what pass 1 raised (a raise by uplift or a filled pit is not a deposit, GEN-111).
    let reference: Vec<f64> = base_w.iter().zip(&fc_w).map(|(b, c)| b + c.max(0.0)).collect();
    drop(fc_w);
    // Pass 1's pit depth at every window cell, in metres (0 where it has no pit or none is given).
    let fill1 = face_fill.map(|f| sample_window_pairs(g, id, f, threads));
    // Pass 1's spill level at the centre of each pass-1 cell that holds a pit in this window: the carved surface there (base plus the face change) plus the cell's depth. The level is flat over
    // a basin, where the fine window surface and the cell's value are not.
    let mut levels: std::collections::HashMap<[u64; 3], f64> = std::collections::HashMap::new();
    if let Some(pairs) = &fill1 {
        for (m, c) in pairs.iter().filter(|(m, _)| *m > 0.0) {
            levels.entry([c.x.to_bits(), c.y.to_bits(), c.z.to_bits()]).or_insert_with(|| base(*c) + face_change(*c) + m);
        }
    }
    let factors = factor.map(|f| sample_window(g, id, f, threads));
    let outlet = grid.outlets(&z0, p.sea_level);
    let stream = face_stream(p.seed, id.face);
    let mut z = z0.clone();
    let local = CarveParams { max_area: p.local_area_cells * g.dx() * g.dx(), ..p.carve };
    carve(&grid, &mut z, &outlet, &local, factors.as_deref(), stream);
    drop(factors);
    let thermal_report = thermal(&grid, &mut z, &p.thermal, threads);
    let z_thermal = z.clone();
    let dep = deposit_on_own_route(
        &grid,
        &mut z,
        &z0,
        p.carve.epsilon,
        stream,
        &DepositParams { kc: Kc::Uniform(p.kc), zeta: p.zeta, sea_level: p.sea_level },
    );
    // What is left of the one-cell pits (thermal's and the sweep's) goes to the neighbours, volume kept.
    spread_pits(&grid, &mut z, p.sea_level, p.pit_min_m, p.pit_passes);
    // The pits of the final surface (GEN-120): the depth below the window's flood, and where pass 1 floods a basin the window cuts, the depth below pass 1's spill level (its
    // surface at the start of this pass plus its fill depth), whichever is deeper. An outlet and a pit under `FILL_MIN_M` count 0.
    let depth_w = fill_depth(&grid, &z, &outlet, p.carve.epsilon, FILL_MIN_M);
    let (w, t, h) = (g.window(), g.tile, g.halo);
    let mut core = Vec::with_capacity(t * t);
    let mut deposited = Vec::with_capacity(t * t);
    let mut fill = Vec::with_capacity(t * t);
    let mut land = 0;
    for j in 0..t {
        for i in 0..t {
            let q = (j + h) * w + i + h;
            let own = f64::from(depth_w[q]);
            let from_pass1 = fill1.as_ref().map_or(0.0, |f| {
                let (m, c) = f[q];
                if m > 0.0 {
                    (levels[&[c.x.to_bits(), c.y.to_bits(), c.z.to_bits()]] - z[q]).max(0.0)
                } else {
                    0.0
                }
            });
            fill.push(fill_code(if z0[q] > p.sea_level { own.max(from_pass1) } else { 0.0 }));
            core.push((z[q] - base_w[q]) as f32);
            // The deposition sweep's rise plus thermal's gain over the reference. What was removed is derived by the reader: max(0, deposited - change), exact where pass 1 did not raise the cell.
            let gained = (z[q] - z_thermal[q]).max(0.0) + (z_thermal[q] - reference[q]).max(0.0);
            deposited.push(depth_code(gained));
            land += usize::from(z0[q] > p.sea_level);
        }
    }
    let core_clamped = core.iter().filter(|v| v.abs() > f32::from(i16::MAX) * QUANT_STEP_M).count();
    TileResult {
        id,
        core,
        deposited,
        fill,
        stats: TileStats {
            land_cells: land,
            thermal_passes: thermal_report.passes_run,
            thermal_moving_at_cap: thermal_report.moved_last,
            eroded_m3: dep.eroded,
            deposited_m3: dep.deposited,
            exported_m3: dep.exported,
            clamped_cells: core_clamped,
        },
    }
}

/// A change in metres as `i16` counts of [`QUANT_STEP_M`], rounded to the nearest and clamped to the range.
pub fn quantise(change: &[f32]) -> Vec<i16> {
    change.iter().map(|&v| (v / QUANT_STEP_M).round().clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16).collect()
}

/// The change in metres of quantised counts.
pub fn dequantise(counts: &[i16]) -> Vec<f32> {
    counts.iter().map(|&c| f32::from(c) * QUANT_STEP_M).collect()
}

impl TileResult {
    /// The core with the pit depth added back: the filled look the runtime shows while lake water and a pit-tolerance rule do not exist (GEN-120). A result without a fill plane is returned as it is.
    pub fn with_fill_added(mut self) -> TileResult {
        if self.fill.len() == self.core.len() {
            for (c, f) in self.core.iter_mut().zip(&self.fill) {
                *c += fill_m(*f) as f32;
            }
        }
        self
    }
}

/// A pair of values per window cell, row-major, on `threads` threads (as [`sample_window`]).
fn sample_window_pairs(g: &TileGrid, id: TileId, field: &(dyn Fn(Vec3) -> (f64, Vec3) + Sync), threads: usize) -> Vec<(f64, Vec3)> {
    let par: &dyn Par = &threads;
    let (w, t, h) = (g.window(), g.tile as i64, g.halo as i64);
    let (i0, j0) = (i64::from(id.ti) * t - h, i64::from(id.tj) * t - h);
    par::map(par, w, &|j| (0..w).map(|i| field(g.direction(id.face, i0 + i as i64, j0 + j as i64))).collect::<Vec<_>>())
        .into_iter()
        .flatten()
        .collect()
}
