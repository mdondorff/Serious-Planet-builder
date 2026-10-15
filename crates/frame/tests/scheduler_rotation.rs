//! STRM-005, STRM-015, STRM-002, tier A: the scheduler fed by the real residency set R and draw set D. Rotation at a
//! fixed position requests and cancels nothing once R is resident; a teleport fills the draw set (with its ancestors)
//! before the rest of R.

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::{residency_set_field, select_nodes_in_view_field, Camera, LodParams, SelectionCache};
use planet_generators::height::{HeightField, Reference};
use planet_streaming::{FakeClock, ManualSpawner, Spawner, StreamConfig, Streamer, TileSource, Wants};
use std::collections::HashSet;
use std::sync::Arc;

const R: f64 = 6_371_000.0;

struct Source;
impl TileSource<u64> for Source {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        Ok(tile.raw())
    }
}

fn params(field: &dyn HeightField) -> LodParams {
    LodParams {
        cells: 16,
        viewport_h_px: 360.0,
        tau_px: 4.0,
        max_level: 16,
        max_height_m: field.max_abs_height_m(),
        ..LodParams::earth_1080p()
    }
}

fn position(dir: Vec3, height: f64) -> PlanetFixed {
    PlanetFixed(dir.normalized() * (R + height))
}

fn camera(pos: PlanetFixed, yaw: f64, pitch: f64) -> Camera {
    let hint = Vec3::new(libm::cos(yaw), libm::sin(yaw), 0.37 * libm::sin(2.0 * yaw) + 0.2);
    Camera::at_surface_point(pos, hint, pitch, 60f64.to_radians(), 16.0 / 9.0, 0.1)
}

fn wants_for(pos: PlanetFixed, yaw: f64, pitch: f64, field: &dyn HeightField, p: &LodParams, cache: &mut SelectionCache) -> Wants {
    let d = select_nodes_in_view_field(&TangentWarp, &camera(pos, yaw, pitch), field, p);
    let r = residency_set_field(&TangentWarp, pos, field, p, cache);
    Wants { draw: d.nodes.iter().map(|n| (n.tile, n.distance_m)).collect(), residency: r.wants(), prefetch: Vec::new() }
}

fn streamer(max_in_flight: usize) -> (Streamer<u64>, Arc<ManualSpawner>) {
    let spawner = Arc::new(ManualSpawner::default());
    let config = StreamConfig { max_in_flight, capacity: 1_000_000, retry_after_ms: 100, record_dispatches: true, ..Default::default() };
    let mut s = Streamer::new(Arc::new(Source), spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config);
    s.load_roots_blocking().unwrap();
    (s, spawner)
}

// spec: STRM-005, STRM-015
#[test]
fn a_full_turn_at_a_fixed_position_requests_nothing_and_cancels_nothing_once_r_is_resident() {
    let field = Reference(1);
    let p = params(&field);
    let pos = position(Vec3::new(0.3, 0.5, 0.8), 800.0);
    let mut cache = SelectionCache::new();
    let (mut s, sp) = streamer(16);
    let first = wants_for(pos, 0.0, -0.4, &field, &p, &mut cache);
    for _ in 0..10_000 {
        s.begin_frame_wants(&first);
        if sp.run(64) == 0 && s.stats().queued == 0 && s.stats().in_flight == 0 {
            break;
        }
    }
    let r_len = first.residency.len();
    assert!(r_len > 200 && first.draw.len() < first.residency.len(), "the scene must be non-trivial: D {} R {r_len}", first.draw.len());
    let before = (s.dispatch_log.len(), s.stats().cancelled, s.stats().completed);
    let mut distinct_draw_sets = HashSet::new();
    for k in 0..36 {
        let (yaw, pitch) = (f64::from(k) * 10f64.to_radians(), [-1.0, -0.4, 0.2][k as usize % 3]);
        let w = wants_for(pos, yaw, pitch, &field, &p, &mut cache);
        assert_eq!(w.residency, first.residency, "R must not depend on the view direction");
        distinct_draw_sets.insert(w.draw.iter().map(|(t, _)| *t).collect::<std::collections::BTreeSet<TileId>>());
        let resolved = s.begin_frame_wants(&w);
        assert!(resolved.iter().all(|r| r.exact()), "yaw {yaw}: every tile of D is resident");
        let st = s.stats();
        assert_eq!((st.queued, st.in_flight), (0, 0), "yaw {yaw}: a rotation must not request");
    }
    assert!(distinct_draw_sets.len() > 5, "the turn must change the draw set, got {} distinct", distinct_draw_sets.len());
    assert_eq!((s.dispatch_log.len(), s.stats().cancelled, s.stats().completed), before);
}

// spec: STRM-005, STRM-002, STRM-015
#[test]
fn after_a_teleport_the_draw_set_and_its_ancestors_are_requested_before_the_rest_of_r() {
    let field = Reference(1);
    let p = params(&field);
    let mut cache = SelectionCache::new();
    let (mut s, sp) = streamer(100_000);
    // Settled at one place, then a jump to the other side of the planet.
    let here = wants_for(position(Vec3::new(0.3, 0.5, 0.8), 800.0), 0.0, -0.4, &field, &p, &mut cache);
    for _ in 0..10_000 {
        s.begin_frame_wants(&here);
        if sp.run(1000) == 0 && s.stats().queued == 0 && s.stats().in_flight == 0 {
            break;
        }
    }
    let log_before = s.dispatch_log.len();
    let there = wants_for(position(Vec3::new(-0.6, -0.2, -0.5), 300.0), 1.0, -0.3, &field, &p, &mut cache);
    s.begin_frame_wants(&there);
    let order = &s.dispatch_log[log_before..];
    let mut draw_and_ancestors: HashSet<TileId> = HashSet::new();
    for &(t, _) in &there.draw {
        let mut x = Some(t);
        while let Some(y) = x {
            draw_and_ancestors.insert(y);
            x = y.parent();
        }
    }
    let new_draw: Vec<TileId> = order.iter().copied().filter(|t| draw_and_ancestors.contains(t)).collect();
    assert!(new_draw.len() > 20, "the jump must need new draw tiles, got {}", new_draw.len());
    let last_draw = order.iter().rposition(|t| draw_and_ancestors.contains(t)).unwrap();
    let first_rest = order.iter().position(|t| !draw_and_ancestors.contains(t)).expect("R is larger than D");
    assert!(last_draw < first_rest, "a tile of the rest of R (#{first_rest}) was requested before a draw tile (#{last_draw})");
    // Ancestors before descendants.
    let pos: std::collections::HashMap<TileId, usize> = order.iter().enumerate().map(|(i, t)| (*t, i)).collect();
    for (t, i) in &pos {
        if let Some(a) = t.parent().and_then(|a| pos.get(&a)) {
            assert!(a < i, "{t:?} requested before its parent");
        }
    }
    assert!(order.len() > new_draw.len(), "the rest of R follows the draw set");
}
