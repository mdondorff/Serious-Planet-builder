//! STRM-039, STRM-041: chunk jobs never block a frame, land nearest first, are evicted by need and come back from the disk cache.

use std::path::PathBuf;
use std::sync::Arc;

use planet_cache::chunk_store::{ChunkSpec, PackChunkDisk};
use planet_generators::chunkfield::{ChunkKey, ChunkLattice, StandIn, PYRAMID_TOP_LEVEL};
use planet_generators::height::BodyId;
use planet_streaming::chunks::{ChunkConfig, ChunkDisk, ChunkManager};
use planet_streaming::{FakeClock, ManualSpawner, Spawner, TimedSpawner};

const R: f64 = 6_371_000.0;

fn stage() -> StandIn {
    StandIn { seed: 7, radius_m: R, amplitude_m: 30.0, steps: 3 }
}

fn manager(spawner: Arc<dyn Spawner>, clock: Arc<FakeClock>, disk: Option<Arc<dyn ChunkDisk>>, cfg: ChunkConfig) -> ChunkManager {
    ChunkManager::new(ChunkLattice::new(16).unwrap(), R, PYRAMID_TOP_LEVEL, Arc::new(stage()), spawner, clock, disk, cfg)
}

fn key(x: u32, y: u32) -> ChunkKey {
    (2, 1000 + x, 1000 + y)
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join("planet-chunks-test").join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

// spec: STRM-039
#[test]
fn jobs_start_nearest_first_at_most_workers_at_once_and_no_update_waits_for_one() {
    let (spawner, clock) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()));
    let m = manager(spawner.clone(), clock, None, ChunkConfig { workers: 2, ..ChunkConfig::default() });
    let wanted = [(key(3, 0), 3000.0), (key(0, 0), 100.0), (key(2, 0), 2000.0), (key(1, 0), 900.0)];
    let up = m.update(&wanted);
    assert_eq!(up.started, 2, "at most `workers` at once");
    assert_eq!(spawner.pending(), 2, "the update returned with its jobs still waiting: no frame waits for a chunk");
    assert!(up.landed.is_empty() && m.snapshot().is_empty(), "nothing is ready, the frame draws the base");
    // Asking again while the two run starts nothing.
    assert_eq!(m.update(&wanted).started, 0);
    assert_eq!(spawner.run(2), 2);
    let up = m.update(&wanted);
    assert_eq!(up.landed, vec![key(0, 0), key(1, 0)], "the nearest two landed, in order");
    assert_eq!(up.started, 2, "the freed workers take the next nearest");
    spawner.run(2);
    let up = m.update(&wanted);
    assert_eq!(up.landed, vec![key(2, 0), key(3, 0)]);
    assert_eq!(m.stats().landed, 4);
    assert_eq!(m.snapshot().len(), 4);
}

