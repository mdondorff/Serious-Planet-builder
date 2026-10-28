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
        planet_streaming::chunks::HorizonConfig { workers, settle_ms: 1_000, keep: 32 },
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
                planet_streaming::GenWorkers::new(n).with_priority(gen.priority).pool_with_idle(1),
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
    /// When a chunk was first asked to prefetch and first seen with a job running (seconds into the flight).
    first_asked: HashMap<ChunkKey, f64>,
    first_running: HashMap<ChunkKey, f64>,
    /// The chunks that landed more than [`LATE_WAIT_S`] after they were first wanted.
    late: Vec<LateChunk>,
}

/// A chunk that was wanted and not ready for longer than this (seconds) is late.
pub const LATE_WAIT_S: f64 = 1.0;

/// A chunk that landed late, and what the trace knows of why (seconds into the flight): `asked_s` is when the prefetch first asked for it, `started_s` when a
/// job for it was first seen running; the solve is `landed_s - started_s` (an upper bound of the job's own time to the frame that saw it land).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LateChunk {
    pub key: ChunkKey,
    pub wanted_s: f64,
    pub asked_s: Option<f64>,
    pub started_s: Option<f64>,
    pub landed_s: f64,
}

impl LateChunk {
    /// How long the chunk was wanted before it landed.
    pub fn wait_s(&self) -> f64 {
        self.landed_s - self.wanted_s
    }
    /// How long its job ran before the landing was seen, when a job was seen.
    pub fn solve_s(&self) -> Option<f64> {
        self.started_s.map(|s| self.landed_s - s)
    }
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
    /// The three latest chunks of the flight (longest wait first) and how many were late.
    pub late: [Option<LateChunk>; 3],
    pub late_count: u32,
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
                if at_s - t > LATE_WAIT_S {
                    self.late.push(LateChunk {
                        key: *k,
                        wanted_s: t,
                        asked_s: self.first_asked.get(k).copied(),
                        started_s: self.first_running.get(k).copied(),
                        landed_s: at_s,
                    });
                }
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
    pub fn prefetch(&mut self, keys: &[ChunkKey], at_s: f64) {
        self.prefetch_asked += keys.len() as u64;
        for k in keys {
            self.first_asked.entry(*k).or_insert(at_s);
        }
    }

    /// The chunks whose job runs at `at_s` among `keys` (the wanted and the asked ones of the frame): the first sighting is the job's start.
    pub fn running(&mut self, keys: impl IntoIterator<Item = ChunkKey>, is_running: impl Fn(ChunkKey) -> bool, at_s: f64) {
        for k in keys {
            if !self.first_running.contains_key(&k) && is_running(k) {
                self.first_running.insert(k, at_s);
            }
        }
    }

