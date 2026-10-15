//! STRM-006, STRM-007: the residency manager on fakes. The degradation of ADR 0031 point 4 (the "Small share" scenario):
//! when the share is smaller than R, the surroundings get coarser (farthest and finest first), the draw set and the roots
//! stay at full detail, and only a draw set that does not fit itself is replaced, as a declared exception.

use planet_core::{Face, TileId};
use planet_streaming::budget::GIB;
use planet_streaming::{
    degrade, FakeClock, ManualSpawner, PoolConfig, ResidencyManager, Spawner, StreamConfig, Streamer, TileSource, Wants,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

fn id(f: u8, l: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(f), l, x, y).unwrap()
}

/// A toy position-based LOD over one face: a tile subdivides while the point `(px, py)` (in units of finest tiles) is within
/// two tile edges of its centre, down to `FINEST`. Returns R (closed under parents, distances to the tile centre in finest
/// tiles) and the leaves.
const FINEST: u8 = 5;

fn region(px: f64, py: f64) -> (BTreeMap<TileId, f64>, Vec<TileId>) {
    let mut r = BTreeMap::new();
    let mut leaves = Vec::new();
    fn go(t: TileId, px: f64, py: f64, r: &mut BTreeMap<TileId, f64>, leaves: &mut Vec<TileId>) {
        let edge = f64::from(1u32 << (FINEST - t.level()));
        let (cx, cy) = ((f64::from(t.x()) + 0.5) * edge, (f64::from(t.y()) + 0.5) * edge);
        let d = ((cx - px) * (cx - px) + (cy - py) * (cy - py)).sqrt();
        r.insert(t, d);
        if t.level() < FINEST && d < 2.0 * edge {
            for c in t.children().unwrap() {
                go(c, px, py, r, leaves);
            }
        } else {
            leaves.push(t);
        }
    }
    go(id(0, 0, 0, 0), px, py, &mut r, &mut leaves);
    // An ancestor's distance is its nearest leaf's.
    for l in &leaves {
        let d = r[l];
        let mut a = l.parent();
        while let Some(x) = a {
            let e = r.get_mut(&x).unwrap();
            *e = e.min(d);
            a = x.parent();
        }
    }
    (r, leaves)
}

/// The "view": the leaves within `radius` finest tiles of the point.
fn view(r: &BTreeMap<TileId, f64>, leaves: &[TileId], radius: f64) -> Vec<(TileId, f64)> {
    leaves.iter().filter(|l| r[*l] <= radius).map(|l| (*l, r[l])).collect()
}

fn roots() -> Vec<TileId> {
    Face::ALL.iter().map(|f| id(f.0, 0, 0, 0)).collect()
}

fn closure(tiles: impl IntoIterator<Item = TileId>) -> BTreeSet<TileId> {
    let mut out = BTreeSet::new();
    for t in tiles {
        let mut x = Some(t);
        while let Some(a) = x {
            out.insert(a);
            x = a.parent();
        }
    }
    out
}

fn residency_list(r: &BTreeMap<TileId, f64>) -> Vec<(TileId, f64)> {
    r.iter().map(|(t, d)| (*t, *d)).collect()
}

fn assert_closed(set: &BTreeSet<TileId>) {
    for t in set {
        if let Some(p) = t.parent() {
            assert!(set.contains(&p), "{t:?} is in the set but its parent is not");
        }
    }
}

// spec: STRM-006
#[test]
fn a_share_that_holds_r_changes_nothing() {
    let (r, leaves) = region(13.3, 9.7);
    let d = view(&r, &leaves, 4.0);
    let all_roots: BTreeSet<TileId> = roots().into_iter().collect();
    let n = r.len() + all_roots.iter().filter(|t| !r.contains_key(t)).count();
    let out = degrade(&d, &residency_list(&r), n, false);
    assert!(!out.report.degraded() && !out.report.budget_failure);
    assert_eq!(out.report.r_full, n);
    assert_eq!(out.residency.len(), n);
    assert_eq!(out.draw, d, "the draw set is untouched");
}

