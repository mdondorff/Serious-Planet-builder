//! Chunk jobs, landing, residency and the disk cache (STRM-039, STRM-041; ADR 0077 point 7, D7).
//!
//! The manager owns the ready chunks. A frame calls [`ChunkManager::update`] with the chunks it wants and their distance to the camera; the
//! call starts jobs for the nearest wanted chunks that are neither ready nor running, at most `workers` at once, on the injected spawner,
//! and never waits for one (CON-22). A job reads the disk cache first and computes only on a miss; the result lands in the ready set, which
//! a frame reads as a snapshot. A chunk that has not been wanted for `evict_ms` on the injected clock leaves RAM (CON-04: the clock and the
//! spawner are injected, so a test runs a flight on a fake clock).

use crate::{Clock, Spawner};
use planet_cache::chunk_store::PackChunkDisk;
use planet_core::{Face, TileId};
use planet_generators::chunk::CHUNK_GRID_LEVEL;
use planet_generators::chunkfield::{compute_chunk, halo_rings, ChunkKey, ChunkLattice, ChunkResult, ChunkStage};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// The disk side of the chunk cache: a result by its tile, as bytes of [`ChunkResult::to_bytes`]. A `get` that fails names the damage; the
/// manager treats it as a miss, recomputes and logs it.
pub trait ChunkDisk: Send + Sync {
    fn get(&self, tile: TileId) -> Result<Option<Vec<u8>>, String>;
    fn put(&self, tile: TileId, bytes: Vec<u8>) -> Result<(), String>;
}

impl ChunkDisk for PackChunkDisk {
    fn get(&self, tile: TileId) -> Result<Option<Vec<u8>>, String> {
        PackChunkDisk::get(self, tile)
    }
    fn put(&self, tile: TileId, bytes: Vec<u8>) -> Result<(), String> {
        PackChunkDisk::put(self, tile, bytes)
    }
}

/// The knobs of the chunk jobs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkConfig {
    /// Jobs running at once (`chunk_workers`; the default of a viewer is half its worker pool, so tiles are not starved).
    pub workers: usize,
    /// A chunk not wanted for this long leaves RAM (`chunk_evict_s` 5, in milliseconds).
    pub evict_ms: u64,
    /// The bytes the ready chunks may hold (their own pool of the RAM caps, STRM-033): the least recently wanted leave first beyond it.
    pub cap_bytes: u64,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        ChunkConfig { workers: 3, evict_ms: 5_000, cap_bytes: u64::MAX }
    }
}

/// What the manager did, cumulative (counts only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkStats {
    /// Results computed by a job, read from the disk cache, and disk entries that failed their check or did not decode.
    pub computed: u64,
    pub disk_hits: u64,
    pub disk_failures: u64,
    /// Chunks that landed and chunks evicted.
    pub landed: u64,
    pub evicted: u64,
}

/// What one [`ChunkManager::update`] changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChunkUpdate {
    /// Chunks that landed since the last update, in landing order.
    pub landed: Vec<ChunkKey>,
    /// Chunks evicted by this update.
    pub evicted: Vec<ChunkKey>,
    /// Jobs started by this update.
    pub started: usize,
}

struct Entry {
    result: Arc<ChunkResult>,
    last_wanted_ms: u64,
}

#[derive(Default)]
struct Shared {
    ready: HashMap<ChunkKey, Entry>,
    running: HashSet<ChunkKey>,
    landed: Vec<ChunkKey>,
    stats: ChunkStats,
    log: Vec<String>,
}

/// The chunk jobs and the ready chunks of one chunk field.
pub struct ChunkManager {
    lat: ChunkLattice,
    radius_m: f64,
    top: u8,
    stage: Arc<dyn ChunkStage>,
    spawner: Arc<dyn Spawner>,
    clock: Arc<dyn Clock>,
    disk: Option<Arc<dyn ChunkDisk>>,
    cfg: ChunkConfig,
    shared: Arc<Mutex<Shared>>,
}

fn tile_of(key: ChunkKey) -> TileId {
    TileId::new(Face(key.0), CHUNK_GRID_LEVEL, key.1, key.2).expect("a chunk of the grid is a tile")
}

