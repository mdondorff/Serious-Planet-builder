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
    let m = manager(spawner.clone(), clock.clone(), None, ChunkConfig { workers: 6, ..ChunkConfig::default() });
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
    clock.advance(3_000);
    m.update(&[(key(0, 0), 10.0)]);
    clock.advance(3_000);
    let up = m.update(&[(key(0, 0), 10.0)]);
    assert_eq!(up.evicted, vec![key(1, 0)], "the one not wanted for 6 s (> 5 s) is gone, the wanted one stays");
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
    clock.advance(6_000);
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
    clock.advance(6_000);
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
