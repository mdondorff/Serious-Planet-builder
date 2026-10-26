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
    /// The bytes the ready chunks may hold (their own pool of the RAM caps, STRM-041; 512 MiB by default, about 250 chunks of 8 m cells): the least
    /// recently wanted leave first beyond it.
    pub cap_bytes: u64,
    /// Whether landed chunks are visible to the readers (`snapshot`, `is_ready`, `result`, `result_hash`) only once published
    /// ([`ChunkManager::publish`], STRM-054): the viewer publishes them one fade at a time. Off: every ready chunk is visible, as before.
    pub publish_gate: bool,
}

impl ChunkConfig {
    /// The config of the viewer and of every scene that measures what the viewer shows (criterion 7's speed envelope included): the publish
    /// gate on (STRM-054), the rest as the default. Stills and tests that wait for every chunk use the default, ungated.
    pub fn viewer(workers: usize) -> ChunkConfig {
        ChunkConfig { workers, publish_gate: true, ..ChunkConfig::default() }
    }
}

impl Default for ChunkConfig {
    fn default() -> Self {
        ChunkConfig { workers: 3, evict_ms: 5_000, cap_bytes: 512 << 20, publish_gate: false }
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
    /// The ready chunks the readers may see when the gate is on (STRM-054).
    published: HashSet<ChunkKey>,
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
        self.update_with(wanted, &[])
    }

    /// [`ChunkManager::update`] with chunks to prefetch (STRM-042): they are started only on workers no wanted chunk needs (below the wanted set
    /// in priority, in the order given), and they count as wanted for eviction, so a prefetched chunk is still there when the camera arrives.
    pub fn update_with(&self, wanted: &[(ChunkKey, f64)], prefetch: &[ChunkKey]) -> ChunkUpdate {
        let now = self.clock.now_ms();
        let mut out = ChunkUpdate::default();
        let to_start: Vec<ChunkKey> = {
            let mut sh = self.shared.lock().expect("chunk state");
            for key in wanted.iter().map(|w| &w.0).chain(prefetch) {
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
                sh.published.remove(k);
            }
            sh.stats.evicted += gone.len() as u64;
            out.evicted = gone;
            out.landed = std::mem::take(&mut sh.landed);
            let free = self.cfg.workers.saturating_sub(sh.running.len());
            let mut candidates: Vec<(f64, ChunkKey)> =
                wanted.iter().filter(|(k, _)| !sh.ready.contains_key(k) && !sh.running.contains(k)).map(|&(k, d)| (d, k)).collect();
            candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            candidates.dedup_by_key(|c| c.1);
            let mut chosen: Vec<ChunkKey> = candidates.into_iter().take(free).map(|(_, k)| k).collect();
            // What is left of the workers goes to the prefetch, in the order given (wanted chunks never wait for it).
            for k in prefetch {
                if chosen.len() >= free {
                    break;
                }
                if !sh.ready.contains_key(k) && !sh.running.contains(k) && !chosen.contains(k) {
                    chosen.push(*k);
                }
            }
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
                    if let Some(disk) = disk.as_ref().filter(|_| stage.healthy()) {
                        if let Err(e) = disk.put(tile, r.to_bytes()) {
                            shared.lock().expect("chunk state").log.push(format!("chunk {key:?}: not stored ({e})"));
                        }
                    }
                    (r, false)
                }
            };
            // The hash of the pyramid is made here, on the worker and outside every lock: the horizon jobs read it every frame (STRM-048).
            let _ = result.hash();
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

    /// Whether the readers see published chunks only (the gate of [`ChunkConfig::publish_gate`]).
    pub fn gated(&self) -> bool {
        self.cfg.publish_gate
    }

    /// Make the ready chunks among `keys` visible to the readers (STRM-054); a no-op without the gate. A chunk that is not ready is ignored.
    pub fn publish(&self, keys: &[ChunkKey]) {
        let mut sh = self.shared.lock().expect("chunk state");
        for k in keys {
            if sh.ready.contains_key(k) {
                sh.published.insert(*k);
            }
        }
    }

