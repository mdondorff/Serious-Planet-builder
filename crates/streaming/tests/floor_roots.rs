//! STRM-001, STRM-007 at render floor 3 (LOD-011, ADR 0062): the 384 level-3 tiles are the roots. They are loaded concurrently on the
//! spawner and become resident in tile-id order whatever the spawner, the thread count or the completion order; one failing root
//! stops the others; no tile above the floor is ever requested, resident or a stand-in; and roots are never evicted.

use planet_core::{Face, TileId};
use planet_streaming::{
    floor_roots, FakeClock, Job, ManualSpawner, ResidentChange, Spawner, StreamConfig, Streamer, ThreadPool, TileSource,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FLOOR: u8 = 3;
const ROOTS: usize = 384;

struct Plain;
impl TileSource<u64> for Plain {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        Ok(tile.raw() ^ 0x5a5a)
    }
}

fn config(capacity: usize) -> StreamConfig {
    StreamConfig { floor: FLOOR, capacity, record_changes: true, record_dispatches: true, ..Default::default() }
}

fn streamer(spawner: Arc<dyn Spawner>, source: Arc<dyn TileSource<u64>>) -> Streamer<u64> {
    Streamer::new(source, spawner, Arc::new(FakeClock::default()), config(4096))
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

// spec: STRM-001, LOD-011
#[test]
fn the_roots_are_the_384_tiles_of_the_floor_and_all_are_dispatched_before_the_wait() {
    let roots = floor_roots(FLOOR);
    assert_eq!(roots.len(), ROOTS);
    assert_eq!(config(1).roots(), roots, "the config names the same roots");
    assert!(roots.windows(2).all(|w| w[0] < w[1]) && roots.iter().all(|t| t.level() == FLOOR));
    let sp = Arc::new(CountingSpawner {
        inner: ManualSpawner::default(),
        spawned: AtomicUsize::new(0),
        spawned_at_first_help: Mutex::new(None),
    });
    let mut s = streamer(sp.clone(), Arc::new(Plain));
    s.load_roots_blocking().unwrap();
    assert_eq!(*sp.spawned_at_first_help.lock().unwrap(), Some(ROOTS), "every root job is handed over before the caller waits");
    assert!(roots.iter().all(|&t| s.is_resident(t)));
    assert_eq!(s.stats().resident, ROOTS);
    assert_eq!(s.stats().in_flight, 0, "root loading leaves nothing in flight");
    assert!(!s.is_resident(TileId::new(Face::ALL[0], 0, 0, 0).unwrap()), "no tile above the floor is loaded");
}

/// Blocks each load until all of them are inside `load` at once (or a timeout): proves the pool runs them concurrently.
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
        while self.arrived.load(Ordering::SeqCst) < ROOTS && t0.elapsed() < Duration::from_secs(5) {
            std::thread::yield_now();
        }
        self.inside.fetch_sub(1, Ordering::SeqCst);
        Ok(tile.raw() ^ 0x5a5a)
    }
}

// spec: STRM-001
#[test]
fn a_pool_as_large_as_the_root_count_has_every_load_in_flight_together() {
    let src = Arc::new(Rendezvous { arrived: AtomicUsize::new(0), inside: AtomicUsize::new(0), peak: AtomicUsize::new(0) });
    let mut s = streamer(Arc::new(ThreadPool::new(ROOTS)), src.clone());
    s.load_roots_blocking().unwrap();
    assert_eq!(src.peak.load(Ordering::SeqCst), ROOTS, "all 384 loads were in flight together");
    assert_eq!(s.stats().resident, ROOTS);
}

