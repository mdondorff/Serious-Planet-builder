//! Tier A, STRM-065 and GEN-101 (m9-bake PR 5): the erosion layer's pass 2 in a store. A killed run resumed equals an uninterrupted one, 1 and 8 threads store the same bytes, a layer made
//! for other parameters or another pass 1 is refused naming both keys, a damaged record is an error, the stored change is within half a quantisation step of the computed one, and the
//! pass-1 faces are dropped only from a store, never from another folder. A tiny planet: tiles of 24 cells, faces of 24 cells; no planet bake.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_store::{load_face_change, run_pass1, LayerSpec};
use planet_cache::layer_tiles::*;
use planet_cache::pack::{PackWriter, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::FaceChange;
use planet_generators::bake::tile::{pass2_tile, Pass2Params, TileGrid, TileId as PassTile, QUANT_STEP_M};
use planet_world_def::WorldDefinition;

const R: f64 = 100.0 * 96.0 / core::f64::consts::FRAC_PI_2;
const RES: u32 = 64;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-layer-tiles-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

fn base(d: Vec3) -> f64 {
    let ridge = 300.0 * libm::sin(9.0 * d.x + 1.0) * libm::cos(8.0 * d.y) + 200.0 * libm::sin(17.0 * d.z + 0.3) + 60.0;
    if ridge > 0.0 {
        ridge + 20.0
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
        carve: CarveParams::default(),
        inputs_id: 1,
        uses_factor: false,
        rock: 0,
        halo: 4,
    }
}

fn spec(w: &GlobalWorld) -> TileLayerSpec {
    TileLayerSpec {
        grid: TileGrid { cells: 96, tile: 24, halo: 6, radius_m: R },
        params: Pass2Params::standard(9, 0.0),
        inputs_id: 1,
        uses_factor: false,
        rock: 0,
        faces: faces_digest(w, RES, &pass1()),
    }
}

fn tiles() -> Vec<PassTile> {
    vec![PassTile { face: Face(0), ti: 0, tj: 0 }, PassTile { face: Face(0), ti: 1, tj: 0 }, PassTile { face: Face(2), ti: 2, tj: 1 }]
}

/// The six faces of pass 1 in `dir`, read back.
fn faces(dir: &Path, w: &GlobalWorld) -> [FaceChange; 6] {
    run_pass1(dir, w, RES, &pass1(), &base, None, &(0..6).map(Face).collect::<Vec<_>>(), 2).unwrap();
    let store = TileStore::open(dir, w.params()).unwrap();
    let v: Vec<FaceChange> = (0..6).map(|f| load_face_change(&store, w, RES, &pass1(), Face(f)).unwrap().expect("a face")).collect();
    v.try_into().expect("six faces")
}

fn run(
    dir: &Path,
    w: &GlobalWorld,
    s: &TileLayerSpec,
    f: &[FaceChange; 6],
    which: &[PassTile],
    threads: usize,
) -> Result<Pass2Report, planet_cache::layer_store::LayerStoreError> {
    let reader = face_change_reader(f);
    run_pass2(
        dir,
        w,
        RES,
        s,
        &Pass2Inputs {
            base: &base,
            face_change: &reader,
            factor: None,
            scratch: None,
            rock: planet_generators::bake::face::RockFields::none(),
        },
        which,
        threads,
        &mut |_| {},
    )
}

fn stored(dir: &Path, w: &GlobalWorld, s: &TileLayerSpec, which: &[PassTile]) -> Vec<Vec<u32>> {
    let store = TileStore::open(dir, w.params()).unwrap();
    which
        .iter()
        .filter_map(|&t| load_tile(&store, w, RES, s, t).expect("a readable tile"))
        .map(|r| r.core.iter().map(|v| v.to_bits()).collect())
        .collect()
}

// spec: STRM-065
#[test]
fn a_killed_run_resumed_stores_the_same_tiles_and_the_same_bytes_on_1_and_8_threads() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let (whole, cut, eight) = (Dir::new("whole"), Dir::new("cut"), Dir::new("eight"));
    let r = run(whole.path(), &w, &s, &f, &tiles(), 1).unwrap();
    assert_eq!((r.computed.len(), r.skipped.len()), (3, 0));
    run(cut.path(), &w, &s, &f, &tiles()[..1], 2).unwrap();
    let r = run(cut.path(), &w, &s, &f, &tiles(), 2).unwrap();
    assert_eq!((r.computed, r.skipped), (tiles()[1..].to_vec(), tiles()[..1].to_vec()));
    run(eight.path(), &w, &s, &f, &tiles(), 8).unwrap();
    let (a, b, c) = (stored(whole.path(), &w, &s, &tiles()), stored(cut.path(), &w, &s, &tiles()), stored(eight.path(), &w, &s, &tiles()));
    assert_eq!(a.len(), 3);
    assert_eq!(a, b, "a resumed layer differs from an uninterrupted one");
    assert_eq!(a, c, "8 threads store other bytes than 1");
    assert_eq!(run(cut.path(), &w, &s, &f, &tiles(), 2).unwrap().computed.len(), 0, "a complete store computed something");
    // A tile listed twice is computed once.
    let twice = Dir::new("twice");
    assert_eq!(run(twice.path(), &w, &s, &f, &[tiles()[0], tiles()[0]], 2).unwrap().computed.len(), 1);
}

