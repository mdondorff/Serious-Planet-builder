//! STRM-009 (the height tile payload codec) and STRM-004 (cache verification), ADR 0019 and ADR 0006.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::codec::{decode_height_tile, encode_height_tile, payload_res, CodecError, CODEC_VERSION, MAX_RES};
use planet_cache::key::height_key;
use planet_cache::pack::{PackWriter, Record, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_cache::tiles::{build_tiles, region_tiles, verify_store, World, KIND_HEIGHT};
use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::height::{generate_tile, generate_tile_field, HeightGenerator, Reference, TileData};
use planet_world_def::{BodyId, GenContext, WorldDefinition};

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

// spec: STRM-009
#[test]
fn a_height_tile_round_trips_bit_for_bit_and_compresses() {
    for res in [1u32, 2, 8, 32, 64] {
        let t = generate_tile(7, &TangentWarp, tile(), res);
        let bytes = encode_height_tile(&t);
        assert_eq!(payload_res(&bytes), Ok(res));
        let back = decode_height_tile(tile(), &bytes).unwrap();
        assert_eq!(back.res, res);
        assert_eq!(back.heights.iter().map(|h| h.to_bits()).collect::<Vec<_>>(), t.heights.iter().map(|h| h.to_bits()).collect::<Vec<_>>());
        assert_eq!(back.content_hash(), t.content_hash());
    }
    let t = generate_tile(7, &TangentWarp, tile(), 64);
    let raw = 4 * 65 * 65;
    let packed = encode_height_tile(&t).len();
    assert!(packed < raw, "shuffled LZ4 must beat the raw {raw} bytes, got {packed}");
}

// spec: STRM-009
#[test]
fn every_f32_bit_pattern_survives_including_nan_payloads_signed_zeros_and_subnormals() {
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
    let back = decode_height_tile(tile(), &encode_height_tile(&t)).unwrap();
    assert_eq!(back.heights.iter().map(|h| h.to_bits()).collect::<Vec<_>>(), t.heights.iter().map(|h| h.to_bits()).collect::<Vec<_>>());
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
    let good = encode_height_tile(&generate_tile(7, &TangentWarp, tile(), 8));
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
    // A damaged LZ4 block.
    let mut v = good.clone();
    let n = v.len();
    for b in &mut v[n / 2..] {
        *b = 0xff;
    }
    assert!(matches!(decode_height_tile(tile(), &v).unwrap_err(), CodecError::Lz4(_) | CodecError::RawLength { .. }));
    assert!(decode_height_tile(tile(), &[]).is_err());
    for e in [CodecError::Short, CodecError::Version(2), CodecError::Res(3), CodecError::Lz4("x".into())] {
        assert!(!e.to_string().is_empty());
    }
}

// spec: STRM-004
#[test]
fn a_region_is_built_once_reused_afterwards_and_verifies_clean() {
    let d = Dir::new("build");
    let w = world(7);
    let tiles = region_tiles(Face(0), 2);
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
    assert_eq!((all.checked, all.matching, all.stale, all.unsupported), (16, 16, 0, 0));
    assert!(all.passed() && all.mismatches.is_empty() && all.unreadable.is_empty());
    let some = verify_store(&store, &w, 4);
    assert_eq!((some.checked, some.matching), (4, 4), "an evenly spaced sample of four");
    assert_eq!(verify_store(&store, &w, 1000).checked, 16, "a sample larger than the store is the whole store");
}

// spec: STRM-004
#[test]
fn a_definition_that_changed_makes_entries_stale_not_wrong() {
    let d = Dir::new("stale");
    let tiles = region_tiles(Face(1), 1);
    build_tiles(d.path(), &world(7), &tiles, 4, DEFAULT_MAX_PACK_BYTES).unwrap();
    let other = world(8);
    let store = TileStore::open(d.path(), other.params()).unwrap();
    let report = verify_store(&store, &other, 0);
    assert_eq!((report.checked, report.stale, report.matching), (4, 4, 0));
    assert!(report.passed(), "stale entries are reclaimable space, not failures");
    // Building the other world into the same directory adds its own tiles and leaves the stale ones in place.
    let r = build_tiles(d.path(), &other, &tiles, 4, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((r.written, r.reused), (4, 0));
    let store = TileStore::open(d.path(), other.params()).unwrap();
    let report = verify_store(&store, &other, 0);
    assert_eq!((report.checked, report.stale, report.matching), (8, 4, 4));
}

// spec: STRM-004
#[test]
fn a_tile_made_with_an_input_that_is_not_in_its_key_is_reported_with_its_tile_pack_offset_and_sample() {
    let d = Dir::new("missing-input");
    let w = world(7);
    let t = tile();
    // A buggy generator: the key is the world's, the payload was made with another seed (an input left out of the key).
    let wrong = generate_tile(8, &TangentWarp, t, 8);
    let mut good_neighbour = w.tile_record(TileId::new(Face(2), 5, 3, 8).unwrap(), 8);
    good_neighbour.node_set_id = [1; 16];
    let bad = Record {
        key: w.tile_key(t, 8),
        kind: KIND_HEIGHT,
        tile: t,
        generator_version: 1,
        hash_version: 1,
        node_set_id: [0; 16],
        payload: encode_height_tile(&wrong),
    };
    let mut writer = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&good_neighbour).unwrap();
    writer.append(&bad).unwrap();
    writer.finish().unwrap();

    let store = TileStore::open(d.path(), w.params()).unwrap();
    let report = verify_store(&store, &w, 0);
    assert_eq!((report.checked, report.matching), (2, 1));
    assert!(!report.passed());
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

// spec: STRM-004
#[test]
fn unreadable_entries_and_foreign_kinds_are_reported_not_skipped_silently() {
    let d = Dir::new("unreadable");
    let w = world(7);
    let t = tile();
    // A record whose CRC is fine but whose payload is not a tile; and a record of a kind the verifier does not regenerate.
    let garbage = Record {
        key: w.tile_key(t, 8),
        kind: KIND_HEIGHT,
        tile: t,
        generator_version: 1,
        hash_version: 1,
        node_set_id: [0; 16],
        payload: vec![CODEC_VERSION, 8, 0, 0, 0, 1, 2, 3, 4, 9, 9, 9],
    };
    let mesh = Record {
        key: planet_cache::key::CacheKey([5; 16]),
        kind: 2,
        tile: t,
        generator_version: 1,
        hash_version: 1,
        node_set_id: [0; 16],
        payload: vec![1, 2, 3],
    };
    let mut writer = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&garbage).unwrap();
    writer.append(&mesh).unwrap();
    writer.finish().unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let report = verify_store(&store, &w, 0);
    assert_eq!((report.checked, report.unsupported, report.unreadable.len()), (2, 1, 1));
    assert!(!report.passed());
    assert!(report.unreadable[0].contains("pack-00000001.tpack") && report.unreadable[0].contains("offset"), "{}", report.unreadable[0]);
}

// spec: STRM-004
#[test]
fn a_world_needs_its_nodes_and_a_region_is_the_whole_grid() {
    let mut d = WorldDefinition::empty();
    assert!(World::from_definition(&d).is_err(), "no seed");
    d.set("world.seed", planet_world_def::NodeValue::U64(7));
    let e = World::from_definition(&d).err().expect("no radius");
    assert!(e.to_string().contains("body.0.radius_m"), "{e}");
    for level in 0..4u8 {
        let tiles = region_tiles(Face(3), level);
        assert_eq!(tiles.len(), 4usize.pow(u32::from(level)));
        assert!(tiles.iter().all(|t| t.face() == Face(3) && t.level() == level));
    }
    // The field generic generator equals the seed generator for the reference field.
    let ctx_def = WorldDefinition::single_planet(R, 7);
    let field = planet_generators::height::ReferenceGenerator.field(&GenContext::new(&ctx_def), BodyId::PLANET).unwrap();
    assert_eq!(
        generate_tile_field(field.as_ref(), &TangentWarp, tile(), 8).content_hash(),
        generate_tile(7, &TangentWarp, tile(), 8).content_hash()
    );
}
