//! STRM-012: the global-coarse bake stored as blocks (ADR 0026 points 2 and 6).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::global_store::{
    block_cells, build_global, decode_block, decode_ocean_block, encode_block, encode_ocean_block, ocean_table_of_store, verify_global,
    BlockCodecError, GlobalWorld, StoreOceanSource, BLOCK_CODEC_VERSION, KIND_GLOBAL_BLOCK, KIND_GLOBAL_OCEAN, OCEAN_CODEC_VERSION,
};
use planet_cache::key::{encode, Kind, PARAM_GRID_RES, PARAM_OCEAN_AREA_FRACTION, PARAM_RESOLUTION};
use planet_cache::pack::{PackWriter, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::global::{bake, bake_block, GlobalSample, GLOBAL_VERSION, MAX_BLOCK_CELLS};
use planet_generators::ocean::{reference_ocean, tiled_ocean, OceanBits, OceanLattice, EARTH_OCEAN_MIN_AREA_FRACTION};
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
    assert_eq!(GLOBAL_VERSION, 5, "the known answers belong to this version");
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
    assert_eq!(
        store.len(),
        73,
        "24 blocks, the 24 blocks of their ocean channel (STRM-023), their 24 wet-floor blocks (STRM-032) and the water summary (STRM-025)"
    );
    let all = verify_global(&store, &w, 0);
    assert_eq!((all.checked, all.matching, all.stale, all.unexplained_stale, all.unsupported), (73, 73, 0, 0, 0));
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
    assert_eq!((r.checked, r.matching, r.unexplained_stale), (73, 0, 73));
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

// Both keys moved with GLOBAL_VERSION 4 (the ocean channel; 3 was the latitude from the +z pole): the stage key names the bake version.
const KNOWN_STAGE_KEY: &str = "9f1fce5d0a4efe935a8fda701db5318a";
const KNOWN_BLOCK_KEY: &str = "bc2026919007000277f1e34774425262";
const KNOWN_NODE_SET_ID: &str = "92572f1974620d913454c89a22aca7be";

// spec: STRM-023
#[test]
fn an_ocean_block_key_covers_the_tile_the_grid_the_threshold_and_the_stage_key() {
    let w = world(7);
    let (res, cells) = (32u32, 8u32);
    let base = w.ocean_key(tile(), cells);
    assert_ne!(base, w.block_key(tile(), cells), "the channel and the samples of one block have different keys");
    assert_ne!(base, w.ocean_key(TileId::new(Face(2), 2, 1, 2).unwrap(), cells));
    assert_ne!(base, w.ocean_key(tile(), 4));
    assert_ne!(base, w.ocean_key(TileId::new(Face(2), 3, 1, 3).unwrap(), cells));
    assert_ne!(base, world(8).ocean_key(tile(), cells), "another seed is another bake");
    assert_ne!(base, world(7).with_ocean_area_fraction(0.01).ocean_key(tile(), cells), "the area threshold is an output parameter");
    let inputs = w.ocean_key_inputs(tile(), cells);
    assert_eq!(inputs.kind, Kind::GlobalOcean);
    assert_eq!(inputs.source, Some(w.stage_key(res)), "the bake's one stage key is the source");
    assert_eq!(inputs.output_params.get(&PARAM_RESOLUTION), Some(&cells.to_le_bytes().to_vec()));
    assert_eq!(inputs.output_params.get(&PARAM_GRID_RES), Some(&res.to_le_bytes().to_vec()));
    assert_eq!(inputs.output_params.get(&PARAM_OCEAN_AREA_FRACTION), Some(&EARTH_OCEAN_MIN_AREA_FRACTION.to_bits().to_le_bytes().to_vec()));
    assert!(inputs.reads.is_empty());
    assert_eq!(encode(&inputs)[..2], [planet_cache::key::KEY_FORMAT_TAG, 4]);
    assert_eq!(KIND_GLOBAL_OCEAN, 4);
    assert_eq!(base.to_hex(), KNOWN_OCEAN_KEY);
    // The sea nodes of the definition set the threshold; a world that sets none has the Earth default.
    let mut def = WorldDefinition::single_planet(R, 7);
    assert_eq!(GlobalWorld::from_definition(&def).unwrap().sea().min_area_fraction, EARTH_OCEAN_MIN_AREA_FRACTION);
    def.set(&planet_world_def::body_sea_min_area_path(BodyId::PLANET), NodeValue::F64(0.01));
    let set = GlobalWorld::from_definition(&def).unwrap().with_block_limit(4);
    assert_eq!(set.ocean_key(tile(), cells), world(7).with_ocean_area_fraction(0.01).ocean_key(tile(), cells));
    assert_eq!(set.block_key(tile(), cells), world(7).block_key(tile(), cells), "the sea is not an input of the bake");
    assert_eq!(set.stage_key(32), world(7).stage_key(32));
    def.set(&planet_world_def::body_sea_min_area_path(BodyId::PLANET), NodeValue::F64(2.0));
    assert!(GlobalWorld::from_definition(&def).is_err(), "a fraction above 1 names its node");
}

// spec: STRM-023
#[test]
fn an_ocean_block_round_trips_and_a_hostile_payload_is_refused_without_allocating() {
    for cells in [1u32, 2, 4, 8, 32] {
        let mut b = OceanBits::zeros(cells);
        for l in 0..=cells {
            for k in 0..=cells {
                if (k * 7 + l * 3) % 5 < 2 {
                    b.set(k, l);
                }
            }
        }
        let bytes = encode_ocean_block(&b).unwrap();
        assert_eq!(bytes[0], OCEAN_CODEC_VERSION);
        assert_eq!(u32::from_le_bytes(bytes[1..5].try_into().unwrap()), cells);
        assert_eq!(u64::from(u32::from_le_bytes(bytes[5..9].try_into().unwrap())), (u64::from(cells) + 1).pow(2).div_ceil(8));
        assert_eq!(decode_ocean_block(&bytes).unwrap(), b, "cells {cells}");
    }
    // A one-cell block written out by hand: four vertices, bits 0 and 3 set, one byte 0b1001.
    let mut one = OceanBits::zeros(1);
    one.set(0, 0);
    one.set(1, 1);
    let bytes = encode_ocean_block(&one).unwrap();
    assert_eq!(bytes[..9], [1, 1, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(lz4_flex::decompress(&bytes[9..], 1).unwrap(), [0b1001]);
    let good = encode_ocean_block(&OceanBits::zeros(4)).unwrap();
    assert!(matches!(decode_ocean_block(&good[..5]), Err(BlockCodecError::Short)));
    let mut v = good.clone();
    v[0] = 2;
    assert_eq!(decode_ocean_block(&v).unwrap_err(), BlockCodecError::OceanVersion(2));
    for cells in [0u32, 3, 2048, u32::MAX] {
        let mut c = good.clone();
        c[1..5].copy_from_slice(&cells.to_le_bytes());
        assert_eq!(decode_ocean_block(&c).unwrap_err(), BlockCodecError::Cells(cells), "a hostile header never allocates");
    }
    let mut raw = good.clone();
    raw[5..9].copy_from_slice(&1u32.to_le_bytes());
    assert!(matches!(decode_ocean_block(&raw).unwrap_err(), BlockCodecError::RawLength { .. }));
    assert!(decode_ocean_block(&good[..good.len() - 1]).is_err());
    // A bit set beyond the last vertex of 4 cells (25 bits in 4 bytes) is damage.
    let mut spare = Vec::from(&good[..9]);
    spare.extend(lz4_flex::compress(&[0, 0, 0, 0x80]));
    assert_eq!(decode_ocean_block(&spare).unwrap_err(), BlockCodecError::OceanBits);
    assert!(matches!(encode_ocean_block(&OceanBits::zeros(3)), Err(BlockCodecError::Cells(3))));
}

// spec: STRM-023
#[test]
fn the_build_stores_the_ocean_channel_once_and_the_store_reads_it_like_the_in_memory_computation() {
    let d = Dir::new("ocean-build");
    let w = world(7);
    let r = build_global(d.path(), &w, 16, 3, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((r.written, r.reused, r.ocean_written, r.ocean_reused), (96, 0, 96, 0));
    assert!(r.ocean_components >= 1 && r.ocean_oceans >= 1 && r.ocean_merger_bytes > 0 && r.peak_resident_blocks <= 3);
    let again = build_global(d.path(), &w, 16, 3, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((again.written, again.reused, again.ocean_written, again.ocean_reused), (0, 96, 0, 96), "a second run writes nothing");
    assert_eq!(again.ocean_components, 0, "and does not recompute the table");

    let store = TileStore::open(d.path(), w.params()).unwrap();
    // The stored channel equals the in-memory computation and the serial reference, vertex by vertex.
    let grid = bake(7, &TangentWarp, 16, 2).unwrap();
    let fraction = w.sea().min_area_fraction;
    let (mem_table, mem) = tiled_ocean(&grid, &TangentWarp, 4, 2, fraction).unwrap();
    let (ref_table, reference) = reference_ocean(&grid, &TangentWarp, fraction).unwrap();
    assert_eq!(mem, reference);
    let table = ocean_table_of_store(&store, &w, 16, 2).unwrap();
    assert_eq!(table, mem_table);
    assert_eq!(table.hash(), ref_table.hash());
    assert_eq!((r.ocean_components, r.ocean_oceans), (table.components.len(), table.ocean_components().count()));
    let src = StoreOceanSource::new(&store, &w, 16).unwrap();
    assert_eq!(src.res(), 16);
    for f in 0..6u8 {
        for j in 0..=16 {
            for i in 0..=16 {
                assert_eq!(src.is_ocean(Face(f), i, j).unwrap(), mem.get(Face(f), i, j), "face {f} vertex ({i}, {j})");
            }
        }
    }
    assert!(src.is_ocean(Face(0), 17, 0).is_err());
    assert!(src.decodes() >= 1 && src.resident_blocks() <= 64);
    assert_eq!(verify_global(&store, &w, 0).matching, 289, "192 blocks, ocean blocks and floor blocks and the summary");
    // Thread count and batch size do not change a bit of the stored channel.
    let d2 = Dir::new("ocean-build-1");
    build_global(d2.path(), &w, 16, 1, DEFAULT_MAX_PACK_BYTES).unwrap();
    let store2 = TileStore::open(d2.path(), w.params()).unwrap();
    for f in 0..6u8 {
        for y in 0..4 {
            for x in 0..4 {
                let t = TileId::new(Face(f), 2, x, y).unwrap();
                let a = store.get(KIND_GLOBAL_OCEAN, t, w.ocean_key(t, 4)).unwrap().unwrap();
                let b = store2.get(KIND_GLOBAL_OCEAN, t, w.ocean_key(t, 4)).unwrap().unwrap();
                assert_eq!(a.payload, b.payload, "face {f} block ({x}, {y})");
            }
        }
    }
}

// spec: STRM-023
#[test]
fn another_threshold_adds_the_ocean_blocks_from_the_stored_bake_without_baking_again() {
    let d = Dir::new("ocean-threshold");
    let w = world(7);
    build_global(d.path(), &w, 8, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    let all_sea = world(7).with_ocean_area_fraction(0.0);
    let r = build_global(d.path(), &all_sea, 8, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((r.written, r.reused, r.ocean_written, r.ocean_reused), (0, 24, 24, 0), "the blocks are reused, the channel is new");
    assert!(r.ocean_components > 0 && r.ocean_oceans == r.ocean_components, "a threshold of 0 makes every sea component ocean");
    let store = TileStore::open(d.path(), w.params()).unwrap();
    // Both channels live side by side under their own keys and read their own answers.
    let (a, b) = (StoreOceanSource::new(&store, &w, 8).unwrap(), StoreOceanSource::new(&store, &all_sea, 8).unwrap());
    let grid = bake(7, &TangentWarp, 8, 1).unwrap();
    let (_, strict) = tiled_ocean(&grid, &TangentWarp, 4, 1, w.sea().min_area_fraction).unwrap();
    let (_, loose) = tiled_ocean(&grid, &TangentWarp, 4, 1, 0.0).unwrap();
    for f in 0..6u8 {
        for j in 0..=8 {
            for i in 0..=8 {
                assert_eq!(a.is_ocean(Face(f), i, j).unwrap(), strict.get(Face(f), i, j));
                assert_eq!(b.is_ocean(Face(f), i, j).unwrap(), loose.get(Face(f), i, j));
            }
        }
    }
    // A source over a store that lacks the channel names the block and its key.
    let empty = Dir::new("ocean-empty");
    fs::create_dir_all(empty.path()).unwrap();
    let nothing = TileStore::open(empty.path(), w.params()).unwrap();
    let missing = StoreOceanSource::new(&nothing, &w, 8).unwrap();
    let e = missing.is_ocean(Face(2), 5, 7).unwrap_err().to_string();
    assert!(
        e.contains("ocean block")
            && e.contains("not stored under the current key")
            && e.contains(&w.ocean_key(TileId::new(Face(2), 1, 1, 1).unwrap(), 4).to_hex()),
        "{e}"
    );
    assert!(StoreOceanSource::new(&nothing, &w, 100).is_err());
}

// spec: STRM-023
#[test]
fn a_wrong_or_damaged_ocean_block_is_reported_by_verify_with_its_vertex() {
    let w = world(7);
    let d = Dir::new("ocean-bad");
    build_global(d.path(), &w, 8, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    // Right key and metadata, one vertex flipped.
    let t = TileId::new(Face(1), 1, 0, 1).unwrap();
    let rec = store.get(KIND_GLOBAL_OCEAN, t, w.ocean_key(t, 4)).unwrap().unwrap();
    let mut bits = decode_ocean_block(&rec.payload).unwrap();
    if bits.get(2, 3) {
        bits.words[0] &= !(1 << (3 * 5 + 2));
    } else {
        bits.set(2, 3);
    }
    let bad = w.ocean_record(t, &bits).unwrap();
    let mut junk = rec.clone();
    junk.tile = TileId::new(Face(1), 1, 1, 1).unwrap();
    junk.key = w.ocean_key(junk.tile, 4);
    junk.payload[0] = 9;
    // Appended after the good records (the later record of a key wins).
    let mut writer = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&bad).unwrap();
    writer.append(&junk).unwrap();
    writer.finish().unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let r = verify_global(&store, &w, 0);
    assert_eq!(r.mismatches.len(), 1, "{r:?}");
    // The damaged ocean block also keeps the wet floor (once) from being recomputed (the summary reads the floor blocks, not the ocean ones): unreadable, naming the ocean block.
    assert_eq!(r.unreadable.len(), 2, "{r:?}");
    assert!(
        r.unreadable[1].contains("wet floor") && r.unreadable[1].contains("ocean block") && r.unreadable[1].contains("version 9"),
        "{}",
        r.unreadable[1]
    );
    assert!(
        r.mismatches[0].detail.contains("vertex (2, 3)") && r.mismatches[0].detail.contains("missing from its key"),
        "{}",
        r.mismatches[0].detail
    );
    assert!(r.unreadable[0].contains("ocean block") && r.unreadable[0].contains("version 9"), "{}", r.unreadable[0]);
    assert!(!r.passed());
    // Another threshold's channel is stale for this world.
    let other = world(7).with_ocean_area_fraction(0.0);
    let r = verify_global(&store, &other, 0);
    assert!(r.unexplained_stale >= 23, "{r:?}");
}

// spec: STRM-023
#[test]
fn the_ocean_channel_needs_the_blocks_and_names_the_missing_one() {
    let w = world(7);
    let d = Dir::new("ocean-partial");
    let mut writer = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    for f in 0..6u8 {
        for y in 0..2 {
            for x in 0..2 {
                if (f, x, y) == (3, 1, 1) {
                    continue;
                }
                let t = TileId::new(Face(f), 1, x, y).unwrap();
                writer.append(&w.block_record(t, 4, &bake_block(7, &TangentWarp, t, 4)).unwrap()).unwrap();
            }
        }
    }
    writer.finish().unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let e = ocean_table_of_store(&store, &w, 8, 2).unwrap_err().to_string();
    assert!(
        e.contains("not stored under the current key") && e.contains(&w.block_key(TileId::new(Face(3), 1, 1, 1).unwrap(), 4).to_hex()),
        "{e}"
    );
    // A build over it bakes the missing block and then has them all.
    let r = build_global(d.path(), &w, 8, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((r.written, r.reused, r.ocean_written), (1, 23, 24));
    assert!(ocean_table_of_store(&TileStore::open(d.path(), w.params()).unwrap(), &w, 8, 2).is_ok());
}

const KNOWN_OCEAN_KEY: &str = "a36a2a536f7b47f4b734fb245a1b8c7e";

// spec: STRM-023
// spec: STRM-013
#[test]
fn an_ocean_block_is_live_by_the_rule_of_the_sample_blocks() {
    let w = world(7);
    let t = TileId::new(Face(0), 1, 0, 0).unwrap();
    let rec = w.ocean_record(t, &OceanBits::zeros(4)).unwrap();
    assert!(w.is_live(&rec));
    let mut old = rec.clone();
    old.generator_version = GLOBAL_VERSION - 1;
    assert!(!w.is_live(&old), "an ocean block of an older global version is dead");
    let mut other_set = rec.clone();
    other_set.node_set_id = [9; 16];
    assert!(!w.is_live(&other_set));
    let mut unknown = rec;
    unknown.kind = 77;
    unknown.generator_version = 1;
    assert!(w.is_live(&unknown), "a record of a kind this world does not judge is kept");
    // The block of another layout is refused by the keyer.
    assert!(matches!(
        GlobalWorld::from_definition(&WorldDefinition::single_planet(R, 7)).unwrap().ocean_record(t, &OceanBits::zeros(4)),
        Err(BlockCodecError::Grid { .. })
    ));
}