    /// The ready chunks the readers do not see yet, in key order; empty without the gate.
    pub fn unpublished(&self) -> Vec<ChunkKey> {
        if !self.cfg.publish_gate {
            return Vec::new();
        }
        let sh = self.shared.lock().expect("chunk state");
        let mut keys: Vec<ChunkKey> = sh.ready.keys().filter(|k| !sh.published.contains(k)).copied().collect();
        keys.sort_unstable();
        keys
    }

    fn visible(&self, sh: &Shared, key: &ChunkKey) -> bool {
        !self.cfg.publish_gate || sh.published.contains(key)
    }

    /// The visible chunks, in key order: what a frame builds its chunk field from (the horizon jobs, the keys and the weights read the same).
    pub fn snapshot(&self) -> Vec<Arc<ChunkResult>> {
        let sh = self.shared.lock().expect("chunk state");
        let mut keys: Vec<&ChunkKey> = sh.ready.keys().filter(|k| self.visible(&sh, k)).collect();
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
        let sh = self.shared.lock().expect("chunk state");
        sh.ready.contains_key(&key) && self.visible(&sh, &key)
    }

    /// The visible result of `key` (what a horizon bake reads of a chunk).
    pub fn result(&self, key: ChunkKey) -> Option<Arc<ChunkResult>> {
        let sh = self.shared.lock().expect("chunk state");
        sh.ready.get(&key).filter(|_| self.visible(&sh, &key)).map(|e| e.result.clone())
    }

    /// The hash of the ready result of `key`: the part of a horizon's state word that this chunk contributes.
    pub fn result_hash(&self, key: ChunkKey) -> Option<u64> {
        // The result is taken under the lock and hashed (made once, then read) outside it: a chunk worker landing its result never waits for a hash.
        let result = {
            let sh = self.shared.lock().expect("chunk state");
            sh.ready.get(&key).filter(|_| self.visible(&sh, &key)).map(|e| e.result.clone())
        };
        result.map(|r| r.hash())
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

// ---------------------------------------------------------------------------------------------------------------------------------
// The horizon job (M8-5, STRM-048): a second job per chunk that lands after its heights

/// What the horizon job needs of the layers above it, injected so this crate stays GPU-free and a test runs with a fake: the chunks a horizon
/// reads and the bake itself (the real one samples the chunk field's heights over the chunk and its halo and calls `bake_horizon`).
pub trait HorizonBaker: Send + Sync {
    /// The chunks the horizon of `key` reads, its own first: the changes of these (where they are ready) are in its heights.
    fn reads(&self, key: ChunkKey) -> Vec<ChunkKey>;
    /// The horizon map of `key`, given the ready results among [`HorizonBaker::reads`] (a chunk that is not ready contributes its base only).
    fn bake(&self, key: ChunkKey, ready: &[(ChunkKey, Arc<ChunkResult>)]) -> Vec<u8>;
    /// A hash of everything that defines the march (texel, azimuths, range, version): a change re-keys every stored map.
    fn spec_hash(&self) -> u64;
}

/// The disk side of the horizon cache: a map by its tile and its state word (the march and the results of the chunk and of the neighbours it
/// read), so a map made against other neighbours is a miss. A failing `get` is a miss, logged.
pub trait HorizonDisk: Send + Sync {
    fn get(&self, tile: TileId, word: u64) -> Result<Option<Vec<u8>>, String>;
    fn put(&self, tile: TileId, word: u64, bytes: Vec<u8>) -> Result<(), String>;
}

/// The knobs of the horizon jobs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HorizonConfig {
    /// Jobs running at once (fewer than the chunk jobs: heights come first).
    pub workers: usize,
    /// A map made against a neighbourhood that has since changed is baked again only once the neighbourhood has been unchanged for this long
    /// (milliseconds of the injected clock), so the landing of the eight neighbours one after another costs one or two bakes, not eight.
    pub settle_ms: u64,
}

impl Default for HorizonConfig {
    fn default() -> Self {
        HorizonConfig { workers: 1, settle_ms: 1_000 }
    }
}

/// What the horizon jobs did, cumulative (counts only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HorizonStats {
    /// Maps baked by a job.
    pub baked: u64,
    /// Maps read from the disk cache.
    pub disk_hits: u64,
    /// Landings of a chunk that already had a map (its neighbourhood had changed): the figure the coalescing keeps small.
    pub rebaked: u64,
    pub landed: u64,
    pub evicted: u64,
}