// spec: STRM-065
#[test]
fn the_stored_change_is_within_half_a_step_of_the_computed_one_and_carries_its_statistics() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let d = Dir::new("round");
    run(d.path(), &w, &s, &f, &tiles()[..1], 2).unwrap();
    let reader = face_change_reader(&f);
    let direct = pass2_tile(&s.grid, tiles()[0], &base, &reader, None, &s.params, 2);
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let back = load_tile(&store, &w, RES, &s, tiles()[0]).unwrap().expect("the tile");
    assert_eq!(back.stats, direct.stats);
    assert!(direct.core.iter().zip(&back.core).all(|(a, b)| (a - b).abs() <= QUANT_STEP_M / 2.0 + 1e-6));
    assert!(back.core.iter().any(|v| *v != 0.0), "the tile changed nothing");
}

// spec: STRM-065
#[test]
fn a_layer_made_for_other_parameters_or_another_pass_1_is_refused_naming_both_keys() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let d = Dir::new("refused");
    run(d.path(), &w, &s, &f, &tiles()[..1], 2).unwrap();
    let other_pass1 = TileLayerSpec { faces: faces_digest(&w, RES, &LayerSpec { seed: 18, ..pass1() }), ..s };
    let other_seed = TileLayerSpec { params: Pass2Params::standard(10, 0.0), ..s };
    let other_grid = TileLayerSpec { grid: TileGrid { halo: 5, ..s.grid }, ..s };
    for (what, o) in [("another pass 1", other_pass1), ("another seed", other_seed), ("another grid", other_grid)] {
        let e = run(d.path(), &w, &o, &f, &tiles()[..1], 2).expect_err(what).to_string();
        assert!(e.contains("another bake, other parameters or another pass 1") && e.contains("key"), "{what}: {e}");
    }
    assert_eq!(stored(d.path(), &w, &s, &tiles()[..1]).len(), 1, "the refusal changed the store");
}

// spec: STRM-065
#[test]
fn a_damaged_tile_record_is_an_error_naming_the_tile() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let reader = face_change_reader(&f);
    let r = pass2_tile(&s.grid, tiles()[1], &base, &reader, None, &s.params, 2);
    let good = tile_record(&w, RES, &s, &r).unwrap();
    assert_eq!(good.kind, 13);
    assert_eq!(&good.payload[57..73], &good.key.0[..]);
    for (name, damage) in [
        ("a short payload", Box::new(|p: &mut Vec<u8>| p.truncate(40)) as Box<dyn Fn(&mut Vec<u8>)>),
        ("another codec", Box::new(|p: &mut Vec<u8>| p[0] = 9)),
        ("the lz4 codec of an earlier layout", Box::new(|p: &mut Vec<u8>| p[0] = 1)),
        ("another tile size", Box::new(|p: &mut Vec<u8>| p[1] = 25)),
        ("another key inside", Box::new(|p: &mut Vec<u8>| p[58] ^= 1)),
        ("counts that do not decompress", Box::new(|p: &mut Vec<u8>| p.truncate(80))),
    ] {
        let d = Dir::new("damaged");
        let mut bad = good.clone();
        damage(&mut bad.payload);
        let mut writer = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
        writer.append(&bad).unwrap();
        writer.write_index().unwrap();
        writer.finish().unwrap();
        let store = TileStore::open(d.path(), w.params()).unwrap();
        let e = load_tile(&store, &w, RES, &s, tiles()[1]).expect_err(name).to_string();
        assert!(e.contains("layer tile 0/1/0") && e.contains("key"), "{name}: {e}");
    }
}

