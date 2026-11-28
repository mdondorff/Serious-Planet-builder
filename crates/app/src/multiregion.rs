//! Several region commits read together (VIEW-079): every ready region of a regions file over one in-memory base. The commits hold disjoint L tiles of one grid; a read of a tile goes to the
//! commit that holds it, and every other tile has no change. The decoded tiles of all commits share one cache budget, and the lakes of two commits keep apart in the water's identity.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use planet_cache::l_layer::CommitLayer;
use planet_cache::lake_read::CommitLakes;
use planet_core::Face;
use planet_generators::bake::tile::TileGrid;
use planet_generators::erosion_layer::{LayerSource, LayerTile};
use planet_generators::global::ReadError;
use planet_generators::lake_water::{LakeCell, LakeMaskSource};

type TileKey = (u8, u32, u32);

/// The tile grid two commits must share to be read together: cells and tile edge (the halo and radius follow the layout and the body).
fn same_grid(a: &TileGrid, b: &TileGrid) -> bool {
    a.cells == b.cells && a.tile == b.tile && a.halo == b.halo && a.radius_m.to_bits() == b.radius_m.to_bits()
}

/// The owner of every tile of `tiles` (the commit's index): refuses a tile two commits hold.
fn owners(tiles: &[Vec<TileKey>]) -> Result<HashMap<TileKey, usize>, String> {
    let mut owner = HashMap::new();
    for (k, ts) in tiles.iter().enumerate() {
        for t in ts {
            if let Some(other) = owner.insert(*t, k) {
                return Err(format!(
                    "regions {other} and {k} both hold L tile {}/{}/{}: boxes that overlap cannot open together",
                    t.0, t.1, t.2
                ));
            }
        }
    }
    Ok(owner)
}

/// The layer of several commits (VIEW-079): a tile is read from the commit that holds it, through one cache of `capacity` decoded tiles for all of them.
pub struct MultiLayer {
    layers: Vec<CommitLayer>,
    owner: HashMap<TileKey, usize>,
    grid: TileGrid,
    range: (f64, f64),
    id: u64,
    capacity: usize,
    cache: Mutex<TileCache>,
}

/// The decoded tiles and their order of use (the oldest first).
type TileCache = (HashMap<TileKey, Arc<LayerTile>>, VecDeque<TileKey>);

impl MultiLayer {
    /// `layers` opened with a cache of one tile each: this layer's cache of `capacity` tiles is the budget of all (12 GB rule).
    pub fn new(layers: Vec<CommitLayer>, capacity: usize) -> Result<MultiLayer, String> {
        let first = layers.first().ok_or("no commit to read")?;
        let grid = *first.grid();
        if let Some(k) = layers.iter().position(|l| !same_grid(l.grid(), &grid)) {
            return Err(format!("region {k}'s commit is of another tile grid than region 0's: they cannot open together"));
        }
        let owner = owners(&layers.iter().map(|l| l.commit().tiles.keys().copied().collect()).collect::<Vec<_>>())?;
        let range = layers.iter().fold((0.0_f64, 0.0_f64), |(lo, hi), l| {
            let (a, b) = l.global_range();
            (lo.min(a), hi.max(b))
        });
        // The identity of the set: the commits in order (another set, or another order of the same, is another layer for the fields that read it).
        let id = layers.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, l| (h ^ l.id()).wrapping_mul(0x0000_0100_0000_01b3));
        Ok(MultiLayer { layers, owner, grid, range, id, capacity: capacity.max(1), cache: Mutex::new((HashMap::new(), VecDeque::new())) })
    }

    /// The decoded tiles the shared cache holds now.
    pub fn cached(&self) -> usize {
        self.cache.lock().expect("the layer cache").0.len()
    }
}

impl LayerSource for MultiLayer {
    fn grid(&self) -> &TileGrid {
        &self.grid
    }

