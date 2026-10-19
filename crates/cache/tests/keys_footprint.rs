//! STRM-003, WDEF-007 (ADR 0018, ADR 0066 point 2): persistent keys over footprint reads (query kind 3).

use std::collections::BTreeMap;

use planet_cache::key::*;
use planet_core::cube::TangentWarp;
use planet_core::{Face, FaceMapping, TileId, Vec3};
use planet_generators::height::{HeightField, Reference};
use planet_world_def::footprint::FootprintItem;
use planet_world_def::{BodyId, GenContext, ItemSpec, ReadRecord, WorldDefinition};

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
            ReadEntry::Query { kind: QUERY_CELL, params: vec![9, 9], result: [3; 16] },
        ],
    }
}

fn dir(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z).normalized()
}

fn hex(b: [u8; 16]) -> String {
    CacheKey(b).to_hex()
}

/// `Reference(7)` that reads the items of a layer under each tile's footprint (query kind 3), as a stamp generator would.
struct Footprinted<'a> {
    inner: Reference,
    def: &'a WorldDefinition,
}

impl HeightField for Footprinted<'_> {
    fn height(&self, d: Vec3) -> f64 {
        self.inner.height(d)
    }
    fn seed(&self) -> u64 {
        self.inner.seed()
    }
    fn implementation_id(&self) -> &'static str {
        "footprinted-test"
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
        ctx.query_footprint(BodyId::PLANET, "items", tile, 0.25).unwrap();
        ctx.read_records()
    }
}

fn stamp(id: u64, at: Vec3, reach_m: f64) -> ItemSpec {
    ItemSpec { id, layer: "items".into(), priority: 0, dir: at, content_hash: 5, reach_m, max_delta_m: 20.0 }
}

// spec: STRM-003, WDEF-007
#[test]
fn a_footprint_edit_changes_the_keys_of_the_tiles_its_reach_touches_and_no_others() {
    let at = dir(1.0, 0.2, 0.1);
    // A tile at the item, one 30 km away that the item's 150 m reach does not touch (yet inside the margin ring of cells), and one far away.
    let near = TileId::from_direction(&TangentWarp, at, 12);
    let ring = TileId::from_direction(&TangentWarp, dir(1.0, 0.2 + 30_000.0 / R, 0.1), 12);
    let far = TileId::from_direction(&TangentWarp, dir(-1.0, 0.3, 0.2), 12);
    let tiles = [near, ring, far];
    let keys = |d: &WorldDefinition| {
        let f = Footprinted { inner: Reference(7), def: d };
        tiles.map(|t| (height_key(&f, &TangentWarp, t, 16, R), mesh_key(&f, &TangentWarp, t, 16, R), f.inputs_hash_for(t, &TangentWarp)))
    };
    let plain = WorldDefinition::single_planet(R, 7);
    let mut edited = plain.clone();
    edited.put_item_spec(BodyId::PLANET, stamp(1, at, 150.0)).unwrap();
    let (before, after) = (keys(&plain), keys(&edited));
    for i in 0..3 {
        let affected = i == 0;
        assert_eq!(before[i].0 != after[i].0, affected, "height key, tile {i}");
        assert_eq!(before[i].1 != after[i].1, affected, "mesh key, tile {i}");
        assert_eq!(before[i].2 != after[i].2, affected, "64-bit per-tile hash, tile {i}: both key paths follow the same reads");
    }
    edited.remove_item(BodyId::PLANET, 1);
    assert_eq!(keys(&edited), before, "removing the item restores every key");
}

