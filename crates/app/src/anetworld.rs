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

use crate::coupledworld::{run_cells, COUPLED_STEPS};
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
            .spawn(move || match world.enable_anet_from_store(ANET_CELL_M, spawner.clone(), clock.clone(), cfg) {
                Ok(true) => {
                    // The horizon shadows (STRM-049, STRM-045): the second job of every chunk and the stored far-field horizon.
                    if let Some(lat) = world.chunk_lattice() {
                        let params = crate::horizonworld::chunk_horizon_params(&lat, world.radius_m());
                        let threads = std::thread::available_parallelism().map_or(1, |t| t.get()).min(4);
                        if let Err(e) =
                            world.enable_horizon(params, spawner, clock, planet_streaming::chunks::HorizonConfig::default(), threads)
                        {
                            eprintln!("horizon: {e}");
                        }
                    }
                    world.request_coarse_horizon();
                }
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
            run_cells(self.res()),
            COUPLED_STEPS,
        )?;
        let Some(run) = run else { return Ok(false) };
        self.enable_anet(run.maps, cell_m, spawner, clock, cfg)?;
        Ok(true)
    }
}

impl WorldTerrain {
    /// A diagnostic, not part of the viewer (`slump_rate` and `steps` replace the stage's defaults for the report only): the unclamped change of the A-net solve over the chunk holding `lat_deg`, `lon_deg` (the stage clamps its
    /// change to `bound_m`): the distribution of the change, how many cells sit at the bound, and the change of the cut cells by drainage area.
    pub fn chunk_incision_report(
        self: &Arc<Self>,
        lat_deg: f64,
        lon_deg: f64,
        slump_rate: Option<f64>,
        steps: Option<u32>,
    ) -> Result<String, String> {
        use planet_core::cube::TangentWarp;
        use planet_core::{TileId, Vec3};
        use planet_generators::chunkfield::{region, ChunkLattice};
        let run = crate::coupledworld::load_stored(
            &self.definition(),
            self.store_dir(),
            self.lattice_source(),
            run_cells(self.res()),
            COUPLED_STEPS,
        )?
        .ok_or("no stored coupled run")?;
        let radius = self.radius_m();
        let per = cells_per_chunk_for(radius, ANET_CELL_M);
        let clat = ChunkLattice::new(per).ok_or("no chunk lattice")?;
        let mut params = ANetParams::spike(ANET_HALO_M, clat.cell_m(radius));
        // What the census says when the hillslope step is stronger or the run longer than the defaults (an argument of the report, never a production value).
        if let Some(v) = slump_rate {
            params.slump_rate = v;
        }
        if let Some(v) = steps {
            params.steps = v;
        }
        let halo = params.halo_rings;
        let stage = ANet { run: run.maps, base: Arc::new(WorldBase::new(self)), radius_m: radius, params };
        let dir = Vec3::from_lat_lon(lat_deg.to_radians(), lon_deg.to_radians());
        let tile = TileId::from_direction(&TangentWarp, dir, planet_generators::chunk::CHUNK_GRID_LEVEL);
        let reg = region(&clat, tile, halo);
        let s = stage.solve(&clat, &reg);
        let net = planet_generators::anet::Network::build_with(&*stage.run, &*stage.base, &clat, &reg, radius, &stage.params.anchor);
        let core = reg.core;
        let mut anchors: Vec<f64> =
            (0..reg.cells.len()).filter(|&c| !net.anchor_z[c].is_nan()).map(|c| net.anchor_z[c] - net.base_z[c]).collect();
        anchors.sort_by(f64::total_cmp);
        let mut change: Vec<f64> = (0..core).map(|c| s.z[c] - s.z_base[c]).collect();
        let n = change.len();
        let count = |f: &dyn Fn(f64) -> bool| change.iter().filter(|d| f(**d)).count();
        let (pinned, deep, cut) = (count(&|d| d <= -299.999), count(&|d| d <= -250.0), count(&|d| d < -20.0));
        let mut by_area: Vec<(f64, f64)> =
            (0..core).filter(|&c| s.z[c] - s.z_base[c] < -20.0).map(|c| (s.area_m2[c], s.z[c] - s.z_base[c])).collect();
        by_area.sort_by(|a, b| a.0.total_cmp(&b.0));
        change.sort_by(f64::total_cmp);
        let na = anchors.len();
        let corridor = net.corridor.iter().filter(|c| **c).count();
        let mut out = format!(
            "corridor cells {corridor}, anchored cells {na}: anchor minus base: min {:.1} m, p10 {:.1}, p50 {:.1}, p90 {:.1}, max {:.1}
",
            anchors.first().copied().unwrap_or(f64::NAN),
            anchors.get(na / 10).copied().unwrap_or(f64::NAN),
            anchors.get(na / 2).copied().unwrap_or(f64::NAN),
            anchors.get(na * 9 / 10).copied().unwrap_or(f64::NAN),
            anchors.last().copied().unwrap_or(f64::NAN)
        );
        out += &format!(
            "tile {tile:?}: {n} core cells of {:.1} m, halo {halo} rings
",
            clat.cell_m(radius)
        );
        out += &format!(
            "change: min {:.1} m, p0.1% {:.1}, p1% {:.1}, p50 {:.2}, max {:.1}
",
            change[0],
            change[n / 1000],
            change[n / 100],
            change[n / 2],
            change[n - 1]
        );
        out += &format!(
            "cells under -20 m: {cut}, under -250 m: {deep}, pinned at -300 m: {pinned}
"
        );
        for q in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
            let i = (by_area.len().saturating_sub(1) as f64 * q) as usize;
            if let Some((a, d)) = by_area.get(i) {
                out += &format!(
                    "  cut cells by area, quantile {q:.2}: area {a:.3e} m2, change {d:.1} m
"
                );
            }
        }
        // The bed along the corridor, following the links from the cells nothing links to: the longest run of equal heights, the largest drop between two
        // linked cells, the links that climb by more than 5 cm, and the median bed slope over 8-link windows.
        {
            use planet_generators::anet::NO_NEXT;
            let cell_m = clat.cell_m(radius);
            let mut has_donor = vec![false; reg.cells.len()];
            for c in 0..reg.cells.len() {
                if net.corridor[c] && net.next[c] != NO_NEXT {
                    has_donor[net.next[c] as usize] = true;
                }
            }
            let (mut longest_flat, mut max_drop, mut climbs, mut links) = (0.0f64, 0.0f64, 0usize, 0usize);
            let mut window_slopes: Vec<f64> = Vec::new();
            for start in 0..reg.cells.len() {
                if !net.corridor[start] || has_donor[start] {
                    continue;
                }
                let (mut at, mut flat, mut chain) = (start, 0.0f64, vec![s.z[start]]);
                while net.next[at] != NO_NEXT {
                    let n = net.next[at] as usize;
                    let drop = s.z[at] - s.z[n];
                    if at < core && n < core {
                        links += 1;
                        max_drop = max_drop.max(drop.abs());
                        climbs += usize::from(drop < -0.05);
                        if drop.abs() < 1e-9 {
                            flat += cell_m;
                            longest_flat = longest_flat.max(flat);
                        } else {
                            flat = 0.0;
                        }
                    }
                    chain.push(s.z[n]);
                    at = n;
                }
                for w in chain.windows(9) {
                    window_slopes.push((w[0] - w[8]) / (8.0 * cell_m));
                }
            }
            let outlets = (0..core).filter(|&c| net.corridor[c] && net.next[c] != NO_NEXT && s.receiver[c] == u32::MAX).count();
            window_slopes.sort_by(f64::total_cmp);
            out += &format!(
                "bed: {links} links in the core; longest flat reach {longest_flat:.0} m; largest drop between linked cells {max_drop:.2} m; links that climb over 5 cm {climbs}; linked corridor cells left without a receiver {outlets}; median slope over 8 links {:.2e}
",
                window_slopes.get(window_slopes.len() / 2).copied().unwrap_or(f64::NAN)
            );
        }
        // The steep pairs of neighbouring core cells of the solved surface: how many are over the angle of repose and over the slope the plan can draw,
        // and whether a protected cell (a channel, the corridor, an anchor: the hillslope steps skip every pair with one) is one of the two.
        let cell_m = clat.cell_m(radius);
        let channel_m2 = stage.params.channel_area_m2;
        let protected = |c: usize| s.area_m2[c] > channel_m2 || net.corridor[c] || !net.anchor_z[c].is_nan();
        let (mut over_repose, mut over_drawable, mut with_protected, mut max_free, mut max_any) = (0usize, 0usize, 0usize, 0.0f64, 0.0f64);
        let mut pairs = 0usize;
        for c in 0..core {
            for (k, &nb) in reg.neighbours[c].iter().enumerate() {
                let q = nb as usize;
                if q >= core || k >= 4 {
                    continue;
                }
                pairs += 1;
                let slope = (s.z[c] - s.z[q]).abs() / cell_m;
                max_any = max_any.max(slope);
                let prot = protected(c) || protected(q);
                if !prot {
                    max_free = max_free.max(slope);
                }
                if slope > 0.9 {
                    over_repose += 1;
                    with_protected += usize::from(prot);
                }
                over_drawable += usize::from(slope > 3.42);
            }
        }
        let (mut base_repose, mut base_max) = (0usize, 0.0f64);
        for c in 0..core {
            for (k, &nb) in reg.neighbours[c].iter().enumerate() {
                let q = nb as usize;
                if q >= core || k >= 4 {
                    continue;
                }
                let slope = (s.z_base[c] - s.z_base[q]).abs() / cell_m;
                base_max = base_max.max(slope);
                base_repose += usize::from(slope > 0.9);
            }
        }
        out += &format!(
            "the base surface alone: pairs over 0.9: {base_repose}, steepest pair {base_max:.2}
"
        );
        out += &format!(
            "edge pairs {pairs}: over the repose slope 0.9: {over_repose} (with a protected cell: {with_protected}), over 3.42: {over_drawable}; steepest pair {max_any:.2}, steepest pair of two unprotected cells {max_free:.2}
"
        );
        Ok(out)
    }
}

