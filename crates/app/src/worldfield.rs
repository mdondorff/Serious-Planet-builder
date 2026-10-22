//! The terrain field of a run, chosen by the world (GEN-018): `test-render --scene terrain --world FILE` resolves the
//! definition's `world.height_field` (the eroded field by default, `cpu-terrain-v1` or the reference when named, an unknown id
//! refused with the known ids) and builds it over an in-memory bake of `--global-res` cells per face edge. A run that names no
//! world file is the seed-only single planet and names `cpu-ref-height-v2` explicitly: every golden, plan snapshot and repro
//! bundle was made with it, so they do not move.

use std::path::Path;
use std::sync::Arc;

use planet_cache::global_store::{GlobalWorld, StoreSource};
use planet_cache::meshpack::TileSpec;
use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::global::{GlobalCoarseGenerator, GlobalGrid, GlobalSample, GridSource, LatticeSource, ReadError};
use planet_generators::height::{HeightField, IMPLEMENTATION_ID as REFERENCE_ID};
use planet_generators::height_fields::{build_field, resolve, HeightFieldKind};
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

/// The reference field reads no bake; this stands in for one.
struct NoBake;

impl LatticeSource for NoBake {
    fn res(&self) -> u32 {
        1
    }

    fn at(&self, _: Face, _: u32, _: u32) -> Result<GlobalSample, ReadError> {
        Err(ReadError::Source("this height field reads no bake".into()))
    }
}

/// An in-memory bake the run owns.
struct OwnedBake(GlobalGrid);

impl LatticeSource for OwnedBake {
    fn res(&self) -> u32 {
        GridSource(&self.0).res()
    }

    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        GridSource(&self.0).at(face, i, j)
    }
}

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

/// The in-memory ocean channel.
struct OwnedOcean(planet_generators::ocean::OceanGrid);

impl OceanLattice for OwnedOcean {
    fn res(&self) -> u32 {
        self.0.res
    }

    fn is_ocean(&self, face: Face, i: u32, j: u32) -> Result<bool, ReadError> {
        self.0.is_ocean(face, i, j)
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
        })
    }

    /// The terrain over a stored bake (the production source: `generate --global-dir`, GEN-014) of `res` cells per face edge in
    /// `dir`. The store and its world are leaked: the field borrows them for the life of the process, a CLI run. Refuses a field
    /// that reads no bake, a directory that is not a store, and a store that does not hold the bake of this definition at `res`.
    pub fn open_store(def: WorldDefinition, dir: &Path, res: u32) -> Result<WorldTerrain, String> {
        let kind = resolve(&def).map_err(|e| e.to_string())?;
        if !kind.needs_bake() {
            return Err(format!("height field {} reads no bake: --store has nothing to read", kind.id()));
        }
        let radius_m =
            GenContext::new(&def).body_radius_m(BodyId::PLANET).map_err(|e| format!("the definition cannot be generated: {e}"))?;
        if !dir.is_dir() {
            return Err(format!("{}: no such store directory (a bake made by `generate --global-dir`)", dir.display()));
        }
        let world = GlobalWorld::from_definition(&def).map_err(|e| format!("the definition cannot be generated: {e}"))?;
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
                    let d = planet_core::Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
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
        Arc::clone(&self.def.read().expect("definition lock"))
    }

    /// Replace the definition the fields are built from, for an edit of its items (the editor's definition after a command). The base
    /// must be the one the bake was made from (`base_hash`), else the call is refused: a change of the nodes needs another terrain. Fields
    /// built before the call keep the definition they were built from; the next `field()` reads the new one. After this call, in the
    /// caller: `ManagedLoop::edit(tiles)`, so the replan sees the new definition.
    pub fn set_definition(&self, def: Arc<WorldDefinition>) -> Result<(), String> {
        let mut current = self.def.write().expect("definition lock");
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
        self.kind.id()
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
        if self.has_bake {
            format!("height field {} over a {}-cell bake", self.kind.id(), self.res)
        } else {
            format!("height field {}", self.kind.id())
        }
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
        if matches!(self.with_water(|f| book.fill(f, &TangentWarp, tile, radius_m)), Ok(Some(Ok(())))) {
            book.set_min_height(tile, built.min_height_m());
        }
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

    /// Build the field (a fresh context per field: its key covers what it read).
    pub fn field(&self) -> Result<Box<dyn HeightField + '_>, String> {
        let started = std::time::Instant::now();
        let def = self.definition();
        let ctx = GenContext::new(&def);
        let built = build_field(self.kind, &ctx, BodyId::PLANET, self.src.as_ref(), self.bake_key)
            .map_err(|e| format!("the definition cannot be generated: {e}"))
            .and_then(|base| with_stamps(base, &def))
            .map(crate::litperf::realism::wrap);
        self.field_builds.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let spent = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.field_ns.fetch_add(spent, std::sync::atomic::Ordering::Relaxed);
        match (built, self.bound_water) {
            (Ok(f), true) => {
                let level = def.sea(BodyId::PLANET).map_err(|e| format!("the sea of the definition: {e}"))?.level_m;
                Ok(Box::new(planet_generators::water::WaterBounded::new(f, level)))
            }
            (built, _) => built,
        }
    }
}

