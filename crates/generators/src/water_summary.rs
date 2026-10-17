//! The coarse level of the water field W: a summary of the bake, written once (ADR 0057, GEN-024).
//!
//! A tile at or above the level of the bake's blocks spans many of them. Asking W at its sample directions (ADR 0053) loads up to 25
//! blocks of the stored bake; the summary answers the same question from a small table instead. The table has one cell per
//! `d x d` cells of the bake (`d = min(B, 64)`, `B` the cells per block edge), so a cell lies inside one block, and per cell it keeps
//! what the questions of W need, as a pure function of the bake and its ocean channel:
//!
//! * **wet**: some lattice cell of the summary cell has an ocean vertex among its corners and a corner whose continentalness is at or
//!   below `SEA_LEVEL_C`, which is necessary for any point of that lattice cell to be sea (the interpolation lies between the corners);
//! * **ocean**: some vertex of the summary cell is ocean (the reach test of the coastal band, ADR 0056);
//! * **band elevation**: the lowest corner elevation among the lattice cells with a land corner (a necessary condition for a band
//!   point, which needs land and an elevation at most `B(W)`). The sea level is not an input, so one summary serves any level.
//!
//! A coarse tile's answer ([`WaterSummary::tile_water`]) is the union over the cells its area touches, so it is a superset of what
//! the tile's sample directions would find, and a parent is wet wherever its children are (ATMO-019). Tiles below the block level are
//! not answered here: they read one block and keep the exact answer of ADR 0053.
//!
//! Class (ADR 0035): the table is a **field** (CON-07) with no GPU twin: bit-exact for every thread count and operating system
//! (CON-14); the answer is made of comparisons and one `libm` cosine.

use crate::global::{GlobalSample, SEA_LEVEL_F32};
use crate::ocean::OceanBits;
use crate::water::{coast_band_elevation_m, WaterBodyId, WaterSample, COAST_REACH_M};
use planet_core::cube::{face_to_direction, FaceMapping};
use planet_core::{Face, TileId, Vec3};
use planet_world_def::SeaParams;
use std::sync::OnceLock;

/// Names this implementation in the summary's cache key (CON-07); bump it with any change of an answer of this module.
pub const SUMMARY_IMPLEMENTATION_ID: &str = "cpu-water-summary-v1";

/// Cells of the bake per summary cell edge, at most (a smaller block has fewer).
pub const SUMMARY_BAKE_CELLS: u32 = 64;

/// The summary cell has a lattice cell that may hold sea.
pub const FLAG_WET: u8 = 1;
/// The summary cell has an ocean vertex.
pub const FLAG_OCEAN: u8 = 2;
const FLAGS_ALL: u8 = FLAG_WET | FLAG_OCEAN;

/// How much of a tile's edge the reach of a coarse tile counts from its centre: more than the 0.71 of the exact test (half the
/// diagonal), so that the cube's anisotropy near a face corner cannot make a parent drier than a child.
pub const COARSE_REACH_EDGE_FRACTION: f64 = 0.8;

/// One cell of the summary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SummaryCell {
    pub flags: u8,
    /// The lowest corner elevation (m) among the lattice cells that have a land corner; `INFINITY` when none has.
    pub band_elevation_m: f32,
}

impl SummaryCell {
    pub const EMPTY: SummaryCell = SummaryCell { flags: 0, band_elevation_m: f32::INFINITY };
}

/// Why a summary could not be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SummaryError {
    /// The resolution or the block size makes no grid of summary cells.
    Shape { res: u32, block_cells: u32 },
    /// A block's samples, ocean bits or cells do not fit its size.
    Block(String),
}

