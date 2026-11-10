//! Tier A, STRM-081 (TEST-108): the local change L of the new chain as kind 13 records, layer version 3, codec 3. A round trip within half a quantisation step with K7's stats, the key following the base
//! and every input, a record of the previous layer version refused naming both versions, a tile under another key refused (never mixed), a damaged record an error, the clamped cells counted.
//! Small tiles; no planet bake.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::global_store::GlobalWorld;
use planet_cache::key::Kind;
use planet_cache::layer_l::*;
use planet_cache::layer_tiles::{KIND_LAYER_TILE, TILE_LAYER_VERSION};
use planet_cache::pack::{PackWriter, Record, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::{Face, TileId};
use planet_generators::bake::k7::K7Params;
use planet_generators::bake::tile::{TileGrid, TileId as PassTile, QUANT_STEP_M};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const RES: u32 = 64;
const T: usize = 24;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-layer-l-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

fn world() -> GlobalWorld {
    GlobalWorld::from_definition(&WorldDefinition::single_planet(R, 3)).unwrap().with_block_limit(4)
}

fn spec() -> LTileSpec {
    LTileSpec { grid: TileGrid { cells: 96, tile: T, halo: 6, radius_m: R }, k7: K7Params::standard(5, 0.0), base_id: 7, inputs_id: 11 }
}

fn id() -> PassTile {
    PassTile { face: Face(2), ti: 1, tj: 3 }
}

fn stats() -> LTileStats {
    LTileStats { land_cells: 400, burned_cells: 37, pit_cells: 5, pit_p50_m: 0.75, pit_p99_m: 2.5, pit_max_m: 4.125 }
}

fn core() -> Vec<f32> {
    (0..T * T).map(|k| 12.0 * libm::sinf(k as f32 * 0.07) - 3.0 + (k % 7) as f32 * 0.31).collect()
}

fn store_of(dir: &Dir, w: &GlobalWorld, records: &[Record]) -> TileStore {
    let mut writer = PackWriter::open(dir.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    for r in records {
        writer.append(r).unwrap();
    }
    writer.finish().unwrap();
    TileStore::open(dir.path(), w.params()).unwrap()
}

// spec: STRM-081, TEST-108
#[test]
fn a_tile_of_l_round_trips_within_half_a_step_with_k7s_stats_and_counts_the_clamped_cells() {
    let (w, s) = (world(), spec());
    let mut c = core();
    c[3] = 9000.0; // beyond the quantisation range (+-8,191.75 m)
    c[4] = -9000.0;
    let d = Dir::new("round");
    let store = store_of(&d, &w, &[l_tile_record(&w, RES, &s, id(), &c, &stats(), &[]).unwrap()]);
    let got = load_l_tile(&store, &w, RES, &s, id()).unwrap().expect("the tile is stored");
    assert_eq!((got.id, got.stats, got.clamped_cells), (id(), stats(), 2));
    assert_eq!(got.core.len(), T * T);
    for (k, (a, b)) in c.iter().zip(&got.core).enumerate() {
        if k == 3 || k == 4 {
            let end = if *a > 0.0 { 8191.75 } else { -8192.0 };
            assert!((b - end).abs() < 1e-3, "a clamped cell is stored at the range's end: {b}");
        } else {
            assert!((a - b).abs() <= QUANT_STEP_M / 2.0 + 1e-4, "cell {k}: {a} against {b}");
        }
    }
    let other = PassTile { face: Face(2), ti: 0, tj: 0 };
    assert!(load_l_tile(&store, &w, RES, &s, other).unwrap().is_none(), "a tile nothing was written for reads as absent");
}

// spec: STRM-081, TEST-108
#[test]
fn the_key_follows_the_base_every_input_and_the_layer_version_and_a_tile_under_another_key_is_refused() {
    let (w, s) = (world(), spec());
    let key = |s: &LTileSpec| l_tile_key(&w, RES, s, id()).unwrap();
    let mut other_k7 = s.k7;
    other_k7.canonical_flood = false;
    let mut other_pit = s.k7;
    other_pit.pass2.pit_min_m += 0.25;
    let mut other_a = s.k7;
    other_a.width.a = 7.3;
    let mut other_b = s.k7;
    other_b.width.b = 0.55;
    for c in [
        LTileSpec { base_id: 8, ..s },
        LTileSpec { inputs_id: 12, ..s },
        LTileSpec { k7: other_k7, ..s },
        LTileSpec { k7: other_pit, ..s },
        LTileSpec { k7: other_a, ..s },
        LTileSpec { k7: other_b, ..s },
        LTileSpec { grid: TileGrid { halo: 5, ..s.grid }, ..s },
        LTileSpec { grid: TileGrid { cells: 192, tile: 48, ..s.grid }, ..s },
    ] {
        assert_ne!(key(&c), key(&s), "{c:?} is another key");
    }
    assert_eq!(key(&s), key(&LTileSpec { ..s }), "the same spec, the same key");
    assert_ne!(key(&s), l_tile_key(&w, RES, &s, PassTile { face: Face(2), ti: 1, tj: 2 }).unwrap(), "a key per tile");
    assert_eq!(TILE_LAYER_VERSION, 3, "L is layer version 3");
    // A store that holds the tile under another base is refused, not mixed, naming the found key.
    let d = Dir::new("keys");
    let store = store_of(&d, &w, &[l_tile_record(&w, RES, &LTileSpec { base_id: 8, ..s }, id(), &core(), &stats(), &[]).unwrap()]);
    let e = load_l_tile(&store, &w, RES, &s, id()).expect_err("another base is refused").0;
    assert!(
        e.contains("made for another base") && e.contains("codec 3") && e.contains("layer version 3") && e.contains("not mixed"),
        "{e}"
    );
}

// spec: STRM-081, TEST-108
#[test]
fn a_record_of_the_previous_layer_version_is_refused_naming_both_versions_and_a_damaged_one_is_an_error() {
    let (w, s) = (world(), spec());
    // A record of the previous layer in the tile's slot: kind 13, codec 2, under some key of its own.
    let tile = planet_cache::layer_tiles::layer_tile_id(&s.grid, id()).unwrap();
    let old = Record {
        key: planet_cache::key::CacheKey([9; 16]),
        kind: KIND_LAYER_TILE,
        tile,
        generator_version: planet_generators::global::GLOBAL_VERSION,
        hash_version: planet_core::hash::HASH_VERSION,
        node_set_id: w.node_set_id(),
        payload: vec![2, 0, 0, 0, 0],
    };
    let d = Dir::new("old");
    let store = store_of(&d, &w, &[old]);
    let e = load_l_tile(&store, &w, RES, &s, id()).expect_err("a record of layer version 2 is refused").0;
    assert!(
        e.contains("codec 2")
            && e.contains("layer version 2")
            && e.contains("codec 3")
            && e.contains("layer version 3")
            && e.contains("rebuild the layer"),
        "{e}"
    );
    assert_eq!(Kind::LayerTile as u8, KIND_LAYER_TILE);

    let good = l_tile_record(&w, RES, &s, id(), &core(), &stats(), &[]).unwrap();
    let damaged = |f: &dyn Fn(&mut Vec<u8>)| -> String {
        let mut r = good.clone();
        f(&mut r.payload);
        let d = Dir::new("damaged");
        let store = store_of(&d, &w, &[r]);
        load_l_tile(&store, &w, RES, &s, id()).expect_err("refused").0
    };
    assert!(damaged(&|p| p[0] = 9).contains("codec 9") && damaged(&|p| p[0] = 9).contains("this build reads codec 3"));
    assert!(damaged(&|p| p[1..5].copy_from_slice(&48u32.to_le_bytes())).contains("48 cells a side"));
    assert!(damaged(&|p| p[61] ^= 1).contains("another key than its record"));
    assert!(damaged(&|p| p[37..45].copy_from_slice(&f64::NAN.to_bits().to_le_bytes())).contains("not finite"));
    assert!(damaged(&|p| p[21..29].copy_from_slice(&u64::MAX.to_le_bytes())).contains("exceeds the tile"));
    assert!(damaged(&|p| p.truncate(60)).contains("codec 3"), "a payload shorter than the header");
    let m = damaged(&|p| p.truncate(p.len() - 6));
    assert!(m.contains("do not decompress") || m.contains("bytes of counts"), "{m}");
    let _ = TileId::new(Face(0), 0, 0, 0);
}

// spec: STRM-081, TEST-108
#[test]
fn the_water_section_is_reserved_empty_carried_opaque_and_bounded() {
    let (w, s) = (world(), spec());
    // Empty by default: a tile that touches no lake.
    let plain = l_tile_record(&w, RES, &s, id(), &core(), &stats(), &[]).unwrap();
    assert_eq!(&plain.payload[77..81], &0u32.to_le_bytes(), "the length of an empty water section is 0");
    // A filled section is carried as it is; the counts after it are unaffected and so is the key.
    let mask: Vec<u8> = (0..300u32).map(|k| (k * 7 % 251) as u8).collect();
    let with = l_tile_record(&w, RES, &s, id(), &core(), &stats(), &mask).unwrap();
    assert_eq!(with.key, plain.key, "the water section is not part of the key");
    assert_eq!(with.payload.len(), plain.payload.len() + mask.len());
    let d = Dir::new("water");
    let store = store_of(&d, &w, &[with]);
    let got = load_l_tile(&store, &w, RES, &s, id()).unwrap().unwrap();
    assert_eq!(got.water, mask);
    let d = Dir::new("water-empty");
    let store = store_of(&d, &w, std::slice::from_ref(&plain));
    let none = load_l_tile(&store, &w, RES, &s, id()).unwrap().unwrap();
    assert!(none.water.is_empty());
    assert_eq!(none.core, got.core, "the counts read the same with and without a section");
    // A length that the record cannot hold, and one over the cap, are refused before anything is reserved.
    for len in [u32::MAX, 1 << 30, plain.payload.len() as u32] {
        let mut r = plain.clone();
        r.payload[77..81].copy_from_slice(&len.to_le_bytes());
        let d = Dir::new("water-bad");
        let store = store_of(&d, &w, &[r]);
        let e = load_l_tile(&store, &w, RES, &s, id()).expect_err("refused").0;
        assert!(e.contains("water section claims"), "{e}");
    }
}
