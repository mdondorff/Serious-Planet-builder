//! The speed-envelope scene (M8-V, TEST-047, M8 exit criterion 7): level flight over ground that is not in the cache, at a constant speed,
//! at fixed altitudes, and the highest speed at which the share of the screen drawn from a tile coarser than the plan wanted (or from none,
//! TEST-046) stays within a bound over every window of a second. The search, the window and the log are pure; the trial runs the real
//! streaming rig. Never run by an agent on the reference machine: the window is the architect's.

use crate::pipelined::Rig;
use crate::streamperf::Driver;
use crate::worldfield::WorldTerrain;
use crate::{err, Options};
use planet_core::cube::{FaceMapping, TangentWarp};
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

/// One trial of a search: the speed and the worst window's share at it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trial {
    pub speed_mps: f64,
    pub share: f64,
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
/// at a speed and returns its worst window's share; the share is taken to rise with the speed.
pub fn search(altitude_m: f64, bound: f64, mut trial: impl FnMut(f64) -> Result<f64, String>) -> Result<Point, String> {
    let mut trials: Vec<Trial> = Vec::new();
    let mut run = |speed: f64, trials: &mut Vec<Trial>| -> Result<bool, String> {
        let share = trial(speed)?;
        trials.push(Trial { speed_mps: speed, share });
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
}

fn number(x: f64) -> String {
    if x.is_finite() {
        format!("{x}")
    } else {
        "null".to_string()
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
    s += "  \"curve\": [\n";
    for (i, p) in points.iter().enumerate() {
        let trials: Vec<String> =
            p.trials.iter().map(|t| format!("{{\"speed_mps\": {}, \"share\": {}}}", number(t.speed_mps), number(t.share))).collect();
        s += &format!(
            "    {{\"altitude_m\": {}, \"highest_speed_mps\": {}, \"share_at_speed\": {}, \"capped\": {}, \"stand_in_share\": null, \"trials\": [{}]}}{}\n",
            number(p.altitude_m),
            p.speed_mps.map_or("null".to_string(), number),
            number(p.share),
            p.capped,
            trials.join(", "),
            if i + 1 < points.len() { "," } else { "" }
        );
    }
    s += "  ],\n  \"summary\": [\n";
    for (i, p) in points.iter().enumerate() {
        s += &format!("    {:?}{}\n", summary_line(p), if i + 1 < points.len() { "," } else { "" });
    }
    s += "  ]\n}\n";
    s
}

/// The start place of trial `n` (counted over the whole run): the scene's site moved north by [`PLACE_STEP_DEG`] a trial, so no two
/// trials fly over the same ground.
pub fn start_place(n: usize) -> (f64, f64) {
    (SITE_LAT_DEG + PLACE_STEP_DEG * n as f64, SITE_LON_DEG)
}

/// One flight on the real streaming rig from an empty tile cache: settle at the start view, fly `cameras`, and return the coarse share of
/// every frame, judged afterwards against the fresh plan of its own camera (off the timed path, like the detail lag).
fn fly(
    ctx: &GpuContext,
    world: &Arc<WorldTerrain>,
    req_of: &(dyn Fn(Camera) -> TerrainRequest + Sync),
    cameras: &[Camera],
    smoke: bool,
    gen: planet_streaming::GenWorkers,
) -> Result<Vec<f64>, String> {
    let first = req_of(cameras[0]);
    let rig = Rig::new(ctx, world, &first, gen)?;
    let mut d = Driver::plain(ctx, rig);
    let started = Instant::now();
    let need = (SETTLE_S / FRAME_STEP_S).round() as usize;
    let (mut quiet, mut frames) = (0usize, 0usize);
    loop {
        let t = d.tick(&first)?;
        frames += 1;
        quiet = if t.settled { quiet + 1 } else { 0 };
        if (smoke && frames >= 6) || quiet >= need {
            break;
        }
        if started.elapsed().as_secs_f64() > SETTLE_LIMIT_S {
            return Err(format!("the start view did not settle within {SETTLE_LIMIT_S} s"));
        }
    }
    d.restart_pacing();
    let mut draws = Vec::with_capacity(cameras.len());
    for cam in cameras {
        let t = d.tick(&req_of(*cam))?;
        draws.push(FrameDraw { camera: Some(*cam), drawn: t.drawn });
    }
    drop(d);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let lags = lag_post_pass(&draws, req_of, &|| world.field(), threads)?;
    Ok(lags.iter().map(|l| l.coarse_share).collect())
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
            let (lat, lon) = start_place(n);
            n += 1;
            let cameras = level_flight_cameras(radius_m, aspect, field.as_ref(), altitude, START_SPEED_MPS, lat, lon, frames);
            let shares = fly(ctx, &world, &req_of, &cameras, true, o.gen_workers())?;
            let share = worst_window(&shares, FRAME_STEP_S, WINDOW_S);
            Point { altitude_m: altitude, speed_mps: None, share, capped: false, trials: vec![Trial { speed_mps: START_SPEED_MPS, share }] }
        } else {
            search(altitude, o.share_bound, |speed| {
                let (lat, lon) = start_place(n);
                n += 1;
                let cameras = level_flight_cameras(radius_m, aspect, field.as_ref(), altitude, speed, lat, lon, frames);
                let shares = fly(ctx, &world, &req_of, &cameras, false, o.gen_workers())?;
                let worst = worst_window(&shares, FRAME_STEP_S, WINDOW_S);
                writeln!(out, "  {altitude:.0} m at {speed:.1} m/s: worst window {:.3} %", 100.0 * worst).map_err(err)?;
                Ok(worst)
            })?
        };
        writeln!(out, "{}", summary_line(&point)).map_err(err)?;
        points.push(point);
    }
    let adapter = ctx.adapter_name();
    let json = log_json(&points, &LogFacts { bound: o.share_bound, source: &source, adapter, smoke, owner_override: &owner_override });
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
    fn fake(limit: f64, bound: f64, log: &std::cell::RefCell<Vec<f64>>) -> impl FnMut(f64) -> Result<f64, String> + '_ {
        move |v| {
            log.borrow_mut().push(v);
            Ok(if v <= limit { 0.5 * bound * v / limit } else { 2.0 * bound })
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
            Ok(0.0)
        })
        .unwrap();
        assert_eq!(p.speed_mps, Some(CAP_SPEED_MPS));
        assert!(p.capped);
        assert_eq!(*log.borrow().last().unwrap(), CAP_SPEED_MPS);
        assert_eq!(log.borrow().len(), 10, "10, 20, ... 2560, then the cap, no bisection");
        let none = search(30.0, 0.01, |_| Ok(0.5)).unwrap();
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
                trials: vec![Trial { speed_mps: 10.0, share: 0.0 }, Trial { speed_mps: 20.0, share: 0.5 }],
            })
            .collect();
        let text = log_json(
            &pts,
            &LogFacts { bound: 0.01, source: "store D:/bake, 16384 cells", adapter: "NVIDIA \"X\"", smoke: false, owner_override: &[] },
        );
        for want in
            ["\"scene\": \"envelope\"", "\"bound\": 0.01", "\"window_s\": 1", "\"stand_in_share\": null", "\"capped\": true", "NVIDIA 'X'"]
        {
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
