//! Tier A, STRM-080 (TEST-108): the basin table as a stored kind (19). A round trip through a store in id order whatever the input order, a table of another spec reading as absent (never wrong), a record of
//! another version or grid refused naming both, a damaged record an error, the exact size, a lake across a tile border. Small tables; no planet bake.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::basin_table::*;
use planet_cache::global_store::GlobalWorld;
use planet_cache::pack::{PackWriter, Record, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::Face;
use planet_generators::bake::hydro::{BasinClass, HydroParams};
use planet_generators::bake::lakes::{Lake, TileRef};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const RES: u32 = 64;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-basin-table-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

fn spec() -> BasinSpec {
    BasinSpec { hydro: HydroParams::default(), tile_cells: 256, tiles_per_edge: 48, base_id: 7 }
}

fn store_of(dir: &Dir, w: &GlobalWorld, records: &[Record]) -> TileStore {
    let mut writer = PackWriter::open(dir.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    for r in records {
        writer.append(r).unwrap();
    }
    writer.finish().unwrap();
    TileStore::open(dir.path(), w.params()).unwrap()
}

fn lake(id: u64, class: BasinClass, tiles: &[(u8, u32, u32)]) -> Lake {
    Lake {
        id,
        floor_id: id * 3 + 1,
        spill_id: id * 3 + 2,
        floor_m: 280.0 + id as f64,
        spill_m: 590.5,
        level_m: 301.25 + id as f64 * 0.5,
        class,
        overflows: class == BasinClass::Lake,
        wet: id % 2 == 1,
        area_m2: 2.304e7,
        cells: 36 + id as u32,
        q_in: 1.5e-3 * id as f64,
        q_out: if class == BasinClass::Lake { 1.25e-3 } else { 0.0 },
        tiles: tiles.iter().map(|&(face, ti, tj)| TileRef { face, ti, tj }).collect(),
    }
}

fn three() -> Vec<Lake> {
    vec![
        lake(900, BasinClass::Endorheic, &[(2, 5, 6)]),
        lake(7, BasinClass::Lake, &[(2, 5, 6), (2, 5, 7), (2, 6, 6), (2, 6, 7)]),
        lake(31, BasinClass::Intentional, &[]),
    ]
}

fn sorted(mut v: Vec<Lake>) -> Vec<Lake> {
    v.sort_by_key(|l| l.id);
    v
}

// spec: STRM-080, TEST-108
#[test]
fn a_table_round_trips_through_a_store_in_id_order_whatever_the_input_order() {
    let (w, s) = (world(), spec());
    for lakes in [vec![], vec![lake(5, BasinClass::Lake, &[(0, 1, 1)])], three()] {
        let mut rev = lakes.clone();
        rev.reverse();
        let d = Dir::new("round");
        let store = store_of(&d, &w, &[basin_table_record(&w, RES, &s, Face(2), &rev)]);
        let got = load_basin_table(&store, &w, RES, &s, Face(2)).unwrap().expect("the table is stored");
        assert_eq!(got, sorted(lakes.clone()), "{} lakes", lakes.len());
        assert_eq!(
            basin_table_record(&w, RES, &s, Face(2), &lakes).payload,
            basin_table_record(&w, RES, &s, Face(2), &rev).payload,
            "the bytes do not depend on the input order"
        );
    }
    // The records of all the faces: each lake on the face its caller names; an empty face is stored as an empty table.
    let d = Dir::new("faces");
    let records = basin_table_records(&w, RES, &s, &three(), &|l| Face(if l.id == 900 { 4 } else { 2 }));
    assert_eq!(records.len(), 6);
    let store = store_of(&d, &w, &records);
    let on = |f: u8| load_basin_table(&store, &w, RES, &s, Face(f)).unwrap().expect("every face has a table");
    assert_eq!(on(2).iter().map(|l| l.id).collect::<Vec<_>>(), vec![7, 31]);
    assert_eq!(on(4).iter().map(|l| l.id).collect::<Vec<_>>(), vec![900]);
    assert!(on(0).is_empty() && on(5).is_empty());
}

// spec: STRM-080, TEST-108
#[test]
fn a_table_of_another_spec_reads_as_absent_and_the_key_follows_every_input() {
    let (w, s) = (world(), spec());
    let d = Dir::new("absent");
    let store = store_of(&d, &w, &[basin_table_record(&w, RES, &s, Face(2), &three())]);
    let key = |s: &BasinSpec| basin_key(&w, RES, s, Face(2));
    let changed = [
        BasinSpec { hydro: HydroParams { pit_min_m: 0.75, ..s.hydro }, ..s },
        BasinSpec { hydro: HydroParams { initiation_m2: 5.0e7, ..s.hydro }, ..s },
        BasinSpec { hydro: HydroParams { min_cells: 9, ..s.hydro }, ..s },
        BasinSpec { hydro: HydroParams { body: 1, ..s.hydro }, ..s },
        BasinSpec { tile_cells: 128, ..s },
        BasinSpec { tiles_per_edge: 96, ..s },
        BasinSpec { base_id: 8, ..s },
    ];
    for c in &changed {
        assert_ne!(key(c), key(&s), "{c:?} is another key");
        assert!(load_basin_table(&store, &w, RES, c, Face(2)).unwrap().is_none(), "{c:?} reads as absent, not as wrong");
    }
    assert_eq!(key(&s), key(&BasinSpec { ..s }), "the same spec, the same key");
    assert_ne!(basin_key(&w, RES, &s, Face(2)), basin_key(&w, RES, &s, Face(3)), "a key per face");
    assert!(load_basin_table(&store, &w, RES, &s, Face(3)).unwrap().is_none(), "a face without a record reads as absent");
}

fn damaged(f: impl FnOnce(&mut Vec<u8>)) -> Result<Option<Vec<Lake>>, BasinTableError> {
    let (w, s) = (world(), spec());
    let mut r = basin_table_record(&w, RES, &s, Face(2), &three());
    f(&mut r.payload);
    let d = Dir::new("damaged");
    let store = store_of(&d, &w, &[r]);
    load_basin_table(&store, &w, RES, &s, Face(2))
}

// spec: STRM-080, TEST-108
#[test]
fn a_record_of_another_version_or_grid_or_a_damaged_one_is_refused_naming_what_was_found() {
    let err = |r: Result<Option<Vec<Lake>>, BasinTableError>| r.expect_err("refused").0;
    // The table version (bytes 1..5) and the producer version (5..9): both versions are named.
    let m = err(damaged(|p| p[1..5].copy_from_slice(&2u32.to_le_bytes())));
    assert!(m.contains("table version 2") && m.contains("the code has 1 and 1") && m.contains("key "), "{m}");
    let m = err(damaged(|p| p[5..9].copy_from_slice(&9u32.to_le_bytes())));
    assert!(m.contains("producer version 9") && m.contains("the code has 1 and 1"), "{m}");
    // The codec byte, the grid, the face.
    assert!(err(damaged(|p| p[0] = 9)).contains("codec version 1"));
    let m = err(damaged(|p| p[13..17].copy_from_slice(&64u32.to_le_bytes())));
    assert!(m.contains("64 tiles of 256 cells") && m.contains("48 tiles of 256 cells"), "{m}");
    assert!(err(damaged(|p| p[17] = 3)).contains("for face 3"));
    // A count the payload cannot hold is refused before anything is reserved.
    let m = err(damaged(|p| p[18..22].copy_from_slice(&u32::MAX.to_le_bytes())));
    assert!(m.contains("cannot fit"), "{m}");
    // The key inside the payload, a cut-off table, trailing bytes, a bad class, unknown flag bits, a tile outside the grid.
    assert!(err(damaged(|p| p[22] ^= 1)).contains("key inside"));
    assert!(err(damaged(|p| p.truncate(p.len() - 4))).contains("cut off"));
    assert!(err(damaged(|p| p.push(0))).contains("bytes follow"));
    let first = 38; // the first lake in id order is id 7
    assert!(err(damaged(|p| p[first + 76] = 3)).contains("class code 3"));
    assert!(err(damaged(|p| p[first + 76] |= 0x10)).contains("flag bits"));
    assert!(err(damaged(|p| p[first + 79 + 1..first + 79 + 5].copy_from_slice(&48u32.to_le_bytes()))).contains("outside the grid"));
    // Lakes out of id order: the id of the first lake raised above the second's.
    assert!(err(damaged(|p| p[first..first + 8].copy_from_slice(&u64::MAX.to_le_bytes()))).contains("out of id order"));
}

// spec: STRM-080, TEST-108
#[test]
fn the_size_is_exact_and_a_lake_across_a_tile_border_lists_both_tiles() {
    let (w, s) = (world(), spec());
    let lakes = three();
    let tiles: usize = lakes.iter().map(|l| l.tiles.len()).sum();
    let r = basin_table_record(&w, RES, &s, Face(2), &lakes);
    assert_eq!(r.payload.len(), 38 + lakes.len() * 79 + tiles * 9, "header + 79 bytes a lake + 9 bytes a tile");
    assert_eq!(basin_table_record(&w, RES, &s, Face(2), &[]).payload.len(), 38);
    let d = Dir::new("border");
    let store = store_of(&d, &w, &[r]);
    let got = load_basin_table(&store, &w, RES, &s, Face(2)).unwrap().unwrap();
    let across = got.iter().find(|l| l.id == 7).unwrap();
    assert_eq!(across.tiles.len(), 4, "the lake of 2 x 2 tiles lists all four");
    assert!(across.tiles.windows(2).all(|p| p[0] < p[1]), "in tile order");
}
