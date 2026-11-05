//! Tier A, GEN-120 and STRM-076 (m9 keep-pits): the pit depth of the erosion layer in a store. A face's pass-1 depth and a tile's depth come back as the computed codes, 1 and 8 threads store the
//! same bytes, a depth of another layer is refused, and the runtime layer reads the change with the depth added back. A tiny planet: tiles of 24 cells, faces of 24 cells.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_fill::*;
use planet_cache::layer_store::{load_face_change, run_pass1, LayerSpec};
use planet_cache::layer_tiles::*;
use planet_cache::pack::TileStore;
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::{pass1_face_with_channels, FaceChange, FaceFill, FaceSpec};
use planet_generators::bake::tile::{pass2_tile_with_fill, Pass2Params, TileGrid, TileId as PassTile};
use planet_generators::erosion_layer::LayerSource;
use planet_world_def::WorldDefinition;

const R: f64 = 100.0 * 96.0 / core::f64::consts::FRAC_PI_2;
const RES: u32 = 64;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-layer-fill-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

const SPOTS: [[f64; 3]; 6] = [[1.0, 0.3, 0.2], [-1.0, 0.2, -0.3], [0.3, 1.0, 0.2], [0.2, -1.0, 0.3], [0.3, 0.2, 1.0], [-0.2, 0.3, -1.0]];

/// Mountains of several hundred metres on land, so the carve leaves basins; a sea of -300 m.
fn base(d: Vec3) -> f64 {
    let ridge = 300.0 * libm::sin(9.0 * d.x + 1.0) * libm::cos(8.0 * d.y) + 200.0 * libm::sin(17.0 * d.z + 0.3) + 60.0;
    // A plateau of 500 m with a bowl of 450 m in it at a spot of every face: closed basins at pass 1's and pass 2's cell sizes.
    let (mut plateau, mut bowl) = (0.0, 0.0);
    for c in SPOTS {
        let n = libm::sqrt(c[0] * c[0] + c[1] * c[1] + c[2] * c[2]);
        let cos = (d.x * c[0] + d.y * c[1] + d.z * c[2]) / n;
        plateau += 500.0 * libm::exp(-(1.0 - cos) / 0.02);
        bowl += 450.0 * libm::exp(-(1.0 - cos) / 0.0015);
    }
    if ridge > 0.0 || plateau > 20.0 {
        ridge.max(0.0) + 20.0 + plateau - bowl
    } else {
        -300.0 + 0.3 * ridge
    }
}

fn pass1() -> LayerSpec {
    LayerSpec {
        cells: 24,
        radius_m: R,
        sea_level: 0.0,
        seed: 17,
        carve: CarveParams { breach_max_m: 5.0, ..CarveParams::default() },
        inputs_id: 1,
        uses_factor: false,
        halo: 4,
    }
}

fn spec(w: &GlobalWorld) -> TileLayerSpec {
    TileLayerSpec {
        grid: TileGrid { cells: 96, tile: 24, halo: 6, radius_m: R },
        params: Pass2Params::standard(9, 0.0),
        inputs_id: 1,
        uses_factor: false,
        faces: faces_digest(w, RES, &pass1()),
    }
}

fn tiles() -> Vec<PassTile> {
    // Every tile of three faces: the pits are where the spots of `base` fall.
    TileGrid { cells: 96, tile: 24, halo: 6, radius_m: R }.all_tiles().into_iter().filter(|t| t.face.0 % 2 == 0).collect()
}

fn face_spec(f: u8) -> FaceSpec {
    FaceSpec { face: Face(f), cells: 24, radius_m: R, sea_level: 0.0, seed: 17, halo: 4 }
}

fn six_fills(dir: &Path, w: &GlobalWorld) -> [FaceFill; 6] {
    let store = TileStore::open(dir, w.params()).unwrap();
    let v: Vec<_> = (0..6).map(|f| load_face_fill(&store, w, RES, &pass1(), Face(f)).unwrap().expect("a face fill")).collect();
    v.try_into().expect("six faces")
}

fn six_changes(dir: &Path, w: &GlobalWorld) -> [FaceChange; 6] {
    let store = TileStore::open(dir, w.params()).unwrap();
    let v: Vec<_> = (0..6).map(|f| load_face_change(&store, w, RES, &pass1(), Face(f)).unwrap().expect("a face")).collect();
    v.try_into().expect("six faces")
}

fn run_p2(dir: &Path, w: &GlobalWorld, s: &TileLayerSpec, c: &[FaceChange; 6], fills: &[FaceFill; 6], threads: usize) -> Pass2Report {
    let (change, fill) = (face_change_reader(c), face_fill_reader(fills));
    run_pass2(
        dir,
        w,
        RES,
        s,
        &Pass2Inputs { base: &base, face_change: &change, factor: None, face_fill: Some(&fill) },
        &tiles(),
        threads,
        &mut |_| {},
    )
    .unwrap()
}

