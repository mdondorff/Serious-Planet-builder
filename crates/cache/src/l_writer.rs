//! The bake writer of the new chain (GEN-167, STRM-082; architect D17/D18/D33/D34): K7 over a set of tiles writes the L records (kind 13, layer version 3) with each tile's lake water mask in the record's
//! water section, the basin tables of the six faces (kind 19) from the lake producer's result, and last a commit record that names them all.
//!
//! - **Input**: `B` and the regional change as functions, the channels per window from a [`ChannelFeed`], and the lake producer's [`Lakes`] (the records, and the water and ring lists the mask reads).
//!   The unit of the discharge is `m3/s`: a feed that carries areas is refused (D33), because the width law turns `Q_w` into metres.
//! - **Water section**: the mask of the tile is decided on `zl = B + L` with `L` as stored (quantised), so the runtime reads the surface the mask was made on; a tile that no lake touches takes the empty section.
//! - **Commit** ([`Commit`]): a plain text file, [`COMMIT_FILE`], beside the layer manifest, written last and atomically. It names the commit id and the key of each L tile and each basin table of the commit.
//!   A reader reads through it only ([`read_l_tile`], [`read_basin_table`]): a tile or table stored under another key is never composed with it, and a bake killed before the commit leaves the previous commit current.
//!   The commit id is a hash of the keys, so a commit with other keys has another id.

use std::collections::BTreeMap;
use std::path::Path;

use planet_core::{Face, Vec3};
use planet_generators::bake::k7::{k7_tile, Channel, K7Params, WidthLaw};
use planet_generators::bake::lakes::{Lake, Lakes};
use planet_generators::bake::tile::{dequantise, quantise, TileGrid, TileId as PassTile};
use planet_generators::bake::water_mask::{self, Status, Window, RING_STEPS};

use crate::basin_table::{basin_table_records, load_basin_table, BasinSpec};
use crate::global_store::GlobalWorld;
use crate::graph_store::{graph_block_records, GraphSpec};
use crate::key::CacheKey;
use crate::layer_l::{l_tile_key, l_tile_record, load_l_tile, LTile, LTileSpec, LTileStats};
use crate::layer_store::LayerStoreError;
use crate::pack::{PackWriter, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_generators::bake::channel_graph::ChannelGraph;
use planet_generators::bake::Grid;

/// The file name of the commit record (beside the layer manifest).
pub const COMMIT_FILE: &str = "layer-commit.txt";
/// Version of the commit record's layout.
pub const COMMIT_VERSION: u32 = 3;

/// The channels K7 burns into a window, in face cell-index space with the discharge `Q_w` at each point.
pub trait ChannelFeed {
    /// Whether `Q_w` is a discharge in m3/s (and not an area): the width law needs metres, a feed in area units is refused by the writer.
    fn q_is_discharge(&self) -> bool;
    /// The segments with a point near the window of `w x w` local cells whose cell `(0, 0)` is the face's local cell `(i0, j0)`.
    fn channels_near(&self, face: Face, i0: i64, j0: i64, w: i64) -> Vec<Channel>;
}

/// The in-memory feed (fixtures and regions): channels made once, e.g. by `k7::channels_from_hydro`, and the segments with a point within `MARGIN` local cells of a window handed out per window.
pub struct MemoryFeed {
    /// The face the channels are on (their points are in that face's local cell-index space); other faces get none.
    pub face: Face,
    pub channels: Vec<Channel>,
    /// Whether the `q` of the channels is a discharge in m3/s (see [`ChannelFeed::q_is_discharge`]).
    pub discharge: bool,
}

/// How far outside a window a segment's point may lie for the segment to be handed out (the line enters the window between its points).
const MARGIN: f64 = 16.0;

impl ChannelFeed for MemoryFeed {
    fn q_is_discharge(&self) -> bool {
        self.discharge
    }
    fn channels_near(&self, face: Face, i0: i64, j0: i64, w: i64) -> Vec<Channel> {
        if face != self.face {
            return Vec::new();
        }
        let (x0, x1, y0, y1) = (i0 as f64 - MARGIN, (i0 + w) as f64 + MARGIN, j0 as f64 - MARGIN, (j0 + w) as f64 + MARGIN);
        self.channels.iter().filter(|c| c.points.iter().any(|&(x, y)| x >= x0 && x <= x1 && y >= y0 && y <= y1)).cloned().collect()
    }
}

/// What the writer needs besides the tiles.
pub struct LBake<'a> {
    pub world: &'a GlobalWorld,
    pub res: u32,
    pub spec: LTileSpec,
    pub basin: BasinSpec,
    pub base: &'a (dyn Fn(Vec3) -> f64 + Sync),
    pub regional: &'a (dyn Fn(Vec3) -> f64 + Sync),
    pub feed: &'a dyn ChannelFeed,
    pub lakes: &'a Lakes,
    /// H1 cells a face edge; the local grid is a whole number of local cells to each.
    pub h1_cells: usize,
    /// The H1 cell (index into the hydrology's grid) of the H1 cell `(i, j)` of a face. A window at a face edge holds H1 cells outside the face: the resolver answers them on the neighbouring face (through the
    /// direction on the sphere), or the lake's water across the seam reads as dry there and the mask has a strip of up to 8 cells missing along it. `None` reads as dry. The planet-scale resolver is the caller's (PR 7).
    pub h1_cell: &'a dyn Fn(u8, i64, i64) -> Option<usize>,
    /// The face a lake's table is filed under.
    pub face_of: &'a dyn Fn(&Lake) -> Face,
    pub threads: usize,
    /// The channel graph to store beside the L tiles (PR 7), if the bake has one.
    pub graph: Option<GraphInput<'a>>,
}