// spec: STRM-039
#[test]
fn a_scripted_flight_and_pan_on_the_fake_clock_lands_every_chunk_and_the_stand_in_share_falls_to_zero() {
    // 6 workers, a chunk of 25 km2 at 0.25 s per km2: 6.25 s per chunk.
    let (spawner, clock) = (Arc::new(TimedSpawner::new(6, 6_250_000)), Arc::new(FakeClock::default()));
    // The pan's chunk must have left RAM by then (unwanted for 20 s): this flight runs with the old 5 s eviction, so that its landing is what is timed.
    let m = manager(spawner.clone(), clock.clone(), None, ChunkConfig { workers: 6, evict_ms: 5_000, ..ChunkConfig::default() });
    // The view over a flight: from 30 km (many chunks wanted, far ones too) down to 1.8 m (the chunk under the camera), then a pan of one chunk.
    let around = |cx: i64, cy: i64, radius: i64| -> Vec<(ChunkKey, f64)> {
        let mut v = Vec::new();
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                let (x, y) = ((1000 + cx + dx) as u32, (1000 + cy + dy) as u32);
                v.push(((2, x, y), 5000.0 * ((dx * dx + dy * dy) as f64).sqrt()));
            }
        }
        v
    };
    let (mut first_landing_after_pan_ms, mut shares) = (None, Vec::new());
    let mut pan_start_ms = None;
    for frame in 0..1200u64 {
        // 100 ms frames: 120 s of model time. 0 to 40 s: the descent (radius 2 shrinking to 0); then hold; at 60 s a pan to the next chunk.
        let t_ms = frame * 100;
        let wanted = match t_ms {
            0..=19_999 => around(0, 0, 2),
            20_000..=39_999 => around(0, 0, 1),
            40_000..=59_999 => around(0, 0, 0),
            _ => around(1, 0, 0),
        };
        if t_ms == 60_000 {
            pan_start_ms = Some(t_ms);
        }
        let up = m.update(&wanted);
        assert!(m.in_flight() <= 6, "never more than `workers` jobs");
        if let (Some(start), None) = (pan_start_ms, first_landing_after_pan_ms) {
            if up.landed.contains(&(2, 1001, 1000)) {
                first_landing_after_pan_ms = Some(t_ms - start);
            }
        }
        let not_ready = wanted.iter().filter(|(k, _)| !m.is_ready(*k)).count();
        shares.push(not_ready as f64 / wanted.len() as f64);
        clock.advance(100);
        spawner.advance(100_000);
    }
    assert!(shares[0] == 1.0, "at the start every wanted chunk is the stand-in");
    assert_eq!(*shares.last().unwrap(), 0.0, "the stand-in share ends at zero");
    let landed_at = first_landing_after_pan_ms.expect("the pan's chunk landed");
    assert!(landed_at <= 10_000, "the chunk under the camera landed within 10 s of the pan, {landed_at} ms");
    assert_eq!(m.stats().landed, 25, "every chunk of the widest view landed once (the pan's chunk is among them): {:?}", m.stats());
}

// spec: STRM-041
#[test]
fn a_chunk_not_wanted_for_the_evict_time_leaves_ram_and_the_least_recently_wanted_go_first_beyond_the_cap() {
    let (spawner, clock) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()));
    let m = manager(spawner.clone(), clock.clone(), None, ChunkConfig::default());
    let both = [(key(0, 0), 10.0), (key(1, 0), 20.0)];
    m.update(&both);
    spawner.run(2);
    assert_eq!(m.update(&both).landed.len(), 2);
    assert!(m.pool_bytes() > 0);
    clock.advance(16_000);
    m.update(&[(key(0, 0), 10.0)]);
    clock.advance(16_000);
    let up = m.update(&[(key(0, 0), 10.0)]);
    assert_eq!(up.evicted, vec![key(1, 0)], "the one not wanted for 32 s (> 30 s) is gone, the wanted one stays");
    assert!(m.is_ready(key(0, 0)) && !m.is_ready(key(1, 0)));
    // A byte cap below the pool evicts the least recently wanted first.
    let (spawner2, clock2) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()));
    let one = m.pool_bytes();
    let m2 = manager(spawner2.clone(), clock2.clone(), None, ChunkConfig { cap_bytes: one, ..ChunkConfig::default() });
    // The first chunk lands and is wanted at 0 ms; the second lands later and is the one wanted last: over the cap the first leaves.
    m2.update(&[(key(0, 0), 1.0)]);
    spawner2.run(1);
    m2.update(&[(key(0, 0), 1.0)]);
    clock2.advance(10);
    m2.update(&[(key(1, 0), 2.0)]);
    spawner2.run(1);
    clock2.advance(10);
    let up = m2.update(&[(key(1, 0), 2.0)]);
    assert_eq!(up.evicted, vec![key(0, 0)], "over the cap, the one wanted longest ago leaves");
}

