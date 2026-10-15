//! The residency set R (STRM-015, ADR 0031 point 1): the tiles worth keeping for a camera *position*.
//!
//! A pure function of the position, the LOD parameters (LOD-008 distances and height bounds) and the horizon cut
//! (`LodParams::cull_horizon`). It is the position-only selection (every direction, no frustum) plus all ancestors
//! of its leaves. No view direction can enter: the functions take a `PlanetFixed`, not a `Camera`. The draw set D
//! of any view from the same position is a subset (LOD-007); the scheduler uses R to decide what is wanted and
//! what is cancelled (STRM-005), D only to order requests.

use crate::lod::{select_nodes_cached, select_nodes_field_cached, NodeSelection, SelectionCache, TileBounds};
use crate::LodParams;
use planet_core::cube::FaceMapping;
use planet_core::{PlanetFixed, TileId};
use planet_generators::height::HeightField;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResidencyEntry {
    pub tile: TileId,
    /// A leaf of the selection (drawn when in view) as opposed to an ancestor kept for the fallback.
    pub leaf: bool,
    /// Distance from the camera to the tile's bounding volume, metres (an ancestor: that of its nearest leaf).
    pub distance_m: f64,
}

/// R: entries in ascending tile-id order, closed under parents.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResidencySet {
    pub tiles: Vec<ResidencyEntry>,
    /// The bounds the selection cached for the tiles of R (empty for R over flat terrain and for sets made by hand): what the
    /// frame path's cover reads to cull and measure like the plan without evaluating a field (STRM-016).
    pub bounds: TileBounds,
}

impl ResidencySet {
    /// R from a position-only selection: its leaves and all their ancestors.
    pub fn from_selection(selection: &NodeSelection) -> ResidencySet {
        let mut all: BTreeMap<TileId, (bool, f64)> = BTreeMap::new();
        for n in &selection.nodes {
            all.insert(n.tile, (true, n.distance_m));
        }
        for n in &selection.nodes {
            let mut t = n.tile.parent();
            while let Some(a) = t {
                let e = all.entry(a).or_insert((false, f64::INFINITY));
                e.1 = e.1.min(n.distance_m);
                t = a.parent();
            }
        }
        ResidencySet {
            tiles: all.into_iter().map(|(tile, (leaf, distance_m))| ResidencyEntry { tile, leaf, distance_m }).collect(),
            bounds: TileBounds::default(),
        }
    }

    pub fn contains(&self, tile: TileId) -> bool {
        self.tiles.binary_search_by_key(&tile, |e| e.tile).is_ok()
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// `(tile, distance)` pairs for the scheduler's residency class.
    pub fn wants(&self) -> Vec<(TileId, f64)> {
        self.tiles.iter().map(|e| (e.tile, e.distance_m)).collect()
    }
}

/// R over flat terrain.
pub fn residency_set(map: &dyn FaceMapping, position: PlanetFixed, p: &LodParams, cache: &mut SelectionCache) -> ResidencySet {
    ResidencySet::from_selection(&select_nodes_cached(map, position, p, cache))
}

/// R over the terrain of `field` (per-node height bounds).
pub fn residency_set_field(
    map: &dyn FaceMapping,
    position: PlanetFixed,
    field: &dyn HeightField,
    p: &LodParams,
    cache: &mut SelectionCache,
) -> ResidencySet {
    let mut r = ResidencySet::from_selection(&select_nodes_field_cached(map, position, field, p, cache));
    // Every tile of R was visited by the selection, so its geometry and volume are in the cache: the bounds are lookups.
    r.bounds = cache.bounds_of(r.tiles.iter().map(|e| e.tile), position, map, field, p);
    r
}
