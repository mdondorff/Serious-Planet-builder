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
    assert_eq!(fade.weight(key, true, 1_000), 0.0, "landed but not drawn yet: the base");
    assert_eq!(fade.weight(key, true, 9_999), 0.0, "the clock does not start at the landing");
    fade.shown(key, 1_000);
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
    fade.shown(key, 5_000);
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
    clock.advance(31_000);
    assert!(m.update_with(&[(here, 10.0)], &[next]).evicted.is_empty());
    clock.advance(31_000);
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

// spec: STRM-052
#[test]
fn the_fade_starts_when_the_change_is_first_drawn_so_a_slow_rebuild_still_fades_and_a_new_version_keeps_the_state() {
    let key = (2u8, 1000u32, 1000u32);
    let mut fade = ChunkFade::new(ChunkFade::DEFAULT_FADE_MS);
    // The chunk lands at 0 and the rebuilt tile reaches the renderer 2 s later (longer than the fade): the change is not there in one frame.
    fade.landed(key, 0);
    assert_eq!(fade.weight(key, true, 2_000), 0.0, "2 s after the landing and still not drawn: the base");
    fade.shown(key, 2_000);
    assert_eq!(fade.weight(key, true, 2_000), 0.0, "the first frame that draws it starts at the base");
    assert!((fade.weight(key, true, 2_750) - 0.5).abs() < 1e-6);
    assert_eq!(fade.weight(key, true, 3_500), 1.0);
    // Later tiles of the same chunk do not restart the clock: one clock for the chunk, so the tiles on either side of a shared edge agree.
    fade.shown(key, 2_900);
    assert!((fade.weight(key, true, 2_750) - 0.5).abs() < 1e-6);
    // The chunk lands again as a new version without having left RAM: the tiles under it keep their weight (no drop to the base, no new fade).
    fade.landed(key, 5_000);
    assert_eq!(fade.weight(key, true, 5_000), 1.0);
    fade.shown(key, 5_100);
    assert_eq!(fade.weight(key, true, 5_100), 1.0);
    // Left RAM and landed again: it fades in again from its first drawing.
    fade.evicted(key);
    fade.landed(key, 6_000);
    assert_eq!(fade.weight(key, true, 6_000), 0.0);
}

// spec: STRM-052
#[test]
fn a_tile_built_before_the_landing_starts_no_clock_and_the_rebuilt_one_starts_it_at_weight_zero() {
    let tile = TileId::new(Face(2), CHUNK_GRID_LEVEL + 3, 8 * 1000 + 7, 8 * 1000 + 3).unwrap();
    let a = (2u8, 1000u32, 1000u32);
    let view = Fixed { reads: vec![a], ready: vec![a] };
    let mut fade = ChunkFade::new(ChunkFade::DEFAULT_FADE_MS);
    fade.landed(a, 0);
    // The tile that was waiting for its upload when the chunk landed reaches the renderer with the key it was built with, not the key the plan
    // expects now: it is drawn, but its change is not the landed one, so nothing starts.
    let none = std::collections::HashSet::new();
    fade.drawn(&view, tile, 111, 222, 400, &none);
    assert_eq!(fade.tile_weight(&view, tile, 400), 0.0);
    assert_eq!(fade.tile_weight(&view, tile, 5_000), 0.0, "still the base: the clock has not started");
    // The rebuilt tile carries the key the plan expects: the clock starts at its first drawn frame, at weight 0.
    // Another tile that reads the chunk is still being rebuilt: the clock waits for it.
    fade.drawn(&view, tile, 222, 222, 5_500, &std::collections::HashSet::from([a]));
    assert_eq!(fade.tile_weight(&view, tile, 6_000), 0.0);
    fade.drawn(&view, tile, 222, 222, 6_000, &none);
    assert_eq!(fade.tile_weight(&view, tile, 6_000), 0.0);
    assert!((fade.tile_weight(&view, tile, 6_750) - 0.5).abs() < 1e-6);
}