// spec: STRM-041
#[test]
fn an_evicted_chunk_wanted_again_comes_back_from_disk_without_computing_and_a_corrupted_entry_recomputes_to_the_same_result() {
    let spec = ChunkSpec {
        body: BodyId::PLANET,
        seed: 7,
        radius_m: R,
        definition: planet_generators::chunkfield::ChunkStage::definition_hash(&stage()),
        cells_per_chunk: 16,
        top_level: PYRAMID_TOP_LEVEL,
        base: [0; 16],
    };
    let d = dir("evict-reload");
    let disk: Arc<dyn ChunkDisk> = Arc::new(PackChunkDisk::new(d.clone(), spec));
    let (spawner, clock) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()));
    let m = manager(spawner.clone(), clock.clone(), Some(disk), ChunkConfig::default());
    let want = [(key(0, 0), 1.0)];
    m.update(&want);
    spawner.run(1);
    m.update(&want);
    let first = m.snapshot()[0].hash();
    assert_eq!((m.stats().computed, m.stats().disk_hits), (1, 0));
    // Evicted, wanted again: the disk entry is read, nothing is computed.
    clock.advance(31_000);
    assert_eq!(m.update(&[]).evicted, vec![key(0, 0)]);
    m.update(&want);
    spawner.run(1);
    m.update(&want);
    assert_eq!((m.stats().computed, m.stats().disk_hits, m.stats().disk_failures), (1, 1, 0), "read from disk, not computed");
    assert_eq!(m.snapshot()[0].hash(), first);
    // Corrupt the stored entry: the next reload names the failure, recomputes and gives the identical result.
    let pack =
        std::fs::read_dir(&d).unwrap().flatten().map(|e| e.path()).find(|p| p.to_string_lossy().ends_with(".tpack")).expect("a pack file");
    let mut bytes = std::fs::read(&pack).unwrap();
    let n = bytes.len();
    bytes[n - 20] ^= 0xff;
    std::fs::write(&pack, bytes).unwrap();
    clock.advance(31_000);
    m.update(&[]);
    m.update(&want);
    spawner.run(1);
    m.update(&want);
    assert_eq!(m.stats().computed, 2, "recomputed");
    assert_eq!(m.stats().disk_failures, 1, "{:?}", m.stats());
    assert!(m.take_log().iter().any(|l| l.contains("recomputing")), "the failure is named in the log");
    assert_eq!(m.snapshot()[0].hash(), first, "the same result");
    let _ = std::fs::remove_dir_all(&d);
}

/// A stage whose source is gone: it computes, and says the result is not good.
struct Unhealthy(StandIn);

impl planet_generators::chunkfield::ChunkStage for Unhealthy {
    fn step_cap(&self) -> u32 {
        self.0.step_cap()
    }
    fn bound_m(&self) -> f64 {
        self.0.bound_m()
    }
    fn definition_hash(&self) -> u64 {
        self.0.definition_hash()
    }
    fn compute(&self, lat: &ChunkLattice, region: &planet_generators::chunkfield::Region) -> Vec<f32> {
        self.0.compute(lat, region)
    }
    fn healthy(&self) -> bool {
        false
    }
}

// spec: STRM-046
#[test]
fn a_result_of_a_stage_that_lost_its_source_lands_but_is_not_written_to_disk() {
    use planet_generators::chunkfield::ChunkStage;
    let spec = ChunkSpec {
        body: BodyId::PLANET,
        seed: 7,
        radius_m: R,
        definition: stage().definition_hash(),
        cells_per_chunk: 16,
        top_level: PYRAMID_TOP_LEVEL,
        base: [0; 16],
    };
    let d = dir("unhealthy");
    let disk = Arc::new(PackChunkDisk::new(d.clone(), spec));
    let (spawner, clock) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()));
    let m = ChunkManager::new(
        ChunkLattice::new(16).unwrap(),
        R,
        PYRAMID_TOP_LEVEL,
        Arc::new(Unhealthy(stage())),
        spawner.clone(),
        clock,
        Some(disk.clone() as Arc<dyn ChunkDisk>),
        ChunkConfig::default(),
    );
    let want = [(key(0, 0), 1.0)];
    m.update(&want);
    spawner.run(1);
    m.update(&want);
    assert_eq!(m.stats().landed, 1, "the chunk is in memory for this session");
    let tile = planet_core::TileId::new(planet_core::Face(2), planet_generators::chunk::CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
    assert_eq!(disk.get(tile), Ok(None), "nothing was written for a later session to read");
}

/// A version source the test moves: the version of every chunk, as the edits stand.
struct Versions(std::sync::atomic::AtomicU64);

