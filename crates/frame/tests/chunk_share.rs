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
    let mut fade = ChunkFade::new(500);
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

// spec: STRM-042
#[test]
fn travel_prefetches_the_next_chunk_before_it_is_wanted_and_rotation_prefetches_nothing() {
    use planet_frame::chunkshare::prefetch_chunks;
    use planet_frame::prefetch::{PrefetchConfig, TravelEstimator};
    let lat = ChunkLattice::new(16).unwrap();
    let field = ChunkField::new(Reference(3), lat, R, &stage(30.0));
    let centre = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
    let (s0, s1, t0, t1) = centre.bounds();
    // Low over the west part of chunk (1000, 1000), flying east along +s at 100 m/s.
    let at = |s: f64| {
        let d = planet_core::cube::face_to_direction(&TangentWarp, Face(2), s, 0.5 * (t0 + t1));
        PlanetFixed(d * (R + 300.0))
    };
    let s_of = |t_s: f64| s0 + (s1 - s0) * 0.1 + 100.0 * t_s / (R * std::f64::consts::FRAC_PI_2) * 2.0;
    let mut est = TravelEstimator::new(PrefetchConfig::default(), R);
    let mut first_wanted_ms = None;
    let mut first_prefetched_ms = None;
    let next = (2u8, 1001u32, 1000u32);
    for frame in 0..600u64 {
        let (t_ms, pos) = (frame * 100, at(s_of(frame as f64 * 0.1)));
        est.observe(t_ms, pos);
        let predicted = est.predict();
        // The wanted set every second of the travel (the selection is the expensive part of this test).
        let wanted: Vec<(u8, u32, u32)> = if frame % 10 == 0 {
            select_nodes_field_cached(&TangentWarp, pos, &field, &params(), &mut SelectionCache::new())
                .chunk_requests(PYRAMID_TOP_LEVEL)
                .iter()
                .map(|c| (c.face().0, c.x(), c.y()))
                .collect()
        } else {
            Vec::new()
        };
        let ahead = prefetch_chunks(pos, predicted, R);
        let in_first = TileId::from_direction(&TangentWarp, pos.0.normalized(), CHUNK_GRID_LEVEL) == centre;
        if frame > 5 && in_first {
            assert!(!ahead.is_empty());
            assert_eq!(ahead[0], next, "one chunk ahead along the travel, at frame {frame}");
        }
        if ahead.contains(&next) && first_prefetched_ms.is_none() {
            first_prefetched_ms = Some(t_ms);
        }
        if wanted.contains(&next) && first_wanted_ms.is_none() {
            first_wanted_ms = Some(t_ms);
        }
    }
    let (p, w) = (first_prefetched_ms.expect("prefetched"), first_wanted_ms.expect("wanted at last"));
    assert!(p + 5_000 <= w, "the next chunk was asked for at {p} ms and wanted at {w} ms: it must be requested well before");
    // Only rotating: the position does not change, the estimator predicts nothing, nothing is prefetched.
    let mut est = TravelEstimator::new(PrefetchConfig::default(), R);
    let here = at(s_of(0.0));
    for frame in 0..100u64 {
        est.observe(frame * 100, here);
        assert!(prefetch_chunks(here, est.predict(), R).is_empty(), "a camera that only turns prefetches no chunk");
    }
}

