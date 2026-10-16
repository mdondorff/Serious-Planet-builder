//! The scheduler's persistent sets (STRM-019): one [`Layer`] per request class holds the tiles the caller listed with their
//! priorities and, with them, the tiles' ancestors, as counts that follow additions and removals. A frame's work then follows
//! what changed in the wants, not their size.
//!
//! A tile is *in a layer* when it is listed or an ancestor of a listed tile. The count of a tile is 1 for being listed plus 1
//! for each child that is in the layer, so adding or removing a listed tile walks up only while membership changes, and the
//! walk reports exactly the tiles whose membership flipped. Priority keys (the order of the queue) are derived lazily from
//! the listed priorities and only when a request needs one.

use crate::{priority_key, FxMap};
use planet_core::TileId;
use std::borrow::Cow;

/// Cheap parent of a tile: the same tile as [`TileId::parent`] by bit arithmetic on the layout (face, level, Morton path)
/// instead of decoding and re-encoding the indices. Equality with `TileId::parent` is tested.
#[inline]
pub(crate) fn parent_of(t: TileId) -> Option<TileId> {
    let raw = t.raw();
    let level = (raw >> 56) & 31;
    if level == 0 {
        return None;
    }
    let face = raw & (7u64 << 61);
    let path = raw & ((1u64 << 56) - 1);
    Some(TileId::from_raw(face | ((level - 1) << 56) | (path >> 2)).expect("the parent of a tile is a tile"))
}

/// What the scheduler's frame touched (CON-20: counts, not time). Cumulative; a test takes the difference of two readings.
/// `scanned` counts elements read in sequential passes (the comparison of a new wants list with the held one, completions
/// drained, the queue's re-keying); `updated` counts elements written (set and map insertions and removals, queue and
/// eviction-candidate changes, key computations, evictions, dispatches).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SchedWork {
    pub scanned: u64,
    pub updated: u64,
    /// Of `updated`, the membership writes of each request class's layer: draw, residency, prefetch.
    pub layer_updates: [u64; 3],
}

impl SchedWork {
    pub fn total(&self) -> u64 {
        self.scanned + self.updated
    }

    /// What happened since `earlier`.
    pub fn since(&self, earlier: &SchedWork) -> SchedWork {
        SchedWork {
            scanned: self.scanned - earlier.scanned,
            updated: self.updated - earlier.updated,
            layer_updates: [
                self.layer_updates[0] - earlier.layer_updates[0],
                self.layer_updates[1] - earlier.layer_updates[1],
                self.layer_updates[2] - earlier.layer_updates[2],
            ],
        }
    }
}

/// Sorted by tile, unique (a duplicate keeps its best priority): the form a layer holds. A list that already is, is borrowed.
pub(crate) fn normalized(input: &[(TileId, f64)]) -> Cow<'_, [(TileId, f64)]> {
    if input.windows(2).all(|w| w[0].0 < w[1].0) {
        return Cow::Borrowed(input);
    }
    let mut v = input.to_vec();
    v.sort_by(|a, b| a.0.cmp(&b.0).then(priority_key(a.1).cmp(&priority_key(b.1))));
    v.dedup_by_key(|e| e.0);
    Cow::Owned(v)
}

/// The tiles whose membership in a layer changed, with whether they were members before.
#[derive(Default)]
pub(crate) struct LayerDiff {
    pub flipped: Vec<(TileId, bool)>,
    /// Anything changed: a tile listed or unlisted, or a priority.
    pub changed: bool,
}

pub(crate) struct Layer {
    /// Listed tiles with priorities, ascending by tile.
    pub items: Vec<(TileId, f64)>,
    count: FxMap<TileId, u32>,
    /// Best priority key over the listed tiles at or below each member (the draw layer adds a depth term, see `build_keys`),
    /// built when a request needs a key and dropped when the layer changes.
    keys: Option<FxMap<TileId, u64>>,
    /// Every member is listed and no tile has a better priority than the tiles above it (a residency set made from a
    /// selection): a tile's key is its own priority and no map of keys is built.
    direct: bool,
    /// The draw layer orders ancestors before descendants through the priority: a tile `d` levels above a listed one is
    /// `d * 1e12` ahead of it.
    depth_term: bool,
}

impl Layer {
    pub fn new(depth_term: bool) -> Layer {
        Layer { items: Vec::new(), count: FxMap::default(), keys: None, direct: !depth_term, depth_term }
    }

    #[inline]
    pub fn has(&self, t: TileId) -> bool {
        self.count.contains_key(&t)
    }

