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

/// The top level of the first design (the chunk grid level), where the steps between the levels were measured.
const TOP11: u8 = 11;

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

const KNOWN_ANSWER: u64 = 13_816_638_891_823_402_749;

fn p99(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[(v.len() * 99 / 100).min(v.len() - 1)]
}

// spec: GEN-035, GEN-036, GEN-053
#[test]
fn the_pyramid_starts_at_the_chunk_grid_level_and_tiles_above_it_are_the_base() {
    let lat = ChunkLattice::new(608).unwrap();
    assert_eq!(apply_level(lat.cell_m(R), R, 32), Some(15), "8 m cells");
    assert_eq!(pyramid_position(&lat, R, TOP11 - 1, TOP11), None);
    let ks: Vec<f64> = (TOP11..=18).map(|l| pyramid_position(&lat, R, l, TOP11).unwrap()).collect();
    assert!(ks.windows(2).all(|w| w[0] >= w[1]), "finer tiles read finer pyramid levels: {ks:?}");
    assert!(ks[0] > 4.0 && ks[0] < 5.0 && ks.last().unwrap().abs() < 1e-12, "{ks:?}");
}

// spec: GEN-036, GEN-037
#[test]
fn a_ready_chunk_changes_levels_from_the_top_down_and_the_step_to_the_parent_is_small() {
    let lat = ChunkLattice::new(608).unwrap();
    let stage = stand_in(8);
    let tile = chunk_tile(2, 1000, 1000);
    let result = Arc::new(compute_chunk(&lat, &stage, R, TOP11, tile, halo_rings(&stage)).unwrap());
    let none = ChunkField::new(Reference(3), lat, R, &stage).with_top_level(TOP11);
    let field = ChunkField::new(Reference(3), lat, R, &stage).with_top_level(TOP11).with_ready(result);

    // Over the chunk's centre: levels above the top are the base, the others carry the change.
    let centre = tile.center_direction(&TangentWarp);
    for level in 0..TOP11 {
        assert_eq!(field.height_at_level(centre, level).to_bits(), Reference(3).height(centre).to_bits(), "level {level}");
    }
    let changed = (TOP11..=16).filter(|&l| field.height_at_level(centre, l) != Reference(3).height(centre)).count();
    assert!(changed >= 5, "levels 11 to 16 should read the change, {changed} did");
    assert_eq!(field.height(centre).to_bits(), field.height_at_level(centre, planet_core::tile::MAX_LEVEL).to_bits());
    // Without a ready chunk nothing differs from the base, at any level.
    for level in 0..=22 {
        assert_eq!(none.height_at_level(centre, level).to_bits(), Reference(3).height(centre).to_bits());
    }

    // The step between each level and its parent, at the parent's vertices, over a level-15 tile and the tiles on the way up.
    let mut tile_l = TileId::new(Face(2), 15, 16 * 1000 + 5, 16 * 1000 + 7).unwrap();
    assert_eq!(TileId::from_direction(&TangentWarp, tile_l.center_direction(&TangentWarp), CHUNK_GRID_LEVEL), tile);
    while tile_l.level() >= TOP11 {
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
            p <= 0.25 || tile_l.level() != 15, // GEN-037 is the step at the apply level; the coarser steps are printed (design.md section 11)
            "p99 step {p} m between level {} and {}, worst {:.3} m at vertex ({}, {}) of {parent:?}",
            tile_l.level(),
            parent.level(),
            worst.0,
            worst.1,
            worst.2
        );
        tile_l = parent;
        if parent.level() < TOP11 - 1 {
            break;
        }
    }
}