// spec: STRM-003, WDEF-008
#[test]
fn a_coarse_tile_keys_on_summary_nodes_with_tag_3_and_an_item_far_away_leaves_its_key_alone() {
    let at = dir(1.0, 0.2, 0.1);
    let coarse = TileId::from_direction(&TangentWarp, at, 4);
    let plain = WorldDefinition::single_planet(R, 7);
    let mut near = plain.clone();
    near.put_item_spec(BodyId::PLANET, stamp(1, at, 150.0)).unwrap();
    let mut far = plain.clone();
    far.put_item_spec(BodyId::PLANET, stamp(2, dir(-1.0, -0.3, -0.2), 150.0)).unwrap();
    let key = |d: &WorldDefinition| {
        let f = Footprinted { inner: Reference(7), def: d };
        height_key(&f, &TangentWarp, coarse, 16, R)
    };
    assert_ne!(key(&plain), key(&near), "an item under the coarse tile re-keys it");
    assert_eq!(key(&plain), key(&far), "an item on the other side of the planet does not");
    let f = Footprinted { inner: Reference(7), def: &near };
    let inputs = mesh_key_inputs(&f, &TangentWarp, coarse, 16, R);
    assert!(inputs.reads.iter().any(|r| matches!(r, ReadEntry::Query { kind: QUERY_SUMMARY, .. })), "region summary entries (kind 4)");
    assert!(!inputs.reads.iter().any(|r| matches!(r, ReadEntry::Query { kind: QUERY_FOOTPRINT, .. })), "no footprint result entry");
    assert_eq!(encode(&inputs)[0], KEY_FORMAT_TAG_FOOTPRINT);
    // The parameters are `body | layer | level | node raw id`.
    let node = TileId::from_direction(&TangentWarp, at, 4);
    let p = summary_params(BodyId(3), "items", node);
    assert_eq!(p.len(), 4 + 4 + 5 + 1 + 8);
    assert_eq!(p[13], 4, "the level byte");
}

// spec: STRM-003
#[test]
fn a_key_without_a_footprint_entry_keeps_tag_2_byte_for_byte_and_one_with_it_starts_with_tag_3() {
    assert_eq!(KEY_FORMAT_TAG, 2);
    assert_eq!(KEY_FORMAT_TAG_FOOTPRINT, 3);
    let plain = base();
    assert_eq!(encode(&plain)[0], KEY_FORMAT_TAG, "today's keys are unchanged (the known answers in keys.rs pin their bytes)");
    for kind in [QUERY_FOOTPRINT, QUERY_SUMMARY] {
        let mut with = base();
        with.reads.push(ReadEntry::Query { kind, params: vec![1], result: [4; 16] });
        let bytes = encode(&with);
        assert_eq!(bytes[0], KEY_FORMAT_TAG_FOOTPRINT, "kind {kind}");
        assert_ne!(cache_key(&with), cache_key(&plain));
    }
    // A cell or body entry alone does not change the tag.
    let mut body_only = base();
    body_only.reads.push(ReadEntry::Query { kind: QUERY_BODY, params: vec![0], result: [4; 16] });
    assert_eq!(encode(&body_only)[0], KEY_FORMAT_TAG);
    // The same remaining bytes under the other tag are another encoding, so the two layouts never alias.
    let mut with = base();
    with.reads.push(ReadEntry::Query { kind: QUERY_FOOTPRINT, params: vec![1], result: [4; 16] });
    let bytes = encode(&with);
    let mut as_old = bytes.clone();
    as_old[0] = KEY_FORMAT_TAG;
    assert_ne!(as_old, bytes);
}