/// What one [`HorizonJobs::update`] changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HorizonUpdate {
    /// Maps that landed since the last update (a new map or a re-bake), in landing order.
    pub landed: Vec<ChunkKey>,
    /// Maps dropped because their chunk left RAM.
    pub evicted: Vec<ChunkKey>,
    pub started: usize,
}

struct HorizonEntry {
    bytes: Arc<Vec<u8>>,
    word: u64,
}

#[derive(Default)]
struct HorizonShared {
    ready: HashMap<ChunkKey, HorizonEntry>,
    running: HashSet<ChunkKey>,
    /// The state word last seen for a chunk and since when (the settle window starts at a change).
    seen: HashMap<ChunkKey, (u64, u64)>,
    ever_baked: HashSet<ChunkKey>,
    landed: Vec<ChunkKey>,
    stats: HorizonStats,
    log: Vec<String>,
}

/// The horizon maps of the ready chunks: a map is baked once its chunk is ready, and again when the chunks it read have changed (a neighbour
/// landed), after the neighbourhood has settled. Never waits for a job (CON-22): a tile whose map is not there reads the coarse horizon alone.
pub struct HorizonJobs {
    chunks: Arc<ChunkManager>,
    baker: Arc<dyn HorizonBaker>,
    spawner: Arc<dyn Spawner>,
    clock: Arc<dyn Clock>,
    disk: Option<Arc<dyn HorizonDisk>>,
    cfg: HorizonConfig,
    shared: Arc<Mutex<HorizonShared>>,
}

impl HorizonJobs {
    pub fn new(
        chunks: Arc<ChunkManager>,
        baker: Arc<dyn HorizonBaker>,
        spawner: Arc<dyn Spawner>,
        clock: Arc<dyn Clock>,
        disk: Option<Arc<dyn HorizonDisk>>,
        cfg: HorizonConfig,
    ) -> HorizonJobs {
        HorizonJobs { chunks, baker, spawner, clock, disk, cfg, shared: Arc::default() }
    }

    /// The state word of the horizon of `key` now: the march, and the result of every chunk it reads (0 for one that is not ready).
    fn word(&self, key: ChunkKey) -> u64 {
        let mut h = planet_core::hash::mix64(self.baker.spec_hash() ^ 0x484f_5249_5a4f_4e00);
        for r in self.baker.reads(key) {
            h = planet_core::hash::mix64(h ^ self.chunks.result_hash(r).unwrap_or(0));
        }
        h
    }

