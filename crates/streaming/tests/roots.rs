//! STRM-001: the six level-0 tiles are loaded concurrently on the spawner, and the result does not depend on
//! the spawner, the thread count or the completion order (CON-14).

use planet_core::{Face, TileId};
use planet_streaming::{FakeClock, Job, ManualSpawner, Spawner, StreamConfig, Streamer, ThreadPool, TileSource};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn root(f: u8) -> TileId {
    TileId::new(Face(f), 0, 0, 0).unwrap()
}

struct Plain;
impl TileSource<u64> for Plain {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        Ok(tile.raw() ^ 0x5a5a)
    }
}

fn resident_set(s: &Streamer<u64>) -> Vec<(TileId, u64, Option<planet_streaming::SlotRef>)> {
    Face::ALL.iter().map(|&f| (root(f.0), *s.get(root(f.0)).expect("root resident"), s.slot_of(root(f.0)))).collect()
}

fn streamer(spawner: Arc<dyn Spawner>, source: Arc<dyn TileSource<u64>>) -> Streamer<u64> {
    Streamer::new(source, spawner, Arc::new(FakeClock::default()), StreamConfig { record_changes: true, ..Default::default() })
}

/// Counts how many jobs were handed over before the caller first asked the spawner to run one (the wait).
struct CountingSpawner {
    inner: ManualSpawner,
    spawned: AtomicUsize,
    spawned_at_first_help: Mutex<Option<usize>>,
}

impl Spawner for CountingSpawner {
    fn spawn(&self, job: Job) {
        self.spawned.fetch_add(1, Ordering::SeqCst);
        self.inner.spawn(job);
    }
    fn help(&self) -> bool {
        self.spawned_at_first_help.lock().unwrap().get_or_insert(self.spawned.load(Ordering::SeqCst));
        self.inner.run_one()
    }
}

// spec: STRM-001
#[test]
fn all_six_roots_are_dispatched_before_the_wait() {
    let sp = Arc::new(CountingSpawner {
        inner: ManualSpawner::default(),
        spawned: AtomicUsize::new(0),
        spawned_at_first_help: Mutex::new(None),
    });
    let mut s = streamer(sp.clone(), Arc::new(Plain));
    s.load_roots_blocking().unwrap();
    assert_eq!(*sp.spawned_at_first_help.lock().unwrap(), Some(6), "every root job is handed to the spawner before the caller waits");
    assert_eq!(resident_set(&s).len(), 6);
    assert_eq!(s.stats().in_flight, 0, "root loading leaves nothing in flight");
}

/// Blocks each load until all six are inside `load` at once (or a timeout): proves the pool runs them concurrently.
struct Rendezvous {
    arrived: AtomicUsize,
    inside: AtomicUsize,
    peak: AtomicUsize,
}

impl TileSource<u64> for Rendezvous {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        let n = self.inside.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(n, Ordering::SeqCst);
        self.arrived.fetch_add(1, Ordering::SeqCst);
        let t0 = Instant::now();
        while self.arrived.load(Ordering::SeqCst) < 6 && t0.elapsed() < Duration::from_secs(2) {
            std::thread::yield_now();
        }
        self.inside.fetch_sub(1, Ordering::SeqCst);
        Ok(tile.raw() ^ 0x5a5a)
    }
}

// spec: STRM-001
#[test]
fn a_pool_runs_the_six_roots_at_once() {
    let src = Arc::new(Rendezvous { arrived: AtomicUsize::new(0), inside: AtomicUsize::new(0), peak: AtomicUsize::new(0) });
    let mut s = streamer(Arc::new(ThreadPool::new(6)), src.clone());
    s.load_roots_blocking().unwrap();
    assert_eq!(src.peak.load(Ordering::SeqCst), 6, "all six loads were in flight together");
}

