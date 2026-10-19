//! Tier A: authored items have 128-bit ids and content hashes (WDEF-009, ADR 0066 point 6). Everything that hashed a 64-bit id keeps its
//! answer for values below 2^64 (the pinned answers in `summary.rs` and the cache's `keys_footprint.rs` check it for the footprint and
//! summary hashes; here the definition, cell and footprint hashes); wider values reach every hash and never alias a narrower one.

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_world_def::cells::{cell_hash, cell_of, ItemEntry};
use planet_world_def::{footprint_hash, BodyId, ItemId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;
const WIDE: u128 = 1u128 << 64;

fn dir(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z).normalized()
}

fn world() -> WorldDefinition {
    WorldDefinition::single_planet(R, 7)
}

fn spec(id: ItemId, at: Vec3, content: u128) -> ItemSpec {
    ItemSpec { id, layer: DEFAULT_ITEM_LAYER.into(), priority: 0, dir: at, content_hash: content, reach_m: 500.0, max_delta_m: 10.0 }
}

// spec: WDEF-009
#[test]
fn ids_and_content_hashes_are_128_bits_and_wide_values_work_everywhere() {
    let at = dir(1.0, 0.2, 0.1);
    let mut d = world();
    let huge: ItemId = u128::MAX;
    let cell = d.put_item_spec(BodyId::PLANET, spec(huge, at, u128::MAX - 3)).unwrap();
    d.put_item_spec(BodyId::PLANET, spec(1, dir(-1.0, 0.3, 0.2), 9)).unwrap();
    assert_eq!(d.item_ids(BodyId::PLANET), [1, huge], "the id order is the numeric order");
    assert_eq!(d.items_in_cell(BodyId::PLANET, cell), vec![(huge, u128::MAX - 3)]);
    let tile = TileId::from_direction(&TangentWarp, at, 12);
    let found = d.footprint_items(BodyId::PLANET, DEFAULT_ITEM_LAYER, tile, 0.0).unwrap();
    assert_eq!(found.iter().map(|i| (i.id, i.content)).collect::<Vec<_>>(), [(huge, u128::MAX - 3)]);
    assert!(d.item_anchor(BodyId::PLANET, huge).is_some());
    assert!(d.remove_item(BodyId::PLANET, huge));
    assert!(!d.remove_item(BodyId::PLANET, huge));
    assert_eq!(d.item_ids(BodyId::PLANET), [1]);
}

// spec: WDEF-009
#[test]
fn a_wide_id_or_content_hash_never_hashes_like_a_narrow_one_with_the_same_low_bits() {
    let at = dir(1.0, 0.2, 0.1);
    let hash_of = |id: u128, content: u128| {
        let mut d = world();
        d.put_item_spec(BodyId::PLANET, spec(id, at, content)).unwrap();
        d.hash()
    };
    let base = hash_of(1, 5);
    assert_ne!(hash_of(1 + WIDE, 5), base, "the high bits of the id are in the definition hash");
    assert_ne!(hash_of(1, 5 + WIDE), base, "and those of the content hash");
    assert_ne!(hash_of(1 + WIDE, 5), hash_of(1, 5 + WIDE));
    assert_ne!(hash_of(1 + 2 * WIDE, 5), hash_of(1 + WIDE, 5));
    assert_eq!(hash_of(1 + WIDE, 5), hash_of(1 + WIDE, 5), "and it is a function of them");
    // Several items: the same set in another insertion order is the same definition.
    let items = [(1u128, 5u128), (2 + WIDE, 6), (3, 7 + 5 * WIDE)];
    let (mut a, mut b) = (world(), world());
    for (i, (id, c)) in items.iter().enumerate() {
        a.put_item_spec(BodyId::PLANET, spec(*id, dir(1.0, 0.2 + i as f64 * 0.01, 0.1), *c)).unwrap();
    }
    for (i, (id, c)) in items.iter().enumerate().rev() {
        b.put_item_spec(BodyId::PLANET, spec(*id, dir(1.0, 0.2 + i as f64 * 0.01, 0.1), *c)).unwrap();
    }
    assert_eq!(a.hash(), b.hash());
}

// spec: WDEF-009
#[test]
fn the_cell_and_footprint_hashes_cover_the_high_bits_and_keep_their_values_below_2_64() {
    let row = |id: u128, content: u128| -> ItemEntry { (id, content, [1, 2, 3]) };
    let narrow = cell_hash(&[row(1, 5), row(2, 6)]);
    assert_ne!(cell_hash(&[row(1 + WIDE, 5), row(2, 6)]), narrow);
    assert_ne!(cell_hash(&[row(1, 5 + WIDE), row(2, 6)]), narrow);
    assert_ne!(cell_hash(&[row(1, 5), row(2 + WIDE, 6)]), cell_hash(&[row(1 + WIDE, 5), row(2, 6)]), "which row is wide matters");
    // A cell hash of narrow rows is the one a 64-bit build computed: it is the function of the low words alone.
    assert_eq!(cell_hash(&[row(1, 5), row(2, 6)]), narrow);
    let d = {
        let mut d = world();
        d.put_item_spec(BodyId::PLANET, spec(7, dir(1.0, 0.2, 0.1), 3)).unwrap();
        d
    };
    let tile = TileId::from_direction(&TangentWarp, dir(1.0, 0.2, 0.1), 12);
    let items = d.footprint_items(BodyId::PLANET, DEFAULT_ITEM_LAYER, tile, 0.0).unwrap();
    let wide: Vec<_> = items
        .iter()
        .cloned()
        .map(|mut i| {
            i.id += WIDE;
            i
        })
        .collect();
    let wide_content: Vec<_> = items
        .iter()
        .cloned()
        .map(|mut i| {
            i.content += WIDE;
            i
        })
        .collect();
    assert_ne!(footprint_hash(&wide), footprint_hash(&items));
    assert_ne!(footprint_hash(&wide_content), footprint_hash(&items));
    assert_ne!(footprint_hash(&wide), footprint_hash(&wide_content));
    let _ = cell_of(R, dir(1.0, 0.2, 0.1));
}
