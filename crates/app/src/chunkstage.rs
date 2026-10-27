//! The A-net chunk stage of a world for the headless scenes (capture stills, the speed-envelope scene): wait for the coupled run, give the world
//! the stage and, if asked, the horizon jobs, and the pure bookkeeping of how long chunks took to arrive.

use crate::worldfield::WorldTerrain;
use crate::{err, Options};
use planet_generators::chunkfield::ChunkKey;
use planet_streaming::chunks::ChunkConfig;
use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What the stage runs besides the chunk jobs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageOptions {
    /// The horizon jobs (REND-038) and the stored coarse horizon.
    pub horizon: bool,
    /// The disk cache of landed chunks beside the store: off, every chunk is computed.
    pub disk: bool,
}

/// Wait for the coupled run of the world (made and stored if the store holds none) and give the world its A-net chunk stage; a second call
/// replaces the stage (its chunk cache starts empty).
pub fn enable_chunks(
    world: &Arc<WorldTerrain>,
    cell_m: f64,
    workers: usize,
    stage: StageOptions,
    out: &mut dyn Write,
) -> Result<Arc<planet_generators::coupled::CoupledMaps>, String> {
    use crate::coupledworld::Standing;
    world.request_coupled();
    let started = Instant::now();
    let maps = loop {
        match world.coupled_standing() {
            Standing::Ready(m, _) => break m,
            Standing::Failed(e) => return Err(format!("the coupled run failed: {e}")),
            _ => std::thread::sleep(Duration::from_millis(100)),
        }
    };
    writeln!(out, "coupled run ready after {:.1} s", started.elapsed().as_secs_f64()).map_err(err)?;
    let pool = planet_streaming::GenWorkers::new(workers).pool();
    let cfg = ChunkConfig { workers, evict_ms: 3_600_000, cap_bytes: 4 << 30, ..ChunkConfig::default() };
    world.enable_anet_with(maps.clone(), cell_m, pool, Arc::new(planet_streaming::SystemClock::new()), cfg, stage.disk)?;
    if !stage.horizon {
        return Ok(maps);
    }
    // The horizon shadows (REND-038): the second job of every chunk, and the stored far-field horizon read now (a still waits for both).
    let lat = world.chunk_lattice().ok_or("the world has no chunk lattice")?;
    let params = crate::horizonworld::chunk_horizon_params(&lat, world.radius_m());
    let horizon_pool = planet_streaming::GenWorkers::new(workers).pool();
    world.enable_horizon(
        params,
        horizon_pool,
        Arc::new(planet_streaming::SystemClock::new()),
        planet_streaming::chunks::HorizonConfig { workers, settle_ms: 1_000 },
        workers,
    )?;
    world.request_coarse_horizon();
    let coarse = loop {
        match world.coarse_standing() {
            crate::horizonworld::CoarseStanding::Idle | crate::horizonworld::CoarseStanding::Working => {
                std::thread::sleep(Duration::from_millis(50))
            }
            crate::horizonworld::CoarseStanding::Ready(h) => {
                break format!("coarse horizon: {} observer cells per face edge, {} azimuths", h.m, h.azimuths)
            }
            crate::horizonworld::CoarseStanding::Absent(t) | crate::horizonworld::CoarseStanding::Failed(t) => {
                break format!("horizon: {t}")
            }
        }
    };
    writeln!(out, "{coarse}").map_err(err)?;
    Ok(maps)
}

/// The viewer's chunk stage for a headless scene: exactly what the window sets up (`anetworld::viewer_chunk_stage` for the pool and the knobs,
/// `WorldTerrain::enable_viewer_chunks` for the stage and the horizon jobs), on the generation workers `gen_workers`; waits for the far-field horizon when
/// `horizon`. A second call replaces the stage (its chunk cache starts empty). `disk` off computes every chunk.
pub fn enable_viewer_stage(
    world: &Arc<WorldTerrain>,
    gen: planet_streaming::GenWorkers,
    horizon: bool,
    disk: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    enable_viewer_stage_sized(world, gen, None, horizon, disk, out)
}

