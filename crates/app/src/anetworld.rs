//! The A-net chunk stage in a world (M8-3 step 4, GEN-052, STRM-046): the stage built from the world's stored coupled run and its bare base
//! field, the chunk disk beside the store, the chunk lattice of the default cell size. GPU-free.

use std::sync::{Arc, Weak};

use planet_cache::chunk_store::{ChunkSpec, PackChunkDisk};
use planet_core::cube::TangentWarp;
use planet_core::hash::mix64;
use planet_core::Vec3;
use planet_generators::anet::{ANet, ANetParams, BaseDetail};
use planet_generators::chunkfield::ChunkStage;
use planet_generators::coupled::CoupledMaps;
use planet_generators::global::read_direction;
use planet_streaming::chunks::{ChunkConfig, ChunkDisk};
use planet_streaming::{Clock, Spawner};
use planet_world_def::BodyId;

use crate::coupledworld::COUPLED_STEPS;
use crate::worldfield::WorldTerrain;

/// The default cell size of an A-net chunk in metres (planner decision 2026-10-24; GEN-052 records 4 and 10 m beside it).
pub const ANET_CELL_M: f64 = 8.0;
/// The top level of the A-net chunk pyramid, the coarsest tile level that reads chunk detail (provisional: the steps between pyramid levels are
/// smallest from level 15; the entry step from the base is metres at every level, design.md section 16).
pub const ANET_TOP_LEVEL: u8 = 15;
/// The default halo of an A-net chunk in metres (GEN-049).
pub const ANET_HALO_M: f64 = 3000.0;

/// The base the A-net changes: the world's field below the chunk stage and the stamps, and what the coupled run's input held.
pub struct WorldBase {
    world: Weak<WorldTerrain>,
    identity: u64,
    /// Set when an answer was a stand-in (the world gone, the field not buildable): nothing made after it is stored.
    failed: std::sync::atomic::AtomicBool,
    /// Threads a batch of base heights uses.
    threads: usize,
}

impl WorldBase {
    pub fn new(world: &Arc<WorldTerrain>) -> WorldBase {
        let mut h = mix64(0x0000_6261_7365);
        for b in world.bake_key().chunks(8) {
            h = mix64(h ^ u64::from_le_bytes(b.try_into().expect("8 bytes")));
        }
        h = mix64(h ^ world.radius_m().to_bits());
        h = mix64(h ^ u64::from(world.res()));
        let seed = world.base_seed();
        h = mix64(h ^ seed);
        // The base field's own definition: the kind, its implementation and version (the bake's key covers the bake alone).
        h = mix64(h ^ fold(world.id()));
        if let Ok(f) = world.base_field() {
            h = mix64(h ^ fold(f.implementation_id()));
            h = mix64(h ^ u64::from(f.version()));
        }
        WorldBase { world: Arc::downgrade(world), identity: h, failed: Default::default(), threads: 4 }
    }
}

impl BaseDetail for WorldBase {
    fn base_height(&self, dir: Vec3) -> f64 {
        self.base_heights(&[dir])[0]
    }

    /// One field for the whole batch (building it costs more than a read); a world that is gone or a field that cannot be built reads as sea level.
    fn base_heights(&self, dirs: &[Vec3]) -> Vec<f64> {
        let Some(world) = self.world.upgrade() else { return self.lost(dirs.len()) };
        let Ok(field) = world.base_field() else { return self.lost(dirs.len()) };
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(self.threads);
        field.heights_threaded_until(dirs, &|| false, threads)
    }

    fn coarse_input(&self, dir: Vec3) -> f64 {
        let Some(world) = self.world.upgrade() else { return self.lost(1)[0] };
        read_direction(world.lattice_source(), &TangentWarp, dir).map_or(0.0, |s| f64::from(s.elevation_m))
    }

    fn identity(&self) -> u64 {
        self.identity
    }

