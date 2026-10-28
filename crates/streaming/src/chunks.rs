//! Chunk jobs, landing, residency and the disk cache (STRM-039, STRM-041; ADR 0077 point 7, D7).
//!
//! The manager owns the ready chunks. A frame calls [`ChunkManager::update`] with the chunks it wants and their distance to the camera; the
//! call starts jobs for the nearest wanted chunks that are neither ready nor running, at most `workers` at once, on the injected spawner,
//! and never waits for one (CON-22). A job reads the disk cache first and computes only on a miss; the result lands in the ready set, which
//! a frame reads as a snapshot. A chunk that has not been wanted for `evict_ms` on the injected clock leaves RAM (CON-04: the clock and the
//! spawner are injected, so a test runs a flight on a fake clock).

use crate::{Clock, JobClass, Spawner};
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
    /// [`ChunkDisk::get`] of the chunk at `version` (GEN-082); a disk that keeps no versions has only the first (0).
    fn get_versioned(&self, tile: TileId, version: u64) -> Result<Option<Vec<u8>>, String> {
        if version == 0 {
            self.get(tile)
        } else {
            Ok(None)
        }
    }
    /// [`ChunkDisk::put`] of the chunk at `version`; a disk that keeps no versions stores only the first.
    fn put_versioned(&self, tile: TileId, version: u64, bytes: Vec<u8>) -> Result<(), String> {
        if version == 0 {
            self.put(tile, bytes)
        } else {
            Ok(())
        }
    }
}

/// The version of a result that was computed while the stage's inputs moved (an edit landed during the job): never a real version, so the chunk is
/// stale at once and is solved again; its bits are neither trusted nor stored.
pub const UNTRUSTED_VERSION: u64 = u64::MAX - 1;

/// The version a chunk should have now (GEN-082): the hash of the edits that can change it, 0 for none. A frame asks it for the wanted chunks; a
/// ready chunk of another version is stale (STRM-056).
pub trait ChunkVersions: Send + Sync {
    fn version(&self, key: ChunkKey) -> u64;
    /// Whether an edit lies in the chunk's own region (its square and halo) and not only in the wider read set (the routed network it reads,
    /// GEN-081): such a chunk is certain to change and is solved first among the stale ones (default: none).
    fn near_edit(&self, _key: ChunkKey) -> bool {
        false
    }
}

impl ChunkDisk for PackChunkDisk {
    fn get(&self, tile: TileId) -> Result<Option<Vec<u8>>, String> {
        PackChunkDisk::get(self, tile)
    }
    fn put(&self, tile: TileId, bytes: Vec<u8>) -> Result<(), String> {
        PackChunkDisk::put(self, tile, bytes)
    }
    fn get_versioned(&self, tile: TileId, version: u64) -> Result<Option<Vec<u8>>, String> {
        PackChunkDisk::get_versioned(self, tile, version)
    }
    fn put_versioned(&self, tile: TileId, version: u64, bytes: Vec<u8>) -> Result<(), String> {
        PackChunkDisk::put_versioned(self, tile, version, bytes)
    }
}

/// The chunk workers of a viewer on the reference machine: an edit re-solves 5 to 7 chunks of the view, and a pool of 6 does it in one round (the
/// window of 2026-10-26: 17 of 19 cases within 10 s, against 3 with 4; 8 adds nothing).
pub const VIEW_CHUNK_WORKERS: usize = 6;

/// The threads of one chunk job's batch of base heights in a viewer (6 jobs of 2 threads use the 12 threads the chunk jobs may take of the 24).
pub const VIEW_BASE_THREADS: usize = 2;

