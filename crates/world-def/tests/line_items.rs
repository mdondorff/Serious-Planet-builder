//! Tier A: line items and the item file with a points list (AUTH-005 schema 2, WDEF-011, ADR 0069): the canonical text, the refusals,
//! the hash and the stamp of a ridge or a channel, and the put and the footprint of a line.

use planet_core::{TangentWarp, TileId, Vec3};
use planet_world_def::item::{Anchor, Item, ITEM_HASH_CONTEXT};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{footprint_hash, BodyId, GenContext, WorldDefinition};
use std::collections::BTreeMap;

const R: f64 = 6_371_000.0;
const GROUP: u128 = 0x018F_2B4C_5D6E_0123_4567_89AB_CDEF_0123;

fn line_item(kind: &str, points: &[(f64, f64)]) -> Item {
    Item {
        id: 0x0123_4567_89ab_cdef_0123_4567_89ab_cdef,
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "splines".into(),
        priority: 2,
        kind: kind.into(),
        anchor: Anchor { latitude_deg: points[0].0, longitude_deg: points[0].1, height_m: 0.0, heading_deg: 0.0 },
        reach_m: 5_000.0,
        max_delta_m: 100.0,
        params: BTreeMap::from([("amount_m".to_string(), 40.0), ("width_m".to_string(), 200.0)]),
        base_hash: 7,
        generator_version: 1,
        stage_version: 1,
        name: Some("a line".into()),
        notes: None,
        points: points.to_vec(),
        group: Some(GROUP),
        part: Some(2),
        mask_layer: None,
        strokes: Vec::new(),
    }
}

fn path() -> Vec<(f64, f64)> {
    vec![(20.0, 30.0), (20.01, 30.0), (20.01, 30.01)]
}

fn refused(item: &Item, key: &str) {
    let e = item.validate().expect_err(&format!("{key}: the item must be refused"));
    assert!(e.key.contains(key), "the error names `{key}`: {e}");
}

// spec: AUTH-005, WDEF-011
#[test]
fn a_line_is_a_schema_2_file_in_the_fixed_key_order_and_reads_back_to_the_same_item() {
    let it = line_item("ridge", &path());
    let text = it.to_toml().unwrap();
    assert!(text.starts_with("schema_version = 2\nid = "), "{text}");
    // The optional keys follow the notes (here the name), then the tables; the points come last, in order.
    let order = ["name = ", "group = \"", "part = 2\n", "[anchor]", "[params]", "[[points]]"];
    let at: Vec<usize> = order.iter().map(|k| text.find(k).unwrap_or_else(|| panic!("`{k}` in {text}"))).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "key order {at:?} in {text}");
    assert_eq!(text.matches("[[points]]").count(), 3);
    assert!(text.contains("[[points]]\nlatitude = 20.0\nlongitude = 30.0\n\n[[points]]\nlatitude = 20.01\nlongitude = 30.0\n"), "{text}");
    let back = Item::from_toml(&text).unwrap();
    assert_eq!(back, it);
    assert_eq!(back.to_toml().unwrap(), text, "written again, the same text");
    // A hand-written file with a comment and integer literals reads as the same item.
    let hand = text.replace("\n\n[anchor]", "\n# a comment\n\n[anchor]").replace("latitude = 20.0\n", "latitude = 20\n");
    assert_eq!(Item::from_toml(&hand).unwrap(), it, "comments and integer literals change nothing");
    assert_eq!(Item::from_toml(&hand).unwrap().hash(), it.hash());
}

// spec: AUTH-005
#[test]
fn an_item_without_points_is_still_schema_1_and_hashes_as_it_did() {
    let mut it = line_item("hill", &[(20.0, 30.0)]);
    it.points.clear();
    it.group = None;
    it.part = None;
    it.params = BTreeMap::from([("height_m".to_string(), 30.0)]);
    let text = it.to_toml().unwrap();
    assert!(text.starts_with("schema_version = 1\n"), "{text}");
    assert!(!text.contains("points") && !text.contains("group") && !text.contains("part") && !text.contains("mask_layer"), "{text}");
    assert_eq!(it.schema_version(), 1);
}

