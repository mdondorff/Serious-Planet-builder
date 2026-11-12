//! The terrain field of a run, chosen by the world (GEN-018): `test-render --scene terrain --world FILE` resolves the
//! definition's `world.height_field` (the eroded field by default, `cpu-terrain-v1` or the reference when named, an unknown id
//! refused with the known ids) and builds it over an in-memory bake of `--global-res` cells per face edge. A run that names no
//! world file is the seed-only single planet and names `cpu-ref-height-v2` explicitly: every golden, plan snapshot and repro
//! bundle was made with it, so they do not move.

use std::path::Path;
use std::sync::Arc;

use crate::coupledworld::{load_or_run, run_cells, Standing, COUPLED_STEPS};
use planet_cache::global_store::{GlobalWorld, StoreSource};
use planet_cache::meshpack::TileSpec;
use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::chain::{field_chain_erosion, CoupledLensField, NoBake, OwnedBake, OwnedOcean};
use planet_generators::coupled::CoupledLens;
use planet_generators::global::{GlobalCoarseGenerator, GlobalSample, LatticeSource, ReadError};
use planet_generators::height::{HeightField, IMPLEMENTATION_ID as REFERENCE_ID};
use planet_generators::height_fields::{resolve, HeightFieldKind};
use planet_generators::ocean::{tiled_ocean, OceanLattice};
use planet_generators::water::{SeaProducer, WaterField};
use planet_world_def::{BodyId, GenContext, WorldDefinition};

use crate::Options;

/// The largest in-memory bake a run builds (cells per face edge): 1024 is about 100 MB.
pub const MAX_RUN_BAKE_RES: u32 = 1024;

/// The definition of a run that names no world file: the single planet of `seed` with the reference field named explicitly.
pub fn legacy_world(seed: u64, radius_m: f64) -> WorldDefinition {
    let mut def = WorldDefinition::single_planet(radius_m, seed);
    def.set_height_field(REFERENCE_ID);
    def
}

/// Load a definition file.
pub fn load_world(path: &Path) -> Result<WorldDefinition, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    planet_world_def::toml_io::parse_definition(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The store the editor opens when `--store` is not given (VIEW-052), the first that exists: the directory in the `PLANET_STORE`
/// environment variable, the directory named in the pointer file `target/stores/default` (written by `cargo xtask store`), then
/// `target/stores/global16384` and `target/accept/global16384` under `root`.
pub fn default_store(root: &Path) -> Option<std::path::PathBuf> {
    default_store_in(root, std::env::var_os("PLANET_STORE").map(std::path::PathBuf::from))
}

/// [`default_store`] with the environment's value given.
pub fn default_store_in(root: &Path, env: Option<std::path::PathBuf>) -> Option<std::path::PathBuf> {
    let pointer = std::fs::read_to_string(root.join("target/stores/default")).ok().map(|t| std::path::PathBuf::from(t.trim()));
    let found = [env, pointer, Some(root.join("target/stores/global16384")), Some(root.join("target/accept/global16384"))];
    found.into_iter().flatten().map(|p| if p.is_relative() { root.join(p) } else { p }).find(|p| p.is_dir())
}

/// What `--erosion on` takes besides the world (VIEW-062).
#[derive(Clone, Debug)]
pub struct ErosionOptions {
    /// The folder of the layer; the world's store when none.
    pub layer_dir: Option<std::path::PathBuf>,
    /// Replacements of the layer manifest's pass-1 and pass-2 cells a face edge (`--face-res`, `--planet-res`); none: the manifest's, or the production bake's when the layer has no manifest.
    pub face_res: Option<usize>,
    pub planet_res: Option<usize>,
    /// Draw the layer although the store holds the legacy generator's bake.
    pub unchecked: bool,
    /// Fill the tiles' driver block from the layer's baked channels (VIEW-066, GEN-119).
    pub channels: bool,
}

/// Decoded tiles the layer keeps (a cap's bounds read up to 36, a read a stencil's four; a core is 37.7 MB at 3,072 cells, so up to about 1.7 GB with the pyramids).
const LAYER_CACHE_TILES: usize = 44;

/// The leaked stored bake as the field's lattice: it keeps the handle for the counters (`WorldTerrain::counters`).
struct SharedStore(&'static StoreSource<'static>);

impl LatticeSource for SharedStore {
    fn res(&self) -> u32 {
        self.0.res()
    }

    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        self.0.at(face, i, j)
    }

    fn block_cells(&self) -> u32 {
        self.0.block_cells()
    }

    fn relief_scale(&self) -> f64 {
        self.0.relief_scale()
    }
}

/// What the world's tile source did so far, recorded only (STRM-022): the block cache of the stored bake, the pack's lookups and the
/// pre-filled coarse tile lookups. The difference of two readings is a stretch of a run. Never an input of a decision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceCounters {
    pub blocks: planet_cache::global_store::SourceStats,
    pub pack: planet_cache::pack::StoreReadStats,
    /// Calls of the pre-filled tile lookup that reached the pack (a tile shallow enough to be stored), those that found the
    /// tile, and the nanoseconds all threads together spent in the lookup (key, pack read, decode).
    pub prefill_lookups: u64,
    pub prefill_found: u64,
    pub prefill_ns: u64,
    /// Calls of the lookup for a tile too deep to be stored (answered `None` at once).
    pub prefill_skipped: u64,
    /// Nanoseconds the tile jobs together spent getting blocks from the shared cache (lock, waiting for another thread's decode,
    /// decoding), a part of their load time.
    pub job_block_wait_ns: u64,
    /// Blocks decoded by the tile jobs themselves (the rest, `blocks.decodes` less this, were decoded for the plan worker).
    pub job_block_decodes: u64,
    /// Fields built (every tile load and every plan builds one) and the nanoseconds that took, all threads together.
    pub field_builds: u64,
    pub field_ns: u64,
}

impl SourceCounters {
    /// What happened between `earlier` and `self`; the block cache's residency figures are those of `self`.
    pub fn since(&self, earlier: &SourceCounters) -> SourceCounters {
        let b = |now: u64, then: u64| now.saturating_sub(then);
        SourceCounters {
            blocks: planet_cache::global_store::SourceStats {
                decodes: b(self.blocks.decodes, earlier.blocks.decodes),
                shared_lookups: b(self.blocks.shared_lookups, earlier.blocks.shared_lookups),
                shared_hits: b(self.blocks.shared_hits, earlier.blocks.shared_hits),
                evictions: b(self.blocks.evictions, earlier.blocks.evictions),
                decode_waits: b(self.blocks.decode_waits, earlier.blocks.decode_waits),
                lock_wait_ns: b(self.blocks.lock_wait_ns, earlier.blocks.lock_wait_ns),
                fetch_ns: b(self.blocks.fetch_ns, earlier.blocks.fetch_ns),
                decode_ns: b(self.blocks.decode_ns, earlier.blocks.decode_ns),
                ..self.blocks
            },
            pack: planet_cache::pack::StoreReadStats {
                gets: b(self.pack.gets, earlier.pack.gets),
                found: b(self.pack.found, earlier.pack.found),
                get_ns: b(self.pack.get_ns, earlier.pack.get_ns),
            },
            prefill_lookups: b(self.prefill_lookups, earlier.prefill_lookups),
            prefill_found: b(self.prefill_found, earlier.prefill_found),
            prefill_ns: b(self.prefill_ns, earlier.prefill_ns),
            prefill_skipped: b(self.prefill_skipped, earlier.prefill_skipped),
            job_block_wait_ns: b(self.job_block_wait_ns, earlier.job_block_wait_ns),
            job_block_decodes: b(self.job_block_decodes, earlier.job_block_decodes),
            field_builds: b(self.field_builds, earlier.field_builds),
            field_ns: b(self.field_ns, earlier.field_ns),
        }
    }
}

/// What the bake holds at a direction (see [`WorldTerrain::bake_probe`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BakeProbe {
    pub continent: f32,
    pub elevation_m: f32,
    pub ocean_vertex: bool,
    /// Whether a vertex of its cell is wet (ADR 0071): ocean, or reached by the sea through ground at most the conduction bound high.
    pub wet_vertex: bool,
}