/// The knobs of the chunk jobs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkConfig {
    /// Jobs on stale chunks (a ready chunk of an older version, STRM-056) running at once, at most `workers`; the default is every worker: a chunk
    /// that is missing still goes first, and an edit's chunks take what is left (ADR 0083's class "visible streaming and edits").
    pub stale_workers: usize,
    /// The longest a stale chunk waits for the visible edit to be drawn (milliseconds of the injected clock): while the loop says the edit's own tiles
    /// are still being built ([`ChunkManager::hold_stale`]) the stale jobs do not start, so they do not compete with them; after this long they start.
    pub stale_hold_cap_ms: u64,
    /// Jobs running at once (`chunk_workers`; the default of a viewer is half its worker pool, so tiles are not starved).
    pub workers: usize,
    /// A chunk not wanted for this long leaves RAM (`chunk_evict_s` 30, in milliseconds). The byte cap below bounds the pool whatever this is.
    pub evict_ms: u64,
    /// The bytes the ready chunks may hold (their own pool of the RAM caps, STRM-041; 512 MiB by default, about 250 chunks of 8 m cells): the least
    /// recently wanted leave first beyond it.
    pub cap_bytes: u64,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        ChunkConfig {
            workers: VIEW_CHUNK_WORKERS,
            stale_workers: usize::MAX,
            stale_hold_cap_ms: 1_000,
            evict_ms: 30_000,
            cap_bytes: 512 << 20,
        }
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
    /// The version the result was made at (0: no edit reaches it).
    version: u64,
}

