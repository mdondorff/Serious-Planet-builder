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

/// Committed known answers (generator version 2, hash version 1): identical on every platform or CI fails.
const KNOWN_TILE_HASHES: [((u8, u8, u32, u32), u64); 4] = [
    ((0, 0, 0, 0), 8318148303301198649),
    ((3, 3, 5, 2), 12304968376432004800),
    ((5, 12, 1000, 77), 5732832144616342928),
    ((2, 28, 123456, 654321), 15460510008799771898),
];

// spec: GEN-001
#[test]
fn generation_is_repeatable_and_not_degenerate() {
    let t = id(3, 3, 5, 2);
    let a = generate_tile(SEED, &TangentWarp, t, RES);
    assert_eq!(a, generate_tile(SEED, &TangentWarp, t, RES));
    assert_ne!(a.content_hash(), generate_tile(SEED + 1, &TangentWarp, t, RES).content_hash(), "the seed must matter");
    assert!(a.heights.iter().all(|h| h.is_finite()), "NaN or infinite height");
    let (min, max) = a.heights.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
    assert!(min.is_finite() && max.is_finite() && min >= -4000.0 && max <= 4000.0, "range {min}..{max}");
    assert!(max - min > 50.0, "tile is nearly flat: {min}..{max}");
    // A neighbouring tile gives different data.
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
                        let bits = |v: &Vec<f32>| v.iter().map(|h| h.to_bits()).collect::<Vec<u32>>();
                        let ok = theirs.iter().any(|v| bits(v) == bits(&mine) || bits(&rev(v)) == bits(&mine));
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
    for t in [id(0, 0, 0, 0), id(4, 3, 2, 5), id(3, 10, 700, 31), id(1, 27, (1 << 27) - 1, 5)] {
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
    // Honest note: the generator is a point function, so agreement is exact by construction; this test pins
    // that property and will start to mean more once a filtered coarse generator exists.
    for c in t.children().unwrap() {
        let child = generate_tile(SEED, &m, c, 4);
        let (ox, oy) = ((c.x() - 2 * t.x()) * 2, (c.y() - 2 * t.y()) * 2);
        // The child's 5x5 lattice covers parent samples ox..ox+2 at every second child sample.
        for l in 0..=2 {
            for k in 0..=2 {
                let (p, c) = (parent.at(ox + k, oy + l), child.at(2 * k, 2 * l));
                assert!(p.is_finite() && c.is_finite());
                worst = worst.max((p - c).abs());
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

// spec: GEN-004
#[test]
fn deep_level_cross_face_borders_agree_bit_for_bit() {
    let m = TangentWarp;
    let n = 1u32 << 28;
    for (face, x, y, side) in
        [(1u8, n - 1, 12345u32, Side::East), (4, 777, n - 1, Side::North), (2, 0, 99, Side::West), (5, n - 1, 0, Side::South)]
    {
        let t = id(face, 28, x, y);
        let nb = t.neighbor(&m, side);
        assert_ne!(nb.face(), t.face(), "test case must cross a face edge");
        let (a, b) = (generate_tile(SEED, &m, t, 16), generate_tile(SEED, &m, nb, 16));
        let mine: Vec<u32> = border(&a, side).iter().map(|h| h.to_bits()).collect();
        let found = Side::ALL.iter().any(|&s| {
            let v: Vec<u32> = border(&b, s).iter().map(|h| h.to_bits()).collect();
            v == mine || v.iter().rev().copied().collect::<Vec<_>>() == mine
        });
        assert!(found, "{t:?} {side:?} border has no bit-identical partner in {nb:?}");
    }
}

// spec: GEN-001
#[test]
fn seeds_and_cache_keys_do_not_alias() {
    use planet_core::cube::FaceMapping;
    let m = TangentWarp;
    // Seeds that differ only in the old octave-shift bit pattern must give unrelated fields.
    let t = id(0, 2, 1, 1);
    let a = generate_tile(1, &m, t, 8);
    let b = generate_tile(1 ^ (1 << 56), &m, t, 8);
    let n = a.heights.len() as f64;
    let (ma, mb) = (a.heights.iter().map(|&h| f64::from(h)).sum::<f64>() / n, b.heights.iter().map(|&h| f64::from(h)).sum::<f64>() / n);
    let (mut cov, mut va, mut vb) = (0.0, 0.0, 0.0);
    for (x, y) in a.heights.iter().zip(&b.heights) {
        let (dx, dy) = (f64::from(*x) - ma, f64::from(*y) - mb);
        cov += dx * dy;
        va += dx * dx;
        vb += dy * dy;
    }
    assert!((cov / (va * vb).sqrt()).abs() < 0.5, "fields of aliasing seeds are correlated");
    // Every ingredient changes the key.
    struct OtherWarp;
    impl FaceMapping for OtherWarp {
        fn id(&self) -> &'static str {
            "other"
        }
        fn face_to_plane(&self, s: f64) -> f64 {
            s
        }
        fn plane_to_face(&self, u: f64) -> f64 {
            u
        }
    }
    let base = planet_generators::cache_key(&m, 1, t, 8);
    assert_ne!(base, planet_generators::cache_key(&m, 2, t, 8));
    assert_ne!(base, planet_generators::cache_key(&m, 1, id(0, 2, 1, 2), 8));
    assert_ne!(base, planet_generators::cache_key(&m, 1, t, 16));
    assert_ne!(base, planet_generators::cache_key(&OtherWarp, 1, t, 8));
}

// spec: GEN-009
#[test]
fn net_cells_are_continuous_across_every_shared_edge() {
    use planet_core::cube::{face_to_direction, FaceMapping};
    let m = TangentWarp;
    let cell = 64u32;
    let cells = planet_generators::dump::NET_CELLS;
    let px = 2.0 / f64::from(cell);
    // Direction at a face-coordinate point of the face drawn in net cell `c`.
    let dir = |f: usize, s: f64, t: f64| face_to_direction(&m, Face(f as u8), s, t);
    let mut pairs = 0;
    for (fa, &(ax, ay)) in cells.iter().enumerate() {
        for (fb, &(bx, by)) in cells.iter().enumerate() {
            // b is the right-hand neighbour of a, or the lower neighbour in the image (t grows upwards in a cell).
            if (bx, by) == (ax + 1, ay) {
                for i in 0..8 {
                    let t = -1.0 + 2.0 * (f64::from(i) + 0.5) / 8.0;
                    let (d1, d2) = (dir(fa, 1.0 - px / 2.0, t), dir(fb, -1.0 + px / 2.0, t));
                    assert!(d1.distance(d2) < 3.0 * px, "faces {fa}|{fb} (east edge) are not continuous at t={t}");
                }
                pairs += 1;
            }
            if (bx, by) == (ax, ay + 1) {
                for i in 0..8 {
                    let s = -1.0 + 2.0 * (f64::from(i) + 0.5) / 8.0;
                    let (d1, d2) = (dir(fa, s, -1.0 + px / 2.0), dir(fb, s, 1.0 - px / 2.0));
                    assert!(d1.distance(d2) < 3.0 * px, "faces {fa}/{fb} (vertical edge) are not continuous at s={s}");
                }
                pairs += 1;
            }
        }
    }
    assert_eq!(pairs, 5, "the cross has five shared edges");
    let _ = m.id();
}

// spec: GEN-009
#[test]
fn net_draws_tile_grid_lines() {
    let cell = 32;
    let img = face_net(&TangentWarp, cell, 2);
    for (f, &(cx, cy)) in planet_generators::dump::NET_CELLS.iter().enumerate() {
        let on_line = img.pixel(cx * cell, cy * cell + 11);
        let off_line = img.pixel(cx * cell + 3, cy * cell + 3);
        let base = FACE_COLORS[f];
        assert_eq!(on_line, [base[0] / 2, base[1] / 2, base[2] / 2, 255], "grid line at the left edge of face {f}");
        assert_eq!(off_line, base, "interior pixel of face {f}");
    }
}
