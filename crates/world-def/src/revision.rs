//! The revision of a definition (STRM-027): a number that differs whenever the definition may have changed and between any two
//! definitions of one process, so a cache that memoises a tile's footprint result by it can neither keep a stale result across an
//! edit nor take one from another definition (a reload builds a fresh definition and so a fresh instance).

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// An instance number (process-unique, fresh for every construction and every clone) and a counter of the changes made to the instance.
/// It takes no part in equality: two definitions with equal content are equal whatever their histories.
#[derive(Debug)]
pub struct Revision {
    instance: u64,
    counter: u64,
}

impl Revision {
    pub fn new() -> Revision {
        Revision { instance: NEXT_INSTANCE.fetch_add(1, Ordering::Relaxed), counter: 0 }
    }

    /// Record a change.
    pub fn bump(&mut self) {
        self.counter += 1;
    }

    /// The revision as one number, never 0 (0 stands for "nothing to track" in the callers).
    pub fn value(&self) -> u64 {
        planet_core::hash::hash_u64s(0x0072_6576, &[self.instance, self.counter]) | 1
    }
}

impl Default for Revision {
    fn default() -> Revision {
        Revision::new()
    }
}

/// A clone is another instance: it may be edited on its own, and its numbers must not meet the original's.
impl Clone for Revision {
    fn clone(&self) -> Revision {
        Revision::new()
    }
}

impl PartialEq for Revision {
    fn eq(&self, _: &Revision) -> bool {
        true
    }
}