#[derive(Default)]
struct Shared {
    ready: HashMap<ChunkKey, Entry>,
    running: HashSet<ChunkKey>,
    /// The running jobs that re-solve a ready chunk of an older version (a subset of `running`).
    stale_running: HashSet<ChunkKey>,
    /// The instant (injected clock) a ready chunk was first seen stale, until it lands again: the hold of the stale jobs ends a cap after the first.
    stale_since: HashMap<ChunkKey, u64>,
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
    versions: Option<Arc<dyn ChunkVersions>>,
    hold_stale: std::sync::atomic::AtomicBool,
    shared: Arc<Mutex<Shared>>,
    /// How far into a neighbouring chunk a tile's reads reach, in cells (a constant of the lattice, body and top level: the plan cache asks `reads` for
    /// every tile it holds when a chunk lands, so it is not recomputed per call).
    reach: i64,
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
        let reach = planet_generators::chunkfield::reach_cells(&lat, radius_m, top);
        ChunkManager {
            lat,
            radius_m,
            top,
            stage,
            spawner,
            clock,
            disk,
            cfg,
            versions: None,
            hold_stale: Default::default(),
            shared: Arc::default(),
            reach,
        }
    }

    /// Whether the visible edit is still being drawn: while it is, the stale chunks wait (at most `stale_hold_cap_ms` from the first one seen stale),
    /// so the tiles of the edit are built first (the class "visible streaming and edits" of ADR 0083 comes before the chunk re-solve it causes).
    pub fn hold_stale(&self, hold: bool) {
        self.hold_stale.store(hold, std::sync::atomic::Ordering::Relaxed);
    }

    /// The manager with a source of chunk versions (GEN-082): a ready chunk whose version differs from the source's is stale and is re-solved when
    /// wanted (STRM-056), while its old result keeps drawing.
    pub fn with_versions(mut self, versions: Arc<dyn ChunkVersions>) -> ChunkManager {
        self.versions = Some(versions);
        self
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
        // The version of every wanted chunk that is ready, asked before the lock is taken (the source reads the world's definition). A chunk that is not
        // ready has no version asked here: making one can take the read set of the chunk, a few hundred milliseconds that must not be the main thread's,
        // so the job of a new chunk asks for it on its worker (`ChunkManager::job`).
        let ready: HashSet<ChunkKey> = self.shared.lock().expect("chunk state").ready.keys().copied().collect();
        let versions: HashMap<ChunkKey, u64> = match &self.versions {
            Some(v) => wanted
                .iter()
                .map(|w| w.0)
                .chain(prefetch.iter().copied())
                .filter(|k| ready.contains(k))
                .map(|k| (k, v.version(k)))
                .collect(),
            None => HashMap::new(),
        };
        let want_version = |k: &ChunkKey| versions.get(k).copied().unwrap_or(0);
        // The version a job starts with: none for a chunk without one asked (its job asks), 0 where there is no source of versions.
        let job_version = |k: &ChunkKey| if self.versions.is_some() { versions.get(k).copied() } else { Some(0) };
        let near: HashMap<ChunkKey, bool> = match &self.versions {
            Some(v) => wanted.iter().map(|w| w.0).filter(|k| ready.contains(k)).map(|k| (k, v.near_edit(k))).collect(),
            None => HashMap::new(),
        };
        // The stage's epoch when the versions were asked: a job that ends under another epoch computed against edits it was not keyed for.
        let epoch0 = self.stage.epoch();
        let to_start: Vec<(ChunkKey, Option<u64>, bool, JobClass)> = {
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
            }
            sh.stats.evicted += gone.len() as u64;
            out.evicted = gone;
            out.landed = std::mem::take(&mut sh.landed);
            let free = self.cfg.workers.saturating_sub(sh.running.len());
            let mut candidates: Vec<(f64, ChunkKey)> =
                wanted.iter().filter(|(k, _)| !sh.ready.contains_key(k) && !sh.running.contains(k)).map(|&(k, d)| (d, k)).collect();
            candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            candidates.dedup_by_key(|c| c.1);
            let mut chosen: Vec<(ChunkKey, Option<u64>, bool, JobClass)> =
                candidates.into_iter().take(free).map(|(_, k)| (k, job_version(&k), false, JobClass::Visible)).collect();
            // Then the stale chunks the plan reads (STRM-056), nearest first, at most `stale_workers` at once: a chunk of an older version keeps
            // drawing, so it waits behind the chunks that are missing, and the workers it does not use stay free for new chunks.
            let stale_free = self.cfg.stale_workers.min(self.cfg.workers).saturating_sub(sh.stale_running.len());
            // The ones with an edit in their own region first (they surely change), then the ones only the wider read set reaches (they may come out
            // as they were), each group nearest to the camera first.
            let mut stale: Vec<(f64, ChunkKey)> = wanted
                .iter()
                .filter(|(k, _)| {
                    // A chunk that landed after the versions were asked has no version here: it is not judged this update (its job made its version).
                    let known = self.versions.is_none() || versions.contains_key(k);
                    known && sh.ready.get(k).is_some_and(|e| e.version != want_version(k)) && !sh.running.contains(k)
                })
                .map(|&(k, d)| (d, k))
                .collect();
            let is_near = |k: &ChunkKey| near.get(k).copied().unwrap_or(false);
            stale.sort_by(|a, b| is_near(&b.1).cmp(&is_near(&a.1)).then(a.0.total_cmp(&b.0)).then(a.1.cmp(&b.1)));
            stale.dedup_by_key(|c| c.1);
            // The instants chunks were first seen stale (kept while they are stale or running), and whether the hold is on: the loop says the edit's
            // tiles are not drawn yet and the first stale chunk has not waited for the cap.
            for (_, k) in &stale {
                sh.stale_since.entry(*k).or_insert(now);
            }
            let stale_keys: HashSet<ChunkKey> = stale.iter().map(|c| c.1).collect();
            let running_stale = sh.stale_running.clone();
            sh.stale_since.retain(|k, _| stale_keys.contains(k) || running_stale.contains(k));
            let first_stale = stale.iter().filter_map(|c| sh.stale_since.get(&c.1)).min().copied();
            let held = self.hold_stale.load(std::sync::atomic::Ordering::Relaxed)
                && first_stale.is_some_and(|t| now.saturating_sub(t) < self.cfg.stale_hold_cap_ms);
            for (_, k) in stale.into_iter().take(if held { 0 } else { stale_free }) {
                if chosen.len() >= free {
                    break;
                }
                chosen.push((k, Some(want_version(&k)), true, JobClass::Visible));
            }
            // What is left of the workers goes to the prefetch, in the order given (wanted chunks never wait for it).
            for k in prefetch {
                if chosen.len() >= free {
                    break;
                }
                if !sh.ready.contains_key(k) && !sh.running.contains(k) && !chosen.iter().any(|c| c.0 == *k) {
                    chosen.push((*k, job_version(k), false, JobClass::Prefetch));
                }
            }
            for (k, _, stale, _) in &chosen {
                sh.running.insert(*k);
                if *stale {
                    sh.stale_running.insert(*k);
                }
            }
            chosen
        };
        out.started = to_start.len();
        for (key, version, _, class) in to_start {
            self.spawner.spawn_in(class, self.job(key, version, epoch0));
        }
        out
    }

    fn job(&self, key: ChunkKey, version: Option<u64>, epoch0: u64) -> crate::Job {
        let (lat, radius_m, top) = (self.lat, self.radius_m, self.top);
        let versions = self.versions.clone();
        let (stage, disk, clock, shared) = (self.stage.clone(), self.disk.clone(), self.clock.clone(), self.shared.clone());
        Box::new(move || {
            let tile = tile_of(key);
            // A new chunk's version is made here, on the worker (its read set is a few hundred ms of work); the staleness check of the ready chunk uses it later.
            let version = version.unwrap_or_else(|| versions.as_ref().map_or(0, |v| v.version(key)));
            let mut from_disk = None;
            let mut moved = false;
            if let Some(disk) = &disk {
                match disk.get_versioned(tile, version) {
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
                    moved = stage.epoch() != epoch0;
                    if let Some(disk) = disk.as_ref().filter(|_| stage.healthy() && !moved) {
                        if let Err(e) = disk.put_versioned(tile, version, r.to_bytes()) {
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
            sh.stale_running.remove(&key);
            sh.stale_since.remove(&key);
            sh.ready.insert(
                key,
                Entry {
                    result: Arc::new(result),
                    last_wanted_ms: clock.now_ms(),
                    version: if moved { UNTRUSTED_VERSION } else { version },
                },
            );
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
        planet_generators::chunkfield::chunks_read_by(&self.lat, self.top, self.reach, tile)
    }

    pub fn is_ready(&self, key: ChunkKey) -> bool {
        self.shared.lock().expect("chunk state").ready.contains_key(&key)
    }

    /// The ready result of `key` (what a horizon bake reads of a chunk).
    pub fn result(&self, key: ChunkKey) -> Option<Arc<ChunkResult>> {
        self.shared.lock().expect("chunk state").ready.get(&key).map(|e| e.result.clone())
    }

    /// The hash of the ready result of `key`: the part of a horizon's state word that this chunk contributes.
    pub fn result_hash(&self, key: ChunkKey) -> Option<u64> {
        // The result is taken under the lock and hashed (made once, then read) outside it: a chunk worker landing its result never waits for a hash.
        let result = self.shared.lock().expect("chunk state").ready.get(&key).map(|e| e.result.clone());
        result.map(|r| r.hash())
    }

    /// Jobs running or waiting for a worker.
    /// Whether a job for `key` is running or waiting for a worker now (a scene's trace stamps when a chunk was started).
    pub fn is_running(&self, key: ChunkKey) -> bool {
        self.shared.lock().expect("chunk state").running.contains(&key)
    }

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
    /// `None`: an edit changed what the map samples while it was being made ([`HorizonBaker::edit_word`]); the job drops it, nothing is published or stored, and
    /// the chunk is baked again against the new word once the neighbourhood has settled (STRM-060).
    fn bake(&self, key: ChunkKey, ready: &[(ChunkKey, Arc<ChunkResult>)]) -> Option<Vec<u8>>;
    /// A hash of everything that defines the march (texel, azimuths, range, version): a change re-keys every stored map.
    fn spec_hash(&self) -> u64;
    /// A word that differs when an edit can change the heights the horizon of `key` samples (the items that touch the chunk and its halo, GEN-082),
    /// 0 for none: the map of a chunk no edit reaches is not baked again (STRM-048). Default 0: a baker whose field never changes.
    fn edit_word(&self, _key: ChunkKey) -> u64 {
        0
    }
    /// A word that differs when anything else the heights depend on changes (the coupled lens, the base of the definition, the stage): mixed into every map's
    /// state word, so such a change bakes every wanted map again after the settle window (STRM-064). Default 0.
    fn world_word(&self) -> u64 {
        0
    }
}

/// The disk side of the horizon cache: a map by its tile and its state word (the march and the results of the chunk and of the neighbours it
/// read), so a map made against other neighbours is a miss. A failing `get` is a miss, logged.
pub trait HorizonDisk: Send + Sync {
    fn get(&self, tile: TileId, word: u64) -> Result<Option<Vec<u8>>, String>;
    fn put(&self, tile: TileId, word: u64, bytes: Vec<u8>) -> Result<(), String>;
}

/// What the horizon jobs need to know of the chunk stage: which chunks are ready and their results. The chunk manager is one source; [`BaseTerrain`]
/// is the world with no chunk stage (every chunk is ready and has no change), so the maps are baked from the base terrain and the stamps alone (STRM-064).
pub trait ChunkSource: Send + Sync {
    fn is_ready(&self, key: ChunkKey) -> bool;
    fn result(&self, key: ChunkKey) -> Option<Arc<ChunkResult>>;
    fn result_hash(&self, key: ChunkKey) -> Option<u64>;
    /// Whether chunks leave this source on their own (then the horizon drops the maps of chunks that left); `false` makes the jobs keep the
    /// [`HorizonConfig::keep`] most recently wanted maps beyond the wanted ones.
    fn evicts(&self) -> bool {
        true
    }
}

impl ChunkSource for ChunkManager {
    fn is_ready(&self, key: ChunkKey) -> bool {
        ChunkManager::is_ready(self, key)
    }
    fn result(&self, key: ChunkKey) -> Option<Arc<ChunkResult>> {
        ChunkManager::result(self, key)
    }
    fn result_hash(&self, key: ChunkKey) -> Option<u64> {
        ChunkManager::result_hash(self, key)
    }
}

/// No chunk stage: every chunk is ready with no change, nothing is ever evicted by the source.
pub struct BaseTerrain;

impl ChunkSource for BaseTerrain {
    fn is_ready(&self, _key: ChunkKey) -> bool {
        true
    }
    fn result(&self, _key: ChunkKey) -> Option<Arc<ChunkResult>> {
        None
    }
    fn result_hash(&self, _key: ChunkKey) -> Option<u64> {
        None
    }
    fn evicts(&self) -> bool {
        false
    }
}

/// The knobs of the horizon jobs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HorizonConfig {
    /// Jobs running at once (fewer than the chunk jobs: heights come first).
    pub workers: usize,
    /// A map made against a neighbourhood that has since changed is baked again only once the neighbourhood has been unchanged for this long
    /// (milliseconds of the injected clock), so the landing of the eight neighbours one after another costs one or two bakes, not eight.
    pub settle_ms: u64,
    /// Maps kept beyond the wanted ones when the source does not evict (a map is about 6 MB at the 8 m lattice): the most recently wanted.
    pub keep: usize,
}

impl Default for HorizonConfig {
    fn default() -> Self {
        HorizonConfig { workers: 1, settle_ms: 1_000, keep: 8 }
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
    /// Bakes dropped because an edit changed their heights while they ran.
    pub cancelled: u64,
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
    /// The update counter at which a chunk was last wanted (the order maps are dropped in when the source does not evict).
    wanted_at: HashMap<ChunkKey, u64>,
    updates: u64,
    landed: Vec<ChunkKey>,
    stats: HorizonStats,
    log: Vec<String>,
}

/// The horizon maps of the ready chunks: a map is baked once its chunk is ready, and again when the chunks it read have changed (a neighbour
/// landed), after the neighbourhood has settled. Never waits for a job (CON-22): a tile whose map is not there reads the coarse horizon alone.
pub struct HorizonJobs {
    chunks: Arc<dyn ChunkSource>,
    baker: Arc<dyn HorizonBaker>,
    spawner: Arc<dyn Spawner>,
    clock: Arc<dyn Clock>,
    disk: Option<Arc<dyn HorizonDisk>>,
    cfg: HorizonConfig,
    shared: Arc<Mutex<HorizonShared>>,
}

impl HorizonJobs {
    pub fn new(
        chunks: Arc<dyn ChunkSource>,
        baker: Arc<dyn HorizonBaker>,
        spawner: Arc<dyn Spawner>,
        clock: Arc<dyn Clock>,
        disk: Option<Arc<dyn HorizonDisk>>,
        cfg: HorizonConfig,
    ) -> HorizonJobs {
        HorizonJobs { chunks, baker, spawner, clock, disk, cfg, shared: Arc::default() }
    }

    /// The state word of the horizon of `key` now: the march, and the result of every chunk it reads (0 for one that is not ready).
    fn word(&self, key: ChunkKey, world: u64) -> u64 {
        let mut h = planet_core::hash::mix64(self.baker.spec_hash() ^ 0x484f_5249_5a4f_4e00);
        h = planet_core::hash::mix64(h ^ self.baker.edit_word(key));
        h = planet_core::hash::mix64(h ^ world);
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
        let world_word = self.baker.world_word();
        let words: Vec<Option<u64>> = wanted.iter().map(|&(k, _)| self.chunks.is_ready(k).then(|| self.word(k, world_word))).collect();
        {
            let mut sh = self.shared.lock().expect("horizon state");
            sh.updates += 1;
            let n = sh.updates;
            for &(k, _) in wanted {
                sh.wanted_at.insert(k, n);
            }
            let gone: Vec<ChunkKey> = {
                let mut v: Vec<ChunkKey> = sh.ready.keys().filter(|k| !self.chunks.is_ready(**k)).copied().collect();
                if !self.chunks.evicts() {
                    // The source never evicts: keep the wanted maps and the most recently wanted of the others, up to the cap.
                    let mut others: Vec<(u64, ChunkKey)> =
                        sh.ready.keys().filter(|k| sh.wanted_at.get(*k) != Some(&n)).map(|k| (sh.wanted_at.get(k).copied().unwrap_or(0), *k)).collect();
                    others.sort_unstable();
                    let excess = sh.ready.len().saturating_sub(wanted.len() + self.cfg.keep);
                    v.extend(others.into_iter().take(excess).map(|(_, k)| k));
                }
                v.sort_unstable();
                v.dedup();
                v
            };
            for k in &gone {
                sh.ready.remove(k);
                sh.seen.remove(k);
                sh.wanted_at.remove(k);
                sh.ever_baked.remove(k);
            }
            // The bookkeeping of keys that are neither held, running nor wanted now is dropped (a key wanted once and never baked would stay for ever).
            {
                let HorizonShared { ready, running, wanted_at, seen, .. } = &mut *sh;
                wanted_at.retain(|k, at| *at == n || ready.contains_key(k) || running.contains(k));
                seen.retain(|k, _| wanted_at.contains_key(k));
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
            self.spawner.spawn_in(JobClass::Idle, self.job(key, word));
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
            let bytes = match bytes {
                Some(b) => b,
                None => {
                    let ready: Vec<(ChunkKey, Arc<ChunkResult>)> =
                        baker.reads(key).into_iter().filter_map(|r| chunks.result(r).map(|res| (r, res))).collect();
                    let Some(made) = baker.bake(key, &ready) else {
                        // Stale before it was done: nothing is stored or published, the next update bakes it against the new word.
                        let mut sh = shared.lock().expect("horizon state");
                        sh.running.remove(&key);
                        sh.stats.cancelled += 1;
                        return;
                    };
                    if let Some(disk) = &disk {
                        if let Err(e) = disk.put(tile, word, made.clone()) {
                            shared.lock().expect("horizon state").log.push(format!("horizon {key:?}: not stored ({e})"));
                        }
                    }
                    made
                }
            };
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

    /// The edit word of `key` now (0: no edit reaches the chunk or the halo of its march).
    pub fn edit_word(&self, key: ChunkKey) -> u64 {
        self.baker.edit_word(key)
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
