//! The runtime's layer source over a commit of the new chain (item 15 PR 5a, STRM-083; architect, 2026-11-10): the L tiles (kind 13, layer version 3) read through a [`Commit`], with no fill channel.
//!
//! - [`CommitLayer`] is a [`LayerSource`]: tiles decoded once while they stay in a bounded least-recently-used cache, each held with its min/max pyramid. A tile the commit does not name is a tile with no
//!   land (`None`). A tile whose key in the commit is not the one this build forms from the commit's spec, a damaged record, or a change outside the layer's range is an error naming the tile.
//! - The surface is `B + L`: the change is the stored core as it is; nothing is added back (a basin is a lake, not a depth).
//! - [`PairedLayer`] holds the commit that is drawn. A new commit becomes the drawn one only when every tile the caller needs of it reads and its basin tables are in the store (architect, 2026-11-10, A3 section 7.4,
//!   CON-22): until then the previous commit is drawn, and a frame never takes L from one commit and lakes from another.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};

use planet_core::Face;
use planet_generators::bake::tile::{TileGrid, TileId as PassTile, TileResult, TileStats, QUANT_STEP_M};
use planet_generators::erosion_layer::{LayerSource, LayerTile};

use crate::basin_table::KIND_BASIN_TABLE;
use crate::global_store::GlobalWorld;
use crate::l_writer::{read_l_tile, Commit};
use crate::layer_l::LTileSpec;
use crate::layer_tiles::{layer_tile_id, KIND_LAYER_TILE};
use crate::pack::TileStore;
use crate::relief_store::face_tile;

/// The range of the change the counts of a tile of L can hold (`i16` steps of [`QUANT_STEP_M`]): the layer's global range when no better one is known.
pub fn l_change_range() -> (f64, f64) {
    let limit = f64::from(i16::MAX) * f64::from(QUANT_STEP_M);
    (-limit, limit)
}

type BuiltTiles = (HashMap<PassTile, Arc<LayerTile>>, VecDeque<PassTile>);

/// The layer of one commit as a [`LayerSource`].
pub struct CommitLayer {
    store: Arc<TileStore>,
    world: Arc<GlobalWorld>,
    res: u32,
    spec: LTileSpec,
    commit: Commit,
    built: Mutex<BuiltTiles>,
    /// A lock for each tile being decoded: workers asking for one tile together decode it once.
    inflight: Mutex<HashMap<PassTile, Arc<Mutex<()>>>>,
    decodes: std::sync::atomic::AtomicU64,
    capacity: usize,
    range: (f64, f64),
}

impl CommitLayer {
    /// The layer of `commit` in `store` beside the bake `world`, with `capacity` decoded tiles cached and `range` the change's range ([`l_change_range`] when nothing narrower is known).
    pub fn with_commit(store: Arc<TileStore>, world: Arc<GlobalWorld>, commit: Commit, capacity: usize, range: (f64, f64)) -> CommitLayer {
        let spec = commit.spec.l_spec();
        CommitLayer {
            store,
            world,
            res: commit.spec.res,
            spec,
            commit,
            built: Mutex::new((HashMap::new(), VecDeque::new())),
            inflight: Mutex::new(HashMap::new()),
            decodes: 0.into(),
            capacity: capacity.max(1),
            range,
        }
    }

    /// Opens the layer of the commit in `dir` (the folder of the tile packs the writer wrote into, which holds [`crate::l_writer::COMMIT_FILE`]): an error naming the folder when it holds no commit,
    /// and when the first tile of the commit does not read under the keys of its spec (a store written for another world or build is refused here, naming both keys).
    pub fn open(dir: &Path, world: Arc<GlobalWorld>, capacity: usize) -> Result<CommitLayer, String> {
        let commit = Commit::read(dir)?.ok_or_else(|| {
            format!(
                "{}: no commit record ({}): the layer of the new chain is read through the commit its bake wrote last",
                dir.display(),
                crate::l_writer::COMMIT_FILE
            )
        })?;
        let store = TileStore::open(dir, world.params()).map_err(|e| format!("{}: {e}", dir.display()))?;
        let layer = CommitLayer::with_commit(Arc::new(store), world, commit, capacity, l_change_range());
        if let Some(&(f, i, j)) = layer.commit.tiles.keys().next() {
            layer.tile(Face(f), i, j)?;
        }
        Ok(layer)
    }

    /// The commit this layer reads.
    pub fn commit(&self) -> &Commit {
        &self.commit
    }

