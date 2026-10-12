//! STRM-009 (the height tile payload codec) and STRM-004 (cache verification), ADR 0019 and ADR 0006.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::codec::{decode_height_tile, encode_height_tile, payload_res, CodecError, CODEC_VERSION, MAX_RES};
use planet_cache::key::{height_key, mesh_key, mesh_key_inputs, CacheKey};
use planet_cache::pack::{pick, PackWriter, Record, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_cache::tiles::{build_tiles, region_tiles, verify_store, BuildError, World, KIND_HEIGHT, MAX_REGION_LEVEL};
use planet_core::cube::TangentWarp;
use planet_core::tile::TileId as Tile;
use planet_core::{Face, FaceMapping, TileId, Vec3};
use planet_generators::height::{
    generate_tile, generate_tile_field, HeightField, HeightGenerator, Reference, ReferenceGenerator, TileData,
};
use planet_world_def::cells::{cell_of, cells_covering, with_margin};
use planet_world_def::{BodyId, GenContext, MissingNode, ReadRecord, WorldDefinition};

const R: f64 = 6_371_000.0;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-tiles-tests-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&p);
        Dir(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn world(seed: u64) -> World {
    World::from_definition(&WorldDefinition::single_planet(R, seed)).unwrap()
}

fn tile() -> TileId {
    TileId::new(Face(2), 5, 3, 7).unwrap()
}

fn enc(t: &TileData) -> Vec<u8> {
    encode_height_tile(t).unwrap()
}

fn bits(t: &TileData) -> Vec<u32> {
    t.heights.iter().map(|h| h.to_bits()).collect()
}

// spec: STRM-009
#[test]
fn a_height_tile_round_trips_bit_for_bit_and_compresses() {
    for res in [1u32, 2, 8, 32, 64] {
        let t = generate_tile(7, &TangentWarp, tile(), res);
        let bytes = enc(&t);
        assert_eq!(payload_res(&bytes), Ok(res));
        let back = decode_height_tile(tile(), &bytes).unwrap();
        assert_eq!(back.res, res);
        assert_eq!(bits(&back), bits(&t));
        assert_eq!(back.content_hash(), t.content_hash());
    }
    let t = generate_tile(7, &TangentWarp, tile(), 64);
    let raw = 4 * 65 * 65;
    let packed = enc(&t).len();
    assert!(packed < raw, "shuffled LZ4 must beat the raw {raw} bytes, got {packed}");
}

// spec: STRM-009
#[test]
fn every_byte_value_at_every_byte_position_and_the_hard_f32_patterns_survive() {
    // The shuffle is a byte permutation, so putting every byte value at every byte position covers every f32 pattern.
    let heights: Vec<f32> = (0..=288u32).map(|b| f32::from_bits((b % 256) * 0x0101_0101)).collect();
    let t = TileData { id: tile(), res: 16, heights };
    let back = decode_height_tile(tile(), &enc(&t)).unwrap();
    assert_eq!(bits(&back), bits(&t));
    let mut x = 0x2545_f491_4f6c_dd1du64;
    let mut heights: Vec<f32> = vec![
        0.0,
        -0.0,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::MAX,
        f32::MIN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::from_bits(0x7fc0_1234),
        f32::from_bits(0xffff_ffff),
    ];
    while heights.len() < 9 * 9 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        heights.push(f32::from_bits(x as u32));
    }
    let t = TileData { id: tile(), res: 8, heights };
    assert_eq!(bits(&decode_height_tile(tile(), &enc(&t)).unwrap()), bits(&t));
}

// spec: STRM-009
#[test]
fn the_payload_layout_is_pinned_by_a_hand_built_block() {
    // Version 1, res 1, 16 raw bytes, then an LZ4 block that is all literals (token 0xF0, extra length byte 1, 16 bytes).
    // The raw bytes are the heights [1.0, 2.0, 3.0, 4.0] as little-endian f32, shuffled by byte index.
    let raw: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0x80, 0, 0x40, 0x80, 0x3f, 0x40, 0x40, 0x40];
    let mut payload = vec![CODEC_VERSION, 1, 0, 0, 0, 16, 0, 0, 0, 0xf0, 0x01];
    payload.extend_from_slice(&raw);
    let t = decode_height_tile(tile(), &payload).unwrap();
    assert_eq!(t.res, 1);
    assert_eq!(t.heights, vec![1.0, 2.0, 3.0, 4.0]);
    assert_eq!(CODEC_VERSION, 1, "a new layout needs a new version and a new hand-built block");
}