    /// Members, unordered: for a rescan after the queue was cleared.
    pub fn member_tiles(&self) -> impl Iterator<Item = TileId> + '_ {
        self.count.keys().copied()
    }

    /// Replace the listed tiles by `new` (sorted, unique) and report what changed. Compares the two lists in one merge pass
    /// and touches the counts of the tiles that differ only.
    pub fn replace(&mut self, new: Vec<(TileId, f64)>, work: &mut SchedWork, class: usize) -> LayerDiff {
        work.scanned += self.items.len().max(new.len()) as u64;
        let (mut removed, mut added) = (Vec::new(), Vec::new());
        let mut prio_changed = false;
        let (old, mut i, mut j) = (&self.items, 0, 0);
        while i < old.len() || j < new.len() {
            match (old.get(i), new.get(j)) {
                (Some(o), Some(n)) => match o.0.cmp(&n.0) {
                    std::cmp::Ordering::Less => {
                        removed.push(o.0);
                        i += 1;
                    }
                    std::cmp::Ordering::Greater => {
                        added.push(n.0);
                        j += 1;
                    }
                    std::cmp::Ordering::Equal => {
                        prio_changed |= o.1.to_bits() != n.1.to_bits();
                        i += 1;
                        j += 1;
                    }
                },
                (Some(o), None) => {
                    removed.push(o.0);
                    i += 1;
                }
                (None, Some(n)) => {
                    added.push(n.0);
                    j += 1;
                }
                (None, None) => break,
            }
        }
        if removed.is_empty() && added.is_empty() && !prio_changed {
            return LayerDiff::default();
        }
        self.items = new;
        self.keys = None;
        self.direct = !self.depth_term && parents_listed(&self.items, true);
        // Removals first, then additions: a tile that leaves and enters again in one change flips twice and nets to nothing.
        let mut was: FxMap<TileId, bool> = FxMap::default();
        for t in removed {
            self.remove(t, &mut was, work, class);
        }
        for t in added {
            self.add(t, &mut was, work, class);
        }
        let flipped = was.into_iter().filter(|&(t, w)| self.has(t) != w).collect();
        LayerDiff { flipped, changed: true }
    }

    fn add(&mut self, t: TileId, was: &mut FxMap<TileId, bool>, work: &mut SchedWork, class: usize) {
        let mut cur = t;
        loop {
            let c = self.count.entry(cur).or_insert(0);
            let member = *c > 0;
            *c += 1;
            work.updated += 1;
            work.layer_updates[class] += 1;
            if member {
                break;
            }
            was.entry(cur).or_insert(false);
            match parent_of(cur) {
                Some(p) => cur = p,
                None => break,
            }
        }
    }

    fn remove(&mut self, t: TileId, was: &mut FxMap<TileId, bool>, work: &mut SchedWork, class: usize) {
        let mut cur = t;
        loop {
            let Some(c) = self.count.get_mut(&cur) else { break };
            *c -= 1;
            work.updated += 1;
            work.layer_updates[class] += 1;
            if *c > 0 {
                break;
            }
            self.count.remove(&cur);
            was.entry(cur).or_insert(true);
            match parent_of(cur) {
                Some(p) => cur = p,
                None => break,
            }
        }
    }

    /// The priority key of member `t`: the best key among the listed tiles at or below it (built on first use).
    pub fn key(&mut self, t: TileId, work: &mut SchedWork) -> u64 {
        if self.direct {
            let i = self.items.binary_search_by_key(&t, |e| e.0).expect("a member of a closed layer is listed");
            return priority_key(self.items[i].1);
        }
        if self.keys.is_none() {
            self.keys = Some(self.build_keys(work));
        }
        *self.keys.as_ref().and_then(|k| k.get(&t)).expect("a request's tile is a member of its class's layer")
    }

    fn build_keys(&self, work: &mut SchedWork) -> FxMap<TileId, u64> {
        use std::collections::hash_map::Entry;
        let mut keys: FxMap<TileId, u64> = FxMap::default();
        if self.depth_term {
            // The priority minus `depth * 1e12`, depth counted in steps from the listed tile, the best key of any listed
            // tile below: exactly the arithmetic of the per-frame rebuild this replaces. Finest tiles first (the list ascends
            // by level within a face): their chains set the best keys of the ancestors, and a shallower tile whose chain
            // meets an ancestor with a strictly better key stops there (a tile `d` levels above its source has the key
            // `p - d * 1e12`, which orders by depth first; rounding keeps the order of strictly different keys).
            // The stop is taken only for priorities far below the depth step (and finite); anything else walks every chain.
            let ordered = self.items.iter().all(|&(_, p)| p.is_finite() && p.abs() < 1.0e11);
            for &(tile, priority) in self.items.iter().rev() {
                let (mut t, mut depth) = (Some(tile), 0.0f64);
                while let Some(x) = t {
                    let k = priority_key(priority - depth * 1.0e12);
                    work.updated += 1;
                    match keys.entry(x) {
                        Entry::Occupied(mut e) => {
                            if ordered && *e.get() < k {
                                break;
                            }
                            e.insert(k.min(*e.get()));
                        }
                        Entry::Vacant(e) => {
                            e.insert(k);
                        }
                    }
                    depth += 1.0;
                    t = parent_of(x);
                }
            }
        } else {
            // One priority for the whole chain above a tile: stop at an ancestor that already has a key at least as good,
            // its own ancestors have one too. Coarse tiles first, so that a listed tile finds its parent done.
            for &(tile, priority) in &self.items {
                let k = priority_key(priority);
                let mut t = Some(tile);
                while let Some(x) = t {
                    work.updated += 1;
                    match keys.entry(x) {
                        Entry::Occupied(mut e) => {
                            if *e.get() <= k {
                                break;
                            }
                            e.insert(k);
                        }
                        Entry::Vacant(e) => {
                            e.insert(k);
                        }
                    }
                    t = parent_of(x);
                }
            }
        }
        keys
    }
}