impl planet_streaming::chunks::ChunkVersions for Versions {
    fn version(&self, _key: ChunkKey) -> u64 {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

// spec: STRM-056
#[test]
fn a_stale_chunk_keeps_drawing_is_re_solved_only_when_wanted_after_the_missing_ones_and_with_at_most_two_workers() {
    let (spawner, clock) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()));
    let v = Arc::new(Versions(0.into()));
    let cfg = ChunkConfig { workers: 3, stale_workers: 2, ..ChunkConfig::default() };
    let m = manager(spawner.clone(), clock, None, cfg).with_versions(v.clone());
    let keys: Vec<(ChunkKey, f64)> = (0..4).map(|x| (key(x, 0), 100.0 * f64::from(x) + 100.0)).collect();
    m.update(&keys);
    spawner.run(3);
    m.update(&keys);
    spawner.run(1);
    m.update(&keys);
    assert_eq!(m.stats().landed, 4);
    let before = m.snapshot();
    // An edit: every chunk is now at version 9. Nothing is dropped, and a chunk nobody wants is not re-solved.
    v.0.store(9, std::sync::atomic::Ordering::SeqCst);
    let up = m.update(&keys[..1]);
    assert!(up.landed.is_empty() && up.evicted.is_empty());
    assert_eq!(up.started, 1, "the one wanted stale chunk is re-solved, the others are not wanted");
    assert_eq!(m.snapshot().len(), 4, "the old results keep drawing while the new one is made");
    assert!(Arc::ptr_eq(&m.snapshot()[0], &before[0]), "the old result is still the one drawn");
    // Four wanted at once: at most two stale jobs run, nearest first, and a new (missing) chunk still gets a worker.
    let want_all: Vec<(ChunkKey, f64)> = keys.iter().copied().chain([(key(9, 9), 5000.0)]).collect();
    spawner.run(1);
    let up = m.update(&want_all);
    assert_eq!(up.landed, vec![key(0, 0)], "the re-solved chunk lands as a second landing");
    assert_eq!(up.started, 3, "the missing chunk first, then two stale ones (at most `stale_workers` stale jobs)");
    assert_eq!(m.in_flight(), 3);
    // Once they land, the next stale chunk is taken, and an unchanged version starts nothing.
    spawner.run(3);
    let up = m.update(&want_all);
    assert_eq!(up.landed.len(), 3);
    spawner.run(2);
    m.update(&want_all);
    assert_eq!(m.update(&want_all).started, 0, "every chunk is at the current version");
}

