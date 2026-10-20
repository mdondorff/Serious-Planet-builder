//! STRM-031: refresh work made stale by a newer edit is not computed. Waiting refreshes stay one entry, a running job is told to stop and its
//! result dropped before the tile is made resident, the settle window holds a re-keyed tile's next request until the edits stop, and drawn
//! tiles refresh first. Fake clock, manual spawner: counts and payloads only.

use planet_core::{Face, TileId};
use planet_streaming::{FakeClock, ManualSpawner, Spawner, StreamConfig, Streamer, TileSource, Wants};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

fn id(f: u8, l: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(f), l, x, y).unwrap()
}

/// A tile is its raw id and the revision of the "field" it was made from.
#[derive(Default)]
struct RevSource {
    rev: AtomicU64,
}

impl TileSource<(u64, u64)> for RevSource {
    fn load(&self, tile: TileId) -> Result<(u64, u64), String> {
        Ok((tile.raw(), self.rev.load(Ordering::SeqCst)))
    }
}

type Fixture = (Streamer<(u64, u64)>, Arc<ManualSpawner>, Arc<RevSource>, Arc<FakeClock>);

fn fixture(max_in_flight: usize) -> Fixture {
    let spawner = Arc::new(ManualSpawner::default());
    let source = Arc::new(RevSource::default());
    let clock = Arc::new(FakeClock::default());
    let config = StreamConfig { max_in_flight, record_dispatches: true, retry_after_ms: 100, ..Default::default() };
    let mut s =
        Streamer::new(Arc::clone(&source) as Arc<dyn TileSource<(u64, u64)>>, spawner.clone() as Arc<dyn Spawner>, clock.clone(), config);
    s.load_roots_blocking().unwrap();
    (s, spawner, source, clock)
}

/// Frames with `wants` until `tile` is resident and nothing runs.
fn settle(s: &mut Streamer<(u64, u64)>, sp: &ManualSpawner, wants: &[(TileId, f64)], tile: TileId) {
    for _ in 0..40 {
        if s.is_resident(tile) && s.stats().in_flight == 0 {
            return;
        }
        s.begin_frame(wants);
        sp.run(4);
    }
    panic!("{tile:?} did not become resident");
}

/// How many times `tile` was dispatched since `mark` entries of the log.
fn dispatched(s: &Streamer<(u64, u64)>, tile: TileId, mark: usize) -> usize {
    s.dispatch_log[mark..].iter().filter(|t| **t == tile).count()
}

// spec: STRM-031
#[test]
fn refreshes_that_find_the_tiles_refresh_waiting_stay_one_entry_and_are_counted() {
    let (mut s, sp, source, _) = fixture(2);
    let t = id(2, 3, 1, 2);
    let wants = [(t, 1.0)];
    settle(&mut s, &sp, &wants, t);
    let mark = s.dispatch_log.len();
    source.rev.store(1, Ordering::SeqCst);
    assert_eq!(s.refresh(&[t]), 1, "the first refresh queues the request");
    source.rev.store(2, Ordering::SeqCst);
    assert_eq!(s.refresh(&[t]), 0, "the second finds it waiting");
    assert_eq!(s.refresh(&[t, t]), 0, "and so do two more in one call");
    assert_eq!(s.stats().refresh_coalesced, 3);
    s.begin_frame(&wants);
    sp.run(4);
    settle(&mut s, &sp, &wants, t);
    assert_eq!(dispatched(&s, t, mark), 1, "one job for four refreshes");
    assert_eq!(s.get(t).as_deref(), Some(&(t.raw(), 2)), "made from the newest field");
    assert_eq!((s.stats().refresh_cancelled, s.stats().refresh_dropped), (0, 0), "nothing was running");
}

