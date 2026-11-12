//! The runtime's read of a commit's lakes (REND-058, item 15 PR 4a): the lake water mask of each L tile (its water section) and the basin tables of the six faces, both read under the one commit the layer
//! and the rivers are read under (CON-22). [`CommitLakes`] is the [`LakeMaskSource`] of [`planet_generators::lake_water::LakeProducer`]: decoded masks are kept in a bounded least-recently-used cache.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use planet_core::Face;
use planet_generators::bake::lakes::Lake;
use planet_generators::bake::tile::TileId as PassTile;
use planet_generators::bake::water_mask::{decode, NO_LAKE};
use planet_generators::global::ReadError;
use planet_generators::lake_water::{LakeCell, LakeMaskSource};

use crate::basin_table::BasinSpec;
use crate::global_store::GlobalWorld;
use crate::l_writer::{read_basin_table, read_l_tile, Commit};
use crate::layer_l::LTileSpec;
use crate::pack::TileStore;

/// Decoded tile masks kept by default (a mask is 8 bytes a cell: 4.7 MB for a tile of 768 cells).
pub const DEFAULT_MASK_TILES: usize = 32;

/// A tile's decoded mask: `None` for a tile whose water section is empty (no lake: nothing is decoded or kept but the entry).
type Mask = Option<Arc<Vec<u64>>>;
type Masks = (HashMap<PassTile, Mask>, VecDeque<PassTile>);

/// The lakes of one commit.
pub struct CommitLakes {
    store: Arc<TileStore>,
    world: Arc<GlobalWorld>,
    commit: Commit,
    spec: LTileSpec,
    /// The basin table of each face, in id order (as stored).
    tables: [Vec<Lake>; 6],
    masks: Mutex<Masks>,
    capacity: usize,
}

impl CommitLakes {
    /// The lakes of `commit` in `store`: the six basin tables are read now (an error names the face whose table the commit names and the store lacks or holds damaged).
    pub fn new(store: Arc<TileStore>, world: Arc<GlobalWorld>, commit: Commit, capacity: usize) -> Result<CommitLakes, String> {
        let s = commit.spec;
        let basin = BasinSpec {
            hydro: planet_generators::bake::hydro::HydroParams::default(),
            tile_cells: s.tile,
            tiles_per_edge: s.cells / s.tile.max(1),
            base_id: s.base_id,
            inputs_id: s.inputs_id,
        };
        let mut tables: [Vec<Lake>; 6] = Default::default();
        for (f, t) in tables.iter_mut().enumerate() {
            *t = read_basin_table(&store, &world, s.res, &basin, &commit, Face(f as u8))?.ok_or_else(|| {
                format!(
                    "commit {:016x} names the basin table of face {f} but the store holds no record of it: the store is incomplete or is not the one the commit was written for",
                    commit.id
                )
            })?;
        }
        Ok(CommitLakes {
            spec: s.l_spec(),
            store,
            world,
            commit,
            tables,
            masks: Mutex::new((HashMap::new(), VecDeque::new())),
            capacity: capacity.max(1),
        })
    }

    /// The commit these lakes are read under.
    pub fn commit(&self) -> &Commit {
        &self.commit
    }

    /// The basin table of `face`, in id order.
    pub fn table(&self, face: Face) -> &[Lake] {
        &self.tables[usize::from(face.0) % 6]
    }

    /// The decoded mask of a tile (`tile x tile` lake ids, [`NO_LAKE`] for a dry cell); `None` when the commit does not hold the tile or the tile holds no lake water (an empty section).
    pub fn mask(&self, id: PassTile) -> Result<Mask, String> {
        {
            let mut g = self.masks.lock().expect("mask lock");
            if let Some(m) = g.0.get(&id).cloned() {
                g.1.retain(|x| *x != id);
                g.1.push_back(id);
                return Ok(m);
            }
        }
        let Some(tile) =
            read_l_tile(&self.store, &self.world, self.commit.spec.res, &self.spec, &self.commit, id).map_err(|e| e.to_string())?
        else {
            return Ok(None);
        };
        let m = if tile.water.is_empty() {
            None
        } else {
            Some(Arc::new(
                decode(&tile.water, self.spec.grid.tile)
                    .map_err(|e| format!("L tile {}/{}/{} of commit {:016x}: {e}", id.face.0, id.ti, id.tj, self.commit.id))?,
            ))
        };
        let mut g = self.masks.lock().expect("mask lock");
        if g.0.insert(id, m.clone()).is_none() {
            g.1.push_back(id);
        }
        while g.0.len() > self.capacity {
            if let Some(old) = g.1.pop_front() {
                g.0.remove(&old);
            }
        }
        Ok(m)
    }

    /// Masks held now (a tile without lake water holds none).
    pub fn masks_held(&self) -> usize {
        self.masks.lock().expect("mask lock").0.values().filter(|m| m.is_some()).count()
    }

    /// The table and index of a lake id: its own face's table first, then the others (a lake is filed under the face of its floor cell).
    fn find(&self, face: Face, lake: u64) -> Option<(Face, u32, f64)> {
        let order = std::iter::once(face.0).chain((0..6u8).filter(|&f| f != face.0));
        for f in order {
            let t = &self.tables[usize::from(f)];
            if let Ok(k) = t.binary_search_by_key(&lake, |l| l.id) {
                return Some((Face(f), k as u32, t[k].level_m));
            }
        }
        None
    }

    /// The lake under local cell `(i, j)` of `face` with the face whose table holds it.
    pub fn lake_cell(&self, face: Face, i: i64, j: i64) -> Result<Option<(Face, LakeCell)>, String> {
        let g = &self.spec.grid;
        let (n, t) = (g.cells as i64, g.tile as i64);
        if i < 0 || j < 0 || i >= n || j >= n {
            return Ok(None);
        }
        let id = PassTile { face, ti: (i / t) as u32, tj: (j / t) as u32 };
        let Some(mask) = self.mask(id)? else { return Ok(None) };
        let lake = mask[((j % t) * t + i % t) as usize];
        if lake == NO_LAKE {
            return Ok(None);
        }
        let (f, index, level_m) = self.find(face, lake).ok_or_else(|| {
            format!(
                "L tile {}/{}/{} of commit {:016x} marks cell ({i}, {j}) wet with lake {lake:016x}, which no basin table of the commit lists",
                face.0, id.ti, id.tj, self.commit.id
            )
        })?;
        Ok(Some((f, LakeCell { face: f, index, level_m })))
    }
}

impl LakeMaskSource for CommitLakes {
    fn cells(&self) -> usize {
        self.spec.grid.cells
    }

    fn lake_at(&self, face: Face, i: i64, j: i64) -> Result<Option<LakeCell>, ReadError> {
        self.lake_cell(face, i, j).map(|c| c.map(|(_, l)| l)).map_err(ReadError::Source)
    }
}
