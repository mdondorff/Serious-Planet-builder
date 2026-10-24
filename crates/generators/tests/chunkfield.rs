//! GEN-036 to GEN-038: the chunk field's level-aware height, its pyramid steps and the border agreement and determinism of the stand-in.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId, Vec3};
use planet_generators::chunk::{apply_level, CHUNK_GRID_LEVEL};
use planet_generators::chunkfield::{
    compute_chunk, halo_rings, pyramid_position, region, ChunkField, ChunkLattice, ChunkStage, StandIn, PYRAMID_TOP_LEVEL,
};
use planet_generators::height::{HeightField, Reference};

const R: f64 = 6_371_000.0;

type Key = (u8, u32, u32);

fn stand_in(steps: u32) -> StandIn {
    StandIn { seed: 7, radius_m: R, amplitude_m: 30.0, steps }
}

fn chunk_tile(face: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(face), CHUNK_GRID_LEVEL, x, y).unwrap()
}

/// The change at the cells of `tile`'s region up to one ring beyond the chunk (the apron), by cell.
fn apron_values(lat: &ChunkLattice, stage: &dyn ChunkStage, tile: TileId, halo: u16) -> HashMap<Key, f32> {
    let reg = region(lat, tile, halo);
    let change = stage.compute(lat, &reg);
    reg.cells.iter().enumerate().filter(|(k, _)| reg.depth[*k] <= 1).map(|(k, c)| ((c.face, c.i, c.j), change[k])).collect()
}

fn core_values(lat: &ChunkLattice, stage: &dyn ChunkStage, tile: TileId, halo: u16) -> Vec<f32> {
    let reg = region(lat, tile, halo);
    stage.compute(lat, &reg)[..reg.core].to_vec()
}

// spec: GEN-038
#[test]
fn a_chunks_own_cells_are_the_cells_of_its_tile() {
    let lat = ChunkLattice::new(16).unwrap();
    for tile in [chunk_tile(2, 1000, 1000), chunk_tile(0, 2047, 5), chunk_tile(5, 0, 0)] {
        let reg = region(&lat, tile, 2);
        assert_eq!(reg.core, 256);
        for k in [0, 17, 255] {
            let d = lat.direction(reg.cells[k]);
            assert_eq!(TileId::from_direction(&TangentWarp, d, CHUNK_GRID_LEVEL), tile, "cell {k} of {tile:?}");
        }
        assert!(reg.depth.windows(2).all(|w| w[0] <= w[1]), "the region is ordered ring by ring");
        assert!(reg.cells.len() > reg.core + 4 * 16, "two rings add at least the four sides");
    }
}

// spec: GEN-038
#[test]
fn neighbouring_chunks_agree_on_the_shared_border_across_a_face_edge_and_at_a_cube_corner() {
    let lat = ChunkLattice::new(16).unwrap();
    let stage = stand_in(4);
    let halo = halo_rings(&stage);
    let cases =
        [("mid-face", chunk_tile(2, 1000, 1000)), ("face edge", chunk_tile(0, 2047, 1000)), ("cube corner", chunk_tile(0, 2047, 2047))];
    for (name, tile) in cases {
        let mine = apron_values(&lat, &stage, tile, halo);
        let mut others: HashMap<Key, HashMap<Key, f32>> = HashMap::new();
        let n = lat.cells_per_chunk();
        let reg = region(&lat, tile, halo);
        let mut compared = 0;
        let mut faces = BTreeSet::new();
        for (k, c) in reg.cells.iter().enumerate() {
            if reg.depth[k] != 1 {
                continue;
            }
            let other = chunk_tile(c.face, c.i / n, c.j / n);
            faces.insert(c.face);
            let theirs = others.entry((c.face, other.x(), other.y())).or_insert_with(|| apron_values(&lat, &stage, other, halo));
            let (a, b) = (mine[&(c.face, c.i, c.j)], theirs[&(c.face, c.i, c.j)]);
            assert_eq!(
                a.to_bits(),
                b.to_bits(),
                "{name}: cell {c:?} of {tile:?} against chunk ({}, {}, {}): {a} m against {b} m",
                c.face,
                other.x(),
                other.y()
            );
            compared += 1;
        }
        assert!(compared >= 4 * 16, "{name}: only {compared} apron cells compared");
        if name != "mid-face" {
            assert!(faces.len() > 1, "{name}: the apron must reach another face, reached {faces:?}");
        }
    }
}

