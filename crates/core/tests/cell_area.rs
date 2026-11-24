//! TEST-152: the cube cell's solid angle and area (COORD-013, lead #197) against references independent of the form under test: for n <= 512 the
//! exact four-term `atan` solid angle of the gnomonic patch (rounding at most about 2e-16 n^2, so 5e-11 at 512), for n > 512 an 8 x 8 Gauss-Legendre
//! rule on the whole cell (no subcells; its truncation is negligible there, not at n of 1 to 4).
use planet_core::cell_area::{cell_solid_angle, pairwise_sum, SUBCELL_LEVEL};
use planet_core::{Face, GlobalCell};
use std::f64::consts::{FRAC_PI_4, PI};

const BOUND: f64 = 1e-9;

/// The angle of face coordinate index `t` (cell edges at integers): pi/4 (2 t / n - 1).
fn angle(n: i64, t: f64) -> f64 {
    FRAC_PI_4 * (2.0 * t / n as f64 - 1.0)
}

/// Reference for n <= 512: the exact solid angle of the gnomonic patch, `G(x1, y1) - G(x0, y1) - G(x1, y0) + G(x0, y0)`, `G = atan(x y / sqrt(1 + x^2 + y^2))`.
fn exact4(n: i64, i: i64, j: i64) -> f64 {
    let x = |t: i64| libm::tan(angle(n, t as f64));
    let g = |x: f64, y: f64| libm::atan(x * y / libm::sqrt(1.0 + x * x + y * y));
    let (x0, x1, y0, y1) = (x(i), x(i + 1), x(j), x(j + 1));
    g(x1, y1) - g(x0, y1) - g(x1, y0) + g(x0, y0)
}

