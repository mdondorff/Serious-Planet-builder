//! STRM-026: a resident tile whose inputs changed is requested again without being dropped, replaced in place when the new version
//! arrives, and its old slot goes back. Fake clock, manual spawner: counts and payloads only.

use planet_core::{Face, TileId};
use planet_streaming::{FakeClock, ManualSpawner, Spawner, StreamConfig, Streamer, TileSource};
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

type Fixture = (Streamer<(u64, u64)>, Arc<ManualSpawner>, Arc<RevSource>);

fn fixture(max_in_flight: usize) -> Fixture {
    let spawner = Arc::new(ManualSpawner::default());
    let source = Arc::new(RevSource::default());
    let config = StreamConfig { max_in_flight, record_dispatches: true, record_changes: true, ..Default::default() };
    let mut s = Streamer::new(
        Arc::clone(&source) as Arc<dyn TileSource<(u64, u64)>>,
        spawner.clone() as Arc<dyn Spawner>,
        Arc::new(FakeClock::default()),
        config,
    );
    s.load_roots_blocking().unwrap();
    (s, spawner, source)
}

/// Frames with `wants` until `tile` is resident.
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

// spec: STRM-026
#[test]
fn a_refreshed_tile_is_served_from_its_old_slot_until_the_new_version_replaces_it_and_the_old_slot_goes_back() {
    let (mut s, sp, source) = fixture(2);
    let t = id(2, 3, 1, 2);
    let wants = [(t, 1.0)];
    settle(&mut s, &sp, &wants, t);
    assert_eq!(s.get(t).as_deref(), Some(&(t.raw(), 0)));
    for _ in 0..8 {
        s.begin_frame(&wants);
    }
    let (old_slot, in_use) = (s.slot_of(t), s.stats().main_in_use);
    s.take_changes();

    source.rev.store(1, Ordering::SeqCst);
    assert_eq!(s.refresh(&[t]), 1, "a wanted resident tile is queued once");
    s.begin_frame(&wants);
    assert_eq!(s.stats().in_flight, 1, "the refresh job runs");
    assert!(s.is_resident(t) && s.get(t).as_deref() == Some(&(t.raw(), 0)), "the old version is still what the tile is");
    assert_eq!(s.slot_of(t), old_slot, "and it is still in its slot");
    assert_eq!(s.stats().main_in_use, in_use + 1, "the new version takes a second slot while it loads");

    sp.run(2);
    s.begin_frame(&wants);
    assert_eq!(s.get(t).as_deref(), Some(&(t.raw(), 1)), "the new version replaced it");
    assert_ne!(s.slot_of(t), old_slot, "in the second slot");
    assert_eq!(s.stats().refreshed, 1);
    assert!(s.take_changes().contains(&planet_streaming::ResidentChange::Added(t)), "the loop is told the tile has a new version");
    for _ in 0..8 {
        s.begin_frame(&wants);
    }
    assert_eq!(s.stats().main_in_use, in_use, "the old slot went back");
}

// spec: STRM-026
#[test]
fn a_resident_tile_in_no_class_is_evicted_by_a_refresh_and_a_tile_that_was_never_loaded_is_left_alone() {
    let (mut s, sp, _) = fixture(2);
    let (t, other) = (id(2, 3, 1, 2), id(4, 3, 0, 0));
    settle(&mut s, &sp, &[(t, 1.0)], t);
    s.begin_frame(&[]);
    assert!(s.is_resident(t), "a tile nothing wants stays resident until its slot is needed");
    assert_eq!(s.refresh(&[t, other]), 0);
    assert!(!s.is_resident(t), "its old version has no use: it is evicted, and loads from the new field the next time it is wanted");
    assert!(!s.is_resident(other));
}

// spec: STRM-026
#[test]
fn a_job_running_when_the_refresh_comes_is_dropped_and_the_tile_is_loaded_again_from_the_new_field() {
    let (mut s, sp, source) = fixture(2);
    let t = id(2, 3, 1, 2);
    let wants = [(t, 1.0)];
    // Not yet resident: its first load is in flight when the edit comes.
    s.begin_frame(&wants);
    s.begin_frame(&wants);
    sp.run(0);
    source.rev.store(1, Ordering::SeqCst);
    assert_eq!(s.refresh(&[t]), 0, "nothing resident to refresh");
    settle(&mut s, &sp, &wants, t);
    assert_eq!(s.get(t).as_deref(), Some(&(t.raw(), 1)), "whatever was in flight, the tile is the new field's");

    // Resident, and a refresh job running when a second edit comes: the running job is dropped, one more refresh is made.
    for _ in 0..6 {
        s.begin_frame(&wants);
    }
    source.rev.store(2, Ordering::SeqCst);
    assert_eq!(s.refresh(&[t]), 1);
    s.begin_frame(&wants);
    assert_eq!(s.stats().in_flight, 1);
    source.rev.store(3, Ordering::SeqCst);
    assert_eq!(s.refresh(&[t]), 0, "the running job is told to stop, no second request is queued now");
    for _ in 0..4 {
        assert!(s.get(t).is_some(), "the tile is served all the time");
        sp.run(4);
        s.begin_frame(&wants);
    }
    settle(&mut s, &sp, &wants, t);
    assert_eq!(s.get(t).as_deref(), Some(&(t.raw(), 3)), "the last field wins");
    assert!(s.stats().cancelled >= 1, "the running jobs were told to stop");
}

// spec: STRM-026, STRM-005
#[test]
fn a_refresh_starts_ahead_of_the_rest_of_the_residency_class() {
    let (mut s, sp, source) = fixture(1);
    let (fine, coarse) = (id(2, 5, 3, 3), id(4, 2, 1, 1));
    s.set_residency(&[(fine, 1.0)]);
    for _ in 0..30 {
        s.step();
        sp.run(4);
        if s.is_resident(fine) && s.stats().in_flight == 0 {
            break;
        }
    }
    assert!(s.is_resident(fine));
    // `coarse` (level 2) would go before `fine` (level 5) in the class; the refresh of `fine` goes first.
    source.rev.store(1, Ordering::SeqCst);
    s.set_residency(&[(fine, 1.0), (coarse, 2.0)]);
    s.refresh(&[fine]);
    let before = s.dispatch_log.len();
    s.step();
    assert_eq!(s.dispatch_log[before], fine, "the refresh of an edited tile goes before the other requests of its class");
}