    pub fn summary(&self) -> TraceSummary {
        let mut late = self.late.clone();
        late.sort_by(|a, b| b.wait_s().total_cmp(&a.wait_s()));
        let mut top = [None; 3];
        for (slot, l) in top.iter_mut().zip(&late) {
            *slot = Some(*l);
        }
        let mut w = self.waits.clone();
        w.sort_by(f64::total_cmp);
        TraceSummary {
            chunks_landed: w.len() as u64,
            horizon_maps_landed: self.horizon_landed,
            prefetch_asked: self.prefetch_asked,
            first_wanted_s: self.wanted_at_landing.iter().copied().min_by(f64::total_cmp),
            late: top,
            late_count: late.len() as u32,
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

/// Seconds of travel the chunk prefetch looks ahead: a chunk job takes 5 to 7 s (window 8) and its tiles a few seconds more to refresh.
pub const PREFETCH_LEAD_S: f64 = 12.0;

// spec: STRM-042
/// The chunks a frame asks the chunk manager to prefetch: those the plan would read along the camera's travel over the next [`PREFETCH_LEAD_S`]
/// (`track_chunks`, with the reach of the chunks wanted now), without the ones already wanted; empty when the camera does not travel or prefetch is
/// off (`travel` is `None`).
pub fn chunk_prefetch_track(
    travel: Option<&planet_frame::prefetch::TravelEstimator>,
    position: planet_core::PlanetFixed,
    wanted: &[(ChunkKey, f64)],
    radius_m: f64,
) -> Vec<ChunkKey> {
    let Some(velocity) = travel.and_then(|e| e.velocity()) else { return Vec::new() };
    let reach = wanted.iter().map(|w| w.1).fold(0.0, f64::max);
    let have: std::collections::HashSet<ChunkKey> = wanted.iter().map(|w| w.0).collect();
    planet_frame::chunkshare::track_chunks(position, velocity, reach, PREFETCH_LEAD_S, radius_m)
        .into_iter()
        .filter(|k| !have.contains(k))
        .collect()
}

/// The features of the per-frame chunk step a caller may turn off on purpose (TEST-058): all on by default, and a scene that turns one off says so in its log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameFeatures {
    /// The chunks the plan would read along the travel are asked for (STRM-042).
    pub prefetch: bool,
    /// The horizon jobs of the chunks the plan reads run (STRM-048).
    pub horizon: bool,
    /// The stale chunks wait while the tiles of an edit are being built (STRM-056).
    pub stale_hold: bool,
}

impl Default for FrameFeatures {
    fn default() -> FrameFeatures {
        FrameFeatures { prefetch: true, horizon: true, stale_hold: true }
    }
}

/// What the chunk step did over a run, one counter per feature (TEST-058).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeatureCounters {
    /// Steps run.
    pub frames: u64,
    /// Steps that asked for at least one chunk to prefetch, and the chunks asked (summed over the steps).
    pub prefetch_frames: u64,
    pub prefetch_asked: u64,
    /// Chunks that landed and chunks evicted.
    pub chunks_landed: u64,
    pub chunks_evicted: u64,
    /// Steps that ran the horizon jobs, and the maps that landed.
    pub horizon_frames: u64,
    pub horizon_landed: u64,
    /// Steps in which the stale chunks were told to wait.
    pub stale_hold_frames: u64,
    /// Item changes announced to the plan cache (STRM-034).
    pub plan_cache_announcements: u64,
}

/// Which features a scene expects to have acted over its run (TEST-058); a feature the scene turned off is never checked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Expect {
    /// Chunks were asked for ahead of a travelling camera.
    pub prefetch: bool,
    /// The horizon step ran.
    pub horizon: bool,
    /// The stale chunks were held while an edit's tiles were built.
    pub stale_hold: bool,
    /// An item change was announced to the plan cache.
    pub announcements: bool,
}

impl FeatureCounters {
    /// The counters as one log line; a feature the scene turned off is named `off` in place of its count (TEST-058).
    pub fn line(&self, features: &FrameFeatures) -> String {
        let on = |enabled: bool, text: String| if enabled { text } else { "off".to_string() };
        format!(
            "frames {}, prefetch {}, chunks landed {} evicted {}, horizon {}, stale hold {}, plan-cache announcements {}",
            self.frames,
            on(features.prefetch, format!("{} asked in {} frames", self.prefetch_asked, self.prefetch_frames)),
            self.chunks_landed,
            self.chunks_evicted,
            on(features.horizon, format!("{} frames, {} maps landed", self.horizon_frames, self.horizon_landed)),
            on(features.stale_hold, format!("{} frames", self.stale_hold_frames)),
            self.plan_cache_announcements
        )
    }

