//! The base heights of a chunk region across an edit (GEN-087, M8-7): the heights a chunk job reads for the cells of its region are remembered with
//! the definition they were made under, and a re-solve after an edit reads again only the cells the changed items reach. The rest are the same
//! bits: an item changes no output of any stage beyond its `reach_m` (WDEF-010, GEN-026), and `WorldDefinition::item_changes_from` names the old
//! and the new place of every item that differs (STRM-034). GPU-free.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use planet_core::hash::mix64;
use planet_core::Vec3;
use planet_world_def::footprint::EditReach;
use planet_world_def::{ChangedItem, WorldDefinition};

/// The batches kept at most (one per chunk region): a view wants 4 to 9 chunks and the edit's ring a few more.
pub const MAX_BATCHES: usize = 24;
/// The heights kept at most, in bytes (8 per cell): the oldest batch goes first.
pub const MAX_BYTES: usize = 256 << 20;

static REUSED: AtomicU64 = AtomicU64::new(0);
static ASKED: AtomicU64 = AtomicU64::new(0);

/// The cells answered from a remembered batch and the cells asked of a batch that had one to reuse, since the last call (the `--edit-resolve` report).
pub fn reuse_stats() -> (u64, u64) {
    (REUSED.swap(0, Ordering::Relaxed), ASKED.swap(0, Ordering::Relaxed))
}

pub(crate) fn count(reused: usize, asked: usize) {
    REUSED.fetch_add(reused as u64, Ordering::Relaxed);
    ASKED.fetch_add(asked as u64, Ordering::Relaxed);
}

/// The identity of a batch of directions: two 64-bit hashes over the bits of every direction and the length (a batch is one chunk's region, always the
/// same cells in the same order).
pub fn batch_key(dirs: &[Vec3]) -> (u64, u64) {
    let (mut a, mut b) = (mix64(dirs.len() as u64 ^ 0x0072_6567_696f), mix64(dirs.len() as u64 ^ 0x0068_6569_6768));
    for d in dirs {
        for v in [d.x, d.y, d.z] {
            let bits = v.to_bits();
            a = mix64(a ^ bits);
            b = mix64(b.rotate_left(17) ^ bits);
        }
    }
    (a, b)
}

/// The indices of the directions that an item change can reach: the ones within `reach_m` of the place of any change (widened as every reach test is,
/// `EditReach::within_angle`, so a direction exactly at the edge counts) and, for a brush, within the radius of one of its strokes (WDEF-007: the
/// strokes bound what the brush changes inside its reach disc, and a stroke changes no bit of a point beyond its radius, GEN-030). Everything else is
/// unchanged by the edit.
pub fn stale_points(dirs: &[Vec3], changes: &[ChangedItem], radius_m: f64) -> Vec<usize> {
    use planet_world_def::footprint::{strokes_touch, Cap};
    dirs.iter()
        .enumerate()
        .filter(|(_, d)| {
            changes.iter().any(|c| {
                let reach = EditReach { dir: c.dir, reach_m: c.reach_m, strokes: None };
                reach.within_angle(**d, 0.0, radius_m)
                    && (c.strokes.is_empty() || strokes_touch(Cap { centre: **d, angle: 0.0 }, &c.strokes, radius_m))
            })
        })
        .map(|(i, _)| i)
        .collect()
}

struct Entry {
    key: (u64, u64),
    def: Arc<WorldDefinition>,
    heights: Arc<Vec<f64>>,
}

/// The remembered batches, the least recently used first. At most [`MAX_BATCHES`] and [`MAX_BYTES`] of heights.
#[derive(Default)]
pub struct RegionCache {
    entries: Mutex<VecDeque<Entry>>,
}

impl RegionCache {
    /// The definition and the heights of the batch `key`, if kept; it becomes the most recently used.
    pub fn get(&self, key: (u64, u64)) -> Option<(Arc<WorldDefinition>, Arc<Vec<f64>>)> {
        let mut q = self.entries.lock().expect("region cache");
        let at = q.iter().position(|e| e.key == key)?;
        let e = q.remove(at).expect("found");
        let out = (e.def.clone(), e.heights.clone());
        q.push_back(e);
        Some(out)
    }

    /// Keep `heights` of the batch `key`, made under `def`; the batch of the same key is replaced, the oldest go when over the bounds.
    pub fn put(&self, key: (u64, u64), def: Arc<WorldDefinition>, heights: Arc<Vec<f64>>) {
        let mut q = self.entries.lock().expect("region cache");
        q.retain(|e| e.key != key);
        q.push_back(Entry { key, def, heights });
        let bytes = |q: &VecDeque<Entry>| q.iter().map(|e| e.heights.len() * 8).sum::<usize>();
        while q.len() > MAX_BATCHES || (q.len() > 1 && bytes(&q) > MAX_BYTES) {
            q.pop_front();
        }
    }

