//! GEN-001..006 and GEN-009 for the CPU reference height generator.

use planet_core::cube::TangentWarp;
use planet_core::{Face, Side, TileId};
use planet_generators::dump::{face_net, tile_height_map, FACE_COLORS, NET_BACKGROUND};
use planet_generators::{generate_region, generate_tile};

const SEED: u64 = 1;
const RES: u32 = 16;

fn id(face: u8, level: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(face), level, x, y).unwrap()
}

/// Committed known answers (generator version 1, hash version 1): identical on every platform or CI fails.
const KNOWN_TILE_HASHES: [((u8, u8, u32, u32), u64); 4] = [
    ((0, 0, 0, 0), 5559967772754406168),
    ((3, 3, 5, 2), 7312764954001723097),
    ((5, 12, 1000, 77), 16302889589339844302),
    ((2, 28, 123456, 654321), 324282905912445789),
];

// spec: GEN-001
#[test]
fn generation_is_repeatable_and_not_degenerate() {
    let t = id(3, 3, 5, 2);
    let a = generate_tile(SEED, &TangentWarp, t, RES);
    assert_eq!(a, generate_tile(SEED, &TangentWarp, t, RES));
    assert_ne!(a.content_hash(), generate_tile(SEED + 1, &TangentWarp, t, RES).content_hash(), "the seed must matter");
    let (min, max) = a.heights.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
    assert!(min.is_finite() && max.is_finite() && min >= -4000.0 && max <= 4000.0, "range {min}..{max}");
    assert!(max - min > 50.0, "tile is nearly flat: {min}..{max}");
    // A different-level tile gives different data.
    assert_ne!(a.heights, generate_tile(SEED, &TangentWarp, id(3, 3, 5, 3), RES).heights);
}

// spec: GEN-001, GEN-003
#[test]
fn tile_hashes_match_the_committed_known_answers() {
    for ((f, l, x, y), expected) in KNOWN_TILE_HASHES {
        let h = generate_tile(SEED, &TangentWarp, id(f, l, x, y), RES).content_hash();
        assert_eq!(h, expected, "tile face {f} level {l} ({x},{y}): hash changed; bump GENERATOR_VERSION if the change is intended");
    }
}

// spec: GEN-002
#[test]
fn region_output_does_not_depend_on_the_thread_count() {
    let ids: Vec<TileId> = (0..4).flat_map(|x| (0..4).map(move |y| id(1, 2, x, y))).chain([id(5, 4, 3, 9), id(0, 6, 40, 41)]).collect();
    let one = generate_region(SEED, &TangentWarp, &ids, 8, 1);
    for threads in [2, 3, 8, 64] {
        let many = generate_region(SEED, &TangentWarp, &ids, 8, threads);
        assert_eq!(one.len(), many.len());
        for (a, b) in one.iter().zip(&many) {
            assert_eq!(a.id, b.id, "order must follow the input with {threads} threads");
            assert_eq!(a.content_hash(), b.content_hash(), "tile {:?} differs with {threads} threads", a.id);
        }
    }
    assert_eq!(one.iter().map(|t| t.id).collect::<Vec<_>>(), ids);
}

fn border(t: &planet_generators::TileData, side: Side) -> Vec<f32> {
    let r = t.res;
    (0..=r)
        .map(|i| match side {
            Side::East => t.at(r, i),
            Side::West => t.at(0, i),
            Side::North => t.at(i, r),
            Side::South => t.at(i, 0),
        })
        .collect()
}

// spec: GEN-004
#[test]
fn neighbouring_tiles_share_border_samples_including_across_faces() {
    let m = TangentWarp;
    let mut across_faces = 0;
    for level in [1u8, 2, 3] {
        let n = 1u32 << level;
        for f in 0..6u8 {
            for x in 0..n {
                for y in 0..n {
                    let t = id(f, level, x, y);
                    let a = generate_tile(SEED, &m, t, 8);
                    for side in Side::ALL {
                        let nb = t.neighbor(&m, side);
                        let b = generate_tile(SEED, &m, nb, 8);
                        // Every border sample of `a` appears bit for bit among the border samples of `b`, in order or reversed.
                        let mine = border(&a, side);
                        let theirs: Vec<Vec<f32>> = Side::ALL.iter().map(|&s| border(&b, s)).collect();
                        let rev = |v: &Vec<f32>| v.iter().rev().copied().collect::<Vec<f32>>();
                        let ok = theirs.iter().any(|v| *v == mine || rev(v) == mine);
                        assert!(ok, "{t:?} {side:?} border differs from every border of {nb:?}: {mine:?}");
                        across_faces += usize::from(nb.face() != t.face());
                    }
                }
            }
        }
    }
    assert!(across_faces > 100, "the test must cross face edges, crossed {across_faces}");
}