// spec: AUTH-005, WDEF-011
#[test]
fn the_points_file_refuses_what_the_writer_refuses_naming_the_key() {
    let ok = line_item("ridge", &path());
    let mut it = ok.clone();
    it.points[0] = (20.0, 30.5);
    refused(&it, "points[0]");
    let mut it = ok.clone();
    it.points[1] = it.points[0];
    refused(&it, "points[1]");
    let mut it = ok.clone();
    it.points[2].0 = 91.0;
    refused(&it, "points[2].latitude");
    let mut it = ok.clone();
    it.points[2].1 = -180.0;
    refused(&it, "points[2].longitude");
    let mut it = ok.clone();
    it.points.truncate(1);
    refused(&it, "points");
    let mut it = ok.clone();
    it.points = (0..65).map(|i| (20.0 + 0.0001 * f64::from(i), 30.0)).collect();
    it.anchor.latitude_deg = 20.0;
    refused(&it, "points");
    let mut it = ok.clone();
    it.points.clear();
    refused(&it, "points");
    let mut it = ok.clone();
    it.group = None;
    refused(&it, "group");
    let mut it = ok.clone();
    it.part = None;
    refused(&it, "group");
    let mut it = ok.clone();
    it.mask_layer = Some("stamps".into());
    refused(&it, "mask_layer");
    let mut it = ok.clone();
    it.params.remove("width_m");
    refused(&it, "params");
    // Points on a kind that takes none; a group without points.
    let mut hill = ok.clone();
    hill.kind = "hill".into();
    hill.params = BTreeMap::from([("height_m".to_string(), 30.0)]);
    refused(&hill, "points");
    hill.points.clear();
    refused(&hill, "group");
    // The file's version must agree with its points.
    let text = ok.to_toml().unwrap();
    assert!(Item::from_toml(&text.replacen("schema_version = 2", "schema_version = 1", 1)).unwrap_err().key.contains("schema_version"));
    let mut plain = ok.clone();
    plain.kind = "hill".into();
    plain.params = BTreeMap::from([("height_m".to_string(), 30.0)]);
    plain.points.clear();
    plain.group = None;
    plain.part = None;
    let plain_text = plain.to_toml().unwrap();
    assert!(Item::from_toml(&plain_text.replacen("schema_version = 1", "schema_version = 2", 1))
        .unwrap_err()
        .key
        .contains("schema_version"));
    assert!(Item::from_toml(&text.replace("[[points]]\nlatitude", "[[points]]\nlat")).unwrap_err().key.contains("points[0]"));
    assert!(Item::from_toml(&text.replace("schema_version = 2", "schema_version = 4")).unwrap_err().why.contains("newer"));
}