// spec: STRM-047
#[test]
fn a_tiles_weight_is_the_minimum_over_the_chunks_it_reads_and_the_default_fade_is_one_and_a_half_seconds() {
    assert_eq!(ChunkFade::DEFAULT_FADE_MS, 1_500, "criterion 3 as amended: the change fades in over about 1.5 s");
    let tile = TileId::new(Face(2), CHUNK_GRID_LEVEL + 3, 8 * 1000 + 7, 8 * 1000 + 3).unwrap();
    let (a, b) = ((2u8, 1000u32, 1000u32), (2u8, 1001u32, 1000u32));
    let mut fade = ChunkFade::new(ChunkFade::DEFAULT_FADE_MS);
    fade.landed(a, 0);
    fade.shown(a, 0);
    fade.landed(b, 1_000);
    fade.shown(b, 1_000);
    let both = Fixed { reads: vec![a, b], ready: vec![a, b] };
    let own = Fixed { reads: vec![a], ready: vec![a, b] };
    // Own chunk alone: its own weight. Across the border: the smaller of the two, so the tiles on either side of the border rise together.
    assert_eq!(fade.tile_weight(&own, tile, 750), 0.5);
    assert_eq!(fade.tile_weight(&both, tile, 750), 0.0, "b landed later, its weight is 0 at 750 ms and the minimum is taken");
    assert!((fade.tile_weight(&both, tile, 1_750) - 0.5).abs() < 1e-6, "a is at 1.0 at 1.5 s, b at 0.5: the minimum");
    assert_eq!(fade.tile_weight(&both, tile, 3_000), 1.0, "both faded in");
    // A chunk that is not ready adds nothing to the tile's plane and has no weight: the tile is at the weight of the ready ones, like its
    // neighbour that reads only those (no crack at the edge between them while the chunk is computed).
    let one_missing = Fixed { reads: vec![a, b], ready: vec![a] };
    assert_eq!(fade.tile_weight(&one_missing, tile, 3_000), fade.tile_weight(&own, tile, 3_000));
    assert_eq!(fade.tile_weight(&one_missing, tile, 750), 0.5);
    let none_ready = Fixed { reads: vec![a, b], ready: vec![] };
    assert_eq!(fade.tile_weight(&none_ready, tile, 750), 1.0, "no ready chunk: no plane, the final height");
    // A tile that reads no chunk has no change plane: its weight is 1, the draw the old arithmetic bit for bit.
    let none = Fixed { reads: vec![], ready: vec![] };
    assert_eq!(fade.tile_weight(&none, tile, 0), 1.0);
    // The step of the weight per frame at 60 fps over the default fade: a change of 17.55 m (the largest entry step of m8-anet-chunk) moves a
    // vertex by 0.195 m per frame (the 256 bake's figure; the shared store's 58.3 m is 0.65 m a frame and the criterion that counts is the screen step, m8-chunk-fade design.md 6a).
    let step = f64::from(fade.weight(a, true, 1_000 / 60) - fade.weight(a, true, 0));
    assert!(step * 17.55 <= 0.25, "{} m per frame", step * 17.55);
}