// spec: STRM-065
#[test]
fn the_pass_1_faces_are_dropped_from_a_store_and_never_from_another_folder() {
    let (w, p1) = (world(), Dir::new("p1"));
    faces(p1.path(), &w);
    drop_pass1(p1.path()).unwrap();
    assert!(!p1.path().exists(), "the pass-1 store is still there");
    let other = Dir::new("other");
    fs::create_dir_all(other.path()).unwrap();
    fs::write(other.path().join("notes.txt"), "mine").unwrap();
    let e = drop_pass1(other.path()).expect_err("a folder that is not a store").to_string();
    assert!(e.contains("refused") && other.path().join("notes.txt").exists(), "{e}");
}

// spec: STRM-065
#[test]
fn a_grid_that_cannot_be_filed_and_a_factor_that_disagrees_with_the_spec_are_refused() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    // Three tiles a face edge are not a power of two: no quadtree id.
    let odd = TileLayerSpec { grid: TileGrid { cells: 72, ..s.grid }, ..s };
    let e = run(Dir::new("odd").path(), &w, &odd, &f, &tiles()[..1], 1).expect_err("a grid of 3 tiles").to_string();
    assert!(e.contains("power of two"), "{e}");
    let with_factor = TileLayerSpec { uses_factor: true, rock: 0, ..s };
    let e = run(Dir::new("factor").path(), &w, &with_factor, &f, &tiles()[..1], 1).expect_err("a factor that is absent").to_string();
    assert!(e.contains("uses_factor"), "{e}");
}

/// A payload of `r`'s record with its stream replaced by a brotli stream of `raw`.
fn with_stream(good: &planet_cache::pack::Record, raw: &[u8]) -> planet_cache::pack::Record {
    use std::io::Write;
    let mut bad = good.clone();
    bad.payload.truncate(73);
    let mut w = brotli::CompressorWriter::new(&mut bad.payload, 4096, 5, 22);
    w.write_all(raw).unwrap();
    drop(w);
    bad
}

// spec: STRM-065
#[test]
fn a_stream_that_decodes_to_more_or_fewer_bytes_than_the_tile_is_refused_without_reading_it_all() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let reader = face_change_reader(&f);
    let good = tile_record(&w, RES, &s, &pass2_tile(&s.grid, tiles()[1], &base, &reader, None, &s.params, 2)).unwrap();
    let t = s.grid.tile;
    // A bomb: a megabyte of zeros where 24 x 24 x 2 bytes are expected.
    for (name, raw) in [
        ("too many bytes", vec![0u8; 1 << 20]),
        ("one byte too many", vec![0u8; t * t * 2 + 1]),
        ("one byte too few", vec![0u8; t * t * 2 - 1]),
    ] {
        let d = Dir::new("stream");
        let bad = with_stream(&good, &raw);
        let mut writer = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
        writer.append(&bad).unwrap();
        writer.write_index().unwrap();
        writer.finish().unwrap();
        let store = TileStore::open(d.path(), w.params()).unwrap();
        let e = load_tile(&store, &w, RES, &s, tiles()[1]).expect_err(name).to_string();
        assert!(e.contains("expected") && e.contains("layer tile 0/1/0"), "{name}: {e}");
    }
}

// spec: STRM-065
#[test]
fn counts_at_the_ends_of_the_i16_range_survive_the_row_delta() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let reader = face_change_reader(&f);
    let mut r = pass2_tile(&s.grid, tiles()[0], &base, &reader, None, &s.params, 2);
    let t = s.grid.tile;
    // Alternate the extreme counts along a row (differences of 65,535 steps wrap) and down a column.
    for (k, v) in r.core.iter_mut().enumerate() {
        *v = if (k % t + k / t).is_multiple_of(2) { -32768.0 * QUANT_STEP_M } else { 32767.0 * QUANT_STEP_M };
    }
    let d = Dir::new("extremes");
    let rec = tile_record(&w, RES, &s, &r).unwrap();
    let mut writer = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&rec).unwrap();
    writer.write_index().unwrap();
    writer.finish().unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let back = load_tile(&store, &w, RES, &s, tiles()[0]).unwrap().expect("the tile");
    assert_eq!(back.core, r.core, "extreme counts changed in the record");
}