/// The channel graph of a bake and what its blocks are made from.
pub struct GraphInput<'a> {
    pub spec: GraphSpec,
    pub graph: &'a ChannelGraph,
    /// The id of each segment of the graph (`Hydro::segments[k].id`).
    pub segment_ids: &'a [u64],
    pub grid: &'a Grid,
    /// The `(face, i, j)` of a cell of `grid` on the face's H1 grid.
    pub cell_xy: &'a dyn Fn(u32) -> (u8, i64, i64),
}

/// What a reader needs of the L spec to form the keys of the commit's tiles: the grid, the seed and sea level of K7's standard parameters, the flood, the width law and the identities of the base and
/// of the other inputs. A bake whose K7 parameters are not the standard ones with these values is refused by the writer (a reader could not rebuild its keys).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommitSpec {
    /// The `res` the records' keys were made with (the bake's resolution, as the caller named it).
    pub res: u32,
    pub cells: u32,
    pub tile: u32,
    pub halo: u32,
    pub radius_m: f64,
    pub seed: u64,
    pub sea_level_m: f64,
    pub canonical_flood: bool,
    pub width_a: f64,
    pub width_b: f64,
    pub base_id: u64,
    pub inputs_id: u64,
}

impl CommitSpec {
    /// The part of `spec` the commit records, or an error when `spec`'s K7 parameters are not describable by it.
    pub fn of(spec: &LTileSpec, res: u32) -> Result<CommitSpec, String> {
        let k = &spec.k7;
        let c = CommitSpec {
            res,
            cells: spec.grid.cells as u32,
            tile: spec.grid.tile as u32,
            halo: spec.grid.halo as u32,
            radius_m: spec.grid.radius_m,
            seed: k.pass2.seed,
            sea_level_m: k.pass2.sea_level,
            canonical_flood: k.canonical_flood,
            width_a: k.width.a,
            width_b: k.width.b,
            base_id: spec.base_id,
            inputs_id: spec.inputs_id,
        };
        if c.l_spec() != *spec {
            return Err("the K7 parameters are not the standard ones for this seed and sea level (with the commit's flood and width law): a reader could not rebuild the keys of the tiles".into());
        }
        Ok(c)
    }

    /// The L spec this describes: K7's standard parameters for the seed and sea level, with the flood and the width law.
    pub fn l_spec(&self) -> LTileSpec {
        let mut k7 = K7Params::standard(self.seed, self.sea_level_m);
        k7.canonical_flood = self.canonical_flood;
        k7.width = WidthLaw { a: self.width_a, b: self.width_b };
        LTileSpec {
            grid: TileGrid { cells: self.cells as usize, tile: self.tile as usize, halo: self.halo as usize, radius_m: self.radius_m },
            k7,
            base_id: self.base_id,
            inputs_id: self.inputs_id,
        }
    }

    fn to_line(self) -> String {
        format!(
            "spec {} {} {} {} {:016x} {} {:016x} {} {:016x} {:016x} {} {}",
            self.res,
            self.cells,
            self.tile,
            self.halo,
            self.radius_m.to_bits(),
            self.seed,
            self.sea_level_m.to_bits(),
            u8::from(self.canonical_flood),
            self.width_a.to_bits(),
            self.width_b.to_bits(),
            self.base_id,
            self.inputs_id
        )
    }

