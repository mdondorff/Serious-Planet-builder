//! GEN-020: batch evaluation is order independent; tiles are built in the order of the bake blocks their reads fall in.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::{Face, TileId, Vec3};
use planet_generators::compact::tile_data_field;
use planet_generators::erosion::{ErosionField, ErosionGenerator};
use planet_generators::global::{bake, GlobalGrid, GlobalSample, GridSource, LatticeSource, ReadError};
use planet_generators::height::HeightField;
use planet_generators::mesh::tile_mesh_field;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;
const SEED: u64 = 7;
const RES: u32 = 16384;
/// Cells per simulated block: 8 x 8 blocks per face (the product's ratio of tile samples to block cells at the roots).
const BLOCK: u32 = 1024;

/// A cheap deterministic bake, smooth and different on every face (no need to bake 6 x 1025^2 samples).
fn synthetic(face: Face, i: u32, j: u32) -> GlobalSample {
    let (x, y, f) = (f64::from(i) / 97.0, f64::from(j) / 83.0, f64::from(face.0));
    let e = 2500.0 * libm::sin(x + f) * libm::cos(y - 0.5 * f) + 700.0 * libm::sin(3.1 * x + 2.7 * y);
    GlobalSample { continent: 1.0, elevation_m: e as f32, temperature_c: 10.0, precipitation_mm: 500.0 }
}

type Lru = (Vec<(u8, u32, u32)>, u64);

/// A source that behaves like a block store with an LRU cache of `capacity` blocks: it counts its decodes and its reads, and
/// reports a block size (the hint of the batch order) only when `by_block`.
struct Simulated {
    capacity: usize,
    by_block: bool,
    reads: AtomicU64,
    state: Mutex<Lru>,
}

impl Simulated {
    fn new(capacity: usize, by_block: bool) -> Simulated {
        Simulated { capacity, by_block, reads: AtomicU64::new(0), state: Mutex::new((Vec::new(), 0)) }
    }
    fn decodes(&self) -> u64 {
        self.state.lock().unwrap().1
    }
}

impl LatticeSource for Simulated {
    fn res(&self) -> u32 {
        RES
    }
    fn block_cells(&self) -> u32 {
        if self.by_block {
            BLOCK
        } else {
            0
        }
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        let key = (face.0, (i / BLOCK).min(RES / BLOCK - 1), (j / BLOCK).min(RES / BLOCK - 1));
        let mut st = self.state.lock().unwrap();
        if let Some(at) = st.0.iter().position(|k| *k == key) {
            let k = st.0.remove(at);
            st.0.push(k);
        } else {
            st.1 += 1;
            if st.0.len() == self.capacity {
                st.0.remove(0);
            }
            st.0.push(key);
        }
        drop(st);
        Ok(synthetic(face, i, j))
    }
}

/// An in-memory bake with a block-size hint, so the batch order applies to it too.
struct Hinted<'a>(GridSource<'a>, u32);

impl LatticeSource for Hinted<'_> {
    fn res(&self) -> u32 {
        self.0.res()
    }
    fn block_cells(&self) -> u32 {
        self.1
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        self.0.at(face, i, j)
    }
}

fn field(src: &dyn LatticeSource) -> ErosionField<'_> {
    let def = WorldDefinition::single_planet(R, SEED);
    ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, src, [9; 16]).unwrap()
}

fn mesh_bits(f: &dyn HeightField, id: TileId, cells: u32) -> Vec<u32> {
    let m = tile_mesh_field(f, &TangentWarp, id, cells, R);
    let mut v = vec![m.key as u32, (m.key >> 32) as u32, m.origin.0.x.to_bits() as u32, m.origin.0.y.to_bits() as u32];
    for p in &m.vertices {
        v.extend(p.pos.iter().chain(&p.normal).chain(&p.coarse).chain([&p.height, &p.coarse_height]).map(|x| x.to_bits()));
    }
    v
}

/// A level-0 root, a coarse tile, a fine tile, a tile on a face edge and one at a cube corner.
fn tiles() -> Vec<TileId> {
    let t = |f, l, x, y| TileId::new(Face(f), l, x, y).unwrap();
    vec![t(2, 0, 0, 0), t(4, 2, 1, 2), t(1, 7, 40, 91), t(0, 3, 7, 3), t(5, 3, 7, 7), t(3, 3, 0, 0)]
}

/// Distinct blocks (with the neighbours the border samples read) a tile's reads touch: the decodes of an unbounded cache.
fn distinct_blocks(id: TileId, cells: u32) -> u64 {
    let src = Simulated::new(usize::MAX, false);
    tile_mesh_field(&field(&src), &TangentWarp, id, cells, R);
    src.decodes()
}

// spec: GEN-020
#[test]
fn a_coarse_tile_decodes_each_block_about_once_with_a_small_cache() {
    let id = TileId::new(Face(2), 0, 0, 0).unwrap();
    let distinct = distinct_blocks(id, 32);
    assert!(distinct >= 64, "the root spans the face: {distinct} blocks");
    for capacity in [1usize, 4, 31] {
        let raster = Simulated::new(capacity, false);
        let a = mesh_bits(&field(&raster), id, 32);
        let ordered = Simulated::new(capacity, true);
        let b = mesh_bits(&field(&ordered), id, 32);
        assert!(a == b, "block order changed a bit");
        println!("cache {capacity}: raster {} block {} distinct {distinct}", raster.decodes(), ordered.decodes());
        // The whole tile's cells are read block by block: a block is decoded once however small the cache.
        assert!(ordered.decodes() <= distinct, "cache of {capacity}: {} decodes for {distinct} distinct blocks", ordered.decodes());
        if capacity < 31 {
            assert!(raster.decodes() >= 3 * ordered.decodes(), "raster order thrashes: {} against {}", raster.decodes(), ordered.decodes());
        }
    }
}