// spec: STRM-001, CON-14
#[test]
fn the_resident_set_its_slots_and_the_change_order_are_identical_for_any_spawner_and_thread_count() {
    let mut spawners: Vec<Arc<dyn Spawner>> = vec![Arc::new(ManualSpawner::default())];
    spawners.extend([1usize, 2, 8].map(|t| Arc::new(ThreadPool::new(t)) as Arc<dyn Spawner>));
    let mut results = Vec::new();
    for sp in spawners {
        let mut s = streamer(sp, Arc::new(Plain));
        s.load_roots_blocking().unwrap();
        let changes = s.take_changes();
        let set: Vec<_> = floor_roots(FLOOR).into_iter().map(|t| (t, *s.get(t).expect("root resident"), s.slot_of(t))).collect();
        results.push((set, changes));
    }
    for r in &results[1..] {
        assert_eq!(r, &results[0], "resident set, slots and change order do not depend on the spawner");
    }
    let order: Vec<TileId> = results[0]
        .1
        .iter()
        .map(|c| match c {
            ResidentChange::Added(t) => *t,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(order, floor_roots(FLOOR), "inserted in tile-id order");
}

/// Fails one root; the others wait on a cancellation-aware stage so the test can see them being stopped.
struct OneFails {
    bad: TileId,
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
        if tile == self.bad {
            return self.load(tile).map(Some);
        }
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
fn a_failing_root_cancels_the_others_gives_the_slots_back_and_names_its_tile() {
    // An early root fails: on the manual spawner (one job at a time) the roots before it each wait out their 40 ms stage.
    let bad = floor_roots(FLOOR)[5];
    for pool in [Some(8usize), None] {
        let src = Arc::new(OneFails { bad, stopped: AtomicUsize::new(0) });
        let sp: Arc<dyn Spawner> = match pool {
            Some(t) => Arc::new(ThreadPool::new(t)),
            None => Arc::new(ManualSpawner::default()),
        };
        let mut s = streamer(sp, src.clone());
        let e = s.load_roots_blocking().unwrap_err();
        assert!(e.contains(&format!("{bad:?}")) && e.contains("disk on fire"), "error names the tile and the cause: {e}");
        assert!(floor_roots(FLOOR).iter().all(|&t| !s.is_resident(t)), "no root is left half-loaded after a failure");
        assert_eq!(s.stats().main_in_use, 0, "slots are given back");
        if pool.is_some() {
            assert!(src.stopped.load(Ordering::SeqCst) >= 1, "at least one running root saw the cancellation and stopped");
        }
    }
}

fn tile(face: usize, level: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face::ALL[face], level, x, y).unwrap()
}

// spec: STRM-001, STRM-007, LOD-011
#[test]
fn no_tile_above_the_floor_is_requested_or_resident_and_a_stand_in_stops_at_the_floor_tile() {
    let sp = Arc::new(ManualSpawner::default());
    let mut s = Streamer::new(Arc::new(Plain), sp.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config(4096));
    s.load_roots_blocking().unwrap();
    let wanted = tile(2, 6, 17, 40);
    let chain: Vec<TileId> = std::iter::successors(Some(wanted), |t| t.parent()).collect();
    let floor_ancestor = *chain.iter().find(|t| t.level() == FLOOR).unwrap();
    // Before anything else is resident, the stand-in is the floor tile above the wanted one: never nothing, never a tile above it.
    assert_eq!(s.resolve(wanted).shown, Some(floor_ancestor));
    // The draw set names a level-6 tile: its ancestors down to the floor are requested (the floor tile is resident already).
    for _ in 0..10 {
        s.begin_frame(&[(wanted, 0.0)]);
        sp.run(100);
    }
    assert!(s.dispatch_log.iter().all(|t| t.level() > FLOOR), "a root or a tile above the floor was requested: {:?}", s.dispatch_log);
    let expected: Vec<TileId> = chain.iter().copied().filter(|t| t.level() > FLOOR).collect();
    for t in &expected {
        assert!(s.is_resident(*t), "{t:?} of the chain below the floor tile is resident");
    }
    assert_eq!(s.resolve(wanted), planet_streaming::Resolved { wanted, shown: Some(wanted) });
    let above: Vec<TileId> = chain.iter().copied().filter(|t| t.level() < FLOOR).collect();
    assert_eq!(above.len(), 3, "levels 2, 1 and 0 lie above the floor");
    assert!(above.iter().all(|t| !s.is_resident(*t)), "no ancestor above the floor is resident");
    assert!(s.stats().resident == ROOTS + expected.len(), "only the roots and the requested chain are resident");
}

// spec: STRM-007, LOD-011
#[test]
fn roots_are_never_evicted_however_full_the_pool_and_however_the_wants_move() {
    let capacity = ROOTS + 24;
    let sp = Arc::new(ManualSpawner::default());
    let mut s = Streamer::new(Arc::new(Plain), sp.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config(capacity));
    s.load_roots_blocking().unwrap();
    let roots = floor_roots(FLOOR);
    let mut evicted_non_roots = 0usize;
    for k in 0..60u32 {
        // A moving wanted set far larger than the free slots: the pool must evict non-roots to make room and may never touch a root.
        let wants: Vec<(TileId, f64)> =
            (0..30u32).map(|i| (tile(((k + i) % 6) as usize, 5, (k * 7 + i * 3) % 32, (k * 5 + i * 11) % 32), f64::from(i))).collect();
        s.begin_frame(&wants);
        sp.run(100);
        s.take_changes().iter().for_each(|c| {
            if let ResidentChange::Evicted(t) = c {
                assert!(t.level() > FLOOR, "a root was evicted: {t:?}");
                evicted_non_roots += 1;
            }
        });
        assert!(roots.iter().all(|&t| s.is_resident(t)), "frame {k}: a root left");
        let st = s.stats();
        assert!(st.main_in_use <= capacity, "frame {k}: {} slots in use of {capacity}", st.main_in_use);
    }
    assert!(evicted_non_roots > 0, "the script must force evictions, or the test proves nothing");
}