impl std::fmt::Display for SummaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SummaryError::Shape { res, block_cells } => {
                write!(f, "a bake of {res} cells in blocks of {block_cells} makes no water summary (powers of two, block no larger than the bake)")
            }
            SummaryError::Block(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for SummaryError {}

/// Cells of the bake per summary cell edge for blocks of `block_cells` cells.
pub fn summary_edge(block_cells: u32) -> u32 {
    block_cells.min(SUMMARY_BAKE_CELLS)
}

/// The summary cells of one block: `(B / d)^2` of them, row-major (`x` fastest), from the block's samples and ocean bits. A pure
/// function of the two (thread count and order of blocks change nothing).
pub fn summarize_block(block_cells: u32, samples: &[GlobalSample], ocean: &OceanBits) -> Result<Vec<SummaryCell>, SummaryError> {
    let b = block_cells as usize;
    let n = b + 1;
    if !block_cells.is_power_of_two() || samples.len() != n * n || ocean.cells != block_cells {
        return Err(SummaryError::Block(format!(
            "a block of {block_cells} cells needs {} samples and {block_cells}-cell ocean bits, got {} samples and {}-cell bits",
            n * n,
            samples.len(),
            ocean.cells
        )));
    }
    let d = summary_edge(block_cells) as usize;
    let gb = b / d;
    let mut out = vec![SummaryCell::EMPTY; gb * gb];
    let sea = SEA_LEVEL_F32;
    for l in 0..b {
        for k in 0..b {
            let idx = [l * n + k, l * n + k + 1, (l + 1) * n + k, (l + 1) * n + k + 1];
            let cell = &mut out[(l / d) * gb + k / d];
            let corner = |i: usize| (i % n, i / n);
            let mut ocean_any = false;
            let (mut sea_corner, mut land_corner) = (false, false);
            let mut low = f32::INFINITY;
            for &i in &idx {
                let c = samples[i].continent;
                // A continentalness that is not a number counts as not above the sea (the producer's test is `c > L` negated).
                sea_corner |= c.partial_cmp(&sea) != Some(std::cmp::Ordering::Greater);
                land_corner |= c > sea;
                low = low.min(samples[i].elevation_m);
                let (ck, cl) = corner(i);
                ocean_any |= ocean.get(ck as u32, cl as u32);
            }
            if ocean_any {
                cell.flags |= FLAG_OCEAN;
                if sea_corner {
                    cell.flags |= FLAG_WET;
                }
            }
            if land_corner {
                cell.band_elevation_m = cell.band_elevation_m.min(low);
            }
        }
    }
    Ok(out)
}

/// The summary of a whole bake: `6 g^2` cells, `g = res / d`.
#[derive(Debug)]
pub struct WaterSummary {
    res: u32,
    block_cells: u32,
    d: u32,
    g: u32,
    cells: Vec<SummaryCell>,
    /// The directions of the cells that have an ocean vertex, made on the first reach test (they depend on the face mapping, whose id
    /// is kept to refuse another).
    ocean_dirs: OnceLock<(&'static str, Vec<Vec3>)>,
}

impl WaterSummary {
    /// An empty summary for a bake of `res` cells per face edge in blocks of `block_cells`.
    pub fn new(res: u32, block_cells: u32) -> Result<WaterSummary, SummaryError> {
        let shape = SummaryError::Shape { res, block_cells };
        if res == 0 || !res.is_power_of_two() || block_cells == 0 || !block_cells.is_power_of_two() || block_cells > res {
            return Err(shape);
        }
        let d = summary_edge(block_cells);
        let g = res / d;
        Ok(WaterSummary {
            res,
            block_cells,
            d,
            g,
            cells: vec![SummaryCell::EMPTY; 6 * g as usize * g as usize],
            ocean_dirs: OnceLock::new(),
        })
    }

    /// A summary from its cells (face-major, row-major): the decoder's way in.
    pub fn from_cells(res: u32, block_cells: u32, cells: Vec<SummaryCell>) -> Result<WaterSummary, SummaryError> {
        let mut s = WaterSummary::new(res, block_cells)?;
        if cells.len() != s.cells.len() {
            return Err(SummaryError::Block(format!(
                "{} summary cells, a bake of {res} cells in blocks of {block_cells} has {}",
                cells.len(),
                s.cells.len()
            )));
        }
        s.cells = cells;
        Ok(s)
    }

    pub fn res(&self) -> u32 {
        self.res
    }

    pub fn block_cells(&self) -> u32 {
        self.block_cells
    }

    /// Summary cells per face edge.
    pub fn edge_cells(&self) -> u32 {
        self.g
    }

    /// Cells of the bake per summary cell edge.
    pub fn bake_cells_per_cell(&self) -> u32 {
        self.d
    }

    pub fn cells(&self) -> &[SummaryCell] {
        &self.cells
    }

    /// The level of the bake's blocks: tiles at this level or above are answered by the summary.
    pub fn block_level(&self) -> u8 {
        (self.res / self.block_cells).trailing_zeros() as u8
    }

    /// Put the cells of block `tile` (from [`summarize_block`]) in place.
    pub fn set_block(&mut self, tile: TileId, block: &[SummaryCell]) -> Result<(), SummaryError> {
        let gb = (self.block_cells / self.d) as usize;
        if tile.level() != self.block_level() || block.len() != gb * gb {
            return Err(SummaryError::Block(format!(
                "block {:016x} at level {} with {} cells does not fit a summary of blocks at level {} with {} cells",
                tile.raw(),
                tile.level(),
                block.len(),
                self.block_level(),
                gb * gb
            )));
        }
        let g = self.g as usize;
        let (x0, y0) = (tile.x() as usize * gb, tile.y() as usize * gb);
        for cy in 0..gb {
            for cx in 0..gb {
                self.cells[(tile.face().0 as usize * g + y0 + cy) * g + x0 + cx] = block[cy * gb + cx];
            }
        }
        self.ocean_dirs = OnceLock::new();
        Ok(())
    }

    fn cell(&self, face: Face, cx: u32, cy: u32) -> SummaryCell {
        self.cells[(face.0 as usize * self.g as usize + cy as usize) * self.g as usize + cx as usize]
    }

    /// The water of `tile` if the summary answers for it (`None`: the tile is below the block level, ask the field), else the sea or
    /// nothing. See the module documentation for the rule; `radius_m` is the body's radius, `sea` its sea.
    pub fn tile_water(&self, map: &dyn FaceMapping, tile: TileId, radius_m: f64, sea: &SeaParams) -> Option<Option<WaterSample>> {
        if tile.level() > self.block_level() {
            return None;
        }
        let w = WaterSample { level_m: sea.level_m, body: WaterBodyId::SEA, kind: sea.liquid };
        // The tile's lattice rectangle (a tile is a rectangle of whole cells at or above the block level) and the summary cells it touches.
        let span = self.res >> tile.level();
        let (x0, y0) = (tile.x() * span, tile.y() * span);
        let (cx0, cx1) = (x0 / self.d, ((x0 + span) / self.d).min(self.g - 1));
        let (cy0, cy1) = (y0 / self.d, ((y0 + span) / self.d).min(self.g - 1));
        let band_limit = coast_band_elevation_m(sea.level_m);
        let mut band = false;
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                let c = self.cell(tile.face(), cx, cy);
                if c.flags & FLAG_WET != 0 {
                    return Some(Some(w));
                }
                band |= f64::from(c.band_elevation_m) <= band_limit;
            }
        }
        if !band {
            return Some(None);
        }
        // The coastal band (ADR 0056): the tile carries the sea when an ocean vertex lies within the reach of the tile. A summary cell
        // that has one counts when its centre is within the reach plus the cell's own size.
        let edge_rad = core::f64::consts::FRAC_PI_2 / f64::from(1u32 << tile.level().min(30));
        let reach_rad = (COAST_REACH_M / radius_m) + COARSE_REACH_EDGE_FRACTION * edge_rad;
        let margin_rad = core::f64::consts::FRAC_PI_2 / f64::from(self.g);
        let limit = reach_rad + margin_rad;
        if limit >= core::f64::consts::PI {
            return Some(self.with_ocean_directions(map, |v| !v.is_empty()).then_some(w));
        }
        let cos_limit = libm::cos(limit);
        let centre = tile.center_direction(map);
        let near = self.with_ocean_directions(map, |v| v.iter().any(|d| d.dot(centre) >= cos_limit));
        Some(near.then_some(w))
    }

    fn with_ocean_directions<R>(&self, map: &dyn FaceMapping, f: impl FnOnce(&[Vec3]) -> R) -> R {
        // Made once per summary (the face mapping of a body does not change); another mapping gets its own list.
        let make = || {
            let mut v = Vec::new();
            let g = self.g as usize;
            for face in 0..6u8 {
                for cy in 0..g {
                    for cx in 0..g {
                        if self.cells[(face as usize * g + cy) * g + cx].flags & FLAG_OCEAN != 0 {
                            let c = |i: usize| 2.0 * ((i as f64 + 0.5) * f64::from(self.d)) / f64::from(self.res) - 1.0;
                            v.push(face_to_direction(map, Face(face), c(cx), c(cy)));
                        }
                    }
                }
            }
            v
        };
        let cached = self.ocean_dirs.get_or_init(|| (map.id(), make()));
        if cached.0 == map.id() {
            f(&cached.1)
        } else {
            f(&make())
        }
    }

    /// Whether every flag bit is one this version knows (a decoder refuses a summary with others).
    pub fn flags_known(&self) -> bool {
        self.cells.iter().all(|c| c.flags & !FLAGS_ALL == 0)
    }
}

/// The summary of an in-memory bake and its ocean channel, in blocks of `block_cells` (the summary a stored bake of that layout
/// has): the reference of the tests and what a run over an in-memory bake would hold.
pub fn summary_of_grid(
    grid: &crate::global::GlobalGrid,
    ocean: &crate::ocean::OceanGrid,
    block_cells: u32,
    threads: usize,
) -> Result<WaterSummary, SummaryError> {
    let tiles = crate::ocean::block_tiles(grid.res, block_cells).map_err(|e| SummaryError::Block(format!("{e:?}")))?;
    let mut summary = WaterSummary::new(grid.res, block_cells)?;
    let blocks = crate::ocean::parallel_map(&tiles, threads, &|t| {
        let samples = crate::ocean::grid_block(grid, *t, block_cells);
        let mut bits = OceanBits::zeros(block_cells);
        for l in 0..=block_cells {
            for k in 0..=block_cells {
                if ocean.get(t.face(), t.x() * block_cells + k, t.y() * block_cells + l) {
                    bits.set(k, l);
                }
            }
        }
        summarize_block(block_cells, &samples, &bits)
    });
    for (t, b) in tiles.iter().zip(blocks) {
        summary.set_block(*t, &b?)?;
    }
    Ok(summary)
}
