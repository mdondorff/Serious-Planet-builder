//! GEN-015: reading the global-coarse bake at tile sample positions (ADR 0027).

use std::collections::HashMap;

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::global::{
    bake, read_lattice, read_tile_sample, sample, GlobalGrid, GlobalSample, GridSource, LatticeSource, ReadError, READER_VERSION,
};

const SEED: u64 = 7;

fn grid(res: u32) -> GlobalGrid {
    bake(SEED, &TangentWarp, res, 4).unwrap()
}

fn bits(s: &GlobalSample) -> [u32; 4] {
    [s.continent.to_bits(), s.elevation_m.to_bits(), s.temperature_c.to_bits(), s.precipitation_mm.to_bits()]
}

/// Bits with a zero normalised to +0 (the reader's blend turns -0.0 into +0.0).
fn norm(s: &GlobalSample) -> [u32; 4] {
    let z = |v: f32| if v == 0.0 { 0.0f32.to_bits() } else { v.to_bits() };
    [z(s.continent), z(s.elevation_m), z(s.temperature_c), z(s.precipitation_mm)]
}

// spec: GEN-015
#[test]
fn a_lattice_point_reads_as_its_stored_sample_at_every_target_resolution() {
    let g = grid(16);
    let src = GridSource(&g);
    for f in 0..6u8 {
        for (i, j) in [(0u32, 0u32), (3, 7), (16, 16), (16, 0), (9, 16)] {
            let want = g.at(Face(f), i, j);
            // The same position on finer lattices (2^4 .. 2^8 cells) and on a coarser one (2^2 cells) when it lies on it.
            for n_log2 in 4..=8u32 {
                let k = 1i64 << (n_log2 - 4);
                let got = read_lattice(&src, Face(f), i64::from(i) * k, i64::from(j) * k, n_log2).unwrap();
                assert_eq!(norm(&got), norm(&want), "face {f} vertex ({i}, {j}) at 2^{n_log2}");
            }
        }
    }
    let coarse = read_lattice(&src, Face(2), 2, 1, 2).unwrap();
    assert_eq!(norm(&coarse), norm(&g.at(Face(2), 8, 4)), "a coarser target lattice reads a lattice point");
}

// spec: GEN-015
#[test]
fn a_position_on_a_face_edge_reads_equal_bits_from_every_face_that_has_it() {
    // Target lattice 4 times finer than the bake: most edge positions are between bake samples (non-zero weights), and the
    // shared edge runs in opposite directions on the two faces (ADR 0027 point 3).
    let g = grid(16);
    let src = GridSource(&g);
    let n_log2 = 6u32;
    let n = 1i64 << n_log2;
    let mut by_direction: HashMap<[u64; 3], [u32; 4]> = HashMap::new();
    let (mut shared, mut off_lattice) = (0, 0);
    for f in 0..6u8 {
        let root = TileId::new(Face(f), 0, 0, 0).unwrap();
        for a in 0..=n {
            for edge in 0..4 {
                let (u, v) = match edge {
                    0 => (a, 0),
                    1 => (a, n),
                    2 => (0, a),
                    _ => (n, a),
                };
                let d = root.sample_direction(&TangentWarp, 1 << n_log2, u as u32, v as u32);
                let got = norm(&read_lattice(&src, Face(f), u, v, n_log2).unwrap());
                if let Some(prev) = by_direction.insert([d.x.to_bits(), d.y.to_bits(), d.z.to_bits()], got) {
                    assert_eq!(prev, got, "face {f} position ({u}, {v}) disagrees with the face that shares it");
                    shared += 1;
                    off_lattice += usize::from((a % 4) != 0);
                }
            }
        }
    }
    assert!(shared > 12 * 60, "{shared} shared positions");
    assert!(off_lattice > 12 * 40, "{off_lattice} of them between bake samples");
}

// spec: GEN-015
#[test]
fn the_same_position_reads_the_same_bits_through_any_tile_decomposition() {
    // Partition invariance (GEN-005 for the reader): sample (k, l) of tile (x, y) at level L with m cells is the face position
    // (x m + k, y m + l) of a lattice of m 2^L cells, so any decomposition of that lattice reads the same bits.
    let g = grid(16);
    let src = GridSource(&g);
    for f in [0u8, 3, 5] {
        for (lvl_a, m_a, lvl_b, m_b) in [(1u8, 16u32, 3u8, 4u32), (2, 8, 0, 32), (3, 8, 1, 32)] {
            let n = u32::from(lvl_a) + m_a.trailing_zeros();
            assert_eq!(n, u32::from(lvl_b) + m_b.trailing_zeros());
            let t_a = TileId::new(Face(f), lvl_a, 1, 1).unwrap();
            for (k, l) in [(0i64, 0i64), (3, 5), (m_a as i64, m_a as i64), (1, m_a as i64)] {
                let (u, v) = (i64::from(t_a.x()) * i64::from(m_a) + k, i64::from(t_a.y()) * i64::from(m_a) + l);
                let a = read_tile_sample(&src, t_a, m_a, k, l).unwrap();
                // The tile of the other decomposition that holds that position (its own k, l inside 0..=m_b).
                let (tx, ty) = ((u / i64::from(m_b)).min((1 << lvl_b) - 1), (v / i64::from(m_b)).min((1 << lvl_b) - 1));
                let t_b = TileId::new(Face(f), lvl_b, tx as u32, ty as u32).unwrap();
                let b = read_tile_sample(&src, t_b, m_b, u - tx * i64::from(m_b), v - ty * i64::from(m_b)).unwrap();
                assert_eq!(bits(&a), bits(&b), "face {f}: ({lvl_a}, {m_a}) vs ({lvl_b}, {m_b}) at ({u}, {v})");
            }
        }
    }
}