/// [`enable_viewer_stage`] with the chunk workers fixed to `workers` (a measurement of other pool sizes); `None` is the viewer's own.
pub fn enable_viewer_stage_sized(
    world: &Arc<WorldTerrain>,
    gen: planet_streaming::GenWorkers,
    workers: Option<usize>,
    horizon: bool,
    disk: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    let (pool, cfg): (Arc<dyn planet_streaming::Spawner>, _) = match workers {
        Some(n) => {
            let n = n.max(1);
            (
                planet_streaming::GenWorkers::new(n).with_priority(gen.priority).pool(),
                planet_streaming::chunks::ChunkConfig { workers: n, ..Default::default() },
            )
        }
        None => crate::anetworld::viewer_chunk_stage(gen),
    };
    let workers = cfg.workers;
    if !world.enable_viewer_chunks(pool, Arc::new(planet_streaming::SystemClock::new()), cfg, horizon, disk)? {
        return Err("the store holds no coupled run: the viewer's chunk stage needs one (generate --coupled-run)".into());
    }
    writeln!(out, "viewer chunk stage: {workers} workers, horizon {horizon}, disk cache {disk}").map_err(err)?;
    if horizon {
        loop {
            match world.coarse_standing() {
                crate::horizonworld::CoarseStanding::Idle | crate::horizonworld::CoarseStanding::Working => {
                    std::thread::sleep(Duration::from_millis(50))
                }
                crate::horizonworld::CoarseStanding::Ready(h) => {
                    writeln!(out, "coarse horizon: {} observer cells per face edge, {} azimuths", h.m, h.azimuths).map_err(err)?;
                    break;
                }
                crate::horizonworld::CoarseStanding::Absent(t) | crate::horizonworld::CoarseStanding::Failed(t) => {
                    writeln!(out, "horizon: {t}").map_err(err)?;
                    break;
                }
            }
        }
    }
    Ok(())
}

/// The generation workers of the options, which the viewer's chunk stage takes its pool size from.
pub fn stage_gen(o: &Options) -> planet_streaming::GenWorkers {
    o.gen_workers()
}

/// How long chunks took to arrive: the time from the first frame that wanted a chunk to the frame its landing was seen, in seconds of the
/// scene's own clock (the caller passes the times, so the bookkeeping is pure).
#[derive(Clone, Debug, Default)]
pub struct ChunkTrace {
    first_wanted: HashMap<ChunkKey, f64>,
    waits: Vec<f64>,
    horizon_landed: u64,
    prefetch_asked: u64,
    /// For every landed chunk the flight asked for: the time it was first wanted.
    wanted_at_landing: Vec<f64>,
}

/// What a flight's [`ChunkTrace`] says.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TraceSummary {
    pub chunks_landed: u64,
    pub horizon_maps_landed: u64,
    /// Chunks the flight asked to be prefetched (STRM-042), counted once per frame they were asked in.
    pub prefetch_asked: u64,
    /// The earliest time (seconds into the flight) a chunk that landed in it was first wanted; `None` when no landed chunk was asked for in the flight. In a cold
    /// trial the model says `6 s - D / v` for the chunk past the edge (D the distance at which a chunk is wanted).
    pub first_wanted_s: Option<f64>,
    /// The median and the largest wait of a landed chunk that the flight itself asked for, seconds; 0 when none landed.
    pub median_wait_s: f64,
    pub max_wait_s: f64,
}

impl ChunkTrace {
    /// The chunks the plan of a frame at `at_s` reads: a chunk is stamped the first time.
    pub fn wanted(&mut self, keys: impl IntoIterator<Item = ChunkKey>, at_s: f64) {
        for k in keys {
            self.first_wanted.entry(k).or_insert(at_s);
        }
    }

    /// The chunks whose landing a frame at `at_s` saw. A chunk nobody asked for in this flight (landed from an earlier request) counts as landed
    /// with no wait.
    pub fn landed(&mut self, keys: &[ChunkKey], at_s: f64) {
        for k in keys {
            if let Some(t) = self.first_wanted.remove(k) {
                self.wanted_at_landing.push(t);
                self.waits.push((at_s - t).max(0.0));
            } else {
                self.waits.push(0.0);
            }
        }
    }

    /// Horizon maps that landed.
    pub fn horizon_landed(&mut self, n: usize) {
        self.horizon_landed += n as u64;
    }

    /// The chunks a frame asked to prefetch.
    pub fn prefetch(&mut self, keys: &[ChunkKey]) {
        self.prefetch_asked += keys.len() as u64;
    }

    pub fn summary(&self) -> TraceSummary {
        let mut w = self.waits.clone();
        w.sort_by(f64::total_cmp);
        TraceSummary {
            chunks_landed: w.len() as u64,
            horizon_maps_landed: self.horizon_landed,
            prefetch_asked: self.prefetch_asked,
            first_wanted_s: self.wanted_at_landing.iter().copied().min_by(f64::total_cmp),
            median_wait_s: w.get(w.len() / 2).copied().unwrap_or(0.0),
            max_wait_s: w.last().copied().unwrap_or(0.0),
        }
    }
}

