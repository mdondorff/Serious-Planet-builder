//! The speed-envelope scene (M8-V, TEST-047, M8 exit criterion 7): level flight over ground that is not in the cache, at a constant speed,
//! at fixed altitudes, and the highest speed at which the share of the screen drawn from a tile coarser than the plan wanted (or from none,
//! TEST-046) stays within a bound over every window of a second. The search, the window and the log are pure; the trial runs the real
//! streaming rig. Never run by an agent on the reference machine: the window is the architect's.

use crate::pipelined::Rig;
use crate::streamperf::Driver;
use crate::worldfield::WorldTerrain;
use crate::{err, Options};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_frame::chunkshare::{ChunkShare, ChunkView};
use planet_frame::scenes::{lag_post_pass, level_flight_cameras, FrameDraw, FRAME_STEP_S, SETTLE_S, SITE_LAT_DEG, SITE_LON_DEG};
use planet_frame::{Camera, LodParams, TerrainRequest, TerrainView};
use planet_render::GpuContext;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// The scene's `--scene` name.
pub const SCENE: &str = "envelope";

pub fn is_envelope_scene(name: &str) -> bool {
    name == SCENE
}

/// The altitude that is flown once more without the horizon jobs, for the comparison line of the log (TEST-054).
pub const COMPARISON_ALTITUDE_M: f64 = 300.0;
/// Altitudes above the ground the scene flies at, metres.
pub const ALTITUDES_M: [f64; 4] = [30.0, 100.0, 300.0, 1000.0];
/// The default bound of the coarse share over a window (`--share-bound`): 1 % of the screen.
pub const DEFAULT_BOUND: f64 = 0.01;
/// The window the bound is judged over, seconds.
pub const WINDOW_S: f64 = 1.0;
/// The first speed of a search, and the cap it never goes beyond, metres per second.
pub const START_SPEED_MPS: f64 = 10.0;
pub const CAP_SPEED_MPS: f64 = 3000.0;
/// Halvings of the gap between the highest passing and the lowest failing speed.
pub const BISECTIONS: usize = 4;
/// Seconds of flight of a trial (the settled start excluded) and of a smoke trial.
pub const FLIGHT_S: f64 = 12.0;
pub const SMOKE_FLIGHT_S: f64 = 0.5;
/// Degrees of latitude between the start places of consecutive trials (about 67 km, more than a trial flies at the cap: 36 km).
const PLACE_STEP_DEG: f64 = 0.6;
/// Seconds a steady-flight trial flies at the speed under test, with the prefetch active, before its measured window starts: the job latency of a
/// chunk (5 to 7 s, window 8) and 5 s more. The window is [`FLIGHT_S`] long and has the chunk edge half way, over ground the run-in did not cover.
pub const RUN_IN_S: f64 = 12.0;
pub const SMOKE_RUN_IN_S: f64 = 0.25;
/// Seconds the settle before a trial may take at most.
const SETTLE_LIMIT_S: f64 = 900.0;

/// The largest mean of `shares` (one per frame of `step_s`) over any run of frames that spans `window_s`; a series shorter than the window
/// gives its mean, an empty one 0. A non-finite share makes the result NaN, which no bound accepts.
pub fn worst_window(shares: &[f64], step_s: f64, window_s: f64) -> f64 {
    if shares.is_empty() {
        return 0.0;
    }
    let w = ((window_s / step_s).round() as usize).clamp(1, shares.len());
    let mut sum: f64 = shares[..w].iter().sum();
    let mut worst = sum;
    for k in w..shares.len() {
        sum += shares[k] - shares[k - w];
        worst = worst.max(sum);
    }
    if sum.is_nan() || shares.iter().any(|s| !s.is_finite()) {
        return f64::NAN;
    }
    worst / w as f64
}

/// What one flight measured: the worst 1 s window of the pixel-weighted coarse share (TEST-046) and of the node-weighted chunk stand-in
/// fraction (STRM-039), and the chunk nodes the flight counted. The bound is judged on the larger of the two shares.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub coarse_share: f64,
    /// `None`: the world has no chunk field to count against.
    pub stand_in_share: Option<f64>,
    /// Sums over the frames of the flight: plan nodes at or finer than the pyramid top level, and those whose own chunk was not ready.
    pub chunk_nodes: u64,
    pub chunk_stand_in: u64,
    /// What the chunk stage did during the flight (TEST-054); `None` without a chunk stage.
    pub stage: Option<StageFacts>,
}

/// What the chunk stage did during one flight: the landings and their waits, and the jobs still in flight at its end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageFacts {
    pub trace: crate::chunkstage::TraceSummary,
    pub chunk_jobs_at_end: usize,
    pub horizon_jobs_at_end: usize,
}

impl Measure {
    /// A measure of a world without a chunk field.
    pub fn coarse_only(coarse_share: f64) -> Measure {
        Measure { coarse_share, stand_in_share: None, chunk_nodes: 0, chunk_stand_in: 0, stage: None }
    }

    /// The same measure with what the chunk stage did.
    pub fn with_stage(mut self, stage: Option<StageFacts>) -> Measure {
        self.stage = stage;
        self
    }

    /// The share the bound is judged on: the larger of the two; NaN (missing data) when either is, which no bound accepts.
    pub fn judged(&self) -> f64 {
        match self.stand_in_share {
            Some(s) if s.is_nan() || self.coarse_share.is_nan() => f64::NAN,
            Some(s) => s.max(self.coarse_share),
            None => self.coarse_share,
        }
    }
}

/// One trial of a search: the speed and what the flight measured at it; `share` is [`Measure::judged`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trial {
    pub speed_mps: f64,
    pub share: f64,
    pub measure: Measure,
}

/// The result of the search at one altitude.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub altitude_m: f64,
    /// The highest speed that held the bound; `None` when the first trial failed already.
    pub speed_mps: Option<f64>,
    /// The worst window's share at that speed (at the first trial's speed when there is none).
    pub share: f64,
    /// The cap was reached and held: the speed is a lower bound.
    pub capped: bool,
    /// Every trial, in the order they ran.
    pub trials: Vec<Trial>,
}

