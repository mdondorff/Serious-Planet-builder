//! STRM-032: the wet-floor channel of the global bake in the store (ADR 0071): its record, key, codec, build, reader and verification.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::global_store::{
    build_global, decode_floor_block, encode_floor_block, verify_global, BlockCodecError, GlobalWorld, StoreWetFloorSource,
    FLOOR_CODEC_VERSION, KIND_GLOBAL_WET_FLOOR,
};
use planet_cache::pack::{TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::global::{bake, GLOBAL_VERSION};
use planet_generators::ocean::{tiled_ocean, EARTH_OCEAN_MIN_AREA_FRACTION};
use planet_generators::wet_floor::{tiled_floors, FloorLattice, INF};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p =
            std::env::temp_dir().join(format!("planet-wet-floor-store-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

fn codes(cells: u32) -> Vec<u16> {
    (0..(cells + 1) * (cells + 1)).map(|i| [0u16, 24_000, 24_128, INF, 1, 0xFFFE][i as usize % 6]).collect()
}

// spec: STRM-032
#[test]
fn the_floor_payload_round_trips_and_a_hostile_payload_is_refused_without_allocating() {
    let c = codes(4);
    let bytes = encode_floor_block(4, &c).unwrap();
    assert_eq!(bytes[0], FLOOR_CODEC_VERSION);
    assert_eq!(u32::from_le_bytes(bytes[1..5].try_into().unwrap()), 4);
    assert_eq!(u32::from_le_bytes(bytes[5..9].try_into().unwrap()), 2 * 25);
    assert_eq!(decode_floor_block(&bytes).unwrap(), (4, c.clone()), "every code, the sentinels included");

    assert!(matches!(decode_floor_block(&bytes[..5]), Err(BlockCodecError::Short)));
    let mut v = bytes.clone();
    v[0] = 9;
    assert!(matches!(decode_floor_block(&v), Err(BlockCodecError::FloorVersion(9))));
    let mut v = bytes.clone();
    v[1..5].copy_from_slice(&3u32.to_le_bytes());
    assert!(matches!(decode_floor_block(&v), Err(BlockCodecError::Cells(3))));
    let mut v = bytes.clone();
    v[1..5].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(decode_floor_block(&v), Err(BlockCodecError::Cells(_))), "a huge edge is refused before anything is allocated");
    let mut v = bytes.clone();
    v[5..9].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(decode_floor_block(&v), Err(BlockCodecError::RawLength { .. })));
    assert!(decode_floor_block(&bytes[..bytes.len() - 3]).is_err(), "a truncated LZ4 block");
    assert!(matches!(encode_floor_block(4, &c[1..]), Err(BlockCodecError::Samples { .. })));
    assert!(matches!(encode_floor_block(3, &c), Err(BlockCodecError::Cells(3))));
}

// spec: STRM-032
#[test]
fn the_floor_record_has_its_own_kind_and_key_and_goes_dead_with_the_global_version() {
    let w = world(7);
    let tile = TileId::new(Face(1), 3, 2, 5).unwrap();
    let r = w.floor_record(tile, 4, &codes(4)).unwrap();
    assert_eq!((r.kind, r.generator_version, r.tile), (KIND_GLOBAL_WET_FLOOR, GLOBAL_VERSION, tile));
    assert_eq!(KIND_GLOBAL_WET_FLOOR, 6);
    assert_ne!(r.key, w.ocean_key(tile, 4), "not the ocean block's key");
    assert_ne!(r.key, w.block_key(tile, 4));
    assert_ne!(w.floor_key(tile, 4), world(8).floor_key(tile, 4), "another seed, another bake, another channel");
    assert_ne!(
        w.floor_key(tile, 4),
        world(7).with_ocean_area_fraction(0.0).floor_key(tile, 4),
        "the area threshold decides the ocean bits"
    );
    assert!(w.is_live(&r));
    let mut old = r.clone();
    old.generator_version = GLOBAL_VERSION - 1;
    assert!(!w.is_live(&old));
    // A block size that is not the canonical layout of its grid makes no record.
    assert!(matches!(w.floor_record(tile, 8, &codes(8)), Err(BlockCodecError::Grid { .. })));
}

// spec: STRM-032
// spec: GEN-031
#[test]
fn a_build_writes_the_floor_blocks_once_they_equal_the_in_memory_floors_and_do_not_depend_on_the_thread_count() {
    let w = world(7);
    let res = 32;
    let (one, three) = (Dir::new("one"), Dir::new("three"));
    let r1 = build_global(one.path(), &w, res, 1, DEFAULT_MAX_PACK_BYTES).unwrap();
    let r3 = build_global(three.path(), &w, res, 3, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((r1.floor_written, r3.floor_written), (6 * 64, 6 * 64));
    let grid = bake(w.seed(), &TangentWarp, res, 2).unwrap();
    let (_, ocean) = tiled_ocean(&grid, &TangentWarp, 4, 2, EARTH_OCEAN_MIN_AREA_FRACTION).unwrap();
    let want = tiled_floors(&grid, &ocean, 4, 2).unwrap();
    let mut finite = 0;
    for dir in [&one, &three] {
        let store = TileStore::open(dir.path(), w.params()).unwrap();
        let src = StoreWetFloorSource::new(&store, &w, res).unwrap();
        assert_eq!(src.res(), res);
        for f in 0..6u8 {
            for j in 0..=res {
                for i in 0..=res {
                    let got = src.floor(Face(f), i, j).unwrap();
                    assert_eq!(got, want.at(Face(f), i, j), "face {f} vertex ({i}, {j})");
                    finite += usize::from(got != INF);
                }
            }
        }
        assert!(src.floor(Face(0), res + 1, 0).is_err());
        assert!(src.decodes() >= 6 * 64, "every block was decoded");
    }
    assert!(finite > 0, "the bake has reachable floors");
    // A second build writes nothing.
    let again = build_global(one.path(), &w, res, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((again.written, again.ocean_written, again.floor_written, again.summary_written), (0, 0, 0, false));
    // Another threshold has another channel, added without baking again.
    let other = world(7).with_ocean_area_fraction(0.0);
    let r = build_global(one.path(), &other, res, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((r.written, r.ocean_written, r.floor_written), (0, 6 * 64, 6 * 64));
}

// spec: STRM-032
#[test]
fn a_store_without_floor_blocks_reads_with_an_error_that_names_the_block_and_the_command_and_verifies_clean_once_built() {
    let w = world(7);
    let res = 16;
    let dir = Dir::new("verify");
    build_global(dir.path(), &w, res, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    let store = TileStore::open(dir.path(), w.params()).unwrap();
    // A world of another threshold has no floor blocks in this store.
    let other = world(7).with_ocean_area_fraction(0.0);
    let src = StoreWetFloorSource::new(&store, &other, res).unwrap();
    let err = src.floor(Face(0), 1, 1).unwrap_err().to_string();
    assert!(err.contains("wet-floor block") && err.contains("generate --global-dir") && err.contains("--global-res 16"), "{err}");
    let report = verify_global(&store, &w, 0);
    assert!(report.mismatches.is_empty() && report.unreadable.is_empty(), "{report:?}");
    assert!(report.matching > 0);
}
