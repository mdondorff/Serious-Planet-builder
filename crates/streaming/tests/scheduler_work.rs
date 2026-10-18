//! The scheduler's frame work follows what changed, not the size of the wants (STRM-019, STRM-005, TEST-018), tier A, counts only
//! (CON-20): the streamer counts the elements its frame touches ([`planet_streaming::SchedWork`]) and these tests read the
//! counter before and after a frame. No clock, no thread: fake clock, manual spawner.

use planet_core::{Face, TileId};
use planet_streaming::{
    degrade, FakeClock, ManualSpawner, ResidencyManager, SchedWork, Spawner, StreamConfig, Streamer, TileSource, Wants,
};
use std::sync::Arc;

struct Source;

impl TileSource<u64> for Source {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        Ok(tile.raw())
    }
}

/// Every tile of face 0 down to `level` (closed under parents), with a priority that grows with the level and, within a level,
/// with the tile's place (a stand-in for distance: a parent is never worse than its children).
fn face(level: u8) -> Vec<(TileId, f64)> {
    let mut out = Vec::new();
    for l in 0..=level {
        let n = 1u32 << l;
        for x in 0..n {
            for y in 0..n {
                out.push((TileId::new(Face(0), l, x, y).unwrap(), f64::from(l) * 1000.0 + f64::from(x + y) * 0.001));
            }
        }
    }
    out
}

/// `k` tiles of `face(level)` at the finest level, none in another's subtree.
fn some_leaves(level: u8, k: u32) -> Vec<TileId> {
    (0..k).map(|i| TileId::new(Face(0), level, i % 2, i / 2).unwrap()).collect()
}

fn streamer(capacity: usize, max_in_flight: usize) -> (Streamer<u64>, Arc<ManualSpawner>) {
    let spawner = Arc::new(ManualSpawner::default());
    let config = StreamConfig { max_in_flight, capacity, retire_frames: 2, record_changes: false, ..Default::default() };
    let mut s = Streamer::new(Arc::new(Source), spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config);
    s.load_roots_blocking().unwrap();
    (s, spawner)
}

/// A streamer with `r` as R and `draw` as D, everything resident and nothing queued or running.
fn settled(r: &[(TileId, f64)], draw: &[(TileId, f64)], capacity: usize) -> (Streamer<u64>, Arc<ManualSpawner>) {
    let (mut s, sp) = streamer(capacity, 64);
    s.set_draw(draw);
    s.set_residency(r);
    for _ in 0..10_000 {
        s.step();
        if sp.run(10_000) == 0 && s.stats().queued == 0 && s.stats().in_flight == 0 {
            break;
        }
    }
    s.step();
    let st = s.stats();
    assert_eq!((st.queued, st.in_flight), (0, 0));
    assert_eq!(st.resident, r.len() + 5, "all of R and the roots of the other five faces are resident");
    (s, sp)
}

fn frame(s: &mut Streamer<u64>) -> SchedWork {
    let before = s.work();
    s.step();
    s.work().since(&before)
}

// spec: STRM-019, STRM-005, TEST-018
#[test]
fn a_settled_frame_does_no_work_whatever_the_size_of_r() {
    let mut seen = Vec::new();
    for level in [4u8, 5, 6] {
        let r = face(level);
        let draw: Vec<(TileId, f64)> = some_leaves(level, 8).into_iter().map(|t| (t, 0.0)).collect();
        let (mut s, _sp) = settled(&r, &draw, 1_000_000);
        for _ in 0..5 {
            let w = frame(&mut s);
            assert_eq!(w, SchedWork::default(), "R of {} tiles: a frame with nothing to do touches nothing", r.len());
        }
        seen.push(r.len());
    }
    assert!(seen[2] >= 4 * seen[1] - 4 && seen[1] >= 4 * seen[0] - 4, "R grows fourfold per step: {seen:?}");
}

