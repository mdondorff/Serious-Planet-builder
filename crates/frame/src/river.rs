//! The river coverage of a tile (item 15 PR 8, REND-059): for every cell centre of a window the signed distance to the nearest river edge, in cells, and its 8-bit code. The plan owns this (CON-03: what is water is
//! decided here, the renderer only reads the texture); the width is the one function of the burn (`trench_width_cells`, capped at 32 cells) with a one-cell minimum, so the drawn river is the burned trench.
//!
//! The code of a distance `d` (cells, negative inside the river): `128 + 32 d` clamped to 0..255, that is +-4 cells at a step of one thirty-second of a cell; the fragment interpolates the texture and draws
//! water where the interpolated code is at or under [`CODE_ZERO`], which makes the banks smooth lines instead of cell staircases. Distances are of the cell centres in the face's cell-index space (cell `i` has
//! its centre at `i`, as the channels' points do) and a function of the channels alone, so any window of the same channels gives the same texels (CON-30).

use planet_generators::bake::k7::{trench_width_cells, Channel, QUnit, WidthLaw};

/// The distance, in cells, from the edge up to which a code differs from the clamps.
pub const RANGE_CELLS: f64 = 4.0;
/// The code of the edge: water where the code is at or under it.
pub const CODE_ZERO: u8 = 128;
/// Codes a cell.
pub const CODE_PER_CELL: f64 = 32.0;

/// The width of a river in cells for a discharge `q` (m3/s) on cells of `cell_m` metres: the burn's trench width, at least one cell (GEN-165: `max(min(w, 32 cells), 1 cell)`).
pub fn drawn_width_cells(law: &WidthLaw, cell_m: f64, q: f64) -> f64 {
    trench_width_cells(law, cell_m, q).max(1.0)
}

/// The code of a signed distance in cells.
pub fn distance_code(d: f64) -> u8 {
    if d.is_nan() {
        return 255;
    }
    libm::round(f64::from(CODE_ZERO) + d * CODE_PER_CELL).clamp(0.0, 255.0) as u8
}

/// The signed distance, in cells, of each cell centre of the window `size` (cells wide, high) whose first cell is `origin` (face cell indices) to the edge of the nearest river: negative inside, zero on the
/// edge. A cell farther than [`RANGE_CELLS`] from every edge reads `f64::INFINITY` (its code is the far-outside one). The river is the channels' polylines with the width of `drawn_width_cells` at the
/// discharge interpolated along each segment (as the burn interpolates it); a channel without discharges is the one-cell river.
pub fn river_distance(channels: &[Channel], origin: (i64, i64), size: (usize, usize), law: &WidthLaw, cell_m: f64) -> Vec<f64> {
    river_distance_grid(channels, (origin.0 as f64, origin.1 as f64), 1.0, size, law, cell_m)
}

