//! STRM-039, STRM-042, LOD-013 over a scripted flight: 30 km down to 1.8 m, a hold, and a 300 m pan, on the fake clock with the chunk
//! manager (6 workers, 0.25 s per km2), the plan of every frame and the share of its nodes still on the base.

use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{Face, PlanetFixed, TileId};
use planet_frame::chunkshare::{prefetch_chunks, ChunkShare, ManagerView};
use planet_frame::prefetch::{PrefetchConfig, TravelEstimator};
use planet_frame::{select_nodes_field_cached, LodParams, SelectionCache};
use planet_generators::chunk::CHUNK_GRID_LEVEL;
use planet_generators::chunkfield::{ChunkField, ChunkKey, ChunkLattice, StandIn, PYRAMID_TOP_LEVEL};
use planet_generators::height::Reference;
use planet_streaming::chunks::{ChunkConfig, ChunkManager};
use planet_streaming::{FakeClock, TimedSpawner};

const R: f64 = 6_371_000.0;

fn params() -> LodParams {
    LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() }
}

// spec: STRM-039, STRM-042, LOD-013
#[test]
fn a_flight_from_30_km_to_1_8_m_and_a_pan_never_wait_for_a_chunk_plan_the_same_ready_or_not_and_end_with_no_stand_in() {
    let lat = ChunkLattice::new(16).unwrap();
    let stage = Arc::new(StandIn { seed: 7, radius_m: R, amplitude_m: 30.0, steps: 3 });
    let (spawner, clock) = (Arc::new(TimedSpawner::new(6, 6_250_000)), Arc::new(FakeClock::default()));
    let manager = Arc::new(ChunkManager::new(
        lat,
        R,
        PYRAMID_TOP_LEVEL,
        stage.clone(),
        spawner.clone(),
        clock.clone(),
        None,
        ChunkConfig { workers: 6, ..ChunkConfig::default() },
    ));
    let view = ManagerView(manager.clone());
    let none = ChunkField::new(Reference(3), lat, R, &*stage);
    let chunk0 = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
    let (_, s1, t0, t1) = chunk0.bounds();
    let ground = |s: f64| planet_core::cube::face_to_direction(&TangentWarp, Face(2), s, 0.5 * (t0 + t1));
    let p = params();
    let mut est = TravelEstimator::new(PrefetchConfig::default(), R);
    let (mut cache, mut shares) = (SelectionCache::new(), Vec::new());
    let mut pan_start_ms = None;
    // When each chunk first landed on the fake clock.
    let mut landed_at: std::collections::HashMap<ChunkKey, u64> = std::collections::HashMap::new();
    // 500 ms frames: 0 to 60 s descent through 30 km, 3 km, 300 m, 30 m to 1.8 m; hold to 100 s; then a 300 m pan east in 3 s and hold.
    let mut plain_cache = SelectionCache::new();
    // 500 ms frames in the descent, 5 s steps in the holds (a plan near the ground is the expensive part of this tier-B check), 500 ms for the pan.
    let mut t_ms = 0u64;
    for frame in 0..u64::MAX {
        if t_ms > 150_000 {
            break;
        }

        let along = if t_ms < 100_000 { 0.0 } else { (((t_ms - 100_000) as f64 / 3_000.0).min(1.0)) * 300.0 };
        let altitude = if t_ms >= 60_000 { 1.8 } else { 30_000.0 * libm::pow(1.8f64 / 30_000.0, t_ms as f64 / 60_000.0) };
        // The ground point is 150 m west of the border to the next chunk, so the 300 m pan crosses it.
        let s = s1 - 150.0 / (R * std::f64::consts::FRAC_PI_2) * 2.0 + along / (R * std::f64::consts::FRAC_PI_2) * 2.0;
        let pos = PlanetFixed(ground(s) * (R + altitude));
        if along > 0.0 && pan_start_ms.is_none() {
            pan_start_ms = Some(t_ms);
        }
        est.observe(t_ms, pos);
        // The frame's plan reads the chunks ready now, as a chunk field of the snapshot.
        let mut field = ChunkField::new(Reference(3), lat, R, &*stage);
        for r in manager.snapshot() {
            field = field.with_ready(r);
        }
        let sel = select_nodes_field_cached(&TangentWarp, pos, &field, &p, &mut cache);
        if frame % 8 == 0 {
            let plain = select_nodes_field_cached(&TangentWarp, pos, &none, &p, &mut plain_cache);
            assert_eq!(
                sel,
                plain,
                "frame {frame}: the plan is the same with these chunks ready as with none ({} ready)",
                manager.snapshot().len()
            );
        }
        let wanted: Vec<(ChunkKey, f64)> = sel
            .chunk_requests_with(|t| view.0.reads(t))
            .iter()
            .map(|c| {
                let d = (c.center_direction(&TangentWarp) * R - pos.0).length();
                ((c.face().0, c.x(), c.y()), d)
            })
            .collect();
        let ahead = prefetch_chunks(pos, est.predict(), R);
        let up = manager.update_with(&wanted, &ahead);
        assert!(manager.in_flight() <= 6, "frame {frame}: never more than the workers' jobs");
        for k in &up.landed {
            landed_at.entry(*k).or_insert(t_ms);
        }
        shares.push(ChunkShare::of(sel.nodes.iter().map(|n| n.tile), &view));
        let dt_ms = if (45_000..100_000).contains(&t_ms) || t_ms >= 106_000 { 5_000 } else { 500 };
        clock.advance(dt_ms);
        spawner.advance(dt_ms * 1000);
        t_ms += dt_ms;
    }
    let high = shares.iter().take(20).map(|s| s.chunk_nodes).max().unwrap();
    assert_eq!(high, 0, "at 30 km down to a few km no node reads chunk detail");
    let first = shares.iter().position(|s| s.chunk_nodes > 0).expect("the descent reaches nodes that read chunk detail");
    assert!(
        shares[first].chunk_stand_in == shares[first].chunk_nodes,
        "when chunk detail is first wanted, nothing is ready: {:?}",
        shares[first]
    );
    let last = shares.last().unwrap();
    assert!(last.chunk_nodes > 0 && last.chunk_stand_in == 0, "the stand-in share ends at zero: {last:?}");
    assert!(manager.stats().landed > 0 && manager.in_flight() == 0, "{:?}", manager.stats());
    // The chunk the pan crosses into landed before the pan or within 10 s of its start, and the chunk the camera was over landed.
    let next: ChunkKey = (2, 1001, 1000);
    let pan_start = pan_start_ms.expect("the pan started");
    let at = landed_at.get(&next).copied().expect("the chunk across the border landed");
    assert!(at <= pan_start + 10_000, "the chunk across the border landed at {at} ms, the pan started at {pan_start} ms");
    assert!(landed_at.contains_key(&(2, 1000, 1000)));
}
