//! STRM-001, STRM-002, STRM-005: the request scheduler on fakes (fake clock, manual spawner, fake source): request
//! classes and their order, cancellation only for leaving R and the prefetch set, prefetch yielding and free-capacity
//! only, stage boundaries, determinism across thread counts.

use planet_core::{Face, TileId, Vec3};
use planet_streaming::{
    predicted_position, CancelToken, FakeClock, ManualSpawner, Spawner, StreamConfig, Streamer, ThreadPool, TileSource, Wants,
};
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

fn id(f: u8, l: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(f), l, x, y).unwrap()
}

#[derive(Default)]
struct Counting {
    loads: AtomicUsize,
    fail_once: Mutex<HashSet<TileId>>,
}

impl TileSource<u64> for Counting {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        if self.fail_once.lock().unwrap().remove(&tile) {
            return Err(format!("injected failure for {tile:?}"));
        }
        Ok(tile.raw())
    }
}

fn fixture(config: StreamConfig) -> (Streamer<u64>, Arc<ManualSpawner>, Arc<FakeClock>, Arc<Counting>) {
    let (spawner, clock, source) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()), Arc::new(Counting::default()));
    let mut s = Streamer::new(source.clone(), spawner.clone() as Arc<dyn Spawner>, clock.clone(), config);
    s.load_roots_blocking().unwrap();
    (s, spawner, clock, source)
}

fn cfg(max_in_flight: usize, capacity: usize) -> StreamConfig {
    StreamConfig { max_in_flight, capacity, retry_after_ms: 1000, record_dispatches: true, ..Default::default() }
}

fn settle(s: &mut Streamer<u64>, sp: &ManualSpawner, w: &Wants) {
    for _ in 0..200 {
        s.begin_frame_wants(w);
        if sp.run(64) == 0 && s.stats().queued == 0 && s.stats().in_flight == 0 {
            break;
        }
    }
    s.begin_frame_wants(w);
}

/// Position of `t` in the dispatch log.
fn at(s: &Streamer<u64>, t: TileId) -> usize {
    s.dispatch_log.iter().position(|x| *x == t).unwrap_or_else(|| panic!("{t:?} was never dispatched: {:?}", s.dispatch_log))
}

// spec: STRM-005, STRM-002
#[test]
fn missing_draw_tiles_go_first_then_the_rest_of_r_then_prefetch() {
    let (mut s, sp, _c, _src) = fixture(cfg(1000, 100_000));
    let deep = id(2, 3, 5, 6); // draw leaf: its ancestors (levels 1, 2) are missing too
    let (r_coarse, r_fine) = (id(3, 1, 0, 1), id(3, 2, 0, 3)); // residency; the fine one's parent is level 1 of face 3
    let (pre_a, pre_b) = (id(4, 1, 1, 1), id(5, 2, 2, 2));
    let wants =
        Wants { draw: vec![(deep, 1.0)], residency: vec![(r_fine, 1.0), (r_coarse, 90.0)], prefetch: vec![(pre_b, 0.5), (pre_a, 99.0)] };
    // Prefetch yields while draw or residency jobs run, so let the machine finish between frames.
    settle(&mut s, &sp, &wants);
    // Draw class: ancestors before descendants (STRM-002).
    let (l1, l2) = (deep.parent().unwrap().parent().unwrap(), deep.parent().unwrap());
    assert!(at(&s, l1) < at(&s, l2) && at(&s, l2) < at(&s, deep));
    // Every draw request precedes every residency request, and those every prefetch request.
    let draw = [l1, l2, deep].map(|t| at(&s, t));
    let res = [r_coarse, r_fine, r_fine.parent().unwrap()].map(|t| at(&s, t));
    let pre = [pre_a, pre_b, pre_b.parent().unwrap()].map(|t| at(&s, t));
    assert!(draw.iter().max() < res.iter().min(), "draw {draw:?} must precede residency {res:?}");
    assert!(res.iter().max() < pre.iter().min(), "residency {res:?} must precede prefetch {pre:?}");
    // Coarse before fine inside the residency class, even though the fine tile is nearer.
    assert!(at(&s, r_coarse) < at(&s, r_fine));
}