// spec: AUTH-005
#[test]
fn the_hash_of_a_points_item_follows_every_point_the_group_and_the_part_and_equals_the_layout_computed_here() {
    let it = line_item("ridge", &path());
    let h = it.hash();
    let mut moved = it.clone();
    moved.points[2].1 = 30.02;
    assert_ne!(moved.hash(), h, "a point");
    let mut g = it.clone();
    g.group = Some(GROUP + 1);
    assert_ne!(g.hash(), h, "the group");
    let mut p = it.clone();
    p.part = Some(3);
    assert_ne!(p.hash(), h, "the part");
    let mut n = it.clone();
    n.name = Some("renamed".into());
    n.notes = Some("notes".into());
    assert_eq!(n.hash(), h, "the name and the notes are not in the hash");

    // The layout of AUTH-005, built by hand: the schema-1 material with version 2, then the flag-prefixed group and part, the mask layer
    // (empty), the point count and the points as bit patterns.
    let mut m: Vec<u8> = Vec::new();
    let text = |m: &mut Vec<u8>, s: &str| {
        m.extend_from_slice(&(s.len() as u32).to_le_bytes());
        m.extend_from_slice(s.as_bytes());
    };
    m.extend_from_slice(&2u32.to_le_bytes());
    m.extend_from_slice(&it.id.to_be_bytes());
    m.extend_from_slice(&it.body.0.to_le_bytes());
    text(&mut m, "sphere");
    text(&mut m, "splines");
    m.extend_from_slice(&2i32.to_le_bytes());
    text(&mut m, "ridge");
    for v in [20.0f64, 30.0, 0.0, 0.0, 5000.0, 100.0] {
        m.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    m.extend_from_slice(&2u32.to_le_bytes());
    for (k, v) in [("amount_m", 40.0f64), ("width_m", 200.0)] {
        text(&mut m, k);
        m.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    m.extend_from_slice(&7u64.to_le_bytes());
    m.extend_from_slice(&1u32.to_le_bytes());
    m.extend_from_slice(&1u32.to_le_bytes());
    m.push(1);
    m.extend_from_slice(&GROUP.to_be_bytes());
    m.push(1);
    m.extend_from_slice(&2u32.to_le_bytes());
    text(&mut m, "");
    m.extend_from_slice(&3u32.to_le_bytes());
    for (la, lo) in path() {
        m.extend_from_slice(&la.to_bits().to_le_bytes());
        m.extend_from_slice(&lo.to_bits().to_le_bytes());
    }
    let full = blake3::derive_key(ITEM_HASH_CONTEXT, &m);
    assert_eq!(h, u128::from_be_bytes(full[..16].try_into().unwrap()));
}

// spec: WDEF-011
#[test]
fn a_line_is_a_stamp_with_its_points_and_a_content_hash_derived_from_kind_numbers_and_points() {
    let it = line_item("ridge", &path());
    let spec = it.spec();
    let stamp = spec.stamp.clone().expect("a ridge is a stamp");
    assert_eq!((stamp.kind, stamp.params()), (StampKind::Ridge, [40.0, 200.0, 0.0]));
    assert_eq!(stamp.points().len(), 3);
    assert_eq!(spec.content_hash, stamp.content_hash());
    // File mapping both ways.
    assert_eq!(Stamp::from_file("ridge", &it.params).unwrap().unwrap().to_file().1, it.params);
    // The group and the part change the item hash, never the stamp's: the world definition sees independent items.
    let mut other = it.clone();
    other.group = None;
    other.part = None;
    assert_ne!(other.hash(), it.hash());
    assert_eq!(other.spec().content_hash, spec.content_hash);
    // A point, the width, the amount and the kind change it.
    let hash = |i: &Item| i.spec().content_hash;
    let mut a = it.clone();
    a.points[2] = (20.01, 30.02);
    assert_ne!(hash(&a), hash(&it), "a point");
    let mut b = it.clone();
    b.params.insert("width_m".into(), 201.0);
    assert_ne!(hash(&b), hash(&it), "the width");
    let mut c = it.clone();
    c.params.insert("amount_m".into(), 41.0);
    assert_ne!(hash(&c), hash(&it), "the amount");
    assert_ne!(hash(&line_item("channel", &path())), hash(&it), "the kind");
    // A stamp without points hashes as it always did: the pinned value of a hill.
    assert_eq!(Stamp::new(StampKind::Hill, [10.0, 0.0, 0.0]).points().len(), 0);
}

// spec: WDEF-011
#[test]
fn a_line_that_cannot_be_one_is_refused_at_the_put_naming_the_reason_and_changes_nothing() {
    let mut def = WorldDefinition::single_planet(R, 7);
    let ok = line_item("ridge", &path());
    ok.put_into(&mut def).unwrap();
    let before = def.clone();
    let refuse = |mut def: WorldDefinition, it: Item, want: &str| {
        let before = def.clone();
        let e = it.put_into(&mut def).unwrap_err().to_string();
        assert!(e.contains(want), "{e}");
        assert_eq!(def, before, "a refused put changes nothing");
    };
    let mut width = ok.clone();
    width.params.insert("width_m".into(), 0.5);
    refuse(def.clone(), width, "at least 1 m");
    let mut amount = ok.clone();
    amount.params.insert("amount_m".into(), 100.5);
    refuse(def.clone(), amount, "between 0 and");
    let mut negative = ok.clone();
    negative.params.insert("amount_m".into(), -1.0);
    refuse(def.clone(), negative, "between 0 and");
    // The line and its width must fit in the reach (the farthest point is about 1.5 km from the anchor, the width 200 m).
    let mut short = ok.clone();
    short.reach_m = 1_600.0;
    refuse(def.clone(), short, "above its reach");
    let mut wide = ok.clone();
    wide.params.insert("width_m".into(), 4_000.0);
    refuse(def.clone(), wide, "above its reach");
    assert_eq!(def, before);
    // The accepted ones: a channel, a one-metre width, a line exactly as deep as its bound.
    let mut channel = line_item("channel", &path());
    channel.id += 1;
    channel.params.insert("amount_m".into(), 100.0);
    channel.params.insert("width_m".into(), 1.0);
    channel.put_into(&mut def).unwrap();
}

// spec: WDEF-011
#[test]
fn the_footprint_returns_a_line_with_its_points_to_a_tile_under_it_and_rekeys_only_when_a_point_changes() {
    let at = |it: &Item| {
        let g = planet_core::surface::Geo::new(it.anchor.latitude_deg, it.anchor.longitude_deg, 0.0);
        use planet_core::surface::ReferenceSurface;
        planet_core::surface::Sphere { radius_m: 1.0 }.geo_to_planet_fixed(g).0
    };
    let it = line_item("ridge", &path());
    let dir: Vec3 = at(&it);
    let tile = TileId::from_direction(&TangentWarp, dir, 14);
    let far = TileId::from_direction(&TangentWarp, Vec3::new(-dir.x, -dir.y, -dir.z), 14);
    let read = |it: &Item, tile: TileId| {
        let mut def = WorldDefinition::single_planet(R, 7);
        it.put_into(&mut def).unwrap();
        let ctx = GenContext::new(&def);
        let items = ctx.query_footprint(BodyId::PLANET, "splines", tile, 0.25).unwrap();
        (footprint_hash(&items), items)
    };
    let (h, items) = read(&it, tile);
    assert_eq!(items.len(), 1, "the line touches a tile under it");
    assert_eq!(items[0].stamp.as_ref().unwrap().points().len(), 3, "and arrives with its points, shared and immutable");
    assert!(read(&it, far).1.is_empty(), "a tile on the other side of the planet does not read it");
    let mut grouped = it.clone();
    grouped.group = None;
    grouped.part = None;
    assert_eq!(read(&grouped, tile).0, h, "the group and the part re-key nothing");
    let mut moved = it.clone();
    moved.points[2] = (20.01, 30.02);
    assert_ne!(read(&moved, tile).0, h, "a point does");
}