// spec: STRM-065
#[test]
fn the_decoded_tile_cache_decodes_a_tile_once_holds_its_capacity_and_forgets_the_oldest() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let d = Dir::new("cache");
    run(d.path(), &w, &s, &f, &tiles(), 2).unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let cache = DecodedTileCache::new(2);
    let t = tiles();
    let a = cache.get(&store, &w, RES, &s, t[0]).unwrap().expect("tile 0");
    let again = cache.get(&store, &w, RES, &s, t[0]).unwrap().expect("tile 0");
    assert!(std::sync::Arc::ptr_eq(&a, &again), "a hit returned another copy");
    assert_eq!(cache.decodes(), 1, "a hit decoded again");
    cache.get(&store, &w, RES, &s, t[1]).unwrap().expect("tile 1");
    cache.get(&store, &w, RES, &s, t[0]).unwrap(); // tile 0 is now the newest
    cache.get(&store, &w, RES, &s, t[2]).unwrap().expect("tile 2"); // evicts tile 1
    assert_eq!((cache.len(), cache.decodes()), (2, 3));
    cache.get(&store, &w, RES, &s, t[0]).unwrap();
    assert_eq!(cache.decodes(), 3, "the newest tile was evicted");
    cache.get(&store, &w, RES, &s, t[1]).unwrap();
    assert_eq!(cache.decodes(), 4, "the oldest tile was kept");
    // A tile the store does not hold is not cached.
    let none = PassTile { face: Face(5), ti: 3, tj: 3 };
    assert!(cache.get(&store, &w, RES, &s, none).unwrap().is_none());
    assert_eq!(cache.len(), 2);
}

// spec: GEN-112
// spec: STRM-066
#[test]
fn a_store_layer_reads_the_stored_change_at_a_cell_centre_decodes_a_tile_once_and_builds_its_pyramid() {
    use planet_generators::erosion_layer::{ErosionLayer, LayerSource};
    use std::sync::Arc;
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let d = Dir::new("store-layer");
    run(d.path(), &w, &s, &f, &tiles(), 2).unwrap();
    let store = Arc::new(TileStore::open(d.path(), w.params()).unwrap());
    let t0 = tiles()[0];
    let want = load_tile(&store, &w, RES, &s, t0).unwrap().expect("tile 0");
    let layer = Arc::new(StoreLayer::new(store.clone(), Arc::new(world()), RES, s, 4, default_change_range()).unwrap());
    let el = ErosionLayer::new(layer.clone()).unwrap();
    // Cell (10, 7) of tile (0, 0) of face 0: the centre of that cell reads its stored value (the change of the tile's core, dequantised).
    let g = &s.grid;
    let dir = g.direction(Face(0), 10, 7);
    let got = el.change_at(dir).unwrap();
    assert!((got - f64::from(want.core[7 * g.tile + 10])).abs() < 1e-6, "read {got}, stored {}", want.core[7 * g.tile + 10]);
    let decodes = layer.decodes();
    for _ in 0..50 {
        el.change_at(dir).unwrap();
    }
    assert_eq!(layer.decodes(), decodes, "a tile already decoded was decoded again");
    // The pyramid of the tile holds its cells; a tile the store does not hold is absent.
    let tile = layer.tile(Face(0), 0, 0).unwrap().expect("tile 0");
    let [lo, hi] = tile.range();
    assert!(want.core.iter().all(|v| *v >= lo && *v <= hi));
    assert!(layer.tile(Face(5), 3, 3).unwrap().is_none());
    // A store opened beside another bake's key refuses, naming the tile, instead of mixing.
    let mut other = s;
    other.inputs_id = 2;
    let wrong = StoreLayer::new(store, Arc::new(world()), RES, other, 4, default_change_range()).unwrap();
    let e = wrong.tile(Face(0), 0, 0).err().expect("a refusal");
    assert!(e.contains("layer tile 0/0/0") && e.contains("refused"), "{e}");
}

