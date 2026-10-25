//! STRM-005, STRM-031: a request still queued when its tile changes class is queued under the new class, whether or not the refresh
//! settle window holds it, so the scheduler never reads a key of a tile from a layer that no longer lists it.

use planet_core::{Face, TileId};
use planet_streaming::{FakeClock, ManualSpawner, Spawner, StreamConfig, Streamer, TileSource, Wants};
use std::sync::Arc;

fn id(f: u8, l: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(f), l, x, y).unwrap()
}

struct Raw;

impl TileSource<u64> for Raw {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        Ok(tile.raw())
    }
}

// spec: STRM-005
#[test]
fn a_queued_refresh_the_settle_window_holds_follows_its_tile_from_the_draw_class_to_the_residency_class() {
    let spawner = Arc::new(ManualSpawner::default());
    let clock = Arc::new(FakeClock::default());
    let config = StreamConfig { max_in_flight: 1, queue_ahead: 0, retry_after_ms: 100, ..Default::default() };
    let mut s = Streamer::new(Arc::new(Raw) as Arc<dyn TileSource<u64>>, spawner.clone() as Arc<dyn Spawner>, clock.clone(), config);
    s.load_roots_blocking().unwrap();
    let (t, busy) = (id(2, 3, 1, 2), id(4, 3, 5, 5));
    let draw = Wants { draw: vec![(t, 1.0)], ..Wants::default() };
    for _ in 0..40 {
        if s.is_resident(t) && s.stats().in_flight == 0 {
            break;
        }
        s.begin_frame_wants(&draw);
        spawner.run(4);
    }
    assert!(s.is_resident(t));
    // A job that never ends keeps the only slot, so the refresh of `t` stays queued.
    let with_busy = Wants { draw: vec![(busy, 0.0), (t, 1.0)], ..Wants::default() };
    s.begin_frame_wants(&with_busy);
    assert_eq!(s.stats().in_flight, 1);
    s.set_refresh_settle(40);
    s.refresh(&[t]);
    clock.advance(50);
    s.begin_frame_wants(&with_busy);
    // The next edit holds the tile again while its request waits in the queue.
    s.refresh(&[t]);
    // `t` leaves the draw class for the residency class.
    let moved = Wants { draw: vec![(busy, 0.0)], residency: vec![(t, 1.0)], ..Wants::default() };
    s.begin_frame_wants(&moved);
    // The busy job ends and the dispatcher reads the queue.
    for _ in 0..8 {
        spawner.run(4);
        clock.advance(10);
        s.begin_frame_wants(&moved);
    }
    assert!(s.is_resident(t));
}
