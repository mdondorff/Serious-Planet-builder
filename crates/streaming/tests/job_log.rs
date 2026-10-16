//! The job log (STRM-022), tier A: the streamer records, for each tile job that ran its load, how long it waited, how long the
//! load took, how many loads ran beside it and whether it produced a tile. The readings are wall-clock and recorded only; these
//! tests count (jobs, concurrency, levels, outcomes) and assert nothing about a duration.

use planet_core::{Face, TileId};
use planet_streaming::{FakeClock, ManualSpawner, Spawner, StreamConfig, Streamer, ThreadPool, TileSource};
use std::sync::{Arc, Barrier};

fn tile(level: u8, x: u32) -> TileId {
    TileId::new(Face(0), level, x, 0).unwrap()
}

struct Plain;

impl TileSource<u64> for Plain {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        Ok(tile.raw())
    }
}

/// Fails every tile of level 3.
struct Failing;

impl TileSource<u64> for Failing {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        if tile.level() == 3 {
            Err("no".into())
        } else {
            Ok(tile.raw())
        }
    }
}

// spec: STRM-022
#[test]
fn every_job_that_ran_is_logged_with_its_level_and_one_load_at_a_time_on_a_manual_spawner() {
    let spawner = Arc::new(ManualSpawner::default());
    let mut s = Streamer::new(
        Arc::new(Plain),
        spawner.clone() as Arc<dyn Spawner>,
        Arc::new(FakeClock::default()),
        StreamConfig { max_in_flight: 4, capacity: 1000, record_dispatches: true, ..Default::default() },
    );
    s.load_roots_blocking().unwrap();
    let log = s.job_log();
    let roots = log.len();
    assert_eq!(roots, 0, "the six roots are loaded outside the scheduler's dispatch: not part of the log");
    let wants: Vec<(TileId, f64)> = (0..6).map(|x| (tile(3, x), f64::from(x))).chain([(tile(1, 0), 0.0), (tile(1, 1), 1.0)]).collect();
    s.set_draw(&wants);
    let mut guard = 0;
    while s.stats().completed < 8 && guard < 100 {
        s.step();
        spawner.run(4);
        guard += 1;
    }
    s.step();
    let samples = log.since(0);
    assert_eq!(samples.len() as u64, s.stats().dispatched, "one sample per job that ran");
    assert!(samples.iter().all(|j| j.done && j.running == 1), "{samples:?}");
    let mut levels: Vec<u8> = samples.iter().map(|j| j.level).collect();
    levels.sort_unstable();
    let mut dispatched: Vec<u8> = s.dispatch_log.iter().map(|t| t.level()).collect();
    dispatched.sort_unstable();
    assert_eq!(levels, dispatched, "the logged levels are the levels of the tiles that were dispatched");
    assert!(levels.contains(&3) && levels.contains(&2), "the draw set and its implied ancestors: {levels:?}");
    assert_eq!(log.since(3).len(), samples.len() - 3, "since() is the tail from an index");
    assert!(log.since(100).is_empty());
}

// spec: STRM-022
#[test]
fn a_failed_load_is_logged_as_not_done_and_a_job_cancelled_before_it_starts_is_not_logged() {
    let spawner = Arc::new(ManualSpawner::default());
    let mut s = Streamer::new(
        Arc::new(Failing),
        spawner.clone() as Arc<dyn Spawner>,
        Arc::new(FakeClock::default()),
        StreamConfig { max_in_flight: 4, capacity: 1000, retry_after_ms: 1_000_000, ..Default::default() },
    );
    s.load_roots_blocking().unwrap();
    s.set_draw(&[(tile(3, 0), 0.0), (tile(3, 1), 1.0), (tile(1, 0), 2.0)]);
    for _ in 0..20 {
        s.step();
        spawner.run(100);
    }
    s.step();
    let samples = s.job_log().since(0);
    let level3 = samples.iter().filter(|j| j.level == 3).count();
    assert_eq!(level3, 2, "each level-3 tile was tried once (a long retry delay): {samples:?}");
    assert!(samples.iter().all(|j| j.done == (j.level != 3)), "the level-3 loads failed and every other load produced a tile: {samples:?}");
    // A job dispatched and then cancelled before a worker takes it never reaches the source: nothing to log.
    let logged = samples.len();
    s.set_draw(&[(tile(4, 0), 0.0), (tile(4, 1), 1.0)]);
    s.step();
    let dispatched_before_cancel = s.stats().dispatched;
    assert!(spawner.pending() > 0 && s.stats().in_flight > 0, "a job is waiting in the spawner: {:?}", s.stats());
    s.cancel_all();
    spawner.run(10);
    assert_eq!(s.stats().dispatched, dispatched_before_cancel);
    assert_eq!(s.job_log().len(), logged, "cancelled jobs are not part of the log");
}

/// A source whose loads meet at a barrier once armed (the roots, loaded first, would never meet three others): with N workers, N
/// loads are inside `load` at the same moment.
struct Switch(Barrier, std::sync::atomic::AtomicBool);

impl TileSource<u64> for Switch {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        if self.1.load(std::sync::atomic::Ordering::SeqCst) {
            self.0.wait();
        }
        Ok(tile.raw())
    }
}

// spec: STRM-022
#[test]
fn the_concurrency_a_load_ran_under_is_the_number_of_loads_inside_the_source_when_it_started() {
    let source = Arc::new(Switch(Barrier::new(4), false.into()));
    let pool = Arc::new(ThreadPool::new(4));
    let mut s = Streamer::new(
        Arc::clone(&source) as Arc<dyn TileSource<u64>>,
        pool as Arc<dyn Spawner>,
        Arc::new(FakeClock::default()),
        StreamConfig { max_in_flight: 4, capacity: 1000, ..Default::default() },
    );
    s.load_roots_blocking().unwrap();
    source.1.store(true, std::sync::atomic::Ordering::SeqCst);
    // The four tiles of level 1 of face 0: their parent, the root, is resident, so all four are dispatched at once.
    let level1: Vec<(TileId, f64)> =
        [(0, 0), (1, 0), (0, 1), (1, 1)].iter().map(|&(x, y)| (TileId::new(Face(0), 1, x, y).unwrap(), f64::from(x + y))).collect();
    s.set_draw(&level1);
    s.step();
    let started = std::time::Instant::now();
    while s.stats().completed < 4 {
        assert!(started.elapsed().as_secs() < 60, "four loads meeting at a barrier never finished: {:?}", s.stats());
        s.step();
        std::thread::yield_now();
    }
    let samples = s.job_log().since(0);
    assert_eq!(samples.len(), 4);
    assert!(samples.iter().all(|j| j.done && (1..=4).contains(&j.running)), "{samples:?}");
    assert_eq!(samples.iter().map(|j| j.running).max(), Some(4), "the last to arrive found the other three inside: {samples:?}");
}
