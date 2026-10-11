//! STRM-008 (tile store ordering and validation, ADR 0019): the Hilbert sort key and the payload CRC.

use planet_cache::crc32c::{crc32c, update};
use planet_cache::hilbert::{d_to_xy, sort_key, xy_to_d};
use planet_core::{Face, TileId};

// spec: STRM-008
#[test]
fn hilbert_known_answers_follow_the_standard_curve() {
    // The standard curve on a 4 x 4 grid (Wikipedia's d2xy listing), as (x, y) in order of d.
    let order2 =
        [(0, 0), (1, 0), (1, 1), (0, 1), (0, 2), (0, 3), (1, 3), (1, 2), (2, 2), (2, 3), (3, 3), (3, 2), (3, 1), (2, 1), (2, 0), (3, 0)];
    for (d, (x, y)) in order2.iter().enumerate() {
        assert_eq!(xy_to_d(2, *x, *y), d as u64, "({x}, {y})");
        assert_eq!(d_to_xy(2, d as u64), (*x, *y), "d = {d}");
    }
    // The single-cell grid and the 2 x 2 grid.
    assert_eq!(xy_to_d(0, 0, 0), 0);
    let order1: Vec<(u32, u32)> = (0..4).map(|d| d_to_xy(1, d)).collect();
    assert_eq!(order1, [(0, 0), (0, 1), (1, 1), (1, 0)]);
    // Corners of a large grid, to catch overflow: the curve starts at (0, 0) and ends at (n-1, 0) or (0, n-1).
    assert_eq!(xy_to_d(28, 0, 0), 0);
    let last = (1u64 << 56) - 1;
    // Every order ends at the far corner of the bottom edge, (n - 1, 0).
    assert_eq!(d_to_xy(28, last), ((1 << 28) - 1, 0));
    assert_eq!(d_to_xy(31, (1u64 << 62) - 1), ((1u32 << 31) - 1, 0));
    assert_eq!(xy_to_d(31, (1u32 << 31) - 1, 0), (1u64 << 62) - 1);
}

// spec: STRM-008
#[test]
fn the_curve_is_a_bijection_and_consecutive_cells_are_neighbours() {
    for bits in 1..=6u8 {
        let n = 1u32 << bits;
        let mut seen = vec![false; (n * n) as usize];
        let mut prev: Option<(u32, u32)> = None;
        for d in 0..u64::from(n) * u64::from(n) {
            let (x, y) = d_to_xy(bits, d);
            assert_eq!(xy_to_d(bits, x, y), d);
            assert!(!seen[(y * n + x) as usize]);
            seen[(y * n + x) as usize] = true;
            if let Some((px, py)) = prev {
                assert_eq!(px.abs_diff(x) + py.abs_diff(y), 1, "order {bits}: d = {d} is not next to d - 1");
            }
            prev = Some((x, y));
        }
        assert!(seen.iter().all(|s| *s));
    }
}

// spec: STRM-008
#[test]
fn a_cells_four_children_take_four_consecutive_indices() {
    for bits in 1..=7u8 {
        let n = 1u32 << bits;
        for (x, y) in [(0, 0), (1, 0), (n / 2, n / 2), (n - 1, 0), (n - 1, n - 1), (3 % n, 5 % n)] {
            let parent = xy_to_d(bits, x, y);
            let mut kids: Vec<u64> = (0..4).map(|i| xy_to_d(bits + 1, 2 * x + (i & 1), 2 * y + (i >> 1))).collect();
            kids.sort_unstable();
            assert_eq!(kids, [4 * parent, 4 * parent + 1, 4 * parent + 2, 4 * parent + 3], "order {bits} cell ({x}, {y})");
        }
    }
}

// spec: STRM-008
#[test]
fn neighbouring_tiles_are_near_each_other_in_the_index() {
    // A bounded index distance on a sample: the median distance to the four neighbours stays small, and no neighbour of a
    // sampled tile is further than the whole grid (sanity) while most are within a few cells' worth of indices.
    let bits = 8u8;
    let n = 1u32 << bits;
    let mut within = 0;
    let mut total = 0;
    for i in 0..400u32 {
        let (x, y) = ((i * 37) % (n - 1), (i * 91) % (n - 1));
        let d = xy_to_d(bits, x, y);
        for (nx, ny) in [(x + 1, y), (x, y + 1)] {
            let e = xy_to_d(bits, nx, ny);
            total += 1;
            if d.abs_diff(e) <= 64 {
                within += 1;
            }
        }
    }
    assert!(within * 100 / total >= 75, "{within} of {total} neighbours within 64 indices");
}

// spec: STRM-008
#[test]
fn the_sort_key_orders_by_kind_face_level_then_curve() {
    let t = |f: u8, l: u8, x: u32, y: u32| TileId::new(Face(f), l, x, y).unwrap();
    let k = |kind: u8, tile: TileId| sort_key(kind, tile);
    assert!(k(1, t(5, 20, 9, 9)) < k(2, t(0, 0, 0, 0)), "kind first");
    assert!(k(1, t(0, 20, 9, 9)) < k(1, t(1, 0, 0, 0)), "then face");
    assert!(k(1, t(1, 3, 7, 7)) < k(1, t(1, 4, 0, 0)), "then level");
    let a = t(1, 4, 3, 2);
    let b = t(1, 4, 3, 3);
    assert_eq!(k(1, a) < k(1, b), xy_to_d(4, 3, 2) < xy_to_d(4, 3, 3), "then the Hilbert index");
    // The pinned layout: kind, face, level, then the index as 8 big-endian bytes.
    let key = k(2, t(3, 5, 7, 11));
    assert_eq!(&key[..3], &[2, 3, 5]);
    assert_eq!(u64::from_be_bytes(key[3..].try_into().unwrap()), xy_to_d(5, 7, 11));
    // Known answer for one tile.
    assert_eq!(key, [2, 3, 5, 0, 0, 0, 0, 0, 0, 0, 208]);
}

// spec: STRM-008
#[test]
fn crc32c_matches_the_published_check_values() {
    assert_eq!(crc32c(b""), 0);
    assert_eq!(crc32c(b"123456789"), 0xE306_9283, "the standard CRC-32C check value");
    assert_eq!(crc32c(&[0u8; 32]), 0x8A91_36AA, "iSCSI test vector: 32 zero bytes");
    assert_eq!(crc32c(&[0xffu8; 32]), 0x62A8_AB43, "iSCSI test vector: 32 bytes of 0xff");
    let data: Vec<u8> = (0..32u8).collect();
    assert_eq!(crc32c(&data), 0x46DD_794E, "iSCSI test vector: ascending bytes");
    let (a, b) = data.split_at(11);
    assert_eq!(update(crc32c(a), b), crc32c(&data), "a CRC continues over more data");
    let mut flipped = data.clone();
    flipped[17] ^= 1;
    assert_ne!(crc32c(&flipped), crc32c(&data));
}