/// Find the highest speed at `altitude_m` whose worst window stays within `bound`: double from [`START_SPEED_MPS`] while a trial holds, up
/// to [`CAP_SPEED_MPS`], then bisect [`BISECTIONS`] times between the highest holding and the lowest failing speed. `trial` runs one flight
/// at a speed and returns what it measured; the judged share is taken to rise with the speed.
pub fn search(altitude_m: f64, bound: f64, mut trial: impl FnMut(f64) -> Result<Measure, String>) -> Result<Point, String> {
    let mut trials: Vec<Trial> = Vec::new();
    let mut run = |speed: f64, trials: &mut Vec<Trial>| -> Result<bool, String> {
        let measure = trial(speed)?;
        let share = measure.judged();
        trials.push(Trial { speed_mps: speed, share, measure });
        Ok(share <= bound)
    };
    let (mut best, mut failed): (Option<Trial>, Option<f64>) = (None, None);
    let mut capped = false;
    let mut v = START_SPEED_MPS;
    loop {
        if run(v, &mut trials)? {
            best = trials.last().copied();
            if v >= CAP_SPEED_MPS {
                capped = true;
                break;
            }
            v = (v * 2.0).min(CAP_SPEED_MPS);
        } else {
            failed = Some(v);
            break;
        }
    }
    if let (Some(lo), Some(hi)) = (best, failed) {
        let (mut lo, mut hi) = (lo, hi);
        for _ in 0..BISECTIONS {
            let mid = 0.5 * (lo.speed_mps + hi);
            if run(mid, &mut trials)? {
                lo = *trials.last().expect("a trial just ran");
            } else {
                hi = mid;
            }
        }
        best = Some(lo);
    }
    Ok(Point {
        altitude_m,
        speed_mps: best.map(|t| t.speed_mps),
        share: best.map_or_else(|| trials.first().map_or(0.0, |t| t.share), |t| t.share),
        capped,
        trials,
    })
}

/// The one-line summary of a point.
pub fn summary_line(p: &Point) -> String {
    match p.speed_mps {
        Some(v) => format!(
            "envelope {:>5.0} m: highest speed {:.0} m/s{} (worst 1 s window {:.3} % of the screen coarse, {} trials)",
            p.altitude_m,
            v,
            if p.capped { " or more (the cap)" } else { "" },
            100.0 * p.share,
            p.trials.len()
        ),
        None => format!(
            "envelope {:>5.0} m: no speed holds the bound, not even {START_SPEED_MPS:.0} m/s (worst 1 s window {:.3} %)",
            p.altitude_m,
            100.0 * p.share
        ),
    }
}

/// What the log says besides the points.
pub struct LogFacts<'a> {
    pub bound: f64,
    pub source: &'a str,
    pub adapter: &'a str,
    pub smoke: bool,
    pub owner_override: &'a [String],
    /// Whether the scene counted against a chunk field; when not, the log says why `stand_in_share` is `null`.
    pub chunk_field: bool,
    /// Whether the curve ran with the horizon jobs (REND-038) and, with a chunk stage, the comparison point of one altitude without them.
    pub horizon: bool,
    pub horizon_off: Option<&'a Point>,
    /// With a chunk stage, where along each flight the start put a chunk edge (0.5: half way); `None` without one.
    pub edge_fraction: Option<f64>,
    /// The steady-flight curve (a run-in before the window, `run_in_s` long) next to the cold one; `None` without a chunk stage.
    pub steady: Option<(&'a [Point], f64)>,
}

/// Why the chunk stand-in share is `null` in a log of a world without a chunk field.
pub const NO_CHUNK_FIELD: &str =
    "the world of this scene has no stored coupled run (it is not a `--store` world): no chunk stage, only the coarse share is judged";

fn number(x: f64) -> String {
    if x.is_finite() {
        format!("{x}")
    } else {
        "null".to_string()
    }
}

/// The `curve` entries of a log: one line per point.
fn curve_json(points: &[Point]) -> String {
    let mut s = String::new();
    let opt = |x: Option<f64>| x.map_or("null".to_string(), number);
    for (i, p) in points.iter().enumerate() {
        let trials: Vec<String> = p
            .trials
            .iter()
            .map(|t| {
                format!(
                    "{{\"speed_mps\": {}, \"share\": {}, \"coarse_share\": {}, \"stand_in_share\": {}, \"chunk_nodes\": {}, \"chunk_stand_in\": {}, \"stage\": {}}}",
                    number(t.speed_mps),
                    number(t.share),
                    number(t.measure.coarse_share),
                    opt(t.measure.stand_in_share),
                    t.measure.chunk_nodes,
                    t.measure.chunk_stand_in,
                    stage_json(t.measure.stage)
                )
            })
            .collect();
        // The shares of the flight at the highest holding speed.
        let at = p.speed_mps.and_then(|v| p.trials.iter().find(|t| t.speed_mps == v)).map(|t| t.measure);
        s += &format!(
            "    {{\"altitude_m\": {}, \"highest_speed_mps\": {}, \"share_at_speed\": {}, \"coarse_share\": {}, \"stand_in_share\": {}, \"capped\": {}, \"trials\": [{}]}}{}\n",
            number(p.altitude_m),
            p.speed_mps.map_or("null".to_string(), number),
            number(p.share),
            opt(at.map(|m| m.coarse_share)),
            opt(at.and_then(|m| m.stand_in_share)),
            p.capped,
            trials.join(", "),
            if i + 1 < points.len() { "," } else { "" }
        );
    }
    s
}

/// What the chunk stage did during a flight, as a log object (`null` without a stage).
fn stage_json(stage: Option<StageFacts>) -> String {
    match stage {
        None => "null".to_string(),
        Some(f) => format!(
            "{{\"chunks_landed\": {}, \"horizon_maps_landed\": {}, \"prefetch_asked\": {}, \"first_wanted_s\": {}, \"median_wait_s\": {}, \"max_wait_s\": {}, \"chunk_jobs_at_end\": {}, \"horizon_jobs_at_end\": {}}}",
            f.trace.chunks_landed,
            f.trace.horizon_maps_landed,
            f.trace.prefetch_asked,
            f.trace.first_wanted_s.map_or("null".to_string(), number),
            number(f.trace.median_wait_s),
            number(f.trace.max_wait_s),
            f.chunk_jobs_at_end,
            f.horizon_jobs_at_end
        ),
    }
}

