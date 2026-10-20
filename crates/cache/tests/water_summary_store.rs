//! STRM-025: the water summary of the global bake in the store (ADR 0058): its record, key, codec, build, migration of a store that has the
//! blocks and the ocean channel but no summary, and verification.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::global_store::{
    build_global, decode_summary, encode_summary, load_water_summary, verify_global, water_summary_of_store, BlockCodecError, GlobalWorld,
    KIND_GLOBAL_WATER_SUMMARY, SUMMARY_CODEC_VERSION,
};
use planet_cache::key::{encode, Kind, PARAM_GRID_RES, PARAM_OCEAN_AREA_FRACTION, PARAM_RESOLUTION};
use planet_cache::pack::{PackWriter, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::cube::TangentWarp;
use planet_generators::global::{bake, GLOBAL_VERSION};
use planet_generators::ocean::{tiled_ocean, EARTH_OCEAN_MIN_AREA_FRACTION};
use planet_generators::water_summary::{summary_of_grid, SummaryCell, WaterSummary, FLAG_OCEAN, FLAG_WET, SUMMARY_IMPLEMENTATION_ID};
use planet_world_def::{BodyId, LiquidKindId, SeaParams, WorldDefinition};

const R: f64 = 6_371_000.0;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "planet-water-summary-store-{}-{}-{name}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
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
    GlobalWorld::from_definition(&WorldDefinition::single_planet(R, seed)).unwrap().with_block_limit(4)
}

fn a_summary() -> WaterSummary {
    // 32 cells in blocks of 4: summary cells of 4, g = 8.
    let mut cells = vec![SummaryCell::EMPTY; 6 * 64];
    for (i, c) in cells.iter_mut().enumerate() {
        c.flags = [0, FLAG_OCEAN, FLAG_OCEAN | FLAG_WET][i % 3];
        if i % 5 == 0 {
            c.band_elevation_m = i as f32 * 0.5 - 40.0;
        }
    }
    WaterSummary::from_cells(32, 4, cells).unwrap()
}

// spec: STRM-025
#[test]
fn the_summary_payload_round_trips_and_a_hostile_payload_is_refused_without_allocating() {
    let s = a_summary();
    let bytes = encode_summary(&s).unwrap();
    assert_eq!(bytes[0], SUMMARY_CODEC_VERSION);
    assert_eq!(u32::from_le_bytes(bytes[1..5].try_into().unwrap()), 32);
    assert_eq!(u32::from_le_bytes(bytes[5..9].try_into().unwrap()), 4);
    assert_eq!(u32::from_le_bytes(bytes[9..13].try_into().unwrap()), 5 * 6 * 64);
    let back = decode_summary(&bytes).unwrap();
    assert_eq!((back.res(), back.block_cells()), (32, 4));
    assert_eq!(
        back.cells().iter().map(|c| (c.flags, c.band_elevation_m.to_bits())).collect::<Vec<_>>(),
        s.cells().iter().map(|c| (c.flags, c.band_elevation_m.to_bits())).collect::<Vec<_>>(),
        "every flag and every elevation, infinities included, bit for bit"
    );

    assert!(matches!(decode_summary(&bytes[..9]), Err(BlockCodecError::Short)));
    let mut v = bytes.clone();
    v[0] = 2;
    assert_eq!(decode_summary(&v).unwrap_err(), BlockCodecError::SummaryVersion(2));
    // A hostile resolution or block size is refused from the header, before any allocation.
    for (res, cells) in [(0u32, 4u32), (31, 4), (u32::MAX, 4), (1 << 20, 4), (32, 0), (32, 3), (32, 2048), (2, 4), (65536, 1), (65536, 2)] {
        let mut h = bytes.clone();
        h[1..5].copy_from_slice(&res.to_le_bytes());
        h[5..9].copy_from_slice(&cells.to_le_bytes());
        assert!(matches!(decode_summary(&h).unwrap_err(), BlockCodecError::Summary(_)), "res {res}, cells {cells}");
    }
    let mut raw = bytes.clone();
    raw[9..13].copy_from_slice(&7u32.to_le_bytes());
    assert!(matches!(decode_summary(&raw).unwrap_err(), BlockCodecError::RawLength { .. }));
    assert!(decode_summary(&bytes[..bytes.len() - 1]).is_err(), "a cut LZ4 block");
    // A flag bit this version does not know.
    let mut forged = Vec::from(&bytes[..13]);
    let mut planes = vec![0u8; 5 * 6 * 64];
    planes[3] = 0x80;
    forged.extend(lz4_flex::compress(&planes));
    assert!(matches!(decode_summary(&forged).unwrap_err(), BlockCodecError::Summary(m) if m.contains("flag")));
}

