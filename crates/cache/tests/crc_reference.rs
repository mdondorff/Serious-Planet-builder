//! The payload CRC (STRM-008, ADR 0019) against its bitwise definition: a faster implementation must give the same value for
//! every length and alignment, and continue across any split.

use planet_cache::crc32c::{crc32c, update};

fn bitwise(c: u32, data: &[u8]) -> u32 {
    let mut c = !c;
    for b in data {
        c ^= u32::from(*b);
        for _ in 0..8 {
            c = if c & 1 != 0 { 0x82F6_3B78 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

// spec: STRM-008
#[test]
fn crc32c_agrees_with_the_bitwise_definition_at_every_length_and_alignment() {
    let mut x = 0x1234_5678u32;
    let data: Vec<u8> = (0..300)
        .map(|_| {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (x >> 24) as u8
        })
        .collect();
    for start in 0..9 {
        for len in 0..=(data.len() - start - 8) {
            let d = &data[start..start + len];
            assert_eq!(crc32c(d), bitwise(0, d), "start {start} len {len}");
        }
    }
    for split in [0usize, 1, 7, 8, 9, 77, 300] {
        let (a, b) = data.split_at(split);
        assert_eq!(update(crc32c(a), b), crc32c(&data), "split {split}");
        assert_eq!(update(bitwise(0, a), b), bitwise(0, &data), "split {split}");
    }
}