// spec: STRM-031
#[test]
fn a_running_job_of_a_re_keyed_tile_is_stopped_once_and_its_result_is_dropped_before_the_tile_is_made_resident() {
    let (mut s, sp, source, _) = fixture(2);
    let t = id(2, 3, 1, 2);
    let wants = [(t, 1.0)];
    settle(&mut s, &sp, &wants, t);
    for _ in 0..4 {
        s.begin_frame(&wants);
    }
    let mark = s.dispatch_log.len();
    source.rev.store(1, Ordering::SeqCst);
    s.refresh(&[t]);
    s.begin_frame(&wants);
    assert_eq!(s.stats().in_flight, 1, "the refresh job was dispatched");
    sp.run(1);
    // The job finished with a result made from revision 1; a newer edit names the tile before the loop collects it.
    source.rev.store(2, Ordering::SeqCst);
    s.refresh(&[t]);
    s.refresh(&[t]);
    assert_eq!((s.stats().refresh_cancelled, s.stats().refresh_coalesced), (1, 1), "stopped once, the second edit found the stop pending");
    s.begin_frame(&wants);
    assert_eq!(s.stats().refresh_dropped, 1, "the result was dropped");
    assert_eq!(s.get(t).as_deref(), Some(&(t.raw(), 0)), "the old version is still the resident one: nothing was made resident");
    assert_eq!(s.stats().refreshed, 0);
    settle(&mut s, &sp, &wants, t);
    assert_eq!(s.get(t).as_deref(), Some(&(t.raw(), 2)), "and the tile is refreshed once from the newest field");
    assert_eq!(s.stats().refreshed, 1);
    assert_eq!(dispatched(&s, t, mark), 2, "the stopped job and the one that counts");
}

// spec: STRM-031
#[test]
fn twenty_quick_edits_with_a_settle_window_start_one_refresh_after_the_last_and_at_most_two_in_all() {
    for running_at_first in [false, true] {
        let (mut s, sp, source, clock) = fixture(2);
        let t = id(2, 3, 1, 2);
        let wants = [(t, 1.0)];
        settle(&mut s, &sp, &wants, t);
        for _ in 0..4 {
            s.begin_frame(&wants);
        }
        let mark = s.dispatch_log.len();
        if running_at_first {
            // A refresh job is already running when the quick sequence starts (before the window was set).
            source.rev.store(1, Ordering::SeqCst);
            s.refresh(&[t]);
            s.begin_frame(&wants);
            assert_eq!(s.stats().in_flight, 1);
        }
        s.set_refresh_settle(40);
        for edit in 0..20u64 {
            source.rev.store(10 + edit, Ordering::SeqCst);
            s.refresh(&[t]);
            clock.advance(10);
            s.begin_frame(&wants);
            sp.run(4);
        }
        assert!(dispatched(&s, t, mark) <= usize::from(running_at_first), "no job starts while the edits keep coming");
        // The edits stop; the window ends 40 ms after the last one.
        clock.advance(20);
        s.begin_frame(&wants);
        assert_eq!(dispatched(&s, t, mark), usize::from(running_at_first), "still held 30 ms after the last edit");
        clock.advance(10);
        s.begin_frame(&wants);
        settle(&mut s, &sp, &wants, t);
        let jobs = dispatched(&s, t, mark);
        assert!((1..=2).contains(&jobs), "running_at_first {running_at_first}: {jobs} jobs for 20 edits");
        assert_eq!(s.get(t).as_deref(), Some(&(t.raw(), 29)), "the result is made from the last edit's field");
        assert!(s.stats().refresh_coalesced >= 19, "the edits that found the entry held are counted: {}", s.stats().refresh_coalesced);
    }
}

// spec: STRM-031
#[test]
fn a_single_edit_is_requested_after_the_window_with_it_and_at_once_without_it_and_a_streamer_never_set_is_as_before() {
    for (settle_ms, set) in [(40u64, true), (0, true), (0, false)] {
        let (mut s, sp, source, clock) = fixture(2);
        let t = id(2, 3, 1, 2);
        let wants = [(t, 1.0)];
        settle(&mut s, &sp, &wants, t);
        for _ in 0..4 {
            s.begin_frame(&wants);
        }
        if set {
            s.set_refresh_settle(settle_ms);
        }
        let mark = s.dispatch_log.len();
        source.rev.store(1, Ordering::SeqCst);
        s.refresh(&[t]);
        s.begin_frame(&wants);
        let at_once = dispatched(&s, t, mark);
        if settle_ms == 0 {
            assert_eq!(at_once, 1, "settle {settle_ms}: dispatched at once");
        } else {
            assert_eq!(at_once, 0, "held for the window");
            clock.advance(39);
            s.begin_frame(&wants);
            assert_eq!(dispatched(&s, t, mark), 0, "39 ms is inside it");
            clock.advance(1);
            s.begin_frame(&wants);
            assert_eq!(dispatched(&s, t, mark), 1, "at 40 ms the request is made");
        }
        sp.run(4);
        settle(&mut s, &sp, &wants, t);
        assert_eq!(s.get(t).as_deref(), Some(&(t.raw(), 1)));
        assert_eq!(s.stats().refresh_coalesced, 0);
    }
}

