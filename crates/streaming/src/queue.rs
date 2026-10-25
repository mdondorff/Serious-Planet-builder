//! The scheduler's request queue (STRM-005, STRM-019): the order in which waiting requests start, by class, then rank, then
//! priority key, then tile; evaluated lazily.
//!
//! A queued request sits in the bucket of its class and rank (draw: one bucket; residency and prefetch: one per level, coarse
//! first). Within a bucket the order is the priority key, which changes whenever the class's wants change (a camera that moves
//! shifts every distance). Keeping every queued request in key order would cost the whole queue per change, so a bucket is
//! *stale* after a change and is put in order only when something reads it: the dispatcher and the eviction look-ahead read from the
//! front, so a change costs the size of the buckets read, not of the queue.

use crate::{FxMap, QueueKey};
use planet_core::TileId;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound::{Excluded, Unbounded};

#[derive(Default)]
struct Bucket {
    /// Queued tiles with the priority key they have in `order`; `None`: not in `order` yet (queued since the bucket was last
    /// read).
    keys: FxMap<TileId, Option<u64>>,
    order: BTreeSet<(u64, TileId)>,
    /// Tiles queued since the bucket was last read, in order of arrival (a tile may have left again).
    added: Vec<TileId>,
    /// The priority keys changed: every key is computed again when the bucket is read.
    stale: bool,
}

