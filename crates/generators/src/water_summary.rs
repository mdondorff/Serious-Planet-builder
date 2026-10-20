//! The coarse level of the water field W: a summary of the bake, written once (ADR 0058, GEN-024).
//!
//! A tile at or above the level of the bake's blocks spans many of them. Asking W at its sample directions (ADR 0053) loads up to 25
//! blocks of the stored bake; the summary answers the same question from a small table instead. The table has one cell per
//! `d x d` cells of the bake (`d = min(B, 64)`, `B` the cells per block edge), so a cell lies inside one block, and per cell it keeps
//! what the questions of W need, as a pure function of the bake and its ocean channel:
//!
//! * **sea**: some lattice cell of the summary cell has a corner whose continentalness is at or below `SEA_LEVEL_C`, which is necessary
//!   for any point of that lattice cell to be sea (the interpolation lies between the corners);
//! * **floor**: the lowest wet floor (ADR 0071, `crate::wet_floor`) over the corners of the lattice cells of the summary cell: 0 when an
//!   ocean vertex is among them, `0xFFFF` when the sea reaches none;
//! * **band elevation**: the lowest corner elevation among the lattice cells with a land corner (a necessary condition for a band
//!   point, which needs land and an elevation at most `B(W)`). The sea level is not an input, so one summary serves any level.
//!
//! A coarse tile's answer ([`WaterSummary::tile_water`]) is the union over the cells its area touches: a cell is wet when its lowest
//! floor is at most the code of the conduction bound `C(W)` and it may hold sea (a sea corner) or band (a band elevation at most
//! `B(W)`). So it is a superset of what the tile's sample directions would find, and a parent is wet wherever its children are (ATMO-019). Tiles below the block level are
//! not answered here: they read one block and keep the exact answer of ADR 0053.
//!
//! Class (ADR 0035): the table is a **field** (CON-07) with no GPU twin: bit-exact for every thread count and operating system
//! (CON-14); the answer is made of comparisons and one `libm` cosine.

use crate::global::{GlobalSample, SEA_LEVEL_F32};
use crate::water::{coast_band_elevation_m, WaterBodyId, WaterSample};
use crate::wet_floor::{bound_code, INF};
use planet_core::cube::FaceMapping;
use planet_core::{Face, TileId};
use planet_world_def::SeaParams;

/// Names this implementation in the summary's cache key (CON-07); bump it with any change of an answer of this module.
pub const SUMMARY_IMPLEMENTATION_ID: &str = "cpu-water-summary-v2";

/// Cells of the bake per summary cell edge, at most (a smaller block has fewer).
pub const SUMMARY_BAKE_CELLS: u32 = 64;

/// The summary cell has a lattice cell with a corner at or below the sea class: some point of it may be sea.
pub const FLAG_SEA: u8 = 1;
const FLAGS_ALL: u8 = FLAG_SEA;

/// The most cells a summary may have (`6 * 1024^2`): a header that implies more is refused before anything is allocated.
pub const MAX_SUMMARY_CELLS: u64 = 6 << 20;

/// The number of cells of the summary of a bake of `res` cells in blocks of `block_cells`, if they make one.
pub fn summary_cell_count(res: u32, block_cells: u32) -> Option<u64> {
    if res == 0 || !res.is_power_of_two() || block_cells == 0 || !block_cells.is_power_of_two() || block_cells > res {
        return None;
    }
    let g = u64::from(res / summary_edge(block_cells));
    Some(6 * g * g).filter(|n| *n <= MAX_SUMMARY_CELLS)
}

/// One cell of the summary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SummaryCell {
    pub flags: u8,
    /// The lowest corner elevation (m) among the lattice cells that have a land corner; `INFINITY` when none has.
    pub band_elevation_m: f32,
    /// The lowest wet floor among the corners of the cell's lattice cells (`0xFFFF` when none is reached).
    pub floor: u16,
}