// spec: STRM-053
#[test]
fn the_shared_edge_of_two_tiles_stays_closed_through_a_neighbour_landing_and_next_to_a_chunk_that_is_not_ready() {
    use planet_generators::compact::{compact_tile_field, CompactTile};
    let lat = ChunkLattice::new(608).unwrap();
    let s = stage(30.0);
    let top = 11u8;
    let (ka, kb) = ((2u8, 1000u32, 1000u32), (2u8, 1001u32, 1000u32));
    let result = |k: (u8, u32, u32)| {
        let t = TileId::new(Face(k.0), CHUNK_GRID_LEVEL, k.1, k.2).unwrap();
        Arc::new(compute_chunk(&lat, &s, R, top, t, halo_rings(&s)).unwrap())
    };
    let (ra, rb) = (result(ka), result(kb));
    let field_a = || ChunkField::new(Reference(3), lat, R, &s).with_top_level(top).with_ready(ra.clone());
    let field_ab = || field_a().with_ready(rb.clone());
    let cells = 32u32;
    // Two tiles side by side across the border of the two chunks: A's east column is B's west column.
    let ta = TileId::new(Face(2), 15, 16 * 1000 + 15, 16 * 1000 + 7).unwrap();
    let tb = TileId::new(Face(2), 15, 16 * 1001, 16 * 1000 + 7).unwrap();
    let build = |f: &dyn planet_generators::height::HeightField, id| compact_tile_field(f, &TangentWarp, id, cells, R);
    let w = (cells + 1) as usize;
    let drawn = |t: &CompactTile, i: usize, j: usize, weight: f32| f64::from(t.shade_vertex_faded(j * w + i, 1.0 - weight).height);
    let edge_gap = |a: &CompactTile, wa: f32, b: &CompactTile, wb: f32| {
        (0..w).map(|j| (drawn(a, cells as usize, j, wa) - drawn(b, 0, j, wb)).abs()).fold(0.0, f64::max)
    };
    // Where the allowance on the edge comes from: the two tiles sample the same field at the same points, but each rounds the height to its own
    // quantisation step (`hr_step`, at most half a step of error each, `height_error_bound_m`), and the plane and the weights are f32 (a few 1e-5
    // m at these heights). So two tiles agree on their edge to half a step of each plus 1e-4 m, and not more: a crack that opens at a swap or
    // with the weight is the change times a weight difference, metres, far above it.
    let tolerance = |a: &CompactTile, b: &CompactTile| a.height_error_bound_m() + b.height_error_bound_m() + 1e-4;

    // Chunk A is ready and faded in; chunk B is not ready: the tile beside the border reads both and weighs what its neighbour weighs.
    let (a0, b0) = (build(&field_a(), ta), build(&field_a(), tb));
    assert!(a0.change.is_some(), "the tile over the ready chunk carries a change");
    let mut fade = ChunkFade::new(1_500);
    fade.landed(ka, 0);
    fade.shown(ka, 0);
    let view_a = Fixed { reads: vec![ka], ready: vec![ka] };
    let view_b = Fixed { reads: vec![kb, ka], ready: vec![ka] };
    for ms in [0u64, 400, 900, 1_500] {
        let (wa, wb) = (fade.tile_weight(&view_a, ta, ms), fade.tile_weight(&view_b, tb, ms));
        assert_eq!(wa, wb, "{ms} ms: both sides weigh what the ready chunk weighs");
        let gap = edge_gap(&a0, wa, &b0, wb);
        assert!(gap <= tolerance(&a0, &b0), "{ms} ms: the shared edge is open by {gap} m next to a chunk that is not ready");
    }

    // Chunk B lands: both tiles are rebuilt; each carries the increment over what it was drawn at, and one clock moves both.
    let (a1, b1) = (build(&field_ab(), ta), build(&field_ab(), tb));
    let mut a2 = a1.clone();
    let mut b2 = b1.clone();
    a2.change = a1.change_replacing(&a0, 1.0);
    b2.change = b1.change_replacing(&b0, 1.0);
    assert!(a2.change.is_some() || b2.change.is_some(), "chunk B changes the tiles beside its border");
    let was = (edge_gap(&a0, 1.0, &b0, 1.0), drawn(&a0, cells as usize, 5, 1.0));
    assert!(was.0 <= tolerance(&a0, &b0), "the old tiles agree on the edge: {}", was.0);
    let mut worst = 0.0f64;
    for epoch in [0.0f32, 0.3, 0.7, 1.0] {
        let gap = edge_gap(&a2, epoch, &b2, epoch);
        worst = worst.max(gap);
        assert!(gap <= tolerance(&a2, &b2), "epoch {epoch}: the edge between the rebuilt tiles is open by {gap} m");
    }
    eprintln!("shared edge: worst gap {worst:.2e} m against an allowance of {:.2e} m", tolerance(&a2, &b2));
    // At weight 0 the rebuilt tile draws what the old one drew, and at weight 1 its own surface (nothing popped at the swap).
    for j in 0..w {
        for i in 0..w {
            assert!((drawn(&a2, i, j, 0.0) - drawn(&a0, i, j, 1.0)).abs() <= tolerance(&a2, &a0), "tile A at ({i}, {j}) pops at the swap");
            assert!((drawn(&b2, i, j, 0.0) - drawn(&b0, i, j, 1.0)).abs() <= tolerance(&b2, &b0), "tile B at ({i}, {j}) pops at the swap");
            assert!((drawn(&a2, i, j, 1.0) - drawn(&a1, i, j, 1.0)).abs() <= tolerance(&a2, &a1));
        }
    }
}