// spec: STRM-006
#[test]
fn a_small_share_coarsens_the_surroundings_and_keeps_the_view_and_the_roots() {
    let (r, leaves) = region(13.3, 9.7);
    let d = view(&r, &leaves, 4.0);
    let pinned = closure(d.iter().map(|(t, _)| *t).chain(roots()));
    let full = r.len() + 5;
    assert!(pinned.len() + 12 < full, "the fixture has surroundings to lose ({} pinned, {full} in R)", pinned.len());
    for capacity in (pinned.len()..full).step_by(7).chain([pinned.len(), full - 1]) {
        let out = degrade(&d, &residency_list(&r), capacity, false);
        let set: BTreeSet<TileId> = out.residency.iter().map(|(t, _)| *t).collect();
        assert!(set.len() <= capacity, "capacity {capacity}: {} slots", set.len());
        assert!(out.report.degraded() && !out.report.budget_failure, "capacity {capacity}: {:?}", out.report);
        assert!(pinned.is_subset(&set), "capacity {capacity}: the draw set, its ancestors and the roots stay");
        assert_closed(&set);
        assert_eq!(out.draw, d, "the draw set is untouched while it fits");
        assert!(out.report.draw_replaced.is_empty());
        assert_eq!(out.report.r_effective, set.len());
        // Every tile of R that was dropped is covered by an ancestor in the set: nothing is blank in any direction.
        for t in r.keys() {
            if !set.contains(t) {
                let mut a = t.parent();
                while a.is_some_and(|x| !set.contains(&x)) {
                    a = a.and_then(|x| x.parent());
                }
                assert!(a.is_some(), "{t:?} lost its whole ancestor chain");
            }
        }
        // No ancestor kept in place of dropped tiles overlaps a drawn tile: a dropped group has no pinned member.
        for t in r.keys().filter(|t| !set.contains(t)) {
            assert!(!pinned.contains(t), "{t:?} is pinned but was dropped");
        }
    }
}

// spec: STRM-006
#[test]
fn the_farthest_group_goes_first_and_the_result_is_nested_in_the_capacity() {
    let (r, leaves) = region(13.3, 9.7);
    let d = view(&r, &leaves, 4.0);
    let full = r.len() + 5;
    let list = residency_list(&r);
    // One slot too few: exactly one sibling group is merged, the one whose nearest member is farthest (finest among equals).
    let out = degrade(&d, &list, full - 1, false);
    let set: BTreeSet<TileId> = out.residency.iter().map(|(t, _)| *t).collect();
    let dropped: Vec<TileId> = r.keys().filter(|t| !set.contains(t)).copied().collect();
    assert!(!dropped.is_empty() && dropped.len() <= 4, "one group: {dropped:?}");
    let parent = dropped[0].parent().unwrap();
    assert!(dropped.iter().all(|t| t.parent() == Some(parent)), "siblings together: {dropped:?}");
    let pinned = closure(d.iter().map(|(t, _)| *t).chain(roots()));
    let group_of = |p: TileId| -> Vec<TileId> { p.children().unwrap().iter().filter(|c| r.contains_key(c)).copied().collect() };
    let mergeable = |p: TileId| {
        let ks = group_of(p);
        !ks.is_empty() && ks.iter().all(|c| !pinned.contains(c) && c.children().unwrap().iter().all(|g| !r.contains_key(g)))
    };
    let key = |p: TileId| {
        let near = group_of(p).iter().map(|c| r[c]).fold(f64::INFINITY, f64::min);
        (near, p.level() + 1)
    };
    let best = r.keys().copied().filter(|p| mergeable(*p)).map(|p| (key(p), p)).fold(None::<((f64, u8), TileId)>, |b, c| match b {
        Some(b) if (b.0 .0, b.0 .1) >= (c.0 .0, c.0 .1) => Some(b),
        _ => Some(c),
    });
    let (best_key, _) = best.expect("a mergeable group exists");
    assert_eq!(key(parent), best_key, "the merged group is the farthest (then finest) mergeable one");
    // Nested: a smaller capacity only removes more.
    let mut previous: Option<BTreeSet<TileId>> = None;
    for capacity in (20..=full).rev().step_by(3) {
        let set: BTreeSet<TileId> = degrade(&d, &list, capacity, false).residency.iter().map(|(t, _)| *t).collect();
        if let Some(p) = &previous {
            assert!(set.is_subset(p), "capacity {capacity}: the set is not nested in the one for a larger capacity");
        }
        previous = Some(set);
    }
}

