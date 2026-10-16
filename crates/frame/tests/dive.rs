//! Dive test (CON-22, STRM-002): from orbit to 1 m with a fake source and a manual spawner. Every frame has something to
//! draw for every selected node, requests are parent-first, stale requests are cancelled, and the view settles.

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::{scripted_views, select_nodes_in_view, Camera, LodParams};
use planet_streaming::{FakeClock, GenWorkers, ManualSpawner, Spawner, StreamConfig, Streamer, TileSource, TimedSpawner};
use std::collections::HashSet;
use std::sync::Arc;

const R: f64 = 6_371_000.0;

struct Source;
impl TileSource<u64> for Source {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        Ok(tile.raw())
    }
}

fn lod() -> LodParams {
    LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() }
}

/// Camera at `height` above the sphere over (20, 30) degrees, looking north pitched down.
fn camera_at(height: f64) -> Camera {
    let (_, template) = scripted_views(R, 4.0 / 3.0, 1)[4];
    let dir = template.position.0.normalized();
    Camera::at_surface_point(
        planet_core::PlanetFixed(dir * (R + height)),
        Vec3::new(0.0, 0.0, 1.0),
        20f64.to_radians(),
        template.fov_y_rad,
        template.aspect,
        0.1,
    )
}

// spec: STRM-002, STRM-005
#[test]
fn a_dive_from_orbit_never_leaves_a_selected_node_without_something_to_draw() {
    let map = TangentWarp;
    let spawner = Arc::new(ManualSpawner::default());
    let config = StreamConfig { max_in_flight: 16, capacity: 3000, retry_after_ms: 100, record_dispatches: true, ..Default::default() };
    let mut streamer = Streamer::new(Arc::new(Source), spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config);
    streamer.load_roots_blocking().unwrap();
    let params = lod();

    let (frames, hold) = (200, 250);
    let mut max_queue = 0;
    let (mut fallback_frames, mut worst_missing_fraction) = (0, 0.0f64);
    let mut last_selection = Vec::new();
    for k in 0..frames + hold {
        // Descend geometrically from 20,000 km to 1 m in `frames` frames, then hold still for `hold` frames.
        let height = 2.0e7 * libm::pow(1.0e-7f64, (k.min(frames) as f64) / frames as f64);
        let cam = camera_at(height);
        let selection = select_nodes_in_view(&map, &cam, &params);
        let wanted: Vec<(TileId, f64)> = selection.nodes.iter().map(|n| (n.tile, n.distance_m)).collect();
        let resolved = streamer.begin_frame(&wanted);
        let mut missing = 0;
        for r in &resolved {
            let shown = r.shown.unwrap_or_else(|| panic!("frame {k}: nothing to show for {:?}", r.wanted));
            // The shown tile is the wanted tile or one of its ancestors.
            let mut t = Some(r.wanted);
            let mut found = false;
            while let Some(x) = t {
                if x == shown {
                    found = true;
                    break;
                }
                t = x.parent();
            }
            assert!(found, "frame {k}: {shown:?} is not an ancestor of {:?}", r.wanted);
            missing += usize::from(!r.exact());
        }
        if missing > 0 {
            fallback_frames += 1;
        }
        worst_missing_fraction = worst_missing_fraction.max(missing as f64 / resolved.len() as f64);
        let st = streamer.stats();
        assert!(st.in_flight <= config.max_in_flight, "frame {k}: {} jobs in flight", st.in_flight);
        max_queue = max_queue.max(st.queued);
        // The machine works between frames: a fixed number of jobs complete per frame (6 since the tight frustum cut, LOD-007:
        // the draw set shrank, and 10 per frame no longer built a queue to cancel from).
        spawner.run(6);
        last_selection = wanted;
    }
    // After holding still, everything wanted is exact.
    let final_frame = streamer.begin_frame(&last_selection);
    let inexact = final_frame.iter().filter(|r| !r.exact()).count();
    assert_eq!(inexact, 0, "{inexact} of {} nodes are still shown from an ancestor after {hold} still frames", final_frame.len());

    // Parent-first: every dispatched tile below level 1 had its parent dispatched earlier.
    let order = &streamer.dispatch_log;
    let mut position: std::collections::HashMap<TileId, usize> = std::collections::HashMap::new();
    for (i, t) in order.iter().enumerate() {
        position.entry(*t).or_insert(i); // first dispatch (a cancelled or evicted tile may be dispatched again later)
    }
    for (i, t) in order.iter().enumerate() {
        if let Some(p) = t.parent().filter(|p| p.level() > 0) {
            let at = position.get(&p).copied();
            assert!(at.is_some_and(|a| a < i), "{t:?} was dispatched (#{i}) before its parent {p:?} ({at:?})");
        }
    }
    let unique: HashSet<_> = order.iter().collect();
    let st = streamer.stats();
    eprintln!(
        "dive: {} dispatches ({} unique), {} cancelled, {} evicted, max queue {max_queue}, {fallback_frames} of {} frames used a coarser ancestor, worst frame {:.0} % inexact, resident {}",
        order.len(),
        unique.len(),
        st.cancelled,
        st.evicted,
        frames + hold,
        worst_missing_fraction * 100.0,
        st.resident
    );
    assert!(st.cancelled > 0, "a dive must cancel requests that fall out of view");
    assert!(fallback_frames > 0, "with 6 jobs per frame the dive must have needed fallbacks (otherwise the test proves nothing)");
    assert!(max_queue < 5000, "the request queue must stay bounded, got {max_queue}");
}