/// [`river_distance`] on a lattice that is not the cells: the texel `(x, y)` of the window lies at `origin + (x, y) * step` in face cell-index space (a terrain node whose samples are `step` cells apart, REND-059);
/// the distances are still in cells. With `step` 1 and an integer origin the texels are the cell centres and the result is that of [`river_distance`] bit for bit.
pub fn river_distance_grid(
    channels: &[Channel],
    origin: (f64, f64),
    step: f64,
    size: (usize, usize),
    law: &WidthLaw,
    cell_m: f64,
) -> Vec<f64> {
    assert!(step > 0.0 && step.is_finite(), "a positive sample step");
    let (w, h) = size;
    let mut out = vec![f64::INFINITY; w * h];
    let (ox, oy) = origin;
    for ch in channels {
        assert!(ch.points.len() == ch.levels.len() && !ch.points.is_empty(), "a level for every point");
        assert!(ch.q.is_empty() || ch.q.len() == ch.points.len(), "a discharge for every point, or none");
        assert!(ch.q.is_empty() || ch.q_unit == QUnit::Discharge, "the river width needs a discharge in m3/s, not an area");
        assert!(ch.q.iter().all(|q| q.is_finite()), "the discharges of a channel are finite");
        assert!(ch.points.iter().all(|p| p.0.is_finite() && p.1.is_finite()), "the points of a channel are finite");
        let q_of = |k: usize| if ch.q.is_empty() { 0.0 } else { ch.q[k] };
        let n = ch.points.len();
        // A single point is a segment of no length.
        let segments = if n == 1 { 1 } else { n - 1 };
        for k in 0..segments {
            let (p0, p1) = (ch.points[k], ch.points[(k + 1).min(n - 1)]);
            let (q0, q1) = (q_of(k), q_of((k + 1).min(n - 1)));
            let reach = drawn_width_cells(law, cell_m, q0).max(drawn_width_cells(law, cell_m, q1)) / 2.0 + RANGE_CELLS;
            // The texels (indices) a point within `reach` of the segment can have: one index of slack each way.
            let x_lo = (libm::floor((p0.0.min(p1.0) - reach - ox) / step) as i64).saturating_sub(1).max(0);
            let x_hi = (libm::ceil((p0.0.max(p1.0) + reach - ox) / step) as i64).saturating_add(1).min(w as i64 - 1);
            let y_lo = (libm::floor((p0.1.min(p1.1) - reach - oy) / step) as i64).saturating_sub(1).max(0);
            let y_hi = (libm::ceil((p0.1.max(p1.1) + reach - oy) / step) as i64).saturating_add(1).min(h as i64 - 1);
            let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
            let len2 = dx * dx + dy * dy;
            let half_at = |t: f64| drawn_width_cells(law, cell_m, q0 + (q1 - q0) * t) / 2.0;
            let (h0, h1) = (half_at(0.0), half_at(1.0));
            for y in y_lo..=y_hi {
                for x in x_lo..=x_hi {
                    let (px, py) = (ox + x as f64 * step, oy + y as f64 * step);
                    // The signed distance at the point of the segment with parameter t: its distance minus the half width there.
                    let sd_at = |t: f64| {
                        let (nx, ny) = (p0.0 + dx * t, p0.1 + dy * t);
                        libm::sqrt((px - nx) * (px - nx) + (py - ny) * (py - ny)) - half_at(t)
                    };
                    let sd = if h0 == h1 {
                        // A river of one width along the segment: the nearest point of the segment.
                        let t = if len2 > 0.0 { (((px - p0.0) * dx + (py - p0.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
                        let (nx, ny) = (p0.0 + dx * t, p0.1 + dy * t);
                        libm::sqrt((px - nx) * (px - nx) + (py - ny) * (py - ny)) - h0
                    } else {
                        // A river that widens along the segment is the union of discs the burn cuts along it (its edge is the envelope of the discs, not the nearest point's width): the minimum over the
                        // segment of `distance - half width`, a convex function of t (the width is concave in t), found by a fixed number of ternary steps (deterministic, the same in every window).
                        let (mut lo, mut hi) = (0.0f64, 1.0f64);
                        for _ in 0..50 {
                            let (m1, m2) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
                            if sd_at(m1) < sd_at(m2) {
                                hi = m2;
                            } else {
                                lo = m1;
                            }
                        }
                        sd_at(0.5 * (lo + hi)).min(sd_at(0.0)).min(sd_at(1.0))
                    };
                    let c = y as usize * w + x as usize;
                    if sd < out[c] {
                        out[c] = sd;
                    }
                }
            }
        }
    }
    // Beyond the range the distance is not kept (the same whatever window or channel set reached the cell).
    for d in &mut out {
        if *d > RANGE_CELLS {
            *d = f64::INFINITY;
        }
    }
    out
}

/// The 8-bit coverage codes of the window: [`distance_code`] of [`river_distance`] (255 where no river is within range).
pub fn river_coverage(channels: &[Channel], origin: (i64, i64), size: (usize, usize), law: &WidthLaw, cell_m: f64) -> Vec<u8> {
    river_distance(channels, origin, size, law, cell_m).into_iter().map(|d| if d.is_finite() { distance_code(d) } else { 255 }).collect()
}

/// The grid of the stored layer the channels live on: the face edge in L cells, the L tile edge in cells and the cell size in metres (the width law needs it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RiverGrid {
    pub cells: u32,
    pub tile: u32,
    pub cell_m: f64,
}

/// Where the plan gets the channels of an L tile: the runtime's reader of the channel graph (STRM-085) in the app, a list in tests. An error names the tile.
pub trait RiverSource {
    fn grid(&self) -> RiverGrid;
    /// The channels that can touch the core of L tile `(ti, tj)` of `face`, in the face's cell-index space.
    fn tile_channels(&self, face: planet_core::Face, ti: u32, tj: u32) -> Result<std::sync::Arc<Vec<Channel>>, String>;
    /// The identity of what the source reads (the commit of a reader): a cache built over another identity is cleared. 0 for a source with no identity.
    fn id(&self) -> u64 {
        0
    }
    /// Whether the source holds L tile `(ti, tj)` of `face` (REND-062): a river is drawn only over the tiles of the commit, whose ground its channels were burnt into. Every tile by default.
    fn has_tile(&self, _face: planet_core::Face, _ti: u32, _tj: u32) -> bool {
        true
    }
    /// Whether local cell `(i, j)` of `face` lies in a lake's wet mask (REND-065): a river ends at the lake's water and is not drawn through it. No lake by default.
    fn lake_wet(&self, _face: planet_core::Face, _i: i64, _j: i64) -> Result<bool, String> {
        Ok(false)
    }
}

/// The width, in samples of a node, below which a river is not drawn in it, and from which it is drawn at full strength.
pub const MIN_RIVER_SAMPLES: f64 = 0.5;
pub const FULL_RIVER_SAMPLES: f64 = 1.0;

/// What the river plan needs.
pub struct RiverRequest<'a> {
    pub source: &'a dyn RiverSource,
    pub law: &'a WidthLaw,
    /// Cells a terrain node's edge (its samples are `res + 1` a side).
    pub res: u32,
    /// A node carries a river while its drawn width is at least this many of the node's samples (below it nothing is drawn) and from [`full_samples`](RiverRequest::full_samples) on at full strength; between the two the wetness
    /// fades linearly with the width over the sample spacing (REND-059, architect 2026-11-12). Named parameters, [`MIN_RIVER_SAMPLES`] and [`FULL_RIVER_SAMPLES`]: the owner's look review tunes them.
    pub min_samples: f64,
    pub full_samples: f64,
    /// The layers of the coverage array a frame holds (the texture array's depth): when more nodes have a river, the nearest keep theirs and the rest are counted in [`RiverPlan::dropped`].
    pub max_layers: usize,
}

/// The river coverage of one terrain node: `size * size` codes (row-major, `size = res + 1`), one a sample of the node.
#[derive(Clone, Debug, PartialEq)]
pub struct RiverNode {
    /// Index into the terrain plan's nodes (the instance the river draws with).
    pub node: u32,
    pub size: u32,
    pub coverage: Vec<u8>,
    /// The strength of the node's river, 0 to 255 (255: full): the river's width in samples of the node between the request's two limits, linear.
    pub fade: u8,
}

/// The rivers of one frame (REND-059): only nodes with a river within range.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RiverPlan {
    pub nodes: Vec<RiverNode>,
    /// Nodes with a river that got no layer because the array was full (never silent: the plan text and the frame's stats carry it).
    pub dropped: u32,
}

/// Where the samples of terrain node `tile` (`res` cells an edge, `res + 1` samples) lie in the face's cell-index space on an L grid of `cells` cells an edge: the first sample's position and the distance between
/// samples, in cells. Exact (every factor is a power of two or an integer) while the face edge is at most 2^20 cells and the node level at most 28; it takes the bake's own face mapping (`TangentWarp`, the only production
/// one) for granted, as the L grid does. The node's face coordinate `s` (the bake's own mapping: cell `i` has its centre at `s = 2 (i + 1/2) / cells - 1`) is at the cell-index position `(s + 1) cells / 2 - 1/2`.
pub fn node_window(tile: planet_core::TileId, res: u32, cells: u32) -> ((f64, f64), f64) {
    let (s0, s1, t0, _) = tile.bounds();
    let half = f64::from(cells) / 2.0;
    let step = (s1 - s0) / f64::from(res) * half;
    (((s0 + 1.0) * half - 0.5, (t0 + 1.0) * half - 0.5), step)
}

/// The checks of a river request; the grid it reads.
fn check_request(req: &RiverRequest<'_>) -> Result<RiverGrid, String> {
    let grid = req.source.grid();
    if req.res == 0
        || req.max_layers == 0
        || !(req.min_samples.is_finite() && req.min_samples > 0.0 && req.full_samples.is_finite() && req.full_samples > req.min_samples)
    {
        return Err(format!(
            "the river plan needs a node of at least one cell, a layer and 0 < min samples < full samples (res {}, layers {}, min {}, full {})",
            req.res, req.max_layers, req.min_samples, req.full_samples
        ));
    }
    if grid.tile == 0 || grid.cells == 0 || !grid.cells.is_multiple_of(grid.tile) {
        return Err(format!("the river grid of {} cells a face edge is not a whole number of tiles of {} cells", grid.cells, grid.tile));
    }
    Ok(grid)
}

/// The river of one terrain node: its coverage codes (`(res + 1)^2`, row-major) and its strength. A function of the node's tile, the request and the channels alone (CON-30): never of the camera or the frame.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeCoverage {
    pub coverage: Vec<u8>,
    pub fade: u8,
}