// spec: STRM-042
#[test]
fn a_prefetched_chunk_takes_only_the_workers_the_wanted_chunks_leave_and_is_not_evicted_before_use() {
    use planet_generators::chunkfield::ChunkKey;
    let (spawner, clock) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()));
    let m = ChunkManager::new(
        ChunkLattice::new(16).unwrap(),
        R,
        PYRAMID_TOP_LEVEL,
        Arc::new(stage(30.0)),
        spawner.clone(),
        clock.clone(),
        None,
        ChunkConfig { workers: 2, ..ChunkConfig::default() },
    );
    let (here, next, further): (ChunkKey, ChunkKey, ChunkKey) = ((2, 1000, 1000), (2, 1001, 1000), (2, 1002, 1000));
    let up = m.update_with(&[(here, 10.0)], &[next, further]);
    assert_eq!(up.started, 2, "the wanted chunk and the first prefetched one");
    spawner.run(2);
    assert_eq!(m.update_with(&[(here, 10.0)], &[next]).landed, vec![here, next]);
    // The prefetched chunk is kept while it stays on the prefetch list, past the evict time, and goes once it is on none.
    clock.advance(6_000);
    assert!(m.update_with(&[(here, 10.0)], &[next]).evicted.is_empty());
    clock.advance(6_000);
    assert_eq!(m.update_with(&[(here, 10.0)], &[]).evicted, vec![next]);
}

/// A chunk view with the reads and the ready flags given.
struct Fixed {
    reads: Vec<planet_generators::chunkfield::ChunkKey>,
    ready: Vec<planet_generators::chunkfield::ChunkKey>,
}

impl ChunkView for Fixed {
    fn top_level(&self) -> u8 {
        11
    }
    fn reads(&self, _tile: TileId) -> Vec<planet_generators::chunkfield::ChunkKey> {
        self.reads.clone()
    }
    fn is_ready(&self, key: planet_generators::chunkfield::ChunkKey) -> bool {
        self.ready.contains(&key)
    }
}

// spec: STRM-047
#[test]
fn a_tiles_weight_is_the_minimum_over_the_chunks_it_reads_and_the_default_fade_is_one_and_a_half_seconds() {
    assert_eq!(ChunkFade::DEFAULT_FADE_MS, 1_500, "criterion 3 as amended: the change fades in over about 1.5 s");
    let tile = TileId::new(Face(2), CHUNK_GRID_LEVEL + 3, 8 * 1000 + 7, 8 * 1000 + 3).unwrap();
    let (a, b) = ((2u8, 1000u32, 1000u32), (2u8, 1001u32, 1000u32));
    let mut fade = ChunkFade::new(ChunkFade::DEFAULT_FADE_MS);
    fade.landed(a, 0);
    fade.landed(b, 1_000);
    let both = Fixed { reads: vec![a, b], ready: vec![a, b] };
    let own = Fixed { reads: vec![a], ready: vec![a, b] };
    // Own chunk alone: its own weight. Across the border: the smaller of the two, so the tiles on either side of the border rise together.
    assert_eq!(fade.tile_weight(&own, tile, 750), 0.5);
    assert_eq!(fade.tile_weight(&both, tile, 750), 0.0, "b landed later, its weight is 0 at 750 ms and the minimum is taken");
    assert!((fade.tile_weight(&both, tile, 1_750) - 0.5).abs() < 1e-6, "a is at 1.0 at 1.5 s, b at 0.5: the minimum");
    assert_eq!(fade.tile_weight(&both, tile, 3_000), 1.0, "both faded in");
    // A chunk that is not ready weighs 0 and holds the tile at the base.
    let one_missing = Fixed { reads: vec![a, b], ready: vec![a] };
    assert_eq!(fade.tile_weight(&one_missing, tile, 3_000), 0.0);
    // A tile that reads no chunk has no change plane: its weight is 1, the draw the old arithmetic bit for bit.
    let none = Fixed { reads: vec![], ready: vec![] };
    assert_eq!(fade.tile_weight(&none, tile, 0), 1.0);
    // The step of the weight per frame at 60 fps over the default fade: a change of 17.55 m (the largest entry step of m8-anet-chunk) moves a
    // vertex by 0.195 m per frame, inside the 0.25 m of criterion 3.
    let step = f64::from(fade.weight(a, true, 1_000 / 60) - fade.weight(a, true, 0));
    assert!(step * 17.55 <= 0.25, "{} m per frame", step * 17.55);
}