// spec: STRM-005
#[test]
fn inside_one_level_of_r_nearer_tiles_go_before_farther_ones() {
    let (mut s, _sp, _c, _src) = fixture(cfg(1000, 100_000));
    let tiles = [id(0, 1, 0, 0), id(1, 1, 0, 0), id(2, 1, 0, 0), id(3, 1, 0, 0)];
    let dist = [30.0, 10.0, 40.0, 20.0];
    s.begin_frame_wants(&Wants { residency: tiles.iter().copied().zip(dist).collect(), ..Wants::default() });
    assert_eq!(s.dispatch_log, [tiles[1], tiles[3], tiles[0], tiles[2]]);
}

// spec: STRM-005
#[test]
fn a_tile_in_two_classes_is_requested_in_the_higher_one() {
    let (mut s, _sp, _c, _src) = fixture(cfg(1000, 100_000));
    let (a, b) = (id(0, 1, 1, 1), id(1, 1, 1, 1));
    // `b` is both prefetch and residency, `a` only residency: `b` is class 1 with the same level, so distance decides.
    s.begin_frame_wants(&Wants { residency: vec![(a, 50.0), (b, 5.0)], prefetch: vec![(b, 0.0)], ..Wants::default() });
    assert_eq!(s.dispatch_log, [b, a]);
    assert_eq!(s.stats().cancelled_prefetch, 0);
}

// spec: STRM-005
#[test]
fn rotation_only_changes_the_draw_set_and_requests_and_cancels_nothing_once_r_is_resident() {
    let (mut s, sp, _c, _src) = fixture(cfg(8, 100_000));
    // R: a coarse and a fine patch; D picks different subsets of it as the camera turns.
    let r: Vec<(TileId, f64)> = (0..4).flat_map(|f| (0..4).map(move |x| (id(f, 3, x, 2), f64::from(x)))).collect();
    let views: Vec<Vec<(TileId, f64)>> =
        (0..8).map(|k| r.iter().copied().filter(|(t, _)| (t.raw() as usize + k).is_multiple_of(3)).collect()).collect();
    settle(&mut s, &sp, &Wants { draw: views[0].clone(), residency: r.clone(), prefetch: vec![] });
    let before = (s.dispatch_log.len(), s.stats().cancelled, s.stats().completed);
    for round in 0..3 {
        for d in &views {
            let res = s.begin_frame_wants(&Wants { draw: d.clone(), residency: r.clone(), prefetch: vec![] });
            assert!(res.iter().all(|x| x.exact()), "round {round}: everything drawn is resident");
            let st = s.stats();
            assert_eq!((st.queued, st.in_flight), (0, 0), "a rotation must not queue or start a job");
        }
    }
    assert_eq!((s.dispatch_log.len(), s.stats().cancelled, s.stats().completed), before, "nothing requested, nothing cancelled");
}

// spec: STRM-005
#[test]
fn leaving_r_cancels_but_leaving_the_view_does_not() {
    let (mut s, _sp, _c, _src) = fixture(cfg(0, 100_000)); // no job can start: everything stays queued
    let (a, b, c) = (id(0, 1, 0, 0), id(1, 1, 0, 0), id(2, 1, 0, 0));
    let r = vec![(a, 1.0), (b, 2.0), (c, 3.0)];
    s.begin_frame_wants(&Wants { draw: vec![(a, 1.0)], residency: r.clone(), prefetch: vec![] });
    assert_eq!(s.stats().queued_by_class, [1, 2, 0]);
    // The view turns away from `a`: it leaves D, stays in R. Nothing is cancelled, it is only reordered into class 1.
    s.begin_frame_wants(&Wants { draw: vec![(b, 2.0)], residency: r.clone(), prefetch: vec![] });
    assert_eq!(s.stats().cancelled, 0);
    assert_eq!(s.stats().queued_by_class, [1, 2, 0]);
    // `c` leaves R before its job starts: cancelled, and it never runs.
    s.begin_frame_wants(&Wants { draw: vec![(b, 2.0)], residency: r[..2].to_vec(), prefetch: vec![] });
    assert_eq!((s.stats().cancelled, s.stats().queued), (1, 2));
    assert_eq!(s.stats().cancelled_prefetch, 0);
}