// spec: GEN-038
#[test]
fn a_halo_below_the_rule_changes_the_chunk_and_the_rule_makes_it_independent_of_the_halo() {
    let lat = ChunkLattice::new(16).unwrap();
    let stage = stand_in(4);
    let tile = chunk_tile(0, 2047, 2047);
    let ruled = core_values(&lat, &stage, tile, halo_rings(&stage));
    let more = core_values(&lat, &stage, tile, halo_rings(&stage) + 3);
    assert!(ruled.iter().zip(&more).all(|(a, b)| a.to_bits() == b.to_bits()), "more halo than the rule changed a bit");
    let short = core_values(&lat, &stage, tile, 2);
    let worst = ruled.iter().zip(&short).map(|(a, b)| f64::from((a - b).abs())).fold(0.0, f64::max);
    println!("halo 2 rings against the rule's {}: largest difference {worst} m on chunk {tile:?}", halo_rings(&stage));
    assert!(worst > 0.0, "a halo below the rule should show");
}

/// The hashes of the chunks of `tiles`, computed on `threads` threads.
fn hashes_on(threads: usize, tiles: &[TileId]) -> Vec<u64> {
    let lat = ChunkLattice::new(16).unwrap();
    let stage = stand_in(4);
    let per = tiles.len().div_ceil(threads);
    let mut out = vec![0u64; tiles.len()];
    std::thread::scope(|s| {
        for (lane, slot) in out.chunks_mut(per).enumerate() {
            let (lat, stage) = (&lat, &stage);
            s.spawn(move || {
                for (k, h) in slot.iter_mut().enumerate() {
                    *h = compute_chunk(lat, stage, R, PYRAMID_TOP_LEVEL, tiles[lane * per + k], halo_rings(stage)).unwrap().hash();
                }
            });
        }
    });
    out
}

// spec: GEN-038
#[test]
fn a_chunks_hash_is_the_same_on_one_two_and_eight_threads_and_equals_the_known_answer() {
    let tiles = [
        chunk_tile(2, 1000, 1000),
        chunk_tile(0, 2047, 1000),
        chunk_tile(0, 2047, 2047),
        chunk_tile(4, 0, 0),
        chunk_tile(1, 17, 2047),
        chunk_tile(3, 1024, 3),
        chunk_tile(5, 2047, 0),
        chunk_tile(2, 5, 1500),
    ];
    let one = hashes_on(1, &tiles);
    assert_eq!(one, hashes_on(2, &tiles), "1 against 2 threads");
    assert_eq!(one, hashes_on(8, &tiles), "1 against 8 threads");
    let all = one.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, x| (h ^ x).wrapping_mul(0x0000_0100_0000_01b3));
    assert_eq!(all, KNOWN_ANSWER, "the stand-in's chunks changed: platform, order or arithmetic ({one:?})");
}

const KNOWN_ANSWER: u64 = 12_714_410_140_094_614_054;

fn p99(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[(v.len() * 99 / 100).min(v.len() - 1)]
}

// spec: GEN-035, GEN-036
#[test]
fn the_pyramid_starts_at_the_chunk_grid_level_and_tiles_above_it_are_the_base() {
    let lat = ChunkLattice::new(608).unwrap();
    assert_eq!(apply_level(lat.cell_m(R), R, 32), Some(15), "8 m cells");
    assert_eq!(pyramid_position(&lat, R, PYRAMID_TOP_LEVEL - 1, PYRAMID_TOP_LEVEL), None);
    let ks: Vec<f64> = (PYRAMID_TOP_LEVEL..=18).map(|l| pyramid_position(&lat, R, l, PYRAMID_TOP_LEVEL).unwrap()).collect();
    assert!(ks.windows(2).all(|w| w[0] >= w[1]), "finer tiles read finer pyramid levels: {ks:?}");
    assert!(ks[0] > 4.0 && ks[0] < 5.0 && ks.last().unwrap().abs() < 1e-12, "{ks:?}");
}