// spec: STRM-031
#[test]
fn the_refreshes_of_drawn_tiles_are_dispatched_before_those_of_the_other_classes_whatever_order_the_edit_names_them_in() {
    let (mut s, sp, source, _) = fixture(1);
    let (drawn, resident, prefetch) = (id(2, 3, 1, 2), id(3, 3, 0, 1), id(4, 3, 2, 2));
    let wants = Wants { draw: vec![(drawn, 1.0)], residency: vec![(resident, 1.0)], prefetch: vec![(prefetch, 1.0)] };
    for _ in 0..60 {
        s.begin_frame_wants(&wants);
        sp.run(2);
        if [drawn, resident, prefetch].iter().all(|t| s.is_resident(*t)) && s.stats().in_flight == 0 {
            break;
        }
    }
    assert!([drawn, resident, prefetch].iter().all(|t| s.is_resident(*t)), "all three resident");
    let mark = s.dispatch_log.len();
    source.rev.store(1, Ordering::SeqCst);
    // Named in the opposite order: prefetch, residency, draw.
    assert_eq!(s.refresh(&[prefetch, resident, drawn]), 3);
    for _ in 0..12 {
        s.begin_frame_wants(&wants);
        sp.run(1);
    }
    let order: Vec<TileId> = s.dispatch_log[mark..].to_vec();
    assert_eq!(order, vec![drawn, resident, prefetch], "draw, then residency, then prefetch");
}

// spec: STRM-031
#[test]
fn a_refresh_the_window_holds_counts_as_queued_for_the_caller_once() {
    let (mut s, sp, _, _) = fixture(2);
    let t = id(2, 3, 1, 2);
    let wants = [(t, 1.0)];
    settle(&mut s, &sp, &wants, t);
    for _ in 0..4 {
        s.begin_frame(&wants);
    }
    s.set_refresh_settle(40);
    assert_eq!(s.refresh(&[t]), 1, "held counts as queued: the caller's refreshed-tiles figure is not 0");
    assert_eq!(s.refresh(&[t]), 0, "the second finds it held");
    assert_eq!(s.held_refreshes(), 1);
}

// spec: STRM-031
#[test]
fn a_refresh_that_finds_a_held_entry_and_a_stopped_job_is_one_coalesced_refresh() {
    let (mut s, sp, source, clock) = fixture(2);
    let t = id(2, 3, 1, 2);
    let wants = [(t, 1.0)];
    settle(&mut s, &sp, &wants, t);
    for _ in 0..4 {
        s.begin_frame(&wants);
    }
    s.set_refresh_settle(40);
    // A refresh job is running (the first edit's window has passed and the request was made).
    source.rev.store(1, Ordering::SeqCst);
    s.refresh(&[t]);
    clock.advance(40);
    s.begin_frame(&wants);
    assert_eq!(s.stats().in_flight, 1);
    // Two more edits while it runs: the first stops it, the second finds the stop pending and the entry held.
    source.rev.store(2, Ordering::SeqCst);
    s.refresh(&[t]);
    assert_eq!((s.stats().refresh_cancelled, s.stats().refresh_coalesced), (1, 0), "the first stops the job and joins nothing");
    source.rev.store(3, Ordering::SeqCst);
    s.refresh(&[t]);
    assert_eq!((s.stats().refresh_cancelled, s.stats().refresh_coalesced), (1, 1), "the second is one coalesced refresh, counted once");
}