/// A world's terrain: the definition, the field it selects and the bake the field reads.
pub struct WorldTerrain {
    /// The definition the fields are built from, replaceable while the terrain is shared ([`WorldTerrain::set_definition`]): a reader takes
    /// the `Arc` once per job, so a job in flight keeps the definition it started with.
    def: std::sync::RwLock<Arc<WorldDefinition>>,
    kind: HeightFieldKind,
    src: Box<dyn LatticeSource + Send>,
    has_bake: bool,
    bake_key: [u8; 16],
    res: u32,
    radius_m: f64,
    /// Where the bake is read from, for the log (`protocol.source`).
    source: String,
    /// The tile pack of a stored bake, where the pre-filled coarse tiles are (STRM-021); none for an in-memory bake.
    pack: Option<&'static planet_cache::pack::TileStore>,
    /// Whether tiles are read from the pack at all (`false`: the cold reader of the `dive-cold` scene).
    use_prefilled: bool,
    prefill_hits: std::sync::atomic::AtomicU64,
    prefill_lookups: std::sync::atomic::AtomicU64,
    prefill_skipped: std::sync::atomic::AtomicU64,
    prefill_ns: std::sync::atomic::AtomicU64,
    job_block_wait_ns: std::sync::atomic::AtomicU64,
    job_block_decodes: std::sync::atomic::AtomicU64,
    field_builds: std::sync::atomic::AtomicU64,
    field_ns: std::sync::atomic::AtomicU64,
    /// The stored bake behind `src`, for the counters (`None`: an in-memory bake or no bake).
    store_source: Option<&'static StoreSource<'static>>,
    /// The ocean channel of the bake (ADR 0041 point 2): the sea of the water field W. `None` for a field that reads no bake (no
    /// bake, no ocean, no water) and for a store without the channel.
    ocean: Option<Box<dyn OceanLattice + Send + Sync>>,
    /// The wet floor of the bake (ADR 0071): with the ocean channel, the coastal water of the water field. `None` exactly when `ocean` is.
    floor: Option<Box<dyn planet_generators::wet_floor::FloorLattice + Send + Sync>>,
    /// The coarse level of W (ADR 0058): the summary of the stored bake, which answers the tiles that span more than one block.
    /// Made from the bake's own fields for an in-memory bake; `None` for a world without water.
    summary: Option<&'static planet_generators::water_summary::WaterSummary>,
    /// Whether the fields this world builds bound their tiles up to the sea level (the viewer's lit path: the water of a tile whose
    /// terrain the view culls must not be culled with it, `WaterBounded`). Off for every other run: plans and keys are unchanged.
    bound_water: bool,
    /// The water of the tiles the jobs loaded (ADR 0056), filled when `bound_water` (the lit path) is on.
    water_book: Arc<planet_frame::water::WaterBook>,
    /// The global coupled run (M8-1 part c): asked for from a thread, read when drawn through the lens.
    coupled: crate::coupledworld::CoupledSlot,
    /// The stored coarse horizon (M8-5, STRM-045), loaded on a thread beside the store: asked for in `horizonworld`.
    pub(crate) coarse: crate::horizonworld::CoarseSlot,
    /// The lens the fields are built with: 0 off, 1 height, 2 drainage area (the climate views show the run, GEN-043).
    coupled_lens: std::sync::atomic::AtomicU8,
    /// The directory of the stored bake the coupled channel lives beside; none for an in-memory bake.
    store_dir: Option<std::path::PathBuf>,
    /// The world of the stored bake (its field source, STRM-073), kept for the keys of the erosion layer; none for an in-memory bake.
    store_world: Option<&'static GlobalWorld>,
    /// The erosion layer of a bake drawn over the stored world (VIEW-062, GEN-112), when the run asked for it ([`WorldTerrain::with_erosion`]).
    erosion: Option<planet_generators::height_fields::ErosionSwitch>,
    /// What the layer is, for the log and the title.
    erosion_note: String,
    /// The river source of the new chain's commit when it holds a channel graph (STRM-086): the frame's river plan reads the channels through it.
    rivers: Option<Arc<crate::rivers::ReaderRiverSource>>,
    /// What the base is, for a world opened by `open_fixture` (empty otherwise).
    fixture_note: String,
    /// Scoped threads of one horizon bake (0: the cores, at most 4).
    horizon_threads: std::sync::atomic::AtomicUsize,
    /// Cells per chunk edge of the horizon lattice (0: the default of [`crate::horizonworld::HORIZON_CELL_M`]); a test sets a coarse one to stay quick.
    pub(crate) horizon_cells: std::sync::atomic::AtomicU32,
    /// The horizon jobs of the chunks (M8-5, STRM-049): none until `enable_horizon` (in `horizonworld`).
    pub(crate) horizon: std::sync::RwLock<Option<Arc<planet_streaming::horizonjobs::HorizonJobs>>>,
    /// The queue of the height warm-up thread (made on the first [`WorldTerrain::warm_heights`]) and the batches it has read.
    warm: std::sync::Mutex<Option<std::sync::mpsc::SyncSender<Vec<planet_core::Vec3>>>>,
    warmed: std::sync::atomic::AtomicU64,
    /// The helper threads of a tile job's water reads (BUDG-004): the rig sets the terrain share's; 8 plain scoped threads without one.
    job_par: std::sync::RwLock<Arc<dyn planet_core::par::Par + Send + Sync>>,
}

/// The ocean channel of a stored bake as the water field's lattice.
struct SharedOcean(&'static planet_cache::global_store::StoreOceanSource<'static>);

impl OceanLattice for SharedOcean {
    fn res(&self) -> u32 {
        self.0.res()
    }

    fn is_ocean(&self, face: Face, i: u32, j: u32) -> Result<bool, ReadError> {
        self.0.is_ocean(face, i, j)
    }
}

/// The wet floor of a stored bake as the water field's lattice.
struct SharedFloor(&'static planet_cache::global_store::StoreWetFloorSource<'static>);

impl planet_generators::wet_floor::FloorLattice for SharedFloor {
    fn res(&self) -> u32 {
        self.0.res()
    }

    fn floor(&self, face: Face, i: u32, j: u32) -> Result<u16, ReadError> {
        self.0.floor(face, i, j)
    }
}