    /// The counters as a JSON object; a feature turned off is `"off"` (TEST-058).
    pub fn json(&self, features: &FrameFeatures) -> String {
        let on = |enabled: bool, value: String| if enabled { value } else { "\"off\"".to_string() };
        format!(
            "{{\"frames\": {}, \"prefetch_asked\": {}, \"prefetch_frames\": {}, \"chunks_landed\": {}, \"chunks_evicted\": {}, \"horizon_frames\": {}, \"horizon_landed\": {}, \"stale_hold_frames\": {}, \"plan_cache_announcements\": {}}}",
            self.frames,
            on(features.prefetch, self.prefetch_asked.to_string()),
            on(features.prefetch, self.prefetch_frames.to_string()),
            self.chunks_landed,
            self.chunks_evicted,
            on(features.horizon, self.horizon_frames.to_string()),
            on(features.horizon, self.horizon_landed.to_string()),
            on(features.stale_hold, self.stale_hold_frames.to_string()),
            self.plan_cache_announcements
        )
    }

    /// A scene fails when a feature it enabled and expects to act counted 0 (TEST-058): the message names the feature and the counter table.
    pub fn check(&self, features: &FrameFeatures, expect: &Expect) -> Result<(), String> {
        let missing: Vec<&str> = [
            ("prefetch", features.prefetch && expect.prefetch && self.prefetch_asked == 0),
            ("horizon", features.horizon && expect.horizon && self.horizon_frames == 0),
            ("stale hold", features.stale_hold && expect.stale_hold && self.stale_hold_frames == 0),
            ("plan-cache announcements", expect.announcements && self.plan_cache_announcements == 0),
        ]
        .into_iter()
        .filter_map(|(name, bad)| bad.then_some(name))
        .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!("an enabled feature counted 0: {} (TEST-058); counters: {}", missing.join(", "), self.line(features)))
        }
    }
}

/// What one step reported: the chunk update, the chunks asked to prefetch and the horizon update.
#[derive(Clone, Debug, Default)]
pub struct StepReport {
    pub update: planet_streaming::chunks::ChunkUpdate,
    pub asked: Vec<ChunkKey>,
    pub horizon: planet_streaming::chunks::HorizonUpdate,
}

/// What the chunk step needs of the managed loop (the real one, or a test's).
pub trait StepLoop {
    /// Whether the tiles of an edit are still being built, so the stale chunks should wait (STRM-056).
    fn edit_tiles_pending(&self) -> bool;
    /// The travel estimator of the loop's prefetch source, if prefetch is on.
    fn travel(&self) -> Option<&planet_frame::prefetch::TravelEstimator>;
    fn chunk_landed(&mut self, landed: &[ChunkKey]);
    fn chunk_evicted(&mut self, evicted: &[ChunkKey]);
}

impl StepLoop for planet_frame::managed::ManagedLoop {
    fn edit_tiles_pending(&self) -> bool {
        !self.pending().is_empty() || self.manager().streamer().unfinished_jobs() > 0
    }
    fn travel(&self) -> Option<&planet_frame::prefetch::TravelEstimator> {
        planet_frame::managed::ManagedLoop::travel(self)
    }
    fn chunk_landed(&mut self, landed: &[ChunkKey]) {
        planet_frame::managed::ManagedLoop::chunk_landed(self, landed);
    }
    fn chunk_evicted(&mut self, evicted: &[ChunkKey]) {
        planet_frame::managed::ManagedLoop::chunk_evicted(self, evicted);
    }
}

/// A loop that is not a managed loop (a settling loop of a still or a measurement): it has no travel, no pending edit and takes the landed chunks as they come.
#[derive(Default)]
pub struct NoLoop;

impl StepLoop for NoLoop {
    fn edit_tiles_pending(&self) -> bool {
        false
    }
    fn travel(&self) -> Option<&planet_frame::prefetch::TravelEstimator> {
        None
    }
    fn chunk_landed(&mut self, _: &[ChunkKey]) {}
    fn chunk_evicted(&mut self, _: &[ChunkKey]) {}
}

/// The features of a settling loop that is not a frame: no prefetch (the camera does not travel), no stale hold, and the horizon jobs when `horizon`.
pub fn settling_features(horizon: bool) -> FrameFeatures {
    FrameFeatures { prefetch: false, horizon, stale_hold: false }
}