fn pack_bytes(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tpack"))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), fs::read(e.path()).unwrap()))
        .collect();
    v.sort();
    v
}

// spec: STRM-076
#[test]
fn a_face_and_a_tile_store_their_pit_depth_as_computed_on_1_and_8_threads_and_another_layer_is_refused() {
    let (w, p1) = (world(), Dir::new("p1"));
    run_pass1(p1.path(), &w, RES, &pass1(), &base, None, &(0..6).map(Face).collect::<Vec<_>>(), 2).unwrap();
    let fills = six_fills(p1.path(), &w);
    let changes = six_changes(p1.path(), &w);
    let mut any_pit = false;
    for f in 0..6u8 {
        let (_, _, codes) = pass1_face_with_channels(&face_spec(f), &base, None, &pass1().carve, 1);
        assert_eq!(fills[f as usize].codes, codes, "face {f}");
        any_pit |= codes.iter().any(|c| *c > 0);
    }
    assert!(any_pit, "the synthetic planet has no pit to test with");
    let s = spec(&w);
    let (a, b) = (Dir::new("one"), Dir::new("eight"));
    assert_eq!(run_p2(a.path(), &w, &s, &changes, &fills, 1).computed.len(), tiles().len());
    run_p2(b.path(), &w, &s, &changes, &fills, 8);
    assert_eq!(pack_bytes(a.path()), pack_bytes(b.path()), "1 and 8 threads store other bytes");
    let store = TileStore::open(a.path(), w.params()).unwrap();
    let (change, fill) = (face_change_reader(&changes), face_fill_reader(&fills));
    for id in tiles() {
        let computed = pass2_tile_with_fill(&s.grid, id, &base, &change, Some(&fill), None, &s.params, 2);
        assert_eq!(load_fill(&store, &w, RES, &s, id).unwrap().expect("fill"), computed.fill, "tile {}/{}/{}", id.face.0, id.ti, id.tj);
    }
    assert_eq!(load_fill(&store, &w, RES, &s, PassTile { face: Face(1), ti: 0, tj: 0 }).unwrap(), None);
    let other = TileLayerSpec { inputs_id: 2, ..s };
    let e = load_fill(&store, &w, RES, &other, tiles()[0]).unwrap_err().to_string();
    assert!(e.contains("another layer") && e.contains("key"), "{e}");
    // A second run finds everything stored.
    let again = run_p2(a.path(), &w, &s, &changes, &fills, 2);
    assert!((again.computed.len(), again.skipped.len()) == (0, tiles().len()), "{again:?}");
}

// spec: GEN-120
#[test]
fn the_runtime_layer_reads_the_change_with_the_pit_depth_added_back() {
    let (w, p1) = (world(), Dir::new("p1"));
    run_pass1(p1.path(), &w, RES, &pass1(), &base, None, &(0..6).map(Face).collect::<Vec<_>>(), 2).unwrap();
    let (fills, changes) = (six_fills(p1.path(), &w), six_changes(p1.path(), &w));
    let s = spec(&w);
    let dir = Dir::new("layer");
    run_p2(dir.path(), &w, &s, &changes, &fills, 2);
    let store = Arc::new(TileStore::open(dir.path(), w.params()).unwrap());
    let layer = StoreLayer::new(store.clone(), Arc::new(world()), RES, s, 4, default_change_range()).unwrap();
    let mut with_pits = 0;
    for id in tiles() {
        let stored = load_tile(&store, &w, RES, &s, id).unwrap().expect("tile");
        let codes = load_fill(&store, &w, RES, &s, id).unwrap().expect("fill");
        let tile = layer.tile(id.face, id.ti, id.tj).unwrap().expect("a tile");
        for (p, (c, f)) in stored.core.iter().zip(&codes).enumerate() {
            let want = c + f32::from(*f);
            assert_eq!(tile.cell(p % 24, p / 24), want, "tile {}/{}/{} cell {p}", id.face.0, id.ti, id.tj);
        }
        with_pits += codes.iter().filter(|c| **c > 0).count();
    }
    assert!(with_pits > 0, "no tile holds a pit, the test reads nothing");
}

// spec: STRM-076
#[test]
fn the_step_of_the_pit_depth_code_is_part_of_the_key_and_the_records_have_keys_of_their_own() {
    assert_eq!(steps_per_m(), 1.0);
    let w = world();
    let s = spec(&w);
    let k = fill_key(&w, RES, &s, tiles()[0]).unwrap();
    assert_ne!(
        k,
        planet_cache::layer_channels::depth_key(&w, RES, &s, tiles()[0]).unwrap(),
        "the pit depth shares the deposited depth's key"
    );
    assert_ne!(face_fill_key(&w, RES, &pass1(), Face(0)), planet_cache::layer_channels::area_key(&w, RES, &pass1(), Face(0)));
}