impl SummaryCell {
    pub const EMPTY: SummaryCell = SummaryCell { flags: 0, band_elevation_m: f32::INFINITY, floor: INF };
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

/// The summary cells of one block: `(B / d)^2` of them, row-major (`x` fastest), from the block's samples and its wet floors
/// (`(B + 1)^2` codes, `k` fastest). A pure function of the two (thread count and order of blocks change nothing).
pub fn summarize_block(block_cells: u32, samples: &[GlobalSample], floors: &[u16]) -> Result<Vec<SummaryCell>, SummaryError> {
    let b = block_cells as usize;
    let n = b + 1;
    if !block_cells.is_power_of_two() || samples.len() != n * n || floors.len() != n * n {
        return Err(SummaryError::Block(format!(
            "a block of {block_cells} cells needs {} samples and as many floors, got {} samples and {} floors",
            n * n,
            samples.len(),
            floors.len()
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
            let (mut sea_corner, mut land_corner) = (false, false);
            let mut low = f32::INFINITY;
            let mut floor = INF;
            for &i in &idx {
                let c = samples[i].continent;
                // A continentalness that is not a number counts as not above the sea (the producer's test is `c > L` negated).
                sea_corner |= c.partial_cmp(&sea) != Some(std::cmp::Ordering::Greater);
                land_corner |= c > sea;
                // A plain comparison, not `f32::min` (which of two zeros of different sign it returns is unspecified, CON-14).
                if samples[i].elevation_m < low {
                    low = samples[i].elevation_m;
                }
                floor = floor.min(floors[i]);
            }
            if sea_corner {
                cell.flags |= FLAG_SEA;
            }
            cell.floor = cell.floor.min(floor);
            if land_corner && low < cell.band_elevation_m {
                cell.band_elevation_m = low;
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
}

impl WaterSummary {
    /// An empty summary for a bake of `res` cells per face edge in blocks of `block_cells`.
    pub fn new(res: u32, block_cells: u32) -> Result<WaterSummary, SummaryError> {
        let n = summary_cell_count(res, block_cells).ok_or(SummaryError::Shape { res, block_cells })?;
        WaterSummary::from_cells(res, block_cells, vec![SummaryCell::EMPTY; n as usize])
    }

    /// A summary from its cells (face-major, row-major): the decoder's way in.
    pub fn from_cells(res: u32, block_cells: u32, cells: Vec<SummaryCell>) -> Result<WaterSummary, SummaryError> {
        let n = summary_cell_count(res, block_cells).ok_or(SummaryError::Shape { res, block_cells })?;
        if cells.len() as u64 != n {
            return Err(SummaryError::Block(format!(
                "{} summary cells, a bake of {res} cells in blocks of {block_cells} has {n}",
                cells.len()
            )));
        }
        let d = summary_edge(block_cells);
        Ok(WaterSummary { res, block_cells, d, g: res / d, cells })
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
        Ok(())
    }

    fn cell(&self, face: Face, cx: u32, cy: u32) -> SummaryCell {
        self.cells[(face.0 as usize * self.g as usize + cy as usize) * self.g as usize + cx as usize]
    }

    /// The water of `tile` if the summary answers for it (`None`: the tile is below the block level, ask the field), else the sea or
    /// nothing. See the module documentation for the rule; `sea` is the body's sea. Since ADR 0071 the face mapping and the radius are no
    /// inputs of the rule (the tile's cells are found on the lattice).
    pub fn tile_water(&self, _map: &dyn FaceMapping, tile: TileId, _radius_m: f64, sea: &SeaParams) -> Option<Option<WaterSample>> {
        if tile.level() > self.block_level() {
            return None;
        }
        let w = WaterSample { level_m: sea.level_m, body: WaterBodyId::SEA, kind: sea.liquid };
        // The tile's lattice rectangle (a tile is a rectangle of whole cells at or above the block level) and the summary cells it touches.
        let span = self.res >> tile.level();
        let (x0, y0) = (tile.x() * span, tile.y() * span);
        // One lattice cell more on the low side: a sample on the tile's low edge can land a rounding error below it, and then the vertices
        // of the cell before the edge carry a tiny weight (GEN-023 counts every vertex with a non-zero weight).
        let (cx0, cx1) = (x0.saturating_sub(1) / self.d, ((x0 + span) / self.d).min(self.g - 1));
        let (cy0, cy1) = (y0.saturating_sub(1) / self.d, ((y0 + span) / self.d).min(self.g - 1));
        let band_limit = coast_band_elevation_m(sea.level_m);
        let bound = bound_code(band_limit);
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                let c = self.cell(tile.face(), cx, cy);
                // A cell is wet when the sea reaches one of its corners through ground at most the bound high and it may hold sea or band.
                if c.floor <= bound && (c.flags & FLAG_SEA != 0 || f64::from(c.band_elevation_m) <= band_limit) {
                    return Some(Some(w));
                }
            }
        }
        Some(None)
    }

    /// Whether every flag bit is one this version knows (a decoder refuses a summary with others).
    pub fn flags_known(&self) -> bool {
        self.cells.iter().all(|c| c.flags & !FLAGS_ALL == 0)
    }
}

/// The summary of an in-memory bake and its ocean channel, in blocks of `block_cells` (the summary a stored bake of that layout
/// has): the reference of the tests. The floors are computed here; a caller that holds them uses [`summary_of_floors`].
pub fn summary_of_grid(
    grid: &crate::global::GlobalGrid,
    ocean: &crate::ocean::OceanGrid,
    block_cells: u32,
    threads: usize,
) -> Result<WaterSummary, SummaryError> {
    let floors = crate::wet_floor::tiled_floors(grid, ocean, block_cells, threads).map_err(|e| SummaryError::Block(format!("{e:?}")))?;
    summary_of_floors(grid, &floors, block_cells, threads)
}

/// The summary of an in-memory bake from its wet floors, in blocks of `block_cells`: what a run over an in-memory bake holds, made
/// from the same [`FloorGrid`](crate::wet_floor::FloorGrid) the producer reads, so the two cannot disagree.
pub fn summary_of_floors(
    grid: &crate::global::GlobalGrid,
    floors: &crate::wet_floor::FloorGrid,
    block_cells: u32,
    threads: usize,
) -> Result<WaterSummary, SummaryError> {
    let tiles = crate::ocean::block_tiles(grid.res, block_cells).map_err(|e| SummaryError::Block(format!("{e:?}")))?;
    let mut summary = WaterSummary::new(grid.res, block_cells)?;
    let blocks = crate::ocean::parallel_map(&tiles, threads, &|t| {
        let samples = crate::ocean::grid_block(grid, *t, block_cells);
        let n = block_cells + 1;
        let codes: Vec<u16> = (0..n * n).map(|v| floors.at(t.face(), t.x() * block_cells + v % n, t.y() * block_cells + v / n)).collect();
        summarize_block(block_cells, &samples, &codes)
    });
    for (t, b) in tiles.iter().zip(blocks) {
        summary.set_block(*t, &b?)?;
    }
    Ok(summary)
}