impl WorldTerrain {
    /// A diagnostic, not part of the viewer: the coupled run's height minus the base field's at the centre of its cells, by ground band of the base:
    /// over the river cells (drainage area of at least `min_area_m2`) and over every `stride`th land cell. The chunk stage anchors its corridor beds
    /// to the run's heights, so this is how far below the base ground an anchor lies.
    pub fn run_against_base_report(self: &Arc<Self>, stride: usize, min_area_m2: f64) -> Result<String, String> {
        let run = crate::coupledworld::load_stored(
            &self.definition(),
            self.store_dir(),
            self.lattice_source(),
            run_cells(self.res()),
            COUPLED_STEPS,
        )?
        .ok_or("no stored coupled run")?;
        let maps = run.maps;
        let lat = maps.lattice();
        let (n, stride) = (lat.n(), stride.max(1));
        let base = WorldBase::new(self);
        let rec = maps.receivers();
        let rivers: Vec<usize> = (0..lat.len())
            .filter(|&c| rec[c] != planet_generators::coupled::NONE && f64::from(maps.network_area_m2(c)) >= min_area_m2)
            .collect();
        let land: Vec<usize> = (0..6)
            .flat_map(|f| {
                (stride / 2..n).step_by(stride).flat_map(move |j| (stride / 2..n).step_by(stride).map(move |i| f * n * n + j * n + i))
            })
            .collect();
        let measure = |cells: &[usize]| -> Vec<(f64, f64)> {
            let dirs: Vec<Vec3> = cells.iter().map(|&c| lat.direction(c)).collect();
            let h = base.base_heights(&dirs);
            cells.iter().zip(h).filter(|(_, h)| *h > 0.0).map(|(&c, h)| (h, f64::from(maps.z(c)) - h)).collect()
        };
        let (land, rivers) = (measure(&land), measure(&rivers));
        let bands = [(0.0, 500.0), (500.0, 1000.0), (1000.0, 2500.0), (2500.0, 5000.0), (5000.0, f64::INFINITY)];
        let pct = |v: &mut Vec<f64>, q: f64| {
            v.sort_by(f64::total_cmp);
            v.get(((v.len() as f64 - 1.0) * q).round() as usize).copied().unwrap_or(f64::NAN)
        };
        let mut out = String::from(
            "# coupled run height minus base height (m) at cell centres, by base ground band
# band_m cells_land p10 p50 p90 cells_river p10 p50 p90
",
        );
        for (lo, hi) in bands {
            let pick = |all: &[(f64, f64)]| -> Vec<f64> { all.iter().filter(|(h, _)| *h >= lo && *h < hi).map(|(_, d)| *d).collect() };
            let (mut l, mut r) = (pick(&land), pick(&rivers));
            let name = if hi.is_finite() { format!("{lo:.0}-{hi:.0}") } else { format!(">{lo:.0}") };
            out += &format!(
                "{name} {} {:.0} {:.0} {:.0} {} {:.0} {:.0} {:.0}
",
                l.len(),
                pct(&mut l, 0.1),
                pct(&mut l, 0.5),
                pct(&mut l, 0.9),
                r.len(),
                pct(&mut r, 0.1),
                pct(&mut r, 0.5),
                pct(&mut r, 0.9)
            );
        }
        Ok(out)
    }
}