// spec: GEN-082
#[test]
fn the_disk_keeps_a_chunk_per_version_and_the_first_version_keeps_the_key_it_always_had() {
    let spec = ChunkSpec {
        body: BodyId::PLANET,
        seed: 7,
        radius_m: R,
        definition: planet_generators::chunkfield::ChunkStage::definition_hash(&stage()),
        cells_per_chunk: 16,
        top_level: PYRAMID_TOP_LEVEL,
        base: [0; 16],
    };
    let tile = planet_core::TileId::new(planet_core::Face(2), planet_generators::chunk::CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
    assert_eq!(spec.key_versioned(tile, 0), spec.key(tile), "version 0 is the key of every chunk stored before versions");
    assert_ne!(spec.key_versioned(tile, 5), spec.key(tile));
    assert_ne!(spec.key_versioned(tile, 5), spec.key_versioned(tile, 6));
    let d = dir("versions");
    let disk = PackChunkDisk::new(d, spec);
    disk.put_versioned(tile, 5, vec![1, 2, 3]).unwrap();
    assert_eq!(disk.get_versioned(tile, 5).unwrap(), Some(vec![1, 2, 3]));
    assert_eq!(disk.get_versioned(tile, 6).unwrap(), None, "another version is a miss");
    assert_eq!(disk.get(tile).unwrap(), None, "so is the first");
}

/// The stand-in stage with an epoch the test moves, and one that moves while a chunk is computed.
struct Moving {
    inner: StandIn,
    epoch: std::sync::atomic::AtomicU64,
    moves_in_compute: bool,
}

impl planet_generators::chunkfield::ChunkStage for Moving {
    fn step_cap(&self) -> u32 {
        self.inner.step_cap()
    }
    fn bound_m(&self) -> f64 {
        self.inner.bound_m()
    }
    fn compute(&self, lat: &ChunkLattice, region: &planet_generators::chunkfield::Region) -> Vec<f32> {
        if self.moves_in_compute {
            self.epoch.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        self.inner.compute(lat, region)
    }
    fn halo_rings(&self) -> u16 {
        self.inner.halo_rings()
    }
    fn definition_hash(&self) -> u64 {
        self.inner.definition_hash()
    }
    fn epoch(&self) -> u64 {
        self.epoch.load(std::sync::atomic::Ordering::SeqCst)
    }
}

// spec: GEN-082
#[test]
fn a_chunk_computed_while_the_stage_inputs_moved_is_not_stored_and_is_solved_again() {
    let spec = ChunkSpec {
        body: BodyId::PLANET,
        seed: 7,
        radius_m: R,
        definition: planet_generators::chunkfield::ChunkStage::definition_hash(&stage()),
        cells_per_chunk: 16,
        top_level: PYRAMID_TOP_LEVEL,
        base: [0; 16],
    };
    let d = dir("moved");
    let disk = Arc::new(PackChunkDisk::new(d, spec));
    let (spawner, clock) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()));
    let moving = Arc::new(Moving { inner: stage(), epoch: 0.into(), moves_in_compute: true });
    let m = ChunkManager::new(
        ChunkLattice::new(16).unwrap(),
        R,
        PYRAMID_TOP_LEVEL,
        moving.clone(),
        spawner.clone(),
        clock,
        Some(disk.clone() as Arc<dyn ChunkDisk>),
        ChunkConfig::default(),
    )
    .with_versions(Arc::new(Versions(0.into())));
    let tile = planet_core::TileId::new(planet_core::Face(2), planet_generators::chunk::CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
    let wanted = [(key(0, 0), 100.0)];
    m.update(&wanted);
    spawner.run(1);
    let up = m.update(&wanted);
    assert_eq!(up.landed, vec![key(0, 0)], "the result lands, so the frame has something to draw");
    assert_eq!(disk.get(tile).unwrap(), None, "but it is not stored: the inputs moved while it was made");
    // It is not trusted as its version: the chunk is stale at once and solved again (the stage keeps moving here, so it stays untrusted).
    assert_eq!(up.started, 1, "an untrusted chunk is solved again when wanted");
    spawner.run(1);
    // With the inputs still, the next solve is trusted, stored, and nothing more starts.
    let still = Arc::new(Moving { inner: stage(), epoch: 5.into(), moves_in_compute: false });
    let spawner2 = Arc::new(ManualSpawner::default());
    let m2 = ChunkManager::new(
        ChunkLattice::new(16).unwrap(),
        R,
        PYRAMID_TOP_LEVEL,
        still,
        spawner2.clone(),
        Arc::new(FakeClock::default()),
        Some(disk.clone() as Arc<dyn ChunkDisk>),
        ChunkConfig::default(),
    )
    .with_versions(Arc::new(Versions(0.into())));
    m2.update(&wanted);
    spawner2.run(1);
    m2.update(&wanted);
    assert!(disk.get(tile).unwrap().is_some(), "a solve with still inputs is stored");
    assert_eq!(m2.update(&wanted).started, 0, "and trusted: nothing is solved again");
}

/// A version source that says which chunks have the edit in their own region.
struct NearVersions {
    version: std::sync::atomic::AtomicU64,
    near: Vec<ChunkKey>,
}

impl planet_streaming::chunks::ChunkVersions for NearVersions {
    fn version(&self, _key: ChunkKey) -> u64 {
        self.version.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn near_edit(&self, key: ChunkKey) -> bool {
        self.near.contains(&key)
    }
}

// spec: STRM-056
#[test]
fn with_the_default_pool_every_stale_chunk_of_a_view_is_solved_in_one_round_and_the_ones_with_the_edit_in_their_own_region_go_first() {
    let defaults = ChunkConfig::default();
    assert!(defaults.workers >= planet_streaming::chunks::VIEW_CHUNK_WORKERS, "the pool holds the chunks of a low view");
    let (spawner, clock) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()));
    let keys: Vec<(ChunkKey, f64)> = (0..6).map(|x| (key(x, 0), 100.0 * f64::from(x) + 100.0)).collect();
    // Chunk 5 (the farthest from the camera) holds the edit; 4 is only reached by the read set.
    let versions = Arc::new(NearVersions { version: 0.into(), near: vec![key(5, 0)] });
    let m = manager(spawner.clone(), clock, None, ChunkConfig { workers: 4, ..ChunkConfig::default() }).with_versions(versions.clone());
    for _ in 0..3 {
        m.update(&keys);
        spawner.run(4);
    }
    assert_eq!(m.stats().landed, 6);
    versions.version.store(7, std::sync::atomic::Ordering::SeqCst);
    // Six stale chunks wanted, four workers, every worker may serve stale chunks: four start at once, the one with the edit first although it is
    // the farthest from the camera, then the nearest of the rest.
    let up = m.update(&keys);
    assert_eq!(up.started, 4, "the default lets every worker serve stale chunks: one round for four chunks");
    spawner.run(4);
    let up = m.update(&keys);
    assert_eq!(up.landed.len(), 4);
    assert!(up.landed.contains(&key(5, 0)), "the chunk with the edit in its own region was in the first round");
    assert!(!up.landed.contains(&key(4, 0)), "a nearer chunk the read set only reaches waited: {:?}", up.landed);
}

