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

/// The fade-in of landed chunks (STRM-039): a chunk's change enters a tile with a weight that rises from 0 to 1 over `fade_ms` of the injected
/// clock after the chunk landed, as a plain f32 (CON-11). A chunk that is not ready weighs 0; a chunk the fade has not seen land, but that is
/// ready, weighs 1 (it was there before the view looked).
#[derive(Clone, Debug)]
pub struct ChunkFade {
    fade_ms: u64,
    landed_ms: HashMap<ChunkKey, u64>,
}

impl ChunkFade {
    /// `chunk_fade_ms` is 500 by default.
    pub const DEFAULT_FADE_MS: u64 = 500;

    pub fn new(fade_ms: u64) -> ChunkFade {
        ChunkFade { fade_ms, landed_ms: HashMap::new() }
    }

    /// A chunk landed at `now_ms`.
    pub fn landed(&mut self, key: ChunkKey, now_ms: u64) {
        self.landed_ms.insert(key, now_ms);
    }

    /// A chunk left RAM: it fades in again if it lands again.
    pub fn evicted(&mut self, key: ChunkKey) {
        self.landed_ms.remove(&key);
    }

    /// The weight of the change of `key` at `now_ms`: linear from the landing, 1 once `fade_ms` have passed.
    pub fn weight(&self, key: ChunkKey, ready: bool, now_ms: u64) -> f32 {
        if !ready {
            return 0.0;
        }
        match self.landed_ms.get(&key) {
            None => 1.0,
            Some(&at) if self.fade_ms == 0 => {
                let _ = at;
                1.0
            }
            Some(&at) => (now_ms.saturating_sub(at) as f64 / self.fade_ms as f64).min(1.0) as f32,
        }
    }

    /// The weight of a tile: that of its own chunk (a tile above the top level reads none and weighs 0, it draws the base).
    pub fn tile_weight(&self, view: &dyn ChunkView, tile: TileId, now_ms: u64) -> f32 {
        view.reads(tile).first().map_or(0.0, |&k| self.weight(k, view.is_ready(k), now_ms))
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