impl ChunkManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        lat: ChunkLattice,
        radius_m: f64,
        top: u8,
        stage: Arc<dyn ChunkStage>,
        spawner: Arc<dyn Spawner>,
        clock: Arc<dyn Clock>,
        disk: Option<Arc<dyn ChunkDisk>>,
        cfg: ChunkConfig,
    ) -> ChunkManager {
        ChunkManager { lat, radius_m, top, stage, spawner, clock, disk, cfg, shared: Arc::default() }
    }

    /// One frame: note which ready chunks are wanted, evict the ones that have not been for `evict_ms` (and the least recently wanted beyond
    /// the byte cap), start jobs for the nearest wanted chunks that are neither ready nor running (`wanted` is `(chunk, distance to the
    /// camera in metres)`), and report what landed since the last call. Never waits for a job.
    pub fn update(&self, wanted: &[(ChunkKey, f64)]) -> ChunkUpdate {
        let now = self.clock.now_ms();
        let mut out = ChunkUpdate::default();
        let to_start: Vec<ChunkKey> = {
            let mut sh = self.shared.lock().expect("chunk state");
            for (key, _) in wanted {
                if let Some(e) = sh.ready.get_mut(key) {
                    e.last_wanted_ms = now;
                }
            }
            let mut gone: Vec<ChunkKey> =
                sh.ready.iter().filter(|(_, e)| now.saturating_sub(e.last_wanted_ms) > self.cfg.evict_ms).map(|(k, _)| *k).collect();
            let mut bytes: u64 = sh.ready.values().map(|e| e.result.size_bytes() as u64).sum();
            for k in &gone {
                bytes = bytes.saturating_sub(sh.ready[k].result.size_bytes() as u64);
            }
            if bytes > self.cfg.cap_bytes {
                let mut rest: Vec<(u64, ChunkKey)> =
                    sh.ready.iter().filter(|(k, _)| !gone.contains(k)).map(|(k, e)| (e.last_wanted_ms, *k)).collect();
                rest.sort_unstable();
                for (_, k) in rest {
                    if bytes <= self.cfg.cap_bytes {
                        break;
                    }
                    bytes = bytes.saturating_sub(sh.ready[&k].result.size_bytes() as u64);
                    gone.push(k);
                }
            }
            gone.sort_unstable();
            for k in &gone {
                sh.ready.remove(k);
            }
            sh.stats.evicted += gone.len() as u64;
            out.evicted = gone;
            out.landed = std::mem::take(&mut sh.landed);
            let free = self.cfg.workers.saturating_sub(sh.running.len());
            let mut candidates: Vec<(f64, ChunkKey)> =
                wanted.iter().filter(|(k, _)| !sh.ready.contains_key(k) && !sh.running.contains(k)).map(|&(k, d)| (d, k)).collect();
            candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            candidates.dedup_by_key(|c| c.1);
            let chosen: Vec<ChunkKey> = candidates.into_iter().take(free).map(|(_, k)| k).collect();
            for k in &chosen {
                sh.running.insert(*k);
            }
            chosen
        };
        out.started = to_start.len();
        for key in to_start {
            self.spawner.spawn(self.job(key));
        }
        out
    }

    fn job(&self, key: ChunkKey) -> crate::Job {
        let (lat, radius_m, top) = (self.lat, self.radius_m, self.top);
        let (stage, disk, clock, shared) = (self.stage.clone(), self.disk.clone(), self.clock.clone(), self.shared.clone());
        Box::new(move || {
            let tile = tile_of(key);
            let mut from_disk = None;
            if let Some(disk) = &disk {
                match disk.get(tile) {
                    Ok(Some(bytes)) => match ChunkResult::from_bytes(tile, &bytes) {
                        Some(r) => from_disk = Some(r),
                        None => {
                            let mut sh = shared.lock().expect("chunk state");
                            sh.stats.disk_failures += 1;
                            sh.log.push(format!("chunk {key:?}: the stored result does not decode, recomputing"));
                        }
                    },
                    Ok(None) => {}
                    Err(e) => {
                        let mut sh = shared.lock().expect("chunk state");
                        sh.stats.disk_failures += 1;
                        sh.log.push(format!("chunk {key:?}: disk entry refused ({e}), recomputing"));
                    }
                }
            }
            let (result, hit) = match from_disk {
                Some(r) => (r, true),
                None => {
                    let r =
                        compute_chunk(&lat, &*stage, radius_m, top, tile, halo_rings(&*stage)).expect("the stage's own halo is accepted");
                    if let Some(disk) = &disk {
                        if let Err(e) = disk.put(tile, r.to_bytes()) {
                            shared.lock().expect("chunk state").log.push(format!("chunk {key:?}: not stored ({e})"));
                        }
                    }
                    (r, false)
                }
            };
            let mut sh = shared.lock().expect("chunk state");
            sh.running.remove(&key);
            sh.ready.insert(key, Entry { result: Arc::new(result), last_wanted_ms: clock.now_ms() });
            sh.landed.push(key);
            sh.stats.landed += 1;
            if hit {
                sh.stats.disk_hits += 1;
            } else {
                sh.stats.computed += 1;
            }
        })
    }

    /// The ready chunks, in key order: what a frame builds its chunk field from.
    pub fn snapshot(&self) -> Vec<Arc<ChunkResult>> {
        let sh = self.shared.lock().expect("chunk state");
        let mut keys: Vec<&ChunkKey> = sh.ready.keys().collect();
        keys.sort_unstable();
        keys.into_iter().map(|k| sh.ready[k].result.clone()).collect()
    }

    /// The pyramid's top tile level the manager's chunks were computed for.
    pub fn top_level(&self) -> u8 {
        self.top
    }

    /// The chunks tile `tile` reads (own chunk first), as a chunk field of this manager's chunks decides them for keys and invalidation.
    pub fn reads(&self, tile: TileId) -> Vec<ChunkKey> {
        let reach = planet_generators::chunkfield::reach_cells(&self.lat, self.radius_m, self.top);
        planet_generators::chunkfield::chunks_read_by(&self.lat, self.top, reach, tile)
    }

    pub fn is_ready(&self, key: ChunkKey) -> bool {
        self.shared.lock().expect("chunk state").ready.contains_key(&key)
    }

    /// Jobs running or waiting for a worker.
    pub fn in_flight(&self) -> usize {
        self.shared.lock().expect("chunk state").running.len()
    }

    /// The bytes the ready chunks hold (the chunk pool of the RAM caps).
    pub fn pool_bytes(&self) -> u64 {
        self.shared.lock().expect("chunk state").ready.values().map(|e| e.result.size_bytes() as u64).sum()
    }

    pub fn stats(&self) -> ChunkStats {
        self.shared.lock().expect("chunk state").stats
    }

    /// The failures and notes since the last call, named.
    pub fn take_log(&self) -> Vec<String> {
        std::mem::take(&mut self.shared.lock().expect("chunk state").log)
    }
}
