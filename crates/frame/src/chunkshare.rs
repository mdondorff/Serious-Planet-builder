//! What a frame knows of the chunks (STRM-039): which of its nodes read chunk detail, how many of those are still the base because their
//! chunk is not ready (the stand-in share, the figure the speed-envelope scene reads), and the weight with which a landed chunk's change fades
//! in. GPU-free: the renderer takes the weight, this decides it.

use planet_core::TileId;
use planet_generators::chunkfield::ChunkKey;
use std::collections::HashMap;

/// What the frame asks of the chunk field: the tile level its pyramid starts at, the chunks a tile reads and whether one is ready.
pub trait ChunkView: Send + Sync {
    /// The coarsest tile level that reads chunk detail (`PYRAMID_TOP_LEVEL` by default).
    fn top_level(&self) -> u8;
    /// The chunks `tile` reads, its own first; empty for a tile above the top level.
    fn reads(&self, tile: TileId) -> Vec<ChunkKey>;
    fn is_ready(&self, key: ChunkKey) -> bool;
}

/// The names of the per-frame log columns of the chunk share, in [`ChunkShare::values`] order. Stable: the speed-envelope scene reads them.
pub const CHUNK_COLUMNS: [&str; 2] = ["chunk_nodes", "chunk_stand_in"];

/// The plan's nodes that read chunk detail and those among them whose own chunk is not ready.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkShare {
    /// Nodes at or finer than the top level: the nodes that would show chunk detail.
    pub chunk_nodes: usize,
    /// Of those, the nodes whose own chunk is not ready, so they draw the base (the stand-in).
    pub chunk_stand_in: usize,
}

impl ChunkShare {
    /// The share of `nodes` under `view`.
    pub fn of(nodes: impl IntoIterator<Item = TileId>, view: &dyn ChunkView) -> ChunkShare {
        let mut s = ChunkShare::default();
        for tile in nodes {
            let reads = view.reads(tile);
            if let Some(own) = reads.first() {
                s.chunk_nodes += 1;
                s.chunk_stand_in += usize::from(!view.is_ready(*own));
            }
        }
        s
    }

    /// The values in the order of [`CHUNK_COLUMNS`].
    pub fn values(&self) -> [u64; 2] {
        [self.chunk_nodes as u64, self.chunk_stand_in as u64]
    }

    /// The stand-in fraction of the nodes that read chunk detail (0 with none).
    pub fn fraction(&self) -> f64 {
        if self.chunk_nodes == 0 {
            0.0
        } else {
            self.chunk_stand_in as f64 / self.chunk_nodes as f64
        }
    }
}

/// The fade-in of landed chunks (STRM-039, STRM-047): a chunk's change enters a tile with a weight that rises from 0 to 1 over `fade_ms` of the
/// injected clock, as a plain f32 (CON-11). The clock of a chunk starts when its change is first drawn, that is when the first tile reading it
/// is uploaded after the chunk landed ([`ChunkFade::shown`]), not when it landed: a rebuild that takes longer than the fade would otherwise
/// show the whole change in one frame, and every tile of the chunk shares the one clock so their shared edges stay closed. Until then the
/// chunk weighs 0, like a chunk that is not ready; a chunk the fade has not seen land, but that is ready, weighs 1 (it was there before the
/// view looked). A chunk that lands again without having left (a new version) keeps its state: the tiles under it do not drop to the base.
#[derive(Clone, Debug)]
pub struct ChunkFade {
    fade_ms: u64,
    /// The chunks that landed and since when their change is drawn (`None`: not drawn yet).
    shown_ms: HashMap<ChunkKey, Option<u64>>,
}

impl ChunkFade {
    /// `chunk_fade_ms` is 1,500 by default (M8 criterion 3, amended by the owner 2026-10-24): the change of a landing chunk fades in over about
    /// 1.5 s. The largest entry step measured on the shared store (58.3 m, `m8-chunk-fade` design 6a) then moves a vertex by 0.65 m per frame at
    /// 60 fps, which is 0.20 px on screen at the distance where it enters; the criterion is the screen step.
    pub const DEFAULT_FADE_MS: u64 = 1_500;

    pub fn new(fade_ms: u64) -> ChunkFade {
        ChunkFade { fade_ms, shown_ms: HashMap::new() }
    }

    /// A chunk landed: its change weighs 0 until a tile that reads it is uploaded ([`ChunkFade::shown`]). One that is known already (it landed
    /// again as a new version) keeps its state.
    pub fn landed(&mut self, key: ChunkKey, _now_ms: u64) {
        self.shown_ms.entry(key).or_insert(None);
    }