// spec: GEN-112
#[test]
fn a_store_layer_refuses_a_tile_outside_its_global_range_and_forgets_the_oldest_tile() {
    use planet_generators::erosion_layer::LayerSource;
    use std::sync::Arc;
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let d = Dir::new("store-layer-range");
    run(d.path(), &w, &s, &f, &tiles(), 2).unwrap();
    let store = Arc::new(TileStore::open(d.path(), w.params()).unwrap());
    let t0 = tiles()[0];
    // The tile with the largest change: a range of half of it must be refused.
    let (big, top) = tiles()
        .into_iter()
        .map(|t| {
            let r = load_tile(&store, &w, RES, &s, t).unwrap().expect("a tile");
            (t, r.core.iter().fold(0.0f32, |m, v| m.max(v.abs())))
        })
        .fold((t0, 0.0f32), |a, b| if b.1 > a.1 { b } else { a });
    assert!(top > 0.5, "a tile with a change to refuse ({top})");
    // A range that does not hold the tile's change is an error naming the tile, not a silently unsound bound.
    let half = f64::from(top) * 0.5;
    let narrow = StoreLayer::new(store.clone(), Arc::new(world()), RES, s, 4, (-half, half)).unwrap();
    let e = narrow.tile(big.face, big.ti, big.tj).err().expect("a refusal");
    assert!(e.contains(&format!("layer tile {}/{}/{}", big.face.0, big.ti, big.tj)) && e.contains("global range"), "{e}");
    // A cache of two tiles decodes the third, forgets the oldest, and decodes it again when it is asked for.
    let layer = StoreLayer::new(store, Arc::new(world()), RES, s, 2, default_change_range()).unwrap();
    for t in tiles() {
        layer.tile(t.face, t.ti, t.tj).unwrap();
    }
    assert_eq!(layer.decodes(), 3);
    layer.tile(tiles()[1].face, tiles()[1].ti, tiles()[1].tj).unwrap();
    assert_eq!(layer.decodes(), 3, "a tile still held was decoded again");
    layer.tile(t0.face, t0.ti, t0.tj).unwrap();
    assert_eq!(layer.decodes(), 4, "the oldest tile was kept");
}

// spec: VIEW-062
#[test]
fn the_bake_specs_of_a_shape_are_the_commands_derivation_and_refuse_a_geometry_that_does_not_fit() {
    use planet_generators::bake::inputs::BakeInputs;
    let w = world();
    let shape = BakeShape { seed: 9, radius_m: R, face_res: 96, planet_res: 96, tile: Some(48), halo: None };
    let (layer, spec) = bake_specs(&w, &shape).unwrap();
    // The derivation of `cache/examples/bake_layer.rs`, written out: a halo of a twenty-fourth of the tile, pass 1's halo an eighth of a percent of the face edge (at least 4), the standard parameters.
    assert_eq!(spec.grid, TileGrid { cells: 96, tile: 48, halo: 2, radius_m: R });
    assert_eq!((layer.cells, layer.halo, layer.seed, layer.inputs_id, layer.uses_factor), (96, 4, 9, BakeInputs::id(), true));
    assert_eq!((spec.params, spec.inputs_id, spec.uses_factor), (Pass2Params::standard(9, 0.0), BakeInputs::id(), true));
    assert_eq!(spec.faces, faces_digest(&w, 96, &layer), "the pass-1 digest is the layer's own");
    let production = BakeShape::production(9, R);
    assert_eq!((production.face_res, production.planet_res), (12_288, 98_304));
    let (_, big) = bake_specs(&w, &production).unwrap();
    assert_eq!((big.grid.tile, big.grid.halo), (3072, 128));
    assert!(bake_specs(&w, &BakeShape { tile: Some(50), ..shape }).is_err(), "a tile that does not divide the face edge");
    assert!(bake_specs(&w, &BakeShape { face_res: 4, ..shape }).is_err(), "a pass 1 of 4 cells");
}

// spec: GEN-154
#[test]
fn the_stored_change_reads_back_as_the_quantisation_of_the_surface_gate() {
    // The gate's quantised L is `round(v / step)` clamped to the i16 counts, times the step; the counts stream decodes to the same values (G-SURF on the stored L, not on a model of it).
    let t = 24usize;
    let change: Vec<f32> = (0..t * t).map(|p| (((p * 37) % 211) as f32 - 100.0) * 0.173 + (p / t) as f32 * 1.7).collect();
    let step = f64::from(QUANT_STEP_M);
    let model: Vec<f64> =
        change.iter().map(|v| (libm::round(f64::from(*v) / step).clamp(f64::from(i16::MIN), f64::from(i16::MAX))) * step).collect();
    let back = decode_change_core(&encode_change_core(&change, t), t).expect("the stream decodes");
    assert_eq!(back.len(), model.len());
    for (b, m) in back.iter().zip(&model) {
        assert_eq!(f64::from(*b), *m, "the stored value is the gate's quantised value");
    }
}