// spec: GEN-005
#[test]
fn one_large_tile_equals_its_four_children() {
    let m = TangentWarp;
    for t in [id(0, 0, 0, 0), id(4, 3, 2, 5), id(3, 10, 700, 31)] {
        let big = generate_tile(SEED, &m, t, 16);
        for (i, c) in t.children().unwrap().into_iter().enumerate() {
            let child = generate_tile(SEED, &m, c, 8);
            let (ox, oy) = ((c.x() - 2 * t.x()) * 8, (c.y() - 2 * t.y()) * 8);
            for l in 0..=8 {
                for k in 0..=8 {
                    assert_eq!(
                        child.at(k, l).to_bits(),
                        big.at(ox + k, oy + l).to_bits(),
                        "tile {t:?} child {i} sample ({k},{l}) differs from the large tile"
                    );
                }
            }
        }
    }
}

// spec: GEN-006
#[test]
fn coarse_tile_agrees_with_children_at_coarse_positions() {
    let m = TangentWarp;
    let tolerance = 0.0f32; // heights come from one point function, so agreement is exact
    let t = id(2, 5, 9, 20);
    let parent = generate_tile(SEED, &m, t, 4);
    let mut worst = 0.0f32;
    for c in t.children().unwrap() {
        let child = generate_tile(SEED, &m, c, 4);
        let (ox, oy) = ((c.x() - 2 * t.x()) * 2, (c.y() - 2 * t.y()) * 2);
        // The child's 5x5 lattice covers parent samples ox..ox+2 at every second child sample.
        for l in 0..=2 {
            for k in 0..=2 {
                worst = worst.max((parent.at(ox + k, oy + l) - child.at(2 * k, 2 * l)).abs());
            }
        }
    }
    assert!(worst <= tolerance, "coarse/fine disagreement {worst} m exceeds {tolerance} m");
}

// spec: GEN-009
#[test]
fn face_net_has_six_distinct_faces_and_no_background_inside_the_net() {
    let cell = 32;
    let img = face_net(&TangentWarp, cell, 2);
    assert_eq!((img.width, img.height), (cell * 4, cell * 3));
    let mut background = 0;
    let mut seen = std::collections::BTreeSet::new();
    for y in 0..img.height {
        for x in 0..img.width {
            let p = img.pixel(x, y);
            if p == NET_BACKGROUND {
                background += 1;
            } else {
                // Face colour or its darkened grid-line variant.
                let face = FACE_COLORS.iter().position(|c| *c == p || [c[0] / 2, c[1] / 2, c[2] / 2, 255] == p);
                assert!(face.is_some(), "unexpected colour {p:?} at ({x},{y})");
                seen.insert(face.unwrap());
            }
        }
    }
    assert_eq!(seen.len(), 6, "all six faces must appear");
    assert_eq!(background, (12 - 6) * cell * cell, "background must fill exactly the six empty cross cells");
    // Each face cell is dominated by its own colour: the face found from each pixel's direction is the cell's face.
    for (f, &(cx, cy)) in planet_generators::dump::NET_CELLS.iter().enumerate() {
        let p = img.pixel(cx * cell + cell / 2 + 3, cy * cell + cell / 2 + 3);
        assert_eq!(p, FACE_COLORS[f], "centre of net cell {f}");
    }
}

// spec: GEN-009
#[test]
fn height_map_dump_has_one_pixel_per_sample_and_uses_the_range() {
    let t = generate_tile(SEED, &TangentWarp, id(3, 3, 5, 2), RES);
    let img = tile_height_map(&t);
    assert_eq!((img.width, img.height), (RES + 1, RES + 1));
    let greys: std::collections::BTreeSet<u8> =
        (0..img.height).flat_map(|y| (0..img.width).map(move |x| (x, y))).map(|(x, y)| img.pixel(x, y)[0]).collect();
    assert!(greys.len() > 20, "height map should use many grey levels, got {}", greys.len());
    let path = std::env::temp_dir().join(format!("planet-height-{}.png", std::process::id()));
    img.write_png(&path).unwrap();
    assert!(path.metadata().unwrap().len() > 50);
}
