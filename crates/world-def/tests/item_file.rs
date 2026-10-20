//! Tier A: the item file (AUTH-001, AUTH-005, ADR 0066 point 6): the ULID text, the canonical TOML, the loader that refuses what it does not
//! know, the content hash `planet-builder item hash v1`, and the stores behind the interface.

use planet_core::surface::{Geo, ReferenceSurface, Sphere};
use planet_world_def::item::{make_ulid, parse_ulid, ulid_text, ActiveVersions, Anchor, Item, ITEM_HASH_CONTEXT};
use planet_world_def::item_store::{ItemStore, LooseFileStore, MemoryStore, StoreError};
use planet_world_def::{BodyId, WorldDefinition};
use std::collections::BTreeMap;
use std::path::PathBuf;

const R: f64 = 6_371_000.0;

fn item(id: u128) -> Item {
    Item {
        id,
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "items".into(),
        priority: 3,
        kind: "crater".into(),
        anchor: Anchor { latitude_deg: 48.137154, longitude_deg: 11.576124, height_m: 519.5, heading_deg: 12.25 },
        reach_m: 900.0,
        max_delta_m: 35.5,
        params: BTreeMap::from([("depth_m".to_string(), 24.0), ("radius_m".to_string(), 310.0)]),
        base_hash: 0x0123_4567_89ab_cdef,
        generator_version: 4,
        stage_version: 2,
        name: Some("Marienplatz crater".into()),
        notes: None,
        points: Vec::new(),
        group: None,
        part: None,
        mask_layer: None,
    }
}