/// Whether a node of this tile can carry no river whatever the channels are: the widest river is `2 * DISC_RADIUS_MAX` cells, and a node whose samples are farther apart than that over the minimum reads no tile.
fn cannot_carry(req: &RiverRequest<'_>, grid: &RiverGrid, tile: planet_core::TileId) -> bool {
    let (_, step) = node_window(tile, req.res, grid.cells);
    ((2 * planet_generators::bake::k7::DISC_RADIUS_MAX) as f64) < req.min_samples * step
}

/// The coverage of terrain node `tile`, or `None` when it carries no river (narrower than the request's minimum of samples, or no river within range of a sample). The channels come from the L tiles the node's window
/// meets widened by the widest trench and the code's range; a channel handed out by two tiles counts once. An error names the tile that failed.
fn node_coverage(req: &RiverRequest<'_>, grid: &RiverGrid, tile: planet_core::TileId) -> Result<Option<NodeCoverage>, String> {
    let per_edge = i64::from(grid.cells / grid.tile);
    let reach = reach_cells();
    let size = req.res as usize + 1;
    let (origin, step) = node_window(tile, req.res, grid.cells);
    if cannot_carry(req, grid, tile) {
        return Ok(None);
    }
    let span = f64::from(req.res) * step;
    // The tile of a position is its floor; a cell centre `i` owns `[i - 1/2, i + 1/2)`, so near a tile's upper edge this can pick the tile below: harmless because the reader widens a tile by the widest trench.
    let range = |o: f64| {
        let lo = (libm::floor((o - reach) / f64::from(grid.tile)) as i64).clamp(0, per_edge - 1);
        let hi = (libm::floor((o + span + reach) / f64::from(grid.tile)) as i64).clamp(0, per_edge - 1);
        (lo, hi)
    };
    let ((ti0, ti1), (tj0, tj1)) = (range(origin.0), range(origin.1));
    let mut seen = std::collections::HashSet::new();
    let mut channels: Vec<Channel> = Vec::new();
    for tj in tj0..=tj1 {
        for ti in ti0..=ti1 {
            if !req.source.has_tile(tile.face(), ti as u32, tj as u32) {
                continue;
            }
            for c in req.source.tile_channels(tile.face(), ti as u32, tj as u32)?.iter() {
                // The whole channel is the key: two pieces that share a start and a length are still two rivers.
                let key: Vec<u64> = c
                    .points
                    .iter()
                    .flat_map(|p| [p.0.to_bits(), p.1.to_bits()])
                    .chain(c.q.iter().map(|q| q.to_bits()))
                    .chain(c.levels.iter().map(|l| l.to_bits()))
                    .collect();
                if c.q.iter().any(|q| !q.is_finite()) {
                    return Err(format!("river tile {}/{ti}/{tj}: a channel carries a discharge that is not finite", tile.face().0));
                }
                if seen.insert(key) {
                    channels.push(c.clone());
                }
            }
        }
    }
    // A river narrower than a sample of the node is not drawn in it; the node's strength follows its widest river: linear in the width in samples between the two limits.
    let widest = |c: &Channel| {
        if c.q.is_empty() {
            1.0
        } else {
            c.q.iter().map(|&q| drawn_width_cells(req.law, grid.cell_m, q)).fold(1.0, f64::max)
        }
    };
    channels.retain(|c| widest(c) >= req.min_samples * step);
    let ratio = channels.iter().map(|c| widest(c) / step).fold(0.0, f64::max);
    let fade = (libm::round(255.0 * ((ratio - req.min_samples) / (req.full_samples - req.min_samples)).clamp(0.0, 1.0))) as u8;
    if channels.is_empty() || fade == 0 {
        return Ok(None);
    }
    let mut coverage: Vec<u8> = river_distance_grid(&channels, origin, step, (size, size), req.law, grid.cell_m)
        .into_iter()
        .map(|d| if d.is_finite() { distance_code(d) } else { 255 })
        .collect();
    // A river is drawn only over the source's tiles (REND-062) and ends at a lake's water (REND-065): a sample whose nearest cell lies in another tile or in a lake's wet mask is far from every river.
    let last = i64::from(grid.cells) - 1;
    for b in 0..size {
        let j = (libm::round(origin.1 + b as f64 * step) as i64).clamp(0, last);
        for a in 0..size {
            if coverage[b * size + a] == 255 {
                continue;
            }
            let i = (libm::round(origin.0 + a as f64 * step) as i64).clamp(0, last);
            // Only over the source's tiles (REND-062), and not through a lake.
            let t = i64::from(grid.tile);
            if !req.source.has_tile(tile.face(), (i / t) as u32, (j / t) as u32) || req.source.lake_wet(tile.face(), i, j)? {
                coverage[b * size + a] = 255;
            }
        }
    }
    if coverage.iter().all(|&c| c == 255) {
        return Ok(None);
    }
    Ok(Some(NodeCoverage { coverage, fade }))
}

