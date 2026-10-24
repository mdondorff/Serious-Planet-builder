//! STRM-039: the stand-in share of a plan and the fade-in weight of a landed chunk.

use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{Face, PlanetFixed, TileId};
use planet_frame::chunkshare::{ChunkFade, ChunkShare, ChunkView, ManagerView, CHUNK_COLUMNS};
use planet_frame::{select_nodes_field_cached, LodParams, SelectionCache};
use planet_generators::chunk::CHUNK_GRID_LEVEL;
use planet_generators::chunkfield::{compute_chunk, halo_rings, ChunkField, ChunkLattice, StandIn, PYRAMID_TOP_LEVEL};
use planet_generators::height::Reference;
use planet_streaming::chunks::{ChunkConfig, ChunkManager};
use planet_streaming::{FakeClock, ManualSpawner};

const R: f64 = 6_371_000.0;

fn stage(amplitude_m: f64) -> StandIn {
    StandIn { seed: 7, radius_m: R, amplitude_m, steps: 3 }
}

fn params() -> LodParams {
    LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() }
}

// spec: STRM-039
#[test]
fn the_stand_in_share_counts_the_nodes_on_the_base_and_falls_to_zero_as_their_chunks_land() {
    let lat = ChunkLattice::new(16).unwrap();
    let (spawner, clock) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()));
    let manager = Arc::new(ChunkManager::new(
        lat,
        R,
        PYRAMID_TOP_LEVEL,
        Arc::new(stage(30.0)),
        spawner.clone(),
        clock,
        None,
        ChunkConfig { workers: 64, ..ChunkConfig::default() },
    ));
    let view = ManagerView(manager.clone());
    let field = ChunkField::new(Reference(3), lat, R, &stage(30.0));
    let over = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap().center_direction(&TangentWarp);
    let sel = select_nodes_field_cached(&TangentWarp, PlanetFixed(over * (R + 300.0)), &field, &params(), &mut SelectionCache::new());
    let tiles: Vec<TileId> = sel.nodes.iter().map(|n| n.tile).collect();
    let before = ChunkShare::of(tiles.iter().copied(), &view);
    assert!(before.chunk_nodes > 0 && before.chunk_nodes < tiles.len(), "{before:?} of {} nodes", tiles.len());
    assert_eq!(before.chunk_stand_in, before.chunk_nodes, "nothing is ready: every node that reads chunk detail is the stand-in");
    assert_eq!(before.fraction(), 1.0);
    // Ask for the chunks the plan reads, nearest first; land half, then all.
    let wanted: Vec<_> =
        sel.chunk_requests_with(|t| view.reads(t)).iter().enumerate().map(|(i, c)| ((c.face().0, c.x(), c.y()), i as f64)).collect();
    manager.update(&wanted);
    spawner.run(wanted.len() / 2);
    manager.update(&wanted);
    let half = ChunkShare::of(tiles.iter().copied(), &view);
    assert!(half.chunk_stand_in < before.chunk_stand_in, "{half:?}");
    spawner.run(wanted.len());
    manager.update(&wanted);
    let after = ChunkShare::of(tiles.iter().copied(), &view);
    assert_eq!((after.chunk_nodes, after.chunk_stand_in), (before.chunk_nodes, 0), "the share ends at zero");
    // The names the speed-envelope scene reads.
    assert_eq!(CHUNK_COLUMNS, ["chunk_nodes", "chunk_stand_in"]);
    assert_eq!(after.values(), [before.chunk_nodes as u64, 0]);
}

// spec: STRM-039
#[test]
fn a_landed_chunks_change_fades_in_linearly_over_the_fade_time_with_a_bounded_step_per_frame() {
    let lat = ChunkLattice::new(64).unwrap();
    let tile = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
    let key = (2u8, 1000u32, 1000u32);
    let mut fade = ChunkFade::new(ChunkFade::DEFAULT_FADE_MS);
    assert_eq!(fade.weight(key, false, 0), 0.0, "not ready: the base");
    fade.landed(key, 1_000);
    // 60 frames per second from the landing: monotone, 0 to 1 over 500 ms, never above 1.
    let weights: Vec<f32> = (0..=40u64).map(|f| fade.weight(key, true, 1_000 + f * 1000 / 60)).collect();
    assert_eq!(weights[0], 0.0);
    assert!(weights.windows(2).all(|w| w[1] >= w[0]));
    assert_eq!(*weights.last().unwrap(), 1.0);
    let step = weights.windows(2).map(|w| f64::from(w[1] - w[0])).fold(0.0, f64::max);
    assert!((step - 1.0 / 30.0).abs() < 1e-3, "about 1/30 of the change per frame at 60 fps over 500 ms: {step}");
    // The drawn height is base + w x change: the step per frame is the weight step times the change. For a change of at most `a` metres the
    // fade time that keeps it within 0.25 m at 60 fps is a / (0.25 x 60) seconds; the stand-in's 30 m needs 2 s, a 7 m change fits 500 ms.
    for (amplitude, fade_ms, fits) in [(7.0, 500u64, true), (30.0, 500, false), (30.0, 2_000, true)] {
        let s = stage(amplitude);
        let result = compute_chunk(&lat, &s, R, PYRAMID_TOP_LEVEL, tile, halo_rings(&s)).unwrap();
        let worst =
            (0..64).flat_map(|j| (0..64).map(move |i| (i, j))).map(|(i, j)| f64::from(result.at(0, i, j)).abs()).fold(0.0, f64::max);
        let per_frame = worst * (1000.0 / 60.0) / fade_ms as f64;
        assert_eq!(per_frame <= 0.25, fits, "change up to {worst:.1} m over {fade_ms} ms: {per_frame:.3} m per frame at 60 fps");
    }
    // A chunk that left RAM fades in again when it lands again.
    fade.evicted(key);
    fade.landed(key, 5_000);
    assert_eq!(fade.weight(key, true, 5_000), 0.0);
    assert!((fade.weight(key, true, 5_250) - 0.5).abs() < 1e-6);
    assert_eq!(ChunkFade::new(0).weight(key, true, 0), 1.0, "no fade: the change is there at once");
}