    fn parse(parts: &[&str]) -> Result<CommitSpec, String> {
        if parts.len() != 12 {
            return Err(format!("a spec line holds 12 numbers, this one {}", parts.len()));
        }
        let n = |i: usize| parts[i].parse::<u64>().map_err(|e| format!("spec field {i}: {e}"));
        let h = |i: usize| u64::from_str_radix(parts[i], 16).map(f64::from_bits).map_err(|e| format!("spec field {i}: {e}"));
        Ok(CommitSpec {
            res: n(0)? as u32,
            cells: n(1)? as u32,
            tile: n(2)? as u32,
            halo: n(3)? as u32,
            radius_m: h(4)?,
            seed: n(5)?,
            sea_level_m: h(6)?,
            canonical_flood: n(7)? != 0,
            width_a: h(8)?,
            width_b: h(9)?,
            base_id: n(10)?,
            inputs_id: n(11)?,
        })
    }
}

/// The commit: the spec and the keys of the records that are read together.
#[derive(Clone, Debug, PartialEq)]
pub struct Commit {
    pub id: u64,
    /// What the keys of the tiles were made from.
    pub spec: CommitSpec,
    /// The key of the basin table of each face.
    pub tables: [CacheKey; 6],
    /// The key of each L tile, by `(face, ti, tj)`.
    pub tiles: BTreeMap<(u8, u32, u32), CacheKey>,
    /// The channel graph of the commit (PR 7): the H1 cells a face edge (0 when the commit has none) and the key of each block that holds a piece, by `(face, bi, bj)`.
    pub graph_h1_cells: u32,
    pub graph: BTreeMap<(u8, u32, u32), CacheKey>,
}

