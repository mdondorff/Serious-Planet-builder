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
    field_n(16, ready)
}

fn field_n(per: u32, ready: &[TileId]) -> ChunkField<Reference> {
    let lat = ChunkLattice::new(per).unwrap();
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
    let (only_a, both) = (field_n(64, &[chunk(1000, 1000)]), field_n(64, &[chunk(1000, 1000), chunk(1001, 1000)]));
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

// spec: STRM-038
#[test]
fn every_chunk_a_tile_reads_is_in_the_chunks_its_key_chains_even_at_a_cube_corner() {
    use planet_generators::chunkfield::{chunks_read_by, ChunkKey, ChunkResult};
    let lat = ChunkLattice::new(16).unwrap();
    let s = stage();
    let tiles: Vec<TileId> = (14..=16u8)
        .flat_map(|level| (0..8u32).flat_map(move |x| (0..8u32).map(move |y| TileId::new(Face(2), level, x, y).unwrap())))
        .collect();
    // Every chunk any of the tiles could read, found with a reach far wider than the key's, plus the chunks around the corner of the faces involved.
    let mut wide = std::collections::BTreeSet::new();
    for &t in &tiles {
        wide.extend(chunks_read_by(&lat, PYRAMID_TOP_LEVEL, 200, t));
    }
    let faces: std::collections::BTreeSet<u8> = wide.iter().map(|k| k.0).collect();
    for &f in &faces {
        for x in (0..4u32).chain(2044..2048) {
            for y in (0..4u32).chain(2044..2048) {
                wide.insert((f, x, y));
            }
        }
    }
    let results: std::collections::BTreeMap<ChunkKey, Arc<ChunkResult>> = wide
        .iter()
        .map(|&k| {
            let t = TileId::new(Face(k.0), CHUNK_GRID_LEVEL, k.1, k.2).unwrap();
            (k, Arc::new(compute_chunk(&lat, &s, R, PYRAMID_TOP_LEVEL, t, halo_rings(&s)).unwrap()))
        })
        .collect();
    let build = |skip: Option<ChunkKey>| {
        let mut f = ChunkField::new(Reference(3), lat, R, &s);
        for (k, r) in &results {
            if Some(*k) != skip {
                f = f.with_ready(r.clone());
            }
        }
        f
    };
    let all = build(None);
    let mut checked = 0;
    for &tile in &tiles {
        let reads = all.chunks_read_by(tile);
        let level = tile.level();
        let dirs: Vec<_> = (-1..=9i64)
            .flat_map(|l| (-1..=9i64).map(move |k| (k, l)))
            .map(|(k, l)| tile.sample_direction_ext(&TangentWarp, 8, k.min(9), l))
            .collect();
        let full: Vec<f64> = dirs.iter().map(|&d| all.height_at_level(d, level)).collect();
        for &n in results.keys().filter(|k| !reads.contains(k)) {
            let without = build(Some(n));
            for (d, h) in dirs.iter().zip(&full) {
                assert_eq!(
                    *h,
                    without.height_at_level(*d, level),
                    "tile {tile:?} reads chunk {n:?}, which its key does not chain ({reads:?})"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 1000, "{checked}");
}