// spec: STRM-009
#[test]
fn damaged_or_hostile_payloads_are_errors_and_never_huge_allocations() {
    let good = enc(&generate_tile(7, &TangentWarp, tile(), 8));
    assert_eq!(decode_height_tile(tile(), &good[..5]).unwrap_err(), CodecError::Short);
    let mut v = good.clone();
    v[0] = 9;
    assert_eq!(decode_height_tile(tile(), &v).unwrap_err(), CodecError::Version(9));
    for res in [0u32, 3, MAX_RES * 2, u32::MAX] {
        let mut v = good.clone();
        v[1..5].copy_from_slice(&res.to_le_bytes());
        assert_eq!(decode_height_tile(tile(), &v).unwrap_err(), CodecError::Res(res), "res {res}");
    }
    // A claimed raw length that does not match the resolution is refused before anything is allocated for it.
    let mut v = good.clone();
    v[5..9].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(decode_height_tile(tile(), &v).unwrap_err(), CodecError::RawLength { .. }));
    // The largest resolution with the right claimed length and a one-token block: bounded, an error, no panic.
    let mut big = vec![CODEC_VERSION];
    big.extend_from_slice(&MAX_RES.to_le_bytes());
    big.extend_from_slice(&(4 * (MAX_RES + 1) * (MAX_RES + 1)).to_le_bytes());
    big.push(0x00);
    assert!(matches!(decode_height_tile(tile(), &big).unwrap_err(), CodecError::Lz4(_) | CodecError::Truncated { .. }));
    // A damaged LZ4 block.
    let mut v = good.clone();
    let n = v.len();
    for b in &mut v[n / 2..] {
        *b = 0xff;
    }
    assert!(decode_height_tile(tile(), &v).is_err());
    assert!(decode_height_tile(tile(), &[]).is_err());
    for e in [
        CodecError::Short,
        CodecError::Version(2),
        CodecError::Res(3),
        CodecError::Lz4("x".into()),
        CodecError::Truncated { got: 1, expected: 16 },
    ] {
        assert!(!e.to_string().is_empty());
    }
}

// spec: STRM-009
#[test]
fn a_block_that_is_too_long_or_too_short_for_its_header_is_pinned_apart() {
    let head = |raw_len: u32| {
        let mut v = vec![CODEC_VERSION, 1, 0, 0, 0];
        v.extend_from_slice(&raw_len.to_le_bytes());
        v
    };
    // 20 literal bytes (token 0xF0 + extension byte 5) where 16 are claimed: more than the output holds.
    let mut long = head(16);
    long.extend_from_slice(&[0xf0, 0x05]);
    long.extend_from_slice(&[7u8; 20]);
    assert!(matches!(decode_height_tile(tile(), &long).unwrap_err(), CodecError::Lz4(_)));
    // 8 literal bytes (token 0x80) where 16 are claimed: fewer than the header says.
    let mut short = head(16);
    short.push(0x80);
    short.extend_from_slice(&[7u8; 8]);
    assert_eq!(decode_height_tile(tile(), &short).unwrap_err(), CodecError::Truncated { got: 8, expected: 16 });
}

// spec: STRM-009
#[test]
fn the_encoder_refuses_what_the_decoder_would_refuse() {
    for res in [0u32, 3, 6, MAX_RES * 2] {
        let t = TileData { id: tile(), res, heights: vec![0.0; 4] };
        assert_eq!(encode_height_tile(&t).unwrap_err(), CodecError::Res(res));
    }
    let t = TileData { id: tile(), res: 4, heights: vec![0.0; 24] };
    assert_eq!(encode_height_tile(&t).unwrap_err(), CodecError::Heights { got: 24, expected: 25 });
    assert!(CodecError::Heights { got: 1, expected: 4 }.to_string().contains("heights"));
}

