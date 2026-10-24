//! LOD-013: the plan returns the chunks under its nodes at or above the apply level; ready or not, the plan is the same.

use std::collections::HashSet;
use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{Face, PlanetFixed, TileId};
use planet_frame::{select_nodes_field_cached, LodParams, SelectionCache};
use planet_generators::chunk::CHUNK_GRID_LEVEL;
use planet_generators::chunkfield::{compute_chunk, halo_rings, ChunkField, ChunkLattice, ChunkStage, StandIn, PYRAMID_TOP_LEVEL};
use planet_generators::height::Reference;

const R: f64 = 6_371_000.0;

fn params() -> LodParams {
    LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() }
}

fn stage() -> StandIn {
    StandIn { seed: 7, radius_m: R, amplitude_m: 30.0, steps: 4 }
}

fn field(ready: &[TileId]) -> ChunkField<Reference> {
    let (lat, s) = (ChunkLattice::new(16).unwrap(), stage());
    let mut f = ChunkField::new(Reference(3), lat, R, &s);
    for &t in ready {
        f = f.with_ready(Arc::new(compute_chunk(&lat, &s, R, PYRAMID_TOP_LEVEL, t, halo_rings(&s)).unwrap()));
    }
    f
}

fn camera(height_m: f64) -> PlanetFixed {
    let over = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap().center_direction(&TangentWarp);
    PlanetFixed(over * (R + height_m))
}

// spec: LOD-013
#[test]
fn the_plan_requests_the_chunks_under_its_fine_nodes_and_is_the_same_whether_they_are_ready_or_not() {
    let p = params();
    for height_m in [300.0, 3_000.0] {
        let none = field(&[]);
        let a = select_nodes_field_cached(&TangentWarp, camera(height_m), &none, &p, &mut SelectionCache::new());
        let requests = a.chunk_requests(PYRAMID_TOP_LEVEL);
        assert!(!requests.is_empty(), "a camera {height_m} m up sees nodes at level {PYRAMID_TOP_LEVEL} or finer");
        // Only chunks under nodes at or finer than the top level, each once.
        let fine: HashSet<TileId> = a
            .nodes
            .iter()
            .filter(|n| n.tile.level() >= PYRAMID_TOP_LEVEL)
            .map(|n| {
                let shift = n.tile.level() - CHUNK_GRID_LEVEL;
                TileId::new(n.tile.face(), CHUNK_GRID_LEVEL, n.tile.x() >> shift, n.tile.y() >> shift).unwrap()
            })
            .collect();
        assert_eq!(requests.iter().copied().collect::<HashSet<_>>(), fine);
        assert_eq!(requests.len(), fine.len(), "each chunk once");
        assert!(a.nodes.iter().any(|n| n.tile.level() < PYRAMID_TOP_LEVEL), "coarser nodes ask for nothing");
        // With every requested chunk ready the plan is the plan.
        let all = field(&requests);
        let b = select_nodes_field_cached(&TangentWarp, camera(height_m), &all, &p, &mut SelectionCache::new());
        assert_eq!(a, b, "ready or not, the selection is the same ({height_m} m)");
        assert_eq!(requests, b.chunk_requests(PYRAMID_TOP_LEVEL));
        // The neighbours a node near a border reads are a superset, own chunks first.
        let with_reach = b.chunk_requests_with(|t| all.chunks_read_by(t));
        assert!(requests.iter().all(|c| with_reach.contains(c)));
    }
    assert_eq!(stage().step_cap(), 4);
}

// spec: STRM-040, LOD-014
#[test]
fn a_landing_drops_the_cached_origin_heights_of_exactly_the_tiles_that_read_the_chunk() {
    let p = params();
    let none = field(&[]);
    let mut cache = SelectionCache::new();
    let sel = select_nodes_field_cached(&TangentWarp, camera(300.0), &none, &p, &mut cache);
    // The plan builder caches origin heights through the terrain plan; fill them as it does.
    let tiles: Vec<TileId> = sel.nodes.iter().map(|n| n.tile).collect();
    for &t in &tiles {
        cache.origin_height_for_test(t, 1.0);
    }
    let landed = (2u8, 1000u32, 1000u32);
    let expected = tiles.iter().filter(|&&t| none.chunks_read_by(t).contains(&landed)).count();
    assert!(expected > 0 && expected < tiles.len(), "some tiles read the chunk, some do not ({expected} of {})", tiles.len());
    let dropped = cache.invalidate_chunk(landed, |t| none.chunks_read_by(t));
    assert_eq!(dropped, expected);
    // Nothing is dropped twice, and a chunk nobody reads drops nothing.
    assert_eq!(cache.invalidate_chunk(landed, |t| none.chunks_read_by(t)), 0);
    assert_eq!(cache.invalidate_chunk((5, 0, 0), |t| none.chunks_read_by(t)), 0);
}
