//! STRM-038: a tile key chains the state of the chunks the tile reads.

use std::sync::Arc;

use planet_cache::key::*;
use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::chunk::CHUNK_GRID_LEVEL;
use planet_generators::chunkfield::{compute_chunk, halo_rings, ChunkField, ChunkLattice, ChunkStage, StandIn, PYRAMID_TOP_LEVEL};
use planet_generators::height::{cache_key_field, HeightField, Reference};

const R: f64 = 6_371_000.0;

fn stage() -> StandIn {
    StandIn { seed: 7, radius_m: R, amplitude_m: 30.0, steps: 4 }
}

fn chunk(x: u32, y: u32) -> TileId {
    TileId::new(Face(2), CHUNK_GRID_LEVEL, x, y).unwrap()
}

/// A chunk field over the reference terrain with the given chunks ready (16 cells per chunk).
fn field(ready: &[TileId]) -> ChunkField<Reference> {
    let lat = ChunkLattice::new(16).unwrap();
    let s = stage();
    let mut f = ChunkField::new(Reference(3), lat, R, &s);
    for &t in ready {
        f = f.with_ready(Arc::new(compute_chunk(&lat, &s, R, PYRAMID_TOP_LEVEL, t, halo_rings(&s)).unwrap()));
    }
    f
}

/// A level-15 tile over the cell `(i, j)` of the chunk (1000, 1000): with 16 cells per chunk a level-15 tile is one cell.
fn tile_at(i: u32, j: u32) -> TileId {
    TileId::new(Face(2), 15, 16 * 1000 + i, 16 * 1000 + j).unwrap()
}

fn keys(f: &dyn HeightField, t: TileId) -> (CacheKey, CacheKey, u64, u64) {
    (
        height_key(f, &TangentWarp, t, 32, R),
        mesh_key(f, &TangentWarp, t, 32, R),
        cache_key_field(f, &TangentWarp, t, 32),
        f.inputs_hash_for(t, &TangentWarp),
    )
}

// spec: STRM-038
#[test]
fn a_tile_at_the_pyramid_top_keys_its_chunk_ready_or_not_and_a_coarser_tile_keys_as_the_base() {
    let (none, ready) = (field(&[]), field(&[chunk(1000, 1000)]));
    let inside = tile_at(7, 7);
    let (a, b) = (keys(&none, inside), keys(&ready, inside));
    assert_ne!(a.0, b.0, "height key");
    assert_ne!(a.1, b.1, "mesh key");
    assert_ne!(a.2, b.2, "64-bit key");
    assert_ne!(a.3, b.3, "inputs hash");
    let plain = Reference(3);
    let coarse = TileId::new(Face(2), PYRAMID_TOP_LEVEL - 1, 8 * 1000 + 3, 8 * 1000 + 3).unwrap();
    for f in [&none as &dyn HeightField, &ready] {
        assert_eq!(height_key(f, &TangentWarp, coarse, 32, R), height_key(&plain, &TangentWarp, coarse, 32, R));
        assert_eq!(mesh_key(f, &TangentWarp, coarse, 32, R), mesh_key(&plain, &TangentWarp, coarse, 32, R));
        assert_eq!(cache_key_field(f, &TangentWarp, coarse, 32), cache_key_field(&plain, &TangentWarp, coarse, 32));
    }
    // A stand-in tile never shares a key with the base's tile, ready or not.
    assert_ne!(a.0, height_key(&plain, &TangentWarp, inside, 32, R));
}

// spec: STRM-038
#[test]
fn a_neighbour_landing_re_keys_only_the_tiles_near_the_shared_border() {
    let (only_a, both) = (field(&[chunk(1000, 1000)]), field(&[chunk(1000, 1000), chunk(1001, 1000)]));
    let at_border = tile_at(15, 7);
    let in_the_middle = tile_at(7, 7);
    assert_ne!(keys(&only_a, at_border).0, keys(&both, at_border).0, "the border tile reads the neighbour's cells");
    assert_ne!(keys(&only_a, at_border).3, keys(&both, at_border).3);
    assert_eq!(keys(&only_a, in_the_middle), keys(&both, in_the_middle), "a tile away from the border does not");
    // Another process gives another result: once its chunk has landed every tile it reaches has another key.
    let lat = ChunkLattice::new(16).unwrap();
    let other = StandIn { steps: 5, ..stage() };
    let landed = |s: &StandIn| {
        ChunkField::new(Reference(3), lat, R, s)
            .with_ready(Arc::new(compute_chunk(&lat, s, R, PYRAMID_TOP_LEVEL, chunk(1000, 1000), halo_rings(s)).unwrap()))
    };
    assert_ne!(keys(&landed(&stage()), in_the_middle).0, keys(&landed(&other), in_the_middle).0);
    assert_eq!(other.step_cap(), 5);
}