    /// A tile that reads `key` was uploaded at `now_ms`: the first such upload after the chunk landed starts the fade of its change.
    pub fn shown(&mut self, key: ChunkKey, now_ms: u64) {
        if let Some(slot @ None) = self.shown_ms.get_mut(&key) {
            *slot = Some(now_ms);
        }
    }

    /// The tile `tile` is drawn at `now_ms` with the mesh `resident_key`, the plan expecting `planned_key` (the key a tile has when it was built
    /// against the chunks' current versions, STRM-038): when they are the same the change of every chunk it reads is being drawn, and the fade of
    /// those that landed starts. A tile that is still the one built before a landing (its key is the old one) starts nothing, so the clock never
    /// runs for a change that is not on screen.
    pub fn drawn(&mut self, view: &dyn ChunkView, tile: TileId, resident_key: u64, planned_key: u64, now_ms: u64) {
        if resident_key == planned_key {
            for k in view.reads(tile) {
                self.shown(k, now_ms);
            }
        }
    }

    /// A chunk left RAM: it fades in again if it lands again.
    pub fn evicted(&mut self, key: ChunkKey) {
        self.shown_ms.remove(&key);
    }

    /// The weight of the change of `key` at `now_ms`: linear from the landing, 1 once `fade_ms` have passed.
    pub fn weight(&self, key: ChunkKey, ready: bool, now_ms: u64) -> f32 {
        if !ready {
            return 0.0;
        }
        match self.shown_ms.get(&key) {
            None => 1.0,
            Some(_) if self.fade_ms == 0 => 1.0,
            Some(None) => 0.0,
            Some(&Some(at)) => (now_ms.saturating_sub(at) as f64 / self.fade_ms as f64).min(1.0) as f32,
        }
    }

    /// The weight of a tile (STRM-047): the minimum of the weights of the chunks it reads, so the tiles on either side of a chunk border rise
    /// together (the cells a tile reads cross borders). A tile that reads none (above the top level) has no change plane, and weighs 1: its draw
    /// is the final height, bit for bit.
    pub fn tile_weight(&self, view: &dyn ChunkView, tile: TileId, now_ms: u64) -> f32 {
        view.reads(tile).into_iter().map(|k| self.weight(k, view.is_ready(k), now_ms)).fold(1.0, f32::min)
    }
}

/// The [`ChunkView`] of a chunk manager: the tiles' reads and the ready set it holds.
pub struct ManagerView(pub std::sync::Arc<planet_streaming::chunks::ChunkManager>);

impl ChunkView for ManagerView {
    fn top_level(&self) -> u8 {
        self.0.top_level()
    }
    fn reads(&self, tile: TileId) -> Vec<ChunkKey> {
        self.0.reads(tile)
    }
    fn is_ready(&self, key: ChunkKey) -> bool {
        self.0.is_ready(key)
    }
}

/// The chunks to prefetch (STRM-042): the chunk one chunk edge ahead of the camera along its travel over the ground, and the chunk the
/// predicted position is in, when they are not the chunk under the camera; empty when the camera is not predicted to travel (`predicted` is
/// `None`: it is stationary, has just teleported, or only rotates, which the estimator never sees), or only dives. The first is the one the
/// camera needs next, requested a chunk's travel time before it is wanted (at 100 m/s through a 5 km chunk, about 49 s).
pub fn prefetch_chunks(position: planet_core::PlanetFixed, predicted: Option<planet_core::PlanetFixed>, radius_m: f64) -> Vec<ChunkKey> {
    use planet_core::cube::TangentWarp;
    let Some(predicted) = predicted else { return Vec::new() };
    let key_of = |p: planet_core::Vec3| {
        let t = TileId::from_direction(&TangentWarp, p.normalized(), planet_generators::chunk::CHUNK_GRID_LEVEL);
        (t.face().0, t.x(), t.y())
    };
    let up = position.0.normalized();
    let travel = predicted.0 - position.0;
    let along = travel - up * travel.dot(up);
    if along.length() < 1e-6 {
        return Vec::new();
    }
    let edge_m = radius_m * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << planet_generators::chunk::CHUNK_GRID_LEVEL);
    let own = key_of(position.0);
    let mut out = Vec::new();
    for p in [position.0 + along.normalized() * edge_m, predicted.0] {
        let k = key_of(p);
        if k != own && !out.contains(&k) {
            out.push(k);
        }
    }
    out
}