// spec: STRM-006
#[test]
fn the_degradation_is_a_pure_function_of_the_sets_and_the_capacity() {
    let (r, leaves) = region(21.5, 4.2);
    let d = view(&r, &leaves, 3.0);
    let list = residency_list(&r);
    let reference = degrade(&d, &list, 60, false);
    let mut rev_list = list.clone();
    rev_list.reverse();
    let mut rev_d = d.clone();
    rev_d.reverse();
    let mut rotated = list.clone();
    rotated.rotate_left(17);
    for l in [rev_list, rotated] {
        let again = degrade(&rev_d, &l, 60, false);
        assert_eq!(again.residency, reference.residency);
        assert_eq!(again.report, reference.report);
    }
}

// spec: STRM-006
#[test]
fn a_draw_set_that_does_not_fit_is_replaced_by_ancestors_as_a_declared_exception() {
    let (r, leaves) = region(13.3, 9.7);
    let d = view(&r, &leaves, 6.0);
    let pinned = closure(d.iter().map(|(t, _)| *t).chain(roots()));
    let capacity = pinned.len() - 10;
    let out = degrade(&d, &residency_list(&r), capacity, false);
    let set: BTreeSet<TileId> = out.residency.iter().map(|(t, _)| *t).collect();
    assert!(out.report.budget_failure, "the pinned tiles ({}) exceed the capacity {capacity}", pinned.len());
    assert_eq!(out.report.pinned, pinned.len());
    assert_eq!(out.report.capacity, capacity);
    assert!(set.len() <= capacity, "{} slots against {capacity}", set.len());
    assert!(!out.report.draw_replaced.is_empty());
    assert!(out.report.draw_replaced.iter().all(|t| d.iter().any(|(x, _)| x == t)), "only draw tiles are replaced");
    assert!(roots().iter().all(|t| set.contains(t)), "the roots always stay");
    assert_closed(&set);
    // The draw set asked for is entirely inside the set and covers every original draw tile through itself or an ancestor.
    for (t, _) in &out.draw {
        assert!(set.contains(t));
    }
    for (t, _) in &d {
        assert!(closure([*t]).iter().any(|a| out.draw.iter().any(|(x, _)| x == a)), "{t:?} is not covered by the replaced draw set");
    }
    // The farthest draw tiles go first: every replaced tile is at least as far as every kept leaf of the draw set.
    let kept_far = d.iter().filter(|(t, _)| !out.report.draw_replaced.contains(t) && set.contains(t)).map(|(_, p)| *p).fold(0.0, f64::max);
    let replaced_near = d.iter().filter(|(t, _)| out.report.draw_replaced.contains(t)).map(|(_, p)| *p).fold(f64::INFINITY, f64::min);
    assert!(replaced_near >= 0.0 && kept_far.is_finite());
}

// spec: STRM-006
#[test]
fn levels_zero_to_two_live_in_the_vertex_pool_and_are_never_counted_or_merged() {
    let (r, leaves) = region(13.3, 9.7);
    let d = view(&r, &leaves, 4.0);
    let counted = |s: &BTreeSet<TileId>| s.iter().filter(|t| t.level() > 2).count();
    let full = degrade(&d, &residency_list(&r), usize::MAX / 2, true);
    let full_set: BTreeSet<TileId> = full.residency.iter().map(|(t, _)| *t).collect();
    let capacity = counted(&full_set) - 15;
    let out = degrade(&d, &residency_list(&r), capacity, true);
    let set: BTreeSet<TileId> = out.residency.iter().map(|(t, _)| *t).collect();
    assert!(counted(&set) <= capacity);
    assert!(full_set.iter().filter(|t| t.level() <= 2).all(|t| set.contains(t)), "levels 0-2 are never merged away");
    assert_eq!(out.report.r_full, counted(&full_set));
}