// spec: STRM-003, WDEF-007
#[test]
fn the_footprint_result_is_in_application_order_with_sixteen_byte_ids_and_pinned_hashes() {
    let item = |id: u64, priority: i32| FootprintItem {
        id,
        priority,
        content: 5,
        dir: [1, 2, 3],
        reach_bits: 150.0f64.to_bits(),
        max_delta_bits: 20.0f64.to_bits(),
        layer: "items".into(),
    };
    assert_eq!(footprint_id_bytes(0x0102_0304_0506_0708), [0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8], "zero-extended, big-endian");
    let a = footprint_result_hash128(&[item(1, 5), item(2, 1)]);
    assert_eq!(a, footprint_result_hash128(&[item(2, 1), item(1, 5)]), "the input order does not matter");
    assert_ne!(a, footprint_result_hash128(&[item(1, 1), item(2, 5)]), "priorities decide the application order");
    assert_ne!(footprint_item_hash128(&item(1, 5)), footprint_item_hash128(&item(1, 6)), "priority is in the item hash");
    let mut moved = item(1, 5);
    moved.dir = [1, 2, 4];
    assert_ne!(footprint_item_hash128(&item(1, 5)), footprint_item_hash128(&moved), "the anchor is in the item hash");
    let p = footprint_params(BodyId(3), "items", tile(), 0.25f64.to_bits());
    assert_eq!(p.len(), 4 + 4 + 5 + 8 + 8, "body, layer length and bytes, tile, halo");
    assert_eq!(&p[..4], &3u32.to_le_bytes());
    assert_ne!(hex(footprint_result_hash128(&[])), hex(query_result_hash128(&[])), "a footprint result never equals a cell result");
    assert_eq!(hex(footprint_result_hash128(&[])), KNOWN_EMPTY_FOOTPRINT);
    assert_eq!(hex(footprint_item_hash128(&item(1, 5))), KNOWN_FOOTPRINT_ITEM);
    assert_eq!(hex(a), KNOWN_FOOTPRINT_RESULT);
}

/// A stamp generator for the contract test: the base `Reference(7)` plus, for every item of the definition within its reach of
/// the point, the bump `A (1 - (d / r)^2)^2` with `A = max_delta_m` and `r = reach_m` (so |change| <= max_delta_m and 0 beyond
/// the reach). It reads items by a scan of every item, independently of the footprint query a tile declares, and records which
/// items changed the height it returned.
struct Stamps<'a> {
    inner: Reference,
    def: &'a WorldDefinition,
    touched: std::sync::Mutex<std::collections::BTreeSet<u64>>,
}

impl Stamps<'_> {
    fn bump(&self, d: Vec3) -> f64 {
        let mut sum = 0.0;
        for id in self.def.item_ids(BodyId::PLANET) {
            let (anchor, reach_m) = self.def.item_anchor(BodyId::PLANET, id).unwrap();
            let dist = R * planet_world_def::footprint::angle_between(anchor, d);
            if dist < reach_m {
                let a_m = self.def.item_max_delta(BodyId::PLANET, id).unwrap();
                let q = 1.0 - (dist / reach_m) * (dist / reach_m);
                let v = a_m * q * q;
                if v != 0.0 {
                    self.touched.lock().unwrap().insert(id);
                }
                sum += v;
            }
        }
        sum
    }
}

impl HeightField for Stamps<'_> {
    fn height(&self, d: Vec3) -> f64 {
        self.inner.height(d) + self.bump(d)
    }
    fn seed(&self) -> u64 {
        self.inner.seed()
    }
    fn implementation_id(&self) -> &'static str {
        "stamps-test"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        self.inner.max_abs_height_m() + 1000.0
    }
    fn reads_cells(&self) -> bool {
        true
    }
    fn read_records_for(&self, tile: TileId, _map: &dyn FaceMapping) -> Vec<ReadRecord> {
        let ctx = GenContext::new(self.def);
        ctx.body_seed(BodyId::PLANET).unwrap();
        ctx.query_footprint(BodyId::PLANET, "items", tile, 0.25).unwrap();
        ctx.read_records()
    }
}