// spec: GEN-015
#[test]
fn interpolation_is_the_edge_symmetric_blend_and_tracks_the_field() {
    let g = grid(16);
    let src = GridSource(&g);
    // A half-way position along s is the mean of the two samples, computed as a (1 - w) + b w with w = 0.5.
    let (a, b) = (g.at(Face(1), 5, 6), g.at(Face(1), 6, 6));
    let mid = read_lattice(&src, Face(1), 11, 12, 5).unwrap();
    assert_eq!(mid.elevation_m, (f64::from(a.elevation_m) * 0.5 + f64::from(b.elevation_m) * 0.5) as f32);
    // Between lattice points the interpolation approaches the field as the bake gets finer: the mean absolute elevation error
    // against the field itself at a 4x finer lattice falls by more than a third from a 16-cell to a 64-cell bake.
    let mean_error = |res: u32| {
        let g = grid(res);
        let src = GridSource(&g);
        let n_log2 = res.trailing_zeros() + 2;
        let n = 1i64 << n_log2;
        let t = TileId::new(Face(4), 0, 0, 0).unwrap();
        let mut sum = 0.0f64;
        for l in 0..=n {
            for k in 0..=n {
                let want = sample(SEED, t.sample_direction(&TangentWarp, 1 << n_log2, k as u32, l as u32));
                let got = read_lattice(&src, Face(4), k, l, n_log2).unwrap();
                sum += (f64::from(got.elevation_m) - f64::from(want.elevation_m)).abs();
            }
        }
        sum / ((n + 1) * (n + 1)) as f64
    };
    let (coarse, fine) = (mean_error(16), mean_error(64));
    assert!(fine < coarse * 0.67, "mean error {coarse} m at 16 cells, {fine} m at 64 cells");
    assert!(fine < 400.0, "{fine} m");
}

// spec: GEN-015
#[test]
fn a_halo_beyond_a_face_edge_holds_the_edge_value_and_bad_shapes_and_sources_are_errors() {
    let g = grid(8);
    let src = GridSource(&g);
    let t = TileId::new(Face(0), 0, 0, 0).unwrap();
    // One sample beyond the face edge reads the edge sample (ADR 0027 point 4), on both axes and at the corner.
    let edge = read_tile_sample(&src, t, 8, 0, 3).unwrap();
    assert_eq!(bits(&read_tile_sample(&src, t, 8, -1, 3).unwrap()), bits(&edge));
    let far = read_tile_sample(&src, t, 8, 8, 8).unwrap();
    assert_eq!(bits(&read_tile_sample(&src, t, 8, 9, 9).unwrap()), bits(&far));
    // A halo inside the face (between tiles of one face) is a real neighbour sample.
    let inner = TileId::new(Face(0), 1, 0, 0).unwrap();
    let across = read_tile_sample(&src, inner, 4, 5, 2).unwrap();
    let next = read_tile_sample(&src, TileId::new(Face(0), 1, 1, 0).unwrap(), 4, 1, 2).unwrap();
    assert_eq!(bits(&across), bits(&next));
    // Errors name what is wrong.
    assert!(matches!(read_tile_sample(&src, t, 6, 0, 0), Err(ReadError::Shape(_))));
    assert!(matches!(read_tile_sample(&src, t, 8, -2, 0), Err(ReadError::Shape(_))));
    assert!(matches!(read_lattice(&src, Face(0), 0, 0, 60), Err(ReadError::Shape(_))));
    struct Broken;
    impl LatticeSource for Broken {
        fn res(&self) -> u32 {
            4
        }
        fn at(&self, _: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
            Err(ReadError::Source(format!("block of vertex ({i}, {j}) is missing")))
        }
    }
    let e = read_lattice(&Broken, Face(1), 1, 1, 2).unwrap_err();
    assert!(e.to_string().contains("is missing"), "{e}");
}

// spec: GEN-015
#[test]
fn the_reader_has_pinned_known_answers() {
    assert_eq!(READER_VERSION, 1, "the known answers belong to this reader version");
    let g = grid(16);
    let src = GridSource(&g);
    let picks = [(Face(0), 7i64, 9i64, 6u32), (Face(3), 33, 1, 6), (Face(5), 64, 64, 6), (Face(2), 5, 60, 6), (Face(1), 3, 4, 4)];
    let got: Vec<[u32; 4]> = picks.iter().map(|(f, u, v, n)| bits(&read_lattice(&src, *f, *u, *v, *n).unwrap())).collect();
    assert_eq!(format!("{got:08x?}"), KNOWN_READS, "pinned reads");
}

const KNOWN_READS: &str = "[[3f1868e5, 440fb307, 414e4bbd, 437c2590], [3e4dd9ee, 42581a35, be1290e0, 449703ce], [3f10cf0d, 440385a8, 410d220c, 44156778], [3f06602a, 45a8088c, c1df24f5, 444c3e9e], [3c3bf224, c2f41cde, 418e9f24, 42eec8b4]]";