// spec: STRM-056
#[test]
fn stale_chunks_wait_while_the_edit_is_being_drawn_and_start_at_the_event_or_at_the_cap() {
    let (spawner, clock) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()));
    let versions = Arc::new(Versions(0.into()));
    let cfg = ChunkConfig { workers: 4, stale_hold_cap_ms: 1_000, ..ChunkConfig::default() };
    let m = manager(spawner.clone(), clock.clone(), None, cfg).with_versions(versions.clone());
    let wanted: Vec<(ChunkKey, f64)> = (0..3).map(|x| (key(x, 0), 100.0 * f64::from(x) + 100.0)).collect();
    for _ in 0..2 {
        m.update(&wanted);
        spawner.run(4);
    }
    m.update(&wanted);
    assert_eq!(m.stats().landed, 3);
    // An edit, and the loop says its tiles are still being built: the stale chunks wait.
    versions.0.store(5, std::sync::atomic::Ordering::SeqCst);
    m.hold_stale(true);
    assert_eq!(m.update(&wanted).started, 0, "held at the first sight of the edit");
    clock.advance(600);
    assert_eq!(m.update(&wanted).started, 0, "still held 600 ms later");
    // The event: the edit's tiles are drawn, the hold is off, the stale chunks start at once (before the cap).
    m.hold_stale(false);
    assert_eq!(m.update(&wanted).started, 3, "released at the event");
    spawner.run(4);
    m.update(&wanted);
    // A second edit: held again, but a stuck refresh cannot hold erosion longer than the cap.
    versions.0.store(6, std::sync::atomic::Ordering::SeqCst);
    m.hold_stale(true);
    assert_eq!(m.update(&wanted).started, 0);
    clock.advance(999);
    assert_eq!(m.update(&wanted).started, 0, "one millisecond short of the cap");
    clock.advance(1);
    assert_eq!(m.update(&wanted).started, 3, "the cap ends the hold");
}

// spec: STRM-042
#[test]
fn a_prefetched_chunk_is_a_prefetch_job_and_a_wanted_chunk_submitted_later_runs_before_it() {
    let (spawner, clock) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()));
    let m = manager(spawner.clone(), clock, None, ChunkConfig { workers: 3, ..ChunkConfig::default() });
    // Two chunks ahead of the camera are prefetched while nothing is wanted...
    let up = m.update_with(&[], &[key(5, 0), key(6, 0)]);
    assert_eq!(up.started, 2);
    // ...then the plan wants a chunk: its job is submitted after theirs and the pool runs it first.
    assert_eq!(m.update_with(&[(key(0, 0), 100.0)], &[key(5, 0), key(6, 0)]).started, 1);
    assert_eq!(spawner.pending(), 3);
    assert!(spawner.run_one());
    assert_eq!(m.update(&[(key(0, 0), 100.0)]).landed, vec![key(0, 0)], "the wanted chunk lands before either prefetched one");
}

