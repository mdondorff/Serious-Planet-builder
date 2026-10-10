//! STRM-001, STRM-002, STRM-005, STRM-006: injected time and spawning, parent-first requests, cancellation,
//! capacity, failure retry, and a real thread pool that never blocks the caller.

use planet_core::{Face, TileId};
use planet_streaming::{FakeClock, ManualSpawner, Spawner, StreamConfig, Streamer, ThreadPool, TileSource};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

fn id(f: u8, l: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(f), l, x, y).unwrap()
}

/// Source returning the raw id, optionally failing the first `fail_first` attempts of chosen tiles.
#[derive(Default)]
struct TestSource {
    attempts: Mutex<HashMap<TileId, u32>>,
    fail: HashSet<TileId>,
    fail_first: u32,
}

impl TileSource<u64> for TestSource {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        let n = {
            let mut a = self.attempts.lock().unwrap();
            let c = a.entry(tile).or_insert(0);
            *c += 1;
            *c
        };
        if self.fail.contains(&tile) && n <= self.fail_first {
            return Err(format!("injected failure {n} for {tile:?}"));
        }
        Ok(tile.raw())
    }
}

type Fixture = (Streamer<u64>, Arc<ManualSpawner>, Arc<FakeClock>);

fn fixture(source: TestSource, config: StreamConfig) -> Fixture {
    let spawner = Arc::new(ManualSpawner::default());
    let clock = Arc::new(FakeClock::default());
    let mut s = Streamer::new(Arc::new(source), spawner.clone() as Arc<dyn Spawner>, clock.clone(), config);
    s.load_roots_blocking().unwrap();
    (s, spawner, clock)
}

// spec: STRM-002
#[test]
fn a_wanted_tile_shows_its_nearest_resident_ancestor_until_it_arrives() {
    let (mut s, sp, _) = fixture(TestSource::default(), StreamConfig { max_in_flight: 1, record_dispatches: true, ..Default::default() });
    let deep = id(2, 4, 5, 9);
    let r = s.begin_frame(&[(deep, 1.0)]);
    assert_eq!(r[0].shown, Some(id(2, 0, 0, 0)), "only the root is resident at first");
    assert!(!r[0].exact());
    // Ancestors arrive one by one; each frame the shown tile gets finer, never coarser, never missing.
    let mut last_level = 0;
    for _ in 0..10 {
        sp.run(1);
        let r = s.begin_frame(&[(deep, 1.0)]);
        let level = r[0].shown.expect("always something to show").level();
        assert!(level >= last_level, "shown level went from {last_level} to {level}");
        last_level = level;
    }
    assert!(s.begin_frame(&[(deep, 1.0)])[0].exact(), "the wanted tile must arrive");
}

// spec: STRM-002, STRM-005
#[test]
fn ancestors_are_dispatched_before_descendants() {
    let (mut s, sp, _) = fixture(TestSource::default(), StreamConfig { max_in_flight: 2, record_dispatches: true, ..Default::default() });
    let deep = id(0, 5, 17, 3);
    for _ in 0..12 {
        s.begin_frame(&[(deep, 1.0)]);
        sp.run(2);
    }
    let order: Vec<TileId> = s.dispatch_log.clone();
    let pos = |t: TileId| order.iter().position(|x| *x == t);
    let mut chain = vec![deep];
    while let Some(p) = chain.last().unwrap().parent() {
        chain.push(p);
    }
    chain.reverse(); // root first
    for w in chain.windows(2) {
        // Roots are resident from start-up and never dispatched.
        if let (Some(a), Some(b)) = (pos(w[0]), pos(w[1])) {
            assert!(a < b, "{:?} was dispatched after its child {:?}", w[0], w[1]);
        }
    }
    assert!(pos(deep).is_some(), "the wanted tile itself must have been dispatched");
}

// spec: STRM-005
#[test]
fn lower_priority_numbers_run_first_and_unwanted_requests_are_cancelled() {
    let (mut s, sp, _) = fixture(TestSource::default(), StreamConfig { max_in_flight: 1, record_dispatches: true, ..Default::default() });
    let (near, far) = (id(1, 1, 0, 0), id(3, 1, 1, 1));
    s.begin_frame(&[(far, 50.0), (near, 5.0)]);
    assert_eq!(s.dispatch_log, [near], "the nearer tile (priority 5) goes first");
    // `far` is dropped from the wanted set before it ever started: it is cancelled, not run.
    s.begin_frame(&[(near, 5.0)]);
    sp.run(5);
    s.begin_frame(&[(near, 5.0)]);
    assert!(s.is_resident(near));
    assert!(!s.is_resident(far) && !s.dispatch_log.contains(&far), "a cancelled request must never run");
    assert!(s.stats().cancelled >= 1);
}