    /// One frame: drop the maps of chunks that left RAM, find the wanted chunks whose map is missing or was made against another neighbourhood
    /// (the state word is checked now, at dispatch, not on each landing), start the nearest at most `workers` at once, and report what landed.
    pub fn update(&self, wanted: &[(ChunkKey, f64)]) -> HorizonUpdate {
        let now = self.clock.now_ms();
        let mut out = HorizonUpdate::default();
        let mut starts: Vec<(ChunkKey, u64)> = Vec::new();
        // The state word of each wanted chunk that is ready, made before the horizon lock is taken (it reads the chunk manager's state).
        let words: Vec<Option<u64>> = wanted.iter().map(|&(k, _)| self.chunks.is_ready(k).then(|| self.word(k))).collect();
        {
            let mut sh = self.shared.lock().expect("horizon state");
            let gone: Vec<ChunkKey> = {
                let mut v: Vec<ChunkKey> = sh.ready.keys().filter(|k| !self.chunks.is_ready(**k)).copied().collect();
                v.sort_unstable();
                v
            };
            for k in &gone {
                sh.ready.remove(k);
                sh.seen.remove(k);
                sh.ever_baked.remove(k);
            }
            sh.stats.evicted += gone.len() as u64;
            out.evicted = gone;
            out.landed = std::mem::take(&mut sh.landed);
            let free = self.cfg.workers.saturating_sub(sh.running.len());
            let mut candidates: Vec<(f64, ChunkKey, u64)> = Vec::new();
            for (&(k, d), word) in wanted.iter().zip(&words) {
                let Some(word) = *word else { continue };
                if !self.chunks.is_ready(k) || sh.running.contains(&k) {
                    continue;
                }
                let since = match sh.seen.get(&k) {
                    Some(&(w, at)) if w == word => at,
                    _ => {
                        sh.seen.insert(k, (word, now));
                        now
                    }
                };
                match sh.ready.get(&k) {
                    Some(e) if e.word == word => {}
                    // A map exists for another neighbourhood: wait for the neighbourhood to settle.
                    Some(_) if now.saturating_sub(since) < self.cfg.settle_ms => {}
                    _ => candidates.push((d, k, word)),
                }
            }
            candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            candidates.dedup_by_key(|c| c.1);
            for (_, k, word) in candidates.into_iter().take(free) {
                sh.running.insert(k);
                starts.push((k, word));
            }
        }
        out.started = starts.len();
        for (key, word) in starts {
            self.spawner.spawn(self.job(key, word));
        }
        out
    }

    fn job(&self, key: ChunkKey, word: u64) -> crate::Job {
        let (chunks, baker, disk, shared) = (self.chunks.clone(), self.baker.clone(), self.disk.clone(), self.shared.clone());
        Box::new(move || {
            let tile = tile_of(key);
            let mut bytes = None;
            if let Some(disk) = &disk {
                match disk.get(tile, word) {
                    Ok(found) => bytes = found,
                    Err(e) => shared.lock().expect("horizon state").log.push(format!("horizon {key:?}: disk entry refused ({e}), baking")),
                }
            }
            let hit = bytes.is_some();
            let bytes = bytes.unwrap_or_else(|| {
                let ready: Vec<(ChunkKey, Arc<ChunkResult>)> =
                    baker.reads(key).into_iter().filter_map(|r| chunks.result(r).map(|res| (r, res))).collect();
                let made = baker.bake(key, &ready);
                if let Some(disk) = &disk {
                    if let Err(e) = disk.put(tile, word, made.clone()) {
                        shared.lock().expect("horizon state").log.push(format!("horizon {key:?}: not stored ({e})"));
                    }
                }
                made
            });
            let mut sh = shared.lock().expect("horizon state");
            sh.running.remove(&key);
            if !sh.ever_baked.insert(key) {
                sh.stats.rebaked += 1;
            }
            sh.ready.insert(key, HorizonEntry { bytes: Arc::new(bytes), word });
            sh.landed.push(key);
            sh.stats.landed += 1;
            if hit {
                sh.stats.disk_hits += 1;
            } else {
                sh.stats.baked += 1;
            }
        })
    }

    /// The horizon map of `key`, if one has landed (`None`: the tile reads the coarse horizon alone).
    pub fn horizon(&self, key: ChunkKey) -> Option<Arc<Vec<u8>>> {
        self.shared.lock().expect("horizon state").ready.get(&key).map(|e| e.bytes.clone())
    }

    pub fn stats(&self) -> HorizonStats {
        self.shared.lock().expect("horizon state").stats
    }

    pub fn in_flight(&self) -> usize {
        self.shared.lock().expect("horizon state").running.len()
    }

    /// The failures and notes since the last call, named.
    pub fn take_log(&self) -> Vec<String> {
        std::mem::take(&mut self.shared.lock().expect("horizon state").log)
    }
}