// spec: STRM-042
/// The chunks a frame asks the chunk manager to prefetch: the ones `prefetch_chunks` names for the camera's predicted position, from the travel
/// estimator of the managed loop (`None` when prefetch is off); empty when the camera does not travel. The viewer and the scenes pass this to
/// `WorldTerrain::chunk_frame`, never an empty slice by default.
pub fn chunk_prefetch(
    travel: Option<&planet_frame::prefetch::TravelEstimator>,
    position: planet_core::PlanetFixed,
    radius_m: f64,
) -> Vec<ChunkKey> {
    planet_frame::chunkshare::prefetch_chunks(position, travel.and_then(|e| e.predict()), radius_m)
}

/// Whether a frame still waits for the chunk stage: a plan node reads a chunk that is not ready, or a chunk job or horizon job is in flight. A
/// scene that settles before a flight waits until this is false.
pub fn chunks_busy(stand_in_nodes: usize, chunk_jobs: usize, horizon_jobs: usize) -> bool {
    stand_in_nodes > 0 || chunk_jobs > 0 || horizon_jobs > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: TEST-055
    #[test]
    fn the_trace_measures_from_the_first_frame_that_wanted_a_chunk_to_the_frame_that_saw_it_land() {
        let mut t = ChunkTrace::default();
        t.wanted([(0, 1, 1), (0, 1, 2)], 1.0);
        t.wanted([(0, 1, 2), (0, 1, 3)], 2.0);
        t.landed(&[(0, 1, 1)], 3.5);
        t.landed(&[(0, 1, 2), (0, 9, 9)], 7.0);
        t.horizon_landed(2);
        let s = t.summary();
        assert_eq!(s.chunks_landed, 3);
        assert_eq!(s.horizon_maps_landed, 2);
        // The waits are 2.5 s, 6 s (asked at 1.0, not again at 2.0) and 0 s (nobody asked in this flight).
        assert_eq!((s.median_wait_s, s.max_wait_s), (2.5, 6.0));
        assert_eq!(s.first_wanted_s, Some(1.0), "the earliest first-wanted time of the chunks that landed");
        assert_eq!(ChunkTrace::default().summary(), TraceSummary::default());
    }

    // spec: STRM-042
    #[test]
    fn a_moving_camera_asks_for_the_next_chunk_and_a_still_or_turning_one_and_no_estimator_ask_for_none() {
        use planet_core::{cube::TangentWarp, Face, PlanetFixed, TileId};
        use planet_frame::prefetch::{PrefetchConfig, TravelEstimator};
        const R: f64 = 6_371_000.0;
        let chunk = TileId::new(Face(2), planet_generators::chunk::CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
        let (s0, s1, t0, t1) = chunk.bounds();
        let at = |s: f64| PlanetFixed(planet_core::cube::face_to_direction(&TangentWarp, Face(2), s, 0.5 * (t0 + t1)) * (R + 300.0));
        // 100 m/s east, 100 ms frames: the next chunk east is asked for within the first second.
        let mut est = TravelEstimator::new(PrefetchConfig::default(), R);
        let mut asked = Vec::new();
        for frame in 0..10u64 {
            let pos = at(s0 + (s1 - s0) * 0.1 + 100.0 * (frame as f64 * 0.1) / (R * std::f64::consts::FRAC_PI_2) * 2.0);
            est.observe(frame * 100, pos);
            asked = chunk_prefetch(Some(&est), pos, R);
        }
        assert_eq!(asked.first(), Some(&(2, 1001, 1000)), "{asked:?}");
        // Standing still: nothing. No estimator (prefetch off): nothing.
        let mut still = TravelEstimator::new(PrefetchConfig::default(), R);
        let here = at(s0 + (s1 - s0) * 0.1);
        for frame in 0..10u64 {
            still.observe(frame * 100, here);
        }
        assert!(chunk_prefetch(Some(&still), here, R).is_empty());
        assert!(chunk_prefetch(None, here, R).is_empty());
        let mut t = ChunkTrace::default();
        t.prefetch(&asked);
        assert_eq!(t.summary().prefetch_asked, asked.len() as u64);
    }

    // spec: TEST-054
    #[test]
    fn a_frame_waits_for_the_chunk_stage_while_a_node_stands_in_or_a_job_runs() {
        assert!(!chunks_busy(0, 0, 0));
        assert!(chunks_busy(1, 0, 0) && chunks_busy(0, 1, 0) && chunks_busy(0, 0, 1));
    }
}