/// The log text (`perf/logs/envelope.json`): the curve and everything that fixes how it was measured.
pub fn log_json(points: &[Point], facts: &LogFacts<'_>) -> String {
    let mut s = String::from("{\n");
    s += "  \"scene\": \"envelope\",\n";
    s += &format!("  \"adapter\": \"{}\",\n", facts.adapter.replace('"', "'"));
    s += &format!("  \"source\": \"{}\",\n", facts.source.replace('"', "'"));
    s += &format!("  \"smoke\": {},\n", facts.smoke);
    s += &format!("  \"owner_override\": {:?},\n", facts.owner_override);
    s += &format!("  \"bound\": {},\n  \"window_s\": {WINDOW_S},\n  \"frame_step_s\": {FRAME_STEP_S},\n", number(facts.bound));
    s += &format!("  \"start_speed_mps\": {START_SPEED_MPS},\n  \"cap_speed_mps\": {CAP_SPEED_MPS},\n  \"bisections\": {BISECTIONS},\n");
    s += "  \"judged\": \"the larger of coarse_share and stand_in_share, the worst 1 s window\",\n";
    s += "  \"coarse_weighting\": \"pixels: nodes of the fresh plan weighted by their projected screen pixels (TEST-046)\",\n";
    s += "  \"stand_in_weighting\": \"nodes: unweighted share of the plan nodes at or finer than the pyramid top level whose chunk is not ready (STRM-039)\",\n";
    s += &format!("  \"chunk_field\": {},\n", facts.chunk_field);
    s += &format!("  \"horizon\": {},\n", facts.chunk_field && facts.horizon);
    s += &format!("  \"edge_fraction\": {},\n", facts.edge_fraction.map_or("null".to_string(), number));
    if !facts.chunk_field {
        s += &format!("  \"stand_in_reason\": {NO_CHUNK_FIELD:?},\n");
    }
    s += "  \"curve\": [\n";
    s += &curve_json(points);
    s += "  ],\n";
    if let Some((steady, run_in_s)) = facts.steady {
        s += &format!("  \"run_in_s\": {},\n  \"steady_curve\": [\n{}  ],\n", number(run_in_s), curve_json(steady));
    }
    if let Some(p) = facts.horizon_off {
        s += &format!("  \"horizon_off_comparison\": [\n{}  ],\n", curve_json(std::slice::from_ref(p)));
    }
    s += "  \"summary\": [\n";
    for (i, p) in points.iter().enumerate() {
        s += &format!("    {:?}{}\n", summary_line(p), if i + 1 < points.len() { "," } else { "" });
    }
    s += "  ]\n}\n";
    s
}

/// Where along a flight with a chunk stage the start puts a chunk edge: half way (TEST-054).
pub const EDGE_FRACTION: f64 = 0.5;

/// The key of the chunk that holds the point (`lat_deg`, `lon_deg`).
pub fn chunk_key_at(lat_deg: f64, lon_deg: f64) -> (u8, u32, u32) {
    let d = planet_core::Vec3::from_lat_lon(lat_deg.to_radians(), lon_deg.to_radians());
    let t = planet_core::TileId::from_direction(&TangentWarp, d, planet_generators::chunk::CHUNK_GRID_LEVEL);
    (t.face().0, t.x(), t.y())
}

/// The latitude to start a flight due north of (`lat0_deg`, `lon_deg`) so that it crosses the first chunk edge north of `lat0_deg` at `fraction` of its
/// length `flight_deg` (TEST-054): every flight then has fresh chunks ahead of the eye, the worst phase, and the curve is a floor that does not depend on
/// where an edge happens to fall. `key_at` gives the chunk key of a point; the edge is found by walking north in steps of at most a quarter of the
/// flight and bisecting to a micro-degree. No edge within 0.2 degrees (22 km) leaves the start where it is.
pub fn straddling_start_lat(
    key_at: &dyn Fn(f64, f64) -> (u8, u32, u32),
    lat0_deg: f64,
    lon_deg: f64,
    flight_deg: f64,
    fraction: f64,
) -> f64 {
    let first = key_at(lat0_deg, lon_deg);
    let step = (flight_deg / 4.0).clamp(0.001, 0.05);
    let (mut below, mut above) = (lat0_deg, lat0_deg + step);
    while key_at(above, lon_deg) == first {
        below = above;
        above = below + step;
        if above - lat0_deg > 0.2 {
            return lat0_deg;
        }
    }
    for _ in 0..40 {
        let mid = 0.5 * (below + above);
        if key_at(mid, lon_deg) == first {
            below = mid;
        } else {
            above = mid;
        }
    }
    above - fraction * flight_deg
}

/// The place trial `n` starts from: [`start_place`], moved north so that the flight (`speed_mps` for `frames` frames) crosses a chunk edge half way when
/// the scene runs the chunk stage.
pub fn trial_start(n: usize, speed_mps: f64, frames: usize, radius_m: f64, chunks: bool) -> (f64, f64) {
    trial_start_at(n, speed_mps, frames, radius_m, chunks, EDGE_FRACTION)
}

/// [`trial_start`] with the chunk edge at `edge_fraction` of the flight (a steady-flight trial puts it half way into the measured window, after the run-in).
pub fn trial_start_at(n: usize, speed_mps: f64, frames: usize, radius_m: f64, chunks: bool, edge_fraction: f64) -> (f64, f64) {
    let (lat, lon) = start_place(n);
    if !chunks {
        return (lat, lon);
    }
    let flight_m = speed_mps * frames.saturating_sub(1) as f64 * FRAME_STEP_S;
    let flight_deg = (flight_m / radius_m).to_degrees();
    (straddling_start_lat(&chunk_key_at, lat, lon, flight_deg, edge_fraction), lon)
}

/// The start place of trial `n` (counted over the whole run): the scene's site moved north by [`PLACE_STEP_DEG`] a trial, so no two
/// trials fly over the same ground.
pub fn start_place(n: usize) -> (f64, f64) {
    (SITE_LAT_DEG + PLACE_STEP_DEG * n as f64, SITE_LON_DEG)
}

/// The chunk stage a flight runs (TEST-054): the generation workers the viewer's pool size comes from (`anetworld::viewer_chunk_stage`) and whether the
/// horizon jobs run.
#[derive(Clone, Copy, Debug)]
struct StageSetup {
    gen: planet_streaming::GenWorkers,
    horizon: bool,
}