/// Reference for n > 512: 8 x 8 Gauss-Legendre in the angles over the whole cell.
fn gl8(n: i64, i: i64, j: i64) -> f64 {
    const X: [f64; 4] = [0.183_434_642_495_649_8, 0.525_532_409_916_329, 0.796_666_477_413_626_7, 0.960_289_856_497_536_3];
    const W: [f64; 4] = [0.362_683_783_378_362, 0.313_706_645_877_887_3, 0.222_381_034_453_374_5, 0.101_228_536_290_376_3];
    let nodes: Vec<(f64, f64)> = (0..4).flat_map(|k| [(-X[k], W[k]), (X[k], W[k])]).collect();
    let half = FRAC_PI_4 / n as f64;
    let f = |a: f64, b: f64| {
        let (x, y) = (libm::tan(a), libm::tan(b));
        let s = 1.0 + x * x + y * y;
        (1.0 + x * x) * (1.0 + y * y) / (s * libm::sqrt(s))
    };
    let (ca, cb) = (angle(n, i as f64 + 0.5), angle(n, j as f64 + 0.5));
    let mut sum = 0.0;
    for &(xb, wb) in &nodes {
        for &(xa, wa) in &nodes {
            sum += wa * wb * f(ca + half * xa, cb + half * xb);
        }
    }
    half * half * sum
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// The fixed reduction of COORD-013: pairwise per face row, then over the rows.
fn face_sum(n: i64) -> f64 {
    let rows: Vec<f64> = (0..n).map(|j| pairwise_sum(&(0..n).map(|i| cell_solid_angle(n, i, j)).collect::<Vec<_>>())).collect();
    pairwise_sum(&rows)
}

// spec: COORD-013, TEST-152
#[test]
fn small_levels_match_the_exact_four_term_solid_angle() {
    // reference: exact4 (n <= 512); every canonical cell of n 1 to 64, and of 128, 256 and 512
    let mut worst = 0.0f64;
    for n in (1i64..=64).chain([128, 256, 512]) {
        for j in 0..(n + 1) / 2 {
            for i in 0..=j {
                let (got, want) = (cell_solid_angle(n, i, j), exact4(n, i, j));
                let r = rel(got, want);
                worst = worst.max(r);
                assert!(r <= BOUND, "n {n} cell ({i}, {j}): {got} against {want}, relative {r:e}");
            }
        }
    }
    assert!(worst > 0.0 && worst < 1e-10, "worst relative error {worst:e}");
}

// spec: COORD-013, TEST-152
#[test]
fn large_levels_match_an_eight_point_quadrature() {
    // reference: gl8 (n > 512); corner, edge, centre and scattered cells, up to 2^30 cells per face edge
    for n in [513i64, 1000, 1024, 4096, 98_304, 1 << 20, 1 << 30] {
        let mut cells = vec![(0, 0), (0, n - 1), (n - 1, n - 1), (0, n / 2), (n / 2, n / 2), (n / 3, n / 7), (n - 2, 1)];
        cells.extend((0..20i64).map(|k| ((k * 7919 + 13) % n, (k * 104_729 + 7) % n)));
        for (i, j) in cells {
            let (got, want) = (cell_solid_angle(n, i, j), gl8(n, i, j));
            assert!(rel(got, want) <= BOUND, "n {n} cell ({i}, {j}): {got} against {want}, relative {:e}", rel(got, want));
        }
    }
    // the rule needs subcells below SUBCELL_LEVEL and none from it on
    assert_eq!(SUBCELL_LEVEL, 512);
}

// spec: COORD-013, TEST-152
#[test]
fn face_and_cube_sums_close_to_the_sphere() {
    // the fixed reduction: pairwise per face row, then rows, then faces (each face has the same bits: the area reads only (i, j, n))
    for n in [1i64, 2, 3, 4, 5, 7, 8, 16, 64, 512, 1024] {
        let face = face_sum(n);
        assert!(rel(face, 4.0 * PI / 6.0) <= BOUND, "n {n}: face {face}, relative {:e}", rel(face, 4.0 * PI / 6.0));
        if [1, 7, 1024].contains(&n) {
            let cube = pairwise_sum(&[face; 6]);
            assert!(rel(cube, 4.0 * PI) <= BOUND, "n {n}: cube {cube}");
        }
    }
    // one cell a face is a sixth of the sphere
    assert!(rel(cell_solid_angle(1, 0, 0), 4.0 * PI / 6.0) <= BOUND);
}

// spec: COORD-013, TEST-152
#[test]
fn every_rotation_of_the_cube_keeps_a_cells_bits() {
    let rots = rotations();
    assert_eq!(rots.len(), 24);
    let check = |n: i64, c: GlobalCell| {
        for m in &rots {
            let r = GlobalCell::of_centre_point(rotate(m, c.centre_point(n)), n).unwrap();
            assert_eq!(r.solid_angle(n).to_bits(), c.solid_angle(n).to_bits(), "n {n}: {c:?} and its image {r:?}");
        }
    };
    for n in [5i64, 6] {
        for f in Face::ALL {
            for j in 0..n {
                for i in 0..n {
                    check(n, GlobalCell::new(f, n, i, j).unwrap());
                }
            }
        }
    }
    for k in 0..200i64 {
        let n = 1024;
        check(n, GlobalCell::new(Face((k % 6) as u8), n, (k * 7919) % n, (k * 3571 + 11) % n).unwrap());
    }
}

// spec: COORD-013, TEST-152
#[test]
fn a_sum_is_the_same_bits_on_one_and_seven_threads() {
    // CON-14: the rows of face 0 at n 1024 on 1 and 7 threads, each row pairwise, then the rows pairwise in order
    let n = 1024i64;
    let rows_on = |threads: usize| -> Vec<f64> {
        let mut rows = vec![0.0f64; n as usize];
        let chunk = (n as usize).div_ceil(threads);
        std::thread::scope(|s| {
            for (t, part) in rows.chunks_mut(chunk).enumerate() {
                s.spawn(move || {
                    for (k, r) in part.iter_mut().enumerate() {
                        let j = (t * chunk + k) as i64;
                        *r = pairwise_sum(&(0..n).map(|i| cell_solid_angle(n, i, j)).collect::<Vec<_>>());
                    }
                });
            }
        });
        rows
    };
    let (one, seven) = (rows_on(1), rows_on(7));
    assert!(one.iter().zip(&seven).all(|(a, b)| a.to_bits() == b.to_bits()));
    assert_eq!(pairwise_sum(&one).to_bits(), pairwise_sum(&seven).to_bits());
    assert_eq!(pairwise_sum(&one).to_bits(), face_sum(n).to_bits());
}

// spec: COORD-013, TEST-152
#[test]
fn the_area_is_the_radius_squared_times_the_solid_angle_and_a_cell_outside_its_level_is_refused() {
    let c = GlobalCell::new(Face(3), 98_304, 12_345, 67_890).unwrap();
    let r = 6_371_000.0f64;
    assert_eq!(c.area(98_304, r).to_bits(), (r * r * c.solid_angle(98_304)).to_bits());
    let big = GlobalCell::new(Face(0), 8, 7, 0).unwrap();
    assert!(std::panic::catch_unwind(|| big.solid_angle(4)).is_err(), "a cell of another level");
    assert!(std::panic::catch_unwind(|| cell_solid_angle(4, 4, 0)).is_err());
    // numerics review: a level over 2^30 cells an edge is refused, not wrapped
    assert!(std::panic::catch_unwind(|| cell_solid_angle((1 << 30) + 1, 0, 0)).is_err());
    assert!(std::panic::catch_unwind(|| cell_solid_angle(i64::MAX, 0, 0)).is_err());
}

/// The 24 rotations of the cube: the signed permutation matrices of determinant +1.
fn rotations() -> Vec<[[i64; 3]; 3]> {
    let perms = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    let mut out = Vec::new();
    for p in perms {
        for s in 0..8 {
            let mut m = [[0i64; 3]; 3];
            for (r, &c) in p.iter().enumerate() {
                m[r][c] = if s >> r & 1 == 1 { -1 } else { 1 };
            }
            let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
                + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
            if det == 1 {
                out.push(m);
            }
        }
    }
    out
}

fn rotate(m: &[[i64; 3]; 3], p: [i64; 3]) -> [i64; 3] {
    [0, 1, 2].map(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2])
}
