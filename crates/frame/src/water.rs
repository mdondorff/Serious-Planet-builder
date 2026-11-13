//! The water part of the frame plan (ADR 0041 points 4 to 6 and 8, ADR 0051 point 6, ADR 0053): which terrain nodes carry water, at
//! what level and of which liquid, the liquids' appearance, and the waves' reduced phases. GPU-free (CON-02, CON-03): the water pass
//! executes this and decides nothing.
//!
//! **Which nodes carry water (ADR 0051 point 6, option 1).** Nothing is stored per tile: at plan time the node's tile is asked for
//! its water by a query of the field W at the `(TILE_SAMPLES + 1)^2` sample directions of the tile (the corners, the edges and the
//! middle), and the answer is memoised per tile in a [`WaterTileCache`] (a tile's answer is a pure function of the tile and the
//! field, so it is computed once, however many frames the tile stays in the plan). A tile carries water when any sample reports
//! water; the water of the tile is its most frequent sample's body (ties: the lower body id), so a tile holds one level and one
//! liquid. The pass then draws the tile's grid at that level wherever the level is above the terrain (`W - h_morph > 0`), which is
//! what makes the coast a function of the terrain and the level, on every LOD (the terrain's own morph height is the surface the
//! water is measured from). Known limit of option 1: a dry basin below the level inside a tile that also holds sea is drawn
//! flooded, and a lake smaller than a tile's sample spacing can be missed at the tile's coarse levels; the per-vertex mask
//! (ADR 0051 point 6, option 3) is the fix if either shows.
//!
//! **Coarse tiles (ADR 0058, GEN-024).** A tile whose area is at least one block of the stored bake is not asked point by point (that
//! reads up to 25 blocks): the source answers it from the coarse level of W, a summary the bake wrote once, as the union over the cells
//! the tile touches (a superset of the point query's answer, and a parent is wet wherever its children are). The query is
//! [`WaterSource::coarse_tile_water`]; a source without a summary answers `None` and the tile is asked point by point.
//!
//! **The cut (ATMO-020).** The nodes handed to the water pass are those that can show water from the camera: a node whose tile lies
//! beyond the horizon of its own level, or whose terrain stays above its level plus the waves' rise, is left out. The cut is made
//! where the plan assembles the node list; the tile's answer above, the [`WaterBook`] and every key are untouched.

use crate::lod::{tile_extent, LodParams, TileExtent};
use crate::look::water::{tile_phases, LiquidAppearance, WaterLook, MAX_LIQUIDS, WAVES};
use crate::terrain::TerrainPlan;
use planet_core::cube::FaceMapping;
use planet_core::{TileId, Vec3};
use planet_generators::global::ReadError;
use planet_generators::height::HeightField;
use planet_generators::lake_water::{lake_body, lake_of_body, LakeMaskSource};
use planet_generators::water::{WaterBodyId, WaterField, WaterSample};
use planet_world_def::{LiquidKindId, LiquidTable};
use std::collections::HashMap;

/// Samples per tile edge of the selection query: the tile's `(TILE_SAMPLES + 1)^2` directions (a power of two, as
/// `TileId::sample_direction` takes).
pub const TILE_SAMPLES: u32 = 4;

/// Which image the water pass writes (debug views, CON-18).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WaterView {
    /// The lit surface.
    #[default]
    Lit,
    /// The class of every water pixel by its vertical depth (`WaterClass`): discrete bands, the shoreline's debug view.
    Class,
    /// The wave displacement in eight levels.
    Waves,
    /// The shading normal.
    Normals,
}

impl WaterView {
    pub const ALL: [WaterView; 4] = [WaterView::Lit, WaterView::Class, WaterView::Waves, WaterView::Normals];

    pub fn name(self) -> &'static str {
        match self {
            WaterView::Lit => "lit",
            WaterView::Class => "class",
            WaterView::Waves => "waves",
            WaterView::Normals => "normals",
        }
    }

    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|v| *v == self).expect("known view") as u32
    }

    pub fn parse(s: &str) -> Option<WaterView> {
        Self::ALL.into_iter().find(|v| v.name() == s)
    }
}

/// One node of the plan that carries water.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterNode {
    /// Index into the terrain plan's `nodes` (the instance the water draws with).
    pub node: u32,
    /// The water level W (m above the reference surface), rounded once to f32 (a height, never a radius, CON-11).
    pub level_m: f32,
    /// Index into [`WaterPlan::liquids`].
    pub liquid: u8,
    /// The waves' reduced phases at the node's origin at the plan's time (`look::water::tile_phases`).
    pub phases: [f32; WAVES],
    /// The sample spacing of the node's tile (m, `LodParams::sample_spacing`): the shader multiplies it by `1 + morph` for the
    /// vertex's own spacing, which decides which waves the grid can hold.
    pub spacing_m: f32,
    /// The node's lake mask (REND-065): an index into [`WaterPlan::masks`] for a lake node, `None` for a node drawn wherever `W - h_morph > 0` (the sea).
    pub mask: Option<u32>,
}

/// Slots of the lake mask atlas a frame holds (REND-065): a lake node takes one; past it the plan counts the rest in [`WaterPlan::mask_dropped`] (drawn without the mask, never silent).
pub const LAKE_MASK_SLOTS: usize = 256;