/// The dive's wants, frame by frame: 20,000 km to 1 m in 220 frames, then a hold of 80 (a bigger viewport than the case above:
/// the draw set must outrun the pool).
fn dive_wants() -> Vec<Vec<(TileId, f64)>> {
    let map = TangentWarp;
    let params = LodParams { viewport_h_px: 600.0, cells: 32, ..lod() };
    (0..300)
        .map(|k| {
            let height = 2.0e7 * libm::pow(1.0e-7f64, (k.min(220) as f64) / 220.0);
            let selection = select_nodes_in_view(&map, &camera_at(height), &params);
            selection.nodes.iter().map(|n| (n.tile, n.distance_m)).collect()
        })
        .collect()
}

/// Completions a second over the frames of the dive in which requests were waiting after the frame.
fn dive_rate(wants: &[Vec<(TileId, f64)>], gen: GenWorkers, job_us: u64) -> (f64, usize) {
    let spawner = Arc::new(TimedSpawner::new(gen.workers, job_us));
    let config = gen.apply(StreamConfig { capacity: 60_000, retry_after_ms: 100, ..Default::default() });
    let mut streamer = Streamer::new(Arc::new(Source), spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config);
    streamer.load_roots_blocking().unwrap();
    let (mut busy_frames, mut done) = (0usize, 0u64);
    let mut before = streamer.stats();
    for (k, wanted) in wants.iter().enumerate() {
        spawner.advance(16_667);
        streamer.begin_frame(wanted);
        let st = streamer.stats();
        if st.queued > 0 {
            busy_frames += 1;
            done += st.completed - before.completed;
            // A worker with a request waiting is never idle at a frame boundary.
            assert_eq!(spawner.busy(), gen.workers, "frame {k}: a worker idles with {} requests waiting", st.queued);
        }
        before = st;
    }
    (done as f64 / (busy_frames as f64 * 16_667e-6), busy_frames)
}

// spec: STRM-020, STRM-005, TEST-018
#[test]
fn a_dive_on_the_generation_model_reaches_the_rate_of_its_pool_and_the_old_frame_quantised_refill_does_not() {
    // 18 ms a tile on one worker: the 1,217 tiles/s of ADR 0032 on 20 threads. The pool sizes of the perf window (8) and of
    // this machine class (20); the queue-ahead is the only difference between the default and the old refill at frame boundaries.
    let wants = dive_wants();
    for n in [8, 20] {
        let job_us = 18_000;
        let model = n as f64 / (job_us as f64 * 1e-6);
        let (now, frames) = dive_rate(&wants, GenWorkers::new(n), job_us);
        let (old, _) = dive_rate(&wants, GenWorkers::new(n).with_ahead(0), job_us);
        eprintln!("dive N={n}: model {model:.0} tiles/s; default {now:.0}; refill at frame boundaries only {old:.0}; {frames} frames with a queue");
        assert!(frames >= 150, "N={n}: the dive must keep requests waiting for the rate to mean anything ({frames} frames)");
        assert!(now >= 0.8 * model, "N={n}: the dive generated {now:.0} tiles/s of the model's {model:.0}");
        assert!(old < 0.7 * model, "N={n}: the control reached {old:.0} of {model:.0}: the dive no longer shows the old limit");
    }
}
