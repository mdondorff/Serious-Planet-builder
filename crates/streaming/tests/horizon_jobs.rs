//! STRM-048: the horizon job is a second job per chunk that lands after its heights; a tile whose map is not there reads the coarse horizon
//! alone; a neighbour landing re-bakes it once the neighbourhood has settled, not once per neighbour; eviction takes the map with its chunk.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use planet_core::TileId;
use planet_generators::chunkfield::{ChunkKey, ChunkLattice, ChunkResult, StandIn, PYRAMID_TOP_LEVEL};
use planet_streaming::chunks::{BaseTerrain, ChunkConfig, ChunkManager, HorizonBaker, HorizonConfig, HorizonDisk, HorizonJobs};
use planet_streaming::{FakeClock, ManualSpawner};

const R: f64 = 6_371_000.0;

fn key(x: u32, y: u32) -> ChunkKey {
    (2, 1000 + x, 1000 + y)
}

/// A baker that reads the chunk and its eight neighbours on the grid, counts its bakes and returns how many neighbours were ready.
struct FakeBaker {
    bakes: AtomicU64,
    spec: u64,
    /// Bakes that report the heights went stale (the next ones this many are dropped).
    stale: AtomicU64,
}

impl HorizonBaker for FakeBaker {
    fn reads(&self, k: ChunkKey) -> Vec<ChunkKey> {
        let mut v = vec![k];
        for dx in -1i64..=1 {
            for dy in -1i64..=1 {
                if (dx, dy) != (0, 0) {
                    v.push((k.0, (i64::from(k.1) + dx) as u32, (i64::from(k.2) + dy) as u32));
                }
            }
        }
        v
    }
    fn bake(&self, k: ChunkKey, ready: &[(ChunkKey, Arc<ChunkResult>)]) -> Option<Vec<u8>> {
        self.bakes.fetch_add(1, Ordering::SeqCst);
        if self.stale.load(Ordering::SeqCst) > 0 {
            self.stale.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(vec![k.1 as u8, k.2 as u8, ready.len() as u8])
    }
    fn spec_hash(&self) -> u64 {
        self.spec
    }
}

fn chunks(clock: Arc<FakeClock>, spawner: Arc<ManualSpawner>) -> Arc<ChunkManager> {
    let stage = StandIn { seed: 7, radius_m: R, amplitude_m: 30.0, steps: 3 };
    Arc::new(ChunkManager::new(
        ChunkLattice::new(16).unwrap(),
        R,
        PYRAMID_TOP_LEVEL,
        Arc::new(stage),
        spawner,
        clock,
        None,
        ChunkConfig { workers: 16, ..ChunkConfig::default() },
    ))
}

/// Land the chunks `keys` (a frame of the chunk manager and the jobs it started run to the end).
fn land(m: &ChunkManager, spawner: &ManualSpawner, keys: &[ChunkKey]) {
    let wanted: Vec<(ChunkKey, f64)> = keys.iter().map(|k| (*k, 1.0)).collect();
    m.update(&wanted);
    spawner.run(keys.len());
    m.update(&wanted);
}

type Rig = (Arc<FakeClock>, Arc<ManualSpawner>, Arc<ManualSpawner>, Arc<ChunkManager>, Arc<FakeBaker>, HorizonJobs);

fn rig(settle_ms: u64, workers: usize) -> Rig {
    let clock = Arc::new(FakeClock::default());
    let (cs, hs) = (Arc::new(ManualSpawner::default()), Arc::new(ManualSpawner::default()));
    let m = chunks(clock.clone(), cs.clone());
    let baker = Arc::new(FakeBaker { bakes: AtomicU64::new(0), spec: 1, stale: AtomicU64::new(0) });
    let jobs = HorizonJobs::new(m.clone(), baker.clone(), hs.clone(), clock.clone(), None, HorizonConfig { workers, settle_ms, keep: 8 });
    (clock, cs, hs, m, baker, jobs)
}

// spec: STRM-048
#[test]
fn the_state_word_of_a_horizon_hashes_a_chunks_pyramid_once_however_many_frames_ask() {
    let (_clock, cs, _hs, m, _baker, jobs) = rig(1_000, 1);
    let keys: Vec<ChunkKey> = (0..3).flat_map(|x| (0..3).map(move |y| key(x, y))).collect();
    land(&m, &cs, &keys);
    let wanted: Vec<(ChunkKey, f64)> = keys.iter().map(|k| (*k, 1.0)).collect();
    // Every frame the state word of every wanted chunk reads the result hash of the nine chunks around it: with the hash made on every call that
    // was 9 x 9 hashes of the whole pyramid a frame, on the main thread and under the chunk-state lock.
    for _ in 0..50 {
        jobs.update(&wanted);
        for k in &keys {
            assert!(m.result_hash(*k).is_some());
        }
    }
    for k in &keys {
        assert_eq!(m.result(*k).unwrap().hash_runs(), 1, "the pyramid of {k:?} was hashed once, in the job that made it");
    }
}

// spec: STRM-048
#[test]
fn a_horizon_is_baked_only_after_its_chunk_is_ready_and_until_it_lands_the_tile_has_none() {
    let (_clock, cs, hs, m, baker, jobs) = rig(1_000, 1);
    let k = key(0, 0);
    let up = jobs.update(&[(k, 1.0)]);
    assert_eq!((up.started, hs.pending()), (0, 0), "no heights, no horizon job");
    assert!(jobs.horizon(k).is_none());
    land(&m, &cs, &[k]);
    let up = jobs.update(&[(k, 1.0)]);
    assert_eq!(up.started, 1, "the chunk is ready: its horizon job starts");
    assert_eq!(hs.pending(), 1, "the update returned with the job waiting: no frame waits for it");
    assert!(jobs.horizon(k).is_none(), "until it lands the coarse horizon alone shades the tile");
    assert_eq!(jobs.update(&[(k, 1.0)]).started, 0, "a running job is not started twice");
    hs.run(1);
    let up = jobs.update(&[(k, 1.0)]);
    assert_eq!(up.landed, vec![k]);
    assert_eq!(*jobs.horizon(k).unwrap(), vec![(1000 % 256) as u8, (1000 % 256) as u8, 1], "baked with only its own chunk ready");
    assert_eq!(baker.bakes.load(Ordering::SeqCst), 1);
    assert_eq!(jobs.update(&[(k, 1.0)]).started, 0, "nothing changed: nothing to bake");
}

// spec: STRM-048
#[test]
fn at_most_workers_run_at_once_and_the_nearest_chunk_goes_first() {
    let (_clock, cs, hs, m, _baker, jobs) = rig(1_000, 2);
    let keys = [key(0, 0), key(5, 0), key(10, 0), key(15, 0)];
    land(&m, &cs, &keys);
    let wanted = [(key(15, 0), 900.0), (key(0, 0), 100.0), (key(10, 0), 500.0), (key(5, 0), 300.0)];
    assert_eq!(jobs.update(&wanted).started, 2);
    hs.run(2);
    let up = jobs.update(&wanted);
    assert_eq!(up.landed, vec![key(0, 0), key(5, 0)], "the two nearest landed first");
    assert_eq!(up.started, 2);
}

// spec: STRM-048
#[test]
fn eight_neighbours_landing_one_after_another_cost_one_rebake_not_eight() {
    let (clock, cs, hs, m, baker, jobs) = rig(1_000, 1);
    let centre = key(5, 5);
    let around: Vec<ChunkKey> = baker.reads(centre).into_iter().skip(1).collect();
    land(&m, &cs, &[centre]);
    jobs.update(&[(centre, 1.0)]);
    hs.run(1);
    jobs.update(&[(centre, 1.0)]);
    assert_eq!(baker.bakes.load(Ordering::SeqCst), 1, "the first bake, with no neighbour ready");
    // One neighbour lands every 100 ms; the jobs are asked each frame. The centre's map is stale after the first landing, but waits.
    let mut seen = Vec::new();
    for (i, n) in around.iter().enumerate() {
        seen.push(*n);
        land(&m, &cs, &seen);
        clock.advance(100);
        let up = jobs.update(&[(centre, 1.0)]);
        assert_eq!(up.started, 0, "after neighbour {i} the neighbourhood has not settled: no bake yet");
    }
    clock.advance(1_000);
    assert_eq!(jobs.update(&[(centre, 1.0)]).started, 1, "once the neighbourhood has been quiet for the settle window: one bake");
    hs.run(1);
    jobs.update(&[(centre, 1.0)]);
    assert_eq!(*jobs.horizon(centre).unwrap().last().unwrap(), 9, "it read the chunk and all eight neighbours");
    clock.advance(10_000);
    assert_eq!(jobs.update(&[(centre, 1.0)]).started, 0, "and is up to date");
    let s = jobs.stats();
    assert_eq!((s.baked, s.rebaked), (2, 1), "one bake and one rebake in all: {s:?}");
}

// spec: STRM-048
#[test]
fn a_map_leaves_with_its_chunk() {
    let (clock, cs, hs, m, baker, jobs) = rig(0, 1);
    let k = key(0, 0);
    land(&m, &cs, &[k]);
    jobs.update(&[(k, 1.0)]);
    hs.run(1);
    jobs.update(&[(k, 1.0)]);
    assert!(jobs.horizon(k).is_some());
    // Not wanted for longer than the chunk's eviction time: the chunk leaves RAM and the map goes with it.
    clock.advance(31_000);
    let evicted = m.update(&[]).evicted;
    assert_eq!(evicted, vec![k]);
    let up = jobs.update(&[(k, 1.0)]);
    assert_eq!(up.evicted, vec![k]);
    assert!(jobs.horizon(k).is_none());
    assert_eq!(jobs.stats().evicted, 1);
    assert_eq!(baker.bakes.load(Ordering::SeqCst), 1, "the first jobs baked once");
}

// spec: STRM-048
#[test]
fn a_map_made_for_another_march_is_not_taken_from_the_disk_and_a_failing_entry_is_a_logged_miss() {
    let disk = Arc::new(MemDisk::default());
    let run = |spec: u64, disk: Arc<dyn HorizonDisk>, k: ChunkKey| {
        let clock = Arc::new(FakeClock::default());
        let (cs, hs) = (Arc::new(ManualSpawner::default()), Arc::new(ManualSpawner::default()));
        let m = chunks(clock.clone(), cs.clone());
        let baker = Arc::new(FakeBaker { bakes: AtomicU64::new(0), spec, stale: AtomicU64::new(0) });
        let jobs = HorizonJobs::new(m.clone(), baker.clone(), hs.clone(), clock, Some(disk), HorizonConfig::default());
        land(&m, &cs, &[k]);
        jobs.update(&[(k, 1.0)]);
        hs.run(1);
        jobs.update(&[(k, 1.0)]);
        (baker.bakes.load(Ordering::SeqCst), jobs)
    };
    let k = key(2, 2);
    let (bakes, first) = run(1, disk.clone(), k);
    assert_eq!((bakes, first.stats().disk_hits), (1, 0));
    // The same chunk and neighbourhood under the march of another version: a different state word, so a miss and a bake of its own.
    let (bakes, other) = run(2, disk.clone(), k);
    assert_eq!((bakes, other.stats().disk_hits), (1, 0), "the map of the first march is not taken for the second");
    assert_eq!(disk.maps.lock().unwrap().len(), 2, "each march has its own entry");
    // A disk whose reads fail: the map is baked, and the log names the chunk.
    struct Failing;
    impl HorizonDisk for Failing {
        fn get(&self, _: TileId, _: u64) -> Result<Option<Vec<u8>>, String> {
            Err("the pack is damaged".into())
        }
        fn put(&self, _: TileId, _: u64, _: Vec<u8>) -> Result<(), String> {
            Err("read only".into())
        }
    }
    let (bakes, broken) = run(1, Arc::new(Failing), k);
    assert_eq!(bakes, 1, "a failing entry is a miss: the map is baked");
    assert!(broken.horizon(k).is_some());
    let log = broken.take_log().join("\n");
    assert!(
        log.contains("disk entry refused")
            && log.contains("the pack is damaged")
            && log.contains("not stored")
            && log.contains(&format!("{k:?}")),
        "{log}"
    );
}

#[derive(Default)]
struct MemDisk {
    maps: Mutex<HashMap<(u64, u64), Vec<u8>>>,
    gets: AtomicU64,
}

impl HorizonDisk for MemDisk {
    fn get(&self, tile: TileId, word: u64) -> Result<Option<Vec<u8>>, String> {
        self.gets.fetch_add(1, Ordering::SeqCst);
        if word == 0 {
            return Err("damaged".into());
        }
        Ok(self.maps.lock().unwrap().get(&(tile.raw(), word)).cloned())
    }
    fn put(&self, tile: TileId, word: u64, bytes: Vec<u8>) -> Result<(), String> {
        self.maps.lock().unwrap().insert((tile.raw(), word), bytes);
        Ok(())
    }
}

// spec: STRM-048
#[test]
fn a_map_stored_for_the_same_neighbourhood_is_read_back_and_one_for_another_is_a_miss() {
    let disk = Arc::new(MemDisk::default());
    let make = |disk: Arc<MemDisk>| {
        let clock = Arc::new(FakeClock::default());
        let (cs, hs) = (Arc::new(ManualSpawner::default()), Arc::new(ManualSpawner::default()));
        let m = chunks(clock.clone(), cs.clone());
        let baker = Arc::new(FakeBaker { bakes: AtomicU64::new(0), spec: 1, stale: AtomicU64::new(0) });
        let jobs =
            HorizonJobs::new(m.clone(), baker.clone(), hs.clone(), clock, Some(disk as Arc<dyn HorizonDisk>), HorizonConfig::default());
        (cs, hs, m, baker, jobs)
    };
    let k = key(3, 3);
    let (cs, hs, m, baker, jobs) = make(disk.clone());
    land(&m, &cs, &[k]);
    jobs.update(&[(k, 1.0)]);
    hs.run(1);
    jobs.update(&[(k, 1.0)]);
    assert_eq!(baker.bakes.load(Ordering::SeqCst), 1);
    let first = (*jobs.horizon(k).unwrap()).clone();
    // A second run of the same world: the same chunk with the same neighbourhood reads the map from the disk, no bake.
    let (cs, hs, m, baker, jobs) = make(disk.clone());
    land(&m, &cs, &[k]);
    jobs.update(&[(k, 1.0)]);
    hs.run(1);
    jobs.update(&[(k, 1.0)]);
    assert_eq!((baker.bakes.load(Ordering::SeqCst), jobs.stats().disk_hits), (0, 1));
    assert_eq!(*jobs.horizon(k).unwrap(), first);
    // With a neighbour ready the state word differs: a miss, a bake, a second entry on the disk.
    let (cs, hs, m, baker, jobs) = make(disk.clone());
    land(&m, &cs, &[k, key(4, 3)]);
    jobs.update(&[(k, 1.0)]);
    hs.run(1);
    jobs.update(&[(k, 1.0)]);
    assert_eq!((baker.bakes.load(Ordering::SeqCst), jobs.stats().disk_hits), (1, 0));
    assert_eq!(disk.maps.lock().unwrap().len(), 2);
    assert!(disk.gets.load(Ordering::SeqCst) >= 3);
}

// spec: STRM-061
#[test]
fn a_bake_that_went_stale_is_dropped_never_published_and_baked_again() {
    let (_clock, cs, hs, m, baker, jobs) = rig(1_000, 1);
    let k = key(0, 0);
    land(&m, &cs, &[k]);
    baker.stale.store(1, Ordering::SeqCst);
    assert_eq!(jobs.update(&[(k, 1.0)]).started, 1);
    hs.run(1);
    let up = jobs.update(&[(k, 1.0)]);
    assert!(up.landed.is_empty(), "a stale bake landed");
    assert!(jobs.horizon(k).is_none(), "a stale bake was published");
    let s = jobs.stats();
    assert_eq!((s.cancelled, s.landed, s.baked), (1, 0, 0));
    assert_eq!(jobs.in_flight(), 1, "the dropped job gave its slot back and the chunk is baked again");
    // The next update bakes the chunk again and that one lands.
    assert_eq!(up.started, 1, "the dropped chunk is baked again");
    hs.run(1);
    assert_eq!(jobs.update(&[(k, 1.0)]).landed, vec![k]);
    assert!(jobs.horizon(k).is_some());
}

/// A baker whose heights an edit changes: its edit word is set by the test.
struct EditBaker {
    word: AtomicU64,
}

impl HorizonBaker for EditBaker {
    fn reads(&self, k: ChunkKey) -> Vec<ChunkKey> {
        vec![k]
    }
    fn bake(&self, k: ChunkKey, ready: &[(ChunkKey, Arc<ChunkResult>)]) -> Option<Vec<u8>> {
        assert!(ready.is_empty(), "no chunk stage: no chunk results");
        Some(vec![k.1 as u8, self.word.load(Ordering::SeqCst) as u8])
    }
    fn spec_hash(&self) -> u64 {
        1
    }
    fn edit_word(&self, _k: ChunkKey) -> u64 {
        self.word.load(Ordering::SeqCst)
    }
}

// spec: STRM-064
#[test]
fn without_a_chunk_stage_the_wanted_chunks_are_baked_from_the_base_kept_to_the_cap_and_baked_again_when_an_edit_reaches_them() {
    let clock = Arc::new(FakeClock::default());
    let hs = Arc::new(ManualSpawner::default());
    let baker = Arc::new(EditBaker { word: AtomicU64::new(0) });
    let cfg = HorizonConfig { workers: 4, settle_ms: 1_000, keep: 2 };
    let jobs = HorizonJobs::new(Arc::new(BaseTerrain), baker.clone(), hs.clone(), clock.clone(), None, cfg);
    let (a, b) = (key(0, 0), key(1, 0));
    assert_eq!(jobs.update(&[(a, 1.0), (b, 2.0)]).started, 2, "every chunk is ready with no chunk stage: both start at once");
    hs.run(2);
    let up = jobs.update(&[(a, 1.0), (b, 2.0)]);
    assert_eq!(up.landed.len(), 2);
    assert!(jobs.horizon(a).is_some() && jobs.horizon(b).is_some());
    assert_eq!(jobs.stats().baked, 2);
    // An edit reaches both: their words change; the maps are baked again once the word has been unchanged for the settle window.
    baker.word.store(7, Ordering::SeqCst);
    assert_eq!(jobs.update(&[(a, 1.0), (b, 2.0)]).started, 0, "settling");
    clock.advance(1_000);
    assert_eq!(jobs.update(&[(a, 1.0), (b, 2.0)]).started, 2);
    hs.run(2);
    jobs.update(&[(a, 1.0), (b, 2.0)]);
    assert_eq!(jobs.stats().rebaked, 2);
    assert_eq!(jobs.horizon(a).unwrap().as_slice(), &[(1000 % 256) as u8, 7]);
    // The camera moves on: of the maps no longer wanted the `keep` (2) most recently wanted stay beyond the wanted ones, the older ones go.
    let (c, d, e, f) = (key(5, 0), key(6, 0), key(7, 0), key(8, 0));
    let next = |wanted: [(ChunkKey, f64); 2]| {
        jobs.update(&wanted);
        hs.run(2);
        jobs.update(&wanted)
    };
    assert_eq!(next([(c, 1.0), (d, 2.0)]).landed.len(), 2);
    assert!(jobs.horizon(c).is_some() && jobs.horizon(d).is_some(), "the wanted maps stay");
    assert!(jobs.horizon(a).is_some() && jobs.horizon(b).is_some(), "a and b are the 2 kept beyond the 2 wanted");
    assert_eq!(next([(e, 1.0), (f, 2.0)]).landed.len(), 2);
    assert!(jobs.horizon(e).is_some() && jobs.horizon(f).is_some(), "the wanted maps stay");
    assert!(jobs.horizon(c).is_some() && jobs.horizon(d).is_some(), "the two most recently wanted are the ones kept");
    assert!(jobs.horizon(a).is_none() && jobs.horizon(b).is_none(), "the older unwanted maps went");
}

/// A baker whose world word (the lens, the base, the stage) is set by the test.
struct WorldBaker {
    world: AtomicU64,
}

impl HorizonBaker for WorldBaker {
    fn reads(&self, k: ChunkKey) -> Vec<ChunkKey> {
        vec![k]
    }
    fn bake(&self, k: ChunkKey, _ready: &[(ChunkKey, Arc<ChunkResult>)]) -> Option<Vec<u8>> {
        Some(vec![k.1 as u8, self.world.load(Ordering::SeqCst) as u8])
    }
    fn spec_hash(&self) -> u64 {
        1
    }
    fn world_word(&self) -> u64 {
        self.world.load(Ordering::SeqCst)
    }
}

// spec: STRM-064
#[test]
fn a_change_of_the_world_word_bakes_every_wanted_map_again_after_the_settle_window() {
    let clock = Arc::new(FakeClock::default());
    let hs = Arc::new(ManualSpawner::default());
    let baker = Arc::new(WorldBaker { world: AtomicU64::new(0) });
    let jobs = HorizonJobs::new(Arc::new(BaseTerrain), baker.clone(), hs.clone(), clock.clone(), None, HorizonConfig::default());
    let k = key(0, 0);
    assert_eq!(jobs.update(&[(k, 1.0)]).started, 1);
    hs.run(1);
    jobs.update(&[(k, 1.0)]);
    // The coupled lens is pressed: nothing in the edit word changed, the world word did.
    baker.world.store(5, Ordering::SeqCst);
    assert_eq!(jobs.update(&[(k, 1.0)]).started, 0, "settling");
    clock.advance(1_000);
    assert_eq!(jobs.update(&[(k, 1.0)]).started, 1, "baked again for the new world");
    hs.run(1);
    jobs.update(&[(k, 1.0)]);
    assert_eq!(jobs.horizon(k).unwrap().as_slice(), &[(1000 % 256) as u8, 5]);
}