const KNOWN_SUMMARY_KEY: &str = "1162f2bb92e005af8ade819a1b9bf017";

// spec: STRM-025
#[test]
fn the_summary_key_covers_the_bake_the_grid_the_threshold_and_not_the_sea_level() {
    let w = world(7);
    let (res, cells) = (32u32, 4u32);
    let base = w.summary_key(res, cells);
    assert_ne!(base, w.ocean_key(GlobalWorld::summary_tile(), cells), "the summary and the root ocean block have different keys");
    assert_ne!(base, w.summary_key(res, 8));
    assert_ne!(base, w.summary_key(64, cells));
    assert_ne!(base, world(8).summary_key(res, cells), "another seed is another bake");
    assert_ne!(
        base,
        world(7).with_ocean_area_fraction(0.01).summary_key(res, cells),
        "the area threshold shapes the ocean bits it summarises"
    );
    let inputs = w.summary_key_inputs(res, cells);
    assert_eq!(inputs.kind, Kind::GlobalWaterSummary);
    assert_eq!(inputs.implementation_id, format!("{SUMMARY_IMPLEMENTATION_ID}+{}", planet_generators::ocean::OCEAN_IMPLEMENTATION_ID));
    assert_eq!(inputs.tile, GlobalWorld::summary_tile());
    assert_eq!(inputs.source, Some(w.stage_key(res)));
    assert_eq!(inputs.output_params.get(&PARAM_RESOLUTION), Some(&cells.to_le_bytes().to_vec()));
    assert_eq!(inputs.output_params.get(&PARAM_GRID_RES), Some(&res.to_le_bytes().to_vec()));
    assert_eq!(inputs.output_params.get(&PARAM_OCEAN_AREA_FRACTION), Some(&EARTH_OCEAN_MIN_AREA_FRACTION.to_bits().to_le_bytes().to_vec()));
    assert!(inputs.reads.is_empty());
    assert_eq!(encode(&inputs)[..2], [planet_cache::key::KEY_FORMAT_TAG, 5]);
    assert_eq!(KIND_GLOBAL_WATER_SUMMARY, 5);
    // The sea level and the liquid of the definition are applied when a tile is asked, so they are no input of the record.
    let mut def = WorldDefinition::single_planet(R, 7);
    def.set_sea(
        BodyId::PLANET,
        &SeaParams { level_m: 120.0, liquid: LiquidKindId::WATER, min_area_fraction: EARTH_OCEAN_MIN_AREA_FRACTION },
    );
    let high = GlobalWorld::from_definition(&def).unwrap().with_block_limit(4);
    assert_eq!(high.summary_key(res, cells), base);
    // A summary of the bake of the generator version in force.
    let r = w.summary_record(&WaterSummary::new(res, cells).unwrap()).unwrap();
    assert_eq!((r.kind, r.generator_version), (KIND_GLOBAL_WATER_SUMMARY, GLOBAL_VERSION));
    assert!(w.is_live(&r));
    let mut old = r.clone();
    old.generator_version = GLOBAL_VERSION - 1;
    assert!(!w.is_live(&old), "the summary of an older global version is dead");
    // A block size that is not the canonical layout of the grid makes no record.
    assert!(matches!(w.summary_record(&WaterSummary::new(32, 8).unwrap()), Err(BlockCodecError::Summary(_))));
    assert_eq!(base.to_hex(), KNOWN_SUMMARY_KEY);
}