/// The lake mask of one water node (REND-065): `size * size` codes (row-major, `size = cells + 1`, one a sample of the node), 255 where the commit's mask has the node's lake at the sample's nearest cell, 0 elsewhere.
#[derive(Clone, Debug, PartialEq)]
pub struct WaterMask {
    pub size: u32,
    pub codes: Vec<u8>,
}

/// The water of one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct WaterPlan {
    /// Presentation parameters of the water surface (look data).
    pub look: WaterLook,
    /// Seconds on the injected clock (CON-04): the only time the waves see. Kept for the text form and the diagnostics; the nodes'
    /// phases already contain it.
    pub time_s: f64,
    pub view: WaterView,
    /// The liquids the nodes use, at most [`MAX_LIQUIDS`].
    pub liquids: Vec<LiquidAppearance>,
    pub nodes: Vec<WaterNode>,
    /// The lake masks the nodes name (REND-065).
    pub masks: Vec<WaterMask>,
    /// Lake nodes that got no mask slot because the atlas was full (drawn wherever `W - h_morph > 0`).
    pub mask_dropped: u32,
}

/// Why a water plan could not be made.
#[derive(Debug, PartialEq)]
pub enum WaterError {
    Read(String),
    /// A tile reported a liquid the table does not define.
    UnknownLiquid(LiquidKindId),
    TooManyLiquids {
        kinds: usize,
        max: usize,
    },
    BadLevel {
        tile: TileId,
        level_m: f64,
    },
}

impl std::fmt::Display for WaterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WaterError::Read(e) => write!(f, "reading the water field: {e}"),
            WaterError::UnknownLiquid(k) => write!(f, "the water field names liquid {}, which the liquid table does not define", k.0),
            WaterError::TooManyLiquids { kinds, max } => {
                write!(f, "the frame has water of {kinds} liquids, the water pass draws at most {max}")
            }
            WaterError::BadLevel { tile, level_m } => {
                write!(f, "tile {:016x}: the water level {level_m} is not a finite height", tile.raw())
            }
        }
    }
}

impl std::error::Error for WaterError {}

impl From<ReadError> for WaterError {
    fn from(e: ReadError) -> Self {
        WaterError::Read(format!("{e:?}"))
    }
}

/// A source of the field W by direction (the production one is [`WaterField`]; a test may stand in).
pub trait WaterSource: Sync {
    fn water_at(&self, dir: Vec3) -> Result<Option<WaterSample>, ReadError>;

    /// The sea of the coastal band at `dir` (ADR 0056); a source without a band has none.
    fn band_at(&self, _dir: Vec3) -> Result<Option<WaterSample>, ReadError> {
        Ok(None)
    }

    /// Whether a vertex that weighs in the interpolation at `dir` is wet (ADR 0071): ocean, or reached by the sea through low ground. A
    /// source without a sea never says so.
    fn wet_at(&self, _dir: Vec3) -> Result<bool, ReadError> {
        Ok(false)
    }

    /// The water of a tile that spans more than one block of the bake, from the coarse level of W (ADR 0058): `None` when the source
    /// has none for the tile (the tile is asked point by point), else the answer for the whole tile.
    fn coarse_tile_water(&self, _map: &dyn FaceMapping, _tile: TileId, _radius_m: f64) -> Result<Option<Option<WaterSample>>, ReadError> {
        Ok(None)
    }
}

impl WaterSource for WaterField<'_> {
    fn water_at(&self, dir: Vec3) -> Result<Option<WaterSample>, ReadError> {
        WaterField::water_at(self, dir)
    }

    fn band_at(&self, dir: Vec3) -> Result<Option<WaterSample>, ReadError> {
        WaterField::band_at(self, dir)
    }

    fn wet_at(&self, dir: Vec3) -> Result<bool, ReadError> {
        WaterField::wet_at(self, dir)
    }

    fn coarse_tile_water(&self, map: &dyn FaceMapping, tile: TileId, radius_m: f64) -> Result<Option<Option<WaterSample>>, ReadError> {
        WaterField::coarse_tile_water(self, map, tile, radius_m)
    }
}

/// The most tiles a [`WaterBook`] holds by default (`waterbook-eviction`): about 100 bytes an entry, so 26 MB, more than ten times the largest
/// resident set R of a deep flight (21,000 tiles at 1,000 km, `clipmap::plan_time_for_a_large_r`) so that no tile of R is evicted.
pub const WATER_BOOK_CAP: usize = 262_144;

/// One tile of the book: its answer, the lowest own-lattice height of its mesh when its job left one (ATMO-020) and when it was last used.
#[derive(Clone, Copy, Debug)]
struct BookEntry {
    answer: Option<WaterSample>,
    min_m: Option<f32>,
    used: u64,
}

#[derive(Default)]
struct BookInner {
    tiles: HashMap<TileId, BookEntry>,
    /// Counts every use: the order of the entries by last use.
    clock: u64,
}

impl BookInner {
    fn touch(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }
}