/// More nodes than layers: the nearest keep theirs (ties by node index: deterministic, CON-03); the rest are counted, never silent.
fn choose_layers(req: &RiverRequest<'_>, mut out: Vec<(f64, RiverNode)>) -> RiverPlan {
    let total = out.len();
    if total > req.max_layers {
        out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.node.cmp(&b.1.node)));
        out.truncate(req.max_layers);
        out.sort_by_key(|(_, n)| n.node);
    }
    RiverPlan { nodes: out.into_iter().map(|(_, n)| n).collect(), dropped: (total - req.max_layers.min(total)) as u32 }
}

/// The river coverage of the nodes `nodes` (index in the terrain plan, tile and distance from the camera): a node whose widest river is narrower than the request's minimum of samples, or with no river within range of a
/// sample, carries none. An error names the tile that failed. Every node is computed afresh: the reference of [`plan_rivers_cached`].
pub fn plan_rivers(req: &RiverRequest<'_>, nodes: &[(u32, planet_core::TileId, f64)]) -> Result<RiverPlan, String> {
    let grid = check_request(req)?;
    let size = req.res + 1;
    let mut out: Vec<(f64, RiverNode)> = Vec::new();
    for &(index, tile, distance_m) in nodes {
        if let Some(c) = node_coverage(req, &grid, tile)? {
            out.push((distance_m, RiverNode { node: index, size, coverage: c.coverage, fade: c.fade }));
        }
    }
    Ok(choose_layers(req, out))
}