// spec: STRM-004
#[test]
fn a_region_is_built_once_reused_afterwards_and_verifies_clean() {
    let d = Dir::new("build");
    let w = world(7);
    let tiles = region_tiles(Face(0), 2).unwrap();
    assert_eq!(tiles.len(), 16);
    let first = build_tiles(d.path(), &w, &tiles, 8, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((first.written, first.reused), (16, 0));
    let second = build_tiles(d.path(), &w, &tiles, 8, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((second.written, second.reused), (0, 16), "a second run generates nothing");

    let store = TileStore::open(d.path(), w.params()).unwrap();
    assert_eq!(store.len(), 16);
    for t in &tiles {
        let rec = store.get(KIND_HEIGHT, *t, w.tile_key(*t, 8)).unwrap().expect("stored under the world's key");
        assert_eq!(rec.tile, *t);
        let kept = decode_height_tile(*t, &rec.payload).unwrap();
        assert_eq!(kept.content_hash(), generate_tile(7, &TangentWarp, *t, 8).content_hash());
    }
    // The world's key is the persistent key of the field built directly from the seed.
    let direct = height_key(&Reference(7), &TangentWarp, tiles[3], 8, R);
    assert_eq!(w.tile_key(tiles[3], 8), direct);

    let all = verify_store(&store, &w, 0);
    assert_eq!((all.checked, all.matching, all.stale, all.unexplained_stale, all.unsupported), (16, 16, 0, 0, 0));
    assert!(all.passed() && all.clean() && !all.compared_nothing());
    let some = verify_store(&store, &w, 4);
    assert_eq!((some.checked, some.matching), (4, 4), "an evenly spaced sample of four");
    assert_eq!(verify_store(&store, &w, 1000).checked, 16, "a sample larger than the store is the whole store");
}

fn hex16(b: [u8; 16]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The node set id of `single_planet` worlds with the reference generator, pinned (STRM-010). Bump `"planet-builder node
/// set v1"` and this together when the encoding changes.
const KNOWN_NODE_SET_ID: &str = "5ac2ac486ee5e93dc41c36dcd7d889c1";

// spec: STRM-010
#[test]
fn the_node_set_id_is_a_pinned_derive_key_hash_of_the_sorted_length_prefixed_paths() {
    let w = world(7);
    // Built by hand from the paths the reference generator reads and the body radius node, independently of `World`.
    let def = WorldDefinition::single_planet(R, 7);
    let reads = GenContext::new(&def);
    let field = ReferenceGenerator.field(&reads, BodyId::PLANET).unwrap();
    let mut paths: Vec<String> = field.read_records().iter().map(|r| r.path()).collect();
    paths.push("body.0.radius_m".to_string());
    paths.sort();
    paths.dedup();
    assert_eq!(paths, ["body.0.radius_m", "derived.body.0.seed", "world.seed"], "the paths the id covers");
    let mut material = Vec::new();
    for p in &paths {
        material.extend_from_slice(&(p.len() as u32).to_le_bytes());
        material.extend_from_slice(p.as_bytes());
    }
    // The spec's bytes written out literally: little-endian u32 byte lengths 15, 19 and 10, no count prefix.
    assert_eq!(
        material,
        b"\x0f\x00\x00\x00body.0.radius_m\x13\x00\x00\x00derived.body.0.seed\x0a\x00\x00\x00world.seed".to_vec(),
        "the material the hash covers"
    );
    let by_hand = blake3::derive_key("planet-builder node set v1", &material);
    assert_eq!(w.node_set_id(), by_hand[..16], "paths {paths:?}");
    assert_eq!(hex16(w.node_set_id()), KNOWN_NODE_SET_ID, "paths {paths:?}: the pinned known answer");

    // A value change of a node leaves the id alone (it is about which nodes are read); another seed reads the same nodes.
    assert_eq!(world(8).node_set_id(), w.node_set_id());
    // A generator whose field declares no field-wide reads covers other nodes: another id.
    let fewer = World::from_generator(&WorldDefinition::single_planet(R, 7), &WorldSeedOnly).unwrap();
    assert_ne!(fewer.node_set_id(), w.node_set_id(), "a different set of nodes read gives a different id");
    // Every stored record carries it.
    let d = Dir::new("node-set");
    let tiles = region_tiles(Face(0), 1).unwrap();
    build_tiles(d.path(), &w, &tiles, 4, DEFAULT_MAX_PACK_BYTES).unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    for t in &tiles {
        let rec = store.get(KIND_HEIGHT, *t, w.tile_key(*t, 4)).unwrap().unwrap();
        assert_eq!(rec.node_set_id, w.node_set_id());
    }
}

// spec: STRM-004
#[test]
fn an_empty_store_compares_nothing_and_fails_even_when_stale_entries_are_allowed() {
    // ADR 0025 point 4: a run that compared nothing fails unless --allow-stale and at least one entry is stale.
    let d = Dir::new("empty");
    let w = world(7);
    let missing = verify_store(&TileStore::open(&d.path().join("none"), w.params()).unwrap(), &w, 0);
    assert_eq!(missing.checked, 0);
    assert!(missing.clean() && missing.compared_nothing() && !missing.passed() && !missing.passed_allowing_stale(), "{missing:?}");

    // An existing empty directory is the same.
    std::fs::create_dir_all(d.path()).unwrap();
    let empty = verify_store(&TileStore::open(d.path(), w.params()).unwrap(), &w, 0);
    assert!(empty.compared_nothing() && !empty.passed() && !empty.passed_allowing_stale(), "{empty:?}");

    // A store whose only records are of another kind: looked at, not compared.
    let mut pack = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    let t = region_tiles(Face(1), 1).unwrap()[0];
    pack.append(&Record {
        key: CacheKey([9; 16]),
        kind: 2,
        tile: t,
        generator_version: 1,
        hash_version: 1,
        node_set_id: [0; 16],
        payload: vec![1, 2, 3],
    })
    .unwrap();
    pack.finish().unwrap();
    let other = verify_store(&TileStore::open(d.path(), w.params()).unwrap(), &w, 0);
    assert_eq!((other.checked, other.unsupported, other.matching, other.stale), (1, 1, 0, 0));
    assert!(other.compared_nothing() && !other.passed() && !other.passed_allowing_stale(), "{other:?}");

    // A store whose only pack was made under another hash version is skipped by the reader: nothing to compare either.
    let old = Dir::new("empty-old-hash");
    let mut pack = PackWriter::open(
        old.path(),
        planet_cache::pack::StoreParams { hash_version: w.params().hash_version + 1, body: 0 },
        DEFAULT_MAX_PACK_BYTES,
    )
    .unwrap();
    pack.append(&w.tile_record(t, 4).unwrap()).unwrap();
    pack.finish().unwrap();
    let store = TileStore::open(old.path(), w.params()).unwrap();
    assert_eq!(store.len(), 0, "the pack of another hash version is skipped");
    let skipped = verify_store(&store, &w, 0);
    assert!(skipped.compared_nothing() && !skipped.passed() && !skipped.passed_allowing_stale(), "{skipped:?}");
}

// spec: STRM-004
#[test]
fn a_stored_height_tile_is_keyed_by_the_number_the_meshs_source_key_uses() {
    // `res` is cells (intervals) per edge everywhere: a mesh of `cells` cells is built from the height tile with `res == cells`,
    // so the key under which that tile is stored is the source key the mesh key chains.
    let w = world(7);
    let planet = Reference(7);
    for cells in [4u32, 16, 32] {
        let inputs = mesh_key_inputs(&planet, &TangentWarp, tile(), cells, R);
        assert_eq!(inputs.source, Some(w.tile_key(tile(), cells)), "cells {cells}");
        assert_eq!(inputs.source, Some(height_key(&planet, &TangentWarp, tile(), cells, R)));
        // A tile keyed by its sample count (cells + 1) would not match the mesh's source.
        assert_ne!(inputs.source, Some(w.tile_key(tile(), cells + 1)), "cells {cells}");
    }
    assert_eq!(
        mesh_key(&planet, &TangentWarp, tile(), 16, R),
        planet_cache::key::cache_key(&mesh_key_inputs(&planet, &TangentWarp, tile(), 16, R))
    );
    // The stored record of a tile at `res` is found under exactly that key, and decodes to `(res + 1)^2` heights.
    let d = Dir::new("unit");
    build_tiles(d.path(), &w, &[tile()], 16, DEFAULT_MAX_PACK_BYTES).unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let rec = store.get(KIND_HEIGHT, tile(), mesh_key_inputs(&planet, &TangentWarp, tile(), 16, R).source.unwrap()).unwrap().unwrap();
    assert_eq!(decode_height_tile(tile(), &rec.payload).unwrap().heights.len(), 17 * 17);
}

// spec: STRM-004
#[test]
fn a_build_refuses_a_resolution_or_a_size_it_cannot_store() {
    let d = Dir::new("build-refusals");
    let w = world(7);
    let tiles = region_tiles(Face(0), 1).unwrap();
    for res in [0u32, 3, MAX_RES * 2] {
        assert_eq!(build_tiles(d.path(), &w, &tiles, res, DEFAULT_MAX_PACK_BYTES).unwrap_err(), BuildError::Res(res), "res {res}");
    }
    let many = region_tiles(Face(0), MAX_REGION_LEVEL).unwrap();
    let e = build_tiles(d.path(), &w, &many, 2048, DEFAULT_MAX_PACK_BYTES).unwrap_err();
    assert!(matches!(e, BuildError::TooLarge { tiles: 4096, res: 2048, .. }), "{e}");
    assert!(e.to_string().contains("use a lower level or resolution"));
    assert!(!d.path().exists(), "nothing was created for a refused build");
    for (face, level) in [(6u8, 1u8), (0, MAX_REGION_LEVEL + 1), (0, 29), (0, 40), (255, 0)] {
        let e = region_tiles(Face(face), level).unwrap_err();
        assert!(e.contains("face") || e.contains("level"), "{e}");
    }
    for level in 0..=MAX_REGION_LEVEL {
        let tiles = region_tiles(Face(3), level).unwrap();
        assert_eq!(tiles.len(), 4usize.pow(u32::from(level)));
        assert!(tiles.iter().all(|t| t.face() == Face(3) && t.level() == level));
    }
}

// spec: STRM-004
#[test]
fn a_run_that_compared_nothing_is_not_a_pass_and_stale_entries_are_told_apart() {
    let d = Dir::new("stale");
    let tiles = region_tiles(Face(1), 1).unwrap();
    build_tiles(d.path(), &world(7), &tiles, 4, DEFAULT_MAX_PACK_BYTES).unwrap();

    // Another seed: the versions and the node set are the same, only a node value changed: unexplained, and nothing was
    // compared, so the run does not pass unless stale entries are expected.
    let other = world(8);
    let store = TileStore::open(d.path(), other.params()).unwrap();
    let r = verify_store(&store, &other, 0);
    assert_eq!((r.checked, r.matching, r.stale, r.unexplained_stale), (4, 0, 0, 4));
    assert!(r.compared_nothing() && !r.passed() && r.passed_allowing_stale() && r.clean());

    // Another generator version: the staleness is explained by the metadata.
    let versioned = World::from_generator(&WorldDefinition::single_planet(R, 7), &Versioned(99)).unwrap();
    let r = verify_store(&TileStore::open(d.path(), versioned.params()).unwrap(), &versioned, 0);
    assert_eq!((r.checked, r.stale, r.unexplained_stale), (4, 4, 0));
    assert!(!r.passed() && r.passed_allowing_stale());

    // Building the other world into the same directory adds its own tiles; now something is compared and the run passes.
    let w = build_tiles(d.path(), &other, &tiles, 4, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((w.written, w.reused), (4, 0));
    let store = TileStore::open(d.path(), other.params()).unwrap();
    let r = verify_store(&store, &other, 0);
    assert_eq!((r.checked, r.unexplained_stale, r.matching), (8, 4, 4));
    assert!(r.passed());
}

/// `Reference` under another generator version: the same heights, another key and other metadata.
struct VersionedField(u32);

impl HeightField for VersionedField {
    fn height(&self, d: Vec3) -> f64 {
        Reference(7).height(d)
    }
    fn seed(&self) -> u64 {
        7
    }
    fn implementation_id(&self) -> &'static str {
        "reference"
    }
    fn version(&self) -> u32 {
        self.0
    }
    fn max_abs_height_m(&self) -> f64 {
        Reference(7).max_abs_height_m()
    }
}

struct Versioned(u32);

impl HeightGenerator for Versioned {
    fn implementation_id(&self) -> &'static str {
        "reference"
    }
    fn version(&self) -> u32 {
        self.0
    }
    fn field(&self, ctx: &GenContext, body: BodyId) -> Result<Box<dyn HeightField>, MissingNode> {
        ctx.body_seed(body)?;
        Ok(Box::new(VersionedField(self.0)))
    }
}

/// `Reference` heights from a field that declares no field-wide reads: only the body radius node is left in its id.
struct NoReadsField;

impl HeightField for NoReadsField {
    fn height(&self, d: Vec3) -> f64 {
        Reference(7).height(d)
    }
    fn seed(&self) -> u64 {
        7
    }
    fn implementation_id(&self) -> &'static str {
        "reference"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        Reference(7).max_abs_height_m()
    }
    fn read_records(&self) -> Vec<ReadRecord> {
        Vec::new()
    }
}

/// A generator whose field reads nothing field-wide, where `Reference` reads the body seed and the world seed.
struct WorldSeedOnly;

impl HeightGenerator for WorldSeedOnly {
    fn implementation_id(&self) -> &'static str {
        "reference"
    }
    fn version(&self) -> u32 {
        1
    }
    fn field(&self, _ctx: &GenContext, _body: BodyId) -> Result<Box<dyn HeightField>, MissingNode> {
        Ok(Box::new(NoReadsField))
    }
}

