//! GEN-020: batch evaluation is order independent; tiles are built in the order of the bake blocks their reads fall in.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::{Face, TileId, Vec3};
use planet_generators::compact::compact_tile_field;
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
        let a = compact_tile_field(&field(&raster), &TangentWarp, id, 32, R);
        let b = compact_tile_field(&field(&ordered), &TangentWarp, id, 32, R);
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

// spec: GEN-020
// spec: CON-14
#[test]
fn a_threaded_batch_equals_the_single_heights_at_every_thread_count_and_reads_each_block_once() {
    let mut rng = SplitMix64(11);
    let mut axis = || rng.next_f64() * 2.0 - 1.0;
    // Few directions over many blocks (a cold plan at altitude), and a larger batch.
    for n in [3usize, 52, 300] {
        let dirs: Vec<Vec3> = (0..n).map(|_| Vec3::new(axis(), axis(), axis()).normalized()).collect();
        let single: Vec<u64> = {
            let src = Simulated::new(usize::MAX, false);
            let f = field(&src);
            dirs.iter().map(|&d| f.height(d).to_bits()).collect()
        };
        let distinct = {
            let src = Simulated::new(usize::MAX, true);
            field(&src).heights(&dirs);
            src.decodes()
        };
        for threads in [0usize, 1, 2, 5, 20] {
            // A cache of every block: the decodes are those of a source that never forgets, per block read.
            let src = Simulated::new(usize::MAX, true);
            let got: Vec<u64> = field(&src).heights_threaded(&dirs, threads).iter().map(|h| h.to_bits()).collect();
            assert!(got == single, "{n} directions on {threads} threads");
            assert_eq!(src.decodes(), distinct, "{n} directions on {threads} threads: every distinct block once");
        }
    }
}

// spec: GEN-020
#[test]
fn a_threaded_batch_reads_nothing_once_the_flag_is_set() {
    let src = Simulated::new(usize::MAX, true);
    let f = field(&src);
    let mut rng = SplitMix64(3);
    let mut axis = || rng.next_f64() * 2.0 - 1.0;
    let dirs: Vec<Vec3> = (0..200).map(|_| Vec3::new(axis(), axis(), axis()).normalized()).collect();
    let none = f.heights_threaded_until(&dirs, &|| true, 8);
    assert!(none.iter().all(|x| *x == 0.0) && src.reads.load(Ordering::Relaxed) == 0, "a set flag reads nothing");
    // Set while the blocks are being read: the threads stop within a bounded number of reads.
    let limit = 300u64;
    let cut = f.heights_threaded_until(&dirs, &|| src.reads.load(Ordering::Relaxed) >= limit, 8);
    let reads = src.reads.load(Ordering::Relaxed);
    assert!(reads <= limit + 8 * 64, "{reads} reads after the flag was set at {limit}");
    assert!(cut.iter().all(|x| *x == 0.0), "an unfinished batch leaves 0.0");
}

/// A compact batch of `n` x `n` directions on one face, `step` apart in face coordinates round `(s0, t0)`: the shape of a chunk's region or a tile.
fn compact_batch(face: u8, s0: f64, t0: f64, step: f64, n: usize) -> Vec<Vec3> {
    let h = n as f64 / 2.0;
    (0..n * n)
        .map(|k| {
            let (a, b) = ((k % n) as f64 - h, (k / n) as f64 - h);
            planet_core::cube::face_to_direction(&TangentWarp, Face(face), s0 + a * step, t0 + b * step)
        })
        .collect()
}

// spec: GEN-020
// spec: CON-14
// spec: GEN-079
#[test]
fn a_compact_batch_takes_its_footprint_from_the_hint_and_has_the_bits_of_the_single_heights_at_1_and_8_threads() {
    // 100 x 100 directions 1.5e-5 apart in face coordinates (a bake cell is 1.2e-4): about 12 cells across. The first batch straddles a block edge (the
    // face centre is the edge of the blocks 7 and 8), the second lies inside a block, the third a few cells from a face edge (the hint is declined there)
    // and the fourth at a cube corner.
    let batches = [
        compact_batch(2, 0.0, 0.0, 1.5e-5, 100),
        compact_batch(1, 0.3, -0.2, 1.5e-5, 100),
        compact_batch(4, 0.99965, 0.2, 1.5e-5, 100),
        compact_batch(5, 0.9998, 0.9998, 1.5e-5, 100),
    ];
    for (b, dirs) in batches.iter().enumerate() {
        let direct = Simulated::new(usize::MAX, false);
        let want: Vec<u64> = dirs.iter().map(|&d| field(&direct).height(d).to_bits()).collect();
        for threads in [1usize, 8] {
            let src = Simulated::new(3, true);
            let f = field(&src);
            let got: Vec<u64> = f.heights_threaded_until(dirs, &|| false, threads).iter().map(|h| h.to_bits()).collect();
            assert!(got == want, "batch {b}, {threads} threads: a height differs from the single read");
            assert!(f.error().is_none(), "batch {b}, {threads} threads: a read failed: {:?}", f.error());
        }
    }
}

