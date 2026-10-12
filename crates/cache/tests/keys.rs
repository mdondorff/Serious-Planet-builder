//! STRM-003 (ADR 0018): persistent 128-bit keys over a canonical encoding.

use std::collections::{BTreeMap, BTreeSet};

use planet_cache::key::*;
use planet_core::cube::TangentWarp;
use planet_core::hash::HASH_VERSION;
use planet_core::{Face, FaceMapping, TileId, Vec3};
use planet_generators::height::{HeightField, HeightGenerator, Reference, ReferenceGenerator};
use planet_world_def::cells::{cell_of, ItemEntry};
use planet_world_def::{BodyId, GenContext, NodeValue, ReadRecord, WorldDefinition};

const R: f64 = 6_371_000.0;

fn tile() -> TileId {
    TileId::new(Face(2), 5, 3, 7).unwrap()
}

fn base() -> KeyInputs {
    KeyInputs {
        kind: Kind::Height,
        hash_version: 1,
        generator_version: 3,
        implementation_id: "impl-a".into(),
        face_mapping_id: "tangent-v1".into(),
        body: BodyId(0),
        body_params: [7; 16],
        tile: tile(),
        source: None,
        output_params: BTreeMap::from([(PARAM_RESOLUTION, 32u32.to_le_bytes().to_vec())]),
        reads: vec![
            ReadEntry::Node { path: "world.seed".into(), hash: [1; 16] },
            ReadEntry::Node { path: "body.0.radius_m".into(), hash: [2; 16] },
            ReadEntry::Query { kind: QUERY_CELL, params: vec![9, 9], result: [3; 16] },
        ],
    }
}

fn dir(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z).normalized()
}

// spec: STRM-003
#[test]
fn a_key_is_sixteen_bytes_shown_as_thirty_two_hex_characters() {
    let k = cache_key(&base());
    let hex = k.to_hex();
    assert_eq!(hex.len(), 32);
    assert!(hex.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    assert_eq!(CacheKey::from_hex(&hex), Some(k));
    assert_eq!(format!("{k}"), hex);
    for bad in ["", "xyz", &hex[..31], &hex.to_uppercase(), &format!("{hex}0"), &hex.replace(&hex[..1], "g")] {
        assert_eq!(CacheKey::from_hex(bad), None, "{bad}");
    }
}

// spec: STRM-003
#[test]
fn every_input_of_a_key_changes_it_on_its_own() {
    let k0 = cache_key(&base());
    assert_eq!(k0, cache_key(&base()), "deterministic");
    let mut variants: Vec<(&str, KeyInputs)> = Vec::new();
    let mut v = |name: &'static str, f: &dyn Fn(&mut KeyInputs)| {
        let mut b = base();
        f(&mut b);
        variants.push((name, b));
    };
    v("kind", &|b| b.kind = Kind::Mesh);
    v("hash version", &|b| b.hash_version = 2);
    v("generator version", &|b| b.generator_version = 4);
    v("implementation id", &|b| b.implementation_id = "impl-b".into());
    v("face mapping", &|b| b.face_mapping_id = "other".into());
    v("body id", &|b| b.body = BodyId(1));
    v("body parameters", &|b| b.body_params = [8; 16]);
    v("tile", &|b| b.tile = TileId::new(Face(2), 5, 3, 8).unwrap());
    v("source key", &|b| b.source = Some(CacheKey([5; 16])));
    v("a different source key", &|b| b.source = Some(CacheKey([6; 16])));
    v("an output parameter value", &|b| {
        b.output_params.insert(PARAM_RESOLUTION, 64u32.to_le_bytes().to_vec());
    });
    v("another output parameter", &|b| {
        b.output_params.insert(PARAM_SKIRT_FRACTION, vec![1]);
    });
    v("a node hash", &|b| b.reads[0] = ReadEntry::Node { path: "world.seed".into(), hash: [9; 16] });
    v("a node path", &|b| b.reads[0] = ReadEntry::Node { path: "world.seeds".into(), hash: [1; 16] });
    v("a query result", &|b| b.reads[2] = ReadEntry::Query { kind: QUERY_CELL, params: vec![9, 9], result: [4; 16] });
    v("a query parameter", &|b| b.reads[2] = ReadEntry::Query { kind: QUERY_CELL, params: vec![9, 8], result: [3; 16] });
    v("a query kind", &|b| b.reads[2] = ReadEntry::Query { kind: QUERY_BODY, params: vec![9, 9], result: [3; 16] });
    v("an extra read", &|b| b.reads.push(ReadEntry::Node { path: "x".into(), hash: [0; 16] }));
    v("a missing read", &|b| {
        b.reads.pop();
    });
    let mut seen = BTreeSet::from([k0]);
    for (name, inputs) in &variants {
        let k = cache_key(inputs);
        assert_ne!(k, k0, "{name}");
        assert!(seen.insert(k), "{name} collides with another variant");
    }
}