/// The water of tiles computed off the frame (ADR 0056): the tile's job fills its answer here and the plan reads it. A tile's answer
/// is a pure function of the tile and the field, so which thread computes it, and when, changes nothing (CON-14). The book is bounded: past
/// its capacity the least recently used eighth of its entries goes, and a tile asked for again is computed again, to the same bits.
pub struct WaterBook {
    inner: std::sync::Mutex<BookInner>,
    cap: usize,
    filled: std::sync::atomic::AtomicU64,
    evicted: std::sync::atomic::AtomicU64,
}

impl Default for WaterBook {
    fn default() -> WaterBook {
        WaterBook::with_capacity(WATER_BOOK_CAP)
    }
}

impl WaterBook {
    pub fn new() -> WaterBook {
        WaterBook::default()
    }

    /// A book that holds at most `cap` tiles (at least 8).
    pub fn with_capacity(cap: usize) -> WaterBook {
        WaterBook {
            inner: std::sync::Mutex::new(BookInner::default()),
            cap: cap.max(8),
            filled: std::sync::atomic::AtomicU64::new(0),
            evicted: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Keep `min_m`, the lowest height of the own lattice vertices of `tile`'s mesh, for the height test of the cut (ATMO-020). Only a
    /// tile whose answer in the book is wet keeps one (a dry tile is never a water node); called by the tile's job after
    /// [`fill`](WaterBook::fill). A pure function of the tile and the field, so two jobs of one tile agree.
    pub fn set_min_height(&self, tile: TileId, min_m: f32) {
        let mut b = self.inner.lock().expect("water book");
        let used = b.touch();
        if let Some(e) = b.tiles.get_mut(&tile).filter(|e| min_m.is_finite() && e.answer.is_some()) {
            e.min_m = Some(min_m);
            e.used = used;
        }
    }

    /// The lowest height of the own lattice vertices of `tile`'s mesh, when its job left one.
    pub fn min_height(&self, tile: TileId) -> Option<f32> {
        let mut b = self.inner.lock().expect("water book");
        let used = b.touch();
        b.tiles.get_mut(&tile).and_then(|e| {
            e.used = used;
            e.min_m
        })
    }

    /// Keep the answer of `tile` if the book has none; the count of entries stays within the capacity. Returns whether it was new.
    fn insert_answer(&self, tile: TileId, answer: Option<WaterSample>) -> bool {
        let mut b = self.inner.lock().expect("water book");
        let used = b.touch();
        if let Some(e) = b.tiles.get_mut(&tile) {
            e.used = used;
            return false;
        }
        b.tiles.insert(tile, BookEntry { answer, min_m: None, used });
        if b.tiles.len() > self.cap {
            // The least recently used eighth goes (ties by tile id: the same entries for any order of the map).
            let drop = (self.cap / 8).max(1);
            let mut order: Vec<(u64, TileId)> = b.tiles.iter().map(|(t, e)| (e.used, *t)).collect();
            order.sort_unstable();
            for (_, t) in order.into_iter().take(drop) {
                b.tiles.remove(&t);
            }
            self.evicted.fetch_add(drop as u64, std::sync::atomic::Ordering::Relaxed);
        }
        true
    }

    /// Compute the water of `tile` (in the caller's thread, a tile job) and keep it. A tile already held is not computed again.
    pub fn fill(&self, src: &dyn WaterSource, map: &dyn FaceMapping, tile: TileId, radius_m: f64) -> Result<(), WaterError> {
        self.fill_par(src, map, tile, radius_m, &8usize)
    }

    /// [`fill`](WaterBook::fill) with the helper threads of the block loads asked from `par` (the job's share, BUDG-004).
    pub fn fill_par(
        &self,
        src: &dyn WaterSource,
        map: &dyn FaceMapping,
        tile: TileId,
        radius_m: f64,
        par: &dyn planet_core::par::Par,
    ) -> Result<(), WaterError> {
        if self.inner.lock().expect("water book").tiles.contains_key(&tile) {
            return Ok(());
        }
        // A tile that spans more than one block is answered by the coarse level of W and reads none (ADR 0058).
        if let Some(answer) = src.coarse_tile_water(map, tile, radius_m)? {
            if self.insert_answer(tile, answer) {
                self.filled.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            return Ok(());
        }
        // The store blocks the samples touch load in parallel first: the job then waits for the slowest block, not for the sum.
        let n = TILE_SAMPLES;
        let dirs: Vec<Vec3> = (0..=n).flat_map(|l| (0..=n).map(move |k| tile.sample_direction(map, n, k, l))).collect();
        // The helper threads run at the job's priority (a new thread starts at normal): the workers run below normal so that the frame
        // thread gets its core back (STRM-020), and a job's helpers must not take it.
        let priority = planet_streaming::priority::current();
        let chunks: Vec<&[Vec3]> = dirs.chunks(dirs.len().div_ceil(8)).collect();
        par.run(chunks.len(), &|c| {
            if let Some(p) = priority {
                planet_streaming::priority::set_current(p);
            }
            for d in chunks[c] {
                let _ = src.water_at(*d);
            }
        });
        let answer = WaterTileCache::new().tile_water(src, map, tile, radius_m)?;
        // Two jobs of one tile compute the same answer; only the first to insert counts.
        if self.insert_answer(tile, answer) {
            self.filled.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn get(&self, tile: TileId) -> Option<Option<WaterSample>> {
        let mut b = self.inner.lock().expect("water book");
        let used = b.touch();
        b.tiles.get_mut(&tile).map(|e| {
            e.used = used;
            e.answer
        })
    }

    /// Tiles computed by [`fill`](WaterBook::fill).
    pub fn filled(&self) -> u64 {
        self.filled.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The tiles the book holds now.
    pub fn len(&self) -> usize {
        self.inner.lock().expect("water book").tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The most tiles the book holds.
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// Entries evicted so far.
    pub fn evicted(&self) -> u64 {
        self.evicted.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// What a [`WaterTileCache`] has counted so far; the difference of two snapshots is the share of the plans between them (the
/// frames log's recorded-only columns).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WaterCounters {
    pub queries: u64,
    pub tile_misses: u64,
    pub band_queries: u64,
}

impl WaterCounters {
    pub fn since(self, earlier: WaterCounters) -> WaterCounters {
        WaterCounters {
            queries: self.queries - earlier.queries,
            tile_misses: self.tile_misses - earlier.tile_misses,
            band_queries: self.band_queries - earlier.band_queries,
        }
    }
}

/// A memoised lake mask (REND-065): `None` when no sample of the tile is the lake's.
type LakeMaskMemo = Option<std::sync::Arc<Vec<u8>>>;

/// The per-tile answers of the selection query, memoised (a pure function of the tile and the field).
#[derive(Default)]
pub struct WaterTileCache {
    tiles: HashMap<TileId, Option<WaterSample>>,
    /// Where the tiles' jobs left their answers (ADR 0056); a tile found there costs no field query.
    book: Option<std::sync::Arc<WaterBook>>,
    /// Tiles answered from the book.
    pub book_hits: u64,
    /// Field queries made and the nanoseconds they took, for the cost report (recorded only).
    pub queries: u64,
    /// Band and connectivity queries (ADR 0056), counted apart from the selection's own.
    pub band_queries: u64,
    pub tile_misses: u64,
    pub query_ns: u64,
    /// Nodes of the last plan left out by the horizon test and by the height test (ATMO-020); recorded only.
    pub cut_horizon: u64,
    pub cut_height: u64,
    /// Of `cut_height`, the nodes cut by the exact minimum of the tile's own vertices (and not by the loose bound).
    pub cut_exact: u64,
    /// Nodes that passed the loose test with no minimum in the book (their job has not left one): they keep the loose verdict.
    pub no_minimum: u64,
    /// The extent and the lowest terrain bound of the tiles the cut has looked at (pure functions of the tile and the height field).
    extents: HashMap<TileId, (TileExtent, Option<f64>)>,
    /// The lake masks of the tiles (REND-065), by tile, node size and body: a pure function of those and the lake source (one source a cache: the viewer's world sets it once). `None`: no sample of the tile is the lake's.
    masks: HashMap<(TileId, u32, u64), LakeMaskMemo>,
}

impl WaterTileCache {
    pub fn new() -> WaterTileCache {
        WaterTileCache::default()
    }

    /// Remember a tile's answer. The memo is bounded like the book: past [`WATER_BOOK_CAP`] tiles it starts over (every answer is a pure function
    /// of the tile and the field, so a forgotten one is read from the book or asked of the field again, to the same bits).
    fn memo(&mut self, tile: TileId, answer: Option<WaterSample>) {
        if self.tiles.len() >= WATER_BOOK_CAP {
            self.tiles.clear();
        }
        self.tiles.insert(tile, answer);
    }

    /// A cache that reads the answers the tile jobs left in `book` before it asks the field.
    pub fn with_book(book: std::sync::Arc<WaterBook>) -> WaterTileCache {
        WaterTileCache { book: Some(book), ..WaterTileCache::default() }
    }

    pub fn counters(&self) -> WaterCounters {
        WaterCounters { queries: self.queries, tile_misses: self.tile_misses, band_queries: self.band_queries }
    }

    pub fn clear(&mut self) {
        self.tiles.clear();
        self.extents.clear();
        self.masks.clear();
    }

    /// The lake mask of `tile` for the lake `body` on nodes of `cells` cells (REND-065), memoised ([`node_lake_mask`]).
    pub fn lake_mask(
        &mut self,
        lakes: &dyn LakeMaskSource,
        tile: TileId,
        cells: u32,
        body: WaterBodyId,
    ) -> Result<Option<std::sync::Arc<Vec<u8>>>, WaterError> {
        if let Some(m) = self.masks.get(&(tile, cells, body.0)) {
            return Ok(m.clone());
        }
        let m = node_lake_mask(lakes, tile, cells, body)?.map(std::sync::Arc::new);
        if self.masks.len() >= WATER_BOOK_CAP {
            self.masks.clear();
        }
        self.masks.insert((tile, cells, body.0), m.clone());
        Ok(m)
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// The water of `tile` (module doc), memoised.
    pub fn tile_water(
        &mut self,
        src: &dyn WaterSource,
        map: &dyn FaceMapping,
        tile: TileId,
        radius_m: f64,
    ) -> Result<Option<WaterSample>, WaterError> {
        if let Some(w) = self.tiles.get(&tile) {
            return Ok(*w);
        }
        if let Some(w) = self.book.as_ref().and_then(|b| b.get(tile)) {
            self.book_hits += 1;
            self.memo(tile, w);
            return Ok(w);
        }
        let started = std::time::Instant::now();
        if let Some(coarse) = src.coarse_tile_water(map, tile, radius_m)? {
            self.memo(tile, coarse);
            return Ok(coarse);
        }
        let n = TILE_SAMPLES;
        let mut found: Vec<(WaterSample, u32)> = Vec::new();
        for l in 0..=n {
            for k in 0..=n {
                self.queries += 1;
                if let Some(w) = src.water_at(tile.sample_direction(map, n, k, l))? {
                    match found.iter_mut().find(|(s, _)| s.body == w.body && s.level_m.to_bits() == w.level_m.to_bits()) {
                        Some((_, c)) => *c += 1,
                        None => found.push((w, 1)),
                    }
                }
            }
        }
        // The most frequent body; ties go to the lower body id (a deterministic rule).
        let mut best = found.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.body.cmp(&a.0.body))).map(|(w, _)| w);
        if best.is_none() {
            best = self.band_water(src, map, tile, radius_m)?;
        }
        self.query_ns += u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.tile_misses += 1;
        self.memo(tile, best);
        Ok(best)
    }
}

impl WaterTileCache {
    /// The sea of the coastal band for a tile with no water of its own (ADR 0071): a sample of the tile is band (`band_at`) and a vertex
    /// that weighs in its interpolation is wet, so the tile carries the sea of the first such sample. Connectivity decides, not distance.
    fn band_water(
        &mut self,
        src: &dyn WaterSource,
        map: &dyn FaceMapping,
        tile: TileId,
        _radius_m: f64,
    ) -> Result<Option<WaterSample>, WaterError> {
        let n = TILE_SAMPLES;
        for l in 0..=n {
            for k in 0..=n {
                self.band_queries += 1;
                let dir = tile.sample_direction(map, n, k, l);
                if let Some(w) = src.band_at(dir)? {
                    self.band_queries += 1;
                    if src.wet_at(dir)? {
                        return Ok(Some(w));
                    }
                }
            }
        }
        Ok(None)
    }
}

/// What the water plan is made from.
pub struct WaterRequest<'a> {
    pub source: &'a dyn WaterSource,
    pub map: &'a dyn FaceMapping,
    pub liquids: &'a LiquidTable,
    pub look: &'a WaterLook,
    pub lod: &'a LodParams,
    /// Seconds on the injected clock.
    pub time_s: f64,
    pub view: WaterView,
    /// The terrain's height field: `Some` switches the cut of ATMO-020 on (the horizon and the height test, the tile's height bounds
    /// being the ones the node selection culls with); `None` keeps every node that carries water.
    pub cut: Option<&'a dyn HeightField>,
    /// The lake masks of the drawn commit (REND-065): a lake node is drawn only where its mask is wet. `None`: no lake masks (every node drawn wherever `W - h_morph > 0`).
    pub lakes: Option<&'a dyn LakeMaskSource>,
}

/// The lake mask of a node (REND-065): the samples of node `tile` (`cells` cells an edge, `cells + 1` samples a side) placed on the lake grid by [`crate::river::node_window`], each read at its nearest
/// cell (clamped to the face: a sample on the face's edge reads the face's border cell): 255 where that cell is the lake `body`'s, 0 elsewhere. `None` when no sample is the lake's.
pub fn node_lake_mask(lakes: &dyn LakeMaskSource, tile: TileId, cells: u32, body: WaterBodyId) -> Result<Option<Vec<u8>>, WaterError> {
    let ((x0, y0), step) = crate::river::node_window(tile, cells, lakes.cells() as u32);
    let size = cells as usize + 1;
    let mut codes = vec![0u8; size * size];
    let mut any = false;
    // A sample on the face's outer edge lies half a cell outside the first or last cell: it reads that border cell of its own face.
    let last = lakes.cells() as i64 - 1;
    for b in 0..size {
        let j = (libm::round(y0 + b as f64 * step) as i64).clamp(0, last);
        for a in 0..size {
            let i = (libm::round(x0 + a as f64 * step) as i64).clamp(0, last);
            let c = lakes.lake_at(tile.face(), i, j).map_err(|e| WaterError::Read(e.to_string()))?;
            if c.is_some_and(|c| lake_body(c.face, c.index) == body) {
                codes[b * size + a] = 255;
                any = true;
            }
        }
    }
    Ok(any.then_some(codes))
}

/// The distance (m) from a camera `height_m` above the level `level_m` out to which a surface that rises `rise_m` above that level
/// can be seen over the sphere of radius `radius_m + level_m`: the tangent length of the camera plus that of the raised surface.
/// `sag_m` lowers the blocking sphere (the drawn surface lies under it); the camera and the rise grow by it. A pure function of its
/// arguments (f64, `libm`); `height_m`, `rise_m` and `sag_m` are not negative.
pub fn visible_distance_m(radius_m: f64, level_m: f64, height_m: f64, rise_m: f64, sag_m: f64) -> f64 {
    let r = radius_m + level_m - sag_m;
    let (h, a) = (height_m + sag_m, rise_m + sag_m);
    libm::sqrt(2.0 * r * h + h * h) + libm::sqrt(2.0 * r * a + a * a)
}

/// The sag of the drawn surface under the sphere of radius `r`: a chord of two sample spacings `spacing_m`.
pub fn surface_sag_m(r: f64, spacing_m: f64) -> f64 {
    let chord = 2.0 * spacing_m;
    chord * chord / (8.0 * r)
}

/// Whether a tile lies beyond the horizon of the level (ATMO-020). `camera` is the planet-fixed position, `dir` the unit direction
/// of the tile's centre, `chord_radius_m` the chord radius of its bounding sphere on the sphere of `radius_m`, `rise_m` the largest
/// vertical displacement of the waves, `shift_m` the largest displacement of a surface point in any direction and `sag_m` the sag of
/// the drawn surface ([`surface_sag_m`]). The camera at or below the level never cuts. The tile is cut when the nearest point its
/// water can reach, the distance from the camera to the tile's centre on the sphere `radius_m + level_m` less the tile's radius
/// scaled to that sphere and less `shift_m`, is farther than [`visible_distance_m`].
#[allow(clippy::too_many_arguments)]
pub fn tile_beyond_horizon(
    camera: Vec3,
    dir: Vec3,
    chord_radius_m: f64,
    radius_m: f64,
    level_m: f64,
    rise_m: f64,
    shift_m: f64,
    sag_m: f64,
) -> bool {
    let r = radius_m + level_m;
    let height = camera.length() - r;
    if height.is_nan() || height <= 0.0 {
        return false;
    }
    let near = (camera - dir * r).length() - chord_radius_m * (r / radius_m) - shift_m;
    near > visible_distance_m(radius_m, level_m, height, rise_m, sag_m)
}

/// Whether the lowest terrain of a tile keeps its water from showing (ATMO-020): the tile's lower height bound is at or above the
/// level plus the waves' rise and four f32 roundings of the larger height (the GPU holds heights as f32).
pub fn tile_above_water(lowest_m: f64, level_m: f64, rise_m: f64) -> bool {
    let eps = 4.0 * f64::from(f32::EPSILON) * lowest_m.abs().max(level_m.abs()).max(1.0);
    lowest_m >= level_m + rise_m + eps
}

/// The water plan for the nodes of `terrain`: the nodes that carry water with their level, liquid and reduced wave phases.
/// Deterministic (a pure function of the plan, the field, the table, the look and the time).
pub fn plan_water(req: &WaterRequest<'_>, cache: &mut WaterTileCache, terrain: &TerrainPlan) -> Result<WaterPlan, WaterError> {
    let mut liquids: Vec<LiquidAppearance> = Vec::new();
    let mut nodes = Vec::new();
    let (mut masks, mut mask_dropped): (Vec<WaterMask>, u32) = (Vec::new(), 0);
    (cache.cut_horizon, cache.cut_height, cache.cut_exact, cache.no_minimum) = (0, 0, 0, 0);
    let camera = terrain.camera.position.0;
    for (i, n) in terrain.nodes.iter().enumerate() {
        let Some(w) = cache.tile_water(req.source, req.map, n.tile, req.lod.radius_m)? else { continue };
        if !w.level_m.is_finite() || w.level_m.abs() > 1.0e5 {
            return Err(WaterError::BadLevel { tile: n.tile, level_m: w.level_m });
        }
        let slot = match liquids.iter().position(|l| l.kind == w.kind.0) {
            Some(s) => s,
            None => {
                let row = req.liquids.get(w.kind).ok_or(WaterError::UnknownLiquid(w.kind))?;
                if liquids.len() == MAX_LIQUIDS {
                    return Err(WaterError::TooManyLiquids { kinds: liquids.len() + 1, max: MAX_LIQUIDS });
                }
                liquids.push(LiquidAppearance::new(w.kind.0, row, req.look));
                liquids.len() - 1
            }
        };
        if let Some(field) = req.cut {
            let rise = liquids[slot].amplitude_sum();
            let shift = rise * (1.0 + req.look.choppy);
            let (extent, lowest) = {
                let entry = cache.extents.entry(n.tile).or_insert_with(|| (tile_extent(req.map, n.tile, req.lod), None));
                (entry.0, entry.1)
            };
            let level = f64::from(w.level_m as f32);
            let sag = surface_sag_m(req.lod.radius_m + level, req.lod.sample_spacing(n.level));
            if tile_beyond_horizon(camera, extent.dir, extent.chord_radius_m, req.lod.radius_m, level, rise, shift, sag) {
                cache.cut_horizon += 1;
                continue;
            }
            let lowest = match lowest {
                Some(l) => l,
                None => {
                    let l = field.height_bounds(extent.dir, extent.angular_radius).0;
                    if let Some(e) = cache.extents.get_mut(&n.tile) {
                        e.1 = Some(l);
                    }
                    l
                }
            };
            if tile_above_water(lowest, level, rise) {
                cache.cut_height += 1;
                continue;
            }
            // The exact minimum of the tile's own mesh vertices, when its job left one (a tile with none keeps the loose bound's
            // verdict). Read from the book on the plan thread on every plan (a lookup, never a computation, and never memoised: a
            // tile rebuilt in another format leaves that format's minimum, and the plan must cut by what is drawn).
            let exact = cache.book.as_ref().and_then(|b| b.min_height(n.tile));
            cache.no_minimum += u64::from(exact.is_none());
            if exact.is_some_and(|m| tile_above_water(f64::from(m), level, rise)) {
                cache.cut_height += 1;
                cache.cut_exact += 1;
                continue;
            }
        }
        // A lake node is drawn only where its lake's mask is wet (REND-065); a node none of whose samples is the lake's carries no water.
        let mut mask = None;
        if let (Some(lakes), true) = (req.lakes, lake_of_body(w.body).is_some()) {
            let Some(codes) = cache.lake_mask(lakes, n.tile, req.lod.cells, w.body)? else { continue };
            if masks.len() < LAKE_MASK_SLOTS {
                mask = Some(masks.len() as u32);
                masks.push(WaterMask { size: req.lod.cells + 1, codes: codes.as_ref().clone() });
            } else {
                mask_dropped += 1;
            }
        }
        let o = n.origin.0;
        nodes.push(WaterNode {
            node: i as u32,
            level_m: w.level_m as f32,
            liquid: slot as u8,
            phases: tile_phases(&liquids[slot], [o.x, o.y, o.z], req.time_s),
            spacing_m: req.lod.sample_spacing(n.level) as f32,
            mask,
        });
    }
    Ok(WaterPlan { look: *req.look, time_s: req.time_s, view: req.view, liquids, nodes, masks, mask_dropped })
}

/// An empty water plan (no node): what a viewer warms the pass with.
pub fn empty_water_plan(look: &WaterLook) -> WaterPlan {
    WaterPlan { look: *look, time_s: 0.0, view: WaterView::Lit, liquids: Vec::new(), nodes: Vec::new(), masks: Vec::new(), mask_dropped: 0 }
}

/// The sea's body id, for tests that look for it.
pub const SEA_BODY: WaterBodyId = WaterBodyId::SEA;

fn h64(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn h32(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

fn p64(s: &str) -> Result<f64, String> {
    u64::from_str_radix(s, 16).map(f64::from_bits).map_err(|e| format!("bad f64 bits '{s}': {e}"))
}

fn p32(s: &str) -> Result<f32, String> {
    u32::from_str_radix(s, 16).map(f32::from_bits).map_err(|e| format!("bad f32 bits '{s}': {e}"))
}

impl WaterPlan {
    /// Check the plan before it reaches the GPU: finite values, liquids within the limit, nodes naming a liquid and a wave phase in
    /// `[0, 2 pi)`.
    pub fn validate(&self) -> Result<(), String> {
        if self.liquids.len() > MAX_LIQUIDS {
            return Err(format!("water plan: {} liquids, at most {MAX_LIQUIDS}", self.liquids.len()));
        }
        if let Err((key, value, why)) = self.look.validate() {
            return Err(format!("water plan: `{key}` = {value}: {why}"));
        }
        for n in &self.nodes {
            if usize::from(n.liquid) >= self.liquids.len() {
                return Err(format!("water plan: node {} names liquid slot {}, the plan has {}", n.node, n.liquid, self.liquids.len()));
            }
            if !n.level_m.is_finite() {
                return Err(format!("water plan: node {} has the level {}", n.node, n.level_m));
            }
            if n.phases.iter().any(|p| !(0.0..=std::f32::consts::TAU).contains(p)) {
                return Err(format!("water plan: node {} has a wave phase outside [0, 2 pi]: {:?}", n.node, n.phases));
            }
            if n.mask.is_some_and(|m| m as usize >= self.masks.len()) {
                return Err(format!("water plan: node {} names lake mask {:?}, the plan has {}", n.node, n.mask, self.masks.len()));
            }
        }
        if self.masks.len() > LAKE_MASK_SLOTS {
            return Err(format!("water plan: {} lake masks, at most {LAKE_MASK_SLOTS}", self.masks.len()));
        }
        if let Some(m) = self.masks.iter().find(|m| m.size < 2 || m.codes.len() != (m.size * m.size) as usize) {
            return Err(format!("water plan: a lake mask of size {} holds {} codes", m.size, m.codes.len()));
        }
        Ok(())
    }

    /// Stable text for plan snapshots and repro bundles (floats as bit patterns): a `water` line, one `liquid` line per liquid and one
    /// `wnode` line per node.
    pub fn to_text(&self) -> String {
        let l = self.look.values().map(h64).join(" ");
        let mut s = format!("water time {} view {} look {l}\n", h64(self.time_s), self.view.name());
        for q in &self.liquids {
            let mut v: Vec<f64> = Vec::new();
            v.extend(q.colour);
            v.extend(q.sigma);
            v.extend(q.emission);
            for w in &q.waves {
                v.extend(w.k);
                v.extend([w.amplitude, w.phi, w.omega]);
            }
            s += &format!("liquid {} {}\n", q.kind, v.into_iter().map(h64).collect::<Vec<_>>().join(" "));
        }
        for n in &self.nodes {
            s += &format!(
                "wnode {} level {} liquid {} phases {} spacing {}{}\n",
                n.node,
                h32(n.level_m),
                n.liquid,
                n.phases.map(h32).join(" "),
                h32(n.spacing_m),
                n.mask.map(|m| format!(" mask {m}")).unwrap_or_default()
            );
        }
        // The lake masks (REND-065): one line each, the codes as hex; a plan without lakes writes none (the text of every older plan is unchanged).
        for m in &self.masks {
            s += &format!("wmask {} {}\n", m.size, m.codes.iter().map(|c| format!("{c:02x}")).collect::<String>());
        }
        if self.mask_dropped > 0 {
            s += &format!("wmaskdropped {}\n", self.mask_dropped);
        }
        // No trailing newline: the caller adds the separators.
        s.pop();
        s
    }

    /// Read the lines [`to_text`](WaterPlan::to_text) writes (the first is the `water` line).
    pub fn from_lines(lines: &[Vec<&str>]) -> Result<WaterPlan, String> {
        let mut plan: Option<WaterPlan> = None;
        for t in lines {
            match t.as_slice() {
                ["water", "time", time, "view", view, "look", rest @ ..] => {
                    let mut look = WaterLook::EARTH;
                    if rest.len() != WaterLook::KEYS.len() {
                        return Err(format!("water line: the look needs {} values, got {}", WaterLook::KEYS.len(), rest.len()));
                    }
                    for (slot, s) in look.values_mut().into_iter().zip(rest) {
                        *slot = p64(s)?;
                    }
                    plan = Some(WaterPlan {
                        look,
                        time_s: p64(time)?,
                        view: WaterView::parse(view).ok_or(format!("unknown water view '{view}'"))?,
                        liquids: Vec::new(),
                        nodes: Vec::new(),
                        masks: Vec::new(),
                        mask_dropped: 0,
                    });
                }
                ["liquid", kind, rest @ ..] => {
                    let p = plan.as_mut().ok_or("a liquid line before the water line")?;
                    let v = rest.iter().map(|s| p64(s)).collect::<Result<Vec<f64>, _>>()?;
                    if v.len() != 9 + WAVES * 6 {
                        return Err(format!("liquid line: {} values, expected {}", v.len(), 9 + WAVES * 6));
                    }
                    let mut waves = [crate::look::water::Wave { k: [0.0; 3], amplitude: 0.0, phi: 0.0, omega: 0.0 }; WAVES];
                    for (i, w) in waves.iter_mut().enumerate() {
                        let o = 9 + 6 * i;
                        *w =
                            crate::look::water::Wave { k: [v[o], v[o + 1], v[o + 2]], amplitude: v[o + 3], phi: v[o + 4], omega: v[o + 5] };
                    }
                    p.liquids.push(LiquidAppearance {
                        kind: kind.parse().map_err(|e| format!("bad liquid kind '{kind}': {e}"))?,
                        colour: [v[0], v[1], v[2]],
                        sigma: [v[3], v[4], v[5]],
                        emission: [v[6], v[7], v[8]],
                        waves,
                    });
                }
                ["wnode", node, "level", level, "liquid", liquid, "phases", a, b, c, d, "spacing", sp, rest @ ..] => {
                    let p = plan.as_mut().ok_or("a wnode line before the water line")?;
                    let mask = match rest {
                        [] => None,
                        ["mask", m] => Some(m.parse().map_err(|e| format!("bad lake mask index '{m}': {e}"))?),
                        _ => return Err(format!("cannot parse the end of a wnode line '{}'", rest.join(" "))),
                    };
                    p.nodes.push(WaterNode {
                        node: node.parse().map_err(|e| format!("bad node index '{node}': {e}"))?,
                        level_m: p32(level)?,
                        liquid: liquid.parse().map_err(|e| format!("bad liquid slot '{liquid}': {e}"))?,
                        phases: [p32(a)?, p32(b)?, p32(c)?, p32(d)?],
                        spacing_m: p32(sp)?,
                        mask,
                    });
                }
                ["wmask", size, hex] => {
                    let p = plan.as_mut().ok_or("a wmask line before the water line")?;
                    let size: u32 = size.parse().map_err(|e| format!("bad lake mask size '{size}': {e}"))?;
                    if hex.len() % 2 != 0 {
                        return Err(format!("a wmask line of size {size} has an odd hex length {}", hex.len()));
                    }
                    let codes = (0..hex.len() / 2)
                        .map(|k| u8::from_str_radix(&hex[2 * k..2 * k + 2], 16).map_err(|e| format!("bad lake mask code: {e}")))
                        .collect::<Result<Vec<u8>, _>>()?;
                    p.masks.push(WaterMask { size, codes });
                }
                ["wmaskdropped", n] => {
                    let p = plan.as_mut().ok_or("a wmaskdropped line before the water line")?;
                    p.mask_dropped = n.parse().map_err(|e| format!("bad dropped mask count '{n}': {e}"))?;
                }
                other => return Err(format!("cannot parse water plan line '{}'", other.join(" "))),
            }
        }
        plan.ok_or_else(|| "no water line".to_string())
    }
}