// spec: STRM-004
#[test]
fn a_tile_made_with_an_input_that_is_not_in_its_key_is_reported_with_its_tile_pack_offset_and_sample() {
    let d = Dir::new("missing-input");
    let w = world(7);
    let t = tile();
    // A buggy writer: the key and metadata are the world's, the payload was made with another seed.
    let mut bad = w.tile_record(t, 8).unwrap();
    bad.payload = enc(&generate_tile(8, &TangentWarp, t, 8));
    let good = w.tile_record(TileId::new(Face(2), 5, 3, 8).unwrap(), 8).unwrap();
    let mut writer = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&good).unwrap();
    writer.append(&bad).unwrap();
    writer.finish().unwrap();

    let store = TileStore::open(d.path(), w.params()).unwrap();
    let report = verify_store(&store, &w, 0);
    assert_eq!((report.checked, report.matching), (2, 1));
    assert!(!report.passed() && !report.passed_allowing_stale());
    assert_eq!(report.mismatches.len(), 1);
    let m = &report.mismatches[0];
    assert_eq!(m.tile, t);
    assert_eq!(m.key, w.tile_key(t, 8));
    assert!(m.pack.ends_with("pack-00000001.tpack") && m.offset > 24, "{} @ {}", m.pack, m.offset);
    assert!(
        m.detail.contains("sample (")
            && m.detail.contains("stored")
            && m.detail.contains("regenerated")
            && m.detail.contains("missing from its key"),
        "{}",
        m.detail
    );
}