impl WorldTerrain {
    /// Resolve the height field of `def` (refusing an unknown id) and bake what it reads at `global_res` cells per face edge.
    pub fn open(def: WorldDefinition, global_res: u32) -> Result<WorldTerrain, String> {
        let kind = resolve(&def).map_err(|e| e.to_string())?;
        let radius_m =
            GenContext::new(&def).body_radius_m(BodyId::PLANET).map_err(|e| format!("the definition cannot be generated: {e}"))?;
        let mut ocean: Option<Box<dyn OceanLattice + Send + Sync>> = None;
        let mut floor: Option<Box<dyn planet_generators::wet_floor::FloorLattice + Send + Sync>> = None;
        let mut summary_owned: Option<&'static planet_generators::water_summary::WaterSummary> = None;
        let (src, bake_key): (Box<dyn LatticeSource + Send>, [u8; 16]) = if kind.needs_bake() {
            if global_res > MAX_RUN_BAKE_RES {
                return Err(format!("--global-res {global_res}: a run bakes in memory, at most {MAX_RUN_BAKE_RES} cells per face edge"));
            }
            let field = GlobalCoarseGenerator
                .field(&GenContext::new(&def), BodyId::PLANET)
                .map_err(|e| format!("the definition cannot be generated: {e}"))?;
            let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
            let grid = field.bake(&TangentWarp, global_res, threads).map_err(|e| format!("--global-res {global_res}: {e:?}"))?;
            let world = GlobalWorld::from_definition(&def).map_err(|e| format!("the definition cannot be generated: {e}"))?;
            let sea = def.sea(BodyId::PLANET).map_err(|e| format!("the sea of the definition: {e}"))?;
            let block = global_res.min(256);
            let (_, grid_ocean) = tiled_ocean(&grid, &TangentWarp, block, threads, sea.min_area_fraction)
                .map_err(|e| format!("the ocean channel of the in-memory bake: {e:?}"))?;
            // The coarse level of W from the same fields, in the store's block layout (blocks of at most 1024 cells), so a run over an
            // in-memory bake selects water by the rule of the stored one (ADR 0058).
            // The floors are made once: the producer's point query and the summary read the same grid.
            let floors = planet_generators::wet_floor::tiled_floors(&grid, &grid_ocean, block, threads)
                .map_err(|e| format!("the wet floor of the in-memory bake: {e:?}"))?;
            let summary = planet_generators::water_summary::summary_of_floors(&grid, &floors, global_res.min(1024), threads)
                .map_err(|e| format!("the water summary of the in-memory bake: {e}"))?;
            summary_owned = Some(Box::leak(Box::new(summary)) as &'static planet_generators::water_summary::WaterSummary);
            floor = Some(Box::new(floors));
            ocean = Some(Box::new(OwnedOcean(grid_ocean)));
            (Box::new(OwnedBake(grid)), world.stage_key(global_res).0)
        } else {
            (Box::new(NoBake), [0; 16])
        };
        let source = if kind.needs_bake() { format!("in-memory {global_res} cells") } else { "no bake (the field reads none)".to_string() };
        Ok(WorldTerrain {
            def: std::sync::RwLock::new(Arc::new(def)),
            kind,
            src,
            has_bake: kind.needs_bake(),
            bake_key,
            res: global_res,
            radius_m,
            source,
            pack: None,
            use_prefilled: true,
            prefill_hits: Default::default(),
            prefill_lookups: Default::default(),
            prefill_skipped: Default::default(),
            prefill_ns: Default::default(),
            job_block_wait_ns: Default::default(),
            job_block_decodes: Default::default(),
            field_builds: Default::default(),
            field_ns: Default::default(),
            store_source: None,
            ocean,
            floor,
            summary: summary_owned,
            bound_water: false,
            water_book: Default::default(),
            coupled: Default::default(),
            warm: Default::default(),
            warmed: Default::default(),
            job_par: std::sync::RwLock::new(Arc::new(8usize)),
            coarse: Default::default(),
            horizon_threads: std::sync::atomic::AtomicUsize::new(0),
            horizon_cells: std::sync::atomic::AtomicU32::new(0),
            horizon: Default::default(),
            coupled_lens: Default::default(),
            store_dir: None,
            store_world: None,
            erosion: None,
            erosion_note: String::new(),
            rivers: None,
            fixture_note: String::new(),
        })
    }

    /// The terrain over a stored bake (the production source: `generate --global-dir`, GEN-014) of `res` cells per face edge in
    /// `dir`. The store and its world are leaked: the field borrows them for the life of the process, a CLI run. Refuses a field
    /// that reads no bake, a directory that is not a store, and a store that does not hold the bake of this definition at `res`.
    pub fn open_store(def: WorldDefinition, dir: &Path, res: u32) -> Result<WorldTerrain, String> {
        Self::open_store_as(def, dir, res, None)
    }

    /// [`WorldTerrain::open_store`] with the field source to read when the store holds both bakes (STRM-073).
    pub fn open_store_as(
        def: WorldDefinition,
        dir: &Path,
        res: u32,
        choice: Option<planet_cache::global_store::FieldSource>,
    ) -> Result<WorldTerrain, String> {
        let kind = resolve(&def).map_err(|e| e.to_string())?;
        if !kind.needs_bake() {
            return Err(format!("height field {} reads no bake: --store has nothing to read", kind.id()));
        }
        let radius_m =
            GenContext::new(&def).body_radius_m(BodyId::PLANET).map_err(|e| format!("the definition cannot be generated: {e}"))?;
        if !dir.is_dir() {
            return Err(format!("{}: no such store directory (a bake made by `generate --global-dir`)", dir.display()));
        }
        // The store records its field source (STRM-073): a class-source store opens as a class world, a legacy store as the legacy one, a store of both is refused.
        let threads = planet_streaming::topology::bake_threads(0).map_or(4, |t| t.0);
        let world = GlobalWorld::for_store(&def, dir, res, choice, threads)?;
        let world: &'static GlobalWorld = Box::leak(Box::new(world));
        let store = planet_cache::pack::TileStore::open(dir, world.params()).map_err(|e| format!("{}: {e}", dir.display()))?;
        let store: &'static planet_cache::pack::TileStore = Box::leak(Box::new(store));
        let src: &'static StoreSource<'static> =
            Box::leak(Box::new(StoreSource::new(store, world, res).map_err(|e| format!("{}: a bake of {res} cells: {e}", dir.display()))?));
        // A store of another definition or resolution reads as holes: say so now, not as NaN heights in the middle of a run.
        src.at(Face(0), 0, 0).map_err(|e| {
            format!("{}: the store holds no bake of {res} cells for this definition (--seed, --world, --global-res): {e}", dir.display())
        })?;
        let bake_key = world.stage_key(res).0;
        // The ocean channel of the store (GLOBAL_VERSION 4): without it the world has no water, and the run says so.
        let ocean: Option<Box<dyn OceanLattice + Send + Sync>> = planet_cache::global_store::StoreOceanSource::new(store, world, res)
            .ok()
            .map(|o| Box::new(SharedOcean(Box::leak(Box::new(o)))) as Box<dyn OceanLattice + Send + Sync>);
        // The wet floor (ADR 0071): a store with the ocean channel and without it is refused, naming the command that adds it.
        let floor: Option<Box<dyn planet_generators::wet_floor::FloorLattice + Send + Sync>> = match &ocean {
            Some(_) => {
                let f = planet_cache::global_store::StoreWetFloorSource::new(store, world, res)
                    .map_err(|e| format!("{}: a bake of {res} cells: {e}", dir.display()))?;
                // Every block is read when the water asks; one now names a store without the channel at once.
                planet_generators::wet_floor::FloorLattice::floor(&f, Face(0), 0, 0).map_err(|e| format!("{}: {e}", dir.display()))?;
                Some(Box::new(SharedFloor(Box::leak(Box::new(f)))))
            }
            None => None,
        };
        // The coarse level of W (ADR 0058): a store with the ocean channel and without the summary is refused, because every coarse tile
        // would then read up to 25 blocks and the dive would lag; the message names the command that adds it.
        let summary = match &ocean {
            Some(_) => Some(&*Box::leak(Box::new(
                planet_cache::global_store::load_water_summary(store, world, res).map_err(|e| format!("{}: {e}", dir.display()))?,
            ))),
            None => None,
        };
        let source = format!("store {}, {res} cells", dir.display());
        Ok(WorldTerrain {
            def: std::sync::RwLock::new(Arc::new(def)),
            kind,
            src: Box::new(SharedStore(src)),
            has_bake: true,
            bake_key,
            res,
            radius_m,
            source,
            pack: Some(store),
            use_prefilled: true,
            prefill_hits: Default::default(),
            prefill_lookups: Default::default(),
            prefill_skipped: Default::default(),
            prefill_ns: Default::default(),
            job_block_wait_ns: Default::default(),
            job_block_decodes: Default::default(),
            field_builds: Default::default(),
            field_ns: Default::default(),
            store_source: Some(src),
            ocean,
            floor,
            summary,
            bound_water: false,
            water_book: Default::default(),
            coupled: Default::default(),
            warm: Default::default(),
            warmed: Default::default(),
            job_par: std::sync::RwLock::new(Arc::new(8usize)),
            coarse: Default::default(),
            horizon_threads: std::sync::atomic::AtomicUsize::new(0),
            horizon_cells: std::sync::atomic::AtomicU32::new(0),
            horizon: Default::default(),
            coupled_lens: Default::default(),
            store_dir: Some(dir.to_path_buf()),
            store_world: Some(world),
            erosion: None,
            erosion_note: String::new(),
            rivers: None,
            fixture_note: String::new(),
        })
    }

    /// The terrain of a run that views, streams or records: the world file's, or the seed-only world with the reference field
    /// (every golden, plan snapshot and old repro bundle was made with it).
    pub fn of_options(o: &Options, radius_m: f64) -> Result<WorldTerrain, String> {
        let def = match &o.world {
            Some(path) => load_world(path)?,
            None => legacy_world(o.seed, radius_m),
        };
        WorldTerrain::open(def, o.global_res)
    }

    /// The terrain of the perf scene: the world file's, or the default world of `--seed` (its field is the eroded one,
    /// GEN-018) over an in-memory bake of `--global-res` cells per face edge. The perf scene measures the world a user gets.
    pub fn of_perf_options(o: &Options, radius_m: f64) -> Result<WorldTerrain, String> {
        let def = match &o.world {
            Some(path) => load_world(path)?,
            None => WorldDefinition::single_planet(radius_m, o.seed),
        };
        WorldTerrain::open(def, o.global_res)
    }

    /// The same terrain with the pack's pre-filled coarse tiles ignored: every tile is generated on demand, as from an empty tile
    /// cache (the `dive-cold` scene, ADR 0045 point 3). The bake is read as before.
    pub fn cold(mut self) -> WorldTerrain {
        self.use_prefilled = false;
        self
    }

    /// Whether this terrain reads pre-filled tiles from the pack.
    pub fn reads_prefilled(&self) -> bool {
        self.use_prefilled && self.pack.is_some()
    }

    /// The pre-filled tile from the pack, if this terrain reads them and the pack holds it (STRM-021); `None` is "generate it".
    pub fn prefilled_tile(
        &self,
        field: &dyn HeightField,
        tile: planet_core::TileId,
        cells: u32,
        max_level: u8,
    ) -> Option<planet_generators::compact::CompactTile> {
        use std::sync::atomic::Ordering::Relaxed;
        let pack = self.pack.filter(|_| self.use_prefilled)?;
        let spec = TileSpec::new(field, self.radius_m, cells, max_level);
        if !spec.storable(tile) {
            // Too deep to be stored: no key, no pack read (the counter says how often a lookup was not needed).
            self.prefill_skipped.fetch_add(1, Relaxed);
            return None;
        }
        let started = std::time::Instant::now();
        let found = spec.lookup(pack, tile);
        self.prefill_ns.fetch_add(u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX), Relaxed);
        self.prefill_lookups.fetch_add(1, Relaxed);
        if found.is_some() {
            self.prefill_hits.fetch_add(1, Relaxed);
        }
        found
    }