// spec: GEN-035, GEN-037, GEN-053
#[test]
fn a_lower_top_level_leaves_the_levels_above_it_the_base_and_keeps_the_step_at_the_apply_level() {
    let lat = ChunkLattice::new(608).unwrap();
    let stage = stand_in(8);
    let tile = chunk_tile(2, 1000, 1000);
    let top = 14;
    let result = Arc::new(compute_chunk(&lat, &stage, R, top, tile, halo_rings(&stage)).unwrap());
    let field = ChunkField::new(Reference(3), lat, R, &stage).with_top_level(top).with_ready(result);
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

// spec: GEN-038
#[test]
fn the_halo_rule_protects_the_apron_a_shorter_halo_shows_and_a_longer_one_changes_nothing() {
    let lat = ChunkLattice::new(16).unwrap();
    let stage = stand_in(4);
    let tile = chunk_tile(0, 2047, 2047);
    let rule = halo_rings(&stage);
    assert!(compute_chunk(&lat, &stage, R, PYRAMID_TOP_LEVEL, tile, rule - 1).is_none(), "below the rule a chunk is refused");
    let ruled = apron_values(&lat, &stage, tile, rule);
    let more = apron_values(&lat, &stage, tile, rule + 3);
    assert!(ruled.iter().all(|(k, v)| v.to_bits() == more[k].to_bits()), "more halo than the rule changed a bit");
    // One ring short (the step cap itself): the chunk's own cells are still exact, the apron is not.
    let short = apron_values(&lat, &stage, tile, rule - 1);
    let reg = region(&lat, tile, rule);
    let (mut own_worst, mut apron_worst) = (0.0f64, 0.0f64);
    for (k, c) in reg.cells.iter().enumerate().filter(|(k, _)| reg.depth[*k] <= 1) {
        let key = (c.face, c.i, c.j);
        let d = f64::from((ruled[&key] - short[&key]).abs());
        if reg.depth[k] == 0 {
            own_worst = own_worst.max(d)
        } else {
            apron_worst = apron_worst.max(d)
        }
    }
    println!(
        "halo {} against the rule's {rule}: own cells differ by {own_worst} m, the apron by up to {apron_worst} m on chunk {tile:?}",
        rule - 1
    );
    assert_eq!(own_worst, 0.0);
    assert!(apron_worst > 0.0, "the apron should need the extra ring");
}

// spec: GEN-036
#[test]
fn the_change_has_no_seam_across_a_face_edge_with_both_chunks_ready() {
    let lat = ChunkLattice::new(16).unwrap();
    let stage = stand_in(4);
    let edge = |s: f64, t: f64| planet_core::cube::face_to_direction(&TangentWarp, Face(0), s, t);
    let (inside, outside) = (edge(1.0 - 1e-9, 0.1), edge(1.0 + 1e-9, 0.1));
    let a = TileId::from_direction(&TangentWarp, inside, CHUNK_GRID_LEVEL);
    let b = TileId::from_direction(&TangentWarp, outside, CHUNK_GRID_LEVEL);
    assert_ne!(a.face(), b.face(), "the two points are on different faces");
    let make = |tile| Arc::new(compute_chunk(&lat, &stage, R, PYRAMID_TOP_LEVEL, tile, halo_rings(&stage)).unwrap());
    let field = ChunkField::new(Reference(3), lat, R, &stage).with_ready(make(a)).with_ready(make(b));
    for level in [11, 15, 22] {
        let (x, y) = (field.change_at_level(inside, level), field.change_at_level(outside, level));
        assert!((x - y).abs() <= 1e-3, "level {level}: {x} m inside, {y} m outside the face edge");
    }
}

// spec: GEN-038
#[test]
fn a_corner_chunk_of_8_m_cells_keeps_its_known_answer() {
    let lat = ChunkLattice::new(608).unwrap();
    let stage = stand_in(8);
    let tile = chunk_tile(0, 2047, 2047);
    let hash = compute_chunk(&lat, &stage, R, PYRAMID_TOP_LEVEL, tile, halo_rings(&stage)).unwrap().hash();
    assert_eq!(hash, CORNER_KNOWN_ANSWER, "the corner chunk changed: platform, order, arithmetic or the tie-break across the face edge");
}

const CORNER_KNOWN_ANSWER: u64 = 5_129_282_273_553_867_978;

// spec: GEN-036
#[test]
fn at_a_level_that_reads_the_raster_the_height_is_the_base_plus_the_stored_change() {
    let lat = ChunkLattice::new(16).unwrap();
    let stage = stand_in(4);
    let tile = chunk_tile(2, 1000, 1000);
    let result = Arc::new(compute_chunk(&lat, &stage, R, PYRAMID_TOP_LEVEL, tile, halo_rings(&stage)).unwrap());
    let field = ChunkField::new(Reference(3), lat, R, &stage).with_ready(result.clone());
    for (i, j) in [(0u32, 0u32), (7, 9), (15, 15), (3, 12)] {
        let cell = planet_generators::chunkfield::Cell { face: 2, i: 1000 * 16 + i, j: 1000 * 16 + j };
        let d = lat.direction(cell);
        let stored = f64::from(result.at(0, i as usize, j as usize));
        let got = field.height_at_level(d, 22) - Reference(3).height(d);
        assert!((got - stored).abs() <= 1e-6, "cell ({i}, {j}): {got} m against the stored {stored} m");
    }
}

// spec: GEN-036, GEN-054
#[test]
fn tile_generation_reads_the_height_at_its_own_level() {
    use planet_generators::height::generate_tile_field;
    use planet_generators::mesh::tile_mesh_field;
    let lat = ChunkLattice::new(16).unwrap();
    let stage = stand_in(4);
    let tile = chunk_tile(2, 1000, 1000);
    let result = Arc::new(compute_chunk(&lat, &stage, R, PYRAMID_TOP_LEVEL, tile, halo_rings(&stage)).unwrap());
    let field = ChunkField::new(Reference(3), lat, R, &stage).with_ready(result);
    // A tile of level 15 over the chunk: its stored heights are the field's heights at level 15, and differ from the base's.
    let fine = TileId::new(Face(2), 15, 16 * 1000 + 7, 16 * 1000 + 9).unwrap();
    let data = generate_tile_field(&field, &TangentWarp, fine, 8);
    let base = generate_tile_field(&Reference(3), &TangentWarp, fine, 8);
    let mut changed = 0;
    for l in 0..=8 {
        for k in 0..=8 {
            let d = fine.sample_direction(&TangentWarp, 8, k, l);
            assert_eq!(data.at(k, l), field.height_at_level(d, 15) as f32);
            changed += usize::from(data.at(k, l) != base.at(k, l));
        }
    }
    assert!(changed > 40, "the chunk's change is in the tile ({changed} of 81 samples)");
    // The mesh reads the same heights: its origin sits at the tile centre at the level's height, and a coarser tile is the base's tile bit for bit.
    let mesh = tile_mesh_field(&field, &TangentWarp, fine, 8, R);
    let centre = fine.center_direction(&TangentWarp);
    let expected = (R + field.height_at_level(centre, 15)) * centre.length();
    assert!((mesh.origin.0.length() - expected).abs() < 1e-6, "origin {} against {}", mesh.origin.0.length(), expected);
    let coarse = TileId::new(Face(2), 13, 4 * 1000 + 1, 4 * 1000 + 1).unwrap();
    assert_eq!(
        generate_tile_field(&field, &TangentWarp, coarse, 8).heights,
        generate_tile_field(&Reference(3), &TangentWarp, coarse, 8).heights
    );
}

// spec: STRM-041
#[test]
fn a_chunk_result_round_trips_through_bytes_and_a_damaged_payload_is_refused() {
    use planet_generators::chunkfield::ChunkResult;
    let lat = ChunkLattice::new(64).unwrap();
    let stage = stand_in(4);
    let tile = chunk_tile(2, 1000, 1000);
    let result = compute_chunk(&lat, &stage, R, 11, tile, halo_rings(&stage)).unwrap();
    assert!(result.k_max() >= 1, "the 64-cell chunk carries pyramid levels");
    let bytes = result.to_bytes();
    let back = ChunkResult::from_bytes(tile, &bytes).unwrap();
    assert_eq!(back.hash(), result.hash());
    assert_eq!(back.size_bytes(), result.size_bytes());
    for cut in [0usize, 3, 8, bytes.len() - 1] {
        assert!(ChunkResult::from_bytes(tile, &bytes[..cut]).is_none(), "{cut} bytes");
    }
    let mut longer = bytes.clone();
    longer.push(0);
    assert!(ChunkResult::from_bytes(tile, &longer).is_none());
    let mut wrong = bytes;
    wrong[0] = 63;
    assert!(ChunkResult::from_bytes(tile, &wrong).is_none(), "a wrong size is refused");
}

// spec: GEN-038
#[test]
fn the_dense_index_of_an_interior_region_gives_the_region_of_the_hash_index() {
    use planet_generators::chunkfield::region_indexed;
    let lat = ChunkLattice::new(16).unwrap();
    // An interior chunk (dense), one at a face edge and one at a cube corner (both through the hash map whatever is asked).
    for tile in [chunk_tile(2, 1000, 1000), chunk_tile(2, 0, 5), chunk_tile(2, 2047, 2047)] {
        let (a, b) = (region_indexed(&lat, tile, 9, true), region_indexed(&lat, tile, 9, false));
        assert_eq!((a.cells.len(), a.core, a.halo), (b.cells.len(), b.core, b.halo), "{tile:?}");
        assert!(a.cells.iter().zip(&b.cells).all(|(x, y)| x.key() == y.key()), "the cells are the same, in the same order: {tile:?}");
        assert_eq!(a.depth, b.depth);
        assert_eq!(a.neighbours, b.neighbours);
        for &c in a.cells.iter().step_by(7) {
            assert_eq!(a.index_of(c), b.index_of(c));
        }
        let outside = planet_generators::chunkfield::Cell { face: 5, i: 3, j: 3 };
        assert_eq!(a.index_of(outside), b.index_of(outside));
    }
}

// spec: STRM-047
#[test]
fn the_level_change_is_the_chunk_change_and_none_without_chunk_detail_and_wrappers_keep_it() {
    let lat = ChunkLattice::new(608).unwrap();
    let stage = stand_in(8);
    let tile = chunk_tile(2, 1000, 1000);
    let result = Arc::new(compute_chunk(&lat, &stage, R, TOP11, tile, halo_rings(&stage)).unwrap());
    let none = ChunkField::new(Reference(3), lat, R, &stage).with_top_level(TOP11);
    let field = ChunkField::new(Reference(3), lat, R, &stage).with_top_level(TOP11).with_ready(result);
    let centre = tile.center_direction(&TangentWarp);
    let dirs: Vec<Vec3> =
        (0..9u32).map(|k| TileId::new(Face(2), 15, 16 * 1000 + k, 16 * 1000 + 8 - k).unwrap().center_direction(&TangentWarp)).collect();
    for level in [TOP11 - 1, TOP11, 13, 15, 18] {
        let got = field.level_change(&dirs, level).expect("a chunk is ready");
        assert_eq!(got.len(), dirs.len());
        for (k, &d) in dirs.iter().enumerate() {
            assert_eq!(got[k].to_bits(), field.change_at_level(d, level).to_bits(), "level {level} direction {k}");
            let by_difference = field.height_at_level(d, level) - Reference(3).height_at_level(d, level);
            assert!((got[k] - by_difference).abs() < 1e-6, "level {level} direction {k}: {} against {by_difference}", got[k]);
        }
    }
    assert!(field.level_change(&dirs, 16).unwrap().iter().any(|&c| c != 0.0), "the stand-in changes the ground");
    assert!(field.level_change(&dirs, TOP11 - 1).unwrap().iter().all(|&c| c == 0.0), "above the top level the base is drawn");
    // Without chunk detail: no change at all, for a chunk field with nothing ready and for a plain field.
    assert_eq!(none.level_change(&dirs, 16), None);
    assert_eq!(Reference(3).level_change(&dirs, 16), None);
    // The wrappers hand it on: a boxed field answers as the chunk field does (the other wrappers forward it the same way; the world's own chain is
    // checked where it is built).
    let want = field.level_change(&[centre], 16);
    let boxed: Box<dyn HeightField> = Box::new(
        ChunkField::new(Reference(3), lat, R, &stage)
            .with_top_level(TOP11)
            .with_ready(Arc::new(compute_chunk(&lat, &stage, R, TOP11, tile, halo_rings(&stage)).unwrap())),
    );
    assert_eq!(boxed.level_change(&[centre], 16), want);
}

// spec: STRM-047
#[test]
fn a_compact_tile_over_a_ready_chunk_carries_its_change_plane_and_one_away_from_chunks_does_not() {
    use planet_generators::compact::compact_tile_field;
    let lat = ChunkLattice::new(608).unwrap();
    let stage = stand_in(8);
    let tile = chunk_tile(2, 1000, 1000);
    let result = Arc::new(compute_chunk(&lat, &stage, R, TOP11, tile, halo_rings(&stage)).unwrap());
    let field = ChunkField::new(Reference(3), lat, R, &stage).with_top_level(TOP11).with_ready(result);
    let id = TileId::new(Face(2), 15, 16 * 1000 + 5, 16 * 1000 + 7).unwrap();
    assert_eq!(TileId::from_direction(&TangentWarp, id.center_direction(&TangentWarp), CHUNK_GRID_LEVEL), tile);
    let cells = 32u32;
    let with = compact_tile_field(&field, &TangentWarp, id, cells, R);
    let plane = with.change.as_ref().expect("the tile reads a change");
    assert_eq!(plane.len(), ((cells + 1) * (cells + 1)) as usize);
    for (k, l) in [(0u32, 0u32), (7, 3), (32, 32), (16, 16), (31, 1)] {
        let d = id.sample_direction(&TangentWarp, cells, k, l);
        let want = field.change_at_level(d, id.level()) as f32;
        assert_eq!(plane[(l * (cells + 1) + k) as usize].to_bits(), want.to_bits(), "texel ({k}, {l})");
    }
    assert!(plane.iter().any(|&c| c != 0.0));
    // The texels hold the final heights: the tile of the field without the chunk differs from this one by the plane (to the height step).
    let bare = compact_tile_field(&Reference(3), &TangentWarp, id, cells, R);
    assert_eq!(bare.change, None, "a field without chunk detail builds no plane");
    // A tile elsewhere reads no change, so it carries no plane even though the field has a chunk ready.
    let far = TileId::new(Face(4), 15, 16 * 300, 16 * 300).unwrap();
    assert_eq!(compact_tile_field(&field, &TangentWarp, far, cells, R).change, None);
    // The plane is part of the hash of the tile; a tile without one hashes as before.
    let mut without = with.clone();
    without.change = None;
    assert_ne!(with.content_hash(), without.content_hash());
    assert_eq!(bare.content_hash(), compact_tile_field(&Reference(3), &TangentWarp, id, cells, R).content_hash());
}

// spec: REND-039
#[test]
fn the_mirror_of_the_vertex_stage_draws_the_final_height_at_weight_one_the_base_at_zero_and_the_old_arithmetic_without_a_plane() {
    use planet_generators::compact::compact_tile_field;
    let lat = ChunkLattice::new(608).unwrap();
    let stage = stand_in(8);
    let tile = chunk_tile(2, 1000, 1000);
    let result = Arc::new(compute_chunk(&lat, &stage, R, TOP11, tile, halo_rings(&stage)).unwrap());
    let field = ChunkField::new(Reference(3), lat, R, &stage).with_top_level(TOP11).with_ready(result);
    let id = TileId::new(Face(2), 15, 16 * 1000 + 5, 16 * 1000 + 7).unwrap();
    let cells = 32u32;
    let with = compact_tile_field(&field, &TangentWarp, id, cells, R);
    let plane = with.change.clone().expect("the tile reads a change");
    let bare = compact_tile_field(&Reference(3), &TangentWarp, id, cells, R);
    assert_eq!(bare.change, None);
    for vi in 0..bare.shade_all().len() {
        // No plane: any fade is the old arithmetic, bit for bit.
        for fade in [0.0f32, 0.25, 1.0] {
            let (a, b) = (bare.shade_vertex(vi), bare.shade_vertex_faded(vi, fade));
            assert_eq!((a.height.to_bits(), a.pos.map(f32::to_bits)), (b.height.to_bits(), b.pos.map(f32::to_bits)), "vertex {vi}");
        }
        // Weight 1 is fade 0: the old arithmetic again, with a plane (the change times 0 adds 0).
        let (a, b) = (with.shade_vertex(vi), with.shade_vertex_faded(vi, 0.0));
        assert_eq!((a.height.to_bits(), a.pos.map(f32::to_bits)), (b.height.to_bits(), b.pos.map(f32::to_bits)), "vertex {vi}");
    }
    // Weight 0 (fade 1): the grid vertices sit at the base's height, final less the change; weight 0.5 halfway.
    let w = (cells + 1) as usize;
    let (mut worst0, mut worst_half) = (0.0f64, 0.0f64);
    for j in 0..w {
        for i in 0..w {
            let k = j * w + i;
            let (fin, base0, half) =
                (with.shade_vertex(k).height, with.shade_vertex_faded(k, 1.0).height, with.shade_vertex_faded(k, 0.5).height);
            // The mirror's own arithmetic, exactly: the origin height plus the texel's tile-relative height less fade times change, in f32.
            for (fade, got) in [(1.0f32, base0), (0.5, half), (0.0, fin)] {
                let want = with.h_origin + (with.rel_height(i, j) - fade * plane[k]);
                assert_eq!(got.to_bits(), want.to_bits(), "vertex ({i}, {j}) at fade {fade}");
            }
            worst0 = worst0.max((f64::from(fin) - f64::from(plane[k]) - f64::from(base0)).abs());
            worst_half = worst_half.max((f64::from(fin) - 0.5 * f64::from(plane[k]) - f64::from(half)).abs());
        }
    }
    eprintln!("faded heights against final - fade x change: weight 0 {worst0:.2e} m, weight 0.5 {worst_half:.2e} m");
    // The heights carry the origin height (f32, about 1e-4 m of spacing at a few thousand metres): the comparison is good to a few of its steps.
    assert!(worst0 < 5e-4 && worst_half < 5e-4, "{worst0}, {worst_half}");
    // And the drawn base is the base field's height at the tile's level (to the quantisation of the texels).
    let (i, j) = (7usize, 11usize);
    let d = id.sample_direction(&TangentWarp, cells, i as u32, j as u32);
    let drawn = f64::from(with.shade_vertex_faded(j * w + i, 1.0).height);
    let base = Reference(3).height_at_level(d, id.level());
    assert!((drawn - base).abs() < 2.0 * with.height_error_bound_m() + 5e-4, "{drawn} against the base {base}");
}