// spec: STRM-025
// spec: GEN-024
#[test]
fn a_build_writes_the_summary_once_it_equals_the_in_memory_one_and_does_not_depend_on_the_thread_count() {
    let w = world(7);
    let res = 32;
    let (one, three) = (Dir::new("one"), Dir::new("three"));
    let r1 = build_global(one.path(), &w, res, 1, DEFAULT_MAX_PACK_BYTES).unwrap();
    let r3 = build_global(three.path(), &w, res, 3, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert!(r1.summary_written && r3.summary_written);
    assert_eq!(r1.summary_bytes, r3.summary_bytes);
    let (s1, s3) = (TileStore::open(one.path(), w.params()).unwrap(), TileStore::open(three.path(), w.params()).unwrap());
    let (a, b) = (load_water_summary(&s1, &w, res).unwrap(), load_water_summary(&s3, &w, res).unwrap());
    assert_eq!(a.cells(), b.cells());
    // The summary of the same bake held in memory.
    let grid = bake(w.seed(), &TangentWarp, res, 2).unwrap();
    let (_, ocean) = tiled_ocean(&grid, &TangentWarp, 4, 2, EARTH_OCEAN_MIN_AREA_FRACTION).unwrap();
    let mem = summary_of_grid(&grid, &ocean, 4, 2).unwrap();
    assert_eq!(a.cells(), mem.cells());
    assert!(a.cells().iter().any(|c| c.flags & FLAG_WET != 0) && a.cells().iter().any(|c| c.flags & FLAG_OCEAN == 0));
    assert_eq!(water_summary_of_store(&s1, &w, res, 2).unwrap().cells(), a.cells());
    // A second build writes nothing: no block, no ocean block, no summary.
    let again = build_global(one.path(), &w, res, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((again.written, again.ocean_written, again.summary_written, again.summary_bytes), (0, 0, false, 0));
    // Another threshold has another channel and so another summary, added without baking again.
    let other = world(7).with_ocean_area_fraction(0.0);
    let r = build_global(one.path(), &other, res, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((r.written, r.summary_written), (0, true));
    let s = TileStore::open(one.path(), other.params()).unwrap();
    assert!(load_water_summary(&s, &other, res).is_ok() && load_water_summary(&s, &w, res).is_ok());
}

// spec: STRM-025
#[test]
fn a_store_with_the_blocks_and_the_ocean_channel_but_no_summary_gains_only_the_summary() {
    let w = world(7);
    let res = 32;
    let full = Dir::new("full");
    build_global(full.path(), &w, res, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    // The store as an earlier build left it: every record but the summary.
    let old = Dir::new("old");
    {
        let src = TileStore::open(full.path(), w.params()).unwrap();
        let mut writer = PackWriter::open(old.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
        let mut copied = 0;
        for item in src.sample_records(0) {
            let rec = item.unwrap().record;
            if rec.kind != KIND_GLOBAL_WATER_SUMMARY {
                writer.append(&rec).unwrap();
                copied += 1;
            }
        }
        writer.finish().unwrap();
        assert_eq!(copied, 3 * 6 * 64, "the sample, ocean and wet-floor blocks of 6 faces of 8 x 8 blocks");
    }
    let store = TileStore::open(old.path(), w.params()).unwrap();
    let e = load_water_summary(&store, &w, res).unwrap_err().to_string();
    assert!(e.contains("not stored under the current key") && e.contains("generate --global-dir"), "{e}");
    let r = build_global(old.path(), &w, res, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((r.written, r.reused, r.ocean_written, r.ocean_reused), (0, 6 * 64, 0, 6 * 64), "no block is baked or analysed again");
    assert!(r.summary_written);
    let migrated = load_water_summary(&TileStore::open(old.path(), w.params()).unwrap(), &w, res).unwrap();
    let fresh = load_water_summary(&TileStore::open(full.path(), w.params()).unwrap(), &w, res).unwrap();
    assert_eq!(migrated.cells(), fresh.cells(), "the migration writes what a build writes");
    let again = build_global(old.path(), &w, res, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert!(!again.summary_written, "idempotent");
}

// spec: STRM-025
#[test]
fn verify_names_the_cell_of_a_wrong_summary_a_damaged_one_and_the_stale_one() {
    let w = world(7);
    let res = 32;
    let d = Dir::new("verify");
    build_global(d.path(), &w, res, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let r = verify_global(&store, &w, 0);
    assert!(r.passed() && r.mismatches.is_empty() && r.unreadable.is_empty(), "{r:?}");
    let good = load_water_summary(&store, &w, res).unwrap();
    // One cell wrong under the right key and metadata (appended after the good record: the later record of a key wins).
    let mut cells = good.cells().to_vec();
    cells[10].flags ^= FLAG_WET;
    let wrong = w.summary_record(&WaterSummary::from_cells(res, 4, cells).unwrap()).unwrap();
    let mut writer = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&wrong).unwrap();
    writer.finish().unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let r = verify_global(&store, &w, 0);
    assert_eq!(r.mismatches.len(), 1, "{r:?}");
    assert!(
        r.mismatches[0].detail.contains("cell 10") && r.mismatches[0].detail.contains("missing from its key"),
        "{}",
        r.mismatches[0].detail
    );
    // A damaged payload is unreadable.
    let mut junk = wrong.clone();
    junk.payload[0] = 9;
    let mut writer = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&junk).unwrap();
    writer.finish().unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let r = verify_global(&store, &w, 0);
    assert!(r.unreadable.iter().any(|u| u.contains("water summary") && u.contains("version 9")), "{r:?}");
    // The summary of another threshold is stale for this world.
    let other = world(7).with_ocean_area_fraction(0.0);
    let r = verify_global(&store, &other, 0);
    assert!(r.unexplained_stale >= 1, "{r:?}");
}