    /// Run one tile job's load and record how much of it the calling thread spent getting blocks (recorded only, STRM-022).
    pub fn timed_load<R>(&self, load: impl FnOnce() -> R) -> R {
        use planet_cache::global_store::{thread_block_decodes, thread_block_wait_ns};
        let (waited, decoded) = (thread_block_wait_ns(), thread_block_decodes());
        let out = load();
        self.job_block_wait_ns.fetch_add(thread_block_wait_ns().saturating_sub(waited), std::sync::atomic::Ordering::Relaxed);
        self.job_block_decodes.fetch_add(thread_block_decodes().saturating_sub(decoded), std::sync::atomic::Ordering::Relaxed);
        out
    }

    /// What the tile source did so far (recorded only, STRM-022); the stored-bake and pack figures are zero for an in-memory run.
    pub fn counters(&self) -> SourceCounters {
        use std::sync::atomic::Ordering::Relaxed;
        SourceCounters {
            blocks: self.store_source.map(StoreSource::stats).unwrap_or_default(),
            pack: self.pack.map(planet_cache::pack::TileStore::read_stats).unwrap_or_default(),
            prefill_lookups: self.prefill_lookups.load(Relaxed),
            prefill_found: self.prefill_hits.load(Relaxed),
            prefill_ns: self.prefill_ns.load(Relaxed),
            prefill_skipped: self.prefill_skipped.load(Relaxed),
            job_block_wait_ns: self.job_block_wait_ns.load(Relaxed),
            job_block_decodes: self.job_block_decodes.load(Relaxed),
            field_builds: self.field_builds.load(Relaxed),
            field_ns: self.field_ns.load(Relaxed),
        }
    }

    /// Tiles served from the pack so far.
    pub fn prefill_hits(&self) -> u64 {
        self.prefill_hits.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// How many of the tiles of levels `floor..=levels` (`floor`: the render floor of the run, LOD-011) the pack holds for a run with
    /// `cells` and `max_level`, and how many there are: `(present, total)`. Reads every present tile (the pack's CRC and
    /// the codec decide), whatever `cold` says.
    pub fn prefill_present(&self, floor: u8, levels: u8, cells: u32, max_level: u8) -> Result<(usize, usize), String> {
        let pack = self.pack.ok_or("this terrain has no tile pack (an in-memory bake)")?;
        let field = self.field()?;
        let spec = TileSpec::new(field.as_ref(), self.radius_m, cells, max_level).with_floor(floor);
        let total = planet_cache::meshpack::prefill_tile_count_from(floor, levels);
        let missing = planet_cache::meshpack::missing_tiles(pack, &spec, levels).len();
        Ok((total - missing, total))
    }

    /// Read the ocean channel at a fixed grid of directions (12 latitudes by 24 longitudes, the poles excluded): `(directions read,
    /// of them sea)`, or `None` when the world has no water. A read error is returned, never counted as land.
    pub fn ocean_probe(&self) -> Result<Option<(usize, usize)>, String> {
        let r = self.with_water(|field| {
            let (mut read, mut sea) = (0usize, 0usize);
            for i in 0..12 {
                let lat = (-82.5 + 15.0 * f64::from(i)).to_radians();
                for j in 0..24 {
                    let lon = (7.5 + 15.0 * f64::from(j)).to_radians();
                    let d = planet_core::Vec3::from_lat_lon(lat, lon);
                    let w = planet_frame::water::WaterSource::water_at(field, d)
                        .map_err(|e| format!("ocean channel read at lat {:.1} lon {:.1}: {e}", lat.to_degrees(), lon.to_degrees()))?;
                    read += 1;
                    sea += usize::from(w.is_some());
                }
            }
            Ok::<_, String>((read, sea))
        })?;
        r.transpose()
    }

    /// The read-only check of a stored production world before a window (TEST-024): the bake answers, the ocean channel reads, and the
    /// tile pack holds levels `floor..=levels` (`floor`: the render floor, LOD-011) in the compact form (the only form;
    /// the vertex form went with the vertex format). Writes nothing: the store is opened for reading only. Every present pack tile is
    /// read and decoded by `prefill_present`.
    pub fn store_check(&self, floor: u8, levels: u8, cells: u32, max_level: u8) -> Result<StoreCheck, String> {
        if self.pack.is_none() {
            return Err("this terrain has no tile pack (an in-memory bake): there is no store to check".into());
        }
        let ocean = self.ocean_probe()?;
        let compact = self.prefill_present(floor, levels, cells, max_level)?;
        Ok(StoreCheck { source: self.source.clone(), res: self.res, floor, levels, ocean, compact })
    }

    /// Bake blocks `(done, total)` for the readiness report (ADR 0045 point 5). A run opens a complete bake (an in-memory one is
    /// built, a stored one is checked), so the two are equal; a field that reads no bake has none. The incremental first-visit
    /// bake of ADR 0045 point 5 reports its progress here when it exists.
    pub fn bake_blocks(&self) -> (u64, u64) {
        if !self.has_bake {
            return (0, 0);
        }
        let per_edge = u64::from(self.res.div_ceil(self.res.min(1024)));
        let total = 6 * per_edge * per_edge;
        (total, total)
    }

    /// The current definition (a snapshot: a later [`WorldTerrain::set_definition`] does not change it).
    pub fn definition(&self) -> Arc<WorldDefinition> {
        Arc::clone(&crate::stall::read(&self.def, "definition lock (read)"))
    }

    /// Replace the definition the fields are built from, for an edit of its items (the editor's definition after a command). The base
    /// must be the one the bake was made from (`base_hash`), else the call is refused: a change of the nodes needs another terrain. Fields
    /// built before the call keep the definition they were built from; the next `field()` reads the new one. After this call, in the
    /// caller: `ManagedLoop::edit(tiles)`, so the replan sees the new definition.
    pub fn set_definition(&self, def: Arc<WorldDefinition>) -> Result<(), String> {
        let mut current = crate::stall::write(&self.def, "definition lock (write)");
        if def.base_hash() != current.base_hash() {
            return Err(format!(
                "the new definition has another base (hash {:016x}, the bake was made from {:016x}): only the items may change",
                def.base_hash(),
                current.base_hash()
            ));
        }
        *current = def;
        Ok(())
    }

    /// Cells per face edge of the bake the field reads (the requested size even for a field that reads none).
    pub fn global_res(&self) -> u32 {
        self.res
    }

    /// The implementation id of the field.
    pub fn id(&self) -> &'static str {
        match &self.erosion {
            Some(e) => e.implementation_id(self.kind),
            None => self.kind.id(),
        }
    }

    pub fn radius_m(&self) -> f64 {
        self.radius_m
    }

    /// Where the bake was read from: `store DIR, N cells` or `in-memory N cells`.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// A line for the run's output: which field, over which bake.
    pub fn describe(&self) -> String {
        let mut s = if self.has_bake {
            // The perf logs record this text as the measured field (TEST-012): a legacy world keeps the text it had, a class-source world says so.
            match self.store_world {
                Some(w) if w.class_source().is_some() => format!("height field {} over a {}-cell bake (class source)", self.id(), self.res),
                _ => format!("height field {} over a {}-cell bake", self.id(), self.res),
            }
        } else {
            format!("height field {}", self.id())
        };
        if !self.erosion_note.is_empty() {
            s += &format!(", {}", self.erosion_note);
        }
        s
    }