// spec: STRM-003
#[test]
fn the_order_of_parameters_and_reads_does_not_matter_and_duplicate_reads_collapse() {
    let k = cache_key(&base());
    let mut shuffled = base();
    shuffled.reads.reverse();
    shuffled.reads.push(shuffled.reads[0].clone());
    assert_eq!(cache_key(&shuffled), k, "read order and duplicates");
    let mut two = base();
    two.output_params.insert(PARAM_SKIRT_FRACTION, vec![1, 2]);
    let mut other_order = base();
    other_order.output_params = BTreeMap::from([(PARAM_SKIRT_FRACTION, vec![1, 2]), (PARAM_RESOLUTION, 32u32.to_le_bytes().to_vec())]);
    assert_eq!(cache_key(&two), cache_key(&other_order), "output parameters are encoded in ascending tag order");
}

// spec: STRM-003
#[test]
fn the_encoding_is_unambiguous_at_every_boundary() {
    // Text that moves across a field boundary is another input.
    let mut a = base();
    a.implementation_id = "ab".into();
    a.face_mapping_id = "c".into();
    let mut b = base();
    b.implementation_id = "a".into();
    b.face_mapping_id = "bc".into();
    assert_ne!(encode(&a), encode(&b));
    assert_ne!(cache_key(&a), cache_key(&b));
    // A parameter value that looks like the next parameter, a node path that looks like a query.
    let mut c = base();
    c.output_params = BTreeMap::from([(1, vec![2, 0, 0, 0, 7])]);
    let mut d = base();
    d.output_params = BTreeMap::from([(1, vec![2]), (7, vec![])]);
    assert_ne!(encode(&c), encode(&d));
    // A generated sample of inputs: every encoding and every key is distinct.
    let mut encodings = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut n = 0;
    for gen_v in 0..4u32 {
        for imp in ["", "a", "ab", "ba"] {
            for map in ["", "a", "tangent-v1"] {
                for body in 0..3u32 {
                    for t in 0..4u32 {
                        for res in [8u32, 16] {
                            for reads in 0..3usize {
                                let mut i = base();
                                i.generator_version = gen_v;
                                i.implementation_id = imp.into();
                                i.face_mapping_id = map.into();
                                i.body = BodyId(body);
                                i.tile = TileId::new(Face(1), 3, t, t + 1).unwrap();
                                i.output_params = BTreeMap::from([(PARAM_RESOLUTION, res.to_le_bytes().to_vec())]);
                                i.reads = (0..reads).map(|r| ReadEntry::Node { path: format!("n{r}"), hash: [r as u8; 16] }).collect();
                                encodings.insert(encode(&i));
                                keys.insert(cache_key(&i));
                                n += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!((encodings.len(), keys.len()), (n, n), "{n} distinct inputs must give {n} encodings and keys");
}

// spec: STRM-003
#[test]
fn hashes_of_different_uses_are_in_different_domains() {
    // The same bytes under the key context are not the plain BLAKE3 hash, and the three hash kinds differ even when their
    // material coincides in size.
    let plain = blake3::hash(&encode(&base()));
    assert_ne!(cache_key(&base()).0[..], plain.as_bytes()[..16]);
    let node = node_hash128("", NodeValue::U64(0));
    let body = body_params_hash128(BodyId(0), 0, 0.0, "");
    let query = query_result_hash128(&[]);
    assert!(node != body && body != query && node != query);
    // The empty query result is the fixed hash of the empty input.
    assert_eq!(query, query_result_hash128(&[]));
    let one: ItemEntry = (1, 2, [3, 4, 5]);
    assert_ne!(query, query_result_hash128(&[one]));
    // Node hashes follow the exact bits: 0.0 and -0.0 differ, an integer and the float of the same bits differ.
    assert_ne!(node_hash128("x", NodeValue::F64(0.0)), node_hash128("x", NodeValue::F64(-0.0)));
    assert_ne!(node_hash128("x", NodeValue::U64(1.0f64.to_bits())), node_hash128("x", NodeValue::F64(1.0)));
    assert_ne!(node_hash128("x", NodeValue::U64(1)), node_hash128("y", NodeValue::U64(1)));
}

// spec: STRM-003
#[test]
fn a_query_that_found_nothing_is_still_part_of_the_key_and_an_item_elsewhere_is_not() {
    let near = dir(1.0, 0.2, 0.1);
    let far = dir(-1.0, 0.3, 0.2);
    let key_after = |def: &WorldDefinition| {
        let ctx = GenContext::new(def);
        ctx.body_seed(BodyId::PLANET).unwrap();
        ctx.query_cell(BodyId::PLANET, cell_of(R, near)).unwrap();
        let mut i = base();
        i.reads = ctx.read_records().iter().map(read_entry).collect();
        cache_key(&i)
    };
    let plain = WorldDefinition::single_planet(R, 7);
    let mut elsewhere = plain.clone();
    elsewhere.put_item(BodyId::PLANET, 1, far, 5).unwrap();
    let mut here = plain.clone();
    here.put_item(BodyId::PLANET, 1, near, 5).unwrap();
    assert_eq!(key_after(&plain), key_after(&elsewhere));
    assert_ne!(key_after(&plain), key_after(&here), "adding an item to a queried, empty cell changes the key");
    let mut edited = here.clone();
    edited.put_item(BodyId::PLANET, 1, near, 6).unwrap();
    assert_ne!(key_after(&here), key_after(&edited));
    let mut moved_within = here.clone();
    moved_within.put_item(BodyId::PLANET, 1, dir(1.0, 0.2000001, 0.1), 5).unwrap();
    assert_ne!(key_after(&here), key_after(&moved_within), "a move inside the cell");
    edited.remove_item(BodyId::PLANET, 1);
    assert_eq!(key_after(&plain), key_after(&edited));
}

// spec: STRM-003
#[test]
fn the_keys_of_a_planet_a_moon_and_a_mesh_follow_their_body_their_source_and_their_parameters() {
    let planet = Reference(7);
    let hk = |f: &dyn HeightField, res: u32, r: f64| height_key(f, &TangentWarp, tile(), res, r);
    let base_key = hk(&planet, 32, R);
    assert_eq!(base_key, hk(&planet, 32, R));
    assert_ne!(base_key, hk(&planet, 16, R), "resolution");
    assert_ne!(base_key, hk(&planet, 32, R + 1.0), "radius (body parameters)");
    assert_ne!(base_key, hk(&Reference(8), 32, R), "seed");
    assert_ne!(base_key, height_key(&planet, &TangentWarp, TileId::new(Face(2), 5, 3, 8).unwrap(), 32, R), "tile");

    // A moon built through the generator has another body id, seed and read set.
    let mut def = WorldDefinition::single_planet(R, 7);
    def.set_body_radius(BodyId(1), 1_737_400.0);
    let moon = ReferenceGenerator.field(&GenContext::new(&def), BodyId(1)).unwrap();
    let on_planet = ReferenceGenerator.field(&GenContext::new(&def), BodyId(0)).unwrap();
    assert_ne!(hk(moon.as_ref(), 32, 1_737_400.0), base_key);
    // The generator-built planet reads the same things as a field built directly from the seed.
    assert_eq!(hk(on_planet.as_ref(), 32, R), base_key);

    // The mesh key chains the height key of its tile and adds its own parameters.
    let mk = |f: &dyn HeightField, cells: u32, r: f64| mesh_key(f, &TangentWarp, tile(), cells, r);
    let mesh = mk(&planet, 32, R);
    assert_ne!(mesh, base_key, "a mesh key is not its height key");
    assert_ne!(mesh, mk(&planet, 16, R), "grid size");
    assert_ne!(mesh, mk(&Reference(8), 32, R), "the source height key");
    assert_ne!(mesh, mk(&planet, 32, R + 1.0), "radius");
    assert_ne!(mk(moon.as_ref(), 32, 1_737_400.0), mesh);
}

// spec: STRM-003
#[test]
fn known_answers_pin_the_encoding_and_the_hashes_on_every_platform() {
    assert_eq!(KEY_FORMAT_TAG, 2, "a new encoding needs a new tag and new known answers");
    assert_eq!(HASH_VERSION, 1, "known answers belong to this hash version");
    assert_eq!(cache_key(&base()).to_hex(), KNOWN_BASE);
    let planet = Reference(7);
    assert_eq!(height_key(&planet, &TangentWarp, tile(), 32, R).to_hex(), KNOWN_HEIGHT);
    assert_eq!(mesh_key(&planet, &TangentWarp, tile(), 32, R).to_hex(), KNOWN_MESH);
    assert_eq!(hex(node_hash128("world.seed", NodeValue::U64(7))), KNOWN_NODE);
    assert_eq!(hex(body_params_hash128(BodyId(1), 9, 1_737_400.0, "sphere")), KNOWN_BODY);
    assert_eq!(hex(query_result_hash128(&[])), KNOWN_EMPTY_QUERY);
}

fn hex(b: [u8; 16]) -> String {
    CacheKey(b).to_hex()
}

const KNOWN_BASE: &str = "b00fa2b4b0f9accc87be74b4dd9d40ca";
const KNOWN_HEIGHT: &str = "340480159ae2ca791db08d1cae1232ce";
const KNOWN_MESH: &str = "2882ad3a00b9418fabe0cd72e655440c"; // mesh version 4 (coarse_height)
const KNOWN_NODE: &str = "dd934e9e858a828bb73c45e8a4eb4d45";
const KNOWN_BODY: &str = "91f0cebc78907779e2f87196650d468e";
const KNOWN_EMPTY_QUERY: &str = "a97668a9d9614a3f4a73f2fceae97219";

// spec: STRM-003
#[test]
fn the_dependency_is_the_official_blake3() {
    // Independent of this crate's encoding: the official BLAKE3 hash of the empty input.
    assert_eq!(blake3::hash(b"").to_hex().as_str(), "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262");
}

/// `Reference(7)` that reads the items under each tile's footprint, as a stamp generator would. It overrides only
/// `reads_cells` and `read_records_for`: the 64-bit per-tile hash comes from the same records.
struct Stamped<'a> {
    inner: Reference,
    def: &'a WorldDefinition,
}

impl HeightField for Stamped<'_> {
    fn height(&self, d: Vec3) -> f64 {
        self.inner.height(d)
    }
    fn seed(&self) -> u64 {
        self.inner.seed()
    }
    fn implementation_id(&self) -> &'static str {
        "stamped-test"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        self.inner.max_abs_height_m()
    }
    fn reads_cells(&self) -> bool {
        true
    }
    fn read_records_for(&self, tile: TileId, _map: &dyn FaceMapping) -> Vec<ReadRecord> {
        let ctx = GenContext::new(self.def);
        ctx.body_seed(BodyId::PLANET).unwrap();
        ctx.query_tile(BodyId::PLANET, tile, 1, 64).unwrap();
        ctx.read_records()
    }
}

/// A field that says it reads cells but reports none: the persistent key refuses to build.
struct Liar;

impl HeightField for Liar {
    fn height(&self, d: Vec3) -> f64 {
        Reference(7).height(d)
    }
    fn seed(&self) -> u64 {
        7
    }
    fn implementation_id(&self) -> &'static str {
        "liar"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        1.0
    }
    fn reads_cells(&self) -> bool {
        true
    }
}

// spec: STRM-003
#[test]
fn an_item_edit_changes_the_persistent_keys_of_exactly_the_tiles_that_read_its_cell() {
    let item_at = dir(1.0, 0.2, 0.1);
    let elsewhere = dir(-1.0, 0.3, 0.2);
    let tiles = [
        TileId::from_direction(&TangentWarp, item_at, 11),
        TileId::from_direction(&TangentWarp, item_at, 7),
        TileId::from_direction(&TangentWarp, elsewhere, 11),
    ];
    let keys = |d: &WorldDefinition| {
        let f = Stamped { inner: Reference(7), def: d };
        tiles.map(|t| (height_key(&f, &TangentWarp, t, 16, R), mesh_key(&f, &TangentWarp, t, 16, R), f.inputs_hash_for(t, &TangentWarp)))
    };
    let plain = WorldDefinition::single_planet(R, 7);
    let mut edited = plain.clone();
    edited.put_item(BodyId::PLANET, 1, item_at, 5).unwrap();
    let (before, after) = (keys(&plain), keys(&edited));
    for i in 0..3 {
        let affected = i < 2;
        assert_eq!(before[i].0 != after[i].0, affected, "height key, tile {i}");
        assert_eq!(before[i].1 != after[i].1, affected, "mesh key, tile {i}");
        assert_eq!(before[i].2 != after[i].2, affected, "64-bit per-tile hash, tile {i}: both key paths follow the same reads");
    }
    edited.remove_item(BodyId::PLANET, 1);
    assert_eq!(keys(&edited), before);
}

// spec: STRM-003
#[test]
#[should_panic(expected = "a field that reads cells must report its cell queries in read_records_for")]
fn a_field_that_reads_cells_but_reports_none_cannot_be_keyed() {
    let _ = height_key(&Liar, &TangentWarp, tile(), 16, R);
}

// spec: STRM-003
#[test]
fn the_radius_of_a_key_must_be_the_finite_positive_radius_of_the_definition() {
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let r = std::panic::catch_unwind(|| height_key(&Reference(7), &TangentWarp, tile(), 16, bad));
        assert!(r.is_err(), "radius {bad}");
    }
}

// spec: STRM-003
#[test]
fn a_missing_source_and_an_all_zero_source_key_are_different_inputs() {
    let none = base();
    let mut zero = base();
    zero.source = Some(CacheKey::NONE);
    assert_ne!(encode(&none), encode(&zero));
    assert_ne!(cache_key(&none), cache_key(&zero));
}

// spec: STRM-003
#[test]
fn a_parameter_that_spells_out_the_next_parameter_is_another_input() {
    // Same number of parameters, same total bytes: one parameter spelling the second one out versus two parameters.
    let mut one = base();
    one.output_params = BTreeMap::from([(1, vec![2, 1, 0, 0, 0, 9]), (9, vec![])]);
    let mut two = base();
    two.output_params = BTreeMap::from([(1, vec![2]), (1 + 1, vec![]), (9, vec![9])]);
    assert_ne!(encode(&one), encode(&two));
    let mut a = base();
    a.output_params = BTreeMap::from([(1, vec![0, 0, 0, 0]), (2, vec![])]);
    let mut b = base();
    b.output_params = BTreeMap::from([(1, vec![]), (2, vec![0, 0, 0, 0])]);
    assert_ne!(encode(&a), encode(&b), "a value moving from one parameter to the next");
}

// spec: STRM-003
#[test]
fn a_cell_record_pins_its_parameter_layout_and_ignores_the_order_of_its_items() {
    let cell = TileId::new(Face(1), 8, 100, 50).unwrap();
    let items: Vec<ItemEntry> = vec![(2, 20, [1, 2, 3]), (1, 10, [4, 5, 6])];
    let reversed: Vec<ItemEntry> = items.iter().rev().copied().collect();
    let a = read_entry(&ReadRecord::Cell { body: BodyId(3), cell, items });
    let b = read_entry(&ReadRecord::Cell { body: BodyId(3), cell, items: reversed });
    assert_eq!(a, b);
    let ReadEntry::Query { kind, params, result } = a else { panic!("a cell read is a query entry") };
    assert_eq!(kind, QUERY_CELL);
    let mut expected = 3u32.to_le_bytes().to_vec();
    expected.extend_from_slice(&cell.raw().to_le_bytes());
    assert_eq!(params, expected, "body u32 LE then cell raw id u64 LE");
    assert_eq!(hex(result), KNOWN_CELL_RESULT);
    let whole = read_entry(&ReadRecord::Body { body: BodyId(3), items: vec![] });
    assert_eq!(whole, ReadEntry::Query { kind: QUERY_BODY, params: 3u32.to_le_bytes().to_vec(), result: query_result_hash128(&[]) });
}

// spec: STRM-003, GEN-016
#[test]
fn a_terrain_height_key_chains_the_bake_it_was_made_from() {
    use planet_generators::global::{bake, GridSource};
    use planet_generators::terrain::TerrainGenerator;
    let def = WorldDefinition::single_planet(R, 7);
    let seed = GenContext::new(&def).body_seed(BodyId::PLANET).unwrap();
    let grid = bake(seed, &TangentWarp, 8, 1).unwrap();
    let src = GridSource(&grid);
    let key_of = |bake_key: [u8; 16]| {
        let f = TerrainGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, bake_key).unwrap();
        height_key(&f, &TangentWarp, tile(), 32, R)
    };
    assert_eq!(key_of([1; 16]), key_of([1; 16]));
    assert_ne!(key_of([1; 16]), key_of([2; 16]), "another bake re-keys the tile");
    // A field without a source (the reference field) keeps the key it had.
    let reference = height_key(&Reference(7), &TangentWarp, tile(), 32, R);
    assert_ne!(reference, key_of([1; 16]));
}

const KNOWN_CELL_RESULT: &str = "b37c2d466779663723ca2943a728a242";
