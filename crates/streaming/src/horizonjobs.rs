//! The horizon jobs (M8-5, STRM-048, STRM-064): a map per chunk, baked from the base terrain and the stamps once its chunk is wanted, kept in RAM
//! and on the pack, never waited for (CON-22). The clock and the spawner are injected (CON-04).

use crate::{Clock, JobClass, Spawner};
use planet_core::{Face, TileId};
use planet_generators::chunk::CHUNK_GRID_LEVEL;
use planet_generators::chunkfield::{ChunkKey, ChunkResult};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

fn tile_of(key: ChunkKey) -> TileId {
    TileId::new(Face(key.0), CHUNK_GRID_LEVEL, key.1, key.2).expect("a chunk of the grid is a tile")
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
    /// A word that differs when an edit can change the heights the horizon of `key` samples (the items that touch the chunk and its halo),
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

/// What the horizon jobs need to know of the chunk stage: which chunks are ready and their results. [`BaseTerrain`] is the source: the world with no chunk stage (every chunk is ready and has no change), so the maps are baked from the base terrain and the stamps alone (STRM-064).
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
        // The state word of each wanted chunk that is ready, made before the horizon lock is taken (it reads the source's state).
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
                    let mut others: Vec<(u64, ChunkKey)> = sh
                        .ready
                        .keys()
                        .filter(|k| sh.wanted_at.get(*k) != Some(&n))
                        .map(|k| (sh.wanted_at.get(k).copied().unwrap_or(0), *k))
                        .collect();
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