// spec: STRM-019, STRM-005
#[test]
fn a_frame_that_removes_c_tiles_from_r_touches_a_number_of_elements_proportional_to_c_not_to_r() {
    let mut per_c = Vec::new();
    for level in [5u8, 6] {
        let r = face(level);
        let mut row = Vec::new();
        for c in [1u32, 10, 100] {
            let (mut s, _sp) = settled(&r, &[], 1_000_000);
            // c finest tiles leave R (the first ones by place), nothing else changes.
            let mut leaves: Vec<TileId> = r.iter().map(|e| e.0).filter(|t| t.level() == level).collect();
            leaves.sort();
            let left: std::collections::HashSet<TileId> = leaves.into_iter().take(c as usize).collect();
            let next: Vec<(TileId, f64)> = r.iter().copied().filter(|e| !left.contains(&e.0)).collect();
            s.set_residency(&next);
            let w = frame(&mut s);
            eprintln!("  R {} remove {c}: {w:?}", r.len());
            // Per tile: its count in the layer, its place among the eviction candidates, its change of class; its parent keeps
            // its own count (it is listed), so a removal never walks further.
            assert!(w.updated <= 4 * u64::from(c), "removing {c} tiles of R: {w:?}");
            assert!(w.layer_updates[1] <= 2 * u64::from(c), "{w:?}");
            // The one pass over R that finds what differs (a sequential comparison, no write).
            assert!(w.scanned <= r.len() as u64 + 4 * u64::from(c) + 8, "{w:?}");
            row.push((w.updated, w.layer_updates[1]));
        }
        per_c.push(row);
    }
    assert_eq!(per_c[0], per_c[1], "the writes of the frame do not depend on the size of R (4x apart)");
}

// spec: STRM-019, STRM-005
#[test]
fn a_frame_that_adds_c_tiles_to_r_touches_a_number_of_elements_proportional_to_c_not_to_r() {
    let mut per_c = Vec::new();
    for level in [5u8, 6] {
        let r = face(level);
        let mut row = Vec::new();
        for c in [1u32, 10, 100] {
            let (mut s, _sp) = settled(&r, &[], 1_000_000);
            // c tiles of the next level appear below the first leaves, their parents already in R.
            let mut leaves: Vec<TileId> = r.iter().map(|e| e.0).filter(|t| t.level() == level).collect();
            leaves.sort();
            let mut next = r.clone();
            let mut added = 0;
            'outer: for parent in leaves {
                for child in parent.children().unwrap() {
                    next.push((child, f64::from(level + 1) * 1000.0));
                    added += 1;
                    if added == c {
                        break 'outer;
                    }
                }
            }
            s.set_residency(&next);
            let w = frame(&mut s);
            eprintln!("  R {} add {c}: {w:?}", r.len());
            // Each new tile is one layer write, one queue entry; the first `max_in_flight` also start (and cost a dispatch).
            assert!(w.updated <= 8 * u64::from(c), "adding {c} tiles to R: {w:?}");
            assert!(w.layer_updates[1] <= 2 * u64::from(c), "{w:?}");
            assert!(w.scanned <= r.len() as u64 + 4 * u64::from(c) + 8, "{w:?}");
            row.push((w.updated, w.layer_updates[1]));
        }
        per_c.push(row);
    }
    assert_eq!(per_c[0], per_c[1], "the writes of the frame do not depend on the size of R (4x apart)");
}