// spec: STRM-001, CON-14
#[test]
fn the_resident_set_is_identical_for_any_spawner_and_thread_count() {
    let mut sets = Vec::new();
    let manual: Arc<dyn Spawner> = Arc::new(ManualSpawner::default());
    sets.push(("manual", manual));
    for t in [1usize, 2, 8] {
        sets.push(("pool", Arc::new(ThreadPool::new(t)) as Arc<dyn Spawner>));
    }
    let mut results = Vec::new();
    for (_, sp) in sets {
        let mut s = streamer(sp, Arc::new(Plain));
        s.load_roots_blocking().unwrap();
        let changes = s.take_changes();
        results.push((resident_set(&s), changes));
    }
    for r in &results[1..] {
        assert_eq!(r, &results[0], "resident set, slots and change order do not depend on the spawner");
    }
    let order: Vec<TileId> = results[0]
        .1
        .iter()
        .map(|c| match c {
            planet_streaming::ResidentChange::Added(t) => *t,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(order, Face::ALL.iter().map(|f| root(f.0)).collect::<Vec<_>>(), "inserted in tile-id order");
}

/// Fails one face; the others wait on a cancellation-aware stage so the test can see them being stopped.
struct OneFails {
    bad: TileId,
    started: AtomicUsize,
    stopped: AtomicUsize,
}

impl TileSource<u64> for OneFails {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        if tile == self.bad {
            Err("disk on fire".into())
        } else {
            Ok(0)
        }
    }
    fn load_staged(&self, tile: TileId, cancel: &planet_streaming::CancelToken) -> Result<Option<u64>, String> {
        self.started.fetch_add(1, Ordering::SeqCst);
        if tile == self.bad {
            return self.load(tile).map(Some);
        }
        // Wait (bounded) for the failure to cancel us, as a stage boundary would observe.
        let t0 = Instant::now();
        while !cancel.is_cancelled() && t0.elapsed() < Duration::from_millis(40) {
            std::thread::yield_now();
        }
        if cancel.is_cancelled() {
            self.stopped.fetch_add(1, Ordering::SeqCst);
            return Ok(None);
        }
        Ok(Some(0))
    }
}

// spec: STRM-001
#[test]
fn a_failing_root_cancels_the_others_and_names_its_tile() {
    for pool in [Some(8usize), None] {
        let src = Arc::new(OneFails { bad: root(3), started: AtomicUsize::new(0), stopped: AtomicUsize::new(0) });
        let sp: Arc<dyn Spawner> = match pool {
            Some(t) => Arc::new(ThreadPool::new(t)),
            None => Arc::new(ManualSpawner::default()),
        };
        let mut s = streamer(sp, src.clone());
        let e = s.load_roots_blocking().unwrap_err();
        assert!(e.contains(&format!("{:?}", root(3))) && e.contains("disk on fire"), "error names the tile and the cause: {e}");
        for f in 0..6 {
            assert!(!s.is_resident(root(f)), "no root is left half-loaded after a failure");
        }
        assert_eq!(s.stats().main_in_use, 0, "slots are given back");
        if pool.is_some() {
            assert!(src.stopped.load(Ordering::SeqCst) >= 1, "at least one running root saw the cancellation and stopped");
        }
    }
}

// spec: STRM-001
#[test]
fn a_capacity_below_six_is_still_reported() {
    let mut s = Streamer::new(
        Arc::new(Plain) as Arc<dyn TileSource<u64>>,
        Arc::new(ManualSpawner::default()) as Arc<dyn Spawner>,
        Arc::new(FakeClock::default()),
        StreamConfig { capacity: 5, ..Default::default() },
    );
    let e = s.load_roots_blocking().unwrap_err();
    assert!(e.contains("below the 6 roots") && e.contains("floor level 0"), "{e}");
}

// spec: STRM-001
#[test]
fn a_capacity_below_the_roots_of_the_floor_is_reported_with_their_count() {
    let mut s = Streamer::new(
        Arc::new(Plain) as Arc<dyn TileSource<u64>>,
        Arc::new(ManualSpawner::default()) as Arc<dyn Spawner>,
        Arc::new(FakeClock::default()),
        StreamConfig { capacity: 383, floor: 3, ..Default::default() },
    );
    let e = s.load_roots_blocking().unwrap_err();
    assert!(e.contains("capacity 383 is below the 384 roots") && e.contains("floor level 3"), "{e}");
}