fn fnv(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h = (*h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
}

impl Commit {
    fn id_of(
        spec: &CommitSpec,
        tables: &[CacheKey; 6],
        tiles: &BTreeMap<(u8, u32, u32), CacheKey>,
        graph_h1_cells: u32,
        graph: &BTreeMap<(u8, u32, u32), CacheKey>,
    ) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        fnv(&mut h, spec.to_line().as_bytes());
        fnv(&mut h, &graph_h1_cells.to_le_bytes());
        for (&(f, i, j), k) in graph {
            fnv(&mut h, &[0xe0, f]);
            fnv(&mut h, &i.to_le_bytes());
            fnv(&mut h, &j.to_le_bytes());
            fnv(&mut h, &k.0);
        }
        for k in tables {
            fnv(&mut h, &k.0);
        }
        for (&(f, i, j), k) in tiles {
            fnv(&mut h, &[f]);
            fnv(&mut h, &i.to_le_bytes());
            fnv(&mut h, &j.to_le_bytes());
            fnv(&mut h, &k.0);
        }
        h
    }

    pub fn new(
        spec: CommitSpec,
        tables: [CacheKey; 6],
        tiles: BTreeMap<(u8, u32, u32), CacheKey>,
        graph_h1_cells: u32,
        graph: BTreeMap<(u8, u32, u32), CacheKey>,
    ) -> Commit {
        Commit { id: Commit::id_of(&spec, &tables, &tiles, graph_h1_cells, &graph), spec, tables, tiles, graph_h1_cells, graph }
    }

    pub fn to_text(&self) -> String {
        let mut t = format!("l-commit {COMMIT_VERSION}\ncommit_id {:016x}\n{}\n", self.id, self.spec.to_line());
        for (f, k) in self.tables.iter().enumerate() {
            t.push_str(&format!("table {f} {k}\n"));
        }
        for (&(f, i, j), k) in &self.tiles {
            t.push_str(&format!("tile {f} {i} {j} {k}\n"));
        }
        if self.graph_h1_cells > 0 {
            t.push_str(&format!("graph_cells {}\n", self.graph_h1_cells));
        }
        for (&(f, i, j), k) in &self.graph {
            t.push_str(&format!("graph {f} {i} {j} {k}\n"));
        }
        t
    }

    /// The commit of `text`: an error naming the line when it is not one this build reads, or when the id does not match the keys.
    pub fn parse(text: &str) -> Result<Commit, String> {
        let mut lines = text.lines();
        let head = lines.next().ok_or("the commit record is empty")?;
        if head != format!("l-commit {COMMIT_VERSION}") {
            return Err(format!("'{head}': this build reads l-commit {COMMIT_VERSION}"));
        }
        let mut id: Option<u64> = None;
        let mut spec: Option<CommitSpec> = None;
        let mut tables: [Option<CacheKey>; 6] = [None; 6];
        let mut tiles = BTreeMap::new();
        let mut graph = BTreeMap::new();
        let mut graph_h1_cells = 0u32;
        let key = |s: &str, l: &str| CacheKey::from_hex(s).ok_or_else(|| format!("'{l}': '{s}' is not a key"));
        let num = |s: &str, l: &str| s.parse::<u32>().map_err(|e| format!("'{l}': {e}"));
        for l in lines.filter(|l| !l.trim().is_empty()) {
            let p: Vec<&str> = l.split(' ').collect();
            match p.as_slice() {
                ["commit_id", v] => id = Some(u64::from_str_radix(v, 16).map_err(|e| format!("'{l}': {e}"))?),
                ["spec", rest @ ..] => spec = Some(CommitSpec::parse(rest).map_err(|e| format!("'{l}': {e}"))?),
                ["table", f, k] => {
                    let f = num(f, l)? as usize;
                    if f >= 6 || tables[f].replace(key(k, l)?).is_some() {
                        return Err(format!("'{l}': a face outside 0..6 or a table named twice"));
                    }
                }
                ["tile", f, i, j, k] => {
                    if num(f, l)? >= 6 {
                        return Err(format!("'{l}': a face outside 0..6"));
                    }
                    let at = (num(f, l)? as u8, num(i, l)?, num(j, l)?);
                    if tiles.insert(at, key(k, l)?).is_some() {
                        return Err(format!("'{l}': a tile named twice"));
                    }
                }
                ["graph_cells", n] => {
                    if graph_h1_cells != 0 {
                        return Err(format!("'{l}': graph_cells named twice"));
                    }
                    graph_h1_cells = num(n, l)?;
                }
                ["graph", f, i, j, k] => {
                    if num(f, l)? >= 6 {
                        return Err(format!("'{l}': a face outside 0..6"));
                    }
                    let at = (num(f, l)? as u8, num(i, l)?, num(j, l)?);
                    if graph.insert(at, key(k, l)?).is_some() {
                        return Err(format!("'{l}': a graph block named twice"));
                    }
                }
                _ => return Err(format!("'{l}' is not a line of the commit record")),
            }
        }
        let id = id.ok_or("the commit record has no commit_id")?;
        let mut t = [CacheKey::NONE; 6];
        for (f, k) in tables.iter().enumerate() {
            t[f] = k.ok_or_else(|| format!("the commit record names no table for face {f}"))?;
        }
        let spec = spec.ok_or("the commit record has no spec line")?;
        if !graph.is_empty() && graph_h1_cells == 0 {
            return Err("the commit record names graph blocks but no graph_cells line".into());
        }
        let c = Commit::new(spec, t, tiles, graph_h1_cells, graph);
        if c.id != id {
            return Err(format!("the commit id {id:016x} does not match its keys (they hash to {:016x}): the record is damaged", c.id));
        }
        Ok(c)
    }

    /// The commit in `dir`: `Ok(None)` when none was written (a bake killed before its commit, or no bake), an error naming the file when it cannot be read.
    pub fn read(dir: &Path) -> Result<Option<Commit>, String> {
        let path = dir.join(COMMIT_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => Commit::parse(&text).map(Some).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Writes the commit to `dir` atomically (a temporary file, then a rename over [`COMMIT_FILE`]).
    pub fn write(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let tmp = dir.join(format!("{COMMIT_FILE}.tmp-{}", std::process::id()));
        std::fs::write(&tmp, self.to_text()).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, dir.join(COMMIT_FILE)).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("{}: {e}", dir.join(COMMIT_FILE).display())
        })
    }
}

/// The L tile of `commit`: `Ok(None)` when the commit does not hold the tile; an error naming both keys when the commit names a key other than the one this build and `spec` make for the tile (another
/// base, parameters or version: the commit is not read with it), and the errors of [`load_l_tile`] otherwise.
pub fn read_l_tile(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    spec: &LTileSpec,
    commit: &Commit,
    id: PassTile,
) -> Result<Option<LTile>, LayerStoreError> {
    let Some(&named) = commit.tiles.get(&(id.face.0, id.ti, id.tj)) else { return Ok(None) };
    let want = l_tile_key(world, res, spec, id)?;
    if named != want {
        return Err(LayerStoreError(format!(
            "commit {:016x} names key {named} for L tile {}/{}/{}, this reader's spec makes {want}: the commit was written for another base, parameters or version",
            commit.id, id.face.0, id.ti, id.tj
        )));
    }
    load_l_tile(store, world, res, spec, id)?.map(Some).ok_or_else(|| {
        LayerStoreError(format!(
            "commit {:016x} names L tile {}/{}/{} (key {named}) but the store holds no record of it: a commit is written after its records, so the store is incomplete or is not the one the commit was written for",
            commit.id, id.face.0, id.ti, id.tj
        ))
    })
}