/// A version source that says on which thread each version was asked.
struct ThreadVersions {
    caller: std::thread::ThreadId,
    on_caller: std::sync::Mutex<Vec<ChunkKey>>,
    elsewhere: std::sync::atomic::AtomicUsize,
}

impl planet_streaming::chunks::ChunkVersions for ThreadVersions {
    fn version(&self, key: ChunkKey) -> u64 {
        if std::thread::current().id() == self.caller {
            self.on_caller.lock().unwrap().push(key);
        } else {
            self.elsewhere.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        5
    }
}

// spec: STRM-063
#[test]
fn the_version_of_a_chunk_that_is_not_ready_is_made_by_its_job_never_on_the_thread_that_updates() {
    let spawner = Arc::new(planet_streaming::ThreadPool::new(2));
    let versions = Arc::new(ThreadVersions { caller: std::thread::current().id(), on_caller: Default::default(), elsewhere: 0.into() });
    let m = manager(spawner, Arc::new(FakeClock::default()), None, ChunkConfig { workers: 2, ..ChunkConfig::default() })
        .with_versions(versions.clone());
    let keys: Vec<(ChunkKey, f64)> = (0..4).map(|x| (key(x, 0), 100.0 * f64::from(x) + 100.0)).collect();
    let started = std::time::Instant::now();
    m.update(&keys);
    assert!(
        versions.on_caller.lock().unwrap().is_empty(),
        "the first update, over four new chunks, asked for no version on its own thread"
    );
    while m.stats().landed < 4 {
        m.update(&keys);
        assert!(started.elapsed().as_secs() < 60, "the chunks did not land");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(versions.on_caller.lock().unwrap().iter().all(|k| m.is_ready(*k)), "the caller asked only for chunks that were ready");
    assert!(
        versions.elsewhere.load(std::sync::atomic::Ordering::SeqCst) >= 4,
        "each new chunk's job asked for its own version on a worker"
    );
    // The version a job made is the chunk's: nothing is stale, so a further update starts nothing.
    assert_eq!(m.update(&keys).started, 0);
}

/// A version source that lands the pending chunk job while the update is asking for the version of a ready chunk: a chunk that lands between the
/// ready snapshot and the lock of the update.
struct LandingVersions {
    spawner: Arc<ManualSpawner>,
    armed: std::sync::atomic::AtomicBool,
}

impl planet_streaming::chunks::ChunkVersions for LandingVersions {
    fn version(&self, key: ChunkKey) -> u64 {
        if key == crate::chunks::key(0, 0) && self.armed.swap(false, std::sync::atomic::Ordering::SeqCst) {
            self.spawner.run_one();
        }
        5
    }
}

// spec: STRM-063
#[test]
fn a_chunk_that_lands_while_the_update_asks_for_versions_is_not_judged_stale_with_a_version_it_was_never_asked() {
    let (spawner, clock) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()));
    let versions = Arc::new(LandingVersions { spawner: spawner.clone(), armed: false.into() });
    let m = manager(spawner.clone(), clock, None, ChunkConfig { workers: 2, ..ChunkConfig::default() }).with_versions(versions.clone());
    let first = [(key(0, 0), 100.0)];
    m.update(&first);
    spawner.run(1);
    m.update(&first);
    assert!(m.is_ready(key(0, 0)));
    // The second chunk is started; its job waits in the spawner.
    let both = [(key(0, 0), 100.0), (key(1, 0), 200.0)];
    assert_eq!(m.update(&both).started, 1);
    // The next update asks for the version of the ready chunk, and the job of the second lands inside that call.
    versions.armed.store(true, std::sync::atomic::Ordering::SeqCst);
    let up = m.update(&both);
    assert!(m.is_ready(key(1, 0)), "the second chunk landed during the update");
    assert_eq!(up.started, 0, "the chunk that landed is not stale: its version (5) was made by its job, not 0");
    assert_eq!(spawner.pending(), 0);
}
