//! Dive test (CON-22, STRM-002): from orbit to 1 m with a fake source and a manual spawner. Every frame has something to
//! draw for every selected node, requests are parent-first, stale requests are cancelled, and the view settles.

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::{scripted_views, select_nodes_in_view, Camera, LodParams};
use planet_streaming::{FakeClock, ManualSpawner, Spawner, StreamConfig, Streamer, TileSource};
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
    let config = StreamConfig { max_in_flight: 16, capacity: 3000, retry_after_ms: 100, record_dispatches: true };
    let mut streamer = Streamer::new(Arc::new(Source), spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config);
    streamer.load_roots_blocking().unwrap();
    let params = lod();

    let (frames, hold) = (200, 250);
    let mut max_queue = 0;
    let (mut fallback_frames, mut worst_missing_fraction) = (0, 0.0f64);
    let mut last_selection = Vec::new();
    for k in 0..frames + hold {
        // Descend geometrically from 20,000 km to 1 m in `frames` frames, then hold still for `hold` frames.
        let height = 2.0e7 * (1.0e-7f64).powf((k.min(frames) as f64) / frames as f64);
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
        // The machine works between frames: a fixed number of jobs complete per frame.
        spawner.run(10);
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
    assert!(fallback_frames > 0, "with 10 jobs per frame the dive must have needed fallbacks (otherwise the test proves nothing)");
    assert!(max_queue < 5000, "the request queue must stay bounded, got {max_queue}");
}