    /// Tiles decoded so far (a hit is not a decode; a tile that left the cache and comes back is).
    pub fn decodes(&self) -> u64 {
        self.decodes.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The reader of this commit's channel graph, with `capacity_bytes` cached ([`crate::river_read::DEFAULT_CACHE_BYTES`]): the rivers are read under the same commit as the tiles and the basin tables (STRM-085, CON-22). An error when the commit holds none.
    pub fn rivers(&self, capacity_bytes: usize) -> Result<crate::river_read::RiverReader, String> {
        crate::river_read::RiverReader::new(self.store.clone(), self.commit.clone(), capacity_bytes)
    }

    /// The lakes of this commit (REND-058): the basin tables and the tiles' lake masks, read under the same commit as the tiles and the rivers (CON-22), with `capacity` decoded masks cached.
    pub fn lakes(&self, capacity: usize) -> Result<crate::lake_read::CommitLakes, String> {
        crate::lake_read::CommitLakes::new(self.store.clone(), self.world.clone(), self.commit.clone(), capacity)
    }

    /// Whether the record of a tile the commit names is in the store (a tile the commit does not name is no land: present).
    pub fn tile_present(&self, id: PassTile) -> Result<bool, String> {
        let Some(&key) = self.commit.tiles.get(&(id.face.0, id.ti, id.tj)) else { return Ok(true) };
        let tile = layer_tile_id(&self.spec.grid, id).map_err(|e| e.to_string())?;
        Ok(self.store.get(KIND_LAYER_TILE, tile, key).map_err(|e| e.to_string())?.is_some())
    }

    /// Whether the basin tables the commit names are in the store (the lakes of the same commit are ready to be read).
    pub fn tables_present(&self) -> bool {
        (0..6u8)
            .all(|f| self.store.get(KIND_BASIN_TABLE, face_tile(Face(f)), self.commit.tables[usize::from(f)]).is_ok_and(|r| r.is_some()))
    }
}

impl LayerSource for CommitLayer {
    fn grid(&self) -> &TileGrid {
        &self.spec.grid
    }

    fn tile(&self, face: Face, ti: u32, tj: u32) -> Result<Option<Arc<LayerTile>>, String> {
        let id = PassTile { face, ti, tj };
        {
            let mut g = self.built.lock().expect("built lock");
            if let Some(t) = g.0.get(&id).cloned() {
                g.1.retain(|x| *x != id);
                g.1.push_back(id);
                return Ok(Some(t));
            }
        }
        let lock = self.inflight.lock().expect("inflight lock").entry(id).or_default().clone();
        let _one = lock.lock().expect("tile lock");
        let found = {
            let mut g = self.built.lock().expect("built lock");
            let t = g.0.get(&id).cloned();
            if t.is_some() {
                g.1.retain(|x| *x != id);
                g.1.push_back(id);
            }
            t
        };
        if let Some(t) = found {
            return Ok(Some(t));
        }
        let Some(stored) = read_l_tile(&self.store, &self.world, self.res, &self.spec, &self.commit, id).map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        let result = TileResult {
            id,
            core: stored.core,
            deposited: Vec::new(),
            fill: Vec::new(),
            heights: Vec::new(),
            stats: TileStats {
                land_cells: stored.stats.land_cells as usize,
                thermal_passes: 0,
                thermal_moving_at_cap: 0,
                eroded_m3: 0.0,
                deposited_m3: 0.0,
                exported_m3: 0.0,
                clamped_cells: stored.clamped_cells as usize,
            },
        };
        let tile = Arc::new(LayerTile::new(Arc::new(result), self.spec.grid.tile)?);
        let [lo, hi] = tile.range();
        if f64::from(lo) < self.range.0 || f64::from(hi) > self.range.1 {
            return Err(format!(
                "layer tile {}/{ti}/{tj}: its change [{lo}, {hi}] m lies outside the layer's range [{}, {}] m",
                face.0, self.range.0, self.range.1
            ));
        }
        self.decodes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut g = self.built.lock().expect("built lock");
        if g.0.insert(id, tile.clone()).is_none() {
            g.1.push_back(id);
        }
        while g.0.len() > self.capacity {
            if let Some(old) = g.1.pop_front() {
                g.0.remove(&old);
            }
        }
        Ok(Some(tile))
    }

    fn global_range(&self) -> (f64, f64) {
        self.range
    }

    fn id(&self) -> u64 {
        // The commit id covers the spec and every key: another commit is another layer for the fields that read it.
        self.commit.id
    }
}

/// The commit that is drawn, and the swap to a new one (architect, 2026-11-10, A3 section 7.4, CON-22).
pub struct PairedLayer {
    drawn: Mutex<Arc<CommitLayer>>,
}

impl PairedLayer {
    pub fn new(first: CommitLayer) -> PairedLayer {
        PairedLayer { drawn: Mutex::new(Arc::new(first)) }
    }

    /// The commit a frame draws: a frame takes this once and reads all of its tiles through it, so it never mixes two commits.
    pub fn drawn(&self) -> Arc<CommitLayer> {
        self.drawn.lock().expect("drawn lock").clone()
    }

    /// Offers a new commit: it becomes the drawn one when every tile in `needed` reads under it (they are decoded into its cache on the way, so the first frame of the new commit has them) and its basin
    /// tables are in the store. Returns whether it swapped; `Ok(false)` while a part is not ready (the previous commit stays drawn), an error when a part is damaged or made for another spec.
    pub fn try_swap(&self, next: CommitLayer, needed: &[PassTile]) -> Result<bool, String> {
        if !next.tables_present() {
            return Ok(false);
        }
        for t in needed {
            if !next.tile_present(*t)? {
                return Ok(false);
            }
        }
        for t in needed {
            next.tile(t.face, t.ti, t.tj)?;
        }
        *self.drawn.lock().expect("drawn lock") = Arc::new(next);
        Ok(true)
    }
}