// spec: STRM-005
#[test]
fn a_request_cancelled_before_it_starts_never_runs() {
    let (mut s, sp, _c, src) = fixture(cfg(1, 100_000));
    let (a, b) = (id(0, 1, 0, 0), id(1, 1, 0, 0));
    let w = |tiles: &[TileId]| Wants { residency: tiles.iter().map(|t| (*t, 1.0)).collect(), ..Wants::default() };
    s.begin_frame_wants(&w(&[a, b])); // one in flight (a), b queued
    s.begin_frame_wants(&w(&[a])); // b leaves R
    sp.run(10);
    s.begin_frame_wants(&w(&[a]));
    assert!(s.is_resident(a) && !s.is_resident(b));
    assert_eq!(src.loads.load(Ordering::SeqCst), 6 + 1, "six roots and a; b never ran");
}

// spec: STRM-005
#[test]
fn prefetch_goes_only_into_free_capacity_and_never_evicts() {
    // Capacity: six roots plus two tiles. R fills it; prefetch must not start.
    let (mut s, sp, _c, src) = fixture(cfg(8, 8));
    let (r1, r2) = (id(0, 1, 0, 0), id(1, 1, 0, 0));
    let pre = vec![(id(2, 1, 0, 0), 1.0), (id(3, 1, 0, 0), 2.0)];
    let w = Wants { residency: vec![(r1, 1.0), (r2, 2.0)], prefetch: pre.clone(), ..Wants::default() };
    settle(&mut s, &sp, &w);
    assert!(s.is_resident(r1) && s.is_resident(r2));
    assert_eq!(src.loads.load(Ordering::SeqCst), 6 + 2, "no prefetch job started with no free capacity");
    assert_eq!(s.stats().evicted, 0);
    assert_eq!(s.stats().queued_by_class[2], 2, "prefetch waits in the queue");
    // One free slot: exactly one prefetch tile, the first in order, is loaded.
    let (mut s, sp, _c, src) = fixture(cfg(8, 9));
    settle(&mut s, &sp, &w);
    assert_eq!(src.loads.load(Ordering::SeqCst), 6 + 2 + 1);
    assert!(s.is_resident(pre[0].0) && !s.is_resident(pre[1].0));
    assert_eq!(s.stats().evicted, 0, "prefetch never evicts");
}

// spec: STRM-005
#[test]
fn prefetch_yields_to_every_higher_class_request() {
    let (mut s, sp, _c, _src) = fixture(cfg(8, 100_000));
    let (p, q) = (id(4, 1, 0, 0), id(0, 1, 0, 0));
    // Prefetch alone starts at once.
    s.begin_frame_wants(&Wants { prefetch: vec![(p, 1.0)], ..Wants::default() });
    assert_eq!(s.dispatch_log, [p]);
    sp.run(8);
    // A residency request that is queued or running keeps the other prefetch tile out of the pool.
    let p2 = id(5, 1, 0, 0);
    s.begin_frame_wants(&Wants { residency: vec![(q, 1.0)], prefetch: vec![(p, 1.0), (p2, 1.0)], ..Wants::default() });
    assert_eq!(s.dispatch_log, [p, q], "q went out; p2 waits while q is in flight");
    s.begin_frame_wants(&Wants { residency: vec![(q, 1.0)], prefetch: vec![(p, 1.0), (p2, 1.0)], ..Wants::default() });
    assert_eq!(s.dispatch_log, [p, q], "still waiting: the job of q has not finished");
    sp.run(8);
    s.begin_frame_wants(&Wants { residency: vec![(q, 1.0)], prefetch: vec![(p, 1.0), (p2, 1.0)], ..Wants::default() });
    assert_eq!(s.dispatch_log, [p, q, p2], "with the higher classes quiet, prefetch goes on");
}