/// The basin table of `face` of `commit`, read under the key the commit names (an error naming both keys when the reader's spec makes another).
pub fn read_basin_table(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    spec: &BasinSpec,
    commit: &Commit,
    face: Face,
) -> Result<Option<Vec<Lake>>, String> {
    let named = commit.tables[usize::from(face.0)];
    let want = crate::basin_table::basin_key(world, res, spec, face);
    if named != want {
        return Err(format!(
            "commit {:016x} names key {named} for the basin table of face {}, this reader's spec makes {want}: the commit was written for another spec",
            commit.id, face.0
        ));
    }
    load_basin_table(store, world, res, spec, face).map_err(|e| e.to_string())
}

/// The status of an H1 cell of a face for the mask: from the producer's sorted lists.
fn status_of(lakes: &Lakes, cell: Option<usize>) -> Status {
    let Some(c) = cell else { return Status::Dry };
    let c = c as u32;
    if let Ok(k) = lakes.water.binary_search_by_key(&c, |e| e.0) {
        Status::Water(lakes.lakes[lakes.water[k].1 as usize].id)
    } else if let Ok(k) = lakes.ring.binary_search_by_key(&c, |e| e.0) {
        Status::Ring(lakes.lakes[lakes.ring[k].1 as usize].id)
    } else {
        Status::Dry
    }
}

/// The record of one tile: K7, the mask on `B + L` as stored, `l_tile_record`.
fn l_tile(b: &LBake, id: PassTile) -> Result<crate::pack::Record, String> {
    let g = &b.spec.grid;
    let (w, t, h) = (g.window(), g.tile, g.halo);
    let (i0, j0) = (i64::from(id.ti) * t as i64 - h as i64, i64::from(id.tj) * t as i64 - h as i64);
    let channels = b.feed.channels_near(id.face, i0, j0, w as i64);
    if channels.iter().any(|c| c.q_unit != planet_generators::bake::k7::QUnit::Discharge) {
        return Err(format!("tile {}/{}/{}: a channel carries Q_w as an area, not a discharge in m3/s (D33)", id.face.0, id.ti, id.tj));
    }
    let r = k7_tile(g, id, b.base, b.regional, &channels, &b.spec.k7, b.threads);
    let last = &r.pits.last().expect("K7 reports the pits after the whole pass").1;
    let stats = LTileStats {
        land_cells: last.land_cells as u64,
        burned_cells: r.burned as u64,
        pit_cells: last.pit_cells as u64,
        pit_p50_m: last.p50_m,
        pit_p99_m: last.p99_m,
        pit_max_m: last.max_m,
    };
    let ratio = (g.cells / b.h1_cells) as i64;
    let status = |i: i64, j: i64| status_of(b.lakes, (b.h1_cell)(id.face.0, i, j));
    // A tile no lake touches takes the empty section without the mask: no H1 cell of its window is water or ring.
    let touches = (i0.div_euclid(ratio)..=(i0 + w as i64 - 1).div_euclid(ratio))
        .any(|i| (j0.div_euclid(ratio)..=(j0 + w as i64 - 1).div_euclid(ratio)).any(|j| status(i, j) != Status::Dry));
    let water = if touches {
        // `L` as the record stores it: f32 metres on the 0.25 m grid, so the mask is made on the surface the runtime reads.
        let change: Vec<f32> = r.z.iter().zip(&r.b).map(|(z, b)| (z - b) as f32).collect();
        let stored = dequantise(&quantise(&change));
        let zl: Vec<f64> = r.b.iter().zip(&stored).map(|(b, l)| b + f64::from(*l)).collect();
        let level = |lid: u64| match b.lakes.lakes.binary_search_by_key(&lid, |l| l.id) {
            Ok(k) => b.lakes.lakes[k].level_m,
            Err(_) => f64::NEG_INFINITY,
        };
        let labels = water_mask::lake_mask(&Window { zl: &zl, w, origin: (i0, j0), ratio }, h, t, &status, &level);
        water_mask::encode(&labels, t).map_err(|e| format!("tile {}/{}/{}: {e}", id.face.0, id.ti, id.tj))?
    } else {
        Vec::new()
    };
    l_tile_record(b.world, b.res, &b.spec, id, &r.change, &stats, &water).map_err(|e| e.to_string())
}