// spec: STRM-055
#[test]
fn the_landing_note_gives_the_chunks_landed_and_the_stand_in_share_and_zero_without_nodes() {
    assert_eq!(planet_frame::chunkshare::landing_note(6, 200, 170), "6 chunks landed, stand-in share 85.0 %");
    assert_eq!(planet_frame::chunkshare::landing_note(0, 0, 0), "0 chunks landed, stand-in share 0.0 %");
}

// spec: STRM-042
#[test]
fn travel_prefetches_the_chunks_beside_the_one_ahead_too() {
    use planet_frame::chunkshare::prefetch_chunks;
    use planet_frame::prefetch::{PrefetchConfig, TravelEstimator};
    let centre = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
    let (s0, s1, t0, t1) = centre.bounds();
    let at = |s: f64| {
        let d = planet_core::cube::face_to_direction(&TangentWarp, Face(2), s, 0.5 * (t0 + t1));
        PlanetFixed(d * (R + 300.0))
    };
    let mut est = TravelEstimator::new(PrefetchConfig::default(), R);
    let mut ahead = Vec::new();
    for frame in 0..10u64 {
        let s = s0 + (s1 - s0) * 0.1 + 100.0 * (frame as f64 * 0.1) / (R * std::f64::consts::FRAC_PI_2) * 2.0;
        let pos = at(s);
        est.observe(frame * 100, pos);
        ahead = prefetch_chunks(pos, est.predict(), R);
    }
    // East along +s: the next chunk first, then its two neighbours across the track (t - 1 and t + 1), and not the one under the camera.
    assert_eq!(ahead[0], (2, 1001, 1000), "{ahead:?}");
    for beside in [(2u8, 1001u32, 999u32), (2, 1001, 1001)] {
        assert!(ahead.contains(&beside), "{beside:?} in {ahead:?}");
    }
    assert!(!ahead.contains(&(2, 1000, 1000)));
}

// spec: STRM-042
#[test]
fn the_track_set_follows_the_reach_of_the_plan_and_is_empty_without_travel_over_the_ground() {
    use planet_frame::chunkshare::track_chunks;
    let centre = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
    let (s0, s1, t0, t1) = centre.bounds();
    let at = |s: f64| {
        let d = planet_core::cube::face_to_direction(&TangentWarp, Face(2), s, 0.5 * (t0 + t1));
        PlanetFixed(d * (R + 300.0))
    };
    // Low over the west part of chunk (1000, 1000), 160 m/s east along +s.
    let here = at(s0 + (s1 - s0) * 0.1);
    let ahead = at(s0 + (s1 - s0) * 0.1 + 1.0e-4);
    let velocity = (ahead.0 - here.0).normalized() * 160.0;
    let edge_m = R * std::f64::consts::FRAC_PI_2 / f64::from(1u32 << CHUNK_GRID_LEVEL);
    let next = (2u8, 1001u32, 1000u32);
    let beside = [(2u8, 1001u32, 999u32), (2, 1001, 1001)];
    // A reach below one chunk edge (a low view) is one edge, and the track asks half an edge further than the centres the plan reads: the chunk
    // across the edge is asked when the camera is still in the current one (window 14: asked 4.7 s before it was wanted, a solve of 6 s).
    let near = track_chunks(here, velocity, 0.2 * edge_m, 12.0, R);
    assert!(near.contains(&next), "{near:?}");
    assert_eq!(near, track_chunks(here, velocity, edge_m, 12.0, R), "a reach under one edge is one edge");
    let on_entering = track_chunks(here, velocity, 0.2 * edge_m, 0.5, R);
    assert!(on_entering.contains(&next), "0.5 s at 160 m/s is 80 m: the next chunk is asked at once: {on_entering:?}");
    // A reach of 1.5 edges (a wider view): the pair beside it too, and a wider reach never gives fewer.
    let wide = track_chunks(here, velocity, 1.6 * edge_m, 12.0, R);
    assert!(beside.iter().all(|b| wide.contains(b)) && wide.contains(&next), "{wide:?}");
    assert!(near.iter().all(|k| wide.contains(k)));
    // The samples nearest the camera come first: the chunk under it is first of all.
    assert_eq!(wide[0], (2, 1000, 1000));
    // Standing, rotating or only diving: no track, so nothing. A longer lead reaches further along the track.
    let up = here.0.normalized();
    assert!(track_chunks(here, planet_core::Vec3::new(0.0, 0.0, 0.0), 1.6 * edge_m, 12.0, R).is_empty());
    assert!(track_chunks(here, up * -50.0, 1.6 * edge_m, 12.0, R).is_empty(), "a dive has no component along the ground");
    let long = track_chunks(here, velocity, 1.1 * edge_m, 60.0, R);
    assert!(long.contains(&(2, 1002, 1000)), "60 s at 160 m/s is 9.6 km: two chunks along the track: {long:?}");
}