// spec: GEN-020
#[test]
fn tiles_are_bit_identical_in_raster_and_block_order_on_the_synthetic_and_the_in_memory_bake() {
    let (raster, ordered) = (Simulated::new(3, false), Simulated::new(3, true));
    for id in tiles() {
        for cells in [8u32, 32] {
            assert!(mesh_bits(&field(&raster), id, cells) == mesh_bits(&field(&ordered), id, cells), "{id:?} cells {cells}");
        }
        let a = tile_data_field(&field(&raster), &TangentWarp, id, 32, R, 22);
        let b = tile_data_field(&field(&ordered), &TangentWarp, id, 32, R, 22);
        assert!(a == b, "{id:?}: the compact or vertex tile differs");
    }
    let def = WorldDefinition::single_planet(R, SEED);
    let grid: GlobalGrid = bake(GenContext::new(&def).body_seed(BodyId::PLANET).unwrap(), &TangentWarp, 128, 4).unwrap();
    let (plain, hinted) = (GridSource(&grid), Hinted(GridSource(&grid), 16));
    for id in tiles() {
        assert!(mesh_bits(&field(&plain), id, 16) == mesh_bits(&field(&hinted), id, 16), "{id:?}: in-memory bake");
    }
}

// spec: GEN-020
#[test]
fn a_batch_equals_the_single_heights_in_any_order() {
    let src = Simulated::new(2, true);
    let f = field(&src);
    let mut rng = SplitMix64(5);
    let mut axis = || rng.next_f64() * 2.0 - 1.0;
    let dirs: Vec<Vec3> = (0..300).map(|_| Vec3::new(axis(), axis(), axis()).normalized()).collect();
    let batch = f.heights(&dirs);
    let reversed: Vec<Vec3> = dirs.iter().rev().copied().collect();
    let back = f.heights(&reversed);
    for (k, d) in dirs.iter().enumerate() {
        assert_eq!(batch[k].to_bits(), f.height(*d).to_bits(), "sample {k}");
        assert_eq!(back[dirs.len() - 1 - k].to_bits(), batch[k].to_bits(), "sample {k} reversed");
    }
}

// spec: GEN-020
// spec: CON-14
#[test]
fn tiles_are_identical_at_1_and_20_threads() {
    let ids = tiles();
    let reference: Vec<Vec<u32>> = {
        let src = Simulated::new(usize::MAX, false);
        ids.iter().map(|&id| mesh_bits(&field(&src), id, 16)).collect()
    };
    for threads in [1usize, 20] {
        // A tiny cache under contention: every thread evicts the others' blocks.
        let src = Simulated::new(3, true);
        let f = field(&src);
        let out: Vec<Mutex<Vec<u32>>> = ids.iter().map(|_| Mutex::new(Vec::new())).collect();
        let next = AtomicU64::new(0);
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::SeqCst) as usize;
                    if i >= ids.len() * 3 {
                        break;
                    }
                    *out[i % ids.len()].lock().unwrap() = mesh_bits(&f, ids[i % ids.len()], 16);
                });
            }
        });
        for (k, o) in out.iter().enumerate() {
            assert!(*o.lock().unwrap() == reference[k], "{threads} threads, tile {k}");
        }
    }
}

// spec: GEN-020
// spec: STRM-005
#[test]
fn the_batch_stops_within_a_bounded_number_of_samples_once_the_flag_is_set() {
    let src = Simulated::new(4, true);
    let f = field(&src);
    let id = TileId::new(Face(2), 0, 0, 0).unwrap();
    let dirs: Vec<Vec3> =
        (0..=32u32).flat_map(|j| (0..=32u32).map(move |i| (i, j))).map(|(i, j)| id.sample_direction(&TangentWarp, 32, i, j)).collect();
    // While the cells are being read: the flag is polled at least every 64 reads.
    let limit = 500u64;
    let h = f.heights_until(&dirs, &|| src.reads.load(Ordering::Relaxed) >= limit);
    let reads = src.reads.load(Ordering::Relaxed);
    assert!(reads <= limit + 64, "{reads} reads after the flag was set at {limit}");
    assert!(h.iter().all(|x| *x == 0.0), "an unfinished batch leaves 0.0");
    // While the samples are being evaluated: the flag is polled at least every 64 samples. A dry run counts the polls.
    let polls = AtomicU64::new(0);
    let full = f.heights_until(&dirs, &|| {
        polls.fetch_add(1, Ordering::Relaxed);
        false
    });
    let total = polls.load(Ordering::Relaxed);
    assert!(full.iter().any(|x| *x != 0.0));
    let calls = AtomicU64::new(0);
    let cut = f.heights_until(&dirs, &|| calls.fetch_add(1, Ordering::Relaxed) + 3 >= total);
    let done = cut.iter().filter(|x| **x != 0.0).count();
    assert!(done > 0 && done + 3 * 64 >= dirs.len() && done < dirs.len(), "{done} of {} samples before the flag", dirs.len());
    assert!(calls.load(Ordering::Relaxed) <= total, "no polling after the flag is set");
    let before = src.reads.load(Ordering::Relaxed);
    let none = f.heights_until(&dirs, &|| true);
    assert!(none.iter().all(|x| *x == 0.0) && src.reads.load(Ordering::Relaxed) == before, "a set flag reads nothing");
}
