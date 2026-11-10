//! GEN-164 (item 3, inbox #123): H0's base read from a stored bake, across the cube edges and a corner. The stored bake is lossless (STRM-012: f32 samples in byte planes), so the store's
//! quantisation step is 0 and the stored read must equal the in-memory read bit for bit; the H0 routing of both is the same routing. Small grids; the world is the global-store tests'.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::global_store::{build_global, GlobalWorld, StoreSource};
use planet_cache::pack::{TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::cube::TangentWarp;
use planet_generators::bake::preview::PreviewBase;
use planet_generators::global::{bake, locate_direction, GridSource, LatticeSource, ReadError};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-h0-stored-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&p);
        Dir(p)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// spec: GEN-164
#[test]
fn h0_read_from_stored_blocks_is_h0_read_from_the_in_memory_bake_across_every_cube_edge_and_corner() {
    let seed = 7;
    let d = Dir::new("h0");
    // Blocks of 4 cells on a 16-cell lattice: 96 blocks, so block borders and every face edge are exercised (the global-store tests' world).
    let w = GlobalWorld::from_definition(&WorldDefinition::single_planet(R, seed)).unwrap().with_block_limit(4);
    build_global(&d.0, &w, 16, 3, DEFAULT_MAX_PACK_BYTES).unwrap();
    let store = TileStore::open(&d.0, w.params()).unwrap();
    let src = StoreSource::new(&store, &w, 16).unwrap();
    let grid = bake(seed, &TangentWarp, 16, 2).unwrap();
    let mem = GridSource(&grid);
    // H0 cells of a grid that is not the lattice (12 cells a face edge against 16): every cell is read between lattice vertices, the edge cells with a stencil that reaches over the edge.
    let n = 12;
    let stored = PreviewBase::from_field(n, &src, &TangentWarp, 1).expect("the stored field reads");
    let memory = PreviewBase::from_field(n, &mem, &TangentWarp, 1).expect("the in-memory field reads");
    let (hs, hm) = (stored.heights(), memory.heights());
    let grid_h0 = planet_generators::bake::h0::GlobalGrid::new(n, 6_371_000.0);
    assert_eq!(hs.len(), 6 * n * n);
    let step = 0.0; // the stored block codec is lossless: the quantisation step of the production store is 0
    let mut worst = 0.0f64;
    let (mut edge_cells, mut corner_cells) = (0, 0);
    for f in 0..6usize {
        for j in 0..n {
            for i in 0..n {
                let id = f * n * n + j * n + i;
                let on_edge = i == 0 || j == 0 || i == n - 1 || j == n - 1;
                let on_corner = (i == 0 || i == n - 1) && (j == 0 || j == n - 1);
                edge_cells += usize::from(on_edge);
                corner_cells += usize::from(on_corner);
                worst = worst.max((hs[id] - hm[id]).abs());
                assert_eq!(hs[id].to_bits(), hm[id].to_bits(), "face {f} cell ({i}, {j}) height");
                assert_eq!(stored.rainfall()[id].to_bits(), memory.rainfall()[id].to_bits(), "face {f} cell ({i}, {j}) rainfall");
                assert_eq!(stored.temperature()[id].to_bits(), memory.temperature()[id].to_bits(), "face {f} cell ({i}, {j}) temperature");
                if on_edge {
                    // The stencil of an edge cell reaches over the face edge: its lattice cell is the face's first or last (a ghost node on the neighbour face is read), and a corner cell reads the diagonal ghost.
                    let at = locate_direction(16, &TangentWarp, grid_h0.direction(id as u32)).unwrap();
                    assert!(
                        at.i == 0 || at.j == 0 || at.i == 15 || at.j == 15,
                        "face {f} cell ({i}, {j}) reads lattice cell ({}, {})",
                        at.i,
                        at.j
                    );
                }
            }
        }
    }
    assert!(worst <= step, "the stored read is within the store's quantisation step ({step} m): worst {worst} m");
    assert_eq!((edge_cells, corner_cells), (6 * (4 * n - 4), 6 * 4), "every face's edge cells and corners were compared");
    // Reported, not gated: the read is an interpolant of a 16-cell lattice, so it differs from the generator's own sample at the same cell centre by the interpolation error, not by the store.
    let gridn = planet_generators::bake::h0::GlobalGrid::new(n, 6_371_000.0);
    let off = (0..6 * n * n)
        .map(|id| (hs[id] - f64::from(planet_generators::global::sample(seed, gridn.direction(id as u32)).elevation_m)).abs())
        .fold(0.0f64, f64::max);
    eprintln!("H0 stored read against the generator's own sample at the cell centres (16-cell lattice, 12-cell H0): worst {off:.1} m");
    assert!(off.is_finite());
    // The routing of both is one routing: receivers, areas and the outlet each cell drains to.
    let (a, b) = (stored.base_routing(), memory.base_routing());
    assert_eq!(a.receiver, b.receiver);
    assert_eq!(a.outlet_of, b.outlet_of);
    assert_eq!(a.area.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), b.area.iter().map(|x| x.to_bits()).collect::<Vec<_>>());
    // A missing block is an error, never a placeholder.
    let empty = Dir::new("empty");
    fs::create_dir_all(&empty.0).unwrap();
    let nothing = TileStore::open(&empty.0, w.params()).unwrap();
    let missing = StoreSource::new(&nothing, &w, 16).unwrap();
    assert!(matches!(PreviewBase::from_field(n, &missing, &TangentWarp, 1), Err(ReadError::Source(_))));
    // Sources and shapes that cannot make a base are errors, not panics or placeholders.
    assert!(matches!(PreviewBase::from_field(1, &src, &TangentWarp, 1), Err(ReadError::Shape(_))));
    assert!(
        matches!(PreviewBase::from_field(n, &Dry(100.0), &TangentWarp, 1), Err(ReadError::Shape(_))),
        "no cell at or below the sea level"
    );
    assert!(
        matches!(PreviewBase::from_field(n, &Dry(f32::NAN), &TangentWarp, 1), Err(ReadError::Source(_))),
        "a height that is not a number"
    );
    assert!(
        matches!(PreviewBase::from_field(n, &Recording(&mem), &TangentWarp, 1), Err(ReadError::Source(_))),
        "a source that only records reads"
    );
}

/// A field with one height everywhere.
struct Dry(f32);

impl LatticeSource for Dry {
    fn res(&self) -> u32 {
        16
    }
    fn at(&self, _: planet_core::Face, _: u32, _: u32) -> Result<planet_generators::global::GlobalSample, ReadError> {
        Ok(planet_generators::global::GlobalSample { continent: 1.0, elevation_m: self.0, temperature_c: 10.0, precipitation_mm: 500.0 })
    }
}

/// A source that only records which cells are asked for (the footprint pass of a gathered batch).
struct Recording<'a>(&'a dyn LatticeSource);

impl LatticeSource for Recording<'_> {
    fn res(&self) -> u32 {
        self.0.res()
    }
    fn at(&self, face: planet_core::Face, i: u32, j: u32) -> Result<planet_generators::global::GlobalSample, ReadError> {
        self.0.at(face, i, j)
    }
    fn records_reads(&self) -> bool {
        true
    }
}