// spec: STRM-005
#[test]
fn a_job_cancelled_while_in_flight_is_not_counted_as_failure() {
    let (mut s, sp, clock) = fixture(
        TestSource::default(),
        StreamConfig { max_in_flight: 1, retry_after_ms: 10, record_dispatches: true, ..Default::default() },
    );
    let t = id(4, 2, 1, 1);
    s.begin_frame(&[(t, 1.0)]);
    // Its parent is dispatched first (parent-first); drop everything before it runs.
    s.begin_frame(&[]);
    sp.run(3);
    clock.advance(100);
    s.begin_frame(&[]);
    assert_eq!(s.stats().failed, 0, "cancellation is not failure");
    assert!(s.failed_tiles().is_empty());
}

// spec: STRM-001, STRM-007
#[test]
fn failed_tiles_are_retried_only_after_the_delay_on_the_injected_clock() {
    let flaky = id(5, 1, 1, 0);
    let (mut s, sp, clock) = fixture(
        TestSource { fail: [flaky].into(), fail_first: 1, ..Default::default() },
        StreamConfig { max_in_flight: 4, retry_after_ms: 1000, record_dispatches: true, ..Default::default() },
    );
    s.begin_frame(&[(flaky, 1.0)]);
    sp.run(4);
    s.begin_frame(&[(flaky, 1.0)]);
    assert_eq!(s.stats().failed, 1);
    let dispatched = s.dispatch_log.len();
    // Before the delay nothing is re-requested, however often frames run.
    for _ in 0..5 {
        s.begin_frame(&[(flaky, 1.0)]);
        sp.run(4);
    }
    clock.advance(999);
    s.begin_frame(&[(flaky, 1.0)]);
    assert_eq!(s.dispatch_log.len(), dispatched, "retried too early");
    clock.advance(1);
    s.begin_frame(&[(flaky, 1.0)]);
    sp.run(4);
    assert!(s.dispatch_log.len() > dispatched, "must retry once the delay has passed");
    assert!(s.begin_frame(&[(flaky, 1.0)])[0].exact(), "the retry succeeds");
}

// spec: STRM-001
#[test]
fn the_same_inputs_give_the_same_dispatch_order() {
    let run = || {
        let (mut s, sp, _) =
            fixture(TestSource::default(), StreamConfig { max_in_flight: 3, record_dispatches: true, ..Default::default() });
        let wanted: Vec<(TileId, f64)> =
            (0..20).map(|i| (id((i % 6) as u8, 3, (i % 8) as u32, ((i * 3) % 8) as u32), f64::from(i) * 0.5)).collect();
        for _ in 0..15 {
            s.begin_frame(&wanted);
            sp.run(3);
        }
        s.dispatch_log.clone()
    };
    assert_eq!(run(), run());
}

// spec: STRM-007
#[test]
fn capacity_evicts_least_recently_used_tiles_but_never_roots_or_tiles_in_use() {
    let (mut s, sp, _) =
        fixture(TestSource::default(), StreamConfig { max_in_flight: 8, capacity: 12, record_dispatches: true, ..Default::default() });
    let a: Vec<(TileId, f64)> = (0..4).map(|x| (id(0, 2, x, 0), 1.0)).collect();
    for _ in 0..6 {
        s.begin_frame(&a);
        sp.run(8);
    }
    s.begin_frame(&a);
    assert!(a.iter().all(|(t, _)| s.is_resident(*t)));
    // Move the interest elsewhere; the old tiles become evictable once the capacity is exceeded.
    let b: Vec<(TileId, f64)> = (0..8).map(|x| (id(3, 3, x, 1), 1.0)).collect();
    for _ in 0..12 {
        s.begin_frame(&b);
        sp.run(8);
    }
    let st = s.stats();
    assert!(st.resident <= 12 + 8, "resident {} stays near the capacity", st.resident);
    assert!(st.evicted > 0, "something had to be evicted");
    for f in Face::ALL {
        assert!(s.is_resident(id(f.0, 0, 0, 0)), "roots are never evicted");
    }
    assert!(b.iter().all(|(t, _)| s.is_resident(*t)), "tiles in use stay resident");
}