/// What a cached lookup of one tile found.
#[derive(Clone, Debug)]
pub enum Lookup {
    /// The tile's coverage is known: `None` for a node that carries no river.
    Ready(Option<std::sync::Arc<NodeCoverage>>),
    /// Not built yet.
    Missing,
}

/// What the cache has done (reported per frame by the viewer and the scenes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CacheCounters {
    pub hits: u64,
    pub misses: u64,
    pub builds: u64,
    pub evictions: u64,
}

struct CacheEntry {
    value: Option<std::sync::Arc<NodeCoverage>>,
    tick: u64,
    bytes: usize,
}

/// The coverage of terrain nodes by tile (REND-064): a node's coverage is a function of its tile alone, so a frame is a lookup. Bounded in bytes (the least recently used tile goes first; a tile larger than the bound is not
/// kept); negative results are kept too (most nodes carry no river). The cache is stamped with the source and the request (`adopt`): another commit, grid, width law, limits or node size clears it.
pub struct RiverCache {
    entries: std::collections::HashMap<planet_core::TileId, CacheEntry>,
    order: std::collections::BTreeMap<u64, planet_core::TileId>,
    tick: u64,
    bytes: usize,
    cap_bytes: usize,
    stamp: Option<u64>,
    counters: CacheCounters,
}