// spec: GEN-036, GEN-037
#[test]
fn a_ready_chunk_changes_levels_from_the_top_down_and_the_step_to_the_parent_is_small() {
    let lat = ChunkLattice::new(608).unwrap();
    let stage = stand_in(8);
    let tile = chunk_tile(2, 1000, 1000);
    let result = Arc::new(compute_chunk(&lat, &stage, R, PYRAMID_TOP_LEVEL, tile, halo_rings(&stage)).unwrap());
    let none = ChunkField::new(Reference(3), lat, R, stage.bound_m());
    let field = ChunkField::new(Reference(3), lat, R, stage.bound_m()).with_ready(result);

    // Over the chunk's centre: levels above the top are the base, the others carry the change.
    let centre = tile.center_direction(&TangentWarp);
    for level in 0..PYRAMID_TOP_LEVEL {
        assert_eq!(field.height_at_level(centre, level).to_bits(), Reference(3).height(centre).to_bits(), "level {level}");
    }
    let changed = (PYRAMID_TOP_LEVEL..=16).filter(|&l| field.height_at_level(centre, l) != Reference(3).height(centre)).count();
    assert!(changed >= 5, "levels 11 to 16 should read the change, {changed} did");
    assert_eq!(field.height(centre).to_bits(), field.height_at_level(centre, planet_core::tile::MAX_LEVEL).to_bits());
    // Without a ready chunk nothing differs from the base, at any level.
    for level in 0..=22 {
        assert_eq!(none.height_at_level(centre, level).to_bits(), Reference(3).height(centre).to_bits());
    }

    // The step between each level and its parent, at the parent's vertices, over a level-15 tile and the tiles on the way up.
    let mut tile_l = TileId::new(Face(2), 15, 16 * 1000 + 5, 16 * 1000 + 7).unwrap();
    assert_eq!(TileId::from_direction(&TangentWarp, tile_l.center_direction(&TangentWarp), CHUNK_GRID_LEVEL), tile);
    while tile_l.level() >= PYRAMID_TOP_LEVEL {
        let parent = tile_l.parent().unwrap();
        let (mut steps, mut worst) = (Vec::new(), (0.0f64, 0u32, 0u32));
        for l in 0..=32u32 {
            for k in 0..=32u32 {
                let d: Vec3 = parent.sample_direction(&TangentWarp, 32, k, l);
                let step = (field.height_at_level(d, tile_l.level()) - field.height_at_level(d, parent.level())).abs();
                if step > worst.0 {
                    worst = (step, k, l);
                }
                steps.push(step);
            }
        }
        let p = p99(steps);
        println!(
            "level {} against {}: p99 {p:.3} m, max {:.3} m at vertex ({}, {}) of {parent:?}",
            tile_l.level(),
            parent.level(),
            worst.0,
            worst.1,
            worst.2
        );
        assert!(
            p <= 0.25 || tile_l.level() != 15,
            "p99 step {p} m between level {} and {}, worst {:.3} m at vertex ({}, {}) of {parent:?}",
            tile_l.level(),
            parent.level(),
            worst.0,
            worst.1,
            worst.2
        );
        tile_l = parent;
        if parent.level() < PYRAMID_TOP_LEVEL - 1 {
            break;
        }
    }
}

// spec: GEN-035, GEN-037
#[test]
fn a_lower_top_level_leaves_the_levels_above_it_the_base_and_keeps_the_step_at_the_apply_level() {
    let lat = ChunkLattice::new(608).unwrap();
    let stage = stand_in(8);
    let tile = chunk_tile(2, 1000, 1000);
    let top = 14;
    let result = Arc::new(compute_chunk(&lat, &stage, R, top, tile, halo_rings(&stage)).unwrap());
    let field = ChunkField::new(Reference(3), lat, R, stage.bound_m()).with_top_level(top).with_ready(result);
    let centre = tile.center_direction(&TangentWarp);
    for level in 0..top {
        assert_eq!(field.height_at_level(centre, level).to_bits(), Reference(3).height(centre).to_bits(), "level {level}");
    }
    assert_ne!(field.height_at_level(centre, top), Reference(3).height(centre));
    let child = TileId::new(Face(2), 15, 16 * 1000 + 5, 16 * 1000 + 7).unwrap();
    let parent = child.parent().unwrap();
    let steps: Vec<f64> = (0..=32u32)
        .flat_map(|l| (0..=32u32).map(move |k| (k, l)))
        .map(|(k, l)| {
            let d = parent.sample_direction(&TangentWarp, 32, k, l);
            (field.height_at_level(d, 15) - field.height_at_level(d, 14)).abs()
        })
        .collect();
    assert!(p99(steps) <= 0.25);
}