// spec: STRM-019, STRM-005
#[test]
fn a_changed_priority_alone_writes_no_tile_of_a_layer_and_a_draw_set_that_moves_touches_only_the_draw_layer() {
    let r = face(6);
    let draw: Vec<(TileId, f64)> = some_leaves(6, 16).into_iter().enumerate().map(|(i, t)| (t, i as f64)).collect();
    let (mut s, _sp) = settled(&r, &draw, 1_000_000);
    // The camera moved a little: every distance of R changes, no tile enters or leaves.
    let moved: Vec<(TileId, f64)> = r.iter().map(|&(t, p)| (t, p + 0.5)).collect();
    s.set_residency(&moved);
    let w = frame(&mut s);
    assert_eq!(w.layer_updates, [0, 0, 0], "no listed tile changed: {w:?}");
    assert_eq!(w.updated, 0, "no request is queued, so no key is computed: {w:?}");
    // The draw set turns: the same tiles in another order, one tile swapped. Only the draw layer is touched.
    let mut turned: Vec<(TileId, f64)> = draw.iter().rev().copied().collect();
    turned.pop();
    turned.push((TileId::new(Face(0), 6, 3, 0).unwrap(), 99.0));
    s.set_draw(&turned);
    let w = frame(&mut s);
    assert_eq!(w.layer_updates[1], 0, "R is not touched by a turn: {w:?}");
    assert!(w.layer_updates[0] <= 2 * 8, "the draw layer pays for the one tile that changed: {w:?}");
    assert!(w.scanned <= 4 * turned.len() as u64 + 8, "and a pass over the draw set only: {w:?}");
}

// spec: STRM-019, STRM-007
#[test]
fn the_manager_reports_the_degradation_numbers_of_the_pure_function_without_fitting_when_the_pool_holds_the_wants() {
    let r = face(5);
    let draw: Vec<(TileId, f64)> = some_leaves(5, 6).into_iter().map(|t| (t, 1.0)).collect();
    for capacity in [100_000usize, 10_000] {
        let (s, _sp) = streamer(capacity, 8);
        let mut m = ResidencyManager::new(s);
        m.set_draw(&draw);
        m.set_residency(&r);
        let report = m.step().clone();
        let want = degrade(&draw, &r, capacity, false, 0).report;
        assert_eq!(report, want, "capacity {capacity}");
        assert_eq!(report.dropped, 0);
    }
    // A pool smaller than R: fitted by the pure function, and the streamer is given the fitted lists.
    let capacity = r.len() / 2;
    let (s, _sp) = streamer(capacity, 8);
    let mut m = ResidencyManager::new(s);
    m.set_draw(&draw);
    m.set_residency(&r);
    let report = m.step().clone();
    assert_eq!(report, degrade(&draw, &r, capacity, false, 0).report);
    assert!(report.dropped > 0, "the pool is smaller than R: {report:?}");
    let _ = Wants::default();
}

// spec: STRM-019, STRM-005, STRM-007, CON-14
#[test]
fn a_manager_that_crosses_the_pools_bound_both_ways_decides_as_the_one_that_fits_every_frame() {
    let draw: Vec<(TileId, f64)> = some_leaves(5, 4).into_iter().map(|t| (t, 1.0)).collect();
    let capacity = face(5).len() / 2;
    let rig = || {
        let spawner = Arc::new(ManualSpawner::default());
        let config = StreamConfig { max_in_flight: 8, capacity, retire_frames: 2, record_dispatches: true, ..Default::default() };
        let mut s = Streamer::new(Arc::new(Source), spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config);
        s.load_roots_blocking().unwrap();
        (ResidencyManager::new(s), spawner)
    };
    let (mut full, sp_full) = rig();
    let (mut inc, sp_inc) = rig();
    // Small R (fits), R larger than the pool (fitted), small R again.
    let sizes = [3u8, 5, 3, 4, 5, 2];
    for (k, level) in sizes.iter().cycle().take(18).enumerate() {
        let r = face(*level);
        full.begin_frame(&Wants { draw: draw.clone(), residency: r.clone(), prefetch: vec![] });
        if k == 0 || sizes[(k) % sizes.len()] != sizes[(k + sizes.len() - 1) % sizes.len()] {
            inc.set_draw(&draw);
            inc.set_residency(&r);
        }
        inc.step();
        sp_full.run(20);
        sp_inc.run(20);
        assert_eq!(full.streamer().dispatch_log, inc.streamer().dispatch_log, "frame {k}");
        assert_eq!(full.streamer().stats(), inc.streamer().stats(), "frame {k}");
    }
}