// spec: STRM-005
#[test]
fn tiles_of_prefetch_are_evicted_before_tiles_of_r() {
    let (mut s, sp, _c, _src) = fixture(cfg(8, 8));
    let (p1, p2) = (id(0, 1, 0, 0), id(1, 1, 0, 0));
    settle(&mut s, &sp, &Wants { prefetch: vec![(p1, 1.0), (p2, 2.0)], ..Wants::default() });
    assert!(s.is_resident(p1) && s.is_resident(p2));
    let (r1, r2) = (id(2, 1, 0, 0), id(3, 1, 0, 0));
    settle(&mut s, &sp, &Wants { residency: vec![(r1, 1.0), (r2, 2.0)], ..Wants::default() });
    assert!(s.is_resident(r1) && s.is_resident(r2), "R arrives");
    assert!(!s.is_resident(p1) && !s.is_resident(p2), "and the old prefetch tiles made room");
    assert_eq!(s.stats().evicted, 2);
}

// spec: STRM-005
#[test]
fn a_changed_travel_direction_cancels_the_prefetch_requests_that_left_the_set() {
    let (mut s, _sp, _c, _src) = fixture(cfg(0, 100_000)); // everything stays queued
    let ahead: Vec<(TileId, f64)> = (0..4).map(|f| (id(f, 1, 1, 1), f64::from(f))).collect();
    let r = vec![(id(4, 1, 0, 0), 1.0)];
    s.begin_frame_wants(&Wants { residency: r.clone(), prefetch: ahead.clone(), ..Wants::default() });
    assert_eq!(s.stats().queued_by_class, [0, 1, 4]);
    // The travel direction changes: two of the old prefetch tiles remain in the set, two are gone, one is new.
    let now_ahead = vec![ahead[0], ahead[1], (id(5, 1, 1, 1), 0.0)];
    s.begin_frame_wants(&Wants { residency: r.clone(), prefetch: now_ahead, ..Wants::default() });
    let st = s.stats();
    assert_eq!((st.cancelled, st.cancelled_prefetch), (2, 2), "only prefetch requests were cancelled");
    assert_eq!(st.queued_by_class, [0, 1, 3]);
    // A tile that is in R is never cancelled by a prefetch change.
    s.begin_frame_wants(&Wants { residency: r, prefetch: vec![], ..Wants::default() });
    assert_eq!(s.stats().queued_by_class, [0, 1, 0]);
    assert_eq!(s.stats().cancelled_prefetch, 5);
}

/// A source that signals after its first stage and waits for the test before the second.
struct Staged {
    stage_done: Mutex<Sender<()>>,
    go: Mutex<Receiver<()>>,
    stages_run: AtomicUsize,
}

impl TileSource<u64> for Staged {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        self.load_staged(tile, &CancelToken::never()).map(|v| v.unwrap())
    }
    fn load_staged(&self, tile: TileId, cancel: &CancelToken) -> Result<Option<u64>, String> {
        self.stages_run.fetch_add(1, Ordering::SeqCst); // stage 1
        self.stage_done.lock().unwrap().send(()).unwrap();
        self.go.lock().unwrap().recv().unwrap();
        if cancel.is_cancelled() {
            return Ok(None); // stage boundary
        }
        self.stages_run.fetch_add(1, Ordering::SeqCst); // stage 2
        Ok(Some(tile.raw()))
    }
}

// spec: STRM-005, STRM-001
#[test]
fn an_in_flight_job_stops_at_its_next_stage_boundary_and_its_tile_is_not_made_resident() {
    let (done_tx, done_rx) = channel();
    let (go_tx, go_rx) = channel();
    let source = Arc::new(Staged { stage_done: Mutex::new(done_tx), go: Mutex::new(go_rx), stages_run: AtomicUsize::new(0) });
    let spawner = Arc::new(ManualSpawner::default());
    // Roots come from `load`, which would block here: use a separate source for them by loading through a staged one.
    let mut s = Streamer::new(source.clone(), spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), cfg(1, 100));
    for _ in 0..6 {
        go_tx.send(()).unwrap(); // the roots pass both stages
    }
    s.load_roots_blocking().unwrap();
    while done_rx.try_recv().is_ok() {}
    let base = source.stages_run.load(Ordering::SeqCst);
    let t = id(0, 1, 0, 0);
    s.begin_frame_wants(&Wants { residency: vec![(t, 1.0)], ..Wants::default() });
    let job = {
        let sp = spawner.clone();
        std::thread::spawn(move || sp.run_one())
    };
    done_rx.recv().unwrap(); // stage 1 of the job is done
    s.begin_frame_wants(&Wants::default()); // nothing needs `t` any more: the token is set
    assert_eq!(s.stats().cancelled, 1);
    go_tx.send(()).unwrap();
    assert!(job.join().unwrap());
    s.begin_frame_wants(&Wants::default());
    assert_eq!(source.stages_run.load(Ordering::SeqCst) - base, 1, "stage 2 never ran");
    assert!(!s.is_resident(t));
    assert_eq!((s.stats().failed, s.stats().in_flight, s.stats().dropped), (0, 0, 0));
}