// spec: WDEF-007
#[test]
fn every_item_a_stamp_generator_touches_in_a_tile_is_in_the_footprint_the_tile_declared_and_nothing_changes_beyond_the_reach() {
    // Items scattered around three tiles, some far, some just out of reach, with reaches from 10 m to the limit.
    let centre = dir(1.0, 0.2, 0.1);
    let mut def = WorldDefinition::single_planet(R, 7);
    let mut id = 0u64;
    for (i, along_m) in [0.0, 10.0, 60.0, 200.0, 900.0, 4_000.0, 20_000.0, 39_000.0, 80_000.0].into_iter().enumerate() {
        for (j, reach_m) in [10.0, 150.0, 2_000.0, 40_000.0].into_iter().enumerate() {
            id += 1;
            let c = dir(1.0, 0.2 + along_m / R, 0.1 + 0.3 * (i + j) as f64 * 1e-5);
            def.put_item_spec(
                BodyId::PLANET,
                ItemSpec { id, layer: "items".into(), priority: 0, dir: c, content_hash: id, reach_m, max_delta_m: 5.0 + reach_m * 0.01 },
            )
            .unwrap();
        }
    }
    let mut seen = 0;
    for level in [10u8, 13, 16] {
        let tile = TileId::from_direction(&TangentWarp, centre, level);
        let f = Stamps { inner: Reference(7), def: &def, touched: Default::default() };
        let declared: Vec<u64> = def.footprint_items(BodyId::PLANET, "items", tile, 0.25).unwrap().iter().map(|i| i.id).collect();
        // Sample the tile and its 0.25-edge halo (a tile samples its border and a halo for normals).
        let (s0, s1, t0, t1) = tile.bounds();
        let (hs, ht) = (0.25 * (s1 - s0), 0.25 * (t1 - t0));
        for i in 0..=6 {
            for j in 0..=6 {
                let s = s0 - hs + (s1 - s0 + 2.0 * hs) * f64::from(i) / 6.0;
                let t = t0 - ht + (t1 - t0 + 2.0 * ht) * f64::from(j) / 6.0;
                let p = planet_core::cube::face_to_direction(&TangentWarp, tile.face(), s, t);
                let _ = f.height(p);
            }
        }
        for touched in f.touched.lock().unwrap().iter() {
            seen += 1;
            assert!(
                declared.contains(touched),
                "level {level}: item {touched} changed the height in the tile but its footprint did not declare it"
            );
        }
    }
    assert!(seen > 10, "the setup must have items that act in the tiles, {seen}");
    // Beyond the reach nothing changes, within it the change is at most max_delta_m.
    let without = |skip: u64| {
        let mut d = def.clone();
        assert!(d.remove_item(BodyId::PLANET, skip));
        d
    };
    for skip in [1u64, 12, 20, 33] {
        let (anchor, reach_m) = def.item_anchor(BodyId::PLANET, skip).unwrap();
        let max_delta = def.item_max_delta(BodyId::PLANET, skip).unwrap();
        let (with_f, without_def) = (Stamps { inner: Reference(7), def: &def, touched: Default::default() }, without(skip));
        let without_f = Stamps { inner: Reference(7), def: &without_def, touched: Default::default() };
        for k in 0..40 {
            let angle = (f64::from(k) * 0.05) * reach_m * 2.0 / R;
            let p = dir(anchor.x, anchor.y + angle, anchor.z);
            let dist = R * planet_world_def::footprint::angle_between(anchor, p);
            let delta = (with_f.height(p) - without_f.height(p)).abs();
            if dist >= reach_m {
                assert_eq!(delta, 0.0, "item {skip}: {dist} m is beyond its reach of {reach_m} m");
            } else {
                assert!(delta <= max_delta + 1e-9, "item {skip}: change {delta} m over the bound {max_delta} m");
            }
        }
    }
}