// spec: AUTH-005
#[test]
fn the_ulid_text_is_26_crockford_characters_that_sort_as_the_ids_do_and_only_the_canonical_form_parses() {
    assert_eq!(ulid_text(0), "00000000000000000000000000");
    assert_eq!(ulid_text(1), "00000000000000000000000001");
    assert_eq!(ulid_text(31), "0000000000000000000000000Z");
    assert_eq!(ulid_text(32), "00000000000000000000000010");
    assert_eq!(ulid_text(u128::MAX), "7ZZZZZZZZZZZZZZZZZZZZZZZZZ");
    let ids = [0u128, 1, 31, 32, 1 << 64, (1 << 64) + 1, 1 << 100, 0x0123_4567_89ab_cdef_0123_4567_89ab_cdef, u128::MAX];
    for id in ids {
        let text = ulid_text(id);
        assert_eq!(text.len(), 26);
        assert_eq!(parse_ulid(&text).unwrap(), id, "{text}");
    }
    let texts: Vec<String> = ids.iter().map(|i| ulid_text(*i)).collect();
    let mut sorted = texts.clone();
    sorted.sort();
    assert_eq!(texts, sorted, "the text order is the numeric order");
    // The ULID layout: 48 bits of milliseconds, then 80 bits of entropy.
    let id = make_ulid(0x0123_4567_89ab, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    assert_eq!(id >> 80, 0x0123_4567_89ab);
    assert_eq!(id & ((1u128 << 80) - 1), 0x0102_0304_0506_0708_090a);
    assert_eq!(make_ulid(u64::MAX, [0; 10]) >> 80, 0xFFFF_FFFF_FFFF, "the time is taken modulo 2^48");
    // Refused: the wrong length, lower case, look-alike letters, a first character above 7, anything else.
    let good = ulid_text(id);
    for bad in [
        "",
        &good[..25],
        &format!("{good}0"),
        &good.to_lowercase(),
        "8ZZZZZZZZZZZZZZZZZZZZZZZZZ",
        "0000000000000000000000000I",
        "0000000000000000000000000L",
        "0000000000000000000000000O",
        "0000000000000000000000000U",
        "0000000000000000000000000-",
        "000000000000000000000000é0",
    ] {
        assert!(parse_ulid(bad).is_err(), "{bad:?} is refused");
    }
}

// spec: AUTH-001, AUTH-005
#[test]
fn an_item_file_has_a_fixed_key_order_and_round_trips_bit_for_bit() {
    let it = item(make_ulid(1_760_000_000_000, [9, 8, 7, 6, 5, 4, 3, 2, 1, 0]));
    let text = it.to_toml().unwrap();
    let expected = format!(
        "schema_version = 1
id = \"{}\"
body = 0
surface = \"sphere\"
layer = \"items\"
priority = 3
kind = \"crater\"
reach_m = 900.0
max_delta_m = 35.5
base_hash = \"0123456789abcdef\"
generator_version = 4
stage_version = 2
name = \"Marienplatz crater\"

[anchor]
latitude = 48.137154
longitude = 11.576124
height = 519.5
heading = 12.25

[params]
depth_m = 24.0
radius_m = 310.0
",
        ulid_text(it.id)
    );
    assert_eq!(text, expected);
    assert_eq!(Item::from_toml(&text).unwrap(), it);
    // Writing what was read gives the same text.
    assert_eq!(Item::from_toml(&text).unwrap().to_toml().unwrap(), text);
    // The anchor comes back within 1 mm on the surface (it comes back exactly).
    let back = Item::from_toml(&text).unwrap();
    let sphere = Sphere { radius_m: R };
    let a = sphere.geo_to_planet_fixed(Geo::new(it.anchor.latitude_deg, it.anchor.longitude_deg, it.anchor.height_m)).0;
    let b = sphere.geo_to_planet_fixed(Geo::new(back.anchor.latitude_deg, back.anchor.longitude_deg, back.anchor.height_m)).0;
    assert!(a.distance(b) < 1e-3);
    // Floats at the edge of the range and awkward ones come back bit for bit.
    for v in [0.1, 1.0 / 3.0, 1e-300, 5e-324, 1.7976931348623157e308, 123456.789e10, 2.2250738585072014e-308, 4.9e-5, 1e21, 1e-7] {
        let mut x = it.clone();
        x.params.insert("w".into(), v);
        let y = Item::from_toml(&x.to_toml().unwrap()).unwrap();
        assert_eq!(y.params["w"].to_bits(), v.to_bits(), "{v:e}");
    }
    // A string with quotes, backslashes, a newline and non-ASCII text comes back.
    let mut x = it.clone();
    x.name = Some("a \"quoted\" name \\ with\na newline, tab\t, ünïcode and \u{1F30B}".into());
    x.notes = Some(String::new());
    assert_eq!(Item::from_toml(&x.to_toml().unwrap()).unwrap(), x);
}

// spec: AUTH-005
#[test]
fn negative_zero_is_zero_in_the_text_the_parsed_form_and_the_hash() {
    let mut a = item(7);
    a.anchor.height_m = 0.0;
    a.anchor.heading_deg = 0.0;
    a.params.insert("z".into(), 0.0);
    let mut b = a.clone();
    b.anchor.height_m = -0.0;
    b.anchor.heading_deg = -0.0;
    b.params.insert("z".into(), -0.0);
    assert_eq!(a.hash(), b.hash());
    assert_eq!(a.to_toml().unwrap(), b.to_toml().unwrap());
    assert!(!b.to_toml().unwrap().contains("-0.0"));
    let read = Item::from_toml(&b.to_toml().unwrap()).unwrap();
    assert!(read.anchor.height_m.is_sign_positive() && read.params["z"].is_sign_positive());
    // And a file that says -0.0 loads to zero.
    let text = a.to_toml().unwrap().replace("height = 0.0", "height = -0.0");
    assert!(Item::from_toml(&text).unwrap().anchor.height_m.is_sign_positive());
}

/// The material of the content hash, written out independently of the implementation.
fn hash_material(it: &Item) -> Vec<u8> {
    let mut m = Vec::new();
    let s = |m: &mut Vec<u8>, t: &str| {
        m.extend((t.len() as u32).to_le_bytes());
        m.extend(t.bytes());
    };
    let f = |m: &mut Vec<u8>, v: f64| m.extend((if v == 0.0 { 0.0 } else { v }).to_bits().to_le_bytes());
    m.extend(1u32.to_le_bytes());
    m.extend(it.id.to_be_bytes());
    m.extend(it.body.0.to_le_bytes());
    s(&mut m, &it.surface);
    s(&mut m, &it.layer);
    m.extend(it.priority.to_le_bytes());
    s(&mut m, &it.kind);
    for v in [it.anchor.latitude_deg, it.anchor.longitude_deg, it.anchor.height_m, it.anchor.heading_deg] {
        f(&mut m, v);
    }
    f(&mut m, it.reach_m);
    f(&mut m, it.max_delta_m);
    m.extend((it.params.len() as u32).to_le_bytes());
    for (k, v) in &it.params {
        s(&mut m, k);
        f(&mut m, *v);
    }
    m.extend(it.base_hash.to_le_bytes());
    m.extend(it.generator_version.to_le_bytes());
    m.extend(it.stage_version.to_le_bytes());
    m
}

// spec: AUTH-005, WDEF-003
#[test]
fn the_content_hash_is_blake3_128_over_every_field_but_the_name_and_the_notes() {
    let it = item(0x0123_4567_89ab_cdef_0011_2233_4455_6677);
    assert_eq!(ITEM_HASH_CONTEXT, "planet-builder item hash v1");
    let full = blake3::derive_key("planet-builder item hash v1", &hash_material(&it));
    assert_eq!(it.hash(), u128::from_be_bytes(full[..16].try_into().unwrap()), "the layout written out in the test");
    // The name and the notes are not in it, every other field is.
    let mut named = it.clone();
    named.name = Some("another".into());
    named.notes = Some("notes".into());
    assert_eq!(named.hash(), it.hash());
    type Change = Box<dyn Fn(&mut Item)>;
    let changes: Vec<(&str, Change)> = vec![
        ("id", Box::new(|i| i.id += 1)),
        ("body", Box::new(|i| i.body = BodyId(1))),
        ("surface", Box::new(|i| i.surface = "sphere2".into())),
        ("layer", Box::new(|i| i.layer = "other".into())),
        ("priority", Box::new(|i| i.priority += 1)),
        ("kind", Box::new(|i| i.kind = "hill".into())),
        ("latitude", Box::new(|i| i.anchor.latitude_deg += 1e-9)),
        ("longitude", Box::new(|i| i.anchor.longitude_deg += 1e-9)),
        ("height", Box::new(|i| i.anchor.height_m += 1e-9)),
        ("heading", Box::new(|i| i.anchor.heading_deg += 1e-9)),
        ("reach", Box::new(|i| i.reach_m += 1e-9)),
        ("max_delta", Box::new(|i| i.max_delta_m += 1e-9)),
        ("a parameter value", Box::new(|i| *i.params.get_mut("depth_m").unwrap() += 1e-9)),
        (
            "a parameter name",
            Box::new(|i| {
                let _ = i.params.insert("extra".into(), 1.0);
            }),
        ),
        ("base_hash", Box::new(|i| i.base_hash ^= 1)),
        ("generator_version", Box::new(|i| i.generator_version += 1)),
        ("stage_version", Box::new(|i| i.stage_version += 1)),
    ];
    let mut seen = vec![it.hash()];
    for (what, change) in changes {
        let mut x = it.clone();
        change(&mut x);
        let h = x.hash();
        assert!(!seen.contains(&h), "{what} must change the hash, and differently from the others");
        seen.push(h);
    }
}

// spec: AUTH-005, WDEF-003
#[test]
fn a_file_with_other_key_order_comments_spacing_and_integer_literals_parses_to_the_same_item_and_hash() {
    let it = item(make_ulid(1_760_000_000_000, [1; 10]));
    let canonical = it.to_toml().unwrap();
    let id = ulid_text(it.id);
    // Top-level keys come before the tables in TOML; within each part the order, the spacing, the comments and the numeric spelling
    // (an integer literal for a float) are the author's.
    let hand = format!(
        "# an item, edited by hand
stage_version = 2
name = \"Marienplatz crater\"
generator_version = 4
base_hash = \"0123456789abcdef\"
max_delta_m = 35.5   # metres
reach_m = 900
kind = \"crater\"
priority = 3
layer = \"items\"
surface = \"sphere\"
body = 0
id = \"{id}\"
schema_version = 1

[params]
radius_m = 310      # an integer literal for a float
depth_m   =   24.0

[anchor]
heading=12.25
height = 519.5
longitude = 11.576124
latitude = 48.137154
"
    );
    let parsed = Item::from_toml(&hand).unwrap();
    assert_eq!(parsed, it);
    assert_eq!(parsed.hash(), it.hash());
    assert_eq!(parsed.to_toml().unwrap(), canonical, "writing it back gives the canonical text");
}

// spec: AUTH-005
#[test]
fn the_loader_refuses_unknown_keys_missing_keys_wrong_types_non_finite_numbers_and_bad_values_naming_the_key() {
    let it = item(make_ulid(1_760_000_000_000, [2; 10]));
    let good = it.to_toml().unwrap();
    let refuse = |text: String, key: &str| {
        let e = Item::from_toml(&text).expect_err(&format!("refused: {key}"));
        assert_eq!(e.key, key, "{e}");
    };
    // Unknown keys: at the top, in the anchor, and a table of another name.
    refuse(good.replacen("kind =", "colour = \"red\"\nkind =", 1), "colour");
    refuse(good.replace("[anchor]\n", "[anchor]\naltitude = 1.0\n"), "anchor.altitude");
    refuse(format!("{good}\n[extra]\nx = 1.0\n"), "extra");
    // Missing keys: each required one.
    for key in
        ["id", "body", "surface", "layer", "priority", "kind", "reach_m", "max_delta_m", "base_hash", "generator_version", "stage_version"]
    {
        let text: String = good.lines().filter(|l| !l.starts_with(&format!("{key} = "))).collect::<Vec<_>>().join("\n");
        refuse(text, key);
    }
    for key in ["latitude", "longitude", "height", "heading"] {
        let text: String = good.lines().filter(|l| !l.starts_with(&format!("{key} = "))).collect::<Vec<_>>().join("\n");
        refuse(text, &format!("anchor.{key}"));
    }
    refuse(good.replace("[anchor]", "[anchorx]"), "anchorx");
    let no_anchor: String = good.split("\n[anchor]").next().unwrap().to_string();
    refuse(no_anchor, "anchor");
    // The version: missing, newer, older, wrong type.
    refuse(good.replace("schema_version = 1\n", ""), "schema_version");
    refuse(good.replace("schema_version = 1", "schema_version = 2"), "schema_version");
    refuse(good.replace("schema_version = 1", "schema_version = 0"), "schema_version");
    refuse(good.replace("schema_version = 1", "schema_version = \"1\""), "schema_version");
    // Wrong types and values.
    refuse(good.replace("body = 0", "body = \"0\""), "body");
    refuse(good.replace("body = 0", "body = -1"), "body");
    refuse(good.replace("priority = 3", "priority = 3.5"), "priority");
    refuse(good.replace("priority = 3", "priority = 3000000000"), "priority");
    refuse(good.replace("reach_m = 900.0", "reach_m = \"900\""), "reach_m");
    refuse(good.replace("generator_version = 4", "generator_version = -4"), "generator_version");
    refuse(good.replace("name = \"Marienplatz crater\"", "name = 5"), "name");
    // Non-finite numbers, in each place a float lives.
    for lit in ["nan", "inf", "-inf"] {
        refuse(good.replace("reach_m = 900.0", &format!("reach_m = {lit}")), "reach_m");
        refuse(good.replace("max_delta_m = 35.5", &format!("max_delta_m = {lit}")), "max_delta_m");
        refuse(good.replace("latitude = 48.137154", &format!("latitude = {lit}")), "anchor.latitude");
        refuse(good.replace("height = 519.5", &format!("height = {lit}")), "anchor.height");
        refuse(good.replace("depth_m = 24.0", &format!("depth_m = {lit}")), "params.depth_m");
    }
    // A parameter that is not a number, a name outside the allowed characters, an integer too large to be exact.
    refuse(good.replace("depth_m = 24.0", "depth_m = \"deep\""), "params.depth_m");
    refuse(good.replace("depth_m = 24.0", "\"deep m\" = 24.0"), "params.deep m");
    refuse(good.replace("depth_m = 24.0", "depth_m = 9007199254740993"), "params.depth_m");
    // Range checks.
    refuse(good.replace("latitude = 48.137154", "latitude = 90.5"), "anchor.latitude");
    refuse(good.replace("longitude = 11.576124", "longitude = 181.0"), "anchor.longitude");
    refuse(good.replace("reach_m = 900.0", "reach_m = 50000.0"), "reach_m");
    refuse(good.replace("reach_m = 900.0", "reach_m = -1.0"), "reach_m");
    refuse(good.replace("max_delta_m = 35.5", "max_delta_m = -0.5"), "max_delta_m");
    refuse(good.replace("max_delta_m = 35.5", "max_delta_m = 100000.5"), "max_delta_m");
    refuse(good.replace("layer = \"items\"", "layer = \"\""), "layer");
    refuse(good.replace("layer = \"items\"", "layer = \"a b\""), "layer");
    refuse(good.replace("kind = \"crater\"", "kind = \"cra ter\""), "kind");
    // The id: lower case, wrong length, a first character above 7.
    let text = ulid_text(it.id);
    refuse(good.replace(&text, &text.to_lowercase()), "id");
    refuse(good.replace(&text, &text[1..]), "id");
    refuse(good.replace(&text, &format!("8{}", &text[1..])), "id");
    // The base hash: not 16 lower-case hex digits.
    refuse(good.replace("0123456789abcdef", "0123456789ABCDEF"), "base_hash");
    refuse(good.replace("0123456789abcdef", "0123456789abcde"), "base_hash");
    // Not TOML at all, and an empty file.
    refuse("this is not toml".into(), "(file)");
    refuse(String::new(), "schema_version");
    // The writer refuses what the loader would (an invalid item is never written).
    let mut bad = it.clone();
    bad.reach_m = f64::NAN;
    assert!(bad.to_toml().is_err());
    bad = it.clone();
    bad.anchor.latitude_deg = 91.0;
    assert!(bad.to_toml().is_err());
    // A good file still loads (the replacements above each changed one thing).
    assert_eq!(Item::from_toml(&good).unwrap(), it);
}

// spec: AUTH-005, WDEF-005
#[test]
fn an_item_goes_into_the_definition_with_its_hash_and_its_anchor_direction() {
    let it = item(make_ulid(1_760_000_000_000, [3; 10]));
    let mut d = WorldDefinition::single_planet(R, 7);
    let before = d.hash();
    let cell = it.put_into(&mut d).unwrap();
    assert_ne!(d.hash(), before);
    // A stamp kind (the fixture is a crater) enters the index under the stamp's hash, not the file's (WDEF-010).
    assert_eq!(d.items_in_cell(BodyId::PLANET, cell), vec![(it.id, it.spec().content_hash)]);
    assert_ne!(it.spec().content_hash, it.hash());
    let (anchor, reach) = d.item_anchor(BodyId::PLANET, it.id).unwrap();
    assert!((anchor.length() - 1.0).abs() < 1e-12 && reach == 900.0);
    let geo = Sphere { radius_m: 1.0 }.planet_fixed_to_geo(planet_core::PlanetFixed(anchor));
    assert!((geo.lat_deg - it.anchor.latitude_deg).abs() < 1e-9 && (geo.lon_deg - it.anchor.longitude_deg).abs() < 1e-9);
    // The same item put again changes nothing; a changed name changes nothing; a changed parameter changes the hash.
    let after = d.hash();
    it.put_into(&mut d).unwrap();
    let mut renamed = it.clone();
    renamed.name = Some("x".into());
    renamed.put_into(&mut d).unwrap();
    assert_eq!(d.hash(), after);
    let mut tweaked = it.clone();
    tweaked.params.insert("depth_m".into(), 25.0);
    tweaked.put_into(&mut d).unwrap();
    assert_ne!(d.hash(), after);
    tweaked.params.insert("depth_m".into(), 24.0);
    tweaked.put_into(&mut d).unwrap();
    assert_eq!(d.hash(), after, "putting the old item back restores the exact hash");
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("planet-items-{tag}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn conformance(store: &mut dyn ItemStore) {
    assert!(store.load_all(BodyId::PLANET).unwrap().is_empty(), "an empty store lists nothing");
    let mut items: Vec<Item> = (0..5u128).map(|i| item(make_ulid(1_760_000_000_000 + i as u64, [i as u8; 10]))).collect();
    items.reverse();
    for it in &items {
        store.put(it).unwrap();
    }
    let all = store.load_all(BodyId::PLANET).unwrap();
    let mut want = items.clone();
    want.sort_by_key(|i| i.id);
    assert_eq!(all, want, "in id order");
    assert_eq!(store.get(BodyId::PLANET, want[2].id).unwrap().unwrap(), want[2]);
    assert!(store.get(BodyId::PLANET, 12345).unwrap().is_none());
    // Replacing an item keeps one item of that id.
    let mut changed = want[1].clone();
    changed.priority = 9;
    store.put(&changed).unwrap();
    assert_eq!(store.load_all(BodyId::PLANET).unwrap().len(), 5);
    assert_eq!(store.get(BodyId::PLANET, changed.id).unwrap().unwrap().priority, 9);
    // Another body has its own items.
    let mut other = item(make_ulid(1_760_000_000_100, [7; 10]));
    other.body = BodyId(2);
    store.put(&other).unwrap();
    assert_eq!(store.load_all(BodyId(2)).unwrap(), vec![other.clone()]);
    assert_eq!(store.load_all(BodyId::PLANET).unwrap().len(), 5);
    // Deleting.
    assert!(store.delete(BodyId::PLANET, want[0].id).unwrap());
    assert!(!store.delete(BodyId::PLANET, want[0].id).unwrap());
    assert_eq!(store.load_all(BodyId::PLANET).unwrap().len(), 4);
    // An invalid item is refused and nothing is written.
    let mut bad = item(make_ulid(1_760_000_000_200, [8; 10]));
    bad.reach_m = f64::INFINITY;
    assert!(matches!(store.put(&bad), Err(StoreError::Invalid(_))));
    assert!(store.get(BodyId::PLANET, bad.id).unwrap().is_none());
}

// spec: AUTH-001
#[test]
fn both_stores_keep_items_by_body_and_id_in_id_order() {
    conformance(&mut MemoryStore::new());
    let dir = temp_dir("conformance");
    conformance(&mut LooseFileStore::new(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

// spec: AUTH-001
#[test]
fn the_loose_store_is_one_toml_file_per_item_and_refuses_what_is_not_one() {
    let dir = temp_dir("loose");
    let mut store = LooseFileStore::new(&dir);
    let it = item(make_ulid(1_760_000_000_000, [5; 10]));
    store.put(&it).unwrap();
    let path = dir.join("0").join(format!("{}.toml", ulid_text(it.id)));
    assert_eq!(store.path_of(BodyId::PLANET, it.id), path);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), it.to_toml().unwrap(), "the file is the canonical text");
    let names: Vec<String> =
        std::fs::read_dir(dir.join("0")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert_eq!(names, [format!("{}.toml", ulid_text(it.id))], "no temporary file is left behind");
    // A hidden file is not an item.
    std::fs::write(dir.join("0").join(".gitkeep"), "").unwrap();
    assert_eq!(store.load_all(BodyId::PLANET).unwrap().len(), 1);
    // A file whose name is not a ULID, one that is not `.toml`, one named for another id, and one in the directory of another body.
    let stray = dir.join("0").join("notes.txt");
    std::fs::write(&stray, "hello").unwrap();
    assert!(matches!(store.load_all(BodyId::PLANET), Err(StoreError::Misplaced { .. })));
    std::fs::remove_file(&stray).unwrap();
    let wrong_name = dir.join("0").join(format!("{}.toml", ulid_text(it.id + 1)));
    std::fs::copy(&path, &wrong_name).unwrap();
    let e = store.load_all(BodyId::PLANET).unwrap_err();
    assert!(matches!(e, StoreError::Misplaced { .. }) && e.to_string().contains("named for"), "{e}");
    assert!(store.get(BodyId::PLANET, it.id + 1).is_err());
    std::fs::remove_file(&wrong_name).unwrap();
    std::fs::create_dir_all(dir.join("1")).unwrap();
    std::fs::copy(&path, dir.join("1").join(format!("{}.toml", ulid_text(it.id)))).unwrap();
    let e = store.load_all(BodyId(1)).unwrap_err();
    assert!(e.to_string().contains("directory of body 1"), "{e}");
    // A file that is not an item names the file.
    std::fs::write(&path, "schema_version = 1\n").unwrap();
    let e = store.load_all(BodyId::PLANET).unwrap_err();
    assert!(matches!(e, StoreError::BadFile { .. }) && e.to_string().contains(&ulid_text(it.id)), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

// spec: AUTH-002
#[test]
fn an_item_authored_against_other_versions_is_refused_naming_both_and_the_options() {
    let it = item(make_ulid(1_760_000_000_000, [4; 10]));
    let active = ActiveVersions { generator_version: 4, stage_version: 2, base_hash: 0x0123_4567_89ab_cdef };
    assert_eq!(it.check_versions(&active), Ok(()), "the versions it records are the active ones");
    let e = it.check_versions(&ActiveVersions { generator_version: 5, ..active }).unwrap_err();
    let text = e.to_string();
    assert!(text.contains("generator version 4 (active 5)"), "{text}");
    assert!(text.contains("pin") && text.contains("freeze") && text.contains("re-author"), "{text}");
    assert!(text.contains(&ulid_text(it.id)), "{text}");
    // An older stage version is not a mismatch: the item loads and is re-keyed (AUTH-002, `restaged`); only a newer one is refused.
    let newer_stage = ActiveVersions { stage_version: 3, ..active };
    assert_eq!(it.check_versions(&newer_stage), Ok(()), "an item of an older stage loads");
    assert!(it.is_older_stage(&newer_stage) && !it.is_older_stage(&active));
    let re = it.clone().restaged(&newer_stage);
    assert_eq!(re.stage_version, 3);
    assert_ne!(re.hash(), it.hash(), "the recorded version is in the item hash");
    assert_eq!(re.spec().content_hash, it.spec().content_hash, "the stamp it makes does not change");
    let e = it.check_versions(&ActiveVersions { stage_version: 1, ..active }).unwrap_err();
    assert!(e.to_string().contains("stage version 2 (active 1") && e.to_string().contains("newer"), "{e}");
    let e = it.check_versions(&ActiveVersions { base_hash: 0xfeed, ..active }).unwrap_err();
    assert!(e.to_string().contains("base hash 0123456789abcdef (active 000000000000feed)"), "{e}");
    // Every difference is named.
    let e = it.check_versions(&ActiveVersions { generator_version: 9, stage_version: 1, base_hash: 1 }).unwrap_err();
    assert_eq!(e.differences.len(), 3);
}

// spec: AUTH-005
#[test]
fn a_place_has_one_spelling_the_antimeridian_the_poles_and_the_heading_have_one_range() {
    let it = item(make_ulid(1_760_000_000_000, [6; 10]));
    let with = |f: &dyn Fn(&mut Item)| {
        let mut x = it.clone();
        f(&mut x);
        x
    };
    // Longitude is in (-180, 180]: 180 is the antimeridian, -180 is refused (the same meridian), and the cell of 180 is the one of its place.
    assert!(with(&|i| i.anchor.longitude_deg = 180.0).validate().is_ok());
    let e = with(&|i| i.anchor.longitude_deg = -180.0).validate().unwrap_err();
    assert_eq!(e.key, "anchor.longitude");
    assert!(with(&|i| i.anchor.longitude_deg = -179.999999).validate().is_ok());
    // At a pole every longitude is the same point: only 0 is accepted.
    for lat in [90.0, -90.0] {
        assert!(with(&|i| (i.anchor.latitude_deg, i.anchor.longitude_deg) = (lat, 0.0)).validate().is_ok());
        for lon in [45.0, -135.0, 180.0, 1e-9] {
            let e = with(&|i| (i.anchor.latitude_deg, i.anchor.longitude_deg) = (lat, lon)).validate().unwrap_err();
            assert_eq!(e.key, "anchor.longitude", "latitude {lat}, longitude {lon}");
        }
    }
    // The heading is in [0, 360): 360 and -1 are another spelling of an orientation.
    assert!(with(&|i| i.anchor.heading_deg = 359.999).validate().is_ok());
    for bad in [360.0, -1.0, 720.0] {
        assert_eq!(with(&|i| i.anchor.heading_deg = bad).validate().unwrap_err().key, "anchor.heading", "{bad}");
    }
    // The pole item goes into the definition at the pole.
    let mut d = WorldDefinition::single_planet(R, 7);
    with(&|i| (i.anchor.latitude_deg, i.anchor.longitude_deg) = (90.0, 0.0)).put_into(&mut d).unwrap();
    let (anchor, _) = d.item_anchor(BodyId::PLANET, it.id).unwrap();
    assert!(anchor.z > 1.0 - 1e-12);
}

// spec: AUTH-005
#[test]
fn putting_into_the_definition_refuses_what_the_file_would_refuse() {
    let it = item(make_ulid(1_760_000_000_000, [7; 10]));
    let mut d = WorldDefinition::single_planet(R, 7);
    let hash = d.hash();
    let mut bad = it.clone();
    bad.anchor.latitude_deg = 95.0;
    assert!(matches!(bad.put_into(&mut d), Err(planet_world_def::item::PutError::Invalid(e)) if e.key == "anchor.latitude"));
    let mut bad = it.clone();
    bad.layer = "a b".into();
    assert!(matches!(bad.put_into(&mut d), Err(planet_world_def::item::PutError::Invalid(e)) if e.key == "layer"));
    let mut bad = it.clone();
    bad.params.insert("x".into(), f64::NAN);
    assert!(matches!(bad.put_into(&mut d), Err(planet_world_def::item::PutError::Invalid(e)) if e.key == "params.x"));
    let mut elsewhere = it.clone();
    elsewhere.body = BodyId(9);
    assert!(matches!(elsewhere.put_into(&mut d), Err(planet_world_def::item::PutError::Index(_))));
    assert_eq!(d.hash(), hash, "a refused item leaves the definition as it was");
}

// spec: AUTH-005
#[test]
fn floats_are_written_in_the_text_of_the_current_formatter_pinned_so_a_toolchain_change_shows() {
    let mut it = item(make_ulid(1_760_000_000_000, [8; 10]));
    let cases: [(f64, &str); 9] = [
        (1e21, "1e21"),
        (1e-7, "1e-7"),
        (1e15, "1000000000000000.0"),
        (1e16, "1e16"),
        (5e-324, "5e-324"),
        (0.1, "0.1"),
        (1.0 / 3.0, "0.3333333333333333"),
        (24.0, "24.0"),
        (1.7976931348623157e308, "1.7976931348623157e308"),
    ];
    for (v, text) in cases {
        it.params.insert("w".into(), v);
        let file = it.to_toml().unwrap();
        assert!(file.lines().any(|l| l == format!("w = {text}")), "{v:e} is written as {text}: {file}");
        assert_eq!(Item::from_toml(&file).unwrap().params["w"].to_bits(), v.to_bits());
    }
}

// spec: AUTH-005
#[test]
fn negative_zero_is_zero_in_every_number_of_an_item() {
    let mut a = item(9);
    // A plain item: a reach of 0 is a stamp's refusal (WDEF-010) and this test is about the sign of zero.
    a.kind = "marker".into();
    a.anchor.latitude_deg = 0.0;
    a.anchor.longitude_deg = 0.0;
    a.anchor.height_m = 0.0;
    a.anchor.heading_deg = 0.0;
    a.reach_m = 0.0;
    a.max_delta_m = 0.0;
    let mut b = a.clone();
    b.anchor.latitude_deg = -0.0;
    b.anchor.longitude_deg = -0.0;
    b.anchor.height_m = -0.0;
    b.anchor.heading_deg = -0.0;
    b.reach_m = -0.0;
    b.max_delta_m = -0.0;
    assert_eq!(a.hash(), b.hash());
    assert_eq!(a.to_toml().unwrap(), b.to_toml().unwrap());
    // The same from a file that says `-0.0` in each place.
    let text = a
        .to_toml()
        .unwrap()
        .replace("latitude = 0.0", "latitude = -0.0")
        .replace("longitude = 0.0", "longitude = -0.0")
        .replace("reach_m = 0.0", "reach_m = -0.0")
        .replace("max_delta_m = 0.0", "max_delta_m = -0.0");
    let read = Item::from_toml(&text).unwrap();
    assert_eq!(read.hash(), a.hash());
    assert!([read.anchor.latitude_deg, read.anchor.longitude_deg, read.reach_m, read.max_delta_m].iter().all(|v| v.is_sign_positive()));
    // The anchor of an item at -0.0 is in the cell of 0.0.
    let (mut da, mut db) = (WorldDefinition::single_planet(R, 7), WorldDefinition::single_planet(R, 7));
    assert_eq!(a.put_into(&mut da).unwrap(), b.put_into(&mut db).unwrap());
    assert_eq!(da.hash(), db.hash());
}

// spec: AUTH-005
#[test]
fn a_pinned_item_has_a_pinned_canonical_text_and_content_hash() {
    let it = Item {
        id: 0x018F_2B4C_5D6E_0123_4567_89AB_CDEF_0123,
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "stamps".into(),
        priority: -2,
        kind: "hill".into(),
        anchor: Anchor { latitude_deg: -33.8688, longitude_deg: 151.2093, height_m: 58.25, heading_deg: 270.5 },
        reach_m: 1500.0,
        max_delta_m: 80.0,
        params: BTreeMap::from([("height_m".to_string(), 120.0), ("radius_m".to_string(), 750.5)]),
        base_hash: 0xfedc_ba98_7654_3210,
        generator_version: 7,
        stage_version: 1,
        name: Some("A hill".into()),
        notes: Some("pinned".into()),
        points: Vec::new(),
        group: None,
        part: None,
        mask_layer: None,
    };
    let text = it.to_toml().unwrap();
    assert_eq!(
        text,
        "schema_version = 1
id = \"01HWNMRQBE04HMASW9NF6YY093\"
body = 0
surface = \"sphere\"
layer = \"stamps\"
priority = -2
kind = \"hill\"
reach_m = 1500.0
max_delta_m = 80.0
base_hash = \"fedcba9876543210\"
generator_version = 7
stage_version = 1
name = \"A hill\"
notes = \"pinned\"

[anchor]
latitude = -33.8688
longitude = 151.2093
height = 58.25
heading = 270.5

[params]
height_m = 120.0
radius_m = 750.5
"
    );
    assert_eq!(format!("{:032x}", it.hash()), "8c2bde9c2d67a1be7f97b85a446a980d", "the pinned content hash");
}

// spec: AUTH-005
#[test]
fn a_file_without_a_params_table_has_no_parameters_and_an_unknown_surface_is_refused() {
    let mut it = item(make_ulid(1_760_000_000_000, [9; 10]));
    // A plain item: a crater without its parameter is refused (WDEF-010).
    it.kind = "marker".into();
    it.params.clear();
    let text = it.to_toml().unwrap();
    let without = text.replace("\n[params]\n", "");
    assert_ne!(without, text);
    assert_eq!(Item::from_toml(&without).unwrap(), it, "a missing [params] is an item without parameters");
    assert_eq!(Item::from_toml(&text).unwrap(), it, "and an empty one is the same");
    let mut other = it.clone();
    other.surface = "ellipsoid".into();
    assert_eq!(other.validate().unwrap_err().key, "surface");
    assert!(other.to_toml().is_err());
    assert!(Item::from_toml(&text.replace("surface = \"sphere\"", "surface = \"ellipsoid\"")).is_err());
}

// spec: AUTH-001
#[test]
fn a_failed_write_leaves_no_temporary_file_behind() {
    let dir = temp_dir("failed-write");
    let mut store = LooseFileStore::new(&dir);
    let it = item(make_ulid(1_760_000_000_000, [10; 10]));
    // The place of the file is a directory: the move into place fails.
    std::fs::create_dir_all(store.path_of(BodyId::PLANET, it.id)).unwrap();
    let e = store.put(&it).unwrap_err();
    assert!(matches!(e, StoreError::Io { what: "moving into place", .. }), "{e}");
    let names: Vec<String> =
        std::fs::read_dir(dir.join("0")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert_eq!(names, [format!("{}.toml", ulid_text(it.id))], "only the directory that was there: {names:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
