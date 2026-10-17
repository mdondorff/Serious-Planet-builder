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
//! **Coarse tiles (ADR 0057, GEN-024).** A tile whose area is at least one block of the stored bake is not asked point by point (that
//! reads up to 25 blocks): the source answers it from the coarse level of W, a summary the bake wrote once, as the union over the cells
//! the tile touches (a superset of the point query's answer, and a parent is wet wherever its children are). The query is
//! [`WaterSource::coarse_tile_water`]; a source without a summary answers `None` and the tile is asked point by point.

use crate::lod::LodParams;
use crate::look::water::{tile_phases, LiquidAppearance, WaterLook, MAX_LIQUIDS, WAVES};
use crate::terrain::TerrainPlan;
use planet_core::cube::FaceMapping;
use planet_core::{TileId, Vec3};
use planet_generators::global::ReadError;
use planet_generators::water::{WaterBodyId, WaterField, WaterSample, COAST_REACH_M};
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

    /// Whether the ocean channel marks a vertex at `dir`'s cell (ADR 0056); a source without one never does.
    fn ocean_bit_at(&self, _dir: Vec3) -> Result<bool, ReadError> {
        Ok(false)
    }

    /// The water of a tile that spans more than one block of the bake, from the coarse level of W (ADR 0057): `None` when the source
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

    fn ocean_bit_at(&self, dir: Vec3) -> Result<bool, ReadError> {
        WaterField::ocean_bit_at(self, dir)
    }

    fn coarse_tile_water(&self, map: &dyn FaceMapping, tile: TileId, radius_m: f64) -> Result<Option<Option<WaterSample>>, ReadError> {
        WaterField::coarse_tile_water(self, map, tile, radius_m)
    }
}

/// The water of tiles computed off the frame (ADR 0056): the tile's job fills its answer here and the plan reads it. A tile's answer
/// is a pure function of the tile and the field, so which thread computes it, and when, changes nothing (CON-14).
#[derive(Default)]
pub struct WaterBook {
    tiles: std::sync::Mutex<HashMap<TileId, Option<WaterSample>>>,
    filled: std::sync::atomic::AtomicU64,
}

impl WaterBook {
    pub fn new() -> WaterBook {
        WaterBook::default()
    }