/// One flight on the real streaming rig from an empty tile cache: settle at the start view, fly `cameras`, and return the coarse share of
/// every frame, judged afterwards against the fresh plan of its own camera (off the timed path, like the detail lag).
#[allow(clippy::too_many_arguments)]
fn fly(
    ctx: &GpuContext,
    world: &Arc<WorldTerrain>,
    req_of: &(dyn Fn(Camera) -> TerrainRequest + Sync),
    cameras: &[Camera],
    run_in: usize,
    smoke: bool,
    gen: planet_streaming::GenWorkers,
    stage: Option<StageSetup>,
    out: &mut dyn Write,
) -> Result<Measure, String> {
    // A flight starts with a chunk stage of its own and nothing resident: no disk cache, an empty chunk cache (TEST-054).
    if let Some(st) = stage {
        crate::chunkstage::enable_viewer_stage(world, st.gen, st.horizon, false, out)?;
    }
    let chunk_view = chunk_view_of(world);
    let first = req_of(cameras[0]);
    let rig = Rig::new(ctx, world, &first, gen)?;
    let mut d = Driver::plain(ctx, rig);
    d.set_chunk_view(chunk_view.clone());
    if let Some(st) = stage {
        d.drive_chunks(world.clone(), st.horizon);
    }
    let started = Instant::now();
    let need = (SETTLE_S / FRAME_STEP_S).round() as usize;
    let (mut quiet, mut frames) = (0usize, 0usize);
    loop {
        let t = d.tick(&first)?;
        frames += 1;
        quiet = if t.settled && !t.chunk_busy { quiet + 1 } else { 0 };
        if (smoke && frames >= 6) || quiet >= need {
            break;
        }
        if started.elapsed().as_secs_f64() > SETTLE_LIMIT_S {
            return Err(format!("the start view did not settle within {SETTLE_LIMIT_S} s"));
        }
    }
    // The run-in: the first `run_in` cameras are flown with the chunk stage and the prefetch running and are not judged; the window follows.
    for cam in &cameras[..run_in] {
        d.tick(&req_of(*cam))?;
    }
    let cameras = &cameras[run_in..];
    d.restart_pacing();
    d.restart_chunk_trace();
    let mut draws = Vec::with_capacity(cameras.len());
    let mut chunks = Vec::with_capacity(cameras.len());
    for cam in cameras {
        let t = d.tick(&req_of(*cam))?;
        draws.push(FrameDraw { camera: Some(*cam), drawn: t.drawn });
        chunks.push(t.chunk);
    }
    let stage_facts = d.chunk_summary().map(|(trace, chunk_jobs_at_end, horizon_jobs_at_end)| StageFacts {
        trace,
        chunk_jobs_at_end,
        horizon_jobs_at_end,
    });
    drop(d);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let lags = lag_post_pass(&draws, req_of, &|| world.field(), threads)?;
    let coarse: Vec<f64> = lags.iter().map(|l| l.coarse_share).collect();
    Ok(measure_of(&coarse, chunk_view.is_some().then_some(chunks.as_slice())).with_stage(stage_facts))
}

/// The chunk field the scene counts its stand-in share against (STRM-039): the world's, `None` while the app builds no chunk manager for a
/// world. The one place to give the scene a [`ManagerView`] when a world gets a chunk field; the log says it had none.
fn chunk_view_of(world: &WorldTerrain) -> Option<Arc<dyn ChunkView>> {
    world.chunk_view()
}

/// The measure of a flight from the per-frame coarse shares and, with a chunk field, the per-frame chunk shares: each judged as its worst
/// window of [`WINDOW_S`] (the stand-in on the node-weighted fraction), the chunk node counts summed over the frames.
pub fn measure_of(coarse: &[f64], chunk: Option<&[ChunkShare]>) -> Measure {
    let coarse_share = worst_window(coarse, FRAME_STEP_S, WINDOW_S);
    match chunk {
        None => Measure::coarse_only(coarse_share),
        Some(frames) => {
            let fractions: Vec<f64> = frames.iter().map(ChunkShare::fraction).collect();
            Measure {
                coarse_share,
                stand_in_share: Some(worst_window(&fractions, FRAME_STEP_S, WINDOW_S)),
                chunk_nodes: frames.iter().map(|f| f.chunk_nodes as u64).sum(),
                chunk_stand_in: frames.iter().map(|f| f.chunk_stand_in as u64).sum(),
                stage: None,
            }
        }
    }
}

