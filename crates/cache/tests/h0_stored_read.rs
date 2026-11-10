//! GEN-164 (item 3, inbox #123, decision D14): H0 read from a stored bake equals the in-memory H0 within the store's quantisation step, at every cell of a cube edge and of a cube corner. H0's
//! base is the C1 read of the height (and the bilinear read of the climate) of the field at each cell centre; here it is read through `StoreSource` and through `GridSource` on the same small
//! bake. The production block codec is lossless (STRM-012: f32 samples in byte planes), so the store's quantisation step is 0 and the two reads are equal bit for bit. No preview code is involved.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::global_store::{build_global, GlobalWorld, StoreSource};
use planet_cache::pack::{TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::cube::TangentWarp;
use planet_generators::bake::h0::GlobalGrid;
use planet_generators::global::{bake, locate_direction, read_direction, read_elevation_c1, GridSource, LatticeSource};
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

/// H0's height, rainfall and temperature of cell `id`: the field read at the cell's centre.
fn h0_cell(src: &dyn LatticeSource, grid: &GlobalGrid, id: usize) -> (f32, f32, f32) {
    let d = grid.direction(id as u32);
    let s = read_direction(src, &TangentWarp, d).expect("the field reads");
    (read_elevation_c1(src, &TangentWarp, d).expect("the field reads"), s.precipitation_mm, s.temperature_c)
}

// spec: GEN-164
#[test]
fn h0_read_from_a_stored_bake_equals_the_in_memory_h0_at_every_cube_edge_and_corner() {
    let seed = 7;
    let d = Dir::new("h0");
    // Blocks of 4 cells on a 16-cell lattice: 96 blocks, so block borders and every face edge are exercised (the global-store tests' world).
    let w = GlobalWorld::from_definition(&WorldDefinition::single_planet(R, seed)).unwrap().with_block_limit(4);
    build_global(&d.0, &w, 16, 3, DEFAULT_MAX_PACK_BYTES).unwrap();
    let store = TileStore::open(&d.0, w.params()).unwrap();
    let src = StoreSource::new(&store, &w, 16).unwrap();
    let grid = bake(seed, &TangentWarp, 16, 2).unwrap();
    let mem = GridSource(&grid);
    // H0 cells of a grid that is not the lattice (12 cells a face edge against 16): every cell is read between lattice vertices, an edge cell with a stencil that reaches over the edge.
    let n = 12;
    let h0 = GlobalGrid::new(n, R);
    let step = 0.0; // the stored block codec is lossless: the quantisation step of the production store is 0
    let (mut edge_cells, mut corner_cells, mut worst) = (0, 0, 0.0f64);
    for f in 0..6usize {
        for j in 0..n {
            for i in 0..n {
                let id = f * n * n + j * n + i;
                let (on_edge, on_corner) = (i == 0 || j == 0 || i == n - 1 || j == n - 1, (i == 0 || i == n - 1) && (j == 0 || j == n - 1));
                let (stored, memory) = (h0_cell(&src, &h0, id), h0_cell(&mem, &h0, id));
                worst = worst
                    .max(f64::from((stored.0 - memory.0).abs()))
                    .max(f64::from((stored.1 - memory.1).abs()))
                    .max(f64::from((stored.2 - memory.2).abs()));
                assert_eq!(
                    (stored.0.to_bits(), stored.1.to_bits(), stored.2.to_bits()),
                    (memory.0.to_bits(), memory.1.to_bits(), memory.2.to_bits()),
                    "face {f} cell ({i}, {j})"
                );
                if on_edge {
                    // The stencil of an edge cell reaches over the face edge: its lattice cell is the face's first or last (a ghost node on the neighbour face is read).
                    let at = locate_direction(16, &TangentWarp, h0.direction(id as u32)).unwrap();
                    assert!(
                        at.i == 0 || at.j == 0 || at.i == 15 || at.j == 15,
                        "face {f} cell ({i}, {j}) reads lattice cell ({}, {})",
                        at.i,
                        at.j
                    );
                    edge_cells += 1;
                    corner_cells += usize::from(on_corner);
                }
            }
        }
    }
    assert_eq!((edge_cells, corner_cells), (6 * (4 * n - 4), 6 * 4), "every face's edge cells and corners were compared");
    assert!(worst <= step, "the stored read is within the store's quantisation step ({step}): worst {worst}");
    // A missing block is an error of the read, never a placeholder.
    let empty = Dir::new("empty");
    fs::create_dir_all(&empty.0).unwrap();
    let nothing = TileStore::open(&empty.0, w.params()).unwrap();
    let missing = StoreSource::new(&nothing, &w, 16).unwrap();
    assert!(read_elevation_c1(&missing, &TangentWarp, h0.direction(0)).is_err());
}