// spec: STRM-002
#[test]
fn the_thread_pool_never_blocks_the_frame() {
    // The source blocks until the test releases it; begin_frame must return while the job is stuck.
    struct Gated(Mutex<std::sync::mpsc::Receiver<()>>);
    impl TileSource<u64> for Gated {
        fn load(&self, tile: TileId) -> Result<u64, String> {
            if tile.level() > 0 {
                self.0.lock().unwrap().recv().map_err(|e| e.to_string())?;
            }
            Ok(tile.raw())
        }
    }
    // Declaration order matters: on a failing assertion `release` must drop before the pool joins its workers, or the
    // worker stuck in `recv` would deadlock the unwind.
    let pool = Arc::new(ThreadPool::new(2));
    let (release, gate) = std::sync::mpsc::channel();
    let clock = Arc::new(FakeClock::default());
    let mut s = Streamer::new(Arc::new(Gated(Mutex::new(gate))), pool.clone() as Arc<dyn Spawner>, clock, StreamConfig::default());
    s.load_roots_blocking().unwrap();
    let t = id(2, 1, 0, 1);
    let r = s.begin_frame(&[(t, 1.0)]);
    assert_eq!(r[0].shown, Some(id(2, 0, 0, 0)), "the frame is served from the root while the job is blocked");
    assert_eq!(s.stats().in_flight, 1);
    release.send(()).unwrap();
    // Poll without sleeping on the job itself: spin on frames until it lands (bounded).
    let mut exact = false;
    for _ in 0..2000 {
        if s.begin_frame(&[(t, 1.0)])[0].exact() {
            exact = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(exact, "the tile must arrive once the source is released");
    drop(s);
    drop(pool);
}

// spec: STRM-005
#[test]
fn an_in_flight_job_cancelled_before_it_starts_never_calls_the_source() {
    let source = Arc::new(TestSource::default());
    let spawner = Arc::new(ManualSpawner::default());
    let mut s = Streamer::new(
        source.clone(),
        spawner.clone() as Arc<dyn Spawner>,
        Arc::new(FakeClock::default()),
        StreamConfig { max_in_flight: 4, record_dispatches: true, ..Default::default() },
    );
    s.load_roots_blocking().unwrap();
    let t = id(1, 1, 0, 1);
    s.begin_frame(&[(t, 1.0)]);
    assert_eq!(s.stats().in_flight, 1, "the job was handed to the spawner but has not run");
    s.begin_frame(&[]); // nobody needs it any more: the token is set
    spawner.run(5);
    s.begin_frame(&[]);
    assert!(!source.attempts.lock().unwrap().contains_key(&t), "a cancelled job must not load");
    assert_eq!(s.stats().failed, 0);
    assert!(!s.is_resident(t));
}

// spec: STRM-007
#[test]
fn eviction_removes_the_least_recently_used_tile_first() {
    let (mut s, sp, _) =
        fixture(TestSource::default(), StreamConfig { max_in_flight: 8, capacity: 9, record_dispatches: true, ..Default::default() });
    // Six roots are resident; three more tiles arrive one after another (a oldest, c newest), then interest moves to d.
    let (a, b, c, d) = (id(0, 1, 0, 0), id(0, 1, 1, 0), id(0, 1, 0, 1), id(0, 1, 1, 1));
    for t in [a, b, c] {
        for _ in 0..4 {
            s.begin_frame(&[(t, 1.0)]);
            sp.run(8);
        }
    }
    for t in [a, b, c] {
        assert!(s.is_resident(t));
    }
    for _ in 0..4 {
        s.begin_frame(&[(d, 1.0)]);
        sp.run(8);
    }
    s.begin_frame(&[(d, 1.0)]);
    // 10 resident, capacity 9: exactly the least recently used one (a) goes.
    assert!(!s.is_resident(a), "the oldest tile must be evicted first");
    assert!(s.is_resident(b) && s.is_resident(c) && s.is_resident(d), "newer tiles must survive");
}

// spec: STRM-001
#[test]
fn a_panicking_source_becomes_a_failure_with_retry_not_a_stuck_tile() {
    struct Panics(Mutex<u32>);
    impl TileSource<u64> for Panics {
        fn load(&self, tile: TileId) -> Result<u64, String> {
            if tile.level() > 0 {
                let mut n = self.0.lock().unwrap();
                *n += 1;
                if *n == 1 {
                    drop(n);
                    panic!("injected panic");
                }
            }
            Ok(tile.raw())
        }
    }
    let spawner = Arc::new(ManualSpawner::default());
    let clock = Arc::new(FakeClock::default());
    let mut s = Streamer::new(
        Arc::new(Panics(Mutex::new(0))),
        spawner.clone() as Arc<dyn Spawner>,
        clock.clone(),
        StreamConfig { max_in_flight: 1, retry_after_ms: 50, ..Default::default() },
    );
    s.load_roots_blocking().unwrap();
    let t = id(3, 1, 1, 1);
    s.begin_frame(&[(t, 1.0)]);
    spawner.run(1);
    s.begin_frame(&[(t, 1.0)]);
    assert_eq!(s.stats().failed, 1);
    assert_eq!(s.stats().in_flight, 0, "the slot must be free again");
    assert!(s.last_error(t).is_some_and(|m| m.contains("panicked")));
    clock.advance(60);
    for _ in 0..4 {
        s.begin_frame(&[(t, 1.0)]);
        spawner.run(2);
    }
    assert!(s.begin_frame(&[(t, 1.0)])[0].exact(), "the retry succeeds");
    assert!(s.failed_tiles().is_empty() && s.last_error(t).is_none(), "success clears the failure record");
}