    fn healthy(&self) -> bool {
        self.world.strong_count() > 0 && !self.failed.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl WorldBase {
    /// The same base reading its heights on `threads` threads (the perf scene measures with one).
    pub fn with_threads(mut self, threads: usize) -> WorldBase {
        self.threads = threads.max(1);
        self
    }

    /// Stand-in answers for a base that lost its source; the failure is remembered.
    fn lost(&self, n: usize) -> Vec<f64> {
        self.failed.store(true, std::sync::atomic::Ordering::SeqCst);
        vec![0.0; n]
    }
}

/// A hash of the bytes of `s`.
fn fold(s: &str) -> u64 {
    s.bytes().fold(0x0000_666f_6c64, |h, b| mix64(h ^ u64::from(b)))
}

/// Cells per chunk edge for chunks of about `cell_m` metres on a body of `radius_m` (608 at 8 m, 1216 at 4 m, 496 at 10 m on Earth).
pub fn cells_per_chunk_for(radius_m: f64, cell_m: f64) -> u32 {
    let edge = radius_m * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << planet_generators::chunk::CHUNK_GRID_LEVEL);
    // A multiple of 16: the chunk's pyramid halves the edge once per level it carries (up to 4 levels, cells down to about 1.2 m).
    ((edge / cell_m / 16.0).round().max(1.0) as u32) * 16
}

impl WorldTerrain {
    /// Give the world the A-net chunk stage from the coupled run `maps` (the stored channel or a run just made): cells of about `cell_m`,
    /// chunk jobs on `spawner`, eviction on `clock`, landed chunks cached beside the store when the world has one. Replaces an earlier stage.
    pub fn enable_anet(
        self: &Arc<Self>,
        maps: Arc<CoupledMaps>,
        cell_m: f64,
        spawner: Arc<dyn Spawner>,
        clock: Arc<dyn Clock>,
        cfg: ChunkConfig,
    ) -> Result<(), String> {
        let per = cells_per_chunk_for(self.radius_m(), cell_m);
        let lat = planet_generators::chunkfield::ChunkLattice::new(per)
            .ok_or_else(|| format!("{per} cells per chunk edge do not fit a lattice"))?;
        let coarse_n = planet_generators::anet::CoarseRun::lattice(&*maps).n() as i64;
        if coarse_n == 0 || lat.n() % coarse_n != 0 {
            return Err(format!(
                "a chunk lattice of {} cells per face edge ({per} per chunk) is not a whole number of cells per cell of the coupled run of {coarse_n}: use a run of at most 2048 cells",
                lat.n()
            ));
        }
        let stage: Arc<dyn ChunkStage> = Arc::new(ANet {
            run: maps,
            base: Arc::new(WorldBase::new(self)),
            radius_m: self.radius_m(),
            params: ANetParams::spike(ANET_HALO_M, lat.cell_m(self.radius_m())),
        });
        let disk: Option<Arc<dyn ChunkDisk>> = self.store_dir().map(|dir| {
            let spec = ChunkSpec {
                body: BodyId::PLANET,
                seed: self.base_seed(),
                radius_m: self.radius_m(),
                definition: stage.definition_hash(),
                cells_per_chunk: per,
                top_level: ANET_TOP_LEVEL,
                base: self.bake_key(),
            };
            Arc::new(PackChunkDisk::new(dir.to_path_buf(), spec)) as Arc<dyn ChunkDisk>
        });
        self.enable_chunks_at(stage, per, ANET_TOP_LEVEL, spawner, clock, disk, cfg)
    }
}

impl WorldTerrain {
    /// Look for the stored coupled channel of the world on a thread and give the world the A-net chunk stage when it holds one (STRM-046): a
    /// world without the channel, or without a bake, keeps no chunk stage and every tile is the base. A second call does nothing. No frame
    /// waits for it (CON-22).
    pub fn request_anet(self: &Arc<Self>, spawner: Arc<dyn Spawner>, clock: Arc<dyn Clock>, cfg: ChunkConfig) {
        if !self.has_bake() || self.anet_asked.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let world = Arc::clone(self);
        std::thread::Builder::new()
            .name("anet-load".into())
            .spawn(move || match world.enable_anet_from_store(ANET_CELL_M, spawner, clock, cfg) {
                Ok(true) => {}
                Ok(false) => eprintln!(
                    "chunks: no stored coupled channel for this world: the terrain is the base (generate it with the coupled lens, K)"
                ),
                Err(e) => eprintln!("chunks: {e}"),
            })
            .expect("spawn the chunk stage's thread");
    }

    /// [`WorldTerrain::enable_anet`] from the stored channel beside the store; `Ok(false)` when the store holds none for this world.
    pub fn enable_anet_from_store(
        self: &Arc<Self>,
        cell_m: f64,
        spawner: Arc<dyn Spawner>,
        clock: Arc<dyn Clock>,
        cfg: ChunkConfig,
    ) -> Result<bool, String> {
        let run = crate::coupledworld::load_stored(
            &self.definition(),
            self.store_dir(),
            self.lattice_source(),
            self.res(),
            COUPLED_STEPS,
            self.res(),
        )?;
        let Some(run) = run else { return Ok(false) };
        self.enable_anet(run.maps, cell_m, spawner, clock, cfg)?;
        Ok(true)
    }
}