    /// [`WorldTerrain::with_erosion`] when the run asked for `--erosion on`, the world unchanged otherwise.
    pub fn with_erosion_if(self, o: &Options) -> Result<WorldTerrain, String> {
        if !o.erosion {
            return Ok(self);
        }
        self.with_erosion(&ErosionOptions {
            layer_dir: o.erosion_store.clone(),
            face_res: o.erosion_face_res,
            planet_res: o.erosion_res,
            unchecked: o.erosion_unchecked,
            channels: o.channels,
        })
    }

    /// The field source of the bake this world reads, for the log and the title (STRM-073, VIEW-065): `class source`, or `legacy source` (the legacy generator's, and the in-memory preview bake's).
    pub fn field_source_name(&self) -> &'static str {
        match self.store_world {
            Some(w) if w.class_source().is_some() => "class source",
            Some(_) => "legacy source",
            None => "legacy source",
        }
    }

    /// What the erosion layer is when it is on (`--erosion on`): empty otherwise.
    pub fn erosion_note(&self) -> &str {
        &self.erosion_note
    }

    /// The river source of the commit the layer is read under, when the commit holds a channel graph (STRM-086).
    pub fn rivers(&self) -> Option<&Arc<crate::rivers::ReaderRiverSource>> {
        self.rivers.as_ref()
    }

    /// Draws the commit of the new chain in `commit_dir` over this world: L with no fill, and the rivers of its channel graph when it holds one (STRM-086). `world` is the world the commit's keys are made under.
    fn use_commit(mut self, commit_dir: &Path, world: Arc<GlobalWorld>, def: &WorldDefinition, seed: u64) -> Result<WorldTerrain, String> {
        let layer = planet_cache::l_layer::CommitLayer::open(commit_dir, world.clone(), LAYER_CACHE_TILES)
            .map_err(|e| format!("--erosion: {e}"))?;
        let commit_id = layer.commit().id;
        // A commit with a channel graph gives the frame its rivers; one without has none (the reader refuses it, and the viewer draws no river).
        self.rivers = layer
            .rivers(planet_cache::river_read::DEFAULT_CACHE_BYTES)
            .ok()
            .map(|r| Arc::new(crate::rivers::ReaderRiverSource::new(Arc::new(r))));
        let layer = planet_generators::erosion_layer::ErosionLayer::new(Arc::new(layer)).map_err(|e| format!("--erosion: {e}"))?;
        let sea_level_m = def.sea(BodyId::PLANET).map_err(|e| format!("the sea of the definition: {e}"))?.level_m;
        self.kind = HeightFieldKind::ErodedC1;
        self.erosion_note = format!(
            "erosion layer {} (commit {commit_id:016x} of the new chain, L with no fill, {}, seed {seed}, coast fade {} m over the sea level {sea_level_m} m, crease band on, relief channels off, C1 base)",
            commit_dir.display(),
            if self.rivers.is_some() { "rivers from its channel graph" } else { "no channel graph" },
            planet_generators::erosion_layer::COAST_FADE_M
        );
        self.source = format!("{} + {}", self.source, self.erosion_note);
        self.erosion =
            Some(planet_generators::height_fields::ErosionSwitch { relief: None, layer: Arc::new(layer), sea_level_m, channels: None });
        Ok(self)
    }

    /// TEST-ONLY (the G-RT fixture, architect 2026-11-12): the terrain of a commit baked by `bake_region` over an in-memory base that needs no store. The base is the 1,024-cell in-memory bake everywhere (ocean, water, far
    /// ground) with the exact 16384-node lattice (`BakeInputs::base`) over the rectangle of the commit's tiles (their halo and one tile of margin), sampled here once, before any frame; the commit is opened under a
    /// `GlobalWorld` of the definition (no store). Reached only by the `river-flight` perf scene; no viewer flag opens it. The seam between the exact rectangle and the coarse ground, and the edge of the commit's tiles, lie
    /// one tile (the margin) from the commit's tiles: a flight stays at least one LOD range inside them. [`WorldTerrain::fixture_note`] says what the base is.
    pub fn open_fixture(def: WorldDefinition, commit_dir: &Path, threads: usize) -> Result<WorldTerrain, String> {
        let started = std::time::Instant::now();
        let commit = planet_cache::l_writer::Commit::read(commit_dir)?
            .ok_or_else(|| format!("{} holds no commit of the new chain", commit_dir.display()))?;
        let spec = commit.spec;
        let seed = GenContext::new(&def).body_seed(BodyId::PLANET).map_err(|e| format!("the definition has no body seed: {e}"))?;
        if spec.seed != seed {
            return Err(format!("the commit was baked for seed {}, the definition's is {seed}", spec.seed));
        }
        let (face, rect) = fixture_rectangle(&spec, commit.tiles.keys().copied())?;
        let res = spec.res;
        let (i1, j1) = (rect.0 as usize + rect.2 - 1, rect.1 as usize + rect.3 - 1);
        let mut world = WorldTerrain::open(def.clone(), MAX_RUN_BAKE_RES)?;
        let coarse = std::mem::replace(&mut world.src, Box::new(NoBake));
        let inputs = planet_cache::global_store::class_inputs(&def, seed, threads);
        let direction = |i: u32, j: u32| planet_cache::region_bake::lattice_direction(face, res as usize, i as usize, j as usize);
        let fixture = crate::fixturebase::FixtureLattice::sample(res, face, rect, &direction, &|d| inputs.base(d), coarse, threads)?;
        world.src = Box::new(fixture);
        world.kind = HeightFieldKind::ErodedC1;
        let stored = Arc::new(GlobalWorld::from_definition(&def).map_err(|e| format!("the definition cannot be generated: {e}"))?);
        let mut world = world.use_commit(commit_dir, stored, &def, seed)?;
        world.fixture_note = format!(
            "base: IN-MEMORY FIXTURE (not the world bake, no store): the {MAX_RUN_BAKE_RES}-cell in-memory bake elsewhere and the exact {res}-node lattice over {} x {} nodes (face {}, nodes {}..{} x {}..{}) of the commit's tiles with one tile of margin, built in {:.1} s on {threads} threads before the first frame; G-RT on it is PROVISIONAL (no base store reads); the flight stays inside the tiles",
            rect.2,
            rect.3,
            face.0,
            rect.0,
            i1,
            rect.1,
            j1,
            started.elapsed().as_secs_f64()
        );
        Ok(world)
    }

    /// What the base of a world opened by [`WorldTerrain::open_fixture`] is (empty for every other world).
    pub fn fixture_note(&self) -> &str {
        &self.fixture_note
    }

    /// Turns the erosion layer of a bake on over this stored world (VIEW-062, GEN-112): the layer is read from `o.layer_dir` (the bake's output folder or its `tiles/`, else the world's
    /// store) under the keys of the bake shape `o` names and the definition's seed, added to the C1 read of the stored lattice (the default eroded field is upgraded to the C1 one) with
    /// the crease band, the relief channels not read. An error naming the problem when the world has no stored bake, its field is not an eroded one, its relief scale is not 1 or its
    /// sea is not at 0 m (what the layer was carved on), the store does not say it was baked from the class source (unless `o.unchecked`), or the layer holds no tile under these keys
    /// (the existing refusal names both keys).
    pub fn with_erosion(mut self, o: &ErosionOptions) -> Result<WorldTerrain, String> {
        let Some(store_dir) = self.store_dir.clone() else {
            return Err("--erosion needs --store: the layer is drawn over a stored bake".into());
        };
        if !matches!(self.kind, HeightFieldKind::Eroded | HeightFieldKind::ErodedC1) {
            return Err(format!("--erosion applies to the eroded fields, the world's is {}", self.kind.id()));
        }
        // The layer was carved on the class source's surface at relief scale 1 and sea level 0: a definition that differs would draw it on another ground without a word.
        let def = self.definition();
        let ctx = GenContext::new(&def);
        let relief = ctx.relief_scale(BodyId::PLANET).map_err(|e| format!("the relief scale of the definition: {e}"))?;
        if relief != 1.0 {
            return Err(format!("--erosion: the layer was carved at relief scale 1, the definition's is {relief}"));
        }
        let sea = def.sea(BodyId::PLANET).map_err(|e| format!("the sea of the definition: {e}"))?.level_m;
        if sea != 0.0 {
            return Err(format!("--erosion: the layer was carved over a sea at 0 m, the definition's is {sea} m"));
        }
        // The world of the store records its field source (STRM-073): the layer was carved on the class source's surface, today's legacy stores hold another one (2 km apart on the mountains tile).
        let Some(stored) = self.store_world else {
            return Err("--erosion needs --store: the layer is drawn over a stored bake".into());
        };
        if stored.class_source().is_none() && !o.unchecked {
            return Err(format!(
                "{}: the store holds the legacy generator's bake, and the erosion layer was carved on the class source's surface (about 2 km apart on the mountains tile): bake the field from the class source (`generate --global-dir --class-source`). \
                 --erosion-unchecked draws the layer anyway (a test of the plumbing, not a look)",
                store_dir.display()
            ));
        }
        let def = self.definition();
        let seed = GenContext::new(&def).body_seed(BodyId::PLANET).map_err(|e| format!("the definition has no body seed: {e}"))?;
        let world = Arc::new(stored.clone());
        let defaults = planet_cache::layer_tiles::BakeShape::production(seed, self.radius_m);
        let over = planet_cache::layer_tiles::ShapeOverride { face_res: o.face_res, planet_res: o.planet_res };
        let dir = o.layer_dir.clone().unwrap_or(store_dir);
        // The layer of the new chain (item 15 PR 5a): a folder that holds a commit record is read through it, as L with no fill; the old layer's reader is used otherwise.
        if let Some(commit_dir) =
            [dir.clone(), dir.join("tiles")].into_iter().find(|d| d.join(planet_cache::l_writer::COMMIT_FILE).is_file())
        {
            if o.channels {
                return Err(format!(
                    "--channels on: {} holds a commit of the new chain, whose drainage and depth channels are not stored (the channel graph is a later item): turn --channels off",
                    commit_dir.display()
                ));
            }
            return self.use_commit(&commit_dir, world.clone(), &def, seed);
        }
        let (layer, manifest, shape) =
            planet_cache::layer_tiles::StoreLayer::open_recorded(&dir, world.clone(), &defaults, over, LAYER_CACHE_TILES)
                .map_err(|e| format!("--erosion: {e}"))?;
        let (face_res, planet_res) = (shape.face_res, shape.planet_res);
        let replaced = o.face_res.is_some() || o.planet_res.is_some();
        // The layer's decoded tiles are shared with the channel reader: the change with the pit depth added back, and the pit codes, are the heights' own tile, not a second copy.
        let layer = Arc::new(layer);
        // The baked channels beside the layer (VIEW-066): read under the keys of the same shape (the areas at pass 1's cells a face edge, the depths at pass 2's), refused when the store holds none.
        let channels: Option<Arc<dyn planet_generators::channels::ChannelSource>> = if o.channels {
            let tiles_dir = if dir.join("tiles").is_dir() { dir.join("tiles") } else { dir.clone() };
            let (area_spec, depth_spec) = planet_cache::layer_tiles::bake_specs(&world, &shape).map_err(|e| format!("--channels: {e}"))?;
            let store = Arc::new(
                planet_cache::pack::TileStore::open(&tiles_dir, world.params())
                    .map_err(|e| format!("--channels: {}: {e}", tiles_dir.display()))?,
            );
            let reader = planet_cache::layer_channels::LayerChannels::new(
                store,
                world.clone(),
                shape.face_res as u32,
                shape.planet_res as u32,
                area_spec,
                depth_spec,
                layer.clone(),
                planet_cache::layer_channels::ChannelCaches::DEFAULT,
            );
            let faces = reader.faces_with_area().map_err(|e| format!("--channels: {e}"))?;
            if faces.is_empty() {
                return Err(format!(
                    "--channels on: {} holds no drainage area (record kind 14) under the keys of this layer: bake the layer with a build that stores the channels, or turn --channels off",
                    tiles_dir.display()
                ));
            }
            Some(Arc::new(reader))
        } else {
            None
        };
        let layer = planet_generators::erosion_layer::ErosionLayer::new(layer).map_err(|e| format!("--erosion: {e}"))?;
        let sea_level_m = def.sea(BodyId::PLANET).map_err(|e| format!("the sea of the definition: {e}"))?.level_m;
        // The layer's ground is the lattice read C1 (a bilinear read leaves a crease along every lattice cell under it): the default field is upgraded.
        self.kind = HeightFieldKind::ErodedC1;
        self.erosion_note = format!(
            "erosion layer {} ({}; pass 1 at {face_res} cells, pass 2 at {planet_res}, seed {seed}, coast fade {} m over the sea level {sea_level_m} m, crease band on, relief channels off, C1 base)",
            dir.display(),
            match (manifest.is_some(), replaced) {
                (true, false) => "its manifest",
                (true, true) => "its manifest, the cells replaced by the options",
                (false, _) => "no manifest: the options and the production shape",
            },
            planet_generators::erosion_layer::COAST_FADE_M
        );
        if channels.is_some() {
            self.erosion_note.push_str("; the driver block from the layer's channels (drainage area, deposited depth)");
        }
        self.source = format!("{} + {}", self.source, self.erosion_note);
        self.erosion =
            Some(planet_generators::height_fields::ErosionSwitch { relief: None, layer: Arc::new(layer), sea_level_m, channels });
        Ok(self)
    }

    /// This world with the fields it builds bounded up to the sea level (see the field `bound_water`); a world without water is
    /// unchanged.
    pub fn bounding_water(mut self) -> WorldTerrain {
        self.bound_water = self.ocean.is_some();
        self
    }

    /// The water of tiles computed off the frame (ADR 0056).
    pub fn water_book(&self) -> Arc<planet_frame::water::WaterBook> {
        Arc::clone(&self.water_book)
    }

    /// Whether a tile's job computes the tile's water (the lit path of a world with water).
    pub fn fills_water(&self) -> bool {
        self.bound_water
    }

    /// Compute the water of `tile` into the book, if this world has water and the lit path is on; called by the tile's job, off the
    /// frame (ADR 0056). A failed read leaves the tile to the plan, which reports it.
    ///
    /// `built` is the mesh the job made for `tile`: its lowest own vertex is kept beside the answer for the exact dry-tile cut
    /// (ATMO-020), computed here in the job and never on the plan thread.
    pub fn fill_water(&self, tile: TileId, built: &planet_generators::compact::CompactTile) {
        if !self.bound_water {
            return;
        }
        let radius_m = self.radius_m;
        let book = &self.water_book;
        let par = Arc::clone(&self.job_par.read().expect("job par"));
        if matches!(self.with_water(|f| book.fill_par(f, &TangentWarp, tile, radius_m, par.as_ref())), Ok(Some(Ok(())))) {
            book.set_min_height(tile, built.min_height_m());
        }
    }

    /// The bake's elevation at `dir` by the C1 interpolant ([`planet_generators::global::read_elevation_c1`], GEN-107): the base of `cpu-erosion-v2`, for diagnostics and the
    /// pyramid evidence. `None` on a read error.
    pub fn base_elevation_c1(&self, dir: planet_core::Vec3) -> Option<f64> {
        planet_generators::global::read_elevation_c1(self.src.as_ref(), &TangentWarp, dir).ok().map(f64::from)
    }

    /// Whether this world has water: a bake with its ocean channel (the sea of the definition, ADR 0041).
    /// The bake's continentalness and elevation at `dir` and whether a vertex of its cell is ocean (ADR 0041 point 2): the facts the
    /// water field decides from, for diagnostics and the coverage oracle. `None` without a bake or an ocean channel, or on a read error.
    pub fn bake_probe(&self, dir: planet_core::Vec3) -> Option<BakeProbe> {
        let ocean = self.ocean.as_deref()?;
        let g = planet_generators::global::read_direction(self.src.as_ref(), &TangentWarp, dir).ok()?;
        let q = planet_generators::global::locate_direction(self.src.res(), &TangentWarp, dir).ok()?;
        let (v, n) = q.weighted_vertices(q.res);
        let ocean_vertex = v[..n].iter().any(|&(i, j)| ocean.is_ocean(q.face, i, j).unwrap_or(false));
        let bound = self
            .definition()
            .sea(BodyId::PLANET)
            .ok()
            .map(|sea| planet_generators::wet_floor::bound_code(planet_generators::water::coast_band_elevation_m(sea.level_m)));
        // A floor that cannot be read is no probe (a read error must not leave pixels out of an oracle's denominator quietly).
        let mut wet_vertex = ocean_vertex;
        if let (Some(floor), Some(bound)) = (self.floor.as_deref(), bound) {
            for &(i, j) in &v[..n] {
                wet_vertex |= floor.floor(q.face, i, j).ok()? <= bound;
            }
        }
        Some(BakeProbe { continent: g.continent, elevation_m: g.elevation_m, ocean_vertex, wet_vertex })
    }

    pub fn has_water(&self) -> bool {
        self.ocean.is_some()
    }

    /// Run `f` with the water field W of this world (the sea of the definition over the bake's ocean channel), `None` when the world
    /// has no water. The field is cheap to assemble, so it is built per call and borrows the bake.
    pub fn with_water<R>(&self, f: impl FnOnce(&WaterField<'_>) -> R) -> Result<Option<R>, String> {
        let Some(ocean) = self.ocean.as_deref() else { return Ok(None) };
        let sea = self.definition().sea(BodyId::PLANET).map_err(|e| format!("the sea of the definition: {e}"))?;
        let mut producer = SeaProducer::new(ocean, sea);
        if let Some(summary) = self.summary {
            producer = producer.with_summary(summary);
        }
        if let Some(floor) = self.floor.as_deref() {
            producer = producer.with_floor(floor);
        }
        let field = WaterField::new(&TangentWarp, self.src.as_ref(), vec![&producer]);
        Ok(Some(f(&field)))
    }

    /// What the next [`WorldTerrain::field`] is built from besides the definition: the coupled lens. A viewer that keeps a field and the plan
    /// bound taken from it builds them again when this changes.
    pub fn field_stage(&self) -> Option<CoupledLens> {
        self.coupled_lens()
    }

    /// The lens the fields are built with, if one is on.
    pub fn coupled_lens(&self) -> Option<CoupledLens> {
        match self.coupled_lens.load(std::sync::atomic::Ordering::Relaxed) {
            1 => Some(CoupledLens::Height),
            2 => Some(CoupledLens::Area),
            _ => None,
        }
    }

    /// Switch the lens (GEN-043). Fields built after the call carry it; the caller refreshes the resident tiles (`ManagedLoop::edit`). A lens
    /// asks for the coupled run, which loads or runs on a thread: until it is [`Standing::Ready`] the fields stay plain.
    pub fn set_coupled_lens(self: &Arc<Self>, lens: Option<CoupledLens>) {
        self.coupled_lens.store(
            match lens {
                None => 0,
                Some(CoupledLens::Height) => 1,
                Some(CoupledLens::Area) => 2,
            },
            std::sync::atomic::Ordering::Relaxed,
        );
        if lens.is_some() {
            self.request_coupled();
        }
    }

    /// Where the coupled run stands.
    pub fn coupled_standing(&self) -> Standing {
        self.coupled.standing()
    }

    /// Start loading or running the coupled run (STRM-044) unless that was done: from the stored channel beside the bake when it holds this
    /// run, else made (and stored there). The lattice is the bake's. No frame waits for it (CON-22).
    pub fn request_coupled(self: &Arc<Self>) {
        let world = Arc::clone(self);
        self.coupled.request(move || {
            if !world.has_bake {
                return Err("this height field reads no bake: there is no coupled run to show".to_string());
            }
            load_or_run(&world.definition(), world.store_dir.as_deref(), world.src.as_ref(), run_cells(world.res), COUPLED_STEPS)
        });
    }

    /// Make the coarse horizon from the stored coupled run beside the bake and store it (the explicit step of STRM-045; minutes at the run's full
    /// size). Errors without a store, a bake, or a stored coupled run (`request_coupled` makes and stores it).
    pub fn make_coarse_horizon(&self, threads: usize) -> Result<usize, String> {
        let dir = self.store_dir.as_deref().ok_or("this world has no store directory to keep a coarse horizon in")?;
        if !self.has_bake {
            return Err("this height field reads no bake: there is no coupled run to take the horizon from".to_string());
        }
        crate::horizonworld::make_coarse_horizon(&self.definition(), dir, self.src.as_ref(), run_cells(self.res), COUPLED_STEPS, threads)
    }

    /// The stored coarse horizon of the coupled run this world shows, read now (the thread of `request_coarse_horizon` calls this).
    pub(crate) fn read_coarse_now(&self) -> crate::horizonworld::CoarseStanding {
        if !self.has_bake {
            return crate::horizonworld::CoarseStanding::Absent("this height field reads no bake: there is no coarse horizon".to_string());
        }
        crate::horizonworld::read_coarse(
            &self.definition(),
            self.store_dir.as_deref(),
            self.src.as_ref(),
            run_cells(self.res),
            COUPLED_STEPS,
        )
    }

    /// The key of the bake behind the world (zeros without one).
    pub fn has_bake(&self) -> bool {
        self.has_bake
    }

    pub(crate) fn bake_key(&self) -> [u8; 16] {
        self.bake_key
    }

    /// The parallelism the tile jobs' helper threads are asked from (the terrain share of the rig, BUDG-004).
    pub fn set_job_par(&self, par: Arc<dyn planet_core::par::Par + Send + Sync>) {
        *self.job_par.write().expect("job par") = par;
    }

    /// Set the scoped threads one horizon bake uses (at least 1).
    pub fn set_horizon_threads(&self, n: usize) {
        self.horizon_threads.store(n.max(1), std::sync::atomic::Ordering::Relaxed);
    }

    /// The threads a measurement set for one horizon bake, `None` for the default.
    pub(crate) fn horizon_threads_set(&self) -> Option<usize> {
        Some(self.horizon_threads.load(std::sync::atomic::Ordering::Relaxed)).filter(|&n| n != 0)
    }

    /// Read the heights at `dirs` on a thread of its own, so that the bake blocks under them are decoded and kept before the main thread reads them
    /// (CON-22: a frame never waits for a block; a block that is cold costs 100 ms and more). Never waits: a batch that finds the queue full is
    /// dropped (the next frame sends another). The thread ends with the world.
    pub fn warm_heights(self: &Arc<Self>, dirs: Vec<planet_core::Vec3>) {
        let mut slot = self.warm.lock().expect("warm queue");
        let tx = slot.get_or_insert_with(|| {
            let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<planet_core::Vec3>>(2);
            let world = Arc::downgrade(self);
            std::thread::Builder::new()
                .name("height-warm".into())
                .spawn(move || {
                    while let Ok(dirs) = rx.recv() {
                        let Some(world) = world.upgrade() else { break };
                        if let Ok(field) = world.field() {
                            for d in dirs {
                                std::hint::black_box(field.height(d));
                            }
                        }
                        world.warmed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                })
                .expect("spawn the height warm-up thread");
            tx
        });
        let _ = tx.try_send(dirs);
    }

    /// The camera's ground read (VIEW-061), for the ground thread of [`crate::cameraground::CameraGround`]: the height at `dir` from a field
    /// made for it, with the bake blocks the read touched pinned in the stored bake's cache, so that the tile streamer cannot push the blocks
    /// under the camera out. NaN when no field can be made (the frame keeps its last answer).
    pub fn ground_pinned(&self, dir: planet_core::Vec3) -> f64 {
        let Ok(field) = self.field() else { return f64::NAN };
        let (h, touched) = planet_cache::global_store::record_touched_blocks(|| field.height(dir));
        if let Some(src) = self.store_source {
            src.pin_touched(&touched);
        }
        h
    }

    /// The blocks the stored bake's cache keeps out of eviction now (VIEW-061); none for an in-memory bake.
    pub fn pinned_blocks(&self) -> Vec<planet_core::TileId> {
        self.store_source.map(|s| s.pinned_blocks()).unwrap_or_default()
    }

    /// Batches the warm-up thread has read so far.
    pub fn warmed_batches(&self) -> u64 {
        self.warmed.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Build the field (a fresh context per field: its key covers what it read).
    pub fn field(&self) -> Result<Box<dyn HeightField + '_>, String> {
        let started = std::time::Instant::now();
        let def = self.definition();
        let built = field_chain_erosion(self.kind, &def, self.src.as_ref(), self.bake_key, self.bound_water, self.erosion.as_ref());
        // The lens over the whole chain (GEN-043); while the run is not ready the field is the plain one.
        let built = match (self.coupled_lens(), self.coupled.standing()) {
            (Some(lens), Standing::Ready(maps, _)) => {
                built.map(|f| Box::new(CoupledLensField::new(f, maps, lens)) as Box<dyn HeightField + '_>)
            }
            _ => built,
        };
        self.field_builds.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let spent = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.field_ns.fetch_add(spent, std::sync::atomic::Ordering::Relaxed);
        built
    }
}

/// A rectangle of lattice nodes: `(i0, j0, w, h)`.
pub(crate) type NodeRect = (u32, u32, usize, usize);

/// The face and the node rectangle `(i0, j0, w, h)` of the lattice a fixture commit needs exact: the tiles with their halo and one tile of margin, widened by the nodes the C1 read of the last local cell needs (3 before, 4
/// after), clamped to the face. The commit's tiles are `(face, ti, tj)` keys of one face.
pub(crate) fn fixture_rectangle(
    spec: &planet_cache::l_writer::CommitSpec,
    tiles: impl Iterator<Item = (u8, u32, u32)> + Clone,
) -> Result<(Face, NodeRect), String> {
    let first = tiles.clone().next().ok_or("the commit holds no tile")?;
    if tiles.clone().any(|k| k.0 != first.0) {
        return Err("the fixture commit spans more than one face".into());
    }
    if spec.res == 0 || !spec.cells.is_multiple_of(spec.res) || spec.cells / spec.res == 0 {
        return Err(format!("the commit's {} local cells are not a whole number of nodes of its {} cells", spec.cells, spec.res));
    }
    let per_node = i64::from(spec.cells / spec.res);
    let (tile, halo) = (i64::from(spec.tile), i64::from(spec.halo));
    let (ti0, ti1) = (tiles.clone().map(|k| k.1).min().expect("a tile"), tiles.clone().map(|k| k.1).max().expect("a tile"));
    let (tj0, tj1) = (tiles.clone().map(|k| k.2).min().expect("a tile"), tiles.map(|k| k.2).max().expect("a tile"));
    let lo = |t: u32| (i64::from(t) * tile - halo - tile).div_euclid(per_node) - 3;
    let hi = |t: u32| ((i64::from(t) + 1) * tile + halo + tile).div_euclid(per_node) + 4;
    let (i0, i1) = (lo(ti0).max(0), hi(ti1).min(i64::from(spec.res)));
    let (j0, j1) = (lo(tj0).max(0), hi(tj1).min(i64::from(spec.res)));
    Ok((Face(first.0), (i0 as u32, j0 as u32, (i1 - i0 + 1) as usize, (j1 - j0 + 1) as usize)))
}

/// Reject what a world-selected terrain run cannot do, before any GPU work (GEN-018). `--world` and `--global-res` belong to
/// the terrain scene: a single view (with `--stream` or `--bundle`), `--flight`, `--perf` and `--perf-smoke`; `--repro` takes
/// the world from the bundle. Without `--world` a view, stream, flight or bundle is the seed-only world (reference field, so
/// `--global-res` has nothing to bake), while the perf scene is the default world (eroded field). An unknown height-field id
/// is refused here with the known ids.
pub fn check_options(o: &Options) -> Result<(), String> {
    // The global-bake perf scene takes the --world and --global-res of the global commands (its own checks).
    if o.perf && matches!(o.scene.as_str(), "global-bake" | "coupled") {
        return Ok(());
    }
    let perf_terrain = (o.perf || o.perf_smoke)
        && (o.scene == "terrain" || crate::streamperf::is_stream_scene(&o.scene) || crate::litperf::is_lit_scene(&o.scene));
    let perf_terrain = perf_terrain || crate::litperf::is_capture_scene(&o.scene);
    let given_res = o.given.contains("--global-res");
    if o.world.is_none() {
        return if given_res && !perf_terrain {
            Err(if o.repro.is_some() {
                "--global-res does not apply to --repro (the bundle records its bake size)".to_string()
            } else {
                "--global-res only applies with --world, or to --perf (the seed-only world's reference field reads no bake)".to_string()
            })
        } else {
            Ok(())
        };
    }
    if o.given.contains("--seed") {
        return Err("--world and --seed both name the definition: give one".into());
    }
    if o.scene != "terrain" && !crate::streamperf::is_stream_scene(&o.scene) && !matches!(o.mode, crate::Mode::Editor) {
        return Err("--world applies to `test-render --scene terrain` (a view, --flight or --perf), not to other scenes".into());
    }
    if o.repro.is_some() {
        return Err(
            "--repro takes the world from the bundle (a bundle records its definition, height field and bake size): drop --world".into()
        );
    }
    let def = load_world(o.world.as_ref().expect("checked above"))?;
    resolve(&def).map_err(|e| e.to_string())?;
    Ok(())
}

/// What [`WorldTerrain::store_check`] found (TEST-024).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreCheck {
    pub source: String,
    pub res: u32,
    /// The levels checked: `floor..=levels`.
    pub floor: u8,
    pub levels: u8,
    /// Directions read of the ocean channel and how many were sea; `None`: the store has no ocean channel.
    pub ocean: Option<(usize, usize)>,
    /// `(present, total)` tiles of levels `floor..=levels` in the compact form.
    pub compact: (usize, usize),
}

impl StoreCheck {
    /// What is wrong with the store for a production window, one text each; empty means it can be used. The ocean must exist and be
    /// neither all land nor all sea on the probe grid (a broken channel reads as one of them), and the compact pack must be complete.
    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        match self.ocean {
            None => p.push(format!(
                "{}: no ocean channel (GLOBAL_VERSION {}): regenerate the store with `generate --global-dir`",
                self.source,
                planet_generators::global::GLOBAL_VERSION
            )),
            Some((read, sea)) if sea == 0 || sea == read => p.push(format!(
                "the ocean channel reads {} on all {read} probe directions: a bad channel",
                if sea == 0 { "land" } else { "sea" }
            )),
            Some(_) => {}
        }
        if self.compact.0 != self.compact.1 {
            p.push(format!(
                "the tile pack holds {} of {} compact tiles of levels {}-{} at MESH_VERSION {}: pre-fill it with `generate --prefill-dir <store> --global-res {}`",
                self.compact.0,
                self.compact.1,
                self.floor,
                self.levels,
                planet_generators::mesh::MESH_VERSION,
                self.res
            ));
        }
        p
    }

    /// The report lines of the check.
    pub fn lines(&self) -> Vec<String> {
        let ocean = match self.ocean {
            Some((read, sea)) => format!("present, {read} directions read, {sea} sea and {} land", read - sea),
            None => "MISSING".to_string(),
        };
        vec![
            format!(
                "store check (smoke, read-only, NOT a measurement): {}; GLOBAL_VERSION {}, MESH_VERSION {}",
                self.source,
                planet_generators::global::GLOBAL_VERSION,
                planet_generators::mesh::MESH_VERSION
            ),
            format!("  ocean channel: {ocean}"),
            format!(
                "  tile pack, levels {}-{}: compact form {} of {} present (each read and decoded)",
                self.floor, self.levels, self.compact.0, self.compact.1
            ),
        ]
    }
}

