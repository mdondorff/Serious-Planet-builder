//! The memo of the edit path's caches with a budget (GEN-153, A3 section 6.6): entries keyed by their inputs, each with the tick it was last used, so that eviction takes the least recently used
//! first and never one the active solve has touched. Eviction removes recomputable derived data only (a miss recomputes the entry), so it can never change a result. The tick is a counter, not
//! a clock: the order of eviction is the same on every run and every thread count (CON-14).

use std::collections::HashMap;
use std::sync::Arc;

/// The key of an entry: the two words of the inputs' hash.
pub type Key = (u64, u64);

/// The bytes one entry holds besides its values: the map slot, the `Arc` and the vector header, about.
pub const ENTRY_OVERHEAD: usize = 64;

struct Slot {
    v: Arc<Vec<f64>>,
    used: u64,
}

/// A memo of `f64` vectors by key with the tick of each entry's last use.
#[derive(Default)]
pub struct Memo {
    map: HashMap<Key, Slot>,
    tick: u64,
    bytes: usize,
}

fn size_of(v: &[f64]) -> usize {
    v.len() * 8 + ENTRY_OVERHEAD
}

impl Memo {
    /// The entry under `k`, marked used now.
    pub fn get(&mut self, k: &Key) -> Option<Arc<Vec<f64>>> {
        self.tick += 1;
        let t = self.tick;
        self.map.get_mut(k).map(|s| {
            s.used = t;
            Arc::clone(&s.v)
        })
    }

    /// The entry under `k` without marking it (for readers that hold only `&self`; mark it with [`Memo::touch`] afterwards).
    pub fn peek(&self, k: &Key) -> Option<&Arc<Vec<f64>>> {
        self.map.get(k).map(|s| &s.v)
    }

    /// Marks the entry under `k` as used now.
    pub fn touch(&mut self, k: &Key) {
        self.tick += 1;
        let t = self.tick;
        if let Some(s) = self.map.get_mut(k) {
            s.used = t;
        }
    }

    /// Stores `v` under `k`, used now (a replaced entry's bytes are released).
    pub fn insert(&mut self, k: Key, v: Arc<Vec<f64>>) {
        self.tick += 1;
        self.bytes += size_of(&v);
        if let Some(old) = self.map.insert(k, Slot { v, used: self.tick }) {
            self.bytes -= size_of(&old.v);
        }
    }

    /// The tick of now: entries used from it on are the ones a solve that starts here touches.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// The bytes held (values and the per-entry overhead).
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Evicts, least recently used first (ties by key), the entries last used before `pin_from`, until `bytes() <= budget` or only entries used from `pin_from` on are left. Returns the entries
    /// and bytes evicted.
    pub fn evict(&mut self, budget: usize, pin_from: u64) -> (usize, usize) {
        if self.bytes <= budget {
            return (0, 0);
        }
        let mut order: Vec<(u64, Key)> = self.map.iter().filter(|(_, s)| s.used < pin_from).map(|(k, s)| (s.used, *k)).collect();
        order.sort_unstable();
        let (mut entries, mut bytes) = (0, 0);
        for (_, k) in order {
            if self.bytes <= budget {
                break;
            }
            if let Some(s) = self.map.remove(&k) {
                let b = size_of(&s.v);
                self.bytes -= b;
                entries += 1;
                bytes += b;
            }
        }
        (entries, bytes)
    }

    /// Drops every entry.
    pub fn clear(&mut self) {
        self.map.clear();
        self.bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(n: usize) -> Arc<Vec<f64>> {
        Arc::new(vec![1.0; n])
    }

    // spec: GEN-153
    #[test]
    fn eviction_takes_the_least_recently_used_first_and_ties_by_key_and_never_a_pinned_entry() {
        let mut m = Memo::default();
        for k in 0..5u64 {
            m.insert((k, 0), v(10));
        }
        // Use entry 1 again: it is now the newest of the five.
        assert!(m.get(&(1, 0)).is_some());
        let pin = m.tick() + 1;
        // The active solve touches entry 4 and inserts entry 9: both are pinned.
        m.touch(&(4, 0));
        m.insert((9, 0), v(10));
        let each = size_of(&v(10));
        let (n, b) = m.evict(2 * each, pin);
        // Six entries are held and the budget keeps two: the four unpinned ones (0, 2, 3, then 1, which was used after them) go in that order; the pinned 4 and 9 stay.
        assert_eq!((n, b), (4, 4 * each));
        assert!(m.peek(&(4, 0)).is_some() && m.peek(&(9, 0)).is_some() && m.len() == 2 && m.bytes() == 2 * each);
        // The order: with a budget of 3 entries out of 4 candidates, the oldest of them goes.
        let mut m = Memo::default();
        for k in [3u64, 1, 2, 0] {
            m.insert((k, 0), v(10));
        }
        m.get(&(3, 0));
        let (n, _) = m.evict(3 * each, m.tick() + 1);
        assert_eq!(n, 1);
        assert!(m.peek(&(1, 0)).is_none(), "entry 1 was inserted second and not used again: the least recently used");
        assert!(m.peek(&(3, 0)).is_some() && m.peek(&(2, 0)).is_some() && m.peek(&(0, 0)).is_some());
    }

    // spec: GEN-153
    #[test]
    fn the_bytes_follow_inserts_replacements_and_evictions_and_a_budget_over_the_bytes_evicts_nothing() {
        let mut m = Memo::default();
        m.insert((1, 1), v(8));
        m.insert((2, 2), v(16));
        assert_eq!(m.bytes(), size_of(&v(8)) + size_of(&v(16)));
        m.insert((1, 1), v(4));
        assert_eq!(m.bytes(), size_of(&v(4)) + size_of(&v(16)), "a replaced entry's bytes are released");
        assert_eq!(m.evict(usize::MAX, u64::MAX), (0, 0));
        let all = m.evict(0, u64::MAX);
        assert_eq!((all.0, m.bytes(), m.len()), (2, 0, 0));
        m.insert((3, 3), v(2));
        m.clear();
        assert!(m.is_empty() && m.bytes() == 0);
    }
}
