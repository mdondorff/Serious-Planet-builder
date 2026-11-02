use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
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

/// A store the tests only read: the slot shared by every test process of this binary (it stays on disk for the next process),
/// or a private copy (`true`) that leaves with the test.
struct Shared(PathBuf, bool);

impl Shared {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        if self.1 {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

/// The manifest of a built store directory: one line per file (sorted by name) with its length and CRC-32C, after a first line
/// with the slot's key. `MANIFEST` itself is not listed.
fn manifest(dir: &Path, key: &str) -> std::io::Result<String> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for e in fs::read_dir(dir)?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name != "MANIFEST" {
            files.push((name, fs::read(e.path())?));
        }
    }
    files.sort();
    let mut out = format!("{key}\n");
    for (name, bytes) in files {
        out.push_str(&format!("{name} {} {:08x}\n", bytes.len(), planet_cache::crc32c::crc32c(&bytes)));
    }
    Ok(out)
}

/// A store directory whose files match its manifest and whose manifest names `key`; a missing, stale or damaged one is not.
fn verified(dir: &Path, key: &str) -> bool {
    fs::read_to_string(dir.join("MANIFEST")).is_ok_and(|m| manifest(dir, key).is_ok_and(|now| now == m))
}

/// The store of `world_with_limit(7, limit)` at `RES` cells, built once and read by every test process of this binary (nextest
/// runs each test in its own process, so an in-memory fixture would share nothing). It lives under `CARGO_TARGET_TMPDIR`.
/// - Key: the bake's content key (generator version, world and resolution), the pack and block codec versions, the block limit
///   and the pack size, plus the test binary's size and time (a relinked binary never reads a store built by older code).
/// - A process builds into a directory of its own, writes its `MANIFEST` and renames it into place, so a reader never sees a
///   half-built store; when two build at once, the first rename wins and the other uses it.
/// - A slot whose files do not match its manifest is never read: the process builds and reads a private copy, and the damaged
///   slot stays until the binary's next build removes it (removing it here could remove a good slot another process just placed).
/// - The tests only open it (`TileStore::open` leaves the files alone), each with its own `TileStore`, so read counters start at 0.
fn shared_store(limit: u32) -> (Shared, GlobalWorld) {
    let w = world_with_limit(7, limit);
    let exe = std::env::current_exe().unwrap();
    let meta = fs::metadata(&exe).unwrap();
    let time = meta.modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let stem = format!("conc-source-{}-", exe.file_stem().unwrap().to_string_lossy());
    let build = format!("{stem}{}-{time}-", meta.len());
    let key = format!(
        "{}-p{}-b{}-limit{limit}-res{RES}-max{DEFAULT_MAX_PACK_BYTES}",
        w.stage_key(RES),
        planet_cache::pack::PACK_FORMAT_VERSION,
        planet_cache::global_store::BLOCK_CODEC_VERSION
    );
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let slot = root.join(format!("{build}{key}"));
    // The slots of this binary's earlier builds (best effort: a slot still open elsewhere stays).
    for e in fs::read_dir(root).unwrap().flatten() {
        let other = e.file_name().to_string_lossy().into_owned();
        if other.starts_with(&stem) && !other.starts_with(&build) {
            let _ = fs::remove_dir_all(e.path());
        }
    }
    if verified(&slot, &key) {
        return (Shared(slot, false), w);
    }
    static N: AtomicUsize = AtomicUsize::new(0);
    let tmp = root.join(format!("{build}tmp-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = fs::remove_dir_all(&tmp);
    build_global(&tmp, &w, RES, 3, DEFAULT_MAX_PACK_BYTES).unwrap();
    fs::write(tmp.join("MANIFEST"), manifest(&tmp, &key).unwrap()).unwrap();
    // A rename can fail for a moment on Windows while a scanner holds a file of the new directory, so it is retried while no
    // slot is there; an existing slot that fails its check leaves this process its own copy.
    for _ in 0..20 {
        if fs::rename(&tmp, &slot).is_ok() {
            return (Shared(slot, false), w);
        }
        if verified(&slot, &key) {
            let _ = fs::remove_dir_all(&tmp);
            return (Shared(slot, false), w);
        }
        if slot.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    (Shared(tmp, true), w)
}

fn stored() -> (Shared, GlobalWorld) {
    shared_store(4)
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