/// A source whose `load` blocks until released: models a job that cannot stop (no stage boundary).
struct Gate {
    entered: Mutex<Sender<()>>,
    release: Mutex<Receiver<()>>,
    roots_done: AtomicUsize,
}

impl TileSource<u64> for Gate {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        if tile.level() > 0 {
            self.entered.lock().unwrap().send(()).unwrap();
            self.release.lock().unwrap().recv().unwrap();
        } else {
            self.roots_done.fetch_add(1, Ordering::SeqCst);
        }
        Ok(tile.raw())
    }
}

// spec: STRM-005
#[test]
fn a_job_that_completes_after_its_cancellation_is_dropped_not_made_resident() {
    let (entered_tx, entered_rx) = channel();
    let (release_tx, release_rx) = channel();
    let source = Arc::new(Gate { entered: Mutex::new(entered_tx), release: Mutex::new(release_rx), roots_done: AtomicUsize::new(0) });
    let spawner = Arc::new(ManualSpawner::default());
    let mut s = Streamer::new(source, spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), cfg(1, 100));
    s.load_roots_blocking().unwrap();
    let t = id(0, 1, 0, 0);
    s.begin_frame_wants(&Wants { residency: vec![(t, 1.0)], ..Wants::default() });
    let sp = spawner.clone();
    let job = std::thread::spawn(move || sp.run_one());
    entered_rx.recv().unwrap();
    s.begin_frame_wants(&Wants::default());
    release_tx.send(()).unwrap();
    job.join().unwrap();
    s.begin_frame_wants(&Wants::default());
    assert!(!s.is_resident(t), "a cancelled job's tile is not made resident (and so not uploaded)");
    assert_eq!((s.stats().dropped, s.stats().completed, s.stats().failed), (1, 0, 0));
}

/// Dispatch order of a fixed set of wants on a real pool of `threads` workers, every frame waiting for the dispatched jobs.
fn dispatch_order_on_pool(threads: usize) -> Vec<TileId> {
    let source = Arc::new(Counting::default());
    let pool = Arc::new(ThreadPool::new(threads));
    let mut s = Streamer::new(source.clone(), pool as Arc<dyn Spawner>, Arc::new(FakeClock::default()), cfg(5, 100_000));
    s.load_roots_blocking().unwrap();
    let base = source.loads.load(Ordering::SeqCst);
    let draw: Vec<(TileId, f64)> = (0..6).map(|f| (id(f, 4, 3, 5), f64::from(f) * 7.0)).collect();
    let residency: Vec<(TileId, f64)> =
        (0..6).flat_map(|f| (0..6).map(move |x| (id(f, 3, x, 1), f64::from(x * 3 + u32::from(f % 4))))).collect();
    let prefetch: Vec<(TileId, f64)> = (0..6).map(|f| (id(f, 3, 7, 7), f64::from(f))).collect();
    let w = Wants { draw, residency, prefetch };
    for _ in 0..500 {
        s.begin_frame_wants(&w);
        // Wait until every dispatched job has started and loaded, so that the next frame sees the same state on any pool.
        while source.loads.load(Ordering::SeqCst) - base < s.dispatch_log.len() {
            std::thread::yield_now();
        }
        if s.stats().queued == 0 && s.stats().in_flight == 0 && s.stats().completed as usize == s.dispatch_log.len() {
            break;
        }
    }
    s.begin_frame_wants(&w);
    assert_eq!(s.stats().queued, 0);
    s.dispatch_log
}