/// The bytes an entry costs: the codes and a fixed overhead (a negative result costs the overhead).
const ENTRY_OVERHEAD: usize = 64;

impl RiverCache {
    pub fn new(cap_bytes: usize) -> RiverCache {
        RiverCache {
            entries: std::collections::HashMap::new(),
            order: std::collections::BTreeMap::new(),
            tick: 0,
            bytes: 0,
            cap_bytes,
            stamp: None,
            counters: CacheCounters::default(),
        }
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn counters(&self) -> CacheCounters {
        self.counters
    }

    /// The identity of what a request's results depend on: the source (its commit), the grid, the width law, the limits and the node size. The layer count is not part of it (it only chooses among nodes).
    fn stamp_of(req: &RiverRequest<'_>, grid: &RiverGrid) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut put = |v: u64| h = (h ^ v).wrapping_mul(0x0000_0100_0000_01b3);
        for v in [
            req.source.id(),
            u64::from(grid.cells),
            u64::from(grid.tile),
            grid.cell_m.to_bits(),
            req.law.a.to_bits(),
            req.law.b.to_bits(),
            req.min_samples.to_bits(),
            req.full_samples.to_bits(),
            u64::from(req.res),
        ] {
            put(v);
        }
        h
    }

    /// Clears the cache when the request is not the one its entries were built for.
    pub fn adopt(&mut self, req: &RiverRequest<'_>) -> Result<u64, String> {
        let grid = check_request(req)?;
        let stamp = Self::stamp_of(req, &grid);
        if self.stamp != Some(stamp) {
            self.entries.clear();
            self.order.clear();
            self.bytes = 0;
            self.stamp = Some(stamp);
        }
        Ok(stamp)
    }

    /// [`RiverCache::insert`] for a build made under `stamp` (what [`RiverCache::adopt`] returned when the build was queued): dropped when the cache has been cleared for another request since, so a late build of the
    /// background fill never lands in a cache of another commit.
    pub fn insert_stamped(&mut self, stamp: u64, tile: planet_core::TileId, value: Option<std::sync::Arc<NodeCoverage>>) {
        if self.stamp == Some(stamp) {
            self.insert(tile, value);
            self.counters.builds += 1;
        }
    }

    /// The coverage of `tile` if the cache holds it (and it counts as used).
    pub fn lookup(&mut self, tile: planet_core::TileId) -> Lookup {
        self.tick += 1;
        let tick = self.tick;
        match self.entries.get_mut(&tile) {
            Some(e) => {
                self.order.remove(&e.tick);
                e.tick = tick;
                self.order.insert(tick, tile);
                self.counters.hits += 1;
                Lookup::Ready(e.value.clone())
            }
            None => {
                self.counters.misses += 1;
                Lookup::Missing
            }
        }
    }

    /// Keeps the coverage of `tile` (replacing an older entry), evicting the least recently used tiles over the byte bound.
    pub fn insert(&mut self, tile: planet_core::TileId, value: Option<std::sync::Arc<NodeCoverage>>) {
        let bytes = ENTRY_OVERHEAD + value.as_ref().map_or(0, |c| c.coverage.len());
        if let Some(old) = self.entries.remove(&tile) {
            self.order.remove(&old.tick);
            self.bytes -= old.bytes;
        }
        if bytes > self.cap_bytes {
            return;
        }
        self.tick += 1;
        self.order.insert(self.tick, tile);
        self.entries.insert(tile, CacheEntry { value, tick: self.tick, bytes });
        self.bytes += bytes;
        while self.bytes > self.cap_bytes {
            let Some((&oldest, &victim)) = self.order.iter().next() else { break };
            self.order.remove(&oldest);
            if let Some(e) = self.entries.remove(&victim) {
                self.bytes -= e.bytes;
                self.counters.evictions += 1;
            }
        }
    }
}

/// The result of a lookup-only plan: the plan of the nodes whose coverage is known, the tiles whose coverage is not built yet (a node without it draws no river this frame) and how many nodes that is.
#[derive(Clone, Debug, PartialEq)]
pub struct LookupPlan {
    pub plan: RiverPlan,
    pub missing: Vec<planet_core::TileId>,
    pub not_ready: u32,
}

/// The plan of a frame from the cache only (REND-064): no coverage is computed. A tile that is not in the cache is listed in `missing` (once, in node order) for the background fill ([`fill_river_cache`]) and its node
/// carries no river until it is built.
pub fn plan_rivers_lookup(
    req: &RiverRequest<'_>,
    nodes: &[(u32, planet_core::TileId, f64)],
    cache: &mut RiverCache,
) -> Result<LookupPlan, String> {
    let grid = check_request(req)?;
    cache.adopt(req)?;
    let size = req.res + 1;
    let (mut out, mut missing, mut not_ready) = (Vec::new(), Vec::new(), 0u32);
    for &(index, tile, distance_m) in nodes {
        if cannot_carry(req, &grid, tile) {
            continue;
        }
        match cache.lookup(tile) {
            Lookup::Ready(Some(c)) => out.push((distance_m, RiverNode { node: index, size, coverage: c.coverage.clone(), fade: c.fade })),
            Lookup::Ready(None) => {}
            Lookup::Missing => {
                not_ready += 1;
                if !missing.contains(&tile) {
                    missing.push(tile);
                }
            }
        }
    }
    Ok(LookupPlan { plan: choose_layers(req, out), missing, not_ready })
}

/// The coverage of one node built outside any cache (what a background fill computes, then hands to [`RiverCache::insert_stamped`]): `None` when the node carries no river.
pub fn build_river_node(req: &RiverRequest<'_>, tile: planet_core::TileId) -> Result<Option<std::sync::Arc<NodeCoverage>>, String> {
    let grid = check_request(req)?;
    Ok(node_coverage(req, &grid, tile)?.map(std::sync::Arc::new))
}

/// Builds the coverage of `tiles` and keeps it in the cache (the body of the background fill). Returns how many were built. An error names the tile that failed.
pub fn fill_river_cache(req: &RiverRequest<'_>, cache: &mut RiverCache, tiles: &[planet_core::TileId]) -> Result<usize, String> {
    let grid = check_request(req)?;
    cache.adopt(req)?;
    let mut built = 0;
    for &tile in tiles {
        let value = node_coverage(req, &grid, tile)?.map(std::sync::Arc::new);
        cache.insert(tile, value);
        cache.counters.builds += 1;
        built += 1;
    }
    Ok(built)
}

/// [`plan_rivers`] through the cache: a node's coverage is looked up and, when missing, built and kept at once. The same plan as [`plan_rivers`] bit for bit, whatever the cache held, evicted or never held.
pub fn plan_rivers_cached(
    req: &RiverRequest<'_>,
    nodes: &[(u32, planet_core::TileId, f64)],
    cache: &mut RiverCache,
) -> Result<RiverPlan, String> {
    let grid = check_request(req)?;
    cache.adopt(req)?;
    let size = req.res + 1;
    let mut out: Vec<(f64, RiverNode)> = Vec::new();
    for &(index, tile, distance_m) in nodes {
        if cannot_carry(req, &grid, tile) {
            continue;
        }
        let value = match cache.lookup(tile) {
            Lookup::Ready(v) => v,
            Lookup::Missing => {
                let v = node_coverage(req, &grid, tile)?.map(std::sync::Arc::new);
                cache.insert(tile, v.clone());
                cache.counters.builds += 1;
                v
            }
        };
        if let Some(c) = value {
            out.push((distance_m, RiverNode { node: index, size, coverage: c.coverage.clone(), fade: c.fade }));
        }
    }
    Ok(choose_layers(req, out))
}

/// How far from a node's window a channel can matter: the widest half trench and the code's range.
pub fn reach_cells() -> f64 {
    planet_generators::bake::k7::DISC_RADIUS_MAX as f64 + RANGE_CELLS
}

impl RiverPlan {
    /// The plan as lines of text: `rivers <dropped>` and one `rnode <node> <hex of the codes>` a node (the coverage is part of the plan's text, so a recorded frame replays the same pixels).
    pub fn to_text(&self) -> String {
        let mut s = format!("rivers {}\n", self.dropped);
        for n in &self.nodes {
            let hex: String = n.coverage.iter().map(|c| format!("{c:02x}")).collect();
            s += &format!("rnode {} {} {hex}\n", n.node, n.fade);
        }
        // No trailing newline: the caller adds the separators.
        s.pop();
        s
    }