/// Tell the plan cache where the items changed from `old` to the world's definition now, as the viewer does after a swap (STRM-034), so the plan after
/// the edit keeps what the change cannot reach; `old_revision` is the item revision of the field before the swap.
pub fn announce_item_change(
    managed: &mut planet_frame::managed::ManagedLoop,
    world: &WorldTerrain,
    old: &planet_world_def::WorldDefinition,
    old_revision: u64,
    counters: &mut FeatureCounters,
) {
    if let Ok(field) = world.field() {
        let changes = world
            .definition()
            .item_changes_from(old)
            .into_iter()
            .map(|(dir, reach_m)| planet_frame::lod::ItemChange { dir, reach_m })
            .collect();
        managed.note_item_change(old_revision, field.item_revision(), changes);
        counters.plan_cache_announcements += 1;
    }
}

/// The per-frame chunk step of the viewer and of every scene that drives chunks (STRM-039, STRM-040, STRM-042, STRM-048, STRM-056, TEST-058): the stale
/// hold, the chunks the plan's `tiles` read with the prefetch along the travel, the landed and evicted chunks to the managed loop, and the horizon jobs.
/// Called after the managed frame of `camera`; the caller does what is its own (the viewer uploads the horizon maps of the report, a scene counts).
pub fn frame_step(
    world: &WorldTerrain,
    managed: &mut impl StepLoop,
    tiles: &[planet_core::TileId],
    camera: planet_core::PlanetFixed,
    features: &FrameFeatures,
    counters: &mut FeatureCounters,
) -> StepReport {
    // The stale chunks wait while the tiles of an edit are still being built (STRM-056), at most the cap.
    let hold = features.stale_hold && managed.edit_tiles_pending();
    world.hold_stale_chunks(hold);
    let (travel, radius_m) = (StepLoop::travel(&*managed), world.radius_m());
    let (update, asked) = world.chunk_frame_with(tiles, camera, |wanted| {
        if features.prefetch {
            chunk_prefetch_track(travel, camera, wanted, radius_m)
        } else {
            Vec::new()
        }
    });
    StepLoop::chunk_landed(managed, &update.landed);
    StepLoop::chunk_evicted(managed, &update.evicted);
    let horizon = if features.horizon { world.horizon_frame(tiles, camera) } else { Default::default() };
    counters.frames += 1;
    counters.prefetch_frames += u64::from(!asked.is_empty());
    counters.prefetch_asked += asked.len() as u64;
    counters.chunks_landed += update.landed.len() as u64;
    counters.chunks_evicted += update.evicted.len() as u64;
    counters.horizon_frames += u64::from(features.horizon);
    counters.horizon_landed += horizon.landed.len() as u64;
    counters.stale_hold_frames += u64::from(hold);
    StepReport { update, asked, horizon }
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
    // spec: TEST-058
    #[test]
    fn a_scene_fails_naming_the_feature_when_an_enabled_feature_it_expects_counted_zero_and_a_feature_turned_off_is_named_off() {
        let all = FrameFeatures::default();
        let expect = Expect { prefetch: true, horizon: true, stale_hold: true, announcements: true };
        let acted = FeatureCounters {
            frames: 10,
            prefetch_asked: 3,
            prefetch_frames: 2,
            horizon_frames: 10,
            stale_hold_frames: 4,
            plan_cache_announcements: 1,
            ..Default::default()
        };
        assert_eq!(acted.check(&all, &expect), Ok(()));
        let err = FeatureCounters { prefetch_asked: 0, plan_cache_announcements: 0, ..acted }.check(&all, &expect).unwrap_err();
        assert!(err.contains("prefetch, plan-cache announcements") && err.contains("frames 10"), "{err}");
        // Turned off: not checked, and the log says so.
        let off = FrameFeatures { prefetch: false, ..all };
        let zero = FeatureCounters { prefetch_asked: 0, ..acted };
        assert_eq!(zero.check(&off, &expect), Ok(()));
        assert!(zero.line(&off).contains("prefetch off") && zero.json(&off).contains("\"prefetch_asked\": \"off\""));
        assert!(acted.line(&all).contains("prefetch 3 asked in 2 frames"));
    }

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
        t.prefetch(&asked, 0.0);
        assert_eq!(t.summary().prefetch_asked, asked.len() as u64);
    }

    // spec: STRM-042
    #[test]
    fn the_prefetch_follows_the_travel_with_the_reach_of_the_wanted_chunks_and_leaves_out_the_wanted_ones() {
        use planet_core::{cube::TangentWarp, Face, PlanetFixed, TileId};
        use planet_frame::prefetch::{PrefetchConfig, TravelEstimator};
        const R: f64 = 6_371_000.0;
        let chunk = TileId::new(Face(2), planet_generators::chunk::CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
        let (s0, s1, t0, t1) = chunk.bounds();
        let at = |s: f64| PlanetFixed(planet_core::cube::face_to_direction(&TangentWarp, Face(2), s, 0.5 * (t0 + t1)) * (R + 300.0));
        let mut est = TravelEstimator::new(PrefetchConfig::default(), R);
        let mut pos = at(s0);
        for frame in 0..10u64 {
            pos = at(s0 + (s1 - s0) * 0.1 + 160.0 * (frame as f64 * 0.1) / (R * std::f64::consts::FRAC_PI_2) * 2.0);
            est.observe(frame * 100, pos);
        }
        let edge_m = R * std::f64::consts::FRAC_PI_2 / f64::from(1u32 << planet_generators::chunk::CHUNK_GRID_LEVEL);
        // The plan reads the chunk under the camera and its western neighbour, 1.1 edges away: the reach follows.
        let wanted = [((2, 1000, 1000), 0.2 * edge_m), ((2, 999, 1000), 1.1 * edge_m)];
        let ahead = chunk_prefetch_track(Some(&est), pos, &wanted, R);
        assert!(ahead.contains(&(2, 1001, 1000)), "the chunk across the edge: {ahead:?}");
        assert!(wanted.iter().all(|w| !ahead.contains(&w.0)), "wanted chunks are not asked twice: {ahead:?}");
        // No estimator, or a camera that stands still: nothing.
        assert!(chunk_prefetch_track(None, pos, &wanted, R).is_empty());
        let mut still = TravelEstimator::new(PrefetchConfig::default(), R);
        for frame in 0..10u64 {
            still.observe(frame * 100, pos);
        }
        assert!(chunk_prefetch_track(Some(&still), pos, &wanted, R).is_empty());
    }

    // spec: TEST-059
    #[test]
    fn a_chunk_that_lands_more_than_a_second_after_it_was_wanted_is_late_and_says_when_it_was_asked_and_started() {
        let mut t = ChunkTrace::default();
        let (slow, quick) = ((4u8, 799u32, 9u32), (4u8, 800u32, 9u32));
        t.wanted([slow, quick], 1.0);
        t.prefetch(&[slow], 0.5);
        t.running([slow, quick], |k| k == slow, 1.5);
        t.running([slow], |_| true, 2.0);
        t.landed(&[quick], 1.4);
        t.landed(&[slow], 9.0);
        let s = t.summary();
        assert_eq!(s.late_count, 1, "only the chunk that waited more than a second is late");
        let l = s.late[0].expect("the late chunk");
        assert_eq!((l.key, l.wanted_s, l.asked_s, l.started_s, l.landed_s), (slow, 1.0, Some(0.5), Some(1.5), 9.0));
        assert_eq!((l.wait_s(), l.solve_s()), (8.0, Some(7.5)));
        assert!(s.late[1].is_none());
    }

    // spec: TEST-054
    #[test]
    fn a_frame_waits_for_the_chunk_stage_while_a_node_stands_in_or_a_job_runs() {
        assert!(!chunks_busy(0, 0, 0));
        assert!(chunks_busy(1, 0, 0) && chunks_busy(0, 1, 0) && chunks_busy(0, 0, 1));
    }
}