// spec: GEN-020
// spec: GEN-079
#[test]
fn the_hinted_footprint_reads_about_the_cells_the_batch_needs_and_decodes_each_block_once() {
    let dirs = compact_batch(2, 0.0, 0.0, 1.5e-5, 100);
    let src = Simulated::new(1, true);
    let f = field(&src);
    let _ = f.heights_threaded_until(&dirs, &|| false, 1);
    // The cells of a 12-cell batch with a 3-cell stencil and a box of 8: a few hundred, on the four blocks round the face centre; never one per read.
    let reads = src.reads.load(Ordering::Relaxed);
    assert!(reads < 2_000, "{reads} cell reads for a batch of about 12 x 12 cells");
    assert!(src.decodes() <= 4, "{} block decodes for the four blocks round the centre", src.decodes());
}

// spec: GEN-079
#[test]
fn a_hinted_batch_at_the_middle_of_a_face_edge_where_the_chart_stretches_most_has_the_bits_of_the_single_heights() {
    // The chart stretches a stencil by sqrt(2) at the middle of a face edge: 100 x 100 directions 1.5e-5 apart, centred 0.0015 (about 12 cells) inside the edge.
    let dirs = compact_batch(4, 0.0, 0.9985, 1.5e-5, 100);
    let direct = Simulated::new(usize::MAX, false);
    let want: Vec<u64> = dirs.iter().map(|&d| field(&direct).height(d).to_bits()).collect();
    for threads in [1usize, 8] {
        let src = Simulated::new(3, true);
        let f = field(&src);
        let got: Vec<u64> = f.heights_threaded_until(&dirs, &|| false, threads).iter().map(|h| h.to_bits()).collect();
        assert!(got == want && f.error().is_none(), "{threads} threads: {:?}", f.error());
        // The hint was taken (a few hundred cells, not one per read).
        assert!(src.reads.load(Ordering::Relaxed) < 3_000, "{} reads", src.reads.load(Ordering::Relaxed));
    }
}

// spec: GEN-079
#[test]
fn a_sparse_batch_takes_the_exact_footprint_and_a_box_that_is_too_small_fails_loudly_instead_of_changing_a_value() {
    use planet_generators::global::{eval_gathered, read_direction};
    // 33 x 33 directions 32 bake cells apart (a coarse tile): the hinted boxes would gather 100 times the cells the reads touch.
    let dirs = compact_batch(2, 0.0, 0.0, 32.0 * 1.2e-4, 33);
    let src = Simulated::new(3, true);
    let _ = field(&src).heights_threaded_until(&dirs, &|| false, 1);
    let reads = src.reads.load(Ordering::Relaxed);
    assert!(
        reads < 60_000,
        "{reads} cell reads for a sparse batch of 33 x 33 (the exact footprint is about 22,000, the boxes would be about 350,000)"
    );
    // A stencil of 5 cells: a box of 8 holds it, a box of 1 does not (the missing cells are errors, here NaN, never another value).
    let stencil = |s: &dyn LatticeSource, d: Vec3| -> f64 {
        let step = 5.0 * 1.2e-4;
        let mut sum = 0.0;
        for (a, b) in [(step, 0.0), (-step, 0.0), (0.0, step), (0.0, -step)] {
            let e = planet_core::cube::direction_to_face(&TangentWarp, d);
            let moved = planet_core::cube::face_to_direction(&TangentWarp, e.0, e.1 + a, e.2 + b);
            sum += read_direction(s, &TangentWarp, moved).map_or(f64::NAN, |x| f64::from(x.elevation_m));
        }
        sum
    };
    let footprint = |g: &dyn LatticeSource, d: Vec3| {
        stencil(g, d);
    };
    let compact = compact_batch(1, 0.3, -0.2, 1.5e-5, 100);
    let src = Simulated::new(3, true);
    let direct: Vec<u64> = compact.iter().map(|&d| stencil(&src, d).to_bits()).collect();
    let hinted = eval_gathered(&src, &compact, &|| false, &footprint, Some(8), &stencil);
    assert!(hinted.iter().map(|h| h.to_bits()).eq(direct.iter().copied()), "a box of 8 changed a value");
    let small = eval_gathered(&src, &compact, &|| false, &footprint, Some(1), &stencil);
    assert!(small.iter().any(|h| h.is_nan()), "a box of 1 did not miss a cell of a 5-cell stencil");
}