    /// Reads the lines [`to_text`](RiverPlan::to_text) writes (the first is the `rivers` line).
    pub fn from_lines(lines: &[Vec<&str>]) -> Result<RiverPlan, String> {
        let mut plan: Option<RiverPlan> = None;
        for t in lines {
            match t.as_slice() {
                ["rivers", dropped] => {
                    plan = Some(RiverPlan {
                        nodes: Vec::new(),
                        dropped: dropped.parse().map_err(|e| format!("bad dropped count '{dropped}': {e}"))?,
                    })
                }
                ["rnode", node, fade, hex] => {
                    let p = plan.as_mut().ok_or("an rnode line before the rivers line")?;
                    if hex.len() % 2 != 0 {
                        return Err(format!("rnode {node}: an odd number of hex digits"));
                    }
                    let coverage = (0..hex.len() / 2)
                        .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|e| format!("rnode {node}: bad hex: {e}")))
                        .collect::<Result<Vec<u8>, String>>()?;
                    let size = libm::round(libm::sqrt(coverage.len() as f64)) as u32;
                    if (size as usize) * (size as usize) != coverage.len() {
                        return Err(format!("rnode {node}: {} codes are not a square", coverage.len()));
                    }
                    p.nodes.push(RiverNode {
                        node: node.parse().map_err(|e| format!("bad node index '{node}': {e}"))?,
                        size,
                        coverage,
                        fade: fade.parse().map_err(|e| format!("bad fade '{fade}': {e}"))?,
                    });
                }
                other => return Err(format!("cannot parse river line {other:?}")),
            }
        }
        plan.ok_or_else(|| "no rivers line".to_string())
    }
}