/// Whether every listed tile's parent is listed too (`items` sorted ascending by tile), and, with `monotone`, no parent has a
/// worse priority than a child: one pass with a cursor per level, no hashing. A residency set built from a selection is both;
/// a hand-made list may not be.
pub(crate) fn parents_listed(items: &[(TileId, f64)], monotone: bool) -> bool {
    let mut cursor = [usize::MAX; 32];
    let mut face = u64::MAX;
    let mut face_start = 0usize;
    for (i, &(t, p)) in items.iter().enumerate() {
        let f = t.raw() >> 61;
        if f != face {
            face = f;
            face_start = i;
            cursor = [usize::MAX; 32];
        }
        let level = usize::from(t.level());
        let Some(parent) = parent_of(t) else { continue };
        if cursor[level] == usize::MAX {
            cursor[level] = face_start + items[face_start..].partition_point(|e| e.0 < parent);
        }
        let mut j = cursor[level];
        while j < items.len() && items[j].0 < parent {
            j += 1;
        }
        cursor[level] = j;
        if j >= items.len() || items[j].0 != parent || (monotone && priority_key(items[j].1) > priority_key(p)) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_core::Face;
    use std::collections::{BTreeMap, BTreeSet};

    struct Rng(u64);

    impl Rng {
        fn next(&mut self, n: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % n
        }
    }

    fn random_tile(r: &mut Rng, max_level: u64) -> TileId {
        let level = r.next(max_level + 1) as u8;
        let n = 1u64 << level;
        TileId::new(Face(r.next(6) as u8), level, r.next(n) as u32, r.next(n) as u32).unwrap()
    }

    // spec: STRM-019, CON-14
    #[test]
    fn the_bit_arithmetic_parent_is_the_tiles_parent() {
        let mut r = Rng(0x1234_5678_9abc_def1);
        for _ in 0..20_000 {
            let t = random_tile(&mut r, 28);
            assert_eq!(parent_of(t), t.parent(), "{t:?}");
        }
        for f in 0..6 {
            let root = TileId::new(Face(f), 0, 0, 0).unwrap();
            assert_eq!(parent_of(root), None);
        }
    }

    /// The ancestors-closed set and the old per-frame key arithmetic, by brute force.
    fn brute(items: &[(TileId, f64)], depth_term: bool) -> (BTreeSet<TileId>, BTreeMap<TileId, u64>) {
        let mut members = BTreeSet::new();
        let mut keys: BTreeMap<TileId, u64> = BTreeMap::new();
        for &(tile, priority) in items {
            let (mut t, mut depth) = (Some(tile), 0.0f64);
            while let Some(x) = t {
                members.insert(x);
                let k = priority_key(if depth_term { priority - depth * 1.0e12 } else { priority });
                let e = keys.entry(x).or_insert(k);
                *e = (*e).min(k);
                depth += 1.0;
                t = x.parent();
            }
        }
        (members, keys)
    }

    fn priorities(r: &mut Rng, kind: u64) -> f64 {
        match kind {
            0 => r.next(50) as f64,
            1 => r.next(1000) as f64 * 0.37,
            2 => r.next(7) as f64 * 1.0e-5 + 3.0,
            3 => [f64::NAN, f64::INFINITY, -1.0e13, 1.0e12, 5.0e11, -0.0, 0.0][r.next(7) as usize],
            _ => r.next(100_000) as f64 * 1.0e6,
        }
    }

    // spec: STRM-019, STRM-005
    #[test]
    fn a_layer_follows_additions_and_removals_with_the_membership_and_the_keys_of_a_rebuild_from_scratch() {
        let mut r = Rng(0xfeed_beef_0bad_cafe);
        for round in 0..300u64 {
            for depth_term in [false, true] {
                let mut layer = Layer::new(depth_term);
                let mut work = SchedWork::default();
                let mut model: BTreeMap<TileId, f64> = BTreeMap::new();
                let kind = round % 5;
                for _step in 0..6 {
                    // Change the list: drop some listed tiles, add some, re-prioritise some.
                    for _ in 0..r.next(12) {
                        if let Some(&t) = model.keys().nth(r.next(model.len() as u64 + 1) as usize) {
                            model.remove(&t);
                        }
                    }
                    for _ in 0..r.next(20) {
                        model.insert(random_tile(&mut r, 5 + round % 4), priorities(&mut r, kind));
                    }
                    if r.next(3) == 0 {
                        for p in model.values_mut() {
                            *p = priorities(&mut r, kind);
                        }
                    }
                    if r.next(3) == 0 {
                        // Closed under parents and monotone (a residency set): the layer keys by the tiles' own priorities.
                        let all: Vec<(TileId, f64)> = model.iter().map(|(&t, &p)| (t, p)).collect();
                        for (t, p) in all {
                            let mut x = t.parent();
                            while let Some(a) = x {
                                let e = model.entry(a).or_insert(p);
                                if priority_key(p) < priority_key(*e) {
                                    *e = p;
                                }
                                x = a.parent();
                            }
                        }
                    }
                    let list: Vec<(TileId, f64)> = model.iter().map(|(&t, &p)| (t, p)).collect();
                    let before: BTreeSet<TileId> = layer.member_tiles().collect();
                    let diff = layer.replace(list.clone(), &mut work, 0);
                    let (members, keys) = brute(&list, depth_term);
                    let now: BTreeSet<TileId> = layer.member_tiles().collect();
                    assert_eq!(now, members, "round {round} depth {depth_term}");
                    let flipped: BTreeSet<TileId> = diff.flipped.iter().map(|e| e.0).collect();
                    assert_eq!(flipped, before.symmetric_difference(&members).copied().collect::<BTreeSet<_>>(), "the tiles that flipped");
                    for (t, was) in &diff.flipped {
                        assert_eq!(*was, before.contains(t));
                    }
                    for &t in &members {
                        assert_eq!(layer.key(t, &mut work), keys[&t], "key of {t:?}, round {round} depth {depth_term} kind {kind}");
                    }
                }
            }
        }
    }

    // spec: STRM-019
    #[test]
    fn parents_listed_agrees_with_the_definition_closed_or_closed_and_monotone() {
        let mut r = Rng(42);
        for _ in 0..500 {
            let mut set: BTreeMap<TileId, f64> = BTreeMap::new();
            for _ in 0..r.next(30) {
                set.insert(random_tile(&mut r, 6), r.next(20) as f64);
            }
            if r.next(2) == 0 {
                // Close it under parents, parents never worse than their children.
                let all: Vec<(TileId, f64)> = set.iter().map(|(&t, &p)| (t, p)).collect();
                for (t, p) in all {
                    let mut x = t.parent();
                    while let Some(a) = x {
                        let e = set.entry(a).or_insert(p);
                        *e = e.min(p);
                        x = a.parent();
                    }
                }
            }
            let items: Vec<(TileId, f64)> = set.iter().map(|(&t, &p)| (t, p)).collect();
            let closed = items.iter().all(|(t, _)| t.parent().is_none_or(|p| set.contains_key(&p)));
            let monotone = items.iter().all(|(t, p)| t.parent().is_none_or(|a| set.get(&a).is_none_or(|ap| priority_key(*ap) <= priority_key(*p))));
            assert_eq!(parents_listed(&items, false), closed);
            assert_eq!(parents_listed(&items, true), closed && monotone);
        }
    }
}
