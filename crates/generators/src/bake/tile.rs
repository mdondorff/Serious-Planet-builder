//! Pass 2 of the bake on one tile of the layer grid (GEN-101; m9-bake PR 5): the base plus the face carve's change, a local carve that only incises, thermal on active cells and one
//! deposition sweep, and the change of the tile's core against the base field.
//!
//! The layer grid has `cells` cells per face edge (98,304 for 101.8 m at Earth's size; fewer for a coarse planet pass), cut into square tiles of `tile` core cells (3,072) with a halo of
//! `halo` cells (256) around each. A tile's window is `tile + 2 halo` cells a side; a window may reach past the face's edge, where the plane of the face continues over the neighbouring
//! ground (the base and the face change are functions of the direction). The routing's jitter and the thermal's pair hash follow the GLOBAL cell of the face's grid, so two windows that
//! hold a cell treat it alike. A window's border still matters: the local carve caps the catchment area, but the deposition sweep routes the whole window, so its loads and capacities
//! depend on where the border falls; the seam this leaves is measured (TEST-084), not assumed away.

use super::carve::{carve, CarveParams};
use super::deposit::{deposit_on_own_route, DepositParams, Kc};
use super::face::face_stream;
use super::thermal::{thermal, ThermalParams};
use super::Grid;
use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::par::{self, Par};
use planet_core::{Face, Vec3};

/// The quantisation step of a stored change in metres (a change is stored as an `i16` count of steps: `+-8 km` at 0.25 m).
pub const QUANT_STEP_M: f32 = 0.25;
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
}

impl Pass2Params {
    /// GEN-101's start values: the local carve of 10 steps without uplift, breach or fill and with the area capped at 1 km^2; thermal at the default; `kc` 1e5 and `zeta` 0.5.
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
            thermal: ThermalParams { sea_level, ..ThermalParams::default() },
            kc: 1.0e5,
            zeta: 0.5,
            local_area_cells: 96.0,
            sea_level,
            seed,
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

/// A tile's result: the core's change against the base field (final minus base, f32, row-major `tile x tile`; it contains the face carve's change), the depth channels (GEN-111) and
/// the statistics. `eroded` and `deposited` are codes of [`depth_code`] per core cell, or empty when the result was read from a record that holds no channels.
#[derive(Clone, Debug, PartialEq)]
pub struct TileResult {
    pub id: TileId,
    pub core: Vec<f32>,
    pub eroded: Vec<u8>,
    pub deposited: Vec<u8>,
    pub stats: TileStats,
}

/// A depth in metres as a byte (GEN-111): `round(16 sqrt(m))`, so 0 to 254 m with a step of 0.06 m at 1 m and 0.5 m at 16 m; deeper is stored as 255.
pub fn depth_code(m: f64) -> u8 {
    (DEPTH_CODE_SCALE * m.max(0.0).sqrt()).round().min(255.0) as u8
}

/// The depth in metres of a code of [`depth_code`].
pub fn depth_m(code: u8) -> f32 {
    let c = f32::from(code) / DEPTH_CODE_SCALE as f32;
    c * c
}

const DEPTH_CODE_SCALE: f64 = 16.0;

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

/// Pass 2 on one tile. `base` is the base field, `face_change` the face carve's change at a direction (the blended reader, GEN-100), `factor` the optional carve factor of GEN-097.
/// The window is `base + face_change`; the local carve incises it; thermal and the deposition sweep follow; the result is the core's change against `base`.
pub fn pass2_tile(
    g: &TileGrid,
    id: TileId,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    face_change: &(dyn Fn(Vec3) -> f64 + Sync),
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
    let base_w = sample_window(g, id, base, threads);
    let fc_w = sample_window(g, id, face_change, threads);
    let z0: Vec<f64> = base_w.iter().zip(&fc_w).map(|(b, c)| b + c).collect();
    drop(fc_w);
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
    let (w, t, h) = (g.window(), g.tile, g.halo);
    let mut core = Vec::with_capacity(t * t);
    let (mut eroded, mut deposited) = (Vec::with_capacity(t * t), Vec::with_capacity(t * t));
    let mut land = 0;
    for j in 0..t {
        for i in 0..t {
            let q = (j + h) * w + i + h;
            core.push((z[q] - base_w[q]) as f32);
            // The net change from the carve's start is deposited - eroded: the thermal sweep's net loss and the incision count as eroded, the deposition sweep's rise and the thermal gain as deposited.
            let (lost, gained) = ((z0[q] - z_thermal[q]).max(0.0), (z[q] - z_thermal[q]).max(0.0) + (z_thermal[q] - z0[q]).max(0.0));
            eroded.push(depth_code(lost));
            deposited.push(depth_code(gained));
            land += usize::from(z0[q] > p.sea_level);
        }
    }
    let core_clamped = core.iter().filter(|v| v.abs() > f32::from(i16::MAX) * QUANT_STEP_M).count();
    TileResult {
        id,
        core,
        eroded,
        deposited,
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