    /// Compute the water of `tile` (in the caller's thread, a tile job) and keep it. A tile already held is not computed again.
    pub fn fill(&self, src: &dyn WaterSource, map: &dyn FaceMapping, tile: TileId, radius_m: f64) -> Result<(), WaterError> {
        if self.tiles.lock().expect("water book").contains_key(&tile) {
            return Ok(());
        }
        // A tile that spans more than one block is answered by the coarse level of W and reads none (ADR 0057).
        if let Some(answer) = src.coarse_tile_water(map, tile, radius_m)? {
            if self.tiles.lock().expect("water book").insert(tile, answer).is_none() {
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
        std::thread::scope(|scope| {
            for chunk in dirs.chunks(dirs.len().div_ceil(8)) {
                scope.spawn(move || {
                    if let Some(p) = priority {
                        planet_streaming::priority::set_current(p);
                    }
                    for d in chunk {
                        let _ = src.water_at(*d);
                    }
                });
            }
        });
        let answer = WaterTileCache::new().tile_water(src, map, tile, radius_m)?;
        // Two jobs of one tile compute the same answer; only the first to insert counts.
        if self.tiles.lock().expect("water book").insert(tile, answer).is_none() {
            self.filled.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn get(&self, tile: TileId) -> Option<Option<WaterSample>> {
        self.tiles.lock().expect("water book").get(&tile).copied()
    }

    /// Tiles computed by [`fill`](WaterBook::fill).
    pub fn filled(&self) -> u64 {
        self.filled.load(std::sync::atomic::Ordering::Relaxed)
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
}

impl WaterTileCache {
    pub fn new() -> WaterTileCache {
        WaterTileCache::default()
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
            self.tiles.insert(tile, w);
            return Ok(w);
        }
        let started = std::time::Instant::now();
        if let Some(coarse) = src.coarse_tile_water(map, tile, radius_m)? {
            self.tiles.insert(tile, coarse);
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
        self.tiles.insert(tile, best);
        Ok(best)
    }
}

/// Probes of the connectivity test per axis at most (a grid of `2 * N + 1` squared over the reach).
const REACH_PROBES: i32 = 25;

impl WaterTileCache {
    /// The sea of the coastal band for a tile with no water of its own (ADR 0056): a sample of the tile is band and an ocean vertex of the
    /// bake lies within [`COAST_REACH_M`] of the tile (of its centre plus half its diagonal). The band sample's level and body.
    fn band_water(
        &mut self,
        src: &dyn WaterSource,
        map: &dyn FaceMapping,
        tile: TileId,
        radius_m: f64,
    ) -> Result<Option<WaterSample>, WaterError> {
        let n = TILE_SAMPLES;
        let mut band = None;
        'samples: for l in 0..=n {
            for k in 0..=n {
                self.band_queries += 1;
                if let Some(w) = src.band_at(tile.sample_direction(map, n, k, l))? {
                    band = Some(w);
                    break 'samples;
                }
            }
        }
        let Some(band) = band else { return Ok(None) };
        let edge_m = core::f64::consts::FRAC_PI_2 * radius_m / f64::from(1u32 << tile.level().min(30));
        // The reach counts from the nearest point of the tile, not from its centre: a tile is wet when any point of it is within the reach, so
        // a parent is wet wherever its children are (no water vanishes as the tiles refine).
        let reach_m = COAST_REACH_M + 0.71 * edge_m;
        let centre = tile.center_direction(map);
        let helper = if centre.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        let east = centre.cross(helper).normalized();
        let north = centre.cross(east).normalized();
        let step = reach_m / f64::from(REACH_PROBES) / radius_m;
        for j in -REACH_PROBES..=REACH_PROBES {
            for i in -REACH_PROBES..=REACH_PROBES {
                if i * i + j * j > REACH_PROBES * REACH_PROBES {
                    continue;
                }
                self.band_queries += 1;
                let d = (centre + east * (f64::from(i) * step) + north * (f64::from(j) * step)).normalized();
                if src.ocean_bit_at(d)? {
                    return Ok(Some(band));
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
}

/// The water plan for the nodes of `terrain`: the nodes that carry water with their level, liquid and reduced wave phases.
/// Deterministic (a pure function of the plan, the field, the table, the look and the time).
pub fn plan_water(req: &WaterRequest<'_>, cache: &mut WaterTileCache, terrain: &TerrainPlan) -> Result<WaterPlan, WaterError> {
    let mut liquids: Vec<LiquidAppearance> = Vec::new();
    let mut nodes = Vec::new();
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
        let o = n.origin.0;
        nodes.push(WaterNode {
            node: i as u32,
            level_m: w.level_m as f32,
            liquid: slot as u8,
            phases: tile_phases(&liquids[slot], [o.x, o.y, o.z], req.time_s),
            spacing_m: req.lod.sample_spacing(n.level) as f32,
        });
    }
    Ok(WaterPlan { look: *req.look, time_s: req.time_s, view: req.view, liquids, nodes })
}

/// An empty water plan (no node): what a viewer warms the pass with.
pub fn empty_water_plan(look: &WaterLook) -> WaterPlan {
    WaterPlan { look: *look, time_s: 0.0, view: WaterView::Lit, liquids: Vec::new(), nodes: Vec::new() }
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
                "wnode {} level {} liquid {} phases {} spacing {}\n",
                n.node,
                h32(n.level_m),
                n.liquid,
                n.phases.map(h32).join(" "),
                h32(n.spacing_m)
            );
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
                ["wnode", node, "level", level, "liquid", liquid, "phases", a, b, c, d, "spacing", sp] => {
                    let p = plan.as_mut().ok_or("a wnode line before the water line")?;
                    p.nodes.push(WaterNode {
                        node: node.parse().map_err(|e| format!("bad node index '{node}': {e}"))?,
                        level_m: p32(level)?,
                        liquid: liquid.parse().map_err(|e| format!("bad liquid slot '{liquid}': {e}"))?,
                        phases: [p32(a)?, p32(b)?, p32(c)?, p32(d)?],
                        spacing_m: p32(sp)?,
                    });
                }
                other => return Err(format!("cannot parse water plan line '{}'", other.join(" "))),
            }
        }
        plan.ok_or_else(|| "no water line".to_string())
    }
}
