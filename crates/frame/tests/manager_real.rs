//! STRM-006, STRM-007, tier A: the residency manager fed by the real residency set R and draw set D, with a share smaller
//! than R ("Small share", ADR 0031 point 4). The view stays at full detail, the surroundings are coarser, the slots never
//! exceed the share; with a share that holds R nothing is degraded and a turn requests nothing.

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::{residency_set_field, select_nodes_in_view_field, Camera, LodParams, SelectionCache};
use planet_generators::height::{HeightField, Reference};
use planet_streaming::{FakeClock, ManualSpawner, ResidencyManager, Spawner, StreamConfig, Streamer, TileSource, Wants};
use std::collections::BTreeSet;
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

fn camera(pos: PlanetFixed, yaw: f64, pitch: f64) -> Camera {
    let hint = Vec3::new(libm::cos(yaw), libm::sin(yaw), 0.37 * libm::sin(2.0 * yaw) + 0.2);
    Camera::at_surface_point(pos, hint, pitch, 60f64.to_radians(), 16.0 / 9.0, 0.1)
}

fn closure(tiles: impl IntoIterator<Item = TileId>) -> BTreeSet<TileId> {
    let mut out = BTreeSet::new();
    for t in tiles {
        let mut x = Some(t);
        while let Some(a) = x {
            out.insert(a);
            x = a.parent();
        }
    }
    out
}

fn manager(capacity: usize) -> (ResidencyManager<u64>, Arc<ManualSpawner>) {
    let spawner = Arc::new(ManualSpawner::default());
    let config = StreamConfig { max_in_flight: 32, capacity, retire_frames: 2, ..Default::default() };
    let mut s = Streamer::new(Arc::new(Source), spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config);
    s.load_roots_blocking().unwrap();
    (ResidencyManager::new(s), spawner)
}

// spec: STRM-006, STRM-007
#[test]
fn a_share_smaller_than_r_keeps_the_view_exact_and_the_slots_inside_the_share() {
    let field = Reference(1);
    let p = params(&field);
    let pos = PlanetFixed(Vec3::new(0.3, 0.5, 0.8).normalized() * (R + 800.0));
    let mut cache = SelectionCache::new();
    let r = residency_set_field(&TangentWarp, pos, &field, &p, &mut cache);
    // Over a full turn the draw sets (with ancestors) differ; the share must hold the largest plus the roots, and is much
    // smaller than R.
    let yaws: Vec<(f64, f64)> = (0..12).map(|k| (f64::from(k) * 30f64.to_radians(), [-1.0, -0.4, 0.2][k as usize % 3])).collect();
    let draws: Vec<Vec<(TileId, f64)>> = yaws
        .iter()
        .map(|&(yaw, pitch)| {
            select_nodes_in_view_field(&TangentWarp, &camera(pos, yaw, pitch), &field, &p)
                .nodes
                .iter()
                .map(|n| (n.tile, n.distance_m))
                .collect()
        })
        .collect();
    let widest = draws.iter().map(|d| closure(d.iter().map(|(t, _)| *t)).len()).max().unwrap();
    let capacity = widest + 6 + 20;
    assert!(r.len() > capacity + 100, "the share ({capacity}) must be much smaller than R ({})", r.len());

    let (mut m, sp) = manager(capacity);
    let mut coarser_seen = false;
    let mut requested_by_a_turn = 0;
    for round in 0..2 {
        for (k, d) in draws.iter().enumerate() {
            let w = Wants { draw: d.clone(), residency: r.wants(), prefetch: vec![] };
            let dispatched_before = m.streamer().stats().completed;
            let mut exact_after = None;
            for frame in 0..40 {
                let f = m.begin_frame(&w);
                sp.run(64);
                let st = m.streamer().stats();
                assert!(st.resident + st.in_flight <= capacity, "round {round} yaw {k}: {st:?}");
                assert!(!f.report.budget_failure, "round {round} yaw {k}: {:?}", f.report);
                coarser_seen |= f.report.degraded();
                if exact_after.is_none() && f.resolved.iter().all(|x| x.exact()) {
                    exact_after = Some(frame);
                }
                if exact_after.is_some() && st.queued == 0 && st.in_flight == 0 && sp.pending() == 0 {
                    break;
                }
            }
            assert!(
                exact_after.is_some(),
                "round {round} yaw {k}: the draw set never became fully resident: {:?} {:?}",
                m.streamer().stats(),
                m.last_report()
            );
            if round == 1 {
                requested_by_a_turn += m.streamer().stats().completed - dispatched_before;
            }
            m.streamer().pools().check().unwrap();
        }
    }
    assert!(coarser_seen, "the surroundings were degraded");
    // ADR 0031 point 4's one exception: under a share smaller than R a turn can need tiles the coarse surroundings do not
    // hold. It happens, and the slots stayed inside the share all the while.
    assert!(requested_by_a_turn > 0, "a turn under a degraded R fetches the finer tiles its new view needs");
}

// spec: STRM-006, STRM-005
#[test]
fn a_share_that_holds_r_degrades_nothing_and_a_turn_requests_nothing() {
    let field = Reference(1);
    let p = params(&field);
    let pos = PlanetFixed(Vec3::new(0.3, 0.5, 0.8).normalized() * (R + 800.0));
    let mut cache = SelectionCache::new();
    let r = residency_set_field(&TangentWarp, pos, &field, &p, &mut cache);
    let (mut m, sp) = manager(r.len() + 10);
    let wants_at = |yaw: f64, pitch: f64| Wants {
        draw: select_nodes_in_view_field(&TangentWarp, &camera(pos, yaw, pitch), &field, &p)
            .nodes
            .iter()
            .map(|n| (n.tile, n.distance_m))
            .collect(),
        residency: r.wants(),
        prefetch: vec![],
    };
    let first = wants_at(0.0, -0.4);
    for _ in 0..200 {
        m.begin_frame(&first);
        sp.run(64);
    }
    let before = m.streamer().stats();
    for k in 0..36 {
        let w = wants_at(f64::from(k) * 10f64.to_radians(), [-1.0, -0.4, 0.2][k as usize % 3]);
        let f = m.begin_frame(&w);
        assert!(!f.report.degraded() && !f.report.budget_failure, "yaw {k}: {:?}", f.report);
        assert!(f.resolved.iter().all(|x| x.exact()), "yaw {k}");
        assert_eq!(sp.pending(), 0, "yaw {k}: a rotation must not request");
    }
    let after = m.streamer().stats();
    assert_eq!((after.completed, after.cancelled, after.evicted), (before.completed, before.cancelled, before.evicted));
}
