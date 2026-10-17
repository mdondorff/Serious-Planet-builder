use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use planet_cache::global_store::{build_global, GlobalWorld, StoreSource, SOURCE_CACHE_BYTES};
use planet_cache::pack::{TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::erosion::ErosionGenerator;
use planet_generators::global::{bake, GlobalSample, GridSource, LatticeSource};
use planet_generators::mesh::tile_mesh_field;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;
const RES: u32 = 64;
// Blocks of 4 cells: 25 samples of 16 bytes; a grid of 64 cells is level 4, 16 x 16 blocks per face.
const BLOCK_BYTES: u64 = 400;
const BLOCKS: u64 = 6 * 16 * 16;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-conc-source-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

fn world_with_limit(seed: u64, limit: u32) -> GlobalWorld {
    GlobalWorld::from_definition(&WorldDefinition::single_planet(R, seed)).unwrap().with_block_limit(limit)
}

fn world(seed: u64) -> GlobalWorld {
    // A small block limit (store policy, ADR 0027 point 1) so small grids still have many blocks.
    world_with_limit(seed, 4)
}

fn bits(s: &GlobalSample) -> [u32; 4] {
    [s.continent.to_bits(), s.elevation_m.to_bits(), s.temperature_c.to_bits(), s.precipitation_mm.to_bits()]
}

fn stored(name: &str) -> (Dir, GlobalWorld) {
    let d = Dir::new(name);
    let w = world(7);
    build_global(d.path(), &w, RES, 3, DEFAULT_MAX_PACK_BYTES).unwrap();
    (d, w)
}

/// Every vertex of the grid in a thread-specific order (a stride coprime to the vertex count), so threads collide on blocks.
fn read_all(src: &dyn LatticeSource, grid: &GridSource, thread: usize) {
    let n = (RES + 1) as usize;
    let total = 6 * n * n;
    let stride = [1usize, 7, 11, 17, 19, 23, 29, 31][thread % 8];
    let mut at = thread * 977 % total;
    for _ in 0..total {
        let f = at / (n * n);
        let (i, j) = ((at % (n * n)) % n, (at % (n * n)) / n);
        let (face, i, j) = (Face(f as u8), i as u32, j as u32);
        assert_eq!(bits(&src.at(face, i, j).unwrap()), bits(&grid.at(face, i, j).unwrap()), "thread {thread} face {f} ({i}, {j})");
        at = (at + stride) % total;
    }
}