#[cfg(test)]
mod default_store_tests {
    use super::*;

    // spec: VIEW-052
    #[test]
    fn the_default_store_is_the_environment_then_the_pointer_then_the_known_folders_and_none_when_nothing_exists() {
        let root = std::env::temp_dir().join(format!("planet-default-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("target/stores")).unwrap();
        assert_eq!(default_store_in(&root, None), None, "no store anywhere: the preview bake");
        std::fs::create_dir_all(root.join("target/accept/global16384")).unwrap();
        assert_eq!(default_store_in(&root, None), Some(root.join("target/accept/global16384")));
        std::fs::create_dir_all(root.join("target/stores/global16384")).unwrap();
        assert_eq!(default_store_in(&root, None), Some(root.join("target/stores/global16384")), "the stores folder wins over accept");
        std::fs::create_dir_all(root.join("elsewhere")).unwrap();
        std::fs::write(root.join("target/stores/default"), "elsewhere\n").unwrap();
        assert_eq!(default_store_in(&root, None), Some(root.join("elsewhere")), "the pointer file wins over the known folders");
        let env = root.join("from-env");
        std::fs::create_dir_all(&env).unwrap();
        assert_eq!(default_store_in(&root, Some(env.clone())), Some(env), "the environment wins over all");
        assert_eq!(
            default_store_in(&root, Some(root.join("missing"))),
            Some(root.join("elsewhere")),
            "a variable that names no folder falls through"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
