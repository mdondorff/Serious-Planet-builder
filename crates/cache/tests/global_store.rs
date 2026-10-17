//! STRM-012: the global-coarse bake stored as blocks (ADR 0026 points 2 and 6).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::global_store::{
    block_cells, build_global, decode_block, encode_block, verify_global, BlockCodecError, GlobalWorld, BLOCK_CODEC_VERSION,
    KIND_GLOBAL_BLOCK,
};
use planet_cache::key::{encode, Kind, PARAM_GRID_RES, PARAM_RESOLUTION};
use planet_cache::pack::{PackWriter, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::global::{bake_block, GlobalSample, GLOBAL_VERSION, MAX_BLOCK_CELLS};
use planet_world_def::{BodyId, NodeValue, WorldDefinition};

const R: f64 = 6_371_000.0;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p =
            std::env::temp_dir().join(format!("planet-global-store-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

fn world(seed: u64) -> GlobalWorld {
    // A small block limit (store policy, ADR 0027 point 1) so small grids still have blocks of level above 0.
    GlobalWorld::from_definition(&WorldDefinition::single_planet(R, seed)).unwrap().with_block_limit(4)
}

fn tile() -> TileId {
    TileId::new(Face(2), 2, 1, 3).unwrap()
}

fn samples(cells: u32) -> Vec<GlobalSample> {
    let n = ((cells + 1) * (cells + 1)) as usize;
    (0..n)
        .map(|i| GlobalSample {
            continent: f32::from_bits(0x3dcc_cccd ^ (i as u32) << 3),
            elevation_m: i as f32 * 0.37 - 100.0,
            temperature_c: if i % 5 == 0 { -0.0 } else { 15.5 - i as f32 },
            precipitation_mm: f32::from_bits(0x7fc0_0001 + i as u32),
        })
        .collect()
}

fn bits(s: &GlobalSample) -> [u32; 4] {
    [s.continent.to_bits(), s.elevation_m.to_bits(), s.temperature_c.to_bits(), s.precipitation_mm.to_bits()]
}

// spec: STRM-012
#[test]
fn a_block_round_trips_bit_for_bit_including_nans_and_signed_zeros() {
    for cells in [1, 2, 8, 32] {
        let s = samples(cells);
        let payload = encode_block(cells, &s).unwrap();
        let (c, back) = decode_block(&payload).unwrap();
        assert_eq!(c, cells);
        assert_eq!(s.iter().map(bits).collect::<Vec<_>>(), back.iter().map(bits).collect::<Vec<_>>(), "{cells} cells");
        assert_eq!(block_cells(&payload).unwrap(), cells);
    }
    // A smooth real block is smaller than its raw size.
    let real = bake_block(7, &TangentWarp, tile(), 64);
    assert!(encode_block(64, &real).unwrap().len() < 16 * 65 * 65);
}

// spec: STRM-012
#[test]
fn the_payload_layout_is_pinned_by_a_hand_built_block() {
    // One cell: 4 samples; raw = four planes of 4 f32, each plane shuffled byte-wise (all first bytes, then second...).
    let s: Vec<GlobalSample> = (0..4u32)
        .map(|i| GlobalSample {
            continent: f32::from_bits(0x0102_0304 + i),
            elevation_m: f32::from_bits(0x1112_1314 + i),
            temperature_c: f32::from_bits(0x2122_2324 + i),
            precipitation_mm: f32::from_bits(0x3132_3334 + i),
        })
        .collect();
    let payload = encode_block(1, &s).unwrap();
    assert_eq!(payload[0], BLOCK_CODEC_VERSION);
    assert_eq!(u32::from_le_bytes(payload[1..5].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(payload[5..9].try_into().unwrap()), 64);
    let mut raw = Vec::new();
    for base in [0x0102_0304u32, 0x1112_1314, 0x2122_2324, 0x3132_3334] {
        // Little-endian bytes of base + i for i in 0..4: byte 0 is 04..07, byte 1 is 03, byte 2 is 02, byte 3 is 01 (and so on).
        let b = |k: usize| (0..4u32).map(|i| (base + i).to_le_bytes()[k]).collect::<Vec<u8>>();
        for k in 0..4 {
            raw.extend(b(k));
        }
    }
    assert_eq!(lz4_flex::decompress(&payload[9..], 64).unwrap(), raw);
}

// spec: STRM-012
#[test]
fn damaged_or_hostile_payloads_are_errors_and_the_encoder_refuses_what_the_decoder_would() {
    let good = encode_block(4, &samples(4)).unwrap();
    assert_eq!(decode_block(&good[..5]).unwrap_err(), BlockCodecError::Short);
    let mut v = good.clone();
    v[0] = 9;
    assert_eq!(decode_block(&v).unwrap_err(), BlockCodecError::Version(9));
    for cells in [0u32, 3, MAX_BLOCK_CELLS * 2, u32::MAX] {
        let mut c = good.clone();
        c[1..5].copy_from_slice(&cells.to_le_bytes());
        assert_eq!(decode_block(&c).unwrap_err(), BlockCodecError::Cells(cells), "a hostile header never allocates");
    }
    let mut raw = good.clone();
    raw[5..9].copy_from_slice(&1u32.to_le_bytes());
    assert!(matches!(decode_block(&raw).unwrap_err(), BlockCodecError::RawLength { .. }));
    let mut cut = good.clone();
    cut.truncate(good.len() - 3);
    assert!(decode_block(&cut).is_err());
    assert!(matches!(encode_block(3, &samples(4)).unwrap_err(), BlockCodecError::Cells(3)));
    assert!(matches!(encode_block(4, &samples(2)).unwrap_err(), BlockCodecError::Samples { .. }));
}

// spec: STRM-012
#[test]
fn a_block_key_covers_the_tile_the_cells_the_grid_and_the_stage_key_and_the_known_answers_are_pinned() {
    let w = world(7);
    let (res, cells) = (32u32, 8u32);
    let base = w.block_key(tile(), cells);
    assert_eq!(base, w.block_key(tile(), cells));
    let other_tile = TileId::new(Face(2), 2, 1, 2).unwrap();
    assert_ne!(base, w.block_key(other_tile, cells), "two blocks of one bake have different keys");
    assert_ne!(base, w.block_key(tile(), 4), "B is an output parameter");
    assert_ne!(base, w.block_key(TileId::new(Face(2), 3, 1, 3).unwrap(), cells), "the grid (cells << level) is an output parameter");
    assert_ne!(base, world(8).block_key(tile(), cells), "another seed");
    let mut two = WorldDefinition::single_planet(R, 7);
    two.set_body_radius(BodyId(1), R);
    let moon = GlobalWorld::from_definition_of_body(&two, BodyId(1)).unwrap();
    assert_ne!(base, moon.block_key(tile(), cells), "another body");
    // The key is the ADR 0018 encoding of kind 3: source key, B under tag 1, res under tag 4, empty read set.
    let inputs = w.block_key_inputs(tile(), cells);
    assert_eq!(inputs.kind, Kind::GlobalBlock);
    assert_eq!(inputs.source, Some(w.stage_key(res)), "the bake's one stage key is the source");
    assert_eq!(inputs.output_params.get(&PARAM_RESOLUTION), Some(&cells.to_le_bytes().to_vec()));
    assert_eq!(inputs.output_params.get(&PARAM_GRID_RES), Some(&res.to_le_bytes().to_vec()));
    assert!(inputs.reads.is_empty());
    assert_eq!(encode(&inputs)[..2], [planet_cache::key::KEY_FORMAT_TAG, 3]);
    // The stage key does not depend on the tile or B: it is the bake's, and changes with res and seed.
    assert_ne!(w.stage_key(32), w.stage_key(64));
    assert_ne!(w.stage_key(32), world(8).stage_key(32));
    assert_eq!(KIND_GLOBAL_BLOCK, 3);
    assert_eq!(GLOBAL_VERSION, 3, "the known answers belong to this version");
    assert_eq!(w.stage_key(32).to_hex(), KNOWN_STAGE_KEY);
    assert_eq!(base.to_hex(), KNOWN_BLOCK_KEY);
}

// spec: STRM-012
#[test]
fn the_node_set_id_covers_the_seed_reads_only_and_is_pinned() {
    let w = world(7);
    // ADR 0026 (consequences), written out from the ADR's text: the body's seed node path only, `world.seed` for body 0,
    // as its u32 little-endian byte length and its bytes; no radius node, no derived seed.
    let material = b"\x0a\x00\x00\x00world.seed".to_vec();
    assert_eq!(w.node_set_id()[..], blake3::derive_key("planet-builder node set v1", &material)[..16]);
    // Another body: `derived.body.N.seed` only.
    let mut def = WorldDefinition::single_planet(R, 7);
    def.set_body_radius(BodyId(1), R);
    let moon = GlobalWorld::from_definition_of_body(&def, BodyId(1)).unwrap();
    let material = b"\x13\x00\x00\x00derived.body.1.seed".to_vec();
    assert_eq!(moon.node_set_id()[..], blake3::derive_key("planet-builder node set v1", &material)[..16]);
    assert_ne!(moon.node_set_id(), w.node_set_id());
    assert_eq!(world(8).node_set_id(), w.node_set_id(), "a value change of a node leaves the id alone");
    let hex: String = w.node_set_id().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, KNOWN_NODE_SET_ID);
}

// spec: STRM-012
#[test]
fn a_bake_is_built_once_reused_afterwards_and_verifies_clean() {
    let d = Dir::new("build");
    let w = world(7);
    let r = build_global(d.path(), &w, 8, 3, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((r.written, r.reused), (24, 0));
    assert!(r.peak_resident_blocks <= 3);
    let again = build_global(d.path(), &w, 8, 3, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((again.written, again.reused), (0, 24), "a second run computes and writes nothing");

    let store = TileStore::open(d.path(), w.params()).unwrap();
    assert_eq!(store.len(), 24);
    let all = verify_global(&store, &w, 0);
    assert_eq!((all.checked, all.matching, all.stale, all.unexplained_stale, all.unsupported), (24, 24, 0, 0, 0));
    assert!(all.passed());
    assert_eq!(verify_global(&store, &w, 5).checked, 5);

    // Every block is the pointwise bake of its tile, bit for bit.
    let t = TileId::new(Face(3), 1, 1, 0).unwrap();
    let rec = store.get(KIND_GLOBAL_BLOCK, t, w.block_key(t, 4)).unwrap().expect("stored under the world's key");
    let (_, kept) = decode_block(&rec.payload).unwrap();
    let fresh = bake_block(w.seed(), &TangentWarp, t, 4);
    assert_eq!(kept.iter().map(bits).collect::<Vec<_>>(), fresh.iter().map(bits).collect::<Vec<_>>());
    assert_eq!(rec.node_set_id, w.node_set_id());

    // Another seed: every block is stale, nothing was compared; building it into the same directory adds its blocks.
    let other = world(8);
    let store = TileStore::open(d.path(), other.params()).unwrap();
    let r = verify_global(&store, &other, 0);
    assert_eq!((r.checked, r.matching, r.unexplained_stale), (24, 0, 24));
    assert!(r.compared_nothing() && !r.passed());
}

// spec: STRM-012
#[test]
fn a_block_made_with_an_input_that_is_not_in_its_key_or_damaged_is_reported() {
    let d = Dir::new("bad");
    let w = world(7);
    let t = TileId::new(Face(0), 1, 0, 0).unwrap();
    let good = w.block_record(t, 4, &bake_block(7, &TangentWarp, t, 4)).unwrap();
    // Right key and metadata, payload made with another seed.
    let mut bad = w
        .block_record(TileId::new(Face(0), 1, 1, 0).unwrap(), 4, &bake_block(8, &TangentWarp, TileId::new(Face(0), 1, 1, 0).unwrap(), 4))
        .unwrap();
    bad.node_set_id = w.node_set_id();
    // A payload that does not decode.
    let mut junk = w
        .block_record(TileId::new(Face(0), 1, 0, 1).unwrap(), 4, &bake_block(7, &TangentWarp, TileId::new(Face(0), 1, 0, 1).unwrap(), 4))
        .unwrap();
    junk.payload[0] = 9;
    let mut writer = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    for r in [&good, &bad, &junk] {
        writer.append(r).unwrap();
    }
    writer.finish().unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let r = verify_global(&store, &w, 0);
    assert_eq!((r.checked, r.matching, r.mismatches.len(), r.unreadable.len()), (3, 1, 1, 1), "{r:?}");
    assert!(
        r.mismatches[0].detail.contains("sample (") && r.mismatches[0].detail.contains("missing from its key"),
        "{}",
        r.mismatches[0].detail
    );
    assert!(!r.passed() && !r.passed_allowing_stale());
    // A store holding no blocks compares nothing and fails (ADR 0025 point 4).
    let empty = Dir::new("empty");
    fs::create_dir_all(empty.path()).unwrap();
    let r = verify_global(&TileStore::open(empty.path(), w.params()).unwrap(), &w, 0);
    assert!(r.compared_nothing() && !r.passed() && !r.passed_allowing_stale());
}

// spec: STRM-012
#[test]
fn a_bad_shape_is_refused_before_anything_is_created_and_a_wrong_cells_record_is_written_again() {
    let w = world(7);
    let d = Dir::new("refuse");
    let target = d.path().join("never");
    for res in [0u32, 3, 100, 1 << 17] {
        assert!(build_global(&target, &w, res, 2, DEFAULT_MAX_PACK_BYTES).is_err(), "res {res}");
    }
    assert!(!target.exists(), "a refused build leaves no directory, lock or pack behind");

    // A record under the key of a 4-cell block whose payload has 8 cells is a wrongly written record: a rebuild does not
    // keep it, it writes the block again.
    let t = TileId::new(Face(0), 1, 0, 0).unwrap();
    let mut wrong = w.block_record(t, 4, &bake_block(7, &TangentWarp, t, 4)).unwrap();
    wrong.payload = encode_block(8, &bake_block(7, &TangentWarp, t, 8)).unwrap();
    let dir = Dir::new("wrong-cells");
    let mut writer = PackWriter::open(dir.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&wrong).unwrap();
    writer.finish().unwrap();
    let r = build_global(dir.path(), &w, 8, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((r.written, r.reused), (24, 0), "the wrong record was not reused");
    let store = TileStore::open(dir.path(), w.params()).unwrap();
    let rec = store.get(KIND_GLOBAL_BLOCK, t, w.block_key(t, 4)).unwrap().unwrap();
    assert_eq!(block_cells(&rec.payload).unwrap(), 4, "the later, correct record wins");

    // A forged record whose cells and level make no grid is unreadable for verify, never keyed with a wrapped shift.
    let forged_tile = TileId::new(Face(0), 22, 5, 5).unwrap();
    let mut forged = w.block_record(t, 4, &bake_block(7, &TangentWarp, t, 4)).unwrap();
    forged.tile = forged_tile;
    forged.payload = encode_block(
        1024,
        &vec![GlobalSample { continent: 0.0, elevation_m: 0.0, temperature_c: 0.0, precipitation_mm: 0.0 }; 1025 * 1025],
    )
    .unwrap();
    let dir = Dir::new("forged");
    let mut writer = PackWriter::open(dir.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&forged).unwrap();
    writer.finish().unwrap();
    let r = verify_global(&TileStore::open(dir.path(), w.params()).unwrap(), &w, 0);
    assert_eq!((r.checked, r.unreadable.len()), (1, 1), "{r:?}");
    assert!(r.unreadable[0].contains("is not a grid"), "{}", r.unreadable[0]);
    assert!(matches!(w.block_record(forged_tile, 1024, &[]), Err(BlockCodecError::Grid { level: 22, cells: 1024 })));
}

// spec: STRM-012
#[test]
fn the_stored_bake_reads_like_the_in_memory_bake_and_a_missing_block_is_an_error_naming_it() {
    use planet_generators::global::{bake, read_lattice, read_tile_sample, GridSource, LatticeSource};
    let d = Dir::new("source");
    let w = world(7);
    // 16 cells per face edge in blocks of 4: level 2, 96 blocks, so block borders and face edges are all exercised.
    build_global(d.path(), &w, 16, 3, DEFAULT_MAX_PACK_BYTES).unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let src = planet_cache::global_store::StoreSource::new(&store, &w, 16).unwrap();
    let grid = bake(7, &TangentWarp, 16, 2).unwrap();
    let mem = GridSource(&grid);
    assert_eq!(src.res(), 16);
    for f in 0..6u8 {
        // Every lattice vertex, including the far edges (the last block of a row holds them).
        for j in 0..=16 {
            for i in 0..=16 {
                assert_eq!(bits(&src.at(Face(f), i, j).unwrap()), bits(&mem.at(Face(f), i, j).unwrap()), "face {f} vertex ({i}, {j})");
            }
        }
        // Reads between lattice points and on a finer target lattice, through the stored blocks.
        for (u, v, n) in [(5i64, 37i64, 6u32), (64, 0, 6), (0, 63, 6), (33, 33, 7), (-1, 70, 6)] {
            assert_eq!(
                bits(&read_lattice(&src, Face(f), u, v, n).unwrap()),
                bits(&read_lattice(&mem, Face(f), u, v, n).unwrap()),
                "face {f} ({u}, {v}) at 2^{n}"
            );
        }
        let t = TileId::new(Face(f), 3, 2, 5).unwrap();
        assert_eq!(bits(&read_tile_sample(&src, t, 8, 3, 8).unwrap()), bits(&read_tile_sample(&mem, t, 8, 3, 8).unwrap()));
    }
    // A store without the blocks: the error names the block, its key and why.
    let empty = Dir::new("source-empty");
    fs::create_dir_all(empty.path()).unwrap();
    let nothing = TileStore::open(empty.path(), w.params()).unwrap();
    let missing = planet_cache::global_store::StoreSource::new(&nothing, &w, 16).unwrap();
    let e = missing.at(Face(2), 5, 9).unwrap_err().to_string();
    assert!(
        e.contains("not stored under the current key")
            && e.contains("face 2 level 2 x 1 y 2")
            && e.contains(&w.block_key(TileId::new(Face(2), 2, 1, 2).unwrap(), 4).to_hex()),
        "{e}"
    );
    assert!(planet_cache::global_store::StoreSource::new(&nothing, &w, 100).is_err());
    assert!(src.at(Face(0), 17, 0).is_err());
}

// spec: STRM-012
#[test]
fn only_the_canonical_layout_of_a_grid_is_built_keyed_or_accepted() {
    // ADR 0027 point 1: blocks of min(res, limit) cells, the tiles of the level that covers the grid.
    let real = GlobalWorld::from_definition(&WorldDefinition::single_planet(R, 7)).unwrap();
    for (res, want) in [(1u32, (0u8, 1u32)), (256, (0, 256)), (1024, (0, 1024)), (2048, (1, 1024)), (16384, (4, 1024))] {
        assert_eq!(real.layout(res).unwrap(), want, "res {res}");
    }
    assert_eq!(world(7).layout(8).unwrap(), (1, 4), "a store policy of 4 cells per block");
    for res in [0u32, 3, 1 << 17] {
        assert!(real.layout(res).is_err(), "res {res}");
    }
    // A block of another layout (4 cells at level 1 is a grid of 8 cells, whose canonical layout is one block per face at the
    // real limit) is refused by the keyer and unreadable for verify, so a grid is never stored twice.
    let t = TileId::new(Face(0), 1, 0, 0).unwrap();
    assert!(matches!(real.block_record(t, 4, &bake_block(7, &TangentWarp, t, 4)), Err(BlockCodecError::Grid { level: 1, cells: 4 })));
    let forged = world(7).block_record(t, 4, &bake_block(7, &TangentWarp, t, 4)).unwrap();
    let d = Dir::new("noncanonical");
    let mut writer = PackWriter::open(d.path(), real.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&forged).unwrap();
    writer.finish().unwrap();
    let r = verify_global(&TileStore::open(d.path(), real.params()).unwrap(), &real, 0);
    assert_eq!((r.checked, r.unreadable.len()), (1, 1), "{r:?}");
    assert!(r.unreadable[0].contains("not the canonical layout"), "{}", r.unreadable[0]);
}

// spec: STRM-012
#[test]
fn another_body_or_a_missing_node_is_refused_and_the_radius_is_in_the_key_but_not_the_node_set() {
    let mut def = WorldDefinition::single_planet(R, 7);
    let a = GlobalWorld::from_definition(&def).unwrap();
    def.set_body_radius(BodyId::PLANET, R * 2.0);
    let b = GlobalWorld::from_definition(&def).unwrap();
    assert_eq!(a.node_set_id(), b.node_set_id(), "the radius is not a node the field reads");
    assert_ne!(a.stage_key(8), b.stage_key(8), "the body parameter hash covers the radius (ADR 0018)");
    def.set(WorldDefinition::SEED_PATH, NodeValue::U64(9));
    assert_ne!(GlobalWorld::from_definition(&def).unwrap().seed(), a.seed());
    assert!(GlobalWorld::from_definition(&WorldDefinition::empty()).is_err());
}

// Both keys moved with GLOBAL_VERSION 3 (latitude from the +z pole): the stage key names the bake version.
const KNOWN_STAGE_KEY: &str = "c91fccae01d58cf954c7c896c5fc4a97";
const KNOWN_BLOCK_KEY: &str = "8a2519d5ec1cc5cdba280e8a56994658";
const KNOWN_NODE_SET_ID: &str = "92572f1974620d913454c89a22aca7be";