    pub fn len(&self) -> usize {
        self.entries.lock().expect("region cache").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: f64 = 6_371_000.0;

    fn changed(dir: Vec3, reach_m: f64) -> ChangedItem {
        ChangedItem { dir, reach_m, strokes: Vec::new() }
    }

    fn at(angle: f64) -> Vec3 {
        Vec3::new(libm::cos(angle), libm::sin(angle), 0.0)
    }

    // spec: GEN-087
    #[test]
    fn a_direction_exactly_at_the_reach_is_stale_one_clearly_beyond_it_is_not() {
        let item = Vec3::new(1.0, 0.0, 0.0);
        let reach_m = 3_000.0;
        let edge = reach_m / R;
        let dirs = [at(0.0), at(edge), at(edge * 0.5), at(edge * 1.5), at(-edge), at(edge * 3.0)];
        let stale = stale_points(&dirs, &[changed(item, reach_m)], R);
        assert!(stale.contains(&0) && stale.contains(&2), "inside: {stale:?}");
        assert!(stale.contains(&1) && stale.contains(&4), "exactly at the edge, on both sides: {stale:?}");
        assert!(!stale.contains(&5), "three reaches away: {stale:?}");
    }

    // spec: GEN-087
    #[test]
    fn a_moved_item_makes_both_its_old_and_its_new_place_stale() {
        let reach_m = 40_000.0;
        let (old, new) = (at(0.0), at(0.01));
        let dirs = [at(0.0), at(0.01), at(0.005), at(0.05)];
        let stale = stale_points(&dirs, &[changed(new, reach_m), changed(old, reach_m)], R);
        assert!(stale.contains(&0) && stale.contains(&1) && stale.contains(&2), "{stale:?}");
        assert!(!stale.contains(&3), "{stale:?}");
        assert!(stale_points(&dirs, &[], R).is_empty(), "no change, nothing stale");
    }

    // spec: GEN-087
    #[test]
    fn the_cache_keeps_at_most_its_batches_and_forgets_the_least_recently_used() {
        let cache = RegionCache::default();
        let def = Arc::new(WorldDefinition::single_planet(R, 1));
        for k in 0..MAX_BATCHES as u64 + 5 {
            cache.put((k, k), def.clone(), Arc::new(vec![0.0; 4]));
            if k == 20 {
                assert!(cache.get((5, 5)).is_some(), "a read makes the batch recent");
            }
        }
        assert_eq!(cache.len(), MAX_BATCHES);
        assert!(cache.get((5, 5)).is_some(), "the batch read after it was put survived");
        assert!(cache.get((0, 0)).is_none() && cache.get((1, 1)).is_none(), "the unread old batches went first");
    }

    // spec: GEN-087
    #[test]
    fn a_brush_makes_stale_the_points_near_its_stroke_not_the_whole_disc_that_holds_it() {
        use planet_world_def::stamp::{BrushKind, Stamp, StampKind, Stroke};
        let radius_m = 400.0;
        // A stroke of 9 points 100 m apart, running along the equator; the reach holds the stroke and its radius.
        let step = 100.0 / R;
        let points: Vec<Vec3> = (0..9).map(|k| at(k as f64 * step)).collect();
        let stamp = Stamp::new(StampKind::Brush, [0.0; 3]).with_strokes(&[Stroke {
            kind: BrushKind::Raise,
            radius_m,
            strength_m: 20.0,
            target_m: None,
            points: points.clone(),
        }]);
        let reach_m = 800.0 + radius_m + 2.0;
        let brush = ChangedItem { dir: points[0], reach_m, strokes: stamp.strokes().to_vec() };
        let disc = ChangedItem { dir: points[0], reach_m, strokes: Vec::new() };
        // A grid round the stroke, 50 m apart, 1.6 km either side.
        let dirs: Vec<Vec3> = (-32..=64)
            .flat_map(|i| {
                (-32..=32).map(move |j| {
                    let (along, across) = (f64::from(i) * 50.0 / R, f64::from(j) * 50.0 / R);
                    (at(along) + Vec3::new(0.0, 0.0, 1.0) * across).normalized()
                })
            })
            .collect();
        let by_disc = stale_points(&dirs, &[disc], R);
        let by_stroke = stale_points(&dirs, &[brush], R);
        assert!(
            by_stroke.len() < by_disc.len() / 2,
            "the stroke's capsule is much less than its disc: {} of {}",
            by_stroke.len(),
            by_disc.len()
        );
        assert!(by_stroke.iter().all(|i| by_disc.contains(i)), "the stroke test only narrows the disc");
        // Every point within the stroke's radius of a point of the stroke is stale (a point of the polyline is on it: the capsule holds its discs).
        for (n, d) in dirs.iter().enumerate() {
            let near = points.iter().any(|p| R * p.angle_to(*d) < radius_m * 0.99);
            assert!(!near || by_stroke.contains(&n), "point {n} within the radius of a stroke point is stale");
        }
    }

    // spec: GEN-087
    #[test]
    fn the_batch_key_tells_other_cells_and_other_orders_apart() {
        let a = [at(0.0), at(0.1), at(0.2)];
        let b = [at(0.0), at(0.2), at(0.1)];
        assert_eq!(batch_key(&a), batch_key(&a));
        assert_ne!(batch_key(&a), batch_key(&b));
        assert_ne!(batch_key(&a), batch_key(&a[..2]));
    }
}