/// A generator whose field reads a definition node behind the context's back: the node is not in the field's read set.
struct LeakyGenerator {
    leak: WorldDefinition,
}

struct LeakyField {
    offset_m: f64,
}

impl HeightField for LeakyField {
    fn height(&self, d: Vec3) -> f64 {
        Reference(7).height(d) + self.offset_m
    }
    fn seed(&self) -> u64 {
        7
    }
    fn implementation_id(&self) -> &'static str {
        "leaky"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        Reference(7).max_abs_height_m() + 1000.0
    }
}

impl HeightGenerator for LeakyGenerator {
    fn implementation_id(&self) -> &'static str {
        "leaky"
    }
    fn version(&self) -> u32 {
        1
    }
    fn field(&self, ctx: &GenContext, body: BodyId) -> Result<Box<dyn HeightField>, MissingNode> {
        ctx.body_seed(body)?;
        let offset_m = match self.leak.get("x.leak") {
            Some(planet_world_def::NodeValue::F64(v)) => v,
            _ => 0.0,
        };
        Ok(Box::new(LeakyField { offset_m }))
    }
}

// spec: STRM-004
#[test]
fn a_generator_that_reads_a_node_behind_the_contexts_back_is_caught_when_that_node_changes() {
    let mut d1 = WorldDefinition::single_planet(R, 7);
    d1.set("x.leak", planet_world_def::NodeValue::F64(0.0));
    let mut d2 = d1.clone();
    d2.set("x.leak", planet_world_def::NodeValue::F64(50.0));
    let w1 = World::from_generator(&d1, &LeakyGenerator { leak: d1.clone() }).unwrap();
    let w2 = World::from_generator(&d2, &LeakyGenerator { leak: d2.clone() }).unwrap();
    let t = tile();
    assert_eq!(w1.tile_key(t, 4), w2.tile_key(t, 4), "the leaked node is not in the key: that is the bug");

    let dir = Dir::new("leaky");
    build_tiles(dir.path(), &w1, &[t], 4, DEFAULT_MAX_PACK_BYTES).unwrap();
    // Verified against the same definition the leak is invisible; against the edited one it is found.
    let store = TileStore::open(dir.path(), w1.params()).unwrap();
    assert!(verify_store(&store, &w1, 0).passed(), "same definition: nothing can be seen");
    let report = verify_store(&store, &w2, 0);
    assert!(!report.clean(), "the edited definition gives the same key and other heights");
    assert_eq!(report.mismatches.len(), 1);
    assert!(report.mismatches[0].detail.contains("missing from its key"));
    // And building after the edit reuses the stale tile, which is exactly why the key must be complete.
    let again = build_tiles(dir.path(), &w2, &[t], 4, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((again.written, again.reused), (0, 1));
}

/// Heights shifted by 100 m inside every cell that holds an item, reading the items through cell queries with a margin.
struct StampGenerator {
    def: WorldDefinition,
}

struct StampField {
    def: WorldDefinition,
}

impl HeightField for StampField {
    fn height(&self, d: Vec3) -> f64 {
        let cell = cell_of(R, d);
        let lifted = !self.def.items_in_cell(BodyId::PLANET, cell).is_empty();
        Reference(7).height(d) + if lifted { 100.0 } else { 0.0 }
    }
    fn seed(&self) -> u64 {
        7
    }
    fn implementation_id(&self) -> &'static str {
        "stamp"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        Reference(7).max_abs_height_m() + 100.0
    }
    fn reads_cells(&self) -> bool {
        true
    }
    fn read_records_for(&self, tile: Tile, _map: &dyn FaceMapping) -> Vec<ReadRecord> {
        let ctx = GenContext::new(&self.def);
        ctx.body_seed(BodyId::PLANET).unwrap();
        ctx.query_tile(BodyId::PLANET, tile, 1, 64).unwrap();
        ctx.read_records()
    }
}