/// `base` with the stamp stages of the definition's layers over it, lowest layer first (GEN-026, ADR 0007); the base itself when the
/// definition holds no stamp. The stages read the snapshot `def`, so the field keeps what it was built from.
fn with_stamps<'a>(base: Box<dyn HeightField + 'a>, def: &Arc<WorldDefinition>) -> Result<Box<dyn HeightField + 'a>, String> {
    use planet_generators::stamp::{ItemReader, StampStage, STAMP_STAGE_VERSION};
    let layers = def.stamp_layers(BodyId::PLANET);
    let Some((top, below)) = layers.split_last() else { return Ok(base) };
    let stage = |field: Arc<dyn HeightField + 'a>, layer: &str, index: usize| {
        let items: Arc<dyn ItemReader> = def.clone();
        StampStage::new(field, items, BodyId::PLANET, layer, index as u32, STAMP_STAGE_VERSION)
            .ok_or_else(|| "the definition has stamps but no usable radius".to_string())
    };
    // The index of a stage is its layer's place in the stack of ADR 0007 (a layer outside it follows the known ones, in name order), not
    // its place among the layers that hold stamps: a layer added below shifts no index above it.
    let unknown: Vec<&String> = layers.iter().filter(|l| !planet_world_def::LAYER_STACK.contains(&l.as_str())).collect();
    let index_of = |layer: &str| {
        planet_world_def::LAYER_STACK.iter().position(|s| *s == layer).unwrap_or_else(|| {
            planet_world_def::LAYER_STACK.len() + unknown.iter().position(|u| u.as_str() == layer).expect("a layer of the list")
        })
    };
    let mut field: Arc<dyn HeightField + 'a> = Arc::from(base);
    for layer in below {
        field = Arc::new(stage(field, layer, index_of(layer))?);
    }
    Ok(Box::new(stage(field, top, index_of(top))?))
}

/// Reject what a world-selected terrain run cannot do, before any GPU work (GEN-018). `--world` and `--global-res` belong to
/// the terrain scene: a single view (with `--stream` or `--bundle`), `--flight`, `--perf` and `--perf-smoke`; `--repro` takes
/// the world from the bundle. Without `--world` a view, stream, flight or bundle is the seed-only world (reference field, so
/// `--global-res` has nothing to bake), while the perf scene is the default world (eroded field). An unknown height-field id
/// is refused here with the known ids.
pub fn check_options(o: &Options) -> Result<(), String> {
    // The global-bake perf scene takes the --world and --global-res of the global commands (its own checks).
    if o.perf && o.scene == "global-bake" {
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
    if o.scene != "terrain" && !crate::streamperf::is_stream_scene(&o.scene) {
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