/// Bakes `tiles` into the store in `dir` and commits them: the basin tables of the six faces, the L tile records with their water sections, then the commit record, written last. A refusal or a failure
/// before the commit leaves the previous commit current (the records appended so far are content-keyed and are read only through a commit that names them).
pub fn bake_l(dir: &Path, b: &LBake, tiles: &[PassTile]) -> Result<Commit, String> {
    if !b.feed.q_is_discharge() {
        return Err("the channel feed carries Q_w as an area, not a discharge in m3/s: the width law needs metres (D33); run the hydrology with runoff".into());
    }
    let g = &b.spec.grid;
    if g.halo < usize::from(RING_STEPS) {
        return Err(format!("the halo is {} cells, the mask's ring needs at least {RING_STEPS}", g.halo));
    }
    if b.h1_cells == 0 || !g.cells.is_multiple_of(b.h1_cells) {
        return Err(format!("{} local cells a face edge are not a whole number of the {} H1 cells", g.cells, b.h1_cells));
    }
    if g.cells / b.h1_cells > usize::from(RING_STEPS) {
        return Err(format!(
            "{} local cells to an H1 cell: the ring reaches {RING_STEPS} steps, one H1 cell at most",
            g.cells / b.h1_cells
        ));
    }
    if b.lakes.lakes.iter().any(|l| l.id == water_mask::NO_LAKE) {
        return Err("a lake has the id u64::MAX, the dry label of the mask".into());
    }
    if !b.lakes.water.windows(2).all(|w| w[0].0 < w[1].0) || !b.lakes.ring.windows(2).all(|w| w[0].0 < w[1].0) {
        return Err("the producer's water and ring lists are not sorted with one lake a cell".into());
    }
    if b.lakes.ring.iter().any(|r| b.lakes.water.binary_search_by_key(&r.0, |e| e.0).is_ok()) {
        return Err("a cell is in the producer's water list and in its ring list".into());
    }
    let mut ids: Vec<PassTile> = tiles.to_vec();
    ids.sort_unstable_by_key(|t| (t.face.0, t.ti, t.tj));
    ids.dedup();
    let mut writer = PackWriter::open(dir, b.world.params(), DEFAULT_MAX_PACK_BYTES).map_err(|e| e.to_string())?;
    let mut tables = [CacheKey::NONE; 6];
    for (f, r) in basin_table_records(b.world, b.res, &b.basin, &b.lakes.lakes, b.face_of).iter().enumerate() {
        tables[f] = r.key;
        writer.append(r).map_err(|e| e.to_string())?;
    }
    let mut keys = BTreeMap::new();
    for id in ids {
        let r = l_tile(b, id)?;
        keys.insert((id.face.0, id.ti, id.tj), r.key);
        writer.append(&r).map_err(|e| e.to_string())?;
    }
    let (mut graph_keys, mut graph_cells) = (BTreeMap::new(), 0u32);
    if let Some(gi) = &b.graph {
        if gi.graph.q_unit != planet_generators::bake::hydro::QUnit::Discharge {
            return Err("the channel graph carries Q_w as an area, not a discharge in m3/s (D33)".into());
        }
        if gi.spec.h1_cells as usize != b.h1_cells || gi.spec.basin != b.basin || gi.spec.h1_cells % gi.spec.basin.tiles_per_edge != 0 {
            return Err(format!(
                "the graph was made for {} H1 cells a face edge with {} blocks, the bake has {} H1 cells and {} blocks (or the specs differ, or the cells do not divide)",
                gi.spec.h1_cells, gi.spec.basin.tiles_per_edge, b.h1_cells, b.basin.tiles_per_edge
            ));
        }
        graph_cells = gi.spec.h1_cells;
        for r in graph_block_records(b.world, b.res, &gi.spec, gi.graph, gi.segment_ids, gi.grid, gi.cell_xy) {
            let (face, bi, bj) = crate::graph_store::block_of(&gi.spec, &r)?;
            graph_keys.insert((face, bi, bj), r.key);
            writer.append(&r).map_err(|e| e.to_string())?;
        }
    }
    writer.finish().map_err(|e| e.to_string())?;
    let commit = Commit::new(CommitSpec::of(&b.spec, b.res)?, tables, keys, graph_cells, graph_keys);
    commit.write(dir)?;
    Ok(commit)
}