// spec: STRM-042
#[test]
fn a_chunk_is_asked_at_least_a_solve_and_a_margin_before_the_plan_wants_it_at_every_flight_speed_and_the_first_frame_chunks_are_asked_during_the_run_in(
) {
    use planet_frame::chunkshare::track_chunks;
    // The solve of a chunk is 6.4 s at the median and 10 s at the most (window 14); the margin is a second of the frame loop's own lag.
    const SOLVE_MAX_S: f64 = 10.0;
    const MARGIN_S: f64 = 1.0;
    const LEAD_S: f64 = 12.0;
    const STEP_S: f64 = 0.25;
    let edge_m = R * std::f64::consts::FRAC_PI_2 / f64::from(1u32 << CHUNK_GRID_LEVEL);
    let start = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap().center_direction(&TangentWarp);
    let tangent = {
        let east = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1001, 1000).unwrap().center_direction(&TangentWarp);
        (east - start * east.dot(start)).normalized()
    };
    let at = |along_m: f64| (start + tangent * (along_m / R)).normalized();
    let centres: Vec<((u8, u32, u32), planet_core::Vec3)> = (990..1030u32)
        .flat_map(|x| (994..1007u32).map(move |y| (x, y)))
        .map(|(x, y)| ((2u8, x, y), TileId::new(Face(2), CHUNK_GRID_LEVEL, x, y).unwrap().center_direction(&TangentWarp)))
        .collect();
    for speed in [100.0, 200.0, 340.0, 440.0, 500.0] {
        // A low view reads the chunks within 1.6 edges of the camera's chunk centres, the nearest tile half an edge nearer.
        for reach_m in [0.2 * edge_m, 1.6 * edge_m, 2.6 * edge_m] {
            let wanted_within_m = reach_m.max(edge_m) - 0.5 * edge_m;
            let (mut asked, mut wanted): (std::collections::HashMap<_, f64>, std::collections::HashMap<_, f64>) = Default::default();
            let steps = ((LEAD_S + 40.0) / STEP_S) as usize;
            for k in 0..steps {
                let t = k as f64 * STEP_S;
                let dir = at(speed * t);
                let velocity = (at(speed * (t + 0.01)) - dir).normalized() * speed;
                let position = PlanetFixed(dir * (R + 300.0));
                for key in track_chunks(position, velocity, reach_m, LEAD_S, R) {
                    asked.entry(key).or_insert(t);
                }
                for (key, c) in &centres {
                    if (*c - dir).length() * R <= wanted_within_m {
                        wanted.entry(*key).or_insert(t);
                    }
                }
            }
            for (key, &t_wanted) in &wanted {
                if t_wanted < LEAD_S {
                    // Wanted inside the first lead: asked at the first frame (t = 0), as early as the track can.
                    assert!(
                        asked.get(key).is_some_and(|&a| a <= 0.0 + STEP_S),
                        "{speed} m/s reach {reach_m}: {key:?} wanted at {t_wanted:.1} s, asked {:?}",
                        asked.get(key)
                    );
                    continue;
                }
                let a = *asked
                    .get(key)
                    .unwrap_or_else(|| panic!("{speed} m/s reach {reach_m}: {key:?} wanted at {t_wanted:.1} s was never asked"));
                assert!(
                    t_wanted - a >= SOLVE_MAX_S + MARGIN_S,
                    "{speed} m/s reach {:.1} edges: {key:?} asked {a:.1} s, wanted {t_wanted:.1} s: only {:.1} s ahead of a solve of {SOLVE_MAX_S} s and a margin of {MARGIN_S} s",
                    reach_m / edge_m,
                    t_wanted - a
                );
            }
        }
    }
}