impl Bucket {
    /// Put the bucket in order under the current priority keys: all of it if its keys changed, else only the tiles added.
    fn freshen(&mut self, mut key: impl FnMut(TileId) -> u64, work: &mut u64) {
        if self.stale {
            *work += self.keys.len() as u64;
            let mut v: Vec<(u64, TileId)> = self.keys.keys().map(|&t| (key(t), t)).collect();
            v.sort_unstable();
            for &(k, t) in &v {
                self.keys.insert(t, Some(k));
            }
            self.order = v.into_iter().collect();
            self.added.clear();
            self.stale = false;
        } else if !self.added.is_empty() {
            *work += self.added.len() as u64;
            for t in std::mem::take(&mut self.added) {
                if self.keys.get(&t) == Some(&None) {
                    let k = key(t);
                    self.keys.insert(t, Some(k));
                    self.order.insert((k, t));
                }
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct RequestQueue {
    buckets: BTreeMap<(u8, u64), Bucket>,
    /// Where each queued tile sits: class, rank and the pool kind (for the counts).
    at: FxMap<TileId, (u8, u64, u8)>,
    by_class: [usize; 3],
    by_kind: [usize; 2],
}

impl RequestQueue {
    pub fn len(&self) -> usize {
        self.at.len()
    }

    pub fn by_class(&self) -> [usize; 3] {
        self.by_class
    }

    pub fn by_kind(&self) -> [usize; 2] {
        self.by_kind
    }

    /// Whether a request of `t` is queued.
    pub fn contains(&self, t: TileId) -> bool {
        self.at.contains_key(&t)
    }

    /// Queue `t` in the bucket of `(class, rank)`; a request already queued moves if its bucket differs and stays otherwise.
    /// Returns whether the queue changed.
    pub fn insert(&mut self, t: TileId, class: u8, rank: u64, kind: usize) -> bool {
        if let Some(&(c, r, _)) = self.at.get(&t) {
            if (c, r) == (class, rank) {
                return false;
            }
            self.remove(t);
        }
        let b = self.buckets.entry((class, rank)).or_default();
        b.keys.insert(t, None);
        b.added.push(t);
        self.at.insert(t, (class, rank, kind as u8));
        self.by_class[usize::from(class)] += 1;
        self.by_kind[kind] += 1;
        true
    }

    /// Take `t` out of the queue; the class it was queued in.
    pub fn remove(&mut self, t: TileId) -> Option<u8> {
        let (class, rank, kind) = self.at.remove(&t)?;
        self.by_class[usize::from(class)] -= 1;
        self.by_kind[usize::from(kind)] -= 1;
        let b = self.buckets.get_mut(&(class, rank)).expect("a queued tile has a bucket");
        if let Some(Some(k)) = b.keys.remove(&t) {
            b.order.remove(&(k, t));
        }
        if b.keys.is_empty() {
            self.buckets.remove(&(class, rank));
        }
        Some(class)
    }

    pub fn clear(&mut self) {
        *self = RequestQueue::default();
    }

    /// The priority keys of `class` changed: its buckets are put in order again when next read.
    pub fn mark_stale(&mut self, class: u8) {
        for (_, b) in self.buckets.range_mut((class, 0)..(class + 1, 0)) {
            b.stale = true;
        }
    }

    /// The request after `after` in queue order (the first when `None`). `key(class, tile)` gives the current priority key of
    /// a tile; `work` counts the keys computed.
    pub fn next_after(&mut self, after: Option<QueueKey>, key: &mut dyn FnMut(u8, TileId) -> u64, work: &mut u64) -> Option<QueueKey> {
        let (start, within) = match after {
            None => ((0u8, 0u64), None),
            Some((c, r, p, t)) => ((c, r), Some((p, t))),
        };
        for (&(c, r), b) in self.buckets.range_mut(start..) {
            b.freshen(|t| key(c, t), work);
            let first = match within {
                Some(w) if (c, r) == start => b.order.range((Excluded(w), Unbounded)).next(),
                _ => b.order.iter().next(),
            };
            if let Some(&(p, t)) = first {
                return Some((c, r, p, t));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_core::{Face, TileId};

    fn tile(i: u32) -> TileId {
        TileId::new(Face(0), 6, i % 64, i / 64 % 64).unwrap()
    }

    // spec: STRM-005, STRM-019
    #[test]
    fn the_queue_reads_in_the_order_of_a_sorted_map_through_inserts_removals_key_changes_and_bucket_moves() {
        // The reference is the map the scheduler kept before: every request under its full key, re-keyed on every change.
        let mut q = RequestQueue::default();
        let mut model: BTreeMap<QueueKey, ()> = BTreeMap::new();
        let mut prio: FxMap<TileId, u64> = FxMap::default();
        let mut at: FxMap<TileId, (u8, u64)> = FxMap::default();
        let mut x = 0x9e37_79b9_7f4a_7c15u64;
        let mut rnd = move |n: u64| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x % n
        };
        let mut work = 0u64;
        for step in 0..3000 {
            match rnd(10) {
                0..=4 => {
                    let t = tile(rnd(400) as u32);
                    let (c, r) = (rnd(3) as u8, rnd(4));
                    q.insert(t, c, r, 0);
                    // The bucket may not have changed (the tile stays where it is, under its current priority).
                    at.insert(t, (c, r));
                    prio.entry(t).or_insert_with(|| rnd(50));
                }
                5..=6 => {
                    let t = tile(rnd(400) as u32);
                    assert_eq!(q.remove(t).is_some(), at.remove(&t).is_some());
                }
                7 => {
                    // The priorities of a class change.
                    let c = rnd(3) as u8;
                    for (t, (tc, _)) in at.iter() {
                        if *tc == c {
                            prio.insert(*t, rnd(50));
                        }
                    }
                    q.mark_stale(c);
                }
                _ => {}
            }
            model.clear();
            for (t, (c, r)) in &at {
                model.insert((*c, *r, prio[t], *t), ());
            }
            // Read a few from the front, and a walk to the end now and then.
            let limit = if step % 50 == 0 { usize::MAX } else { 5 };
            let mut cursor = None;
            let mut got = Vec::new();
            while got.len() < limit {
                let Some(k) = q.next_after(cursor, &mut |_, t| prio[&t], &mut work) else { break };
                got.push(k);
                cursor = Some(k);
            }
            let want: Vec<QueueKey> = model.keys().take(limit).copied().collect();
            assert_eq!(got, want, "step {step}");
            assert_eq!(q.len(), at.len());
        }
    }
}