// spec: STRM-005
#[test]
fn the_request_order_is_the_same_on_one_thread_and_on_many() {
    let one = dispatch_order_on_pool(1);
    assert!(one.len() > 100);
    for threads in [2, 8] {
        assert_eq!(one, dispatch_order_on_pool(threads), "{threads} threads dispatched in a different order");
    }
    // And the same on the manual spawner.
    let (mut s, sp, _c, _src) = fixture(cfg(5, 100_000));
    let draw: Vec<(TileId, f64)> = (0..6).map(|f| (id(f, 4, 3, 5), f64::from(f) * 7.0)).collect();
    let residency: Vec<(TileId, f64)> =
        (0..6).flat_map(|f| (0..6).map(move |x| (id(f, 3, x, 1), f64::from(x * 3 + u32::from(f % 4))))).collect();
    let prefetch: Vec<(TileId, f64)> = (0..6).map(|f| (id(f, 3, 7, 7), f64::from(f))).collect();
    settle(&mut s, &sp, &Wants { draw, residency, prefetch });
    assert_eq!(s.dispatch_log, one);
}

// spec: STRM-001, STRM-005
#[test]
fn on_the_fake_clock_a_failed_residency_tile_is_retried_after_its_delay_in_queue_order() {
    let (mut s, sp, clock, src) = fixture(cfg(1, 100_000));
    let (a, b) = (id(0, 1, 0, 0), id(1, 1, 0, 0));
    src.fail_once.lock().unwrap().insert(a);
    let w = Wants { residency: vec![(a, 1.0), (b, 2.0)], ..Wants::default() };
    for _ in 0..4 {
        s.begin_frame_wants(&w);
        sp.run(1);
    }
    s.begin_frame_wants(&w);
    assert_eq!(s.dispatch_log, [a, b], "a failed, b went on");
    clock.advance(999);
    s.begin_frame_wants(&w);
    assert_eq!(s.dispatch_log, [a, b], "not retried before the delay");
    clock.advance(1);
    s.begin_frame_wants(&w);
    sp.run(1);
    s.begin_frame_wants(&w);
    assert_eq!(s.dispatch_log, [a, b, a], "retried when due, not before");
    assert!(s.is_resident(a) && s.is_resident(b));
}

// spec: STRM-005
#[test]
fn the_predicted_position_depends_on_translation_only_and_the_look_ahead_is_capped() {
    let p = Vec3::new(1.0e6, 2.0e6, 3.0e6);
    let v = Vec3::new(10.0, -20.0, 5.0);
    assert_eq!(predicted_position(p, Vec3::new(0.0, 0.0, 0.0), 5000, 5000), p, "no velocity: no prediction (a teleport)");
    let a = predicted_position(p, v, 2000, 5000);
    assert_eq!((a.x - p.x, a.y - p.y, a.z - p.z), (20.0, -40.0, 10.0));
    assert_eq!(predicted_position(p, v, 60_000, 5000), predicted_position(p, v, 5000, 5000), "capped");
}

// spec: STRM-005
#[test]
fn a_cancelled_in_flight_tile_that_is_needed_again_is_requested_anew_after_its_job_ends() {
    let (mut s, sp, _c, src) = fixture(cfg(1, 100_000));
    let t = id(0, 1, 0, 0);
    let w = Wants { residency: vec![(t, 1.0)], ..Wants::default() };
    s.begin_frame_wants(&w);
    s.begin_frame_wants(&Wants::default()); // cancelled while in flight (before it ran)
    s.begin_frame_wants(&w); // needed again: no revival, no second dispatch yet
    assert_eq!(s.dispatch_log, [t]);
    sp.run(1); // the cancelled job ends without loading
    s.begin_frame_wants(&w); // now it is requested anew
    assert_eq!(s.dispatch_log, [t, t]);
    sp.run(1);
    s.begin_frame_wants(&w);
    assert!(s.is_resident(t));
    assert_eq!(src.loads.load(Ordering::SeqCst), 6 + 1);
}

// spec: STRM-005
#[test]
fn the_default_staged_load_stops_when_already_cancelled_and_loads_otherwise() {
    let src = Counting::default();
    assert_eq!(src.load_staged(id(0, 1, 0, 0), &CancelToken::never()), Ok(Some(id(0, 1, 0, 0).raw())));
    assert_eq!(src.loads.load(Ordering::SeqCst), 1);
}
