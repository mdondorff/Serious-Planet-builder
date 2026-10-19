//! Tier A: 128-bit item ids and content hashes in the keys (WDEF-009, STRM-003). A query result of items below 2^64 hashes exactly as it
//! did when ids and hashes were 64 bits (the old encoding is written out here, independently); wider values change the hash and never
//! alias a narrower row.

use planet_cache::key::{footprint_id_bytes, footprint_result_hash128, query_result_hash128};
use planet_world_def::cells::ItemEntry;
use planet_world_def::footprint::FootprintItem;

const WIDE: u128 = 1u128 << 64;

/// The encoding from before ids were 128 bits, written out: 40-byte rows of id, content hash and the three direction words, little-endian.
fn old_query_hash(rows: &[(u64, u64, [u64; 3])]) -> [u8; 16] {
    let mut sorted = rows.to_vec();
    sorted.sort_unstable();
    let mut m = Vec::new();
    for (id, content, dir) in sorted {
        m.extend_from_slice(&id.to_le_bytes());
        m.extend_from_slice(&content.to_le_bytes());
        for d in dir {
            m.extend_from_slice(&d.to_le_bytes());
        }
    }
    blake3::derive_key("planet-builder query result v1", &m)[..16].try_into().unwrap()
}

fn row(id: u128, content: u128, d: u64) -> ItemEntry {
    (id, content, [d, d + 1, d + 2])
}

// spec: WDEF-009
#[test]
fn a_query_result_of_narrow_ids_hashes_as_it_did_with_64_bit_ids() {
    let narrow: Vec<ItemEntry> = vec![row(9, 40, 100), row(2, 7, 200), row(5, 7, 300)];
    let old: Vec<(u64, u64, [u64; 3])> = narrow.iter().map(|(i, c, d)| (*i as u64, *c as u64, *d)).collect();
    assert_eq!(query_result_hash128(&narrow), old_query_hash(&old));
    assert_eq!(query_result_hash128(&[]), old_query_hash(&[]), "an empty result is the empty input");
    // The largest narrow values too.
    let top = vec![row(u128::from(u64::MAX), u128::from(u64::MAX), 1)];
    assert_eq!(query_result_hash128(&top), old_query_hash(&[(u64::MAX, u64::MAX, [1, 2, 3])]));
}

// spec: WDEF-009
#[test]
fn a_wide_id_or_content_hash_changes_the_query_hash_and_no_two_wide_layouts_collide() {
    let base = query_result_hash128(&[row(1, 5, 10), row(2, 6, 20)]);
    let variants = [
        vec![row(1 + WIDE, 5, 10), row(2, 6, 20)],
        vec![row(1, 5 + WIDE, 10), row(2, 6, 20)],
        vec![row(1, 5, 10), row(2 + WIDE, 6, 20)],
        vec![row(1, 5, 10), row(2, 6 + WIDE, 20)],
        vec![row(1 + WIDE, 5, 10), row(2 + WIDE, 6, 20)],
        vec![row(1 + 2 * WIDE, 5, 10), row(2, 6, 20)],
    ];
    let mut seen = vec![base];
    for v in &variants {
        let h = query_result_hash128(v);
        assert!(!seen.contains(&h), "{v:?}");
        seen.push(h);
    }
    // A function of the set: the order the rows come in does not matter.
    let mut reversed = variants[0].clone();
    reversed.reverse();
    assert_eq!(query_result_hash128(&reversed), query_result_hash128(&variants[0]));
}

fn item(id: u128, content: u128) -> FootprintItem {
    FootprintItem { id, priority: 0, content, dir: [1, 2, 3], reach_bits: 4, max_delta_bits: 5, layer: "items".into(), stamp: None }
}

// spec: WDEF-009
#[test]
fn the_footprint_result_orders_by_the_16_byte_id_and_wide_values_reach_its_hash() {
    assert_eq!(footprint_id_bytes(1), 1u128.to_be_bytes());
    assert_eq!(footprint_id_bytes(WIDE + 1)[..8], 1u64.to_be_bytes(), "the high word comes first");
    let narrow = footprint_result_hash128(&[item(1, 5), item(2, 6)]);
    let wide_id = footprint_result_hash128(&[item(1, 5), item(2 + WIDE, 6)]);
    let wide_content = footprint_result_hash128(&[item(1, 5), item(2, 6 + WIDE)]);
    assert!(narrow != wide_id && narrow != wide_content && wide_id != wide_content);
    // Application order: a wide id sorts after every narrow one of the same priority, whatever order the items arrive in.
    assert_eq!(footprint_result_hash128(&[item(2 + WIDE, 6), item(1, 5)]), wide_id);
}

// spec: WDEF-009
#[test]
fn sets_with_different_row_counts_never_give_the_same_query_hash_the_review_found_a_pair_for_a_tail_read_from_the_front() {
    let z = [0u64, 0, 0];
    // The numerics review's pair: the four-row set's last row is made of the first 40 bytes of the other's tail when the tail has no row
    // count of its own.
    let b: Vec<ItemEntry> = vec![(1, WIDE, z), (2, 0x01_0100_0000u128 << 64, z), (3, WIDE, z)];
    let a: Vec<ItemEntry> = vec![(1, 0, z), (2, 0, z), (3, WIDE, z), (0x301, 0, [0x100, 0x100, 0])];
    assert_ne!(query_result_hash128(&a), query_result_hash128(&b));
    // A sweep over small sets of rows with and without wide values and different counts: every set hashes differently from every other.
    let rows: Vec<ItemEntry> = (0..4u128)
        .flat_map(|i| [(i + 1, 5, z), (i + 1 + WIDE, 5, z), (i + 1, 5 + WIDE, z), (i + 1 + 3 * WIDE, 5 + 7 * WIDE, [i as u64, 1, 2])])
        .collect();
    let mut seen = std::collections::BTreeMap::new();
    for mask in 0u32..(1 << 12) {
        let set: Vec<ItemEntry> = (0..12).filter(|b| mask & (1 << b) != 0).map(|b| rows[b]).collect();
        // The hash is of the set sorted, as the function sorts; duplicates of one id are different rows and kept.
        let mut key = set.clone();
        key.sort_unstable();
        let hash = query_result_hash128(&set);
        if let Some(other) = seen.insert(hash, key.clone()) {
            assert_eq!(other, key, "two different sets gave one hash");
        }
    }
    assert!(seen.len() > 4000);
}
