//! Integer hashing and a small RNG (ADR 0010). Pure integer arithmetic: identical on every platform.
//!
//! `HASH_VERSION` is part of generator versioning; changing any output below requires a bump and new
//! known-answer values in the tests.

pub const HASH_VERSION: u32 = 1;

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

/// SplitMix64 finaliser (Steele, Lea, Flood 2014; public domain algorithm).
pub fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Hash a sequence of integers with a seed. Order matters; length is mixed in.
pub fn hash_u64s(seed: u64, values: &[u64]) -> u64 {
    let mut h = mix64(seed ^ GOLDEN);
    for &v in values {
        h = mix64(h.wrapping_add(GOLDEN) ^ v);
    }
    mix64(h ^ values.len() as u64)
}

/// Lattice hash for noise: integer cell coordinates (may be negative) and a seed.
pub fn hash_lattice2(seed: u64, ix: i64, iy: i64) -> u64 {
    hash_u64s(seed, &[ix as u64, iy as u64])
}

/// Uniform value in [0, 1) from the top 53 bits of a hash.
pub fn unit_f64(h: u64) -> f64 {
    (h >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// SplitMix64 generator.
#[derive(Clone, Debug)]
pub struct SplitMix64(pub u64);

impl SplitMix64 {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(GOLDEN);
        mix64(self.0)
    }

    pub fn next_f64(&mut self) -> f64 {
        unit_f64(self.next_u64())
    }

    /// Uniform in [lo, hi).
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        let v = lo + (hi - lo) * self.next_f64();
        if v >= hi {
            hi.next_down()
        } else {
            v
        }
    }
}