// spec: WDEF-007
#[test]
fn a_tile_generated_alone_equals_the_same_samples_cut_from_its_parents_grid_for_a_stage_that_reads_items() {
    use planet_generators::height::generate_tile_field;
    let at = dir(1.0, 0.2, 0.1);
    let mut def = WorldDefinition::single_planet(R, 7);
    for (id, (along_m, reach_m)) in [(0.0, 3_000.0), (700.0, 900.0), (5_000.0, 40_000.0), (-2_500.0, 600.0)].into_iter().enumerate() {
        def.put_item_spec(BodyId::PLANET, stamp(id as u64 + 1, dir(1.0, 0.2 + along_m / R, 0.1), reach_m)).unwrap();
    }
    let f = Stamps { inner: Reference(7), def: &def, touched: Default::default() };
    let parent = TileId::from_direction(&TangentWarp, at, 14);
    let whole = generate_tile_field(&f, &TangentWarp, parent, 32);
    let mut cut = 0;
    for child in parent.children().unwrap() {
        // The child's grid of 16 cells is every second sample of the parent's grid of 32 cells over the child's quadrant.
        let own = generate_tile_field(&f, &TangentWarp, child, 16);
        let (cx, cy) = (child.x() - 2 * parent.x(), child.y() - 2 * parent.y());
        for l in 0..=16u32 {
            for k in 0..=16u32 {
                let (pk, pl) = (16 * cx + k, 16 * cy + l);
                assert_eq!(own.at(k, l).to_bits(), whole.at(pk, pl).to_bits(), "child {child:?} sample ({k}, {l})");
                cut += 1;
            }
        }
    }
    assert_eq!(cut, 4 * 17 * 17);
    assert!(whole.heights.iter().any(|h| *h != 0.0), "the tile is not flat: the items act in it");
    // The order of generation does not matter: the neighbours first, then the tile, give the same bits.
    let again = {
        for side in planet_core::Side::ALL {
            let _ = generate_tile_field(&f, &TangentWarp, parent.neighbor(&TangentWarp, side), 32);
        }
        generate_tile_field(&f, &TangentWarp, parent, 32)
    };
    assert_eq!(again, whole);
}

// spec: STRM-010
#[test]
fn a_field_that_reads_items_has_a_node_set_id_per_tile_that_names_its_reads_and_not_what_they_found() {
    use planet_cache::meshpack::{node_set_id_of, node_set_id_of_tile};
    let at = dir(1.0, 0.2, 0.1);
    let a = TileId::from_direction(&TangentWarp, at, 12);
    let b = TileId::from_direction(&TangentWarp, dir(-1.0, 0.3, 0.2), 12);
    let plain = WorldDefinition::single_planet(R, 7);
    let mut edited = plain.clone();
    edited.put_item_spec(BodyId::PLANET, stamp(1, at, 150.0)).unwrap();
    let (fp, fe) = (Footprinted { inner: Reference(7), def: &plain }, Footprinted { inner: Reference(7), def: &edited });
    let id = |f: &dyn HeightField, t: TileId| node_set_id_of_tile(f, t, &TangentWarp);
    assert_ne!(id(&fp, a), id(&fp, b), "two tiles make two different queries");
    assert_eq!(id(&fp, a), id(&fe, a), "an edit changes what the query finds, never which reads the tile makes");
    // By hand: the sorted, length-prefixed paths of the tile's reads and of the radius node, under the pinned context.
    let mut paths = vec![
        "world.seed".to_string(),
        "derived.body.0.seed".to_string(),
        "body.0.radius_m".to_string(),
        format!("derived.footprint.0.5:items.{:016x}.{:016x}", a.raw(), 0.25f64.to_bits()),
    ];
    paths.sort();
    let mut material = Vec::new();
    for p in &paths {
        material.extend_from_slice(&(p.len() as u32).to_le_bytes());
        material.extend_from_slice(p.as_bytes());
    }
    assert_eq!(id(&fp, a)[..], blake3::derive_key("planet-builder node set v1", &material)[..16]);
    // A field that reads no items keeps its one id for the world, whatever the tile.
    let seed_only = Reference(7);
    assert_eq!(id(&seed_only, a), node_set_id_of(&seed_only));
    assert_eq!(id(&seed_only, a), id(&seed_only, b));
}

const KNOWN_EMPTY_FOOTPRINT: &str = "e99c6b49146b3b01808bed3d94a3bb02";
const KNOWN_FOOTPRINT_ITEM: &str = "545f0ddd4f9bf86df105c9727134ea89";
const KNOWN_FOOTPRINT_RESULT: &str = "0644d35398ab52e3dad17e01a1d97d18";