    fn tile(&self, face: Face, ti: u32, tj: u32) -> Result<Option<Arc<LayerTile>>, String> {
        let key = (face.0, ti, tj);
        let Some(&k) = self.owner.get(&key) else { return Ok(None) };
        if let Some(t) = self.cache.lock().expect("the layer cache").0.get(&key).cloned() {
            return Ok(Some(t));
        }
        // Decoded outside the lock (CON-22): the commit's own lock keeps two decodes of one tile apart.
        let Some(t) = self.layers[k].tile(face, ti, tj)? else { return Ok(None) };
        let mut c = self.cache.lock().expect("the layer cache");
        let (tiles, order) = &mut *c;
        if let std::collections::hash_map::Entry::Vacant(v) = tiles.entry(key) {
            v.insert(t.clone());
            order.push_back(key);
            while order.len() > self.capacity {
                if let Some(old) = order.pop_front() {
                    tiles.remove(&old);
                }
            }
        }
        Ok(Some(t))
    }

    fn global_range(&self) -> (f64, f64) {
        self.range
    }

    fn id(&self) -> u64 {
        self.id
    }
}

/// The lakes of several commits (VIEW-079): a cell is read from the commit that holds its tile, and a lake's index is offset past the lakes of the commits before it on its face, so
/// two commits' lakes are two water bodies.
pub struct MultiLakes {
    parts: Vec<Arc<CommitLakes>>,
    owner: HashMap<TileKey, usize>,
    cells: usize,
    tile: i64,
    /// `offset[k][face]`: the lakes of commits `0..k` on that face.
    offset: Vec<[u32; 6]>,
}

impl MultiLakes {
    pub fn new(parts: Vec<Arc<CommitLakes>>) -> Result<MultiLakes, String> {
        let first = parts.first().ok_or("no commit to read")?;
        let (cells, tile) = (first.cells(), i64::from(first.commit().spec.tile));
        if parts.iter().any(|p| p.cells() != cells || i64::from(p.commit().spec.tile) != tile) {
            return Err("the regions' lake grids differ: they cannot open together".into());
        }
        let owner = owners(&parts.iter().map(|p| p.commit().tiles.keys().copied().collect()).collect::<Vec<_>>())?;
        let mut offset = Vec::with_capacity(parts.len());
        let mut sum = [0u32; 6];
        for p in &parts {
            offset.push(sum);
            for f in 0..6u8 {
                sum[usize::from(f)] += p.table(Face(f)).len() as u32;
            }
        }
        Ok(MultiLakes { parts, owner, cells, tile, offset })
    }

    /// The commit's lakes that hold cell `(i, j)` of `face`, with its index.
    fn part(&self, face: Face, i: i64, j: i64) -> Option<usize> {
        let n = self.cells as i64;
        if i < 0 || j < 0 || i >= n || j >= n {
            return None;
        }
        self.owner.get(&(face.0, (i / self.tile) as u32, (j / self.tile) as u32)).copied()
    }

    /// Whether the cell is lake water (the rivers end there, REND-065).
    pub fn wet(&self, face: Face, i: i64, j: i64) -> Result<bool, String> {
        match self.part(face, i, j) {
            Some(k) => Ok(self.parts[k].lake_cell(face, i, j)?.is_some()),
            None => Ok(false),
        }
    }
}

impl LakeMaskSource for MultiLakes {
    fn cells(&self) -> usize {
        self.cells
    }

    fn lake_at(&self, face: Face, i: i64, j: i64) -> Result<Option<LakeCell>, ReadError> {
        let Some(k) = self.part(face, i, j) else { return Ok(None) };
        let cell = self.parts[k].lake_at(face, i, j)?;
        Ok(cell.map(|c| LakeCell { index: c.index + self.offset[k][usize::from(c.face.0)], ..c }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: VIEW-079
    #[test]
    fn a_tile_two_regions_hold_is_refused_and_disjoint_ones_each_have_their_owner() {
        let a = vec![(1, 84, 8), (1, 85, 8)];
        let b = vec![(1, 90, 8)];
        let o = owners(&[a.clone(), b]).unwrap();
        assert_eq!((o[&(1, 84, 8)], o[&(1, 85, 8)], o[&(1, 90, 8)]), (0, 0, 1));
        assert!(!o.contains_key(&(1, 86, 8)), "a tile no region holds has no owner: no change there");
        let err = owners(&[a.clone(), vec![(1, 85, 8)]]).unwrap_err();
        assert!(err.contains("1/85/8") && err.contains("overlap"), "{err}");
    }
}