impl HeightGenerator for StampGenerator {
    fn implementation_id(&self) -> &'static str {
        "stamp"
    }
    fn version(&self) -> u32 {
        1
    }
    fn field(&self, ctx: &GenContext, body: BodyId) -> Result<Box<dyn HeightField>, MissingNode> {
        ctx.body_seed(body)?;
        Ok(Box::new(StampField { def: self.def.clone() }))
    }
}

// spec: STRM-004
#[test]
fn an_item_edit_rebuilds_exactly_the_tiles_whose_footprint_and_margin_hold_its_cell() {
    let d1 = WorldDefinition::single_planet(R, 7);
    let item_at = Vec3::new(1.0, 0.2, 0.1).normalized();
    let mut d2 = d1.clone();
    let item_cell = d2.put_item(BodyId::PLANET, 1, item_at, 42).unwrap();
    let w1 = World::from_generator(&d1, &StampGenerator { def: d1.clone() }).unwrap();
    let w2 = World::from_generator(&d2, &StampGenerator { def: d2.clone() }).unwrap();

    // Every tile of face 0 at level 6 (4,096): each covers 16 cells, so with a one-cell margin 36 at most.
    let tiles = region_tiles(Face(0), 6).unwrap();
    let dir = Dir::new("stamp");
    let first = build_tiles(dir.path(), &w1, &tiles, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((first.written, first.reused), (4096, 0));

    // The tiles that must change, computed from the cell grid alone.
    let expected: usize = tiles.iter().filter(|t| with_margin(&cells_covering(**t, 8, 64).unwrap(), 1).contains(&item_cell)).count();
    assert!((1..=40).contains(&expected), "the item must touch a few tiles, got {expected}");
    let second = build_tiles(dir.path(), &w2, &tiles, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!(second.written, expected, "an edit of one item rebuilds exactly the tiles that read its cell");
    assert_eq!(second.reused, 4096 - expected);

    // The store now verifies against the edited definition: the current tiles match, the replaced ones are stale.
    let store = TileStore::open(dir.path(), w2.params()).unwrap();
    let r = verify_store(&store, &w2, 0);
    assert!(r.passed(), "{r:?}");
    assert_eq!((r.matching, r.unexplained_stale), (4096, expected));
    // And the lifted heights differ from the old ones where the item is.
    let changed = tiles.iter().filter(|t| w1.tile_key(**t, 2) != w2.tile_key(**t, 2)).count();
    assert_eq!(changed, expected);
    let _ = generate_tile_field(&StampField { def: d2 }, &TangentWarp, tiles[0], 2);
}

// spec: STRM-004
#[test]
fn metadata_that_disagrees_with_the_definition_and_damage_in_stale_or_scanned_records_are_reported() {
    let d = Dir::new("metadata");
    let w = world(7);
    let t = tile();
    // The key is the definition's, the metadata is not: written wrongly.
    let mut wrong_meta = w.tile_record(t, 8).unwrap();
    wrong_meta.node_set_id = [9; 16];
    wrong_meta.generator_version = 77;
    // A stale entry (not the definition's key) whose payload is damaged: still damage.
    let other_tile = TileId::new(Face(2), 5, 3, 8).unwrap();
    let mut stale_damaged = w.tile_record(other_tile, 8).unwrap();
    stale_damaged.key = CacheKey([5; 16]);
    stale_damaged.payload.truncate(stale_damaged.payload.len() / 2);
    let mut writer = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&wrong_meta).unwrap();
    writer.append(&stale_damaged).unwrap();
    writer.finish().unwrap();
    let report = verify_store(&TileStore::open(d.path(), w.params()).unwrap(), &w, 0);
    assert_eq!(report.mismatches.len(), 1);
    assert!(
        report.mismatches[0].detail.contains("disagrees") && report.mismatches[0].detail.contains("generator version 77"),
        "{}",
        report.mismatches[0].detail
    );
    assert_eq!(report.unreadable.len(), 1, "{:?}", report.unreadable);
    assert!(report.unreadable[0].contains("offset") && report.unreadable[0].contains("pack-00000001.tpack"));
    assert!(!report.clean() && !report.passed_allowing_stale());

    // A payload byte that fails its CRC in the last bytes of a small pack: the scan skips the record, and verify must still
    // report it, with pack, offset and key, instead of a warning and a pass.
    let d2 = Dir::new("scan-damage");
    build_tiles(d2.path(), &w, &region_tiles(Face(0), 1).unwrap(), 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    let data = d2.path().join("pack-00000001.tpack");
    let mut bytes = fs::read(&data).unwrap();
    let n = bytes.len();
    bytes[n - 20] ^= 0x40;
    fs::write(&data, &bytes).unwrap();
    let store = TileStore::open(d2.path(), w.params()).unwrap();
    assert_eq!(store.damaged().len(), 1);
    let report = verify_store(&store, &w, 0);
    assert_eq!(report.unreadable.len(), 1, "{:?}", report.unreadable);
    assert!(
        report.unreadable[0].contains("key ") && report.unreadable[0].contains("offset") && report.unreadable[0].contains("CRC"),
        "{}",
        report.unreadable[0]
    );
    assert!(!report.clean());
    assert_eq!(report.matching, 3, "the other records still verify");
}

// spec: STRM-004
#[test]
fn samples_are_evenly_spaced_without_overflow_and_a_world_needs_its_nodes() {
    assert_eq!((0..4).map(|i| pick(i, 4, 16)).collect::<Vec<_>>(), vec![0, 4, 8, 12]);
    let total = usize::MAX / 2;
    let count = total - 1;
    let picks: Vec<usize> = [0, 1, count / 2, count - 1].iter().map(|i| pick(*i, count, total)).collect();
    assert!(picks.windows(2).all(|w| w[0] < w[1]) && picks.iter().all(|p| *p < total), "{picks:?}");
    let mut d = WorldDefinition::empty();
    assert!(World::from_definition(&d).is_err(), "no seed");
    d.set("world.seed", planet_world_def::NodeValue::U64(7));
    let e = World::from_definition(&d).err().expect("no radius");
    assert!(e.to_string().contains("body.0.radius_m"), "{e}");
    // The field generic generator equals the seed generator for the reference field.
    let def = WorldDefinition::single_planet(R, 7);
    let field = planet_generators::height::ReferenceGenerator.field(&GenContext::new(&def), BodyId::PLANET).unwrap();
    assert_eq!(
        generate_tile_field(field.as_ref(), &TangentWarp, tile(), 8).content_hash(),
        generate_tile(7, &TangentWarp, tile(), 8).content_hash()
    );
}