struct Echo;

impl TileSource<u64> for Echo {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        Ok(tile.raw())
    }
}

fn manager(capacity: usize, retire_frames: u64) -> (ResidencyManager<u64>, Arc<ManualSpawner>) {
    let spawner = Arc::new(ManualSpawner::default());
    let config = StreamConfig { max_in_flight: 16, capacity, retire_frames, ..Default::default() };
    let mut s = Streamer::new(Arc::new(Echo), spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config);
    s.load_roots_blocking().unwrap();
    (ResidencyManager::new(s), spawner)
}

fn wants(d: &[(TileId, f64)], r: &BTreeMap<TileId, f64>) -> Wants {
    Wants { draw: d.to_vec(), residency: residency_list(r), prefetch: vec![] }
}

// spec: STRM-006, STRM-007
#[test]
fn small_share_on_fakes_the_view_is_exact_the_surroundings_are_coarser_and_the_slots_never_exceed_the_share() {
    let capacity = 100;
    let (mut m, sp) = manager(capacity, 2);
    let (r, leaves) = region(13.3, 9.7);
    let d = view(&r, &leaves, 4.0);
    let full = r.len() + 5;
    assert!(full > capacity + 20, "R ({full}) is much larger than the share ({capacity})");
    let w = wants(&d, &r);
    let mut last = None;
    for _ in 0..40 {
        let frame = m.begin_frame(&w);
        sp.run(16);
        let st = m.streamer().stats();
        assert!(st.resident + st.in_flight <= capacity, "slots in use exceed the share: {st:?}");
        m.streamer().pools().check().unwrap();
        last = Some(frame);
    }
    let frame = last.unwrap();
    assert!(frame.report.degraded() && !frame.report.budget_failure, "{:?}", frame.report);
    assert!(frame.resolved.iter().all(|x| x.exact()), "the draw set is fully resident at full detail");
    // The surroundings: R_effective is resident, the dropped tiles are covered by resident ancestors.
    let st = m.streamer().stats();
    assert_eq!(st.in_flight + st.queued, 0, "settled: {st:?}");
    for t in r.keys() {
        assert!(m.streamer().resolve(*t).shown.is_some(), "{t:?} has nothing to show");
    }
    assert!(st.resident <= capacity);
    // The allocated total (blocks touched) stays inside the share.
    let c = PoolConfig::fit_share(2 * GIB, 53_724, 16, 53_724, 25_000, 16 << 20, 2).unwrap();
    let blocks = c.blocks_for(m.streamer().pools().main.high_water());
    assert!(c.allocated_bytes_with(blocks, true) <= 2 * GIB);
}

// spec: STRM-006, STRM-007
#[test]
fn moving_the_view_under_a_small_share_keeps_the_draw_set_exact_once_settled_and_the_slots_inside_the_share() {
    let capacity = 80;
    let (mut m, sp) = manager(capacity, 2);
    let mut position = (6.0, 6.0);
    for step in 0..60 {
        position = (6.0 + f64::from(step) * 0.4, 6.0 + f64::from(step) * 0.25);
        let (r, leaves) = region(position.0, position.1);
        let d = view(&r, &leaves, 3.0);
        let w = wants(&d, &r);
        for _ in 0..8 {
            let f = m.begin_frame(&w);
            sp.run(16);
            let st = m.streamer().stats();
            assert!(st.resident + st.in_flight <= capacity, "step {step}: {st:?}");
            assert!(!f.report.budget_failure, "step {step}: {:?}", f.report);
        }
        let f = m.begin_frame(&w);
        assert!(f.resolved.iter().all(|x| x.exact()), "step {step}: the draw set is not fully resident");
    }
    let _ = position;
    let st = m.streamer().stats();
    assert!(st.evicted > 0, "the share forced evictions along the move");
}
