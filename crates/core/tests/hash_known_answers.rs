//! COORD-008: integer hashing with committed known answers.

use planet_core::hash::{hash_lattice2, hash_u64s, mix64, SplitMix64, HASH_VERSION};

// spec: COORD-008
#[test]
fn splitmix64_matches_the_reference_sequence() {
    // Reference outputs of SplitMix64 seeded with 0 (public-domain reference implementation).
    let mut r = SplitMix64(0);
    assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
    assert_eq!(r.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    assert_eq!(r.next_u64(), 0x06C4_5D18_8009_454F);
}

// spec: COORD-008
#[test]
fn hash_known_answers_for_version_1() {
    assert_eq!(HASH_VERSION, 1);
    assert_eq!(mix64(0), 0);
    assert_eq!(mix64(1), KAT_MIX_1);
    assert_eq!(hash_u64s(0, &[]), KAT_EMPTY);
    assert_eq!(hash_u64s(42, &[1, 2, 3]), KAT_123);
    assert_ne!(hash_u64s(42, &[1, 2, 3]), hash_u64s(42, &[3, 2, 1]), "order matters");
    assert_ne!(hash_u64s(0, &[0]), hash_u64s(0, &[0, 0]), "length matters");
    assert_eq!(hash_lattice2(7, -3, 5), KAT_LATTICE);
    assert_ne!(hash_lattice2(7, -3, 5), hash_lattice2(7, 5, -3));
}

const KAT_MIX_1: u64 = 6238072747940578789;
const KAT_EMPTY: u64 = 5197578548964807871;
const KAT_123: u64 = 4328046119131358040;
const KAT_LATTICE: u64 = 15985609064533783241;
