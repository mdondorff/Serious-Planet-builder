//! New item ids (ADR 0066 point 6): a ULID made by the command from an injected clock and injected entropy, never by the generator.
//!
//! Within one millisecond the entropy of the next id is the last one plus one (the monotonic ULID rule), so ids made in a row sort in the
//! order they were made and never repeat; when the clock goes back or stands still the ids still increase.

use planet_world_def::item::make_ulid;
use planet_world_def::ItemId;

/// The time source of the editor, in milliseconds since the Unix epoch.
pub trait Clock {
    fn now_ms(&mut self) -> u64;
}

/// The entropy source of the editor: ten random bytes per call.
pub trait Entropy {
    fn fill(&mut self, out: &mut [u8; 10]);
}

/// The system clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&mut self) -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
    }
}

/// A clock that says what it is told (tests).
pub struct FixedClock(pub u64);

impl Clock for FixedClock {
    fn now_ms(&mut self) -> u64 {
        self.0
    }
}

/// Entropy from a seed by a fixed generator (tests and reproducible sessions): splitmix64, deterministic on every platform.
pub struct SeededEntropy(pub u64);

impl SeededEntropy {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

impl Entropy for SeededEntropy {
    fn fill(&mut self, out: &mut [u8; 10]) {
        let (a, b) = (self.next().to_le_bytes(), self.next().to_le_bytes());
        out[..8].copy_from_slice(&a);
        out[8..].copy_from_slice(&b[..2]);
    }
}

/// Operating-system entropy: the standard library's randomly keyed hasher, one per call (no extra dependency; not for secrets, only
/// for telling two people's ids apart).
pub struct OsEntropy;

impl Entropy for OsEntropy {
    fn fill(&mut self, out: &mut [u8; 10]) {
        use std::hash::{BuildHasher, Hasher};
        for (i, chunk) in out.chunks_mut(8).enumerate() {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_usize(i);
            chunk.copy_from_slice(&h.finish().to_le_bytes()[..chunk.len()]);
        }
    }
}

/// Makes ULIDs from a clock and an entropy source.
pub struct IdMaker {
    clock: Box<dyn Clock>,
    entropy: Box<dyn Entropy>,
    last: Option<(u64, [u8; 10])>,
}

impl IdMaker {
    pub fn new(clock: Box<dyn Clock>, entropy: Box<dyn Entropy>) -> IdMaker {
        IdMaker { clock, entropy, last: None }
    }

    /// The system clock and operating-system entropy.
    pub fn system() -> IdMaker {
        IdMaker::new(Box::new(SystemClock), Box::new(OsEntropy))
    }

    /// The next id: later than every id this maker made before.
    pub fn next_id(&mut self) -> ItemId {
        let now = self.clock.now_ms() & 0xFFFF_FFFF_FFFF;
        let (ms, entropy) = match self.last {
            // The same millisecond, or a clock that went back: one more than the last entropy.
            Some((last_ms, last_entropy)) if now <= last_ms => match increment(last_entropy) {
                Some(next) => (last_ms, next),
                // 2^80 ids in one millisecond: move to the next millisecond with fresh entropy.
                None => (last_ms + 1, self.fresh()),
            },
            _ => (now, self.fresh()),
        };
        self.last = Some((ms, entropy));
        make_ulid(ms, entropy)
    }

    fn fresh(&mut self) -> [u8; 10] {
        let mut e = [0u8; 10];
        self.entropy.fill(&mut e);
        e
    }
}

/// The 80-bit big-endian number plus one, or `None` on overflow.
fn increment(mut e: [u8; 10]) -> Option<[u8; 10]> {
    for b in e.iter_mut().rev() {
        let (v, carry) = b.overflowing_add(1);
        *b = v;
        if !carry {
            return Some(e);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: AUTH-004
    #[test]
    fn ids_come_from_the_injected_clock_and_entropy_and_increase_whatever_the_clock_does() {
        struct Steps(Vec<u64>);
        impl Clock for Steps {
            fn now_ms(&mut self) -> u64 {
                if self.0.len() > 1 {
                    self.0.remove(0)
                } else {
                    self.0[0]
                }
            }
        }
        let mut maker = IdMaker::new(Box::new(Steps(vec![1_000, 1_000, 1_000, 999, 1_001, 1_001])), Box::new(SeededEntropy(7)));
        let ids: Vec<ItemId> = (0..6).map(|_| maker.next_id()).collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "strictly increasing: {ids:x?}");
        assert_eq!(ids[0] >> 80, 1_000);
        assert_eq!(ids[1], ids[0] + 1, "the same millisecond: one more");
        assert_eq!(ids[2], ids[1] + 1);
        assert_eq!(ids[3], ids[2] + 1, "a clock that went back keeps the last millisecond");
        assert_eq!(ids[4] >> 80, 1_001, "the clock moved on: its time and fresh entropy");
        assert_eq!(ids[5], ids[4] + 1);
        // The same sources give the same ids; other entropy gives other ids.
        let again: Vec<ItemId> = {
            let mut m = IdMaker::new(Box::new(FixedClock(1_000)), Box::new(SeededEntropy(7)));
            (0..2).map(|_| m.next_id()).collect()
        };
        assert_eq!(again, ids[..2]);
        let other = IdMaker::new(Box::new(FixedClock(1_000)), Box::new(SeededEntropy(8))).next_id();
        assert_ne!(other, ids[0]);
    }

    // spec: AUTH-004
    #[test]
    fn the_entropy_carry_runs_through_all_ten_bytes_and_overflows_to_the_next_millisecond() {
        assert_eq!(increment([0, 0, 0, 0, 0, 0, 0, 0, 0, 255]), Some([0, 0, 0, 0, 0, 0, 0, 0, 1, 0]));
        assert_eq!(increment([0, 0, 0, 0, 0, 0, 0, 255, 255, 255]), Some([0, 0, 0, 0, 0, 0, 1, 0, 0, 0]));
        assert_eq!(increment([255; 10]), None);
        struct Ones;
        impl Entropy for Ones {
            fn fill(&mut self, out: &mut [u8; 10]) {
                *out = [255; 10];
            }
        }
        let mut maker = IdMaker::new(Box::new(FixedClock(5)), Box::new(Ones));
        let first = maker.next_id();
        let second = maker.next_id();
        assert_eq!(first >> 80, 5);
        assert_eq!(second >> 80, 6, "2^80 ids in one millisecond move to the next one");
        assert!(second > first);
    }

    // spec: AUTH-004
    #[test]
    fn the_system_sources_make_distinct_increasing_ids() {
        let mut maker = IdMaker::system();
        let ids: Vec<ItemId> = (0..2_000).map(|_| maker.next_id()).collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]));
        let mut other = IdMaker::system();
        assert_ne!(other.next_id() & ((1 << 80) - 1), ids[0] & ((1 << 80) - 1), "operating-system entropy differs between makers");
    }
}