/// `test-render --perf --scene envelope` (TEST-047): the curve of the highest speed per altitude, written to `perf/logs/envelope.json`.
/// `--smoke` (`--perf-smoke`) runs one short trial per altitude on the software adapter and measures nothing.
pub fn perf_envelope(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    let smoke = o.perf_smoke;
    crate::streamperf::check_options(o)?;
    if !(o.share_bound > 0.0 && o.share_bound < 1.0) {
        return Err(format!("--share-bound {}: a share of the screen strictly between 0 and 1", o.share_bound));
    }
    let opened = Instant::now();
    let world = Arc::new(crate::streamperf::open_world(o, false)?);
    writeln!(out, "world opened in {:.1} s", opened.elapsed().as_secs_f64()).map_err(err)?;
    let radius_m = world.radius_m();
    let mut owner_override: Vec<String> = Vec::new();
    if !smoke {
        planet_perf::check_reference_adapter(&crate::facts(ctx)).map_err(|r| r.to_string())?;
        owner_override = crate::readiness_gate(o, ctx, out)?;
    }
    let size = if smoke { (640u32, 360u32) } else { (2560u32, 1440u32) };
    // The same viewport-to-tolerance ratio as the real run (240 at the defaults): the plan's steepness check depends on it.
    let tau = if smoke { o.tau * f64::from(size.1) / 1440.0 } else { o.tau };
    // With a stored world the scene runs the chunk stage, horizon jobs included, as the viewer does (TEST-054); a world of an in-memory bake has no
    // stored coupled run to start from and keeps the tile-only scene.
    let stage = (o.store.is_some() && !o.allow_memory_bake).then(|| StageSetup { gen: crate::chunkstage::stage_gen(o), horizon: true });
    // The plan's height bound has to include the change of the chunk stage (as the window does), so the stage exists before the LOD parameters are
    // made; every flight then starts one of its own.
    if let Some(st) = stage {
        crate::chunkstage::enable_viewer_stage(&world, st.gen, st.horizon, false, out)?;
    }
    let field = world.field()?;
    let lod = LodParams {
        cells: o.cells,
        viewport_h_px: f64::from(size.1),
        tau_px: tau,
        max_level: o.finest_level_for(o.cells),
        min_level: o.min_level(),
        radius_m,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    lod.validate().map_err(|e| format!("--cells {} --tau {}: {e}", o.cells, tau))?;
    let aspect = f64::from(size.0) / f64::from(size.1);
    let seed = field.seed();
    let req_of = |camera: Camera| TerrainRequest {
        camera,
        size,
        view: TerrainView::Level,
        lod: lod.clone(),
        face_mapping: TangentWarp.id().to_string(),
        seed,
    };
    let source = crate::streamperf::source_label(&world, o.store.is_some());
    if smoke {
        writeln!(out, "SMOKE RUN: one short trial per altitude, {}x{}, no gate; the numbers are not a measurement", size.0, size.1)
            .map_err(err)?;
    }
    writeln!(
        out,
        "envelope: {}x{}, cells {}, tau {} px, finest level {}, bound {:.2} % over {WINDOW_S} s windows; source: {source}",
        size.0,
        size.1,
        o.cells,
        tau,
        lod.max_level,
        100.0 * o.share_bound
    )
    .map_err(err)?;
    let flight_s = if smoke { SMOKE_FLIGHT_S } else { FLIGHT_S };
    let frames = (flight_s / FRAME_STEP_S).round() as usize;
    let altitudes: &[f64] = &ALTITUDES_M;
    let mut points: Vec<Point> = Vec::new();
    let mut n = 0usize;
    for &altitude in altitudes {
        let point = if smoke {
            // One trial at the start speed: proves the path, decides nothing.
            let (lat, lon) = trial_start(n, START_SPEED_MPS, frames, radius_m, stage.is_some());
            n += 1;
            let cameras = level_flight_cameras(radius_m, aspect, field.as_ref(), altitude, START_SPEED_MPS, lat, lon, frames);
            let measure = fly(ctx, &world, &req_of, &cameras, 0, true, o.gen_workers(), stage, out)?;
            let share = measure.judged();
            Point {
                altitude_m: altitude,
                speed_mps: None,
                share,
                capped: false,
                trials: vec![Trial { speed_mps: START_SPEED_MPS, share, measure }],
            }
        } else {
            search(altitude, o.share_bound, |speed| {
                let (lat, lon) = trial_start(n, speed, frames, radius_m, stage.is_some());
                n += 1;
                let cameras = level_flight_cameras(radius_m, aspect, field.as_ref(), altitude, speed, lat, lon, frames);
                let measure = fly(ctx, &world, &req_of, &cameras, 0, false, o.gen_workers(), stage, out)?;
                writeln!(
                    out,
                    "  {altitude:.0} m at {speed:.1} m/s: worst window {:.3} % coarse (pixels){}",
                    100.0 * measure.coarse_share,
                    measure.stand_in_share.map_or(String::new(), |s| format!(", {:.3} % chunk stand-in (nodes)", 100.0 * s))
                )
                .map_err(err)?;
                Ok(measure)
            })?
        };
        writeln!(out, "{}", summary_line(&point)).map_err(err)?;
        points.push(point);
    }
    // The steady flight (STRM-042): the same altitudes with a run-in at the speed under test before the window, so the prefetch has been running for longer than a
    // chunk takes when the window starts. The cold curve above is the first visit after a stop.
    let run_in_s = if smoke { SMOKE_RUN_IN_S } else { RUN_IN_S };
    let steady: Vec<Point> = if stage.is_some() {
        let run_in = (run_in_s / FRAME_STEP_S).round() as usize;
        let total = run_in + frames;
        let edge = (run_in as f64 + EDGE_FRACTION * frames as f64) / total as f64;
        let mut points = Vec::new();
        for &altitude in altitudes {
            let point = if smoke {
                let (lat, lon) = trial_start_at(n, START_SPEED_MPS, total, radius_m, true, edge);
                n += 1;
                let cameras = level_flight_cameras(radius_m, aspect, field.as_ref(), altitude, START_SPEED_MPS, lat, lon, total);
                let measure = fly(ctx, &world, &req_of, &cameras, run_in, true, o.gen_workers(), stage, out)?;
                let share = measure.judged();
                Point {
                    altitude_m: altitude,
                    speed_mps: None,
                    share,
                    capped: false,
                    trials: vec![Trial { speed_mps: START_SPEED_MPS, share, measure }],
                }
            } else {
                search(altitude, o.share_bound, |speed| {
                    let (lat, lon) = trial_start_at(n, speed, total, radius_m, true, edge);
                    n += 1;
                    let cameras = level_flight_cameras(radius_m, aspect, field.as_ref(), altitude, speed, lat, lon, total);
                    let measure = fly(ctx, &world, &req_of, &cameras, run_in, false, o.gen_workers(), stage, out)?;
                    writeln!(
                        out,
                        "  steady (run-in {run_in_s} s), {altitude:.0} m at {speed:.1} m/s: worst window {:.3} % coarse (pixels){}",
                        100.0 * measure.coarse_share,
                        measure.stand_in_share.map_or(String::new(), |s| format!(", {:.3} % chunk stand-in (nodes)", 100.0 * s))
                    )
                    .map_err(err)?;
                    Ok(measure)
                })?
            };
            writeln!(out, "steady: {}", summary_line(&point)).map_err(err)?;
            points.push(point);
        }
        points
    } else {
        Vec::new()
    };
    // The comparison line: one altitude (300 m) again with the horizon jobs off, so the log shows what they cost the curve.
    let horizon_off: Option<Point> = match stage {
        Some(st) => {
            let off = StageSetup { horizon: false, ..st };
            let altitude = COMPARISON_ALTITUDE_M;
            let point = if smoke {
                let (lat, lon) = trial_start(n, START_SPEED_MPS, frames, radius_m, true);
                let cameras = level_flight_cameras(radius_m, aspect, field.as_ref(), altitude, START_SPEED_MPS, lat, lon, frames);
                let measure = fly(ctx, &world, &req_of, &cameras, 0, true, o.gen_workers(), Some(off), out)?;
                let share = measure.judged();
                Point {
                    altitude_m: altitude,
                    speed_mps: None,
                    share,
                    capped: false,
                    trials: vec![Trial { speed_mps: START_SPEED_MPS, share, measure }],
                }
            } else {
                search(altitude, o.share_bound, |speed| {
                    let (lat, lon) = trial_start(n, speed, frames, radius_m, true);
                    n += 1;
                    let cameras = level_flight_cameras(radius_m, aspect, field.as_ref(), altitude, speed, lat, lon, frames);
                    let measure = fly(ctx, &world, &req_of, &cameras, 0, false, o.gen_workers(), Some(off), out)?;
                    writeln!(
                        out,
                        "  horizon off, {altitude:.0} m at {speed:.1} m/s: worst window {:.3} % coarse (pixels){}",
                        100.0 * measure.coarse_share,
                        measure.stand_in_share.map_or(String::new(), |s| format!(", {:.3} % chunk stand-in (nodes)", 100.0 * s))
                    )
                    .map_err(err)?;
                    Ok(measure)
                })?
            };
            writeln!(out, "horizon off: {}", summary_line(&point)).map_err(err)?;
            Some(point)
        }
        None => None,
    };
    let adapter = ctx.adapter_name();
    let chunk_field = stage.is_some() && chunk_view_of(&world).is_some();
    if !chunk_field {
        writeln!(out, "stand-in share: {NO_CHUNK_FIELD}").map_err(err)?;
    }
    let json = log_json(
        &points,
        &LogFacts {
            bound: o.share_bound,
            source: &source,
            adapter,
            smoke,
            owner_override: &owner_override,
            chunk_field,
            horizon: true,
            horizon_off: horizon_off.as_ref(),
            edge_fraction: stage.map(|_| EDGE_FRACTION),
            steady: stage.map(|_| (steady.as_slice(), run_in_s)),
        },
    );
    let path = o.out.clone().unwrap_or_else(|| Path::new("perf").join("logs").join("envelope.json"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    std::fs::write(&path, json).map_err(err)?;
    writeln!(out, "wrote {}", path.display()).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake trial that holds up to `limit` and fails above it; its share rises with the speed.
    fn fake(limit: f64, bound: f64, log: &std::cell::RefCell<Vec<f64>>) -> impl FnMut(f64) -> Result<Measure, String> + '_ {
        move |v| {
            log.borrow_mut().push(v);
            Ok(Measure::coarse_only(if v <= limit { 0.5 * bound * v / limit } else { 2.0 * bound }))
        }
    }

    // spec: TEST-047
    #[test]
    fn the_search_doubles_until_a_trial_fails_then_bisects_four_times_and_lists_its_trials_in_order() {
        let log = std::cell::RefCell::new(Vec::new());
        let p = search(100.0, 0.01, fake(137.0, 0.01, &log)).unwrap();
        // 10, 20, 40, 80 hold; 160 fails; then 120 holds, 140 fails, 130 holds, 135 holds.
        assert_eq!(*log.borrow(), [10.0, 20.0, 40.0, 80.0, 160.0, 120.0, 140.0, 130.0, 135.0]);
        assert_eq!(p.speed_mps, Some(135.0));
        assert!(!p.capped && p.trials.len() == 9 && p.altitude_m == 100.0);
        assert_eq!(p.trials.iter().map(|t| t.speed_mps).collect::<Vec<_>>(), *log.borrow());
        assert!(p.share <= 0.01 && p.share > 0.0, "the share is the one at the highest holding speed");
        assert!(137.0 - p.speed_mps.unwrap() <= (160.0 - 80.0) / 16.0, "within one bisection step below the limit");
    }

    // spec: TEST-047
    #[test]
    fn a_trial_that_never_fails_stops_at_the_cap_and_one_that_fails_at_once_gives_no_speed() {
        let log = std::cell::RefCell::new(Vec::new());
        let p = search(30.0, 0.01, |v| {
            log.borrow_mut().push(v);
            Ok(Measure::coarse_only(0.0))
        })
        .unwrap();
        assert_eq!(p.speed_mps, Some(CAP_SPEED_MPS));
        assert!(p.capped);
        assert_eq!(*log.borrow().last().unwrap(), CAP_SPEED_MPS);
        assert_eq!(log.borrow().len(), 10, "10, 20, ... 2560, then the cap, no bisection");
        let none = search(30.0, 0.01, |_| Ok(Measure::coarse_only(0.5))).unwrap();
        assert_eq!((none.speed_mps, none.capped, none.share, none.trials.len()), (None, false, 0.5, 1));
        assert!(summary_line(&none).contains("no speed holds"));
        let failing = search(30.0, 0.01, |_| Err("device lost".to_string()));
        assert_eq!(failing.unwrap_err(), "device lost", "a failed trial ends the search with its message");
    }

    // spec: TEST-047
    #[test]
    fn the_worst_window_is_the_largest_mean_over_a_second_of_frames() {
        let step = 1.0 / 60.0;
        let mut series = vec![0.0; 300];
        series[100] = 0.6;
        // A spike of one frame spread over the 60 frames of the window.
        assert!((worst_window(&series, step, 1.0) - 0.6 / 60.0).abs() < 1e-12);
        for s in &mut series[100..160] {
            *s = 0.3;
        }
        assert!((worst_window(&series, step, 1.0) - 0.3).abs() < 1e-12, "a spike of the window's length");
        assert!((worst_window(&[0.1, 0.3], step, 1.0) - 0.2).abs() < 1e-12, "a series shorter than the window: its mean");
        assert_eq!(worst_window(&[], step, 1.0), 0.0);
        assert!(worst_window(&[0.0, f64::NAN, 0.0], step, 1.0).is_nan(), "a missing share is never a pass");
    }

    // spec: TEST-047
    #[test]
    fn the_chunk_stand_in_share_is_a_node_fraction_judged_with_the_pixel_share_by_the_larger() {
        let frames = |nodes: usize, stand_in: usize, n: usize| vec![ChunkShare { chunk_nodes: nodes, chunk_stand_in: stand_in }; n];
        // Without a chunk field: only the coarse share, and the log says so.
        let none = measure_of(&[0.002; 120], None);
        assert_eq!((none.stand_in_share, none.chunk_nodes, none.chunk_stand_in), (None, 0, 0));
        assert!((none.judged() - 0.002).abs() < 1e-12);
        // With one: 1 of 4 chunk nodes on the base for the whole second is 25 % of the nodes, whatever the pixels say.
        let m = measure_of(&[0.002; 120], Some(&frames(4, 1, 120)));
        assert!((m.stand_in_share.unwrap() - 0.25).abs() < 1e-12, "{m:?}");
        assert_eq!((m.chunk_nodes, m.chunk_stand_in), (480, 120), "sums over the frames");
        assert_eq!(m.judged(), 0.25, "the larger of the two");
        // A frame without chunk nodes has fraction 0; the larger side can also be the pixels.
        let m = measure_of(&[0.3; 60], Some(&frames(0, 0, 60)));
        assert_eq!(m.stand_in_share, Some(0.0));
        assert!((m.judged() - 0.3).abs() < 1e-12);
        // Missing data on either side is never a pass.
        let mut nan = Measure::coarse_only(0.0);
        nan.stand_in_share = Some(f64::NAN);
        assert!(nan.judged().is_nan());
        let mut nan = Measure::coarse_only(f64::NAN);
        nan.stand_in_share = Some(0.0);
        assert!(nan.judged().is_nan());
        // The search bounds on the larger share: the stand-in rises first here and sets the speed.
        let p = search(100.0, 0.01, |v| {
            Ok(Measure {
                coarse_share: 0.0,
                stand_in_share: Some(if v <= 50.0 { 0.0 } else { 0.5 }),
                chunk_nodes: 10,
                chunk_stand_in: 0,
                stage: None,
            })
        })
        .unwrap();
        assert_eq!(p.speed_mps.map(|v| v <= 50.0), Some(true), "{p:?}");
    }

    // spec: TEST-047
    #[test]
    fn the_log_names_the_weightings_and_the_reason_a_stand_in_share_is_missing() {
        let pt = |measure: Measure| Point {
            altitude_m: 30.0,
            speed_mps: Some(80.0),
            share: measure.judged(),
            capped: false,
            trials: vec![Trial { speed_mps: 80.0, share: measure.judged(), measure }],
        };
        let facts = |chunk_field| LogFacts {
            bound: 0.01,
            source: "s",
            adapter: "a",
            smoke: false,
            owner_override: &[],
            chunk_field,
            horizon: true,
            horizon_off: None,
            edge_fraction: None,
            steady: None,
        };
        let without = log_json(&[pt(Measure::coarse_only(0.004))], &facts(false));
        assert!(
            without.contains("\"chunk_field\": false") && without.contains("\"stand_in_reason\"") && without.contains("no chunk stage"),
            "{without}"
        );
        assert!(without.contains("\"stand_in_share\": null") && without.contains("pixels") && without.contains("nodes"), "{without}");
        let with = log_json(
            &[pt(Measure { coarse_share: 0.004, stand_in_share: Some(0.02), chunk_nodes: 400, chunk_stand_in: 8, stage: None })],
            &facts(true),
        );
        assert!(with.contains("\"chunk_field\": true") && !with.contains("stand_in_reason"), "{with}");
        assert!(
            with.contains("\"stand_in_share\": 0.02") && with.contains("\"chunk_nodes\": 400") && with.contains("\"chunk_stand_in\": 8"),
            "{with}"
        );
        assert!(with.contains("\"coarse_share\": 0.004"), "{with}");
    }

    // spec: TEST-047
    #[test]
    fn a_steady_flight_trial_puts_the_chunk_edge_half_way_into_the_window_after_the_run_in_and_the_log_gives_both_curves() {
        let radius_m = 6_371_000.0;
        let (frames, run_in) = ((FLIGHT_S / FRAME_STEP_S).round() as usize, (RUN_IN_S / FRAME_STEP_S).round() as usize);
        let total = run_in + frames;
        let edge = (run_in as f64 + EDGE_FRACTION * frames as f64) / total as f64;
        assert!((edge - 0.75).abs() < 1e-9, "12 s of run-in and a 12 s window: the edge is three quarters along: {edge}");
        for speed in [50.0, 200.0, 600.0] {
            let (lat, lon) = trial_start_at(3, speed, total, radius_m, true, edge);
            let flight_deg = (speed * (total - 1) as f64 * FRAME_STEP_S / radius_m).to_degrees();
            let at = |f: f64| chunk_key_at(lat + f * flight_deg, lon);
            // The flight crosses a chunk edge at the planned point, inside the measured window (the run-in may cross others on the way).
            let (before, after) = (at(edge - 0.01), at(edge + 0.01));
            assert_ne!(before, after, "{speed} m/s: an edge at {edge} of the flight");
            assert!(edge * (total - 1) as f64 >= run_in as f64, "{speed} m/s: after the run-in");
        }
        let steady = Point {
            altitude_m: 300.0,
            speed_mps: Some(190.0),
            share: 0.004,
            capped: false,
            trials: vec![Trial { speed_mps: 190.0, share: 0.004, measure: Measure::coarse_only(0.004) }],
        };
        let text = log_json(
            &[],
            &LogFacts {
                bound: 0.01,
                source: "s",
                adapter: "a",
                smoke: false,
                owner_override: &[],
                chunk_field: false,
                horizon: true,
                horizon_off: None,
                edge_fraction: None,
                steady: Some((std::slice::from_ref(&steady), RUN_IN_S)),
            },
        );
        assert!(
            text.contains("\"run_in_s\": 12") && text.contains("\"steady_curve\"") && text.contains("\"highest_speed_mps\": 190"),
            "{text}"
        );
    }

    // spec: TEST-055
    #[test]
    fn the_log_gives_what_the_chunk_stage_did_per_trial_and_the_horizon_off_comparison() {
        let stage = StageFacts {
            trace: crate::chunkstage::TraceSummary {
                chunks_landed: 12,
                horizon_maps_landed: 9,
                prefetch_asked: 0,
                first_wanted_s: Some(0.7),
                median_wait_s: 2.5,
                max_wait_s: 6.0,
            },
            chunk_jobs_at_end: 3,
            horizon_jobs_at_end: 1,
        };
        let measure = Measure { coarse_share: 0.004, stand_in_share: Some(0.02), chunk_nodes: 400, chunk_stand_in: 8, stage: Some(stage) };
        let pt = |altitude_m: f64, measure: Measure| Point {
            altitude_m,
            speed_mps: Some(80.0),
            share: measure.judged(),
            capped: false,
            trials: vec![Trial { speed_mps: 80.0, share: measure.judged(), measure }],
        };
        let off = pt(300.0, Measure { stage: Some(StageFacts { horizon_jobs_at_end: 0, ..stage }), ..measure });
        let facts = |chunk_field: bool, horizon_off: Option<&'static Point>| LogFacts {
            bound: 0.01,
            source: "s",
            adapter: "a",
            smoke: false,
            owner_override: &[],
            chunk_field,
            horizon: true,
            horizon_off,
            edge_fraction: chunk_field.then_some(EDGE_FRACTION),
            steady: None,
        };
        let off: &'static Point = Box::leak(Box::new(off));
        let with = log_json(&[pt(30.0, measure)], &facts(true, Some(off)));
        for want in [
            "\"horizon\": true",
            "\"chunks_landed\": 12",
            "\"horizon_maps_landed\": 9",
            "\"median_wait_s\": 2.5",
            "\"max_wait_s\": 6",
            "\"first_wanted_s\": 0.7",
            "\"chunk_jobs_at_end\": 3",
            "\"horizon_jobs_at_end\": 1",
            "\"horizon_off_comparison\"",
            "\"edge_fraction\": 0.5",
            "\"horizon_jobs_at_end\": 0",
        ] {
            assert!(with.contains(want), "missing {want}: {with}");
        }
        // Without a chunk stage the trials say `null` and there is no comparison.
        let without = log_json(&[pt(30.0, Measure::coarse_only(0.004))], &facts(false, None));
        assert!(
            without.contains("\"stage\": null")
                && !without.contains("horizon_off_comparison")
                && without.contains("\"horizon\": false")
                && without.contains("\"edge_fraction\": null")
        );
    }

    // spec: TEST-054
    #[test]
    fn the_start_puts_a_chunk_edge_half_way_along_the_flight_whichever_the_speed_and_the_phase() {
        // Chunks of 0.04 degrees of latitude, edges at multiples of it; a fake key, a function of the point alone.
        let key = |lat: f64, _lon: f64| (0u8, (lat / 0.04).floor() as u32, 7u32);
        let edge_of = |lat0: f64| ((lat0 / 0.04).floor() + 1.0) * 0.04;
        for flight_deg in [0.003, 0.011, 0.019, 0.05] {
            for k in 0..40 {
                let lat0 = 3.0 + 0.0093 * f64::from(k);
                let start = straddling_start_lat(&key, lat0, 20.0, flight_deg, 0.5);
                let crossing = start + 0.5 * flight_deg;
                assert!(
                    (crossing - edge_of(lat0)).abs() < 1e-6,
                    "flight {flight_deg}, start {lat0}: the edge is {} deg along",
                    crossing - lat0
                );
                // The flight starts in the chunk before the edge and ends in the one after it: exactly one edge crossed.
                assert_ne!(key(start, 20.0), key(start + flight_deg, 20.0));
                assert_eq!(key(start, 20.0).1 + 1, key(start + flight_deg, 20.0).1);
            }
        }
        // No edge within reach leaves the start where it is.
        assert_eq!(straddling_start_lat(&|_, _| (0, 0, 0), 3.0, 20.0, 0.01, 0.5), 3.0);
    }

    // spec: TEST-054
    #[test]
    fn the_real_chunk_grid_has_an_edge_within_reach_of_every_trial_start_and_the_trial_start_keeps_the_site_without_chunks() {
        for n in 0..12 {
            let (lat, lon) = trial_start(n, 150.0, 721, 6_371_000.0, true);
            let flight_deg = (150.0 * 720.0 * FRAME_STEP_S / 6_371_000.0).to_degrees();
            // The flight starts in one chunk and ends in the next.
            assert_ne!(chunk_key_at(lat, lon), chunk_key_at(lat + flight_deg, lon), "trial {n} crosses no chunk edge");
            // A start stays within one chunk edge (about 5 km) of the site's place.
            assert!((lat - start_place(n).0).abs() < 0.1, "trial {n} moved {} degrees", lat - start_place(n).0);
            assert_eq!(trial_start(n, 150.0, 721, 6_371_000.0, false), start_place(n));
        }
    }

    // spec: TEST-047
    #[test]
    fn the_trials_fly_over_places_of_their_own_and_the_log_holds_every_altitude_with_its_curve_and_the_bound() {
        let places: Vec<(f64, f64)> = (0..40).map(start_place).collect();
        let flown_deg = (CAP_SPEED_MPS * FLIGHT_S / 6_371_000.0).to_degrees();
        for w in places.windows(2) {
            assert!(w[1].0 - w[0].0 > flown_deg, "consecutive trials share no ground");
        }
        let pts: Vec<Point> = ALTITUDES_M
            .iter()
            .map(|&a| Point {
                altitude_m: a,
                speed_mps: Some(a * 2.0),
                share: 0.004,
                capped: a == 1000.0,
                trials: vec![
                    Trial { speed_mps: 10.0, share: 0.0, measure: Measure::coarse_only(0.0) },
                    Trial { speed_mps: 20.0, share: 0.5, measure: Measure::coarse_only(0.5) },
                ],
            })
            .collect();
        let text = log_json(
            &pts,
            &LogFacts {
                bound: 0.01,
                source: "store D:/bake, 16384 cells",
                adapter: "NVIDIA \"X\"",
                smoke: false,
                owner_override: &[],
                chunk_field: false,
                horizon: true,
                horizon_off: None,
                edge_fraction: None,
                steady: None,
            },
        );
        for want in [
            "\"scene\": \"envelope\"",
            "\"bound\": 0.01",
            "\"window_s\": 1",
            "\"stand_in_share\": null",
            "\"chunk_field\": false",
            "stand_in_reason",
            "\"coarse_weighting\"",
            "\"stand_in_weighting\"",
            "\"capped\": true",
            "NVIDIA 'X'",
        ] {
            assert!(text.contains(want), "{want} missing in {text}");
        }
        for a in ALTITUDES_M {
            assert!(text.contains(&format!("\"altitude_m\": {a}")), "{a}");
        }
        assert_eq!(text.matches("\"highest_speed_mps\"").count(), 4);
        assert_eq!(text.matches("\"speed_mps\"").count(), 8, "every trial is listed");
        assert!(text.contains("highest speed 2000 m/s or more (the cap)"), "{text}");
    }
}
